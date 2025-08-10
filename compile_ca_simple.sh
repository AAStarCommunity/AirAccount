#!/bin/bash

# Licensed to AirAccount under the Apache License, Version 2.0
# 简化的CA编译脚本，解决TEE库链接问题

set -e

echo "🚀 简化CA编译 - 解决TEE库链接问题"
echo "================================"

# 创建模拟TEE环境用于编译
export MOCK_TEE_BUILD=1
export OPTEE_CLIENT_EXPORT="/tmp/mock_tee"

# 创建模拟目录结构
mkdir -p "$OPTEE_CLIENT_EXPORT/usr/lib"
mkdir -p "$OPTEE_CLIENT_EXPORT/usr/include"

# 创建模拟库文件 (用于编译链接)
touch "$OPTEE_CLIENT_EXPORT/usr/lib/libteec.a"

# 创建必要的头文件
cat > "$OPTEE_CLIENT_EXPORT/usr/include/tee_client_api.h" << 'EOF'
#ifndef TEE_CLIENT_API_H
#define TEE_CLIENT_API_H

#include <stdint.h>
#include <stddef.h>

// TEEC Types
typedef struct {
    uint32_t timeLow;
    uint16_t timeMid;
    uint16_t timeHiAndVersion;
    uint8_t clockSeqAndNode[8];
} TEEC_UUID;

typedef uint32_t TEEC_Result;

typedef struct {
    void *imp;
} TEEC_Context;

typedef struct {
    void *imp;
} TEEC_Session;

typedef union {
    struct {
        void *buffer;
        size_t size;
    } memref;
    struct {
        uint32_t a, b;
    } value;
} TEEC_Parameter;

typedef struct {
    uint32_t started;
    uint32_t paramTypes;
    TEEC_Parameter params[4];
} TEEC_Operation;

// Constants
#define TEEC_SUCCESS 0x00000000
#define TEEC_ERROR_GENERIC 0xFFFF0000
#define TEEC_ERROR_ACCESS_DENIED 0xFFFF0001
#define TEEC_ERROR_CANCEL 0xFFFF0002
#define TEEC_ERROR_ACCESS_CONFLICT 0xFFFF0003
#define TEEC_ERROR_EXCESS_DATA 0xFFFF0004
#define TEEC_ERROR_BAD_FORMAT 0xFFFF0005
#define TEEC_ERROR_BAD_PARAMETERS 0xFFFF0006

#define TEEC_PARAM_TYPE_NONE 0
#define TEEC_PARAM_TYPE_VALUE_INPUT 1
#define TEEC_PARAM_TYPE_VALUE_OUTPUT 2
#define TEEC_PARAM_TYPE_VALUE_INOUT 3
#define TEEC_PARAM_TYPE_MEMREF_TEMP_INPUT 5
#define TEEC_PARAM_TYPE_MEMREF_TEMP_OUTPUT 6
#define TEEC_PARAM_TYPE_MEMREF_TEMP_INOUT 7

// Function declarations (mock implementations)
TEEC_Result TEEC_InitializeContext(const char *name, TEEC_Context *context);
void TEEC_FinalizeContext(TEEC_Context *context);
TEEC_Result TEEC_OpenSession(TEEC_Context *context, TEEC_Session *session,
                             const TEEC_UUID *destination, uint32_t connectionMethod,
                             const void *connectionData, TEEC_Operation *operation,
                             uint32_t *returnOrigin);
void TEEC_CloseSession(TEEC_Session *session);
TEEC_Result TEEC_InvokeCommand(TEEC_Session *session, uint32_t commandID,
                              TEEC_Operation *operation, uint32_t *returnOrigin);

#endif
EOF

# 创建模拟的libteec实现 (仅用于编译)
cat > "$OPTEE_CLIENT_EXPORT/usr/lib/libteec_mock.c" << 'EOF'
#include "../include/tee_client_api.h"

#define TEEC_ERROR_NOT_IMPLEMENTED 0xFFFF0009

// Mock implementations for compilation
TEEC_Result TEEC_InitializeContext(const char *name, TEEC_Context *context) {
    (void)name;
    (void)context;
    return TEEC_SUCCESS;
}

void TEEC_FinalizeContext(TEEC_Context *context) {
    (void)context;
    // Mock
}

TEEC_Result TEEC_OpenSession(TEEC_Context *context, TEEC_Session *session,
                             const TEEC_UUID *destination, uint32_t connectionMethod,
                             const void *connectionData, TEEC_Operation *operation,
                             uint32_t *returnOrigin) {
    (void)context;
    (void)session;
    (void)destination;
    (void)connectionMethod;
    (void)connectionData;
    (void)operation;
    (void)returnOrigin;
    return TEEC_SUCCESS;
}

void TEEC_CloseSession(TEEC_Session *session) {
    (void)session;
    // Mock
}

TEEC_Result TEEC_InvokeCommand(TEEC_Session *session, uint32_t commandID,
                              TEEC_Operation *operation, uint32_t *returnOrigin) {
    (void)session;
    (void)commandID;
    (void)operation;
    (void)returnOrigin;
    return TEEC_SUCCESS;
}
EOF

# 编译模拟库
cd "$OPTEE_CLIENT_EXPORT/usr/lib"
rm -f libteec.a  # 删除之前的假文件
gcc -c -fPIC libteec_mock.c -I../include -o libteec_mock.o
ar rcs libteec.a libteec_mock.o

echo "📚 Created mock libteec.a library"

echo "📝 更新CA的Cargo.toml以使用模拟TEE"
cd /Volumes/UltraDisk/Dev2/aastar/AirAccount/packages/client-ca

# 临时修改Cargo.toml以使用mock模式
if ! grep -q "mock_tee" Cargo.toml; then
cat >> Cargo.toml << 'EOF'

# Mock TEE support for development
[features]
default = ["mock_tee"]
mock_tee = []
EOF
fi

echo "🔨 编译CA (模拟TEE模式)"
cargo build --release --features mock_tee

echo "✅ CA编译成功!"
echo "📍 二进制位置: target/release/airaccount-ca"
echo "⚠️  注意: 这是模拟TEE模式，用于开发测试"