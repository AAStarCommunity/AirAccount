//! 安全原语:启动暴露自检、密码校验、token、常量时间比较。

use std::net::{IpAddr, SocketAddr};

use argon2::{Argon2, PasswordHash, PasswordVerifier};
use rand::RngCore;
use subtle::ConstantTimeEq;

/// Tailscale CGNAT 段 100.64.0.0/10。
pub fn is_tailscale_ip(s: &str) -> bool {
    match s.parse::<IpAddr>() {
        Ok(IpAddr::V4(v4)) => {
            let o = v4.octets();
            o[0] == 100 && (o[1] & 0xC0) == 0x40 // 100.64.0.0/10
        }
        Ok(IpAddr::V6(_)) => false,
        Err(_) => false,
    }
}

fn is_loopback(ip: &IpAddr) -> bool { ip.is_loopback() }

fn is_public(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => !(v4.is_loopback() || v4.is_private() || v4.is_link_local()
            || v4.is_unspecified() || is_tailscale_ip(&v4.to_string())),
        IpAddr::V6(v6) => !(v6.is_loopback() || v6.is_unspecified()
            || (v6.segments()[0] & 0xffc0) == 0xfe80  // link-local
            || (v6.segments()[0] & 0xfe00) == 0xfc00), // ULA
    }
}

/// 启动自检(§6.1 / pr-daemon H2):
///  - 绑定地址必须是回环(或显式开了 Tailscale 才允许 tailscale/私网);**绝不**公网。
///  - 端口不得出现在 cloudflared / frp 配置里(防被隧道到公网)。
///
/// 任一不满足 → Err,拒绝启动(fail-closed)。
pub fn preflight_bind_check(bind: &SocketAddr, allow_tailscale: bool) -> Result<(), String> {
    let ip = bind.ip();
    if is_public(&ip) {
        return Err(format!("绑定地址 {ip} 是公网 —— 管理台绝不可公网暴露"));
    }
    if !is_loopback(&ip) && !allow_tailscale {
        return Err(format!("绑定地址 {ip} 非回环,且未显式 ADMIN_BIND_TAILSCALE=1 —— 拒绝"));
    }
    if allow_tailscale && !is_loopback(&ip) && !is_tailscale_ip(&ip.to_string()) {
        // 开了 tailscale 但地址既非回环也非 tailscale 段 → 可疑
        // (私网 LAN 也不算安全:管理台不该暴露给整个局域网)
        return Err(format!("绑定地址 {ip} 既非回环也非 Tailscale(100.64/10)—— 拒绝"));
    }
    check_not_proxied(bind.port())?;
    Ok(())
}

/// 扫常见隧道/反代配置,发现本端口被暴露就拒绝启动。best-effort(文件不存在=没配=放行)。
fn check_not_proxied(port: u16) -> Result<(), String> {
    let needle_port = port.to_string();
    // cloudflared ingress(yml)/ frp(toml)/ nginx 常见路径
    let files = [
        "/etc/cloudflared/config.yml",
        "/etc/cloudflared/config.yaml",
        "/root/.cloudflared/config.yml",
        "/etc/frp/frpc.toml",
        "/etc/frp/frpc.ini",
    ];
    for f in files {
        if let Ok(content) = std::fs::read_to_string(f) {
            // 端口出现 + 该文件本就是隧道/反代 → 强烈信号
            if content.contains(&format!(":{needle_port}")) || content.contains(&format!("= {needle_port}"))
                || content.contains(&format!("localPort = {needle_port}"))
            {
                return Err(format!("端口 {port} 疑似被 {f} 暴露到隧道/公网 —— 拒绝启动(移除该 ingress)"));
            }
        }
    }
    // nginx 全局扫(可能多文件)—— 只做轻量:若 sites-enabled 里出现 proxy_pass 到本端口就拒。
    if let Ok(rd) = std::fs::read_dir("/etc/nginx/sites-enabled") {
        for e in rd.flatten() {
            if let Ok(content) = std::fs::read_to_string(e.path()) {
                if content.contains("proxy_pass") && content.contains(&format!(":{needle_port}")) {
                    return Err(format!("端口 {port} 疑似被 nginx({:?}) 反代 —— 拒绝启动", e.path()));
                }
            }
        }
    }
    // Tailscale serve/funnel:ADMIN_BIND_TAILSCALE=1 模式下最可能的「误发公网」路径 ——
    // `tailscale funnel` 会把本端口发布成公开 *.ts.net HTTPS。best-effort(取不到状态就跳过)。
    for sub in [["funnel", "status"], ["serve", "status"]] {
        if let Ok(out) = std::process::Command::new("tailscale").args(sub).output() {
            let s = String::from_utf8_lossy(&out.stdout);
            // funnel status 会列出被发布的端口;本端口出现即视为暴露。
            if out.status.success()
                && (s.contains(&format!(":{needle_port}")) || s.contains(&format!("127.0.0.1:{needle_port}")))
                && (sub[0] == "funnel" || s.to_lowercase().contains("funnel"))
            {
                return Err(format!("端口 {port} 疑似被 tailscale {} 发布到公网 —— 拒绝启动(tailscale funnel off)", sub[0]));
            }
        }
    }
    // 注:本进程以非 root 运行,root-only 的 /root/.cloudflared/config.yml 读不到会被静默跳过;
    // 隧道/反代的**权威**防线是绑定回环 + 上面的启动自检 + 部署侧 nftables/防火墙,配置扫描仅作 tripwire。
    Ok(())
}

