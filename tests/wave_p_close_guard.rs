//! Wave P, P19 — the window-close unsaved guard, pinned
//! on THIS tree (master = the Rune line).
//!
//! THE AUDIT (full table in wave-p/findings/p19.md): the
//! shell's one trusted dirty predicate is
//! `session_dirty(core) = core.edits_since_autosave() > 0`
//! (`rune_sec::project_tools::session_dirty`) — edits the
//! recovery snapshot has not captured. Every in-app
//! destructive action (New / Open / Reload / Recover, on
//! the project panel, Home, and the app bar's Load) reads
//! it through the `ConfirmAction` arm-first flow. The OS
//! window close read NOTHING: it never reaches app code
//! at all. The pinned Rune framework (git rev 87caf2a)
//! handles winit's `CloseRequested` inside
//! `window/events.rs` by closing unconditionally, and
//! `AppSpec` — the entry this app uses — exposes no
//! close hook: `App::new(spec)` hardcodes
//! `on_close: None`. Worse, the framework offers NO
//! close veto anywhere: its `close()` runs the `on_close`
//! hook, then closes regardless of what the hook did.
//! A "discard changes?" confirmation on window close is
//! therefore impossible on this framework, exactly the
//! bypass class the Tauri line found — with no dialog
//! fix available.
//!
//! THE FAIL-CLOSED SETTLEMENT: if the close cannot be
//! refused, it must lose NOTHING. The shell already
//! trusts the recovery snapshot (`.recovery.json` in
//! `./projects`) as its safety net — the dirty counter IS
//! "edits the snapshot has not captured", and the
//! startup offer (`shell::recovery_is_newer_than_last_
//! project`) already surfaces a snapshot newer than the
//! last save. But the autosave cadence only writes every
//! `AUTOSAVE_EVERY` (25) counted edits, so up to 24 edits
//! could sit outside the snapshot at close time.
//! `EditorCore::flush_recovery_on_close` (landed in
//! src/editor.rs by this task) closes that gap: while
//! the session is dirty by the shell's own predicate, it
//! writes the snapshot NOW; while clean it writes
//! nothing, so the snapshot's mtime keeps meaning
//! "unsaved work" to the startup offer instead of being
//! refreshed by every clean exit.
//!
//! WHAT IS PINNED HERE — the testable half:
//!   1. A clean flush writes nothing at all (no
//!      `projects/` directory appears, no snapshot is
//!      readable, the counter and status are untouched).
//!   2. A dirty flush — dirtied by a non-snapshot
//!      counted edit (`mark_dirty`, the Wave 13 B2 seam)
//!      and by a snapshot-taking edit alike — writes the
//!      CURRENT project to the recovery snapshot (the
//!      shell's own `read_recovery` returns it) and the
//!      session then reads clean.
//!   3. A failed flush (the projects path is blocked)
//!      is honest: the dirty counter is left standing
//!      and the status reports the failure — the dirty
//!      truth is never silently cleared.
//!   4. The routing pin: src/main.rs registers the
//!      flush on the framework's two exit hooks
//!      (`on_close` + `on_quit`). The hooks live in the
//!      integrator-owned src/main.rs; this pin is RED
//!      until Integrator Action IA-P19-1 (exact code in
//!      wave-p/findings/p19.md) is applied, and goes
//!      green with it. That is deliberate: the bypass
//!      must not be integrable-away in silence.
//!
//! WHAT NO TEST HERE CAN PROVE: that a real window
//! close fires the hook in a live session — that needs
//! a live window (the headless pass is P43's; the
//! scenario it would need is named in the findings).

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::shell::read_recovery;

/// Serialises this file's tests: they each change the
/// process current directory, which is process-global
/// (the wave_p_figlib idiom). Poison-tolerant.
static CLOSE_GUARD_CWD_LOCK: Mutex<()> = Mutex::new(());

fn lock_cwd() -> std::sync::MutexGuard<'static, ()> {
    CLOSE_GUARD_CWD_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "styk-wave-p-close-guard-{}-{tag}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

/// Restores the process current directory on drop,
/// including when the test panics.
struct CwdGuard(PathBuf);

