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
import json, os, re, secrets, subprocess, urllib.request
from http.server import BaseHTTPRequestHandler, HTTPServer

HERE = os.path.dirname(os.path.abspath(__file__))
SIGNER_URL = os.environ.get("KMS_SIGNER_URL", "http://127.0.0.1:3100")
KMS_URL = os.environ.get("KMS_URL", "http://127.0.0.1:3000")
CONFIG_DIR = os.environ.get("NODE_CONFIG_DIR", "/etc/airaccount")
LISTEN_PORT = int(os.environ.get("SETUP_PORT", "8088"))
# 一次性 setup token — 首启生成,只 root 可读 + 打到 console/serial;社区(有板子
# SSH/串口访问)读出后填进表单,防同网他人配置(#156 review 的认证 TODO)。
TOKEN_FILE = os.environ.get("SETUP_TOKEN_FILE", "/run/aastar-node-setup.token")
ADDR_RE = re.compile(r"^0x[0-9a-fA-F]{40}$")


def ensure_setup_token():
    """首启生成一次性 token(存 0600 文件 + 返回,供打到 console)。已存在则复用。"""
    if os.path.exists(TOKEN_FILE):
        with open(TOKEN_FILE) as f:
            return f.read().strip()
    tok = secrets.token_hex(16)
    fd = os.open(TOKEN_FILE, os.O_CREAT | os.O_WRONLY | os.O_TRUNC, 0o600)
    with os.fdopen(fd, "w") as f:
        f.write(tok)
    return tok


def check_setup_token(submitted):
    """常量时间比对提交的 token 与文件(constant-time,避免 timing)。"""
    try:
        with open(TOKEN_FILE) as f:
            expected = f.read().strip()
    except OSError:
        return False
    return bool(submitted) and secrets.compare_digest(str(submitted), expected)


def is_configured():
    """幂等:已写过 config 就算已配置。"""
    return os.path.exists(os.path.join(CONFIG_DIR, "kms.env"))


def kms_reachable():
    try:
        with urllib.request.urlopen(f"{KMS_URL}/health", timeout=5) as r:
            return r.status == 200
    except Exception:
        return False


def finalize():
    """配置成功后收尾(向导以 root 跑,可调 systemctl):
    ① 关 KMS provisioning(删 drop-in,防再被 /gen-key 灌)② 重启 kms-api/dvt 让它们
    读新 /etc/airaccount/*.env ③ disable 本向导(下次不再首启)。
    尽力而为:某步失败不阻断,收集 warnings 回给前端。SKIP_FINALIZE=1 可跳过(测试)。"""
    warnings = []
    if os.environ.get("SKIP_FINALIZE") == "1":
        return ["finalize 跳过(SKIP_FINALIZE=1)"]

    def run(cmd):
        try:
            subprocess.run(cmd, check=True, timeout=30, capture_output=True)
        except Exception as e:
            # #159 review Low: 带上 systemd stderr,否则只有 return code 无从排障。
            err = getattr(e, "stderr", b"") or b""
            detail = err.decode(errors="replace").strip() if isinstance(err, (bytes, bytearray)) else str(err)
            warnings.append(f"{' '.join(cmd)}: {e}" + (f" — {detail}" if detail else ""))

    prov = "/etc/systemd/system/kms-api.service.d/prov.conf"
    if os.path.exists(prov):
        try:
            os.remove(prov)
        except OSError as e:
            warnings.append(f"删 {prov}: {e}")
    run(["systemctl", "daemon-reload"])
    run(["systemctl", "restart", "kms-api.service"])
    run(["systemctl", "restart", "dvt.service"])
    run(["systemctl", "disable", "aastar-node-setup.service"])
    return warnings


def provision_bls_key():
    """调 KMS internal signer /gen-key —— 密钥在 TEE 内生成密封,只回 key_id+pubkey。"""
    req = urllib.request.Request(f"{SIGNER_URL}/gen-key", method="POST")
    tok = os.environ.get("KMS_BLS_SIGNER_TOKEN")
    if tok:
        req.add_header("X-Signer-Token", tok)
    with urllib.request.urlopen(req, timeout=25) as r:
        d = json.load(r)
    return d["key_id"], d["public_key"]


def _write_secure(path, content):
    """0600 原子创建(O_CREAT 带 mode)——消除先 open(默认 umask 可能 0644)后
    chmod 之间文件短暂 world-readable 的 TOCTOU 窗口(#156 review Low)。"""
    fd = os.open(path, os.O_CREAT | os.O_WRONLY | os.O_TRUNC, 0o600)
    with os.fdopen(fd, "w") as f:
        f.write(content)


