//! Shell tools — theme + settings (Wave 14, B7).
//!
//! The MCP-facing half of the Settings card
//! (`rune_sec::settings_popover`): three tools —
//! `shell_set_theme`, `shell_get_settings`,
//! `shell_set_setting` — whose executors run against
//! the GUI shell's `State`, not against `EditorCore`
//! alone. That is why this file lives in the BIN crate
//! beside `live_mcp` (the integrator declares
//! `mod shell_tools_settings;` in `main.rs` next to
//! `mod live_mcp;`): the settings it drives are split
//! across `State` fields, the core and the layout
//! prefs, and only shell-side code can see all three.
//!
//! ## How these plug in (as landed)
//!
//! The registry's handler type
//! (`mcp::Tool.handler`: `fn(&mut EditorCore, &Value)`)
//! cannot reach `State`, so on the live endpoint these
//! tools are dispatched BY NAME from
//! `crate::shell_control`'s drain straight to the
//! executors in this file:
//!
//! * `shell_set_theme` → [`exec_shell_set_theme`]
//! * `shell_get_settings` → [`exec_shell_get_settings`]
//! * `shell_set_setting` → [`exec_shell_set_setting`]
//!   (the raw args travel with the call; ALL parsing
//!   and validation lives in this file, so the refusal
//!   wording has exactly one home)
//!
//! In the registry the same three names carry the
//! shared `shell_unavailable` stub — the one canonical
//! live-editor refusal: the stdio server has no GUI
//! shell to drive, and a tool that silently did
//! nothing there would be a fake. (This file's first
//! draft specced its own refusal const; the landed
//! design has ONE wording for the whole shell family,
//! stamped by the registry's macro.)
//!
//! ## The settings census (from `settings_popover.rs`)
//!
//! Every control the card exposes, and where its
//! truth lives:
//!
//! | Setting | State | Persistence |
//! |---|---|---|
//! | Theme (dark/light) | `EditorCore.light_theme` (`editor.rs:69`) | `LayoutPrefs.dark = !light_theme`, written by `poll`'s write-back (`main.rs`) on change — the popover buttons persist exactly this way (they write the field and let `poll` save), and so does [`exec_shell_set_theme`] |
//! | Live MCP control | the bridge itself, `live_mcp::is_enabled()` | `LayoutPrefs.live_mcp`, load-modify-save of the RESULTING state (the popover's `toggle_live_mcp` discipline) — written by [`apply_pending_bridge_actions`], see the deferred-stop section below |
//! | Language | `EditorCore.lang` (`editor.rs:88`) | `LayoutPrefs.lang`, seeded by `main.rs`'s `init` and written by `poll`'s write-back on change (Roadmap F-9; before that field existed the choice was session-only and this row wrongly claimed it persisted with the project file); the write is `settings_popover::set_language`, status line included |
//! | Default easing | `Project.easing` | persists with the project file; the write is the core op `EditorCore::set_project_easing` (the easing picker's own direct setter — the Settings card itself only cycles, via `cycle_easing`) |
//! | Guides / Camera frame / Safe frames | `State.guides` / `guides_frustum` / `guides_safe` | NONE — session-only shell flags; the card is their only writer and no prefs field exists. The tools say so rather than imply persistence |
//! | Loop playback | `State.loop_playback` | NONE — session-only shell flag (the transport's `pb-loop` home) |
//! | Playback speed | `State.playback_speed` | NONE — session-only; `poll` reads the field every tick |
//! | Startup workspace | `State.workspace` + `LayoutPrefs.workspace` | READ here; WRITTEN by B3's `shell_set_workspace` through `workspaces::select_workspace`. One writer per setting across the shell family — `shell_set_setting` refuses the name and points at B3's tool |
//! | Stage size W/H | `Project.stage` | project data, not a preference; the card's steppers move it by ±40 through `apply_stage_step`. Read here; not settable here (it is not a boolean/enum popover toggle) |
//! | Playback audio | — | a READOUT (the `ffplay` process's liveness, `EditorCore::playback_audio_active`), not a setting; read here, refused as a set target |
//! | Autosave | — | NO toggle exists anywhere in the GUI: recovery autosave is unconditional engine behaviour (every [`EditorCore::AUTOSAVE_EVERY`] snapshot edits, `maybe_autosave`). Read here as a status block; refused as a set target rather than inventing a switch the GUI does not have |
//! | Reset layout | — | an ACTION, not a setting; not exposed by these tools |
//!
//! ## Restart-needed list: EMPTY
//!
//! No setting in the census needs a restart, and the
//! tools never claim one does. Theme applies the same
//! tick (`view` builds `app_theme(!core.light_theme)`
//! every frame); language, guides, loop and speed are
//! read live by the shell; the bridge starts/stops
//! immediately (the popover's own contract). The one
//! timing nuance is the live-MCP stop below — a
//! deferral of microseconds within one poll tick, not
//! a restart — and the set reply states it plainly.
//!
//! ## The live-MCP stop MUST be deferred (two proofs)
//!
//! `shell_set_setting {setting: "live_mcp", value:
//! false}` asks the endpoint to switch off the very
//! bridge that is serving the request. Calling
//! `live_mcp::set_enabled(false)` synchronously from
//! the executor is broken twice over, from source:
//!
//! 1. **Deadlock.** `live_mcp::drain` takes
//!    `lock_bridge()` at its head and holds the guard
//!    across the whole drain loop (`live_mcp.rs`,
//!    `drain`); `set_enabled`'s off branch takes the
//!    same non-reentrant `Mutex`. A synchronous stop
//!    inside any drained request wedges the UI thread
//!    permanently.
//! 2. **The reply would never be sent.** Even with
//!    the lock free, `LiveBridge::stop` joins the
//!    listener thread, whose reply-wait loop treats
//!    the raised stop flag as "abandon this
//!    connection" on its next 25 ms timeout tick —
//!    the client would see its connection close with
//!    no response to a call that in fact succeeded.
//!
//! So the executor NEVER touches the bridge. It
//! records the intent ([`request_live_mcp_stop`] /
//! the start twin), answers successfully, and the
//! shell applies the intent at ONE named point:
//!
//! **Integrator splice (main.rs `poll`):** inside the
//! `if live_mcp::is_enabled()` block, immediately
//! after the `live_mcp::drain(...)` call and before
//! the `if drained > 0` bookkeeping, insert
//! `crate::shell_tools_settings::apply_pending_bridge_actions(s);`
//!
//! Why that point is safe: `drain` sends every queued
//! reply (`req.reply.send(response)`) before it
//! returns, and the listener's wait loop matches a
//! queued reply before it ever re-checks the stop
//! flag (the flag is consulted only on a receive
//! timeout) — so by the time `apply_pending_bridge_actions`
//! raises the flag and joins the listener, this
//! request's response is already written to the
//! socket. Requests drained in the same batch AFTER
//! the stop request still execute and reply normally;
//! the stop lands at the end of the batch. One
//! consequence, stated for honesty: a
//! `shell_get_settings` drained in the same batch
//! after a stop request can still read `on: true` —
//! its `live_mcp.stop_pending: true` field is how a
//! client tells the stop is already decided.
//!
//! The apply step mirrors `toggle_live_mcp` exactly:
//! `set_enabled`'s RESULTING state is what gets
//! persisted to `LayoutPrefs.live_mcp` (a failed
//! start is never remembered as on), the outcome is
//! recorded via `shell::note_live_mcp_start`, and the
//! status line gets the popover's own wording.
//! Turning ON is deferred through the same point for
//! symmetry and for the same lock reason: while the
//! bridge is on, `set_enabled(true)` happens to early-
//! return before the lock, but the executor does not
//! rely on that — it checks `is_enabled()` itself,
//! answers an already-on bridge idempotently, and
//! defers a real start.
//!
//! ## Undo
//!
//! None of these tools spends an undo step, and that
//! is parity, not an omission: every state here is
//! shell state or project-settings state, which the
//! GUI's own paths never snapshot either
//! (`set_project_easing`'s docs: "project-settings
//! tweaks are not undoable edits in this core";
//! undo stores `Project` only, so `lang`/theme could
//! not be captured even by a snapshot). The Wave 14
//! one-undo-step contract governs project-mutating
//! tools; E10's re-verification should list these
//! three as named shell exceptions.
//!
//! ## Accent colours
//!
//! Untouched, by standing user decision: the theme
//! tool switches between the two EXISTING themes
//! (`dark` / `light`) and edits no token, ever.

