//! Scene & weather panel (Rune Phase D, section 7 of 40).
//! Declared from the shell's section registry — bin-crate code,
//! not part of the engine library.
//!
//! Honest wiring note: there are **no** weather, stage-size or
//! background ops in `editor.rs`. The GPUI app's weather panel
//! documented this itself: its listeners wrote
//! `core.project.weather` directly and reported through
//! `core.status`, and its stage steppers called a view-local
//! helper that wrote `core.project.stage`. Every handler here
//! does exactly the same through `s.core` — same fields, same
//! clamps, same status strings — so behaviour is identical,
//! only the toolkit changed.
//!
//! Stage size is a plain project field (`Project::stage`):
//! the exporters read it as the render size, and the stage
//! steppers have always written it directly. The Phase F
//! preset row writes it exactly the same way. Resizing —
//! by stepper or preset — changes the canvas only: figure
//! positions are absolute stage coordinates and are NOT
//! rescaled to fit the new size (the panel says so in a
//! note line under the presets).
//!
//! The two GPUI buttons both labelled "Weather", disambiguated
//! by their listener bodies (`git show a5e0b18:src/app.rs`):
//! - `btn("weather")` in the Scene group **cycled** the kind
//!   (Off → Rain → Snow → Fireflies → Off, per-kind presets).
//!   Here it is the button labelled "Cycle weather", key
//!   `weather`, doing exactly that cycle.
//! - `btn("weather-panel")` only **toggled panel visibility**
//!   (`show_weather`); it never touched the weather itself.
//!   Panel visibility is shell view-state, so that toggle is
//!   not this section's control to build.
//! The panel's own kind picker (the GPUI weather panel's
//! segmented control) is the row of kind buttons below: picking
//! a kind keeps the tuned amount/wind when switching, and uses
//! the cycle button's per-kind preset when enabling from off.
//!
//! The kind buttons carry Font Awesome glyphs (Phase H):
//! each segment is an `icon_btn` showing the kind's glyph
//! beside its unchanged label — Off (clear sky) `sun`, Rain
//! `cloud-rain`, Snow `snowflake`, Fireflies `star`, Fog
//! `cloud`, Storm `bolt` (see [`kind_icon`]). Keys, labels,
//! active state and handlers are the text buttons' own;
//! only the leading glyph is new.
//!
//! Amount and Wind also have drag sliders (Phase E). The
//! track follows the Rune settings example's `speed_control`
//! (`examples/settings/src/main.rs`): `on_drag` with the
//! press value stored in `Drag::data`, field writes on every
//! Move, `tx.instant()` while scrubbing, and a cancelled
//! drag restoring the press value. Two deliberate
//! differences: the value comes from the pure [`amount_at`] /
//! [`wind_at`] mapping over the element's own width (no grab
//! offset — the knob is small, and a track click jumps, as
//! in the example), and the status line updates **only on
//! release** — mid-drag writes stay silent so a scrub does
//! not spam the status line or the toasts fed from it. The
//! steppers stay for fine control.
//!
//! Custom background colour (Phase G): the 16 swatches stay
//! the quick path; three RGB stepper rows under the
//! Background header fine-tune `project.background` one
//! channel at a time, for colours no swatch has. The
//! channel arithmetic is NOT re-implemented here — it is
//! imported from `inspector_style` (`rgb_split` / `rgb_join`
//! / `step_channel`, the segment style panel's helpers:
//! step 8, clamped 0..=255), so both panels step colours
//! identically, from one source of truth. The write itself
//! keeps this panel's own discipline: the stepped colour
//! goes through [`set_background`], the same direct field
//! write + status string the swatches use, and — per the
//! snapshot note below — no undo snapshot (the stage-preset
//! write, `set_stage_preset`, snapshots nothing either;
//! nothing in this module does). Stepping off a swatch
//! colour simply leaves every swatch unhighlighted while
//! the header shows the raw hex; stepping back onto a
//! swatch colour lights it again, because the highlight is
//! an exact-equality check on the field.
//!
//! Stage readout (Phase I): under the preset row, a muted
//! line ([`stage_readout`]) names the size the panel is
//! actually set to — `960×720 · 4:3` — so a stage nudged
//! off every preset still says what it is, aspect included,
//! instead of the user reading two bare stepper values.
//!
//! Frame rate + fine stage steppers (Phase K): a "Frame
//! rate" row (`scene-fps-row`) steps `project.fps` by 1
//! through the editor op `set_project_fps`, and a second
//! stage-size block (`scene-stage-size-row`) steps each
//! axis by 20 through `set_stage_size` — finer steps than
//! the ±40 `adjust_stage` rows above them, which stay as
//! they were. Unlike this module's own writes, these two
//! rows delegate to editor ops (§12), so the write, the
//! clamping ([1, 60] fps, [64, 4096] per axis) and the
//! status wording are the ops' — the panel only computes
//! the stepped value ([`fps_after`] / [`stage_axis_after`],
//! mirroring the ops' clamps so the arithmetic is testable
//! here) and hands it over. The readout grows the rate as
//! a suffix: `960×720 · 4:3 · 12 fps`.
//!
//! Background presets (Phase L): a row of six named
//! colours ([`BG_PRESETS`], row key `scene-bg-presets`)
//! sits under the Background header, above the RGB
//! steppers. The chips mirror the swatch idiom below —
//! same 26×26 slot, accent ring on the exact-equality
//! match ([`bg_preset_active`]), 22×22 colour core — but
//! the keys are positional (`scene-bg-preset-{i}`),
//! because two preset names repeat swatch names with
//! different colours (the preset Sky and Forest are not
//! the swatch Sky and Forest). A press writes through
//! [`set_background`], the swatches' own handler body:
//! same field write, same status string, no snapshot.
//! The first preset, Midnight, is the engine default
//! (`Project::new_demo`'s `background`, 0x232830), so a
//! fresh project lights exactly one preset chip.
//!
//! No undo snapshots, on purpose: weather writes were never
//! snapshotted. The GPUI listeners wrote `project.weather`
//! directly (the wiring note above), and in this engine a
//! snapshot exists only where a backing fn calls
//! `EditorCore::snapshot()` — none of this module's fns do,
//! steppers included. A drag therefore costs exactly as
//! many undo entries as the steppers: zero.
//!
//! Phase N (adoption) restyles the panel to the charter's
//! card language: the root is the kit's `design::panel_col`,
//! the header is `design::panel_title` over a kit divider,
//! local labels are `design::section_label`, and steppers
//! use the charter order with `design::stepper_value`
//! readouts. The Amount slider's visual is the kit's
//! unipolar `design::slider_track`; the Wind slider keeps
//! its local bipolar centre-fill and centre tick (the
//! ratified exception) on the kit rail/knob colours. Both
//! drag roots grow from 22 to 28 pt tall. The named
//! background presets are kit `chip_btn`s, superseding the
//! Phase L colour-core chip construction. Keys, handlers,
//! status strings and value formatters are unchanged.
//!
//! Placed-media readout (Wave 12, B5; Wave 13, A7): the
//! panel ends with a read-only "Placed media" section
//! ([`media_readout`] + [`media_note`]). In Wave 12 no
//! sprite surface existed in the shell, so the readout
//! was the only place placed sprites were visible at
//! all, and its note warned that they rendered in
//! exports only and could not be edited. Wave 13 lands
//! the real surface — engine ops in `sprite_ops.rs`
//! (A1) and the Sprites panel (sprites_panel.rs, A3) —
//! so this section is now the entry point: the counts
//! stay, and the note points to the Sprites panel
//! instead of claiming sprites are uneditable. The
//! section remains deliberately read-only — no buttons,
//! nothing that pretends to edit here; editing lives in
//! the Sprites panel.

use rune::tok::{self, type_, Theme};
use rune::{col, el, row, text, Drag, El, Phase, Tx};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::vfx::{Weather, WeatherKind};

use crate::chrome_view::btn;
use crate::rune_sec::design;
use crate::rune_sec::icon_btn::{chip_btn, icon_btn};
use crate::rune_sec::inspector_style::{rgb_join, rgb_split, step_channel};
use crate::stage_view::hex_color;
use crate::State;

/// Segment order for the kind picker: index 0 is Off, 1..=5
/// are the kinds (the GPUI segmented control's order).
pub const KIND_LABELS: [&str; 6] = ["Off", "Rain", "Snow", "Fireflies", "Fog", "Storm"];

/// The kinds behind segments 1..=5, in `KIND_LABELS` order.
pub const KINDS: [WeatherKind; 5] = [
    WeatherKind::Rain,
    WeatherKind::Snow,
    WeatherKind::Fireflies,
    WeatherKind::Fog,
    WeatherKind::Storm,
];

/// Display name of a weather kind (the GPUI panel's table).
pub fn kind_name(kind: WeatherKind) -> &'static str {
    match kind {
        WeatherKind::Rain => "Rain",
        WeatherKind::Snow => "Snow",
        WeatherKind::Fireflies => "Fireflies",
        WeatherKind::Fog => "Fog",
        WeatherKind::Storm => "Storm",
    }
}

/// Font Awesome glyph for a kind picker's segment (Phase
/// H): Off is the clear sky (`sun`), and each kind gets the
/// glyph that pictures it — rain cloud, snowflake, a star
/// for the fireflies' glow, a plain cloud for fog, and the
/// lightning bolt for the storm. Every segment has an
/// honest glyph, so no picker button stays text-only.
pub fn kind_icon(kind: Option<WeatherKind>) -> &'static str {
    match kind {
        None => "sun",
        Some(WeatherKind::Rain) => "cloud-rain",
        Some(WeatherKind::Snow) => "snowflake",
        Some(WeatherKind::Fireflies) => "star",
        Some(WeatherKind::Fog) => "cloud",
        Some(WeatherKind::Storm) => "bolt",
    }
}

