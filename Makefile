CRAFTING_INTERPRETERS ?= ../craftinginterpreters
DEBUG_BIN := target/debug/clox-rs

test_level := chap27_classes
sources := src/*.rs src/compiler/*.rs Cargo.toml

$(DEBUG_BIN): $(sources)
	cargo build

.PHONY: cargo-test
cargo-test:
	cargo test

.PHONY: craftinginterpreters-test
craftinginterpreters-test: $(DEBUG_BIN)
	$(eval home := $(shell pwd))
	cd $(CRAFTING_INTERPRETERS) && \
		dart tool/bin/test.dart $(test_level) --interpreter $(home)/$(DEBUG_BIN) --arguments --std

.PHONY: craftinginterpreters-test-stress-gc
craftinginterpreters-test-stress-gc: $(DEBUG_BIN)
	$(eval home := $(shell pwd))
	cd $(CRAFTING_INTERPRETERS) && \
		dart tool/bin/test.dart $(test_level) --interpreter $(home)/$(DEBUG_BIN) --arguments --std --arguments --stress-gc

.PHONY: craftinginterpreters-test-both
craftinginterpreters-test-both: craftinginterpreters-test craftinginterpreters-test-stress-gc

.PHONY: custom-dart-test
custom-dart-test: $(DEBUG_BIN)
	dart $(CRAFTING_INTERPRETERS)/tool/bin/test.dart clox --interpreter $(DEBUG_BIN) --arguments --stress-gc
	dart $(CRAFTING_INTERPRETERS)/tool/bin/test.dart clox --interpreter $(DEBUG_BIN)

.PHONY: test
test: cargo-test craftinginterpreters-test custom-dart-test
