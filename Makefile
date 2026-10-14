# Stack-neutral entry points. Module targets dispatch by directory: a module
# name maps to the first of engine/<m>, app/<m>, tools/<m> and tests/<m> that
# holds a Cargo.toml, so new modules need no change here.

CARGO ?= cargo
MODULE_ROOTS := engine app tools tests

module_dir = $(firstword $(foreach root,$(MODULE_ROOTS),$(if $(wildcard $(root)/$(1)/Cargo.toml),$(root)/$(1))))

.PHONY: help check fmt test-install run-viewer run-game bench replay

help:
	@echo "make check                     format check, clippy and all unit tests"
	@echo "make check-<module>            unit tests of one module (assets, frame, render, sim, game, ...)"
	@echo "make test-install MODULE=<m>   tests against your own install (needs CLAW_INSTALL)"
	@echo "make run-viewer ARGS=...       run the level viewer"
	@echo "make run-game ARGS=...         run the game"
	@echo "make bench ARGS=...            run the game with --bench SECONDS in ARGS"
	@echo "make replay ARGS=...           run the replay tool"

check:
	$(CARGO) fmt --all --check
	$(CARGO) clippy --workspace --all-targets --locked -- -D warnings
	$(CARGO) test --workspace --locked

fmt:
	$(CARGO) fmt --all

check-%:
	@dir="$(call module_dir,$*)"; \
	if [ -z "$$dir" ]; then echo "unknown module '$*'" >&2; exit 2; fi; \
	echo "$(CARGO) test --locked --manifest-path $$dir/Cargo.toml"; \
	$(CARGO) test --locked --manifest-path $$dir/Cargo.toml

test-install:
	@if [ -z "$(CLAW_INSTALL)" ]; then echo "CLAW_INSTALL is not set; point it at your own install" >&2; exit 2; fi
	@if [ -z "$(MODULE)" ]; then echo "MODULE is not set, for example MODULE=assets" >&2; exit 2; fi
	@dir="$(call module_dir,$(MODULE))"; \
	if [ -z "$$dir" ]; then echo "unknown module '$(MODULE)'" >&2; exit 2; fi; \
	if ! ls $$dir/tests/install_*.rs >/dev/null 2>&1; then echo "module '$(MODULE)' has no install tests" >&2; exit 2; fi; \
	echo "$(CARGO) test --release --locked --manifest-path $$dir/Cargo.toml --test 'install_*'"; \
	CLAW_INSTALL="$(CLAW_INSTALL)" $(CARGO) test --release --locked --manifest-path $$dir/Cargo.toml --test 'install_*'

run-viewer:
	$(CARGO) run --release --locked -p claw-viewer -- $(ARGS)

run-game:
	$(CARGO) run --release --locked -p claw-game --bin rusty-claw -- $(ARGS)

bench:
	$(CARGO) run --release --locked -p claw-game --bin rusty-claw -- $(ARGS)

replay:
	$(CARGO) run --release --locked -p claw-replay -- $(ARGS)
