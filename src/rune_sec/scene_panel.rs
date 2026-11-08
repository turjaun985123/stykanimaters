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
//! No undo snapshots, on purpose: weather writes were never
//! snapshotted. The GPUI listeners wrote `project.weather`
//! directly (the wiring note above), and in this engine a
//! snapshot exists only where a backing fn calls
//! `EditorCore::snapshot()` — none of this module's fns do,
//! steppers included. A drag therefore costs exactly as
//! many undo entries as the steppers: zero.

use rune::tok::{self, type_, Theme};
use rune::{col, el, row, text, Drag, El, Phase, Tx};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::vfx::{Weather, WeatherKind};

use crate::chrome_view::btn;
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
    core.project.weather = next;
    core.status = label.into();
}

/// Density stepper backing: adjust the particle amount by
/// `delta`, clamped to 0..=200 (the `Weather` doc's range).
/// With weather off this is status-only, as in the GPUI panel.
pub fn step_weather_amount(core: &mut EditorCore, delta: f32) {
    match core.project.weather {
        Some(mut w) => {
            w.amount = (w.amount + delta).clamp(0.0, 200.0);
            core.project.weather = Some(w);
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
            w.wind = (w.wind + delta).clamp(-200.0, 200.0);
            core.project.weather = Some(w);
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
            }
        }
    }
}

/// Stage resize backing (the GPUI view's `adjust_stage`):
/// each axis moves by its delta and clamps to 200..=4000,
/// reporting the new size in the status line.
pub fn adjust_stage(core: &mut EditorCore, dw: f32, dh: f32) {
    core.project.stage[0] = (core.project.stage[0] + dw).clamp(200.0, 4000.0);
    core.project.stage[1] = (core.project.stage[1] + dh).clamp(200.0, 4000.0);
    core.status = format!(
        "Stage size: {} x {}.",
        core.project.stage[0] as i32, core.project.stage[1] as i32
    );
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
    core.project.background = rgb & 0xFF_FFFF;
    core.status = format!("Background: #{:06X}.", core.project.background);
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
        .child(text(label).font(type_::LABEL).color(t.text_secondary))
        .child(btn(dn_key, "−", false, t, dn))
        .child(text(value).font(type_::LABEL).color(t.text))
        .child(btn(up_key, "+", false, t, up))
}

/// Slider track width/height and knob diameter (the
/// settings example's proportions, slightly wider).
const SLIDER_W: f32 = 200.0;
const SLIDER_H: f32 = 22.0;
const KNOB: f32 = 16.0;

/// One weather drag slider (the settings example's
/// `speed_control` shape): a fixed-size track element with
/// track, fill and knob children drawn absolutely inside
/// it. The fill runs from `fill_from`'s knob centre (0.0
/// for Amount, 0.5 for Wind) to the current value's knob
/// centre; `centre_tick` marks zero on the bipolar wind
/// track. All gesture logic lives in the drag handlers.
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
    let mut slider = el()
        .key(key)
        .size(SLIDER_W, SLIDER_H)
        .on_drag(handler)
        .child(
            el()
                .size(SLIDER_W, 4.0)
                .absolute(0.0, (SLIDER_H - 4.0) / 2.0)
                .radius(2.0)
                .fill(t.control_off),
        )
        .child(
            el()
                .size(fill_w.max(4.0), 4.0)
                .absolute(fill_x, (SLIDER_H - 4.0) / 2.0)
                .radius(2.0)
                .fill(tok::accent(0, true)),
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
            .fill(t.text),
    )
}

/// The Scene panel: weather (kind picker + cycle + density
/// and wind steppers with drag sliders under them), stage
/// size steppers, and background colour swatches.
pub fn scene_panel(s: &State, t: &Theme) -> El {
    let core = &s.core;
    let weather = core.project.weather;
    let active = kind_index(weather);

    // Kind picker: one button per segment, the current kind
    // accent-filled (keys preserve the GPUI segmented ids
    // ("wx-seg", n) in string form).
    let mut kind_row = row().key("wx-kind").gap(4.0).center_y();
    for (i, label) in KIND_LABELS.iter().enumerate() {
        let kind = if i == 0 { None } else { Some(KINDS[i - 1]) };
        kind_row = kind_row.child(btn(
            &format!("wx-seg-{}", i),
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

    col()
        .key("scene-panel")
        .gap(8.0)
        .padding(12.0)
        .fill(t.card)
        .radius(8.0)
        .child(text("Scene").font(type_::TITLE).color(t.text))
        .child(
            row()
                .gap(8.0)
                .center_y()
                .child(text("Weather").font(type_::LABEL).color(t.text_secondary))
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
                .child(text("Stage").font(type_::LABEL).color(t.text_secondary))
                .child(
                    text(format!(
                        "{} × {}",
                        core.project.stage[0] as i32, core.project.stage[1] as i32
                    ))
                    .font(type_::LABEL)
                    .color(t.text),
                ),
        )
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
            row()
                .gap(8.0)
                .center_y()
                .child(text("Background").font(type_::LABEL).color(t.text_secondary))
                .child(
                    text(format!("#{:06X}", core.project.background))
                        .font(type_::LABEL)
                        .color(t.text),
                ),
        )
        .child(swatches)
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
    fn background_swatch_sets_field_and_reports_hex() {
        let mut core = EditorCore::new();
        set_background(&mut core, 0x87B5D6);
        assert_eq!(core.project.background, 0x87B5D6);
        assert_eq!(core.status, "Background: #87B5D6.");
        // The default swatch is the engine's own default colour.
        assert_eq!(BACKGROUND_SWATCHES[0].1, EditorCore::new().project.background);
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
}
