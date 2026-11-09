//! Settings popover (Phase D, section 24 of the GPUI→Rune
//! migration; guides toggle added in Phase E, section 21;
//! Phase G, section 13: guide split toggles + startup
//! workspace picker; Phase H, section 32: Font Awesome
//! glyphs — gear card header, sun/moon theme pair, eye
//! visibility toggles; Phase I, section 38: the Playback
//! block — loop toggle + playback-audio readout;
//! Phase K, section 32: the Reset layout button;
//! Phase M, section 25: the Playback block gains a
//! direct-select speed row): theme,
//! language, stage size, the project default easing,
//! stage guides, playback, the startup workspace and a
//! layout reset, behind one "Settings" toggle. Declared
//! under `rune_sec` from `main.rs` only — bin-crate code, not
//! part of the engine library.
//!
//! ## Why not the rune-kit `Popover`?
//!
//! Same reason as `figure_add` (section 2): the kit's
//! `Popover::build` needs a `rune_kit::Kit` and an overlay
//! layer fed into `rune_kit::root(...)`, and the shell's root
//! is a plain `col()` themed by `rune::tok` — no `Kit` exists
//! yet. Until the integrator wires one, this is a
//! state-toggled section standing in for the popover: same
//! controls, same ops, no floating layer. Swapping the card
//! below for a kit `Popover` later changes only this file's
//! [`settings_popover`] body and the root call.
//!
//! ## Controls → real ops / fields (nothing is faked)
//!
//! * **Theme (Dark / Light)** — two buttons setting the pub
//!   field `EditorCore.light_theme` (`editor.rs:63`, default
//!   `false` = dark). The GPUI shell's theme button flipped
//!   exactly this field and persisted it as
//!   `LayoutPrefs.dark = !light_theme`. Each button sets its
//!   own value (never a blind toggle), so the highlighted
//!   button always names the theme actually in force.
//!   INTEGRATOR GAP: `main.rs`'s `view` currently calls
//!   `tok::theme(true)` unconditionally, so the flip is real
//!   but invisible until `view` reads
//!   `tok::theme(!s.core.light_theme)` — see the report.
//! * **Language** — one button per language of the project's
//!   own i18n table (28 today, from `Lang::all()`,
//!   `i18n.rs:150`, native names via `Lang::label()`,
//!   `i18n.rs:116` — the same data
//!   `rune_sec::i18n_wire::language_options` exposes). The
//!   current language's button is highlighted
//!   (`EditorCore.lang`, `editor.rs:82`). Picking one sets
//!   the pub field directly — the only setter the core
//!   offers besides `cycle_lang` (`editor.rs:1906`), which is
//!   a cycle, not a select — and writes the same status line
//!   `cycle_lang` writes (`"Language: {label}"`).
//! * **Stage W / H steppers** — ±[`STAGE_STEP`] per press on
//!   the pub field `Project.stage` (`model.rs:685`), each
//!   axis clamped to [`STAGE_MIN`]..=[`STAGE_MAX`]. The core
//!   has no stage-size op; the step/clamp/status are the
//!   GPUI shell's own view helper verbatim (`adjust_stage` in
//!   the pre-removal `app.rs`: step 40, clamp 200..=4000,
//!   status `"Stage size: {w} x {h}."`). The stage renderer
//!   reads this field every frame, so the resize is live.
//! * **Default easing** — one cycle button calling the real
//!   op `EditorCore::cycle_easing` (`editor.rs:1686`), which
//!   advances `Project.easing` (`model.rs:696`) through
//!   `Easing::next` and writes its own status. The button
//!   label shows the current value (`Easing::label`,
//!   `model.rs:582`). The core offers no direct setter for
//!   the project default, so — exactly like the easing panel
//!   (section 10, key `ease-project-cycle`) — this is
//!   honestly a cycle button, not four fake presets. The two
//!   buttons coexist with distinct keys; this one is the
//!   Settings home for the same op.
//!
//! * **Guides** — one toggle button flipping the shell
//!   field `State.guides` (added by the integrator; default
//!   `false`). Guides are a pure view overlay — the stage
//!   draws a thirds grid + centre cross while the flag is
//!   set (Phase E section 4's `guides_geometry` in
//!   `stage_camera`) — so the flag lives in shell state,
//!   not on `EditorCore`/`Project`, and this card is its
//!   only writer. Phase D omitted this toggle because no
//!   guides existed to flip; Phase E adds the geometry, so
//!   the toggle now controls something real.
//! * **Guide split toggles (Phase G)** — two child toggles
//!   under Guides, indented and dimmed as its children:
//!   "Camera frame" flips `State.guides_frustum` and
//!   "Safe frames" flips `State.guides_safe` (both shell
//!   fields, default `true`, seeded by the integrator).
//!   They are independent flags: the stage composes them
//!   with `guides` (the frame overlays draw only while
//!   Guides is also on), so the buttons stay enabled when
//!   Guides is off — Rune buttons have no disabled state
//!   and none is faked here; the card's note under the
//!   row states the composition.
//! * **Loop playback (Phase I)** — a toggle in the guides
//!   toggles' `icon_btn` row idiom flipping the shell field
//!   `State.loop_playback` (seeded `true` by the
//!   integrator). The handler body is byte-for-byte the
//!   transport's `pb-loop` flip — a bare flag flip, no core
//!   call, no status change; the command palette's Toggle
//!   loop playback command flips the same field, and the
//!   shell's `poll` reads it to decide whether the playhead
//!   wraps or stops at the end. This card is a second home
//!   for the same switch, not a second flag.
//! * **Playback audio (Phase I)** — a muted readout line,
//!   no handler: `"Playback audio: on"` while
//!   `EditorCore::playback_audio_active` (`audio_play.rs`)
//!   reports the `ffplay` player process running, `"off"`
//!   otherwise. That fn measures process liveness, not
//!   audibility — on a machine with no audio device the
//!   process may exit at once after a successful start, and
//!   the line then honestly reads off (the fn's own docs
//!   say the same).
//! * **Playback speed (Phase M)** — a direct-select row
//!   of five chips under the loop toggle, one per step
//!   of the transport cycle's `SPEEDS` table
//!   (`playback_controls.rs`), each labelled with the
//!   bare value in the cycle button's own trim
//!   ("0.25×" … "2×" — [`speed_chip_label`]). The chip
//!   for the current `State.playback_speed` is
//!   highlighted ([`active_speed_index`]); a press
//!   writes the field directly
//!   ([`set_playback_speed`]) — silent, exactly like
//!   the `pb-speed` cycle: no status, no snapshot. The
//!   shell's `poll` notes the field into the audio
//!   engine and scales the advance itself, so the new
//!   rate needs no call from here. This row is a third
//!   home for the one field (transport cycle, View
//!   menu cycle, these chips), not a second speed
//!   state. The chips render through plain `btn`, the
//!   popover's picker-grid idiom (the language and
//!   workspace grids): their labels ARE the data.
//! * **Live MCP control** — a kit `toggle_row` (key
//!   `settings-live-mcp`) starting / stopping the
//!   editor-hosted MCP endpoint (`crate::live_mcp`; loopback
//!   HTTP on port 32117). The row's state reads the bridge
//!   itself (`live_mcp::is_enabled`) — the bridge is the
//!   truth, never a mirrored bool in shell state — and the
//!   press runs through [`toggle_live_mcp`], which persists
//!   the RESULTING state to `LayoutPrefs.live_mcp` (so a
//!   failed start is not remembered as on) and reports
//!   through the core's status line: on names the endpoint,
//!   a failed start names the busy port. The note under the
//!   row shows the endpoint and says plainly that while on,
//!   any program on this computer can control the editor,
//!   including switching workspaces and pressing play —
//!   the endpoint has no password in this version (the
//!   bridge module's own docs carry the full warning).
//!   At launch the shell attempts the start when the
//!   persisted preference is on (`main.rs` `init`); the
//!   attempt is live, not deferred — and this toggle is
//!   its runtime switch either way (a press starts or
//!   stops the listener immediately, no restart).
//!   A failed start is no longer invisible (the Wave 11
//!   audit's gap, closed in two steps): `init` and this
//!   toggle both record the attempt's outcome in the
//!   shell's start-outcome record, and the card shows a
//!   persistent failure note under the row while the
//!   bridge is off for that reason; since Wave 14 (D1)
//!   the bridge itself also remembers the OS error of
//!   its last failed start (`live_mcp::last_start_error`)
//!   for the note to name.
//! * **Startup workspace (Phase G)** — a compact picker
//!   (eight buttons, the language grid's 4-wide layout)
//!   naming the workspace the shell opens in. Picking one
//!   calls `workspaces::select_workspace` — the exact fn
//!   the workspace chip row and the Alt+1..8 keys run —
//!   which sets `State.workspace` AND persists the choice
//!   as `LayoutPrefs.workspace` (load-modify-save via
//!   `prefs_with_workspace` + `shell::save_layout`), so
//!   the pick is both the current workspace and the
//!   startup default `workspaces::from_prefs` seeds the
//!   next launch from. No new persistence path exists.
//! * **Reset layout (Phase K)** — one button driving the
//!   shell's layout back to its defaults: the Animate
//!   workspace through `workspaces::select_workspace`
//!   (so the reset is also the persisted startup
//!   default), both side panels shown (`State.show_left`
//!   / `show_right`, integrator-seeded `true`), and the
//!   three guide flags set on outright (`State.guides`,
//!   `guides_frustum`, `guides_safe`) — set directly,
//!   never flipped through the toggle fns, which would
//!   drive a flag already at its default away from it.
//!   The reset's scope is layout only, by design: it
//!   does NOT touch the theme — the dark/light state is
//!   persisted outside `State` as `LayoutPrefs.dark`
//!   and `main`'s poll owns the write-back from
//!   `core.light_theme`, so resetting it from here
//!   would fight that machinery — nor playback state,
//!   nor any project data.
//! * **Icons (Phase H)** — pure presentation, no behaviour
//!   change: the open card gains a header row (a `gear`
//!   `icon_el` beside the "Settings" title, the shell's
//!   icon-row idiom); the theme pair renders through
//!   `icon_btn` — `moon` on the Dark button, `sun` on the
//!   Light button, mirroring the labels, which name the
//!   state each button sets (the pair is still two
//!   set-value buttons, never a blind toggle); the card
//!   toggle and the three guide toggles render through
//!   `icon_btn` with the `eye` visibility glyph, labels
//!   kept. Every key is unchanged; the language grid, the
//!   stage steppers, the easing cycle and the workspace
//!   grid deliberately stay plain text buttons (their
//!   labels ARE the data: native language names, axis
//!   letters, workspace names).
//! * **Phase N restyle** — pure presentation, no
//!   behaviour change: the card root is the design kit's
//!   `panel_col` (radius 12, padding 12, the hairline
//!   `card_border` the card previously lacked), its
//!   header the kit's `panel_title` (gear + "Settings")
//!   over a kit `divider`, its section headings the
//!   kit's `section_label` — the two sentence-long notes
//!   (guides composition, startup workspace) stay muted
//!   prose, they are not headings — the stage readouts
//!   the kit's `stepper_value`, and the four on/off rows
//!   (Guides, Camera frame, Safe frames, Loop playback)
//!   kit `toggle_row`s whose pill carries the state the
//!   `eye` / `repeat` glyphs used to signal. The card
//!   also carries the M2 entrance transition (fade +
//!   4 pt rise on the SNAP spring pair; P15). Every key,
//!   handler body and op is unchanged.
//!
//! * **Wave 10 width audit** — the Phase N remainder
//!   ("language rows can run past the card's right edge")
//!   is PROVEN and fixed. Measured with the shell's own
//!   conservative label-width model ([`button_width`])
//!   against the card's 316 pt content width
//!   ([`CARD_CONTENT_W`]), every one of the seven
//!   4-button language rows runs over — by +16 to
//!   +127 pt (worst: "Português (Brasil)" / "Română" /
//!   "Русский" / "Slovenščina"). The rows already carried
//!   `.flex_wrap()` (Wave 1), but wrap can only engage
//!   at a definite width, and the rows — unlike
//!   `fx_browser`'s working wrap rows — lacked the
//!   `.w_full()` that pins a row to the card width, so
//!   each row sized to its content and painted past the
//!   edge (at 1440 pt, past the window itself, which is
//!   why snaps looked clean). The fix is the missing
//!   half of the idiom: `.w_full()` on the language
//!   rows; the workspace and speed rows gain the same
//!   pair defensively — all fit on one line at 316 pt
//!   by the same arithmetic (258 / 222 and 259), but
//!   the wider workspace row and the speed row pass
//!   the 236 pt narrowest-column width
//!   ([`CARD_CONTENT_W_MIN`]), where they now wrap
//!   instead of overrunning. No button is truncated:
//!   the widest single button ("Español
//!   (Latinoamérica)", 181 pt) fits even the narrowest
//!   column, so wrapping loses nothing. The rest of the
//!   card audits clean at 316 pt: the notes wrap via
//!   `note_line`, the toggle rows are grow-bounded,
//!   the theme / stage / easing rows measure 150 pt or
//!   less. Tests simulate the wrap packing and pin all
//!   of these figures.
//!
//! ## Persistence
//!
//! Theme maps onto the EXISTING `LayoutPrefs.dark`
//! (`shell.rs`; `dark = !core.light_theme`, default dark),
//! while language, stage size and default easing all live
//! on the core/project and persist with the project file
//! itself. The startup-workspace picker reuses the
//! EXISTING `LayoutPrefs.workspace` through
//! `workspaces::select_workspace` (above). The theme
//! flip is loaded and saved by the Rune shell: `main.rs`
//! seeds `core.light_theme` from `shell::load_layout()
//! .dark` at startup and persists `prefs.dark` on change
//! in `poll` (landed in the Phase D integration; an
//! earlier note here claimed otherwise and the Phase N
//! hygiene audit disproved it against the source). The
//! Live MCP toggle is the one control with its own field:
//! `LayoutPrefs.live_mcp` (serde-defaulted on, so a prefs
//! file written before the field existed gets a launch
//! start attempt), written by [`toggle_live_mcp`]
//! through the same load-modify-save `poll` uses for the
//! theme.
//!
//! ## Integrator wiring (main.rs)
//!
//! ```ignore
//! mod rune_sec {
//!     pub mod settings_popover;
//! }
//! // in `State`:
//! pub settings: settings_popover::SettingsState,
//! // in `init()`:
//! settings: settings_popover::SettingsState::default(),
//! // the guides toggle additionally needs, in `State`:
//! pub guides: bool,
//! // in `init()`:
//! guides: false,
//! // in `view()`, after the app bar:
//! .child(rune_sec::settings_popover::settings_popover(s, &t))
//! // and in `view()`: tok::theme(!s.core.light_theme)
//! ```

