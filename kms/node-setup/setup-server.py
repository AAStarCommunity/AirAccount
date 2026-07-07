#!/usr/bin/env python3
"""
aastar-node-setup — Phase 1 极简 web 向导(社区自助上手,见 docs/community-onboarding-program.md)

首次开机在板子上跑(LAN/Tailscale IP:8088),社区开浏览器填表 → 本服务:
  1. provision BLS 密钥(调 KMS internal signer /gen-key,密钥在 TEE 内生成密封)
  2. 生成 signer token(KMS_BLS_SIGNER_TOKEN == DVT RUST_SIGNER_TOKEN 共享密钥)
  3. 写 config(KMS + DVT env,rpId、RUST_SIGNER_URL 等)
  4. [Phase 1 stub] 链上注册指引(gasless via SuperPaymaster);Phase 3 做成一键闭环

⚠️ 骨架:steps 1-3 已接线,step 4 先给指引。生产前需加认证(setup token)+ 幂等 + 校验。
用法:  KMS_BLS_PROVISIONING=1 python3 setup-server.py   # 需先让 KMS 允许 provisioning
"""
import json, os, secrets, subprocess, urllib.request
from http.server import BaseHTTPRequestHandler, HTTPServer

HERE = os.path.dirname(os.path.abspath(__file__))
SIGNER_URL = os.environ.get("KMS_SIGNER_URL", "http://127.0.0.1:3100")
CONFIG_DIR = os.environ.get("NODE_CONFIG_DIR", "/etc/airaccount")
LISTEN_PORT = int(os.environ.get("SETUP_PORT", "8088"))


def provision_bls_key():
    """调 KMS internal signer /gen-key —— 密钥在 TEE 内生成密封,只回 key_id+pubkey。"""
    req = urllib.request.Request(f"{SIGNER_URL}/gen-key", method="POST")
    tok = os.environ.get("KMS_BLS_SIGNER_TOKEN")
    if tok:
        req.add_header("X-Signer-Token", tok)
    with urllib.request.urlopen(req, timeout=25) as r:
        d = json.load(r)
    return d["key_id"], d["public_key"]


def write_config(cfg):
    """写 KMS + DVT env(config 化,社区只改这里)。0600,只 root 可读。"""
    os.makedirs(CONFIG_DIR, exist_ok=True)
    kms_env = os.path.join(CONFIG_DIR, "kms.env")
    dvt_env = os.path.join(CONFIG_DIR, "dvt.env")
    with open(kms_env, "w") as f:
        f.write(
            f"KMS_RP_ID={cfg['rp_id']}\n"
            f"KMS_BLS_KEY_ID={cfg['key_id']}\n"
            f"KMS_BLS_PUBKEY={cfg['bls_pubkey']}\n"
            f"KMS_BLS_SIGNER_TOKEN={cfg['token']}\n"
        )
    os.chmod(kms_env, 0o600)
    with open(dvt_env, "w") as f:
        f.write(
            f"RUST_SIGNER_URL={SIGNER_URL}\n"
            f"RUST_SIGNER_REQUIRED=true\n"
            f"RUST_SIGNER_TOKEN={cfg['token']}\n"
        )
    os.chmod(dvt_env, 0o600)
    return kms_env, dvt_env


class Handler(BaseHTTPRequestHandler):
    def _send(self, code, body, ctype="application/json"):
        b = body.encode() if isinstance(body, str) else body
        self.send_response(code)
        self.send_header("Content-Type", ctype)
        self.send_header("Content-Length", str(len(b)))
        self.end_headers()
        self.wfile.write(b)

    def do_GET(self):
        if self.path in ("/", "/setup.html"):
            with open(os.path.join(HERE, "setup.html"), "rb") as f:
                self._send(200, f.read(), "text/html; charset=utf-8")
        elif self.path == "/health":
            self._send(200, json.dumps({"status": "ok", "service": "aastar-node-setup"}))
        else:
            self._send(404, json.dumps({"error": "not found"}))

    def do_POST(self):
        if self.path != "/setup":
            return self._send(404, json.dumps({"error": "not found"}))
        try:
            n = int(self.headers.get("Content-Length", 0))
            data = json.loads(self.rfile.read(n) or b"{}")
            rp_id = data["rp_id"].strip()
            operator = data["operator"].strip()
            network = data.get("network", "testnet")
            if rp_id.endswith("aastar.io"):
                raise ValueError("rpId 必须是你自己的域名,不能用 aastar.io(身份独立)")
            # 1. TEE 内生成密封 BLS 密钥
            key_id, bls_pubkey = provision_bls_key()
            # 2. 共享 signer token
            token = os.environ.get("KMS_BLS_SIGNER_TOKEN") or secrets.token_hex(32)
            # 3. 写 config
            write_config({"rp_id": rp_id, "key_id": key_id, "bls_pubkey": bls_pubkey, "token": token})
            # 4. [Phase 1 stub] 链上注册指引
            next_steps = (
                "下一步(Phase 1 手动;Phase 3 一键):\n"
                f"链上注册 TEE pubkey({network}):用 SDK dvtOperatorActions.registerWithProof\n"
                f"  operator={operator}\n"
                f"  blsPubkey={bls_pubkey}\n"
                "AAStar 可用 SuperPaymaster gasless 代付这笔注册。\n"
                "注册后重启 kms-api + dvt 服务即加入门限网络。"
            )
            self._send(200, json.dumps({"bls_pubkey": bls_pubkey, "rp_id": rp_id, "next_steps": next_steps}))
        except Exception as e:
            self._send(400, json.dumps({"error": str(e)}))


if __name__ == "__main__":
    print(f"aastar-node-setup 向导 → http://0.0.0.0:{LISTEN_PORT}  (社区开浏览器访问板子 IP:{LISTEN_PORT})")
    HTTPServer(("0.0.0.0", LISTEN_PORT), Handler).serve_forever()
