//! airaccount-admin —— 社区节点更新 Web 管理台(Phase 2 增量 1)。
//!
//! 设计:kms/docs/auto-update-web-admin-design.md。这是全系统**权限最高的攻击面**(能触发换 CA),
//! 故安全地基先行,逐条对齐设计 §6 + Codex/pr-daemon 评审:
//!   - 只绑 127.0.0.1(默认)/ Tailscale;启动自检拒绝公网/被代理暴露(§6.1,H2)
//!   - argon2id 密码 + 会话 cookie(HttpOnly/SameSite=Strict/短 TTL)(§6.2)
//!   - 所有写操作:会话 + CSRF token + 精确 Origin/Host(防 DNS rebinding,§6.3,H3)
//!   - 严格 CSP + X-Frame-Options: DENY(防点击劫持/XSS)
//!   - 非 root 运行,经 airaccount-admin-helper(固定 argv 白名单 + 清 env)调 updater(C1)
//!   - apply/rollback 强制二因子:Telegram 发一次性码,用户回填(决策 C;绑 version+nonce+短 TTL)
//!   - TA 在线一键**不提供**(决策 D;helper/updater 层也拒)
//!
//! 增量 1 范围:安全地基 + 会话 + /status /candidates(读)+ 五屏页面 + apply/rollback(经 helper
//! + Telegram 二次确认)。SSE 实时进度、审计链式 hash 的完整持久化留增量 2。

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use warp::http::{HeaderValue, StatusCode};
use warp::{Filter, Rejection, Reply};

mod security;
mod helper;
use security::{ct_eq, gen_token};

// ── 配置(env 覆盖,默认生产路径)─────────────────────────────────────
struct Config {
    bind: SocketAddr,
    admin_hash_file: String, // /etc/airaccount/admin.hash(argon2id PHC 串)
    helper: String,          // 特权 helper 绝对路径(经 sudo 调)
    node_id: String,
    session_ttl: Duration,
    twofa_ttl: Duration,
    allow_tailscale: bool,
    allowed_hosts: Vec<String>, // 精确 Host/Origin authority 白名单(小写)
    max_body: u64,              // 请求体上限(字节),防内存 DoS
    twofa_max_attempts: u32,    // 每个 2FA 挑战允许的错误次数
    #[cfg(test)]
    telegram_override: Option<bool>, // 仅测试:短路 Telegram 投递结果,避免真打 API / 全局 env 竞态
}

impl Config {
    fn from_env() -> Config {
        let host = std::env::var("ADMIN_BIND_HOST").unwrap_or_else(|_| "127.0.0.1".into());
        let port: u16 = std::env::var("ADMIN_BIND_PORT").ok().and_then(|s| s.parse().ok()).unwrap_or(8788);
        let ip: IpAddr = host.parse().unwrap_or(IpAddr::from([127, 0, 0, 1]));
        let allow_tailscale = std::env::var("ADMIN_BIND_TAILSCALE").ok().as_deref() == Some("1");
        let extra = std::env::var("ADMIN_ALLOWED_HOSTS").unwrap_or_default();
        Config {
            bind: SocketAddr::new(ip, port),
            admin_hash_file: env_or("ADMIN_HASH_FILE", "/etc/airaccount/admin.hash"),
            helper: env_or("ADMIN_HELPER", "/opt/airaccount/updater/airaccount-admin-helper"),
            node_id: std::env::var("AU_NODE_ID").unwrap_or_else(|_| hostname()),
            session_ttl: Duration::from_secs(1800), // 30 min
            twofa_ttl: Duration::from_secs(300),     // 5 min
            allow_tailscale,
            allowed_hosts: security::build_allowed_hosts(port, &ip, allow_tailscale, &extra),
            max_body: 16 * 1024, // 16 KiB:登录/apply body 都极小
            twofa_max_attempts: 5,
            #[cfg(test)]
            telegram_override: None,
        }
    }
}

/// 投递 Telegram(生产走 helper::telegram;测试可用 cfg.telegram_override 短路,避免真打 API)。
async fn deliver_telegram(cfg: &Config, msg: &str) -> bool {
    #[cfg(test)]
    if let Some(b) = cfg.telegram_override { return b; }
    #[cfg(not(test))]
    let _ = cfg;
    helper::telegram(msg).await
}

fn env_or(k: &str, d: &str) -> String { std::env::var(k).unwrap_or_else(|_| d.into()) }
fn hostname() -> String {
    std::process::Command::new("hostname").output().ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".into())
}

