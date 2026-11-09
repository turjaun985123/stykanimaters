//! Wave 14 (B5) — MCP shell VIEW tools: `shell_view_zoom`,
//! `shell_view_fit`, `shell_view_centre`, `shell_view_reset`
//! and `shell_get_view`.
//!
//! # What these tools drive — and what they never touch
//!
//! The only state here is the shell's view-navigation layer
//! in `stage_view` (binary crate, `src/stage_view.rs`):
//! the `VIEW_NAV` thread-local holding
//! `ViewNav { zoom: f32, pan: [f32; 2] }`
//! (`stage_view.rs:143-149`, bounds `VIEW_ZOOM_MIN = 0.1` /
//! `VIEW_ZOOM_MAX = 8.0` at `stage_view.rs:165-167`). It is
//! a zoom/pan composed OVER the authored camera transform
//! so a detail can be inspected without editing the film.
//!
//! Wave 5 doctrine, which these tools preserve exactly:
//! view navigation NEVER writes a camera key, NEVER takes
//! an undo snapshot and NEVER marks the project dirty.
//! Accordingly every handler below:
//!
//! * calls only `stage_view` view-nav functions — no
//!   `EditorCore` op is invoked, and the `&mut EditorCore`
//!   parameter (kept so the handler shape matches the rest
//!   of the MCP surface) is only READ, for the camera
//!   zoom / selected figure the view math needs;
//! * is NOT an undo step and does NOT bump the project's
//!   edit count — there is no snapshot call anywhere in
//!   this file, and a test below pins the undo depth and
//!   edit count across every mutating handler;
//! * mirrors a camera-overlay pill control one-for-one
//!   (`rune_sec/camera_panel.rs` `overlay`): Fit =
//!   `reset_view_nav()`, Centre = `view_centre`
//!   (`camera_panel.rs:1243`), 1:1 = `view_actual`
//!   (`camera_panel.rs:1230`), the steppers =
//!   `view_zoom_by` (`camera_panel.rs:1329,1333`). The
//!   Camera PANEL's same-named Fit / Centre / 1:1 are the
//!   authoring versions (they upsert camera keys) and are
//!   deliberately NOT what these tools do.
//!
//! # Crate placement — INTEGRATOR SPLICE (read first)
//!
//! This module belongs to the BINARY crate: declare it in
//! `src/main.rs` beside `mod stage_view;` (main.rs:24) as
//! `mod shell_tools_view;`. It must NOT go into
//! `src/lib.rs`: `stage_view` and `rune_sec` are declared
//! only in `main.rs` (main.rs:22-24), so the library crate
//! — where `mcp_tools.rs` / `media.rs` live — cannot name
//! `ViewNav` at all. That split is the reason shell tools
//! exist separately from the registry tools.
//!
//! # Dispatch contract — INTEGRATOR / B2 SPLICE
//!
//! `VIEW_NAV` is a `thread_local`: these handlers are only
//! correct when invoked on the GUI thread, which is what
//! B2's shell-command queue + main-poll-loop drain
//! guarantees for the live endpoint. Route calls through
//! [`dispatch_view_tool`] from that drain. The stdio
//! server must NOT dispatch these tools (on its thread
//! the view cell is a different, invisible one) — per
//! B1's design it answers them with an honest refusal
//! ("shell tools need the live editor"). Do not add these
//! tools to the stdio `all_tools()` registry.

use serde_json::{json, Value};
use stykanimate_rs::editor::EditorCore;

use crate::stage_view::{
    reset_view_nav, set_view_nav, view_centre_on, view_nav, view_zoom_factor, ViewNav,
    VIEW_ZOOM_MAX, VIEW_ZOOM_MIN,
};

// ---------------------------------------------------------------------
// Argument helpers (the mcp_tools.rs family shape, duplicated
// because that module is in the library crate and its helpers
// are private there; the finiteness rule is identical).
// ---------------------------------------------------------------------

fn get<'a>(args: &'a Value, key: &str) -> Option<&'a Value> {
    args.get(key).filter(|v| !v.is_null())
}

