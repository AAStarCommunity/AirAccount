#!/usr/bin/env python3
"""
test_setup_server.py — 社区节点 web 向导(setup-server.py)的 fresh-board E2E 测试(#20)。

纯 stdlib(unittest + http.server),零外部依赖、不碰真板/TEE/网络:
  - 起一个 mock KMS(答 /health、/gen-key、/pop),把真实 setup-server.py 起成子进程指向它;
  - 端到端跑一遍:token 认证 → 校验 → KMS 可达性 → provision → 写 config(0600) → 幂等。

跑:  python3 kms/node-setup/test_setup_server.py     (CI 可直接调,退出码即结果)
"""
import json
import os
import socket
import subprocess
import sys
import tempfile
import threading
import time
import unittest
import urllib.error
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

HERE = os.path.dirname(os.path.abspath(__file__))
SETUP_SERVER = os.path.join(HERE, "setup-server.py")
MOCK_PUBKEY = "0x" + "ab" * 128  # 128B EIP-2537 布局(向导只透传存储)
MOCK_KEY_ID = "11111111-2222-3333-4444-555555555555"


def _free_port():
    s = socket.socket()
    s.bind(("127.0.0.1", 0))
    p = s.getsockname()[1]
    s.close()
    return p


class _MockKMS(BaseHTTPRequestHandler):
    """mock KMS internal signer。healthy 标志由外部切换以测 503 分支。"""

    healthy = True  # 类变量,测试用例切换

    def log_message(self, *a):
        pass  # 静音

    def _json(self, code, obj):
        b = json.dumps(obj).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(b)))
        self.end_headers()
        self.wfile.write(b)

    def do_GET(self):
        if self.path == "/health":
            if _MockKMS.healthy:
                return self._json(200, {"status": "ok"})
            return self._json(503, {"status": "down"})
        self._json(404, {"error": "nf"})

    def do_POST(self):
        n = int(self.headers.get("Content-Length", 0) or 0)
        if n:
            self.rfile.read(n)
        if self.path == "/gen-key":
            return self._json(200, {"key_id": MOCK_KEY_ID, "public_key": MOCK_PUBKEY})
        if self.path == "/pop":
            return self._json(200, {
                "public_key": MOCK_PUBKEY,
                "pop_point": "0x" + "cd" * 128,
                "pop_signature": "0x" + "ef" * 128,
            })
        self._json(404, {"error": "nf"})


def _http(method, url, body=None, timeout=10):
    data = json.dumps(body).encode() if body is not None else None
    req = urllib.request.Request(url, data=data, method=method,
                                 headers={"Content-Type": "application/json"})
    try:
        with urllib.request.urlopen(req, timeout=timeout) as r:
            return r.status, r.read().decode()
    except urllib.error.HTTPError as e:
        return e.code, e.read().decode()