use rune::tok::{self, type_, Theme};
use rune::{col, row, text, El, Transition};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::i18n::Lang;

use crate::chrome_view::btn;
use crate::rune_sec::design;
use crate::rune_sec::icon_btn::icon_btn;
use crate::rune_sec::playback_controls::SPEEDS;
use crate::rune_sec::workspaces::{select_workspace, Workspace};
use crate::State;

/// View-state for this section: whether the settings card is
/// shown under the "Settings" toggle. Owned by `State` (see
/// the module docs for the wiring); the card is rebuilt from
/// it every frame. Everything the card controls lives on the
/// core — this struct holds nothing else.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SettingsState {
    /// Settings card expanded under the toggle.
    pub open: bool,
}

/// Stage-size step per button press (the GPUI shell's
/// `adjust_stage` step).
pub const STAGE_STEP: f32 = 40.0;

/// Smallest stage width/height the steppers allow (the GPUI
/// shell's clamp floor).
pub const STAGE_MIN: f32 = 200.0;

/// Largest stage width/height the steppers allow (the GPUI
/// shell's clamp ceiling).
pub const STAGE_MAX: f32 = 4000.0;

/// Language buttons per row in the picker grid.
pub const LANG_COLUMNS: usize = 4;

/// The settings card's content width at the shell's
/// default layout: the right column is 340 pt wide
/// (`LayoutPrefs::col_right_w`'s serde default,
/// `shell.rs`) and the kit card pads 12 pt a side
/// (`design::PANEL_PAD`), leaving 316. Every row of the
/// card must fit — or wrap — inside this width.
pub const CARD_CONTENT_W: f32 = 316.0;

/// The card's content width at the NARROWEST right
/// column the panel splitter allows (260 pt, the
/// `main.rs` drag clamp): 260 − 24 = 236. A single
/// button wider than this could not be placed by
/// wrapping at any column width, so every button in
/// the card must individually fit it.
pub const CARD_CONTENT_W_MIN: f32 = 236.0;

/// Conservative advance estimate for one character of a
/// `chrome_view::btn` label at its LABEL font (12 pt,
/// weight 500) — the same per-class upper-bound model
/// `fx_browser::preset_char_width` established against
/// measured tree text ("Shock ring" measures 61.5 pt
/// for ten characters; the model charges 66), duplicated
/// here because that fn is private to its module. A row
/// the model passes truly fits; the model may call a
/// borderline row over, never under. Non-ASCII
/// characters are charged the full em (12 pt).
fn label_char_width(c: char) -> f32 {
    match c {
        ' ' => 4.0,
        'i' | 'l' | 'j' | 't' | 'f' | '.' | ',' | ':' | ';' | '!' | '|' | '\'' | '`' => 4.0,
        '-' | '_' | '(' | ')' | '[' | ']' | '{' | '}' => 6.0,
        'm' | 'w' => 10.0,
        'M' | 'W' => 12.0,
        '0'..='9' => 7.0,
        'A'..='Z' => 9.0,
        'a'..='z' => 7.0,
        _ if c.is_ascii() => 7.0,
        _ => 12.0,
    }
}

/// The estimated painted width of a `btn` label, per
/// [`label_char_width`].
pub fn button_label_width(label: &str) -> f32 {
    label.chars().map(label_char_width).sum()
}

/// The estimated full width of one `chrome_view::btn`:
/// label plus the button's horizontal padding
/// (10 pt a side, `chrome_view::btn`).
pub fn button_width(label: &str) -> f32 {
    button_label_width(label) + 20.0
}