// ── 会话 / 2FA 挑战(内存态;单实例、短命,足够 MVP)────────────────────
#[derive(Clone)]
struct Session { expires: Instant }
#[derive(Clone)]
struct TwoFa {
    id: String,               // 挑战 id:confirm 必须回带,防同会话内被静默改换动作
    code: String,             // 一次性数字码(Telegram OOB)
    action: PendingAction,
    expires: Instant,
    attempts_left: u32,       // 剩余错误次数,归零即作废(防本机暴力)
}
#[derive(Clone)]
enum PendingAction { Apply(String), Rollback }

#[derive(Default)]
struct AppState {
    sessions: HashMap<String, Session>, // token -> Session
    twofa: HashMap<String, TwoFa>,      // session token -> pending 2FA
    csrf: HashMap<String, String>,      // session token -> csrf token
    login_fails: u32,                   // 连续登录失败计数(暴力/DoS 限速)
    login_locked_until: Option<Instant>,// 达阈值后锁定登录到此刻
}
impl AppState {
    /// 清掉过期会话,并连带清掉其 csrf / 2FA(否则永不回访的会话会内存泄漏)。
    fn purge(&mut self, now: Instant) {
        self.sessions.retain(|_, s| s.expires > now);
        let live: std::collections::HashSet<String> = self.sessions.keys().cloned().collect();
        self.csrf.retain(|k, _| live.contains(k));
        self.twofa.retain(|k, t| t.expires > now && live.contains(k));
    }
    /// 登录是否处于锁定期(连续失败过多)。
    fn login_locked(&self, now: Instant) -> bool {
        matches!(self.login_locked_until, Some(t) if t > now)
    }
}
type Shared = Arc<Mutex<AppState>>;

// ── 主 ───────────────────────────────────────────────────────────────
#[tokio::main]
async fn main() {
    // 运维子命令:`airaccount-admin hash-password`(从 stdin 读一行明文,输出 argon2id PHC 串)。
    // 不落盘、不进 argv,写入 /etc/airaccount/admin.hash 由运维自行重定向。
    if std::env::args().nth(1).as_deref() == Some("hash-password") {
        use std::io::BufRead;
        let mut pw = String::new();
        std::io::stdin().lock().read_line(&mut pw).ok();
        match security::hash_password(pw.trim_end_matches(['\n', '\r'])) {
            Ok(h) => { println!("{h}"); }
            Err(e) => { eprintln!("{e}"); std::process::exit(1); }
        }
        return;
    }

    let cfg = Arc::new(Config::from_env());

    // 启动自检:绝不在公网/被代理的地址上开(§6.1)。失败即退出,不带病上线。
    if let Err(e) = security::preflight_bind_check(&cfg.bind, cfg.allow_tailscale) {
        eprintln!("[admin] 启动自检失败,拒绝启动:{e}");
        std::process::exit(2);
    }
    eprintln!("[admin] 绑定 {} (仅本机/Tailscale);helper={}", cfg.bind, cfg.helper);

    let state: Shared = Arc::new(Mutex::new(AppState::default()));
    // 安全头在 recover 之后加 → OK 与错误(401/403/400)响应都带 CSP/X-Frame-Options。
    let routes = routes(cfg.clone(), state).recover(handle_rejection)
        .with(warp::reply::with::headers(security_headers()));
    warp::serve(routes).run(cfg.bind).await;
}