/// Wave 11 finiteness rule: a JSON number like 1e999 parses
/// to f64 infinity, and a finite f64 like 1e300 still casts
/// to f32 infinity — both are refused.
fn finite_f32(f: f64, key: &str) -> Result<f32, String> {
    let v = f as f32;
    if f.is_finite() && v.is_finite() {
        Ok(v)
    } else {
        Err(format!("argument '{key}' must be a finite number"))
    }
}

fn opt_f32(args: &Value, key: &str) -> Result<Option<f32>, String> {
    match get(args, key) {
        Some(v) => match v.as_f64() {
            Some(f) => finite_f32(f, key).map(Some),
            None => Err(format!("argument '{key}' must be a number")),
        },
        None => Ok(None),
    }
}

// ---------------------------------------------------------------------
// The shared readout (every handler returns it, so a caller
// can always verify the view it ended in).
// ---------------------------------------------------------------------

/// The view state as JSON — the reply body of every tool in
/// this file (mutating replies add their own fields).
///
/// * `view_zoom` — the view-layer multiplier
///   (`ViewNav::zoom`), 1.0 = the camera's own framing.
/// * `pan` — the view pan in canvas element points
///   (`ViewNav::pan`); the displayed image shifts by
///   exactly this vector.
/// * `at_camera_framing` — the "fit state": true when the
///   view IS the camera framing (zoom 1.0, pan [0, 0]).
///   There is no stored fit flag anywhere in the shell;
///   identity is the state, so identity is what is
///   reported.
/// * `camera_zoom` — the AUTHORED camera zoom at the
///   current frame (`EditorCore::camera_here().2`),
///   reported for context only; no tool here changes it.
/// * `effective_zoom` / `effective_percent` — what the
///   stage actually shows: `stage_view::view_zoom_factor`
///   (camera × view) and its percentage rounded exactly
///   as the overlay readout rounds it
///   (`camera_panel::view_zoom_pct`).
/// * `stage_element_size` — the size the stage element
///   last drew at (`stage_view::last_stage_size`);
///   [0, 0] before the first frame has been laid out.
pub fn view_state_json(core: &EditorCore) -> Value {
    let nav = view_nav();
    let effective = view_zoom_factor(core);
    json!({
        "view_zoom": nav.zoom,
        "pan": [nav.pan[0], nav.pan[1]],
        "at_camera_framing": nav.zoom == 1.0 && nav.pan == [0.0, 0.0],
        "camera_zoom": core.camera_here().2,
        "effective_zoom": effective,
        "effective_percent": (effective * 100.0).round() as i64,
        "frame": core.current,
        "stage_element_size": crate::stage_view::last_stage_size(),
        "stage_size": core.project.stage,
    })
}

fn ok_with(core: &EditorCore, extra: Value) -> Value {
    let mut reply = json!({ "ok": true });
    if let (Some(r), Some(e)) = (reply.as_object_mut(), extra.as_object()) {
        for (k, v) in e {
            r.insert(k.clone(), v.clone());
        }
    }
    if let (Some(r), Some(s)) = (reply.as_object_mut(), view_state_json(core).as_object()) {
        for (k, v) in s {
            r.insert(k.clone(), v.clone());
        }
    }
    reply
}

// ---------------------------------------------------------------------
// Handlers. Signature matches the MCP handler family
// (core, args) -> Result<Value, String>; see the module
// docs for why `core` is read-only here.
// ---------------------------------------------------------------------