/// Segment index of the current weather (0 = off) — the
/// picker's active segment.
pub fn kind_index(weather: Option<Weather>) -> usize {
    match weather {
        None => 0,
        Some(w) => match w.kind {
            WeatherKind::Rain => 1,
            WeatherKind::Snow => 2,
            WeatherKind::Fireflies => 3,
            WeatherKind::Fog => 4,
            WeatherKind::Storm => 5,
        },
    }
}

/// The preset applied when a kind is enabled from off (the
/// Scene cycle button's per-kind values; the panel reuses
/// them). Switching kinds keeps the tuned amount/wind instead
/// — see [`set_weather_kind`].
pub fn kind_preset(kind: WeatherKind) -> Weather {
    match kind {
        WeatherKind::Rain => Weather { kind, amount: 90.0, wind: 25.0 },
        WeatherKind::Snow => Weather { kind, amount: 90.0, wind: 12.0 },
        WeatherKind::Fireflies => Weather { kind, amount: 60.0, wind: 4.0 },
        WeatherKind::Fog => Weather { kind, amount: 60.0, wind: 0.0 },
        WeatherKind::Storm => Weather { kind, amount: 140.0, wind: 30.0 },
    }
}

/// The panel subtitle (the GPUI panel's summary format).
pub fn weather_summary(weather: Option<Weather>) -> String {
    match weather {
        None => "Off — pick a kind".to_string(),
        Some(w) => format!(
            "{} · amount {:.0} · wind {}",
            kind_name(w.kind),
            w.amount,
            w.wind
        ),
    }
}

/// Pick a kind (or Off) — the segmented picker's handler body.
/// Switching kinds keeps the tuned amount/wind; enabling from
/// off applies the kind's preset. Status strings are the GPUI
/// listeners' own.
pub fn set_weather_kind(core: &mut EditorCore, kind: Option<WeatherKind>) {
    // Wave 13 (census E3): no undo snapshots (the module's
    // documented weather discipline), but a real change
    // counts for dirty/autosave via the seam.
    let before = core.project.weather;
    match kind {
        None => {
            core.project.weather = None;
            core.status = "Weather: off.".into();
        }
        Some(kind) => {
            core.project.weather = Some(match core.project.weather {
                Some(mut w) => {
                    w.kind = kind;
                    w
                }
                None => kind_preset(kind),
            });
            core.status = match kind {
                WeatherKind::Rain => "Weather: rain.",
                WeatherKind::Snow => "Weather: snow.",
                WeatherKind::Fireflies => "Weather: fireflies.",
                WeatherKind::Fog => "Weather: fog.",
                WeatherKind::Storm => "Weather: storm.",
            }
            .into();
        }
    }
    if core.project.weather != before {
        core.mark_dirty();
    }
}

/// The Scene group's old `weather` button, truthfully: cycle
/// Off → Rain → Snow → Fireflies → Off, applying each kind's
/// preset wholesale (unlike the picker, cycling does not keep
/// tuned amount/wind — the GPUI listener built a fresh
/// `Weather` per step). Fog and Storm are not in the cycle;
/// from either, cycling turns weather off.
pub fn cycle_weather(core: &mut EditorCore) {
    let (next, label) = match core.project.weather {
        None => (Some(kind_preset(WeatherKind::Rain)), "Weather: rain."),
        Some(w) => match w.kind {
            WeatherKind::Rain => (Some(kind_preset(WeatherKind::Snow)), "Weather: snow."),
            WeatherKind::Snow => {
                (Some(kind_preset(WeatherKind::Fireflies)), "Weather: fireflies.")
            }
            WeatherKind::Fireflies | WeatherKind::Fog | WeatherKind::Storm => {
                (None, "Weather: off.")
            }
        },
    };
    // Wave 13 (census E3): mark on a real change only.
    if core.project.weather != next {
        core.project.weather = next;
        core.mark_dirty();
    }
    core.status = label.into();
}

/// Density stepper backing: adjust the particle amount by
/// `delta`, clamped to 0..=200 (the `Weather` doc's range).
/// With weather off this is status-only, as in the GPUI panel.
pub fn step_weather_amount(core: &mut EditorCore, delta: f32) {
    match core.project.weather {
        Some(mut w) => {
            // Wave 13 (census E3): a step clamped to the
            // stored value marks nothing.
            let stepped = (w.amount + delta).clamp(0.0, 200.0);
            if stepped != w.amount {
                w.amount = stepped;
                core.project.weather = Some(w);
                core.mark_dirty();
            }
            core.status = format!("Weather amount: {:.0}.", w.amount);
        }
        None => core.status = "Weather is off — pick a kind first.".into(),
    }
}

/// Wind stepper backing: adjust drift (stage units per
/// second) by `delta`, clamped to -200..=200 (at 200 a
/// particle crosses the 960-wide stage in under five seconds,
/// and rain's slant is already a third of its maximum).
pub fn step_weather_wind(core: &mut EditorCore, delta: f32) {
    match core.project.weather {
        Some(mut w) => {
            // Wave 13 (census E3): as `step_weather_amount`.
            let stepped = (w.wind + delta).clamp(-200.0, 200.0);
            if stepped != w.wind {
                w.wind = stepped;
                core.project.weather = Some(w);
                core.mark_dirty();
            }
            core.status = format!("Weather wind: {}.", w.wind);
        }
        None => core.status = "Weather is off — pick a kind first.".into(),
    }
}

/// Slider mapping for Amount: the value at local `x` on a
/// track `w` points wide — 0 at the left edge, 200 at the
/// right, linear between, clamped outside. A zero-width
/// track (no layout yet) maps to 0 rather than dividing by
/// zero.
pub fn amount_at(x: f32, w: f32) -> i32 {
    if w <= 0.0 {
        return 0;
    }
    ((x / w).clamp(0.0, 1.0) * 200.0).round() as i32
}

/// Slider mapping for Wind: −200 at the left edge, 0 at the
/// centre, +200 at the right edge, clamped outside — the
/// exact mirror of [`amount_at`] about the centre.
pub fn wind_at(x: f32, w: f32) -> i32 {
    if w <= 0.0 {
        return 0;
    }
    ((x / w).clamp(0.0, 1.0) * 400.0 - 200.0).round() as i32
}

/// Track fill fraction for an amount (0.0 at 0, 1.0 at 200).
pub fn amount_frac(amount: f32) -> f32 {
    (amount / 200.0).clamp(0.0, 1.0)
}

/// Track fill fraction for a wind value (0.0 at −200, 0.5
/// at 0, 1.0 at +200).
pub fn wind_frac(wind: f32) -> f32 {
    ((wind + 200.0) / 400.0).clamp(0.0, 1.0)
}

/// Slider backing (amount): set the density to an absolute
/// value with the stepper's clamp. `report` is true only on
/// drag release — mid-drag writes stay silent (see the
/// module docs). With weather off this is status-only, as
/// in the GPUI panel.
pub fn set_weather_amount(core: &mut EditorCore, value: i32, report: bool) {
    match core.project.weather {
        Some(mut w) => {
            w.amount = (value as f32).clamp(0.0, 200.0);
            core.project.weather = Some(w);
            if report {
                core.status = format!("Weather amount: {:.0}.", w.amount);
            }
        }
        None => core.status = "Weather is off — pick a kind first.".into(),
    }
}

/// Slider backing (wind): set the drift to an absolute
/// value with the stepper's clamp; `report` as in
/// [`set_weather_amount`].
pub fn set_weather_wind(core: &mut EditorCore, value: i32, report: bool) {
    match core.project.weather {
        Some(mut w) => {
            w.wind = (value as f32).clamp(-200.0, 200.0);
            core.project.weather = Some(w);
            if report {
                core.status = format!("Weather wind: {}.", w.wind);
            }
        }
        None => core.status = "Weather is off — pick a kind first.".into(),
    }
}

/// Amount slider drag (the settings example's shape): Down
/// stores the press value in `Drag::data` and jumps to the
/// pressed point, Move rewrites the field and asks for an
/// instant redraw, Up reports — or, when the gesture was
/// cancelled, restores the press value. With weather off,
/// Down only posts the off status.
fn amount_drag(s: &mut State, d: &mut Drag, tx: &mut Tx) {
    let core = &mut s.core;
    match d.phase {
        Phase::Down => {
            d.data = core.project.weather.map(|w| w.amount).unwrap_or(0.0);
            if core.project.weather.is_none() {
                core.status = "Weather is off — pick a kind first.".into();
            } else {
                set_weather_amount(core, amount_at(d.x, d.w), false);
            }
        }
        Phase::Move => {
            if core.project.weather.is_some() {
                set_weather_amount(core, amount_at(d.x, d.w), false);
                tx.instant();
            }
        }
        Phase::Up => {
            if d.cancelled() {
                if let Some(mut w) = core.project.weather {
                    w.amount = d.data;
                    core.project.weather = Some(w);
                }
            } else if core.project.weather.is_some() {
                set_weather_amount(core, amount_at(d.x, d.w), true);
                // Wave 13 (census E4): one counted edit per
                // gesture — the setters never mark (a drag
                // fires them per Move); at Up, a final value
                // that differs from the press value stashed
                // in `d.data` marks exactly once. A cancelled
                // drag restored the press value above and
                // marks nothing.
                if core.project.weather.map(|w| w.amount) != Some(d.data) {
                    core.mark_dirty();
                }
            }
        }
    }
}

