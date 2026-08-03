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

const HELPER_TIMEOUT: Duration = Duration::from_secs(180); // updater apply/check 的上限
const OUTPUT_CAP: usize = 64 * 1024;                       // 截断 helper 输出,防内存膨胀

/// 限制并发 helper 进程数:防被反复触发 apply 造成 sudo/进程风暴(updater 自身还有 flock 串行)。
fn helper_sem() -> &'static Semaphore {
    static SEM: OnceLock<Semaphore> = OnceLock::new();
    SEM.get_or_init(|| Semaphore::new(2))
}

fn cap(mut s: String) -> String {
    if s.len() > OUTPUT_CAP { s.truncate(OUTPUT_CAP); s.push_str("\n…(输出已截断)"); }
    s
}

/// 经 sudo 调 helper。返回 Ok(stdout) 或 Err(拼接的 stderr/说明)。
/// args 已是定长白名单动词(status/check/apply <ver>/rollback),逐个作为独立 argv 传入。
pub async fn run(helper: &str, args: &[&str]) -> Result<String, String> {
    let _permit = helper_sem().acquire().await.map_err(|_| "并发信号量关闭".to_string())?;
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

    let out = match tokio::time::timeout(HELPER_TIMEOUT, child.wait_with_output()).await {
        Ok(Ok(o)) => o,
        Ok(Err(e)) => return Err(format!("helper 执行错误: {e}")),
        Err(_) => return Err(format!("helper 超时(>{}s),已终止", HELPER_TIMEOUT.as_secs())),
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
