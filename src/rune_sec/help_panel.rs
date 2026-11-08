//! Help panel for the Rune editor shell (Rune migration
//! Phase D, section 25/40; Phase E, section 23/40): a pure
//! builder over `&EditorCore` — it reads project facts for
//! its header and never mutates the core. The panel's
//! open/close state lives in the shell's menu state, not
//! here.
//!
//! Content discipline (the GPUI-era `ui::help` rule, kept):
//! every shortcut row names a key that does something real.
//! A row survives if the Rune shell handles the key today:
//! the `main.rs` `on_key` bindings, `chrome_view` buttons,
//! the command palette's key handler (which `on_key`
//! consults first), or — since Phase E — the shell's
//! workspace keys. Two groups of GPUI-era rows were dropped
//! in Phase D; Phase E restores one of them:
//!
//! - `Alt+1` … `Alt+8` (workspace switching) — dropped in
//!   Phase D because the Rune shell had no workspace state;
//!   restored in Phase E, where the shell's `on_key` binds
//!   them to the workspaces in table order (Animate, Draw,
//!   Graph, Effects, Audio, 3D, Export, Home). They are
//!   shell bindings, not `EditorCore` ops.
//! - The nine text-authoring capture rows (`Enter`, `Esc`,
//!   `Backspace`, `Home`, `End`, typing, and the text-mode
//!   `Delete`/`Left`/`Right`) — still dropped: the engine's
//!   `text_edit` module survives, but the Rune shell does
//!   not wire a text-authoring capture, so in that sense
//!   those keys do nothing. `Delete` still appears once,
//!   with its real shell meaning: delete the selected
//!   segment. (The guides toggle is a button in the View
//!   menu, not a key, so it is not a row either.)
//!
//! Search is a later section; the rows are static data
//! ([`shortcut_rows`], [`section_rows`], [`group_rows`]) so a
//! search field can filter them without changing this panel.
//!
//! Phase G (section 30/40) audit, 2026-11-08: every binding
//! below was re-enumerated from source — `main.rs` `on_key`
//! read in full, `command_palette::palette_handle_key` and
//! `palette_field_key`, plus a tree-wide grep for
//! `KeyEvent` / `is_command` consumers (only those two
//! handlers exist; `menus.rs` mentions keys only in doc
//! comments for its shortcut hints, and `graph_panel`'s
//! `Key::` is graph keyframe data, not keyboard input).
//! The audit added the palette-open rows (`Down`, `Up`,
//! `Enter`) and the `Escape` row (the palette handler
//! closes the palette; `on_key` closes an open menu), and
//! recorded the `text_focused` guard in the group
//! descriptions: `on_key` consults the palette handler
//! first, then returns early while any text field holds
//! focus — so every shell binding except Ctrl+K pauses
//! while typing. Field-local Enter (rename and name fields
//! applying on submit) stays excluded, like the other
//! text-authoring keys above.
//!
//! Phase I (section 30/40) refresh, 2026-11-08: Phases G–I
//! added user-facing features that are NOT keyboard
//! shortcuts, and the audit above found no new shell key
//! bindings to list. They are documented in a sixth
//! section, `Features`, whose rows keep the (label, action)
//! shape but whose label names the feature rather than a
//! key — inventing key names for buttons would break this
//! panel's honesty rule. The seven audited items map to
//! six rows: the keyframe-jump buttons (Phase I,
//! `playback_controls` `pb-prev-key` / `pb-next-key`) and
//! the clickable timeline markers (Phase I,
//! `timeline_marks` `marker-cam-{frame}` /
//! `marker-graph-{frame}`) are one navigation story, so
//! they share a row to keep the panel at 30 rows. Loop
//! playback is named as what it is — the transport Loop
//! button (`pb-loop`) or the palette's Toggle loop
//! playback command — never as a key.
//!
//! Phase J (section 29/40) refresh, 2026-11-08: four more
//! Features rows, each verified against the tree at audit
//! time rather than taken from the phase plan. The
//! timeline ruler is real (`timeline_marks`' `ruler_label`
//! and the `timeline-ruler` strip above the chips). Split
//! FX clip is real twice over: the `fx-split` Split at
//! playhead button in `fx_clips` and the palette's
//! `split-fx-clip` command, both over
//! `EditorCore::fx_split_clip`. Range export is real:
//! `xc-svg` and `xc-sheet` are `range_row`s in
//! `export_panel` whose actions honour the loop range.
//! Mirror whole figure names both places a user can
//! reach it: the Pose tools `pose-mirror-all` button
//! (landed by section 20 after this section's first
//! verification pass — corrected at integration) and the
//! palette's `mirror-figure` command, both over
//! `EditorCore::mirror_figure_all_frames`.
//! The panel now holds 34 rows.
//!
//! Phase K (section 39/40) refresh, 2026-11-08: six more
//! Features rows, each verified against the tree the
//! same way — the named control exists and is where the
//! row says it is. Playback speed is the transport's
//! `pb-speed` gauge button over `cycle_playback_speed`
//! (`SPEEDS` 0.25×–2×, `playback_controls`). Clear keys
//! in loop range is `keyframe_tools`' `key-clear-range`
//! button over `graph_keys::delete_keys_in_range` — the
//! conditional section: its channel resolution landed,
//! and the button exists in live and inert forms.
//! Delete graph channel is the Graph panel's
//! `graph-chan-delete` button over
//! `graph_keys::delete_channel`. Frame rate & stage
//! size are the Scene panel's `scene-fps` steppers and
//! `scene-stage-size-row` width / height steppers.
//! Quick save is real: `main.rs`' `is_command('s')`
//! branch runs the app-bar Save button's own op. Delete
//! project file is the Home rows' two-step
//! `home-proj-del-{i}` → confirm, over
//! `shell::delete_project_file`. Two further
//! candidates were dropped at §39's census and restored
//! at the Phase K close-out, each verified
//! user-reachable by then: **Right-press selects** —
//! `stage_view`' `secondary_select` was registered on
//! the stage canvas (`on_secondary_press_local` in
//! `main.rs`) by the integrator after §39's census; and
//! **Reload from disk** — §27's `proj-reload` button
//! landed in `project_tools` after §39's census ran.
//! The panel now holds 42 rows.
//!
//! Phase L (section 28/40) refresh, 2026-11-08: six
//! more Features rows, verified against the tree the
//! same way. 3D preview is real: the 3D panel builds a
//! live canvas keyed `panel-3d-preview` (the
//! `PREVIEW_KEY` constant in `three_d_panel`) drawing
//! `fig3_preview::preview_prims`, with the
//! `d3-grid-toggle` button under it flipping
//! `fig3_preview`'s ground-grid flag. Head ring size
//! is the figure inspector's `fig-ring-dn` /
//! `fig-ring-up` steppers over `step_head_ring_scale`
//! (the project's `head_ring_scale`). Loop range
//! steppers are the timeline's `timeline-loop-row`:
//! `loop-set` plus the `loop-a-dn` / `loop-a-up` and
//! `loop-b-dn` / `loop-b-up` Start / End steppers and
//! `loop-clear`, all over `loop_range` /
//! `set_loop_range`. Three more rows complete the
//! six: Stroke simplify is the stroke inspector's
//! `stroke-simplify` button under the width row,
//! running Douglas-Peucker over the one selected
//! stroke through `draw_ops::simplify_stroke_at`.
//! Palette recents persist: `command_palette` loads
//! them through `shell::palette_recents` and writes
//! them back through `shell::save_palette_recents`,
//! so recent commands survive a restart. Camera key
//! delete is the camera panel's `cam-delete-key`
//! button over `delete_camera_key_at` at the current
//! frame. A census note, for the record: those last
//! three landed DURING this section's census — the
//! first greps found no `stroke-simplify`, no
//! `cam-delete-key` and no `palette_recents` call,
//! and only re-verification against the tree (the
//! Phase K lesson: a census is a claim too) found
//! all three present with their handlers. No
//! candidate was dropped this phase. The panel now
//! holds 48 rows.
//!
//! Phase M (section 36/40) refresh, 2026-11-08: six
//! more Features rows. Shift graph channel is verified
//! whole: the `graph-shift-l` / `graph-shift-r` buttons
//! in `graph_panel` over `graph_keys::shift_keys`, and
//! the row's second half names `keyframe_tools`'
//! `key-fill-range` button (`fill_range_enabled`: a
//! loop range and a key at the playhead). Swap frames
//! is verified at its buttons — `fop-swap-prev` /
//! `fop-swap-next` in `frame_ops` over
//! `frame_swap::swap_frame_with_neighbour`, whose own
//! census remaps camera, particles, strokes, audio,
//! FX, graph keys, texts and sprites with the frames.
//! Hide FX clips is verified at `fx_clips`' `fx-hide`
//! toggle: `stage_fx` emits nothing for a hidden clip
//! and `export.rs` skips it. Mute audio clips is
//! verified at the `audio-mute` button in `audio_panel`
//! over `audio_ops::audio_toggle_muted_at`, with
//! `audio.rs`' playback plan filtering muted clips out
//! of direct play and mixes alike. Stroke opacity is
//! verified in its plumbing: a stroke carries
//! `opacity`, recorded from the brush by
//! `recorded_stroke_opacity` in `editor.rs`, and the
//! stage (`stage_layers`) and the SVG export honour
//! it. 3D figure on the stage is verified in its pass:
//! `stage_fig3::stage_fig3_prims` builds the figure,
//! its ground shadow and the ground grid in stage
//! space, and `stage_view` draws them when the
//! overlays' `fig3_stage` flag is set and a session
//! exists. Five briefed pieces were NOT in the tree
//! at this census and are flagged for the integrator's
//! re-check rather than silently dropped (the Phase K
//! lesson, run the other way): the shell wiring that
//! sets `fig3_stage` in the 3D workspace (the
//! `StageOverlays` literal in `main.rs` does not name
//! it yet); the Mute command-palette command and
//! Edit-menu item; the Swap palette command; the
//! `stroke-opacity-dn` / `stroke-opacity-up` steppers
//! in the stroke panel; and the timeline's
//! `timeline-clips` lane. The panel now holds 54 rows.
//!
//! Phase N adoption, 2026-11-08: the panel adopts the
//! charter's card language without touching a single
//! row, string or key. The hand-rolled root (fill
//! `t.card`, border `control_track`, radius 8) becomes
//! `design::panel_col` (hairline `card_border`, radius
//! 12, kit gap); the header becomes a `design::
//! panel_title` cluster (the project line rides as its
//! meta, in the same LABEL / `text_secondary` it already
//! used) over a kit `divider`; section titles move from
//! TITLE to SECTION / `t.text` through the same
//! `panel_title` treatment (p11's ruling for this panel:
//! a section title heads a section inside the card, so
//! it takes the panel-header treatment, glyph kept).
//! The local `key_cap` is retired onto `design::kbd` —
//! but only for the 24 true-key rows (Shortcuts,
//! Workspaces, Posing, Timeline): the 30 Features rows
//! name features, not keys, and a chip would claim a
//! binding that does not exist (p14 §3), so their first
//! column now renders as plain LABEL text. No copy
//! changes in this pass — the stale Delete row and the
//! Ctrl+S placement are the V-wave copy pass's, per
//! p14. The row census is unchanged: 54 rows.
//!
//! Phase N (V-wave copy pass), 2026-11-08: the two copy
//! fixes p14 scheduled for this pass. The Delete row
//! now names the key's real priority cascade —
//! "Delete the selected stroke, particle event or
//! segment" — instead of the segment alone (the
//! cascade itself landed in Phase J). Quick save
//! moved from a Features row, where Ctrl+S hid in its
//! prose, to a true-key Shortcuts row,
//! `("Ctrl+S", "Quick save")`: it now wears a `kbd`
//! chip like the other 24 real bindings, and the
//! Features description ("none of these is a keyboard
//! shortcut") is exact again. The census is unchanged
//! in total — 54 rows — but the split moved with the
//! row: Shortcuts 9 → 10, Features 30 → 29, so the
//! true-key rows the adoption paragraph above counts
//! are now 25 and the Features rows 29. The command
//! palette's drift guard retired its Ctrl+S exception
//! in the same pass — the hint is a key label now.
//!
//! UI mission Wave 9 refresh, 2026-11-08: two more
//! Features rows for the mission's newest surfaces,
//! verified against the tree the usual way, plus one
//! correction of emphasis. Live MCP control is real:
//! the settings card's `settings-live-mcp` toggle
//! (`settings_popover`) starts `live_mcp`'s loopback
//! bridge (port 32117 in `src/live_mcp.rs`), whose
//! note states plainly that any program on this
//! computer can drive the editor while it is on, with
//! no password in this version. View zoom is real:
//! the stage wheel runs `stage_view::view_zoom_at`
//! against the session-only `ViewNav` (`src/stage_view.rs`;
//! the wheel handler in `main.rs` never touches the
//! camera), the stage overlay's Fit / Centre / 1:1 and
//! −/+ keys steer the same view, its readout shows the
//! effective zoom, and authoring the real camera stays
//! in the Camera panel. The Loop range steppers row
//! now says what Wave 5 of the mission made true: the
//! timeline's loop row is the range's ONLY home —
//! `playback_controls` and `frame_ops` no longer mount
//! duplicate loop controls. Workspace switching
//! (Alt+1 … Alt+8) was checked and needed no row: it
//! has been listed in the Workspaces section since
//! Phase E. The panel now holds 56 rows.
//!
//! Wave 10 refresh, 2026-11-09: one more Features
//! row, for the rebuilt dopesheet lane in the Animate
//! timeline — one row per figure, a ◆ in each cell
//! whose frame is a keyframe for that figure (the
//! engine's `stats::figure_keyframes`), click a cell
//! to go to that frame and select the figure. The
//! lane lands in this same wave; at this row's census
//! its module was not yet in the tree, so the wording
//! is held to the wave's spec and to the timeline's
//! existing, verified marker behaviour —
//! `timeline_marks` already draws ◆ for keyframes in
//! its marker row and seeks on a marker press — and
//! claims nothing beyond that. The palette's next /
//! previous keyframe commands were checked for in
//! `command_palette.rs` (112 commands censused) and
//! are not there, so no palette row was added: the
//! transport's Prev key / Next key buttons remain
//! the keyframe-jump route, as the Keyframe jump row
//! already says. The panel now holds 57 rows.

