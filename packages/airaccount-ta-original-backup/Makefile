# AirAccount TA Makefile

UUID ?= 11223344-5566-7788-99aa-bbccddeeff00

TARGET ?= aarch64-unknown-linux-gnu
CROSS_COMPILE ?= aarch64-linux-gnu-
OBJCOPY := $(CROSS_COMPILE)objcopy
LINKER_CFG := target.$(TARGET).linker=\"$(CROSS_COMPILE)gcc\"

TA_SIGN_KEY ?= ../../third_party/incubator-teaclave-trustzone-sdk/keys/default_ta.pem
SIGN := ../../third_party/incubator-teaclave-trustzone-sdk/scripts/sign_encrypt.py
OUT_DIR := $(CURDIR)/target/$(TARGET)/release

# STD version build (compatible with eth_wallet)
ifeq ($(STD),)
all:
	@echo "Please \`export STD=y\` then rerun to build the STD version"
	@echo "Or use: make STD=y"
else
all: ta strip sign
endif

ta:
	@echo "Building AirAccount TA..."
	@xargo build --target $(TARGET) --release --config $(LINKER_CFG)

strip: ta
	@$(OBJCOPY) --strip-unneeded $(OUT_DIR)/libairaccount_ta.rlib $(OUT_DIR)/stripped_ta

sign: strip
	@$(SIGN) --uuid $(UUID) --key $(TA_SIGN_KEY) --in $(OUT_DIR)/stripped_ta --out $(OUT_DIR)/$(UUID).ta
	@echo "SIGN =>  ${UUID}"

clean:
	@cargo clean

.PHONY: all ta strip sign clean