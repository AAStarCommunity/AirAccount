#!/usr/bin/env python3
"""dk2-serial.py — DK2(STM32MP157F-DK2)串口 run helper。

DK2 串口 = micro-USB CN11(ST-LINK VCP,必须【数据线】)。串口自动 root 登录。
用:
  dk2-serial.py <dev> run "<命令>" [超时秒]   在 DK2 跑命令,打印其 stdout(去回显)
  dk2-serial.py <dev> put <本地文件> <远程路径>  串口传【小】文件(base64,慢,仅 config 级)
需 pyserial:  pip3 install pyserial
"""
import base64
import sys
import time

try:
    import serial
except ImportError:
    sys.exit("需 pyserial: pip3 install pyserial")

BAUD = 115200
M1, M2 = "__DK2_S__", "__DK2_E__"


def _open(dev):
    s = serial.Serial(dev, BAUD, timeout=1)
    time.sleep(0.3)
    s.reset_input_buffer()
    return s


def run(dev, cmd, timeout=8):
    """跑一条命令,返回 M1..M2 之间的真实输出(去掉命令回显)。"""
    s = _open(dev)
    s.write(b"\r\n")
    time.sleep(0.2)
    s.write(f"echo {M1}; {cmd}; echo {M2}\r\n".encode())
    buf = ""
    t0 = time.time()
    while time.time() - t0 < timeout:
        buf += s.read(4096).decode(errors="replace")
        i1 = buf.rfind(M1)
        if i1 >= 0 and buf.find(M2, i1 + len(M1)) >= 0:
            break
    s.close()
    i1 = buf.rfind(M1)
    i2 = buf.find(M2, i1 + len(M1)) if i1 >= 0 else -1
    if i1 >= 0 and i2 >= 0:
        return buf[i1 + len(M1):i2].strip("\r\n")
    return buf.strip()


def put(dev, local, remote):
    """把小文件 base64 过串口写到 DK2(仅 config 级小文件;大文件走网络)。"""
    with open(local, "rb") as f:
        b64 = base64.b64encode(f.read()).decode()
    if len(b64) > 60000:
        sys.exit("文件过大,串口传不划算(>~45KB),请走网络")
    # 分块写,避免行过长
    run(dev, f"rm -f {remote}.b64", 4)
    for i in range(0, len(b64), 400):
        run(dev, f"printf '%s' '{b64[i:i+400]}' >> {remote}.b64", 4)
    out = run(dev, f"base64 -d {remote}.b64 > {remote} && rm -f {remote}.b64 && echo OK-$(wc -c <{remote})", 6)
    print(out)


if __name__ == "__main__":
    if len(sys.argv) < 3:
        sys.exit(__doc__)
    dev, action = sys.argv[1], sys.argv[2]
    if action == "run":
        print(run(dev, sys.argv[3], int(sys.argv[4]) if len(sys.argv) > 4 else 8))
    elif action == "put":
        put(dev, sys.argv[3], sys.argv[4])
    else:
        sys.exit("action: run | put")