use rune::tok::{type_, Theme};
use rune::{col, row, text, El};
use stykanimate_rs::editor::EditorCore;

use crate::rune_sec::design;

/// The keyboard shortcuts that are real in this shell, as
/// (key label, action) rows, grouped by section in panel
/// order (see [`group_rows`]). Concatenating [`section_rows`]
/// over the six sections yields exactly this list. The
/// `Features` section's labels name features, not keys —
/// see the module docs.
pub fn shortcut_rows() -> Vec<(&'static str, &'static str)> {
    let mut rows = Vec::new();
    for (name, _) in group_rows() {
        rows.extend(section_rows(name));
    }
    rows
}

/// The shortcut rows belonging to one help section, by the
/// section names from [`group_rows`]. Drawing currently has
/// no live keys, so it returns an empty list — the panel
/// renders its [`section_note`] text instead.
pub fn section_rows(section: &str) -> Vec<(&'static str, &'static str)> {
    match section {
        "Shortcuts" => vec![
            ("Ctrl+Z", "Undo"),
            ("Ctrl+Shift+Z", "Redo"),
            ("Ctrl+Y", "Redo"),
            ("Ctrl+S", "Quick save"),
            ("Ctrl+K", "Open the command palette"),
            ("Down", "Next result in the command palette (palette open)"),
            ("Up", "Previous result in the command palette (palette open)"),
            ("Enter", "Run the selected command (command palette open)"),
            ("Escape", "Close the open menu or command palette"),
            ("Delete", "Delete the selected stroke, particle event or segment"),
        ],
        "Workspaces" => vec![
            ("Alt+1", "Switch to the Animate workspace"),
            ("Alt+2", "Switch to the Draw workspace"),
            ("Alt+3", "Switch to the Graph workspace"),
            ("Alt+4", "Switch to the Effects workspace"),
            ("Alt+5", "Switch to the Audio workspace"),
            ("Alt+6", "Switch to the 3D workspace"),
            ("Alt+7", "Switch to the Export workspace"),
            ("Alt+8", "Switch to the Home workspace"),
        ],
        "Posing" => vec![
            ("C", "Toggle camera mode"),
            ("O", "Toggle onion skin"),
        ],
        "Timeline" => vec![
            ("Space", "Play / pause"),
            ("Left", "Previous frame (wraps around)"),
            ("Right", "Next frame (wraps around)"),
            ("N", "Add a frame after the current one"),
            ("I", "Insert an inbetween frame"),
        ],
        "Drawing" => vec![],
        "Features" => vec![
            (
                "Loop playback",
                "Transport Loop button or the command palette",
            ),
            (
                "Playback speed",
                "Transport gauge button cycles 0.25×, 0.5×, 1×, 1.5×, 2×",
            ),
            (
                "Keyframe jump",
                "Prev key / Next key transport buttons — or click a timeline marker (◆ camera, ● graph)",
            ),
            (
                "Timeline ruler",
                "Second labels above the frame chips",
            ),
            (
                "Dopesheet",
                "Timeline — one row per figure; ◆ marks that figure's keyframes; click a cell to go to that frame and select the figure",
            ),
            (
                "Clear keys in loop range",
                "Keyframes — eraser button clears the selected joint's keys inside the loop range",
            ),
            (
                "Shift graph channel",
                "Graph panel — Shift −1 / +1 buttons move every key in the channel; Keyframes Fill range copies the current key across the loop range",
            ),
            (
                "Delete graph channel",
                "Graph panel — Delete channel button removes the drawn channel",
            ),
            (
                "Loop range steppers",
                "Timeline — the loop range's only home: Set loop button, Start / End steppers and Clear below the frame chips",
            ),
            (
                "Camera key delete",
                "Camera panel — Delete key button removes the camera key at the current frame",
            ),
            (
                "Swap frames",
                "Frame ops — Swap with previous / next buttons or the command palette; camera, particle and graph keys follow the swap",
            ),
            (
                "View zoom",
                "Stage — the mouse wheel zooms the view, not the camera; the stage overlay's Fit / Centre / 1:1 and −/+ steer the same view, and the Camera panel authors the real camera",
            ),
            (
                "Stroke selection",
                "Click a stroke on the stage to select it",
            ),
            (
                "Right-press selects",
                "Stage — right-press a figure to select it and its joint",
            ),
            (
                "Stroke simplify",
                "Stroke inspector — Simplify button reduces the selected stroke's points",
            ),
            (
                "Stroke opacity",
                "Draw workspace — stroke panel opacity steppers; brushes record opacity on strokes, and the stage and exports honour it",
            ),
            (
                "Particles panel",
                "Effects workspace — add, select and delete particle events",
            ),
            (
                "Mute audio clips",
                "Audio panel — Mute button, command palette or Edit menu; muted clips are excluded from playback and exports",
            ),
            (
                "Audio clip selection",
                "Click a clip row — the effects then act on that clip",
            ),
            (
                "3D preview",
                "3D workspace — live preview of the 3D figure; drag to orbit, Grid toggle shows the ground grid",
            ),
            (
                "3D figure on the stage",
                "3D workspace — follows the open 3D session: the stage itself draws the 3D figure and its ground grid, like the panel preview",
            ),
            (
                "Bend steppers",
                "Segment inspector buttons bend the selected segment by −5°, −1°, +1° or +5°",
            ),
            (
                "Split FX clip",
                "Effects workspace — Split at playhead button or the command palette",
            ),
            (
                "Hide FX clips",
                "Effects workspace — FX panel Hide button keeps a clip in the project but out of the stage and exports; the timeline clips lane dims muted and hidden spans",
            ),
            (
                "Mirror whole figure",
                "Pose tools or command palette — Mirror figure (all frames)",
            ),
            (
                "Head ring size",
                "Figure inspector — −/+ steppers scale the project's head ring",
            ),
            (
                "Frame rate & stage size",
                "Scene panel — Frame rate stepper and Stage size width / height steppers",
            ),
            (
                "Delete project file",
                "Home workspace — Delete on a project row, then confirm",
            ),
            (
                "Reload from disk",
                "Project tools — Reload from disk button re-reads the saved file",
            ),
            (
                "Palette recents",
                "Command palette — recent commands are remembered between sessions",
            ),
            (
                "Range export",
                "Export panel — SVG and Spritesheet now take the loop range too",
            ),
            (
                "Live MCP control",
                "Settings — the Live MCP control toggle lets other programs on this computer drive the editor at http://127.0.0.1:32117/mcp; no password in this version",
            ),
        ],
        _ => vec![],
    }
}

