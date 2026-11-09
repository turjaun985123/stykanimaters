//! Shell-control MCP handlers, layout family (Wave 14, B8):
//! panel section collapse, side-column widths, and the
//! layout read-back. These tools act on the GUI *shell*
//! rather than the project: the handlers take the shell
//! `State` (dispatched by `shell_control`'s drain), they
//! spend **no undo step** — the GUI controls they mirror
//! spend none either — and they only work on the live
//! endpoint, where a shell exists to act on. On stdio the
//! registered stub refuses honestly (see the seam below).
//!
//! ---- The layout state model, as found in source -------
//!
//! * **Sections.** `rune_sec::workspaces::Section`
//!   (`src/rune_sec/workspaces.rs:143`) has 30 variants;
//!   `sections_of(ws)` gates them per workspace (Animate
//!   17, Draw 4, Graph 4, Effects 5, Audio 4, Build3D 3,
//!   Export 3, Home 3). A section outside the active
//!   workspace is not mounted at all — there is no panel
//!   to collapse — so `shell_set_section` refuses it, and
//!   the refusal is the applier's (it alone sees the live
//!   `State::workspace`).
//! * **Collapse state.** Exactly two panels own one:
//!   `FiguresPanelState.collapsed: bool`
//!   (`src/rune_sec/figures_panel.rs:214`) and
//!   `LibraryPanelState.collapsed: bool`
//!   (`src/rune_sec/library_panel.rs:416`). `true` renders
//!   the header only; the header's Hide/Show button is the
//!   sole writer (figures_panel.rs:974,
//!   library_panel.rs:923). The state is session-only
//!   view state: it lives in the shell `State`, is never
//!   written to `LayoutPrefs`, and resets on restart —
//!   the tool persists it exactly the same way, i.e. not
//!   at all. **The other 28 sections have no collapse
//!   control anywhere in the editor**, so the tool
//!   refuses them by name rather than inventing state the
//!   GUI cannot show. (A general `collapsed_sections`
//!   set on `State` would be a shell feature, not an MCP
//!   one; it is not specced here.)
//! * **Column widths.** Prefs-backed and live:
//!   `LayoutPrefs.col_left_w` / `col_right_w`
//!   (`src/shell.rs:402`, defaults 300 / 340). The view
//!   re-reads them from `load_layout()` on every pass
//!   (`column_widths()`, `src/main.rs:830`), and the
//!   draggable splitters write them by load-modify-save
//!   with clamps 220..=420 (left) and 260..=480 (right)
//!   (`src/main.rs:1148-1152`, `:1161-1165`; the same
//!   bounds clamp on load in `shell.rs`). The tool's
//!   applier performs the identical load-modify-save and
//!   reports the widths read back after the save, so a
//!   clamped request truthfully reports the clamped
//!   value.
//! * **Timeline collapse — EXCLUDED, with the reason.**
//!   `LayoutPrefs.timeline_collapsed` exists and
//!   round-trips (shell.rs:409 + the salvage reader),
//!   but nothing in the running shell reads it: there is
//!   no `State` field, no toggle control, and the
//!   timeline card in `main.rs`'s view renders
//!   unconditionally. A `shell_set_timeline_collapsed`
//!   tool could only write a prefs field the GUI
//!   ignores — a fake control — so it is NOT provided.
//!   `shell_get_layout` reports the prefs value with
//!   `timeline_collapse_wired: false` so the gap is
//!   visible to clients. Wiring it for real is a shell
//!   change (a `State` field seeded from the prefs in
//!   `init`, the timeline card gated on it in the view,
//!   a toggle that load-modify-saves the prefs) and
//!   belongs to the integrator / a future wave, not to
//!   this handler file. `LayoutPrefs.sidebar_visible`
//!   is in the same state (the live shell uses the
//!   ephemeral `State.show_left` / `show_right`,
//!   `src/main.rs:191-192`, View-menu toggles) and is
//!   reported the same way, as `sidebar_visible_pref`
//!   with `sidebar_pref_wired: false`; the live column
//!   visibility itself is `show_left` / `show_right` in
//!   the same reply.
//!
//! ---- The shell-control seam (as landed) --------------
//!
//! The handlers take `&mut State` and apply directly —
//! they are dispatched by name from
//! `crate::shell_control::drain` on the live endpoint's
//! poll tick, on the UI thread, so no submit/queue seam
//! sits between validation and the write. (The file's
//! first draft specced a `shell_control::submit`
//! command channel; the landed shell control dispatches
//! by tool name instead — see `shell_control.rs`'s
//! module doc.) On stdio the names are registered with
//! the shared `shell_unavailable` stub, so a stdio call
//! refuses with the live-editor wording and never
//! reaches these handlers.
//!
//! The applied semantics, per tool:
//!
//! * `shell_set_section`: the name must be a known
//!   section, one of the two collapsible ones, and
//!   visible in the live workspace (the
//!   [`WORKSPACE_SECTIONS`] table below is the gate);
//!   then the panel field is written
//!   (`s.figures_panel.collapsed` /
//!   `s.library_panel.collapsed`). Reply:
//!   `{"section": name, "collapsed": collapsed}`.
//! * `shell_set_column_widths`: load-modify-save the
//!   layout prefs with the drag clamps applied (left
//!   220..=420, right 260..=480), then reply from a
//!   FRESH `load_layout()`:
//!   `{"left": w, "right": w}` — the read-back widths.
//! * `shell_get_layout`: live state + one
//!   `load_layout()`: the workspace, its sections in
//!   table order (`collapsed` the field's value for
//!   Figures/Library, `null` for the rest), the hidden
//!   sections, `show_left` / `show_right`, the column
//!   widths, and the two unwired prefs with their
//!   `wired: false` flags.

