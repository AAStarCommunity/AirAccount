# 威胁模型：当运行 KMS 的 CA 作恶时，私钥和签名还安全吗？

> 创建时间：2026-06-14
> 文档性质：威胁模型 / 对抗性分析 / 用户问答
> 适用版本：AirAccount KMS（Beta3 开发期，v0.20.0 之后）
> 配套阅读：`docs/design/37-remote-attestation-research.md`、`docs/design/37-remote-attestation-design.md`
> 补充：docs/dKMS.md 已有去中心化 KMS 草案(BLS+链上策略+slash),

---

## 0. 这篇文档回答一个尖锐的问题

很多人第一次听说 AirAccount 时会问同一句话：

> **「钱包是 AAStar 部署运营的。如果 AAStar 自己变坏了，或者运行 KMS 的那台服务器被黑客控制了，它能不能偷偷用我的私钥签一笔我没批准的交易？或者伪造一个看起来合法的签名骗过别人？」**

这是一个**完全正确、必须正面回答**的问题。本文不回避、不美化，逐个攻击手法拆开讲：**哪些已经防住了、哪些发现了但还在补、哪些目前确实还没完全堵死、堵不死的地方又拿什么兜底。**

为了让普通用户和技术人员都能看懂，每个攻击向量都用统一的**四段式**展开：

```
① 风险是什么（用大白话 + 类比讲清攻击者想干嘛）
② 技术上怎么表现（讲透机制、背景知识、攻击者的具体操作和前提假设）
③ 现在的机制怎么防（我们部署了什么）
④ 为什么能防住（给出代码/密码学/原理层面的凭证）
```

每个向量末尾会诚实标注：**✅ 已防住 / ⚠️ 部分缓解 / ❌ 还没完全防住（正在补）**。

---

## 1. 先认清牌桌上有谁（信任模型）

要谈「谁能作恶」，先得说清系统里有哪几个角色，以及**我们对每个角色信任到什么程度**。

```
   ┌──────────────┐        ┌────────────────────────────────────┐
   │  用户设备     │        │      AAStar 部署的服务器 (一台机器)    │
   │              │        │  ┌──────────────┐   ┌─────────────┐  │
   │ passkey 私钥  │        │  │   CA          │   │   TEE / TA   │  │
   │ (FIDO2,锁在   │ HTTPS  │  │ kms-api-server│──>│  OP-TEE      │  │
   │  安全芯片里,  │<──────>│  │ (普通世界)    │   │  安全世界     │  │
   │  谁都拿不到)  │        │  │ ⚠️ 不完全可信  │   │  私钥在这里面 │  │
   └──────────────┘        │  └──────────────┘   │  ✅ 信任根    │  │
        ▲                  │      恶意方在这       └─────────────┘  │
        │                  └────────────────────────────────────┘
   ┌────┴────────┐
   │  客户端/验签方 │  发请求、最后验证签名对不对
   └─────────────┘
```

| 角色 | 是什么 | 我们信任它吗？ |
|---|---|---|
| **CA**（kms-api-server） | 跑在普通操作系统里的 KMS 服务进程，AAStar 部署。负责收 HTTP 请求、转发给 TEE。 | **⚠️ 不完全可信**——本文假设它**就是坏的**。这是整篇文档的攻击者。 |
| **TEE / TA** | OP-TEE 的可信应用（Trusted Application），跑在 i.MX93 芯片的「安全世界」（TrustZone）里。**私钥在它里面，永不出 TEE。** | **✅ 信任根**——我们的安全建立在「安全世界和普通世界硬件隔离」之上。 |
| **用户 passkey** | FIDO2/WebAuthn 凭证，私钥锁在**用户自己设备**的安全芯片（secure enclave）里。 | **✅ 用户独占**——连 AAStar、连用户自己都导不出这把私钥。 |
| **客户端 / 验签方** | 发请求、最后验证签名的一方（可能是用户的 App，也可能是依赖方如 SuperPaymaster）。 | 中立，按规则验。 |

**核心安全哲学**：AirAccount 是「半去中心化」的——代码开源可 fork，passkey 锚定在 FIDO2 的 rpId 域名上，没有 admin 后门。在这个模型里，**CA 是那个「我们故意不完全信任」的角色**。安全设计的目标就是：**即使 CA 整个被攻陷，攻击者也偷不到私钥、签不了用户没批准的东西。**

> 一个贯穿全文的关键认知：**「谁来验证」比「谁来执行」更重要。** CA 负责传话，但真正的安全检查（验 passkey、验 challenge、验 JWT）我们尽量都搬进了 **TEE 内部**——因为 CA 可能撒谎，TEE 不会。

---

## 2. 攻击向量逐个拆解

### 向量 1：CA 跳过 passkey 验证，直接命令 TEE 签名 ✅ 已防住

**① 风险是什么（大白话）**
最直接的攻击：CA 是传话人，用户的签名请求都要经过它。那 CA 能不能**干脆不验用户指纹/passkey，自己捏造一个「用户已授权」的请求**直接丢给 TEE，让 TEE 乖乖签？

类比：你去银行取钱要刷脸。如果「刷脸这一步」是由柜员（CA）在他自己的电脑上做的，那柜员完全可以假装「他刷过脸了」然后把钱给坏人。**验证由谁做、在哪做，是整个安全的命门。**

**② 技术上怎么表现（讲透）**
WebAuthn/FIDO2 的认证流程会产生一个 **assertion**（断言）：用户在自己设备上用指纹/PIN 解锁，设备的安全芯片用 **passkey 私钥**对一段数据做 ECDSA 签名，证明「这个用户本人此刻在场并同意」。

