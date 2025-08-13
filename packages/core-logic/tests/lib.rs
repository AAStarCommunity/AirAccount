// Licensed to AirAccount under the Apache License, Version 2.0
// Test library for AirAccount core logic

// 导入测试模块
pub mod test_framework;
pub mod integration;
pub mod business_scenarios;
pub mod stress_performance;
pub mod security_enhanced;
pub mod fault_recovery;

// 重新导出测试功能
pub use test_framework::*;