/// The explanatory note rendered in place of rows for a
/// section that has no live shortcuts yet. `None` for
/// sections that have rows.
pub fn section_note(section: &str) -> Option<&'static str> {
    match section {
        "Drawing" => Some(
            "No keyboard shortcuts yet. Draw mode exists in the core \
             and stage drags paint while it is on, but no key toggles it.",
        ),
        _ => None,
    }
}

/// The help panel's sections, as (name, one-line description)
/// rows, in display order.
pub fn group_rows() -> Vec<(&'static str, &'static str)> {
    vec![
        (
            "Shortcuts",
            "General editing keys — all but Ctrl+K pause while typing in a text field",
        ),
        (
            "Workspaces",
            "Workspace switching keys — paused while typing in a text field",
        ),
        (
            "Posing",
            "Keys that change how stage drags behave and what the stage shows — paused while typing in a text field",
        ),
        (
            "Timeline",
            "Playback and frame keys — paused while typing in a text field",
        ),
        ("Drawing", "Freehand drawing keys"),
        (
            "Features",
            "Features on buttons, panels and the stage — none of these is a keyboard shortcut",
        ),
    ]
}

/// One shortcut row: a kit `kbd` chip, then the action it
/// performs. Only for rows whose first column names a real
/// key (the Shortcuts, Workspaces, Posing and Timeline
/// sections) — Features rows render through
/// [`feature_row`] instead.
fn shortcut_row(key: &str, action: &str, t: &Theme) -> El {
    row()
        .gap(8.0)
        .center_y()
        .child(design::kbd(key, t))
        .child(text(action.to_string()).font(type_::LABEL).color(t.text))
}