> **背景知识 1：FIDO2 私钥为什么 CA 拿不到？**
> passkey 的私钥在用户设备出厂时就被关在「安全芯片（secure enclave / TPM）」里，**设计上不可导出**——连用户本人、连操作系统都读不出来，只能「请芯片用它签个名」。所以攻击者想伪造一个合法 assertion，必须拿到这把私钥，而这把私钥根本不在服务器上、不在 CA 手里。

> **背景知识 2：ECDSA 验签的原理（浅显版）**
> 椭圆曲线签名是「私钥签、公钥验」的非对称密码。用户注册时把**公钥**交给系统存着；之后每次签名，验证方用这把公钥就能确认「这个签名确实是对应私钥签的」，但**反推不出私钥**。所以只要验证方手里有正确的公钥、并且亲自做验签，就没人能伪造。

攻击的关键在于：**这个 assertion 到底是谁来验的？** 如果验证发生在 CA（普通世界），CA 当然可以跳过；但如果验证发生在 **TEE 内部**，CA 就插不上手。

**③ 现在的机制怎么防**
**C-1 修复**把 WebAuthn assertion 的验证**下沉到了 TA 内部（TEE 里）**。TA 拿到请求后，自己独立做三件事：
1. 校验 assertion 里的 **rpId hash** 等于本系统注册时锚定的域名 hash（确保是给本系统的，不是别处骗来的）；
2. 校验 **User Presence（UP）标志位**确实被设置（证明用户本人在场操作）；
3. 用注册时存下的**用户 passkey 公钥**对 assertion 做 **ECDSA 验签**。

**④ 为什么能防住（凭证）**
代码在 `kms/ta/src/main.rs` 的 `verify_passkey_for_wallet()` 里，**全部跑在 TA（TEE）内**：

```rust
// rpId hash 比对（常数时间，防侧信道）—— authenticator_data[0..32]
let actual_rp_id_hash = &_assertion.authenticator_data[0..32];
// ... 逐字节异或比对 EXPECTED_RP_ID_HASH ...

// User Presence (UP) flag —— flags 字节 bit0
let flags = _assertion.authenticator_data[32];
if flags & 0x01 == 0 {
    // "WebAuthn User Presence flag not set" → 拒绝
}

// 用钱包注册的 passkey 公钥做 P-256 ECDSA 验签（p256-m，C 实现，TA 内）
p256_ecdsa_verify(sig, pubkey, hash, hlen)
```

> 这是双层防御：CA 侧会用 Rust p256 crate 先验一遍（快速过滤），**TA 内再用 p256-m 独立验一遍作为纵深防御**（见代码注释 "Two-layer defense"）。**CA 那一层验不验都无所谓——TA 自己会验。**

**为什么这就堵死了攻击**：CA 即使跳过自己那层、或捏造一个「已授权」请求，**TA 会自己重新验 passkey**。要骗过 TA，攻击者必须拿出一个能通过 ECDSA 验签的合法 assertion——而这需要用户的 passkey 私钥，**那把私钥锁在用户设备的安全芯片里，CA 永远拿不到**。

**结论：✅ 已防住。** 信任边界画在 TEE 上，验证在 TEE 内做，CA 无法绕过。

---

### 向量 2：CA 重放用户的合法 assertion，去签任意东西 ✅ 已防住（strict 模式下）

**① 风险是什么（大白话）**
向量 1 说 CA 伪造不出 assertion。但 CA 是传话人，它能**截留**用户某一次的合法 assertion（一张「用户同意」的有效凭证），然后**反复使用**它，去签很多笔用户根本没批准的操作。

类比：你签了一张支票忘了写金额，传递人偷偷复印了很多张，每张填上不同金额到处用。

**② 技术上怎么表现（讲透）**
原始的 WebAuthn assertion 如果**没有绑定「一次性凭据」**，那它就是一张可以被复用的「通行证」。CA 只要保存一份合法 assertion，就能在后续任意 `Sign` 请求里反复附带它，TA 每次看到「assertion 验签通过」就放行——这叫**重放攻击（replay attack）**。

> **背景知识：怎么防重放？用一次性的 nonce（随机数）。**
> 标准做法是「挑战-应答」：验证方先发一个**随机的、一次性的 challenge（nonce）**，要求对方把这个 nonce 包进签名内容里，并且**用完即焚**。这样旧的签名因为带的是旧 nonce，再拿来就对不上了。

**③ 现在的机制怎么防（机制 #49 challenge binding）**
TA 引入了一次性 nonce 绑定：
1. 签名前，客户端先调 **`GetChallenge`**，TA 生成一个 **32 字节随机 nonce**，记在 TA 内部的待验表里（带签发时间）；
2. 用户做 WebAuthn 认证时，这个 nonce 必须出现在 assertion 的 **clientDataJSON 的 `challenge` 字段**里；
3. TA 收到签名请求后，**自己验**：`SHA-256(clientDataJSON) == client_data_hash`（确保 JSON 没被篡改）→ 取出 challenge → 和 TA 待验表里的 nonce **常数时间比对** → 检查**未过期** → **用后即焚**（consume）。

重放旧 assertion → 它带的 nonce 已经被消费掉了（或对不上当前的）→ **拒绝**。

**④ 为什么能防住（凭证）**
`kms/ta/src/main.rs` 的 `verify_challenge_binding()`：