use serde_json::{json, Value};

use crate::State;

/// Every section name on the wire: the
/// `rune_sec::workspaces::Section` variant names, in
/// `Section::all()` order (workspaces.rs, the impl at
/// "All 30 gated sections"). The applier resolves these
/// against the real enum; the count and membership are
/// pinned by this file's tests.
pub const SECTION_NAMES: [&str; 30] = [
    "Figures",
    "Library",
    "Layers",
    "Project",
    "InspectorSegment",
    "InspectorFigure",
    "InspectorStyle",
    "PoseTools",
    "KeyframeTools",
    "Camera",
    "Scene",
    "Onion",
    "Playback",
    "FrameOps",
    "Graph",
    "Easing",
    "DrawTools",
    "StrokeInspector",
    "TextTool",
    "FxBrowser",
    "FxClips",
    "Particles",
    "Audio",
    "Waveform",
    "Clips",
    "Sprites",
    "ThreeD",
    "Export",
    "Stats",
    "Home",
];

/// The sections that own a collapse flag in the GUI —
/// the only ones `shell_set_section` can act on (see
/// the module doc for the two state fields).
pub const COLLAPSIBLE_SECTIONS: [&str; 2] = ["Figures", "Library"];

/// The workspace gating table over [`SECTION_NAMES`],
/// transcribed from `sections_of`'s eight lists in
/// workspaces.rs (Animate 17, Draw 4, Graph 4, Effects
/// 5, Audio 4, Build3D 3, Export 3, Home 3), keyed by
/// the workspace's persisted name (`Workspace::name`;
/// Build3D persists as `"3D"`). The applier gates on
/// the real `section_visible` — this table is the
/// cross-check pin (its union is all 30 names) and the
/// data behind the module doc's counts.
pub const WORKSPACE_SECTIONS: [(&str, &[&str]); 8] = [
    (
        "Animate",
        &[
            "Figures",
            "Library",
            "Layers",
            "Project",
            "InspectorSegment",
            "InspectorFigure",
            "InspectorStyle",
            "PoseTools",
            "KeyframeTools",
            "Camera",
            "Scene",
            "Onion",
            "Playback",
            "FrameOps",
            "Clips",
            "Sprites",
            "Stats",
        ],
    ),
    ("Draw", &["DrawTools", "StrokeInspector", "TextTool", "Figures"]),
    ("Graph", &["Graph", "Easing", "KeyframeTools", "Figures"]),
    (
        "Effects",
        &["FxBrowser", "FxClips", "Particles", "Scene", "Figures"],
    ),
    ("Audio", &["Audio", "Waveform", "Playback", "Figures"]),
    ("3D", &["ThreeD", "Figures", "Camera"]),
    ("Export", &["Export", "Stats", "Project"]),
    ("Home", &["Home", "Project", "Stats"]),
];

