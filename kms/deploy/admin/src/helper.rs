//! 与特权边界(airaccount-admin-helper)及 Telegram 的交互。
//!
//! Web 服务**非 root**。所有触达 updater 的动作都经 `sudo -n <helper> <verb> [ver]`;
//! helper 自身清 env + 固定 argv 白名单(见 airaccount-admin-helper)。这里绝不拼 shell、
//! 绝不把用户输入放进 env —— 只把已在上层校验过的定长 argv 作为独立参数传入。

use std::process::Stdio;
use std::sync::OnceLock;
use std::time::Duration;

use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::sync::Semaphore;

const OUTPUT_CAP: usize = 64 * 1024;                       // 截断 helper 输出,防内存膨胀

/// 按动词分档的 helper 超时。apply/check 会下载 tarball(updater `fetch --max-time 300`)+ 装 +
/// 健康门,上限必须 **> 下载预算**,否则一次网络慢的例行 check 就超时置 STUCK 闩、把 rollback 也
/// 锁死(pr-daemon #195 finding3:旧的固定 180s < 300s 下载预算)。rollback 纯本地(换软链 +
/// restart,不下载);status/list-candidates 只读(list-candidates 拉一份很小的 manifest)。
fn helper_timeout(args: &[&str]) -> Duration {
    match args.first().copied() {
        Some("apply") | Some("check") => Duration::from_secs(420), // 300 下载 + 验签/装/健康门余量
        Some("rollback")              => Duration::from_secs(120), // 本地换软链 + restart + 健康门
        _                             => Duration::from_secs(90),  // status(纯本地)/ list-candidates(拉小 manifest)
    }
}

/// 变更动词(apply/rollback)串行化到 **1** —— 超时 kill_on_drop 的 SIGKILL 打不动已 exec 进
/// root updater 的 sudo 子进程(Linux 按当前 uid 判权),放两个并发会起第二个 apply。
fn mut_sem() -> &'static Semaphore {
    static SEM: OnceLock<Semaphore> = OnceLock::new();
    SEM.get_or_init(|| Semaphore::new(1))
}
/// 读动词(status/list-candidates)单独限流,**不**排在 apply 后面(否则一次 apply 能让面板
/// hang 满 apply 的超时预算)。注意 check 现在算变更(会真装)、不走这里。
fn read_sem() -> &'static Semaphore {
    static SEM: OnceLock<Semaphore> = OnceLock::new();
    SEM.get_or_init(|| Semaphore::new(3))
}

/// 变更动词一旦超时(root updater 可能还活着、杀不掉),置此闩 → 拒绝后续变更动词自动重试,
/// 要求人工介入(fail-closed;updater 自身的 flock 也会挡住第二个 apply,这是纵深防御)。
fn helper_stuck() -> &'static std::sync::atomic::AtomicBool {
    static STUCK: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    &STUCK
}
fn is_mutating(args: &[&str]) -> bool {
    // 会改 state/软链的动词都走 mut_sem 串行(避免并发写 state)。check 也算:updater 的 cmd_check
    // 在候选 auto_apply_allowed + 策略过关时会**真装**(download_verify_apply:换软链 + restart + 写 state)。
    matches!(args.first().copied(), Some("apply") | Some("rollback") | Some("check"))
}
/// 受 STUCK 闩保护的动词 = **只有会下载的 apply/check**。它们超时后 root updater 可能仍在后台
/// 跑(杀不掉),故置闩拒绝后续自动重试。**rollback 不在此列**(pr-daemon #195 finding3):它纯
/// 本地(换软链 + restart,不下载)、有 updater flock 兜底,是「板子卡住时的救命按钮」——绝不能被
/// 一次 check 超时置的闩连坐锁死。rollback 仍走 mut_sem 串行,只是不看闩、也不置闩。
fn is_latchable(args: &[&str]) -> bool {
    matches!(args.first().copied(), Some("apply") | Some("check"))
}

/// 清 STUCK 闩(人工确认 updater 已无后台残留后调用)。返回清之前是否处于 stuck。
/// 由 main.rs 的**已认证 + CSRF**端点触发 —— 见 finding3「再给一个已认证的 clear-latch 端点」。
pub fn clear_stuck() -> bool {
    helper_stuck().swap(false, std::sync::atomic::Ordering::SeqCst)
}

fn cap(mut s: String) -> String {
    if s.len() > OUTPUT_CAP { s.truncate(OUTPUT_CAP); s.push_str("\n…(输出已截断)"); }
    s
}