// ── 路由 ─────────────────────────────────────────────────────────────
fn routes(cfg: Arc<Config>, st: Shared) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    // `c`/`s` 是把 cfg/state **注入 handler 参数**的过滤器;`st` 保留原始 Arc,
    // 供 auth()/csrf_guard() 这类接收 `Shared` 值(而非过滤器)的辅助函数直接 clone。
    let c = { let cfg = cfg.clone(); warp::any().map(move || cfg.clone()) };
    let s = { let st = st.clone(); warp::any().map(move || st.clone()) };
    let max_body = cfg.max_body;
    // 每路由各自 `content_length_limit(max_body).and(body::json())`(json 类型逐路由推断,
    // 不能共用一个闭包 —— 那会把所有路由的 body 类型强行统一)。

    // 页面(静态,内嵌;GET 不改状态,免 CSRF,但仍加安全头)
    let pages = warp::get().and(warp::path::end()).map(|| html(pages::LOGIN))
        .or(warp::get().and(warp::path("dashboard")).and(warp::path::end()).map(|| html(pages::DASHBOARD)));

    // POST /api/login {password}
    let login = warp::post().and(warp::path!("api" / "login"))
        .and(origin_guard(cfg.clone())).and(c.clone()).and(s.clone()).and(warp::body::content_length_limit(max_body)).and(warp::body::json())
        .and_then(api_login);

    // GET /api/status —— 只读,会话即可
    let status = warp::get().and(warp::path!("api" / "status"))
        .and(auth(st.clone())).and(c.clone()).and_then(api_status);
    // GET /api/candidates —— 只读(updater list-candidates:列候选不安装)。保留 Origin 门作纵深防御。
    let candidates = warp::get().and(warp::path!("api" / "candidates"))
        .and(origin_guard(cfg.clone())).and(auth(st.clone())).and(c.clone()).and_then(api_candidates);

    // 写:会话 + CSRF + Origin(apply/rollback → 先发 2FA;confirm 才执行)
    let apply = warp::post().and(warp::path!("api" / "apply"))
        .and(origin_guard(cfg.clone())).and(auth(st.clone())).and(csrf_guard(st.clone()))
        .and(c.clone()).and(s.clone()).and(warp::body::content_length_limit(max_body)).and(warp::body::json()).and_then(api_apply_begin);
    let apply_confirm = warp::post().and(warp::path!("api" / "apply" / "confirm"))
        .and(origin_guard(cfg.clone())).and(auth(st.clone())).and(csrf_guard(st.clone()))
        .and(c.clone()).and(s.clone()).and(warp::body::content_length_limit(max_body)).and(warp::body::json()).and_then(api_confirm);
    let rollback = warp::post().and(warp::path!("api" / "rollback"))
        .and(origin_guard(cfg.clone())).and(auth(st.clone())).and(csrf_guard(st.clone()))
        .and(c.clone()).and(s.clone()).and(warp::body::content_length_limit(max_body)).and(warp::body::json()).and_then(api_rollback_begin);
    // logout 也是 POST 写操作:一致地加 Origin/CSRF 门(SameSite=Strict 已挡跨站,这里补全纵深)。
    let logout = warp::post().and(warp::path!("api" / "logout"))
        .and(origin_guard(cfg.clone())).and(auth(st.clone())).and(csrf_guard(st.clone()))
        .and(s.clone()).and_then(api_logout);
    // POST /api/clear-latch —— 人工解除 apply/check 超时置的 STUCK 闩(已认证 + CSRF + Origin 三重门)
    let clear_latch = warp::post().and(warp::path!("api" / "clear-latch"))
        .and(origin_guard(cfg.clone())).and(auth(st.clone())).and(csrf_guard(st.clone()))
        .and_then(api_clear_latch);

    // 注:安全头在 main/app 里于 `.recover()` **之后**统一加,确保 401/403/400 错误响应也带 CSP/XFO。
    pages.or(login).or(status).or(candidates).or(apply).or(apply_confirm).or(rollback).or(logout).or(clear_latch)
}

