//! Autosave policy and recovery-snapshot generations
//! (Roadmap Wave 2, W2-63 — PL-8 settings slice 2).
//!
//! The audit this module answers (see findings2/w2-63.md
//! for the full record): the app already autosaves —
//! `EditorCore::maybe_autosave` writes the recovery
//! snapshot (shell.rs's `.recovery.json`, atomic replace
//! in the AppDirs state dir) every 25 snapshot-counted
//! edits, `flush_recovery_on_close` covers the window
//! close, and the Home panel already offers the snapshot
//! at launch when it is newer than the last named save.
//! What did not exist before this seat: any user control
//! over that behaviour. The cadence was a hard-coded
//! const, the writer could not be turned off, and the
//! snapshot was a single file — every autosave destroyed
//! the previous copy, so one bad state (an accidental
//! mass delete, say) overwrote the only recovery point
//! within 25 edits.
//!
//! This module is the policy + machinery half of the
//! fix, pure and testable without the shell:
//!
//! - [`AutosavePrefs`] — the three persisted choices
//!   (on/off, cadence in edits, generations kept) with
//!   their sanitising clamps. The values live in the
//!   layout prefs (`LayoutPrefs.autosave_enabled` /
//!   `autosave_every` / `recovery_keep`, IA-W2-63-1);
//!   `EditorCore` mirrors the struct while running
//!   (IA-W2-63-2, seeded by the shell's `init`,
//!   IA-W2-63-3).
//! - [`write_recovery_keeping`] — the recovery writer
//!   the cadence now calls: before the live snapshot is
//!   replaced, the previous generations rotate
//!   (`.recovery.json` → `.recovery.1.json` →
//!   `.recovery.2.json` …), so `keep` counts the total
//!   snapshots retained, the live one included. With
//!   `keep == 1` the behaviour is exactly the historical
//!   single file.
//! - [`read_recovery_fallback_in`] — the read side: the
//!   live snapshot first, then the generations newest
//!   to oldest, returning the first that parses (the
//!   Home / project Recover wiring is IA-W2-63-5).
//!
//! Defaults are the historical behaviour in all three
//! choices (on, 25 edits, 1 generation): a prefs file
//! written before the fields existed behaves exactly
//! as every build before this seat did.

use std::io;
use std::path::{Path, PathBuf};

use crate::model::Project;

/// The cadence every build shipped before the
/// preference existed: one recovery write per 25
/// snapshot-counted edits (Wave 9's
/// `EditorCore::AUTOSAVE_EVERY`, whose value this
/// mirrors — the editor's const stays as the record of
/// the historical default its tests pin).
pub const DEFAULT_EVERY: u64 = 25;

/// The smallest cadence the preference accepts: below
/// this the writer would rewrite the whole project
/// JSON every few edits for no recovery benefit.
pub const MIN_EVERY: u64 = 5;

/// The largest cadence the preference accepts.
pub const MAX_EVERY: u64 = 500;

/// Generations retained before the preference existed:
/// the single live snapshot and nothing else.
pub const DEFAULT_KEEP: u32 = 1;

/// The smallest keep the preference accepts (the live
/// snapshot itself — rotation off, in effect).
pub const MIN_KEEP: u32 = 1;

/// The largest keep the preference accepts: ten full
/// project snapshots is already a generous safety net
/// for files of this size.
pub const MAX_KEEP: u32 = 10;

/// Upper bound on the generation files a rotation or
/// scan will touch. The keep ceiling is 10, so anything
/// past this is outside the contract by an order of
/// magnitude; the bound exists so a directory a hand
/// filled with lookalike names cannot make an autosave
/// scan forever.
const MAX_GENERATION_SCAN: u32 = 128;

/// The user's autosave policy, sanitised: whether the
/// recovery writer runs at all, how many
/// snapshot-counted edits accumulate between writes,
/// and how many snapshot generations are retained.
///
/// One struct rides every surface — the layout prefs
/// store its three fields (IA-W2-63-1), the Settings
/// card's sanitising reader rebuilds it from them, and
/// `EditorCore` holds it as its live policy
/// (IA-W2-63-2) — so the clamping rules live here,
/// once, and every reader sanitises identically.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AutosavePrefs {
    /// Whether autosave writes recovery snapshots. Off
    /// means NO recovery writes at all — neither the
    /// cadence nor the window-close flush (the flush
    /// honouring the switch is what makes the switch
    /// honest; existing snapshots are left on disk,
    /// never deleted by the toggle).
    pub enabled: bool,
    /// Snapshot-counted edits between autosaves
    /// ([`MIN_EVERY`]..=[`MAX_EVERY`] after sanitising).
    pub every: u64,
    /// Snapshot generations retained, the live
    /// `.recovery.json` included
    /// ([`MIN_KEEP`]..=[`MAX_KEEP`] after sanitising).
    pub keep: u32,
}

