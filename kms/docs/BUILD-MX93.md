# MX93 (i.MX93 / aarch64) TA + CA 交叉编译与部署

> 目标板：NXP FRDM-IMX93（aarch64, Cortex-A55）
> TA UUID：`4319f351-0b24-4097-b659-80ee4f824cdd`
> 一键脚本：`scripts/mx93-build.sh`（build）+ `scripts/mx93-deploy.sh`（deploy）
> 本文档记录手动流程与**四个必踩的坑**，脚本失效时照此排查。

## 0. 环境

- 编译在 Docker 容器 `teaclave_dev_env` 内进行。
- 容器把 Mac 的 `kms/` 和 `third_party/teaclave-trustzone-sdk` bind-mount 进去，**产物在 Mac 端直接可见**（无需 docker cp 出来）：
  - 容器 `/root/teaclave_sdk_src/projects/web3/kms` = Mac `<repo>/kms`
- 两个 target、两个 toolchain：

| 组件 | target | toolchain | cargo | 产物 |
|------|--------|-----------|-------|------|
| TA | `aarch64-unknown-optee` | `nightly-2024-05-15` | 1.80 | `kms/ta/target/aarch64-unknown-optee/release/<UUID>.ta` |
| CA | `aarch64-unknown-linux-gnu` | `stable` | 1.88 | `kms/host/target/aarch64-unknown-linux-gnu/release/kms-api-server` |

> TA 必须用 nightly-2024-05-15（OP-TEE std 适配锁定该版本）。CA 是普通 Linux 二进制，**必须用 stable 1.88**——nightly 1.80 解析不了依赖树里 getrandom 0.4.2 等较新 crate 的 manifest（见坑 4）。

## 1. Build TA

```bash
docker exec teaclave_dev_env bash -c '
  export PATH=/root/.cargo/bin:$PATH
  export OPTEE_OS_DIR=/opt/teaclave/optee/optee_os
  export TA_DEV_KIT_DIR=$OPTEE_OS_DIR/out/arm-plat-vexpress/export-ta_arm64
  export TARGET_TA=aarch64-unknown-optee
  export CROSS_COMPILE_TA=aarch64-linux-gnu-
  export RUST_TARGET_PATH=/opt/teaclave/std
  export RUSTUP_TOOLCHAIN=nightly-2024-05-15
  export CARGO_NET_OFFLINE=true
  # 坑 1 + 坑 2：链接器 + C 依赖交叉编译器
  export CARGO_TARGET_AARCH64_UNKNOWN_OPTEE_LINKER=aarch64-linux-gnu-gcc
  export CC_aarch64_unknown_optee=aarch64-linux-gnu-gcc
  export AR_aarch64_unknown_optee=aarch64-linux-gnu-ar
  export HOST_CC=gcc
  unset http_proxy https_proxy HTTP_PROXY HTTPS_PROXY
  cd /root/teaclave_sdk_src/projects/web3/kms/ta
  UUID=4319f351-0b24-4097-b659-80ee4f824cdd
  OUT=target/aarch64-unknown-optee/release
  rm -f $OUT/ta $OUT/stripped_ta $OUT/$UUID.ta   # 坑 3：清旧产物
  xargo build --release --target aarch64-unknown-optee
  aarch64-linux-gnu-objcopy --strip-unneeded $OUT/ta $OUT/stripped_ta
  python3 $TA_DEV_KIT_DIR/scripts/sign_encrypt.py sign-enc \
    --uuid $UUID --ta-version 0 \
    --in $OUT/stripped_ta --out $OUT/$UUID.ta \
    --key $TA_DEV_KIT_DIR/keys/default_ta.pem
  file $OUT/ta   # 必须是 "ELF 64-bit ... ARM aarch64"
'
```

## 2. Build CA

```bash
docker exec teaclave_dev_env bash -c '
  export PATH=/root/.cargo/bin:$PATH
  export RUSTUP_TOOLCHAIN=stable                 # 坑 4：CA 用 stable 1.88
  export OPTEE_CLIENT_EXPORT=/opt/teaclave/optee/optee_client/export_arm64
  export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc
  export CC_aarch64_unknown_linux_gnu=aarch64-linux-gnu-gcc
  export AR_aarch64_unknown_linux_gnu=aarch64-linux-gnu-ar
  export CXX_aarch64_unknown_linux_gnu=aarch64-linux-gnu-g++
  export HOST_CC=gcc
  export CARGO_NET_OFFLINE=true
  unset http_proxy https_proxy HTTP_PROXY HTTPS_PROXY
  cd /root/teaclave_sdk_src/projects/web3/kms/host
  rm -f target/aarch64-unknown-linux-gnu/release/kms-api-server   # 坑 3
  cargo build --release --target aarch64-unknown-linux-gnu --bin kms-api-server
  file target/aarch64-unknown-linux-gnu/release/kms-api-server
'
```