impl CwdGuard {
    fn enter(dir: &Path) -> Self {
        let old = std::env::current_dir().expect("read current dir");
        std::env::set_current_dir(dir).expect("enter temp dir");
        CwdGuard(old)
    }
}

impl Drop for CwdGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.0);
    }
}

#[test]
fn clean_flush_writes_nothing() {
    let _lock = lock_cwd();
    let dir = temp_dir("clean");
    let _cwd = CwdGuard::enter(&dir);

    let mut core = EditorCore::new();
    assert_eq!(core.edits_since_autosave(), 0, "a fresh core is clean");
    let status_before = core.status.clone();

    core.flush_recovery_on_close();

    assert_eq!(
        core.edits_since_autosave(),
        0,
        "clean flush: the counter is untouched"
    );
    assert_eq!(
        core.status, status_before,
        "clean flush: silent — no status written"
    );
    assert!(
        !dir.join("projects").exists(),
        "clean flush: no projects directory is even created — the \
         snapshot's mtime must keep meaning 'unsaved work'"
    );
    assert!(
        read_recovery().is_none(),
        "clean flush: no recovery snapshot is readable"
    );
}

#[test]
fn dirty_flush_writes_current_project_and_reads_clean() {
    let _lock = lock_cwd();
    let dir = temp_dir("dirty");
    let _cwd = CwdGuard::enter(&dir);

    let mut core = EditorCore::new();
    core.project.name = "Close Guard Fixture".to_string();
    // One counted edit of each kind the counter sees: a
    // snapshot-taking edit and a mark_dirty edit.
    core.snapshot();
    core.mark_dirty();
    assert_eq!(
        core.edits_since_autosave(),
        2,
        "both edit kinds count toward the dirty truth"
    );

    core.flush_recovery_on_close();

    assert_eq!(
        core.edits_since_autosave(),
        0,
        "after the flush the session reads clean: the snapshot \
         now covers every counted edit"
    );
    let recovered = read_recovery().expect("the flush wrote a recovery snapshot");
    assert_eq!(
        recovered.name, "Close Guard Fixture",
        "the snapshot holds the CURRENT project, not a stale one"
    );

    // Idempotent: a second flush (the second exit hook
    // firing) writes nothing and changes nothing.
    core.flush_recovery_on_close();
    assert_eq!(core.edits_since_autosave(), 0);
}

#[test]
fn failed_flush_keeps_the_dirty_truth_and_reports() {
    let _lock = lock_cwd();
    let dir = temp_dir("blocked");
    let _cwd = CwdGuard::enter(&dir);
    // Block the snapshot write: `projects` exists as a
    // regular file, so the store cannot be created in it.
    fs::write(dir.join("projects"), b"not a directory").expect("block the projects path");

    let mut core = EditorCore::new();
    core.mark_dirty();
    assert_eq!(core.edits_since_autosave(), 1);

    core.flush_recovery_on_close();

    assert_eq!(
        core.edits_since_autosave(),
        1,
        "a failed flush must NOT clear the dirty counter — the \
         edit is still uncaptured and must still read dirty"
    );
    assert!(
        core.status.starts_with("Autosave failed"),
        "the failure is reported on the status line, not \
         swallowed: got {:?}",
        core.status
    );
}

#[test]
fn main_rs_registers_the_flush_on_both_exit_hooks() {
    // The routing half of the fix lives in the
    // integrator-owned src/main.rs (IA-P19-1). This pin
    // reads that file's text — the only way to pin a
    // wiring fact about a binary entry point — and is
    // RED until the integrator applies the action.
    let main_rs = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/main.rs");
    let text = fs::read_to_string(&main_rs).expect("read src/main.rs");
    assert!(
        text.contains("flush_recovery_on_close"),
        "src/main.rs must route the window-exit flush \
         (EditorCore::flush_recovery_on_close) — IA-P19-1"
    );
    assert!(
        text.contains(".on_close("),
        "the flush must be registered on the framework's on_close \
         hook (window close button / Tx::close_window) — IA-P19-1"
    );
    assert!(
        text.contains(".on_quit("),
        "the flush must ALSO be registered on on_quit (OS quit / \
         session end, where on_close may not run first) — IA-P19-1"
    );
}
