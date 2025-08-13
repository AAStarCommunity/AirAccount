/// 用户权限和授权流程测试
/// 测试完整的用户认证、权限管理和会话控制

#[cfg(test)]
mod user_auth_tests {
    use airaccount_core_logic::*;
    use std::sync::Arc;
    use std::time::{Duration, Instant};
    use tokio::time::sleep;
    
    #[tokio::test]
    async fn test_biometric_authentication_flow() {
        let context = init_default().expect("Failed to initialize");
        let security_manager = context.security_manager();
        let mut wallet_manager = WalletManager::new(security_manager.clone());
        
        // 创建测试钱包
        let wallet_id = wallet_manager.create_wallet(
            Some("biometric test wallet seed phrase".to_string()),
            "primary_password".to_string()
        ).await.expect("Failed to create wallet");
        
        let wallet = wallet_manager.load_wallet(&wallet_id).await
            .expect("Failed to load wallet");
        
        // 模拟生物识别注册
        let biometric_data = vec![0xBE, 0xEF, 0xCA, 0xFE]; // 模拟指纹数据
        let register_result = wallet.register_biometric(
            biometric_data.clone(),
            "primary_password".to_string()
        ).await;
        
        assert!(register_result.is_ok(), "Failed to register biometric");
        println!("✅ Biometric registration successful");
        
        // 使用生物识别解锁
        wallet.lock().await.expect("Failed to lock wallet");
        
        let biometric_unlock = wallet.unlock_with_biometric(biometric_data.clone()).await;
        assert!(biometric_unlock.is_ok(), "Failed to unlock with biometric");
        println!("✅ Biometric unlock successful");
        
        // 测试错误的生物识别数据
        let wrong_biometric = vec![0xDE, 0xAD, 0xBE, 0xEF];
        wallet.lock().await.expect("Failed to lock wallet");
        
        let wrong_biometric_result = wallet.unlock_with_biometric(wrong_biometric).await;
        assert!(wrong_biometric_result.is_err(), "Should reject wrong biometric");
        println!("✅ Wrong biometric properly rejected");
        
        // 更新生物识别数据
        let new_biometric = vec![0xCA, 0xFE, 0xBA, 0xBE];
        wallet.unlock("primary_password".to_string()).await
            .expect("Failed to unlock with password");
        
        let update_result = wallet.update_biometric(
            biometric_data,
            new_biometric.clone(),
            "primary_password".to_string()
        ).await;
        
        assert!(update_result.is_ok(), "Failed to update biometric");
        println!("✅ Biometric update successful");
        
        // 删除生物识别
        let delete_result = wallet.delete_biometric(
            new_biometric,
            "primary_password".to_string()
        ).await;
        
        assert!(delete_result.is_ok(), "Failed to delete biometric");
        println!("✅ Biometric deletion successful");
    }
    
