#!/bin/bash
# 使用 tmux 启动所有监控 - 真实日志版本
# 显示真实的 CA 和 TA 日志（通过共享目录和 dmesg）

SESSION="kms-monitor-direct"

# 检查 tmux 是否安装
if ! command -v tmux &> /dev/null; then
    echo "❌ tmux 未安装！"
    echo "请安装 tmux: brew install tmux"
    exit 1
fi

# 如果会话已存在，先杀掉
tmux has-session -t $SESSION 2>/dev/null
if [ $? -eq 0 ]; then
    echo "🔄 关闭现有的监控会话..."
    tmux kill-session -t $SESSION
fi

echo "🚀 启动 KMS API 完整监控系统 (真实日志版)"
echo ""

# 首先确保 KMS API Server 将日志写入共享目录
echo "📋 准备工作："
echo "   1. 检查 KMS API Server 日志配置..."

if ! docker exec teaclave_dev_env test -f /opt/teaclave/shared/kms-api.log; then
    echo ""
    echo "⚠️  日志文件不存在，正在重新配置..."
    echo ""
    ./scripts/restart-kms-with-shared-log.sh
    echo ""
    echo "等待服务稳定..."
    sleep 5
fi

# 创建新会话
tmux new-session -d -s $SESSION -n "KMS-Direct"

# 设置布局
# 窗口分为 4 个面板：
# ┌─────────┬─────────┐
# │    1    │    2    │
# │  QEMU   │CA (Real)│
# ├─────────┼─────────┤
# │    3    │    4    │
# │TA (Real)│  Cloud  │
# └─────────┴─────────┘

# 分割成 4 个面板
tmux split-window -h -t $SESSION:0
tmux split-window -v -t $SESSION:0.0
tmux split-window -v -t $SESSION:0.2

# 面板 0 (左上): QEMU
tmux send-keys -t $SESSION:0.0 "cd $(pwd) && ./scripts/monitor-terminal1-qemu.sh" C-m

# 面板 1 (右上): CA (真实日志 - 从共享目录读取)
tmux send-keys -t $SESSION:0.1 "cd $(pwd) && ./scripts/monitor-terminal2-ca-direct.sh" C-m

# 面板 2 (左下): TA (真实日志 - 从 dmesg 读取)
tmux send-keys -t $SESSION:0.2 "cd $(pwd) && ./scripts/monitor-terminal3-ta-direct.sh" C-m

# 面板 3 (右下): Cloudflared
tmux send-keys -t $SESSION:0.3 "cd $(pwd) && ./scripts/monitor-terminal4-cloudflared.sh" C-m

# 设置面板标题
tmux select-pane -t $SESSION:0.0 -T "QEMU"
tmux select-pane -t $SESSION:0.1 -T "CA-Real"
tmux select-pane -t $SESSION:0.2 -T "TA-Real"
tmux select-pane -t $SESSION:0.3 -T "Cloudflared"

echo ""
echo "✅ 监控系统已启动！(真实日志版)"
echo ""
echo "🎯 特性:"
echo "   - Terminal 2: 真实的 CA Rust 日志（log::info!等）"
echo "   - Terminal 3: 真实的 OP-TEE 内核日志（dmesg）"
echo "   - 无需依赖 Cloudflared 日志推断"
echo ""
echo "📺 进入监控界面："
echo "   tmux attach-session -t $SESSION"
echo ""
echo "⌨️  快捷键："
echo "   Ctrl+B, 方向键  - 在面板间切换"
echo "   Ctrl+B, [       - 进入滚动模式（q 退出）"
echo "   Ctrl+B, d       - 断开会话（监控继续运行）"
echo "   Ctrl+B, &       - 关闭整个会话"
echo ""
echo "🧪 测试 API："
echo "   在浏览器访问: https://kms.aastar.io/test"
echo "   或运行: curl -s https://kms.aastar.io/health"
echo ""
echo "================================================"
echo ""

# 自动连接到会话
tmux attach-session -t $SESSION