/// Wind slider drag — [`amount_drag`] over the wind field
/// and mapping.
fn wind_drag(s: &mut State, d: &mut Drag, tx: &mut Tx) {
    let core = &mut s.core;
    match d.phase {
        Phase::Down => {
            d.data = core.project.weather.map(|w| w.wind).unwrap_or(0.0);
            if core.project.weather.is_none() {
                core.status = "Weather is off — pick a kind first.".into();
            } else {
                set_weather_wind(core, wind_at(d.x, d.w), false);
            }
        }
        Phase::Move => {
            if core.project.weather.is_some() {
                set_weather_wind(core, wind_at(d.x, d.w), false);
                tx.instant();
            }
        }
        Phase::Up => {
            if d.cancelled() {
                if let Some(mut w) = core.project.weather {
                    w.wind = d.data;
                    core.project.weather = Some(w);
                }
            } else if core.project.weather.is_some() {
                set_weather_wind(core, wind_at(d.x, d.w), true);
                // Wave 13 (census E4): as `amount_drag` —
                // one mark per gesture, at Up.
                if core.project.weather.map(|w| w.wind) != Some(d.data) {
                    core.mark_dirty();
                }
            }
        }
    }
}

/// Stage resize backing (the GPUI view's `adjust_stage`):
/// each axis moves by its delta and clamps to 200..=4000,
/// reporting the new size in the status line.
pub fn adjust_stage(core: &mut EditorCore, dw: f32, dh: f32) {
    // Wave 13 (census E5): the settings-tweak family —
    // no undo step, but a step that actually moves an
    // axis counts for dirty/autosave; a fully-clamped
    // step marks nothing.
    let before = core.project.stage;
    core.project.stage[0] = (core.project.stage[0] + dw).clamp(200.0, 4000.0);
    core.project.stage[1] = (core.project.stage[1] + dh).clamp(200.0, 4000.0);
    if core.project.stage != before {
        core.mark_dirty();
    }
    core.status = format!(
        "Stage size: {} x {}.",
        core.project.stage[0] as i32, core.project.stage[1] as i32
    );
}

/// Stage-size presets (Phase F): (label, width, height).
/// The first is the engine's own default (`Project::stage`
/// in `model.rs`), labelled "Default". Every size sits
/// inside the stepper clamp (200..=4000 per axis), so a
/// preset write needs no clamping of its own.
pub const STAGE_PRESETS: [(&str, f32, f32); 6] = [
    ("Default", 960.0, 720.0),
    ("HD wide", 1280.0, 720.0),
    ("HD tall", 720.0, 1280.0),
    ("Full HD", 1920.0, 1080.0),
    ("Full HD tall", 1080.0, 1920.0),
    ("Small", 640.0, 480.0),
];

/// True when `stage` is exactly the `w` × `h` preset — the
/// preset button's active highlight. Exact comparison is
/// right here: presets write whole numbers, and a stage
/// nudged one unit off a preset by the steppers is honestly
/// no longer that preset.
pub fn stage_matches_preset(stage: [f32; 2], w: f32, h: f32) -> bool {
    stage[0] == w && stage[1] == h
}

/// Stage-preset handler body: write the size directly to
/// the project field (the stepper pattern — no op exists)
/// and report it with [`adjust_stage`]'s status string.
pub fn set_stage_preset(core: &mut EditorCore, w: f32, h: f32) {
    // Wave 13 (census E5): mark only when the size differs.
    if core.project.stage != [w, h] {
        core.project.stage = [w, h];
        core.mark_dirty();
    }
    core.status = format!("Stage size: {} x {}.", w as i32, h as i32);
}

/// Greatest common divisor of two integers (Euclid's
/// algorithm); sign-insensitive, and `gcd(0, n)` is `n`.
fn gcd(a: i64, b: i64) -> i64 {
    let (mut a, mut b) = (a.abs(), b.abs());
    while b != 0 {
        let r = a % b;
        a = b;
        b = r;
    }
    a
}

/// A frame rate as the shell writes it everywhere (the
/// status strip's `fps_text`, the stats panel's
/// `format_fps` — each panel keeps its own copy of this
/// two-liner): whole rates as a plain integer (`12`), a
/// fractional rate at one decimal (`23.5`).
pub fn fps_label(fps: f32) -> String {
    if fps.fract() == 0.0 {
        format!("{}", fps as i64)
    } else {
        format!("{fps:.1}")
    }
}

/// The stage readout (Phase I; rate suffix Phase K):
/// `"{w}×{h} · {aspect} · {fps} fps"` for the project's
/// current stage size and frame rate — for example
/// `960×720 · 4:3 · 12 fps`. Width and height are the
/// field's values **rounded** to whole units (every preset
/// and stepper step is whole, so rounding only ever tidies
/// a loaded file's fractions; truncating would misreport
/// 959.6 as 959). The aspect is the ratio reduced by the
/// gcd when both reduced sides are small integers (≤ 40:
/// `16:9`, `4:3`, `9:16`, `1:1`, …). A size whose reduced
/// ratio still has a side over 40 has no honest
/// small-integer name, so it falls back to ONE decimal
/// form: the width over the height at two decimals,
/// `"{:.2}:1"` — a 1000×333 stage reads
/// `1000×333 · 3.00:1 · 12 fps`. Pure: reads the project
/// fields, writes nothing.
pub fn stage_readout(core: &EditorCore) -> String {
    let w = core.project.stage[0].round() as i64;
    let h = core.project.stage[1].round() as i64;
    let fps = fps_label(core.project.fps);
    let g = gcd(w, h);
    let (rw, rh) = if g > 0 { (w / g, h / g) } else { (w, h) };
    if g > 0 && (1..=40).contains(&rw) && (1..=40).contains(&rh) {
        format!("{w}×{h} · {rw}:{rh} · {fps} fps")
    } else {
        // Degenerate or irreducible sizes: the decimal
        // form, with the divisor guarded so a zero height
        // from a hand-edited file cannot divide by zero.
        let ratio = w as f32 / h.max(1) as f32;
        format!("{w}×{h} · {ratio:.2}:1 · {fps} fps")
    }
}

/// The placed-media readout (Wave 12, B5): one line
/// naming what the project actually carries in its asset
/// table and sprite list — `"{s} sprite(s) placed ·
/// {a} image asset(s)."`, or the empty form when both are
/// zero. Pure: reads `project.sprites` / `project.assets`,
/// writes nothing. The counts are this section's whole
/// job; the instances themselves are listed and edited in
/// the Sprites panel (Wave 13). Assets also enter the
/// table via node media (`EditorCore::attach_image_to_node`),
/// so assets can be nonzero with zero sprites placed.
pub fn media_readout(core: &EditorCore) -> String {
    let sprites = core.project.sprites.len();
    let assets = core.project.assets.len();
    if sprites == 0 && assets == 0 {
        return "No image assets or sprites in this project.".to_string();
    }
    format!(
        "{} sprite{} placed · {} image asset{}.",
        sprites,
        if sprites == 1 { "" } else { "s" },
        assets,
        if assets == 1 { "" } else { "s" },
    )
}

/// The placed-media pointer note (Wave 12, B5; Wave 13,
/// A7): present only when the project carries at least
/// one free-floating sprite. In Wave 12 this note warned
/// that sprites render in exports only and cannot be
/// moved or edited in the app; Wave 13 lands the Sprites
/// panel (sprites_panel.rs, A3), where instances are
/// added, selected, deleted and keyframed (position,
/// rotation, scale), so the note now points there — this
/// readout is the entry point, the panel is the editor.
/// `None` when the project carries no sprites to point at.
pub fn media_note(core: &EditorCore) -> Option<&'static str> {
    if core.project.sprites.is_empty() {
        None
    } else {
        Some(
            "Sprites are edited in the Sprites panel — move, rotate, scale and keyframe them there.",
        )
    }
}

/// The frame rate one stepper press asks for (Phase K):
/// the current rate stepped by `dir` (±1) whole frames,
/// clamped to the `set_project_fps` op's range [1, 60] —
/// the op clamps again on write, so this mirroring only
/// keeps the panel's arithmetic (and its tests) honest
/// about where a press at the edge lands. Pure.
pub fn fps_after(core: &EditorCore, dir: i32) -> f32 {
    (core.project.fps + dir as f32).clamp(1.0, 60.0)
}

/// One frame-rate stepper press (Phase K): hand the
/// stepped rate to the editor op `set_project_fps`, which
/// owns the write (and its status line) — see
/// [`fps_after`] for the value pressed.
pub fn step_fps(core: &mut EditorCore, dir: i32) {
    core.set_project_fps(fps_after(core, dir));
}

/// One stage axis' stepper step (Phase K): finer than the
/// ±40 of the `adjust_stage` rows, for nudging a size the
/// presets do not name.
pub const STAGE_AXIS_STEP: f32 = 20.0;

/// The stage size one axis stepper press asks for
/// (Phase K): axis `axis` (0 = width, 1 = height) stepped
/// by `dir` (±1) × [`STAGE_AXIS_STEP`], the other axis
/// carried over untouched, the stepped axis clamped to
/// the `set_stage_size` op's range [64, 4096] — the op
/// clamps again on write, as with [`fps_after`]. A bad
/// axis index returns the size unchanged (the
/// `step_background_channel` bad-channel discipline).
/// Pure.
pub fn stage_axis_after(core: &EditorCore, axis: usize, dir: i32) -> (f32, f32) {
    let mut stage = core.project.stage;
    if axis < stage.len() {
        stage[axis] = (stage[axis] + dir as f32 * STAGE_AXIS_STEP).clamp(64.0, 4096.0);
    }
    (stage[0], stage[1])
}

