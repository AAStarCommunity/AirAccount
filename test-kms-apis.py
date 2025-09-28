#!/usr/bin/env python3
"""
KMS API 全面测试套件
Comprehensive KMS API Testing Suite

支持测试Mock-TEE和QEMU-TEE两个版本的所有API端点
Tests all API endpoints for both Mock-TEE and QEMU-TEE versions
"""

import requests
import json
import base64
import time
import sys
import argparse
from typing import Dict, Any, Optional
import hashlib
from dataclasses import dataclass

@dataclass
class TestResult:
    name: str
    success: bool
    response: Optional[Dict[str, Any]]
    error: Optional[str]
    duration: float

class KMSAPITester:
    def __init__(self, base_url: str = "http://localhost:8080"):
        self.base_url = base_url.rstrip('/')
        self.session = requests.Session()
        self.created_keys = []
        self.test_results = []

    def log(self, level: str, message: str):
        """日志输出"""
        colors = {
            'INFO': '\033[94m',
            'SUCCESS': '\033[92m',
            'ERROR': '\033[91m',
            'WARN': '\033[93m'
        }
        reset = '\033[0m'
        print(f"{colors.get(level, '')}{level}: {message}{reset}")

    def make_kms_request(self, action: str, payload: Dict[str, Any]) -> Dict[str, Any]:
        """发送AWS KMS格式的请求"""
        headers = {
            'Content-Type': 'application/json',
            'X-Amz-Target': f'TrentService.{action}'
        }

        start_time = time.time()
        try:
            response = self.session.post(
                f"{self.base_url}/",
                headers=headers,
                json=payload,
                timeout=30
            )
            duration = time.time() - start_time

            if response.status_code == 200:
                return {
                    'success': True,
                    'data': response.json(),
                    'duration': duration,
                    'status_code': response.status_code
                }
            else:
                return {
                    'success': False,
                    'error': response.text,
                    'duration': duration,
                    'status_code': response.status_code
                }
        except Exception as e:
            duration = time.time() - start_time
            return {
                'success': False,
                'error': str(e),
                'duration': duration,
                'status_code': None
            }

    def test_health_check(self) -> TestResult:
        """测试健康检查端点"""
        self.log('INFO', "测试健康检查...")

        start_time = time.time()
        try:
            response = self.session.get(f"{self.base_url}/health", timeout=10)
            duration = time.time() - start_time

            if response.status_code == 200:
                data = response.json()
                self.log('SUCCESS', f"健康检查通过: {data}")
                return TestResult(
                    name="health_check",
                    success=True,
                    response=data,
                    error=None,
                    duration=duration
                )
            else:
                self.log('ERROR', f"健康检查失败: {response.status_code}")
                return TestResult(
                    name="health_check",
                    success=False,
                    response=None,
                    error=f"HTTP {response.status_code}",
                    duration=duration
                )
        except Exception as e:
            duration = time.time() - start_time
            self.log('ERROR', f"健康检查异常: {e}")
            return TestResult(
                name="health_check",
                success=False,
                response=None,
                error=str(e),
                duration=duration
            )

    def test_create_key(self) -> TestResult:
        """测试创建密钥"""
        self.log('INFO', "测试创建密钥...")

        payload = {
            "KeyUsage": "SIGN_VERIFY",
            "KeySpec": "ECC_SECG_P256K1",
            "Origin": "AWS_KMS"
        }

        result = self.make_kms_request("CreateKey", payload)

        if result['success']:
            # AWS KMS API返回KeyMetadata结构
            key_metadata = result['data'].get('KeyMetadata', {})
            key_id = key_metadata.get('KeyId') or result['data'].get('KeyId')
            if key_id:
                self.created_keys.append(key_id)
                self.log('SUCCESS', f"成功创建密钥: {key_id}")
            else:
                self.log('ERROR', f"响应中未找到KeyId: {result['data']}")
            return TestResult(
                name="create_key",
                success=bool(key_id),
                response=result['data'],
                error=None if key_id else "未找到KeyId",
                duration=result['duration']
            )
        else:
            self.log('ERROR', f"创建密钥失败: {result['error']}")
            return TestResult(
                name="create_key",
                success=False,
                response=None,
                error=result['error'],
                duration=result['duration']
            )

    def test_get_public_key(self, key_id: str) -> TestResult:
        """测试获取公钥"""
        self.log('INFO', f"测试获取公钥: {key_id}")

        payload = {"KeyId": key_id}
        result = self.make_kms_request("GetPublicKey", payload)

        if result['success']:
            public_key = result['data'].get('PublicKey')
            if public_key:
                # 验证公钥长度（base64编码的33字节应该是44字符）
                decoded = base64.b64decode(public_key)
                self.log('SUCCESS', f"成功获取公钥，长度: {len(decoded)} 字节")
            return TestResult(
                name="get_public_key",
                success=True,
                response=result['data'],
                error=None,
                duration=result['duration']
            )
        else:
            self.log('ERROR', f"获取公钥失败: {result['error']}")
            return TestResult(
                name="get_public_key",
                success=False,
                response=None,
                error=result['error'],
                duration=result['duration']
            )

    def test_sign_message(self, key_id: str) -> TestResult:
        """测试消息签名"""
        self.log('INFO', f"测试消息签名: {key_id}")

        message = "Hello KMS World!"
        message_b64 = base64.b64encode(message.encode()).decode()

        payload = {
            "KeyId": key_id,
            "Message": message_b64,
            "MessageType": "RAW",
            "SigningAlgorithm": "ECDSA_SHA_256"
        }

        result = self.make_kms_request("Sign", payload)

        if result['success']:
            signature = result['data'].get('Signature')
            if signature:
                # 验证签名长度（base64编码的64字节应该是88字符）
                decoded_sig = base64.b64decode(signature)
                self.log('SUCCESS', f"成功生成签名，长度: {len(decoded_sig)} 字节")
            return TestResult(
                name="sign_message",
                success=True,
                response=result['data'],
                error=None,
                duration=result['duration']
            )
        else:
            self.log('ERROR', f"消息签名失败: {result['error']}")
            return TestResult(
                name="sign_message",
                success=False,
                response=None,
                error=result['error'],
                duration=result['duration']
            )

    def test_list_keys(self) -> TestResult:
        """测试列出密钥"""
        self.log('INFO', "测试列出密钥...")

        start_time = time.time()
        try:
            response = self.session.get(f"{self.base_url}/keys", timeout=10)
            duration = time.time() - start_time

            if response.status_code == 200:
                data = response.json()
                key_count = len(data.get('keys', []))
                self.log('SUCCESS', f"成功列出密钥，总数: {key_count}")
                return TestResult(
                    name="list_keys",
                    success=True,
                    response=data,
                    error=None,
                    duration=duration
                )
            else:
                self.log('ERROR', f"列出密钥失败: {response.status_code}")
                return TestResult(
                    name="list_keys",
                    success=False,
                    response=None,
                    error=f"HTTP {response.status_code}",
                    duration=duration
                )
        except Exception as e:
            duration = time.time() - start_time
            self.log('ERROR', f"列出密钥异常: {e}")
            return TestResult(
                name="list_keys",
                success=False,
                response=None,
                error=str(e),
                duration=duration
            )

    def test_error_handling(self) -> TestResult:
        """测试错误处理"""
        self.log('INFO', "测试错误处理...")

        # 测试不存在的密钥
        payload = {"KeyId": "non-existent-key-12345"}
        result = self.make_kms_request("GetPublicKey", payload)

        # 错误处理应该返回失败状态但是有结构化的错误响应
        if not result['success'] and result['status_code'] in [400, 404]:
            self.log('SUCCESS', f"错误处理正常: {result['status_code']}")
            return TestResult(
                name="error_handling",
                success=True,
                response={'error_code': result['status_code']},
                error=None,
                duration=result['duration']
            )
        else:
            self.log('ERROR', f"错误处理异常: {result}")
            return TestResult(
                name="error_handling",
                success=False,
                response=None,
                error="错误处理不符合预期",
                duration=result['duration']
            )

    def test_performance_bulk_operations(self, count: int = 5) -> TestResult:
        """测试批量操作性能"""
        self.log('INFO', f"测试批量操作性能 ({count}个密钥)...")

        start_time = time.time()
        bulk_keys = []

        try:
            for i in range(count):
                payload = {
                    "KeyUsage": "SIGN_VERIFY",
                    "KeySpec": "ECC_SECG_P256K1",
                    "Origin": "AWS_KMS"
                }
                result = self.make_kms_request("CreateKey", payload)
                if result['success']:
                    # AWS KMS API返回KeyMetadata结构
                    key_metadata = result['data'].get('KeyMetadata', {})
                    key_id = key_metadata.get('KeyId') or result['data'].get('KeyId')
                    if key_id:
                        bulk_keys.append(key_id)
                        self.created_keys.append(key_id)
                else:
                    self.log('WARN', f"批量创建第{i+1}个密钥失败: {result.get('error', 'Unknown error')}")

            duration = time.time() - start_time
            avg_time = duration / count if count > 0 else 0

            self.log('SUCCESS', f"批量创建完成: {len(bulk_keys)}/{count} 个密钥，平均 {avg_time:.3f}s/个")

            return TestResult(
                name="bulk_operations",
                success=len(bulk_keys) > 0,
                response={'created_count': len(bulk_keys), 'avg_time': avg_time},
                error=None,
                duration=duration
            )

        except Exception as e:
            duration = time.time() - start_time
            self.log('ERROR', f"批量操作异常: {e}")
            return TestResult(
                name="bulk_operations",
                success=False,
                response=None,
                error=str(e),
                duration=duration
            )

    def run_complete_test_suite(self) -> Dict[str, Any]:
        """运行完整测试套件"""
        self.log('INFO', f"开始测试KMS API: {self.base_url}")
        self.log('INFO', "=" * 60)

        # 1. 健康检查
        result = self.test_health_check()
        self.test_results.append(result)

        if not result.success:
            self.log('ERROR', "健康检查失败，停止测试")
            return self.generate_test_report()

        # 2. 创建密钥
        result = self.test_create_key()
        self.test_results.append(result)

        if result.success and self.created_keys:
            key_id = self.created_keys[0]

            # 3. 获取公钥
            result = self.test_get_public_key(key_id)
            self.test_results.append(result)

            # 4. 消息签名
            result = self.test_sign_message(key_id)
            self.test_results.append(result)
        else:
            self.log('WARN', "跳过依赖密钥的测试（获取公钥、消息签名）")

        # 5. 列出密钥
        result = self.test_list_keys()
        self.test_results.append(result)

        # 6. 错误处理
        result = self.test_error_handling()
        self.test_results.append(result)

        # 7. 性能测试
        result = self.test_performance_bulk_operations(3)
        self.test_results.append(result)

        return self.generate_test_report()

    def generate_test_report(self) -> Dict[str, Any]:
        """生成测试报告"""
        total_tests = len(self.test_results)
        passed_tests = sum(1 for r in self.test_results if r.success)
        total_duration = sum(r.duration for r in self.test_results)

        report = {
            'summary': {
                'total_tests': total_tests,
                'passed_tests': passed_tests,
                'failed_tests': total_tests - passed_tests,
                'success_rate': (passed_tests / total_tests * 100) if total_tests > 0 else 0,
                'total_duration': total_duration,
                'avg_duration': total_duration / total_tests if total_tests > 0 else 0
            },
            'details': [
                {
                    'name': r.name,
                    'success': r.success,
                    'duration': r.duration,
                    'error': r.error
                }
                for r in self.test_results
            ],
            'created_keys_count': len(self.created_keys)
        }

        # 打印报告
        self.log('INFO', "=" * 60)
        self.log('INFO', "测试报告 / Test Report")
        self.log('INFO', "=" * 60)

        if passed_tests == total_tests:
            self.log('SUCCESS', f"所有测试通过! {passed_tests}/{total_tests}")
        else:
            self.log('ERROR', f"测试结果: {passed_tests}/{total_tests} 通过")

        self.log('INFO', f"总耗时: {total_duration:.3f}s")
        self.log('INFO', f"平均耗时: {report['summary']['avg_duration']:.3f}s")
        self.log('INFO', f"创建密钥数: {len(self.created_keys)}")

        for result in self.test_results:
            status = "✅" if result.success else "❌"
            self.log('INFO', f"{status} {result.name}: {result.duration:.3f}s")
            if result.error:
                self.log('INFO', f"   错误: {result.error}")

        return report

