#!/bin/bash
# TEE服务启动脚本 - Docker容器内使用

set -e

echo "🚀 Starting TEE Service in Container"
echo "==================================="

# 环境变量设置
export TEE_MODE=${TEE_MODE:-simulation}
export TEE_LOG_LEVEL=${TEE_LOG_LEVEL:-info}
export TEE_PORT=${TEE_PORT:-5000}

# 日志函数
log() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] $1"
}

log "TEE Service Configuration:"
log "  Mode: $TEE_MODE"
log "  Log Level: $TEE_LOG_LEVEL"
log "  Port: $TEE_PORT"

# 创建必要的目录
mkdir -p /tee/data
mkdir -p /tee/logs
mkdir -p /tee/keys

# 设置权限
chmod 750 /tee/data
chmod 750 /tee/logs
chmod 700 /tee/keys

log "Created TEE directories with appropriate permissions"

# 初始化TEE环境
if [ "$TEE_MODE" = "simulation" ]; then
    log "Initializing TEE simulation environment..."
    
    # 在模拟模式下，我们不需要实际的硬件TEE
    # 创建模拟的TEE密钥和配置
    if [ ! -f "/tee/keys/tee_master_key" ]; then
        openssl rand -hex 32 > /tee/keys/tee_master_key
        chmod 600 /tee/keys/tee_master_key
        log "Generated TEE master key"
    fi
    
elif [ "$TEE_MODE" = "qemu" ]; then
    log "Initializing QEMU TEE environment..."
    
    # 检查QEMU和OP-TEE环境
    if [ ! -d "/workspace/third_party/optee_os" ]; then
        log "ERROR: OP-TEE OS not found. Please run setup script first."
        exit 1
    fi
    
    # 启动QEMU TEE环境
    log "Starting QEMU with OP-TEE..."
    # 这里应该启动QEMU TEE环境，但现在我们先模拟
    
else
    log "ERROR: Unsupported TEE mode: $TEE_MODE"
    exit 1
fi

# 启动TEE服务进程
log "Starting TEE service processes..."

# 创建TEE服务监听脚本
cat > /tee/scripts/tee-listener.py << 'EOF'
#!/usr/bin/env python3
import socket
import json
import threading
import time
import os
from datetime import datetime

class TEEService:
    def __init__(self, port=5000):
        self.port = port
        self.running = False
        
    def start(self):
        self.running = True
        server_socket = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        server_socket.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        server_socket.bind(('0.0.0.0', self.port))
        server_socket.listen(5)
        
        print(f"TEE Service listening on port {self.port}")
        
        while self.running:
            try:
                client_socket, addr = server_socket.accept()
                thread = threading.Thread(
                    target=self.handle_client, 
                    args=(client_socket, addr)
                )
                thread.daemon = True
                thread.start()
            except Exception as e:
                if self.running:
                    print(f"Error accepting connection: {e}")
                break
    
    def handle_client(self, client_socket, addr):
        try:
            data = client_socket.recv(1024).decode('utf-8')
            if not data:
                return
                
            request = json.loads(data)
            response = self.process_request(request)
            
            client_socket.send(json.dumps(response).encode('utf-8'))
        except Exception as e:
            print(f"Error handling client {addr}: {e}")
        finally:
            client_socket.close()
    
    def process_request(self, request):
        cmd = request.get('command', 'unknown')
        
        if cmd == 'ping':
            return {
                'status': 'ok',
                'timestamp': datetime.now().isoformat(),
                'tee_mode': os.environ.get('TEE_MODE', 'simulation')
            }
        elif cmd == 'generate_key':
            return {
                'status': 'ok',
                'key_id': f"key_{int(time.time())}",
                'algorithm': 'ecdsa'
            }
        elif cmd == 'sign':
            return {
                'status': 'ok',
                'signature': 'mock_signature_' + str(int(time.time())),
                'algorithm': 'ecdsa'
            }
        elif cmd == 'encrypt':
            return {
                'status': 'ok',
                'encrypted_data': 'mock_encrypted_' + str(int(time.time()))
            }
        else:
            return {
                'status': 'error',
                'message': f'Unknown command: {cmd}'
            }

