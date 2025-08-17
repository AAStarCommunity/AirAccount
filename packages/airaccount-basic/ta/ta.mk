# Basic TA Makefile - 用于编译
# UUID 必须与 CA 匹配
CFG_TEE_TA_UUID ?= 11223344-5566-7788-99aa-bbccddeeff01

# 基本设置
CROSS_COMPILE ?= aarch64-linux-gnu-
CC = $(CROSS_COMPILE)gcc
LD = $(CROSS_COMPILE)ld

# 编译目标
TARGET = $(CFG_TEE_TA_UUID).ta

# 源文件
RUST_TARGET_PATH = ../../../target/aarch64-unknown-optee-trustzone/release/airaccount_basic_ta

.PHONY: all clean

all: $(TARGET)

$(TARGET): 
	cd .. && cargo build --release
	cp $(RUST_TARGET_PATH) $(TARGET)

clean:
	rm -f $(TARGET)
	cd .. && cargo clean