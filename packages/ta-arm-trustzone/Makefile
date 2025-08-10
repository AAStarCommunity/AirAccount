# Licensed to AirAccount under the Apache License, Version 2.0

TARGET ?= aarch64-unknown-optee
CROSS_COMPILE ?= aarch64-linux-gnu-

# Export environment variables
export STD=y
export OPTEE_DIR=../../third_party/incubator-teaclave-trustzone-sdk/optee/
export TA_DEV_KIT_DIR=../../third_party/incubator-teaclave-trustzone-sdk/optee/optee_os/out/arm-plat-vexpress/export-ta_arm64
export OPTEE_CLIENT_EXPORT=../../third_party/incubator-teaclave-trustzone-sdk/optee/optee_client/export_arm64

.PHONY: all
all: 
	@cargo build --target $(TARGET) --release
	@echo "Built TA: $(TARGET)"

.PHONY: clean
clean:
	@cargo clean