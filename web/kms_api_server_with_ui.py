#!/usr/bin/env python3

import http.server
import socketserver
import json
import uuid
import base64
from datetime import datetime

# 内存中的密钥存储
KEY_STORE = {}

class KMSAPIHandler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        """显示 KMS API 测试页面"""
        self.send_response(200)
        self.send_header('Content-type', 'text/html; charset=utf-8')
        self.send_header('Access-Control-Allow-Origin', '*')
        self.end_headers()

        html = """<!DOCTYPE html>
<html lang="zh">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>🔐 AirAccount KMS API 测试工具</title>
    <style>
        * { margin: 0; padding: 0; box-sizing: border-box; }
        body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', system-ui; background: linear-gradient(135deg, #667eea 0%, #764ba2 100%); min-height: 100vh; padding: 20px; }
        .container { max-width: 1200px; margin: 0 auto; background: rgba(255,255,255,0.95); border-radius: 20px; padding: 30px; box-shadow: 0 20px 40px rgba(0,0,0,0.1); }
        h1 { text-align: center; color: #2d3748; margin-bottom: 30px; font-size: 2.5em; }
        .api-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(350px, 1fr)); gap: 20px; margin-bottom: 30px; }
        .api-card { background: white; border-radius: 15px; padding: 25px; box-shadow: 0 8px 25px rgba(0,0,0,0.08); border: 2px solid transparent; transition: all 0.3s; }
        .api-card:hover { border-color: #667eea; transform: translateY(-5px); }
        .api-card h3 { color: #2d3748; margin-bottom: 15px; display: flex; align-items: center; gap: 10px; }
        .api-card textarea { width: 100%; height: 120px; border: 2px solid #e2e8f0; border-radius: 8px; padding: 12px; font-family: 'Courier New', monospace; font-size: 13px; resize: vertical; }
        .api-card button { width: 100%; padding: 15px; background: linear-gradient(135deg, #667eea, #764ba2); color: white; border: none; border-radius: 8px; font-size: 16px; font-weight: 600; cursor: pointer; margin-top: 15px; transition: all 0.3s; }
        .api-card button:hover { transform: translateY(-2px); box-shadow: 0 10px 20px rgba(102, 126, 234, 0.3); }
        .result { margin-top: 30px; padding: 25px; background: #1a202c; color: #e2e8f0; border-radius: 15px; font-family: 'Courier New', monospace; font-size: 13px; line-height: 1.6; white-space: pre-wrap; word-wrap: break-word; max-height: 400px; overflow-y: auto; }
        .status-bar { text-align: center; padding: 15px; background: linear-gradient(90deg, #48bb78, #38a169); color: white; border-radius: 10px; margin-bottom: 20px; font-weight: 600; }
        .icon { font-size: 1.2em; }
        .loading { opacity: 0.7; pointer-events: none; }
        .error { background: linear-gradient(90deg, #f56565, #e53e3e) !important; }
    </style>
</head>
<body>
    <div class="container">
        <h1>🔐 AirAccount KMS API 测试工具</h1>

        <div class="status-bar">
            🌐 连接状态: ✅ kms.aastar.io 在线 | 🔧 支持 6 个 AWS KMS 兼容 API 端点
        </div>

        <div class="api-grid">
            <div class="api-card">
                <h3><span class="icon">🔑</span> CreateKey - 创建密钥</h3>
                <textarea id="create-input">{"Description":"Web Test Key","KeyUsage":"SIGN_VERIFY","KeySpec":"ECC_SECG_P256K1","Origin":"AWS_KMS"}</textarea>
                <button onclick="callAPI('CreateKey', 'create-input')">创建密钥</button>
            </div>

            <div class="api-card">
                <h3><span class="icon">📝</span> DescribeKey - 查询密钥</h3>
                <textarea id="describe-input">{"KeyId":"请先创建密钥获取 KeyId"}</textarea>
                <button onclick="callAPI('DescribeKey', 'describe-input')">查询密钥</button>
            </div>

            <div class="api-card">
                <h3><span class="icon">📋</span> ListKeys - 列出所有密钥</h3>
                <textarea id="list-input">{}</textarea>
                <button onclick="callAPI('ListKeys', 'list-input')">列出密钥</button>
            </div>

            <div class="api-card">
                <h3><span class="icon">🔍</span> GetPublicKey - 获取公钥</h3>
                <textarea id="getpub-input">{"KeyId":"请先创建密钥获取 KeyId"}</textarea>
                <button onclick="callAPI('GetPublicKey', 'getpub-input')">获取公钥</button>
            </div>

            <div class="api-card">
                <h3><span class="icon">✍️</span> Sign - 数字签名</h3>
                <textarea id="sign-input">{"KeyId":"请先创建密钥获取 KeyId","Message":"SGVsbG8gV29ybGQ=","SigningAlgorithm":"ECDSA_SHA_256"}</textarea>
                <button onclick="callAPI('Sign', 'sign-input')">数字签名</button>
            </div>

            <div class="api-card">
                <h3><span class="icon">🗑️</span> ScheduleKeyDeletion - 删除密钥</h3>
                <textarea id="delete-input">{"KeyId":"请先创建密钥获取 KeyId","PendingWindowInDays":7}</textarea>
                <button onclick="callAPI('ScheduleKeyDeletion', 'delete-input')">删除密钥</button>
            </div>
        </div>

        <div id="result" class="result">📊 API 调用结果将在这里显示...</div>
    </div>

    <script>
        let lastKeyId = '';

        async function callAPI(action, inputId) {
            const input = document.getElementById(inputId);
            const result = document.getElementById('result');
            const button = event.target;

            button.classList.add('loading');
            button.textContent = '⏳ 调用中...';

            try {
                const requestData = JSON.parse(input.value);

                result.textContent = `🚀 正在调用 ${action}...\\n\\n📤 请求数据:\\n${JSON.stringify(requestData, null, 2)}`;

                const response = await fetch('/', {
                    method: 'POST',
                    headers: {
                        'Content-Type': 'application/json',
                        'X-Amz-Target': `TrentService.${action}`
                    },
                    body: JSON.stringify(requestData)
                });

                const responseData = await response.json();

                // 自动更新其他输入框的 KeyId
                if (action === 'CreateKey' && responseData.KeyMetadata && responseData.KeyMetadata.KeyId) {
                    lastKeyId = responseData.KeyMetadata.KeyId;
                    updateKeyIds();
                }

                result.innerHTML = `<div style="color: #48bb78; font-weight: bold;">✅ ${action} 调用成功</div>\\n\\n📤 请求:\\n${JSON.stringify(requestData, null, 2)}\\n\\n📥 响应 (${response.status}):\\n${JSON.stringify(responseData, null, 2)}\\n\\n⏰ 时间: ${new Date().toLocaleString('zh-CN')}`;

            } catch (error) {
                result.innerHTML = `<div style="color: #f56565; font-weight: bold;">❌ ${action} 调用失败</div>\\n\\n🚨 错误信息:\\n${error.message}\\n\\n⏰ 时间: ${new Date().toLocaleString('zh-CN')}`;
            } finally {
                button.classList.remove('loading');
                button.textContent = getButtonText(action);
            }
        }

        function updateKeyIds() {
            if (lastKeyId) {
                document.getElementById('describe-input').value = `{"KeyId":"${lastKeyId}"}`;
                document.getElementById('getpub-input').value = `{"KeyId":"${lastKeyId}"}`;
                document.getElementById('sign-input').value = `{"KeyId":"${lastKeyId}","Message":"SGVsbG8gV29ybGQ=","SigningAlgorithm":"ECDSA_SHA_256"}`;
                document.getElementById('delete-input').value = `{"KeyId":"${lastKeyId}","PendingWindowInDays":7}`;
            }
        }

        function getButtonText(action) {
            const texts = {
                'CreateKey': '创建密钥',
                'DescribeKey': '查询密钥',
                'ListKeys': '列出密钥',
                'GetPublicKey': '获取公钥',
                'Sign': '数字签名',
                'ScheduleKeyDeletion': '删除密钥'
            };
            return texts[action] || action;
        }

        // 初始化时自动调用 ListKeys
        window.addEventListener('load', () => {
            setTimeout(() => callAPI('ListKeys', 'list-input'), 1000);
        });
    </script>
</body>
</html>"""

        self.wfile.write(html.encode('utf-8'))

    def do_POST(self):
        content_length = int(self.headers['Content-Length'])
        post_data = self.rfile.read(content_length)

        # 解析AWS KMS API请求
        target = self.headers.get('X-Amz-Target', '')

        print(f'🚀 KMS API请求: {target}')
        print(f'📝 请求数据: {post_data.decode()}')

        if target == 'TrentService.CreateKey':
            self.handle_create_key(post_data)
        elif target == 'TrentService.DescribeKey':
            self.handle_describe_key(post_data)
        elif target == 'TrentService.ListKeys':
            self.handle_list_keys(post_data)
        elif target == 'TrentService.GenerateDataKey':
            self.handle_generate_data_key(post_data)
        elif target == 'TrentService.Sign':
            self.handle_sign(post_data)
        elif target == 'TrentService.GetPublicKey':
            self.handle_get_public_key(post_data)
        elif target == 'TrentService.ScheduleKeyDeletion':
            self.handle_delete_key(post_data)
        else:
            self.send_error(400, f'Unknown target: {target}')

    def handle_create_key(self, data):
        key_id = str(uuid.uuid4())
        print(f'✅ 创建KMS密钥: {key_id}')

        # 生成模拟的 secp256k1 公钥（33字节压缩格式）
        mock_pubkey_bytes = bytes([0x03]) + bytes(32)  # 压缩公钥前缀 + 32字节X坐标
        public_key_b64 = base64.b64encode(mock_pubkey_bytes).decode()

        # 存储密钥信息
        KEY_STORE[key_id] = {
            'public_key': public_key_b64,
            'created_at': datetime.now().isoformat(),
            'key_spec': 'ECC_SECG_P256K1',
            'key_usage': 'SIGN_VERIFY'
        }

        response = {
            'KeyMetadata': {
                'KeyId': key_id,
                'Arn': f'arn:aws:kms:us-east-1:123456789012:key/{key_id}',
                'CreationDate': datetime.now().isoformat(),
                'Enabled': True,
                'Description': 'KMS TA Key (TEE Environment)',
                'KeyUsage': 'SIGN_VERIFY',
                'KeyState': 'Enabled',
                'Origin': 'AWS_KMS',
                'KeySpec': 'ECC_SECG_P256K1'
            }
        }
        self.send_json_response(response)

    def handle_describe_key(self, data):
        req = json.loads(data)
        key_id = req.get('KeyId', 'unknown')

        response = {
            'KeyMetadata': {
                'KeyId': key_id,
                'Arn': f'arn:aws:kms:us-east-1:123456789012:key/{key_id}',
                'CreationDate': datetime.now().isoformat(),
                'Enabled': True,
                'Description': 'KMS TA Key (TEE Environment)',
                'KeyUsage': 'SIGN_VERIFY',
                'KeyState': 'Enabled',
                'Origin': 'AWS_KMS',
                'KeySpec': 'ECC_SECG_P256K1'
            }
        }
        self.send_json_response(response)

    def handle_list_keys(self, data):
        keys = []
        for key_id in KEY_STORE:
            keys.append({
                'KeyId': key_id,
                'KeyArn': f'arn:aws:kms:us-east-1:123456789012:key/{key_id}'
            })

        # 如果没有密钥，添加一个默认的示例
        if not keys:
            keys.append({
                'KeyId': '12345678-1234-1234-1234-123456789012',
                'KeyArn': 'arn:aws:kms:us-east-1:123456789012:key/12345678-1234-1234-1234-123456789012'
            })

        response = {
            'Keys': keys
        }
        self.send_json_response(response)

    def handle_generate_data_key(self, data):
        req = json.loads(data)
        key_id = req.get('KeyId', 'unknown')

        response = {
            'KeyId': key_id,
            'Plaintext': 'dGVzdF9kYXRhX2tleQ==',  # base64
            'CiphertextBlob': 'ZW5jcnlwdGVkX2RhdGFfa2V5'  # base64
        }
        self.send_json_response(response)

    def handle_sign(self, data):
        req = json.loads(data)
        key_id = req.get('KeyId', 'unknown')
        message = req.get('Message', '')

        print(f'🔐 模拟TA签名: KeyID={key_id}, Message={message}')

        # 生成模拟的 ECDSA 签名（64字节：32字节r + 32字节s）
        mock_signature_bytes = bytes(64)  # 全零签名作为模拟
        signature_b64 = base64.b64encode(mock_signature_bytes).decode()

        response = {
            'KeyId': key_id,
            'Signature': signature_b64,
            'SigningAlgorithm': 'ECDSA_SHA_256'
        }
        self.send_json_response(response)

    def handle_get_public_key(self, data):
        req = json.loads(data)
        key_id = req.get('KeyId', 'unknown')

        print(f'🔍 获取公钥: KeyID={key_id}')

        # 从存储中获取公钥，或生成新的模拟公钥
        if key_id in KEY_STORE:
            public_key = KEY_STORE[key_id]['public_key']
        else:
            # 为未知密钥生成默认模拟公钥
            mock_pubkey_bytes = bytes([0x03]) + bytes(32)  # 压缩公钥格式
            public_key = base64.b64encode(mock_pubkey_bytes).decode()

        response = {
            'KeyId': key_id,
            'PublicKey': public_key,
            'KeyUsage': 'SIGN_VERIFY',
            'KeySpec': 'ECC_SECG_P256K1',
            'SigningAlgorithms': ['ECDSA_SHA_256'],
            'EncryptionAlgorithms': []
        }

        print(f'✅ 返回公钥: {public_key[:20]}... (base64, {len(base64.b64decode(public_key))} bytes)')
        self.send_json_response(response)

    def handle_delete_key(self, data):
        req = json.loads(data)
        key_id = req.get('KeyId', 'unknown')

        # 从存储中移除密钥
        if key_id in KEY_STORE:
            del KEY_STORE[key_id]
            print(f'🗑️ 密钥已删除: {key_id}')

        response = {
            'KeyId': key_id,
            'DeletionDate': datetime.now().isoformat()
        }
        self.send_json_response(response)

    def send_json_response(self, data):
        self.send_response(200)
        self.send_header('Content-type', 'application/json')
        self.send_header('Access-Control-Allow-Origin', '*')
        self.end_headers()
        response_json = json.dumps(data, indent=2)
        print(f'📤 KMS响应: {response_json}')
        self.wfile.write(response_json.encode())

def start_server():
    PORT = 3000
    with socketserver.TCPServer(('', PORT), KMSAPIHandler) as httpd:
        print(f'🚀 KMS API Server (TEE环境) 启动在端口 {PORT}')
        print(f'🔗 主机访问地址: http://localhost:{PORT}')
        print(f'🌐 公网访问地址: https://kms.aastar.io')
        print(f'🎯 支持的AWS KMS API调用:')
        print(f'  - GET  /     -> 显示 Web 测试界面')
        print(f'  - POST /     -> AWS KMS API 调用')
        print(f'    - CreateKey (TrentService.CreateKey)')
        print(f'    - DescribeKey (TrentService.DescribeKey)')
        print(f'    - ListKeys (TrentService.ListKeys)')
        print(f'    - GenerateDataKey (TrentService.GenerateDataKey)')
        print(f'    - Sign (TrentService.Sign)')
        print(f'    - GetPublicKey (TrentService.GetPublicKey) ✨ 新增!')
        print(f'    - ScheduleKeyDeletion (TrentService.ScheduleKeyDeletion)')
        print(f'📊 当前存储的密钥数量: {len(KEY_STORE)}')
        httpd.serve_forever()

if __name__ == '__main__':
    start_server()