## 3. 四个坑（按踩到的顺序）

### 坑 1 — release 链接报 `cannot represent machine 'aarch64'`
`xargo check` 不链接，所以编译检查通过不代表能 build。`build --release` 链接时默认 linker 不认 aarch64。
**修**：`CARGO_TARGET_<TARGET大写下划线>_LINKER=aarch64-linux-gnu-gcc`。

### 坑 2 — C 依赖被编成 x86：`Relocations in generic ELF (EM: 62)`（最隐蔽）
`secp256k1-sys` 等含 C 代码的 crate 由 cc crate 用 `HOST_CC` 编译。不设交叉 CC 时它用 host gcc 编成 **x86-64**（EM 62），链接 aarch64 时报错。
**修**：`CC_<target>=aarch64-linux-gnu-gcc` + `AR_<target>=aarch64-linux-gnu-ar`，并删掉被污染的缓存重建：
```bash
rm -rf target/aarch64-unknown-optee/release/build/secp256k1-sys* \
       target/aarch64-unknown-optee/release/deps/libsecp256k1_sys*
```

### 坑 3 — 失败的 build 留下旧产物，`file` 显示旧 ARM 二进制会骗你
build 失败不会覆盖上一次的产物。`file` 看到一个 ARM aarch64 文件 ≠ 这次 build 成功。
**修**：每次 build 前 `rm` 目标产物；只认命令输出的 `Finished` 和新的 BuildID/时间戳。

### 坑 4 — 容器无外网 + cargo registry hash 目录不匹配 + toolchain 太旧
- 容器连不上 crates.io（ClashX TUN + DNS）。CA 的依赖需要从 Mac `~/.cargo/registry` 来。
- 但容器 cargo 1.80 用 sparse hash 目录 `index.crates.io-6f17d22bba15001f`，Mac 新 cargo 用 `index.crates.io-1949cf8c6b5b557f`。`docker cp` 进来后要把 `.crate` 归位：
  ```bash
  docker cp ~/.cargo/registry/cache teaclave_dev_env:/root/.cargo/registry/
  docker cp ~/.cargo/registry/index teaclave_dev_env:/root/.cargo/registry/
  docker exec teaclave_dev_env bash -c '
    S=/root/.cargo/registry/cache/index.crates.io-1949cf8c6b5b557f
    D=/root/.cargo/registry/cache/index.crates.io-6f17d22bba15001f
    find "$S" -name "*.crate" -exec cp -n {} "$D/" \;'
  ```
- 即便 `.crate` 齐了，nightly-1.80 仍解析不了 getrandom 0.4.2 等新 manifest。**CA 改用 stable（cargo 1.88）**即可。

## 4. 部署（scp，不要走串口）

Mac 与板子同网段（Mac `192.168.2.40` / 板 `192.168.2.30`，板 WiFi `mlan0`），SSH 免密已配。**串口 115200 传 CA(~15MB) 半小时且易错，必须走 scp。**

```bash
MX93_BOARD_IP=192.168.2.30 ./scripts/mx93-deploy.sh
# 首次安装 systemd 服务： … ./scripts/mx93-deploy.sh --first-run
```

deploy 脚本做：停 kms-api → 推 TA 到 `/lib/optee_armtz/<UUID>.ta` → 推 CA 到 `/root/AirAccount/target/release/kms-api-server` → 重启 tee-supplicant → 起 kms-api → `GET /health` 冒烟。

## 5. 验证

```bash
# 板上
ssh root@192.168.2.30 'systemctl status kms-api.service; curl -s localhost:3000/health'
# E2E（本机，需板子 IP 可达）
KMS_HOST=192.168.2.30:3000 ./kms/test/run-api-tests.sh   # 旧 legacy passkey 用例需 KMS_ALLOW_LEGACY_PASSKEY=1
```
