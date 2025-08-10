# Scripts Directory

This directory contains all build, test, and utility scripts for the AirAccount project.

## Build Scripts

### TEE Build
- `build_tee.sh` - Main TEE build script
- `build_ca.sh` - Client Application build script
- `build_real_tee.sh` - Real hardware TEE build script
- `compile_ca_simple.sh` - Simple CA compilation script

### Test Scripts
- `test_airaccount_manual.sh` - Manual testing script
- `test_ca_simple.sh` - Simple CA test script
- `run_final_validation.sh` - Final validation test suite
- `create_test_summary.sh` - Generate test summary reports

### Utility Scripts
- `optimize_build_performance.sh` - Build performance optimization
- `cleanup_rust_cache.sh` - Clean Rust build cache
- `fly.sh` - Quick deployment script
- `simple_test.sh` - Simple test runner
- `verify_build.sh` - Build verification script

## Usage

All scripts are executable. Run from the project root:

```bash
./scripts/build_tee.sh
./scripts/test_ca_simple.sh
```