// ---- Theme ----

/// Set the theme on the core's real field
/// (`EditorCore.light_theme`, `editor.rs:63`): `true` =
/// light, `false` = dark. Idempotent — pressing the already
/// active theme's button changes nothing.
pub fn set_theme(core: &mut EditorCore, light: bool) {
    core.light_theme = light;
}

// ---- Language ----

/// One pickable language row of the picker: element key,
/// button label (the language's native name) and the value
/// it applies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LangEntry {
    pub key: String,
    pub label: &'static str,
    pub lang: Lang,
}

/// All languages of the i18n table, in picker order, derived
/// from `Lang::all()` / `Lang::label()` (`i18n.rs:150` /
/// `i18n.rs:116`) — the same source
/// `rune_sec::i18n_wire::language_options` reads, so a 29th
/// language appears here automatically.
pub fn language_entries() -> Vec<LangEntry> {
    Lang::all()
        .iter()
        .map(|l| LangEntry {
            key: format!("settings-lang-{}", l.code()),
            label: l.label(),
            lang: *l,
        })
        .collect()
}

/// Apply a picked language: set the pub field
/// `EditorCore.lang` (`editor.rs:82`) and write the status
/// line the core's own `cycle_lang` writes
/// (`editor.rs:1906`), so the feedback reads identically
/// whichever path changed the language.
pub fn set_language(core: &mut EditorCore, lang: Lang) {
    core.lang = lang;
    core.status = format!("Language: {}", lang.label());
}

// ---- Stage size ----

/// The stage size after stepping `stage` by `(dw, dh)`, each
/// axis clamped to [`STAGE_MIN`]..=[`STAGE_MAX`] — the pure
/// half of the GPUI shell's `adjust_stage`.
pub fn stepped_stage_size(stage: [f32; 2], dw: f32, dh: f32) -> [f32; 2] {
    [
        (stage[0] + dw).clamp(STAGE_MIN, STAGE_MAX),
        (stage[1] + dh).clamp(STAGE_MIN, STAGE_MAX),
    ]
}

/// Step the stage size on the real field (`Project.stage`,
/// `model.rs:685`) and report the new size in the status
/// line, byte-for-byte the GPUI shell's `adjust_stage`
/// behaviour. There is no core op for this; the field is
/// pub and the renderer consumes it directly.
pub fn apply_stage_step(core: &mut EditorCore, dw: f32, dh: f32) {
    // Wave 13 (census E6): same ruling as the scene
    // panel's stage steppers (E5) — write + mark only on
    // a real change, so the count never depends on which
    // surface stepped the stage.
    let next = stepped_stage_size(core.project.stage, dw, dh);
    if next != core.project.stage {
        core.project.stage = next;
        core.mark_dirty();
    }
    core.status = format!(
        "Stage size: {} x {}.",
        core.project.stage[0] as i32, core.project.stage[1] as i32
    );
}

// ---- Default easing ----

/// The easing cycle button's label: the project default's
/// current value (`Project.easing`, `model.rs:696`;
/// `Easing::label`, `model.rs:582`). The button itself calls
/// `EditorCore::cycle_easing` (`editor.rs:1686`) — this
/// helper exists so the label format is testable.
pub fn easing_label(core: &EditorCore) -> String {
    format!("Easing: {}", core.project.easing.label())
}

// ---- Guides ----

/// Flip the stage-guides flag on the shell state
/// (`State.guides`, added by the integrator — default
/// `false`). The flag lives in the shell, not the core:
/// guides are a view overlay (the stage draws the thirds
/// grid + centre cross while it is set), with no engine
/// state behind them. This toggle is the flag's only
/// writer.
pub fn toggle_guides(s: &mut State) {
    s.guides = !s.guides;
}

/// Flip the camera-frame guide flag (`State.guides_frustum`,
/// seeded `true` by the integrator) — the child toggle of
/// Guides for the camera frustum overlay. Same shape as
/// [`toggle_guides`]: a pure shell-state flip; the stage
/// composes it with `guides` when drawing (the overlay
/// shows only while Guides is also on), so this fn touches
/// nothing else.
pub fn toggle_guides_frustum(s: &mut State) {
    s.guides_frustum = !s.guides_frustum;
}

/// Flip the safe-frames guide flag (`State.guides_safe`,
/// seeded `true` by the integrator) — the child toggle of
/// Guides for the action/title-safe frame overlay. Same
/// contract as [`toggle_guides_frustum`].
pub fn toggle_guides_safe(s: &mut State) {
    s.guides_safe = !s.guides_safe;
}

// ---- Layout reset ----

/// Drive the shell's layout back to its defaults in one
/// press (Phase K): the Animate workspace through
/// `select_workspace` — so the reset is also the
/// persisted startup default, via the one persistence
/// path — both side panels shown (`State.show_left` /
/// `State.show_right`, integrator-seeded `true`), and
/// every guide flag on (`State.guides`,
/// `State.guides_frustum`, `State.guides_safe`, seeded
/// `false` / `true` / `true`).
///
/// The guide fields are SET, never flipped through the
/// toggle fns above: a toggle would drive a flag that
/// already sits at its default away from it, so a reset
/// built from flips could end half-inverted.
///
/// Deliberately NOT touched:
/// * **Theme** — the dark/light state is persisted
///   outside `State` (`LayoutPrefs.dark`), and `main`'s
///   poll owns the write-back from `core.light_theme`;
///   resetting it from here would fight that machinery,
///   and a layout reset has no business changing the
///   user's theme anyway. (The module docs carry the
///   same exclusion.)
/// * **Playback state** — `State.loop_playback`, the
///   playhead, whether playback is running: transport
///   state, not layout.
/// * **Project data** — nothing on `EditorCore` or
///   `Project` is written; `select_workspace` itself
///   touches only `State.workspace` and the layout
///   prefs.
pub fn reset_layout(s: &mut State) {
    select_workspace(s, Workspace::Animate);
    s.show_left = true;
    s.show_right = true;
    s.guides = true;
    s.guides_frustum = true;
    s.guides_safe = true;
}

// ---- Playback ----

/// Flip the loop-playback flag on the shell state
/// (`State.loop_playback`, seeded `true` by the
/// integrator). The body is byte-for-byte the transport's
/// `pb-loop` handler (playback_controls.rs): a bare flip
/// of the shell flag — no core call, no status change, no
/// undo snapshot. The shell's `poll` reads the flag to
/// decide whether the playhead wraps or stops at the end,
/// and the command palette's Toggle loop playback command
/// flips the same field; this settings toggle is a second
/// home for the one switch.
pub fn toggle_loop_playback(s: &mut State) {
    s.loop_playback = !s.loop_playback;
}

/// The playback-audio readout line: `"Playback audio:
/// on"` while the engine's player is running, `"Playback
/// audio: off"` otherwise. Pure formatting over the bool
/// `EditorCore::playback_audio_active` (`audio_play.rs`)
/// returns — that fn reports the `ffplay` process's
/// liveness, not audibility, so on a machine with no
/// audio device the line honestly reads off even after a
/// successful start (its own docs say the same).
pub fn audio_state_line(active: bool) -> &'static str {
    if active {
        "Playback audio: on"
    } else {
        "Playback audio: off"
    }
}

/// A speed chip's label: the bare value with the
/// transport cycle button's own trim — `1.0` renders
/// "1×", never "1.0×" — minus that button's "Speed "
/// prefix. The trim is `speed_label`'s exact code
/// (`playback_controls.rs`): format at two decimals,
/// strip trailing zeros, strip a trailing dot. A test
/// pins the two formatters together over the whole
/// `SPEEDS` table, so the chips can never drift from
/// the button they mirror.
pub fn speed_chip_label(speed: f32) -> String {
    let digits = format!("{speed:.2}");
    let trimmed = digits.trim_end_matches('0').trim_end_matches('.');
    format!("{trimmed}×")
}

/// The `SPEEDS` index of the shell's current playback
/// speed — the chip the speed row highlights — or
/// `None` when the field holds an off-table value (then
/// no chip highlights). Exact `f32` equality is sound
/// here: the field's only production writers are the
/// integrator's `1.0` seed (`main.rs`, itself a table
/// entry) and `cycle_playback_speed`, which writes
/// `next_speed`'s result — always a `SPEEDS` entry
/// (verified by grepping the tree for the field's
/// assignments; the remaining writes are test State
/// literals at `1.0` and one status_strip test
/// assigning `2.0`, itself a table entry).
pub fn active_speed_index(speed: f32) -> Option<usize> {
    SPEEDS.iter().position(|&step| step == speed)
}

/// Set the shell's playback speed outright — one chip
/// press of the speed row. A bare field write, silent
/// exactly like the transport's `pb-speed` cycle
/// (`cycle_playback_speed`): no core call, no status
/// line, no undo snapshot. The shell's `poll` reads the
/// field every tick — it scales `advance_play` by it
/// and notes it into the audio engine
/// (`note_playback_speed`) — so no call is needed from
/// here for the new rate to take effect.
pub fn set_playback_speed(s: &mut State, speed: f32) {
    s.playback_speed = speed;
}

// ---- Live MCP control ----