/// argon2id 校验:读 hash 文件(PHC 串),与明文密码比对。
pub fn verify_password(hash_file: &str, password: &str) -> Result<bool, String> {
    let phc = std::fs::read_to_string(hash_file)
        .map_err(|e| format!("读密码 hash 失败 {hash_file}: {e}"))?;
    let phc = phc.trim();
    let parsed = PasswordHash::new(phc).map_err(|e| format!("hash 格式错误: {e}"))?;
    Ok(Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok())
}

/// 生成 argon2id PHC 串(供运维 `airaccount-admin hash-password` 子命令写入 admin.hash)。
pub fn hash_password(pw: &str) -> Result<String, String> {
    use argon2::password_hash::{PasswordHasher, SaltString};
    let salt = SaltString::generate(&mut rand::rngs::OsRng);
    Argon2::default()
        .hash_password(pw.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| format!("hash 失败: {e}"))
}

/// 128-bit 随机 token(会话 / CSRF),hex。
pub fn gen_token() -> String {
    let mut b = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut b);
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// n 位数字验证码(Telegram 二次确认)。用拒绝采样消除 `% 10` 的模偏(0..255 里 0-5 会略多)。
pub fn gen_numeric_code(n: usize) -> String {
    let mut rng = rand::thread_rng();
    let mut out = String::with_capacity(n);
    let mut buf = [0u8; 1];
    while out.len() < n {
        rng.fill_bytes(&mut buf);
        if buf[0] < 250 { // 250 = 25*10,拒绝 250..=255 使每位数字等概率
            out.push(char::from(b'0' + (buf[0] % 10)));
        }
    }
    out
}

/// 常量时间字符串比较(防会话/CSRF/2FA token 的时序侧信道)。
pub fn ct_eq(a: &str, b: &str) -> bool {
    a.as_bytes().ct_eq(b.as_bytes()).into()
}

/// 构造允许的 Host/Origin authority 精确集合(小写,含带端口与不带端口两式)。
/// 回环三式 + 可选 Tailscale 绑定地址 + `ADMIN_ALLOWED_HOSTS`(逗号分隔的 FQDN/IP)。
/// **精确匹配**,绝不前缀/后缀 —— 否则 `127.0.0.1.evil.com` / `x.ts.net` 能骗过 DNS-rebinding 门。
pub fn build_allowed_hosts(port: u16, bind_ip: &IpAddr, allow_tailscale: bool, extra_csv: &str) -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    let mut push = |base: &str| { v.push(base.to_ascii_lowercase()); v.push(format!("{}:{port}", base.to_ascii_lowercase())); };
    push("127.0.0.1"); push("localhost"); push("[::1]");
    if allow_tailscale {
        // 只把**实际绑定的** IP 加进白名单(而非整个 100.64/10 段);IPv6 需方括号。
        let b = match bind_ip { IpAddr::V6(_) => format!("[{bind_ip}]"), IpAddr::V4(_) => bind_ip.to_string() };
        push(&b);
    }
    for e in extra_csv.split(',') {
        let e = e.trim();
        if !e.is_empty() { push(e); }
    }
    v.sort(); v.dedup(); v
}

/// authority 是否在允许集合内(精确、大小写不敏感)。
pub fn host_allowed(allowed: &[String], host: &str) -> bool {
    let h = host.trim().to_ascii_lowercase();
    allowed.iter().any(|a| a == &h)
}

/// 从 Origin 头(`scheme://authority[/...]`)提取 authority 并精确校验。空 Origin 交由调用方决定。
pub fn origin_authority_allowed(allowed: &[String], origin: &str) -> bool {
    let o = origin.trim().to_ascii_lowercase();
    let no_scheme = o.strip_prefix("http://").or_else(|| o.strip_prefix("https://")).unwrap_or(&o);
    let authority = no_scheme.split('/').next().unwrap_or("");
    allowed.iter().any(|a| a == authority)
}