// ── 安全头(全响应)──────────────────────────────────────────────────
fn security_headers() -> warp::http::HeaderMap {
    let mut h = warp::http::HeaderMap::new();
    // 纯静态、无第三方脚本、无 iframe。default-src 'self',内联样式允许(页面用 <style>)。
    h.insert("content-security-policy", HeaderValue::from_static(
        "default-src 'none'; style-src 'self' 'unsafe-inline'; script-src 'self' 'unsafe-inline'; \
         connect-src 'self'; form-action 'self'; frame-ancestors 'none'; base-uri 'none'"));
    h.insert("x-frame-options", HeaderValue::from_static("DENY"));
    h.insert("x-content-type-options", HeaderValue::from_static("nosniff"));
    h.insert("referrer-policy", HeaderValue::from_static("no-referrer"));
    h.insert("cache-control", HeaderValue::from_static("no-store"));
    h
}
fn html(body: &'static str) -> warp::reply::Html<&'static str> { warp::reply::html(body) }

// ── Origin/Host 守卫(所有写操作 + 有副作用的 GET;防 DNS rebinding / 跨站,H3)──────
// **精确**匹配 authority 白名单(见 security::build_allowed_hosts),不做前缀/后缀。
fn origin_guard(cfg: Arc<Config>) -> impl Filter<Extract = (), Error = Rejection> + Clone {
    warp::header::optional::<String>("origin")
        .and(warp::header::optional::<String>("host"))
        .and(warp::any().map(move || cfg.clone()))
        .and_then(|origin: Option<String>, host: Option<String>, cfg: Arc<Config>| async move {
            let host = host.unwrap_or_default();
            if !security::host_allowed(&cfg.allowed_hosts, &host) {
                return Err(warp::reject::custom(Denied("Host 不在白名单")));
            }
            // Origin 若存在必须命中同一白名单(跨站请求带的 Origin ≠ 我们的 authority)。
            if let Some(o) = origin {
                if !o.is_empty() && !security::origin_authority_allowed(&cfg.allowed_hosts, &o) {
                    return Err(warp::reject::custom(Denied("Origin 不在白名单")));
                }
            }
            Ok::<(), Rejection>(())
        }).untuple_one()
}

// ── 会话守卫:校验 session cookie,注入 token 字符串 ────────────────────
fn auth(st: Shared) -> impl Filter<Extract = (String,), Error = Rejection> + Clone {
    warp::cookie::optional("sid").and(warp::any().map(move || st.clone()))
        .and_then(|sid: Option<String>, st: Shared| async move {
            let sid = sid.ok_or_else(|| warp::reject::custom(Unauthorized))?;
            let mut g = st.lock().await;
            g.purge(Instant::now()); // 顺手清过期会话/csrf/2FA
            match g.sessions.get(&sid) {
                Some(s) if s.expires > Instant::now() => Ok(sid),
                _ => { g.sessions.remove(&sid); Err(warp::reject::custom(Unauthorized)) }
            }
        })
}

// ── CSRF 守卫:双提交,X-CSRF 头必须等于会话绑定的 csrf token ─────────────
fn csrf_guard(st: Shared) -> impl Filter<Extract = (), Error = Rejection> + Clone {
    warp::cookie::optional("sid").and(warp::header::optional::<String>("x-csrf-token"))
        .and(warp::any().map(move || st.clone()))
        .and_then(|sid: Option<String>, hdr: Option<String>, st: Shared| async move {
            let sid = sid.unwrap_or_default();
            let hdr = hdr.unwrap_or_default();
            let g = st.lock().await;
            match g.csrf.get(&sid) {
                Some(tok) if ct_eq(tok, &hdr) => Ok::<(), Rejection>(()),
                _ => Err(warp::reject::custom(Denied("bad CSRF"))),
            }
        }).untuple_one()
}

// ── Handlers ─────────────────────────────────────────────────────────
#[derive(Deserialize)] struct LoginReq { password: String }
#[derive(Serialize)] struct LoginResp { csrf: String }

/// 串行化 argon2 校验(permit=1):argon2id 默认 ~19MiB/次,不限并发会被登录洪泛撑爆内存;
/// 且**串行**是让失败计数真正生效的关键 —— 并发时一批请求会在计数递增前都通过「未锁定」检查,
/// 锁形同虚设。本服务拒绝任何反代,没有外部限速可依赖,限速必须在进程内做。
fn login_sem() -> &'static tokio::sync::Semaphore {
    static SEM: std::sync::OnceLock<tokio::sync::Semaphore> = std::sync::OnceLock::new();
    SEM.get_or_init(|| tokio::sync::Semaphore::new(1))
}
const LOGIN_MAX_FAILS: u32 = 8;                 // 连续失败达此数即锁定
const LOGIN_LOCK: Duration = Duration::from_secs(60);

async fn api_login(cfg: Arc<Config>, st: Shared, req: LoginReq) -> Result<impl Reply, Rejection> {
    // 先拿 login permit(串行),**再**查锁定 —— 复查在拿锁之后,避免一批并发请求在计数递增前
    // 全部通过「未锁定」检查(check-before-await 竞争,与 helper STUCK 同类)。
    let _permit = login_sem().acquire().await.map_err(|_| warp::reject::reject())?;
    {
        let mut g = st.lock().await;
        if g.login_locked(Instant::now()) {
            return Ok(reply_status(StatusCode::TOO_MANY_REQUESTS, "登录失败过多,已临时锁定,请稍后再试"));
        }
        g.purge(Instant::now());
    }
    // argon2 校验:放 spawn_blocking(~19MiB/50ms,别阻塞 async worker)。
    let ok = {
        let hash_file = cfg.admin_hash_file.clone();
        let pw = req.password.clone();
        tokio::task::spawn_blocking(move || security::verify_password(&hash_file, &pw).unwrap_or(false))
            .await.unwrap_or(false)
    };
    if !ok {
        let mut g = st.lock().await;
        g.login_fails = g.login_fails.saturating_add(1);
        if g.login_fails >= LOGIN_MAX_FAILS {
            g.login_locked_until = Some(Instant::now() + LOGIN_LOCK);
            g.login_fails = 0;
        }
        return Ok(reply_status(StatusCode::UNAUTHORIZED, "密码错误"));
    }
    let sid = gen_token(); let csrf = gen_token();
    {
        let mut g = st.lock().await;
        g.login_fails = 0; g.login_locked_until = None; // 成功即清失败计数
        g.sessions.insert(sid.clone(), Session { expires: Instant::now() + cfg.session_ttl });
        g.csrf.insert(sid.clone(), csrf.clone());
    }
    let cookie = format!("sid={sid}; HttpOnly; SameSite=Strict; Path=/; Max-Age={}", cfg.session_ttl.as_secs());
    let mut resp = warp::reply::json(&LoginResp { csrf }).into_response();
    resp.headers_mut().insert("set-cookie", HeaderValue::from_str(&cookie).unwrap());
    Ok(resp)
}

