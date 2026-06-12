#!/usr/bin/env python3
"""Fire ONE CreateKey and capture BOTH MX93 UARTs continuously to catch the
OP-TEE secure-world panic, wherever it prints (A55 console ...901 or M33 ...903).

Reads the full window with NO early break (avoids the command-echo false marker).
Run right after a fresh power-cycle, with CreateKey enabled in the service.
"""
import serial, time, sys, threading

A55 = "/dev/cu.usbmodem5B6D0044901"   # Linux + (usually) OP-TEE console
M33 = "/dev/cu.usbmodem5B6D0044903"   # Cortex-M33 (OP-TEE may log here)
BAUD = 115200
WINDOW = 90
PASSKEY = "04f77294861c7328e3ef41abd95e64508dc796dbd6843809dcbece1f940ea3e3d25d1fa87012cdb276229c738408e038fee6f8dd46327f83e28da6dc25c2538122"
CURL = (
    'curl -s -m 40 -X POST http://localhost:3000/CreateKey '
    '-H "x-amz-target: TrentService.CreateKey" '
    '-H "Content-Type: application/json" '
    '-d \'{"KeySpec":"ECC_NIST_P256","KeyUsage":"SIGN_VERIFY",'
    '"Description":"panic-cap","Origin":"AWS_KMS",'
    '"PasskeyPublicKey":"' + PASSKEY + '"}\' > /tmp/ck.out 2>&1; '
    'echo CKDONE_$? > /tmp/ck.done\r\n'
)

stop = False
def reader(dev, tag):
    try:
        s = serial.Serial(dev, BAUD, timeout=0.3)
    except Exception as e:
        print(f"[{tag}] open failed: {e}"); return
    while not stop:
        c = s.read(4096)
        if c:
            for line in c.decode(errors="replace").splitlines():
                if line.strip():
                    print(f"[{tag}] {line}", flush=True)
    s.close()

# start both readers
t1 = threading.Thread(target=reader, args=(A55, "A55"), daemon=True)
t2 = threading.Thread(target=reader, args=(M33, "M33"), daemon=True)
t1.start(); t2.start()
time.sleep(1)

# fire CreateKey on the A55 console
print("=== firing CreateKey on A55 ===", flush=True)
sa = serial.Serial(A55, BAUD, timeout=0.3)
sa.write(b"\r\n"); time.sleep(0.5)
sa.write(CURL.encode()); sa.close()

t0 = time.time()
while time.time() - t0 < WINDOW:
    time.sleep(1)
stop = True
time.sleep(1)
print(f"=== capture window ({WINDOW}s) ended ===", flush=True)
