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
SOUNDFONT_URL ?= https://github.com/andre-motta/rusty-claw/releases/download/soundfont-generaluser-gs-2.0.3/GeneralUser-GS.sf2
SOUNDFONT_SHA256 := 9575028c7a1f589f5770fccc8cff2734566af40cd26ed836944e9a5152688cfe
SOUNDFONT_FILE := GeneralUser-GS.sf2
SOUNDFONT_DIR ?= $(or $(XDG_DATA_HOME),$(HOME)/.local/share)/rusty-claw/soundfonts

.PHONY: help check fmt test-install compare run-viewer run-game bench replay soundfont

help:
	@echo "make check                     format check, clippy and all unit tests"
	@echo "make check-<module>            unit tests of one module (assets, frame, render, sim, game, ...)"
	@echo "make test-install MODULE=<m>   tests against your own install (needs CLAW_INSTALL)"
	@echo "make compare REF=<level>/<name> [CLOCK=c18|c60] [STRICT=1]"
	@echo "                               replay a route of the reference store and compare it with"
	@echo "                               its reference traces (needs CLAW_INSTALL and CLAW_REFERENCE;"
	@echo "                               prints the first divergence and a per-column summary for each"
	@echo "                               trace; fails on a divergence that references.toml does not"
	@echo "                               list, or comes earlier than listed; STRICT=1 fails on any"
	@echo "                               divergence and on a game trace shorter than the reference;"
	@echo "                               a store's meta.toml teleport = \"X,Y\" is the start point;"
	@echo "                               tolerances in tests/acceptance/tolerances/; the tenth level's"
	@echo "                               boss stores replay through claw-replay from the store's time"
	@echo "                               zero, exact at c18 and feel at c60, failing on any divergence)"
	@echo "make run-viewer ARGS=...       run the level viewer"
	@echo "make run-game ARGS=...         run the game"
	@echo "make bench ARGS=...            run the game with --bench SECONDS in ARGS, or along a route"
	@echo "                               with --bench-route FILE.csv --headless"
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
	echo "$(CARGO) test --release --locked --manifest-path $$dir/Cargo.toml --tests"; \
	CLAW_INSTALL="$(CLAW_INSTALL)" $(CARGO) test --release --locked --manifest-path $$dir/Cargo.toml --tests

# Replays $(CLAW_REFERENCE)/$(REF)/route.csv with the tuning, random state
# and start point (teleport) its meta.toml names for each clock (or only
# CLOCK) and compares the game with every reference trace of that clock,
# each under the hermit crab values its repeat carries in the store
# (crabs-<clock>-r<k>.replay.txt, else crabs-<clock>-r<k>.txt, for the k-th
# trace the clock's traces list names; the route header's values without
# either). It fails on a divergence that tests/acceptance/tolerances/references.toml
# does not list for that route and clock (or that comes earlier than
# listed), and with STRICT=1 on any divergence and on a game trace that ends
# before the reference trace; see tests/acceptance/tests/install_reference.rs.
#
# The tenth level's boss stores (REF=level10/boss-...) were run with Claw
# placed at the store's start point on the level's first tick and left there
# until the route's time zero (t0_ms), with the random state there
# (rng_at_t0). They replay through claw-replay, which starts a store's
# segment that way (see tools/replay), once per reference trace of each
# clock (or only CLOCK): c18 with the original tuning at the exact
# tolerance, c60 with the default tuning at the feel tolerance. Any
# divergence fails, as with STRICT=1; the trace comparisons in
# engine/sim/tests/install_marrow.rs list the known ones.
COMPARE_FROM_T0 := level10/boss-%
compare:
	@if [ -z "$(CLAW_INSTALL)" ]; then echo "CLAW_INSTALL is not set; point it at your own install" >&2; exit 2; fi
	@if [ -z "$(CLAW_REFERENCE)" ]; then echo "CLAW_REFERENCE is not set; point it at the reference store" >&2; exit 2; fi
	@if [ -z "$(REF)" ]; then echo "REF is not set, for example REF=level1/opening" >&2; exit 2; fi
	@case "$(CLOCK)" in ""|c18|c60) ;; *) echo "CLOCK is c18 or c60" >&2; exit 2;; esac
	@case "$(STRICT)" in ""|0|1) ;; *) echo "STRICT is 0 or 1" >&2; exit 2;; esac
ifneq ($(filter $(COMPARE_FROM_T0),$(REF)),)
	@dir="$(CLAW_REFERENCE)/$(REF)"; \
	if [ ! -f "$$dir/meta.toml" ] || [ ! -f "$$dir/route.csv" ]; then echo "$$dir lacks its meta.toml or route.csv" >&2; exit 2; fi; \
	$(CARGO) build --release --locked -q -p claw-replay || exit 2; \
	n=0; status=0; \
	for t in "$$dir"/trace-*-r*.csv; do \
		[ -f "$$t" ] || continue; \
		clock=$$(basename "$$t" | sed 's/^trace-\([^-]*\)-.*/\1/'); \
		if [ -n "$(CLOCK)" ] && [ "$$clock" != "$(CLOCK)" ]; then continue; fi; \
		case "$$clock" in c18) tuning=original; tol=exact;; c60) tuning=default; tol=feel;; *) continue;; esac; \
		echo "== $(REF) $$clock: $$t (tuning $$tuning, tolerance $$tol)"; \
		$(CARGO) run --release --locked -q -p claw-replay -- --install "$(CLAW_INSTALL)" \
			--script "$$dir/route.csv" --tuning $$tuning --compare "$$t" --tolerance $$tol; \
		rc=$$?; if [ $$rc -gt $$status ]; then status=$$rc; fi; \
		n=$$((n + 1)); \
	done; \
	if [ $$n -eq 0 ]; then echo "$$dir holds no reference trace$(if $(CLOCK), for $(CLOCK))" >&2; exit 2; fi; \
	exit $$status
else
	CLAW_INSTALL="$(CLAW_INSTALL)" CLAW_REFERENCE="$(CLAW_REFERENCE)" CLAW_COMPARE_REF="$(REF)" CLAW_COMPARE_CLOCK="$(CLOCK)" CLAW_COMPARE_STRICT="$(STRICT)" \
		$(CARGO) test --release --locked -p claw-acceptance --test install_reference -- --exact make_compare --nocapture
endif

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