use std::sync::atomic::{AtomicBool, Ordering};

use serde_json::{json, Value};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::i18n::Lang;
use stykanimate_rs::model::Easing;

use crate::rune_sec::playback_controls::SPEEDS;
use crate::rune_sec::settings_popover;
use crate::State;

// ---------------------------------------------------------------------
// Deferred bridge actions (see the module docs for
// why the bridge is never touched from an executor)
// ---------------------------------------------------------------------

/// A stop was requested by `shell_set_setting` and
/// not yet applied by the shell's poll.
static PENDING_LIVE_MCP_STOP: AtomicBool = AtomicBool::new(false);

/// A start was requested by `shell_set_setting` and
/// not yet applied by the shell's poll.
static PENDING_LIVE_MCP_START: AtomicBool = AtomicBool::new(false);

/// Record that live control is to stop at the next
/// apply point. Called by the set executor only;
/// idempotent.
pub fn request_live_mcp_stop() {
    PENDING_LIVE_MCP_STOP.store(true, Ordering::SeqCst);
}

/// Record that live control is to start at the next
/// apply point. Called by the set executor only;
/// idempotent.
pub fn request_live_mcp_start() {
    PENDING_LIVE_MCP_START.store(true, Ordering::SeqCst);
}

/// Whether a stop is recorded but not yet applied —
/// surfaced by `shell_get_settings` so a client can
/// tell "off, decided" from "still on".
pub fn pending_live_mcp_stop() -> bool {
    PENDING_LIVE_MCP_STOP.load(Ordering::SeqCst)
}

/// Take (and clear) the pending stop flag.
fn take_pending_live_mcp_stop() -> bool {
    PENDING_LIVE_MCP_STOP.swap(false, Ordering::SeqCst)
}

/// Take (and clear) the pending start flag.
fn take_pending_live_mcp_start() -> bool {
    PENDING_LIVE_MCP_START.swap(false, Ordering::SeqCst)
}

