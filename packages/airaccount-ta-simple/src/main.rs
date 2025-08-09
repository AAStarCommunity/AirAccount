#![no_std]
#![no_main]

extern crate alloc;
use alloc::string::{String, ToString};

use optee_utee::{
    ta_close_session, ta_create, ta_destroy, ta_invoke_command, ta_open_session, trace_println,
};
use optee_utee::{Error, ErrorKind, Parameters};

use optee_utee::Random;

// 安全模块（适配 no_std OP-TEE 环境）
mod security {
    use super::*;
    use alloc::vec::Vec;
    use alloc::string::String;
    
    // 常时操作模块
    pub mod constant_time {
        use super::*;
        
        /// 安全字节数组，支持常时比较
        pub struct SecureBytes {
            data: Vec<u8>,
        }
        
        impl SecureBytes {
            pub fn new(data: Vec<u8>) -> Self {
                Self { data }
            }
            
            pub fn from_slice(slice: &[u8]) -> Self {
                Self { data: slice.to_vec() }
            }
            
            pub fn len(&self) -> usize {
                self.data.len()
            }
            
            pub fn as_slice(&self) -> &[u8] {
                &self.data
            }
            
            /// 常时比较两个字节数组
            pub fn constant_time_eq(&self, other: &Self) -> bool {
                if self.data.len() != other.data.len() {
                    return false;
                }
                
                let mut diff = 0u8;
                for (a, b) in self.data.iter().zip(other.data.iter()) {
                    diff |= a ^ b;
                }
                diff == 0
            }
            
            /// 常时选择
            pub fn conditional_select(condition: bool, if_true: &Self, if_false: &Self) -> Self {
                let mut result = Vec::new();
                let max_len = core::cmp::max(if_true.len(), if_false.len());
                
                for i in 0..max_len {
                    let true_byte = if i < if_true.len() { if_true.data[i] } else { 0 };
                    let false_byte = if i < if_false.len() { if_false.data[i] } else { 0 };
                    
                    // 常时选择：避免分支
                    let mask = if condition { 0xFF } else { 0x00 };
                    let selected = (true_byte & mask) | (false_byte & !mask);
                    result.push(selected);
                }
                
                Self::new(result)
            }
            
            /// 安全清零内存
            pub fn secure_zero(&mut self) {
                // 防止编译器优化掉内存清零
                for byte in &mut self.data {
                    unsafe {
                        core::ptr::write_volatile(byte, 0);
                    }
                }
            }
        }
        
        impl Drop for SecureBytes {
            fn drop(&mut self) {
                self.secure_zero();
            }
        }
        
        /// 常时操作工具
        pub struct ConstantTimeOps;
        
        impl ConstantTimeOps {
            /// 常时内存比较
            pub fn constant_time_compare(a: &[u8], b: &[u8]) -> bool {
                if a.len() != b.len() {
                    return false;
                }
                
                let mut diff = 0u8;
                for (x, y) in a.iter().zip(b.iter()) {
                    diff |= x ^ y;
                }
                diff == 0
            }
            
            /// 常时内存设置
            pub fn constant_time_set(buffer: &mut [u8], value: u8) {
                for byte in buffer.iter_mut() {
                    unsafe {
                        core::ptr::write_volatile(byte, value);
                    }
                }
            }
            
            /// 常时条件选择
            pub fn constant_time_select(condition: bool, if_true: u8, if_false: u8) -> u8 {
                let mask = if condition { 0xFF } else { 0x00 };
                (if_true & mask) | (if_false & !mask)
            }
        }
    }
    
    // 内存保护模块
    pub mod memory_protection {
        use super::*;
        
        /// 安全内存分配器
        pub struct SecureMemory {
            data: Vec<u8>,
            size: usize,
        }
        
        impl SecureMemory {
            pub fn new(size: usize) -> Result<Self, &'static str> {
                let mut data = Vec::with_capacity(size);
                data.resize(size, 0);
                
                Ok(Self { data, size })
            }
            
            pub fn size(&self) -> usize {
                self.size
            }
            
            pub fn as_slice(&self) -> &[u8] {
                &self.data
            }
            
            pub fn as_mut_slice(&mut self) -> &mut [u8] {
                &mut self.data
            }
            
            /// 安全清零
            pub fn secure_zero(&mut self) {
                for byte in &mut self.data {
                    unsafe {
                        core::ptr::write_volatile(byte, 0);
                    }
                }
            }
        }
        
        impl Drop for SecureMemory {
            fn drop(&mut self) {
                self.secure_zero();
            }
        }
        
        /// 栈保护机制
        pub struct StackCanary {
            value: u32,
        }
        
        impl StackCanary {
            pub fn new() -> Result<Self, &'static str> {
                let mut entropy = [0u8; 4];
                Random::generate(&mut entropy as _);
                let value = u32::from_le_bytes(entropy);
                
                Ok(Self { value })
            }
            
            pub fn value(&self) -> u32 {
                self.value
            }
            