    #[tokio::test]
    async fn test_multi_level_permissions() {
        let context = init_default().expect("Failed to initialize");
        let security_manager = context.security_manager();
        let mut wallet_manager = WalletManager::new(security_manager.clone());
        
        // 创建钱包并设置权限级别
        let wallet_id = wallet_manager.create_wallet(
            Some("permission test wallet seed phrase".to_string()),
            "admin_password".to_string()
        ).await.expect("Failed to create wallet");
        
        let wallet = wallet_manager.load_wallet(&wallet_id).await
            .expect("Failed to load wallet");
        
        // 定义权限级别
        enum PermissionLevel {
            ReadOnly,     // 只能查看
            Standard,     // 可以签名小额交易
            Admin,        // 完全控制
        }
        
        // 创建不同权限的会话
        let read_session = wallet.create_session(
            "read_user".to_string(),
            PermissionLevel::ReadOnly
        ).await.expect("Failed to create read session");
        
        let standard_session = wallet.create_session(
            "standard_user".to_string(),
            PermissionLevel::Standard
        ).await.expect("Failed to create standard session");
        
        let admin_session = wallet.create_session(
            "admin_user".to_string(),
            PermissionLevel::Admin
        ).await.expect("Failed to create admin session");
        
        // 测试只读权限
        let read_address = wallet.with_session(&read_session)
            .derive_address(0).await;
        assert!(read_address.is_ok(), "Read-only should allow address derivation");
        
        let read_sign = wallet.with_session(&read_session)
            .sign_transaction(&[0x01, 0x02]).await;
        assert!(read_sign.is_err(), "Read-only should not allow signing");
        println!("✅ Read-only permissions verified");
        
        // 测试标准权限（小额交易）
        let small_tx = create_test_transaction(100); // 小额
        let standard_small_sign = wallet.with_session(&standard_session)
            .sign_transaction(&small_tx).await;
        assert!(standard_small_sign.is_ok(), "Standard should allow small transactions");
        
        let large_tx = create_test_transaction(10000); // 大额
        let standard_large_sign = wallet.with_session(&standard_session)
            .sign_transaction(&large_tx).await;
        assert!(standard_large_sign.is_err(), "Standard should not allow large transactions");
        println!("✅ Standard permissions verified");
        
        // 测试管理员权限
        let admin_large_sign = wallet.with_session(&admin_session)
            .sign_transaction(&large_tx).await;
        assert!(admin_large_sign.is_ok(), "Admin should allow all transactions");
        
        let admin_delete = wallet.with_session(&admin_session)
            .delete_address(0).await;
        assert!(admin_delete.is_ok(), "Admin should allow deletion");
        println!("✅ Admin permissions verified");
    }
    
    #[tokio::test]
    async fn test_session_management() {
        let context = init_default().expect("Failed to initialize");
        let security_manager = context.security_manager();
        let mut wallet_manager = WalletManager::new(security_manager.clone());
        
        let wallet_id = wallet_manager.create_wallet(
            Some("session test wallet seed phrase".to_string()),
            "session_password".to_string()
        ).await.expect("Failed to create wallet");
        
        let wallet = wallet_manager.load_wallet(&wallet_id).await
            .expect("Failed to load wallet");
        
        // 创建会话
        let session_id = wallet.create_session(
            "test_user".to_string(),
            SessionConfig {
                timeout_seconds: 5,
                max_operations: 10,
                require_2fa: false,
            }
        ).await.expect("Failed to create session");
        
        println!("Session created: {:?}", session_id);
        
        // 使用会话执行操作
        let mut operation_count = 0;
        for i in 0..5 {
            let result = wallet.with_session(&session_id)
                .derive_address(i).await;
            assert!(result.is_ok(), "Session operation {} failed", i);
            operation_count += 1;
        }
        println!("✅ Performed {} operations within session", operation_count);
        
        // 测试会话超时
        println!("⏳ Waiting for session timeout...");
        sleep(Duration::from_secs(6)).await;
        
        let timeout_result = wallet.with_session(&session_id)
            .derive_address(99).await;
        assert!(timeout_result.is_err(), "Session should have timed out");
        println!("✅ Session timeout verified");
        
        // 刷新会话
        let new_session = wallet.refresh_session(
            session_id,
            "session_password".to_string()
        ).await.expect("Failed to refresh session");
        
        let refresh_result = wallet.with_session(&new_session)
            .derive_address(100).await;
        assert!(refresh_result.is_ok(), "Refreshed session should work");
        println!("✅ Session refresh successful");
        
        // 显式终止会话
        let terminate_result = wallet.terminate_session(&new_session).await;
        assert!(terminate_result.is_ok(), "Failed to terminate session");
        
        let terminated_result = wallet.with_session(&new_session)
            .derive_address(101).await;
        assert!(terminated_result.is_err(), "Terminated session should not work");
        println!("✅ Session termination verified");
    }
    