/// MCP handler: set the shell VIEW zoom — view navigation
/// only (see module docs: no camera key, no undo step, no
/// dirty mark).
///
/// Exactly one of:
/// * `factor` — multiply the current view zoom by this
///   (the overlay steppers' operation, `view_zoom_by`;
///   the GUI step is 1.25×). Must be finite and > 0.
/// * `percent` — set the EFFECTIVE zoom (authored camera
///   zoom × view zoom — the number the overlay readout
///   shows) to this percent, by setting the view layer to
///   `percent / 100 / camera_zoom`: the general form of
///   the pill's 1:1 (`view_actual`). Must be finite
///   and > 0.
///
/// The resulting view zoom is clamped to the GUI's own
/// range, `VIEW_ZOOM_MIN..=VIEW_ZOOM_MAX` (0.1..=8.0 —
/// the clamp `set_view_nav` itself applies), and the
/// reply's `clamped` says whether the request hit it, so
/// a caller is never silently shown a different zoom
/// than it asked for.
///
/// TOOL ENTRY SPEC (integrator: live shell registry only,
/// NOT the stdio registry — see module docs):
///   name: "shell_view_zoom"
///   description: "Set the editor's VIEW zoom without touching the authored camera: 'factor' multiplies the current view zoom (the overlay steppers' operation), 'percent' sets the effective on-screen zoom (camera × view, the overlay readout's number) to that percent. Exactly one of the two is required. The view zoom is clamped to the GUI's own 0.1×–8× range and the reply reports 'clamped' when it is. View navigation only: no camera key is written, it is not an undo step and it does not mark the project dirty."
///   schema: obj_schema(json!({
///       "factor":  { "type": "number" },
///       "percent": { "type": "number" }
///   }), &[])
///   handler: crate::shell_tools_view::tool_shell_view_zoom
///
/// MCP.md row spec:
/// | `shell_view_zoom` | `factor` ⊕ `percent` | View-only zoom: `factor` multiplies the view zoom; `percent` sets the effective (camera × view) zoom %. Clamped to the view range 0.1×–8×; reply reports the resulting view state + `clamped`. No camera key, no undo step, no dirty mark. Live shell only. |
pub fn tool_shell_view_zoom(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let factor = opt_f32(args, "factor")?;
    let percent = opt_f32(args, "percent")?;
    let nav = view_nav();
    let (mode, requested, requested_view) = match (factor, percent) {
        (Some(f), None) => {
            if f <= 0.0 {
                return Err(format!(
                    "argument 'factor' must be greater than 0 (got {f})"
                ));
            }
            ("factor", f, nav.zoom * f)
        }
        (None, Some(p)) => {
            if p <= 0.0 {
                return Err(format!(
                    "argument 'percent' must be greater than 0 (got {p})"
                ));
            }
            let cam_zoom = core.camera_here().2;
            if cam_zoom <= 0.0 {
                return Err(format!(
                    "cannot set an effective zoom percent: the authored camera zoom at frame {} is {cam_zoom}, not positive",
                    core.current + 1
                ));
            }
            ("percent", p, (p / 100.0) / cam_zoom)
        }
        (Some(_), Some(_)) => {
            return Err(
                "arguments 'factor' and 'percent' are alternatives — pass exactly one"
                    .to_string(),
            );
        }
        (None, None) => {
            return Err("missing argument: pass 'factor' or 'percent'".to_string());
        }
    };
    set_view_nav(ViewNav {
        zoom: requested_view,
        pan: nav.pan,
    });
    let applied = view_nav().zoom;
    Ok(ok_with(
        core,
        json!({
            "mode": mode,
            "requested": requested,
            "clamped": applied != requested_view,
        }),
    ))
}

/// MCP handler: Fit — restore the camera's own framing in
/// the VIEW (the overlay pill's Fit, `reset_view_nav`:
/// view zoom 1.0, pan [0, 0]). View navigation only: no
/// camera key, no undo step, no dirty mark. This is NOT
/// the Camera panel's Fit (`frame_all_figures`), which
/// authors a camera key — that stays panel/palette
/// territory.
///
/// TOOL ENTRY SPEC (integrator: live shell registry only):
///   name: "shell_view_fit"
///   description: "Fit the stage view to the authored camera's own framing (the stage pill's Fit): resets the view zoom to 1× and the view pan to zero. View navigation only — unlike the Camera panel's Fit it writes no camera key, is not an undo step and does not mark the project dirty."
///   schema: obj_schema(json!({}), &[])
///   handler: crate::shell_tools_view::tool_shell_view_fit
///
/// MCP.md row spec:
/// | `shell_view_fit` | — | Pill Fit: view zoom 1×, pan zero (the camera's own framing). No camera key, no undo step, no dirty mark. Live shell only. |
pub fn tool_shell_view_fit(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let _ = args; // Fit takes no arguments.
    reset_view_nav();
    Ok(ok_with(core, json!({})))
}