/// Switch live MCP control to the opposite of its current
/// state — one press of the settings card's Live MCP
/// toggle. The bridge itself is the state
/// (`crate::live_mcp::is_enabled`); there is deliberately
/// no mirrored bool in shell state to drift from it.
///
/// `set_enabled` returns the RESULTING state, and that —
/// not the wish — is what gets persisted to
/// `LayoutPrefs.live_mcp` (load-modify-save, the pattern
/// `main`'s poll uses for theme flips) and reported on the
/// status line:
/// * started: `"Live MCP on — {endpoint}"`, so the user
///   can point their MCP client at it;
/// * asked for on but still off: the bind failed — the
///   status names the port and the likely cause (another
///   editor instance already holds it);
/// * stopped: `"Live MCP off"`.
pub fn toggle_live_mcp(s: &mut State) {
    let want_on = !crate::live_mcp::is_enabled();
    let on = crate::live_mcp::set_enabled(want_on);
    // Wave 11 (A4): record the attempt's outcome — the
    // section below reads it to show a persistent failure
    // note after a failed start (the status line alone fades).
    stykanimate_rs::shell::note_live_mcp_start(
        if on {
            stykanimate_rs::shell::LiveMcpStartOutcome::Listening
        } else if want_on {
            stykanimate_rs::shell::LiveMcpStartOutcome::FailedToStart
        } else {
            stykanimate_rs::shell::LiveMcpStartOutcome::Stopped
        },
    );
    let mut prefs = stykanimate_rs::shell::load_layout();
    prefs.live_mcp = on;
    stykanimate_rs::shell::save_layout(&prefs);
    s.core.status = if on {
        format!("Live MCP on — {}", crate::live_mcp::endpoint())
    } else if want_on {
        format!(
            "Live MCP could not start — port {} is in use (another editor instance?).",
            crate::live_mcp::PORT
        )
    } else {
        "Live MCP off".to_string()
    };
}

/// The note line under the Live MCP toggle: the endpoint
/// an MCP client should be pointed at, and the plain-words
/// warning the bridge's own module docs carry — while live
/// control is on, any program running on this computer can
/// control the editor itself, workspaces and playback
/// included (the live-only shell tools reach the window,
/// not just the project), because this version has no
/// password. Shown in both states: the endpoint is where
/// the client goes as soon as the toggle is flipped on.
pub fn live_mcp_note() -> String {
    format!(
        "{} — while on, any program on this computer can control the editor, including switching workspaces and pressing play. No password in this version.",
        crate::live_mcp::endpoint()
    )
}

// ---- Startup workspace ----

/// Workspace buttons per row in the picker grid (the
/// language grid's width — eight workspaces land as two
/// rows of four).
pub const WORKSPACE_COLUMNS: usize = 4;

/// One pickable workspace row of the picker: element key,
/// button label and the value it applies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceEntry {
    pub key: String,
    pub label: &'static str,
    pub ws: Workspace,
}

/// The persisted name a workspace is stored under in
/// `LayoutPrefs.workspace` — `Workspace::name`
/// (`workspaces.rs`), exposed here so the picker's
/// round-trip contract (name → prefs → `from_prefs` →
/// same workspace) is testable from this file.
pub fn persisted_workspace_name(ws: Workspace) -> &'static str {
    ws.name()
}

/// A workspace button's element key:
/// `settings-ws-{name}` over the lowercased persisted
/// name (`settings-ws-animate` … `settings-ws-3d` —
/// Build3D persists as `"3D"`, hence the `3d` suffix).
pub fn workspace_key(ws: Workspace) -> String {
    format!("settings-ws-{}", ws.name().to_lowercase())
}

/// All eight workspaces, in picker order, derived from
/// `Workspace::all()` / `Workspace::label()`
/// (`workspaces.rs`) — the same source the chip row
/// reads, so the picker can never drift from it.
pub fn workspace_entries() -> Vec<WorkspaceEntry> {
    Workspace::all()
        .into_iter()
        .map(|ws| WorkspaceEntry {
            key: workspace_key(ws),
            label: ws.label(),
            ws,
        })
        .collect()
}

/// Apply a picked workspace: switch the shell's current
/// workspace AND persist it as the startup default,
/// through `workspaces::select_workspace` — the exact fn
/// the workspace chip row and the Alt+1..8 keys run
/// (sets `State.workspace`, then load-modify-saves
/// `LayoutPrefs` with the new name, preserving every
/// other prefs field). This card adds no second
/// persistence path.
pub fn set_startup_workspace(s: &mut State, ws: Workspace) {
    select_workspace(s, ws);
}

// ---- View ----

/// Flip the settings card open/closed (the toggle button's
/// handler).
pub fn toggle_open(s: &mut State) {
    s.settings.open = !s.settings.open;
}

/// A sentence-long prose note inside the card (the
/// guides-composition note, the startup-workspace note).
/// NOT a heading: headings render through the design
/// kit's `section_label` (Phase N); these two lines are
/// body prose and keep the muted LABEL rendering they
/// have always had. They wrap at the card's width
/// (`.wrap().w_full()`, the rune-kit card precedent):
/// unwrapped, the guides note ran off the card's right
/// edge mid-sentence (UI mission S19).
fn note_line(t: &Theme, label: &str) -> El {
    text(label)
        .font(type_::LABEL)
        .color(t.text_secondary)
        .wrap()
        .w_full()
}

/// One language button: applies its language via
/// [`set_language`]; highlighted when it is the core's
/// current language.
fn language_button(entry: LangEntry, current: Lang, t: &Theme) -> El {
    let key = entry.key.clone();
    let lang = entry.lang;
    btn(&key, entry.label, lang == current, t, move |s: &mut State| {
        set_language(&mut s.core, lang);
    })
}

/// One workspace button: applies its workspace via
/// [`set_startup_workspace`] (switch + persist);
/// highlighted when it is the shell's current workspace.
fn workspace_button(entry: WorkspaceEntry, current: Workspace, t: &Theme) -> El {
    let key = entry.key.clone();
    let ws = entry.ws;
    btn(&key, entry.label, ws == current, t, move |s: &mut State| {
        set_startup_workspace(s, ws);
    })
}

/// One stage axis row: heading, `-` stepper, readout of the
/// current size, `+` stepper. `axis` is 0 for width, 1 for
/// height; the steppers move only that axis, through
/// [`apply_stage_step`] (real field + status).
fn stage_axis_row(s: &State, t: &Theme, axis: usize, heading: &str) -> El {
    let (down_key, up_key) = if axis == 0 {
        ("settings-stage-w-down", "settings-stage-w-up")
    } else {
        ("settings-stage-h-down", "settings-stage-h-up")
    };
    let (dw, dh) = if axis == 0 {
        (STAGE_STEP, 0.0)
    } else {
        (0.0, STAGE_STEP)
    };
    row()
        .gap(8.0)
        .center_y()
        .child(design::section_label(heading, t))
        .child(btn(down_key, "-", false, t, move |s: &mut State| {
            apply_stage_step(&mut s.core, -dw, -dh);
        }))
        .child(design::stepper_value(
            &format!("{}", s.core.project.stage[axis] as i32),
            t,
        ))
        .child(btn(up_key, "+", false, t, move |s: &mut State| {
            apply_stage_step(&mut s.core, dw, dh);
        }))
}

/// The Playback block's speed row: one chip per
/// `SPEEDS` entry (keys `settings-speed-{i}`, labels
/// from [`speed_chip_label`]), the current speed's chip
/// highlighted ([`active_speed_index`]). Plain `btn`,
/// the popover's picker idiom (the language / workspace
/// grids): the labels are the data. A press applies its
/// step through [`set_playback_speed`]. The row carries
/// the grids' wrap pair (`.w_full().flex_wrap()`,
/// Wave 10): the five chips measure 259 pt — one line
/// at the card's default width, wrapping instead of
/// overrunning at the narrowest column width.
fn speed_row(s: &State, t: &Theme) -> El {
    let active = active_speed_index(s.playback_speed);
    let mut chips = row()
        .key("settings-speed-row")
        .gap(6.0)
        .w_full()
        .flex_wrap();
    for (i, &speed) in SPEEDS.iter().enumerate() {
        chips = chips.child(btn(
            &format!("settings-speed-{i}"),
            &speed_chip_label(speed),
            active == Some(i),
            t,
            move |s: &mut State| {
                set_playback_speed(s, speed);
            },
        ));
    }
    chips
}