```rust
// (1) clientDataJSON 必须哈希到 client_data_hash（防 CA 伪造 JSON）
let computed = sha2::Sha256::digest(client_data_json);
// 常数时间比对 == assertion.client_data_hash ...

// (2) 取出 challenge 字段，base64url 解码
// (3) PEEK（先不消费）TA 待验表里这个钱包的 nonce
let (nonce, issued_at) = challenge_peek(wallet_id).ok_or_else(|| {
    anyhow!("No pending challenge ... (replay, expired, or GetChallenge not called)")
})?;
// 常数时间比对 challenge_bytes == nonce，不等就拒
// 检查 age <= CHALLENGE_TTL_SECS（新鲜性）
// 全过 → challenge_consume(wallet_id)  // 用后即焚，严格一次性
```

> 一个细节体现了设计的严谨：先 `peek`（窥视）再 `consume`（消费）——只有所有检查都通过才真正烧掉 nonce。这样攻击者拿一个错的 challenge 来，**烧不掉受害者那把还有效的 nonce**（防「针对 nonce 的 DoS」）。

**⚠️ 但必须讲透：过渡模式（transition）vs 严格模式（strict）—— 这是 issue #63 的核心**

这里有一个**目前真实存在的后门**，必须诚实说明。代码里有个开关：

```rust
// kms/ta/src/main.rs:306
const ENFORCE_TA_CHALLENGE: bool = false;  // 当前 = false = 过渡模式
```

- **为什么有过渡模式？** #49 是**新机制**。老客户端还没升级到「调 GetChallenge + 带 clientDataJSON + nonce」的新流程。如果一刀切强制要求，所有老客户端会**全部失效**。所以设了过渡期：
  - 带 `clientDataJSON` 的 assertion → **严格验 nonce**（有防重放）✅
  - **不带** `clientDataJSON` 的 assertion → 走 **legacy ECDSA-only 路径（只验签，不验 nonce）**，打个警告就放行 ⚠️**无防重放**
- **风险**：过渡模式留了一个 legacy 后门。**一个作恶的 CA 可以故意走这条「不带 clientDataJSON」的老路径，从而重放老的 assertion**——因为这条路径压根不检查 nonce。
- **strict 模式（`= true`）**：强制**所有** assertion 都必须带 clientDataJSON 并通过 nonce 验证，**关掉 legacy 后门**。

> **类比**：过渡期 = 公司换门禁，新卡（带防复制芯片）和旧磁卡并行，老员工还能刷没防复制的旧磁卡。strict = 旧磁卡全部作废，只认新卡。

**🔑 mainnet 前必须 flip 到 strict（#63 的一部分）**：在真金白银上线前，必须把 `ENFORCE_TA_CHALLENGE` 翻成 `true` + **重新编译/刷写 TA** + **确保所有客户端都已升级**到带 clientDataJSON 的流程。否则可重放后门一直敞着。

**结论：✅ 已防住（strict 模式下）。** 机制完整、代码到位；但**当前默认是过渡模式，留了 legacy 可重放后门，mainnet 前必须 flip strict（#63）**。

---

### 向量 3：CA 伪造 agent JWT 骗取签名 ✅ 已防住

**① 风险是什么（大白话）**
AirAccount 支持「代理签名」：用户可以主动创建一个 **agent key**，发一个有时效、有范围限制的「授权令牌（JWT）」，让某个自动化程序（agent）在限定范围内替自己签名（比如自动续费、定投）。问题来了：CA 能不能**自己捏一个假 JWT**，假装「用户授权了某个 agent」，从而调动签名？

**② 技术上怎么表现（讲透）**
> **背景知识：JWT 是什么？**
> JWT（JSON Web Token）是一段「带签名的授权声明」，里面写着 `sub`（主体）、`wallet_id`、`agent_index`、`iat`（签发时间）、`exp`（过期时间）等 claim（声明）。它的安全性来自**签名**：只有持有签发密钥的一方才能造出合法 JWT，验证方用对应密钥能验真伪。

> **背景知识：HMAC 对称签名为什么 CA 伪造不了？**
> 这里用的是 **HMAC-SHA256**（对称签名）：签发和验证用**同一把密钥**。关键在于——**这把 HMAC 密钥生成、保存、签发、验证全在 TA（TEE）内部**，CA 从头到尾**接触不到这把密钥**。没有密钥，就造不出能通过验证的 JWT。

**③ 现在的机制怎么防（H-3/4/5 + #15）**
agent JWT 由 **TA 内的 HMAC 密钥签发，也由 TA 内验证**。CA 全程只是搬运 JWT 字符串，碰不到密钥。TA 在签名前会校验：
- JWT 的 HMAC 签名有效（密钥在 TEE，CA 伪造不了）；
- `wallet_id` / `agent_index` 这些 claim **匹配本次请求的参数**（防 CA 张冠李戴）；
- 用 **TEE 侧可信时钟** `tee_unix_secs()` 校验 `exp`，**过期即拒**（#15）。

**④ 为什么能防住（凭证）**
`kms/ta/src/main.rs`：