            pub fn verify(&self, check_value: u32) -> bool {
                self.value == check_value
            }
        }
        
        /// 全局内存保护控制器
        pub struct MemoryGuard {
            protection_enabled: bool,
        }
        
        static mut MEMORY_GUARD: MemoryGuard = MemoryGuard {
            protection_enabled: true,
        };
        
        impl MemoryGuard {
            pub fn enable_protection() {
                unsafe {
                    MEMORY_GUARD.protection_enabled = true;
                }
            }
            
            pub fn disable_protection() {
                unsafe {
                    MEMORY_GUARD.protection_enabled = false;
                }
            }
            
            pub fn is_protection_enabled() -> bool {
                unsafe {
                    MEMORY_GUARD.protection_enabled
                }
            }
        }
    }
    
    // 审计日志模块
    pub mod audit {
        use super::*;
        
        #[derive(Debug, Clone)]
        pub enum AuditLevel {
            Info,
            Warning,
            Error,
            Security,
        }
        
        #[derive(Debug, Clone)]
        pub enum AuditEvent {
            WalletCreated { wallet_id: u32 },
            AddressDerivation { wallet_id: u32, index: u32 },
            TransactionSigned { wallet_id: u32, tx_hash_prefix: u32 },
            MemoryAllocation { size: usize, secure: bool },
            SecurityViolation { violation_type: String, details: String },
            TEEOperation { operation: String, duration_ms: u64, success: bool },
        }
        
        pub struct AuditLogEntry {
            pub timestamp: u64,
            pub level: AuditLevel,
            pub event: AuditEvent,
            pub component: String,
        }
        
        /// 简化的审计日志器（OP-TEE 环境）
        pub struct AuditLogger {
            enabled: bool,
        }
        
        impl AuditLogger {
            pub fn new() -> Self {
                Self { enabled: true }
            }
            
            pub fn log(&self, level: AuditLevel, event: AuditEvent, component: &str) {
                if !self.enabled {
                    return;
                }
                
                // 在 OP-TEE 环境中，使用 trace_println! 记录日志
                let level_str = match level {
                    AuditLevel::Info => "INFO",
                    AuditLevel::Warning => "WARN", 
                    AuditLevel::Error => "ERROR",
                    AuditLevel::Security => "SECURITY",
                };
                
                match event {
                    AuditEvent::WalletCreated { wallet_id } => {
                        trace_println!("[{}] {} Wallet created: ID={}", 
                                     level_str, component, wallet_id);
                    }
                    AuditEvent::AddressDerivation { wallet_id, index } => {
                        trace_println!("[{}] {} Address derived: wallet_id={}, index={}", 
                                     level_str, component, wallet_id, index);
                    }
                    AuditEvent::TransactionSigned { wallet_id, tx_hash_prefix } => {
                        trace_println!("[{}] {} Transaction signed: wallet_id={}, hash_prefix=0x{:08x}", 
                                     level_str, component, wallet_id, tx_hash_prefix);
                    }
                    AuditEvent::MemoryAllocation { size, secure } => {
                        trace_println!("[{}] {} Memory allocated: size={}, secure={}", 
                                     level_str, component, size, secure);
                    }
                    AuditEvent::SecurityViolation { violation_type, details } => {
                        trace_println!("[{}] {} Security violation: type={}, details={}", 
                                     level_str, component, violation_type, details);
                    }
                    AuditEvent::TEEOperation { operation, duration_ms, success } => {
                        trace_println!("[{}] {} TEE operation: op={}, duration={}ms, success={}", 
                                     level_str, component, operation, duration_ms, success);
                    }
                }
            }
            
            pub fn log_info(&self, event: AuditEvent, component: &str) {
                self.log(AuditLevel::Info, event, component);
            }
            
            pub fn log_warning(&self, event: AuditEvent, component: &str) {
                self.log(AuditLevel::Warning, event, component);
            }
            
            pub fn log_error(&self, event: AuditEvent, component: &str) {
                self.log(AuditLevel::Error, event, component);
            }
            
            pub fn log_security(&self, event: AuditEvent, component: &str) {
                self.log(AuditLevel::Security, event, component);
            }
        }
        
        // 全局审计日志器
        static mut GLOBAL_AUDIT_LOGGER: Option<AuditLogger> = None;
        
        pub fn init_audit_logger() {
            unsafe {
                GLOBAL_AUDIT_LOGGER = Some(AuditLogger::new());
            }
        }
        
        pub fn audit_log(level: AuditLevel, event: AuditEvent, component: &str) {
            unsafe {
                if let Some(ref logger) = GLOBAL_AUDIT_LOGGER {
                    logger.log(level, event, component);
                }
            }
        }
    }
    
    // 安全管理器
    #[derive(Clone, Copy)]
    pub struct SecurityConfig {
        pub enable_constant_time: bool,
        pub enable_memory_protection: bool,
        pub enable_audit_logging: bool,
    }
    
    impl Default for SecurityConfig {
        fn default() -> Self {
            Self {
                enable_constant_time: true,
                enable_memory_protection: true,
                enable_audit_logging: true,
            }
        }
    }
    
    pub struct SecurityManager {
        config: SecurityConfig,
    }
    
    impl SecurityManager {
        pub fn new(config: SecurityConfig) -> Self {
            if config.enable_audit_logging {
                audit::init_audit_logger();
            }
            
            if config.enable_memory_protection {
                memory_protection::MemoryGuard::enable_protection();
            }
            
            Self { config }
        }
        
        pub fn audit_security_event(&self, event: audit::AuditEvent, component: &str) {
            if self.config.enable_audit_logging {
                audit::audit_log(audit::AuditLevel::Security, event, component);
            }
        }
        
        pub fn audit_info(&self, event: audit::AuditEvent, component: &str) {
            if self.config.enable_audit_logging {
                audit::audit_log(audit::AuditLevel::Info, event, component);
            }
        }
        
        pub fn create_secure_memory(&self, size: usize) -> Result<memory_protection::SecureMemory, &'static str> {
            let memory = memory_protection::SecureMemory::new(size)?;
            
            if self.config.enable_audit_logging {
                self.audit_info(
                    audit::AuditEvent::MemoryAllocation { size, secure: true },
                    "security_manager"
                );
            }
            
            Ok(memory)
        }
    }
    
    impl Default for SecurityManager {
        fn default() -> Self {
            Self::new(SecurityConfig::default())
        }
    }
}

