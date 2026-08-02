#!/usr/bin/env python3
"""serial-run.py 的本地测试:用 pty + bash 当假串口(无需硬件)。

直接验 PR#193 的两条 Blocking 修复:
  B1 —— nonce 标记框住真实输出(不再被命令回显截断丢弃)
  B2 —— drain 绝对 deadline:控制台无限刷日志时 run_cmd 仍在 timeout 内返回(不挂死)
以及 rc 正确、多行输出、失败命令 rc、伪标记不误判、连续命令不错位。
"""
import importlib.util, os, pty, select, sys, time

HERE = os.path.dirname(os.path.abspath(__file__))
spec = importlib.util.spec_from_file_location("serialrun", os.path.join(HERE, "serial-run.py"))
m = importlib.util.module_from_spec(spec)
spec.loader.exec_module(m)

PASS = 0; FAIL = 0
def check(cond, msg):
    global PASS, FAIL
    if cond:
        PASS += 1; print(f"  \033[0;32mPASS\033[0m {msg}")
    else:
        FAIL += 1; print(f"  \033[0;31mFAIL\033[0m {msg}")


class PtyTransport(m.Transport):
    def __init__(self, argv, env):
        self.pid, self.fd = pty.fork()
        if self.pid == 0:                       # 子进程 = 假板子的 shell
            os.environ.update(env)
            os.execvp(argv[0], argv)
        # 父进程

    def write(self, b): os.write(self.fd, b)

    def read(self, timeout):
        try:
            r, _, _ = select.select([self.fd], [], [], timeout)
        except OSError:
            return b''
        if r:
            try:
                return os.read(self.fd, 4096)
            except OSError:
                return b''
        return b''

    def close(self):
        try: os.close(self.fd)
        except OSError: pass
        try: os.waitpid(self.pid, os.WNOHANG)
        except OSError: pass


def new_shell():
    # 交互 bash 当假 getty shell:pty 默认回显开(和真串口/getty 一样会回显输入)
    tp = PtyTransport(["bash", "--norc", "--noprofile", "-i"], {"PS1": "> ", "TERM": "dumb"})
    m.drain(tp, quiet=0.4, hard_deadline=time.time() + 2)   # 吃掉启动 + 首个提示符
    return tp


print("== serial-run.py pty+bash 测试 ==")

tp = new_shell()
try:
    rc, out = m.run_cmd(tp, "echo hello", 5, os.urandom(5).hex())
    check(rc == 0 and out == "hello", f"B1 单行真实输出被捕获 (out={out!r} rc={rc})")

    rc, out = m.run_cmd(tp, "echo l1; echo l2", 5, os.urandom(5).hex())
    check(rc == 0 and out == "l1\nl2", f"多行输出完整 (out={out!r} rc={rc})")

    rc, out = m.run_cmd(tp, "false", 5, os.urandom(5).hex())
    check(rc == 1, f"失败命令 rc=1 (rc={rc} out={out!r})")

    rc, out = m.run_cmd(tp, "echo __E__deadbeef_0", 5, os.urandom(5).hex())
    check(rc == 0 and out == "__E__deadbeef_0", f"输出含伪标记不误判 (out={out!r} rc={rc})")

    r1, _ = m.run_cmd(tp, "true", 5, os.urandom(5).hex())
    r2, o2 = m.run_cmd(tp, "echo again", 5, os.urandom(5).hex())
    check(r1 == 0 and r2 == 0 and o2 == "again", f"连续命令 rc/输出不错位 (r1={r1} r2={r2} o2={o2!r})")
finally:
    tp.close()

# B2: 无限刷日志时 run_cmd 在 timeout 内返回(不挂死),rc=124
tp = new_shell()
try:
    t0 = time.time()
    rc, out = m.run_cmd(tp, "yes spam", 2, os.urandom(5).hex())   # yes = 无限输出,永不结束
    elapsed = time.time() - t0
    check(elapsed < 5 and rc == 124, f"B2 无限刷日志 {elapsed:.1f}s 内返回 rc=124(未挂死)")
finally:
    tp.close()

print(f"\n结果: PASS={PASS} FAIL={FAIL}")
sys.exit(1 if FAIL else 0)
