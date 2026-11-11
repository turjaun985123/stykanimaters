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
//! * **Guide variant toggles (Roadmap Wave 2, W2-33)** —
//!   a second child row under the Phase G pair: "Golden
//!   ratio" flips `State.guides_golden` and "Diagonals"
//!   flips `State.guides_diagonals` (shell fields, default
//!   `false` — opt-in composition variants; RM18's
//!   IA-RM18-3 priced a default flip and it was explicitly
//!   NOT taken). They are children of Camera frame as
//!   well as of Guides: the stage draws them inside the
//!   framed view only while both are on.
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
//! * **Export defaults (Roadmap Wave 2, W2-62 — PL-8
//!   slice 1)** — three persisted choices the export
//!   panel uses as its defaults: the root directory
//!   (the historical split `export/` + `out/` roots, or
//!   one root for everything), the naming template
//!   riding RM23's project stems (the stem itself, the
//!   stem plus the export date, or the pre-RM23 shared
//!   `animation` stem), and the encoder quality level
//!   (W2-42's `EncodeQuality`, persisted as its `name()`
//!   — the persistence W2-42 explicitly deferred to
//!   this seat). All three live in the layout prefs
//!   (`LayoutPrefs.export_dir` / `export_naming` /
//!   `export_quality`, IA-W2-62-1) and follow the Live
//!   MCP discipline: the card reads them from
//!   `shell::load_layout()` when it renders and writes
//!   each press load-modify-save, so the prefs file is
//!   the only copy and nothing in shell state can drift
//!   from it. The export panel reads the same fields on
//!   every press (IA-W2-62-2 / IA-W2-62-3); until those
//!   splices land the choices persist but no export
//!   consumes them yet — the findings carry that split
//!   plainly. The example line under the rows previews
//!   the MP4 destination and quality the current
//!   choices produce, computed from the same helpers
//!   the consumer wiring uses.
//! * **Autosave & recovery (Roadmap Wave 2, W2-63 —
//!   PL-8 slice 2)** — the recovery writer's three
//!   persisted choices: whether it runs at all, how
//!   many counted edits accumulate between snapshots,
//!   and how many snapshot generations are retained.
//!   The writer itself predates the controls (Wave 9's
//!   25-edit cadence, hard-coded; a single
//!   `.recovery.json` overwritten each time), so the
//!   defaults are that exact behaviour — on, 25 edits,
//!   1 generation. The values live in the layout prefs
//!   (`LayoutPrefs.autosave_enabled` / `autosave_every`
//!   / `recovery_keep`, IA-W2-63-1) as a bool and two
//!   numbers, sanitised on every read through the lib's
//!   policy type (`stykanimate_rs::autosave::
//!   AutosavePrefs`, whose clamps the loader mirrors).
//!   The setters follow the Export defaults discipline
//!   — load-modify-save the resulting value — and
//!   additionally mirror the saved policy into
//!   `EditorCore`'s live copy (`core.autosave`,
//!   IA-W2-63-2), so a press takes effect in the
//!   running session, not just the next launch; the
//!   shell's `init` seeds that copy from the prefs
//!   (IA-W2-63-3). The status line under the rows
//!   reports the snapshot on disk as it stands — its
//!   age from `shell::recovery_modified`, its copy
//!   count from the generations actually present —
//!   never a claim the files do not back.
//! * **Startup workspace (Phase G)** — a compact picker
//!   (eight buttons, the language grid's 4-wide layout)
//!   naming the workspace the shell opens in. Picking one
//!   calls `workspaces::select_workspace` — the exact fn
//!   the workspace chip row and the Alt+1..9 keys run —
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
//!   by the same arithmetic (258 / 220 and 259), but
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
//! and language onto `LayoutPrefs.lang` (Roadmap F-9:
//! seeded into `EditorCore.lang` by `main.rs`'s `init`,
//! written back by `poll`'s shadow compare on change —
//! it was session-only before that field existed, and
//! an earlier version of this section wrongly said it
//! persisted with the project file), while stage size
//! and default easing live on the core/project and
//! persist with the project file itself. The
//! startup-workspace picker reuses the
//! EXISTING `LayoutPrefs.workspace` through
//! `workspaces::select_workspace` (above). The theme
//! flip is loaded and saved by the Rune shell: `main.rs`
//! seeds `core.light_theme` from `shell::load_layout()
//! .dark` at startup and persists `prefs.dark` on change
//! in `poll` (landed in the Phase D integration; an
//! earlier note here claimed otherwise and the Phase N
//! hygiene audit disproved it against the source). The
//! Live MCP toggle is the one control with its own field:
//! `LayoutPrefs.live_mcp` (serde-defaulted off, so live
//! control starts only when the user has enabled it and
//! the choice persisted), written by [`toggle_live_mcp`]
//! through the same load-modify-save `poll` uses for the
//! theme. The export defaults ride the same file as
//! three serde-defaulted string fields
//! (`export_dir` / `export_naming` / `export_quality`,
//! IA-W2-62-1): a prefs file written before they existed
//! loads with the historical behaviour in all three
//! (split roots, project-stem naming, Standard quality),
//! and the loader sanitises each value — an unknown
//! naming/quality name falls back to its default, and a
//! directory that is absolute or climbs out of the
//! working directory falls back to the split roots.
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
use crate::rune_sec::i18n_wire;
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
/// button label (the language's native name), the value
/// it applies, and its measured translation tier
/// ([`lang_tier`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LangEntry {
    pub key: String,
    pub label: &'static str,
    pub lang: Lang,
    pub tier: LangTier,
}

impl LangEntry {
    /// The button's full label: the native name plus the
    /// tier suffix, so the picker's honesty survives at a
    /// glance — a language that will render mostly English
    /// says so on its own button.
    pub fn display_label(&self) -> String {
        format!("{}{}", self.label, self.tier.suffix())
    }
}

/// How completely a language's table is genuinely
/// translated, in the bands the coverage instrument
/// (`scripts/i18n_coverage.py`, Roadmap PL-9) measures:
/// `Full` at ≥85% of values differing from the English
/// placeholder text, `Partial` at 40–85%, `Basic` below
/// 40% (most labels still render in English). The suffix
/// is the word the picker button carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LangTier {
    Full,
    Partial,
    Basic,
}

impl LangTier {
    /// The picker's suffix for this tier (leading space
    /// included; ASCII so the width model in this module
    /// charges it exactly).
    pub fn suffix(self) -> &'static str {
        match self {
            LangTier::Full => " (full)",
            LangTier::Partial => " (partial)",
            LangTier::Basic => " (basic)",
        }
    }
}

/// The measured tier of each language (PL-9 machinery,
/// seat RM27). The classification is data, reviewed per
/// translation tranche: `scripts/i18n_coverage.py`
/// re-measures every table from `i18n.rs` and FAILS when
/// this match drifts from the measured bands, so the
/// labels cannot quietly go stale. First measurement
/// (2026-11-10, 697 keys): the eight genuine-tier tables
/// sit at 89.5–95.6% translated; the nineteen placeholder
/// tables at 4.0–5.2%; no language is in the Partial band
/// yet — tranches will move languages through it.
/// `Es419` is Full BY FALLBACK: its own table measures
/// Basic (5.0%), but the PL-9 base-language chain
/// (IA-RM27-1: es-419 resolves through es before English)
/// serves the es table for its placeholder cells, for an
/// effective 92.8% — the tier names what the user will
/// actually see once the chain lands in the same wave.
pub fn lang_tier(lang: Lang) -> LangTier {
    match lang {
        Lang::En => LangTier::Full,
        Lang::Bn => LangTier::Full,
        Lang::Es => LangTier::Full,
        Lang::Ar => LangTier::Basic,
        Lang::Cs => LangTier::Basic,
        Lang::Da => LangTier::Basic,
        Lang::De => LangTier::Full,
        Lang::El => LangTier::Basic,
        Lang::Es419 => LangTier::Full,
        Lang::Fi => LangTier::Basic,
        Lang::Fr => LangTier::Full,
        Lang::He => LangTier::Basic,
        Lang::Hr => LangTier::Basic,
        Lang::Hu => LangTier::Basic,
        Lang::Id => LangTier::Basic,
        Lang::It => LangTier::Full,
        Lang::Kk => LangTier::Basic,
        Lang::Nb => LangTier::Basic,
        Lang::Nl => LangTier::Basic,
        Lang::Pl => LangTier::Basic,
        Lang::PtBr => LangTier::Full,
        Lang::Ro => LangTier::Basic,
        Lang::Ru => LangTier::Full,
        Lang::Sl => LangTier::Basic,
        Lang::Tr => LangTier::Basic,
        Lang::Uk => LangTier::Basic,
        Lang::Vi => LangTier::Basic,
        Lang::ZhHans => LangTier::Full,
    }
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
            tier: lang_tier(*l),
        })
        .collect()
}