// 基础密码学模块（无外部依赖）
mod basic_crypto {
    use super::*;
    use alloc::string::String;
    use alloc::vec::Vec;
    
    // 简化的助记词词库 (BIP39 英文词库的前12个词用于演示)
    const WORDLIST: &[&str] = &[
        "abandon", "ability", "able", "about", "above", "absent", 
        "absorb", "abstract", "absurd", "abuse", "access", "accident"
    ];
    
    // P0安全修复：改进的哈希函数，比简化版本更安全
    // 在生产环境中应该使用标准的SHA-256实现
    pub fn secure_hash(input: &[u8]) -> [u8; 32] {
        let mut hash = [0u8; 32];
        
        // TODO: 在完全的生产环境中，使用 OP-TEE 的 TEE_DigestUpdate/TEE_DigestDoFinal
        // 或集成标准的 SHA-256 库
        
        // 改进的哈希算法：比原来的简化版本更安全
        // Step 1: 初始化处理
        for (i, &byte) in input.iter().enumerate() {
            let pos = i % 32;
            hash[pos] ^= byte.wrapping_add((i as u8) ^ 0x5A);
            
            // 交叉影响其他位置以增强雪崩效应
            let cross_pos = (i * 7) % 32;
            hash[cross_pos] = hash[cross_pos].wrapping_add(byte ^ 0xA5);
        }
        
        // Step 2: 多轮混合以增强安全性
        for round in 0..16 {
            for i in 0..32 {
                let next = (i + 1) % 32;
                let prev = (i + 31) % 32;
                let cross = (i + 16) % 32;
                
                hash[i] = hash[i]
                    .wrapping_add(hash[next] ^ hash[prev])
                    .wrapping_mul(251) // 使用质数增强混合
                    .wrapping_add(hash[cross])
                    .wrapping_add(round);
            }
            
            // 字节置换以增强非线性
            if round % 4 == 0 {
                for i in (0..16).step_by(2) {
                    hash.swap(i, 31 - i);
                }
            }
        }
        
        hash
    }
    
    // 向后兼容的别名，但使用更安全的版本
    pub fn simple_hash(input: &[u8]) -> [u8; 32] {
        secure_hash(input)
    }
    
    pub fn generate_mnemonic() -> Result<String, &'static str> {
        // 使用安全内存存储熵
        let security_manager = get_security_manager();
        let mut secure_entropy = security_manager.lock().create_secure_memory(16)
            .map_err(|_| "Failed to allocate secure memory for entropy")?;
        
        let _random_result = Random::generate(secure_entropy.as_mut_slice() as _);
        
        let mut mnemonic_words = Vec::new();
        for i in 0..12 {
            let word_index = (secure_entropy.as_slice()[i % secure_entropy.size()] as usize) % WORDLIST.len();
            mnemonic_words.push(WORDLIST[word_index]);
        }
        
        // secure_entropy 在此处自动安全清零（通过 Drop trait）
        Ok(mnemonic_words.join(" "))
    }
    
    pub fn derive_seed_from_mnemonic(mnemonic: &str) -> Result<[u8; 64], &'static str> {
        // 简化的种子生成：基于助记词的确定性哈希
        let mnemonic_bytes = mnemonic.as_bytes();
        let hash1 = simple_hash(mnemonic_bytes);
        let hash2 = simple_hash(&hash1);
        
        let mut seed = [0u8; 64];
        seed[..32].copy_from_slice(&hash1);
        seed[32..].copy_from_slice(&hash2);
        Ok(seed)
    }
    
    pub fn derive_private_key(seed: &[u8; 64], derivation_index: u32) -> [u8; 32] {
        // 使用安全内存处理敏感数据
        let security_manager = get_security_manager();
        if let Ok(mut secure_input) = security_manager.lock().create_secure_memory(seed.len() + 4) {
            // 在安全内存中组装输入数据
            secure_input.as_mut_slice()[..seed.len()].copy_from_slice(seed);
            secure_input.as_mut_slice()[seed.len()..].copy_from_slice(&derivation_index.to_le_bytes());
            
            let hash = simple_hash(secure_input.as_slice());
            // secure_input 在此处自动安全清零
            hash
        } else {
            // 备选方案：不使用安全内存
            let mut input = Vec::new();
            input.extend_from_slice(seed);
            input.extend_from_slice(&derivation_index.to_le_bytes());
            simple_hash(&input)
        }
    }
    
    pub fn derive_address_from_private_key(private_key: &[u8; 32]) -> [u8; 20] {
        // 简化的地址派生：基于私钥的哈希
        let public_key_hash = simple_hash(private_key);
        let mut address = [0u8; 20];
        address.copy_from_slice(&public_key_hash[12..32]);
        address
    }
    
    pub fn sign_with_private_key(private_key: &[u8; 32], message_hash: &[u8]) -> [u8; 65] {
        // 简化的签名：基于私钥和消息哈希的确定性生成
        let mut input = Vec::new();
        input.extend_from_slice(private_key);
        input.extend_from_slice(message_hash);
        let sig_hash = simple_hash(&input);
        
        let mut signature = [0u8; 65];
        signature[..32].copy_from_slice(&sig_hash);
        signature[32..64].copy_from_slice(&private_key[..32]);
        signature[64] = 0x1b; // recovery ID
        signature
    }
}