/// Column-width clamps, identical to the splitter
/// drags' and the prefs load clamps (see module doc).
pub const COL_LEFT_MIN: f32 = 220.0;
pub const COL_LEFT_MAX: f32 = 420.0;
pub const COL_RIGHT_MIN: f32 = 260.0;
pub const COL_RIGHT_MAX: f32 = 480.0;

// ---------------------------------------------------------------------
// Argument helpers (the media.rs `ast_*` shapes, local
// to this file as every handler file keeps its own).
// ---------------------------------------------------------------------

fn lay_get<'a>(args: &'a Value, key: &str) -> Option<&'a Value> {
    args.as_object().and_then(|o| o.get(key))
}

fn lay_req_str<'a>(args: &'a Value, key: &str) -> Result<&'a str, String> {
    match lay_get(args, key) {
        Some(v) => v
            .as_str()
            .ok_or_else(|| format!("argument '{key}' must be a string")),
        None => Err(format!("missing required argument '{key}'")),
    }
}

fn lay_req_bool(args: &Value, key: &str) -> Result<bool, String> {
    match lay_get(args, key) {
        Some(v) => v
            .as_bool()
            .ok_or_else(|| format!("argument '{key}' must be a boolean")),
        None => Err(format!("missing required argument '{key}'")),
    }
}

/// An optional finite f32 argument. JSON numbers arrive
/// as f64; a value that is not a number, or that
/// overflows f32 to an infinity on the cast, is refused
/// — the Wave 11 finiteness contract.
fn lay_opt_f32(args: &Value, key: &str) -> Result<Option<f32>, String> {
    match lay_get(args, key) {
        Some(v) => {
            let f = v
                .as_f64()
                .ok_or_else(|| format!("argument '{key}' must be a number"))?;
            let f = f as f32;
            if !f.is_finite() {
                return Err(format!("argument '{key}' must be a finite number"));
            }
            Ok(Some(f))
        }
        None => Ok(None),
    }
}

/// Validate a section name against the wire table.
/// The refusal quotes the offending name and lists the
/// vocabulary, in the house refusal shape.
fn lay_check_section(name: &str) -> Result<(), String> {
    if SECTION_NAMES.contains(&name) {
        Ok(())
    } else {
        Err(format!(
            "unknown section '{name}' (expected one of: {})",
            SECTION_NAMES.join(", ")
        ))
    }
}

/// Validate that a (known) section owns a collapse
/// control in the GUI. The refusal names the only two
/// that do — the truth a client needs to stop asking.
fn lay_check_collapsible(name: &str) -> Result<(), String> {
    if COLLAPSIBLE_SECTIONS.contains(&name) {
        Ok(())
    } else {
        Err(format!(
            "section '{name}' has no collapse control in the editor (only Figures and Library can be collapsed)"
        ))
    }
}

// ---------------------------------------------------------------------
// Handlers. All three ignore the registry's `core`
// parameter: shell state is not project state (see the
// module doc). Each returns the applier's reply
// verbatim once its arguments validate.
// ---------------------------------------------------------------------

