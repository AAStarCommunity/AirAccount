export TEACLAVE_TOOLCHAIN_BASE ?= /opt/teaclave
export OPTEE_DIR ?= $(TEACLAVE_TOOLCHAIN_BASE)/optee
export OPTEE_OS_DIR ?= $(OPTEE_DIR)/optee_os
export OPTEE_CLIENT_DIR ?= $(OPTEE_DIR)/optee_client
export IMG_DIRECTORY ?= $(TEACLAVE_TOOLCHAIN_BASE)/images
export IMG_NAME ?= optee-qemu_v8.bin

include $(TEACLAVE_TOOLCHAIN_BASE)/config/ta/active

TA_UUID := 8aaaf200-2450-11e4-abe2-0002a5d5c51b
TA_BINARY := $(TA_UUID).ta

.PHONY: all ta host clean

all: ta host

ta:
	@echo "Building KMS TA..."
	cd ta && cargo build --release --target $(TARGET_TA)
	cp ta/target/$(TARGET_TA)/release/kms-ta $(TA_BINARY)

host:
	@echo "Building KMS Host..."
	cd host && cargo build --release

clean:
	cd ta && cargo clean
	cd host && cargo clean
	rm -f $(TA_BINARY)

install-ta: ta
	@echo "Installing TA to QEMU shared directory..."
	mkdir -p $(QEMU_HOST_SHARE_DIR)/ta
	cp $(TA_BINARY) $(QEMU_HOST_SHARE_DIR)/ta/

install-host: host
	@echo "Installing Host to QEMU shared directory..."
	mkdir -p $(QEMU_HOST_SHARE_DIR)/host
	cp host/target/release/kms-host $(QEMU_HOST_SHARE_DIR)/host/

install: install-ta install-host

test: install
	@echo "Testing KMS in OP-TEE..."
	$(QEMU_HOST_SHARE_DIR)/host/kms-host