//! Pure helpers for per-segment rendering styles (see
//! `model::NodeStyle`): cap cycling for the editor, taper labels,
//! and the colour interpolation / blending the rasterizer and the
//! SVG exporter share. Everything here is deterministic.

use crate::editor::EditorCore;
use crate::model::{CapKind, NodeStyle};
use serde_json::{json, Value};

/// Cycle a segment's end cap: Round → Square → Point → Round.
pub fn cycle_cap(c: CapKind) -> CapKind {
    match c {
        CapKind::Round => CapKind::Square,
        CapKind::Square => CapKind::Point,
        CapKind::Point => CapKind::Round,
    }
}

/// Short human label for a taper value (1.0 = uniform width),
/// e.g. `Taper 100%` or `Taper 25%`.
pub fn taper_label(t: f32) -> String {
    format!("Taper {:.0}%", t * 100.0)
}

/// Interpolate two `0xRRGGBB` colours; `t` is clamped to 0..=1
/// and channels are rounded to the nearest step.
pub fn lerp_color(a: u32, b: u32, t: f32) -> u32 {
    let t = t.clamp(0.0, 1.0);
    let ch = |shift: u32| -> u32 {
        let x = ((a >> shift) & 0xFF) as f32;
        let y = ((b >> shift) & 0xFF) as f32;
        (x + (y - x) * t).round() as u32
    };
    (ch(16) << 16) | (ch(8) << 8) | ch(0)
}

/// Blend `src` over the current pixel `dst` with the given
/// opacity (clamped to 0..=1). Opacity 1 returns `src` exactly,
/// which keeps default-styled rendering pixel-identical.
pub fn blend_over(dst: [u8; 3], src: [u8; 3], opacity: f32) -> [u8; 3] {
    let o = opacity.clamp(0.0, 1.0);
    [
        (dst[0] as f32 * (1.0 - o) + src[0] as f32 * o) as u8,
        (dst[1] as f32 * (1.0 - o) + src[1] as f32 * o) as u8,
        (dst[2] as f32 * (1.0 - o) + src[2] as f32 * o) as u8,
    ]
}

// ---------------------------------------------------------------------
// Wave 11 (B10): MCP style parity — presets, the taper write the
// engine never had, a whole-style read, and the atomic whole-style
// write (the panel's paste path), as `tool_*` handlers for the MCP
// registry in `mcp_tools.rs`. The panel side lives in
// `rune_sec/inspector_style.rs`; every behaviour below mirrors a
// named panel op, cited inline.
// ---------------------------------------------------------------------

/// One curated style preset: a name, a colour and a cap.
/// Applying a preset changes exactly those two fields of a
/// node's style (its gradient end and opacity pass through).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StylePreset {
    pub name: &'static str,
    pub color: u32,
    pub cap: CapKind,
}

/// The six style presets, in the style panel's display order.
///
/// **Copied verbatim** from `rune_sec/inspector_style.rs`
/// (`STYLE_PRESETS`, lines 218–225 at the time of writing): the
/// panel's table lives in the bin crate, which the library
/// cannot import, so the MCP side carries its own copy. The two
/// tables must be edited together — the unit test
/// `style_presets_match_the_panel_table` pins these triples so
/// at least this copy cannot drift silently. Integrator note:
/// the durable fix is for the panel to consume this table.
pub const STYLE_PRESETS: [StylePreset; 6] = [
    StylePreset { name: "Ink", color: 0x20_242C, cap: CapKind::Round },
    StylePreset { name: "Crimson", color: 0xD2_3B3B, cap: CapKind::Round },
    StylePreset { name: "Ocean", color: 0x2B_6FD2, cap: CapKind::Square },
    StylePreset { name: "Forest", color: 0x2E_9E5B, cap: CapKind::Round },
    StylePreset { name: "Violet", color: 0x7A_4FD0, cap: CapKind::Square },
    StylePreset { name: "Amber", color: 0xE8_A33D, cap: CapKind::Square },
];

/// Look a preset up by name (case-insensitive, surrounding
/// whitespace ignored). `None` for an unknown name.
pub fn style_preset_by_name(name: &str) -> Option<&'static StylePreset> {
    let want = name.trim();
    STYLE_PRESETS
        .iter()
        .find(|p| p.name.eq_ignore_ascii_case(want))
}

