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
//!   with clamps 220..=420 (left) and 240..=480 (right;
//!   the right minimum was 260 before W2-10)
//!   (`src/main.rs:1148-1152`, `:1161-1165`; the same
//!   bounds clamp on load in `shell.rs`). The tool's
//!   applier performs the identical load-modify-save and
//!   reports the widths read back after the save, so a
//!   clamped request truthfully reports the clamped
//!   value.
//! * **Timeline collapse — WIRED (Focused Wave, D).**
//!   In Wave 14 this tool was excluded: the pref
//!   round-tripped but nothing in the running shell read
//!   it, so a setter could only have been a fake control.
//!   The shell now wires it for real (D1): the flag
//!   lives in the timeline's own sub-state,
//!   `State.frame_ops.timeline_collapsed`, seeded from
//!   `LayoutPrefs.timeline_collapsed` in `init`, and
//!   the timeline builder gates the card on it. (The
//!   Rune shell has no GUI collapse toggle — the GPUI
//!   one was lost in the rewrite — so the writers are
//!   the startup pref and this tool.)
//!   `shell_set_timeline` below drives that one field —
//!   never a private copy — and persists it through the
//!   layout prefs' load-modify-save, the write the
//!   workspace switch uses, so a restart restores it.
//!   `shell_get_layout` reports the live field with
//!   `timeline_collapse_wired: true`.
//! * **Sidebar pref — WIRED (Focused Wave, D1).**
//!   `LayoutPrefs.sidebar_visible` is the persisted
//!   mirror of the LEFT column only (`State.show_left`,
//!   per the pref's own doc in `src/shell.rs`): `init`
//!   seeds `show_left` from it, and poll's layout mirror
//!   writes View-menu / palette flips back. The right
//!   column (`show_right`) has no pref in any era and
//!   stays session-only. `shell_get_layout` reports
//!   `sidebar_pref_wired: true`; the live visibility
//!   itself is `show_left` / `show_right` in the same
//!   reply.
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
//!   220..=420, right 240..=480), then reply from a
//!   FRESH `load_layout()`:
//!   `{"left": w, "right": w}` — the read-back widths.
//! * `shell_set_timeline`: write the shell's
//!   `timeline_collapsed` field (the field the view
//!   gates the timeline card on and the UI toggle
//!   writes), persist it by load-modify-save of the
//!   layout prefs, and reply with the live value plus
//!   the persisted read-back:
//!   `{"collapsed": b, "timeline_collapsed": b}`.
//! * `shell_get_layout`: live state + one
//!   `load_layout()`: the workspace, its sections in
//!   table order (`collapsed` the field's value for
//!   Figures/Library, `null` for the rest), the hidden
//!   sections, `show_left` / `show_right`, the column
//!   widths, the wired timeline state with
//!   `timeline_collapse_wired: true`, and the wired
//!   sidebar pref with `sidebar_pref_wired: true`.

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
/// The right minimum was 260 until Roadmap Wave 2
/// (W2-10, stage-share recovery): RM12's design
/// (§4.3.2) priced the Animate layout profile at a
/// 240 pt right column, so every copy of the bound
/// moves together — this constant, the load clamp in
/// `shell.rs` and the per-workspace record clamp.
pub const COL_LEFT_MIN: f32 = 220.0;
pub const COL_LEFT_MAX: f32 = 420.0;
pub const COL_RIGHT_MIN: f32 = 240.0;
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
// Handlers. All four ignore the registry's `core`
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
//       description: "Set the left and/or right panel column widths in points — the draggable splitters, by target width. At least one of 'left' / 'right' is required. Widths persist in the layout prefs exactly as a drag does (load-modify-save) and are clamped to 220-420 (left) / 240-480 (right); the reply reports the widths read back after the save, so a clamped request truthfully reports the clamped value. No undo step. Live endpoint only.",
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
//   | `shell_set_column_widths` | Set the side-column widths (the splitters, by target). At least one of `left` / `right`; clamped to 220–420 / 240–480 and persisted via the layout prefs; the reply reports the read-back widths. No undo step. Live endpoint only. | `left`?, `right`? | — |
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

/// Collapse or expand the timeline — the timeline
/// header's collapse toggle, by target state. This
/// tool was excluded in Wave 14, when the pref it would
/// have written was read by nothing; the Focused Wave
/// wired the pref into the shell (see the module doc),
/// so the tool now drives the real state.
//
// TOOL ENTRY SPEC: for the integrator's registry splice
// in `mcp_tools::tools()`, in the shell-control group,
// after `shell_set_column_widths`:
//
//   Tool {
//       name: "shell_set_timeline",
//       description: "Collapse ('collapsed': true) or expand the timeline in the live editor — the timeline header's collapse toggle. The write lands on the shell's own timeline_collapsed state, so the timeline card hides or shows immediately, and persists through the layout prefs (load-modify-save, as a workspace switch persists), so a restart restores it; the reply echoes the live value and the persisted read-back. No undo step. Live endpoint only.",
//       schema: obj_schema(
//           json!({
//               "collapsed": { "type": "boolean" }
//           }),
//           &["collapsed"],
//       ),
//       handler: crate::shell_tools_layout::tool_shell_set_timeline,
//   }
//
// MCP.md ROW SPEC: for the shell-control table, after
// the `shell_set_column_widths` row:
//
//   | `shell_set_timeline` | Collapse or expand the timeline (the header collapse toggle). Writes the shell's live `timeline_collapsed` state and persists it via the layout prefs; the reply echoes the live value and the persisted read-back. No undo step. Live endpoint only. | `collapsed` | — |
/// Parse + validate a `shell_set_timeline` call (the
/// pure half of the handler — see
/// [`parse_set_section`]).
fn parse_set_timeline(args: &Value) -> Result<bool, String> {
    lay_req_bool(args, "collapsed")
}