/// Apply any deferred bridge action — THE bridge
/// touch-point for these tools, called by the shell's
/// `poll` at the splice point named in the module
/// docs (after the drain has sent the batch's
/// replies, never inside it). Returns `true` when an
/// action was applied. Mirrors
/// `settings_popover::toggle_live_mcp`'s discipline:
/// the RESULTING state is persisted to
/// `LayoutPrefs.live_mcp` via the same
/// load-modify-save, the outcome is noted for the
/// settings card, and the status line carries the
/// popover's own wording.
pub fn apply_pending_bridge_actions(s: &mut State) -> bool {
    if take_pending_live_mcp_stop() {
        let on = crate::live_mcp::set_enabled(false);
        stykanimate_rs::shell::note_live_mcp_start(
            stykanimate_rs::shell::LiveMcpStartOutcome::Stopped,
        );
        persist_live_mcp_pref(on);
        s.core.status = "Live MCP off".to_string();
        return true;
    }
    if take_pending_live_mcp_start() {
        let on = crate::live_mcp::set_enabled(true);
        stykanimate_rs::shell::note_live_mcp_start(if on {
            stykanimate_rs::shell::LiveMcpStartOutcome::Listening
        } else {
            stykanimate_rs::shell::LiveMcpStartOutcome::FailedToStart
        });
        persist_live_mcp_pref(on);
        s.core.status = if on {
            format!("Live MCP on — {}", crate::live_mcp::endpoint())
        } else {
            format!(
                "Live MCP could not start — port {} is in use (another editor instance?).",
                crate::live_mcp::PORT
            )
        };
        return true;
    }
    false
}

/// Persist the bridge's resulting state through the
/// one prefs path (`shell::load_layout` /
/// `shell::save_layout`), preserving every other
/// field — the write `toggle_live_mcp` performs.
fn persist_live_mcp_pref(on: bool) {
    let mut prefs = stykanimate_rs::shell::load_layout();
    prefs.live_mcp = on;
    stykanimate_rs::shell::save_layout(&prefs);
}

// ---------------------------------------------------------------------
// Argument parsing — pure, fully validated before
// any state is touched, so a refusal never leaves a
// half-applied setting behind.
// ---------------------------------------------------------------------

/// The setting names `shell_set_setting` accepts.
pub const SETTABLE_SETTINGS: [&str; 9] = [
    "theme",
    "live_mcp",
    "language",
    "default_easing",
    "guides",
    "camera_frame",
    "safe_frames",
    "loop_playback",
    "playback_speed",
];

/// One validated setting change, ready to apply.
/// Parsing produces this; applying it cannot fail
/// validation (the live-MCP start can still fail at
/// the bridge — reported by the apply point, not
/// here).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SettingPatch {
    /// `light`: true = light theme, false = dark.
    Theme { light: bool },
    LiveMcp { on: bool },
    Language { lang: Lang },
    DefaultEasing { easing: Easing },
    Guides { on: bool },
    CameraFrame { on: bool },
    SafeFrames { on: bool },
    LoopPlayback { on: bool },
    PlaybackSpeed { speed: f32 },
}

/// Parse the `shell_set_theme` argument: the required
/// string `theme`, `"dark"` or `"light"` (matched
/// case-insensitively). Returns `light`.
pub fn parse_theme_arg(args: &Value) -> Result<bool, String> {
    let raw = args
        .get("theme")
        .ok_or_else(|| "missing required argument 'theme'".to_string())?;
    let text = raw
        .as_str()
        .ok_or_else(|| format!("argument 'theme' must be a string, got {}", type_name(raw)))?;
    match text.to_ascii_lowercase().as_str() {
        "dark" => Ok(false),
        "light" => Ok(true),
        _ => Err(format!(
            "argument 'theme' must be \"dark\" or \"light\", got \"{text}\""
        )),
    }
}