/// The preset names, for error messages: "Ink, Crimson, …".
fn style_preset_names() -> String {
    STYLE_PRESETS
        .iter()
        .map(|p| p.name)
        .collect::<Vec<_>>()
        .join(", ")
}

/// Set the selected segment's taper directly — the engine
/// change the style panel's docs defer to ("the engine has NO
/// `set_node_taper`, and adding one is an engine change, not a
/// panel change", `inspector_style.rs` module docs): the panel
/// can only cycle taper through `EditorCore::cycle_node_taper`,
/// so before this op no direct write existed anywhere.
///
/// Discipline mirrors `EditorCore::set_node_style_atomic`:
/// refuse without a selection (status-only, no snapshot),
/// otherwise exactly one `snapshot()`, the write, and the
/// cycle op's own status wording (`Segment Taper NN%.` via
/// [`taper_label`]). `taper` must already be validated to
/// 0.0..=1.0 by the caller (the handler does); the op stores
/// it verbatim — exporters interpolate any value in that
/// range (`export.rs` / `svg.rs` use `style.taper.max(0.0)`).
/// Returns whether the taper was written.
pub fn set_node_taper(core: &mut EditorCore, taper: f32) -> bool {
    let Some(ni) = core.selected_node else {
        core.status = "Select a joint first.".into();
        return false;
    };
    let valid = core
        .project
        .figures
        .get(core.selected_figure)
        .and_then(|f| f.def.nodes.get(ni))
        .is_some();
    if !valid {
        core.status = "Select a joint first.".into();
        return false;
    }
    core.snapshot();
    let label = {
        let node = &mut core.project.figures[core.selected_figure].def.nodes[ni];
        node.style.taper = taper;
        taper_label(node.style.taper)
    };
    core.status = format!("Segment {label}.");
    true
}

// ---------------------------------------------------------------------
// Local argument helpers for the handlers in this file (the
// registry's own helpers are private to `mcp_tools.rs`). Every
// one returns Err(String) on bad input, in the registry's
// wording, so no handler can panic on malformed arguments.
// ---------------------------------------------------------------------

fn sty_get<'a>(args: &'a Value, key: &str) -> Option<&'a Value> {
    args.get(key).filter(|v| !v.is_null())
}

fn sty_as_usize(v: &Value, key: &str) -> Result<usize, String> {
    if let Some(u) = v.as_u64() {
        return usize::try_from(u).map_err(|_| format!("argument '{key}' is too large"));
    }
    if let Some(f) = v.as_f64() {
        if f >= 0.0 && f.fract() == 0.0 && f <= usize::MAX as f64 {
            return Ok(f as usize);
        }
    }
    Err(format!("argument '{key}' must be a non-negative integer"))
}

fn sty_req_usize(args: &Value, key: &str) -> Result<usize, String> {
    match sty_get(args, key) {
        Some(v) => sty_as_usize(v, key),
        None => Err(format!("missing required argument '{key}'")),
    }
}

fn sty_req_f32(args: &Value, key: &str) -> Result<f32, String> {
    match sty_get(args, key) {
        Some(v) => v
            .as_f64()
            .map(|f| f as f32)
            .ok_or_else(|| format!("argument '{key}' must be a number")),
        None => Err(format!("missing required argument '{key}'")),
    }
}

fn sty_req_str<'a>(args: &'a Value, key: &str) -> Result<&'a str, String> {
    match sty_get(args, key) {
        Some(v) => v
            .as_str()
            .ok_or_else(|| format!("argument '{key}' must be a string")),
        None => Err(format!("missing required argument '{key}'")),
    }
}

/// Colours travel as `0xRRGGBB` integers (registry convention).
fn sty_opt_color(args: &Value, key: &str) -> Result<Option<u32>, String> {
    match sty_get(args, key) {
        Some(v) => {
            let n = sty_as_usize(v, key)?;
            u32::try_from(n)
                .map(Some)
                .map_err(|_| format!("argument '{key}' is too large for a colour"))
        }
        None => Ok(None),
    }
}

fn sty_req_color(args: &Value, key: &str) -> Result<u32, String> {
    sty_opt_color(args, key)?
        .ok_or_else(|| format!("missing required argument '{key}'"))
}