/// MCP handler: Centre — centre the VIEW on the selected
/// figure's root, exactly as the overlay pill's Centre
/// does (`camera_panel::view_centre`): pan the view layer
/// so the root sits at the element centre; if no stage
/// has been laid out yet, fall back to clearing the view
/// pan (the reply's `fallback` names it). With explicit
/// `x` + `y` (stage units, both required together) the
/// view centres on that stage point instead, through the
/// same `view_centre_on` primitive — and with no laid-out
/// stage that is a refusal, since there is no fallback
/// point to clear to. With no figures and no point:
/// refused, mirroring the pill's "No figures to centre
/// on." status. View navigation only: no camera key, no
/// undo step, no dirty mark.
///
/// TOOL ENTRY SPEC (integrator: live shell registry only):
///   name: "shell_view_centre"
///   description: "Centre the stage VIEW on the selected figure's root (the stage pill's Centre), panning the view layer only. Optional 'x' and 'y' (stage units, both together) centre on that stage point instead. Writes no camera key, is not an undo step and does not mark the project dirty."
///   schema: obj_schema(json!({
///       "x": { "type": "number" },
///       "y": { "type": "number" }
///   }), &[])
///   handler: crate::shell_tools_view::tool_shell_view_centre
///
/// MCP.md row spec:
/// | `shell_view_centre` | `x`, `y` (optional pair) | Pill Centre: pan the view so the selected figure's root (or the given stage point) sits at the element centre; clears the pan when no stage is laid out yet (figure form only). No camera key, no undo step, no dirty mark. Live shell only. |
pub fn tool_shell_view_centre(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let x = opt_f32(args, "x")?;
    let y = opt_f32(args, "y")?;
    match (x, y) {
        (Some(x), Some(y)) => {
            if !view_centre_on(core, [x, y]) {
                return Err(
                    "the stage has not been laid out yet, so there is no view to centre"
                        .to_string(),
                );
            }
            Ok(ok_with(core, json!({ "centred_on": [x, y] })))
        }
        (None, None) => {
            // The pill's `view_centre`, step for step.
            let root = core
                .project
                .poses_at(core.current as f32)
                .get(core.selected_figure)
                .map(|pose| pose.root);
            match root {
                Some(root) => {
                    if view_centre_on(core, root) {
                        Ok(ok_with(core, json!({ "centred_on": root })))
                    } else {
                        let nav = view_nav();
                        set_view_nav(ViewNav {
                            zoom: nav.zoom,
                            pan: [0.0, 0.0],
                        });
                        Ok(ok_with(
                            core,
                            json!({ "fallback": "pan_cleared_no_stage_layout" }),
                        ))
                    }
                }
                None => Err("no figures to centre on".to_string()),
            }
        }
        _ => Err("arguments 'x' and 'y' must be passed together".to_string()),
    }
}

/// MCP handler: 1:1 — set the VIEW zoom so the EFFECTIVE
/// zoom is exactly 100% (the overlay pill's 1:1,
/// `camera_panel::view_actual`): view zoom =
/// 1 / authored camera zoom, pan unchanged. Despite the
/// name this is not a view reset — Fit
/// (`shell_view_fit`) is the reset. The view clamp
/// (0.1..=8.0) still applies, so with an authored camera
/// zoom below 0.125 the effective zoom lands below 100%
/// and the reply's `clamped` says so. View navigation
/// only: no camera key, no undo step, no dirty mark.
///
/// TOOL ENTRY SPEC (integrator: live shell registry only):
///   name: "shell_view_reset"
///   description: "The stage pill's 1:1: set the VIEW zoom so the effective on-screen zoom (camera × view) is exactly 100%, leaving the view pan unchanged. Despite the name this is not a reset to the camera framing — that is shell_view_fit. Writes no camera key, is not an undo step and does not mark the project dirty."
///   schema: obj_schema(json!({}), &[])
///   handler: crate::shell_tools_view::tool_shell_view_reset
///
/// MCP.md row spec:
/// | `shell_view_reset` | — | Pill 1:1: view zoom = 1 / camera zoom (effective zoom 100%), pan unchanged; `clamped` reported when the view range prevents exactly 100%. No camera key, no undo step, no dirty mark. Live shell only. |
pub fn tool_shell_view_reset(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let _ = args;
    let cam_zoom = core.camera_here().2;
    if cam_zoom <= 0.0 {
        return Err(format!(
            "cannot set the 1:1 view: the authored camera zoom at frame {} is {cam_zoom}, not positive",
            core.current + 1
        ));
    }
    let nav = view_nav();
    let requested = 1.0 / cam_zoom;
    set_view_nav(ViewNav {
        zoom: requested,
        pan: nav.pan,
    });
    let applied = view_nav().zoom;
    Ok(ok_with(
        core,
        json!({ "clamped": applied != requested }),
    ))
}