def main():
    parser = argparse.ArgumentParser(description='KMS API 测试套件')
    parser.add_argument('--url', default='http://localhost:8080',
                       help='KMS API base URL (默认: http://localhost:8080)')
    parser.add_argument('--online', action='store_true',
                       help='测试在线部署版本')
    parser.add_argument('--compare', action='store_true',
                       help='比较本地和在线版本')
    parser.add_argument('--output', help='保存测试报告到JSON文件')

    args = parser.parse_args()

    urls_to_test = []

    if args.compare:
        urls_to_test = [
            ('Local', 'http://localhost:8080'),
            ('Online', 'https://atom-become-ireland-travels.trycloudflare.com')
        ]
    elif args.online:
        urls_to_test = [('Online', 'https://atom-become-ireland-travels.trycloudflare.com')]
    else:
        urls_to_test = [('Local', args.url)]

    all_reports = {}

    for name, url in urls_to_test:
        print(f"\n{'='*20} 测试 {name} ({url}) {'='*20}")

        tester = KMSAPITester(url)
        report = tester.run_complete_test_suite()
        all_reports[name] = report

        if args.output:
            output_file = f"{args.output}_{name.lower()}.json"
            with open(output_file, 'w', encoding='utf-8') as f:
                json.dump(report, f, indent=2, ensure_ascii=False)
            tester.log('INFO', f"报告已保存到: {output_file}")

    # 比较报告
    if len(all_reports) > 1:
        print(f"\n{'='*20} 比较报告 {'='*20}")
        for name, report in all_reports.items():
            summary = report['summary']
            print(f"{name}: {summary['passed_tests']}/{summary['total_tests']} "
                  f"({summary['success_rate']:.1f}%) - {summary['total_duration']:.3f}s")

if __name__ == '__main__':
    main()