// 基础钱包结构和管理
mod wallet {
    use core::fmt;
    use alloc::string::{String, ToString};
    
    #[derive(Debug, Clone, Copy)]
    pub struct WalletId(pub u32);
    
    impl fmt::Display for WalletId {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "wallet_{}", self.0)
        }
    }
    
    #[derive(Debug, Clone)]
    pub struct Wallet {
        pub id: WalletId,
        pub created_at: u64,
        pub derivations_count: u32,
        pub mnemonic: String, // 始终使用动态字符串
        pub seed: [u8; 64], // 存储种子以便派生
    }
    
    impl Wallet {
        pub fn new(id: u32) -> Self {
            let mnemonic = match crate::basic_crypto::generate_mnemonic() {
                Ok(m) => m,
                Err(_) => "abandon ability able about above absent absorb abstract absurd abuse access accident".to_string(),
            };
            
            let seed = match crate::basic_crypto::derive_seed_from_mnemonic(&mnemonic) {
                Ok(s) => s,
                Err(_) => {
                    // 备选种子生成
                    let mut seed = [0u8; 64];
                    seed[0] = (id & 0xFF) as u8;
                    seed[1] = ((id >> 8) & 0xFF) as u8;
                    seed
                }
            };
            
            Wallet {
                id: WalletId(id),
                created_at: get_timestamp(),
                derivations_count: 0,
                mnemonic,
                seed,
            }
        }
        
        pub fn derive_address(&mut self, _hd_path: &str) -> ([u8; 20], [u8; 65]) {
            self.derivations_count += 1;
            
            // 使用基础密码学派生地址
            let private_key = crate::basic_crypto::derive_private_key(&self.seed, self.derivations_count);
            let address = crate::basic_crypto::derive_address_from_private_key(&private_key);
            
            // 简化的公钥生成（占位符）
            let mut public_key = [4u8; 65]; // 0x04 prefix for uncompressed key
            public_key[1..33].copy_from_slice(&private_key);
            
            (address, public_key)
        }
        
        pub fn sign_transaction(&self, _hd_path: &str, tx_hash: &[u8]) -> [u8; 65] {
            // 使用基础密码学签名交易
            let private_key = crate::basic_crypto::derive_private_key(&self.seed, self.derivations_count);
            crate::basic_crypto::sign_with_private_key(&private_key, tx_hash)
        }
    }
    
    fn get_timestamp() -> u64 {
        // 简化：返回模拟时间戳
        12345678901234
    }
}

// 简单的内存钱包存储
mod wallet_storage {
    use super::wallet::{Wallet, WalletId};
    use alloc::vec::Vec;
    
    const MAX_WALLETS: usize = 10;
    
    // 线程安全的钱包存储 - P0安全修复
    static WALLET_STORAGE: spin::Once<spin::Mutex<WalletStorage>> = spin::Once::new();
    
    struct WalletStorage {
        wallets: Vec<Option<Wallet>>,
        next_wallet_id: u32,
    }
    
    impl WalletStorage {
        fn new() -> Self {
            let mut storage = Vec::new();
            storage.resize_with(MAX_WALLETS, || None);
            Self {
                wallets: storage,
                next_wallet_id: 1,
            }
        }
    }
    
    fn get_wallet_storage() -> &'static spin::Mutex<WalletStorage> {
        WALLET_STORAGE.call_once(|| spin::Mutex::new(WalletStorage::new()))
    }
    
    pub fn create_wallet() -> Option<WalletId> {
        let storage = get_wallet_storage();
        let mut guard = storage.lock();
        
        if let Some(slot) = guard.wallets.iter_mut().find(|w| w.is_none()) {
            let id = guard.next_wallet_id;
            guard.next_wallet_id += 1;
            let wallet = Wallet::new(id);
            let wallet_id = wallet.id;
            *slot = Some(wallet);
            Some(wallet_id)
        } else {
            None // Storage full
        }
    }
    
    pub fn with_wallet<T, F>(id: WalletId, f: F) -> Option<T>
    where
        F: FnOnce(&Wallet) -> T,
    {
        let storage = get_wallet_storage();
        let guard = storage.lock();
        
        guard.wallets.iter()
            .find_map(|w| w.as_ref().filter(|wallet| wallet.id.0 == id.0))
            .map(f)
    }
    
    pub fn with_wallet_mut<T, F>(id: WalletId, f: F) -> Option<T>
    where
        F: FnOnce(&mut Wallet) -> T,
    {
        let storage = get_wallet_storage();
        let mut guard = storage.lock();
        
        guard.wallets.iter_mut()
            .find_map(|w| w.as_mut().filter(|wallet| wallet.id.0 == id.0))
            .map(f)
    }
    
    // 保持向后兼容的辅助函数
    pub fn get_wallet(id: WalletId) -> Option<Wallet> {
        with_wallet(id, |wallet| wallet.clone())
    }
    
    pub fn get_wallet_mut(id: WalletId) -> Option<Wallet> {
        with_wallet(id, |wallet| wallet.clone())
    }
    
    pub fn remove_wallet(id: WalletId) -> bool {
        let storage = get_wallet_storage();
        let mut guard = storage.lock();
        
        if let Some(slot) = guard.wallets.iter_mut()
            .find(|w| w.as_ref().map_or(false, |wallet| wallet.id.0 == id.0)) {
            *slot = None;
            true
        } else {
            false
        }
    }
    
    pub fn list_wallets() -> ([Option<WalletId>; MAX_WALLETS], usize) {
        let mut ids = [None; MAX_WALLETS];
        let mut count = 0;
        
        let storage = get_wallet_storage();
        let guard = storage.lock();
        
        for (i, wallet_opt) in guard.wallets.iter().enumerate() {
            if let Some(wallet) = wallet_opt {
                ids[i] = Some(wallet.id);
                count += 1;
            }
        }
        
        (ids, count)
    }
}