async fn api_logout(sid: String, st: Shared) -> Result<impl Reply, Rejection> {
    let mut g = st.lock().await;
    g.sessions.remove(&sid); g.csrf.remove(&sid); g.twofa.remove(&sid);
    let mut resp = warp::reply::json(&serde_json::json!({"ok":true})).into_response();
    resp.headers_mut().insert("set-cookie", HeaderValue::from_static("sid=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0"));
    Ok(resp)
}

async fn api_status(_sid: String, cfg: Arc<Config>) -> Result<impl Reply, Rejection> {
    // 经 helper 读 updater status(state.json)。read-only,无副作用。
    let out = helper::run(&cfg.helper, &["status"]).await;
    let state = out.as_ref().ok().and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
        .unwrap_or(serde_json::Value::Null);
    Ok(warp::reply::json(&serde_json::json!({
        "node": cfg.node_id, "state": state,
        "ok": out.is_ok(),
    })))
}

async fn api_candidates(_sid: String, cfg: Arc<Config>) -> Result<impl Reply, Rejection> {
    // 只读:updater `list-candidates` 拉+验签 manifest,列出候选及分类(apply/notify/skip/denied),
    // **绝不安装**。pr-daemon #195 finding4:旧实现走 `check` = 面板「检查更新」零 2FA 触发真安装
    // (授权自相矛盾);拆开 → 本端点只读,安装一律走 apply + 2FA。
    // ⚠️ 部署侧:root helper(airaccount-admin-helper)的 argv 白名单须放行只读动词 `list-candidates`。
    let out = helper::run(&cfg.helper, &["list-candidates"]).await;
    let (ok, body) = match out {
        Ok(s) => (true, serde_json::from_str::<serde_json::Value>(&s).unwrap_or(serde_json::Value::Null)),
        Err(e) => (false, serde_json::json!({ "error": e })),
    };
    Ok(warp::reply::json(&serde_json::json!({ "ok": ok, "candidates": body })))
}

// apply:发起 → 生成 2FA 码 + 推 Telegram,等 confirm(决策 C)。
#[derive(Deserialize)] struct ApplyReq { version: String }
async fn api_apply_begin(_sid: String, cfg: Arc<Config>, st: Shared, req: ApplyReq) -> Result<impl Reply, Rejection> {
    if !security::is_semver(&req.version) {
        return Ok(warp::reply::with_status(warp::reply::json(&err("版本号非法")), StatusCode::BAD_REQUEST).into_response());
    }
    begin_2fa(_sid, cfg, st, PendingAction::Apply(req.version.clone()),
              format!("确认应用更新到 {}", req.version)).await
}

async fn api_rollback_begin(sid: String, cfg: Arc<Config>, st: Shared, _body: serde_json::Value)
    -> Result<impl Reply, Rejection>
{
    // 先读 status 解析回滚**目标版本**写进 2FA 摘要 —— 否则 Telegram 里只有「回滚到上一个健康版本」、
    // 没有版本号,运维盲签,二次确认形同虚设(pr-daemon #195 finding5)。目标逻辑与 updater
    // cmd_rollback 一致:pending 非空(中断的 apply)→ 目标=current;否则(正常撤销)→ 目标=previous。
    let target = helper::run(&cfg.helper, &["status"]).await.ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .and_then(|st| {
            let g = |k: &str| st.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
            let (pending, cur, prev) = (g("pending"), g("current"), g("previous"));
            let t = if !pending.is_empty() { cur } else { prev };
            if t.is_empty() { None } else { Some(t) }
        });
    let summary = match &target {
        Some(v) => format!("确认回滚到 {v}"),
        None    => "确认回滚到上一个健康版本(目标版本未知:status 读取失败/无 previous;确认前请自行核对)".into(),
    };
    begin_2fa(sid, cfg, st, PendingAction::Rollback, summary).await
}

async fn api_clear_latch(_sid: String) -> Result<impl Reply, Rejection> {
    // 人工确认 updater 已无后台残留后,清 STUCK 闩(apply/check 超时置的);已认证 + CSRF + Origin
    // 三重门(pr-daemon #195 finding3「已认证 clear-latch 端点」)。rollback 本就不受闩限,不必等这个。
    let was_stuck = helper::clear_stuck();
    Ok(warp::reply::json(&serde_json::json!({ "ok": true, "was_stuck": was_stuck })))
}

