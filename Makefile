# Stack-neutral entry points. Module targets dispatch by directory: a module
# name maps to the first of engine/<m>, app/<m>, tools/<m> and tests/<m> that
# holds a Cargo.toml, so new modules need no change here.

CARGO ?= cargo
MODULE_ROOTS := engine app tools tests

module_dir = $(firstword $(foreach root,$(MODULE_ROOTS),$(if $(wildcard $(root)/$(1)/Cargo.toml),$(root)/$(1))))

# The default General MIDI SoundFont for music: GeneralUser GS 2.0.3 by
# S. Christian Collins (https://www.schristiancollins.com/generaluser), under
# its own permissive licence. It is never committed and never fetched by
# cargo; `make soundfont` downloads this pinned file into the user data
# directory, where the game finds it, and checks its SHA-256.
SOUNDFONT_URL ?= https://raw.githubusercontent.com/mrbumpy409/GeneralUser-GS/684543d5e5efaef08d02be50dcda8d552478fa60/GeneralUser-GS.sf2
SOUNDFONT_SHA256 := 9575028c7a1f589f5770fccc8cff2734566af40cd26ed836944e9a5152688cfe
SOUNDFONT_FILE := GeneralUser-GS.sf2
SOUNDFONT_DIR ?= $(or $(XDG_DATA_HOME),$(HOME)/.local/share)/rusty-claw/soundfonts

.PHONY: help check fmt test-install run-viewer run-game bench replay soundfont

help:
	@echo "make check                     format check, clippy and all unit tests"
	@echo "make check-<module>            unit tests of one module (assets, frame, render, sim, game, ...)"
	@echo "make test-install MODULE=<m>   tests against your own install (needs CLAW_INSTALL)"
	@echo "make run-viewer ARGS=...       run the level viewer"
	@echo "make run-game ARGS=...         run the game"
	@echo "make bench ARGS=...            run the game with --bench SECONDS in ARGS"
	@echo "make replay ARGS=...           run the replay tool"
	@echo "make soundfont                 fetch the music SoundFont into your data directory"

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

soundfont:
	@dest="$(SOUNDFONT_DIR)/$(SOUNDFONT_FILE)"; \
	if [ -f "$$dest" ] && echo "$(SOUNDFONT_SHA256)  $$dest" | sha256sum -c --status; then \
		echo "$$dest is already in place"; exit 0; \
	fi; \
	mkdir -p "$(SOUNDFONT_DIR)" || exit 1; \
	part="$$dest.part"; \
	echo "fetching $(SOUNDFONT_URL)"; \
	if ! curl -fL --retry 2 -o "$$part" "$(SOUNDFONT_URL)"; then rm -f "$$part"; exit 1; fi; \
	if ! echo "$(SOUNDFONT_SHA256)  $$part" | sha256sum -c --status; then \
		rm -f "$$part"; echo "the download does not match SHA-256 $(SOUNDFONT_SHA256); nothing was installed" >&2; exit 1; \
	fi; \
	mv "$$part" "$$dest" && echo "installed $$dest"
