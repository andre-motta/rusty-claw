//! The game against reference traces of the same routes, in the same trace
//! format, on your own install (skipped without `CLAW_INSTALL` or
//! `CLAW_REFERENCE`).
//!
//! Every `[[compare]]` entry of `tests/acceptance/tolerances/references.toml`
//! names a route in the reference store, a clock and a tolerance file. The
//! route is replayed in the game with the clock's tuning and the random
//! state at the route's time zero (see `claw_acceptance::reference`), and
//! compared with each reference trace of that clock. Every compared column
//! must stay within its tolerance up to the first divergence the entry
//! records for it as known (`known.<column> = <ms>`), and throughout when it
//! records none: a divergence that is new or comes earlier fails the test.
//! A known divergence that comes later or goes away is reported, so the
//! entry can be tightened.
//!
//! `make compare REF=<level>/<name> [CLOCK=c18|c60] [STRICT=1]` runs
//! [`make_compare`]: one route's comparison at each clock its store holds,
//! printed in full. It fails on a divergence that `references.toml` does not
//! list for that route and clock, or that comes earlier than listed; a route
//! and clock with no entry fails on any divergence, and so does every route
//! with `STRICT=1`.

use std::collections::BTreeMap;
use std::path::Path;

use claw_acceptance::install;
use claw_acceptance::reference::{
    self, Clock, ClockResult, Entry, Store, first_outside_rng, load, load_entries, reference_root,
    summary_line, tolerance_dir, tolerances, verdict,
};
use claw_game::LevelBundle;

/// The level bundles loaded so far, by level number.
type Levels = BTreeMap<u32, LevelBundle>;

fn level<'a>(levels: &'a mut Levels, install: &Path, n: u32) -> &'a LevelBundle {
    levels
        .entry(n)
        .or_insert_with(|| load(install, n).unwrap_or_else(|e| panic!("{e}")))
}

fn known_line(entry: &Entry) -> String {
    if entry.known.is_empty() {
        return "references.toml records no known divergence".to_string();
    }
    let list: Vec<String> = entry
        .known
        .iter()
        .map(|(c, ms)| format!("{c} at {ms} ms"))
        .collect();
    format!(
        "references.toml records these first divergences as known: {}",
        list.join(", ")
    )
}