#[derive(Deserialize)] struct ConfirmReq { challenge_id: String, code: String }
async fn api_confirm(sid: String, cfg: Arc<Config>, st: Shared, req: ConfirmReq) -> Result<impl Reply, Rejection> {
    // 校验(全程持锁):挑战存在且未过期 → id 精确匹配(防同会话动作被静默改换)→ 剩余次数 → 码匹配。
    // 码错则扣次数,归零即作废(防本机对 8 位码暴力);任何一步失败都不执行 helper。
    let action = {
        let mut g = st.lock().await;
        let now = Instant::now();
        let Some(tf) = g.twofa.get_mut(&sid) else {
            return Ok(reply_status(StatusCode::UNAUTHORIZED, "没有待确认的操作"));
        };
        if tf.expires <= now { g.twofa.remove(&sid); return Ok(reply_status(StatusCode::UNAUTHORIZED, "确认已过期")); }
        if !ct_eq(&tf.id, &req.challenge_id) {
            return Ok(reply_status(StatusCode::UNAUTHORIZED, "挑战 id 不匹配(可能已被新的操作取代)"));
        }
        if ct_eq(&tf.code, &req.code) {
            let a = tf.action.clone(); g.twofa.remove(&sid); a
        } else {
            tf.attempts_left = tf.attempts_left.saturating_sub(1);
            let left = tf.attempts_left;
            if left == 0 { g.twofa.remove(&sid); }
            return Ok(reply_status(StatusCode::UNAUTHORIZED,
                &format!("确认码错误(剩余 {left} 次)")));
        }
    };
    // 二因子通过 → 经 helper 调 updater(helper 清 env + 固定 argv;updater 全程验签)
    let args: Vec<&str> = match &action {
        PendingAction::Apply(v) => vec!["apply", v.as_str()],
        PendingAction::Rollback => vec!["rollback"],
    };
    let out = helper::run(&cfg.helper, &args).await;
    Ok(warp::reply::json(&serde_json::json!({ "ok": out.is_ok(), "log": out.unwrap_or_else(|e| e) })).into_response())
}

async fn begin_2fa(sid: String, cfg: Arc<Config>, st: Shared, action: PendingAction, summary: String)
    -> Result<warp::reply::Response, Rejection>
{
    let id = gen_token();
    let code = security::gen_numeric_code(8); // 8 位:配合 5 次上限,本机暴力不可行
    // 先发 Telegram,**发成功才登记挑战** —— 否则运维拿不到码,登记了也没用,徒增内存/误导。
    let msg = format!(
        "🔐 管理台二次确认\n{summary}\n确认码: {code}\n(node={}, {}s 内有效)",
        cfg.node_id, cfg.twofa_ttl.as_secs());
    if !deliver_telegram(&cfg, &msg).await {
        return Ok(reply_status(StatusCode::BAD_GATEWAY, "Telegram 发送失败,未生成待确认操作(检查 TOKEN/CHAT_ID/网络)"));
    }
    {
        let mut g = st.lock().await;
        g.twofa.insert(sid, TwoFa {
            id: id.clone(), code, action,
            expires: Instant::now() + cfg.twofa_ttl,
            attempts_left: cfg.twofa_max_attempts,
        });
    }
    Ok(warp::reply::json(&serde_json::json!({
        "ok": true, "twofa": "sent", "challenge_id": id,
        "note": "一次性确认码已发到 Telegram,请回填 /api/apply/confirm"
    })).into_response())
}

fn reply_status(code: StatusCode, msg: &str) -> warp::reply::Response {
    warp::reply::with_status(warp::reply::json(&err(msg)), code).into_response()
}

// ── 错误 ─────────────────────────────────────────────────────────────
#[derive(Debug)] struct Unauthorized;
impl warp::reject::Reject for Unauthorized {}
#[derive(Debug)] struct Denied(&'static str);
impl warp::reject::Reject for Denied {}

fn err(m: &str) -> serde_json::Value { serde_json::json!({ "ok": false, "error": m }) }

async fn handle_rejection(r: Rejection) -> Result<impl Reply, std::convert::Infallible> {
    let (code, msg) = if r.find::<Unauthorized>().is_some() {
        (StatusCode::UNAUTHORIZED, "需要登录")
    } else if let Some(Denied(why)) = r.find::<Denied>() {
        (StatusCode::FORBIDDEN, *why)
    } else if r.is_not_found() {
        (StatusCode::NOT_FOUND, "not found")
    } else {
        (StatusCode::BAD_REQUEST, "bad request")
    };
    Ok(warp::reply::with_status(warp::reply::json(&err(msg)), code))
}

mod pages;

// ── HTTP 层安全守卫测试(真实攻击面:auth / CSRF / Origin-Host / semver)──────
#[cfg(test)]
mod http_tests {
    use super::*;

