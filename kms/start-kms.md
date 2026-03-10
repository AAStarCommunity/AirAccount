# 如何启动kms

## 进入目录
进入mac mini
/Volumes/UltraDisk/Dev2/aastar/AirAccount

确认在kms分支

## 一般流程
完整流程：

### 1. 修改代码
vim kms/ta/src/main.rs
vim kms/host/src/main.rs

### 2. 编译并部署
./scripts/kms-deploy.sh

### 3. 测试
curl http://localhost:3000/health

如果需要完全重启（包括 QEMU）：

### 方式 1: 使用一键启动
./scripts/kms-deploy.sh      # 先部署
./scripts/kms-auto-start-with-wallets.sh
会自动创建 3 个固定测试钱包：
  1. dev-wallet-1
  2. dev-wallet-2
  3. dev-wallet-3

  重启后地址保持不变
- ✅ 自动启动 + 自动钱包
- ❌ 看不到 TA 日志

### 方式 2: 使用三终端模式
./scripts/kms-deploy.sh                    # 先部署
./scripts/kms-qemu-terminal3.sh           # T3（自动清理旧进程并启动）
./scripts/kms-qemu-terminal2.sh  # T2，需要tail log监控，也可以手工部署在这里
./scripts/kms-qemu-terminal1.sh           # T1
## 初始化固定钱包（3个）
./scripts/kms-init-wallets-wait.sh

一个钱。包之后。我们是。可以。用。1。export。key。去。导。出它。的私。钥的。那你。导出。私钥。之后。呢？你把。它的。keyid，address，和这个。\私钥存储到 JSON 文件，还是这个备份的这个思路，对。
创建的固定测试钱包，分析一下这个是不是可行

- dev-wallet-1: 0x35cfbc5170465721118b4798fd7ef25055ebe6e7
- dev-wallet-2: 0x54ff96c162441cf489598dc6e42da52fea3da3d4
- dev-wallet-3: 0x4cb9b4bce794d00b3035d9700a9a5c3e089d4cbe

  1. dev-wallet-1 (0x35cfbc5170465721118b4798fd7ef25055ebe6e7)
    - KeyId: 862ae409-843f-456a-83c8-ebd8f884d1e1
    - 0x29fec7da916c64ef96ea140f3baec501572f213c304f610875a11fba9affd268
  2. dev-wallet-2 (0x54ff96c162441cf489598dc6e42da52fea3da3d4)
    - KeyId: 38f39e59-f2da-4007-a197-a7eeed3352bf
    - 0xb186a6cb03115860a62313809f892f4336b26e7888d75e799f34d61e5a9be4ad
  3. dev-wallet-3 (0x4cb9b4bce794d00b3035d9700a9a5c3e089d4cbe)
    - KeyId: c4a501d2-2075-488d-98f1-b7c6dfb37ff3
    - 0x0b9e79cbc0353ac69235d19d9fced53ce288ae45e8a9e81cac4fb7b5c6b30ad5

## 导出私钥
在 Guest VM 中执行
  cd /root/shared
  ./export_key <wallet-id> "m/44'/60'/0'/0/0"
## monitor CA
./scripts/kms-qemu-terminal2-enhanced.sh

## 测试本地
 curl -s http://localhost:3000/health | jq .

启动后测试本地API：
curl -s -X POST http://localhost:3000/heath \
   -H "Content-Type: application/json" \
   -H "x-amz-target: TrentService.health" \
   -d '{}' | jq .

 curl -s -X POST http://localhost:3000/ListKeys \
    -H "Content-Type: application/json" \
    -H "x-amz-target: TrentService.ListKeys" \
    -d '{}' | jq .

## 启动并测试tunnel
./scripts/kms-tunnel-run.sh 2>&1 | head -20

curl -s https://kms.aastar.io/health | jq .
{
  "endpoints": {
    "GET": [
      "/health"
    ],
    "POST": [
      "/CreateKey",
      "/DescribeKey",
      "/ListKeys",
      "/DeriveAddress",
      "/Sign",
      "/SignHash",
      "/DeleteKey"
    ]
  },
  "service": "kms-api",
  "status": "healthy",
  "ta_mode": "real",
  "version": "0.1.0"
}