impl Default for AutosavePrefs {
    /// The historical behaviour in all three choices:
    /// on, every 25 edits, one generation.
    fn default() -> Self {
        AutosavePrefs {
            enabled: true,
            every: DEFAULT_EVERY,
            keep: DEFAULT_KEEP,
        }
    }
}

impl AutosavePrefs {
    /// Build the policy from the persisted fields,
    /// sanitising each: the cadence and the keep are
    /// clamped to their ranges (a hand-edited or corrupt
    /// prefs file can name any number; the writer only
    /// ever honours a sane one). The Settings card's
    /// reader and the shell's `init` seed both build
    /// through here.
    pub fn from_persisted(enabled: bool, every: u64, keep: u32) -> Self {
        AutosavePrefs {
            enabled,
            every: sanitize_every(every),
            keep: sanitize_keep(keep),
        }
    }

    /// The autosave decision the poll-driven cadence
    /// asks every tick: at least `every` counted edits
    /// have accumulated since the last successful write
    /// — and the writer is enabled at all. Saturating,
    /// like the counter arithmetic it reads.
    pub fn is_due(&self, edit_count: u64, last_autosave_count: u64) -> bool {
        self.enabled && edit_count.saturating_sub(last_autosave_count) >= self.every
    }
}

/// Sanitise a persisted cadence: clamped to
/// [`MIN_EVERY`]..=[`MAX_EVERY`]. (A stored 0 — a hand
/// edit's "every edit" — reads as [`MIN_EVERY`]: the
/// floor exists so a corrupt value cannot make the
/// writer rewrite the project on literally every edit.)
/// The shell's loader applies the same clamp on load
/// (IA-W2-63-1); the rule lives in both places and
/// both sides pin it, the lib/bin-wall pattern
/// IA-W2-62-1 established for the export directory.
pub fn sanitize_every(raw: u64) -> u64 {
    raw.clamp(MIN_EVERY, MAX_EVERY)
}

/// Sanitise a persisted keep: clamped to
/// [`MIN_KEEP`]..=[`MAX_KEEP`] (a stored 0 reads as 1 —
/// "keep no snapshot at all" is the enabled switch's
/// job, not the keep's).
pub fn sanitize_keep(raw: u32) -> u32 {
    raw.clamp(MIN_KEEP, MAX_KEEP)
}

/// The file name of generation `index` inside the
/// state dir: generation 0 is the live snapshot,
/// [`crate::shell::RECOVERY_FILE`] itself
/// (`.recovery.json`); rotated generations are
/// `.recovery.1.json`, `.recovery.2.json`, … — the
/// dotfile family stays together in the state dir and
/// the live name never changes, so every existing
/// reader (the Home offer gate, the status strip's
/// mtime segment) keeps reading the newest snapshot
/// exactly as before.
pub fn generation_name(index: u32) -> String {
    if index == 0 {
        crate::shell::RECOVERY_FILE.to_string()
    } else {
        format!(".recovery.{index}.json")
    }
}

/// Rotate the generations in `dir` ahead of a write,
/// retaining `keep` snapshots in total (the live one
/// included): the oldest retained generation is
/// deleted, each remaining generation moves one slot
/// older, and the live snapshot becomes generation 1.
/// Generations at or beyond the retained window —
/// including any a previously larger keep left behind
/// after the user lowered it — are removed, so a
/// lowered keep actually frees its files.
///
/// Best-effort by design: every step tolerates a
/// missing file (a fresh state dir has nothing to
/// rotate) and swallows other errors. Rotation is the
/// safety net's bookkeeping; it must never be the
/// reason the newest snapshot fails to write.
pub fn rotate_generations_in(dir: &Path, keep: u32) {
    let keep = sanitize_keep(keep);
    remove_generations_from(dir, keep);
    if keep <= 1 {
        return;
    }
    for n in (1..keep - 1).rev() {
        let _ = std::fs::rename(
            dir.join(generation_name(n)),
            dir.join(generation_name(n + 1)),
        );
    }
    let _ = std::fs::rename(
        dir.join(generation_name(0)),
        dir.join(generation_name(1)),
    );
}