/// Parse and validate a `shell_set_setting` call
/// (`{"setting": name, "value": …}`) into a
/// [`SettingPatch`]. Known-but-not-settable names are
/// refused with the reason and, where another tool
/// owns the setting, that tool's name; unknown names
/// are refused with the full accepted list.
pub fn parse_setting_patch(args: &Value) -> Result<SettingPatch, String> {
    let raw_name = args
        .get("setting")
        .ok_or_else(|| "missing required argument 'setting'".to_string())?;
    let name = raw_name.as_str().ok_or_else(|| {
        format!(
            "argument 'setting' must be a string, got {}",
            type_name(raw_name)
        )
    })?;
    let value = args
        .get("value")
        .ok_or_else(|| format!("missing required argument 'value' for setting '{name}'"))?;
    match name {
        "theme" => {
            let text = req_str(value, name)?;
            match text.to_ascii_lowercase().as_str() {
                "dark" => Ok(SettingPatch::Theme { light: false }),
                "light" => Ok(SettingPatch::Theme { light: true }),
                _ => Err(format!(
                    "setting 'theme' must be \"dark\" or \"light\", got \"{text}\""
                )),
            }
        }
        "live_mcp" => Ok(SettingPatch::LiveMcp {
            on: req_bool(value, name)?,
        }),
        "language" => {
            let code = req_str(value, name)?;
            match lang_from_code(code) {
                Some(lang) => Ok(SettingPatch::Language { lang }),
                None => Err(format!(
                    "setting 'language': no language with code \"{code}\" \
                     (the i18n table has {} languages; use a BCP 47 code such as \"en\")",
                    Lang::all().len()
                )),
            }
        }
        "default_easing" => {
            let text = req_str(value, name)?;
            match easing_from_name(text) {
                Some(easing) => Ok(SettingPatch::DefaultEasing { easing }),
                None => Err(format!(
                    "setting 'default_easing' must be one of \"linear\", \"smooth\", \
                     \"ease-in\", \"ease-out\", got \"{text}\""
                )),
            }
        }
        "guides" => Ok(SettingPatch::Guides {
            on: req_bool(value, name)?,
        }),
        "camera_frame" => Ok(SettingPatch::CameraFrame {
            on: req_bool(value, name)?,
        }),
        "safe_frames" => Ok(SettingPatch::SafeFrames {
            on: req_bool(value, name)?,
        }),
        "loop_playback" => Ok(SettingPatch::LoopPlayback {
            on: req_bool(value, name)?,
        }),
        "playback_speed" => Ok(SettingPatch::PlaybackSpeed {
            speed: req_speed(value, name)?,
        }),
        // Known names this tool deliberately does NOT
        // set — each refusal names the reason (and the
        // owning tool where one exists).
        "autosave" => Err(
            "setting 'autosave' cannot be set: the GUI has no autosave switch — \
             recovery autosave is always on (every 25 edits). Refusing to invent one."
                .to_string(),
        ),
        "startup_workspace" | "workspace" => Err(
            "setting 'startup_workspace' is owned by shell_set_workspace \
             (it switches the workspace and persists the startup default in one op); \
             shell_get_settings reports it read-only."
                .to_string(),
        ),
        "stage_size" | "stage_width" | "stage_height" => Err(
            "stage size is project data, not a shell setting: the Settings card \
             steps it by ±40 per press; shell_set_setting covers the card's \
             boolean/enum settings only."
                .to_string(),
        ),
        "playback_audio" => Err(
            "setting 'playback_audio' is a readout, not a setting: it reports \
             whether the playback audio process is running; there is nothing to set."
                .to_string(),
        ),
        "reset_layout" => Err(
            "'reset_layout' is an action, not a setting, and is not exposed \
             by the settings tools."
                .to_string(),
        ),
        _ => Err(format!(
            "unknown setting '{name}'; shell_set_setting accepts: {}",
            SETTABLE_SETTINGS.join(", ")
        )),
    }
}

/// A language by its BCP 47 code (`Lang::code`),
/// matched case-insensitively over the whole i18n
/// table — the same table the popover's grid is
/// built from, so a language the card offers can
/// never be refused here.
pub fn lang_from_code(code: &str) -> Option<Lang> {
    Lang::all()
        .iter()
        .find(|l| l.code().eq_ignore_ascii_case(code))
        .copied()
}

/// An easing by name: the `Easing::label` strings
/// ("linear", "smooth", "ease-in", "ease-out"),
/// accepting `_`/space spellings and any case.
pub fn easing_from_name(text: &str) -> Option<Easing> {
    let norm = text
        .to_ascii_lowercase()
        .replace(['_', ' '], "-");
    match norm.as_str() {
        "linear" => Some(Easing::Linear),
        "smooth" => Some(Easing::Smooth),
        "ease-in" => Some(Easing::EaseIn),
        "ease-out" => Some(Easing::EaseOut),
        _ => None,
    }
}

/// A playback speed: a finite JSON number that is
/// one of the transport's `SPEEDS` entries — the
/// popover's chips are a direct select over exactly
/// that table, so an off-table value is refused
/// rather than written.
fn req_speed(value: &Value, name: &str) -> Result<f32, String> {
    let n = value.as_f64().ok_or_else(|| {
        format!(
            "setting '{name}' must be a number, got {}",
            type_name(value)
        )
    })?;
    let f = n as f32;
    if !n.is_finite() || !f.is_finite() {
        return Err(format!(
            "setting '{name}' must be a finite number, got {n}"
        ));
    }
    if SPEEDS.contains(&f) {
        Ok(f)
    } else {
        Err(format!(
            "setting '{name}' must be one of the playback speeds ({}), got {f}",
            SPEEDS
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ))
    }
}

/// A required boolean setting value.
fn req_bool(value: &Value, name: &str) -> Result<bool, String> {
    value.as_bool().ok_or_else(|| {
        format!(
            "setting '{name}' must be a boolean, got {}",
            type_name(value)
        )
    })
}

/// A required string setting value.
fn req_str<'a>(value: &'a Value, name: &str) -> Result<&'a str, String> {
    value.as_str().ok_or_else(|| {
        format!(
            "setting '{name}' must be a string, got {}",
            type_name(value)
        )
    })
}

/// A JSON value's type, in the refusal messages'
/// plain wording.
fn type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

// ---------------------------------------------------------------------
// Executors — run against the live shell `State` via
// B2's shell-command channel.
// ---------------------------------------------------------------------