/// One Features row: the feature name as plain text, then
/// how the feature is reached. The first column names a
/// feature, not a key, so it must not wear a `kbd` chip —
/// a chip would claim a keyboard binding that does not
/// exist (p14 §3).
fn feature_row(label: &str, action: &str, t: &Theme) -> El {
    row()
        .gap(8.0)
        .center_y()
        .child(text(label.to_string()).font(type_::LABEL).color(t.text))
        .child(text(action.to_string()).font(type_::LABEL).color(t.text))
}

/// One help section: a title, its description, then either
/// its rows or its explanatory note. The title carries a
/// small glyph (Phase H): `key` for the Shortcuts section,
/// `list` for the rest — decoration only; the section
/// names and descriptions are untouched. Phase N renders
/// the title through `design::panel_title` (SECTION /
/// `t.text`, the kit's header treatment), and the rows
/// split by kind: true-key sections get `kbd` chips,
/// Features rows stay plain text (see [`feature_row`]).
fn help_section(name: &str, description: &str, t: &Theme) -> El {
    let header_icon = if name == "Shortcuts" { "key" } else { "list" };
    let mut section = col()
        .key(format!("help-section-{}", name.to_lowercase()))
        .gap(4.0)
        .child(
            row()
                .w_full()
                .gap(8.0)
                .center_y()
                .child(design::panel_title(Some(header_icon), name, None, t)),
        )
        .child(
            text(description.to_string())
                .font(type_::LABEL)
                .color(t.text_secondary),
        );
    let rows = section_rows(name);
    if rows.is_empty() {
        if let Some(note) = section_note(name) {
            section = section.child(
                text(note.to_string())
                    .font(type_::LABEL)
                    .color(t.text_secondary),
            );
        }
    } else if name == "Features" {
        for (label, action) in rows {
            section = section.child(feature_row(label, action, t));
        }
    } else {
        for (key, action) in rows {
            section = section.child(shortcut_row(key, action, t));
        }
    }
    section
}

/// The help panel: a header (title + the open project's
/// name, frame count and fps, read from the core) over the
/// six sections from [`group_rows`]. Pure: it builds
/// elements from `core` and never calls an op on it.
pub fn help_panel(core: &EditorCore, t: &Theme) -> El {
    let fps = core.project.fps;
    let fps_txt = if fps.fract() == 0.0 {
        format!("{}", fps as i64)
    } else {
        format!("{fps:.1}")
    };
    let meta = format!(
        "{} · {} frames · {} fps",
        core.project.name,
        core.frame_count(),
        fps_txt
    );
    let mut panel = design::panel_col("help-panel", t)
        .child(
            row()
                .w_full()
                .gap(8.0)
                .center_y()
                .child(design::panel_title(
                    Some("circle-question"),
                    "Help",
                    Some(&meta),
                    t,
                )),
        )
        .child(design::divider(t));
    for (name, description) in group_rows() {
        panel = panel.child(help_section(name, description, t));
    }
    panel
}

#[cfg(test)]
mod tests {
    use super::{group_rows, section_note, section_rows, shortcut_rows};