/// One stage-axis stepper press (Phase K): hand the
/// stepped size to the editor op `set_stage_size`, which
/// owns the write (and its status line) — see
/// [`stage_axis_after`] for the value pressed. A bad axis
/// index is a no-op: the op is not called at all.
pub fn step_stage_axis(core: &mut EditorCore, axis: usize, dir: i32) {
    if axis > 1 {
        return;
    }
    let (w, h) = stage_axis_after(core, axis, dir);
    core.set_stage_size(w, h);
}

/// Stage background presets: (name, `0xRRGGBB`). The first is
/// the engine's own default (`Project::background` in
/// `model.rs`). The GPUI app never offered a background
/// control — the field was render-only data — so these
/// swatches write `core.project.background` directly, the
/// same field-write pattern the weather controls use.
pub const BACKGROUND_SWATCHES: [(&str, u32); 16] = [
    ("Slate", 0x232830),
    ("Midnight", 0x10131A),
    ("Deep blue", 0x1B2A4A),
    ("Deep green", 0x0E2A1E),
    ("Umber", 0x2E1A12),
    ("Graphite", 0x3A3F4A),
    ("Sky", 0x87B5D6),
    ("Paper", 0xF2EAD8),
    ("White", 0xFFFFFF),
    // Phase E additions: pure black, a warmer paper, richer
    // stage colours (forest, sunset, violet, ocean, sand).
    ("Black", 0x000000),
    ("Warm paper", 0xE8CFA3),
    ("Forest", 0x1E4634),
    ("Sunset", 0xE2703A),
    ("Violet", 0x4B2E63),
    ("Ocean", 0x0F4C81),
    ("Sand", 0xC2A878),
];

/// Background swatch handler body: set the stage background
/// colour and report it in the status line.
pub fn set_background(core: &mut EditorCore, rgb: u32) {
    // Wave 13 (census E7): the appearance-tweak family —
    // no undo step in the panel (the MCP sibling
    // snapshots deliberately; the asymmetry is the
    // census's recorded call), but a real change counts
    // for dirty/autosave. This one site covers swatches,
    // presets and `step_background_channel`.
    let rgb = rgb & 0xFF_FFFF;
    if core.project.background != rgb {
        core.project.background = rgb;
        core.mark_dirty();
    }
    core.status = format!("Background: #{:06X}.", core.project.background);
}

/// Background colour presets (Phase L): (name, `0xRRGGBB`).
/// A named quick path beside the 16-swatch grid: the
/// swatches are bare colours, these six are the curated
/// stage moods. Midnight is the engine's own default —
/// `Project::new_demo`'s `background` field (model.rs) —
/// so a fresh project starts on a lit preset. Two names
/// repeat swatch names at different colours (Sky, Forest):
/// harmless, because preset keys are positional
/// (`scene-bg-preset-{i}`), never name-based.
pub const BG_PRESETS: [(&str, u32); 6] = [
    ("Midnight", 0x232830),
    ("Black", 0x000000),
    ("Paper", 0xF2EDE4),
    ("Sky", 0x87B5D9),
    ("Forest", 0x1E3327),
    ("Plum", 0x2A2138),
];

/// Is preset `i` the project's current background? The
/// highlight check, factored out of the builder: an
/// exact-equality match on `project.background`, the same
/// discipline the swatch ring uses, so a colour reached
/// by stepper or swatch lights its preset too — and a
/// stepped-off colour lights none. An out-of-range index
/// is never active.
pub fn bg_preset_active(core: &EditorCore, i: usize) -> bool {
    match BG_PRESETS.get(i) {
        Some(&(_, rgb)) => core.project.background == rgb,
        None => false,
    }
}

/// One background RGB stepper press (Phase G): step channel
/// `channel` (0 = R, 1 = G, 2 = B) of `project.background`
/// by [`step_channel`]'s step (8, clamped 0..=255) in
/// direction `dir` (−1 / +1) and write the composed colour
/// back through [`set_background`] — the swatches' own
/// direct field write, so a stepped colour and a swatch
/// colour land identically, status string included, and
/// with this module's usual zero undo snapshots. A bad
/// channel index is a no-op (the `inspector_style`
/// stepper's discipline).
pub fn step_background_channel(core: &mut EditorCore, channel: usize, dir: i32) {
    let (r, g, b) = rgb_split(core.project.background);
    let (r, g, b) = match channel {
        0 => (step_channel(r, dir), g, b),
        1 => (r, step_channel(g, dir), b),
        2 => (r, g, step_channel(b, dir)),
        _ => return,
    };
    set_background(core, rgb_join(r, g, b));
}

/// One stepper row: label, − button, value, + button (the
/// GPUI weather panel's row shape). Keys carry the GPUI ids.
fn stepper_row(
    label: &str,
    value: String,
    dn_key: &'static str,
    up_key: &'static str,
    t: &Theme,
    dn: impl Fn(&mut State) + 'static,
    up: impl Fn(&mut State) + 'static,
) -> El {
    row()
        .gap(6.0)
        .center_y()
        .child(design::section_label(label, t).grow(1.0))
        .child(btn(dn_key, "−", false, t, dn))
        .child(design::stepper_value(&value, t))
        .child(btn(up_key, "+", false, t, up))
}

/// One background RGB channel stepper row (Phase G): channel
/// letter, the channel's current value, and − / + buttons
/// stepping it via [`step_background_channel`] — the
/// `inspector_style` channel row's shape, with scene keys:
/// row `scene-bg-<letter>`, buttons `scene-bg-<letter>-dn`
/// / `scene-bg-<letter>-up`.
fn bg_channel_row(channel: usize, label: &str, value: u8, t: &Theme) -> El {
    let lower = label.to_lowercase();
    row()
        .key(format!("scene-bg-{lower}"))
        .gap(8.0)
        .center_y()
        .child(design::section_label(label, t).grow(1.0))
        .child(btn(&format!("scene-bg-{lower}-dn"), "−", false, t, move |s| {
            step_background_channel(&mut s.core, channel, -1)
        }))
        .child(design::stepper_value(&value.to_string(), t))
        .child(btn(&format!("scene-bg-{lower}-up"), "+", false, t, move |s| {
            step_background_channel(&mut s.core, channel, 1)
        }))
}

/// The frame-rate stepper row (Phase K): the
/// `bg_channel_row` shape — label, current value, − / +
/// buttons — keyed `scene-fps-row`, buttons
/// `scene-fps-dn` / `scene-fps-up`, each press stepping
/// the project rate by 1 via [`step_fps`].
fn fps_row(core: &EditorCore, t: &Theme) -> El {
    row()
        .key("scene-fps-row")
        .gap(8.0)
        .center_y()
        .child(design::section_label("Frame rate", t).grow(1.0))
        .child(btn("scene-fps-dn", "−", false, t, |s| {
            step_fps(&mut s.core, -1)
        }))
        .child(design::stepper_value(&fps_label(core.project.fps), t))
        .child(btn("scene-fps-up", "+", false, t, |s| {
            step_fps(&mut s.core, 1)
        }))
}

/// One fine stage-axis stepper row (Phase K): the
/// `bg_channel_row` shape again — label, current value,
/// − / + buttons stepping one axis by
/// [`STAGE_AXIS_STEP`] via [`step_stage_axis`]. Buttons
/// `scene-stage-w-dn` / `scene-stage-w-up` for the width,
/// `scene-stage-h-dn` / `scene-stage-h-up` for the
/// height; the two rows sit in the block keyed
/// `scene-stage-size-row`.
fn stage_axis_row(axis: usize, label: &str, value: f32, t: &Theme) -> El {
    let (dn_key, up_key) = if axis == 0 {
        ("scene-stage-w-dn", "scene-stage-w-up")
    } else {
        ("scene-stage-h-dn", "scene-stage-h-up")
    };
    row()
        .gap(8.0)
        .center_y()
        .child(design::section_label(label, t).grow(1.0))
        .child(btn(dn_key, "−", false, t, move |s| {
            step_stage_axis(&mut s.core, axis, -1)
        }))
        .child(design::stepper_value(&format!("{}", value as i32), t))
        .child(btn(up_key, "+", false, t, move |s| {
            step_stage_axis(&mut s.core, axis, 1)
        }))
}

/// Slider track width and knob diameter (the settings
/// example's proportions, slightly wider).
const SLIDER_W: f32 = 200.0;
/// Slider drag-root height. 28 since Phase N (P18: a
/// 22 pt drag root fell under the 24 pt hit-target floor;
/// the drag maps x only, so growing the root is safe).
/// The layers panel's copy of this const grows to the
/// same 28 in its own adoption, so the two sliders do
/// not drift.
const SLIDER_H: f32 = 28.0;
const KNOB: f32 = 16.0;
/// Height of the kit's visual-only slider track
/// (`design::slider_track`), centred inside the taller
/// drag root for the unipolar Amount slider.
const KIT_SLIDER_TRACK_H: f32 = 22.0;

