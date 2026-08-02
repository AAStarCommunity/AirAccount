#!/usr/bin/env python3
"""串口控制台命令执行器 —— 带外救板(板子上电但 SSH/tailscale 不通)时用。

对着板子的调试串口发命令、抓输出。每次运行生成一个 nonce,用一对标记
`__B__<nonce>` / `__E__<nonce>_<rc>` 精确框住每条命令的真实输出与退出码 ——
命令回显里出现的是 `%s`/`$?` 字面量,不会撞到标记(见 PR#193 Blocking 1)。
`drain()` 用绝对 deadline + 缓冲上限,控制台持续刷日志也不会挂死(Blocking 2)。

依赖: pyserial  (pip3 install pyserial)

用法:
  serial-run.py --dev /dev/cu.usbmodem…831 'ip -br addr' 'systemctl is-active kms-api'
  serial-run.py --json 'whoami' 'hostname'      # 机器可读:每条 {cmd,rc,out}
  serial-run.py --list                          # 列候选设备
  serial-run.py --print-dev                     # 解析并打印将使用的设备(供 wrapper 定一次)
  serial-run.py --dev … --read-secs 25 'systemctl poweroff'   # 关机/重启:流式读 N 秒

密码从环境变量 SERIAL_PASSWORD 读(不走 argv,避免 ps 泄露);默认空(root 免密板)。
退出码:非 --read-secs 模式 = 最后一条命令 rc(未读到标记=124 超时);--read-secs 恒 0。
"""
import argparse, fcntl, glob, json, os, re, sys, time

ANSI = re.compile(r'\x1b\[[0-9;?]*[A-Za-z]|\x1b[()][B0]|\x1b[78]')
CAP = 512 * 1024          # drain 缓冲上限,防无界增长
QUIET = 0.3               # 静默判定秒


def list_devices():
    pats = ('/dev/cu.usbmodem*', '/dev/cu.usbserial*', '/dev/ttyUSB*', '/dev/ttyACM*')
    out = []
    for p in pats:
        out += glob.glob(p)
    return sorted(out)


def find_dev(spec):
    if spec and spec != 'auto':
        return spec
    cands = list_devices()
    if len(cands) == 1:
        return cands[0]
    if not cands:
        sys.exit("找不到串口设备(/dev/cu.usbmodem* 等)—— 检查线是数据线、口是调试口")
    sys.exit("多个串口设备,请用 --dev 明确指定:\n  " + "\n  ".join(cands))


class Transport:
    """传输层抽象:真串口用 pyserial;测试用 pty(见 test-serial-run.py)。"""
    def write(self, b): raise NotImplementedError
    def read(self, timeout): raise NotImplementedError   # 返回 bytes(可空)
    def close(self): pass


