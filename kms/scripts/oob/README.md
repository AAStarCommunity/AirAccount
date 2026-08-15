# 带外串口救板工具(OOB serial rescue)

板子**上电但 off-network**(SSH / tailscale 不通,比如公寓/学校门户没授权、热点没上行)时,
用调试串口发命令诊断、必要时关机。

> 会用到这套工具的人本来就已经处在糟糕的处境里 —— 所以工具本身要稳、要 fail-safe。

## 前置

- `python3` + `pyserial`(`pip3 install pyserial`)
- `jq`(关机守卫解析用)
- **先断开其它串口会话**(screen / minicom)—— `serial-run.py` 会用 `flock` 独占串口,
  被占用会直接报错退出而不是读到别人会话的串。

## 设备 = 物理板身份

macOS 上每块板的调试适配器是一个独立 USB serial,如 `/dev/cu.usbmodem…831`(板 B console;
`…833` 是第二接口)。**板 A/B 主机名同族(都是 `imx93-…-frdm`),无法靠主机名区分** ——
靠你**显式选中的串口设备**确定是哪块物理板。所以破坏性操作(关机)**必须显式传设备,禁用 auto**。

## serial-run.py —— 通用命令执行器

```bash
./serial-run.py --list                     # 列候选串口
./serial-run.py --print-dev                # 解析并打印将用的设备
./serial-run.py --dev /dev/cu.usbmodem…831 'ip -br addr' 'systemctl is-active kms-api'
./serial-run.py --dev … --json 'whoami' 'hostname'          # 机器可读 [{cmd,rc,out}]
./serial-run.py --dev … --read-secs 25 'systemctl poweroff' # 关机/重启:流式读 N 秒
```

- 每条命令用**一次性 nonce 标记** `__B__<nonce>` / `__E__<nonce>_<rc>` 精确框住真实输出与 rc
  —— 命令回显不会撞标记(修 PR#193 Blocking 1)。
- `drain()` 用**绝对 deadline + 缓冲上限** —— 控制台无限刷日志也不挂死(修 Blocking 2);
  未读到结束标记 → `rc=124`(超时),绝不沿用上一条 rc。
- 密码走环境变量 `SERIAL_PASSWORD`(不走 argv,免 `ps` 泄露);只在 `Password:` 处发,
  发完仍停在 `Password:` = 硬错误。
- 退出码 = 最后一条命令 rc;`--read-secs` 模式恒 0。

## mx93b-serial-poweroff.sh —— 优雅关闭板 B

```bash
./mx93b-serial-poweroff.sh /dev/cu.usbmodem…831
# 或 MX93B_SERIAL=/dev/cu.usbmodem…831 ./mx93b-serial-poweroff.sh
```

守卫(全过才关机,否则中止):
1. **必须显式设备**,禁 auto(防关错板,Blocking 4);守卫与关机用同一个设备(无 TOCTOU)。
2. `whoami` 精确等于 `root` 且 `rc=0`,`hostname` 精确等于 `$MX93B_HOSTNAME`
   (默认 `imx93-11x11-lpddr4x-frdm`)且 `rc=0` —— 用带 rc 的机器可读结果判定,
   **不 grep 自由文本**(启动日志/提示符 `root@imx93…#` 能骗过 grep,Blocking 3)。

**守卫中止怎么办**:说明串口没拿到活的 root shell(板子还在刷启动日志 / 未登录 / 串口被别的会话占用),
这时**绝不会盲发关机**。先解决登录/占用,或确认设备选对(`--list` / `--print-dev`)。

## serial-selfupdate.sh —— 带外一键自拉 release 升级

板子够不到(SSH/tailscale 全挂)但**上电、串口可达**时,从 Mac 端驱动板子:从 GitHub Release
自拉指定版本 → 验签 → 换 CA → 烟测 → 失败自动回滚。是自动 updater(`kms/deploy/updater/`)的
**手动带外对应物**(updater 管日常自动,本脚本管"够不到、要手动救+升")。

```bash
./serial-selfupdate.sh /dev/cu.usbmodem…831 airaccount-node-v0.29.1
ENSURE_NET=1 ./serial-selfupdate.sh /dev/cu.usbmodem…831 airaccount-node-v0.29.1   # 升级前先 WiFi 救网
```

两段验证(信任模型):
- **Mac 端**:`gh` 下载 release → `minisign` 验签(**pin 死可信公钥**,不信 release 自带的 `updater.pub`,避免循环信任)+ sha256 声明==实际。
- **板子端**:`curl` 同一 tarball → sha256==Mac 已验证哈希 + tar 加固(拒绝绝对路径/../解压越界)。板上无 minisign,故 authenticity 在 Mac 端完成。

实测坑(已在脚本内处理):
- 板子 restart 时会往串口**异步打 TA 生命周期噪声**(`[+] TA close/create/open session`),会混进命令 out
  → 所有关键值用 `TAG<value>` 包裹再正则取,躲开噪声(否则 `is-active` 被打成 `[+]TAcreate…active` 触发假回滚)。
- 板上 userland **无 `jq`** → 一律 grep 解析,不依赖板上 jq。
- restart 后 systemd 虽 `active`,HTTP `/health` 要等 TA session 重建数秒才 ready → 健康门**轮询等就绪**。
- 只换 **CA**;bundle 若含 TA(`*.ta`)只告警不处理(TA 牵动 secure storage,走专门流程)。

选项(env):`ENSURE_NET`(救网)、`WIFI_IFACE`(默 `mlan0`)、`WIFI_SELECT_ID`(默 0=@JumboPlusIoT5GHz)、
`PORTAL_MARKER`(默 `data-lang=.th.` 三语门,设空跳过)、`EXPECT_VERSION`、`REMOTE_BIN`、`KMS_SERVICE`。

## 测试

```bash
./test-serial-run.py     # pty + bash 当假串口,无需硬件;验 nonce 标记 + 硬 deadline
```