fn sty_check_node(core: &EditorCore, fi: usize, ni: usize) -> Result<(), String> {
    let figures = core.project.figures.len();
    if fi >= figures {
        return Err(format!(
            "figure index {fi} out of range (project has {figures} figure(s))"
        ));
    }
    let n = core.project.figures[fi].def.nodes.len();
    if ni >= n {
        return Err(format!(
            "node index {ni} out of range (figure {fi} has {n} node(s))"
        ));
    }
    Ok(())
}

/// Select (figure, node) so the selected-node style ops act on
/// it — the same select-then-delegate pattern the registry's
/// own style setters use (`mcp_tools.rs` `select_node`).
fn sty_select_node(core: &mut EditorCore, fi: usize, ni: usize) {
    core.selected_figure = fi;
    core.selected_node = Some(ni);
}

/// The wire name of a cap kind: "round" / "square" / "point"
/// (the registry's `set_node_cap` response convention).
fn sty_cap_name(cap: CapKind) -> String {
    format!("{cap:?}").to_lowercase()
}

/// Parse a wire cap name; the error names the real enum, so a
/// value the model cannot hold is refused, never mapped.
fn sty_parse_cap(text: &str) -> Result<CapKind, String> {
    match text {
        "round" => Ok(CapKind::Round),
        "square" => Ok(CapKind::Square),
        "point" => Ok(CapKind::Point),
        other => Err(format!(
            "unknown cap '{other}' (expected \"round\", \"square\" or \"point\")"
        )),
    }
}

/// The addressed node's style plus its colour context, read
/// back for handler responses.
fn sty_read(core: &EditorCore, fi: usize, ni: usize) -> (NodeStyle, Option<u32>, u32) {
    let fig = &core.project.figures[fi];
    let node = &fig.def.nodes[ni];
    (node.style, node.color, fig.color)
}

/// MCP handler: read one segment's complete style.
///
/// The registry's style setters echo single fields in their
/// own responses, but before this tool no MCP call could read
/// a style — and taper could not be read at all (the word does
/// not appear in `mcp_tools.rs`). `color` is the node's own
/// override (`null` = inherits the figure colour);
/// `effective_color` is what actually renders, the panel's
/// `selected_color` rule (node colour, else figure colour).
pub fn tool_get_node_style(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let fi = sty_req_usize(args, "figure")?;
    let ni = sty_req_usize(args, "node")?;
    sty_check_node(core, fi, ni)?;
    let (style, color, figure_color) = sty_read(core, fi, ni);
    Ok(json!({
        "ok": true,
        "figure": fi,
        "node": ni,
        "color": color,
        "effective_color": color.unwrap_or(figure_color),
        "color2": style.color2,
        "cap": sty_cap_name(style.cap),
        "taper": style.taper,
        "opacity": style.opacity,
    }))
}

/// MCP handler: cycle the addressed segment's taper one step,
/// exactly the style panel's Step button
/// (`EditorCore::cycle_node_taper`: 1.0 → 0.6 → 0.3 → 0.0 →
/// 1.0, one undo step). The response reads the stored taper
/// back.
pub fn tool_cycle_node_taper(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let fi = sty_req_usize(args, "figure")?;
    let ni = sty_req_usize(args, "node")?;
    sty_check_node(core, fi, ni)?;
    sty_select_node(core, fi, ni);
    core.cycle_node_taper();
    let (style, _, _) = sty_read(core, fi, ni);
    Ok(json!({
        "ok": true,
        "figure": fi,
        "node": ni,
        "taper": style.taper,
        "status": core.status,
    }))
}

/// MCP handler: set the addressed segment's taper directly to
/// any value in 0.0..=1.0, through [`set_node_taper`] (one
/// undo step). Values outside the model's range are refused,
/// not clamped, so the caller learns the truth.
pub fn tool_set_node_taper(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let fi = sty_req_usize(args, "figure")?;
    let ni = sty_req_usize(args, "node")?;
    sty_check_node(core, fi, ni)?;
    let taper = sty_req_f32(args, "taper")?;
    if !taper.is_finite() || !(0.0..=1.0).contains(&taper) {
        return Err("argument 'taper' must be a number between 0.0 and 1.0".into());
    }
    sty_select_node(core, fi, ni);
    if !set_node_taper(core, taper) {
        return Err(core.status.clone());
    }
    let (style, _, _) = sty_read(core, fi, ni);
    Ok(json!({
        "ok": true,
        "figure": fi,
        "node": ni,
        "taper": style.taper,
        "status": core.status,
    }))
}

