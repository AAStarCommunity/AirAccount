#!/usr/bin/env python3
"""Capture the OP-TEE console/panic output during ONE CreateKey on the MX93.

Opens the A55 serial console, fires a single localhost CreateKey, and reads the
console continuously (printing with elapsed time) so we capture any OP-TEE
secure-world panic that prints to the UART right before the board freezes.

This is a DIAGNOSTIC that intentionally risks crashing the board once.
"""
import serial, time, sys

DEV = "/dev/cu.usbmodem5B6D0044901"
BAUD = 115200
PASSKEY = "04f77294861c7328e3ef41abd95e64508dc796dbd6843809dcbece1f940ea3e3d25d1fa87012cdb276229c738408e038fee6f8dd46327f83e28da6dc25c2538122"
CURL = (
    'curl -s -m 30 -X POST http://localhost:3000/CreateKey '
    '-H "x-amz-target: TrentService.CreateKey" '
    '-H "Content-Type: application/json" '
    '-d \'{"KeySpec":"ECC_NIST_P256","KeyUsage":"SIGN_VERIFY",'
    '"Description":"panic-capture","Origin":"AWS_KMS",'
    '"PasskeyPublicKey":"' + PASSKEY + '"}\'; echo RESP_DONE_$?\r\n'
)

s = serial.Serial(DEV, BAUD, timeout=0.2)
# wake + drain
s.write(b"\r\n")
time.sleep(1)
s.reset_input_buffer()

print("=== firing CreateKey, capturing console for 75s ===", flush=True)
t0 = time.time()
s.write(CURL.encode())

buf = b""
last_print = t0
while time.time() - t0 < 75:
    chunk = s.read(4096)
    if chunk:
        buf += chunk
        sys.stdout.write(chunk.decode(errors="replace"))
        sys.stdout.flush()
    # if we saw the response marker, the call returned (no crash)
    if b"RESP_DONE_" in buf:
        print("\n=== CreateKey RETURNED (no crash) ===", flush=True)
        break

print(f"\n=== capture ended at t+{time.time()-t0:.0f}s, {len(buf)} bytes ===", flush=True)
s.close()