```rust
const JWT_SECRET_STORE_ID: &str = "jwt_hmac";   // HMAC 密钥存 TA secure storage
type HmacSha256 = Hmac<Sha256>;

// 签发：iat/exp 由 TA 自己算并写进 HMAC 签名的 payload，CA 注入不了
// (注释原文: host ... still cannot inject `iat`/`exp` into the HMAC-signed JWT payload directly
//            — the TA computes and [signs them])

// 验证 verify_jwt_wallet_claims()：
if jwt_agent_index_u64 as u32 != expected_agent_index {
    return Err(anyhow!("JWT agent_index claim does not match request"));
}
// exp/iat 结构性检查 + TTL 上限
if exp <= iat || exp.saturating_sub(iat) > MAX_AGENT_JWT_TTL as u64 { ... }
// #15: 用 TEE 可信时钟 tee_unix_secs() 做运行时过期检查
```

**为什么这就堵死了**：HMAC 密钥**从不离开 TEE**。CA 想伪造 JWT，需要这把密钥——拿不到。改 claim？签名立刻不匹配。用过期 JWT？TA 用自己的可信时钟拒掉。

**结论：✅ 已防住。** 签发与验证都在 TEE 内，密钥不出 TEE。

---

### 向量 3b：CA 窃取一个用户合法的 JWT 来用 ⚠️ 部分缓解（trade-off，非漏洞）

**① 风险是什么（大白话）**
CA 伪造不了 JWT（向量 3），但它能**偷**——用户合法发出的 JWT 经过 CA 时，CA 把它存下来。在这个 JWT 过期前，CA 能不能拿它去干坏事？

**② 技术上怎么表现（讲透）**
这是所有 **bearer token（持有即有效令牌）** 的固有特性：**谁持有 token，谁就能在它的有效范围内使用它。** 这不是 AirAccount 特有的缺陷，是 OAuth、API key 等所有 bearer 机制共有的。窃取者能用这个 JWT 的范围 = 它的 **scope（限定的 derivation path / agent）** + **exp（有效期）**。

**③ 现在的机制怎么防**
- **scope 限制**：agent JWT 只能驱动它绑定的那个 agent 的派生路径（`m/44'/60'/0'/1/<index>`），不能签主账户、不能签别的 agent。
- **exp 限制**：JWT 有 TTL 上限（`MAX_AGENT_JWT_TTL`），过期自动失效。
- **敏感度本身就低**：agent JWT 是**用户主动创建、主动授权的「代理签名」**，本就授权给一个自动化程序去用——它的权限天然小于「用户本人 passkey 直接授权」。

**④ 为什么这是 trade-off 而不是漏洞**
代理签名的本质就是「我授权一个程序在限定范围内替我签」。一旦你引入「代理」，被代理的令牌就**必然**可被持有它的环境使用——这是功能定义本身，不是实现 bug。我们能做的是**用 scope + exp 把滥用范围压到最小**，并让用户清楚：agent key 是低敏感度的便利功能，**真正高价值的操作应该走 passkey 本人授权**（向量 1/2 覆盖）。

**结论：⚠️ 部分缓解。** bearer token 固有特性，靠 scope + exp 限制滥用面，敏感度本就低于 passkey。诚实地说，这是设计权衡，不是可以「彻底防住」的东西。

---

### 向量 4：CA 偷换签名内容（payload） ❌ 真实开放风险（正在补：#68）

**① 风险是什么（大白话）**
这是比重放更阴险的一招。用户在自己设备上看到「授权交易 A」，做了指纹认证。CA 拿到这次合法的认证后，**不去签交易 A，而是偷偷把内容换成交易 B**（比如「转 100 给商家」换成「转全部余额给攻击者」），再让 TEE 签 B。

类比：你对着摄像头说「我同意」，但你同意的是 A 合同；坏人把你的「我同意」录音剪到了 B 合同上。

**② 技术上怎么表现（讲透 —— 这是关键）**
向量 2 的 nonce 防住了「重放」，但**防不住「偷换内容」**，原因在于一个微妙但致命的点：

> **WebAuthn 的 assertion 只证明「用户对某个 challenge 在场点了头」，它并不包含业务 payload（要签的交易内容）。**
> 而 #49 的 nonce 是**纯随机数**，它只保证「这次认证是新鲜的、一次性的」，**它并不和要签的交易内容绑定**。

于是攻击成立：用户做了一次认证，产生 `(nonce, assertion)`。CA 拿着这对合法凭据，去调 `Sign(交易B, assertion)`。TA 一看：assertion 验签通过 ✅、nonce 匹配且新鲜 ✅ → **签 B**。

**这比重放更深**：重放是「同一个授权被用多次」；偷换是「**一次合法认证，能签 CA 任意挑选的内容**」。因为用户「同意」的那个动作，从密码学上根本没和「同意的具体内容」绑在一起。

**③ 正在加的机制怎么防（payload-bound challenge，issue #68）**
正在开发的 **payload-bound challenge** 把「内容」绑进「同意」：
1. 客户端调 `GetChallenge` 时，**把要签的 payload 的 hash 一起传进去**；
2. TA 把这个 nonce 和 `payload_hash` **绑定**记录；
3. 用户认证产生 assertion；
4. TA 在真正签名前，**校验「这次 Sign 请求的实际 payload」的 hash == 当初这个 challenge 绑定的 payload hash**。不等就拒。

这样，CA 想偷换成交易 B，B 的 hash 对不上 challenge 绑的 A 的 hash → **拒绝**。「用户同意的内容」和「实际签的内容」被密码学锁死。

**④ 为什么现在还没防住**
- 现状：`GetChallenge` 生成的 nonce **是纯随机的，不绑 payload**（见向量 2 的 `challenge_issue()`——只生成随机 nonce，不接受也不记录 payload hash）。
- 所以 TA 目前**没有任何机制**校验「实际签的内容 == 用户同意的内容」。
- 这是一个**已发现、已确认、但尚未实现修复**的真实开放风险。

