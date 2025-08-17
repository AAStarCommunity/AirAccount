#!/bin/bash
# 快速CA测试脚本

echo "🧪 快速测试CA-TA通信修复"

# 连接到QEMU并执行测试
screen -S qemu_5stage -p 0 -X stuff $'/shared/airaccount-ca hello\n'
sleep 3

screen -S qemu_5stage -p 0 -X stuff $'/shared/airaccount-ca echo "Test Fix"\n'
sleep 3

screen -S qemu_5stage -p 0 -X stuff $'/shared/airaccount-ca test\n'
sleep 5

echo "✅ 命令已发送到QEMU会话"
echo "📋 查看结果，请运行: screen -r qemu_5stage"