# Licensed to AirAccount under the Apache License, Version 2.0

TARGET ?= aarch64-unknown-linux-gnu
CROSS_COMPILE ?= aarch64-linux-gnu-

.PHONY: all
all: 
	@cargo build --target $(TARGET) --release
	@echo "Built CA: $(TARGET_HOST)"

.PHONY: clean
clean:
	@cargo clean

.PHONY: run
run:
	@cargo run --release