def write_config(cfg):
    """写 KMS + DVT env(config 化,社区只改这里)。0600 原子创建,只 root 可读。"""
    os.makedirs(CONFIG_DIR, exist_ok=True)
    kms_env = os.path.join(CONFIG_DIR, "kms.env")
    dvt_env = os.path.join(CONFIG_DIR, "dvt.env")
    _write_secure(
        kms_env,
        f"KMS_RP_ID={cfg['rp_id']}\n"
        f"KMS_BLS_KEY_ID={cfg['key_id']}\n"
        f"KMS_BLS_PUBKEY={cfg['bls_pubkey']}\n"
        f"KMS_BLS_SIGNER_TOKEN={cfg['token']}\n",
    )
    _write_secure(
        dvt_env,
        f"RUST_SIGNER_URL={SIGNER_URL}\n"
        f"RUST_SIGNER_REQUIRED=true\n"
        f"RUST_SIGNER_TOKEN={cfg['token']}\n",
    )
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
            if n > 8192:
                raise ValueError("请求体过大")
            data = json.loads(self.rfile.read(n) or b"{}")
            # 认证:一次性 setup token(社区从板子 console/SSH 读出后填)。
            if not check_setup_token(data.get("setup_token")):
                return self._send(403, json.dumps({"error": "setup token 错误或缺失(见板子 console/串口打印)"}))
            # 幂等:已配置则拒(避免覆盖已用密钥/config)。
            if is_configured():
                return self._send(409, json.dumps({"error": "本节点已配置过(如需重配,先清 " + CONFIG_DIR + ")"}))
            rp_id = data["rp_id"].strip()
            operator = data["operator"].strip()
            network = data.get("network", "testnet")
            # 校验:非空 + operator 是合法地址(#156 review Low)。
            if not rp_id or not operator:
                raise ValueError("rp_id 和 operator 必填,不能为空")
            if rp_id.endswith("aastar.io"):
                raise ValueError("rpId 必须是你自己的域名,不能用 aastar.io(身份独立)")
            if not ADDR_RE.match(operator):
                raise ValueError("operator 必须是合法以太坊地址(0x + 40 hex)")
            if network not in ("testnet", "mainnet"):
                raise ValueError("network 只能是 testnet/mainnet")
            # 前置:KMS 得先起来(否则 provision 会失败,给人话)。
            if not kms_reachable():
                return self._send(503, json.dumps({"error": "KMS 服务还没起来,稍等再试(检查 kms-api.service)"}))
            # 1. TEE 内生成密封 BLS 密钥
            key_id, bls_pubkey = provision_bls_key()
            # 2. 共享 signer token
            token = os.environ.get("KMS_BLS_SIGNER_TOKEN") or secrets.token_hex(32)
            # 3. 写 config
            write_config({"rp_id": rp_id, "key_id": key_id, "bls_pubkey": bls_pubkey, "token": token})
            # 4. finalize:关 provisioning + 重启服务 + disable 向导(自动收尾)
            warns = finalize()
            # 5. 链上注册指引(节点不能自注册 → validator owner=AAStar 注册,或社区自建 validator)
            next_steps = (
                "✅ 本地配置完成,KMS+DVT 已带新配置重启。\n\n"
                "链上注册(节点无法自注册,需 validator owner 注册你的 pubkey):\n"
                f"  network:   {network}\n"
                f"  operator:  {operator}\n"
                f"  blsPubkey: {bls_pubkey}\n"
                "→ 把上面三项发给 AAStar(开 issue / 协同中枢)登记到 validator;\n"
                "  AAStar 可用 SuperPaymaster gasless 代付。注册通过后你的节点即加入 ≥3 门限池。\n"
                "  (Phase 3 会把这步做成向导内一键 gasless 提交。)"
            )
            resp = {"bls_pubkey": bls_pubkey, "rp_id": rp_id, "next_steps": next_steps}
            if warns:
                resp["finalize_warnings"] = warns  # 收尾有非致命告警时告知(如某服务名不同)
            self._send(200, json.dumps(resp))
        except Exception as e:
            self._send(400, json.dumps({"error": str(e)}))


if __name__ == "__main__":
    tok = ensure_setup_token()
    print("=" * 60)
    print("aastar-node-setup 配置向导")
    print(f"  地址:   http://<板子IP>:{LISTEN_PORT}")
    print(f"  SETUP TOKEN(填进表单认证): {tok}")
    print(f"  (也在 {TOKEN_FILE},只 root 可读)")
    print("=" * 60)
    HTTPServer(("0.0.0.0", LISTEN_PORT), Handler).serve_forever()