class WizardE2E(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.mkdtemp(prefix="wizard-e2e-")
        cls.cfg_dir = os.path.join(cls.tmp, "airaccount")
        cls.token_file = os.path.join(cls.tmp, "setup.token")
        mock_port = _free_port()
        cls.wiz_port = _free_port()

        # mock KMS
        _MockKMS.healthy = True
        cls.mock = ThreadingHTTPServer(("127.0.0.1", mock_port), _MockKMS)
        cls.mock_thread = threading.Thread(target=cls.mock.serve_forever, daemon=True)
        cls.mock_thread.start()
        mock_url = f"http://127.0.0.1:{mock_port}"

        # 真实向导子进程
        env = {
            **os.environ,
            "KMS_URL": mock_url,
            "KMS_SIGNER_URL": mock_url,
            "SETUP_PORT": str(cls.wiz_port),
            "NODE_CONFIG_DIR": cls.cfg_dir,
            "SETUP_TOKEN_FILE": cls.token_file,
            "SKIP_FINALIZE": "1",  # 不碰 systemctl
        }
        cls.proc = subprocess.Popen(
            [sys.executable, SETUP_SERVER], env=env,
            stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
        )
        cls.base = f"http://127.0.0.1:{cls.wiz_port}"
        # 等就绪
        for _ in range(50):
            try:
                s, _b = _http("GET", cls.base + "/health", timeout=1)
                if s == 200:
                    break
            except Exception:
                pass
            time.sleep(0.1)
        else:
            out = cls.proc.stdout.read().decode(errors="replace") if cls.proc.stdout else ""
            raise RuntimeError(f"向导没起来:\n{out}")
        with open(cls.token_file) as f:
            cls.token = f.read().strip()

    @classmethod
    def tearDownClass(cls):
        try:
            cls.proc.terminate()
            cls.proc.wait(timeout=5)
        except Exception:
            cls.proc.kill()
        if cls.proc.stdout:
            cls.proc.stdout.close()
        cls.mock.shutdown()
        cls.mock.server_close()

    def _post(self, body):
        return _http("POST", self.base + "/setup", body)

    def test_00_index_and_health(self):
        s, b = _http("GET", self.base + "/")
        self.assertEqual(s, 200)
        s, b = _http("GET", self.base + "/health")
        self.assertEqual(s, 200)
        self.assertEqual(json.loads(b)["status"], "ok")

    def test_01_reject_missing_token(self):
        s, _ = self._post({"rp_id": "kms.demo.org", "operator": "0x" + "1" * 40})
        self.assertEqual(s, 403)

    def test_02_reject_wrong_token(self):
        s, _ = self._post({"setup_token": "deadbeef", "rp_id": "kms.demo.org",
                           "operator": "0x" + "1" * 40})
        self.assertEqual(s, 403)

    def test_03_reject_bad_operator(self):
        s, _ = self._post({"setup_token": self.token, "rp_id": "kms.demo.org",
                           "operator": "not-an-address"})
        self.assertEqual(s, 400)

    def test_04_reject_aastar_rpid(self):
        s, _ = self._post({"setup_token": self.token, "rp_id": "kms.aastar.io",
                           "operator": "0x" + "1" * 40})
        self.assertEqual(s, 400)

    def test_05_kms_unreachable_503(self):
        _MockKMS.healthy = False
        try:
            s, _ = self._post({"setup_token": self.token, "rp_id": "kms.demo.org",
                               "operator": "0x" + "1" * 40})
            self.assertEqual(s, 503)
        finally:
            _MockKMS.healthy = True

    def test_06_happy_path_writes_config(self):
        s, b = self._post({"setup_token": self.token, "rp_id": "kms.demo.org",
                           "operator": "0x" + "1" * 40, "network": "testnet"})
        self.assertEqual(s, 200, b)
        resp = json.loads(b)
        self.assertEqual(resp["bls_pubkey"], MOCK_PUBKEY)
        self.assertFalse(resp.get("registered"))  # 无 operator key → 回落
        self.assertIn("next_steps", resp)
        # config 落盘 + 0600 + 内容
        kms_env = os.path.join(self.cfg_dir, "kms.env")
        dvt_env = os.path.join(self.cfg_dir, "dvt.env")
        self.assertTrue(os.path.exists(kms_env) and os.path.exists(dvt_env))
        self.assertEqual(oct(os.stat(kms_env).st_mode & 0o777), "0o600")
        self.assertEqual(oct(os.stat(dvt_env).st_mode & 0o777), "0o600")
        with open(kms_env) as f:
            kms_txt = f.read()
        with open(dvt_env) as f:
            dvt_txt = f.read()
        self.assertIn("KMS_RP_ID=kms.demo.org", kms_txt)
        self.assertIn(f"KMS_BLS_KEY_ID={MOCK_KEY_ID}", kms_txt)
        self.assertIn(f"KMS_BLS_PUBKEY={MOCK_PUBKEY}", kms_txt)
        self.assertIn("RUST_SIGNER_REQUIRED=true", dvt_txt)

    def test_07_idempotent_409(self):
        s, _ = self._post({"setup_token": self.token, "rp_id": "kms.demo.org",
                           "operator": "0x" + "1" * 40})
        self.assertEqual(s, 409)


if __name__ == "__main__":
    unittest.main(verbosity=2)
