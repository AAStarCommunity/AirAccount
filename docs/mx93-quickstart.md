# MX93 快速部署手册（一次性成功指南）

> 版本：v0.19.0 | 板子：NXP FRDM-IMX93 | 连接：USB Serial `/dev/cu.usbmodem5B6D0044901`
> 创建于：2026-06-07

这份文档记录了在 NXP FRDM-IMX93 (aarch64 Cortex-A55, OP-TEE 4.8) 上部署 AirAccount KMS 的完整过程和**关键经验教训**。按本文档操作，应能一次成功。

---

## 硬件信息

| 项目 | 值 |
|------|-----|
| SoC | NXP i.MX93, Cortex-A55 × 2 @ 1.7GHz |
| 内存 | 2GB LPDDR4x |
| 存储 | 32GB eMMC |
| OS | Yocto Linux |
| OP-TEE | 4.8 (revision e7ed997) |
| TEE TRNG | EdgeLock ELE |
| WiFi | mlan0 |
| 串口 | `/dev/cu.usbmodem5B6D0044901` @115200 |

---

## 前置条件

**在 Mac 上**（交叉工具链 + OP-TEE 4.8 签名工具）：

```bash
# Python 依赖（TA 重签名需要）
pip3 install cryptography

# 获取 OP-TEE 4.8 默认签名密钥（RSA-4096！不是旧版的 RSA-2048）
gh api 'repos/OP-TEE/optee_os/contents/keys/default_ta.pem' -X GET -F ref=4.8.0 \
  | python3 -c "import sys,json,base64; d=json.load(sys.stdin); print(base64.b64decode(d['content']).decode())" \
  > /tmp/optee48_default_ta.pem

# 获取 OP-TEE 4.8 sign_encrypt.py
gh api 'repos/OP-TEE/optee_os/contents/scripts/sign_encrypt.py' -X GET -F ref=4.8.0 \
  | python3 -c "import sys,json,base64; d=json.load(sys.stdin); print(base64.b64decode(d['content']).decode())" \
  > /tmp/sign_encrypt_48.py
```

---

## Step 1：编译 Host（在板子上原生编译）

MX93 上已有 Rust 1.96.0。**Host（kms-api-server）在板子上原生编译，TA 在 Mac 上交叉编译**。

```bash
# 通过串口连接（pyserial 或 screen）
screen /dev/cu.usbmodem5B6D0044901 115200

# 在板子上（root，无密码）
export PATH="/root/.cargo/bin:$PATH"

# 创建 libteec 符号链接（Linker 需要无版本 .so）
ln -sf /usr/lib/libteec.so.2.0.0 /usr/lib/libteec.so

# 拉取代码
cd /root && wget -q 'https://github.com/AAStarCommunity/AirAccount/archive/refs/heads/main.tar.gz' \
  -O /tmp/airaccount.tar.gz && tar xzf /tmp/airaccount.tar.gz && mv AirAccount-main AirAccount

# 编译（OPTEE_CLIENT_EXPORT="/" 让 build.rs 找到 /usr/lib/libteec.so）
cd /root/AirAccount
OPTEE_CLIENT_EXPORT="/" cargo build -p kms --release 2>&1 | tee /root/build.log
# 约 3-4 分钟，完成后看到：Finished release [optimized]
```

> **关键**：`OPTEE_CLIENT_EXPORT="/"` 告诉 optee-teec-sys build.rs 在 `/usr/lib/` 下找 libteec，而不是默认的 TA Dev Kit 路径。

---

## Step 2：编译并签名 TA（在 Mac 上）

TA（Trusted Application）运行在 OP-TEE Secure World，必须用 **RSA-4096 密钥** 签名（OP-TEE 4.8 升级）。

### 2a. 编译 TA

```bash
# 方式 A: Docker 交叉编译（推荐）
# 使用 OP-TEE 4.8 兼容的 Docker 镜像
docker run --rm -v $(pwd):/work OP-TEE-4.8-builder \
  make -C /work/kms/ta TA_DEV_KIT_DIR=/path/to/export-ta_arm64

# 输出：kms/ta/target/aarch64-unknown-optee/release/stripped_ta
```