/// Delete every generation file at index `first` or
/// beyond it, up to the scan bound. A missing file is
/// the common case and stops nothing — the scan is
/// over a bounded index range, not a chain.
fn remove_generations_from(dir: &Path, first: u32) {
    for n in first..=MAX_GENERATION_SCAN {
        let _ = std::fs::remove_file(dir.join(generation_name(n)));
    }
}

/// Write `project` as the recovery snapshot in `dir`,
/// rotating the retained generations first (see
/// [`rotate_generations_in`]): the write the autosave
/// cadence and the close flush make once the policy
/// lands (IA-W2-63-2). The write itself is
/// [`crate::shell::write_recovery_in`] — the same
/// atomic replace every recovery write has always
/// used — so a crash during the write still cannot
/// truncate the newest snapshot; the rotation ahead of
/// it means the previous snapshot survives at
/// generation 1 even so.
pub fn write_recovery_keeping_in(
    dir: &Path,
    project: &Project,
    keep: u32,
) -> io::Result<PathBuf> {
    rotate_generations_in(dir, keep);
    crate::shell::write_recovery_in(dir, project)
}

/// [`write_recovery_keeping_in`] in the shell's state
/// dir — the form the editor's recovery write calls.
pub fn write_recovery_keeping(project: &Project, keep: u32) -> io::Result<PathBuf> {
    write_recovery_keeping_in(&crate::shell::state_dir(), project, keep)
}

/// How many snapshot generations are present in `dir`,
/// counted consecutively from the live snapshot: 0
/// when no snapshot exists at all, otherwise 1 + the
/// rotated generations behind it. The Settings card's
/// status line reads this so the copies it reports are
/// the copies on disk.
pub fn generations_present_in(dir: &Path) -> u32 {
    let mut n = 0;
    while n <= MAX_GENERATION_SCAN && dir.join(generation_name(n)).exists() {
        n += 1;
    }
    n
}

/// [`generations_present_in`] in the shell's state dir.
pub fn generations_present() -> u32 {
    generations_present_in(&crate::shell::state_dir())
}

/// Read the newest RECOVERABLE snapshot in `dir`: the
/// live snapshot when it parses (the overwhelmingly
/// common case — exactly [`crate::shell::read_recovery_in`]),
/// otherwise the rotated generations newest to oldest,
/// returning the first that parses. The fallback exists
/// for the one failure the rotation makes recoverable:
/// a live snapshot corrupted after it was written
/// (a truncated copy, a disk error) no longer costs
/// the user the whole safety net while an intact
/// generation sits beside it. `None` when no generation
/// exists or none parses — the same answer the live-only
/// reader gives for a missing file.
pub fn read_recovery_fallback_in(dir: &Path) -> Option<Project> {
    if let Some(project) = crate::shell::read_recovery_in(dir) {
        return Some(project);
    }
    let mut n = 1;
    while n <= MAX_GENERATION_SCAN {
        let path = dir.join(generation_name(n));
        if !path.exists() {
            break;
        }
        if let Ok(text) = std::fs::read_to_string(&path) {
            if let Ok(project) = serde_json::from_str(&text) {
                return Some(project);
            }
        }
        n += 1;
    }
    None
}