/// Apply a picked language: set the pub field
/// `EditorCore.lang` (`editor.rs:82`) and write the status
/// line the core's own `cycle_lang` writes
/// (`editor.rs:1906`), so the feedback reads identically
/// whichever path changed the language. Persistence is
/// the shell's: `main.rs`'s `poll` shadow-compares
/// `core.lang` and writes `LayoutPrefs.lang` on change
/// (Roadmap F-9), so this setter — like the theme
/// buttons — writes the field and lets `poll` save.
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
    // Roadmap Wave 1 (RM24, P-14): same ruling as the
    // scene panel's stage steppers — a coalesced undo
    // step only on a real change, so the step count
    // never depends on which surface stepped the stage
    // (Wave 13, census E6, only marked it).
    let next = stepped_stage_size(core.project.stage, dw, dh);
    if next != core.project.stage {
        core.snapshot_coalesced("Stage size");
        core.project.stage = next;
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

/// Flip the golden-ratio guide flag (`State.guides_golden`,
/// seeded `false` by the integrator — Roadmap Wave 2,
/// W2-33, wiring RM18's IA-RM18-3) — the child toggle of
/// Guides for the golden-ratio grid inside the camera
/// frame. Default OFF and stays that way: RM18 priced a
/// default flip and this wave explicitly does not take
/// it (a user ruling, not a wiring detail). Same contract
/// as [`toggle_guides_frustum`]: a pure shell-state flip;
/// the stage draws the grid only while Guides and Camera
/// frame are also on.
pub fn toggle_guides_golden(s: &mut State) {
    s.guides_golden = !s.guides_golden;
}

/// Flip the diagonals guide flag (`State.guides_diagonals`,
/// seeded `false` like [`toggle_guides_golden`]'s flag) —
/// the child toggle of Guides for the framed view's two
/// diagonals. Same default-off footing and contract as
/// [`toggle_guides_golden`].
pub fn toggle_guides_diagonals(s: &mut State) {
    s.guides_diagonals = !s.guides_diagonals;
}

/// Flip the stored-guides flag (`State.guides_stored`,
/// seeded `true` by the integrator — Roadmap Wave 2,
/// W2-05) — the toggle for the project's user-placed
/// guide lines. NOT a child of Guides in the drawing
/// sense: stored guides are authored content and draw
/// while this flag is on whatever `guides` says (see
/// `stage_view::StageOverlays::guides_stored`); the row
/// sits in the guides family because that is where a
/// user looks for guide switches. Default ON, the
/// opposite footing from the composition variants: a
/// fresh project has no stored guides, so the default
/// costs nothing, and a project that has them shows
/// them. Same pure-flip contract as the rest.
pub fn toggle_guides_stored(s: &mut State) {
    s.guides_stored = !s.guides_stored;
}

/// Flip the rulers flag (`State.rulers`, seeded `false`
/// by the integrator — W2-05) — the toggle for the
/// graduated stage-coordinate rulers along the stage's
/// top/left edges. Independent of [`toggle_guides_stored`]:
/// rulers are the stored guides' source chrome, but a
/// user may keep the lines and hide the chrome (guide
/// creation/deletion by drag then closes; moving a line
/// still works — the stage session prices it). Default
/// OFF: opt-in chrome, like the master Guides switch.
pub fn toggle_rulers(s: &mut State) {
    s.rulers = !s.rulers;
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

// ---- Export defaults (Roadmap Wave 2, W2-62 — PL-8 slice 1) ----
//
// The persisted export choices: root directory, naming
// template, encoder quality. The values live in the
// layout prefs (fields filed as IA-W2-62-1) as plain
// strings — the on-disk shape a hand edit reads — and
// every helper here works on those strings, so the pure
// core (sanitising, naming, root resolution) is pinned
// without a prefs file or a disk. The setters follow
// the Live MCP discipline exactly: load-modify-save of
// the RESULTING value, preserving every other field,
// and a status line naming the choice. The export
// panel consumes the same fields through
// `export_defaults()` (its wiring is IA-W2-62-2 for
// directory + naming and IA-W2-62-3 for quality).

/// The export defaults as the prefs file stores them,
/// sanitised: the three raw strings, each already
/// passed through its sanitiser, so a reader can trust
/// every value (see [`sanitize_export_dir`],
/// [`ExportNaming::from_persisted`],
/// [`export_quality_from_persisted`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportDefaults {
    /// The root-directory choice: `""` is the historical
    /// split roots (`export/` for the primary formats,
    /// `out/` for JPEG / APNG); any other value is the
    /// single relative root every export writes under.
    pub dir: String,
    /// The naming template (see [`ExportNaming`]).
    pub naming: ExportNaming,
    /// The encoder quality level (W2-42's vocabulary).
    pub quality: stykanimate_rs::export::EncodeQuality,
}

impl Default for ExportDefaults {
    /// The historical behaviour in all three choices:
    /// split roots, project-stem naming, Standard
    /// quality — what every export did before this
    /// section existed.
    fn default() -> Self {
        ExportDefaults {
            dir: String::new(),
            naming: ExportNaming::Project,
            quality: stykanimate_rs::export::DEFAULT_ENCODE_QUALITY,
        }
    }
}

/// One naming template for exported files, riding
/// RM23's project stems: the template chooses the stem
/// every named destination is built from, and the
/// formats keep their RM23 suffixes on top of it
/// (`-spritesheet`, `-contact-sheet`, the delivery
/// presets' `-1080p` / `-4k`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportNaming {
    /// The project stem itself — RM23's naming, exactly
    /// as shipped (`the-last-promise.mp4`).
    Project,
    /// The project stem plus the export date
    /// (`the-last-promise-2026-11-10.mp4`), so successive
    /// days' masters never share a name (the overwrite
    /// guard still protects same-day re-exports).
    ProjectDate,
    /// The pre-RM23 shared stem, `animation` — every
    /// project writes the same file names the exports
    /// used before they were named. Offered because the
    /// fallback stem is real, shipped behaviour (a
    /// nameless project still lands here), not nostalgia.
    Legacy,
}

impl ExportNaming {
    /// All templates, in chip order.
    pub fn all() -> [ExportNaming; 3] {
        [
            ExportNaming::Project,
            ExportNaming::ProjectDate,
            ExportNaming::Legacy,
        ]
    }

    /// The persisted name of this template — the string
    /// `LayoutPrefs.export_naming` stores.
    pub fn persisted_name(self) -> &'static str {
        match self {
            ExportNaming::Project => "project",
            ExportNaming::ProjectDate => "project-date",
            ExportNaming::Legacy => "animation",
        }
    }

    /// Read a template back from its persisted name. An
    /// unknown name — a hand edit's typo, a newer build's
    /// value — resolves to `None`, never to a guess; the
    /// sanitising reader ([`export_defaults_from_prefs`])
    /// falls back to [`ExportNaming::Project`].
    pub fn from_persisted(name: &str) -> Option<ExportNaming> {
        match name {
            "project" => Some(ExportNaming::Project),
            "project-date" => Some(ExportNaming::ProjectDate),
            "animation" => Some(ExportNaming::Legacy),
            _ => None,
        }
    }

    /// The chip label for this template.
    pub fn label(self) -> &'static str {
        match self {
            ExportNaming::Project => "Project name",
            ExportNaming::ProjectDate => "Name + date",
            ExportNaming::Legacy => "animation",
        }
    }

    /// The chip's element key.
    pub fn key(self) -> &'static str {
        match self {
            ExportNaming::Project => "settings-export-naming-project",
            ExportNaming::ProjectDate => "settings-export-naming-project-date",
            ExportNaming::Legacy => "settings-export-naming-legacy",
        }
    }
}

/// The stem a naming template produces from RM23's
/// project stem: `Project` is the stem itself, `Legacy`
/// is the historical shared stem, and `ProjectDate`
/// appends the export date. `date` is the export day as
/// `YYYY-MM-DD` and is read ONLY by `ProjectDate` — the
/// other templates ignore it, so a caller can pass the
/// preview placeholder (`"YYYY-MM-DD"`) for them. Pure:
/// the consumer wiring (IA-W2-62-2) computes the real
/// date at press time and passes it in.
pub fn named_stem(naming: ExportNaming, stem: &str, date: &str) -> String {
    match naming {
        ExportNaming::Project => stem.to_string(),
        ExportNaming::ProjectDate => format!("{stem}-{date}"),
        ExportNaming::Legacy => "animation".to_string(),
    }
}

/// The root-directory choices the card's chips offer.
/// A hand-edited prefs file may name any other relative
/// directory — the field is a plain string and the
/// consumer resolves it through the same
/// [`export_root_for`] — but the chips offer the three
/// roots the shell itself has ever used, and a custom
/// value simply lights no chip (the example line still
/// names it).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportDirChoice {
    /// The historical split: `export/` primary, `out/`
    /// secondary (persisted as `""`).
    Default,
    /// Everything under `export/`.
    Export,
    /// Everything under `out/`.
    Out,
}

impl ExportDirChoice {
    /// All choices, in chip order.
    pub fn all() -> [ExportDirChoice; 3] {
        [
            ExportDirChoice::Default,
            ExportDirChoice::Export,
            ExportDirChoice::Out,
        ]
    }

    /// The persisted string this choice stores in
    /// `LayoutPrefs.export_dir`.
    pub fn persisted_value(self) -> &'static str {
        match self {
            ExportDirChoice::Default => "",
            ExportDirChoice::Export => "export",
            ExportDirChoice::Out => "out",
        }
    }

    /// The choice a persisted (sanitised) directory
    /// names, or `None` for a custom hand-edited root.
    pub fn from_persisted(dir: &str) -> Option<ExportDirChoice> {
        match dir {
            "" => Some(ExportDirChoice::Default),
            "export" => Some(ExportDirChoice::Export),
            "out" => Some(ExportDirChoice::Out),
            _ => None,
        }
    }

    /// The chip label for this choice.
    pub fn label(self) -> &'static str {
        match self {
            ExportDirChoice::Default => "Default roots",
            ExportDirChoice::Export => "export/",
            ExportDirChoice::Out => "out/",
        }
    }

    /// The chip's element key.
    pub fn key(self) -> &'static str {
        match self {
            ExportDirChoice::Default => "settings-export-dir-default",
            ExportDirChoice::Export => "settings-export-dir-export",
            ExportDirChoice::Out => "settings-export-dir-out",
        }
    }
}