/// Collapse or expand a panel section — the panel
/// header's Hide/Show control, by name and target
/// state.
//
// TOOL ENTRY SPEC: for the integrator's registry splice
// in `mcp_tools::tools()`, in the shell-control group
// (beside B3's `set_workspace` / `get_shell_state`):
//
//   Tool {
//       name: "shell_set_section",
//       description: "Collapse ('collapsed': true) or expand a panel section in the live editor — the panel header's Hide/Show control. Only the Figures and Library sections have a collapse control; any other section is refused by name. A section not shown in the active workspace is refused (sections are workspace-gated). Shell view state only: no undo step, nothing persisted. Live endpoint only.",
//       schema: obj_schema(
//           json!({
//               "section": { "type": "string" },
//               "collapsed": { "type": "boolean" }
//           }),
//           &["section", "collapsed"],
//       ),
//       handler: crate::shell_tools_layout::tool_shell_set_section,
//   }
//
// MCP.md ROW SPEC: for the shell-control table:
//
//   | `shell_set_section` | Collapse or expand a panel section (the header Hide/Show control). Only Figures and Library are collapsible; a section outside the active workspace is refused. No undo step; session-only. Live endpoint only. | `section`, `collapsed` | — |
/// Parse + validate a `shell_set_section` call (the
/// pure half of the handler, factored out so the tests
/// can pin every refusal without constructing a shell
/// `State`).
fn parse_set_section(args: &Value) -> Result<(String, bool), String> {
    let section = lay_req_str(args, "section")?;
    let collapsed = lay_req_bool(args, "collapsed")?;
    lay_check_section(section)?;
    lay_check_collapsible(section)?;
    Ok((section.to_string(), collapsed))
}

pub fn tool_shell_set_section(s: &mut State, args: &Value) -> Result<Value, String> {
    let (section, collapsed) = parse_set_section(args)?;
    // Workspace gate: a section outside the active
    // workspace is not mounted at all — there is no
    // panel to collapse.
    let ws_name = s.workspace.name();
    let visible = WORKSPACE_SECTIONS
        .iter()
        .find(|(n, _)| *n == ws_name)
        .map(|(_, secs)| secs.contains(&section.as_str()))
        .unwrap_or(false);
    if !visible {
        return Err(format!(
            "section '{section}' is not shown in the {ws_name} workspace"
        ));
    }
    match section.as_str() {
        "Figures" => s.figures_panel.collapsed = collapsed,
        "Library" => s.library_panel.collapsed = collapsed,
        _ => unreachable!("the collapsible check passed"),
    }
    Ok(json!({ "section": section, "collapsed": collapsed }))
}

/// Set the side-column widths — the panel splitters'
/// drag, by target width instead of by delta.
//
// TOOL ENTRY SPEC: for the integrator's registry splice
// in `mcp_tools::tools()`, in the shell-control group,
// after `shell_set_section`:
//
//   Tool {
//       name: "shell_set_column_widths",
//       description: "Set the left and/or right panel column widths in points — the draggable splitters, by target width. At least one of 'left' / 'right' is required. Widths persist in the layout prefs exactly as a drag does (load-modify-save) and are clamped to 220-420 (left) / 260-480 (right); the reply reports the widths read back after the save, so a clamped request truthfully reports the clamped value. No undo step. Live endpoint only.",
//       schema: obj_schema(
//           json!({
//               "left": { "type": "number" },
//               "right": { "type": "number" }
//           }),
//           &[],
//       ),
//       handler: crate::shell_tools_layout::tool_shell_set_column_widths,
//   }
//
// MCP.md ROW SPEC: for the shell-control table, after
// the `shell_set_section` row:
//
//   | `shell_set_column_widths` | Set the side-column widths (the splitters, by target). At least one of `left` / `right`; clamped to 220–420 / 260–480 and persisted via the layout prefs; the reply reports the read-back widths. No undo step. Live endpoint only. | `left`?, `right`? | — |
/// Parse + validate a `shell_set_column_widths` call
/// (the pure half of the handler — see
/// [`parse_set_section`]).
fn parse_set_column_widths(args: &Value) -> Result<(Option<f32>, Option<f32>), String> {
    let left = lay_opt_f32(args, "left")?;
    let right = lay_opt_f32(args, "right")?;
    if left.is_none() && right.is_none() {
        return Err(
            "argument 'left' or 'right' is required (give at least one column width)".into(),
        );
    }
    Ok((left, right))
}

