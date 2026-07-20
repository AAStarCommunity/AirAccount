#!/usr/bin/env bash
# dk2-wifi-school.sh — 通过【串口】把 DK2 配成开机自动连【学校 @JumboPlusIoT】(WPA-PSK)。
#
# 做什么(全走 dk2.sh 串口,不用拔卡):
#   1. 去掉公寓 MAC 克隆(macclone drop-in)→ DK2 用【真 MAC】(学校门户按真 MAC 注册)
#   2. 写 /etc/wpa_supplicant/wpa_supplicant-wlan0.conf(SSID+PSK 从 ~/Dev/.env 读,不硬编码/不入库)
#   3. 装 wifi-up.sh + wifi-autoconnect.service(接口自探测 + wpa -B + dhcp + 黑匣子)并 enable
#
# 前提: ~/Dev/.env 有 DK2_WIFI_SSID_SCHOOL / DK2_WIFI_PSK_SCHOOL / DK2_WIFI_MAC;DK2 串口连着。
# 用:  ./dk2-wifi-school.sh
#
# 注: DK2 配成学校网后,在公寓不会连(除非你在 UniFi 授权 DK2 真 MAC)。目的=它到学校/机房自动上线,与板A同网。
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
DK2="$HERE/dk2.sh"
WIFI_ASSETS="$HERE/../../docs/dk2-school-wifi"   # 复用现成 wifi-up.sh + service
ENVF="${ENV_FILE:-$HOME/Dev/.env}"

# 读 env:剥行内注释(# 起)+ 两端空白 + 包围引号(PSK/SSID/MAC 都无内部 # 或空格)。
getenv() { sed -n "s/^$1=//p" "$ENVF" | head -1 | sed -E 's/#.*$//' | sed -E 's/^[[:space:]]*"?//; s/"?[[:space:]]*$//'; }

SSID="$(getenv DK2_WIFI_SSID_SCHOOL)"
PSK="$(getenv DK2_WIFI_PSK_SCHOOL)"
MAC="$(getenv DK2_WIFI_MAC)"
[ -n "$SSID" ] && [ -n "$PSK" ] || { echo "❌ ~/Dev/.env 缺 DK2_WIFI_SSID_SCHOOL / DK2_WIFI_PSK_SCHOOL"; exit 1; }
BASE="${SSID%5GHz}"   # env SSID 可能已含 5GHz 后缀 → 剥掉取 base(2.4GHz),再拼 5GHz 那条
echo "== 配 DK2 学校 wifi: base=$BASE (+5GHz)  真MAC=${MAC:-<hw默认>} =="

# 1. 本地生成 wpa 配置(5GHz 优先 + 2.4GHz base,PSK 从 env)
TMP="$(mktemp)"; trap 'rm -f "$TMP"' EXIT
cat > "$TMP" <<EOF
ctrl_interface=/var/run/wpa_supplicant
ctrl_interface_group=0
update_config=1

network={
	ssid="${BASE}5GHz"
	psk="$PSK"
	key_mgmt=WPA-PSK
	scan_ssid=1
	priority=50
}
network={
	ssid="$BASE"
	psk="$PSK"
	key_mgmt=WPA-PSK
	scan_ssid=1
	priority=49
}
EOF

echo "[1/4] 串口推 wpa 配置 → DK2"
"$DK2" put "$TMP" /etc/wpa_supplicant/wpa_supplicant-wlan0.conf

echo "[2/4] 去掉公寓 MAC 克隆(用真 MAC)"
"$DK2" run 'rm -f /etc/systemd/system/wpa_supplicant@wlan0.service.d/macclone.conf /etc/systemd/system/wifi-wlan0.service.d/macclone.conf; echo removed-macclone' 6

echo "[3/4] 装 wifi-up.sh + wifi-autoconnect.service"
"$DK2" put "$WIFI_ASSETS/wifi-up.sh" /home/root/wifi-up.sh
"$DK2" put "$WIFI_ASSETS/wifi-autoconnect.service" /etc/systemd/system/wifi-autoconnect.service
"$DK2" run 'chmod 0755 /home/root/wifi-up.sh; systemctl daemon-reload; systemctl enable wifi-autoconnect.service 2>&1 | tail -1; echo enabled' 8

echo "[4/4] 验证配置(SSID + 无克隆 + 服务 enabled)"
"$DK2" run 'echo "--wpa SSID--"; grep ssid= /etc/wpa_supplicant/wpa_supplicant-wlan0.conf; echo "--macclone(应无)--"; ls /etc/systemd/system/*/macclone.conf 2>/dev/null || echo none; echo "--autoconnect enabled?--"; systemctl is-enabled wifi-autoconnect.service 2>/dev/null' 8

echo "✅ DK2 已配学校 wifi 自启。带到学校/机房上电即自动连 @JumboPlusIoT(真 MAC + PSK)。"
echo "   公寓这边它不会连(除非在 UniFi 授权 DK2 真 MAC)。"