/// MCP handler: read the shell view state — see
/// [`view_state_json`] for the fields. Read-only: it
/// changes nothing at all (no view change, no camera
/// key, no undo step, no dirty mark).
///
/// TOOL ENTRY SPEC (integrator: live shell registry only):
///   name: "shell_get_view"
///   description: "Read the shell view state: the view zoom multiplier and pan (canvas points), 'at_camera_framing' (the view is exactly the authored camera framing), the authored camera zoom at the current frame, the effective on-screen zoom (camera × view) as a factor and a percent, and the stage element/stage sizes. Read-only."
///   schema: obj_schema(json!({}), &[])
///   handler: crate::shell_tools_view::tool_shell_get_view
///
/// MCP.md row spec:
/// | `shell_get_view` | — | Read the view state: `view_zoom`, `pan`, `at_camera_framing`, `camera_zoom`, `effective_zoom`, `effective_percent`, `frame`, `stage_element_size`, `stage_size`. Read-only. Live shell only. |
pub fn tool_shell_get_view(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let _ = args;
    Ok(ok_with(core, json!({})))
}

// ---------------------------------------------------------------------
// Router for B2's shell-command drain.
// ---------------------------------------------------------------------

/// Dispatch one shell view tool by name. Returns `None`
/// for a name this module does not own, so B2's drain can
/// fall through to the other shell families (and answer
/// −32602 "unknown tool" for a name nobody owns, per the
/// standing contract). Must be called on the GUI thread —
/// see the module docs.
pub fn dispatch_view_tool(
    name: &str,
    core: &mut EditorCore,
    args: &Value,
) -> Option<Result<Value, String>> {
    Some(match name {
        "shell_view_zoom" => tool_shell_view_zoom(core, args),
        "shell_view_fit" => tool_shell_view_fit(core, args),
        "shell_view_centre" => tool_shell_view_centre(core, args),
        "shell_view_reset" => tool_shell_view_reset(core, args),
        "shell_get_view" => tool_shell_get_view(core, args),
        _ => return None,
    })
}

/// The tool names this module owns (for B2's / E9's
/// shell census).
pub const VIEW_TOOL_NAMES: [&str; 5] = [
    "shell_view_zoom",
    "shell_view_fit",
    "shell_view_centre",
    "shell_view_reset",
    "shell_get_view",
];

#[cfg(test)]
mod tests {
    use super::*;

    fn core() -> EditorCore {
        reset_view_nav();
        EditorCore::new()
    }

    #[test]
    fn zoom_factor_multiplies_and_reports_clamp() {
        let mut c = core();
        let out = tool_shell_view_zoom(&mut c, &json!({ "factor": 2.0 })).unwrap();
        assert_eq!(out["view_zoom"].as_f64(), Some(2.0));
        assert_eq!(out["clamped"].as_bool(), Some(false));
        // Past the GUI's own max (8.0): clamped, and the
        // reply says so instead of echoing the request.
        let out = tool_shell_view_zoom(&mut c, &json!({ "factor": 100.0 })).unwrap();
        assert_eq!(out["view_zoom"].as_f64(), Some(VIEW_ZOOM_MAX as f64));
        assert_eq!(out["clamped"].as_bool(), Some(true));
        // And at the min (0.1) on the way back down.
        let out = tool_shell_view_zoom(&mut c, &json!({ "factor": 0.0001 })).unwrap();
        assert_eq!(out["view_zoom"].as_f64(), Some(VIEW_ZOOM_MIN as f64));
        assert_eq!(out["clamped"].as_bool(), Some(true));
    }

