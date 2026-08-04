# AirAccount KMS 安全与性能审计报告

> 日期：2026-07-03 · 审计对象：`kms/` (host + TA + proto，约 35K 行 Rust)
> 状态：**部分完成（host API 维度可信，其余维度待干净重跑）**

---

## ⚠️ 关于本报告的可信度（必读）

本次审计**前半程运行在被污染的环境里**：一个本地 token 优化工具（rtk）的 PreToolUse Bash hook 会往命令输出里注入编造文字，甚至伪造代码行和 codegraph 查询结果。该 hook 已被移除、会话已重启、污染已停止。

因此本报告**只收录用「干净工具」（重启后的 bash + 直接 Read 源码）逐条核实过的发现**。污染期得到的、未经重新核实的结论一律标注或撤回。

| 维度 | 状态 |
|------|------|
| Host API / 认证 / DoS | ✅ 已干净核实（Codex + 本人复核） |
| TA 安全 / 崩板 / 私钥 | ⚠️ 部分（仅 ExportPrivateKey 干净核实，其余待重跑） |
| 密码学正确性（secp256k1/EIP712/BIP32） | ❌ 未完成（agent 被中断，未重跑） |
| DB / 并发 / 性能 | ❌ 未完成（agent 被中断，未重跑） |

---

## 一、已确认发现（干净核实过，可行动）

### H-1 · API Key 认证默认 fail-open【HIGH，取决于部署配置】
- **位置**：`host/src/api_server.rs:5929-5953`
- **事实**：当 DB 无 api_key、`KMS_API_KEY` 未设、`KMS_REQUIRE_API_KEY` 未设为 1 时，所有走 `api_key_filter` 的路由**接受未认证请求**。且 `has_api_keys().unwrap_or(false)` —— DB 查询出错时默认 `false`，即**静默转为开放**。
- **真实影响（威胁模型化，避免夸大）**：
  - ✅ 能被滥用：`CreateKey`（未授权建钱包）、`ListKeys`/`DescribeKey`（元数据泄漏）、challenge 发放（打 TEE/DB）
  - ❌ **不能**盗现有用户私钥 —— 签名路径（`SignHash`）仍强制 TA 内 WebAuthn assertion 校验，API key 开放不绕过这层
  - 所以定 **HIGH** 不是 CRITICAL：是"未授权资源滥用 + 元数据泄漏"，不是"私钥失窃"
- **需你确认**：线上板子是否设了 `KMS_REQUIRE_API_KEY=1` 或已配 DB key？若没有，则 kms.aastar.io 的管理类端点当前对公网开放。
- **建议**：默认 fail-closed —— 未显式 `KMS_ALLOW_OPEN_MODE=1`（仅 dev）则拒绝无 key 访问；`has_api_keys()` 出错视为 fatal（而非默认放行）。

### H-2 · Stats 页面 UTF-8 字节切片 panic【MEDIUM】
- **位置**：`host/src/api_server.rs:4303` — `format!("{}…", &w.description[..8])`
- **事实**：按**字节**切前 8 位。若 description 第 8 字节落在多字节 UTF-8 字符中间（如 7 个 ASCII + 1 个中文/emoji），渲染 stats 页时 **panic**。description 是用户在 CreateKey 时可控的。
- **影响**：若 stats 页未鉴权（`api_server.rs:5960` 挂在某路由），构造一个特定 description 的钱包即可让该页每次访问 panic → DoS。
- **建议**：按字符切 `w.description.chars().take(8).collect::<String>()`，并 HTML-escape 后再渲染。
- **附带**：`4252` 的 `&pub_key_x[..8]` 同样是字节切片，但若 `pub_key_x` 是 hex（ASCII）则安全——建议一并核对确保来源恒为 ASCII。