/// Sanitise a persisted export directory: the value is
/// a root RELATIVE to the working directory, so an
/// absolute path or a `..` component would let a hand
/// edit (or a corrupt file) aim exports anywhere on
/// disk — those fall back to `""`, the split roots, as
/// do empty and `.` values. A clean relative path is
/// kept, normalised to forward slashes with no trailing
/// slash (`renders/daily/` → `renders/daily`). This is
/// the load-time discipline IA-W2-62-1 applies in
/// `shell::load_layout_in`; it lives here so the card
/// and the loader sanitise identically.
pub fn sanitize_export_dir(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let path = std::path::Path::new(trimmed);
    if path.is_absolute() {
        return String::new();
    }
    let mut parts: Vec<&str> = Vec::new();
    for component in path.components() {
        match component {
            std::path::Component::Normal(part) => {
                parts.push(part.to_str().unwrap_or_default())
            }
            std::path::Component::CurDir => {}
            // RootDir / Prefix are absolute-path shapes;
            // ParentDir climbs out of the working dir.
            _ => return String::new(),
        }
    }
    if parts.iter().any(|p| p.is_empty()) {
        return String::new();
    }
    parts.join("/")
}

/// The root directory one export writes under, resolved
/// from the persisted directory choice: the empty
/// choice keeps the historical split (`secondary`
/// formats — JPEG, APNG — under `out/`, everything else
/// under `export/`); any other value is the single root
/// for both classes. Relative to the working directory,
/// exactly like the historical roots — the consumer
/// (IA-W2-62-2) joins it onto the current dir the way
/// `export_panel::export_dir` does today.
pub fn export_root_for(dir: &str, secondary: bool) -> std::path::PathBuf {
    if dir.is_empty() {
        std::path::PathBuf::from(if secondary { "out" } else { "export" })
    } else {
        std::path::PathBuf::from(dir)
    }
}

/// The display form of an export root (`./export/`,
/// `./out/`, `./renders/`) — the shape the panel's
/// recaps and this card's example line print.
pub fn export_root_display(dir: &str, secondary: bool) -> String {
    format!("./{}/", export_root_for(dir, secondary).display())
}

/// Read an encoder quality back from its persisted
/// name (W2-42's `EncodeQuality::name`): an unknown
/// name falls back to `DEFAULT_ENCODE_QUALITY`, so a
/// hand edit can never invent a fourth level.
pub fn export_quality_from_persisted(
    name: &str,
) -> stykanimate_rs::export::EncodeQuality {
    stykanimate_rs::export::EncodeQuality::from_name(name)
        .unwrap_or(stykanimate_rs::export::DEFAULT_ENCODE_QUALITY)
}

/// The quality chip's element key for a level — the
/// three levels are a closed vocabulary, so the keys
/// are a match, not a format string.
pub fn export_quality_key(
    quality: stykanimate_rs::export::EncodeQuality,
) -> &'static str {
    match quality {
        stykanimate_rs::export::EncodeQuality::Draft => {
            "settings-export-quality-draft"
        }
        stykanimate_rs::export::EncodeQuality::Standard => {
            "settings-export-quality-standard"
        }
        stykanimate_rs::export::EncodeQuality::Master => {
            "settings-export-quality-master"
        }
    }
}

/// The quality chip's label for a level: the level's
/// own name, capitalised as the panel copy writes it.
pub fn export_quality_label(
    quality: stykanimate_rs::export::EncodeQuality,
) -> &'static str {
    match quality {
        stykanimate_rs::export::EncodeQuality::Draft => "Draft",
        stykanimate_rs::export::EncodeQuality::Standard => "Standard",
        stykanimate_rs::export::EncodeQuality::Master => "Master",
    }
}

/// Build the sanitised defaults from a loaded prefs
/// value — the one reader every consumer shares (this
/// card's render, the export panel via IA-W2-62-2/3),
/// so the sanitising can never drift between surfaces.
pub fn export_defaults_from_prefs(
    prefs: &stykanimate_rs::shell::LayoutPrefs,
) -> ExportDefaults {
    ExportDefaults {
        dir: sanitize_export_dir(&prefs.export_dir),
        naming: ExportNaming::from_persisted(&prefs.export_naming)
            .unwrap_or(ExportNaming::Project),
        quality: export_quality_from_persisted(&prefs.export_quality),
    }
}

/// The current export defaults, read from the layout
/// prefs — one load, sanitised. The prefs file is the
/// only copy of these choices (no shell-state mirror
/// exists to drift), so every reader calls this.
pub fn export_defaults() -> ExportDefaults {
    export_defaults_from_prefs(&stykanimate_rs::shell::load_layout())
}

/// Persist a directory choice (one press of a directory
/// chip): load-modify-save through the layout prefs,
/// the Live MCP pattern, preserving every other field.
/// The status line names the resulting root so the
/// choice is never silent.
pub fn set_export_dir(s: &mut State, choice: ExportDirChoice) {
    let mut prefs = stykanimate_rs::shell::load_layout();
    prefs.export_dir = choice.persisted_value().to_string();
    stykanimate_rs::shell::save_layout(&prefs);
    s.core.status = match choice {
        ExportDirChoice::Default => {
            "Export directory: default roots (export/ + out/)".to_string()
        }
        ExportDirChoice::Export => "Export directory: export/".to_string(),
        ExportDirChoice::Out => "Export directory: out/".to_string(),
    };
}

/// Persist a naming template (one press of a naming
/// chip): same load-modify-save discipline as
/// [`set_export_dir`].
pub fn set_export_naming(s: &mut State, naming: ExportNaming) {
    let mut prefs = stykanimate_rs::shell::load_layout();
    prefs.export_naming = naming.persisted_name().to_string();
    stykanimate_rs::shell::save_layout(&prefs);
    s.core.status = format!("Export naming: {}", naming.label());
}

/// Persist an encoder quality level (one press of a
/// quality chip): same load-modify-save discipline as
/// [`set_export_dir`]. The stored string is the level's
/// own `EncodeQuality::name()`, so W2-42's vocabulary
/// has exactly one spelling.
pub fn set_export_quality(
    s: &mut State,
    quality: stykanimate_rs::export::EncodeQuality,
) {
    let mut prefs = stykanimate_rs::shell::load_layout();
    prefs.export_quality = quality.name().to_string();
    stykanimate_rs::shell::save_layout(&prefs);
    s.core.status = format!("Export quality: {}", export_quality_label(quality));
}

/// The example line under the Export defaults rows:
/// the MP4 destination and quality the current choices
/// produce for THIS project, so the three chip rows'
/// combined effect is visible before any export runs.
/// The stem rides RM23's own `project_stem` (the
/// export panel's naming home) through [`named_stem`];
/// under the Name + date template the date slot shows
/// its `YYYY-MM-DD` shape rather than today's date, so
/// the line is a template preview, not a promise about
/// a file that does not exist yet.
pub fn export_example_line(core: &EditorCore, defaults: &ExportDefaults) -> String {
    let stem = crate::rune_sec::export_panel::project_stem(&core.project);
    let named = named_stem(defaults.naming, &stem, "YYYY-MM-DD");
    format!(
        "Example MP4: {}{}.mp4 · {} quality",
        export_root_display(&defaults.dir, false),
        named,
        export_quality_label(defaults.quality),
    )
}

/// One chip row of the Export defaults section: a muted
/// row label plus its chips, in the card's picker idiom
/// (plain `btn`, the active chip highlighted). The rows
/// carry the grids' wrap pair (`.w_full().flex_wrap()`,
/// Wave 10): label + three chips measure past the
/// narrowest column width, where they wrap instead of
/// overrunning the card.
fn export_defaults_row(
    row_key: &'static str,
    label: &'static str,
    chips: Vec<El>,
    t: &Theme,
) -> El {
    let mut r = row().key(row_key).gap(6.0).w_full().flex_wrap().center_y();
    r = r.child(text(label).font(type_::LABEL).color(t.text_secondary));
    for chip in chips {
        r = r.child(chip);
    }
    r
}

/// The Directory row: the three root choices
/// ([`ExportDirChoice`]). The active chip is the choice
/// the persisted directory names; a custom hand-edited
/// root lights no chip (and the example line names it).
fn export_dir_row(defaults: &ExportDefaults, t: &Theme) -> El {
    let active = ExportDirChoice::from_persisted(&defaults.dir);
    let chips = ExportDirChoice::all()
        .iter()
        .map(|&choice| {
            btn(
                choice.key(),
                choice.label(),
                active == Some(choice),
                t,
                move |s: &mut State| {
                    set_export_dir(s, choice);
                },
            )
        })
        .collect();
    export_defaults_row("settings-export-dir-row", "Directory", chips, t)
}

/// The Naming row: the three templates
/// ([`ExportNaming`]) riding RM23's project stems.
fn export_naming_row(defaults: &ExportDefaults, t: &Theme) -> El {
    let chips = ExportNaming::all()
        .iter()
        .map(|&naming| {
            btn(
                naming.key(),
                naming.label(),
                naming == defaults.naming,
                t,
                move |s: &mut State| {
                    set_export_naming(s, naming);
                },
            )
        })
        .collect();
    export_defaults_row("settings-export-naming-row", "Naming", chips, t)
}

/// The Quality row: W2-42's three encoder levels. The
/// highlighted chip is the persisted default the export
/// panel will use (IA-W2-62-3); Standard is the exact
/// encode every export used before the levels existed.
fn export_quality_row(defaults: &ExportDefaults, t: &Theme) -> El {
    let levels = [
        stykanimate_rs::export::EncodeQuality::Draft,
        stykanimate_rs::export::EncodeQuality::Standard,
        stykanimate_rs::export::EncodeQuality::Master,
    ];
    let chips = levels
        .iter()
        .map(|&quality| {
            btn(
                export_quality_key(quality),
                export_quality_label(quality),
                quality == defaults.quality,
                t,
                move |s: &mut State| {
                    set_export_quality(s, quality);
                },
            )
        })
        .collect();
    export_defaults_row("settings-export-quality-row", "Quality", chips, t)
}