pub fn tool_shell_set_column_widths(
    _s: &mut State,
    args: &Value,
) -> Result<Value, String> {
    let (left, right) = parse_set_column_widths(args)?;
    // The splitters' own write: load-modify-save the
    // layout prefs, clamped to the same bounds the drag
    // clamps to. The reply reads the widths back from a
    // fresh load, so a clamped request reports the
    // clamped truth.
    let mut prefs = stykanimate_rs::shell::load_layout();
    if let Some(l) = left {
        prefs.col_left_w = l.clamp(COL_LEFT_MIN, COL_LEFT_MAX);
    }
    if let Some(r) = right {
        prefs.col_right_w = r.clamp(COL_RIGHT_MIN, COL_RIGHT_MAX);
    }
    stykanimate_rs::shell::save_layout(&prefs);
    let back = stykanimate_rs::shell::load_layout();
    Ok(json!({ "left": back.col_left_w, "right": back.col_right_w }))
}

/// Read the live layout back: workspace, the visible
/// sections with their collapse state, the column
/// visibility and widths, and the two prefs fields the
/// GUI does not currently wire (reported as such).
//
// TOOL ENTRY SPEC: for the integrator's registry splice
// in `mcp_tools::tools()`, in the shell-control group,
// after `shell_set_column_widths`:
//
//   Tool {
//       name: "shell_get_layout",
//       description: "Read the live editor layout: the active workspace, its sections in panel order (each with 'collapsible' and, for Figures/Library, 'collapsed'; the rest report null), the sections the workspace hides, the left/right column visibility (show_left / show_right) and widths, and the persisted timeline_collapsed / sidebar_visible prefs — each with a 'wired' flag, currently false: those prefs round-trip but the shell does not read them yet. Read-only. Live endpoint only.",
//       schema: obj_schema(json!({}), &[]),
//       handler: crate::shell_tools_layout::tool_shell_get_layout,
//   }
//
// MCP.md ROW SPEC: for the shell-control table, after
// the `shell_set_column_widths` row:
//
//   | `shell_get_layout` | Read the live layout: active workspace, visible sections + collapse state, hidden sections, column visibility and widths, and the (currently unwired) timeline/sidebar prefs with their `wired` flags. Read-only. Live endpoint only. | — | — |
pub fn tool_shell_get_layout(s: &State, _args: &Value) -> Result<Value, String> {
    let prefs = stykanimate_rs::shell::load_layout();
    let ws_name = s.workspace.name();
    let visible: &[&str] = WORKSPACE_SECTIONS
        .iter()
        .find(|(n, _)| *n == ws_name)
        .map(|(_, secs)| *secs)
        .unwrap_or(&[]);
    let sections: Vec<Value> = visible
        .iter()
        .map(|name| {
            let collapsed = match *name {
                "Figures" => Some(s.figures_panel.collapsed),
                "Library" => Some(s.library_panel.collapsed),
                _ => None,
            };
            json!({
                "name": name,
                "collapsible": COLLAPSIBLE_SECTIONS.contains(name),
                "collapsed": collapsed,
            })
        })
        .collect();
    let hidden: Vec<&str> = SECTION_NAMES
        .iter()
        .filter(|n| !visible.contains(n))
        .copied()
        .collect();
    Ok(json!({
        "workspace": ws_name,
        "sections": sections,
        "hidden_sections": hidden,
        "show_left": s.show_left,
        "show_right": s.show_right,
        "col_left_w": prefs.col_left_w,
        "col_right_w": prefs.col_right_w,
        // Two prefs fields round-trip through the layout
        // file but no shell code reads them yet — the
        // honest report carries that as a flag, not a
        // fake success (see the module doc).
        "timeline_collapsed": prefs.timeline_collapsed,
        "timeline_collapse_wired": false,
        "sidebar_visible": prefs.sidebar_visible,
        "sidebar_pref_wired": false,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn section_table_is_the_full_thirty_with_no_duplicates() {
        assert_eq!(SECTION_NAMES.len(), 30);
        let mut sorted = SECTION_NAMES.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), 30, "section names must be unique");
    }

    #[test]
    fn workspace_table_counts_and_union_match_the_gating_source() {
        // Per-workspace counts pinned in workspaces.rs
        // (`section_counts_per_workspace_are_pinned`).
        let counts: Vec<usize> = WORKSPACE_SECTIONS.iter().map(|(_, ss)| ss.len()).collect();
        assert_eq!(counts, vec![17, 4, 4, 5, 4, 3, 3, 3]);
        // The union of the eight lists is exactly the
        // 30-name table — every section lives somewhere,
        // no phantom names.
        let mut union: Vec<&str> = WORKSPACE_SECTIONS
            .iter()
            .flat_map(|(_, ss)| ss.iter().copied())
            .collect();
        union.sort_unstable();
        union.dedup();
        assert_eq!(union.len(), 30);
        for name in SECTION_NAMES {
            assert!(union.contains(&name), "{name} gated nowhere");
        }
        // The two collapsible sections' homes: Figures
        // in six workspaces, Library in Animate only —
        // the gating facts the set_section refusals rely
        // on.
        let figures_homes = WORKSPACE_SECTIONS
            .iter()
            .filter(|(_, ss)| ss.contains(&"Figures"))
            .count();
        assert_eq!(figures_homes, 6);
        let library_homes: Vec<&str> = WORKSPACE_SECTIONS
            .iter()
            .filter(|(_, ss)| ss.contains(&"Library"))
            .map(|(ws, _)| *ws)
            .collect();
        assert_eq!(library_homes, vec!["Animate"]);
    }

    #[test]
    fn set_section_refuses_an_unknown_section_by_name() {
        let err = parse_set_section(&json!({ "section": "Bogus", "collapsed": true }))
            .expect_err("an unknown section must be refused");
        assert!(err.contains("unknown section 'Bogus'"), "err: {err}");
        assert!(err.contains("Figures"), "err lists the vocabulary: {err}");
    }

    #[test]
    fn set_section_refuses_a_section_with_no_collapse_control() {
        for name in ["Camera", "Sprites", "Graph", "Home"] {
            let err = parse_set_section(&json!({ "section": name, "collapsed": true }))
                .expect_err("a non-collapsible section must be refused");
            assert!(
                err.contains("has no collapse control"),
                "{name}: {err}"
            );
            assert!(err.contains(name), "{err}");
        }
    }

    #[test]
    fn set_section_validates_its_arguments() {
        let err = parse_set_section(&json!({ "collapsed": true }))
            .expect_err("a missing section must be refused");
        assert!(err.contains("missing required argument 'section'"), "{err}");
        let err = parse_set_section(&json!({ "section": "Figures" }))
            .expect_err("a missing collapsed flag must be refused");
        assert!(
            err.contains("missing required argument 'collapsed'"),
            "{err}"
        );
        let err = parse_set_section(&json!({ "section": "Figures", "collapsed": "yes" }))
            .expect_err("a non-boolean collapsed flag must be refused");
        assert!(err.contains("must be a boolean"), "{err}");
    }

    #[test]
    fn set_section_with_a_valid_request_parses() {
        // The application half needs a live shell
        // `State`; the parse half is what a bad client
        // can reach, and it accepts exactly the two
        // collapsible names.
        assert_eq!(
            parse_set_section(&json!({ "section": "Figures", "collapsed": true })),
            Ok(("Figures".to_string(), true))
        );
        assert_eq!(
            parse_set_section(&json!({ "section": "Library", "collapsed": false })),
            Ok(("Library".to_string(), false))
        );
    }

    #[test]
    fn set_column_widths_requires_at_least_one_side() {
        let err = parse_set_column_widths(&json!({}))
            .expect_err("no width at all must be refused");
        assert!(err.contains("'left' or 'right' is required"), "{err}");
    }

    #[test]
    fn set_column_widths_refuses_non_finite_and_non_number_widths() {
        // 1e300 is a finite f64 that overflows the f32
        // cast to +inf — the finiteness contract refuses
        // it (the Wave 11 rule, incl. cast overflow).
        let err = parse_set_column_widths(&json!({ "left": 1e300 }))
            .expect_err("an overflowing width must be refused");
        assert!(err.contains("must be a finite number"), "{err}");
        let err = parse_set_column_widths(&json!({ "right": "wide" }))
            .expect_err("a string width must be refused");
        assert!(err.contains("must be a number"), "{err}");
    }
}
