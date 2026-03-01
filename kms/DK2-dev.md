# STM32MP157F-DK2 KMS Development Guide

## Network Setup

```
Mac Mini M4 (192.168.7.3) ──RJ45──> STM32MP157F-DK2 (192.168.7.2)
SSH: ssh root@192.168.7.2
```

Board: Cortex-A7 650MHz, ARMv7-A 32-bit, OP-TEE 3.16, OpenSTLinux kirkstone v22.06

## Docker Build Environment

```bash
# Start builder container (first time)
docker run -it --name stm32-builder \
  -v /Volumes/UltraDisk/Dev2/aastar/AirAccount:/workspace \
  stm32-builder:latest /bin/bash

# Resume existing container
docker start -ai stm32-builder
```

### Container Setup (one-time)

```bash
# Fix optee-utee-build symlink to use our patched version
ln -sf /workspace/sdks/rust-sdk/optee-utee-build /optee-utee-build

# Install signing dependency
pip3 install cryptography

# GCC wrapper at /tmp/arm-wrapper-gcc
cat > /tmp/arm-wrapper-gcc << 'EOF'
#!/bin/bash
exec /opt/st/stm32mp1/6.6-v26.02.18/sysroots/aarch64-ostl_sdk-linux/usr/bin/arm-ostl-linux-gnueabi/arm-ostl-linux-gnueabi-gcc \
  -nostartfiles \
  --sysroot=/opt/st/stm32mp1/6.6-v26.02.18/sysroots/cortexa7t2hf-neon-vfpv4-ostl-linux-gnueabi \
  -mthumb -mfpu=neon-vfpv4 -mfloat-abi=hard -mcpu=cortex-a7 "$@"
EOF
chmod +x /tmp/arm-wrapper-gcc
```

## Build TA (Trusted Application)

```bash
cd /workspace/kms/ta

# Set environment
export TA_DEV_KIT_DIR=/opt/st/stm32mp1/6.6-v26.02.18/sysroots/cortexa7t2hf-neon-vfpv4-ostl-linux-gnueabi/usr/include/optee/export-user_ta
export TARGET_TA=arm-unknown-linux-gnueabihf
export RUST_TARGET_PATH=/workspace/sdks/rust-sdk
export CC_arm_unknown_optee=/tmp/arm-wrapper-gcc

# Build with xargo (nightly-2024-05-15)
HOST_CC=gcc xargo build --target arm-unknown-optee --release \
  --config 'target.arm-unknown-optee.linker="/tmp/arm-wrapper-gcc"'
```

## Post-Build Pipeline: Strip -> Fix -> Sign

```bash
TA_UUID=4319f351-0b24-4097-b659-80ee4f824cdd
TARGET_DIR=target/arm-unknown-optee/release

# 1. Strip
/opt/st/stm32mp1/6.6-v26.02.18/sysroots/aarch64-ostl_sdk-linux/usr/bin/arm-ostl-linux-gnueabi/arm-ostl-linux-gnueabi-objcopy \
  --strip-unneeded $TARGET_DIR/ta $TARGET_DIR/stripped_ta

# 2. Fix ELF (remove NOTE segment, fix GNU_STACK flags RW->RWE)
python3 fix_ta_elf.py $TARGET_DIR/stripped_ta $TARGET_DIR/fixed_ta

# 3. Sign
python3 $TA_DEV_KIT_DIR/scripts/sign_encrypt.py \
  --uuid $TA_UUID \
  --key $TA_DEV_KIT_DIR/keys/default_ta.pem \
  --in $TARGET_DIR/fixed_ta \
  --out $TARGET_DIR/${TA_UUID}.ta
```

## Build CA (Host Application)

CA runs in Normal World, compiled as standard ARM Linux binary.

