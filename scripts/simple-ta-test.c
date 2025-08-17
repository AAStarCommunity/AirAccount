/*
 * Simple TA Test Tool - 直接测试TA功能而不依赖CA
 * 用于在构建CA之前验证TA是否正常工作
 */

#include <err.h>
#include <stdio.h>
#include <string.h>
#include <stdlib.h>
#include <unistd.h>

/* OP-TEE headers */
#include <tee_client_api.h>

/* UUID of the trusted application */
static const TEEC_UUID ta_uuid = {
    0x11223344, 0x5566, 0x7788,
    { 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x01 }
};

/* 测试函数 */
static int test_hello_world(TEEC_Session *session) {
    TEEC_Operation op;
    TEEC_Result res;
    char output_buffer[256] = {0};
    uint32_t output_len = 0;

    printf("[TEST] Hello World Command (CMD_ID=0)...\n");

    memset(&op, 0, sizeof(op));
    op.paramTypes = TEEC_PARAM_TYPES(TEEC_NONE, TEEC_MEMREF_TEMP_OUTPUT, 
                                    TEEC_VALUE_OUTPUT, TEEC_NONE);

    op.params[1].tmpref.buffer = output_buffer;
    op.params[1].tmpref.size = sizeof(output_buffer);

    res = TEEC_InvokeCommand(session, 0, &op, NULL);
    if (res != TEEC_SUCCESS) {
        printf("❌ Hello World failed: 0x%x\n", res);
        return -1;
    }

    output_len = op.params[2].value.a;
    output_buffer[output_len] = '\0';
    
    printf("✅ Hello World response: %s\n", output_buffer);
    printf("✅ Response length: %u bytes\n", output_len);
    return 0;
}

static int test_echo(TEEC_Session *session) {
    TEEC_Operation op;
    TEEC_Result res;
    char input_message[] = "Test Echo Message";
    char output_buffer[256] = {0};
    uint32_t output_len = 0;

    printf("[TEST] Echo Command (CMD_ID=1)...\n");

    memset(&op, 0, sizeof(op));
    op.paramTypes = TEEC_PARAM_TYPES(TEEC_MEMREF_TEMP_INPUT, TEEC_MEMREF_TEMP_OUTPUT, 
                                    TEEC_VALUE_OUTPUT, TEEC_NONE);

    op.params[0].tmpref.buffer = input_message;
    op.params[0].tmpref.size = strlen(input_message);
    
    op.params[1].tmpref.buffer = output_buffer;
    op.params[1].tmpref.size = sizeof(output_buffer);

    res = TEEC_InvokeCommand(session, 1, &op, NULL);
    if (res != TEEC_SUCCESS) {
        printf("❌ Echo failed: 0x%x\n", res);
        return -1;
    }

    output_len = op.params[2].value.a;
    output_buffer[output_len] = '\0';
    
    printf("✅ Echo input: %s\n", input_message);
    printf("✅ Echo output: %s\n", output_buffer);
    printf("✅ Response length: %u bytes\n", output_len);
    
    if (strcmp(input_message, output_buffer) == 0) {
        printf("✅ Echo test PASSED\n");
        return 0;
    } else {
        printf("❌ Echo test FAILED - output doesn't match input\n");
        return -1;
    }
}

static int test_version(TEEC_Session *session) {
    TEEC_Operation op;
    TEEC_Result res;
    char output_buffer[256] = {0};
    uint32_t output_len = 0;

    printf("[TEST] Version Command (CMD_ID=2)...\n");

    memset(&op, 0, sizeof(op));
    op.paramTypes = TEEC_PARAM_TYPES(TEEC_NONE, TEEC_MEMREF_TEMP_OUTPUT, 
                                    TEEC_VALUE_OUTPUT, TEEC_NONE);

    op.params[1].tmpref.buffer = output_buffer;
    op.params[1].tmpref.size = sizeof(output_buffer);

    res = TEEC_InvokeCommand(session, 2, &op, NULL);
    if (res != TEEC_SUCCESS) {
        printf("❌ Version failed: 0x%x\n", res);
        return -1;
    }

    output_len = op.params[2].value.a;
    output_buffer[output_len] = '\0';
    
    printf("✅ Version response: %s\n", output_buffer);
    printf("✅ Response length: %u bytes\n", output_len);
    return 0;
}

