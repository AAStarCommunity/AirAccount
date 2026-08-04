# 社区节点更新签名密钥(minisign)

updater 只安装被这把 minisign 私钥签过的 channel manifest —— 这是整条自动更新链路的信任根。

## 公钥(可公开,已编入仓库/节点)

```
untrusted comment: minisign public key B4D4EC2546A19EB2
RWSynqFGJezUtHE8RGgt/hza4GLIjbaivYjnBKwuQ+liM52mXNbgVzAo
```

- **key ID**: `B4D4EC2546A19EB2`
- 仓库内文件:[`updater-pubkey.pub`](./updater-pubkey.pub)
- 节点上路径:`/etc/airaccount/updater-pubkey.pub`(installer 落地;updater `AU_PUBKEY` 默认指向它)
- 校验单个文件:`minisign -Vm <file> -P RWSynqFGJezUtHE8RGgt/hza4GLIjbaivYjnBKwuQ+liM52mXNbgVzAo`

## 私钥(绝密,永不入库)

- 路径:`~/.ssh/aastar-updater.key`(jason 本机;已用密码加密)
- 用途:发版时给 `channels/<channel>.json` 签名(见 [`release-sign.sh`](./release-sign.sh))
- 备份:密码管理器 + 离线介质各一份。**丢失=无法再发版**(需换密钥并刷新所有节点公钥);
  **泄露=攻击者可伪造更新**(需立即轮换 + 通过 OOB 给所有节点换公钥)。
- ⚠️ **manifest 状态也是无备份的单点信任根**:`channels/<channel>.json` 的 `metadata_version`(防回滚
  单调计数)+ 累积的 `releases[]` 丢了,同样发不出能被节点接受的新版(计数倒退 → 节点持久化的
  `seen_metadata_version` 永久拒绝)。缓解:把签名后的 `channels/<channel>.json`(+ `.minisig`)入库/
  备份,且 `release-sign.sh` 默认会从已发布 URL **读回** `metadata_version` 作基线(`--base-url`),
  跨机器/新 clone 也单调。发版机与私钥一起纳入备份预案。
- 生成方式(已完成,记录备查):
  ```bash
  minisign -G -p ~/.ssh/aastar-updater.pub -s ~/.ssh/aastar-updater.key
  ```

## 轮换预案

1. 生成新 keypair。
2. 通过 **OOB(串口 / 现场)** 把新公钥写进每个节点的 `/etc/airaccount/updater-pubkey.pub`
   —— 不能用自动更新通道下发公钥(先有鸡还是先有蛋 + 一旦旧私钥泄露则通道不可信)。
3. 之后的 manifest 改用新私钥签。
4. 更新本文件 + `updater-pubkey.pub`。

> 未来叠加:cosign keyless + Rekor 透明日志(CI 侧,叠在 minisign 之上,见 auto-update-design.md §9)。