#[test]
fn references_hold_their_known_divergences() {
    let Some(install) = install() else { return };
    let Some(root) = reference_root() else { return };
    let entries = load_entries().unwrap_or_else(|e| panic!("{e}"));
    let mut levels = Levels::new();
    let mut failures = Vec::new();
    for entry in &entries {
        let store = Store::open(&root, &entry.reference).unwrap_or_else(|e| panic!("{e}"));
        let run = store
            .run(entry.clock)
            .unwrap_or_else(|| panic!("{} holds no {} run", entry.reference, entry.clock.name()));
        let tol = tolerances(&entry.tolerance).unwrap_or_else(|e| panic!("{e}"));
        let bundle = level(&mut levels, &install, store.level);
        let result = store
            .compare_clock(bundle, run, &tol)
            .unwrap_or_else(|e| panic!("{}: {e}", entry.reference));
        println!("{}", summary_line(&store, &result));
        for t in &result.traces {
            let v = verdict(entry, &t.comparison);
            let name = t.trace.display();
            for line in &v.improvements {
                println!("  {name}: {line}; references.toml can be tightened");
            }
            for line in &v.regressions {
                failures.push(format!(
                    "{} {} ({name}): {line}",
                    entry.reference,
                    entry.clock.name()
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "divergences beyond the known ones:\n  {}",
        failures.join("\n  ")
    );
}

/// Prints one clock's comparison in full.
fn print_clock(store: &Store, result: &ClockResult, tolerance: &str, entry: Option<&Entry>) {
    let run = &result.run;
    println!(
        "\n== {} {}: level {}, route {}, tuning {}, seed {}, tolerance {}, {} reference trace(s)",
        store.name,
        run.clock.name(),
        store.level,
        store.route.display(),
        run.tuning.name(),
        result.seed,
        tolerance_dir().join(format!("{tolerance}.toml")).display(),
        result.traces.len()
    );
    for t in &result.traces {
        println!("\n-- {}", t.trace.display());
        print!("{}", t.report);
        if t.comparison
            .first_divergence
            .as_ref()
            .is_some_and(|d| d.column == "rng")
            && let Some((ms, column)) = first_outside_rng(&t.comparison)
        {
            println!("\nfirst divergence outside the random stream: {ms} ms in {column}");
        }
    }
    if let Some(entry) = entry {
        println!("\n{}", known_line(entry));
        for t in &result.traces {
            let v = verdict(entry, &t.comparison);
            for line in v.regressions.iter().chain(&v.improvements) {
                println!("  {}: {line}", t.trace.display());
            }
        }
    }
}

/// One route's comparison, for `make compare`: runs only when
/// `CLAW_COMPARE_REF` names a route (`CLAW_COMPARE_CLOCK` may pick one
/// clock), prints every report and fails on a divergence that the route's
/// `references.toml` entry does not list (or lists later). With no entry,
/// or with `CLAW_COMPARE_STRICT=1`, it fails on any divergence.
#[test]
fn make_compare() {
    let Some(name) = std::env::var("CLAW_COMPARE_REF")
        .ok()
        .filter(|n| !n.is_empty())
    else {
        eprintln!("skipped: CLAW_COMPARE_REF is not set (run `make compare REF=...`)");
        return;
    };
    let Some(install) = install() else {
        panic!("CLAW_INSTALL is not set; point it at your own install");
    };
    let Some(root) = reference_root() else {
        panic!("CLAW_REFERENCE is not set; point it at the reference store");
    };
    let clock = match std::env::var("CLAW_COMPARE_CLOCK") {
        Ok(c) if !c.is_empty() => Some(
            Clock::from_name(&c)
                .unwrap_or_else(|| panic!("unknown clock {c:?}, expected c18 or c60")),
        ),
        _ => None,
    };
    let strict = match std::env::var("CLAW_COMPARE_STRICT") {
        Ok(s) => match s.as_str() {
            "" | "0" => false,
            "1" => true,
            _ => panic!("CLAW_COMPARE_STRICT is 0 or 1, not {s:?}"),
        },
        Err(_) => false,
    };
    let entries = load_entries().unwrap_or_else(|e| panic!("{e}"));
    let store = Store::open(&root, &name).unwrap_or_else(|e| panic!("{e}"));
    let runs: Vec<_> = store
        .runs
        .iter()
        .filter(|r| clock.is_none_or(|c| c == r.clock))
        .collect();
    assert!(
        !runs.is_empty(),
        "{name} holds no {} run",
        clock.map_or("", Clock::name)
    );
    let bundle = load(&install, store.level).unwrap_or_else(|e| panic!("{e}"));
    let mut summaries = Vec::new();
    let mut failures = Vec::new();
    for run in runs {
        let entry = entries
            .iter()
            .find(|e| e.reference == name && e.clock == run.clock);
        let tolerance = entry.map_or_else(
            || run.clock.default_tolerance().to_string(),
            |e| e.tolerance.clone(),
        );
        let tol = reference::tolerances(&tolerance).unwrap_or_else(|e| panic!("{e}"));
        let result = store
            .compare_clock(&bundle, run, &tol)
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        print_clock(&store, &result, tolerance.trim_end_matches(".toml"), entry);
        for t in &result.traces {
            let lines = match entry {
                Some(entry) if !strict => verdict(entry, &t.comparison).regressions,
                _ if t.comparison.passed() => Vec::new(),
                _ => vec![t.comparison.first_divergence.as_ref().map_or_else(
                    || "nothing was compared".to_string(),
                    |d| {
                        format!(
                            "first leaves the tolerance at {} ms in {}",
                            d.t_ms, d.column
                        )
                    },
                )],
            };
            for line in lines {
                failures.push(format!(
                    "{} {} ({}): {line}",
                    name,
                    run.clock.name(),
                    t.trace.display()
                ));
            }
        }
        summaries.push(summary_line(&store, &result));
    }
    println!("\n{}", summaries.join("\n"));
    let rule = if strict {
        "STRICT=1: any divergence fails"
    } else {
        "divergences listed in references.toml are known; any other fails"
    };
    if failures.is_empty() {
        println!("\n{name}: no divergence beyond the known ones ({rule})");
    }
    assert!(
        failures.is_empty(),
        "the game leaves the tolerance of a reference trace ({rule}):\n  {}",
        failures.join("\n  ")
    );
}
