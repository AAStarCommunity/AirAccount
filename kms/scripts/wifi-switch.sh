#!/bin/sh
# wifi-switch.sh — 手动切换 MX93 WiFi 网络
# 用法：wifi-switch.sh [home|school|status]
#   home   → ChinaNet-AuRfsu-5G（公寓）
#   school → @JumboPlusIoT5GHz（CMU学校）
#   status → 显示当前连接（默认）

IFACE=mlan0

status() {
    STATE=$(wpa_cli -i $IFACE status 2>/dev/null | grep wpa_state | cut -d= -f2)
    SSID=$(wpa_cli -i $IFACE status 2>/dev/null | grep "^ssid=" | cut -d= -f2)
    IP=$(ip addr show $IFACE 2>/dev/null | grep 'inet ' | awk '{print $2}')
    echo "WiFi: $STATE | SSID: $SSID | IP: $IP"
}

select_network() {
    TARGET_SSID="$1"
    # 精确匹配 ssid 字段(第 2 列,tab 分隔),不用 `$0 ~ s` 整行子串/正则 —— 否则
    # SSID 与别的行前缀重叠(如 @JumboPlusIoT vs @JumboPlusIoT5GHz)会连错网。
    NETID=$(wpa_cli -i $IFACE list_networks 2>/dev/null | awk -F'\t' -v s="$TARGET_SSID" '$2==s {print $1}' | head -1)
    if [ -z "$NETID" ]; then
        echo "[ERROR] 未找到网络: $TARGET_SSID（检查 /etc/wpa_supplicant.conf）"
        exit 1
    fi
    echo "[INFO] 切换到 $TARGET_SSID (id=$NETID)..."
    wpa_cli -i $IFACE select_network "$NETID" > /dev/null

    # 等连接（最多 30 秒）
    for i in $(seq 1 30); do
        STATE=$(wpa_cli -i $IFACE status 2>/dev/null | grep wpa_state | cut -d= -f2)
        SSID=$(wpa_cli -i $IFACE status 2>/dev/null | grep "^ssid=" | cut -d= -f2)
        printf "%ds: %s / %s\r" "$i" "$STATE" "$SSID"
        if [ "$STATE" = "COMPLETED" ] && [ "$SSID" = "$TARGET_SSID" ]; then
            echo
            # 续 DHCP
            udhcpc -i $IFACE -n -q 2>/dev/null
            status
            echo "[OK] 连接成功"
            return 0
        fi
        sleep 1
    done

    echo
    echo "[WARN] 未能连接 $TARGET_SSID，当前状态："
    status
    return 1
}

case "$1" in
    home)
        select_network "ChinaNet-AuRfsu-5G"
        ;;
    school)
        select_network "@JumboPlusIoT5GHz"
        ;;
    status|"")
        status
        ;;
    *)
        echo "用法: $0 [home|school|status]"
        exit 1
        ;;
esac