if __name__ == '__main__':
    port = int(os.environ.get('TEE_PORT', 5000))
    service = TEEService(port)
    
    try:
        service.start()
    except KeyboardInterrupt:
        print("Shutting down TEE service...")
        service.running = False
EOF

chmod +x /tee/scripts/tee-listener.py

# 启动TEE监听器
log "Starting TEE listener on port $TEE_PORT..."
python3 /tee/scripts/tee-listener.py &
TEE_PID=$!

# 创建健康检查端点
log "Creating health check endpoint..."
cat > /tee/scripts/health-server.py << 'EOF'
#!/usr/bin/env python3
import socket
import json
from datetime import datetime
import os
import psutil

def health_check():
    return {
        'status': 'healthy',
        'timestamp': datetime.now().isoformat(),
        'tee_mode': os.environ.get('TEE_MODE', 'simulation'),
        'uptime': time.time() - start_time,
        'memory_usage': psutil.virtual_memory().percent,
        'disk_usage': psutil.disk_usage('/').percent
    }

start_time = time.time()

server = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
server.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
server.bind(('0.0.0.0', 8080))
server.listen(1)

print("Health check server listening on port 8080")

while True:
    client, addr = server.accept()
    try:
        request = client.recv(1024).decode('utf-8')
        if 'GET /health' in request:
            health = health_check()
            response = f"""HTTP/1.1 200 OK
Content-Type: application/json
Content-Length: {len(json.dumps(health))}

{json.dumps(health, indent=2)}"""
        else:
            response = "HTTP/1.1 404 Not Found\r\n\r\n404 Not Found"
        
        client.send(response.encode('utf-8'))
    except Exception as e:
        print(f"Health check error: {e}")
    finally:
        client.close()
EOF

# 需要安装psutil
pip3 install psutil > /dev/null 2>&1 || echo "Warning: Could not install psutil"

chmod +x /tee/scripts/health-server.py
python3 /tee/scripts/health-server.py &
HEALTH_PID=$!

# 等待服务启动
sleep 2

# 验证服务是否启动成功
if kill -0 $TEE_PID 2>/dev/null; then
    log "✅ TEE service started successfully (PID: $TEE_PID)"
else
    log "❌ TEE service failed to start"
    exit 1
fi

if kill -0 $HEALTH_PID 2>/dev/null; then
    log "✅ Health check service started successfully (PID: $HEALTH_PID)"
else
    log "❌ Health check service failed to start"
fi

# 创建PID文件
echo $TEE_PID > /tee/tee-service.pid
echo $HEALTH_PID > /tee/health-service.pid

log "TEE services are running. Logs available at /tee/logs/"

# 优雅关闭处理
cleanup() {
    log "Shutting down TEE services..."
    if [ -f /tee/tee-service.pid ]; then
        kill $(cat /tee/tee-service.pid) 2>/dev/null || true
    fi
    if [ -f /tee/health-service.pid ]; then
        kill $(cat /tee/health-service.pid) 2>/dev/null || true
    fi
    exit 0
}

trap cleanup SIGTERM SIGINT

# 保持容器运行并监控服务
while true; do
    if ! kill -0 $TEE_PID 2>/dev/null; then
        log "❌ TEE service died, restarting..."
        python3 /tee/scripts/tee-listener.py &
        TEE_PID=$!
        echo $TEE_PID > /tee/tee-service.pid
    fi
    
    if ! kill -0 $HEALTH_PID 2>/dev/null; then
        log "❌ Health service died, restarting..."
        python3 /tee/scripts/health-server.py &
        HEALTH_PID=$!
        echo $HEALTH_PID > /tee/health-service.pid
    fi
    
    sleep 30
done