/// MCP handler: write a complete style to one segment in
/// exactly one undo step — the write half of the style
/// panel's copy/paste (`EditorCore::set_node_style_atomic`,
/// the op the paste path uses). Together with
/// `get_node_style` this makes copy/paste round-trippable
/// over MCP; the panel's own clipboard is process-local
/// shell state and is deliberately not shared. Taper is not
/// part of the atomic four (the engine op has no taper
/// parameter, exactly as in the panel's clipboard) — the
/// response reports the taper it left untouched; use
/// `set_node_taper` for it. An absent or null `color2`
/// clears the gradient, the registry's `set_node_color2`
/// convention. Opacity follows the atomic op's 0.05..=1.0
/// clamp and is read back as stored.
pub fn tool_set_node_style(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let fi = sty_req_usize(args, "figure")?;
    let ni = sty_req_usize(args, "node")?;
    sty_check_node(core, fi, ni)?;
    let color = sty_req_color(args, "color")?;
    let color2 = sty_opt_color(args, "color2")?;
    let cap = sty_parse_cap(sty_req_str(args, "cap")?)?;
    let opacity = sty_req_f32(args, "opacity")?;
    if opacity < 0.0 {
        return Err("argument 'opacity' must not be negative".into());
    }
    if !opacity.is_finite() {
        return Err("argument 'opacity' must be a number".into());
    }
    sty_select_node(core, fi, ni);
    core.set_node_style_atomic(color, color2, cap, opacity);
    let (style, stored_color, _) = sty_read(core, fi, ni);
    Ok(json!({
        "ok": true,
        "figure": fi,
        "node": ni,
        "color": stored_color,
        "color2": style.color2,
        "cap": sty_cap_name(style.cap),
        "opacity": style.opacity,
        "taper": style.taper,
        "status": core.status,
    }))
}