/// 经 sudo 调 helper。返回 Ok(stdout) 或 Err(拼接的 stderr/说明)。
/// args 已是定长白名单动词(status/check/apply <ver>/rollback),逐个作为独立 argv 传入。
pub async fn run(helper: &str, args: &[&str]) -> Result<String, String> {
    use std::sync::atomic::Ordering;
    let mutating = is_mutating(args);
    let latchable = is_latchable(args);      // 仅 apply/check 受 STUCK 闩;rollback 不被闩连坐(finding3)
    let timeout = helper_timeout(args);
    let stuck_err = || "上一次下载类操作(apply/check)超时且可能仍在后台运行,已拒绝新的 apply/check(rollback 不受此限;需人工检查 updater/flock 后经 clear-latch 端点或重启 admin 服务解除)".to_string();
    // 先 acquire 对应信号量,**再**(对 latchable 动词)复查 STUCK 闩 —— 复查必须在拿到 permit 之后:
    // 否则 B 在闩置位前通过检查、随后阻塞等 permit,A 超时置闩并释放 permit,B 会带着过期的
    // 「未 stuck」判断继续起第二个 apply(正是本闩要堵的竞争)。读动词与 rollback 不看闩。
    let _permit = if mutating {
        if latchable && helper_stuck().load(Ordering::SeqCst) { return Err(stuck_err()); }
        let p = mut_sem().acquire().await.map_err(|_| "并发信号量关闭".to_string())?;
        if latchable && helper_stuck().load(Ordering::SeqCst) { return Err(stuck_err()); } // 拿锁后再查一次(仅 latchable)
        p
    } else {
        read_sem().acquire().await.map_err(|_| "并发信号量关闭".to_string())?
    };
    let child = Command::new("sudo")
        .arg("-n")
        .arg(helper)
        .args(args)
        .env_clear() // 本进程也不给 helper 带任何环境(helper 内部还会 env -i,双保险)
        .env("PATH", "/usr/sbin:/usr/bin:/sbin:/bin")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true) // 超时丢弃 future → 杀子进程,不留孤儿
        .spawn()
        .map_err(|e| format!("调 helper 失败: {e}"))?;

    let out = match tokio::time::timeout(timeout, child.wait_with_output()).await {
        Ok(Ok(o)) => o,
        Ok(Err(e)) => return Err(format!("helper 执行错误: {e}")),
        Err(_) => {
            // 只有 latchable(apply/check,会下载、超时后 root updater 可能仍在跑且杀不掉)才置闩;
            // rollback 超时不置闩(纯本地 + flock 兜底),否则救命按钮会把自己也锁死(finding3)。
            if latchable {
                helper_stuck().store(true, Ordering::SeqCst);
                eprintln!("[admin] 下载类 helper 超时(>{}s),置 STUCK 闩:root updater 可能仍在后台运行,\
                           已锁定不再自动重试 apply/check(rollback 不受限),需人工检查后经 clear-latch 解除", timeout.as_secs());
            }
            return Err(format!("helper 超时(>{}s);若为 apply/check,updater 可能仍在后台运行,已锁定不再自动重试(rollback 不受限)", timeout.as_secs()));
        }
    };
    let stdout = cap(String::from_utf8_lossy(&out.stdout).to_string());
    let stderr = cap(String::from_utf8_lossy(&out.stderr).to_string());
    if out.status.success() {
        Ok(stdout)
    } else {
        Err(format!("helper 退出码 {:?}\n{}\n{}", out.status.code(), stdout, stderr))
    }
}

/// 发一条 Telegram 消息(二次确认码 / 通知)。返回是否**投递成功**(curl 退出码 0)。
/// token/chat_id 从进程 env 读(TELEGRAM_BOT_TOKEN / TELEGRAM_CHAT_ID);url 含 token,故
/// **整份参数经 stdin(curl --config -)传入**,绝不进 argv → 防 `ps`/`/proc` 泄漏 token。
pub async fn telegram(msg: &str) -> bool {
    let token = std::env::var("TELEGRAM_BOT_TOKEN").ok().filter(|s| !s.is_empty());
    let chat = std::env::var("TELEGRAM_CHAT_ID").ok().filter(|s| !s.is_empty());
    let (token, chat) = match (token, chat) {
        (Some(t), Some(c)) => (t, c),
        _ => { eprintln!("[admin] Telegram 未配置(TELEGRAM_BOT_TOKEN/CHAT_ID),跳过 OOB 推送"); return false; }
    };
    let url = format!("https://api.telegram.org/bot{token}/sendMessage");
    // curl -q:忽略 ~/.curlrc,不受环境 curlrc 影响。--config -:全部参数走 stdin。
    let mut child = match Command::new("curl")
        .args(["-q", "-sS", "--max-time", "10", "--config", "-"])
        .stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
    {
        Ok(c) => c,
        Err(e) => { eprintln!("[admin] 起 curl 失败: {e}"); return false; }
    };
    if let Some(mut stdin) = child.stdin.take() {
        // curl config 双引号值支持 \n \r \t \\ \" 转义;必须转义,否则值里的真实换行会截断解析。
        let body = format!(
            "url = \"{}\"\ndata-urlencode = \"chat_id={}\"\ndata-urlencode = \"text={}\"\n",
            cfg_escape(&url), cfg_escape(&chat), cfg_escape(msg));
        if stdin.write_all(body.as_bytes()).await.is_err() { return false; }
    }
    match tokio::time::timeout(Duration::from_secs(12), child.wait()).await {
        Ok(Ok(status)) => status.success(),
        _ => false,
    }
}

/// 转义进 curl `--config` 双引号串:反斜杠/双引号,以及会截断解析的换行/回车/制表符。
fn cfg_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
     .replace('\n', "\\n").replace('\r', "\\r").replace('\t', "\\t")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn escape_neutralizes_newlines_and_quotes() {
        let e = cfg_escape("a\"b\nc\\d\te");
        assert!(!e.contains('\n'), "真实换行必须被转义,否则截断 curl config");
        assert_eq!(e, "a\\\"b\\nc\\\\d\\te");
    }
    #[test]
    fn output_cap_truncates() {
        let big = "x".repeat(OUTPUT_CAP + 100);
        let c = cap(big);
        assert!(c.len() <= OUTPUT_CAP + 40);
        assert!(c.ends_with("(输出已截断)"));
    }
}
