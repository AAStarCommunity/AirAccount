#!/usr/bin/env python3
"""
Run a shell command on the MX93 board via serial port.
Claude uses this to trigger deploys, builds, and tests on the board.

Usage:
    python3 serial-run.py "echo hello"
    python3 serial-run.py "bash /root/AirAccount/scripts/mx93/deploy.sh main" --timeout 1200
    python3 serial-run.py "bash /root/AirAccount/scripts/mx93/test-smoke.sh"

Environment:
    SERIAL_DEVICE  (default: /dev/cu.usbmodem5B6D0044901)
    SERIAL_BAUD    (default: 115200)
"""
import sys, os, time, serial, argparse, base64

DEVICE = os.environ.get("SERIAL_DEVICE", "/dev/cu.usbmodem5B6D0044901")
BAUD   = int(os.environ.get("SERIAL_BAUD", "115200"))
MARKER = f"__DONE_{int(time.time())}__"


def read_until(ser, markers, timeout=30):
    buf = b""
    deadline = time.time() + timeout
    while time.time() < deadline:
        if ser.in_waiting:
            buf += ser.read(ser.in_waiting)
            for m in markers:
                if m.encode() in buf:
                    return buf.decode(errors="replace")
        time.sleep(0.05)
    return buf.decode(errors="replace")


def ensure_logged_in(ser):
    ser.reset_input_buffer()
    ser.write(b"\n")
    r = read_until(ser, ["#", "$", "login:"], timeout=4)
    if "login:" in r:
        ser.write(b"root\n")
        r = read_until(ser, ["#", "$", "Password:"], timeout=5)
        if "Password:" in r:
            ser.write(b"\n")
            r = read_until(ser, ["#", "$", "incorrect", "login:"], timeout=5)
        if "login:" in r or "incorrect" in r:
            # Retry
            ser.write(b"root\n")
            r = read_until(ser, ["#", "$"], timeout=5)


def run_command(cmd: str, timeout: int = 120) -> tuple[str, int]:
    """Run cmd on board, return (output, exit_code). exit_code=-1 if timed out."""
    with serial.Serial(DEVICE, BAUD, timeout=1) as ser:
        time.sleep(0.3)
        ensure_logged_in(ser)

        # Wrap command to emit marker + exit code
        wrapped = f"{cmd} ; echo {MARKER}:$?\n"
        ser.write(wrapped.encode())

        out = read_until(ser, [MARKER], timeout=timeout)

        # Parse exit code
        exit_code = -1
        for line in out.splitlines():
            if MARKER in line:
                try:
                    exit_code = int(line.split(":")[-1].strip())
                except ValueError:
                    pass

        # Strip the marker line from output
        clean = "\n".join(l for l in out.splitlines() if MARKER not in l)
        # Strip the echo of the command itself (first line)
        lines = clean.splitlines()
        if lines and cmd[:20].rstrip() in lines[0]:
            lines = lines[1:]
        return "\n".join(lines), exit_code


def send_file(local_path: str, remote_path: str):
    """Copy a local file to the board via base64 encoding over serial."""
    with open(local_path, "rb") as f:
        content = f.read()
    encoded = base64.b64encode(content).decode()

    # Split into chunks to avoid serial buffer overflow
    chunk_size = 512
    chunks = [encoded[i:i+chunk_size] for i in range(0, len(encoded), chunk_size)]

    with serial.Serial(DEVICE, BAUD, timeout=1) as ser:
        time.sleep(0.3)
        ensure_logged_in(ser)

        # Start file
        ser.write(f"rm -f /tmp/__transfer_b64 && touch /tmp/__transfer_b64\n".encode())
        read_until(ser, ["#"], timeout=5)

        for chunk in chunks:
            ser.write(f"printf '%s' '{chunk}' >> /tmp/__transfer_b64\n".encode())
            read_until(ser, ["#"], timeout=5)

        # Decode
        ser.write(f"base64 -d /tmp/__transfer_b64 > {remote_path} && rm /tmp/__transfer_b64\n".encode())
        read_until(ser, ["#"], timeout=10)

    print(f"Sent: {local_path} → {remote_path}")


def main():
    parser = argparse.ArgumentParser(description="Run command on MX93 via serial")
    parser.add_argument("command", nargs="?", help="Shell command to run on board")
    parser.add_argument("--timeout", type=int, default=120, help="Command timeout in seconds")
    parser.add_argument("--send", nargs=2, metavar=("LOCAL", "REMOTE"), help="Send local file to board")
    parser.add_argument("--device", default=DEVICE, help=f"Serial device (default: {DEVICE})")
    args = parser.parse_args()

    if args.device != DEVICE:
        os.environ["SERIAL_DEVICE"] = args.device

    if args.send:
        send_file(args.send[0], args.send[1])
        return

    if not args.command:
        parser.print_help()
        sys.exit(1)

    print(f"[serial-run] {args.command}")
    output, code = run_command(args.command, timeout=args.timeout)
    if output.strip():
        print(output)
    if code != 0:
        print(f"[exit code: {code}]", file=sys.stderr)
        sys.exit(code if code >= 0 else 1)


if __name__ == "__main__":
    main()