### monitor tunnel
tail -f /tmp/cloudflared.log

### monitor CA
./scripts/kms-qemu-terminal2-enhanced.sh

## 常用命令
📝 常用命令示例

### 查看共享目录文件
./scripts/kms-guest-exec.sh "ls -lh /root/shared"

### 检查 TA 文件
./scripts/kms-guest-exec.sh "ls -lh /lib/optee_armtz/*.ta"

### 查看 API 进程
./scripts/kms-guest-exec.sh "ps aux | grep kms-api-server"

### 重启 API Server (如果需要重新部署 TA)
./scripts/kms-guest-exec.sh "pkill kms-api-server && cd /root/shared && nohup ./kms-api-server > kms-api.log 2>&1 &"

### 导出私钥（如果钱包存在）
./scripts/kms-guest-exec.sh "cd /root/shared && ./export_key 03e0ad6f-9019-450d-9426-26203ab08ef1
\"m/44'/60'/0'/0/0\""

  2. 查看共享目录文件

  docker exec teaclave_dev_env ls -lh /opt/teaclave/shared/

  3. 查看 TA 文件

  docker exec teaclave_dev_env ls -lh /opt/teaclave/shared/ta/

  4. 重新部署（如果需要更新 TA）

  ./scripts/kms-deploy.sh


  是不是因为我们用./scripts/kms-auto-start.sh启动后占用了端口？
  意味着 Guest VM 的 serial console 不在可以接收命令的状态。这是因为 API server 已经启动并占用了 console。
  那如何改进./scripts/kms-auto-start.sh，成为自动启动但不占用端口，然后我运行比如一个shell（类似于
./scripts/kms-qemu-terminal2-enhanced.sh，
./scripts/kms-qemu-terminal3.sh，和host，同时可以运行一些命令，监控CA，TA的运行状态？
  你可以停止API server，改进kms-auto-start.shshell后再重启

  记得v2要启动 cloudflared tunnel啊

  ------

  ## Simple
  - 快速启动（推荐用于开发调试）
  ./scripts/kms-auto-start-v2.sh

  - 使用交互式工具管理
  ./scripts/kms-guest-interactive.sh

  - 监控 CA/TA 日志
  ./scripts/kms-qemu-terminal2-enhanced.sh
  ./scripts/kms-qemu-terminal3.sh



  所以我有两种方法启动：
     # 只需要运行这一个命令
    ./scripts/kms-auto-start.sh

    它会自动：
    1. ✅ 启动监听器（54320, 54321）
    2. ✅ 启动 QEMU
    3. ✅ 等待 Guest VM 启动
    4. ✅ 自动挂载 shared 目录
    5. ✅ 自动启动 API Server

    但是我看不了TA log，只能：
       ./scripts/kms-qemu-terminal2-enhanced.sh
       看CA log
-----------

  我还可以使用旧版本的三终端模式，正确顺序是：

    # Terminal 3 (先启动 - Secure World 日志监听器)
    ./scripts/kms-qemu-terminal3.sh

    # Terminal 2 (第二启动 - Guest VM 监听器)
    ./scripts/kms-qemu-terminal2-enhanced.sh

    # Terminal 1 (最后启动 - QEMU)
    ./scripts/kms-qemu-terminal1.sh
-----
如果现在我运行旧版本，是不是需要kill qemu？还是直接运行321？

----


# 开发交互指令

