#!/usr/bin/env python3
"""通用串口控制台命令执行器 —— 带外救板(板子上电但 SSH/tailscale 不通时)用。

对着板子的调试串口发命令、抓输出。用 `echo __E__$?` 标记分隔每条命令输出,
自动登录(login:/Password: 提示时),剥除 ANSI 转义。对会导致系统 halt 的命令
(poweroff/reboot)用 --read-secs 直接原始流读 N 秒(不等标记)。

依赖: pyserial  (pip3 install pyserial)

示例:
  # 诊断:每条命令带 rc、干净输出
  ./serial-run.py --dev /dev/cu.usbmodem5B6D0040831 'ip -br addr' 'systemctl is-active kms-api'
  # 只有一个 usbmodem 时自动选设备
  ./serial-run.py 'uname -a'
  # 列出候选设备
  ./serial-run.py --list
  # 关机/重启:发命令后原始流读 25s 看关机日志
  ./serial-run.py --dev /dev/cu.usbmodem5B6D0040831 --read-secs 25 'systemctl poweroff'

退出码 = 最后一条(带标记)命令的 rc;--read-secs 模式恒 0。
"""
import argparse, glob, re, sys, time

try:
    import serial  # pyserial
except ImportError:
    sys.exit("需要 pyserial: pip3 install pyserial")

ANSI = re.compile(r'\x1b\[[0-9;?]*[A-Za-z]|\x1b[()][B0]|\x1b[78]')


def list_devices():
    return sorted(glob.glob('/dev/cu.usbmodem*') + glob.glob('/dev/cu.usbserial*')
                  + glob.glob('/dev/ttyUSB*') + glob.glob('/dev/ttyACM*'))


def find_dev(spec):
    if spec and spec != 'auto':
        return spec
    cands = list_devices()
    if len(cands) == 1:
        return cands[0]
    if not cands:
        sys.exit("找不到串口设备(/dev/cu.usbmodem* 等)—— 检查线是数据线、口是调试口")
    sys.exit("多个串口设备,请用 --dev 明确指定:\n  " + "\n  ".join(cands))


def main():
    ap = argparse.ArgumentParser(description="串口控制台命令执行器(带外救板)")
    ap.add_argument('--dev', default='auto', help="串口设备,或 'auto'(仅一个时自动选,默认)")
    ap.add_argument('--baud', type=int, default=115200)
    ap.add_argument('--login', default='root', help="遇到 login: 时发送的用户名(默认 root)")
    ap.add_argument('--password', default='', help="遇到 Password: 时发送(默认空)")
    ap.add_argument('--timeout', type=float, default=8, help="每条命令等待秒数(默认 8)")
    ap.add_argument('--read-secs', type=float, default=0,
                    help=">0 时:最后一条命令发出后原始流读 N 秒(用于 poweroff/reboot,不等标记)")
    ap.add_argument('--list', action='store_true', help="列出候选串口设备后退出")
    ap.add_argument('cmds', nargs='*', help="按顺序执行的命令")
    a = ap.parse_args()

    if a.list:
        devs = list_devices()
        print("\n".join(devs) if devs else "(无串口设备)")
        return

    dev = find_dev(a.dev)
    print(f"[serial-run] dev={dev} baud={a.baud}", file=sys.stderr)
    s = serial.Serial(dev, a.baud, timeout=1)

    def drain(t):
        end = time.time() + t
        b = b''
        while time.time() < end:
            n = s.in_waiting
            if n:
                b += s.read(n)
                end = time.time() + t     # 有数据就续读,直到静默 t 秒
            else:
                time.sleep(0.1)
        return b.decode('utf-8', 'replace')

    def slow(txt):
        # 逐字慢写:getty/行规程偶尔吞开头字符,慢写更稳
        for ch in txt:
            s.write(ch.encode())
            s.flush()
            time.sleep(0.004)

    def send(line):
        slow(line + '\r')

    # 唤醒 + 按需登录
    s.write(b'\r')
    time.sleep(0.5)
    o = drain(1.2)
    if 'login:' in o:
        send(a.login)
        time.sleep(1.5)
        o2 = drain(1.5)
        if 'assword' in o2:
            send(a.password)
            time.sleep(1.5)
            drain(1.5)

    rc = 0
    for i, cmd in enumerate(a.cmds):
        last = (i == len(a.cmds) - 1)
        drain(0.3)
        s.write(b'\r')
        time.sleep(0.2)
        drain(0.3)
        print(f"$ {cmd}")

        if last and a.read_secs > 0:
            send(cmd)
            buf = ''
            end = time.time() + a.read_secs
            while time.time() < end:
                buf += drain(0.6)
            sys.stdout.write(ANSI.sub('', buf))
            break

        send(cmd + '; echo __E__$?')
        buf = ''
        end = time.time() + a.timeout
        while time.time() < end:
            buf += drain(0.4)
            if '__E__' in buf:
                break
        clean = ANSI.sub('', buf)
        m = re.search(r'__E__(\d+)', clean)
        if m:
            rc = int(m.group(1))
        body = clean[:clean.find('__E__')] if '__E__' in clean else clean
        # 丢掉第一行「命令回显」,其余即命令输出
        lines = body.splitlines()
        dropped = False
        out = []
        for ln in lines:
            if not dropped and cmd.split(';')[0].strip()[:12] in ln:
                dropped = True
                continue
            out.append(ln)
        text = '\n'.join(out).strip('\n')
        if text.strip():
            sys.stdout.write(text + '\n')

    s.close()
    sys.exit(rc)


if __name__ == '__main__':
    main()