/// `shell_set_theme`: set the theme outright — never
/// a blind toggle, exactly like the card's Dark /
/// Light pair. The write is the popover's own
/// `set_theme` (the pub field `EditorCore.light_theme`);
/// persistence is the shell's existing `poll`
/// write-back to `LayoutPrefs.dark`, the same path
/// the buttons rely on. Idempotent. No undo step
/// (shell state; the GUI spends none).
//
// TOOL ENTRY SPEC: shell_set_theme
//   name: "shell_set_theme"
//   description: "Set the editor theme: 'dark' or 'light'. Sets the existing
//     theme outright (never a blind toggle) via the same field the Settings
//     card's Dark/Light buttons write; the shell persists it to the layout
//     prefs on the same tick. Applies immediately, no restart. Accent colours
//     are not touched. Live endpoint only — on stdio this name returns the
//     shell-refusal error. No undo step (shell state)."
//   schema: obj_schema(json!({ "theme": { "type": "string", "enum": ["dark", "light"] } }), &["theme"])
//   dispatch: ShellCommand::SetTheme { light } -> exec_shell_set_theme
// MCP.md ROW SPEC:
//   | `shell_set_theme` | Set the editor theme outright ('dark' or 'light') — the Settings card's Dark/Light buttons as a tool. Applies immediately and persists via the shell's layout-prefs write-back; accent colours untouched. Live endpoint only. No undo step (shell state). | `theme` | — |
pub fn exec_shell_set_theme(s: &mut State, args: &Value) -> Result<Value, String> {
    let light = parse_theme_arg(args)?;
    apply_theme(s, light);
    Ok(json!({
        "ok": true,
        "theme": if light { "light" } else { "dark" },
        "persisted_as": "LayoutPrefs.dark (shell poll write-back, same tick)",
    }))
}

/// The one theme write, shared by `shell_set_theme`
/// and the `theme` setting of `shell_set_setting`:
/// the popover's own setter on the core's real field.
fn apply_theme(s: &mut State, light: bool) {
    settings_popover::set_theme(&mut s.core, light);
}

/// `shell_get_settings`: every user-facing setting
/// the Settings card exposes, read from its real
/// home (bridge, core, shell fields, prefs file) —
/// see the module docs' census for the full table,
/// including the read-only members (stage size,
/// startup workspace, playback audio, autosave
/// status) and why they are read-only here. Read-only
/// tool; changes nothing.
//
// TOOL ENTRY SPEC: shell_get_settings
//   name: "shell_get_settings"
//   description: "Read the editor's user-facing settings (the Settings card as
//     a tool): theme, live-MCP state with its endpoint and persisted
//     preference, language, the project default easing, the stage-guides
//     flags, loop playback and playback speed, the current and startup
//     workspaces, the stage size, and the autosave status. Read-only. Live
//     endpoint only — on stdio this name returns the shell-refusal error."
//   schema: obj_schema(json!({}), &[])
//   dispatch: ShellCommand::GetSettings -> exec_shell_get_settings
// MCP.md ROW SPEC:
//   | `shell_get_settings` | Read every user-facing setting the Settings card exposes, from its real home: theme, live-MCP (on/off, endpoint, persisted preference, pending stop, last start outcome), language, default easing, guides flags, loop/speed, workspaces, stage size, autosave status. Read-only. Live endpoint only. | — | — |
pub fn exec_shell_get_settings(s: &State) -> Result<Value, String> {
    Ok(settings_snapshot(s))
}

/// The settings readout `shell_get_settings`
/// returns. Split from the executor so its shape has
/// one definition.
pub fn settings_snapshot(s: &State) -> Value {
    let prefs = stykanimate_rs::shell::load_layout();
    let core = &s.core;
    json!({
        "theme": if core.light_theme { "light" } else { "dark" },
        "live_mcp": {
            "on": crate::live_mcp::is_enabled(),
            "endpoint": crate::live_mcp::endpoint(),
            "port": crate::live_mcp::PORT,
            "persisted_preference": prefs.live_mcp,
            "stop_pending": pending_live_mcp_stop(),
            "last_start_outcome": start_outcome_str(
                stykanimate_rs::shell::live_mcp_start_outcome(),
            ),
            "auth": "none — while on, any program on this computer can drive the editor",
        },
        "language": {
            "code": core.lang.code(),
            "label": core.lang.label(),
        },
        "default_easing": core.project.easing.label(),
        "stage": {
            "width": core.project.stage[0],
            "height": core.project.stage[1],
            "note": "project data (persists with the project file); the Settings card steps it, these tools only read it",
        },
        "guides": {
            "guides": s.guides,
            "camera_frame": s.guides_frustum,
            "safe_frames": s.guides_safe,
            "persisted": false,
        },
        "playback": {
            "loop": s.loop_playback,
            "speed": s.playback_speed,
            "speeds": SPEEDS,
            "audio_active": core.playback_audio_active(),
            "persisted": false,
        },
        "workspace": {
            "current": s.workspace.name(),
            "startup": prefs.workspace,
        },
        "autosave": {
            "mode": "always_on",
            "every_edits": EditorCore::AUTOSAVE_EVERY,
            "edits_since_autosave": core.edits_since_autosave(),
            "note": "no GUI toggle exists; recovery autosave cannot be switched off",
        },
        "settings_card_open": s.settings.open,
    })
}

