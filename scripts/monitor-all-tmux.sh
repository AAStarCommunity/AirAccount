#!/bin/bash
# 使用 tmux 在单个窗口中启动所有监控

SESSION="kms-monitor"

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

echo "🚀 启动 KMS API 完整监控系统..."
echo ""

# 创建新会话
tmux new-session -d -s $SESSION -n "KMS-Monitor"

# 设置布局
# 窗口分为 4 个面板：
# ┌─────────┬─────────┐
# │    1    │    2    │
# │  QEMU   │   CA    │
# ├─────────┼─────────┤
# │    3    │    4    │
# │   TA    │  Cloud  │
# └─────────┴─────────┘

# 分割成 4 个面板
tmux split-window -h -t $SESSION:0
tmux split-window -v -t $SESSION:0.0
tmux split-window -v -t $SESSION:0.2

# 面板 0 (左上): QEMU
tmux send-keys -t $SESSION:0.0 "cd $(pwd) && ./scripts/monitor-terminal1-qemu.sh" C-m

# 面板 1 (右上): CA (KMS API Server)
tmux send-keys -t $SESSION:0.1 "cd $(pwd) && ./scripts/monitor-terminal2-ca.sh" C-m

# 面板 2 (左下): TA (Secure World)
tmux send-keys -t $SESSION:0.2 "cd $(pwd) && ./scripts/monitor-terminal3-ta.sh" C-m

# 面板 3 (右下): Cloudflared
tmux send-keys -t $SESSION:0.3 "cd $(pwd) && ./scripts/monitor-terminal4-cloudflared.sh" C-m

# 设置面板标题（如果 tmux 支持）
tmux select-pane -t $SESSION:0.0 -T "QEMU"
tmux select-pane -t $SESSION:0.1 -T "CA"
tmux select-pane -t $SESSION:0.2 -T "TA"
tmux select-pane -t $SESSION:0.3 -T "Cloudflared"

echo ""
echo "✅ 监控系统已启动！"
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
echo "   在浏览器访问: https://kms.aastar.io"
echo "   或运行: curl -s https://kms.aastar.io/health | jq ."
echo ""
echo "================================================"
echo ""

# 自动连接到会话
tmux attach-session -t $SESSION