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
}

impl Config {
    fn from_env() -> Config {
        let host = std::env::var("ADMIN_BIND_HOST").unwrap_or_else(|_| "127.0.0.1".into());
        let port: u16 = std::env::var("ADMIN_BIND_PORT").ok().and_then(|s| s.parse().ok()).unwrap_or(8788);
        let ip: IpAddr = host.parse().unwrap_or(IpAddr::from([127, 0, 0, 1]));
        Config {
            bind: SocketAddr::new(ip, port),
            admin_hash_file: env_or("ADMIN_HASH_FILE", "/etc/airaccount/admin.hash"),
            helper: env_or("ADMIN_HELPER", "/opt/airaccount/updater/airaccount-admin-helper"),
            node_id: std::env::var("AU_NODE_ID").unwrap_or_else(|_| hostname()),
            session_ttl: Duration::from_secs(1800), // 30 min
            twofa_ttl: Duration::from_secs(300),     // 5 min
            allow_tailscale: std::env::var("ADMIN_BIND_TAILSCALE").ok().as_deref() == Some("1"),
        }
    }
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
struct TwoFa { code: String, action: PendingAction, expires: Instant }
#[derive(Clone)]
enum PendingAction { Apply(String), Rollback }

#[derive(Default)]
struct AppState {
    sessions: HashMap<String, Session>, // token -> Session
    twofa: HashMap<String, TwoFa>,      // session token -> pending 2FA
    csrf: HashMap<String, String>,      // session token -> csrf token
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
    let routes = routes(cfg.clone(), state).recover(handle_rejection);
    warp::serve(routes).run(cfg.bind).await;
}

// ── 路由 ─────────────────────────────────────────────────────────────
fn routes(cfg: Arc<Config>, st: Shared) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    // `c`/`s` 是把 cfg/state **注入 handler 参数**的过滤器;`st` 保留原始 Arc,
    // 供 auth()/csrf_guard() 这类接收 `Shared` 值(而非过滤器)的辅助函数直接 clone。
    let c = { let cfg = cfg.clone(); warp::any().map(move || cfg.clone()) };
    let s = { let st = st.clone(); warp::any().map(move || st.clone()) };

    // 页面(静态,内嵌;GET 不改状态,免 CSRF,但仍加安全头)
    let pages = warp::get().and(warp::path::end()).map(|| html(pages::LOGIN))
        .or(warp::get().and(warp::path("dashboard")).and(warp::path::end()).map(|| html(pages::DASHBOARD)));

    // POST /api/login {password}
    let login = warp::post().and(warp::path!("api" / "login"))
        .and(origin_guard()).and(c.clone()).and(s.clone()).and(warp::body::json())
        .and_then(api_login);

    // 需要会话的路由(读)
    let status = warp::get().and(warp::path!("api" / "status"))
        .and(auth(st.clone())).and(c.clone()).and_then(api_status);
    let candidates = warp::get().and(warp::path!("api" / "candidates"))
        .and(auth(st.clone())).and(c.clone()).and_then(api_candidates);

    // 写:会话 + CSRF + Origin(apply/rollback → 先发 2FA;confirm 才执行)
    let apply = warp::post().and(warp::path!("api" / "apply"))
        .and(origin_guard()).and(auth(st.clone())).and(csrf_guard(st.clone()))
        .and(c.clone()).and(s.clone()).and(warp::body::json()).and_then(api_apply_begin);
    let apply_confirm = warp::post().and(warp::path!("api" / "apply" / "confirm"))
        .and(origin_guard()).and(auth(st.clone())).and(csrf_guard(st.clone()))
        .and(c.clone()).and(s.clone()).and(warp::body::json()).and_then(api_confirm);
    let rollback = warp::post().and(warp::path!("api" / "rollback"))
        .and(origin_guard()).and(auth(st.clone())).and(csrf_guard(st.clone()))
        .and(c.clone()).and(s.clone()).and(warp::body::json()).and_then(api_rollback_begin);
    let logout = warp::post().and(warp::path!("api" / "logout"))
        .and(auth(st.clone())).and(s.clone()).and_then(api_logout);