    fn test_cfg() -> Arc<Config> { test_cfg_tg(None) }
    fn test_cfg_tg(telegram_override: Option<bool>) -> Arc<Config> {
        let ip = "127.0.0.1".parse().unwrap();
        Arc::new(Config {
            bind: "127.0.0.1:8788".parse().unwrap(),
            admin_hash_file: "/nonexistent/admin.hash".into(),
            helper: "/nonexistent/helper".into(),
            node_id: "test-node".into(),
            session_ttl: Duration::from_secs(60),
            twofa_ttl: Duration::from_secs(60),
            allow_tailscale: false,
            allowed_hosts: security::build_allowed_hosts(8788, &ip, false, ""),
            max_body: 16 * 1024,
            twofa_max_attempts: 5,
            telegram_override,
        })
    }
    fn fresh_state() -> Shared { Arc::new(Mutex::new(AppState::default())) }
    // 与 main() 一致:带上 recover,让自定义 rejection 映射成 401/403/400 而非默认 500。
    fn app(cfg: Arc<Config>, st: Shared) -> impl Filter<Extract = impl Reply, Error = std::convert::Infallible> + Clone {
        routes(cfg, st).recover(handle_rejection)
            .with(warp::reply::with::headers(security_headers()))
    }
    async fn seed(st: &Shared) -> (String, String) {
        let sid = gen_token(); let csrf = gen_token();
        let mut g = st.lock().await;
        g.sessions.insert(sid.clone(), Session { expires: Instant::now() + Duration::from_secs(60) });
        g.csrf.insert(sid.clone(), csrf.clone());
        (sid, csrf)
    }

    #[tokio::test]
    async fn status_requires_session() {
        let r = app(test_cfg(), fresh_state());
        let resp = warp::test::request().path("/api/status").reply(&r).await;
        assert_eq!(resp.status(), 401, "无会话读 status 必须 401");
    }

    #[tokio::test]
    async fn status_ok_with_session() {
        let st = fresh_state();
        let (sid, _csrf) = seed(&st).await;
        let r = app(test_cfg(), st);
        let resp = warp::test::request().path("/api/status")
            .header("cookie", format!("sid={sid}")).reply(&r).await;
        // helper 不存在 → ok:false,但**会话校验通过**,应为 200 而非 401。
        assert_eq!(resp.status(), 200, "有会话应过 auth 门");
    }

    #[tokio::test]
    async fn write_rejected_without_csrf() {
        let st = fresh_state();
        let (sid, _csrf) = seed(&st).await;
        let r = app(test_cfg(), st);
        let resp = warp::test::request().method("POST").path("/api/apply")
            .header("host", "127.0.0.1:8788")           // 过 Origin/Host 门
            .header("cookie", format!("sid={sid}"))      // 过 auth 门
            .json(&serde_json::json!({"version":"1.2.3"}))
            .reply(&r).await;                            // 无 X-CSRF-Token → 必拒
        assert_eq!(resp.status(), 403, "缺 CSRF 的写操作必须 403");
    }

    #[tokio::test]
    async fn write_rejected_bad_host() {
        let st = fresh_state();
        let (sid, csrf) = seed(&st).await;
        let r = app(test_cfg(), st);
        let resp = warp::test::request().method("POST").path("/api/apply")
            .header("host", "evil.example.com")          // DNS rebinding 伪 Host
            .header("cookie", format!("sid={sid}"))
            .header("x-csrf-token", csrf)
            .json(&serde_json::json!({"version":"1.2.3"}))
            .reply(&r).await;
        assert_eq!(resp.status(), 403, "非回环/Tailscale Host 必须 403");
    }

    #[tokio::test]
    async fn apply_rejects_bad_semver() {
        let st = fresh_state();
        let (sid, csrf) = seed(&st).await;
        let r = app(test_cfg(), st);
        let resp = warp::test::request().method("POST").path("/api/apply")
            .header("host", "127.0.0.1:8788")
            .header("cookie", format!("sid={sid}"))
            .header("x-csrf-token", csrf)
            .json(&serde_json::json!({"version":"latest; rm -rf"}))
            .reply(&r).await;
        assert_eq!(resp.status(), 400, "非法版本号必须 400(不进 helper)");
    }

    #[tokio::test]
    async fn apply_good_semver_sends_2fa() {
        let st = fresh_state();
        let (sid, csrf) = seed(&st).await;
        let r = app(test_cfg_tg(Some(true)), st.clone()); // Telegram 投递成功
        let resp = warp::test::request().method("POST").path("/api/apply")
            .header("host", "127.0.0.1:8788")
            .header("cookie", format!("sid={sid}"))
            .header("x-csrf-token", csrf)
            .json(&serde_json::json!({"version":"0.29.1"}))
            .reply(&r).await;
        assert_eq!(resp.status(), 200, "合法 apply + Telegram 成功 → 进入 2FA 流程");
        let g = st.lock().await;
        assert!(g.twofa.contains_key(&sid), "应生成待确认的 2FA 挑战");
    }