**结论：❌ 还没完全防住。** 已定位、已有明确方案（payload-bound challenge / #68），**修复正在进行中**。在 #68 落地前，这是 passkey 授权路径上一个真实存在的缺口。

---

### 向量 5：CA 根本不用真 TEE / 伪造整个 TEE ❌ 最根本的开放风险（未闭合）

**① 风险是什么（大白话）**
前面所有防御都建立在一个前提上：「CA 把请求转给了**真正的 TEE**」。但如果 CA **压根不调用真 TA**，而是自己跑一个**假冒的「TA」程序**，假装它是 TEE，里面既不验 passkey 也不绑 nonce，私钥还可能就明文存在普通内存里——那前面所有的「TEE 内验证」都是空的。

类比：你以为你的钱锁在银行的金库里（TEE），但其实柜员（CA）把金库门画在了墙上，钱就堆在他抽屉里。**客户端凭什么相信「签名真的来自一个真 TEE」？**

**② 技术上怎么表现（讲透）**
客户端拿到一个签名，它怎么知道这个签名是「真 OP-TEE + 真 i.MX93 芯片」产生的，而不是「CA 在普通 Linux 进程里用软件库签的」？**目前它没法知道。** 签名本身（ECDSA / secp256k1）从数学上看，真 TEE 签的和假 TEE 签的**长得一模一样**——签名不携带「我来自哪里」的硬件证明。

这正是 **#37 远程证明（remote attestation）** 要解决的问题：让客户端能**密码学地验证**「这个签名响应确实来自一台真实 NXP i.MX93、跑着未篡改的 OP-TEE、加载的是我们公开发布的那个 KMS TA 二进制」。

**③ 现在的机制 / 为什么还没防住——卡在 R-1（信任根锚定）**
远程证明的标准模型（RATS / RFC 9334）需要一条**证书链**，最终锚定到**硬件厂商的根 CA**（NXP），这样客户端才能验「这把签名密钥确实出自真 NXP 芯片」。AirAccount 做远程证明卡在了这条链的**第一环 R-1**，这是已经用一手官方资料核对过的硬结论：

- **OP-TEE 自带的 attestation 密钥是「设备自生成、零锚定」的**：经 OP-TEE 官方源码 `core/pta/attestation.c` 逐行核对 [官方源确证]——这把 key 是设备首次使用时自己生成的（`generate_key()` → `crypto_acipher_gen_rsa_key()`），**没有证书链、没有厂商 CA、不连任何硬件根**。它只能证明「同一个 TEE 实例前后一致」，**无法向一个从没见过这台设备的客户端证明「这是真 NXP 芯片」**——客户端凭什么信那把自签公钥？

- **NXP ELE 能给硬件签名的设备身份，但「连不连 NXP 根」官方手册沉默**：ELE 的 `hsm_dev_attest`（RM00284 §3.12 [官方手册确证]）能输出硬件签名的设备身份（芯片唯一 ID `uid`、安全启动根 hash `oem_srkh`、固件度量 `sha_fw`、lifecycle `lmda_val`），ECDSA P-384 签名，实机已跑通（`hsm_dev_attest exchange Passed`）。**但是**——RM00284 §3.12 的「Detailed description」一节**完全是空的**，对「这把签名密钥到底是不是 NXP 工厂注入的、有没有一条能被第三方离线验证到 NXP 根 CA 的证书链」**只字未提**。

> **这就是 R-1（一手核对结论，诚实标注）**：即便通读了整本 RM00284，也无法确认 ELE 设备签名密钥是否连 NXP 根。这是 API 参考手册的固有边界（不含 key provisioning / 信任根架构）。必须另查 **NXP 安全参考手册 / EdgeLock 2GO / NDA 材料 / 实机逆向**才能收口。详见 `docs/design/37-remote-attestation-design.md` §0、§7（R-1）。

**④ 后果**
R-1 不闭合 → **目前客户端无法从密码学上证明「CA 用的是真 TEE」**。一个作恶的 CA 理论上可以跑一个假 TEE 自签，而客户端**分不出真假**（因为连真 TEE 自己的 attestation key 也只是自签的，没有 NXP 根背书）。

**这是整个威胁模型里最根本、也最难闭合的开放风险**——因为它的信任根在 NXP 硬件，**不是软件能补的**。需要 **#37 Phase 0**（实机 + NXP 一手源 / EdgeLock 2GO / NDA）去收口 R-1。

**结论：⚠️ 半缓解（MVP 已落地），全闭合仍卡 R-1。** 这是「不信任部署方」这一最强目标能否成立的**唯一阻塞前提**。在 R-1 收口前，MVP 阶段的信任根只能暂时是「设备自签 / TOFU 登记表」——这是安全降级，不是去中心化优势（见 §4）。

---

### 向量 5 的进展：#37 远程证明 MVP 已实现（2026-06-14，真机验证）

上面写于 MVP 落地之前。现在 **#37 Phase 1 MVP 已在真机（NXP FRDM-IMX93）端到端打通**：KMS 多了一个 `GET /attestation?nonce=` 接口，TA 在安全世界里调 OP-TEE 的 attestation PTA，产出可被客户端密码学验证的「证据」。下面两张图，一张画清**现在做到的「半信任」**，一张画清**要到「全信任」还差哪一段**。

#### 图 A：MVP「半信任」——能证明「真 TEE + 跑的是这个 TA」，但信任根只到「首次见到即信」（TOFU）