> **注意**：TA 不能在板子上编译，Yocto 镜像没有 TA Dev Kit (`export-ta_arm64`)。

### 2b. 重签名（OP-TEE 4.8 RSA-4096）

```bash
UUID="4319f351-0b24-4097-b659-80ee4f824cdd"
STRIPPED_TA="kms/ta/target/aarch64-unknown-optee/release/stripped_ta"

python3 /tmp/sign_encrypt_48.py sign-enc \
  --uuid "$UUID" \
  --key /tmp/optee48_default_ta.pem \
  --in "$STRIPPED_TA" \
  --out "${UUID}.ta"

# 验证签名大小（4.8 RSA-4096 = sig_size 0x200 = 512 bytes）
od -A x -t x1z "${UUID}.ta" | head -5
# 应看到 sig_size 字段为 00 02 00 00（小端 = 512）
```

**历史错误**：旧版 TA 用 RSA-2048 签名（sig_size = 256），OP-TEE 4.8 会返回 `TEE_ERROR_SECURITY (0xffff000f)`。必须用 4.8 的 key 重签。

---

## Step 3：传输 TA 到板子

WiFi AP 隔离导致 Mac ↔ 板子无法直连，使用串口 base64 传输：

```python
import serial, time, base64

ser = serial.Serial('/dev/cu.usbmodem5B6D0044901', 115200, timeout=2)

def slow_send(cmd, wait=2):
    for c in cmd:
        ser.write(c.encode()); time.sleep(0.007)
    ser.write(b'\r\n'); time.sleep(wait)
    return ser.read(ser.in_waiting).decode('utf-8', errors='replace')

UUID = "4319f351-0b24-4097-b659-80ee4f824cdd"
TA_PATH = f"{UUID}.ta"

# 编码
with open(TA_PATH, 'rb') as f:
    data = base64.b64encode(f.read()).decode()
lines = [data[i:i+60] for i in range(0, len(data), 60)]

# 传输（15ms/行）
slow_send(f"cat > /tmp/ta.b64 << 'TAEOF'", wait=1)
for i, line in enumerate(lines):
    for c in line:
        ser.write(c.encode()); time.sleep(0.007)
    ser.write(b'\r\n'); time.sleep(0.015)
slow_send("TAEOF", wait=2)

# 解码并安装
slow_send("base64 -d /tmp/ta.b64 > /tmp/ta.bin && echo OK")
slow_send(f"cp /tmp/ta.bin /usr/lib/optee_armtz/{UUID}.ta && echo INSTALLED")
```

---

## Step 4：配置并启动 systemd 服务

```bash
# 在板子上
cat > /etc/systemd/system/kms-api.service << 'EOF'
[Unit]
Description=AirAccount KMS API Server
After=network.target tee-supplicant@teepriv0.service
Requires=tee-supplicant@teepriv0.service

[Service]
Type=simple
User=root
WorkingDirectory=/root/AirAccount
ExecStart=/root/AirAccount/target/release/kms-api-server
Restart=on-failure
RestartSec=5
StandardOutput=append:/var/log/kms-api.log
StandardError=append:/var/log/kms-api.log
Environment=RUST_LOG=info
Environment=RUST_BACKTRACE=1

[Install]
WantedBy=multi-user.target
EOF

systemctl daemon-reload
systemctl enable kms-api.service
systemctl start kms-api.service
```

> **关键**：tee-supplicant 服务名是 `tee-supplicant@teepriv0.service`（不是 `tee_supplicant.service`）。

---

## Step 5：验证

