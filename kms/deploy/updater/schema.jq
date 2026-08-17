# schema.jq —— channel manifest schema **单一事实源**(#203)
#
# 曾经是两份手写 jq 谓词:节点 aastar-node-updater.sh 的 load_manifest 与签发端
# release-sign.sh 的自检。已漂移 3 次(canary_ring 元素类型、expires 格式、revoked),
# 每次都在 #196 某轮被 daemon 抓到。抽成一份、两边 `jq --from-file` 引用后,漂移在
# **语法层**就不可能——改一处即两处同变。
#
# 两侧的**真实差异**显式化为 --argjson 参数(不再是各自手写的分叉):
#   $nmax                    (number)  notes 上限(两侧都 280)
#   $require_floor           (bool)    true=rollback_floor 必是 string(签发端:自己总会设);
#                                      false=允许 null(节点:兼容墓碑前发布的旧 manifest)
#   $check_revoked_releases  (bool)    true=releases 不得含 revoked 里的版本(签发端强不变量,
#                                      墓碑=唯一撤销机制 #196 R6);节点在读取侧过滤,schema 不查
#
# 逐字段一律取**更宽容的超集形**(`// 默认` + 显式 type 判定):签发端产物恒含这些字段,
# 宽容形对它零影响;节点本就要宽容(继承的旧条目/缺省)。fail-closed:任一不满足 → jq -e 非零。
#
# 调用:
#   节点  jq -e --argjson nmax 280 --argjson require_floor false --argjson check_revoked_releases false --from-file schema.jq m.json
#   签发  echo "$MANIFEST" | jq -e --argjson nmax "$NOTES_MAX" --argjson require_floor true --argjson check_revoked_releases true --from-file schema.jq

def _semver: test("^v?[0-9]+\\.[0-9]+\\.[0-9]+$");

  # metadata_version 必是整数(.==floor:等于自身向下取整)—— 防小数绕过防回滚比较
  (.metadata_version | type=="number" and . == floor)
  # expires 严格 ISO-8601 UTC(新鲜度锚点)
  and (.expires | type=="string" and test("^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z$"))
  # rollback_floor:$require_floor 决定是否允许 null
  and (if $require_floor
       then (.rollback_floor | type=="string" and _semver)
       else (.rollback_floor == null or (.rollback_floor | type=="string" and _semver)) end)
  # revoked[]:恒为字符串 semver 数组(缺字段当空数组)
  and ((.revoked // []) | type=="array" and (all(.[]; type=="string" and _semver)))
  # 签发端强不变量:releases 不得含 revoked 里的版本(墓碑=唯一撤销机制;节点读取侧过滤,不在此查)
  and (if $check_revoked_releases
       then (((.revoked // []) | map(ltrimstr("v"))) as $rv
             | (.releases | all((.version | ltrimstr("v")) as $vv | ($rv | index($vv)) == null)))
       else true end)
  # releases 允许空数组:撤销最后一版是合法的(#196 R8 finding3)
  and (.releases | type=="array")
  and (all(.releases[];
        (.version | type=="string" and _semver)
        and (.tarball | type=="string" and (length > 0))
        and (.sha256 | type=="string" and test("^[0-9a-fA-F]{64}$"))
        and ((.security // false) | type=="boolean")
        and ((.auto_apply_allowed // false) | type=="boolean")
        and ((.ta_changed // false) | type=="boolean")
        and ((.severity // "none") | type=="string" and test("^(none|low|medium|high|critical)$"))
        and (.notes == null or (.notes | type=="string" and (test("[[:cntrl:]]") | not) and (length <= $nmax)))
        and ((.min_version // "0.0.0") | type=="string" and _semver)
        and (.requires_ta_version == null or (.requires_ta_version | type=="string" and _semver))
        # canary_ring 元素必须是**字符串** node id(数值会被节点整份拒 → 全网冻结)
        and (.canary_ring == null or ((.canary_ring | type=="array") and (all(.canary_ring[]; type=="string"))))
      ))