/// One weather drag slider (the settings example's
/// `speed_control` shape): a fixed-size drag root whose
/// visual is drawn inside it. The unipolar Amount slider
/// delegates its track, fill and knob to the kit's
/// `design::slider_track`. The Wind slider is the
/// ratified bipolar exception: its local fill still runs
/// from `fill_from`'s centre knob position to the current
/// value's knob centre, and `centre_tick` still marks
/// zero. All gesture logic lives in the drag handlers.
fn weather_slider(
    key: &'static str,
    frac: f32,
    fill_from: f32,
    centre_tick: bool,
    t: &Theme,
    handler: impl Fn(&mut State, &mut Drag, &mut Tx) + 'static,
) -> El {
    let frac = frac.clamp(0.0, 1.0);
    let travel = SLIDER_W - KNOB;
    let knob_cx = KNOB / 2.0 + frac * travel;
    let origin_cx = KNOB / 2.0 + fill_from * travel;
    let (fill_x, fill_w) = if knob_cx >= origin_cx {
        (origin_cx, knob_cx - origin_cx)
    } else {
        (knob_cx, origin_cx - knob_cx)
    };
    let root = el().key(key).size(SLIDER_W, SLIDER_H).on_drag(handler);
    if !centre_tick {
        // Unipolar (Amount): the kit owns the whole visual;
        // this root keeps only the key, size and drag.
        return root.child(
            design::slider_track(frac, t)
                .absolute(0.0, (SLIDER_H - KIT_SLIDER_TRACK_H) / 2.0),
        );
    }
    let mut slider = root
        .child(
            el()
                .size(SLIDER_W, 4.0)
                .absolute(0.0, (SLIDER_H - 4.0) / 2.0)
                .radius(2.0)
                .fill(t.control_track),
        )
        .child(
            el()
                .size(fill_w.max(4.0), 4.0)
                .absolute(fill_x, (SLIDER_H - 4.0) / 2.0)
                .radius(2.0)
                .fill(design::accent()),
        );
    if centre_tick {
        slider = slider.child(
            el()
                .size(2.0, 10.0)
                .absolute(SLIDER_W / 2.0 - 1.0, (SLIDER_H - 10.0) / 2.0)
                .radius(1.0)
                .fill(t.text_secondary),
        );
    }
    slider.child(
        el()
            .size(KNOB, KNOB)
            .absolute(frac * travel, (SLIDER_H - KNOB) / 2.0)
            .radius(KNOB / 2.0)
            .fill(t.knob),
    )
}