    #[tokio::test]
    async fn apply_502_when_telegram_fails() {
        let st = fresh_state();
        let (sid, csrf) = seed(&st).await;
        let r = app(test_cfg_tg(Some(false)), st.clone()); // Telegram 投递失败
        let resp = warp::test::request().method("POST").path("/api/apply")
            .header("host", "127.0.0.1:8788")
            .header("cookie", format!("sid={sid}"))
            .header("x-csrf-token", csrf)
            .json(&serde_json::json!({"version":"0.29.1"}))
            .reply(&r).await;
        assert_eq!(resp.status(), 502, "Telegram 投递失败必须拒绝(fail-closed),不留挑战");
        assert!(!st.lock().await.twofa.contains_key(&sid), "投递失败不应登记挑战");
    }

    #[tokio::test]
    async fn confirm_rejects_wrong_code_and_bad_challenge() {
        // 直接种一个已知挑战,绕过 Telegram,测 confirm 的三道门。
        let st = fresh_state();
        let (sid, csrf) = seed(&st).await;
        {
            let mut g = st.lock().await;
            g.twofa.insert(sid.clone(), TwoFa {
                id: "chal-xyz".into(), code: "12345678".into(),
                action: PendingAction::Rollback,
                expires: Instant::now() + Duration::from_secs(60),
                attempts_left: 5,
            });
        }
        let r = app(test_cfg(), st.clone());
        let post = |body: serde_json::Value| {
            let sid = sid.clone(); let csrf = csrf.clone();
            warp::test::request().method("POST").path("/api/apply/confirm")
                .header("host", "127.0.0.1:8788")
                .header("cookie", format!("sid={sid}"))
                .header("x-csrf-token", csrf)
                .json(&body)
        };
        // 错误挑战 id → 401,且不扣次数
        let resp = post(serde_json::json!({"challenge_id":"wrong","code":"12345678"})).reply(&r).await;
        assert_eq!(resp.status(), 401, "挑战 id 不符必须拒");
        // 错误码 → 401,扣次数
        let resp = post(serde_json::json!({"challenge_id":"chal-xyz","code":"00000000"})).reply(&r).await;
        assert_eq!(resp.status(), 401, "错误码必须拒");
        assert_eq!(st.lock().await.twofa.get(&sid).unwrap().attempts_left, 4, "错误码应扣 1 次");
    }

    #[tokio::test]
    async fn login_wrong_password_401() {
        let r = app(test_cfg(), fresh_state());
        let resp = warp::test::request().method("POST").path("/api/login")
            .header("host", "127.0.0.1:8788")
            .json(&serde_json::json!({"password":"whatever"}))
            .reply(&r).await;
        assert_eq!(resp.status(), 401, "hash 文件不存在/密码错 → 401(fail-closed)");
    }

    #[tokio::test]
    async fn login_lockout_after_repeated_fails() {
        let st = fresh_state();
        let r = app(test_cfg(), st.clone()); // hash 文件不存在 → 每次都失败
        for _ in 0..LOGIN_MAX_FAILS {
            let resp = warp::test::request().method("POST").path("/api/login")
                .header("host", "127.0.0.1:8788")
                .json(&serde_json::json!({"password":"x"})).reply(&r).await;
            assert_eq!(resp.status(), 401, "失败登录应 401");
        }
        // 达阈值后锁定 → 429
        let resp = warp::test::request().method("POST").path("/api/login")
            .header("host", "127.0.0.1:8788")
            .json(&serde_json::json!({"password":"x"})).reply(&r).await;
        assert_eq!(resp.status(), 429, "连续失败达阈值后必须锁定(429)");
    }

    #[tokio::test]
    async fn error_responses_carry_security_headers() {
        // 401 错误响应也必须带 CSP/XFO(headers 在 recover 之后加)。
        let r = app(test_cfg(), fresh_state());
        let resp = warp::test::request().path("/api/status").reply(&r).await; // 无会话 → 401
        assert_eq!(resp.status(), 401);
        assert_eq!(resp.headers().get("x-frame-options").unwrap(), "DENY");
        assert!(resp.headers().get("content-security-policy").is_some(), "错误响应也要带 CSP");
    }

    #[tokio::test]
    async fn security_headers_present() {
        let r = app(test_cfg(), fresh_state());
        let resp = warp::test::request().path("/").reply(&r).await;
        let h = resp.headers();
        assert_eq!(h.get("x-frame-options").unwrap(), "DENY");
        assert!(h.get("content-security-policy").is_some(), "必须带 CSP");
        assert_eq!(h.get("x-content-type-options").unwrap(), "nosniff");
    }
}
