#!/usr/bin/env bash
# finalize-helper.sh —— aastar-node-setup 收尾里【唯一需要 root】的动作,单列成特权 helper。
#
# 目的(#23):让向导本体可降权(service `User=` 非 root)跑,只这条 helper 经 sudo NOPASSWD
# 白名单拿 root,把向导的 root 攻击面收窄到"这几条固定 systemctl + 删一个 drop-in"。
#
# 幂等:重复跑安全(rm -f / systemctl 尽力而为)。向导优先 `sudo -n` 调它;不可用则向导
# 回落直接 systemctl(仅当向导以 root 跑,向后兼容旧部署)。
set -u

PROV="/etc/systemd/system/kms-api.service.d/prov.conf"

# 失败信息保留到 stderr(向导会捕获成 warnings 回前端),但 `|| true` 保证退出码 0
# (尽力而为,某步失败不阻断收尾)。安全关键的关 gate(①)最先做。
# ① 关 KMS provisioning gate(删 drop-in,防再被 /gen-key 灌)。
rm -f "$PROV" || true
# ② 让 kms-api/dvt 读新 /etc/airaccount/*.env。
systemctl daemon-reload || true
systemctl restart kms-api.service || true
systemctl restart dvt.service || true
# ③ disable 本向导(下次开机不再首启)。
systemctl disable aastar-node-setup.service || true

exit 0