```
客户端/验签方                KMS(CA,不可信)        OP-TEE 安全世界(信任根)
     │                          │                       │
     │ ① 我发一个随机数 nonce ──>│                       │
     │                          │ ② 原样转进去 ───────>│  attestation PTA:
     │                          │                       │   · 量出"此刻在跑的 TA 指纹"
     │                          │                       │   · 用设备密钥签(nonce+指纹)
     │                          │<──── ③ 证据 ──────────│
     │<──── ④ 证据 ─────────────│  {TA指纹, 签名, 设备公钥}
     │
     │ ⑤ 客户端本地自己验(不信 CA 自报):
     │     ✅ 签名对吗(用设备公钥验)?      → 确实在真 OP-TEE 里产生的
     │     ✅ TA指纹 == 我期望的版本?       → 跑的正是公开发布的那个 TA
     │     ✅ nonce 是我刚发的那个吗?       → 不是录像重放
     │     ⚠️ 但"设备公钥"本身可信吗? ◀──── 缺口在这一行
     │         它是 OP-TEE 自己生成的,没有任何"出生证明"
     │         客户端只能"第一次见到就记下来,以后比对是否同一把"(TOFU)
     │
   ✔ 能证明:「这是一个真 OP-TEE,跑着正确的 TA,且刚刚真的回应了我」
   ✘ 不能证明:「这是一块我从没信过、货真价实的 NXP 芯片」
                 (作恶 CA 理论上能拿另一台真 TEE / 自己生成的 key 冒充,
                  客户端首次见到时分不出——所以叫"半信任")
```

#### 图 B：要到「全信任」还差什么——补一条从设备密钥到 NXP 出厂根的证书链（这就是 R-1）

```
                              ┌──────────────────────────────┐
                              │  NXP 根 CA(芯片厂出厂即存在,    │ ← 客户端可内置,
                              │  全球公开)                     │   天然就信它
                              └───────────────┬──────────────┘
                                              │ 签发
              【缺口 R-1:要补的两段】 ┌────────▼──────────────────┐
                                     │ 这颗芯片的"出厂设备证书"     │ ← NXP 工厂注入
                                     │ 证明:此密钥确实出自真 NXP 芯片│
                                     └────────┬──────────────────┘
                                              │ 背书
┌─────────────────────────────┐     ┌────────▼──────────────────┐
│ 设备 attestation 公钥         │ ==> │ 还是同一把公钥,但现在有"出处" │
│ MVP 现状:自签,只能 TOFU 兜底  │     │ 全信任:能一路验到 NXP 根     │
└─────────────────────────────┘     └────────────────────────────┘

补法(任一条成立即可闭合 R-1):
  · 查 NXP 安全参考手册,确认 dev_attest 设备密钥连出厂根 + 拿到根证书;或
  · 用 EdgeLock 2GO 给芯片 provision 一张连 NXP 根的证书。

补上之后的区别:
  半信任(现在): 客户端第一次见到这台机器,只能"姑且信",事后靠比对一致兜底。
  全信任(目标): 客户端第一次见到就能验真——CA 想用假 TEE 蒙混,
                它拿不出"连到 NXP 根的证书",当场被拆穿,无需任何"首次信任"。
```

> 一句话总结这两张图：**MVP 已经把「真不真 TEE、对不对的 TA」变成客户端可以亲手验的事；唯一还要靠「首次信任」兜底的，是「这把设备密钥到底出自不出自真 NXP 芯片」——补上图 B 那条证书链（R-1），半信任就升级成全信任。**

---

## 3. DVT 二次验证：到底有没有用？（用户特别关心）

**DVT（Distributed Validator Technology / 分布式验证者）** = 引入**外部独立节点**（参考 YetAnotherAA-Validator 这类分布式验证），不只听 CA 一面之词。

> **重要：DVT 在 AirAccount 里其实有两个不同角色，价值定位不同、容易混淆，先分清：**
> - **角色 A —— 独立验证者**：对 passkey / attestation 做第二次独立核验，防「CA 作恶 / 假 TEE」（向量 1/4/5）。**绕不过 R-1**（见 §3.2）。下面 §3.1–3.3 论证这个角色。
> - **角色 B —— 独立 co-signer**：大额操作需 DVT 节点 BLS 共签 + DVT 有独立策略，防「owner key（passkey/TEE 私钥）被盗后，攻击者单凭它动大额」。这是一个**新的、独立的安全维度**，且**不卡 R-1**，见 §3.0。

### 3.0 角色 B：DVT 作为独立 co-signer（防 owner key 被盗）

**机制**：DVT = 一组独立链下验证节点（YetAnotherAA-Validator 那些），每个持自己的 BLS 私钥。大额操作时这些节点对操作 BLS 聚合共签，合约验证「≥门限个注册 DVT 节点签了」。**「DVT 因子」= 这笔操作拿到了 N 个独立 DVT 节点共签批准**，是独立于 owner key 的第二道闸门。

**解决什么**：owner key（passkey / TEE 私钥）可能被盗、被钓鱼、TEE 被攻破。单靠一把 owner key，被盗就全完。DVT 因子的意义：即使 owner key 被盗，攻击者还得让 DVT 节点也批准 —— 大额（>$1000）要 DVT 共签，给「owner key 单点被盗」加一道独立保险。

**⚠️ 三个必须说透的关键点（否则 DVT 是假保险）：**