目前重启后就丢失所有存储在qemu内的安全世界的私钥等信息，不方便开发和测试，请给出一个方案来解决这个问题，例如定时导出备份，然后重启服务后自动导入？或者其他好的解决方案
当前不支持持久化吧？因为是模拟的，是么
另外我们的test page已经滞后了，请对比我们实际对外提供的api，更新下

  2. 备份 + 恢复

  # 重启前备份
  ./scripts/kms-backup-wallets.sh

  # 重启后恢复
  ./scripts/kms-restore-wallets.sh ~/.kms-backup/backup.json
  - 原理：导出明文私钥到 JSON，重启后重新导入
  - 优点：保留所有钱包（包括随机创建的）

    ✅ 固定助记词 + 备份恢复 - 最简单实用的开发方案

    # 一键启动 + 自动创建测试钱包
    ./scripts/kms-auto-start-with-wallets.sh
    这个可以集成到terminal1脚本么？这样就可以一键启动并创建固定钱包

    # 每次重启运行这个命令，测试钱包自动恢复（因为助记词固定



  1. ✅ CreateKey - 完整实现,自动返回地址
  2. ✅ DescribeKey - 完整实现
  3. ✅ ListKeys - 完整实现
  4. ✅ DeriveAddress - 完整实现,手动派生地址
  5. ✅ Sign - 完整实现,支持 Transaction/Message/Address 三种模式
  6. ✅ SignHash - 完整实现,支持 KeyId 和 Address 两种模式
  7. ✅ DeleteKey - 完整实现

  ⏺ 完美! SignHash API 现在已经支持三种模式了:
    1. ✅ Address 模式 - 从缓存查找 (优先级最高)
    2. ✅ KeyId + DerivationPath 模式 - 手动指定路径 (向后兼容)
    3. ✅ KeyId only 模式 - 自动使用默认路径 (新增功能)

  请列出我们的安全storage的钱包存储结构，我记得有public key？
  我们要支持：
    1. KeyId + DerivationPath (手动指定路径)
    2. KeyId only (自动使用默认路径) ← 需要添加这个支持
    3. Address先不用管

    添加一个cron定时任务，每小时执行一次：./scripts/kms-backup-wallets.sh，自动备份
    所以现在执行了么？备份文件在哪里啊？我如何执行回复到最新状态呢？

    为啥kms-deploy之前部署不了？为何v2也不行？为何现在你直接命令行又可以部署了？所以现在test page是更细了么？

  特性:
  - ✅ 在 Docker 容器内直接编译 (使用所有 CPU 核心)
  - ✅ 自动同步源码到 SDK 目录
  - ✅ 自动复制二进制到 QEMU 共享目录
  - ✅ 智能重启 API Server
  - ✅ 支持 --clean 参数 (完全重新编译)
  - ✅ 支持 --no-restart 参数 (只编译不重启)
  - ✅ 详细的步骤输出和错误处理

  使用方法:
  # 增量编译并部署
  ./scripts/kms-deploy-v2.sh

  # 完全重新编译
  ./scripts/kms-deploy-v2.sh --clean

  # 查看帮助
  ./scripts/kms-deploy-v2.sh --help

创建钱包时有助记词，但是api没有给返回，恢复钱包需要助记词的话可以修改api server，只输出到指定备份share目录，包括钱助记词和包信息，这样可以么？创建一个，备份一个；恢复就从这里恢复，共享目录应用可以操作本地目录
但对外api不显示，在我们完成备份后，再更改字符串替换为占位符可以么？
去掉cron相关的脚本，定时任务和其他

为何判断是编译环境问题？之前两次修改CA，都编译并发布，验证成功了

禁止新建任何脚本，只使用命令行来和docker交互，完成kms的编译和发布，可以参考历史脚本的环境配置

严重怀疑你今天的智商，deploy都两个小时了，还失败

  📝 部署脚本使用方法

  # 标准部署（增量编译）
  ./scripts/kms-deploy-clean.sh

  # 完全重新编译（清理所有缓存）
  ./scripts/kms-deploy-clean.sh --force

    创建带助记词的钱包会自动备份到：
    - Guest VM: /root/shared/kms-wallets-backup/wallet_<UUID>.json
    - Docker Host: /opt/teaclave/shared/kms-wallets-backup/wallet_<UUID>.json

    备份文件格式：
    {
      "wallet_id": "63e3366c-bcec-4b85-8417-32f4713ac68c",
      "address": "0x671569e478f27ab0d3839bee97ec4441fd8a0691",
      "mnemonic": "test new deployment",
      "derivation_path": "m/44'/60'/0'/0/0",
      "backup_time": "2025-10-16T09:30:10Z",
      "version": "2.0"
    }

我有一个疑问，就是为什么你那个创建三个固定钱包的脚本可以用助记词来创建？那我们备份的这些钱包是有助记词的，你为什么不能用这些助记词去重新创建这些钱包？
助记词的那个你修改的 CA 就是 API Server 那个功能，原来返回的是站位符，现在你还恢复到这个就是 create wallet 这个功能 API 它返回的还是站位符。