    #[test]
    fn shortcut_rows_has_exactly_57_rows() {
        // 10 Shortcuts + 8 Workspaces + 2 Posing + 5 Timeline
        // + 32 Features; Drawing contributes none (see the
        // module docs for which GPUI-era rows were dropped
        // and why). Phase G added the four palette/menu rows
        // to Shortcuts; Phase I added the Features section;
        // Phase J added four rows to it; Phase K six more,
        // and the Phase K close-out two more (Right-press
        // selects and Reload from disk — both verified
        // user-reachable at integration time); Phase L
        // six more; Phase M six more. Phase N (V wave)
        // moved Quick save from Features to Shortcuts as
        // a Ctrl+S row — one row each way, total stands.
        // The UI mission Wave 9 refresh added two Features
        // rows (View zoom, Live MCP control). The Wave 10
        // refresh added one more (Dopesheet).
        assert_eq!(shortcut_rows().len(), 57);
    }

    #[test]
    fn no_row_text_duplicates_another() {
        // No two rows may carry the same (label, action)
        // text — a duplicated row tells the user the same
        // thing twice and crowds out a real feature.
        let rows = shortcut_rows();
        for (i, row) in rows.iter().enumerate() {
            assert!(
                !rows[..i].contains(row),
                "duplicate row text: {row:?}"
            );
        }
    }

    #[test]
    fn phase_i_feature_rows_are_listed() {
        // The Phases G–I features the Phase I audit found
        // missing, each described by how it is really
        // reached (button, palette, panel or stage click) —
        // none of them is a keyboard shortcut, and no row
        // may pretend otherwise.
        let rows = shortcut_rows();
        assert!(rows.contains(&(
            "Loop playback",
            "Transport Loop button or the command palette"
        )));
        assert!(rows.contains(&(
            "Keyframe jump",
            "Prev key / Next key transport buttons — or click a timeline marker (◆ camera, ● graph)"
        )));
        assert!(rows.contains(&(
            "Stroke selection",
            "Click a stroke on the stage to select it"
        )));
        assert!(rows.contains(&(
            "Particles panel",
            "Effects workspace — add, select and delete particle events"
        )));
        assert!(rows.contains(&(
            "Audio clip selection",
            "Click a clip row — the effects then act on that clip"
        )));
        assert!(rows.contains(&(
            "Bend steppers",
            "Segment inspector buttons bend the selected segment by −5°, −1°, +1° or +5°"
        )));
        // The Features rows live in their own section, in
        // this order, and nowhere else. (The count covers
        // the Phase J, Phase K, Phase L and Phase M rows
        // too — see those tests.)
        assert_eq!(section_rows("Features").len(), 32);
    }

    #[test]
    fn phase_j_feature_rows_are_listed() {
        // The Phase J features, each verified against the
        // tree before listing (see the module docs): the
        // timeline ruler (`timeline_marks`), Split at
        // playhead (`fx_clips` button + `split-fx-clip`
        // palette command), whole-figure mirroring and the
        // SVG / Spritesheet range rows (`export_panel`).
        // Mirror whole figure names both homes: the Pose
        // tools button (`pose-mirror-all`, section 20) and
        // the palette command — both verified in the tree
        // at integration.
        let rows = shortcut_rows();
        assert!(rows.contains(&(
            "Timeline ruler",
            "Second labels above the frame chips"
        )));
        assert!(rows.contains(&(
            "Split FX clip",
            "Effects workspace — Split at playhead button or the command palette"
        )));
        assert!(rows.contains(&(
            "Mirror whole figure",
            "Pose tools or command palette — Mirror figure (all frames)"
        )));
        assert!(rows.contains(&(
            "Range export",
            "Export panel — SVG and Spritesheet now take the loop range too"
        )));
        // The ruler sits with the timeline rows, directly
        // after Keyframe jump; the section held thirty
        // feature rows at Phase M (ten at Phase J, six
        // more in Phase K, two at the Phase K close-out,
        // six in Phase L, six in Phase M). The Phase N
        // V-wave pass moved Quick save into Shortcuts, so
        // twenty-nine remained after that move; the UI
        // mission Wave 9 refresh added View zoom and Live
        // MCP control, so thirty-one stood then; the
        // Wave 10 refresh added the Dopesheet row, so
        // thirty-two stand now.
        let features = section_rows("Features");
        assert_eq!(features.len(), 32);
        let jump = features
            .iter()
            .position(|(label, _)| *label == "Keyframe jump")
            .expect("Keyframe jump row");
        assert_eq!(features[jump + 1].0, "Timeline ruler");
    }

    #[test]
    fn phase_k_feature_rows_are_listed() {
        // The Phase K features, each verified against the
        // tree before listing (see the module docs):
        // Playback speed — the `pb-speed` gauge button and
        // `cycle_playback_speed` in `playback_controls`
        // (SPEEDS 0.25×–2×); Clear keys in loop range —
        // the `key-clear-range` button in
        // `keyframe_tools`, live and inert forms, over
        // `graph_keys::delete_keys_in_range`; Delete graph
        // channel — the `graph-chan-delete` button in
        // `graph_panel` over `graph_keys::delete_channel`;
        // Frame rate & stage size — the `scene-fps`
        // steppers and the `scene-stage-size-row` Width /
        // Height steppers in `scene_panel`; Quick save —
        // the `is_command('s')` branch in `main.rs` running
        // the app-bar Save op; Delete project file — the
        // two-step `home-proj-del-{i}` → confirm rows in
        // `home_panel` over `shell::delete_project_file`.
        // Right-press selects and Reload from disk were
        // added at the Phase K close-out: §39's census ran
        // before §27's `proj-reload` landed and before the
        // integrator registered the stage's
        // `on_secondary_press_local` handler, so both were
        // dropped mid-phase as not user-reachable; both
        // verifiably are now (see the module docs).
        let rows = shortcut_rows();
        assert!(rows.contains(&(
            "Playback speed",
            "Transport gauge button cycles 0.25×, 0.5×, 1×, 1.5×, 2×"
        )));
        assert!(rows.contains(&(
            "Clear keys in loop range",
            "Keyframes — eraser button clears the selected joint's keys inside the loop range"
        )));
        assert!(rows.contains(&(
            "Delete graph channel",
            "Graph panel — Delete channel button removes the drawn channel"
        )));
        assert!(rows.contains(&(
            "Frame rate & stage size",
            "Scene panel — Frame rate stepper and Stage size width / height steppers"
        )));
        // Quick save was listed as that Features pair in
        // Phase K; the Phase N V-wave pass moved it to
        // the Shortcuts section as the true-key row it
        // always was (the binding itself is unchanged).
        assert!(rows.contains(&("Ctrl+S", "Quick save")));
        assert!(rows.contains(&(
            "Delete project file",
            "Home workspace — Delete on a project row, then confirm"
        )));
        assert!(rows.contains(&(
            "Right-press selects",
            "Stage — right-press a figure to select it and its joint"
        )));
        assert!(rows.contains(&(
            "Reload from disk",
            "Project tools — Reload from disk button re-reads the saved file"
        )));
        // Placement: Playback speed sits with the
        // transport rows directly after Loop playback.
        let features = section_rows("Features");
        assert_eq!(features.len(), 32);
        let loop_row = features
            .iter()
            .position(|(label, _)| *label == "Loop playback")
            .expect("Loop playback row");
        assert_eq!(features[loop_row + 1].0, "Playback speed");
    }

