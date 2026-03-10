#!/bin/bash
# 开发监控 - 四屏幕布局（Terminal 1/2/3 + 本地命令行）
# 使用 tmux 在一个窗口中显示 4 个面板

SESSION="kms-dev"

# 检查 tmux 是否安装
if ! command -v tmux &> /dev/null; then
    echo "❌ tmux 未安装！"
    echo "安装: brew install tmux"
    exit 1
fi

# 如果会话已存在，先杀掉
tmux has-session -t $SESSION 2>/dev/null
if [ $? -eq 0 ]; then
    echo "🔄 关闭现有的监控会话..."
    tmux kill-session -t $SESSION
fi

echo "🚀 启动 KMS 开发监控（四屏幕）..."
echo ""

# 创建新会话
tmux new-session -d -s $SESSION -n "KMS-Dev"

# 设置布局为四个面板：
# ┌─────────┬─────────┐
# │    0    │    1    │
# │ Term1   │ Term2   │
# │ QEMU    │   CA    │
# ├─────────┼─────────┤
# │    2    │    3    │
# │ Term3   │  Local  │
# │   TA    │  Shell  │
# └─────────┴─────────┘

# 分割成 4 个面板
tmux split-window -h -t $SESSION:0        # 水平分割，创建面板 1
tmux split-window -v -t $SESSION:0.0      # 垂直分割面板 0，创建面板 2
tmux split-window -v -t $SESSION:0.2      # 垂直分割面板 2（右侧），创建面板 3

# 面板 0 (左上): Terminal 1 - QEMU
tmux send-keys -t $SESSION:0.0 "cd $(pwd) && echo '🖥️  Terminal 1: QEMU Monitor' && ./scripts/kms-qemu-terminal1.sh" C-m

# 面板 1 (右上): Terminal 2 - CA (使用增强版，支持 3000 端口)
tmux send-keys -t $SESSION:0.1 "cd $(pwd) && echo '🔐 Terminal 2: CA (KMS API Server)' && ./scripts/kms-qemu-terminal2-enhanced.sh" C-m

# 面板 2 (左下): Terminal 3 - TA
tmux send-keys -t $SESSION:0.2 "cd $(pwd) && echo '🔒 Terminal 3: TA (Secure World)' && ./scripts/kms-qemu-terminal3.sh" C-m

# 面板 3 (右下): 本地命令行
tmux send-keys -t $SESSION:0.3 "cd $(pwd) && clear" C-m
tmux send-keys -t $SESSION:0.3 "echo '💻 Local Shell - 本地命令行'" C-m
tmux send-keys -t $SESSION:0.3 "echo ''" C-m
tmux send-keys -t $SESSION:0.3 "echo '常用命令:'" C-m
tmux send-keys -t $SESSION:0.3 "echo '  ./scripts/kms-deploy.sh          # 编译部署'" C-m
tmux send-keys -t $SESSION:0.3 "echo '  curl https://kms.aastar.io/health # 测试 API'" C-m
tmux send-keys -t $SESSION:0.3 "echo '  docker exec teaclave_dev_env curl http://127.0.0.1:3000/health # Docker 内测试'" C-m
tmux send-keys -t $SESSION:0.3 "echo ''" C-m
tmux send-keys -t $SESSION:0.3 "echo '=============================================='" C-m
tmux send-keys -t $SESSION:0.3 "echo ''" C-m

# 设置面板标题（如果 tmux 支持）
tmux select-pane -t $SESSION:0.0 -T "QEMU"
tmux select-pane -t $SESSION:0.1 -T "CA"
tmux select-pane -t $SESSION:0.2 -T "TA"
tmux select-pane -t $SESSION:0.3 -T "Local"

# 默认聚焦到本地命令行面板
tmux select-pane -t $SESSION:0.3

echo ""
echo "✅ 监控系统已启动！"
echo ""
echo "📺 四屏幕布局:"
echo "   ┌─────────┬─────────┐"
echo "   │  QEMU   │   CA    │"
echo "   ├─────────┼─────────┤"
echo "   │   TA    │  Local  │"
echo "   └─────────┴─────────┘"
echo ""
echo "⌨️  tmux 快捷键（按 Ctrl+B 后再按）:"
echo "   ↑ ↓ ← →     在面板间跳转"
echo "   [           进入滚动模式（按 q 退出，可以用方向键/Page Up/Down 滚动）"
echo "   d           断开会话（监控继续运行后台）"
echo "   &           关闭整个会话"
echo "   z           最大化/恢复当前面板"
echo "   {           与上一个面板交换位置"
echo "   }           与下一个面板交换位置"
echo ""
echo "💡 重新连接会话: tmux attach-session -t $SESSION"
echo ""
echo "================================================"
echo ""

# 自动连接到会话
tmux attach-session -t $SESSION