static int test_security_check(TEEC_Session *session) {
    TEEC_Operation op;
    TEEC_Result res;
    char output_buffer[256] = {0};
    uint32_t output_len = 0;

    printf("[TEST] Security Check Command (CMD_ID=10)...\n");

    memset(&op, 0, sizeof(op));
    op.paramTypes = TEEC_PARAM_TYPES(TEEC_NONE, TEEC_MEMREF_TEMP_OUTPUT, 
                                    TEEC_VALUE_OUTPUT, TEEC_NONE);

    op.params[1].tmpref.buffer = output_buffer;
    op.params[1].tmpref.size = sizeof(output_buffer);

    res = TEEC_InvokeCommand(session, 10, &op, NULL);
    if (res != TEEC_SUCCESS) {
        printf("❌ Security Check failed: 0x%x\n", res);
        return -1;
    }

    output_len = op.params[2].value.a;
    output_buffer[output_len] = '\0';
    
    printf("✅ Security Check response: %s\n", output_buffer);
    printf("✅ Response length: %u bytes\n", output_len);
    return 0;
}

int main(int argc, char *argv[]) {
    TEEC_Result res;
    TEEC_Context ctx;
    TEEC_Session session;
    TEEC_Operation op;
    int test_count = 0;
    int passed_count = 0;

    printf("🔧 AirAccount Simple TA Test Tool\n");
    printf("📝 Testing TA directly without CA dependency\n\n");

    /* 初始化TEE上下文 */
    res = TEEC_InitializeContext(NULL, &ctx);
    if (res != TEEC_SUCCESS) {
        errx(1, "❌ TEEC_InitializeContext failed: 0x%x", res);
    }
    printf("✅ TEE Context initialized\n");

    /* 打开会话 */
    memset(&op, 0, sizeof(op));
    op.paramTypes = TEEC_PARAM_TYPES(TEEC_NONE, TEEC_NONE, TEEC_NONE, TEEC_NONE);

    res = TEEC_OpenSession(&ctx, &session, &ta_uuid, 
                          TEEC_LOGIN_PUBLIC, NULL, &op, NULL);
    if (res != TEEC_SUCCESS) {
        errx(1, "❌ TEEC_OpenSession failed: 0x%x", res);
    }
    printf("✅ Session opened with AirAccount TA\n\n");

    /* 运行测试 */
    printf("🚀 Starting TA functionality tests...\n\n");

    // 测试Hello World
    test_count++;
    if (test_hello_world(&session) == 0) {
        passed_count++;
    }
    printf("\n");

    // 测试Echo
    test_count++;
    if (test_echo(&session) == 0) {
        passed_count++;
    }
    printf("\n");

    // 测试Version
    test_count++;
    if (test_version(&session) == 0) {
        passed_count++;
    }
    printf("\n");

    // 测试Security Check
    test_count++;
    if (test_security_check(&session) == 0) {
        passed_count++;
    }
    printf("\n");

    /* 测试结果 */
    printf("📊 Test Results: %d/%d tests passed (%.1f%%)\n", 
           passed_count, test_count, 
           (float)passed_count / test_count * 100.0);

    if (passed_count == test_count) {
        printf("🎉 All tests PASSED! TA is working correctly.\n");
    } else {
        printf("⚠️  Some tests FAILED. Check TA implementation.\n");
    }

    /* 清理 */
    TEEC_CloseSession(&session);
    TEEC_FinalizeContext(&ctx);

    return (passed_count == test_count) ? 0 : 1;
}