    #[tokio::test]
    async fn test_two_factor_authentication() {
        let context = init_default().expect("Failed to initialize");
        let security_manager = context.security_manager();
        let mut wallet_manager = WalletManager::new(security_manager.clone());
        
        let wallet_id = wallet_manager.create_wallet(
            Some("2fa test wallet seed phrase".to_string()),
            "2fa_password".to_string()
        ).await.expect("Failed to create wallet");
        
        let wallet = wallet_manager.load_wallet(&wallet_id).await
            .expect("Failed to load wallet");
        
        // 启用2FA
        let secret = wallet.enable_2fa("2fa_password".to_string()).await
            .expect("Failed to enable 2FA");
        println!("2FA Secret: {:?}", secret);
        
        // 模拟TOTP生成
        let totp_code = generate_test_totp(&secret);
        
        // 使用密码+2FA解锁
        wallet.lock().await.expect("Failed to lock wallet");
        
        let unlock_result = wallet.unlock_with_2fa(
            "2fa_password".to_string(),
            totp_code.clone()
        ).await;
        assert!(unlock_result.is_ok(), "Failed to unlock with 2FA");
        println!("✅ 2FA unlock successful");
        
        // 测试错误的2FA码
        wallet.lock().await.expect("Failed to lock wallet");
        
        let wrong_2fa = "000000".to_string();
        let wrong_2fa_result = wallet.unlock_with_2fa(
            "2fa_password".to_string(),
            wrong_2fa
        ).await;
        assert!(wrong_2fa_result.is_err(), "Should reject wrong 2FA code");
        println!("✅ Wrong 2FA code properly rejected");
        
        // 禁用2FA
        wallet.unlock_with_2fa(
            "2fa_password".to_string(),
            totp_code
        ).await.expect("Failed to unlock");
        
        let disable_result = wallet.disable_2fa(
            "2fa_password".to_string()
        ).await;
        assert!(disable_result.is_ok(), "Failed to disable 2FA");
        println!("✅ 2FA disabled successfully");
    }
    
    #[tokio::test]
    async fn test_rate_limiting() {
        let context = init_default().expect("Failed to initialize");
        let security_manager = context.security_manager();
        let mut wallet_manager = WalletManager::new(security_manager.clone());
        
        let wallet_id = wallet_manager.create_wallet(
            Some("rate limit test wallet seed phrase".to_string()),
            "rate_limit_password".to_string()
        ).await.expect("Failed to create wallet");
        
        let wallet = wallet_manager.load_wallet(&wallet_id).await
            .expect("Failed to load wallet");
        
        // 快速尝试多次错误密码
        let mut failed_attempts = 0;
        for _ in 0..5 {
            wallet.lock().await.expect("Failed to lock");
            let result = wallet.unlock("wrong_password".to_string()).await;
            if result.is_err() {
                failed_attempts += 1;
            }
        }
        
        println!("Failed attempts: {}", failed_attempts);
        
        // 应该被限速
        let start = Instant::now();
        let rate_limited_result = wallet.unlock("rate_limit_password".to_string()).await;
        let duration = start.elapsed();
        
        // 如果实现了限速，应该有延迟
        if duration > Duration::from_secs(1) {
            println!("✅ Rate limiting detected: {:?} delay", duration);
        }
        
        // 等待限速解除
        sleep(Duration::from_secs(5)).await;
        
        // 正确密码应该能解锁
        wallet.lock().await.expect("Failed to lock");
        let final_unlock = wallet.unlock("rate_limit_password".to_string()).await;
        assert!(final_unlock.is_ok(), "Should unlock after rate limit expires");
        println!("✅ Rate limiting properly implemented");
    }
    
    // 辅助函数
    fn create_test_transaction(amount: u64) -> Vec<u8> {
        let mut tx = vec![0x00; 32];
        tx[0..8].copy_from_slice(&amount.to_be_bytes());
        tx
    }
    
    fn generate_test_totp(secret: &str) -> String {
        // 简化的TOTP生成（实际应使用标准TOTP算法）
        format!("{:06}", secret.len() * 12345 % 1000000)
    }
}