    #[test]
    fn phase_l_feature_rows_are_listed() {
        // The Phase L features, each verified against the
        // tree before listing (see the module docs): 3D
        // preview — the `panel-3d-preview` canvas
        // (`PREVIEW_KEY`) in `three_d_panel` drawing
        // `fig3_preview::preview_prims`, with the
        // `d3-grid-toggle` button flipping the ground
        // grid; Head ring size — the `fig-ring-dn` /
        // `fig-ring-up` steppers in `inspector_figure`
        // over `step_head_ring_scale`; Loop range
        // steppers — the `timeline-loop-row` in
        // `timeline_marks` (`loop-set`, the Start / End
        // `loop-a-*` / `loop-b-*` steppers, `loop-clear`).
        // Stroke simplify — the `stroke-simplify` button
        // in `stroke_inspector`, over
        // `draw_ops::simplify_stroke_at`; Palette recents
        // — `command_palette` loading through
        // `shell::palette_recents` and saving through
        // `shell::save_palette_recents`; Camera key
        // delete — the `cam-delete-key` button in
        // `camera_panel`, over `delete_camera_key_at`.
        // Those last three landed during this section's
        // census — the first greps found none of them,
        // and re-verification against the tree found all
        // three with their handlers (see the module
        // docs). No candidate was dropped this phase.
        let rows = shortcut_rows();
        assert!(rows.contains(&(
            "3D preview",
            "3D workspace — live preview of the 3D figure; drag to orbit, Grid toggle shows the ground grid"
        )));
        assert!(rows.contains(&(
            "Head ring size",
            "Figure inspector — −/+ steppers scale the project's head ring"
        )));
        assert!(rows.contains(&(
            "Loop range steppers",
            "Timeline — the loop range's only home: Set loop button, Start / End steppers and Clear below the frame chips"
        )));
        assert!(rows.contains(&(
            "Stroke simplify",
            "Stroke inspector — Simplify button reduces the selected stroke's points"
        )));
        assert!(rows.contains(&(
            "Palette recents",
            "Command palette — recent commands are remembered between sessions"
        )));
        assert!(rows.contains(&(
            "Camera key delete",
            "Camera panel — Delete key button removes the camera key at the current frame"
        )));
        // Placement: the loop steppers sit with the
        // timeline / graph rows, directly after Delete
        // graph channel, with Camera key delete directly
        // after them; Stroke simplify sits with the
        // stroke rows, directly after Right-press
        // selects; the 3D preview sits with the
        // workspace panel rows, directly after Audio clip
        // selection; the head ring sits with the figure
        // rows, directly after Mirror whole figure;
        // Palette recents sits with the project rows,
        // directly after Reload from disk.
        let features = section_rows("Features");
        assert_eq!(features.len(), 32);
        let channel_row = features
            .iter()
            .position(|(label, _)| *label == "Delete graph channel")
            .expect("Delete graph channel row");
        assert_eq!(features[channel_row + 1].0, "Loop range steppers");
        assert_eq!(features[channel_row + 2].0, "Camera key delete");
        let press_row = features
            .iter()
            .position(|(label, _)| *label == "Right-press selects")
            .expect("Right-press selects row");
        assert_eq!(features[press_row + 1].0, "Stroke simplify");
        let audio_row = features
            .iter()
            .position(|(label, _)| *label == "Audio clip selection")
            .expect("Audio clip selection row");
        assert_eq!(features[audio_row + 1].0, "3D preview");
        let mirror_row = features
            .iter()
            .position(|(label, _)| *label == "Mirror whole figure")
            .expect("Mirror whole figure row");
        assert_eq!(features[mirror_row + 1].0, "Head ring size");
        let reload_row = features
            .iter()
            .position(|(label, _)| *label == "Reload from disk")
            .expect("Reload from disk row");
        assert_eq!(features[reload_row + 1].0, "Palette recents");
    }