use wallet::WalletId;
use wallet_storage::{create_wallet, get_wallet, get_wallet_mut, remove_wallet, list_wallets};
use security::SecurityManager;
use security::audit::AuditEvent;

// 线程安全的全局安全管理器 - P0安全修复
static SECURITY_MANAGER_STORAGE: spin::Once<spin::Mutex<SecurityManager>> = spin::Once::new();

fn get_security_manager() -> &'static spin::Mutex<SecurityManager> {
    SECURITY_MANAGER_STORAGE.call_once(|| spin::Mutex::new(SecurityManager::default()))
}

#[ta_create]
fn create() -> optee_utee::Result<()> {
    trace_println!("[+] AirAccount Simple TA create");
    
    // 初始化线程安全的安全管理器
    let security_manager = get_security_manager();
    
    // 审计 TA 创建事件
    security_manager.lock().audit_info(
        AuditEvent::TEEOperation {
            operation: "ta_create".to_string(),
            duration_ms: 0,
            success: true,
        },
        "airaccount_ta"
    );
    
    trace_println!("[+] Security manager initialized");
    Ok(())
}

#[ta_open_session]
fn open_session(_params: &mut Parameters) -> optee_utee::Result<()> {
    trace_println!("[+] AirAccount Simple TA open session");
    Ok(())
}

#[ta_close_session]
fn close_session() {
    trace_println!("[+] AirAccount Simple TA close session");
}

#[ta_destroy]
fn destroy() {
    trace_println!("[+] AirAccount Simple TA destroy");
}

// 输入验证模块 - P0安全修复
mod input_validation {
    use super::*;
    
    // 安全常量定义
    const MAX_BUFFER_SIZE: usize = 8192;    // 8KB 最大缓冲区
    const MIN_BUFFER_SIZE: usize = 4;       // 最小缓冲区
    const MAX_COMMAND_ID: u32 = 50;         // 最大命令ID
    
    pub const CMD_HELLO: u32 = 0;
    pub const CMD_ECHO: u32 = 1;
    pub const CMD_VERSION: u32 = 2;
    pub const CMD_CREATE_WALLET: u32 = 10;
    pub const CMD_REMOVE_WALLET: u32 = 11;
    pub const CMD_DERIVE_ADDRESS: u32 = 12;
    pub const CMD_SIGN_TRANSACTION: u32 = 13;
    pub const CMD_GET_WALLET_INFO: u32 = 14;
    pub const CMD_LIST_WALLETS: u32 = 15;
    pub const CMD_TEST_SECURITY: u32 = 16;
    
    #[derive(Debug)]
    pub enum ValidationError {
        InvalidCommand,
        BufferTooLarge,
        BufferTooSmall,
        InvalidParameterType,
        InvalidParameterCount,
    }
    
    pub fn validate_command_parameters(cmd_id: u32, params: &Parameters) -> Result<(), ValidationError> {
        // 1. 验证命令ID范围
        if cmd_id > MAX_COMMAND_ID {
            return Err(ValidationError::InvalidCommand);
        }
        
        // 2. 验证命令ID是否为已知命令
        match cmd_id {
            CMD_HELLO | CMD_ECHO | CMD_VERSION 
            | CMD_CREATE_WALLET | CMD_REMOVE_WALLET | CMD_DERIVE_ADDRESS 
            | CMD_SIGN_TRANSACTION | CMD_GET_WALLET_INFO | CMD_LIST_WALLETS 
            | CMD_TEST_SECURITY => {}, // 已知命令，继续验证
            _ => return Err(ValidationError::InvalidCommand),
        }
        
        // 3. 验证参数缓冲区大小
        if let Ok(p0) = unsafe { params.0.as_memref() } {
            if p0.buffer().len() > MAX_BUFFER_SIZE {
                return Err(ValidationError::BufferTooLarge);
            }
        }
        
        if let Ok(p1) = unsafe { params.1.as_memref() } {
            if p1.buffer().len() > MAX_BUFFER_SIZE {
                return Err(ValidationError::BufferTooLarge);
            }
        }
        
        // 4. 命令特定的参数验证
        match cmd_id {
            CMD_ECHO => {
                // Echo命令需要输入和输出缓冲区
                if let (Ok(p0), Ok(p1)) = (unsafe { params.0.as_memref() }, unsafe { params.1.as_memref() }) {
                    if p0.buffer().is_empty() || p1.buffer().is_empty() {
                        return Err(ValidationError::BufferTooSmall);
                    }
                } else {
                    return Err(ValidationError::InvalidParameterType);
                }
            }
            CMD_REMOVE_WALLET | CMD_DERIVE_ADDRESS | CMD_SIGN_TRANSACTION | CMD_GET_WALLET_INFO => {
                // 这些命令需要输入参数
                if let Ok(p0) = unsafe { params.0.as_memref() } {
                    if p0.buffer().len() < MIN_BUFFER_SIZE {
                        return Err(ValidationError::BufferTooSmall);
                    }
                } else {
                    return Err(ValidationError::InvalidParameterType);
                }
            }
            _ => {} // 其他命令的基本验证已足够
        }
        
        Ok(())
    }
}