/// The start-outcome record as a stable string for
/// the settings readout.
fn start_outcome_str(
    outcome: stykanimate_rs::shell::LiveMcpStartOutcome,
) -> &'static str {
    use stykanimate_rs::shell::LiveMcpStartOutcome;
    match outcome {
        LiveMcpStartOutcome::NotAttempted => "not_attempted",
        LiveMcpStartOutcome::Listening => "listening",
        LiveMcpStartOutcome::FailedToStart => "failed_to_start",
        LiveMcpStartOutcome::Stopped => "stopped",
    }
}

/// `shell_set_setting`: set ONE of the Settings
/// card's boolean/enum settings by name —
/// `{"setting": …, "value": …}` over
/// [`SETTABLE_SETTINGS`]. Every accepted setting
/// takes effect immediately (no restart for any of
/// them) and persists exactly as the popover persists
/// it — same field, same prefs write path; the
/// session-only flags (guides, loop, speed) persist
/// nowhere, exactly as in the GUI, and the reply's
/// `persisted_as` says which. The one deferred case
/// is `live_mcp: false` over the live endpoint: the
/// stop is applied by the shell as soon as this
/// reply has been delivered (see the module docs —
/// a synchronous stop would deadlock the drain and
/// eat this reply), and the reply says so in
/// `applied`. No undo step (shell/project-settings
/// state; the GUI paths spend none).
//
// TOOL ENTRY SPEC: shell_set_setting
//   name: "shell_set_setting"
//   description: "Set one Settings-card setting by name: 'theme' (\"dark\"/
//     \"light\"), 'live_mcp' (bool), 'language' (BCP 47 code), 'default_easing'
//     (\"linear\"/\"smooth\"/\"ease-in\"/\"ease-out\"), 'guides', 'camera_frame',
//     'safe_frames', 'loop_playback' (bools), 'playback_speed' (one of 0.25,
//     0.5, 1, 1.5, 2). Each persists exactly as the Settings card persists it;
//     guides/loop/speed are session-only, as in the GUI. All apply
//     immediately, no restart — except live_mcp=false, which the shell
//     applies the moment this reply has been delivered, so the reply is
//     never eaten by the shutdown it orders. Names the GUI does not expose
//     as settable (autosave, startup_workspace, stage size, playback_audio)
//     are refused with the reason. Live endpoint only. No undo step."
//   schema: obj_schema(json!({ "setting": { "type": "string" }, "value": {} }), &["setting", "value"])
//   dispatch: ShellCommand::SetSetting { args } -> exec_shell_set_setting
// MCP.md ROW SPEC:
//   | `shell_set_setting` | Set one Settings-card boolean/enum setting by name (theme, live_mcp, language, default_easing, guides, camera_frame, safe_frames, loop_playback, playback_speed). Persists exactly as the card persists it; session-only flags say so. All immediate, no restart; live_mcp=false is applied just after the reply is delivered. Non-settable names are refused with the reason. Live endpoint only. No undo step. | `setting`, `value` | — |
pub fn exec_shell_set_setting(s: &mut State, args: &Value) -> Result<Value, String> {
    let patch = parse_setting_patch(args)?;
    apply_setting_patch(s, patch)
}

/// Apply one validated patch. Split from the
/// executor: parsing is pure and unit-tested below;
/// this half is the thin set of field writes, each
/// the popover's own.
pub fn apply_setting_patch(s: &mut State, patch: SettingPatch) -> Result<Value, String> {
    match patch {
        SettingPatch::Theme { light } => {
            apply_theme(s, light);
            Ok(json!({
                "ok": true,
                "setting": "theme",
                "value": if light { "light" } else { "dark" },
                "applied": "immediate",
                "persisted_as": "LayoutPrefs.dark (shell poll write-back, same tick)",
            }))
        }
        SettingPatch::LiveMcp { on } => apply_live_mcp_patch(s, on),
        SettingPatch::Language { lang } => {
            // The popover's own apply: field + the
            // status line `cycle_lang` writes.
            settings_popover::set_language(&mut s.core, lang);
            Ok(json!({
                "ok": true,
                "setting": "language",
                "value": { "code": lang.code(), "label": lang.label() },
                "applied": "immediate",
                "persisted_as": "LayoutPrefs.lang (shell poll write-back, same tick)",
                "status": s.core.status,
            }))
        }
        SettingPatch::DefaultEasing { easing } => {
            // The core's direct setter (the easing
            // picker's preset op): status + dirty mark
            // inside, no snapshot — as in the GUI.
            s.core.set_project_easing(easing);
            Ok(json!({
                "ok": true,
                "setting": "default_easing",
                "value": easing.label(),
                "applied": "immediate",
                "persisted_as": "Project.easing (persists with the project file)",
                "status": s.core.status,
            }))
        }
        SettingPatch::Guides { on } => {
            s.guides = on;
            Ok(bool_setting_reply("guides", on))
        }
        SettingPatch::CameraFrame { on } => {
            s.guides_frustum = on;
            Ok(bool_setting_reply("camera_frame", on))
        }
        SettingPatch::SafeFrames { on } => {
            s.guides_safe = on;
            Ok(bool_setting_reply("safe_frames", on))
        }
        SettingPatch::LoopPlayback { on } => {
            s.loop_playback = on;
            Ok(bool_setting_reply("loop_playback", on))
        }
        SettingPatch::PlaybackSpeed { speed } => {
            // The popover chips' own apply: a bare
            // field write; `poll` picks the rate up.
            settings_popover::set_playback_speed(s, speed);
            Ok(json!({
                "ok": true,
                "setting": "playback_speed",
                "value": speed,
                "applied": "immediate",
                "persisted_as": "nowhere — session-only shell state, as in the GUI",
            }))
        }
    }
}

