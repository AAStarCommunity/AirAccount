#!/bin/sh
# DK2 学校 WiFi 自动连接(用自己真 MAC 24:cd..,无克隆) + 诊断黑匣子
WPA=/usr/sbin/wpa_supplicant
UDHCP=/sbin/udhcpc
CONF=/etc/wpa_supplicant/wpa_supplicant-wlan0.conf
DBG=/home/root/wifi-debug.txt
IFACE=""
for d in /sys/class/net/*/wireless; do [ -e "$d" ] && IFACE=$(basename $(dirname "$d")) && break; done
[ -z "$IFACE" ] && IFACE=$(ls /sys/class/net/ 2>/dev/null | grep -E '^(wlan|mlan|wlp)' | head -1)
[ -z "$IFACE" ] && IFACE=wlan0
{ echo "########## boot $(date) iface=[$IFACE] ##########"; echo "[网卡]"; ip -o link show; echo "[真MAC应=24:cd:8d:4e:4f:28]"; cat /sys/class/net/$IFACE/address 2>/dev/null; } >> "$DBG" 2>&1
ip link set "$IFACE" up 2>>"$DBG"
$WPA -B -i "$IFACE" -c "$CONF" -P /run/wpa-$IFACE.pid 2>>"$DBG"; echo "wpa start rc=$?" >> "$DBG"
# wpa_supplicant 自身会一直重试关联;我们在循环里:关联上但没IP就(重)跑 udhcpc
i=0
while [ $i -lt 12 ]; do
  STATE=$(wpa_cli -i "$IFACE" status 2>/dev/null | grep -E '^wpa_state=' | cut -d= -f2)
  HASIP=$(ip -4 addr show "$IFACE" 2>/dev/null | grep -c 'inet ')
  if [ "$STATE" = "COMPLETED" ] && [ "$HASIP" = "0" ]; then
    $UDHCP -i "$IFACE" -n -q 2>>"$DBG"   # 关联成功但无IP -> 拿DHCP
  fi
  { echo "===== snap $i $(date) state=$STATE hasip=$HASIP ====="
    echo "[MAC(应=24:cd:8d:4e:4f:28)]"; cat /sys/class/net/$IFACE/address 2>/dev/null
    echo "[IP]"; ip -4 addr show "$IFACE" | grep inet
    echo "[wpa]"; wpa_cli -i "$IFACE" status 2>/dev/null | grep -E 'wpa_state|^ssid|ip_address'
    echo "[看得到JumboPlus吗]"; wpa_cli -i "$IFACE" scan_results 2>/dev/null | grep -iE 'JumboPlus'
    sync; } >> "$DBG" 2>&1
  i=$((i+1)); sleep 15
done
