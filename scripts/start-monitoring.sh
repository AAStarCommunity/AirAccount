#!/bin/bash
# KMS API 完整监控启动脚本
# 在多个终端窗口中启动监控

cat << 'EOF'
🔐 AirAccount KMS API 监控系统
================================================

完整的调用链监控：

  Web UI (kms.aastar.io)
      ↓
  Terminal 4: Cloudflared Tunnel
      ↓
  Terminal 2: KMS API Server (CA - Normal World)
      ↓
  Terminal 3: OP-TEE TA (Secure World)
      ↓
  Terminal 1: QEMU Guest VM

================================================

请在 4 个独立的终端窗口中分别运行：

📟 Terminal 1 - QEMU 监控:
   ./scripts/monitor-terminal1-qemu.sh

📟 Terminal 2 - CA (KMS API Server) 监控:
   ./scripts/monitor-terminal2-ca.sh

📟 Terminal 3 - TA (Secure World) 监控:
   ./scripts/monitor-terminal3-ta.sh

📟 Terminal 4 - Cloudflared 监控:
   ./scripts/monitor-terminal4-cloudflared.sh

================================================

测试流程：

1. 打开 4 个终端窗口，分别运行上述脚本

2. 访问 https://kms.aastar.io

3. 点击 "CreateKey - 创建密钥"

4. 观察调用链：
   ✅ Terminal 4: 看到 HTTPS 请求进入
   ✅ Terminal 2: KMS API Server 处理请求
   ✅ Terminal 3: TA 在 Secure World 执行
   ✅ Terminal 4: 响应返回到公网

================================================

快速测试命令（在新终端执行）：

# 创建密钥
curl -X POST https://kms.aastar.io/CreateKey \\
  -H "Content-Type: application/json" \\
  -H "X-Amz-Target: TrentService.CreateKey" \\
  -d '{"Description":"test","KeyUsage":"SIGN_VERIFY","KeySpec":"ECC_SECG_P256K1","Origin":"AWS_KMS"}'

# 查询密钥
curl -X POST https://kms.aastar.io/DescribeKey \\
  -H "Content-Type: application/json" \\
  -H "X-Amz-Target: TrentService.DescribeKey" \\
  -d '{"KeyId":"<your-key-id>"}'

================================================
EOF

echo ""
echo "提示: 如果需要自动化启动，可以使用 tmux 或 screen"
echo ""
echo "tmux 示例:"
echo "  tmux new-session -d -s kms-monitor"
echo "  tmux split-window -h"
echo "  tmux split-window -v"
echo "  tmux select-pane -t 0"
echo "  tmux split-window -v"
echo "  tmux select-pane -t 0"
echo "  tmux send-keys './scripts/monitor-terminal1-qemu.sh' C-m"
echo "  tmux select-pane -t 1"
echo "  tmux send-keys './scripts/monitor-terminal2-ca.sh' C-m"
echo "  tmux select-pane -t 2"
echo "  tmux send-keys './scripts/monitor-terminal3-ta.sh' C-m"
echo "  tmux select-pane -t 3"
echo "  tmux send-keys './scripts/monitor-terminal4-cloudflared.sh' C-m"
echo "  tmux attach-session -t kms-monitor"
echo ""