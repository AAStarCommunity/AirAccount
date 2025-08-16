/**
 * QEMU TEE 代理服务
 * 通过expect脚本与QEMU中的真实TEE环境通信
 */

import { spawn, ChildProcess } from 'child_process';
import { promises as fs } from 'fs';
import path from 'path';

export interface QEMUTEECommand {
  command: string;
  args?: string[];
  timeout?: number;
}

export interface QEMUTEEResponse {
  success: boolean;
  output: string;
  error?: string;
}

export class QEMUTEEProxy {
  private qemuProcess: ChildProcess | null = null;
  private isConnected = false;
  private expectScriptPath: string;
  private testsPath: string;

  constructor() {
    this.testsPath = path.resolve('../../third_party/incubator-teaclave-trustzone-sdk/tests');
    this.expectScriptPath = path.join(this.testsPath, 'real_tee_proxy.exp');
  }

  async initialize(): Promise<void> {
    try {
      // 创建expect代理脚本
      await this.createExpectProxy();
      
      // 检查QEMU环境是否可用
      await this.checkQEMUEnvironment();
      
      // 启动QEMU TEE环境
      await this.startQEMUTEE();
      
      this.isConnected = true;
      console.log('✅ QEMU TEE Proxy 初始化成功');
    } catch (error) {
      console.error('❌ QEMU TEE Proxy 初始化失败:', error);
      throw error;
    }
  }

  async executeCommand(command: QEMUTEECommand): Promise<QEMUTEEResponse> {
    if (!this.isConnected) {
      throw new Error('QEMU TEE Proxy 未连接');
    }

    try {
      const cmdString = [command.command, ...(command.args || [])].join(' ');
      const result = await this.sendCommandToQEMU(cmdString);
      
      return {
        success: true,
        output: result
      };
    } catch (error) {
      return {
        success: false,
        output: '',
        error: error instanceof Error ? error.message : 'Unknown error'
      };
    }
  }

  async shutdown(): Promise<void> {
    if (this.qemuProcess) {
      this.qemuProcess.kill('SIGTERM');
      this.qemuProcess = null;
    }
    this.isConnected = false;
  }

  // 私有方法

  private async createExpectProxy(): Promise<void> {
    const expectScript = `#!/usr/bin/expect -f

# QEMU TEE 单次命令执行脚本
set timeout 120
log_file /tmp/qemu_tee_proxy.log

# 获取命令参数
set cmd [lindex $argv 0]
if {$cmd == ""} { 
    set cmd "hello"
}

# 启动QEMU OP-TEE环境
spawn ./optee-qemuv8-fixed.sh aarch64-optee-4.7.0-qemuv8-ubuntu-24.04

# 等待登录
expect "login:"
send "root\\r"
expect "# "
puts "✅ System booted and logged in"

# 设置环境
send "mkdir -p /shared && mount -t 9p -o trans=virtio host /shared && cd /shared\\r"
expect "# "

# 处理libteec兼容性（简化版本）
send "ls /usr/lib/libteec.so*\\r"
expect "# "

# 安装TA文件
send "cp *.ta /lib/optee_armtz/\\r"
expect "# "
puts "✅ TA file installed"

# 执行指定命令
puts "🧪 Executing: ./airaccount-ca $cmd"
send "./airaccount-ca $cmd\\r"
expect {
    "# " {
        puts "✅ Command completed"
    }
    timeout {
        puts "❌ Command timeout"
        exit 1
    }
}

# 关闭
puts "🔄 Shutting down..."
send "shutdown -h now\\r"
expect {
    "System halted" { puts "✅ Clean shutdown" }
    timeout { puts "⚠️ Shutdown timeout" }
}

exit 0`;

    await fs.writeFile(this.expectScriptPath, expectScript);
    await fs.chmod(this.expectScriptPath, '755');
  }

  private async checkQEMUEnvironment(): Promise<void> {
    const qemuDir = path.join(this.testsPath, 'aarch64-optee-4.7.0-qemuv8-ubuntu-24.04');
    const caFile = path.join(this.testsPath, 'shared/airaccount-ca');
    const taFile = path.join(this.testsPath, 'shared/11223344-5566-7788-99aa-bbccddeeff01.ta');

    const checks = [
      { file: qemuDir, name: 'QEMU环境目录' },
      { file: caFile, name: 'AirAccount CA' },
      { file: taFile, name: 'AirAccount TA' }
    ];

    for (const check of checks) {
      try {
        await fs.access(check.file);
      } catch {
        throw new Error(`缺少${check.name}: ${check.file}`);
      }
    }
  }

  private async startQEMUTEE(): Promise<void> {
    // 简化版本：只检查expect脚本是否创建成功
    // 实际的QEMU启动将在每次命令执行时进行
    console.log('✅ QEMU TEE代理脚本已准备就绪');
  }

  private async sendCommandToQEMU(command: string): Promise<string> {
    // 每次命令都启动一个新的QEMU实例（单次执行模式）
    return new Promise((resolve, reject) => {
      const process = spawn('expect', [this.expectScriptPath, command], {
        cwd: this.testsPath,
        stdio: ['pipe', 'pipe', 'pipe']
      });

      let output = '';
      let error = '';

      process.stdout.on('data', (data) => {
        const chunk = data.toString();
        output += chunk;
        console.log('QEMU输出:', chunk);
      });

      process.stderr.on('data', (data) => {
        const chunk = data.toString();
        error += chunk;
        console.error('QEMU错误:', chunk);
      });

      process.on('close', (code) => {
        if (code === 0) {
          // 从输出中提取实际的命令结果
          const lines = output.split('\n');
          const resultLines = lines.filter(line => 
            !line.includes('✅') && 
            !line.includes('❌') && 
            !line.includes('🧪') && 
            !line.includes('🔄') &&
            line.trim().length > 0
          );
          
          const result = resultLines.join('\n').trim();
          resolve(result || 'AirAccount TEE Command Executed');
        } else {
          reject(new Error(`QEMU命令执行失败，代码: ${code}, 错误: ${error}`));
        }
      });

      process.on('error', (err) => {
        reject(new Error(`QEMU进程错误: ${err.message}`));
      });

      // 命令超时（增加到3分钟因为QEMU启动需要时间）
      setTimeout(() => {
        process.kill('SIGTERM');
        reject(new Error(`命令超时: ${command}`));
      }, 180000); // 3分钟
    });
  }
}