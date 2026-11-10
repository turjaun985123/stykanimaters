//! Roadmap Wave 2 (W2-70, IA-RM24-4 slice C) — the census pin
//! that closes the undo-label migration.
//!
//! RM24 labelled every `EditorCore::snapshot` call site inside
//! `src/editor.rs`; slices A (W2-68), B (W2-69) and C (W2-70)
//! migrated the remaining files family by family, so every undo
//! step outside `editor.rs` now carries the label of the
//! operation that spent it (`snapshot_labeled` /
//! `snapshot_coalesced`) instead of the generic `"Edit"` the
//! bare `snapshot()` delegates to. This pin is the migration's
//! ratchet: a NEW bare `.snapshot()` call on an `EditorCore`
//! anywhere under `src/` (outside `editor.rs`) fails here, so
//! the generic label cannot silently come back. If a failure
//! below is deliberate — a new doc mention, a new exception
//! class — update the named exception tables in this file in
//! the same change, with the reason recorded.
//!
//! Scope: the migration's own census basis, `src/**/*.rs`
//! outside `src/editor.rs`. `editor.rs` is excluded because it
//! defines `snapshot()` itself and its call sites are RM24's
//! (its remaining bare tokens are `#[cfg(test)]` fixtures
//! inside its `mod tests`, past line 11480 — a remainder the
//! W2-70 findings flag for a future editor.rs seat).
//!
//! Named exceptions (everything else fails):
//!
//! 1. DOC PROSE — the token mentioned on a `//` comment line,
//!    describing the discipline rather than executing it.
//!    Censused per file in [`PROSE_CENSUS`]; any other file
//!    growing a doc mention, or a count moving either way,
//!    fails until the table is updated deliberately.
//! 2. PHASE PROGRESS — `src/rune_sec/export_panel.rs` carries a
//!    second, unrelated `snapshot()` method:
//!    `PhaseProgress::snapshot(&self) -> PhaseSnapshot`, the
//!    export jobs' progress report (`.seen` / `.done`). Its
//!    call sites are reads, not undo steps, and are exempt as
//!    a class — but only while their receiver is not an
//!    `EditorCore` receiver (`core` / `s.core` / `self`): a
//!    bare undo snapshot in that file still fails.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The doc-prose census: file (relative to `src/`) → number of
/// comment lines mentioning `.snapshot()`. See exception 1.
const PROSE_CENSUS: &[(&str, usize)] = &[
    ("draw_ops.rs", 1),
    ("rune_sec/clips_panel.rs", 1),
    ("rune_sec/graph_panel.rs", 2),
    ("rune_sec/stroke_inspector.rs", 8),
    ("shell_tools_selection.rs", 1),
];

/// The one file carrying the PhaseProgress exception class
/// (exception 2), relative to `src/`.
const PHASE_PROGRESS_FILE: &str = "rune_sec/export_panel.rs";

fn collect_rs(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("read {}: {e}", dir.display()))
        .map(|e| e.expect("dir entry").path())
        .collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            collect_rs(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

/// Does this line call `snapshot()` on an `EditorCore`
/// receiver (`core`, `s.core`, or `self`)?
fn is_core_snapshot_call(line: &str) -> bool {
    line.contains("core.snapshot()") || line.contains("self.snapshot()")
}

#[test]
fn no_bare_editor_core_snapshot_outside_the_named_exceptions() {
    let src = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/src"));
    let mut files = Vec::new();
    collect_rs(src, &mut files);
    assert!(
        files.len() >= 100,
        "scan found only {} .rs files under src/ — the walk is broken, not the tree clean",
        files.len()
    );

    let mut failures: Vec<String> = Vec::new();
    let mut prose_seen: BTreeMap<String, usize> = BTreeMap::new();
    let mut labelled = 0usize;
    let mut phase_reads = 0usize;

    for path in &files {
        let rel = path
            .strip_prefix(src)
            .expect("under src/")
            .to_string_lossy()
            .replace('\\', "/");
        if rel == "editor.rs" {
            continue; // the definition site; RM24's file (see header)
        }
        let text = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        for (i, line) in text.lines().enumerate() {
            if line.contains(".snapshot_labeled(") || line.contains(".snapshot_coalesced(") {
                labelled += 1;
            }
            if !line.contains(".snapshot()") {
                continue;
            }
            if line.trim_start().starts_with("//") {
                // Exception 1 candidate: doc prose. Censused below.
                *prose_seen.entry(rel.clone()).or_default() += 1;
                continue;
            }
            if rel == PHASE_PROGRESS_FILE && !is_core_snapshot_call(line) {
                // Exception 2: a PhaseProgress progress read.
                phase_reads += 1;
                continue;
            }
            failures.push(format!(
                "{rel}:{}: bare .snapshot() call — label it (snapshot_labeled) \
                 or record a named exception in tests/w2_undo_label_census.rs: {}",
                i + 1,
                line.trim()
            ));
        }
    }

    let prose_expected: BTreeMap<String, usize> = PROSE_CENSUS
        .iter()
        .map(|(f, n)| (f.to_string(), *n))
        .collect();
    assert_eq!(
        prose_seen, prose_expected,
        "doc-prose census moved — a `.snapshot()` mention was added to or removed \
         from a comment; update PROSE_CENSUS in tests/w2_undo_label_census.rs \
         deliberately if the change is intended"
    );
    assert!(
        phase_reads > 0,
        "no PhaseProgress reads found in {PHASE_PROGRESS_FILE} — the exception \
         class vanished or the scan is broken"
    );
    assert!(
        labelled >= 250,
        "only {labelled} labelled snapshot sites found — the scan is broken, \
         not the tree unlabelled"
    );
    assert!(
        failures.is_empty(),
        "bare EditorCore snapshot(s) outside the named exceptions:\n{}",
        failures.join("\n")
    );
}