    pages.or(login).or(status).or(candidates).or(apply).or(apply_confirm).or(rollback).or(logout)
        .with(warp::reply::with::headers(security_headers()))
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

// ── Origin/Host 守卫(所有写操作;防 DNS rebinding / CSRF via localhost,H3)──────
fn origin_guard() -> impl Filter<Extract = (), Error = Rejection> + Clone {
    warp::header::optional::<String>("origin")
        .and(warp::header::optional::<String>("host"))
        .and_then(|origin: Option<String>, host: Option<String>| async move {
            // Host 必须是回环/tailscale 名(不接受任意 Host → 防 DNS rebinding)。
            let host = host.unwrap_or_default();
            let host_ok = host.starts_with("127.0.0.1") || host.starts_with("localhost")
                || host.starts_with("[::1]") || host.ends_with(".ts.net")
                || host.split(':').next().map(security::is_tailscale_ip).unwrap_or(false);
            if !host_ok { return Err(warp::reject::custom(Denied("bad Host"))); }
            // Origin 若存在必须与 Host 精确同源;缺失(同源 fetch 常缺)放行但已有 Host 门槛。
            if let Some(o) = origin {
                let o_host = o.rsplit('/').next().unwrap_or("");
                if o_host != host && !o.is_empty() {
                    return Err(warp::reject::custom(Denied("origin!=host")));
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

async fn api_login(cfg: Arc<Config>, st: Shared, req: LoginReq) -> Result<impl Reply, Rejection> {
    // argon2id 校验(hash 存 /etc/airaccount/admin.hash,PHC 串)。失败限速由前置 fail2ban/nginx 兜。
    let ok = security::verify_password(&cfg.admin_hash_file, &req.password).unwrap_or(false);
    if !ok {
        return Ok(warp::reply::with_status(warp::reply::json(&err("密码错误")), StatusCode::UNAUTHORIZED).into_response());
    }
    let sid = gen_token(); let csrf = gen_token();
    {
        let mut g = st.lock().await;
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
    // 触发一次 check(拉+验签 manifest)→ 由 updater 决定候选/通知。这里只回传 helper 的原始输出摘要。
    // 完整「结构化候选列表」增量 2(需 updater 增 `list-candidates --json`)。
    let out = helper::run(&cfg.helper, &["check"]).await;
    Ok(warp::reply::json(&serde_json::json!({
        "ok": out.is_ok(),
        "log": out.unwrap_or_else(|e| e),
        "note": "结构化候选列表见增量 2;当前回传 check 日志"
    })))
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
    begin_2fa(sid, cfg, st, PendingAction::Rollback, "确认回滚到上一个健康版本".into()).await
}

#[derive(Deserialize)] struct ConfirmReq { code: String }
async fn api_confirm(sid: String, cfg: Arc<Config>, st: Shared, req: ConfirmReq) -> Result<impl Reply, Rejection> {
    let action = {
        let mut g = st.lock().await;
        match g.twofa.get(&sid) {
            Some(tf) if tf.expires > Instant::now() && ct_eq(&tf.code, &req.code) => {
                let a = tf.action.clone(); g.twofa.remove(&sid); a
            }
            _ => return Ok(warp::reply::with_status(warp::reply::json(&err("二次确认码错误或已过期")), StatusCode::UNAUTHORIZED).into_response()),
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
    let code = security::gen_numeric_code(6);
    {
        let mut g = st.lock().await;
        g.twofa.insert(sid, TwoFa { code: code.clone(), action, expires: Instant::now() + cfg.twofa_ttl });
    }
    // OOB 发到 Telegram(复用 notify-telegram.sh 的环境;这里直接 shell 出一条)。
    let msg = format!("🔐 管理台二次确认\n{}\n确认码: {}\n(node={}, {}s 内有效)", summary, code, cfg.node_id, cfg.twofa_ttl.as_secs());
    helper::telegram(&msg).await; // best-effort
    Ok(warp::reply::json(&serde_json::json!({
        "ok": true, "twofa": "sent",
        "note": "一次性确认码已发到 Telegram,请回填 /api/apply/confirm"
    })).into_response())
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

    fn test_cfg() -> Arc<Config> {
        Arc::new(Config {
            bind: "127.0.0.1:8788".parse().unwrap(),
            admin_hash_file: "/nonexistent/admin.hash".into(),
            helper: "/nonexistent/helper".into(),
            node_id: "test-node".into(),
            session_ttl: Duration::from_secs(60),
            twofa_ttl: Duration::from_secs(60),
            allow_tailscale: false,
        })
    }
    fn fresh_state() -> Shared { Arc::new(Mutex::new(AppState::default())) }
    // 与 main() 一致:带上 recover,让自定义 rejection 映射成 401/403/400 而非默认 500。
    fn app(cfg: Arc<Config>, st: Shared) -> impl Filter<Extract = impl Reply, Error = std::convert::Infallible> + Clone {
        routes(cfg, st).recover(handle_rejection)
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
        let r = app(test_cfg(), st.clone());
        let resp = warp::test::request().method("POST").path("/api/apply")
            .header("host", "127.0.0.1:8788")
            .header("cookie", format!("sid={sid}"))
            .header("x-csrf-token", csrf)
            .json(&serde_json::json!({"version":"0.29.1"}))
            .reply(&r).await;
        assert_eq!(resp.status(), 200, "合法 apply 应进入 2FA 流程");
        // 断言 pending 2FA 已登记(尚未真正执行 helper)
        let g = st.lock().await;
        assert!(g.twofa.contains_key(&sid), "应生成待确认的 2FA 挑战");
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
    async fn security_headers_present() {
        let r = app(test_cfg(), fresh_state());
        let resp = warp::test::request().path("/").reply(&r).await;
        let h = resp.headers();
        assert_eq!(h.get("x-frame-options").unwrap(), "DENY");
        assert!(h.get("content-security-policy").is_some(), "必须带 CSP");
        assert_eq!(h.get("x-content-type-options").unwrap(), "nosniff");
    }
}