    #[test]
    fn zoom_percent_sets_the_effective_zoom() {
        let mut c = core();
        // Fresh core: authored camera zoom 1.0, so 200%
        // effective is view zoom 2.0.
        let out = tool_shell_view_zoom(&mut c, &json!({ "percent": 200.0 })).unwrap();
        assert_eq!(out["view_zoom"].as_f64(), Some(2.0));
        assert_eq!(out["effective_percent"].as_i64(), Some(200));
    }

    #[test]
    fn zoom_refuses_bad_arguments() {
        let mut c = core();
        assert!(tool_shell_view_zoom(&mut c, &json!({})).is_err());
        assert!(
            tool_shell_view_zoom(&mut c, &json!({ "factor": 2.0, "percent": 200.0 }))
                .is_err()
        );
        assert!(tool_shell_view_zoom(&mut c, &json!({ "factor": 0.0 })).is_err());
        assert!(tool_shell_view_zoom(&mut c, &json!({ "factor": -1.5 })).is_err());
        assert!(tool_shell_view_zoom(&mut c, &json!({ "percent": "200" })).is_err());
        // 1e300 is a finite f64 that casts to f32 infinity.
        assert!(tool_shell_view_zoom(&mut c, &json!({ "factor": 1e300 })).is_err());
        // Refusals spend nothing: the view is untouched.
        assert_eq!(view_nav(), ViewNav::IDENTITY);
    }

    #[test]
    fn fit_restores_the_camera_framing() {
        let mut c = core();
        tool_shell_view_zoom(&mut c, &json!({ "factor": 3.0 })).unwrap();
        let out = tool_shell_view_fit(&mut c, &json!({})).unwrap();
        assert_eq!(out["at_camera_framing"].as_bool(), Some(true));
        assert_eq!(view_nav(), ViewNav::IDENTITY);
    }

    #[test]
    fn reset_makes_the_effective_zoom_100_percent() {
        let mut c = core();
        tool_shell_view_zoom(&mut c, &json!({ "factor": 4.0 })).unwrap();
        let out = tool_shell_view_reset(&mut c, &json!({})).unwrap();
        assert_eq!(out["effective_percent"].as_i64(), Some(100));
        assert_eq!(out["clamped"].as_bool(), Some(false));
    }

    #[test]
    fn view_tools_never_snapshot_or_mark_dirty() {
        let mut c = core();
        let undo_before = c.undo_depth();
        // `edit_count` is private; `edits_since_autosave`
        // is the public counter the dirty dot reads, and
        // it moves iff `edit_count` does on a core no save
        // path touches.
        let edits_before = c.edits_since_autosave();
        tool_shell_view_zoom(&mut c, &json!({ "factor": 2.0 })).unwrap();
        tool_shell_view_centre(&mut c, &json!({})).unwrap();
        tool_shell_view_reset(&mut c, &json!({})).unwrap();
        tool_shell_view_fit(&mut c, &json!({})).unwrap();
        tool_shell_get_view(&mut c, &json!({})).unwrap();
        assert_eq!(c.undo_depth(), undo_before, "view tools are not undo steps");
        assert_eq!(c.edits_since_autosave(), edits_before, "view tools never mark dirty");
    }

    #[test]
    fn get_view_reports_the_readout_fields() {
        let mut c = core();
        let out = tool_shell_get_view(&mut c, &json!({})).unwrap();
        assert_eq!(out["ok"].as_bool(), Some(true));
        assert_eq!(out["view_zoom"].as_f64(), Some(1.0));
        assert_eq!(out["at_camera_framing"].as_bool(), Some(true));
        assert_eq!(out["effective_percent"].as_i64(), Some(100));
        assert!(out["pan"].is_array());
        assert!(out["stage_size"].is_array());
    }

    #[test]
    fn dispatch_routes_known_names_only() {
        let mut c = core();
        assert!(dispatch_view_tool("shell_get_view", &mut c, &json!({})).is_some());
        assert!(dispatch_view_tool("camera_set_key", &mut c, &json!({})).is_none());
        assert_eq!(VIEW_TOOL_NAMES.len(), 5);
    }
}