/// The reply for a session-only boolean shell flag
/// (guides trio, loop): immediate, persisted nowhere
/// — stated, not implied.
fn bool_setting_reply(setting: &str, on: bool) -> Value {
    json!({
        "ok": true,
        "setting": setting,
        "value": on,
        "applied": "immediate",
        "persisted_as": "nowhere — session-only shell state, as in the GUI",
    })
}

/// The live-MCP patch. NEVER calls `set_enabled`
/// (module docs: deadlock + lost reply). An
/// already-on bridge asked to stay on is answered
/// idempotently; a real start or stop is recorded
/// as a deferred intent for
/// [`apply_pending_bridge_actions`], and the reply
/// says exactly when it lands.
fn apply_live_mcp_patch(_s: &mut State, on: bool) -> Result<Value, String> {
    // The patch touches no shell field — it records a
    // deferred intent, and the apply point does the
    // rest. (The parameter keeps the executor shape
    // uniform across patches.)
    if on {
        if crate::live_mcp::is_enabled() {
            Ok(json!({
                "ok": true,
                "setting": "live_mcp",
                "value": true,
                "applied": "already_on",
                "endpoint": crate::live_mcp::endpoint(),
                "persisted_as": "LayoutPrefs.live_mcp",
            }))
        } else {
            request_live_mcp_start();
            Ok(json!({
                "ok": true,
                "setting": "live_mcp",
                "value": true,
                "applied": "deferred_to_shell_poll",
                "note": "the shell starts the endpoint at its post-drain apply point, \
                         in the same tick; shell_get_settings then reports on: true \
                         (or last_start_outcome: failed_to_start if the port is held)",
                "endpoint": crate::live_mcp::endpoint(),
                "persisted_as": "LayoutPrefs.live_mcp (the resulting state, written by the apply point)",
            }))
        }
    } else if crate::live_mcp::is_enabled() {
        request_live_mcp_stop();
        Ok(json!({
            "ok": true,
            "setting": "live_mcp",
            "value": false,
            "applied": "after_this_reply_is_delivered",
            "note": "the endpoint stops as soon as this response has been written — \
                     a synchronous stop would deadlock the request drain and the \
                     reply would never reach you",
            "persisted_as": "LayoutPrefs.live_mcp (the resulting state, written by the apply point)",
        }))
    } else {
        Ok(json!({
            "ok": true,
            "setting": "live_mcp",
            "value": false,
            "applied": "already_off",
            "persisted_as": "LayoutPrefs.live_mcp",
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The executors need a full shell `State`, which
    // only the shell's own test builders construct;
    // Runtime-level coverage of the three tools is
    // B10's. What is pinned here is everything that
    // can refuse: the parsers, the name census, and
    // the deferred-intent flags.

    #[test]
    fn theme_arg_sets_dark_and_light_outright() {
        assert_eq!(parse_theme_arg(&json!({ "theme": "dark" })), Ok(false));
        assert_eq!(parse_theme_arg(&json!({ "theme": "light" })), Ok(true));
        assert_eq!(parse_theme_arg(&json!({ "theme": "Light" })), Ok(true));
    }

    #[test]
    fn theme_arg_refuses_missing_wrong_type_and_bad_value() {
        assert!(parse_theme_arg(&json!({})).unwrap_err().contains("'theme'"));
        assert!(parse_theme_arg(&json!({ "theme": 1 }))
            .unwrap_err()
            .contains("must be a string"));
        let err = parse_theme_arg(&json!({ "theme": "sepia" })).unwrap_err();
        assert!(err.contains("\"dark\" or \"light\"") && err.contains("sepia"), "{err}");
    }

    #[test]
    fn patch_parses_every_settable_setting() {
        assert_eq!(
            parse_setting_patch(&json!({ "setting": "theme", "value": "dark" })),
            Ok(SettingPatch::Theme { light: false })
        );
        assert_eq!(
            parse_setting_patch(&json!({ "setting": "live_mcp", "value": true })),
            Ok(SettingPatch::LiveMcp { on: true })
        );
        assert_eq!(
            parse_setting_patch(&json!({ "setting": "language", "value": "en" })),
            Ok(SettingPatch::Language { lang: Lang::En })
        );
        assert_eq!(
            parse_setting_patch(&json!({ "setting": "default_easing", "value": "ease-in" })),
            Ok(SettingPatch::DefaultEasing {
                easing: Easing::EaseIn
            })
        );
        assert_eq!(
            parse_setting_patch(&json!({ "setting": "guides", "value": false })),
            Ok(SettingPatch::Guides { on: false })
        );
        assert_eq!(
            parse_setting_patch(&json!({ "setting": "camera_frame", "value": true })),
            Ok(SettingPatch::CameraFrame { on: true })
        );
        assert_eq!(
            parse_setting_patch(&json!({ "setting": "safe_frames", "value": true })),
            Ok(SettingPatch::SafeFrames { on: true })
        );
        assert_eq!(
            parse_setting_patch(&json!({ "setting": "loop_playback", "value": false })),
            Ok(SettingPatch::LoopPlayback { on: false })
        );
        assert_eq!(
            parse_setting_patch(&json!({ "setting": "playback_speed", "value": 1.5 })),
            Ok(SettingPatch::PlaybackSpeed { speed: 1.5 })
        );
    }

    #[test]
    fn patch_refuses_non_bool_for_bool_settings() {
        let err = parse_setting_patch(&json!({ "setting": "guides", "value": "yes" }))
            .unwrap_err();
        assert!(err.contains("'guides'") && err.contains("boolean"), "{err}");
    }

    #[test]
    fn patch_refuses_off_table_and_non_finite_speeds() {
        let err = parse_setting_patch(&json!({ "setting": "playback_speed", "value": 3.0 }))
            .unwrap_err();
        assert!(err.contains("0.25") && err.contains("got 3"), "{err}");
        // A finite f64 that overflows f32 on the cast
        // (1e300) is the reachable non-finite case:
        // raw JSON cannot carry an actual infinity
        // through serde_json at all.
        let err = parse_setting_patch(&json!({ "setting": "playback_speed", "value": 1e300 }))
            .unwrap_err();
        assert!(err.contains("finite"), "{err}");
    }

    #[test]
    fn patch_refuses_unknown_language_code_and_bad_easing() {
        let err = parse_setting_patch(&json!({ "setting": "language", "value": "xx" }))
            .unwrap_err();
        assert!(err.contains("\"xx\"") && err.contains("28"), "{err}");
        let err = parse_setting_patch(
            &json!({ "setting": "default_easing", "value": "bouncy" }),
        )
        .unwrap_err();
        assert!(err.contains("bouncy") && err.contains("ease-in"), "{err}");
    }

    #[test]
    fn patch_refuses_the_named_non_settable_settings_with_reasons() {
        let autosave = parse_setting_patch(&json!({ "setting": "autosave", "value": false }))
            .unwrap_err();
        assert!(autosave.contains("no autosave switch"), "{autosave}");
        let ws = parse_setting_patch(
            &json!({ "setting": "startup_workspace", "value": "Draw" }),
        )
        .unwrap_err();
        assert!(ws.contains("shell_set_workspace"), "{ws}");
        let stage = parse_setting_patch(&json!({ "setting": "stage_width", "value": 800 }))
            .unwrap_err();
        assert!(stage.contains("project data"), "{stage}");
        let audio = parse_setting_patch(
            &json!({ "setting": "playback_audio", "value": true }),
        )
        .unwrap_err();
        assert!(audio.contains("readout"), "{audio}");
    }

    #[test]
    fn patch_refuses_unknown_names_with_the_accepted_list() {
        let err = parse_setting_patch(&json!({ "setting": "accent", "value": "red" }))
            .unwrap_err();
        assert!(err.contains("unknown setting 'accent'"), "{err}");
        for name in SETTABLE_SETTINGS {
            assert!(err.contains(name), "list missing {name}: {err}");
        }
    }

    #[test]
    fn language_codes_resolve_case_insensitively_over_the_whole_table() {
        for lang in Lang::all() {
            assert_eq!(lang_from_code(lang.code()), Some(lang));
            assert_eq!(lang_from_code(&lang.code().to_uppercase()), Some(lang));
        }
        assert_eq!(lang_from_code("xx"), None);
    }

    #[test]
    fn easing_names_accept_label_and_underscore_spellings() {
        assert_eq!(easing_from_name("linear"), Some(Easing::Linear));
        assert_eq!(easing_from_name("ease-in"), Some(Easing::EaseIn));
        assert_eq!(easing_from_name("Ease_Out"), Some(Easing::EaseOut));
        assert_eq!(easing_from_name("SMOOTH"), Some(Easing::Smooth));
        assert_eq!(easing_from_name("bouncy"), None);
    }

    #[test]
    fn deferred_bridge_flags_record_and_take_exactly_once() {
        // Start from a clean slate (other tests in this
        // binary may run in any order, but only this
        // one touches the flags).
        let _ = take_pending_live_mcp_stop();
        let _ = take_pending_live_mcp_start();
        assert!(!pending_live_mcp_stop());
        request_live_mcp_stop();
        request_live_mcp_stop(); // idempotent
        assert!(pending_live_mcp_stop());
        assert!(take_pending_live_mcp_stop());
        assert!(!take_pending_live_mcp_stop());
        request_live_mcp_start();
        assert!(take_pending_live_mcp_start());
        assert!(!take_pending_live_mcp_start());
    }

    #[test]
    fn start_outcome_strings_are_stable() {
        use stykanimate_rs::shell::LiveMcpStartOutcome as O;
        assert_eq!(start_outcome_str(O::NotAttempted), "not_attempted");
        assert_eq!(start_outcome_str(O::Listening), "listening");
        assert_eq!(start_outcome_str(O::FailedToStart), "failed_to_start");
        assert_eq!(start_outcome_str(O::Stopped), "stopped");
    }
}
