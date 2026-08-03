# airaccount-admin —— 社区节点更新 Web 管理台

CLI updater(`aastar-node-updater.sh`)的**可选** Web 前端。设计见
[`kms/docs/auto-update-web-admin-design.md`](../../docs/auto-update-web-admin-design.md)。

> CLI 始终是兜底:即便本服务没装/挂了,`ssh` 上板直接跑 updater 一样能更新。
> Web 台只是把「检查 / 应用 / 回滚」搬到浏览器,并加一层 Telegram 二次确认。

## 这是什么(增量 1)

- **只监听 127.0.0.1**(默认)或 **Tailscale 私网**;启动自检拒绝公网 / 被 cloudflared·frp·nginx 代理暴露。
- 管理员**密码登录**(argon2id)→ 会话 cookie(HttpOnly / SameSite=Strict / 30 分钟)。
- 所有写操作:会话 + **CSRF token** + 精确 **Origin/Host**(防 DNS rebinding)。
- **应用更新 / 回滚**:先发一次性码到 **Telegram**,回填才执行(二因子)。
- **非 root** 运行;一切改盘经唯一特权边界 `airaccount-admin-helper`(清 env + 固定 argv 白名单)。
- **含 TA 变更的更新拒绝在线一键**(updater 自身拒),需人工现场刷。

页面:登录 + 面板(状态 / 检查更新 / 应用 / 回滚 + 2FA)。实时进度 SSE、审计链留增量 2。

## 构建

```bash
cargo build -p airaccount-admin --release
# 产物:target/release/airaccount-admin(aarch64 板子用交叉编译,同 kms/host 流程)
```

## 部署(板子,root)

```bash
# 1) 专用用户(非 root,无 shell、无家目录)
useradd --system --no-create-home --shell /usr/sbin/nologin airaccount-admin

# 2) 放二进制 + helper
install -Dm0755 airaccount-admin              /opt/airaccount/admin/airaccount-admin
install -Dm0755 airaccount-admin-helper       /opt/airaccount/updater/airaccount-admin-helper
# helper 必须 root 拥有、airaccount-admin 不可写:
chown root:root /opt/airaccount/updater/airaccount-admin-helper

# 3) 管理员密码 hash(不落明文;直接写进受保护文件)
install -d -m0750 -o airaccount-admin -g airaccount-admin /etc/airaccount
read -rs PW; echo
printf '%s' "$PW" | /opt/airaccount/admin/airaccount-admin hash-password \
  > /etc/airaccount/admin.hash
chmod 0640 /etc/airaccount/admin.hash
chown root:airaccount-admin /etc/airaccount/admin.hash
unset PW

# 4) 环境文件(Telegram 二次确认 + 可选 Tailscale 绑定)
cat > /etc/airaccount/admin.env <<'EOF'
# 二次确认码走 AAStarMonitorBot(token/chat_id 见 ~/Dev/.env)
TELEGRAM_BOT_TOKEN=123456:abc...
TELEGRAM_CHAT_ID=-100...
AU_NODE_ID=mx93b
# 默认 127.0.0.1:8788。要经 Tailscale 访问再放开下面两行:
# ADMIN_BIND_TAILSCALE=1
# ADMIN_BIND_HOST=100.x.y.z
EOF
chmod 0640 /etc/airaccount/admin.env; chown root:airaccount-admin /etc/airaccount/admin.env

# 5) 极窄 sudoers(校验后再放)
install -m0440 sudoers.d-airaccount-admin /etc/sudoers.d/airaccount-admin
visudo -cf /etc/sudoers.d/airaccount-admin

# 6) systemd
install -m0644 airaccount-admin.service /etc/systemd/system/
systemctl daemon-reload && systemctl enable --now airaccount-admin
```

## 访问

- 本机:`http://127.0.0.1:8788/`
- 远程:先上 Tailscale,再 `http://<板子 100.x 地址>:8788/`(或 `ssh -L 8788:127.0.0.1:8788` 端口转发,免开 Tailscale 绑定)。
- **绝不**把 8788 挂到 cloudflared / frp / nginx 公网 —— 启动自检会直接拒绝。

## 安全边界一览

| 层 | 机制 |
|---|---|
| 网络 | 仅回环/Tailscale;启动扫隧道配置,发现暴露即拒启 |
| 认证 | argon2id 密码 → 会话 cookie(HttpOnly/SameSite=Strict) |
| 写操作 | 会话 + CSRF + Origin/Host 精确校验 |
| 高危动作 | apply/rollback 需 Telegram 一次性码二次确认 |
| 提权 | 非 root;仅经 helper(env -i + argv 白名单)触达 updater |
| TA 变更 | 在线一键拒绝,需人工现场 |
| 完整性 | updater 全程 minisign 验签 + sha256 + 健康门 + 自动回滚 |

## 测试

```bash
cargo test -p airaccount-admin   # 安全守卫单测 + warp::test HTTP 层(auth/CSRF/Origin/semver/2FA)
```