1. **安全增益完全来自「独立性」，不来自「再签一次」。** 若 DVT 盲签（CA 说签它就签），攻击者控制 owner key 的同时通过 CA 一并触发 DVT 盲签 → DVT 形同虚设。**DVT 的真正价值 = 它有「owner key 之外的独立信号」判断该不该签**：独立私钥（BLS，不在同一 TEE/设备）+ 独立策略（链上限额/白名单/速率/异常检测，CA 改不了）+ 独立通道（直接和用户另一设备确认，不经 CA）。没有这些「独立信号」，DVT 只是多一个会盲签的橡皮图章。

2. **co-signer 角色不卡 R-1 —— 这是它比「验证者角色」更强的地方。** 验证者角色（§3.2）绕不过 R-1（attestation 信任根）；但 co-signer 角色**不依赖 attestation 信任根**：它不需要验「签名来自真 NXP 芯片」，只需要「≥门限个独立节点按各自策略批准了这笔操作」。所以**即使 R-1 没解、即使 KMS 是假 TEE（向量 5），大额操作仍需独立 DVT 节点共签** —— 假 TEE 骗不了有独立策略/通道的 DVT。这让 co-signer DVT 成为向量 5 在「R-1 收口之前」的一个**现实缓解手段**（前提仍是 DVT 独立于 CA）。

3. **DVT 节点自身也要被防。** 单节点也可能作恶/被攻破，所以靠 **≥N of M 门限 + 节点多样性**（不同运营者、不同地理/法律辖区，降低串谋）。信任的不是单个节点，是「≥N 个独立节点不会串谋」。

**价值有多大（评估）**：co-signer DVT 把「owner key 单点被盗 = 全损」变成「owner key 被盗 + 攻破 DVT 门限策略才损」。攻击成本从「偷一把 key」跃升到「偷 key + 攻破 N 个独立节点的独立策略/通道」。提升量级 = 独立性 × 策略强度 × 门限。**上限**：DVT 的策略/通道越依赖 CA，增益越打折（CA 作恶能同时骗 DVT）；越独立于 CA，价值越大。因有复杂度 + 延迟 + 激励成本，合理设计是**只对大额/高风险操作上 DVT 共签**（分层：小额 owner key 即可，大额加 DVT）。

**和激励 / 贡献记录的关系**：DVT 节点提供「在线 + 正确执行策略 + 共签」是有成本的劳动，对应 SuperPaymaster 的 **ROLE_DVT** 角色 + PGL 贡献记录（contribution record / 不可转让 SBT）。两个要点：① 激励应与「保护的资产规模 / 风险」匹配（守护大额的节点应获更多贡献记录 / 分账）；② **激励必须和「正确执行策略」绑定，绝不能和「签的次数」绑定** —— 否则节点为赚积分盲签，反而摧毁 DVT 的安全价值。盲签 / 签错要能被惩罚（SuperPaymaster 的 slash 机制）。这样「贡献记录」才真正度量「安全贡献」而非「盖章次数」。

### 3.1 角色 A：DVT 对每个向量有没有用、提升了什么

| 攻击向量 | DVT 的作用 | 价值定位 |
|---|---|---|
| **向量 1**（CA 跳过 passkey） | DVT 独立再验一遍 passkey。 | **冗余纵深防御**——TA 其实已经挡住了（验证在 TEE 内），DVT 是「双保险」，锦上添花。 |
| **向量 4**（偷换 payload） | DVT 独立校验「assertion 绑定的内容 == 实际要签的 payload」。 | **能挡**——但**前提是 challenge 已经绑定了 payload（#68 落地）**。#68 不做，DVT 也没有「正确的内容」可比对。 |
| **向量 5**（假 TEE） | **这才是 DVT 的核心价值**：DVT 独立要求 KMS 出具 attestation evidence，并**独立验证**，不信 CA 自报。CA 若用假 TEE → DVT 验 attestation 失败 → 拦下。 | **核心价值所在**——把「信任」从「CA 自己说」变成「独立第三方验」。 |

一句话：**DVT 的最大意义是对付向量 5（假 TEE）**——它逼着系统拿出可被独立验证的 attestation 证据，而不是让客户端盲信 CA。

### 3.2 DVT 的硬缺陷：它绕不过 R-1

但必须把话说透——**DVT 不是万能的，它有一个绕不过去的天花板**：

> **DVT 能验出来的强度，等于「attestation 信任根能否锚定到 NXP」（也就是 R-1）能否成立。**

- 如果 R-1 **没解决**：真 TEE 的 attestation key 是自签的，假 TEE 的 attestation key 也是自签的——**两者在密码学上都是「自说自话」，DVT 拿到证据也分不出哪个是真 NXP 芯片**。DVT 验了个寂寞。
- 所以 **DVT 是一个「放大器」，不是「信任根的替代品」**。它能放大「有信任根时」的安全性（多个独立节点验，更难串通），但**它自己变不出信任根**。地基（R-1）不牢，DVT 这层楼盖得再漂亮也是空中楼阁。

### 3.3 DVT 覆盖不到的，用什么补（圆满闭环）

| DVT 覆盖不到的 | 用什么补 |
|---|---|
| **R-1（信任根锚定 NXP）** | **只能靠硬件信任根收口**：EdgeLock 2GO / NXP 安全参考手册 / NDA / 实机逆向（#37 Phase 0）。DVT 帮不上，这是硬件层的事。 |
| **向量 4（偷换 payload）的前提** | **payload-bound challenge（#68）**——先把内容绑进 challenge，DVT 才有东西可验。 |

### 3.4 闭环结论

