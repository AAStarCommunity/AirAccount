# AirAccount Basic Hello World Makefile - following eth_wallet pattern

BINARY = airaccout-basic

.PHONY: all
all: ta ca

.PHONY: ta  
ta:
	$(MAKE) -C ../ta-basic

.PHONY: ca
ca:
	$(MAKE) -C ../ca-basic

.PHONY: clean
clean:
	$(MAKE) -C ../ta-basic clean
	$(MAKE) -C ../ca-basic clean

.PHONY: test
test: all
	@echo "Running basic TA-CA communication test"
	cd ../ca-basic && ./target/aarch64-unknown-linux-gnu/release/airaccout-ca-basic test