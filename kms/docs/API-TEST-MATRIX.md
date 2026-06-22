# AirAccount KMS — API 测试覆盖矩阵

> 2026-06-22 · v0.26.0 (Beta5) · 配套 OpenAPI: [`api/openapi.yaml`](./api/openapi.yaml) · Swagger UI: [`api/index.html`](./api/index.html)

## 测试套件

| 套件 | 路径 | 覆盖 | 鉴权方式 |
|---|---|---|---|
| **E2E（真机 FRDM-IMX93）** | `kms/test/run-full-e2e.sh`（上板跑） | 全部功能端点 | 真实 WebAuthn ceremony + Bearer JWT |
| **API 链测试** | `kms/test/run-api-tests.sh` | ~30 端点 + 负向 | legacy passkey + 负向用例 |
| **单元测试 · proto** | `cargo test -p proto` | Command/序列化 | — |
| **单元测试 · host** | `kms/test/run-host-unit-tests.sh`（交叉编译上板） | 缓存/CLI/DB/限流/WebAuthn/请求反序列化 | — |

## 最新结果

| 层 | 结果 | 日期 |
|---|---|---|
| **E2E 全量**（39 断言） | **✅ 39/39 通过**（真机 FRDM-IMX93） | 2026-06-13 |
| 单元 · proto | ✅ 39 passed | 2026-06-12 |
| 单元 · host | ✅ 56 passed（上板） | 2026-06-12 |

> ✅ **2026-06-13 复验**:板子重启上线后重跑 `run-full-e2e.sh`,**39/39 全部通过**(含新增的 `/stats`、`/`、`/test`、`Sign(transaction)`、`admin/purge-key` 负向)。

## 端点矩阵(35 operations / 32 路径)

图例:✅ 真机验证 · 🔒 鉴权门 · U 有单元测试

### Infrastructure(无鉴权)
| 端点 | 方法 | E2E | 单元 | 状态 |
|---|---|---|---|---|
| `/health` | GET | ✅ | | ✅ |
| `/version` | GET | ✅ | | ✅ |
| `/QueueStatus` | GET | ✅ | | ✅ |
| `/RollbackCounter` | GET | ✅ | | ✅ |
| `/stats` | GET | ✅ | | ✅ |
| `/`(dashboard) | GET | ✅ | | ✅ |
| `/test`(test UI) | GET | ✅ | | ✅ |

### Wallet Lifecycle / Metadata
| 端点 | 方法 | E2E | 单元 | 状态 |
|---|---|---|---|---|
| `/CreateKey` | POST | ✅ | U | ✅ |
| `/KeyStatus` | GET | ✅ | | ✅ |
| `/DeleteKey`(ScheduleKeyDeletion) | POST 🔒 | ✅ | U | ✅ |
| `/UnfreezeKey` (#42) | POST 🔒 | ✅ (test-freeze-unfreeze.sh 5/5) | U | ✅ |
| `/ListKeys` | POST | ✅ | | ✅ |
| `/DescribeKey` | POST | ✅ | | ✅ |
| `/GetPublicKey` | POST | ✅ | | ✅ |

### Signing(WebAuthn 门)
| 端点 | 方法 | E2E | 单元 | 状态 |
|---|---|---|---|---|
| `/DeriveAddress` | POST 🔒 | ✅ | U | ✅ |
| `/SignHash` | POST 🔒 | ✅ + 负向 | | ✅ |
| `/Sign`(message) | POST 🔒 | ✅ | | ✅ |
| `/Sign`(transaction) | POST 🔒 | ✅ | | ✅ |
| `/ChangePasskey` | POST 🔒 | ✅ | | ✅ |

### WebAuthn Ceremony
| 端点 | 方法 | E2E | 状态 |
|---|---|---|---|
| `/BeginRegistration` | POST | ✅ | ✅ |
| `/CompleteRegistration` | POST | ✅(手工 none-attestation) | ✅ |
| `/BeginAuthentication` | POST | ✅(驱动所有 ceremony) | ✅ |
| `/kms/begin-grant-session-auth` | GET | ✅ | ✅ |

### Agent Keys
| 端点 | 方法 | E2E | 状态 |
|---|---|---|---|
| `/kms/create-agent-key` | POST 🔒 | ✅(TA panic 修复验证) | ✅ |
| `/kms/sign-agent` | POST 🔒Bearer | ✅ | ✅ |
| `/kms/refresh-agent-credential` | POST 🔒Bearer | ✅ | ✅ |
| `/kms/revoke-agent-credential` | POST 🔒 | ✅ | ✅ |

### EIP-712 & SuperPaymaster
| 端点 | 方法 | E2E | 状态 |
|---|---|---|---|
| `/kms/SignTypedData` | POST 🔒 | ✅ + 负向(no-auth/malformed/invalid-HMAC) | ✅ |
| `/kms/SignMicropaymentVoucher` | POST 🔒 | ✅ | ✅ |
| `/kms/SignGTokenAuthorization` | POST 🔒 | ✅ | ✅ |
| `/kms/SignX402Payment` | POST 🔒 | ✅ + no-auth 负向 | ✅ |

### Grant Sessions / P256 Sessions
| 端点 | 方法 | E2E | 状态 |
|---|---|---|---|
| `/kms/sign-grant-session` | POST 🔒 | ✅ + no-auth 负向 | ✅ |
| `/kms/sign-p256-grant-session` | POST 🔒 | ✅ | ✅ |
| `/kms/create-p256-session-key` | POST 🔒 | ✅ | ✅ |
| `/kms/sign-p256-user-op` | POST 🔒Bearer | ✅(149字节格式断言) | ✅ |
| `/kms/revoke-p256-session-key` | POST 🔒 | ✅ + 负向(no-WebAuthn/幂等) | ✅ |

### Admin
| 端点 | 方法 | E2E | 状态 |
|---|---|---|---|
| `/admin/purge-key` | POST 🔒AdminToken | ✅ 负向(no-token→400) | ✅;正向为运维手动(破坏性) |

## 如何复跑

```bash
# E2E(板子上线后)— 把测试目录推上板再跑
scp -r kms/test root@<board-ip>:/tmp/kmstest
ssh root@<board-ip> 'cd /tmp/kmstest && bash run-full-e2e.sh'

# proto 单元测试(任意机器)
cargo test -p proto

# host 单元测试(交叉编译 → 上板)
bash kms/test/run-host-unit-tests.sh <board-ip>
```

## 覆盖总结

- **功能端点 E2E 覆盖:32/32 = 100%**(含正向 + 关键负向 auth-gate)
- 全部 39 项真机 39/39 通过(2026-06-13)
- 单元测试:proto 39 + host 56 = 95,全部通过
- TA 层:`aarch64-unknown-optee` 无 std test harness,逻辑由真机 E2E 覆盖(见 RELEASE-PLAN)