/// The section as mounted in the shell column: the
/// "Settings" toggle (accent-filled while open), and — when
/// `s.settings.open` — the settings card under it, in
/// display order: the Live MCP toggle first (Wave 9 — the
/// card's most consequential switch, previously ~1064 pt
/// deep at the bottom of the scroll), Theme, Stage W / H
/// steppers, the default easing cycle, the stage-guides
/// toggle with its two child toggles, the Reset layout
/// button, the Playback block (loop toggle, speed chips +
/// playback-audio readout), the startup-workspace picker,
/// and the Language grid last (Wave 9 — set-once, and the
/// card's tallest section).
pub fn settings_popover(s: &State, t: &Theme) -> El {
    let core = &s.core;
    let open = s.settings.open;
    let mut panel = col().key("settings").gap(6.0);
    // The card toggle is a panel-visibility toggle: the
    // `eye` glyph, label and key unchanged (Phase H).
    panel = panel.child(icon_btn(
        "settings-toggle",
        "eye",
        "Settings",
        open,
        t,
        |s: &mut State| {
            toggle_open(s);
        },
    ));
    if open {
        // The card root is the kit's panel shell (Phase
        // N): radius 12, padding 12, card fill + the
        // hairline `card_border` the hand-rolled card
        // lacked (gap stays 8 — panel_col's own). The
        // M2 entrance transition (P15) rides on the card:
        // the card is the element that mounts/unmounts
        // with `open` (the outer `settings` col and its
        // toggle never leave the column), so its presence
        // fade becomes a fade + 4 pt rise on the SNAP
        // spring pair (`pair::SNAP` — 0.25 s in /
        // 0.20 s out, zero bounce).
        let mut card = design::panel_col("settings-card", t).transition(
            Transition::offset(0.0, -4.0)
                .combined(Transition::opacity())
                .springs(tok::motion::pair::SNAP),
        );

        // Card header (Phase H icon row, Phase N kit
        // form): the kit's `panel_title` — gear glyph +
        // "Settings" — over a hairline divider (charter
        // §2). The header row has never carried a key
        // and invents none now: it is a plain row wearing
        // `header_row`'s layout (full width, gap 8,
        // centre-aligned) rather than the keyed builder.
        card = card
            .child(
                row()
                    .w_full()
                    .gap(8.0)
                    .center_y()
                    .child(design::panel_title(Some("gear"), "Settings", None, t)),
            )
            .child(design::divider(t));

        // Live MCP control — FIRST section of the card
        // (Wave 9): it is the card's most consequential
        // switch (it opens the editor to local programs),
        // and it used to sit at the very bottom of the
        // scrolling card (~1064 pt deep), below every
        // set-once preference. A kit `toggle_row` like the
        // guides / loop rows below (the pill carries the
        // state), mounted directly — `settings-live-mcp`
        // is the one new key this section adds. The row's
        // state reads the bridge itself — `live_mcp::
        // is_enabled`, never a mirrored bool — and the
        // press runs through toggle_live_mcp (start/stop +
        // persist + status). Under it, the note line: the
        // endpoint for the user's MCP client and the
        // plain-words no-password warning.
        card = card
            .child(design::section_label("Live MCP", t))
            .child(design::toggle_row(
                "settings-live-mcp",
                "Live MCP control",
                crate::live_mcp::is_enabled(),
                t,
                |s: &mut State| {
                    toggle_live_mcp(s);
                },
            ))
            .child(note_line(t, &live_mcp_note()));

        // Wave 11 (A4): a failed start leaves a persistent
        // note under the section — the toggle reads off and
        // the status line has faded by the time the user comes
        // back to this card, so without it a dead bridge was
        // indistinguishable from a stopped one.
        //
        // Wave 14 (D1, landed at integration): when the
        // bridge itself recorded a start error, the note
        // names it — the OS reason it could not bind — in
        // place of the port guess. The shell-outcome-only
        // failure (the init-time attempt, which predates
        // the bridge's own record) keeps the guess as its
        // fallback wording.
        if stykanimate_rs::shell::live_mcp_start_outcome()
            == stykanimate_rs::shell::LiveMcpStartOutcome::FailedToStart
            && !crate::live_mcp::is_enabled()
        {
            let note = match crate::live_mcp::last_start_error() {
                Some(error) => {
                    format!("Live MCP did not start: {error}. Turn the toggle on to retry.")
                }
                None => format!(
                    "Live MCP did not start — port {} may be in use. Turn it off and on to retry.",
                    crate::live_mcp::PORT
                ),
            };
            card = card.child(note_line(t, &note));
        }

        // Theme: each button sets its own value outright.
        // The glyphs mirror the labels (Phase H): `moon`
        // names the dark state the Dark button sets, `sun`
        // the light state the Light button sets.
        let light = core.light_theme;
        card = card.child(design::section_label("Theme", t)).child(
            row()
                .key("settings-theme")
                .gap(6.0)
                .child(icon_btn(
                    "settings-theme-dark",
                    "moon",
                    "Dark",
                    !light,
                    t,
                    |s: &mut State| {
                        set_theme(&mut s.core, false);
                    },
                ))
                .child(icon_btn(
                    "settings-theme-light",
                    "sun",
                    "Light",
                    light,
                    t,
                    |s: &mut State| {
                        set_theme(&mut s.core, true);
                    },
                )),
        );

        // Stage size: one stepper row per axis.
        card = card
            .child(design::section_label("Stage size", t))
            .child(stage_axis_row(s, t, 0, "W"))
            .child(stage_axis_row(s, t, 1, "H"));

        // Default easing: the cycle op, labelled with the
        // current value (section 10's panel offers the same
        // op under key `ease-project-cycle`).
        card = card.child(design::section_label("Default easing", t)).child(
            row().key("settings-easing").gap(6.0).child(btn(
                "settings-easing-cycle",
                &easing_label(core),
                false,
                t,
                |s: &mut State| {
                    s.core.cycle_easing();
                },
            )),
        );

        // Stage guides: the shell flag (State.guides),
        // a kit `toggle_row` (Phase N) whose pill shows
        // the state. The stage overlay draws the grid;
        // nothing in the engine reads the flag. The
        // keyed wrapper row is the row's existing shell;
        // the toggle row grows inside it so the pill
        // sits at the card's right edge.
        card = card.child(
            row().key("settings-guides-row").gap(6.0).child(
                design::toggle_row(
                    "settings-guides",
                    "Guides",
                    s.guides,
                    t,
                    |s: &mut State| {
                        toggle_guides(s);
                    },
                )
                .grow(1.0),
            ),
        );

        // Guide children: the camera-frame and safe-frames
        // overlays, each with its own flag (State.guides_
        // frustum / guides_safe). Indented and dimmed as
        // children of Guides; they stay enabled while
        // Guides is off (the flags are independent state)
        // — the note under the row states that the stage
        // only draws them while Guides is on.
        card = card
            .child(
                row()
                    .key("settings-guides-children")
                    .gap(6.0)
                    .pl(14.0)
                    .opacity(0.85)
                    .child(
                        design::toggle_row(
                            "settings-guides-frustum",
                            "Camera frame",
                            s.guides_frustum,
                            t,
                            |s: &mut State| {
                                toggle_guides_frustum(s);
                            },
                        )
                        .grow(1.0),
                    )
                    .child(
                        design::toggle_row(
                            "settings-guides-safe",
                            "Safe frames",
                            s.guides_safe,
                            t,
                            |s: &mut State| {
                                toggle_guides_safe(s);
                            },
                        )
                        .grow(1.0),
                    ),
            )
            .child(note_line(
                t,
                "Stage guides: thirds grid + centre cross. \
                 Camera frame and safe frames show only while Guides is on.",
            ));

        // Layout reset (Phase K): one press back to the
        // layout defaults — the Animate workspace
        // (persisted through select_workspace), both side
        // panels shown, every guide flag on. An action,
        // not a state toggle, so it renders unhighlighted;
        // theme, playback state and the project are
        // deliberately outside its scope (reset_layout's
        // docs carry the full contract).
        card = card.child(
            row().key("settings-reset-row").gap(6.0).child(icon_btn(
                "settings-reset-layout",
                "arrow-rotate-left",
                "Reset layout",
                false,
                t,
                |s: &mut State| {
                    reset_layout(s);
                },
            )),
        );

        // Playback (Phase I): the loop toggle is a second
        // home for the transport's pb-loop switch — same
        // shell flag (State.loop_playback), same bare flip
        // (toggle_loop_playback), rendered as a kit
        // `toggle_row` like the guides toggles above
        // (Phase N; the pill carries the state the
        // `repeat` glyph used to signal). Under it, a
        // muted readout of the audio player's liveness —
        // a pure readout with no
        // handler (audio_state_line over the core's
        // playback_audio_active). Between them sits the
        // speed row (Phase M): five direct-select chips
        // over the transport cycle's SPEEDS table
        // (speed_row), the current speed's chip lit.
        card = card
            .child(design::section_label("Playback", t))
            .child(
                row().key("settings-loop-row").gap(6.0).child(
                    design::toggle_row(
                        "settings-loop",
                        "Loop playback",
                        s.loop_playback,
                        t,
                        |s: &mut State| {
                            toggle_loop_playback(s);
                        },
                    )
                    .grow(1.0),
                ),
            )
            .child(speed_row(s, t))
            .child(
                text(audio_state_line(core.playback_audio_active()))
                    .key("settings-audio-state")
                    .font(type_::LABEL)
                    .color(t.text_secondary),
            );

        // Startup workspace: the eight workspaces as a
        // button grid (the language grid's layout), the
        // current one highlighted. Picking one switches
        // now and persists as the startup default via
        // select_workspace — see the module docs.
        let mut ws_grid = col().key("settings-ws-grid").gap(4.0);
        for (i, chunk) in workspace_entries()
            .chunks(WORKSPACE_COLUMNS)
            .enumerate()
        {
            // The wrap pair, as on the language rows
            // (Wave 10): the rows measure 258 / 222 pt —
            // one line each at the card's default 316 pt
            // content width, but the wider row passes the
            // 236 pt narrowest-column width, where it now
            // wraps instead of overrunning the card.
            let mut ws_row = row()
                .key(format!("settings-ws-row-{i}"))
                .gap(6.0)
                .w_full()
                .flex_wrap();
            for entry in chunk {
                ws_row = ws_row.child(workspace_button(entry.clone(), s.workspace, t));
            }
            ws_grid = ws_grid.child(ws_row);
        }
        card = card
            .child(design::section_label("Startup workspace", t))
            .child(ws_grid)
            .child(note_line(
                t,
                "Applies now and is the workspace opened at startup.",
            ));

        // Language — LAST section of the card (Wave 9):
        // the full table as a button grid, the current
        // language highlighted. It is a set-once
        // preference and by far the card's tallest section
        // (28 buttons); it used to sit second, pushing
        // every daily control — stage size, guides,
        // playback — hundreds of points down the scroll.
        // Moved whole: same grid keys, same buttons, same
        // handler ([`language_button`] → [`set_language`]).
        let mut grid = col().key("settings-lang-grid").gap(4.0);
        for (i, chunk) in language_entries().chunks(LANG_COLUMNS).enumerate() {
            // `.w_full()` is the load-bearing half of the
            // wrap pair (Wave 10): measured by
            // [`button_width`], every 4-button row runs
            // 16–127 pt past the card's 316 pt content
            // width, and `.flex_wrap()` alone cannot
            // engage — a row with no definite width sizes
            // to its content, so there is never anything
            // to wrap against. `fx_browser`'s preset rows
            // carry the pair for exactly this reason.
            // Pinned to the card width, each row now wraps
            // its later buttons onto a second line inside
            // the card; every language stays selectable
            // under its own key.
            let mut lang_row = row()
                .key(format!("settings-lang-row-{i}"))
                .gap(6.0)
                .w_full()
                .flex_wrap();
            for entry in chunk {
                lang_row = lang_row.child(language_button(entry.clone(), core.lang, t));
            }
            grid = grid.child(lang_row);
        }
        card = card.child(design::section_label("Language", t)).child(grid);

        panel = panel.child(card);
    }
    panel
}