pub fn tool_shell_set_timeline(s: &mut State, args: &Value) -> Result<Value, String> {
    let collapsed = parse_set_timeline(args)?;
    // The one wired field: the timeline builder gates
    // the card on it — the tool writes the same field,
    // never a private copy.
    s.frame_ops.timeline_collapsed = collapsed;
    // Persist through the layout prefs' load-modify-save
    // — the workspace switch's own write — so a restart
    // restores the choice (`init` seeds the field from
    // the prefs). The reply reads the persisted value
    // back from a fresh load, like the column widths do.
    let mut prefs = stykanimate_rs::shell::load_layout();
    prefs.timeline_collapsed = collapsed;
    stykanimate_rs::shell::save_layout(&prefs);
    let back = stykanimate_rs::shell::load_layout();
    Ok(json!({
        "collapsed": s.frame_ops.timeline_collapsed,
        "timeline_collapsed": back.timeline_collapsed,
    }))
}

/// Read the live layout back: workspace, the visible
/// sections with their collapse state, the column
/// visibility and widths, the wired timeline state,
/// and the sidebar pref the GUI does not wire yet
/// (reported as such).
//
// TOOL ENTRY SPEC: for the integrator's registry splice
// in `mcp_tools::tools()`, in the shell-control group,
// after `shell_set_column_widths`:
//
//   Tool {
//       name: "shell_get_layout",
//       description: "Read the live editor layout: the active workspace, its sections in panel order (each with 'collapsible' and, for Figures/Library, 'collapsed'; the rest report null), the sections the workspace hides, the left/right column visibility (show_left / show_right) and widths, the timeline_collapsed state (with 'timeline_collapse_wired': true — the shell reads and writes it, see shell_set_timeline), and the persisted sidebar_visible pref (with 'sidebar_pref_wired': true — it is the left column's persisted mirror: show_left seeds from it and flips persist back; live column visibility is show_left / show_right). Read-only. Live endpoint only.",
//       schema: obj_schema(json!({}), &[]),
//       handler: crate::shell_tools_layout::tool_shell_get_layout,
//   }
//
// MCP.md ROW SPEC: for the shell-control table, after
// the `shell_set_column_widths` row:
//
//   | `shell_get_layout` | Read the live layout: active workspace, visible sections + collapse state, hidden sections, column visibility and widths, the wired `timeline_collapsed` state (`timeline_collapse_wired: true`) and the wired `sidebar_visible` pref (`sidebar_pref_wired: true` — the left column's persisted mirror). Read-only. Live endpoint only. | — | — |
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
        // Both prefs are wired (Focused Wave, D1):
        // the timeline state is the live shell field
        // (seeded from the prefs at startup, written
        // back on change), and the sidebar pref is the
        // persisted mirror of `show_left` — seeded from
        // it in `init`, written back by poll's layout
        // mirror whenever the field diverges. The flags
        // say so (see the module doc).
        "timeline_collapsed": s.frame_ops.timeline_collapsed,
        "timeline_collapse_wired": true,
        "sidebar_visible": prefs.sidebar_visible,
        "sidebar_pref_wired": true,
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

    #[test]
    fn set_timeline_validates_its_arguments() {
        let err = parse_set_timeline(&json!({}))
            .expect_err("a missing collapsed flag must be refused");
        assert!(
            err.contains("missing required argument 'collapsed'"),
            "{err}"
        );
        let err = parse_set_timeline(&json!({ "collapsed": "yes" }))
            .expect_err("a non-boolean collapsed flag must be refused");
        assert!(err.contains("must be a boolean"), "{err}");
        assert_eq!(parse_set_timeline(&json!({ "collapsed": true })), Ok(true));
        assert_eq!(parse_set_timeline(&json!({ "collapsed": false })), Ok(false));
    }

    #[test]
    fn set_timeline_flips_the_state_and_get_layout_reports_it_wired() {
        // Handler-level, on a real shell `State` (the
        // `shell_tools_file` tests' `crate::init()`
        // precedent). The handler persists through the
        // shared layout prefs, so the test restores the
        // recorded prefs value before it leaves.
        let mut s = crate::init();
        let original = stykanimate_rs::shell::load_layout().timeline_collapsed;
        let target = !original;
        let out = tool_shell_set_timeline(&mut s, &json!({ "collapsed": target }))
            .expect("the timeline set succeeds");
        assert_eq!(s.frame_ops.timeline_collapsed, target, "the live field flipped");
        assert_eq!(out["collapsed"], json!(target), "the reply echoes the live value");
        assert_eq!(
            out["timeline_collapsed"],
            json!(target),
            "the reply's persisted read-back agrees"
        );
        // The prefs round-trip: a fresh load sees the
        // written value — this is what a restart's
        // `init` would seed the field from.
        assert_eq!(
            stykanimate_rs::shell::load_layout().timeline_collapsed,
            target,
            "the choice persisted to the layout prefs"
        );
        let layout = tool_shell_get_layout(&s, &json!({}))
            .expect("the layout read succeeds");
        assert_eq!(layout["timeline_collapsed"], json!(target));
        assert_eq!(
            layout["timeline_collapse_wired"],
            json!(true),
            "the timeline pref is wired now"
        );
        assert_eq!(
            layout["sidebar_pref_wired"],
            json!(true),
            "the sidebar pref is wired to the left column (D1)"
        );
        // Restore the prefs (and the field) for the next
        // test/process that reads them.
        tool_shell_set_timeline(&mut s, &json!({ "collapsed": original }))
            .expect("the restore succeeds");
        assert_eq!(s.frame_ops.timeline_collapsed, original);
    }
}
