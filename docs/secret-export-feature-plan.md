# Secret Export Dev Feature Plan

> Created: 2026-06-04
> Tracking issue: <https://github.com/AAStarCommunity/AirAccount/issues/29>

## Goal

开发调试阶段允许显式导出 mnemonic/private key，便于验证地址派生、链上签名和回归测试；生产发布阶段必须保证 mnemonic 和私钥明文不能跨出 TEE。安全边界应落在 TA 编译产物上，而不是只靠 host CLI 或 HTTP API 隐藏入口。

## Current State

仓库已经有部分 feature-gate：

- `kms/ta/Cargo.toml` 定义了 `export-secrets`，注释为 dev/test only。
- `kms/ta/src/main.rs` 的 `create_wallet()` 已经按 `export-secrets` 控制 `CreateWalletOutput.mnemonic`：生产构建返回空字符串。
- `kms/host/Cargo.toml` 的 `export_key` binary 已设置 `required-features = ["export-secrets"]`。
- `kms/host/src/api_server.rs` 的 CreateKey 返回固定占位值 `[MNEMONIC_IN_SECURE_WORLD]`。

主要缺口：

- 生产 TA 仍然编译 `ExportPrivateKey` 命令处理函数，只是要求 passkey；这仍然违反“私钥永不出 TEE”的生产目标。
- `Wallet::export_private_key()` 是普通方法，生产路径也能调用。当前签名路径内部用它取出派生私钥再签名，这在 TA 内部可接受，但命名和 API 边界容易被误用。
- 发布脚本中仍有复制 `export_key` 的路径，需要确认生产脚本不会构建或分发这个 binary。
- CI 还没有检查生产构建中禁止 `export-secrets`，也没有静态检查生产 TA 是否拒绝 `ExportPrivateKey`。

## Proposed Design

### 1. TA 端强隔离

`ExportPrivateKey` 命令只在 `export-secrets` feature 下可用：

```rust
#[cfg(feature = "export-secrets")]
fn export_private_key(...) -> Result<proto::ExportPrivateKeyOutput> {
    ...
}

#[cfg(not(feature = "export-secrets"))]
fn export_private_key(...) -> Result<proto::ExportPrivateKeyOutput> {
    Err(anyhow!("ExportPrivateKey is disabled in production TA builds"))
}
```

生产构建中，即使 host 直接调用 command id 7，TA 也必须返回错误，不返回任何私钥材料。

### 2. 区分“TEE 内部签名用私钥”和“导出私钥”

把 `Wallet::export_private_key()` 改名或拆分为两个语义：

- `derive_signing_key()`：TA 内部签名使用，返回值只在 TA 内存中使用。
- `export_private_key_for_debug()`：只在 `export-secrets` feature 下编译，用于调试导出。

这样生产代码仍可完成 `SignHash`、`SignTypedData`、`SignAgentUserOp`、`SignGrantSession` 等签名，但没有公开的“导出”语义。

### 3. Host/CLI 端只保留开发入口

保留 `kms/host` 的 `export-secrets` feature 和 `export_key` binary，但进一步要求：

- 默认构建不包含 `export_key`。
- `kms-api-server` 不暴露 private key export HTTP endpoint。
- 开发文档中明确调试命令必须带 `--features export-secrets`。
- 生产部署脚本不得复制 `export_key`。

### 4. CI 和发布检查

新增 release guard：

- CI 检查 `kms/host` 默认 feature 下 `export_key` 不可构建。
- CI 或脚本检查生产构建命令不包含 `--features export-secrets`。
- 添加一条文本/源码检查：生产 TA 对 `Command::ExportPrivateKey` 的处理必须返回 disabled error。
- 发布前检查产物目录中没有 `export_key` binary。

### 5. API/Proto 兼容策略

短期保留 `CreateWalletOutput.mnemonic: String` 和 `ExportPrivateKey` command id，避免破坏协议兼容；生产构建返回空 mnemonic，私钥导出命令返回 disabled error。后续大版本可以把 mnemonic 字段替换成 `secret_exported: bool` 或从 output 中移除。

## Implementation Steps

1. 修改 `kms/ta/src/main.rs`：让 `ExportPrivateKey` 命令在非 `export-secrets` 构建下直接拒绝。
2. 修改 `kms/ta/src/wallet.rs`：拆分或重命名私钥派生方法，避免生产路径调用名为 export 的方法。
3. 修改签名函数调用点：TA 内部签名改用 `derive_signing_key()`。
4. 检查部署脚本：删除生产脚本中复制 `export_key` 的步骤，或用 dev flag 控制。
5. 增加测试/CI：覆盖生产构建禁用导出、开发构建允许导出。
6. 更新 API 文档：明确 mnemonic/private key export 仅限 dev/test feature，生产发布禁止。

## Acceptance Criteria

- 默认/生产 TA 构建中，`CreateWalletOutput.mnemonic` 为空。
- 默认/生产 TA 构建中，调用 `ExportPrivateKey` 返回明确错误，不返回私钥。
- 只有显式启用 `export-secrets` 的 TA 和 host 才能使用 `export_key` 调试工具。
- 生产部署产物中没有 `export_key` binary。
- 文档和 CI 均能防止误把 `export-secrets` 带入发布流程。
