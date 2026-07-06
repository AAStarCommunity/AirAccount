<!-- Created: 2026-07-06 -->
# KMS + DVT co-location @ imx93 — 性能基线报告

> 实测环境:NXP FRDM-IMX93(aarch64,**2× Cortex-A55**,**1939MB RAM**,OP-TEE 4.8),
> KMS v0.27.4(dev profile,strict challenge)+ DVT/aNode v1.7.1 同板共跑。
> 测试日期:2026-07-06 · 分支 `feat/kms-dvt-imx93-colocation`。

## 结论(TL;DR)

**2GB / 2 核 imx93 单板同时跑 KMS + DVT,为 20 并发的小社区提供服务绰绰有余。** RAM 不是约束(共跑仍空 1.5GB),签名吞吐天花板 ~80 sign/s,20 并发签名 p99 < 40ms,零内存泄漏。

## 部署形态(可复制性)

- DVT 提供**自包含 arm64 bundle**(`aastar-dvt-1.7.1-linux-arm64.tar.gz`,66M,**内含 node 运行时**)→ scp + 解压 + systemd,**板上无需装 node**。
- KMS 现网(`kms-api.service`)**全程未动**;DVT 独立端口 8080 + 独立 `dvt.service`。
- DVT 模块全集(health capabilities):`confirm / keeper / notify / policy / relay / x402-facilitator` —— **facilitator(x402)已在 DVT 包内,co-locate 零额外成本**。

## 签名延迟(server-side,`curl -w %{time_total}`,localhost)

| 场景 | 值 |
|---|---|
| 暖签名(C=1) | **p50 1–2ms** |
| 冷签名(钱包首用,LRU 未命中) | ~28ms(前 ~11 次,之后转暖 ~18ms 端到端) |
| 单次 TA 暖签名成本 | 几 ms(health 无 TA 也 18ms 端到端 → 18ms 主要是 shell 进程开销) |

## 并发曲线(暖 SignHash,单 TA worker 串行)

| 并发 C | 吞吐 | p50 | p99 | max |
|---|---|---|---|---|
| 1 | (单请求,进程开销主导) | 1–2ms | 1–2ms | 2ms |
| 5 | ~78 sign/s | 5ms | 9–12ms | 12ms |
| 20 | ~80 sign/s | 6–12ms | 23–39ms | 39ms |

- **吞吐天花板 ≈ 80 sign/s**(单 worker 单会话串行,见 [TA 并发模型](#ta-并发模型))。
- **20 并发全部完成,p99 < 40ms,无超时、无熔断,`queue_depth` 回落 0。**
- 20 sustained sign/s 仅占天花板 ~25%,小社区偶发签名远够。

## 内存(200+ 请求 + 60 并发 + 签名负载,无泄漏)

| 进程 | RSS | 泄漏 |
|---|---|---|
| kms-api-server(Rust) | **7.5MB**(纹丝不动,+84KB/200req) | 无 |
| DVT node(NestJS) | ~103–107MB(V8 堆带,GC 回收) | 无 |
| 系统 available(共跑) | **~1.49GB / 1939MB** | —— |

CPU:空载两服务 ~0%,签名负载 load 0.15–0.24。

## 回答原始三问

1. **mx93 够不够 20 并发?** → **够,且富余**。20 并发 p99<40ms,天花板 ~80 sign/s,RAM 空 1.5GB。
2. **其他社区快速复制?** → 部署已证明极简:自包含 arm64 bundle,scp+解压+systemd。配置模板化见 `community.toml`(待做)。
3. **共通配置 + DVT 要不要 Rust 重写?** → **RAM 不是约束 → 不必为内存重写 DVT**。hybrid(Node 编排 + Rust signer)在板上验证 OK,全量 Rust 重写属过早优化。共通配置走 `community.toml`。

## TA 并发模型

签名串行是设计使然(单 worker + 单持久会话 + mpsc 队列 + 熔断器 + 30s 超时),且 RPMB 单计数器 / secure storage 本就要求写串行。天花板 = 单会话延迟,提升靠**降单次延迟**而非并行。详见调研结论(队列改进 / 保守优化任务)。

## 未覆盖 / 后续

- 本轮是 SignHash 暖签名;写操作(CreateWallet/ChangePasskey,碰 RPMB)吞吐未单独压测。
- DVT 各 capability 均 disabled(最小 env 启动);启用 relay/BLS 聚合后的 CPU 尖峰待测。
- 队列改进(有界+429/超时丢弃/两车道)+ 保守优化(减 TA 往返/缓存可配)见对应任务。