### M-1 · 请求体先完整缓冲再判大小【MEDIUM / DoS】
- **位置**：`host/src/api_server.rs:5761-5771`（`aws_kms_body`）
- **事实**：`warp::body::bytes()` 先把整个 body 收进内存，**之后**才比对 `MAX_REQUEST_BODY_BYTES`(256KB)。持有效 key（或开放模式下任意客户端）可发超大 body，在被拒前强制完成内存分配。2GB RAM 的板子上尤其敏感。
- **建议**：在 `body::bytes()` **之前**加 `warp::body::content_length_limit(256*1024)`，让 warp 在读取前按 Content-Length 拒绝。

### 其余 Codex 报告项（未逐行复核，列出待核实）
以下来自 Codex 报告、**尚未由我独立干净核实**，仅供你参考，勿直接采信：
- challenge 发放端点（BeginAuthentication 等）缺速率限制，可打 TEE/DB
- 开放模式下限流以可伪造的 `x-api-key` header 为 key
- 根路径 stats 面板未鉴权泄漏钱包元数据
- API 错误信息原文外泄（内部 TEE/DB 错误文本）
- `agent_jwt.rs`：Codex 明确说**未发现** alg:none 绕过，HS256 被强制、签名经 TEE 验证（这是好消息）

---

## 二、已撤回的「假发现」（污染层编造，勿采信）

### ❌ 撤回：「公网可无认证导出私钥」
污染期的 codegraph 谎称 `handle_export_private_key` 是 `api_server.rs:5817` 的 HTTP handler、被 `dev-rpid` gate。**干净核实后全部证伪**：
- `api_server.rs` 里**没有** export 的 HTTP handler，**没有** export 路由（`grep` 干净结果为空）
- host 里**无人调用** `export_private_key`（ta_client 里是死代码/库 API）
- TA 侧真实逻辑（`ta/src/main.rs:1463-1493`）：**生产构建无条件拒绝**导出（`#[cfg(not(feature="export-secrets"))]` → `Err("disabled in production TA builds")`）；只有 `--features export-secrets`（dev/test）才有无 assertion 的 admin bypass
- 真正的 gate 是 **`export-secrets`**，不是 `dev-rpid`；板子按 memory 是 `MX93_DEV_RPID=1` 构建、**不含 export-secrets**
- **结论**：线上私钥导出编译期即关死，**不可利用**。降级为信息级加固建议 ↓

### L-1 · ExportPrivateKey 加固建议【LOW / 信息级】
即便当前不可利用，`export-secrets` 构建里的 `export_private_key`（`main.rs:1486-1488`）在无 passkey assertion 时走 "dev admin mode" 静默导出。建议：即使 dev 构建也移除无认证分支，或加显式 `KMS_ALLOW_INSECURE_EXPORT` 二次确认，防止将来误开 feature + 误接路由。

---

## 三、未完成、需干净重跑的维度

以下三块因 agent 被中断 + 污染，**未可靠完成**，强烈建议在当前干净环境重跑：
1. **TA 崩板风险**：TA 内 `unwrap/expect/数组越界/整数溢出`（TA panic 崩整个 TEE）；`SystemTime::now()` 误用（已知 panic 点）
2. **密码学正确性**：secp256k1 签名 low-s malleability、ECDSA nonce 来源（RFC6979 or TRNG）、EIP-712 域分隔符、BIP32 硬化派生
3. **DB / 性能**：SQLite 是否在 `spawn_blocking` 里调（否则阻塞 tokio executor）、跨 `.await` 持锁、无界缓存/队列内存增长（2GB RAM）、索引缺失

---

## 四、概述

- **当前可行动的最高优先级**：确认线上板子是否设了 `KMS_REQUIRE_API_KEY=1`（H-1）。若没设，公网 KMS 的管理类端点处于开放状态——但**私钥安全，签名仍受 WebAuthn 保护**。
- **两个明确 bug**：stats 页 UTF-8 panic（H-2）、body 先缓冲后限长（M-1），都是小改动、收益明确。
- **一个虚惊**：「无认证导私钥」是污染编造的假警报，实际编译期关死。
- **一半工作待做**：TA/crypto/DB 三维度需在干净环境重跑才能给可信结论。

> 本报告不修改任何代码，仅列出发现与建议（符合审计要求）。