/// MCP handler: apply one of the six curated style presets to
/// the addressed segment — the panel's preset chips
/// (`inspector_style.rs` `apply_preset`): the preset's colour
/// and cap are written through the atomic style op while the
/// node's current gradient end and opacity pass through
/// untouched, in one undo step, and the status names the
/// preset. (The panel also records the colour in its
/// shell-local recent-colours list; that list is UI state,
/// not project state, and has no MCP counterpart.) Unknown
/// preset names are refused with the real list.
pub fn tool_apply_style_preset(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let fi = sty_req_usize(args, "figure")?;
    let ni = sty_req_usize(args, "node")?;
    sty_check_node(core, fi, ni)?;
    let name = sty_req_str(args, "preset")?;
    let preset = style_preset_by_name(name).ok_or_else(|| {
        format!(
            "unknown style preset '{name}' (expected one of: {})",
            style_preset_names()
        )
    })?;
    let (current, _, _) = sty_read(core, fi, ni);
    sty_select_node(core, fi, ni);
    core.set_node_style_atomic(preset.color, current.color2, preset.cap, current.opacity);
    core.status = format!("Style preset: {}.", preset.name);
    let (style, stored_color, _) = sty_read(core, fi, ni);
    Ok(json!({
        "ok": true,
        "figure": fi,
        "node": ni,
        "preset": preset.name,
        "color": stored_color,
        "color2": style.color2,
        "cap": sty_cap_name(style.cap),
        "opacity": style.opacity,
        "status": core.status,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cap_cycles_through_all_three_and_back() {
        assert_eq!(cycle_cap(CapKind::Round), CapKind::Square);
        assert_eq!(cycle_cap(CapKind::Square), CapKind::Point);
        assert_eq!(cycle_cap(CapKind::Point), CapKind::Round);
    }

    #[test]
    fn taper_labels_show_percent() {
        assert_eq!(taper_label(1.0), "Taper 100%");
        assert_eq!(taper_label(0.25), "Taper 25%");
        assert_eq!(taper_label(0.0), "Taper 0%");
    }

    #[test]
    fn lerp_color_hits_endpoints_and_midpoint() {
        assert_eq!(lerp_color(0xFF0000, 0x0000FF, 0.0), 0xFF0000);
        assert_eq!(lerp_color(0xFF0000, 0x0000FF, 1.0), 0x0000FF);
        // 255 * 0.5 = 127.5 rounds to 128 on both fading channels.
        assert_eq!(lerp_color(0xFF0000, 0x0000FF, 0.5), 0x800080);
        assert_eq!(lerp_color(0x000000, 0xFFFFFF, 0.5), 0x808080);
    }

    #[test]
    fn lerp_color_clamps_out_of_range_t() {
        assert_eq!(lerp_color(0x102030, 0x405060, -1.0), 0x102030);
        assert_eq!(lerp_color(0x102030, 0x405060, 2.0), 0x405060);
    }

    #[test]
    fn blend_over_respects_opacity() {
        assert_eq!(blend_over([0, 0, 0], [255, 0, 0], 1.0), [255, 0, 0]);
        assert_eq!(blend_over([0, 0, 0], [255, 0, 0], 0.0), [0, 0, 0]);
        // 255 * 0.5 = 127.5 truncates to 127 in the u8 cast.
        assert_eq!(blend_over([0, 0, 0], [255, 0, 0], 0.5), [127, 0, 0]);
        assert_eq!(blend_over([200, 100, 0], [0, 100, 200], 0.5), [100, 100, 100]);
    }

    // ---- Wave 11 (B10): MCP style parity ----

    use crate::editor::EditorCore;

    fn demo_core() -> EditorCore {
        // The demo project: figure 0 is the humanoid with
        // 11 nodes; node 1 is a plain default-styled segment.
        EditorCore::new()
    }

    #[test]
    fn style_presets_match_the_panel_table() {
        // Pins this copy of the table against the values the
        // panel ships (inspector_style.rs STYLE_PRESETS). If
        // the panel table changes, this test is the reminder
        // that the MCP copy must change with it.
        let triples: Vec<(&str, u32, CapKind)> =
            STYLE_PRESETS.iter().map(|p| (p.name, p.color, p.cap)).collect();
        assert_eq!(
            triples,
            vec![
                ("Ink", 0x20242C, CapKind::Round),
                ("Crimson", 0xD23B3B, CapKind::Round),
                ("Ocean", 0x2B6FD2, CapKind::Square),
                ("Forest", 0x2E9E5B, CapKind::Round),
                ("Violet", 0x7A4FD0, CapKind::Square),
                ("Amber", 0xE8A33D, CapKind::Square),
            ]
        );
        assert_eq!(style_preset_by_name("ocean").map(|p| p.name), Some("Ocean"));
        assert_eq!(style_preset_by_name(" Amber ").map(|p| p.name), Some("Amber"));
        assert!(style_preset_by_name("pointy").is_none());
    }

    #[test]
    fn set_node_taper_op_writes_and_reports() {
        let mut core = demo_core();
        core.selected_node = Some(1);
        assert!(set_node_taper(&mut core, 0.3));
        assert_eq!(core.project.figures[0].def.nodes[1].style.taper, 0.3);
        assert_eq!(core.status, "Segment Taper 30%.");
        // No selection: refusal, nothing written.
        core.selected_node = None;
        assert!(!set_node_taper(&mut core, 0.6));
        assert_eq!(core.project.figures[0].def.nodes[1].style.taper, 0.3);
        assert_eq!(core.status, "Select a joint first.");
    }

    #[test]
    fn get_node_style_reads_the_whole_style() {
        let mut core = demo_core();
        let out = tool_get_node_style(
            &mut core,
            &serde_json::json!({ "figure": 0, "node": 1 }),
        )
        .unwrap();
        assert_eq!(out["cap"], "round");
        assert_eq!(out["taper"], 1.0);
        assert_eq!(out["opacity"], 1.0);
        assert!(out["color"].is_null(), "no override on a fresh node");
        assert!(out["color2"].is_null());
        assert_eq!(
            out["effective_color"],
            serde_json::json!(core.project.figures[0].color),
            "effective colour falls back to the figure colour"
        );
        let err = tool_get_node_style(
            &mut core,
            &serde_json::json!({ "figure": 0, "node": 99 }),
        )
        .unwrap_err();
        assert!(err.contains("out of range"), "{err}");
    }

    #[test]
    fn set_node_taper_handler_validates_range() {
        let mut core = demo_core();
        let out = tool_set_node_taper(
            &mut core,
            &serde_json::json!({ "figure": 0, "node": 1, "taper": 0.45 }),
        )
        .unwrap();
        assert!(
            (out["taper"].as_f64().unwrap() - 0.45).abs() < 1e-6,
            "stored taper reads back: {out}"
        );
        assert_eq!(core.project.figures[0].def.nodes[1].style.taper, 0.45);
        for bad in [1.5, -0.1] {
            let err = tool_set_node_taper(
                &mut core,
                &serde_json::json!({ "figure": 0, "node": 1, "taper": bad }),
            )
            .unwrap_err();
            assert!(err.contains("between 0.0 and 1.0"), "{err}");
        }
        assert_eq!(
            core.project.figures[0].def.nodes[1].style.taper, 0.45,
            "refused writes change nothing"
        );
    }

    #[test]
    fn cycle_node_taper_handler_steps_like_the_panel() {
        let mut core = demo_core();
        let out = tool_cycle_node_taper(
            &mut core,
            &serde_json::json!({ "figure": 0, "node": 1 }),
        )
        .unwrap();
        assert!(
            (out["taper"].as_f64().unwrap() - 0.6).abs() < 1e-6,
            "1.0 cycles to 0.6, the panel's step: {out}"
        );
        assert_eq!(core.project.figures[0].def.nodes[1].style.taper, 0.6);
    }

    #[test]
    fn set_node_style_handler_is_atomic_and_leaves_taper() {
        let mut core = demo_core();
        core.project.figures[0].def.nodes[1].style.taper = 0.3;
        let undos_before = core.undo_depth();
        let out = tool_set_node_style(
            &mut core,
            &serde_json::json!({
                "figure": 0, "node": 1,
                "color": 0x112233, "color2": 0x445566,
                "cap": "point", "opacity": 0.5,
            }),
        )
        .unwrap();
        let node = &core.project.figures[0].def.nodes[1];
        assert_eq!(node.color, Some(0x112233));
        assert_eq!(node.style.color2, Some(0x445566));
        assert_eq!(node.style.cap, CapKind::Point);
        assert_eq!(node.style.opacity, 0.5);
        assert_eq!(node.style.taper, 0.3, "the atomic four exclude taper");
        assert!(
            (out["taper"].as_f64().unwrap() - 0.3).abs() < 1e-6,
            "and the response says so: {out}"
        );
        assert_eq!(core.undo_depth(), undos_before + 1, "exactly one undo step");
        // A bad cap is refused with the enum's real values.
        let err = tool_set_node_style(
            &mut core,
            &serde_json::json!({
                "figure": 0, "node": 1,
                "color": 0x112233, "cap": "pointy", "opacity": 0.5,
            }),
        )
        .unwrap_err();
        assert!(err.contains("\"point\""), "{err}");
    }

    #[test]
    fn apply_style_preset_handler_matches_the_panel() {
        let mut core = demo_core();
        // Pre-existing gradient + opacity must survive a preset.
        core.project.figures[0].def.nodes[1].style.color2 = Some(0x010203);
        core.project.figures[0].def.nodes[1].style.opacity = 0.7;
        let out = tool_apply_style_preset(
            &mut core,
            &serde_json::json!({ "figure": 0, "node": 1, "preset": "Ocean" }),
        )
        .unwrap();
        let node = &core.project.figures[0].def.nodes[1];
        assert_eq!(node.color, Some(0x2B6FD2));
        assert_eq!(node.style.cap, CapKind::Square);
        assert_eq!(node.style.color2, Some(0x010203), "gradient passes through");
        assert_eq!(node.style.opacity, 0.7, "opacity passes through");
        assert_eq!(out["status"], "Style preset: Ocean.");
        let err = tool_apply_style_preset(
            &mut core,
            &serde_json::json!({ "figure": 0, "node": 1, "preset": "Neon" }),
        )
        .unwrap_err();
        assert!(err.contains("Ink"), "error names the real presets: {err}");
    }
}
