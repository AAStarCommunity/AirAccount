#!/usr/bin/env bash
# 串口优雅关闭板 B(mx93b)。真实脚本在 kms/scripts/oob/,这里按本文件位置解析,任何 cwd 都能跑。
HERE="$(cd "$(dirname "$0")" && pwd)"
exec "$HERE/kms/scripts/oob/mx93b-serial-poweroff.sh" /dev/cu.usbmodem5B6D0040831