```bash
# Health check
curl http://localhost:3000/health
# 期望: {"status":"healthy","ta_mode":"real","version":"0.19.0",...}

# CreateKey（必须包含 x-amz-target 和 PasskeyPublicKey）
TEST_PK=$(python3 -c "
from cryptography.hazmat.primitives.asymmetric.ec import generate_private_key, SECP256R1
from cryptography.hazmat.backends import default_backend
from cryptography.hazmat.primitives.serialization import Encoding, PublicFormat
key = generate_private_key(SECP256R1(), default_backend())
pub = key.public_key().public_bytes(Encoding.X962, PublicFormat.UncompressedPoint)
print('0x' + pub.hex())
")

curl -s -X POST http://localhost:3000/CreateKey \
  -H 'Content-Type: application/json' \
  -H 'x-amz-target: TrentService.CreateKey' \
  -d "{\"KeySpec\":\"ECC_SECG_P256K1\",\"KeyUsage\":\"SIGN_VERIFY\",\"Description\":\"test\",\"Origin\":\"EXTERNAL\",\"PasskeyPublicKey\":\"$TEST_PK\"}" | python3 -m json.tool
```

---

## Step 6：Cloudflare Tunnel（公网发布到 kms.aastar.io）

参考 `docs/cloudflare-mx93-setup.md`。

---

## 关键经验教训（必读）

### 1. 必须加 x-amz-target header

所有 AWS KMS 格式端点（`/CreateKey`、`/Sign` 等）必须携带：

```
x-amz-target: TrentService.<OperationName>
```

缺少此 header 时，Warp 路由匹配失败，返回 500（不是 400），极易误导排查方向。

### 2. CreateKey 必填 PasskeyPublicKey

```json
{
  "KeySpec": "ECC_SECG_P256K1",
  "KeyUsage": "SIGN_VERIFY",
  "Description": "my-key",
  "Origin": "EXTERNAL",
  "PasskeyPublicKey": "0x04<64字节hex，P256 uncompressed>"
}
```

缺少 `PasskeyPublicKey` 或 `Origin` 时请求失败（JSON 反序列化错误），日志中无错误（错误在路由层）。

### 3. TA 签名密钥：OP-TEE 版本决定 RSA 长度

| OP-TEE 版本 | 签名算法 | sig_size |
|-------------|---------|----------|
| ≤ 4.6 | RSA-2048 | 0x100 (256) |
| ≥ 4.7 | RSA-4096 | 0x200 (512) |

错误签名 → `TEE_ERROR_SECURITY (0xffff000f)`。检查方法：`od -A x -t x1z <UUID>.ta | head`。

### 4. TA 不能在板子上编译

Yocto 最小镜像没有 `export-ta_arm64/` TA Dev Kit，必须在 Mac/Linux 上交叉编译。
Host（kms-api-server）可以（也应该）在板子上原生编译。

### 5. libteec.so 符号链接

Linker 找 `-lteec` 需要无版本 `.so`：

```bash
ln -sf /usr/lib/libteec.so.2.0.0 /usr/lib/libteec.so
```

### 6. WiFi AP 隔离

WiFi AP 开启了客户端隔离，Mac 无法直连板子。所有文件传输通过串口 base64。

### 7. tee-supplicant 服务名

```bash
# 正确
tee-supplicant@teepriv0.service
# 错误（不存在）
tee_supplicant.service
```

---

## 故障排查

| 错误 | 原因 | 解决 |
|------|------|------|
| HTTP 500 "Internal server error" | 缺少 `x-amz-target` header | 加 `-H 'x-amz-target: TrentService.<Op>'` |
| HTTP 500 on CreateKey | 缺少 `PasskeyPublicKey` 或 `Origin` 字段 | 补全5个必填字段 |
| TEE_ERROR_ITEM_NOT_FOUND (0xffff0008) | TA 未安装或 UUID 不匹配 | 检查 `/usr/lib/optee_armtz/*.ta` |
| TEE_ERROR_SECURITY (0xffff000f) | TA 签名密钥版本不匹配 | 用 OP-TEE 4.8 RSA-4096 key 重签 |
| `OPTEE_CLIENT_EXPORT is not set` | build.rs 找不到 libteec | 设置 `export OPTEE_CLIENT_EXPORT="/"` |
| Linker: `-lteec` not found | 缺少无版本 .so | `ln -sf /usr/lib/libteec.so.2.0.0 /usr/lib/libteec.so` |
| kms-api.service failed to start | tee-supplicant 未就绪 | 检查 `systemctl status tee-supplicant@teepriv0` |