class SerialTransport(Transport):
    def __init__(self, dev, baud):
        import serial
        self.s = serial.Serial(dev, baud, timeout=0)
        # 独占锁:macOS /dev/cu.* 非排他,防和 screen/minicom 抢串口(PR#193 M)
        try:
            fcntl.flock(self.s.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
        except OSError:
            self.s.close()
            sys.exit(f"串口 {dev} 被占用(screen/minicom 还连着?)—— 先断开再试")

    def write(self, b):
        self.s.write(b); self.s.flush()

    def read(self, timeout):
        end = time.time() + timeout
        while time.time() < end:
            n = self.s.in_waiting
            if n:
                return self.s.read(n)
            time.sleep(0.02)
        return b''

    def close(self):
        try: self.s.close()
        except Exception: pass


def drain(tp, quiet=QUIET, hard_deadline=None, cap=CAP):
    """读到静默 quiet 秒、或到绝对 hard_deadline、或缓冲达 cap —— 三者任一即返回。"""
    buf = b''; last = time.time()
    while True:
        now = time.time()
        if hard_deadline is not None and now >= hard_deadline:
            break
        chunk = tp.read(0.1)
        if chunk:
            buf += chunk; last = time.time()
            if len(buf) >= cap:
                break
        elif time.time() - last >= quiet:
            break
    return buf.decode('utf-8', 'replace')


def slow_write(tp, txt):
    for ch in txt:                       # 逐字慢写:getty 偶尔吞开头字符
        tp.write(ch.encode()); time.sleep(0.004)


def run_cmd(tp, cmd, timeout, nonce):
    """执行一条命令,用 nonce 标记框住输出。返回 (rc, out)。未读到结束标记 → rc=124。"""
    drain(tp, quiet=0.2, hard_deadline=time.time() + 0.6)   # 清残留
    tp.write(b'\r'); drain(tp, quiet=0.2, hard_deadline=time.time() + 0.4)
    # printf 的 \n 在回显里是字面量,只有真正执行时才产生真换行 → 标记不撞回显
    line = "printf '__B__%s\\n'; %s; printf '__E__%s_%%s\\n' \"$?\"" % (nonce, cmd, nonce)
    slow_write(tp, line + '\r')
    raw = drain(tp, quiet=QUIET, hard_deadline=time.time() + timeout)
    text = ANSI.sub('', raw)
    mB = re.search(r'__B__%s\r?\n' % nonce, text)
    mE = re.search(r'__E__%s_(\d+)' % nonce, text)
    if mB and mE and mB.end() <= mE.start():
        body = text[mB.end():mE.start()].replace('\r\n', '\n').replace('\r', '\n')
        return int(mE.group(1)), body.strip('\n')
    return 124, ''            # 超时/没框到 —— 必须是错误,绝不沿用上一条 rc


def ensure_login(tp, user, password):
    tp.write(b'\r')
    txt = drain(tp, quiet=0.4, hard_deadline=time.time() + 3)
    if 'login:' in txt or txt.rstrip().endswith('login:'):
        slow_write(tp, user + '\r')
        txt2 = drain(tp, quiet=0.5, hard_deadline=time.time() + 4)
        if re.search(r'[Pp]assword:', txt2):
            if not password:
                sys.exit("板子要密码但未提供 —— 设 SERIAL_PASSWORD 环境变量")
            slow_write(tp, password + '\r')       # 只在 Password: 处发密码,绝不在 login: 处
            txt3 = drain(tp, quiet=0.5, hard_deadline=time.time() + 5)
            if re.search(r'[Pp]assword:', txt3):
                sys.exit("登录失败(仍停在 Password:)—— 密码错?")
    # 预热:首条真实命令前用 nonce 往返确认 shell 就绪 —— 否则登录/唤醒延迟会吃掉第一条命令
    # 导致 rc=124 假失败(PR#193 M:首命令时序)。板子真没起时 3 次都失败,不在此中止,
    # 交给后续命令各自的 rc/超时如实反映(守卫据此 fail-safe)。
    for _ in range(3):
        r, _ = run_cmd(tp, 'true', 4, os.urandom(5).hex())
        if r == 0:
            return


def main():
    ap = argparse.ArgumentParser(description="串口控制台命令执行器(带外救板)")
    ap.add_argument('--dev', default='auto', help="串口设备,或 'auto'(仅一个时自动选)")
    ap.add_argument('--baud', type=int, default=115200)
    ap.add_argument('--login', default='root')
    ap.add_argument('--timeout', type=float, default=8, help="每条命令上界秒(默认 8)")
    ap.add_argument('--read-secs', type=float, default=0,
                    help=">0:最后一条命令发出后流式读 N 秒(poweroff/reboot 用),恒退出 0")
    ap.add_argument('--json', action='store_true', help="输出每条 {cmd,rc,out} 的 JSON 数组")
    ap.add_argument('--list', action='store_true')
    ap.add_argument('--print-dev', action='store_true', help="解析并打印设备后退出(供 wrapper 定一次)")
    ap.add_argument('cmds', nargs='*')
    a = ap.parse_args()

    if a.list:
        d = list_devices(); print("\n".join(d) if d else "(无串口设备)"); return
    if a.print_dev:
        print(find_dev(a.dev)); return

    dev = find_dev(a.dev)
    password = os.environ.get('SERIAL_PASSWORD', '')
    if not a.json:
        print(f"[serial-run] dev={dev} baud={a.baud}", file=sys.stderr)

    tp = SerialTransport(dev, a.baud)
    results = []
    rc = 0
    try:
        ensure_login(tp, a.login, password)
        for i, cmd in enumerate(a.cmds):
            last = (i == len(a.cmds) - 1)
            if last and a.read_secs > 0:
                if not a.json:
                    print(f"$ {cmd}")
                slow_write(tp, cmd + '\r')
                end = time.time() + a.read_secs           # 流式:实时打印,不缓冲到最后
                while time.time() < end:
                    chunk = tp.read(0.3)
                    if chunk:
                        sys.stdout.write(ANSI.sub('', chunk.decode('utf-8', 'replace')))
                        sys.stdout.flush()
                rc = 0
                break
            nonce = os.urandom(5).hex()
            r, out = run_cmd(tp, cmd, a.timeout, nonce)
            rc = r
            results.append({"cmd": cmd, "rc": r, "out": out})
            if not a.json:
                print(f"$ {cmd}")
                if out:
                    print(out)
                if r == 124:
                    print("  [!] 超时/未读到结果标记", file=sys.stderr)
    finally:
        tp.close()

    if a.json:
        print(json.dumps(results, ensure_ascii=False))
    sys.exit(0 if a.read_secs > 0 else rc)


if __name__ == '__main__':
    main()