/// [`read_recovery_fallback_in`] in the shell's state
/// dir — the form the Recover actions read through
/// once IA-W2-63-5 lands.
pub fn read_recovery_fallback() -> Option<Project> {
    read_recovery_fallback_in(&crate::shell::state_dir())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(test: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "styk-autosave-test-{}-{test}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A project whose identity a test can tell apart:
    /// the demo project with its stage width set to a
    /// marker value (the stage size round-trips through
    /// the snapshot JSON like any other field).
    fn marked_project(marker: f32) -> Project {
        let mut project = Project::new_demo();
        project.stage = [marker, marker];
        project
    }

    fn read_generation(dir: &Path, index: u32) -> Option<Project> {
        let text = std::fs::read_to_string(dir.join(generation_name(index))).ok()?;
        serde_json::from_str(&text).ok()
    }

    #[test]
    fn defaults_are_the_historical_behaviour() {
        let prefs = AutosavePrefs::default();
        assert!(prefs.enabled);
        assert_eq!(prefs.every, 25);
        assert_eq!(prefs.keep, 1);
        assert_eq!(
            AutosavePrefs::from_persisted(true, 25, 1),
            AutosavePrefs::default()
        );
    }

    #[test]
    fn persisted_values_are_sanitised() {
        assert_eq!(sanitize_every(0), MIN_EVERY);
        assert_eq!(sanitize_every(4), MIN_EVERY);
        assert_eq!(sanitize_every(25), 25);
        assert_eq!(sanitize_every(u64::MAX), MAX_EVERY);
        assert_eq!(sanitize_keep(0), 1);
        assert_eq!(sanitize_keep(3), 3);
        assert_eq!(sanitize_keep(u32::MAX), MAX_KEEP);
        let prefs = AutosavePrefs::from_persisted(false, 1, 999);
        assert!(!prefs.enabled);
        assert_eq!(prefs.every, MIN_EVERY);
        assert_eq!(prefs.keep, MAX_KEEP);
    }

    #[test]
    fn due_needs_enabled_and_the_full_cadence() {
        let prefs = AutosavePrefs::from_persisted(true, 25, 1);
        assert!(!prefs.is_due(0, 0));
        assert!(!prefs.is_due(24, 0));
        assert!(prefs.is_due(25, 0));
        assert!(prefs.is_due(50, 25));
        assert!(!prefs.is_due(49, 25));
        // A counter that went backwards (a project swap
        // restarting the baseline) saturates, never
        // wraps into a spurious due.
        assert!(!prefs.is_due(0, 25));
        let off = AutosavePrefs::from_persisted(false, 25, 1);
        assert!(!off.is_due(10_000, 0), "off is never due");
    }

    #[test]
    fn generation_names_keep_the_live_name() {
        assert_eq!(generation_name(0), crate::shell::RECOVERY_FILE);
        assert_eq!(generation_name(0), ".recovery.json");
        assert_eq!(generation_name(1), ".recovery.1.json");
        assert_eq!(generation_name(9), ".recovery.9.json");
    }

    #[test]
    fn keep_one_writes_exactly_the_historical_file() {
        let dir = temp_dir("keep-one");
        write_recovery_keeping_in(&dir, &marked_project(101.0), 1).unwrap();
        write_recovery_keeping_in(&dir, &marked_project(102.0), 1).unwrap();
        assert_eq!(generations_present_in(&dir), 1);
        assert!(!dir.join(generation_name(1)).exists());
        let live = read_generation(&dir, 0).expect("live snapshot parses");
        assert_eq!(live.stage, [102.0, 102.0], "the newest write wins");
    }

    #[test]
    fn rotation_retains_the_newest_generations_in_order() {
        let dir = temp_dir("rotate");
        for marker in [201.0, 202.0, 203.0, 204.0] {
            write_recovery_keeping_in(&dir, &marked_project(marker), 3).unwrap();
        }
        assert_eq!(generations_present_in(&dir), 3);
        assert_eq!(read_generation(&dir, 0).unwrap().stage, [204.0, 204.0]);
        assert_eq!(read_generation(&dir, 1).unwrap().stage, [203.0, 203.0]);
        assert_eq!(read_generation(&dir, 2).unwrap().stage, [202.0, 202.0]);
        assert!(
            !dir.join(generation_name(3)).exists(),
            "the oldest generation (201) rotated out"
        );
    }

    #[test]
    fn lowering_the_keep_frees_the_extra_generations() {
        let dir = temp_dir("lower-keep");
        for marker in [301.0, 302.0, 303.0] {
            write_recovery_keeping_in(&dir, &marked_project(marker), 3).unwrap();
        }
        assert_eq!(generations_present_in(&dir), 3);
        write_recovery_keeping_in(&dir, &marked_project(304.0), 1).unwrap();
        assert_eq!(generations_present_in(&dir), 1);
        assert_eq!(read_generation(&dir, 0).unwrap().stage, [304.0, 304.0]);
    }

    #[test]
    fn fallback_reads_a_generation_when_the_live_file_is_corrupt() {
        let dir = temp_dir("fallback");
        write_recovery_keeping_in(&dir, &marked_project(401.0), 2).unwrap();
        write_recovery_keeping_in(&dir, &marked_project(402.0), 2).unwrap();
        // The live-only reader sees the newest snapshot.
        assert_eq!(
            read_recovery_fallback_in(&dir).unwrap().stage,
            [402.0, 402.0]
        );
        // Corrupt the live file: the live-only reader
        // gives up, the fallback returns generation 1.
        std::fs::write(dir.join(generation_name(0)), "{ not json").unwrap();
        assert!(crate::shell::read_recovery_in(&dir).is_none());
        assert_eq!(
            read_recovery_fallback_in(&dir).unwrap().stage,
            [401.0, 401.0]
        );
        // Nothing on disk at all: no snapshot.
        let empty = temp_dir("fallback-empty");
        assert!(read_recovery_fallback_in(&empty).is_none());
        assert_eq!(generations_present_in(&empty), 0);
    }
}
