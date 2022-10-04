CRAFTING_INTERPRETERS ?= ../craftinginterpreters
DEBUG_BIN := target/debug/clox-rs

test_level := chap18_types
sources := src/*.rs Cargo.toml

$(DEBUG_BIN): $(sources)
	cargo build

.PHONY: cargo-test
cargo-test:
	cargo test

.PHONY: craftinginterpreters-test
craftinginterpreters-test: $(DEBUG_BIN)
	$(eval home := $(shell pwd))
	cd $(CRAFTING_INTERPRETERS) && \
		dart tool/bin/test.dart jlox $(test_level) --interpreter $(home)/$(DEBUG_BIN)

.PHONY: test
test: cargo-test craftinginterpreters-test
