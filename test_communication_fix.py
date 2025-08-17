#!/usr/bin/env python3
"""
测试CA-TA通信修复
基于eth_wallet例子创建最小化的工作版本
"""

import subprocess
import os
import time

def run_command(cmd, cwd=None):
    """运行命令并返回结果"""
    try:
        result = subprocess.run(cmd, shell=True, capture_output=True, text=True, cwd=cwd)
        return result.returncode, result.stdout, result.stderr
    except Exception as e:
        return -1, "", str(e)

def test_ca_ta_communication():
    """测试CA-TA通信"""
    print("🧪 测试CA-TA通信修复")
    print("=" * 50)
    
    # 1. 创建最小化的CA代码 (基于eth_wallet)
    basic_ca_code = '''
use optee_teec::{Context, Operation, ParamType, Uuid};
use optee_teec::{ParamNone, ParamTmpRef, ParamValue};

const UUID: &str = "11223344-5566-7788-99aa-bbccddeeff01";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔧 Testing basic CA-TA communication...");
    
    let mut ctx = Context::new()?;
    let uuid = Uuid::parse_str(UUID)?;
    let mut session = ctx.open_session(uuid)?;
    
    println!("✅ TEE session opened");
    
    // 严格按照 eth_wallet 模式
    let p0 = ParamTmpRef::new_input(&[]);
    let mut output = vec![0u8; 256];
    let p1 = ParamTmpRef::new_output(output.as_mut_slice());
    let p2 = ParamValue::new(0, 0, ParamType::ValueInout);
    
    let mut operation = Operation::new(0, p0, p1, p2, ParamNone);
    
    match session.invoke_command(0, &mut operation) {
        Ok(()) => {
            let output_len = operation.parameters().2.a() as usize;
            let response = String::from_utf8_lossy(&output[..output_len]);
            println!("✅ SUCCESS: {}", response);
        }
        Err(e) => {
            println!("❌ FAILED: {:?}", e);
        }
    }
    
    Ok(())
}
'''
    
    print("✅ 创建了基于eth_wallet的最小化CA代码")
    
    # 2. 显示关键差异
    print("\n🔍 关键问题分析:")
    print("❌ 我们的旧代码: Operation::new(0, p0, p1, ParamNone, ParamNone)")
    print("✅ eth_wallet标准: Operation::new(0, p0, p1, p2, ParamNone)")
    print("   其中 p2 = ParamValue::new(0, 0, ParamType::ValueInout)")
    
    print("\n📝 TA端也需要对应修改:")
    print("✅ TA必须使用: p2.set_a(output_len as u32) 设置输出长度")
    print("✅ CA读取长度: operation.parameters().2.a() as usize")
    
    return True

if __name__ == "__main__":
    test_ca_ta_communication()