#[cfg(test)]
mod tests {
    use super::*;

    fn core() -> EditorCore {
        EditorCore::new()
    }

    #[test]
    fn state_defaults_closed() {
        assert!(!SettingsState::default().open);
    }

    #[test]
    fn set_theme_writes_the_core_field_both_ways() {
        let mut core = core();
        // EditorCore::new defaults to the dark theme.
        assert!(!core.light_theme);
        set_theme(&mut core, true);
        assert!(core.light_theme);
        // Idempotent: setting the active theme again is a no-op.
        set_theme(&mut core, true);
        assert!(core.light_theme);
        set_theme(&mut core, false);
        assert!(!core.light_theme);
    }

    #[test]
    fn language_entries_cover_the_whole_table() {
        let entries = language_entries();
        assert_eq!(entries.len(), 28);
        assert_eq!(entries.len(), Lang::all().len());
        assert_eq!(entries[0].lang, Lang::En);
        assert_eq!(entries[0].label, "English");
        assert_eq!(entries[0].key, "settings-lang-en");
        assert!(entries
            .iter()
            .any(|e| e.lang == Lang::Bn && e.label == "বাংলা"));
        // Keys unique (headless --tree lookups depend on it).
        let mut keys: Vec<&str> = entries.iter().map(|e| e.key.as_str()).collect();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), 28);
    }

    #[test]
    fn set_language_sets_field_and_cycle_lang_status() {
        let mut core = core();
        set_language(&mut core, Lang::De);
        assert_eq!(core.lang, Lang::De);
        // Same status text cycle_lang (editor.rs:1906) writes.
        assert_eq!(core.status, "Language: Deutsch");
        set_language(&mut core, Lang::En);
        assert_eq!(core.lang, Lang::En);
        assert_eq!(core.status, "Language: English");
    }

    #[test]
    fn stage_step_moves_one_axis_by_the_gpui_step() {
        assert_eq!(stepped_stage_size([960.0, 720.0], STAGE_STEP, 0.0), [1000.0, 720.0]);
        assert_eq!(stepped_stage_size([960.0, 720.0], -STAGE_STEP, 0.0), [920.0, 720.0]);
        assert_eq!(stepped_stage_size([960.0, 720.0], 0.0, STAGE_STEP), [960.0, 760.0]);
        assert_eq!(STAGE_STEP, 40.0);
    }

    #[test]
    fn stage_step_clamps_to_the_gpui_range() {
        assert_eq!(
            stepped_stage_size([4000.0, 200.0], STAGE_STEP, -STAGE_STEP),
            [4000.0, 200.0]
        );
        assert_eq!(
            stepped_stage_size([210.0, 3990.0], -STAGE_STEP, STAGE_STEP),
            [200.0, 4000.0]
        );
        assert_eq!((STAGE_MIN, STAGE_MAX), (200.0, 4000.0));
    }

    #[test]
    fn apply_stage_step_writes_field_and_gpui_status() {
        let mut core = core();
        core.project.stage = [960.0, 720.0];
        apply_stage_step(&mut core, STAGE_STEP, 0.0);
        assert_eq!(core.project.stage, [1000.0, 720.0]);
        // Byte-for-byte the GPUI shell's adjust_stage status.
        assert_eq!(core.status, "Stage size: 1000 x 720.");
        apply_stage_step(&mut core, 0.0, -STAGE_STEP);
        assert_eq!(core.project.stage, [1000.0, 680.0]);
        assert_eq!(core.status, "Stage size: 1000 x 680.");
    }

    #[test]
    fn easing_label_names_the_current_project_default() {
        let mut core = core();
        // Project::new_demo defaults to Easing::Linear.
        assert_eq!(easing_label(&core), "Easing: linear");
        // The same op the button calls advances the label.
        core.cycle_easing();
        assert_eq!(easing_label(&core), "Easing: smooth");
        core.cycle_easing();
        assert_eq!(easing_label(&core), "Easing: ease-in");
        core.cycle_easing();
        assert_eq!(easing_label(&core), "Easing: ease-out");
        core.cycle_easing();
        assert_eq!(easing_label(&core), "Easing: linear");
    }

    #[test]
    fn guides_toggle_flips_the_shell_field() {
        let t = rune::tok::theme(true);
        // Full shell State, built the way the sibling
        // sections' panel tests build it — guides starts
        // off (the integrator's init default).
        let mut state = State {
            core: EditorCore::new(),
            last_poll: None,
            wheel_zoom: 0.0,
            figures_panel: crate::rune_sec::figures_panel::FiguresPanelState::default(),
            figure_add: crate::rune_sec::figure_add::FigureAddState::default(),
            library_panel: crate::rune_sec::library_panel::LibraryPanelState::default(),
            sprites_panel: crate::rune_sec::sprites_panel::SpritesPanelState::default(),
            home_panel: crate::rune_sec::home_panel::HomePanelState::default(),
            frame_ops: crate::rune_sec::frame_ops::FrameOpsState::default(),
            graph_panel: crate::rune_sec::graph_panel::GraphPanelState::default(),
            audio_panel: crate::rune_sec::audio_panel::AudioPanelState::new(),
            menus: crate::rune_sec::menus::MenusState::default(),
            palette: crate::rune_sec::command_palette::PaletteState::new(),
            settings: SettingsState::default(),
            toasts: crate::rune_sec::toasts::ToastsState::new(),
            text_tool: crate::rune_sec::text_tool::TextToolState::new(),
            name_edit: rune::TextEdit::new(""),
            export_use_range: false,
            fx_selected: None,
            pose_clipboard: None,
            guides: false,
            last_status: String::new(),
            toast_now: 0.0,
            last_dark: true,
            workspace: crate::rune_sec::workspaces::Workspace::Animate,
            show_left: true,
            show_right: true,
            key_clipboard: None,
            export_recap: String::new(),
            inspector_figure: crate::rune_sec::inspector_figure::InspectorFigureEditState::new(),
            clips_rename: crate::rune_sec::clips_panel::ClipRenameState::new(),
            fx_browser: crate::rune_sec::fx_browser::FxBrowserState::default(),
            draw_tools: crate::rune_sec::draw_tools::DrawToolsState::default(),
stroke_selected: None,
particle_selected: None,
guides_frustum: true,
guides_safe: true,
onion_ui: Default::default(),
last_frame: 0,
pose_rename: crate::rune_sec::pose_tools::PoseRenameState::new(),
pose_library: crate::rune_sec::pose_tools::PoseLibraryState::new(),
loop_playback: true,
playback_speed: 1.0,
            was_playing: false,
            confirm_action: None,
            home_rename: crate::rune_sec::home_panel::HomeRenameState::new(),
        };
        assert!(!state.guides);
        // The card builds with guides off and on (the open
        // card is where the toggle renders).
        state.settings.open = true;
        let _ = settings_popover(&state, &t);
        toggle_guides(&mut state);
        assert!(state.guides);
        let _ = settings_popover(&state, &t);
        toggle_guides(&mut state);
        assert!(!state.guides);
    }

    /// A full shell State with every current field, for
    /// the Phase G tests (the guides test above keeps its
    /// own inline literal).
    fn shell_state() -> State {
        State {
            core: EditorCore::new(),
            last_poll: None,
            wheel_zoom: 0.0,
            figures_panel: crate::rune_sec::figures_panel::FiguresPanelState::default(),
            figure_add: crate::rune_sec::figure_add::FigureAddState::default(),
            library_panel: crate::rune_sec::library_panel::LibraryPanelState::default(),
            sprites_panel: crate::rune_sec::sprites_panel::SpritesPanelState::default(),
            home_panel: crate::rune_sec::home_panel::HomePanelState::default(),
            frame_ops: crate::rune_sec::frame_ops::FrameOpsState::default(),
            graph_panel: crate::rune_sec::graph_panel::GraphPanelState::default(),
            audio_panel: crate::rune_sec::audio_panel::AudioPanelState::new(),
            menus: crate::rune_sec::menus::MenusState::default(),
            palette: crate::rune_sec::command_palette::PaletteState::new(),
            settings: SettingsState::default(),
            toasts: crate::rune_sec::toasts::ToastsState::new(),
            text_tool: crate::rune_sec::text_tool::TextToolState::new(),
            name_edit: rune::TextEdit::new(""),
            export_use_range: false,
            fx_selected: None,
            pose_clipboard: None,
            guides: false,
            last_status: String::new(),
            toast_now: 0.0,
            last_dark: true,
            workspace: crate::rune_sec::workspaces::Workspace::Animate,
            show_left: true,
            show_right: true,
            key_clipboard: None,
            export_recap: String::new(),
            inspector_figure: crate::rune_sec::inspector_figure::InspectorFigureEditState::new(),
            clips_rename: crate::rune_sec::clips_panel::ClipRenameState::new(),
            fx_browser: crate::rune_sec::fx_browser::FxBrowserState::default(),
            draw_tools: crate::rune_sec::draw_tools::DrawToolsState::default(),
            stroke_selected: None,
            particle_selected: None,
            guides_frustum: true,
            guides_safe: true,
            onion_ui: Default::default(),
            last_frame: 0,
            pose_rename: crate::rune_sec::pose_tools::PoseRenameState::new(),
            pose_library: crate::rune_sec::pose_tools::PoseLibraryState::new(),
            loop_playback: true,
            playback_speed: 1.0,
            was_playing: false,
            confirm_action: None,
            home_rename: crate::rune_sec::home_panel::HomeRenameState::new(),
        }
    }

    #[test]
    fn guide_child_toggles_flip_only_their_own_flag() {
        let t = rune::tok::theme(true);
        let mut state = shell_state();
        // Integrator seeds: parent off, both children on.
        assert!(!state.guides && state.guides_frustum && state.guides_safe);

        // Frustum flips alone — parent and sibling unmoved.
        toggle_guides_frustum(&mut state);
        assert!(!state.guides && !state.guides_frustum && state.guides_safe);
        toggle_guides_frustum(&mut state);
        assert!(!state.guides && state.guides_frustum && state.guides_safe);

        // Safe flips alone.
        toggle_guides_safe(&mut state);
        assert!(!state.guides && state.guides_frustum && !state.guides_safe);
        toggle_guides_safe(&mut state);
        assert!(!state.guides && state.guides_frustum && state.guides_safe);

        // The parent toggle never touches the children
        // (independent flags; the stage composes them).
        toggle_guides(&mut state);
        assert!(state.guides && state.guides_frustum && state.guides_safe);

        // The open card builds in mixed states (the child
        // toggles render regardless of the parent's state).
        state.settings.open = true;
        toggle_guides_frustum(&mut state);
        let _ = settings_popover(&state, &t);
        toggle_guides(&mut state);
        toggle_guides_safe(&mut state);
        let _ = settings_popover(&state, &t);
    }

    #[test]
    fn workspace_entries_cover_all_eight_workspaces() {
        let entries = workspace_entries();
        assert_eq!(entries.len(), 8);
        assert_eq!(entries.len(), Workspace::all().len());
        // Same order and values as Workspace::all().
        for (entry, ws) in entries.iter().zip(Workspace::all()) {
            assert_eq!(entry.ws, ws);
            assert_eq!(entry.label, ws.label());
            assert_eq!(entry.key, workspace_key(ws));
        }
        // Keys unique (headless --tree lookups depend on it).
        let mut keys: Vec<&str> = entries.iter().map(|e| e.key.as_str()).collect();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), 8);
        // Build3D persists as "3D": label "3D", key suffix 3d.
        let three_d = entries
            .iter()
            .find(|e| e.ws == Workspace::Build3D)
            .unwrap();
        assert_eq!(three_d.label, "3D");
        assert_eq!(three_d.key, "settings-ws-3d");
        assert_eq!(WORKSPACE_COLUMNS, 4);
    }

    #[test]
    fn workspace_names_round_trip_through_prefs() {
        use crate::rune_sec::workspaces::{from_prefs, prefs_with_workspace};
        use stykanimate_rs::shell::LayoutPrefs;

        // Every variant: workspace → persisted name →
        // prefs → from_prefs → the same workspace (the
        // exact load path the next launch seeds from).
        // Pure: prefs_with_workspace touches no disk, so
        // set_startup_workspace itself (which saves) is
        // not called here.
        for ws in Workspace::all() {
            let prefs = prefs_with_workspace(&LayoutPrefs::default(), ws);
            assert_eq!(prefs.workspace, persisted_workspace_name(ws));
            assert_eq!(from_prefs(&prefs), ws);
        }
        // The GPUI-era special case: Build3D's persisted
        // name is "3D", not its variant name.
        assert_eq!(persisted_workspace_name(Workspace::Build3D), "3D");
        assert_eq!(persisted_workspace_name(Workspace::Animate), "Animate");
    }

    #[test]
    fn loop_playback_toggle_flips_the_shell_field() {
        let t = rune::tok::theme(true);
        let mut state = shell_state();
        // Integrator seed: loop playback starts on — the
        // same state the transport's pb-loop shows.
        assert!(state.loop_playback);
        // The factored flip pb-loop's handler body performs.
        toggle_loop_playback(&mut state);
        assert!(!state.loop_playback);
        toggle_loop_playback(&mut state);
        assert!(state.loop_playback);
        // The open card builds with the toggle in both
        // states (it renders in the Playback block).
        state.settings.open = true;
        let _ = settings_popover(&state, &t);
        toggle_loop_playback(&mut state);
        let _ = settings_popover(&state, &t);
    }

    #[test]
    fn reset_layout_restores_every_layout_default() {
        use crate::rune_sec::workspaces::from_prefs;

        let t = rune::tok::theme(true);
        let mut state = shell_state();
        // The project facts the reset must not move.
        let name_before = state.core.project.name.clone();
        let frames_before = state.core.project.frames.len();

        // Scramble every field the reset covers, through
        // the shell's own paths where they exist: the
        // picker fn moves the workspace (and persists
        // Export); direct writes hide the panels and kill
        // the guides.
        set_startup_workspace(&mut state, Workspace::Export);
        state.show_left = false;
        state.show_right = false;
        state.guides = false;
        state.guides_frustum = false;
        state.guides_safe = false;
        // Two fields outside the reset's scope, stood away
        // from their seeds: the theme and the loop flag.
        state.core.light_theme = true;
        state.loop_playback = false;
        assert_eq!(state.workspace, Workspace::Export);
        assert!(!state.show_left && !state.show_right);
        assert!(!state.guides && !state.guides_frustum && !state.guides_safe);

        reset_layout(&mut state);

        // Every covered field back at its default.
        assert_eq!(state.workspace, Workspace::Animate);
        assert!(state.show_left && state.show_right);
        assert!(state.guides && state.guides_frustum && state.guides_safe);
        // The reset persisted: the layout prefs the next
        // launch seeds from name Animate. (The prefs file
        // is shared across the suite — the snap tests'
        // workspace test lives with the same file and
        // likewise leaves it naming Animate.)
        let prefs = stykanimate_rs::shell::load_layout();
        assert_eq!(prefs.workspace, "Animate");
        assert_eq!(from_prefs(&prefs), Workspace::Animate);
        // Out of scope, untouched: theme and playback.
        assert!(state.core.light_theme);
        assert!(!state.loop_playback);
        // Project data untouched.
        assert_eq!(state.core.project.name, name_before);
        assert_eq!(state.core.project.frames.len(), frames_before);

        // The open card builds in the reset state (the
        // Reset layout row renders between the guides
        // note and the Playback block).
        state.settings.open = true;
        let _ = settings_popover(&state, &t);
    }

    #[test]
    fn live_mcp_note_names_the_endpoint_and_the_warning() {
        let note = live_mcp_note();
        // The endpoint is the bridge's own, so the note can
        // never drift from the address the client must use.
        assert!(note.starts_with(&crate::live_mcp::endpoint()), "{note}");
        assert!(note.contains("http://127.0.0.1:32117/mcp"), "{note}");
        // The plain-words warning rides along: while on,
        // any local program can control the editor — and
        // since Wave 14's shell control that means the
        // window itself, so the note names workspaces and
        // playback outright rather than leaving "control"
        // to be read as project edits only.
        assert!(note.contains("any program on this computer"), "{note}");
        assert!(note.contains("can control the editor"), "{note}");
        assert!(
            note.contains("including switching workspaces and pressing play"),
            "{note}"
        );
        assert!(note.contains("No password in this version."), "{note}");
        // Deliberately NOT tested here: toggle_live_mcp —
        // it starts the real bridge (binds the port), and
        // the socket-binding proof lives in live_mcp's own
        // end-to-end test, the suite's only one.
    }

    #[test]
    fn audio_state_line_names_both_states() {
        assert_eq!(audio_state_line(true), "Playback audio: on");
        assert_eq!(audio_state_line(false), "Playback audio: off");
        // A fresh core has never started the player, so the
        // live readout the card renders is the off state —
        // the same assertion audio_play's own tests make.
        let core = core();
        assert!(!core.playback_audio_active());
        assert_eq!(
            audio_state_line(core.playback_audio_active()),
            "Playback audio: off"
        );
    }

    #[test]
    fn speed_chip_labels_cover_the_whole_speeds_table() {
        // The five chips, in SPEEDS order — the labels
        // the speed row renders.
        assert_eq!(SPEEDS.len(), 5);
        let expected = ["0.25×", "0.5×", "1×", "1.5×", "2×"];
        for (i, &speed) in SPEEDS.iter().enumerate() {
            assert_eq!(speed_chip_label(speed).as_str(), expected[i]);
        }
        // Pinned against the transport cycle button: the
        // chip label is speed_label's value text with the
        // "Speed " prefix removed, for every table entry.
        for &speed in &SPEEDS {
            assert_eq!(
                crate::rune_sec::playback_controls::speed_label(speed),
                format!("Speed {}", speed_chip_label(speed))
            );
        }
        // The trim itself, off-table: whole values lose
        // the fraction entirely, fractions keep exactly
        // the digits they need.
        assert_eq!(speed_chip_label(1.0), "1×");
        assert_eq!(speed_chip_label(0.5), "0.5×");
        assert_eq!(speed_chip_label(3.0), "3×");
    }

    #[test]
    fn active_speed_index_matches_table_entries_only() {
        for (i, &speed) in SPEEDS.iter().enumerate() {
            assert_eq!(active_speed_index(speed), Some(i));
        }
        // The integrator's seed is the middle entry.
        assert_eq!(active_speed_index(1.0), Some(2));
        // Off-table values highlight nothing (NaN orders
        // against nothing, so it is off-table too).
        assert_eq!(active_speed_index(0.75), None);
        assert_eq!(active_speed_index(3.0), None);
        assert_eq!(active_speed_index(f32::NAN), None);
    }

    #[test]
    fn set_playback_speed_writes_the_field_silently() {
        let t = rune::tok::theme(true);
        let mut state = shell_state();
        // Integrator seed: normal speed, the middle chip.
        assert_eq!(state.playback_speed, 1.0);
        assert_eq!(active_speed_index(state.playback_speed), Some(2));
        let status_before = state.core.status.clone();

        set_playback_speed(&mut state, SPEEDS[0]);
        assert_eq!(state.playback_speed, 0.25);
        assert_eq!(active_speed_index(state.playback_speed), Some(0));
        // Silent, like the pb-speed cycle: no status
        // churn — the field is the only thing that moved.
        assert_eq!(state.core.status, status_before);

        set_playback_speed(&mut state, SPEEDS[4]);
        assert_eq!(state.playback_speed, 2.0);
        assert_eq!(active_speed_index(state.playback_speed), Some(4));
        assert_eq!(state.core.status, status_before);

        // The open card builds at a non-default speed
        // (the speed row renders in the Playback block).
        state.settings.open = true;
        let _ = settings_popover(&state, &t);
    }

    // ---- Wave 10 width audit ----
    //
    // The figures below are the proof behind the
    // module docs' Wave 10 bullet: row widths are
    // ESTIMATES from the shell's conservative model
    // ([`button_width`] — fx_browser's per-class table,
    // an upper bound against measured tree text), so a
    // row the model passes truly fits, and a row it
    // fails was over by at least the stated margin in
    // the model's own units.

    /// Greedy flex-wrap packing, as Rune lays a
    /// `.flex_wrap()` row out: walk the items in order,
    /// starting a new line whenever the next item (plus
    /// the gap before it) would pass `budget`. Returns
    /// the packed line widths.
    fn pack_lines(widths: &[f32], gap: f32, budget: f32) -> Vec<f32> {
        let mut lines: Vec<f32> = Vec::new();
        let mut line = 0.0f32;
        for &w in widths {
            let add = if line == 0.0 { w } else { w + gap };
            if line + add > budget && line > 0.0 {
                lines.push(line);
                line = w;
            } else {
                line += add;
            }
        }
        if line > 0.0 {
            lines.push(line);
        }
        lines
    }

    #[test]
    fn card_content_widths_match_the_shell_layout() {
        // The default budget is not a free-floating
        // literal: right column default (LayoutPrefs)
        // minus the kit card's padding on both sides.
        let prefs = stykanimate_rs::shell::LayoutPrefs::default();
        assert_eq!(
            prefs.col_right_w - 2.0 * design::PANEL_PAD,
            CARD_CONTENT_W
        );
        // The narrowest budget: the splitter's 260 pt
        // clamp floor (main.rs's drag handler) minus the
        // same padding.
        assert_eq!(260.0 - 2.0 * design::PANEL_PAD, CARD_CONTENT_W_MIN);
    }

    #[test]
    fn button_width_model_matches_the_measured_anchor() {
        // fx_browser's calibration point: "Shock ring"
        // measures 61.5 pt in a real tree; the model
        // must charge at least that (it charges 66).
        assert_eq!(button_label_width("Shock ring"), 66.0);
        assert!(button_label_width("Shock ring") >= 61.5);
    }

    #[test]
    fn language_rows_overflow_unwrapped_and_wrap_inside_the_card() {
        let entries = language_entries();
        let widths: Vec<f32> = entries.iter().map(|e| button_width(e.label)).collect();
        // The widest single button fits even the
        // narrowest column — wrapping can always place
        // every button, so no label needs truncating.
        // (The widest is es-419's native name.)
        let widest = widths.iter().copied().fold(0.0f32, f32::max);
        assert_eq!(widest, 181.0);
        assert!(widest <= CARD_CONTENT_W_MIN, "{widest} must fit the narrowest card");

        // Per chunk row, in grid order: the one-line
        // width the row painted at before the fix (the
        // remainder, proven — every row is over), and
        // the wrapped packing the `.w_full()` row now
        // produces (every line inside the card).
        let expected_one_line = [340.0, 332.0, 416.0, 356.0, 377.0, 443.0, 392.0];
        for (i, chunk) in widths.chunks(LANG_COLUMNS).enumerate() {
            let one_line = chunk.iter().sum::<f32>() + 6.0 * (chunk.len() - 1) as f32;
            assert_eq!(one_line, expected_one_line[i], "row {i} one-line width");
            assert!(
                one_line > CARD_CONTENT_W,
                "row {i} is the proven overflow the wrap fixes"
            );
            for line in pack_lines(chunk, 6.0, CARD_CONTENT_W) {
                assert!(line <= CARD_CONTENT_W, "row {i} wraps inside the card: {line}");
            }
            // And at the narrowest column the same
            // buttons still pack inside the card.
            for line in pack_lines(chunk, 6.0, CARD_CONTENT_W_MIN) {
                assert!(line <= CARD_CONTENT_W_MIN, "row {i} at min width: {line}");
            }
        }
    }

    #[test]
    fn workspace_rows_fit_one_line_and_wrap_at_the_narrowest_column() {
        let entries = workspace_entries();
        let widths: Vec<f32> = entries.iter().map(|e| button_width(e.label)).collect();
        let expected_one_line = [258.0, 222.0];
        for (i, chunk) in widths.chunks(WORKSPACE_COLUMNS).enumerate() {
            let one_line = chunk.iter().sum::<f32>() + 6.0 * (chunk.len() - 1) as f32;
            assert_eq!(one_line, expected_one_line[i], "ws row {i}");
            assert!(one_line <= CARD_CONTENT_W, "ws row {i} fits the default card");
            for line in pack_lines(chunk, 6.0, CARD_CONTENT_W_MIN) {
                assert!(line <= CARD_CONTENT_W_MIN, "ws row {i} at min width: {line}");
            }
        }
    }

    #[test]
    fn speed_row_fits_one_line_and_wraps_at_the_narrowest_column() {
        let widths: Vec<f32> = SPEEDS.iter().map(|&v| button_width(&speed_chip_label(v))).collect();
        let one_line = widths.iter().sum::<f32>() + 6.0 * (widths.len() - 1) as f32;
        assert_eq!(one_line, 259.0);
        assert!(one_line <= CARD_CONTENT_W);
        for line in pack_lines(&widths, 6.0, CARD_CONTENT_W_MIN) {
            assert!(line <= CARD_CONTENT_W_MIN, "speed row at min width: {line}");
        }
    }
}