    #[test]
    fn phase_m_feature_rows_are_listed() {
        // The Phase M features, censused against the tree
        // before listing (see the module docs): Shift
        // graph channel — the `graph-shift-l` /
        // `graph-shift-r` buttons in `graph_panel` over
        // `graph_keys::shift_keys`, and `key-fill-range`
        // in `keyframe_tools` (`fill_range_enabled`);
        // Swap frames — the `fop-swap-prev` /
        // `fop-swap-next` buttons in `frame_ops` over
        // `frame_swap::swap_frame_with_neighbour` (its
        // remap census covers camera, particles and
        // graph keys); Hide FX clips — the `fx-hide`
        // toggle in `fx_clips`, with `stage_fx` and
        // `export.rs` both honouring the hidden flag;
        // Mute audio clips — the `audio-mute` button in
        // `audio_panel` over
        // `audio_ops::audio_toggle_muted_at`, with
        // `audio.rs`' plan filtering muted clips out;
        // Stroke opacity — `Stroke.opacity` recorded
        // from the brush (`recorded_stroke_opacity` in
        // `editor.rs`), honoured by `stage_layers` and
        // the SVG export; 3D figure on the stage — the
        // `stage_fig3::stage_fig3_prims` pass drawn by
        // `stage_view` under its `fig3_stage` overlay
        // flag while a session exists. Five briefed
        // pieces were not yet in the tree at census
        // (the `fig3_stage` shell wiring, Mute's palette
        // and Edit-menu homes, Swap's palette home, the
        // `stroke-opacity-dn` / `stroke-opacity-up`
        // steppers, the `timeline-clips` lane) and are
        // flagged in the module docs for the
        // integrator's re-check.
        let rows = shortcut_rows();
        assert!(rows.contains(&(
            "3D figure on the stage",
            "3D workspace — follows the open 3D session: the stage itself draws the 3D figure and its ground grid, like the panel preview"
        )));
        assert!(rows.contains(&(
            "Mute audio clips",
            "Audio panel — Mute button, command palette or Edit menu; muted clips are excluded from playback and exports"
        )));
        assert!(rows.contains(&(
            "Stroke opacity",
            "Draw workspace — stroke panel opacity steppers; brushes record opacity on strokes, and the stage and exports honour it"
        )));
        assert!(rows.contains(&(
            "Swap frames",
            "Frame ops — Swap with previous / next buttons or the command palette; camera, particle and graph keys follow the swap"
        )));
        assert!(rows.contains(&(
            "Shift graph channel",
            "Graph panel — Shift −1 / +1 buttons move every key in the channel; Keyframes Fill range copies the current key across the loop range"
        )));
        assert!(rows.contains(&(
            "Hide FX clips",
            "Effects workspace — FX panel Hide button keeps a clip in the project but out of the stage and exports; the timeline clips lane dims muted and hidden spans"
        )));
        // Placement: Shift graph channel sits with the
        // keyframe / graph rows, directly after Clear
        // keys in loop range; Swap frames sits with the
        // frame rows, directly after Camera key delete;
        // Stroke opacity sits with the stroke rows,
        // directly after Stroke simplify; Mute audio
        // clips sits with the audio rows, directly after
        // Particles panel; 3D figure on the stage sits
        // with the 3D rows, directly after 3D preview;
        // Hide FX clips sits with the FX rows, directly
        // after Split FX clip. No earlier phase's pinned
        // adjacency is disturbed by any of these.
        let features = section_rows("Features");
        assert_eq!(features.len(), 32);
        let clear_row = features
            .iter()
            .position(|(label, _)| *label == "Clear keys in loop range")
            .expect("Clear keys in loop range row");
        assert_eq!(features[clear_row + 1].0, "Shift graph channel");
        let camera_row = features
            .iter()
            .position(|(label, _)| *label == "Camera key delete")
            .expect("Camera key delete row");
        assert_eq!(features[camera_row + 1].0, "Swap frames");
        let simplify_row = features
            .iter()
            .position(|(label, _)| *label == "Stroke simplify")
            .expect("Stroke simplify row");
        assert_eq!(features[simplify_row + 1].0, "Stroke opacity");
        let particles_row = features
            .iter()
            .position(|(label, _)| *label == "Particles panel")
            .expect("Particles panel row");
        assert_eq!(features[particles_row + 1].0, "Mute audio clips");
        let preview_row = features
            .iter()
            .position(|(label, _)| *label == "3D preview")
            .expect("3D preview row");
        assert_eq!(features[preview_row + 1].0, "3D figure on the stage");
        let split_row = features
            .iter()
            .position(|(label, _)| *label == "Split FX clip")
            .expect("Split FX clip row");
        assert_eq!(features[split_row + 1].0, "Hide FX clips");
    }

    #[test]
    fn wave_9_feature_rows_are_listed() {
        // The UI mission Wave 9 refresh, verified against
        // the tree before listing (see the module docs):
        // Live MCP control — the `settings-live-mcp`
        // toggle in `settings_popover` over `src/live_mcp.rs`'
        // loopback bridge (port 32117, endpoint
        // `http://127.0.0.1:32117/mcp`, its note carrying
        // the same no-password wording the row repeats);
        // View zoom — `stage_view::view_zoom_at` over the
        // session-only `ViewNav` in `src/stage_view.rs`,
        // driven by the wheel handler in `main.rs`, with
        // the stage overlay's Fit / Centre / 1:1 and −/+
        // keys in `camera_panel` steering the same view
        // and the Camera panel authoring the real camera.
        // The Loop range steppers row (pinned in the
        // Phase L test) now names the timeline's loop row
        // as the range's only home: `playback_controls`
        // and `frame_ops` mount no loop-range controls
        // since the mission's Wave 5 de-duplication.
        // Workspace switching needed no new row — Alt+1 …
        // Alt+8 have been listed since Phase E (see the
        // workspace test below).
        let rows = shortcut_rows();
        assert!(rows.contains(&(
            "View zoom",
            "Stage — the mouse wheel zooms the view, not the camera; the stage overlay's Fit / Centre / 1:1 and −/+ steer the same view, and the Camera panel authors the real camera"
        )));
        assert!(rows.contains(&(
            "Live MCP control",
            "Settings — the Live MCP control toggle lets other programs on this computer drive the editor at http://127.0.0.1:32117/mcp; no password in this version"
        )));
        // Placement: View zoom sits with the stage /
        // camera rows, directly after Swap frames; Live
        // MCP control closes the Features section, after
        // Range export. No earlier phase's pinned
        // adjacency is disturbed by either.
        let features = section_rows("Features");
        assert_eq!(features.len(), 32);
        let swap_row = features
            .iter()
            .position(|(label, _)| *label == "Swap frames")
            .expect("Swap frames row");
        assert_eq!(features[swap_row + 1].0, "View zoom");
        assert_eq!(features.last().map(|(label, _)| *label), Some("Live MCP control"));
    }

    #[test]
    fn wave_10_feature_rows_are_listed() {
        // The Wave 10 refresh (see the module docs): the
        // rebuilt dopesheet lane in the Animate timeline.
        // The row is phrased to the wave's spec and to
        // the timeline's existing marker behaviour —
        // `timeline_marks` draws ◆ for keyframes in its
        // marker row and seeks on a marker press — and
        // the per-figure keyframe sets behind the lane
        // are the engine's `stats::figure_keyframes`.
        // The palette's next / previous keyframe
        // commands were censused in `command_palette.rs`
        // and are not there, so — unlike the rows above
        // that name a palette home where one exists —
        // this row names none.
        let rows = shortcut_rows();
        assert!(rows.contains(&(
            "Dopesheet",
            "Timeline — one row per figure; ◆ marks that figure's keyframes; click a cell to go to that frame and select the figure"
        )));
        // Placement: the dopesheet sits with the
        // timeline rows, directly after the Timeline
        // ruler. No earlier phase's pinned adjacency is
        // disturbed by the insertion.
        let features = section_rows("Features");
        assert_eq!(features.len(), 32);
        let ruler_row = features
            .iter()
            .position(|(label, _)| *label == "Timeline ruler")
            .expect("Timeline ruler row");
        assert_eq!(features[ruler_row + 1].0, "Dopesheet");
    }