对「CA 完全作恶」的**完整防御**，需要这几块**合起来**才成立：

```
  DVT（独立二次验证，放大器）
   +  #37 远程证明（让 CA 必须出具可验证据）
   +  #68 payload-bound challenge（锁住"同意的内容"）
   +  #63 strict 模式（关掉 legacy 可重放后门）
   ───────────────────────────────────────────
   =  对"CA 完全作恶"的完整防御
```

**但地基是 R-1（信任根锚定 NXP）**。地基不牢，上面这些都打折扣：

- R-1 不解 → DVT 分不出真假 TEE、远程证明只是「自签自验」。
- 所以 **R-1 收口（#37 Phase 0）是一切的前提**，是必须落实的、DVT 替代不了的硬件层工作。

---

## 4. 闭环总表：攻击向量 × 现状 × 对应机制

| # | 攻击向量 | 现状 | 防御机制 / 关联 issue | 还差什么 |
|---|---|---|---|---|
| 1 | CA 跳过 passkey 直接命令 TEE 签 | **✅ 已防住** | 验证下沉 TA 内（**C-1**）：rpId hash + User Presence + ECDSA 验签 | — |
| 2 | CA 重放合法 assertion 签任意 payload | **✅ 已防住（strict 下）** | 一次性 nonce challenge binding（**#49**） | **当前过渡模式留 legacy 可重放后门，mainnet 前必须 flip strict（#63）** |
| 3 | CA 伪造 agent JWT | **✅ 已防住** | JWT 由 TA 内 HMAC 签发+验证，验 exp/claim（**H-3/4/5 + #15**） | — |
| 3b | CA 窃取合法 JWT | **⚠️ 部分缓解** | scope + exp 限制；agent JWT 本就低敏感度 | bearer token 固有 trade-off，非漏洞 |
| 4 | CA 偷换签名 payload | **❌ 开放（正在补）** | payload-bound challenge（**#68**，开发中） | #68 落地：GetChallenge 绑 payload hash，签名前校验 |
| 5 | CA 用假 TEE / 伪造整个 TEE | **❌ 开放（最根本，未闭合）** | 远程证明（**#37**） | **R-1**：信任根能否锚 NXP（RM00284 沉默）→ 需 Phase 0 实机 / NXP 一手源 / EdgeLock 2GO / NDA |
| — | （纵深）独立二次验证 | 增强项 | **DVT** | 强度上限 = R-1；是放大器非信任根替代 |

---

## 5. 诚实的「现在还不能保证什么」

把丑话说在前面，这是这份文档最重要的部分：

1. **「完全不信任部署方」目前还不成立。** 它依赖远程证明 + 信任根锚定 NXP（**R-1**），而 R-1 尚未收口（RM00284 已通读但对签名密钥的 provenance / 证书链完全沉默）。在 R-1 闭合前，客户端**无法从密码学上证明「CA 用的是真 TEE」**——这是向量 5 的本质。

2. **MVP 阶段的信任根暂时是自签 / TOFU（首次见到即信任的登记表）。** 这是**安全降级**——牺牲了「无需信任部署方即可验真」这一最强属性，换取当前可用性。**它不是「更去中心化所以更好」，恰恰相反，是硬件给不出更强锚定时的无奈兜底。** 详见 `37-remote-attestation-design.md` §9（M-5 澄清）。

3. **passkey 授权路径上「偷换内容」（向量 4）在 #68 落地前是真实缺口。** 已定位、有方案、开发中，但**尚未上线**。

4. **过渡模式（#63 未 flip）下，legacy 可重放后门是真实存在的。** mainnet 前必须 flip strict + 重编 TA + 客户端全部升级。

5. **硬件根永远是 NXP，这一点无法去中心化、也不假装能去中心化。** 我们能去中心化的是**验证逻辑、参考值分发、验证发生的位置**（开源、可自部署 Veraison、可上链），**不是「信 NXP 造的芯片是真的」这个前提**。

**反过来，已经实打实做到的**：私钥永不出 TEE；passkey 验证、challenge 绑定、JWT 签发/验证全部下沉到 TEE 内做（CA 碰不到密钥、绕不过验证）；passkey 私钥锁在用户设备安全芯片里，连 AAStar 都拿不到。**向量 1、2（strict）、3 在密码学层面是闭合的**——这意味着「CA 偷私钥」「CA 伪造用户授权」「CA 伪造代理令牌」这三类最常见的担忧，已经有硬防御。

---

## 附录：本文引用的代码与设计凭证

- TA 内 passkey 验证（向量 1）：`kms/ta/src/main.rs` → `verify_passkey_for_wallet()`（rpId hash / UP flag / p256-m ECDSA 验签）
- TA 内 challenge 绑定（向量 2）：`kms/ta/src/main.rs` → `verify_challenge_binding()`、`challenge_issue/peek/consume()`；过渡开关 `ENFORCE_TA_CHALLENGE`（`main.rs:306`）
- TA 内 JWT（向量 3）：`kms/ta/src/main.rs` → `JWT_SECRET_STORE_ID`、`HmacSha256`、`verify_jwt_wallet_claims()`（agent_index/exp/iat，#15 用 `tee_unix_secs()`）
- 远程证明 / R-1（向量 5）：`docs/design/37-remote-attestation-research.md` §2.1、§3.2、§0；`docs/design/37-remote-attestation-design.md` §0、§2、§7（R-1/R-8/H-2）
- 过渡 → strict 路线（#63）：`kms/CHANGELOG.md`
