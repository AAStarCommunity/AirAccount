# MX93 / MX95 自启动配置

*创建时间: 2026-06-08*

## 目标

板子加电后自动启动 OP-TEE 守护进程（tee-supplicant）和 KMS CA（kms-api-server），
TA 在 CA 第一次调用时由 OP-TEE 按需加载。

## 各组件启动时序

```
加电
 └─ U-Boot → Linux 内核引导
     └─ systemd target: multi-user.target
         ├─ tee-supplicant.service  ← OP-TEE 用户态守护进程（Yocto 默认已启用）
         │    完成后 ↓
         └─ kms-api.service         ← KMS CA（需手动 enable）
              第一个 API 请求到来时 ↓
              TA 文件从 /lib/optee_armtz/ 加载进 Secure World
```

**TA 不是开机驻留**——它是懒加载，CA 第一次 `TEEC_InvokeCommand` 时才进入 Secure World。

---

## 配置步骤（在板子上运行）

通过串口或 SSH 连接到板子后执行：

```bash
# 1. 确认 OP-TEE 守护进程已启用（Yocto 一般默认启用）
systemctl is-enabled tee-supplicant || systemctl enable tee-supplicant

# 2. 确认 kms-api.service 文件存在
cat /etc/systemd/system/kms-api.service

# 3. 如果不存在，创建它
cat > /etc/systemd/system/kms-api.service << 'EOF'
[Unit]
Description=AirAccount KMS API Server
After=network-online.target tee-supplicant.service
Wants=network-online.target
Requires=tee-supplicant.service

[Service]
Type=simple
User=root
WorkingDirectory=/root/AirAccount
ExecStart=/root/AirAccount/target/release/kms-api-server
Restart=on-failure
RestartSec=5
Environment=RUST_LOG=info
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
EOF

# 4. 重新加载 systemd 配置
systemctl daemon-reload

# 5. 启用 kms-api（开机自启）
systemctl enable kms-api.service

# 6. 验证
systemctl is-enabled kms-api.service    # 应输出 "enabled"
systemctl is-enabled tee-supplicant     # 应输出 "enabled"

# 7. 立刻测试启动
systemctl start kms-api.service
systemctl status kms-api.service
```

---

## 验证自启是否生效

```bash
# 重启板子（软重启）
reboot

# 重启后（约 30 秒），在 Mac 上测试
curl https://kms.aastar.io/health
# 应返回 {"status":"healthy",...}
```

---

## 排错

| 现象 | 检查命令 | 常见原因 |
|------|---------|---------|
| 服务没起来 | `journalctl -u kms-api -n 50` | 二进制路径错误、tee-supplicant 未启动 |
| TA 加载失败 | `journalctl -u tee-supplicant -n 50` | TA 文件不在 `/lib/optee_armtz/` |
| 服务起了但 API 返回 500 | `curl localhost:3000/health` | TEE 调用失败，检查 OP-TEE 日志 |

---

## IMX95 说明

IMX95 是全新板子，没有预先部署 kms-api-server。加电后只有：
- Linux（Yocto）运行
- OP-TEE 4.x 守护进程启动

需要先完成一次完整部署（参见 `docs/mx93-quickstart.md`），再做上面的 `systemctl enable` 步骤。