    #[test]
    fn shortcut_rows_have_no_duplicate_keys() {
        let rows = shortcut_rows();
        for (i, (key, _)) in rows.iter().enumerate() {
            assert!(
                !rows[..i].iter().any(|(k, _)| k == key),
                "duplicate shortcut key: {key}"
            );
        }
    }

    #[test]
    fn shortcut_rows_spot_checks() {
        let rows = shortcut_rows();
        // First row is the undo binding; the shell-handled and
        // surviving GPUI-era bindings are all present.
        assert_eq!(rows[0], ("Ctrl+Z", "Undo"));
        assert!(rows.contains(&("Ctrl+Shift+Z", "Redo")));
        assert!(rows.contains(&("Ctrl+Y", "Redo")));
        assert!(rows.contains(&("Ctrl+K", "Open the command palette")));
        assert!(rows.contains(&("Ctrl+S", "Quick save")));
        assert!(rows.contains(&("Alt+1", "Switch to the Animate workspace")));
        assert!(rows.contains(&("Alt+8", "Switch to the Home workspace")));
        assert!(rows.contains(&("Space", "Play / pause")));
        assert!(rows.contains(&("Left", "Previous frame (wraps around)")));
        assert!(rows.contains(&("Right", "Next frame (wraps around)")));
        assert!(rows.contains(&("Delete", "Delete the selected stroke, particle event or segment")));
        assert!(rows.contains(&("N", "Add a frame after the current one")));
        assert!(rows.contains(&("I", "Insert an inbetween frame")));
        assert!(rows.contains(&("C", "Toggle camera mode")));
        assert!(rows.contains(&("O", "Toggle onion skin")));
    }

    #[test]
    fn workspace_rows_cover_all_eight_workspaces_in_table_order() {
        // Phase E wires Alt+1..Alt+8 in the shell's on_key,
        // in the workspaces table order; the section lists
        // exactly those eight rows, in that order.
        let expected: Vec<(&str, &str)> = vec![
            ("Alt+1", "Switch to the Animate workspace"),
            ("Alt+2", "Switch to the Draw workspace"),
            ("Alt+3", "Switch to the Graph workspace"),
            ("Alt+4", "Switch to the Effects workspace"),
            ("Alt+5", "Switch to the Audio workspace"),
            ("Alt+6", "Switch to the 3D workspace"),
            ("Alt+7", "Switch to the Export workspace"),
            ("Alt+8", "Switch to the Home workspace"),
        ];
        assert_eq!(section_rows("Workspaces"), expected);
    }

    #[test]
    fn phase_g_audit_rows_are_listed() {
        // The bindings the Phase G audit found in
        // `command_palette::palette_handle_key` (consulted
        // by `on_key` before anything else) and in
        // `on_key`'s menu branch — all real, none listed
        // before Phase G.
        let rows = shortcut_rows();
        assert!(rows.contains(&(
            "Down",
            "Next result in the command palette (palette open)"
        )));
        assert!(rows.contains(&(
            "Up",
            "Previous result in the command palette (palette open)"
        )));
        assert!(rows.contains(&(
            "Enter",
            "Run the selected command (command palette open)"
        )));
        assert!(rows.contains(&("Escape", "Close the open menu or command palette")));
    }

    #[test]
    fn group_descriptions_state_the_typing_guard() {
        // `on_key` returns early while a text field holds
        // focus, after the palette handler has had first
        // claim — so every group whose keys are shell
        // bindings must say they pause while typing, and
        // Shortcuts must name the one exception the routing
        // order creates (Ctrl+K reaches the palette handler
        // before the guard). Drawing and Features have no
        // keys to guard — Features is buttons and panels,
        // and its description says so instead.
        for (name, description) in group_rows() {
            if name == "Drawing" || name == "Features" {
                continue;
            }
            assert!(
                description.contains("typing"),
                "{name} description must state the typing guard: {description}"
            );
        }
        let shortcuts = group_rows()[0].1;
        assert!(
            shortcuts.contains("Ctrl+K"),
            "Shortcuts must name the Ctrl+K exception: {shortcuts}"
        );
    }

    #[test]
    fn dropped_gpui_era_rows_stay_dropped() {
        let rows = shortcut_rows();
        // The text-authoring capture is not wired in the
        // Rune shell, so none of its rows may appear.
        // (Workspace keys, dropped in Phase D, are live
        // again since Phase E — see the workspace test.)
        assert!(
            !rows.iter().any(|(_, a)| a.starts_with("Text editing:")),
            "text-editing rows must not be listed as live shortcuts"
        );
    }

    #[test]
    fn section_rows_partition_shortcut_rows() {
        let mut joined = Vec::new();
        for (name, _) in group_rows() {
            joined.extend(section_rows(name));
        }
        assert_eq!(joined, shortcut_rows());
    }

    #[test]
    fn empty_sections_carry_notes_and_filled_sections_do_not() {
        assert!(section_rows("Drawing").is_empty());
        assert!(section_note("Drawing").is_some());
        for name in ["Shortcuts", "Workspaces", "Posing", "Timeline", "Features"] {
            assert!(!section_rows(name).is_empty(), "{name} should have rows");
            assert!(section_note(name).is_none(), "{name} should have no note");
        }
    }

    #[test]
    fn group_rows_covers_all_six_sections_in_order() {
        let rows = group_rows();
        assert_eq!(rows.len(), 6);
        let names: Vec<&str> = rows.iter().map(|(name, _)| *name).collect();
        assert_eq!(
            names,
            [
                "Shortcuts",
                "Workspaces",
                "Posing",
                "Timeline",
                "Drawing",
                "Features"
            ]
        );
    }

    #[test]
    fn group_names_are_unique() {
        let rows = group_rows();
        for (i, (name, _)) in rows.iter().enumerate() {
            assert!(
                !rows[..i].iter().any(|(n, _)| n == name),
                "duplicate section name: {name}"
            );
        }
    }

    #[test]
    fn no_empty_strings_anywhere() {
        for (key, action) in shortcut_rows() {
            assert!(!key.trim().is_empty(), "empty shortcut key");
            assert!(!action.trim().is_empty(), "empty action for {key}");
        }
        for (name, description) in group_rows() {
            assert!(!name.trim().is_empty(), "empty section name");
            assert!(
                !description.trim().is_empty(),
                "empty description for {name}"
            );
        }
    }
}
