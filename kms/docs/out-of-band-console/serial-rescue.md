# 串口救板工具(已移至 kms/scripts/oob/)

板子上电但 off-network 时用调试串口发命令/关机的工具已从本目录移到
**[`kms/scripts/oob/`](../../scripts/oob/)**(它们是 mode 100755 的可执行运维工具,
放 scripts 下才进 lint/评审/测试覆盖,不该藏在 docs)。

- `serial-run.py` —— 通用串口命令执行器(nonce 标记 + 硬 deadline + JSON)
- `mx93b-serial-poweroff.sh` —— 优雅关闭板 B(带 fail-safe 守卫)
- `test-serial-run.py` —— pty+bash 本地测试
- 说明见 [`kms/scripts/oob/README.md`](../../scripts/oob/README.md)