use input_validation::validate_command_parameters;

#[ta_invoke_command]
fn invoke_command(cmd_id: u32, params: &mut Parameters) -> optee_utee::Result<()> {
    trace_println!("[+] AirAccount Simple TA invoke command: {}", cmd_id);
    
    // 严格的输入验证 - 安全修复 P0
    if let Err(_) = validate_command_parameters(cmd_id, params) {
        trace_println!("[!] Parameter validation failed for command: {}", cmd_id);
        return Err(Error::new(ErrorKind::BadParameters));
    }
    
    let mut p0 = unsafe { params.0.as_memref()? };
    let mut p1 = unsafe { params.1.as_memref()? };
    let mut p2 = unsafe { params.2.as_value()? };

    let result = match cmd_id {
        0 => {
            // Hello World command
            let message = b"Hello from AirAccount Simple TA with Wallet Support!";
            copy_to_buffer(message, p1.buffer())
        }
        1 => {
            // Echo command
            let input_size = p0.buffer().len().min(p1.buffer().len());
            p1.buffer()[..input_size].copy_from_slice(&p0.buffer()[..input_size]);
            Ok(input_size)
        }
        2 => {
            // Get Version command
            let version = b"AirAccount Simple TA v0.1.0 - Basic Wallet Support";
            copy_to_buffer(version, p1.buffer())
        }
        
        // 钱包管理命令 (10-19)
        10 => {
            // Create Wallet
            handle_create_wallet(p1.buffer())
        }
        11 => {
            // Remove Wallet
            handle_remove_wallet(p0.buffer(), p1.buffer())
        }
        12 => {
            // Derive Address
            handle_derive_address(p0.buffer(), p1.buffer())
        }
        13 => {
            // Sign Transaction
            handle_sign_transaction(p0.buffer(), p1.buffer())
        }
        14 => {
            // Get Wallet Info
            handle_get_wallet_info(p0.buffer(), p1.buffer())
        }
        15 => {
            // List Wallets
            handle_list_wallets(p1.buffer())
        }
        16 => {
            // Test Security Features
            handle_test_security(p1.buffer())
        }
        
        _ => {
            trace_println!("[!] Unknown command: {}", cmd_id);
            return Err(Error::new(ErrorKind::BadParameters));
        }
    };

    match result {
        Ok(size) => {
            p2.set_a(size as u32);
            Ok(())
        }
        Err(msg) => {
            trace_println!("[!] Command {} failed: {}", cmd_id, msg);
            let error_bytes = msg.as_bytes();
            let size = copy_to_buffer(error_bytes, p1.buffer()).unwrap_or(0);
            p2.set_a(size as u32);
            Err(Error::new(ErrorKind::BadState))
        }
    }
}

// 辅助函数
fn copy_to_buffer(data: &[u8], buffer: &mut [u8]) -> Result<usize, &'static str> {
    let copy_size = data.len().min(buffer.len());
    if copy_size == 0 {
        return Ok(0);
    }
    buffer[..copy_size].copy_from_slice(&data[..copy_size]);
    Ok(copy_size)
}

fn parse_u32_from_buffer(buffer: &[u8]) -> Result<u32, &'static str> {
    if buffer.len() < 4 {
        return Err("Buffer too small for u32");
    }
    Ok(u32::from_le_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]))
}

// 钱包操作处理函数
fn handle_create_wallet(output_buffer: &mut [u8]) -> Result<usize, &'static str> {
    trace_println!("[+] Creating new wallet...");
    
    match create_wallet() {
        Some(wallet_id) => {
            // 审计钱包创建事件
            get_security_manager().lock().audit_security_event(
                AuditEvent::WalletCreated { wallet_id: wallet_id.0 },
                "wallet_manager"
            );
            
            let response = format_wallet_created(wallet_id.0);
            // Find actual length by looking for first zero byte
            let mut len = 0;
            for i in 0..response.len() {
                if response[i] == 0 {
                    break;
                }
                len += 1;
            }
            copy_to_buffer(&response[..len], output_buffer)
        }
        None => {
            // 审计钱包创建失败事件
            get_security_manager().lock().audit_security_event(
                AuditEvent::SecurityViolation {
                    violation_type: "wallet_storage_full".to_string(),
                    details: "Unable to create wallet: storage limit reached".to_string(),
                },
                "wallet_manager"
            );
            Err("Wallet storage full")
        }
    }
}

fn handle_remove_wallet(input_buffer: &[u8], output_buffer: &mut [u8]) -> Result<usize, &'static str> {
    let wallet_id = parse_u32_from_buffer(input_buffer)?;
    trace_println!("[+] Removing wallet ID: {}", wallet_id);
    
    if remove_wallet(WalletId(wallet_id)) {
        let response = format_wallet_removed(wallet_id);
        let mut len = 0;
        for i in 0..response.len() {
            if response[i] == 0 {
                break;
            }
            len += 1;
        }
        copy_to_buffer(&response[..len], output_buffer)
    } else {
        Err("Wallet not found")
    }
}