/// The Scene panel: weather (kind picker + cycle + density
/// and wind steppers with drag sliders under them), stage
/// size steppers with the preset row under them, a frame
/// rate stepper and finer stage-size steppers beside them
/// (Phase K), and background colour swatches with RGB
/// channel steppers under the Background header, with a
/// named-preset row between the header and the steppers
/// (Phase L). A read-only placed-media readout
/// ([`media_readout`] / [`media_note`]) closes the panel
/// (Wave 12, B5).
pub fn scene_panel(s: &State, t: &Theme) -> El {
    let core = &s.core;
    let weather = core.project.weather;
    let active = kind_index(weather);
    let (bg_r, bg_g, bg_b) = rgb_split(core.project.background);

    // Kind picker: one button per segment, the current kind
    // accent-filled (keys preserve the GPUI segmented ids
    // ("wx-seg", n) in string form). The row wraps (Phase O,
    // p05 O-6): one line measured 434.3 pt against the
    // 316 pt column budget, with wx-seg-5 past the window.
    let mut kind_row = row().key("wx-kind").gap(4.0).center_y().flex_wrap();
    for (i, label) in KIND_LABELS.iter().enumerate() {
        let kind = if i == 0 { None } else { Some(KINDS[i - 1]) };
        kind_row = kind_row.child(icon_btn(
            &format!("wx-seg-{}", i),
            kind_icon(kind),
            label,
            i == active,
            t,
            move |s: &mut State| set_weather_kind(&mut s.core, kind),
        ));
    }

    // Background swatches, eight per row (16 no longer fit
    // one row in the panel column): the current colour gets
    // an accent ring (a larger accent-filled slot behind
    // the swatch).
    let accent = tok::accent(0, true);
    let mut swatches = col().key("bg-swatches").gap(4.0);
    for chunk in BACKGROUND_SWATCHES.chunks(8) {
        let mut swatch_row = row().gap(4.0).center_y();
        for &(name, rgb) in chunk {
            let is_current = core.project.background == rgb;
            swatch_row = swatch_row.child(
                el()
                    .key(format!("bg-swatch-{}", name))
                    .size(26.0, 26.0)
                    .radius(8.0)
                    .fill(if is_current { accent } else { t.bg })
                    .center()
                    .on_press(move |s: &mut State| set_background(&mut s.core, rgb))
                    .child(
                        el()
                            .size(22.0, 22.0)
                            .radius(6.0)
                            .fill(hex_color(rgb).unwrap_or(t.bg)),
                    ),
            );
        }
        swatches = swatches.child(swatch_row);
    }

    // Background presets (Phase L; kit chips in Phase N):
    // six named colours under the Background header, three
    // per row so the named chips fit the panel. Positional
    // keys and the active highlight are the Phase L ones,
    // and a press writes through the swatches' own
    // `set_background`.
    let mut bg_presets = col().key("scene-bg-presets").gap(4.0);
    for chunk_start in (0..BG_PRESETS.len()).step_by(3) {
        let mut preset_row = row().gap(4.0).center_y();
        let chunk_end = (chunk_start + 3).min(BG_PRESETS.len());
        for i in chunk_start..chunk_end {
            let (name, rgb) = BG_PRESETS[i];
            preset_row = preset_row.child(chip_btn(
                &format!("scene-bg-preset-{}", i),
                None,
                name,
                bg_preset_active(core, i),
                t,
                move |s: &mut State| set_background(&mut s.core, rgb),
            ));
        }
        bg_presets = bg_presets.child(preset_row);
    }

    // Stage-size presets (Phase F): one button per preset,
    // the one matching the current stage accent-filled (the
    // kind picker's highlight). Three per row — six labels
    // do not fit a single panel row.
    let mut stage_presets = col().key("scene-stage-presets").gap(4.0);
    for chunk in STAGE_PRESETS.chunks(3) {
        let mut preset_row = row().gap(4.0).center_y();
        for &(label, w, h) in chunk {
            preset_row = preset_row.child(btn(
                &format!("scene-stage-{}x{}", w as i32, h as i32),
                label,
                stage_matches_preset(core.project.stage, w, h),
                t,
                move |s: &mut State| set_stage_preset(&mut s.core, w, h),
            ));
        }
        stage_presets = stage_presets.child(preset_row);
    }

    // Placed-media readout (Wave 12, B5): a read-only
    // section at the panel's foot — the counts line, and
    // the pointer note only when sprites are present (see
    // `media_note`). No controls here by design: this
    // section is the entry point, and editing lives in
    // the Sprites panel (Wave 13, A3).
    let mut media_section = col()
        .gap(4.0)
        .child(design::section_label("Placed media", t))
        .child(
            text(media_readout(core))
                .key("scene-media-readout")
                .font(type_::LABEL)
                .color(t.text),
        );
    if let Some(note) = media_note(core) {
        media_section = media_section.child(
            text(note)
                .key("scene-media-note")
                .font(type_::LABEL)
                .color(t.text_secondary),
        );
    }

    design::panel_col("scene-panel", t)
        .child(
            row()
                .w_full()
                .gap(8.0)
                .center_y()
                .child(design::panel_title(None, "Scene", None, t)),
        )
        .child(design::divider(t))
        .child(
            row()
                .gap(8.0)
                .center_y()
                .child(design::section_label("Weather", t))
                .child(
                    text(weather_summary(weather))
                        .font(type_::LABEL)
                        .color(t.text),
                )
                .child(btn("weather", "Cycle weather", false, t, |s| {
                    cycle_weather(&mut s.core)
                })),
        )
        .child(kind_row)
        .child(stepper_row(
            "Amount",
            match weather {
                Some(w) => format!("{:.0}", w.amount),
                None => "—".to_string(),
            },
            "wx-amt-dn",
            "wx-amt-up",
            t,
            |s: &mut State| step_weather_amount(&mut s.core, -20.0),
            |s: &mut State| step_weather_amount(&mut s.core, 20.0),
        ))
        .child(weather_slider(
            "wx-amt-slider",
            weather.map(|w| amount_frac(w.amount)).unwrap_or(0.0),
            0.0,
            false,
            t,
            amount_drag,
        ))
        .child(stepper_row(
            "Wind",
            match weather {
                Some(w) => format!("{}", w.wind),
                None => "—".to_string(),
            },
            "wx-wind-dn",
            "wx-wind-up",
            t,
            |s: &mut State| step_weather_wind(&mut s.core, -0.5),
            |s: &mut State| step_weather_wind(&mut s.core, 0.5),
        ))
        .child(weather_slider(
            "wx-wind-slider",
            weather.map(|w| wind_frac(w.wind)).unwrap_or(0.5),
            0.5,
            true,
            t,
            wind_drag,
        ))
        .child(
            row()
                .gap(8.0)
                .center_y()
                .child(design::section_label("Stage", t))
                .child(
                    text(format!(
                        "{} × {}",
                        core.project.stage[0] as i32, core.project.stage[1] as i32
                    ))
                    .font(type_::LABEL)
                    .color(t.text),
                ),
        )
        .child(fps_row(core, t))
        .child(stepper_row(
            "Width",
            format!("{}", core.project.stage[0] as i32),
            "stage-w-dn",
            "stage-w-up",
            t,
            |s: &mut State| adjust_stage(&mut s.core, -40.0, 0.0),
            |s: &mut State| adjust_stage(&mut s.core, 40.0, 0.0),
        ))
        .child(stepper_row(
            "Height",
            format!("{}", core.project.stage[1] as i32),
            "stage-h-dn",
            "stage-h-up",
            t,
            |s: &mut State| adjust_stage(&mut s.core, 0.0, -40.0),
            |s: &mut State| adjust_stage(&mut s.core, 0.0, 40.0),
        ))
        .child(
            col()
                .key("scene-stage-size-row")
                .gap(4.0)
                .child(stage_axis_row(0, "Width", core.project.stage[0], t))
                .child(stage_axis_row(1, "Height", core.project.stage[1], t)),
        )
        .child(design::section_label("Stage size", t))
        .child(stage_presets)
        .child(
            text(stage_readout(core))
                .key("scene-stage-readout")
                .font(type_::LABEL)
                .color(t.text_secondary),
        )
        .child(
            text("Resizing does not rescale figure positions.")
                .font(type_::LABEL)
                .color(t.text_secondary),
        )
        .child(
            row()
                .key("scene-bg-header")
                .gap(8.0)
                .center_y()
                .child(design::section_label("Background", t))
                .child(
                    el()
                        .key("scene-bg-preview")
                        .size(18.0, 18.0)
                        .radius(4.0)
                        .fill(hex_color(core.project.background).unwrap_or(t.bg)),
                )
                .child(
                    text(format!("#{:06X}", core.project.background))
                        .font(type_::LABEL)
                        .color(t.text),
                ),
        )
        .child(bg_presets)
        .child(bg_channel_row(0, "R", bg_r, t))
        .child(bg_channel_row(1, "G", bg_g, t))
        .child(bg_channel_row(2, "B", bg_b, t))
        .child(swatches)
        .child(media_section)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn core_with(kind: WeatherKind, amount: f32, wind: f32) -> EditorCore {
        let mut core = EditorCore::new();
        core.project.weather = Some(Weather { kind, amount, wind });
        core
    }

    #[test]
    fn kind_index_maps_off_and_every_kind() {
        assert_eq!(kind_index(None), 0);
        for (i, kind) in KINDS.iter().enumerate() {
            let w = Weather { kind: *kind, amount: 1.0, wind: 0.0 };
            assert_eq!(kind_index(Some(w)), i + 1, "kind {kind:?}");
            assert_eq!(KIND_LABELS[i + 1], kind_name(*kind));
        }
    }

    #[test]
    fn summary_matches_the_gpui_format() {
        assert_eq!(weather_summary(None), "Off — pick a kind");
        let w = Weather { kind: WeatherKind::Rain, amount: 90.0, wind: 25.0 };
        assert_eq!(weather_summary(Some(w)), "Rain · amount 90 · wind 25");
    }

    #[test]
    fn enabling_from_off_applies_the_kind_preset() {
        let mut core = EditorCore::new();
        set_weather_kind(&mut core, Some(WeatherKind::Storm));
        assert_eq!(core.project.weather, Some(kind_preset(WeatherKind::Storm)));
        assert_eq!(core.status, "Weather: storm.");
    }

    #[test]
    fn switching_kind_keeps_tuned_amount_and_wind() {
        let mut core = core_with(WeatherKind::Rain, 42.0, -7.0);
        set_weather_kind(&mut core, Some(WeatherKind::Fog));
        let w = core.project.weather.expect("weather stays on");
        assert_eq!(w.kind, WeatherKind::Fog);
        assert_eq!(w.amount, 42.0);
        assert_eq!(w.wind, -7.0);
        assert_eq!(core.status, "Weather: fog.");
    }

    #[test]
    fn picking_off_clears_weather() {
        let mut core = core_with(WeatherKind::Snow, 90.0, 12.0);
        set_weather_kind(&mut core, None);
        assert_eq!(core.project.weather, None);
        assert_eq!(core.status, "Weather: off.");
    }

    #[test]
    fn cycle_walks_the_gpui_sequence_and_wraps_to_off() {
        let mut core = EditorCore::new();
        cycle_weather(&mut core);
        assert_eq!(core.project.weather, Some(kind_preset(WeatherKind::Rain)));
        cycle_weather(&mut core);
        assert_eq!(core.project.weather, Some(kind_preset(WeatherKind::Snow)));
        cycle_weather(&mut core);
        assert_eq!(core.project.weather, Some(kind_preset(WeatherKind::Fireflies)));
        cycle_weather(&mut core);
        assert_eq!(core.project.weather, None);
        assert_eq!(core.status, "Weather: off.");
    }

    #[test]
    fn cycle_from_fog_or_storm_turns_off() {
        for kind in [WeatherKind::Fog, WeatherKind::Storm] {
            let mut core = core_with(kind, 10.0, 0.0);
            cycle_weather(&mut core);
            assert_eq!(core.project.weather, None, "kind {kind:?}");
        }
    }

    #[test]
    fn amount_step_clamps_to_the_weather_range() {
        let mut core = core_with(WeatherKind::Rain, 195.0, 0.0);
        step_weather_amount(&mut core, 20.0);
        assert_eq!(core.project.weather.map(|w| w.amount), Some(200.0));
        step_weather_amount(&mut core, -500.0);
        assert_eq!(core.project.weather.map(|w| w.amount), Some(0.0));
        assert_eq!(core.status, "Weather amount: 0.");
    }

    #[test]
    fn amount_and_wind_steps_are_status_only_when_off() {
        let mut core = EditorCore::new();
        core.project.weather = None;
        step_weather_amount(&mut core, 20.0);
        assert_eq!(core.status, "Weather is off — pick a kind first.");
        step_weather_wind(&mut core, 0.5);
        assert_eq!(core.status, "Weather is off — pick a kind first.");
        assert_eq!(core.project.weather, None);
    }

    #[test]
    fn wind_step_clamps_symmetrically() {
        let mut core = core_with(WeatherKind::Snow, 90.0, 199.8);
        step_weather_wind(&mut core, 0.5);
        assert_eq!(core.project.weather.map(|w| w.wind), Some(200.0));
        step_weather_wind(&mut core, -1000.0);
        assert_eq!(core.project.weather.map(|w| w.wind), Some(-200.0));
    }

    #[test]
    fn stage_steps_clamp_and_report() {
        let mut core = EditorCore::new();
        let (w0, h0) = (core.project.stage[0], core.project.stage[1]);
        adjust_stage(&mut core, 40.0, -40.0);
        assert_eq!(core.project.stage, [w0 + 40.0, h0 - 40.0]);
        assert_eq!(
            core.status,
            format!("Stage size: {} x {}.", (w0 + 40.0) as i32, (h0 - 40.0) as i32)
        );
        adjust_stage(&mut core, 100_000.0, -100_000.0);
        assert_eq!(core.project.stage, [4000.0, 200.0]);
    }

    #[test]
    fn stage_preset_writes_the_field_and_reports_like_the_steppers() {
        let mut core = EditorCore::new();
        set_stage_preset(&mut core, 1920.0, 1080.0);
        assert_eq!(core.project.stage, [1920.0, 1080.0]);
        assert_eq!(core.status, "Stage size: 1920 x 1080.");
        set_stage_preset(&mut core, 720.0, 1280.0);
        assert_eq!(core.project.stage, [720.0, 1280.0]);
        assert_eq!(core.status, "Stage size: 720 x 1280.");
    }

    #[test]
    fn stage_preset_matcher_is_exact_and_orientation_aware() {
        // A fresh project's stage is the first preset.
        let core = EditorCore::new();
        let (_, w, h) = STAGE_PRESETS[0];
        assert!(stage_matches_preset(core.project.stage, w, h));
        // Transposed dimensions are a different preset.
        assert!(!stage_matches_preset([720.0, 1280.0], 1280.0, 720.0));
        assert!(stage_matches_preset([720.0, 1280.0], 720.0, 1280.0));
        // One unit off (a stepper nudge) matches nothing.
        assert!(!stage_matches_preset([961.0, 720.0], 960.0, 720.0));
        assert!(!stage_matches_preset([960.0, 721.0], 960.0, 720.0));
    }

    #[test]
    fn stage_presets_are_six_unique_and_start_with_the_engine_default() {
        assert_eq!(STAGE_PRESETS.len(), 6);
        // The briefed table, in order.
        let sizes: Vec<(i32, i32)> = STAGE_PRESETS
            .iter()
            .map(|&(_, w, h)| (w as i32, h as i32))
            .collect();
        assert_eq!(
            sizes,
            vec![
                (960, 720),
                (1280, 720),
                (720, 1280),
                (1920, 1080),
                (1080, 1920),
                (640, 480),
            ]
        );
        // Sizes key the elements (scene-stage-{w}x{h}), so
        // they must be unique; so must the labels.
        let mut deduped = sizes.clone();
        deduped.sort_unstable();
        deduped.dedup();
        assert_eq!(deduped.len(), 6, "preset sizes must be unique");
        // "Default" really is the engine default.
        let (_, w, h) = STAGE_PRESETS[0];
        assert_eq!(STAGE_PRESETS[0].0, "Default");
        assert_eq!(EditorCore::new().project.stage, [w, h]);
        // Every preset sits inside the stepper clamp, so a
        // preset write never needs clamping of its own.
        for &(_, w, h) in &STAGE_PRESETS {
            assert!((200.0..=4000.0).contains(&w), "width {w}");
            assert!((200.0..=4000.0).contains(&h), "height {h}");
        }
    }

    #[test]
    fn background_swatch_sets_field_and_reports_hex() {
        let mut core = EditorCore::new();
        set_background(&mut core, 0x87B5D6);
        assert_eq!(core.project.background, 0x87B5D6);
        assert_eq!(core.status, "Background: #87B5D6.");
        // The default swatch is the engine's own default colour.
        assert_eq!(BACKGROUND_SWATCHES[0].1, EditorCore::new().project.background);
    }

    #[test]
    fn background_stepper_writes_the_stepped_colour_to_the_project() {
        // Start from the engine default 0x232830 (35, 40, 48):
        // one press steps exactly one channel by 8 and the
        // result lands on `project.background`, reported with
        // the swatch handler's own status string.
        let mut core = EditorCore::new();
        assert_eq!(core.project.background, 0x232830);
        step_background_channel(&mut core, 0, 1);
        assert_eq!(core.project.background, 0x2B2830);
        assert_eq!(core.status, "Background: #2B2830.");
        step_background_channel(&mut core, 1, -1);
        assert_eq!(core.project.background, 0x2B2030);
        step_background_channel(&mut core, 2, 1);
        assert_eq!(core.project.background, 0x2B2038);
        assert_eq!(core.status, "Background: #2B2038.");
    }

    #[test]
    fn background_stepper_clamps_at_black_and_white() {
        let mut core = EditorCore::new();
        core.project.background = 0x020202;
        for channel in 0..3 {
            step_background_channel(&mut core, channel, -1);
        }
        assert_eq!(core.project.background, 0x000000);
        // Further presses stay pinned at the clamp.
        step_background_channel(&mut core, 0, -1);
        assert_eq!(core.project.background, 0x000000);
        core.project.background = 0xFDFDFD;
        for channel in 0..3 {
            step_background_channel(&mut core, channel, 1);
        }
        assert_eq!(core.project.background, 0xFFFFFF);
        step_background_channel(&mut core, 2, 1);
        assert_eq!(core.project.background, 0xFFFFFF);
    }

    #[test]
    fn background_stepper_lands_on_and_leaves_swatch_colours() {
        // One press per channel from 0x080808 lands exactly
        // on the Black swatch — the panel's exact-equality
        // highlight would light it.
        let mut core = EditorCore::new();
        core.project.background = 0x080808;
        for channel in 0..3 {
            step_background_channel(&mut core, channel, -1);
        }
        assert_eq!(core.project.background, 0x000000);
        let hit = BACKGROUND_SWATCHES
            .iter()
            .find(|&&(_, rgb)| rgb == core.project.background);
        assert_eq!(hit.map(|&(name, _)| name), Some("Black"));
        // One press back up leaves every swatch: off-preset,
        // no highlight, the header's raw hex is the readout.
        step_background_channel(&mut core, 0, 1);
        assert_eq!(core.project.background, 0x080000);
        assert!(
            !BACKGROUND_SWATCHES
                .iter()
                .any(|&(_, rgb)| rgb == core.project.background),
            "0x080000 is not a swatch colour"
        );
    }

    #[test]
    fn background_stepper_ignores_a_bad_channel() {
        let mut core = EditorCore::new();
        let before = core.project.background;
        core.status = "untouched".into();
        step_background_channel(&mut core, 3, 1);
        assert_eq!(core.project.background, before);
        assert_eq!(core.status, "untouched");
    }

    #[test]
    fn amount_at_maps_edges_middle_and_clamps() {
        assert_eq!(amount_at(0.0, 200.0), 0);
        assert_eq!(amount_at(200.0, 200.0), 200);
        assert_eq!(amount_at(100.0, 200.0), 100);
        assert_eq!(amount_at(50.0, 200.0), 50);
        assert_eq!(amount_at(-30.0, 200.0), 0);
        assert_eq!(amount_at(260.0, 200.0), 200);
        // No layout yet: no divide-by-zero, reads as empty.
        assert_eq!(amount_at(10.0, 0.0), 0);
    }

    #[test]
    fn wind_at_maps_edges_centre_and_clamps() {
        assert_eq!(wind_at(0.0, 200.0), -200);
        assert_eq!(wind_at(200.0, 200.0), 200);
        assert_eq!(wind_at(100.0, 200.0), 0);
        assert_eq!(wind_at(-30.0, 200.0), -200);
        assert_eq!(wind_at(260.0, 200.0), 200);
        assert_eq!(wind_at(10.0, 0.0), 0);
    }

    #[test]
    fn wind_at_is_symmetric_about_the_centre() {
        for x in [0.0, 10.0, 25.0, 40.0, 50.0, 75.0, 100.0, 130.0, 175.0, 200.0] {
            assert_eq!(wind_at(x, 200.0), -wind_at(200.0 - x, 200.0), "x {x}");
        }
    }

    #[test]
    fn slider_fractions_hit_the_track_ends() {
        assert_eq!(amount_frac(0.0), 0.0);
        assert_eq!(amount_frac(100.0), 0.5);
        assert_eq!(amount_frac(200.0), 1.0);
        assert_eq!(wind_frac(-200.0), 0.0);
        assert_eq!(wind_frac(0.0), 0.5);
        assert_eq!(wind_frac(200.0), 1.0);
    }

    #[test]
    fn slider_set_writes_clamped_value_and_reports_only_on_release() {
        let mut core = core_with(WeatherKind::Rain, 10.0, 0.0);
        core.status = "untouched".into();
        set_weather_amount(&mut core, 130, false);
        assert_eq!(core.project.weather.map(|w| w.amount), Some(130.0));
        assert_eq!(core.status, "untouched", "mid-drag writes stay silent");
        set_weather_amount(&mut core, 999, true);
        assert_eq!(core.project.weather.map(|w| w.amount), Some(200.0));
        assert_eq!(core.status, "Weather amount: 200.");
        set_weather_wind(&mut core, -260, true);
        assert_eq!(core.project.weather.map(|w| w.wind), Some(-200.0));
        assert_eq!(core.status, "Weather wind: -200.");
    }

    #[test]
    fn slider_set_is_status_only_when_off() {
        let mut core = EditorCore::new();
        core.project.weather = None;
        set_weather_amount(&mut core, 100, true);
        assert_eq!(core.status, "Weather is off — pick a kind first.");
        set_weather_wind(&mut core, 100, true);
        assert_eq!(core.status, "Weather is off — pick a kind first.");
        assert_eq!(core.project.weather, None);
    }

    #[test]
    fn background_swatches_are_sixteen_and_unique() {
        assert_eq!(BACKGROUND_SWATCHES.len(), 16);
        let mut rgbs: Vec<u32> = BACKGROUND_SWATCHES.iter().map(|s| s.1).collect();
        rgbs.sort_unstable();
        rgbs.dedup();
        assert_eq!(rgbs.len(), 16, "swatch colours must be unique");
        // Names key the elements, so they must be unique too.
        let mut names: Vec<&str> = BACKGROUND_SWATCHES.iter().map(|s| s.0).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), 16, "swatch names must be unique");
    }

    fn core_with_stage(w: f32, h: f32) -> EditorCore {
        let mut core = EditorCore::new();
        core.project.stage = [w, h];
        core
    }

    #[test]
    fn stage_readout_names_every_preset_size() {
        // The briefed examples, one per aspect family.
        // Every fixture core carries the default rate, so
        // each line ends with its Phase K suffix.
        assert_eq!(
            stage_readout(&core_with_stage(960.0, 720.0)),
            "960×720 · 4:3 · 12 fps"
        );
        assert_eq!(
            stage_readout(&core_with_stage(1920.0, 1080.0)),
            "1920×1080 · 16:9 · 12 fps"
        );
        assert_eq!(
            stage_readout(&core_with_stage(1080.0, 1920.0)),
            "1080×1920 · 9:16 · 12 fps"
        );
        assert_eq!(
            stage_readout(&core_with_stage(720.0, 720.0)),
            "720×720 · 1:1 · 12 fps"
        );
        // Every preset in the table reduces to small integers,
        // so none of them may hit the decimal fallback.
        for &(_, w, h) in &STAGE_PRESETS {
            let line = stage_readout(&core_with_stage(w, h));
            assert!(
                !line.contains('.'),
                "preset {w}x{h} read out as {line:?}, expected a reduced ratio"
            );
            assert!(
                line.starts_with(&format!("{}×{} · ", w as i32, h as i32)),
                "readout {line:?} does not name the size"
            );
        }
    }

    #[test]
    fn stage_readout_follows_the_project_field() {
        // A fresh core reports the engine default, and a
        // preset press moves the readout with the field —
        // the line is a readout, not a label for the row.
        let mut core = EditorCore::new();
        assert_eq!(stage_readout(&core), "960×720 · 4:3 · 12 fps");
        set_stage_preset(&mut core, 640.0, 480.0);
        assert_eq!(stage_readout(&core), "640×480 · 4:3 · 12 fps");
        adjust_stage(&mut core, 40.0, 0.0);
        assert_eq!(stage_readout(&core), "680×480 · 17:12 · 12 fps");
    }

    #[test]
    fn stage_readout_rounds_rather_than_truncates() {
        // Fractions from a loaded file round to the nearest
        // whole unit: 959.6 is a 960 stage, not a 959 one.
        assert_eq!(
            stage_readout(&core_with_stage(959.6, 719.6)),
            "960×720 · 4:3 · 12 fps"
        );
        assert_eq!(
            stage_readout(&core_with_stage(1919.5, 1080.4)),
            "1920×1080 · 16:9 · 12 fps"
        );
    }

    #[test]
    fn stage_readout_falls_back_to_a_decimal_for_irreducible_sizes() {
        // 1000/333 is already in lowest terms and both sides
        // are far over 40, so the decimal form names it:
        // 1000/333 = 3.003… → 3.00:1.
        assert_eq!(
            stage_readout(&core_with_stage(1000.0, 333.0)),
            "1000×333 · 3.00:1 · 12 fps"
        );
        // A wide irreducible size: 3999/2000 is in lowest
        // terms too (3999 = 3 × 31 × 43), 3999/2000 = 1.9995.
        assert_eq!(
            stage_readout(&core_with_stage(3999.0, 2000.0)),
            "3999×2000 · 2.00:1 · 12 fps"
        );
    }

    fn core_with_fps(w: f32, h: f32, fps: f32) -> EditorCore {
        let mut core = core_with_stage(w, h);
        core.project.fps = fps;
        core
    }

    #[test]
    fn stage_readout_appends_the_frame_rate() {
        // The briefed Phase K case: size, aspect, rate.
        assert_eq!(
            stage_readout(&core_with_fps(960.0, 720.0, 12.0)),
            "960×720 · 4:3 · 12 fps"
        );
        // A stepped rate moves the suffix with the field.
        assert_eq!(
            stage_readout(&core_with_fps(960.0, 720.0, 30.0)),
            "960×720 · 4:3 · 30 fps"
        );
        // A fractional rate keeps one decimal (fps_label's
        // rule, the shell's convention).
        assert_eq!(
            stage_readout(&core_with_fps(960.0, 720.0, 23.5)),
            "960×720 · 4:3 · 23.5 fps"
        );
    }

    #[test]
    fn fps_label_writes_whole_rates_as_integers() {
        assert_eq!(fps_label(12.0), "12");
        assert_eq!(fps_label(60.0), "60");
        assert_eq!(fps_label(23.5), "23.5");
        assert_eq!(fps_label(29.97), "30.0");
    }

    #[test]
    fn fps_after_steps_by_one_and_clamps_to_the_op_range() {
        let core = EditorCore::new();
        assert_eq!(core.project.fps, 12.0, "the engine default rate");
        assert_eq!(fps_after(&core, 1), 13.0);
        assert_eq!(fps_after(&core, -1), 11.0);
        // At the clamps a press stays in [1, 60] — the
        // range the op enforces again on write.
        assert_eq!(fps_after(&core_with_fps(960.0, 720.0, 60.0), 1), 60.0);
        assert_eq!(fps_after(&core_with_fps(960.0, 720.0, 1.0), -1), 1.0);
        assert_eq!(fps_after(&core_with_fps(960.0, 720.0, 59.5), 1), 60.0);
    }

    #[test]
    fn stage_axis_after_steps_one_axis_by_twenty() {
        let core = core_with_stage(960.0, 720.0);
        // Width presses move only the width…
        assert_eq!(stage_axis_after(&core, 0, 1), (980.0, 720.0));
        assert_eq!(stage_axis_after(&core, 0, -1), (940.0, 720.0));
        // …and height presses only the height.
        assert_eq!(stage_axis_after(&core, 1, 1), (960.0, 740.0));
        assert_eq!(stage_axis_after(&core, 1, -1), (960.0, 700.0));
        // A bad axis changes nothing.
        assert_eq!(stage_axis_after(&core, 2, 1), (960.0, 720.0));
    }

    #[test]
    fn stage_axis_after_clamps_to_the_op_range() {
        // [64, 4096] per axis — the range the op enforces
        // again on write.
        assert_eq!(
            stage_axis_after(&core_with_stage(4090.0, 720.0), 0, 1),
            (4096.0, 720.0)
        );
        assert_eq!(
            stage_axis_after(&core_with_stage(70.0, 720.0), 0, -1),
            (64.0, 720.0)
        );
        assert_eq!(
            stage_axis_after(&core_with_stage(960.0, 4096.0), 1, 1),
            (960.0, 4096.0)
        );
        assert_eq!(
            stage_axis_after(&core_with_stage(960.0, 64.0), 1, -1),
            (960.0, 64.0)
        );
    }

    #[test]
    fn steppers_write_through_the_editor_ops() {
        // The press handlers hand the stepped value to
        // §12's ops; the project fields carry the result.
        let mut core = EditorCore::new();
        step_fps(&mut core, 1);
        assert_eq!(core.project.fps, 13.0);
        step_fps(&mut core, -1);
        step_fps(&mut core, -1);
        assert_eq!(core.project.fps, 11.0);
        step_stage_axis(&mut core, 0, 1);
        assert_eq!(core.project.stage, [980.0, 720.0]);
        step_stage_axis(&mut core, 1, -1);
        assert_eq!(core.project.stage, [980.0, 700.0]);
        // A bad axis never reaches the op.
        step_stage_axis(&mut core, 2, 1);
        assert_eq!(core.project.stage, [980.0, 700.0]);
    }

    #[test]
    fn bg_presets_are_six_named_colours_and_midnight_is_the_default() {
        assert_eq!(BG_PRESETS.len(), 6);
        // Midnight leads and IS the engine default, so a
        // fresh project lights the first preset.
        assert_eq!(BG_PRESETS[0].0, "Midnight");
        assert_eq!(BG_PRESETS[0].1, EditorCore::new().project.background);
        assert_eq!(BG_PRESETS[0].1, 0x232830);
        let mut rgbs: Vec<u32> = BG_PRESETS.iter().map(|p| p.1).collect();
        rgbs.sort_unstable();
        rgbs.dedup();
        assert_eq!(rgbs.len(), 6, "preset colours must be unique");
        let mut names: Vec<&str> = BG_PRESETS.iter().map(|p| p.0).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), 6, "preset names must be unique");
    }

    #[test]
    fn bg_preset_press_sets_the_background_through_the_panel_op() {
        // The chip handler body is this module's
        // `set_background` — the swatches' own op — so a
        // press lands the colour on the field, reports the
        // hex in the status line, and (panel discipline)
        // takes no undo snapshot.
        let mut core = EditorCore::new();
        for &(name, rgb) in &BG_PRESETS {
            set_background(&mut core, rgb);
            assert_eq!(core.project.background, rgb, "preset {name}");
            assert_eq!(core.status, format!("Background: #{:06X}.", rgb));
            assert_eq!(core.undo_depth(), 0, "preset {name} must not snapshot");
        }
    }

    #[test]
    fn bg_preset_active_follows_the_current_background() {
        let mut core = EditorCore::new();
        // Fresh project: only Midnight (index 0) is lit.
        assert!(bg_preset_active(&core, 0));
        for i in 1..BG_PRESETS.len() {
            assert!(!bg_preset_active(&core, i), "preset {i} lit on a fresh project");
        }
        // Jump to the last preset through the op: only it
        // lights.
        let last = BG_PRESETS.len() - 1;
        set_background(&mut core, BG_PRESETS[last].1);
        assert!(bg_preset_active(&core, last));
        assert!(!bg_preset_active(&core, 0));
        // A colour reached by the RGB steppers lights its
        // preset too (exact-equality on the field): step
        // from Plum 0x2A2138 down one R step lands off
        // every preset, and stepping back restores it.
        step_background_channel(&mut core, 0, -1);
        for i in 0..BG_PRESETS.len() {
            assert!(!bg_preset_active(&core, i), "preset {i} lit off-preset");
        }
        step_background_channel(&mut core, 0, 1);
        assert!(bg_preset_active(&core, last));
        // Out-of-range indices are never active.
        assert!(!bg_preset_active(&core, BG_PRESETS.len()));
        assert!(!bg_preset_active(&core, 99));
    }

    fn core_with_media(assets: usize, sprites: usize) -> EditorCore {
        let mut core = EditorCore::new();
        for i in 0..assets {
            core.project.assets.push(stykanimate_rs::model::ImageAsset {
                name: format!("a{i}.png"),
                png: Vec::new(),
            });
        }
        for _ in 0..sprites {
            core.project.sprites.push(stykanimate_rs::model::SpriteInstance {
                asset: 0,
                keys: Vec::new(),
            });
        }
        core
    }

    #[test]
    fn media_readout_names_the_empty_project_honestly() {
        // A fresh project carries no assets and no sprites
        // (model.rs's defaults), and the readout says so
        // instead of the section vanishing.
        let core = EditorCore::new();
        assert_eq!(
            media_readout(&core),
            "No image assets or sprites in this project."
        );
        assert_eq!(media_note(&core), None);
    }

    #[test]
    fn media_readout_counts_sprites_and_assets_with_plurals() {
        assert_eq!(
            media_readout(&core_with_media(1, 1)),
            "1 sprite placed · 1 image asset."
        );
        assert_eq!(
            media_readout(&core_with_media(3, 2)),
            "2 sprites placed · 3 image assets."
        );
        // Assets without sprites are real state too (node
        // media imports land in the table): counted, and no
        // pointer note is raised for them.
        let core = core_with_media(2, 0);
        assert_eq!(media_readout(&core), "0 sprites placed · 2 image assets.");
        assert_eq!(media_note(&core), None);
    }

    #[test]
    fn media_note_points_to_the_sprites_panel_only_when_sprites_are_present() {
        // The note is the readout's entry-point pointer:
        // with a sprite placed it must be present and name
        // the Sprites panel (Wave 13, A3) as the place
        // sprites are edited — the Wave 12 claim that they
        // render in exports only and cannot be moved or
        // edited is gone. (The no-sprite cases asserting
        // `None` live in the two readout tests above.)
        let core = core_with_media(1, 1);
        assert_eq!(
            media_note(&core),
            Some(
                "Sprites are edited in the Sprites panel — move, rotate, scale and keyframe them there."
            )
        );
    }
}