/// 严格 semver x.y.z(可带 v 前缀)。
pub fn is_semver(v: &str) -> bool {
    let v = v.strip_prefix('v').unwrap_or(v);
    let parts: Vec<&str> = v.split('.').collect();
    parts.len() == 3 && parts.iter().all(|p| !p.is_empty() && p.bytes().all(|c| c.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;

    #[test]
    fn bind_check_rejects_public() {
        assert!(preflight_bind_check(&"8.8.8.8:8788".parse::<SocketAddr>().unwrap(), true).is_err());
        assert!(preflight_bind_check(&"0.0.0.0:8788".parse::<SocketAddr>().unwrap(), true).is_err());
    }
    #[test]
    fn bind_check_allows_loopback() {
        assert!(preflight_bind_check(&"127.0.0.1:8788".parse::<SocketAddr>().unwrap(), false).is_ok());
    }
    #[test]
    fn bind_check_tailscale_needs_optin() {
        let ts = "100.69.249.7:8788".parse::<SocketAddr>().unwrap();
        assert!(preflight_bind_check(&ts, false).is_err(), "tailscale 需显式 opt-in");
        assert!(preflight_bind_check(&ts, true).is_ok());
    }
    #[test]
    fn tailscale_range() {
        assert!(is_tailscale_ip("100.64.0.1"));
        assert!(is_tailscale_ip("100.127.255.254"));
        assert!(!is_tailscale_ip("100.128.0.1")); // 超出 /10
        assert!(!is_tailscale_ip("192.168.1.1"));
    }
    #[test]
    fn semver() {
        assert!(is_semver("1.2.3")); assert!(is_semver("v0.29.1"));
        assert!(!is_semver("latest")); assert!(!is_semver("1.2")); assert!(!is_semver("1.2.3.4"));
        assert!(!is_semver("1.2.x"));
    }
    #[test]
    fn host_allowlist_is_exact() {
        let ip = "127.0.0.1".parse().unwrap();
        let allowed = build_allowed_hosts(8788, &ip, false, "");
        assert!(host_allowed(&allowed, "127.0.0.1:8788"));
        assert!(host_allowed(&allowed, "localhost:8788"));
        assert!(host_allowed(&allowed, "127.0.0.1"));
        assert!(host_allowed(&allowed, "LOCALHOST:8788")); // 大小写不敏感
        // 关键:前缀/后缀混淆一律拒
        assert!(!host_allowed(&allowed, "127.0.0.1.evil.com"));
        assert!(!host_allowed(&allowed, "localhost.evil.com"));
        assert!(!host_allowed(&allowed, "evil.ts.net"));
        assert!(!host_allowed(&allowed, "127.0.0.1:9999")); // 端口不符
        assert!(!host_allowed(&allowed, "evil.com"));
    }
    #[test]
    fn tailscale_host_only_when_bound() {
        let ts: IpAddr = "100.69.249.7".parse().unwrap();
        let a = build_allowed_hosts(8788, &ts, true, "");
        assert!(host_allowed(&a, "100.69.249.7:8788"));
        assert!(!host_allowed(&a, "100.64.0.1:8788")); // 段内但非绑定地址 → 拒
        let extra = build_allowed_hosts(8788, &"127.0.0.1".parse().unwrap(), false, "mx93b.tailnet.ts.net");
        assert!(host_allowed(&extra, "mx93b.tailnet.ts.net:8788"));
        assert!(!host_allowed(&extra, "other.tailnet.ts.net:8788"));
    }
    #[test]
    fn origin_authority_exact() {
        let ip = "127.0.0.1".parse().unwrap();
        let allowed = build_allowed_hosts(8788, &ip, false, "");
        assert!(origin_authority_allowed(&allowed, "http://127.0.0.1:8788"));
        assert!(origin_authority_allowed(&allowed, "http://localhost:8788/dashboard"));
        assert!(!origin_authority_allowed(&allowed, "http://127.0.0.1.evil.com:8788"));
        assert!(!origin_authority_allowed(&allowed, "https://evil.com"));
    }
    #[test]
    fn ct_eq_works() {
        assert!(ct_eq("abc", "abc")); assert!(!ct_eq("abc", "abd")); assert!(!ct_eq("abc", "ab"));
    }
    #[test]
    fn password_hash_roundtrip() {
        let phc = hash_password("s3cret-pw").unwrap();
        let dir = std::env::temp_dir();
        let f = dir.join(format!("aa-admin-test-{}.hash", std::process::id()));
        std::fs::write(&f, &phc).unwrap();
        assert!(verify_password(f.to_str().unwrap(), "s3cret-pw").unwrap(), "正确密码应通过");
        assert!(!verify_password(f.to_str().unwrap(), "wrong").unwrap(), "错误密码应拒绝");
        std::fs::remove_file(&f).ok();
    }
    #[test]
    fn verify_missing_file_errs() {
        assert!(verify_password("/nonexistent/admin.hash", "x").is_err());
    }
    #[test]
    fn code_is_numeric() {
        let c = gen_numeric_code(6);
        assert_eq!(c.len(), 6);
        assert!(c.bytes().all(|b| b.is_ascii_digit()));
    }
}