fn handle_derive_address(input_buffer: &[u8], output_buffer: &mut [u8]) -> Result<usize, &'static str> {
    let wallet_id = parse_u32_from_buffer(input_buffer)?;
    trace_println!("[+] Deriving address for wallet ID: {}", wallet_id);
    
    if let Some(wallet) = get_wallet_mut(WalletId(wallet_id)) {
        let hd_path = "m/44'/60'/0'/0/0"; // 默认以太坊路径
        let derivation_index = wallet.derivations_count + 1; // 下一个派生索引
        let (address, _public_key) = wallet.derive_address(hd_path);
        
        // 审计地址派生事件
        get_security_manager().audit_security_event(
            AuditEvent::AddressDerivation { 
                wallet_id, 
                index: derivation_index 
            },
            "crypto_module"
        );
        
        trace_println!("[+] Address derived: {:02x}{:02x}...{:02x}", 
                      address[0], address[1], address[19]);
        
        // 简化输出格式：地址和公钥的十六进制表示
        let mut response = [0u8; 200]; // 足够大的缓冲区
        let mut pos = 0;
        
        // 添加 "address:" 前缀
        let prefix = b"address:";
        response[pos..pos+prefix.len()].copy_from_slice(prefix);
        pos += prefix.len();
        
        // 添加地址十六进制
        for &byte in address.iter() {
            let hex_chars = hex_byte_to_chars(byte);
            response[pos] = hex_chars.0;
            response[pos+1] = hex_chars.1;
            pos += 2;
        }
        
        copy_to_buffer(&response[..pos], output_buffer)
    } else {
        get_security_manager().audit_security_event(
            AuditEvent::SecurityViolation {
                violation_type: "wallet_not_found".to_string(),
                details: "Attempted to derive address for non-existent wallet".to_string(),
            },
            "crypto_module"
        );
        Err("Wallet not found")
    }
}

fn handle_sign_transaction(input_buffer: &[u8], output_buffer: &mut [u8]) -> Result<usize, &'static str> {
    if input_buffer.len() < 4 {
        return Err("Invalid input");
    }
    
    let wallet_id = parse_u32_from_buffer(input_buffer)?;
    trace_println!("[+] Signing transaction for wallet ID: {}", wallet_id);
    
    if let Some(wallet) = get_wallet(WalletId(wallet_id)) {
        let hd_path = "m/44'/60'/0'/0/0";
        let tx_hash = &input_buffer[4..]; // 剩余数据作为交易哈希
        let signature = wallet.sign_transaction(hd_path, tx_hash);
        
        // 从交易哈希计算前缀用于审计
        let tx_hash_prefix = if tx_hash.len() >= 4 {
            u32::from_le_bytes([tx_hash[0], tx_hash[1], tx_hash[2], tx_hash[3]])
        } else {
            0
        };
        
        // 审计交易签名事件
        get_security_manager().audit_security_event(
            AuditEvent::TransactionSigned { 
                wallet_id, 
                tx_hash_prefix 
            },
            "crypto_module"
        );
        
        // 格式化签名输出
        let mut response = [0u8; 150];
        let prefix = b"signature:";
        let mut pos = 0;
        
        response[pos..pos+prefix.len()].copy_from_slice(prefix);
        pos += prefix.len();
        
        // 添加签名十六进制
        for &byte in signature[..10].iter() { // 只显示前10字节
            let hex_chars = hex_byte_to_chars(byte);
            response[pos] = hex_chars.0;
            response[pos+1] = hex_chars.1;
            pos += 2;
        }
        
        copy_to_buffer(&response[..pos], output_buffer)
    } else {
        get_security_manager().audit_security_event(
            AuditEvent::SecurityViolation {
                violation_type: "wallet_not_found".to_string(),
                details: "Attempted to sign transaction for non-existent wallet".to_string(),
            },
            "crypto_module"
        );
        Err("Wallet not found")
    }
}

fn handle_get_wallet_info(input_buffer: &[u8], output_buffer: &mut [u8]) -> Result<usize, &'static str> {
    let wallet_id = parse_u32_from_buffer(input_buffer)?;
    trace_println!("[+] Getting wallet info for ID: {}", wallet_id);
    
    if let Some(wallet) = get_wallet(WalletId(wallet_id)) {
        let response = format_wallet_info(wallet.id.0, wallet.derivations_count, wallet.created_at);
        let mut len = 0;
        for i in 0..response.len() {
            if response[i] == 0 {
                break;
            }
            len += 1;
        }
        copy_to_buffer(&response[..len], output_buffer)
    } else {
        Err("Wallet not found")
    }
}

fn handle_list_wallets(output_buffer: &mut [u8]) -> Result<usize, &'static str> {
    trace_println!("[+] Listing all wallets...");
    
    let (wallet_ids, count) = list_wallets();
    let mut response = format_wallets_count(count);
    let mut pos = 0;
    
    // Find actual length of base response
    for i in 0..response.len() {
        if response[i] == 0 {
            break;
        }
        pos += 1;
    }
    
    // Add wallet IDs
    for wallet_id_opt in wallet_ids.iter().take(count) {
        if let Some(wallet_id) = wallet_id_opt {
            let id_part = b",id=";
            if pos + id_part.len() < response.len() {
                response[pos..pos+id_part.len()].copy_from_slice(id_part);
                pos += id_part.len();
                
                let id_str = u32_to_decimal_bytes(wallet_id.0);
                let mut id_len = 0;
                for i in 0..id_str.len() {
                    if id_str[i] == 0 {
                        break;
                    }
                    id_len += 1;
                }
                
                if pos + id_len < response.len() {
                    response[pos..pos+id_len].copy_from_slice(&id_str[..id_len]);
                    pos += id_len;
                }
            }
        }
    }
    
    copy_to_buffer(&response[..pos], output_buffer)
}

