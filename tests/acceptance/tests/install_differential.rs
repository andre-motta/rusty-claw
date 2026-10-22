//! The differential gate: the game played through its own frame loop
//! against reference traces of the same routes, in the same trace format,
//! on your own install (skipped without `CLAW_INSTALL` or
//! `CLAW_REFERENCE`).
//!
//! For every `[[compare]]` entry of
//! `tests/acceptance/tolerances/references.toml`, the store's route is
//! replayed through the game's [`claw_game::Session`] fed by a simulated
//! 60 Hz display, with the clock's tuning, the random state at the route's
//! time zero and, for a segment, Claw placed at the store's start point
//! (see `claw_acceptance::reference`). It is compared with every reference
//! trace of the clock at the gate's preset: `c18` runs tick for tick with
//! `exact.toml`, `c60` runs for feel with `feel.toml`. An entry that names
//! another tolerance file fails.
//!
//! The test fails on the first divergence `references.toml` does not list
//! for that route and clock (new, or earlier than listed), printing it with
//! the rows of both traces leading up to it, and when the game's trace ends
//! before the reference trace. Known divergences that come later or go
//! away are reported so the entry can be tightened. Full-level routes at
//! `c60` are compared up to their known divergences and need not finish the
//! level: frames of 16 and 17 ms against ticks of 18 ms move a close
//! landing by a tick, and a route played without looking at the game can
//! go another way from there.
//!
//! The same route played tick by tick, as `make compare` and
//! `install_reference` play it, must give exactly the same trace: the
//! frame loop adds nothing of its own to the game. Each reference trace
//! is compared with its repeat's own copy of the route and under its
//! repeat's hermit crab values when the store carries them, with either
//! driver.

use std::collections::BTreeMap;
use std::path::Path;

use claw_acceptance::install;
use claw_acceptance::reference::{
    Driver, Store, load, load_entries, reference_root, summary_line, tolerances, unlisted_report,
    verdict,
};
use claw_game::LevelBundle;

/// The level bundles loaded so far, by level number.
type Levels = BTreeMap<u32, LevelBundle>;

fn level<'a>(levels: &'a mut Levels, install: &Path, n: u32) -> &'a LevelBundle {
    levels
        .entry(n)
        .or_insert_with(|| load(install, n).unwrap_or_else(|e| panic!("{e}")))
}

#[test]
fn routes_follow_the_reference_traces_through_the_frame_loop() {
    let Some(install) = install() else { return };
    let Some(root) = reference_root() else { return };
    let entries = load_entries().unwrap_or_else(|e| panic!("{e}"));
    assert!(!entries.is_empty(), "references.toml lists no route");
    let mut levels = Levels::new();
    let mut failures = Vec::new();
    for entry in &entries {
        let label = format!("{} {}", entry.reference, entry.clock.name());
        let gate = entry.clock.default_tolerance();
        if entry.tolerance.trim_end_matches(".toml") != gate {
            failures.push(format!(
                "{label}: references.toml compares it with {}, the gate compares {} at {gate}",
                entry.tolerance,
                entry.clock.name()
            ));
            continue;
        }
        let store = Store::open(&root, &entry.reference).unwrap_or_else(|e| panic!("{e}"));
        let Some(run) = store.run(entry.clock) else {
            failures.push(format!("{label}: the store holds no run at this clock"));
            continue;
        };
        let tol = tolerances(gate).unwrap_or_else(|e| panic!("{e}"));
        let bundle = level(&mut levels, &install, store.level);
        let result = store
            .compare_clock_with(bundle, run, &tol, Driver::SESSION_60)
            .unwrap_or_else(|e| panic!("{label}: {e}"));
        println!("{}", summary_line(&store, &result));
        let ticks = store
            .compare_clock(bundle, run, &tol)
            .unwrap_or_else(|e| panic!("{label}: {e}"));
        for (framed, ticked) in result.traces.iter().zip(&ticks.traces) {
            let (a, b) = (&framed.rebuild, &ticked.rebuild);
            if let Some(i) = (0..a.len().max(b.len())).find(|&i| a.get(i) != b.get(i)) {
                failures.push(format!(
                    "{label} ({}): the frame loop at 60 Hz and tick-by-tick play differ at tick {i}:\n    \
                     frame loop {:?}\n    tick by tick {:?}",
                    framed.trace.display(),
                    a.get(i),
                    b.get(i)
                ));
            }
        }
        for t in &result.traces {
            let name = t.trace.display();
            let v = verdict(entry, &t.comparison);
            for line in &v.improvements {
                println!("  {name}: {line}; references.toml can be tightened");
            }
            if v.holds() {
                continue;
            }
            let mut text = format!("{label} ({name}): {}", v.regressions.join("; "));
            if let Some(report) = unlisted_report(entry, t, &tol, &t.rebuild) {
                text.push_str("\n    the first divergence references.toml does not list:\n");
                for line in report.lines() {
                    text.push_str("      ");
                    text.push_str(line);
                    text.push('\n');
                }
            }
            failures.push(text);
        }
    }
    assert!(
        failures.is_empty(),
        "the game leaves the gate's tolerance beyond the known divergences:\n  {}",
        failures.join("\n  ")
    );
}