```bash
cd /workspace/kms/host

# Create CA linker wrapper (one-time, same as TA wrapper but without -nostartfiles)
cat > /tmp/arm-ca-gcc << 'EOF'
#!/bin/bash
exec /opt/st/stm32mp1/6.6-v26.02.18/sysroots/aarch64-ostl_sdk-linux/usr/bin/arm-ostl-linux-gnueabi/arm-ostl-linux-gnueabi-gcc \
  --sysroot=/opt/st/stm32mp1/6.6-v26.02.18/sysroots/cortexa7t2hf-neon-vfpv4-ostl-linux-gnueabi \
  -mthumb -mfpu=neon-vfpv4 -mfloat-abi=hard -mcpu=cortex-a7 "$@"
EOF
chmod +x /tmp/arm-ca-gcc

# Set environment
export PATH="/root/.cargo/bin:$PATH"
export OPTEE_CLIENT_EXPORT=/opt/st/stm32mp1/6.6-v26.02.18/sysroots/cortexa7t2hf-neon-vfpv4-ostl-linux-gnueabi
export CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABIHF_LINKER=/tmp/arm-ca-gcc
export CC_armv7_unknown_linux_gnueabihf=/tmp/arm-ca-gcc

# Build
cargo build --release --target armv7-unknown-linux-gnueabihf
```

Output binaries:
- `target/armv7-unknown-linux-gnueabihf/release/kms` — CLI tool
- `target/armv7-unknown-linux-gnueabihf/release/kms-api-server` — API server

## Deploy to Board

```bash
# From Mac (outside Docker, files are in shared volume)
TA_UUID=4319f351-0b24-4097-b659-80ee4f824cdd

# Deploy TA
scp kms/ta/target/arm-unknown-optee/release/${TA_UUID}.ta root@192.168.7.2:/lib/optee_armtz/

# Deploy CA binaries
scp kms/host/target/armv7-unknown-linux-gnueabihf/release/kms root@192.168.7.2:/usr/local/bin/
scp kms/host/target/armv7-unknown-linux-gnueabihf/release/kms-api-server root@192.168.7.2:/usr/local/bin/

# Restart service
ssh root@192.168.7.2 "systemctl restart kms-api"
```

## Systemd Service

File: `/etc/systemd/system/kms.service`

```ini
[Unit]
Description=KMS API Server
After=tee-supplicant.service network.target
Requires=tee-supplicant.service

[Service]
Type=simple
ExecStart=/usr/local/bin/kms-api-server
Restart=on-failure
RestartSec=5
Environment=RUST_LOG=info

[Install]
WantedBy=multi-user.target
```

```bash
systemctl enable kms
systemctl start kms
journalctl -u kms -f  # View logs
```

## Testing

```bash
# Health check
curl http://192.168.7.2:3000/health

# Create wallet (fast, entropy only)
curl -X POST http://192.168.7.2:3000/CreateKey \
  -H "Content-Type: application/json" \
  -H "x-amz-target: TrentService.CreateKey" \
  -d '{"Description":"test","KeyUsage":"SIGN_VERIFY","KeySpec":"ECC_SECG_P256K1","Origin":"AWS_KMS"}'

# Derive address (slow, BIP39+BIP32+secp256k1 on 650MHz ARM)
curl -X POST http://192.168.7.2:3000/DeriveAddress \
  -H "Content-Type: application/json" \
  -H "x-amz-target: TrentService.DeriveAddress" \
  -d '{"KeyId":"<wallet-id>","DerivationPath":"m/44h/60h/0h/0/0"}'

# CLI
ssh root@192.168.7.2 "kms create-wallet"
ssh root@192.168.7.2 "kms derive-address --wallet-id <id> --hd-path m/44h/60h/0h/0/0"
```

## Key Fixes Applied

1. **ta_head at vaddr 0** — linker script places `.ta_head` before `.text`
2. **DT_HASH required** — `--hash-style=both` linker flag (OP-TEE 3.16 needs DT_HASH, not just GNU_HASH)
3. **No NOTE segment** — `/DISCARD/` for `.note` sections
4. **No build-id** — `--build-id=none` removes PT_NOTE from build-id
5. **GNU_STACK RWE** — `fix_ta_elf.py` patches flags from RW (6) to RWE (7)

## Performance Notes

- CreateKey: ~instant (entropy generation only)
- DeriveAddress: 10-30+ seconds (PBKDF2-HMAC-SHA512 2048 rounds + BIP32 + secp256k1 on 32-bit 650MHz)
- Sign: similar to DeriveAddress (secp256k1 key derivation + signing)
- TEE concurrency: limited to 1 concurrent operation (semaphore)