// ---- Autosave & recovery (Roadmap Wave 2, W2-63 — PL-8 slice 2) ----
//
// The recovery writer's persisted choices: on/off, the
// cadence in counted edits, and the snapshot
// generations retained. The values live in the layout
// prefs (fields filed as IA-W2-63-1) as a bool and two
// plain numbers — the on-disk shape a hand edit reads —
// and every helper here works through the lib's policy
// type (`stykanimate_rs::autosave::AutosavePrefs`), so
// the sanitising (cadence 5..=500, keep 1..=10) is the
// one the loader and the writer share. The setters
// follow the Export defaults discipline — load-modify-
// save of the RESULTING value, preserving every other
// field, and a status line naming the choice — plus one
// step the export setters do not need: each also
// mirrors the saved policy into the core's live copy
// (`EditorCore::autosave`, IA-W2-63-2), because the
// writer reads the core, not the prefs file, on every
// poll tick. The shell's `init` seeds that copy from
// the prefs at launch (IA-W2-63-3).

/// The autosave choices as the card renders them,
/// sanitised: the policy rebuilt from the prefs through
/// [`stykanimate_rs::autosave::AutosavePrefs::from_persisted`],
/// so a reader can trust every value (a hand-edited
/// cadence of 999 renders as the 500 the writer will
/// actually use, and its chip row lights no chip — the
/// custom-value precedent the Export defaults set).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AutosaveSettings {
    /// Whether the recovery writer runs at all.
    pub enabled: bool,
    /// Counted edits between autosaves (sanitised).
    pub every: u64,
    /// Snapshot generations retained, live included
    /// (sanitised).
    pub keep: u32,
}

impl Default for AutosaveSettings {
    /// The historical behaviour in all three choices:
    /// on, every 25 edits, one generation — what every
    /// build before this section shipped.
    fn default() -> Self {
        AutosaveSettings {
            enabled: true,
            every: stykanimate_rs::autosave::DEFAULT_EVERY,
            keep: stykanimate_rs::autosave::DEFAULT_KEEP,
        }
    }
}

/// One cadence the Interval row offers, in counted
/// edits. The persisted value is the plain number (a
/// hand edit may store any value in range — it lights
/// no chip and still applies), but the chips offer the
/// four cadences that bracket the useful range: brisk
/// (10), the historical default (25), relaxed (50),
/// and sparse (100).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutosaveInterval {
    /// Every 10 counted edits.
    Every10,
    /// Every 25 counted edits — the historical cadence.
    Every25,
    /// Every 50 counted edits.
    Every50,
    /// Every 100 counted edits.
    Every100,
}

impl AutosaveInterval {
    /// All cadences, in chip order.
    pub fn all() -> [AutosaveInterval; 4] {
        [
            AutosaveInterval::Every10,
            AutosaveInterval::Every25,
            AutosaveInterval::Every50,
            AutosaveInterval::Every100,
        ]
    }

    /// The counted edits this cadence stores in
    /// `LayoutPrefs.autosave_every`.
    pub fn edits(self) -> u64 {
        match self {
            AutosaveInterval::Every10 => 10,
            AutosaveInterval::Every25 => 25,
            AutosaveInterval::Every50 => 50,
            AutosaveInterval::Every100 => 100,
        }
    }

    /// The cadence a persisted (sanitised) value names,
    /// or `None` for an in-range custom value no chip
    /// offers (a hand edit's 30) — the row then lights
    /// no chip and the status line still reports the
    /// number faithfully after any press.
    pub fn from_edits(every: u64) -> Option<AutosaveInterval> {
        AutosaveInterval::all()
            .iter()
            .copied()
            .find(|interval| interval.edits() == every)
    }

    /// The chip label for this cadence.
    pub fn label(self) -> &'static str {
        match self {
            AutosaveInterval::Every10 => "10 edits",
            AutosaveInterval::Every25 => "25 edits",
            AutosaveInterval::Every50 => "50 edits",
            AutosaveInterval::Every100 => "100 edits",
        }
    }

    /// The chip's element key.
    pub fn key(self) -> &'static str {
        match self {
            AutosaveInterval::Every10 => "settings-autosave-every-10",
            AutosaveInterval::Every25 => "settings-autosave-every-25",
            AutosaveInterval::Every50 => "settings-autosave-every-50",
            AutosaveInterval::Every100 => "settings-autosave-every-100",
        }
    }
}

/// One retention the Copies row offers, in snapshot
/// generations (the live snapshot included). 1 is the
/// historical single file; the larger values keep a
/// short history behind it, so a bad state autosaved
/// over a good one no longer destroys the only
/// recovery point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryKeep {
    /// One generation — the live snapshot only.
    One,
    /// Two generations.
    Two,
    /// Three generations.
    Three,
    /// Five generations.
    Five,
}

impl RecoveryKeep {
    /// All retentions, in chip order.
    pub fn all() -> [RecoveryKeep; 4] {
        [
            RecoveryKeep::One,
            RecoveryKeep::Two,
            RecoveryKeep::Three,
            RecoveryKeep::Five,
        ]
    }

    /// The generation count this retention stores in
    /// `LayoutPrefs.recovery_keep`.
    pub fn count(self) -> u32 {
        match self {
            RecoveryKeep::One => 1,
            RecoveryKeep::Two => 2,
            RecoveryKeep::Three => 3,
            RecoveryKeep::Five => 5,
        }
    }

    /// The retention a persisted (sanitised) count
    /// names, or `None` for an in-range custom count
    /// no chip offers (a hand edit's 4).
    pub fn from_count(keep: u32) -> Option<RecoveryKeep> {
        RecoveryKeep::all()
            .iter()
            .copied()
            .find(|retention| retention.count() == keep)
    }

    /// The chip label for this retention.
    pub fn label(self) -> &'static str {
        match self {
            RecoveryKeep::One => "1 copy",
            RecoveryKeep::Two => "2 copies",
            RecoveryKeep::Three => "3 copies",
            RecoveryKeep::Five => "5 copies",
        }
    }

    /// The chip's element key.
    pub fn key(self) -> &'static str {
        match self {
            RecoveryKeep::One => "settings-autosave-keep-1",
            RecoveryKeep::Two => "settings-autosave-keep-2",
            RecoveryKeep::Three => "settings-autosave-keep-3",
            RecoveryKeep::Five => "settings-autosave-keep-5",
        }
    }
}

/// Build the sanitised settings from a loaded prefs
/// value — the one reader every consumer shares (this
/// card's render, the setters' core mirror), so the
/// sanitising can never drift between surfaces.
pub fn autosave_settings_from_prefs(
    prefs: &stykanimate_rs::shell::LayoutPrefs,
) -> AutosaveSettings {
    let policy = stykanimate_rs::autosave::AutosavePrefs::from_persisted(
        prefs.autosave_enabled,
        prefs.autosave_every,
        prefs.recovery_keep,
    );
    AutosaveSettings {
        enabled: policy.enabled,
        every: policy.every,
        keep: policy.keep,
    }
}

/// The current autosave settings, read from the layout
/// prefs — one load, sanitised. The prefs file is the
/// only stored copy of these choices, so every reader
/// calls this.
pub fn autosave_settings() -> AutosaveSettings {
    autosave_settings_from_prefs(&stykanimate_rs::shell::load_layout())
}

/// The shared half of every autosave setter: persist
/// the (already edited) prefs, then mirror the saved
/// policy into the core's live copy, rebuilt from the
/// saved values — the prefs file stays the source of
/// truth and the core can never drift from it within
/// the session.
fn save_autosave_prefs(s: &mut State, prefs: &stykanimate_rs::shell::LayoutPrefs) {
    stykanimate_rs::shell::save_layout(prefs);
    let policy = stykanimate_rs::autosave::AutosavePrefs::from_persisted(
        prefs.autosave_enabled,
        prefs.autosave_every,
        prefs.recovery_keep,
    );
    s.core.autosave = policy;
}

/// Flip the autosave switch (the toggle row's press):
/// load-modify-save the resulting value and mirror it
/// into the core, the Live MCP discipline. Turning off
/// stops every recovery write — the cadence and the
/// window-close flush alike (the flush honouring the
/// switch is what makes it honest); snapshots already
/// on disk are left untouched, never deleted.
pub fn toggle_autosave(s: &mut State) {
    let mut prefs = stykanimate_rs::shell::load_layout();
    prefs.autosave_enabled = !prefs.autosave_enabled;
    let on = prefs.autosave_enabled;
    save_autosave_prefs(s, &prefs);
    s.core.status = if on {
        "Autosave on — recovery snapshots resume".to_string()
    } else {
        "Autosave off — no new recovery snapshots will be written".to_string()
    };
}

/// Persist a cadence (one press of an Interval chip):
/// same discipline as [`toggle_autosave`].
pub fn set_autosave_interval(s: &mut State, interval: AutosaveInterval) {
    let mut prefs = stykanimate_rs::shell::load_layout();
    prefs.autosave_every = interval.edits();
    save_autosave_prefs(s, &prefs);
    s.core.status = format!("Autosave interval: {}", interval.label());
}

/// Persist a retention (one press of a Copies chip):
/// same discipline as [`toggle_autosave`]. A lowered
/// retention frees its extra generations at the next
/// autosave's rotation, not at the press — the copies
/// on disk until then are still the user's safety net.
pub fn set_recovery_keep(s: &mut State, keep: RecoveryKeep) {
    let mut prefs = stykanimate_rs::shell::load_layout();
    prefs.recovery_keep = keep.count();
    save_autosave_prefs(s, &prefs);
    s.core.status = format!("Recovery copies kept: {}", keep.label());
}

