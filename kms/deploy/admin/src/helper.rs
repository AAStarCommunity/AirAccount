//! 与特权边界(airaccount-admin-helper)及 Telegram 的交互。
//!
//! Web 服务**非 root**。所有触达 updater 的动作都经 `sudo -n <helper> <verb> [ver]`;
//! helper 自身清 env + 固定 argv 白名单(见 airaccount-admin-helper)。这里绝不拼 shell、
//! 绝不把用户输入放进 env —— 只把已在上层校验过的定长 argv 作为独立参数传入。

use std::process::Stdio;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

/// 经 sudo 调 helper。返回 Ok(stdout) 或 Err(拼接的 stderr/说明)。
/// args 已是定长白名单动词(status/check/apply <ver>/rollback),逐个作为独立 argv 传入。
pub async fn run(helper: &str, args: &[&str]) -> Result<String, String> {
    let out = Command::new("sudo")
        .arg("-n")
        .arg(helper)
        .args(args)
        .env_clear() // 本进程也不给 helper 带任何环境(helper 内部还会 env -i,双保险)
        .env("PATH", "/usr/sbin:/usr/bin:/sbin:/bin")
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(|e| format!("调 helper 失败: {e}"))?;
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    if out.status.success() {
        Ok(stdout)
    } else {
        Err(format!("helper 退出码 {:?}\n{}\n{}", out.status.code(), stdout, stderr))
    }
}

/// 发一条 Telegram 消息(二次确认码 / 通知)。best-effort:失败只记日志,不阻断。
/// token/chat_id 从 updater.env 或进程 env 读(TELEGRAM_BOT_TOKEN / TELEGRAM_CHAT_ID);
/// token 经 stdin 传给 curl(--config -),绝不进 argv(防 ps 泄漏)。
pub async fn telegram(msg: &str) {
    let token = std::env::var("TELEGRAM_BOT_TOKEN").ok()
        .filter(|s| !s.is_empty());
    let chat = std::env::var("TELEGRAM_CHAT_ID").ok().filter(|s| !s.is_empty());
    let (token, chat) = match (token, chat) {
        (Some(t), Some(c)) => (t, c),
        _ => { eprintln!("[admin] Telegram 未配置(TELEGRAM_BOT_TOKEN/CHAT_ID),跳过 OOB 推送"); return; }
    };
    let url = format!("https://api.telegram.org/bot{token}/sendMessage");
    // curl --config - 从 stdin 读全部参数(url 含 token、chat_id、text 一并走 config),
    // 都不进 argv → 防 `ps`/`/proc/<pid>/cmdline` 泄漏 token。
    let mut child = match Command::new("curl")
        .args(["-sS", "--max-time", "10", "--config", "-"])
        .stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => { eprintln!("[admin] 起 curl 失败: {e}"); return; }
    };
    if let Some(mut stdin) = child.stdin.take() {
        let body = format!(
            "url = \"{url}\"\ndata-urlencode = \"chat_id={chat}\"\ndata-urlencode = \"text={}\"\n",
            curl_escape(msg)
        );
        let _ = stdin.write_all(body.as_bytes()).await;
    }
    let _ = child.wait().await;
}

/// 转义进 curl config 文件的双引号串(反斜杠 + 双引号)。
fn curl_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}