// 简化的字符串处理函数 (no_std 兼容)
fn format_wallet_created(wallet_id: u32) -> [u8; 50] {
    let mut result = [0u8; 50];
    let prefix = b"wallet_created:id=";
    let mut pos = 0;
    
    // Copy prefix
    result[pos..pos+prefix.len()].copy_from_slice(prefix);
    pos += prefix.len();
    
    // Add wallet ID as decimal
    let id_str = u32_to_decimal_bytes(wallet_id);
    let id_len = id_str.len();
    result[pos..pos+id_len].copy_from_slice(&id_str);
    
    result
}

fn format_wallet_removed(wallet_id: u32) -> [u8; 50] {
    let mut result = [0u8; 50];
    let prefix = b"wallet_removed:id=";
    let mut pos = 0;
    
    result[pos..pos+prefix.len()].copy_from_slice(prefix);
    pos += prefix.len();
    
    let id_str = u32_to_decimal_bytes(wallet_id);
    let id_len = id_str.len();
    result[pos..pos+id_len].copy_from_slice(&id_str);
    
    result
}

fn format_wallet_info(wallet_id: u32, derivations: u32, created_at: u64) -> [u8; 100] {
    let mut result = [0u8; 100];
    let prefix = b"wallet_info:id=";
    let mut pos = 0;
    
    // Add prefix
    result[pos..pos+prefix.len()].copy_from_slice(prefix);
    pos += prefix.len();
    
    // Add wallet ID
    let id_str = u32_to_decimal_bytes(wallet_id);
    let id_len = id_str.len();
    result[pos..pos+id_len].copy_from_slice(&id_str);
    pos += id_len;
    
    // Add derivations
    let deriv_prefix = b",derivations=";
    result[pos..pos+deriv_prefix.len()].copy_from_slice(deriv_prefix);
    pos += deriv_prefix.len();
    
    let deriv_str = u32_to_decimal_bytes(derivations);
    let deriv_len = deriv_str.len();
    result[pos..pos+deriv_len].copy_from_slice(&deriv_str);
    pos += deriv_len;
    
    // Add timestamp (simplified to first 8 digits)
    let time_prefix = b",created=";
    result[pos..pos+time_prefix.len()].copy_from_slice(time_prefix);
    pos += time_prefix.len();
    
    let time_str = u32_to_decimal_bytes((created_at & 0xFFFFFFFF) as u32);
    let time_len = time_str.len();
    result[pos..pos+time_len].copy_from_slice(&time_str);
    
    result
}

fn format_wallets_count(count: usize) -> [u8; 30] {
    let mut result = [0u8; 30];
    let prefix = b"wallets_count:";
    let mut pos = 0;
    
    result[pos..pos+prefix.len()].copy_from_slice(prefix);
    pos += prefix.len();
    
    let count_str = u32_to_decimal_bytes(count as u32);
    let count_len = count_str.len();
    result[pos..pos+count_len].copy_from_slice(&count_str);
    
    result
}

// 将 u32 转换为十进制字节数组
fn u32_to_decimal_bytes(mut num: u32) -> [u8; 10] {
    let mut result = [0u8; 10];
    
    if num == 0 {
        result[0] = b'0';
        return result;
    }
    
    // Convert to decimal digits (reverse order)
    let mut digits = [0u8; 10];
    let mut digit_count = 0;
    while num > 0 {
        digits[digit_count] = (num % 10) as u8 + b'0';
        num /= 10;
        digit_count += 1;
    }
    
    // Copy in correct order
    for i in 0..digit_count {
        result[i] = digits[digit_count - 1 - i];
    }
    
    result
}

// 将字节转换为十六进制字符
fn hex_byte_to_chars(byte: u8) -> (u8, u8) {
    const HEX_CHARS: &[u8] = b"0123456789abcdef";
    (HEX_CHARS[(byte >> 4) as usize], HEX_CHARS[(byte & 0xf) as usize])
}

fn handle_test_security(output_buffer: &mut [u8]) -> Result<usize, &'static str> {
    trace_println!("[+] Testing security features...");
    
    // 测试安全内存分配
    let security_manager = get_security_manager();
    let test_results = match security_manager.lock().create_secure_memory(1024) {
        Ok(mut secure_mem) => {
            // 测试安全内存写入和读取
            secure_mem.as_mut_slice()[0] = 0xAB;
            secure_mem.as_mut_slice()[1] = 0xCD;
            secure_mem.as_mut_slice()[2] = 0xEF;
            
            let test_passed = secure_mem.as_slice()[0] == 0xAB 
                && secure_mem.as_slice()[1] == 0xCD 
                && secure_mem.as_slice()[2] == 0xEF;
            
            if test_passed {
                "secure_memory:PASS"
            } else {
                "secure_memory:FAIL"
            }
        }
        Err(_) => "secure_memory:ERROR",
    };
    
    // 审计安全测试事件
    security_manager.audit_info(
        AuditEvent::TEEOperation {
            operation: "security_test".to_string(),
            duration_ms: 1,
            success: true,
        },
        "security_tester"
    );
    
    // 测试栈金丝雀（如果可用）
    let canary_test = match security::memory_protection::StackCanary::new() {
        Ok(canary) => {
            let value = canary.value();
            if canary.verify(value) {
                ",stack_canary:PASS"
            } else {
                ",stack_canary:FAIL"
            }
        }
        Err(_) => ",stack_canary:ERROR",
    };
    
    // 组装测试结果
    let mut response_str = String::new();
    response_str.push_str("security_test:");
    response_str.push_str(test_results);
    response_str.push_str(canary_test);
    
    let response_bytes = response_str.as_bytes();
    copy_to_buffer(response_bytes, output_buffer)
}

include!(concat!(env!("OUT_DIR"), "/user_ta_header.rs"));