/// The status line under the Autosave rows, from the
/// two disk facts: when the live snapshot was last
/// written (`shell::recovery_modified`'s answer, the
/// status strip's own source) and how many generations
/// are actually present. Pure over its inputs so the
/// wording is pinned without a disk; the live wrapper
/// is [`autosave_status_line`].
pub fn autosave_status_line_from(
    modified: Option<std::time::SystemTime>,
    generations: u32,
    now: std::time::SystemTime,
) -> String {
    let Some(written) = modified else {
        return "Recovery snapshot: none yet.".to_string();
    };
    let age = now
        .duration_since(written)
        .unwrap_or(std::time::Duration::ZERO);
    let age_text = if age.as_secs() < 60 {
        "just now".to_string()
    } else if age.as_secs() < 3_600 {
        format!("{} min ago", age.as_secs() / 60)
    } else if age.as_secs() < 86_400 {
        format!("{} h ago", age.as_secs() / 3_600)
    } else {
        format!("{} d ago", age.as_secs() / 86_400)
    };
    let copies = if generations == 1 {
        "1 copy kept".to_string()
    } else {
        format!("{generations} copies kept")
    };
    format!("Recovery snapshot: saved {age_text} · {copies}")
}

/// The status line under the Autosave rows, read from
/// the disk as it stands at render time.
pub fn autosave_status_line() -> String {
    autosave_status_line_from(
        stykanimate_rs::shell::recovery_modified(),
        stykanimate_rs::autosave::generations_present(),
        std::time::SystemTime::now(),
    )
}

/// The Interval row: the four cadences
/// ([`AutosaveInterval`]). The active chip is the one
/// the persisted cadence names; a custom in-range
/// value lights no chip (the status line after any
/// press still reports truthfully).
fn autosave_interval_row(settings: &AutosaveSettings, t: &Theme) -> El {
    let active = AutosaveInterval::from_edits(settings.every);
    let chips = AutosaveInterval::all()
        .iter()
        .map(|&interval| {
            btn(
                interval.key(),
                interval.label(),
                active == Some(interval),
                t,
                move |s: &mut State| {
                    set_autosave_interval(s, interval);
                },
            )
        })
        .collect();
    export_defaults_row("settings-autosave-interval-row", "Interval", chips, t)
}

/// The Copies row: the four retentions
/// ([`RecoveryKeep`]) — how many snapshot generations
/// the rotation retains, the live one included.
fn autosave_keep_row(settings: &AutosaveSettings, t: &Theme) -> El {
    let active = RecoveryKeep::from_count(settings.keep);
    let chips = RecoveryKeep::all()
        .iter()
        .map(|&keep| {
            btn(
                keep.key(),
                keep.label(),
                active == Some(keep),
                t,
                move |s: &mut State| {
                    set_recovery_keep(s, keep);
                },
            )
        })
        .collect();
    export_defaults_row("settings-autosave-keep-row", "Copies", chips, t)
}

// ---- Startup workspace ----

/// Workspace buttons per row in the picker grid (the
/// language grid's width — nine workspaces land as three
/// rows of four: 4+4+1).
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

/// All nine workspaces, in picker order, derived from
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
/// the workspace chip row and the Alt+1..9 keys run
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
/// current language. The label is the entry's display
/// label — native name plus tier suffix ([`LangTier`]),
/// so an untranslated language is never presented as if
/// it were a full localisation.
fn language_button(entry: LangEntry, current: Lang, t: &Theme) -> El {
    let key = entry.key.clone();
    let lang = entry.lang;
    let label = entry.display_label();
    btn(&key, &label, lang == current, t, move |s: &mut State| {
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
    let stage_v = s.core.project.stage[axis];
    row()
        .gap(8.0)
        .center_y()
        .child(design::section_label(heading, t))
        .child(design::step_btn(
            down_key,
            "-",
            // Wave P: inert where the step clamps to
            // the stored size ([`STAGE_MIN`]..=
            // [`STAGE_MAX`], the apply_stage_step
            // mirror).
            if (stage_v - STAGE_STEP).clamp(STAGE_MIN, STAGE_MAX) == stage_v {
                design::StepForm::Noop
            } else {
                design::StepForm::Live
            },
            t,
            move |s: &mut State| {
                apply_stage_step(&mut s.core, -dw, -dh);
            },
        ))
        .child(design::stepper_value(
            &format!("{}", s.core.project.stage[axis] as i32),
            t,
        ))
        .child(design::step_btn(
            up_key,
            "+",
            if (stage_v + STAGE_STEP).clamp(STAGE_MIN, STAGE_MAX) == stage_v {
                design::StepForm::Noop
            } else {
                design::StepForm::Live
            },
            t,
            move |s: &mut State| {
                apply_stage_step(&mut s.core, dw, dh);
            },
        ))
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
/// playback-audio readout), the Export defaults rows
/// (directory / naming / quality chips + the example
/// line, W2-62), the startup-workspace picker,
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
        &i18n_wire::tr(core, "settings"),
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
                    .child(design::panel_title(Some("gear"), &i18n_wire::tr(core, "settings"), None, t)),
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
                &i18n_wire::tr(core, "live_mcp_control"),
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
        // only draws them while Guides is on. Roadmap
        // Wave 2 (W2-33) adds a second child row — the
        // golden-ratio and diagonals variants (State.
        // guides_golden / guides_diagonals, both default
        // OFF) — so the keyed child is now a column of two
        // unkeyed rows under the same key/pl/dim.
        card = card
            .child(
                col()
                    .key("settings-guides-children")
                    .gap(6.0)
                    .pl(14.0)
                    .opacity(0.85)
                    .child(
                        row()
                            .gap(6.0)
                            .w_full()
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
                    .child(
                        row()
                            .gap(6.0)
                            .w_full()
                            .child(
                                design::toggle_row(
                                    "settings-guides-golden",
                                    "Golden ratio",
                                    s.guides_golden,
                                    t,
                                    |s: &mut State| {
                                        toggle_guides_golden(s);
                                    },
                                )
                                .grow(1.0),
                            )
                            .child(
                                design::toggle_row(
                                    "settings-guides-diagonals",
                                    "Diagonals",
                                    s.guides_diagonals,
                                    t,
                                    |s: &mut State| {
                                        toggle_guides_diagonals(s);
                                    },
                                )
                                .grow(1.0),
                            ),
                    )
                    // Roadmap Wave 2 (W2-05): a third child
                    // row — the stored guides and the
                    // rulers they are dragged from. Unlike
                    // the derived families above, neither is
                    // gated by Guides in the drawing (stored
                    // guides are authored content; rulers
                    // are their own chrome), and their
                    // defaults differ: stored guides ON,
                    // rulers OFF. Literal labels, like
                    // every row in this family.
                    .child(
                        row()
                            .gap(6.0)
                            .w_full()
                            .child(
                                design::toggle_row(
                                    "settings-guides-stored",
                                    "Stored guides",
                                    s.guides_stored,
                                    t,
                                    |s: &mut State| {
                                        toggle_guides_stored(s);
                                    },
                                )
                                .grow(1.0),
                            )
                            .child(
                                design::toggle_row(
                                    "settings-rulers",
                                    "Rulers",
                                    s.rulers,
                                    t,
                                    |s: &mut State| {
                                        toggle_rulers(s);
                                    },
                                )
                                .grow(1.0),
                            ),
                    ),
            )
            .child(note_line(
                t,
                "Stage guides: thirds grid + centre cross. \
                 Camera frame and safe frames show only while Guides is on. \
                 Golden ratio and diagonals draw inside the camera frame. \
                 Stored guides are saved with the project and show whenever their toggle is on; \
                 drag from a ruler to add one.",
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

        // Export defaults (Roadmap Wave 2, W2-62 —
        // PL-8 slice 1): the persisted directory /
        // naming / quality choices, read from the layout
        // prefs for this render (one load while the card
        // is open — the `column_widths` precedent) and
        // written load-modify-save by each chip press, so
        // the prefs file is the only copy. The example
        // line previews the combined effect on this
        // project's MP4 destination. The export panel
        // reads the same prefs on every press
        // (IA-W2-62-2 / IA-W2-62-3).
        let defaults = export_defaults();
        card = card
            .child(design::section_label("Export defaults", t))
            .child(export_dir_row(&defaults, t))
            .child(export_naming_row(&defaults, t))
            .child(export_quality_row(&defaults, t))
            .child(
                text(export_example_line(core, &defaults))
                    .key("settings-export-example")
                    .font(type_::LABEL)
                    .color(t.text_secondary)
                    .wrap()
                    .w_full(),
            );

        // Autosave & recovery (Roadmap Wave 2, W2-63 —
        // PL-8 slice 2): the recovery writer's persisted
        // choices, read from the layout prefs for this
        // render (the Export defaults pattern just above)
        // and written load-modify-save by each press,
        // which also mirrors the saved policy into the
        // core's live copy — so a press takes effect in
        // this session. A kit `toggle_row` like the Live
        // MCP row carries the switch (its state reads the
        // prefs, never a mirrored bool); under the rows,
        // the status line reports the snapshot on disk —
        // its age and the generations actually present.
        let autosave = autosave_settings();
        card = card
            .child(design::section_label("Autosave & recovery", t))
            .child(design::toggle_row(
                "settings-autosave-toggle",
                "Autosave recovery snapshots",
                autosave.enabled,
                t,
                |s: &mut State| {
                    toggle_autosave(s);
                },
            ))
            .child(autosave_interval_row(&autosave, t))
            .child(autosave_keep_row(&autosave, t))
            .child(
                text(autosave_status_line())
                    .key("settings-autosave-status")
                    .font(type_::LABEL)
                    .color(t.text_secondary)
                    .wrap()
                    .w_full(),
            );

        // Startup workspace: the nine workspaces as a
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
            // (Wave 10): the rows measure 258 / 220 pt —
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
        card = card
            .child(design::section_label("Language", t))
            .child(grid)
            // The tier legend (PL-9): what the button
            // suffixes mean, in the card's own note prose.
            // A Basic language still switches — most of
            // its labels simply render in English until
            // its translation tranche lands.
            .child(note_line(
                t,
                "full = translated · partial = partly translated · basic = most labels still in English",
            ));

        panel = panel.child(card);
    }
    panel
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_i18n_keys_exist_and_match_the_english_labels() {
        // Wave V (V11, from U10's catalogue-hygiene
        // ledger): the "Settings" toggle + card title and
        // the "Live MCP control" row resolve through
        // i18n_wire::tr under pre-existing catalogue keys.
        // The keys already existed x28, so this pin passes
        // on its own merits — no IA was owed.
        let core = EditorCore::new();
        for (key, english) in [
            ("settings", "Settings"),
            ("live_mcp_control", "Live MCP control"),
        ] {
            assert!(i18n_wire::key_in_table(key), "missing i18n key: {key}");
            assert_eq!(i18n_wire::tr(&core, key), english);
        }
    }

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
    fn lang_tier_matches_the_measured_bands() {
        // PL-9 (RM27): the picker classification, pinned
        // per language. The coverage instrument
        // (scripts/i18n_coverage.py) re-measures the real
        // tables and fails on drift from these bands, so
        // a tranche that crosses a band must move the arm
        // here in the same change.
        for lang in Lang::all() {
            let want = match lang {
                Lang::En
                | Lang::Bn
                | Lang::Es
                | Lang::De
                | Lang::Fr
                | Lang::It
                | Lang::PtBr
                | Lang::Ru
                | Lang::ZhHans => LangTier::Full,
                // Full by fallback: es-419's own table is
                // Basic; the base-language chain serves es
                // for its placeholder cells (IA-RM27-1).
                Lang::Es419 => LangTier::Full,
                _ => LangTier::Basic,
            };
            assert_eq!(lang_tier(lang), want, "{lang:?}");
        }
        // No language measures in the Partial band yet;
        // the band (and its suffix) exists for tranches
        // in flight.
        assert_eq!(LangTier::Full.suffix(), " (full)");
        assert_eq!(LangTier::Partial.suffix(), " (partial)");
        assert_eq!(LangTier::Basic.suffix(), " (basic)");
        // Entries carry the tier and compose the display
        // label from it; keys and native names unchanged.
        let entries = language_entries();
        let es419 = entries.iter().find(|e| e.lang == Lang::Es419).unwrap();
        assert_eq!(es419.display_label(), "Español (Latinoamérica) (full)");
        let cs = entries.iter().find(|e| e.lang == Lang::Cs).unwrap();
        assert_eq!(cs.tier, LangTier::Basic);
        assert_eq!(cs.display_label(), "Čeština (basic)");
        assert!(entries.iter().all(|e| e.tier == lang_tier(e.lang)));
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
            migration_panel: crate::rune_sec::migration_prompt::initial_state(),
            frame_ops: crate::rune_sec::frame_ops::FrameOpsState::default(),
            graph_panel: crate::rune_sec::graph_panel::GraphPanelState::default(),
            audio_panel: crate::rune_sec::audio_panel::AudioPanelState::new(),
            subtitle_panel: crate::rune_sec::subtitle_panel::SubtitlePanelState::new(),
            menus: crate::rune_sec::menus::MenusState::default(),
            palette: crate::rune_sec::command_palette::PaletteState::new(),
            settings: SettingsState::default(),
            toasts: crate::rune_sec::toasts::ToastsState::new(),
            text_tool: crate::rune_sec::text_tool::TextToolState::new(),
            name_edit: rune::TextEdit::new(""),
            export_use_range: false,
            fx_selected: None,
            subtitle_selected: None,
            pose_clipboard: None,
            guides: false,
            last_status: String::new(),
            toast_now: 0.0,
            last_dark: true,
            workspace: crate::rune_sec::workspaces::Workspace::Animate,
            setup_mode_before_build: false,
            show_left: true,
            show_right: true,
            stage_focus_restore: None,
            key_clipboard: None,
            key_follow_graph: false,
            key_sel: Default::default(),
            keyframe_tools: Default::default(),
            help_panel: Default::default(),
            camera_panel: Default::default(),
            segment_numbers: Default::default(),
            export_recap: String::new(),
            export_job: None,
            inspector_figure: crate::rune_sec::inspector_figure::InspectorFigureEditState::new(),
            clips_rename: crate::rune_sec::clips_panel::ClipRenameState::new(),
            fx_browser: crate::rune_sec::fx_browser::FxBrowserState::default(),
            draw_tools: crate::rune_sec::draw_tools::DrawToolsState::default(),
stroke_selected: None,
particle_selected: None,
guides_frustum: true,
guides_safe: true,
limit_arcs: false,
guides_golden: false,
guides_diagonals: false,
guides_stored: true,
rulers: false,
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
            migration_panel: crate::rune_sec::migration_prompt::initial_state(),
            frame_ops: crate::rune_sec::frame_ops::FrameOpsState::default(),
            graph_panel: crate::rune_sec::graph_panel::GraphPanelState::default(),
            audio_panel: crate::rune_sec::audio_panel::AudioPanelState::new(),
            subtitle_panel: crate::rune_sec::subtitle_panel::SubtitlePanelState::new(),
            menus: crate::rune_sec::menus::MenusState::default(),
            palette: crate::rune_sec::command_palette::PaletteState::new(),
            settings: SettingsState::default(),
            toasts: crate::rune_sec::toasts::ToastsState::new(),
            text_tool: crate::rune_sec::text_tool::TextToolState::new(),
            name_edit: rune::TextEdit::new(""),
            export_use_range: false,
            fx_selected: None,
            subtitle_selected: None,
            pose_clipboard: None,
            guides: false,
            last_status: String::new(),
            toast_now: 0.0,
            last_dark: true,
            workspace: crate::rune_sec::workspaces::Workspace::Animate,
            setup_mode_before_build: false,
            show_left: true,
            show_right: true,
            stage_focus_restore: None,
            key_clipboard: None,
            key_follow_graph: false,
            key_sel: Default::default(),
            keyframe_tools: Default::default(),
            help_panel: Default::default(),
            camera_panel: Default::default(),
            segment_numbers: Default::default(),
            export_recap: String::new(),
            export_job: None,
            inspector_figure: crate::rune_sec::inspector_figure::InspectorFigureEditState::new(),
            clips_rename: crate::rune_sec::clips_panel::ClipRenameState::new(),
            fx_browser: crate::rune_sec::fx_browser::FxBrowserState::default(),
            draw_tools: crate::rune_sec::draw_tools::DrawToolsState::default(),
            stroke_selected: None,
            particle_selected: None,
            guides_frustum: true,
            guides_safe: true,
            limit_arcs: false,
            guides_golden: false,
            guides_diagonals: false,
            guides_stored: true,
            rulers: false,
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
    fn guide_variant_toggles_flip_only_their_own_flag() {
        // Roadmap Wave 2 (W2-33, IA-RM18-3): the golden /
        // diagonals child toggles. Integrator seeds: both
        // OFF (the default flip RM18 priced was not taken),
        // while the Phase G family keeps its own seeds.
        let t = rune::tok::theme(true);
        let mut state = shell_state();
        assert!(!state.guides_golden && !state.guides_diagonals);
        assert!(state.guides_frustum && state.guides_safe && !state.guides);

        // Golden flips alone — parent, grandparent and
        // sibling unmoved.
        toggle_guides_golden(&mut state);
        assert!(state.guides_golden && !state.guides_diagonals);
        assert!(state.guides_frustum && state.guides_safe && !state.guides);
        toggle_guides_golden(&mut state);
        assert!(!state.guides_golden && !state.guides_diagonals);

        // Diagonals flips alone.
        toggle_guides_diagonals(&mut state);
        assert!(!state.guides_golden && state.guides_diagonals);
        toggle_guides_diagonals(&mut state);
        assert!(!state.guides_golden && !state.guides_diagonals);

        // The open card builds with a variant on (the
        // second child row renders regardless of the
        // parent flags' states).
        state.settings.open = true;
        toggle_guides_golden(&mut state);
        toggle_guides_diagonals(&mut state);
        let _ = settings_popover(&state, &t);
    }

    #[test]
    fn stored_guides_and_rulers_toggles_flip_only_their_own_flag() {
        // Roadmap Wave 2 (W2-05): the stored-guides and
        // rulers toggles. Integrator seeds differ on
        // purpose: stored guides ON (authored lines show;
        // a fresh project has none), rulers OFF (opt-in
        // chrome). Each flip moves only its own flag —
        // in particular neither touches the Guides family,
        // because neither is drawn under the master gate.
        let t = rune::tok::theme(true);
        let mut state = shell_state();
        assert!(state.guides_stored && !state.rulers);
        assert!(!state.guides && state.guides_frustum && state.guides_safe);

        toggle_guides_stored(&mut state);
        assert!(!state.guides_stored && !state.rulers);
        assert!(!state.guides && state.guides_frustum && state.guides_safe);
        toggle_guides_stored(&mut state);
        assert!(state.guides_stored && !state.rulers);

        toggle_rulers(&mut state);
        assert!(state.guides_stored && state.rulers);
        toggle_rulers(&mut state);
        assert!(state.guides_stored && !state.rulers);

        // The open card builds with both flipped (the
        // third child row renders regardless of the
        // parent flags' states).
        state.settings.open = true;
        toggle_guides_stored(&mut state);
        toggle_rulers(&mut state);
        let _ = settings_popover(&state, &t);
    }

    #[test]
    fn workspace_entries_cover_all_nine_workspaces() {
        let entries = workspace_entries();
        assert_eq!(entries.len(), 9);
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
        assert_eq!(keys.len(), 9);
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

    // ---- Export defaults (Roadmap Wave 2, W2-62) ----

    #[test]
    fn export_naming_round_trips_and_applies_to_stems() {
        for naming in ExportNaming::all() {
            assert_eq!(
                ExportNaming::from_persisted(naming.persisted_name()),
                Some(naming)
            );
        }
        // Unknown names never resolve to a guess.
        assert_eq!(ExportNaming::from_persisted("bogus"), None);
        assert_eq!(ExportNaming::from_persisted(""), None);
        // The templates ride RM23's stem: Project is the
        // stem itself (RM23's shipped naming, byte for
        // byte), Legacy is the pre-RM23 shared stem, and
        // ProjectDate appends the date it is handed.
        assert_eq!(
            named_stem(ExportNaming::Project, "the-last-promise", "2026-11-10"),
            "the-last-promise"
        );
        assert_eq!(
            named_stem(ExportNaming::Legacy, "the-last-promise", "2026-11-10"),
            "animation"
        );
        assert_eq!(
            named_stem(ExportNaming::ProjectDate, "the-last-promise", "2026-11-10"),
            "the-last-promise-2026-11-10"
        );
        // The date slot is read only by ProjectDate: the
        // preview placeholder never leaks into the other
        // templates' stems.
        assert_eq!(
            named_stem(ExportNaming::Project, "untitled", "YYYY-MM-DD"),
            "untitled"
        );
        assert_eq!(
            named_stem(ExportNaming::Legacy, "untitled", "YYYY-MM-DD"),
            "animation"
        );
    }

    #[test]
    fn export_dir_choices_round_trip() {
        for choice in ExportDirChoice::all() {
            assert_eq!(
                ExportDirChoice::from_persisted(choice.persisted_value()),
                Some(choice)
            );
        }
        // A custom hand-edited root lights no chip.
        assert_eq!(ExportDirChoice::from_persisted("renders"), None);
    }

    #[test]
    fn sanitize_export_dir_rejects_escapes_and_normalises() {
        // The historical split roots.
        assert_eq!(sanitize_export_dir(""), "");
        assert_eq!(sanitize_export_dir("  "), "");
        assert_eq!(sanitize_export_dir("."), "");
        // Clean relative roots survive, normalised.
        assert_eq!(sanitize_export_dir("export"), "export");
        assert_eq!(sanitize_export_dir("renders/daily/"), "renders/daily");
        assert_eq!(sanitize_export_dir("./renders"), "renders");
        // Escapes fall back to the split roots: absolute
        // paths and parent climbs may not aim exports
        // outside the working directory.
        assert_eq!(sanitize_export_dir("/tmp/elsewhere"), "");
        assert_eq!(sanitize_export_dir("../outside"), "");
        assert_eq!(sanitize_export_dir("renders/../outside"), "");
    }

    #[test]
    fn export_roots_follow_the_directory_choice() {
        use std::path::PathBuf;
        // Empty = the historical split.
        assert_eq!(export_root_for("", false), PathBuf::from("export"));
        assert_eq!(export_root_for("", true), PathBuf::from("out"));
        // Any set root is the single root for both classes.
        assert_eq!(export_root_for("export", true), PathBuf::from("export"));
        assert_eq!(export_root_for("out", false), PathBuf::from("out"));
        assert_eq!(
            export_root_for("renders", true),
            PathBuf::from("renders")
        );
        // The display form the recaps print.
        assert_eq!(export_root_display("", false), "./export/");
        assert_eq!(export_root_display("", true), "./out/");
        assert_eq!(export_root_display("renders", false), "./renders/");
    }

    #[test]
    fn export_quality_round_trips_through_the_w2_42_vocabulary() {
        use stykanimate_rs::export::EncodeQuality;
        // The persisted strings are EncodeQuality::name()'s
        // own spellings — one vocabulary, two seats.
        assert_eq!(export_quality_from_persisted("draft"), EncodeQuality::Draft);
        assert_eq!(
            export_quality_from_persisted("standard"),
            EncodeQuality::Standard
        );
        assert_eq!(
            export_quality_from_persisted("master"),
            EncodeQuality::Master
        );
        // Unknown names fall back to the default level,
        // which is Standard — the pre-option encode.
        assert_eq!(
            export_quality_from_persisted("bogus"),
            stykanimate_rs::export::DEFAULT_ENCODE_QUALITY
        );
        assert_eq!(
            export_quality_from_persisted("bogus"),
            EncodeQuality::Standard
        );
        // Keys and labels cover the closed vocabulary.
        assert_eq!(export_quality_key(EncodeQuality::Draft), "settings-export-quality-draft");
        assert_eq!(export_quality_key(EncodeQuality::Standard), "settings-export-quality-standard");
        assert_eq!(export_quality_key(EncodeQuality::Master), "settings-export-quality-master");
        assert_eq!(export_quality_label(EncodeQuality::Master), "Master");
    }

    #[test]
    fn export_defaults_from_prefs_sanitises_every_field() {
        use stykanimate_rs::shell::LayoutPrefs;
        // The struct default is the historical behaviour.
        assert_eq!(
            export_defaults_from_prefs(&LayoutPrefs::default()),
            ExportDefaults::default()
        );
        // Stored choices survive as chosen.
        let prefs = LayoutPrefs {
            export_dir: "renders".to_string(),
            export_naming: "project-date".to_string(),
            export_quality: "master".to_string(),
            ..Default::default()
        };
        let defaults = export_defaults_from_prefs(&prefs);
        assert_eq!(defaults.dir, "renders");
        assert_eq!(defaults.naming, ExportNaming::ProjectDate);
        assert_eq!(
            defaults.quality,
            stykanimate_rs::export::EncodeQuality::Master
        );
        // A corrupt set sanitises field by field — the
        // escaping directory, the unknown naming and the
        // unknown quality each fall back on their own.
        let prefs = LayoutPrefs {
            export_dir: "../outside".to_string(),
            export_naming: "bogus".to_string(),
            export_quality: "bogus".to_string(),
            ..Default::default()
        };
        assert_eq!(
            export_defaults_from_prefs(&prefs),
            ExportDefaults::default()
        );
    }

    #[test]
    fn export_example_line_previews_the_combined_choices() {
        let core = core();
        // EditorCore::new's demo project stems to
        // "untitled" (the export panel's own pins).
        assert_eq!(
            export_example_line(&core, &ExportDefaults::default()),
            "Example MP4: ./export/untitled.mp4 · Standard quality"
        );
        let dated = ExportDefaults {
            naming: ExportNaming::ProjectDate,
            ..ExportDefaults::default()
        };
        assert_eq!(
            export_example_line(&core, &dated),
            "Example MP4: ./export/untitled-YYYY-MM-DD.mp4 · Standard quality"
        );
        let legacy_out = ExportDefaults {
            dir: "out".to_string(),
            naming: ExportNaming::Legacy,
            quality: stykanimate_rs::export::EncodeQuality::Master,
        };
        assert_eq!(
            export_example_line(&core, &legacy_out),
            "Example MP4: ./out/animation.mp4 · Master quality"
        );
    }

    #[test]
    fn export_defaults_setters_persist_and_report() {
        let mut state = shell_state();
        // Quality: stored under EncodeQuality::name()'s
        // spelling, reported on the status line.
        set_export_quality(
            &mut state,
            stykanimate_rs::export::EncodeQuality::Master,
        );
        assert_eq!(state.core.status, "Export quality: Master");
        assert_eq!(
            export_defaults().quality,
            stykanimate_rs::export::EncodeQuality::Master
        );
        // Naming and directory ride the same round-trip.
        set_export_naming(&mut state, ExportNaming::ProjectDate);
        assert_eq!(state.core.status, "Export naming: Name + date");
        assert_eq!(export_defaults().naming, ExportNaming::ProjectDate);
        set_export_dir(&mut state, ExportDirChoice::Out);
        assert_eq!(state.core.status, "Export directory: out/");
        assert_eq!(export_defaults().dir, "out");
        // Restore the historical defaults so the shared
        // prefs file is left as the other tests expect.
        set_export_quality(
            &mut state,
            stykanimate_rs::export::DEFAULT_ENCODE_QUALITY,
        );
        set_export_naming(&mut state, ExportNaming::Project);
        set_export_dir(&mut state, ExportDirChoice::Default);
        assert_eq!(export_defaults(), ExportDefaults::default());
        // The open card builds with the section mounted.
        state.settings.open = true;
        let t = rune::tok::theme(true);
        let _ = settings_popover(&state, &t);
    }

    #[test]
    fn export_defaults_keys_are_unique() {
        let mut keys: Vec<&str> = vec![
            "settings-export-dir-row",
            "settings-export-naming-row",
            "settings-export-quality-row",
            "settings-export-example",
        ];
        keys.extend(ExportDirChoice::all().iter().map(|c| c.key()));
        keys.extend(ExportNaming::all().iter().map(|n| n.key()));
        keys.push(export_quality_key(stykanimate_rs::export::EncodeQuality::Draft));
        keys.push(export_quality_key(stykanimate_rs::export::EncodeQuality::Standard));
        keys.push(export_quality_key(stykanimate_rs::export::EncodeQuality::Master));
        let total = keys.len();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), total);
        assert_eq!(total, 13);
    }

    #[test]
    fn export_defaults_rows_wrap_inside_the_card() {
        // Width audit (the Wave 10 model): each row is
        // its label (charged at the label model) plus its
        // chips (button_width), with 6 pt gaps. A row may
        // exceed the default card width only if its
        // wrapped packing fits — and every line must fit
        // the narrowest column too.
        let rows: Vec<Vec<f32>> = vec![
            {
                let mut v = vec![button_label_width("Directory")];
                v.extend(
                    ExportDirChoice::all()
                        .iter()
                        .map(|c| button_width(c.label())),
                );
                v
            },
            {
                let mut v = vec![button_label_width("Naming")];
                v.extend(
                    ExportNaming::all().iter().map(|n| button_width(n.label())),
                );
                v
            },
            {
                let mut v = vec![button_label_width("Quality")];
                v.extend(
                    [
                        stykanimate_rs::export::EncodeQuality::Draft,
                        stykanimate_rs::export::EncodeQuality::Standard,
                        stykanimate_rs::export::EncodeQuality::Master,
                    ]
                    .iter()
                    .map(|&q| button_width(export_quality_label(q))),
                );
                v
            },
        ];
        // The quality row's one-line width, pinned: label
        // 42 + chips 51 / 75 / 64 + three gaps.
        let quality_one_line: f32 =
            rows[2].iter().sum::<f32>() + 6.0 * (rows[2].len() - 1) as f32;
        assert_eq!(quality_one_line, 250.0);
        for widths in &rows {
            // No single item is wider than the narrowest
            // card, so wrapping can always place it.
            let widest = widths.iter().copied().fold(0.0f32, f32::max);
            assert!(widest <= CARD_CONTENT_W_MIN, "{widest}");
            for line in pack_lines(widths, 6.0, CARD_CONTENT_W) {
                assert!(line <= CARD_CONTENT_W, "row wraps in card: {line}");
            }
            for line in pack_lines(widths, 6.0, CARD_CONTENT_W_MIN) {
                assert!(line <= CARD_CONTENT_W_MIN, "row at min width: {line}");
            }
        }
    }

    // ---- Autosave & recovery (Roadmap Wave 2, W2-63) ----

    #[test]
    fn autosave_settings_from_prefs_sanitises_every_field() {
        use stykanimate_rs::shell::LayoutPrefs;

        // The defaults are the historical behaviour:
        // on, 25 edits, one generation.
        assert_eq!(
            autosave_settings_from_prefs(&LayoutPrefs::default()),
            AutosaveSettings::default()
        );
        // Out-of-range numbers clamp to the writer's
        // real ranges, so the card never renders a value
        // the writer would not honour.
        let mut prefs = LayoutPrefs::default();
        prefs.autosave_enabled = false;
        prefs.autosave_every = 999;
        prefs.recovery_keep = 0;
        let settings = autosave_settings_from_prefs(&prefs);
        assert!(!settings.enabled);
        assert_eq!(settings.every, stykanimate_rs::autosave::MAX_EVERY);
        assert_eq!(settings.keep, stykanimate_rs::autosave::MIN_KEEP);
        // An in-range custom cadence survives as itself
        // (and lights no chip — the from_edits miss is
        // the card's honest rendering of it).
        prefs.autosave_every = 30;
        prefs.recovery_keep = 4;
        let settings = autosave_settings_from_prefs(&prefs);
        assert_eq!(settings.every, 30);
        assert_eq!(AutosaveInterval::from_edits(settings.every), None);
        assert_eq!(settings.keep, 4);
        assert_eq!(RecoveryKeep::from_count(settings.keep), None);
    }

    #[test]
    fn autosave_setters_persist_mirror_and_report() {
        let mut state = shell_state();
        // Interval: persisted as the plain number,
        // mirrored into the core's live policy, reported
        // on the status line.
        set_autosave_interval(&mut state, AutosaveInterval::Every50);
        assert_eq!(state.core.status, "Autosave interval: 50 edits");
        assert_eq!(autosave_settings().every, 50);
        assert_eq!(state.core.autosave.every, 50);
        // Retention rides the same round-trip.
        set_recovery_keep(&mut state, RecoveryKeep::Three);
        assert_eq!(state.core.status, "Recovery copies kept: 3 copies");
        assert_eq!(autosave_settings().keep, 3);
        assert_eq!(state.core.autosave.keep, 3);
        // The switch flips from the persisted value and
        // the core mirror follows it. Pin the starting
        // value first: the prefs file is shared with
        // every other load-modify-save writer in this
        // binary, so a disk re-read here can lose to a
        // concurrent stale save — only the status and
        // the core mirror (both derived from the flipped
        // prefs in memory) are race-free assertions. The
        // persisted round-trip is already pinned above
        // by the interval/keep re-reads.
        let was_enabled = autosave_settings().enabled;
        toggle_autosave(&mut state);
        assert_eq!(
            state.core.status,
            if was_enabled {
                "Autosave off — no new recovery snapshots will be written"
            } else {
                "Autosave on — recovery snapshots resume"
            }
        );
        assert_eq!(state.core.autosave.enabled, !was_enabled);
        toggle_autosave(&mut state);
        assert_eq!(
            state.core.status,
            if was_enabled {
                "Autosave on — recovery snapshots resume"
            } else {
                "Autosave off — no new recovery snapshots will be written"
            }
        );
        assert_eq!(state.core.autosave.enabled, was_enabled);
        // The two toggles net to the starting value; if
        // the file started OFF, flip once more so the
        // shared prefs file is left ON, as the other
        // tests expect.
        if !was_enabled {
            toggle_autosave(&mut state);
        }
        // Restore the historical defaults so the shared
        // prefs file is left as the other tests expect.
        set_autosave_interval(&mut state, AutosaveInterval::Every25);
        set_recovery_keep(&mut state, RecoveryKeep::One);
        // No disk re-read of the restore here: the shared prefs file can lose
        // to a concurrent stale save from another test (same race as the
        // toggle asserts above), so only the in-memory core mirror below
        // proves the restore — the interval/keep re-reads above already
        // prove the setters persist.
        assert_eq!(
            state.core.autosave,
            stykanimate_rs::autosave::AutosavePrefs::default()
        );
        // The open card builds with the section mounted.
        state.settings.open = true;
        let t = rune::tok::theme(true);
        let _ = settings_popover(&state, &t);
    }

    #[test]
    fn autosave_keys_are_unique() {
        let mut keys: Vec<&str> = vec![
            "settings-autosave-toggle",
            "settings-autosave-interval-row",
            "settings-autosave-keep-row",
            "settings-autosave-status",
        ];
        keys.extend(AutosaveInterval::all().iter().map(|i| i.key()));
        keys.extend(RecoveryKeep::all().iter().map(|k| k.key()));
        let total = keys.len();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), total);
        assert_eq!(total, 12);
    }

    #[test]
    fn autosave_status_line_reports_the_disk_facts() {
        let now = std::time::SystemTime::UNIX_EPOCH
            + std::time::Duration::from_secs(200_000);
        // No snapshot: the line says so, whatever the
        // generation count claims.
        assert_eq!(
            autosave_status_line_from(None, 0, now),
            "Recovery snapshot: none yet."
        );
        // Ages: seconds read as "just now", then
        // minutes, hours, days — and the copy count
        // follows the generations present, singular
        // only for exactly one.
        let written = |age_secs: u64| {
            Some(now - std::time::Duration::from_secs(age_secs))
        };
        assert_eq!(
            autosave_status_line_from(written(30), 1, now),
            "Recovery snapshot: saved just now · 1 copy kept"
        );
        assert_eq!(
            autosave_status_line_from(written(300), 2, now),
            "Recovery snapshot: saved 5 min ago · 2 copies kept"
        );
        assert_eq!(
            autosave_status_line_from(written(7_200), 3, now),
            "Recovery snapshot: saved 2 h ago · 3 copies kept"
        );
        assert_eq!(
            autosave_status_line_from(written(172_800), 1, now),
            "Recovery snapshot: saved 2 d ago · 1 copy kept"
        );
    }

    #[test]
    fn autosave_rows_wrap_inside_the_card() {
        // The same width audit as the Export defaults
        // rows: label + chips at the model's widths,
        // 6 pt gaps; every packed line must fit the
        // default and the narrowest card budgets.
        let rows: Vec<Vec<f32>> = vec![
            {
                let mut v = vec![button_label_width("Interval")];
                v.extend(
                    AutosaveInterval::all().iter().map(|i| button_width(i.label())),
                );
                v
            },
            {
                let mut v = vec![button_label_width("Copies")];
                v.extend(RecoveryKeep::all().iter().map(|k| button_width(k.label())));
                v
            },
        ];
        for widths in &rows {
            let widest = widths.iter().copied().fold(0.0f32, f32::max);
            assert!(widest <= CARD_CONTENT_W_MIN, "{widest}");
            for line in pack_lines(widths, 6.0, CARD_CONTENT_W) {
                assert!(line <= CARD_CONTENT_W, "row wraps in card: {line}");
            }
            for line in pack_lines(widths, 6.0, CARD_CONTENT_W_MIN) {
                assert!(line <= CARD_CONTENT_W_MIN, "row at min width: {line}");
            }
        }
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
        // Widths are measured on the DISPLAY label (native
        // name + tier suffix, PL-9) — the text the button
        // actually paints since the tier labels landed.
        let widths: Vec<f32> = entries
            .iter()
            .map(|e| button_width(&e.display_label()))
            .collect();
        // The widest single button fits even the
        // narrowest column — wrapping can always place
        // every button, so no label needs truncating.
        // (The widest is es-419's native name + suffix.)
        let widest = widths.iter().copied().fold(0.0f32, f32::max);
        assert_eq!(widest, 216.0);
        assert!(widest <= CARD_CONTENT_W_MIN, "{widest} must fit the narrowest card");

        // Per chunk row, in grid order: the one-line
        // width the row painted at before the fix (the
        // remainder, proven — every row is over), and
        // the wrapped packing the `.w_full()` row now
        // produces (every line inside the card).
        let expected_one_line = [493.0, 511.0, 582.0, 535.0, 569.0, 609.0, 571.0];
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
        // Row 1 is 220, not 222 (Audio 54 + Build 51 + 3D 36 +
        // Export 61 + 18 gaps); the third chunk is Home alone.
        let expected_one_line = [258.0, 220.0, 53.0];
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
