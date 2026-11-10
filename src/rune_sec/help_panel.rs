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
//!
//! Wave 11 refresh, 2026-11-09: MCP end-to-end. The
//! wave brief claimed Help did not explain MCP control
//! at all — the census corrects that: the Live MCP
//! control row has documented the live endpoint since
//! the UI mission Wave 9 refresh above. What was
//! actually missing is the OTHER transport and the
//! discovery framing, and this refresh adds exactly
//! those, verified against source. The new MCP stdio
//! server row names the standalone `mcp_server`
//! program (`src/bin/mcp_server.rs` — Cargo.toml has
//! no `[[bin]]` rename, so the file stem is the binary
//! name): JSON-RPC over stdio, one message per line,
//! the same tool dispatch (`McpServer`), and the
//! distinction a user must not miss — it works on its
//! own private `EditorCore` and never sees or drives
//! the open editor window (the framing is
//! `src/live_mcp.rs`' own module docs). The Live MCP
//! control row is improved in place rather than
//! duplicated: it now states that live control is off
//! by default and must be enabled explicitly
//! (`LayoutPrefs.live_mcp` defaults to `false` in
//! `src/shell.rs` — the W4 explicit-enable rule — and
//! the settings census records
//! the bridge starting at launch when the preference
//! is on), and it carries the security truth in the
//! bridge's own terms — no password, any program on
//! this computer can drive the editor while it is on,
//! turn it off in Settings when you do not want that
//! (`src/live_mcp.rs`' "Security — read this before
//! enabling" docs; the health identity states
//! `"auth": "none"`). No shortcut row was added for
//! the toggle: no key binding for it exists, and the
//! Features label discipline forbids inventing one —
//! the Settings toggle (`settings-live-mcp` in
//! `settings_popover`) is the only path, and the row
//! says Settings. The panel now holds 58 rows.
//!
//! Wave 12 refresh, 2026-11-09: three more Features
//! rows, for the wave's asset surface. The census
//! first checked this panel for a row describing the
//! old attach flow and found none to correct: no row
//! here has ever mentioned attaching — the only
//! sprite text in the panel is the Spritesheet in the
//! Range export row (an export, not an attachment)
//! and the frame-swap census in the Phase M paragraph
//! above. The old flow itself is `EditorCore::
//! attach_image_to_node`: one step that takes the
//! first PNG in the import folder, appends it as a new
//! asset and attaches it to the selected segment.
//! Wave 12 separates managing from attaching: the MCP
//! asset tools (`assets_list`, `asset_info`,
//! `asset_import`, `asset_rename`, `asset_remove`,
//! `asset_attach`, written by the wave's media.rs
//! squad) over the project's asset table
//! (`Project.assets` — named PNGs, `model.rs`), with
//! panel parity in the segment inspector (attach and
//! clear). Integration correction, recorded honestly
//! at the Wave 12 close: the wave's audit (B5) proved
//! the Scene panel has no asset list and no sprite
//! surface — it carries a read-only Placed media
//! counts readout, and sprite instances arrived only
//! via .nodes import and could not be placed or
//! edited in the app — so the Project assets and
//! Sprites rows stated exactly that rather than the
//! charter's assumed panels. That wording was true
//! for Wave 12. **Wave 13 changed it:** the sprite
//! surface the audit specced has landed — a Sprites
//! panel (add an instance from the assets, select it,
//! step its position / rotation / scale keys at the
//! current frame), sprite engine ops behind it, and
//! the MCP sprite tools (`sprite_list`, `sprite_add`,
//! `sprite_remove`, `sprite_set_key`,
//! `sprite_delete_key`) — so the Sprites row below
//! now states the landed truth. The Scene panel still
//! has no asset list (that half of the Wave 12
//! correction stands); its readout now points at the
//! Sprites panel. The Project assets row states the
//! removal refusal in plain words: an asset a
//! segment's media or a sprite instance still
//! references cannot be removed — detach it first.
//! The panel now holds 61 rows.
//!
//! Wave V (V5) i18n, 2026-11-10: the panel's display prose
//! resolves through the i18n catalogue at RENDER time, while
//! the canonical data above ([`group_rows`], [`section_rows`],
//! [`section_note`], [`shortcut_rows`]) stays English
//! `&'static str` pairs byte-identical — the command
//! palette's drift guard and this file's row census pin
//! those pairs, so they remain the canonical source the
//! renderer translates FROM. [`help_text_key`] maps each
//! canonical display string to its `help_*` key and
//! [`help_text`] renders `tr` for it (the audio panel's
//! `health_word_tr` resolver shape); the header meta is a
//! `{slot}` template composed by [`header_meta`] (the
//! camera `follow_label` precedent). Two strings are
//! deliberately NOT wired here: the panel title "Help" is
//! U10's WIRE site for the catalogue's existing `help` key
//! (IA-U10-1 Part 2 names this exact call; V11/integrator
//! apply it), and the Features label "Live MCP control" is
//! U10's WIRE candidate for the existing `live_mcp_control`
//! key — minting a competing `help_*` key for either would
//! fork the catalogue decision. The key-cap labels
//! (`Ctrl+Z`, `Alt+1`, …) stay canonical: they name
//! physical keys, are shared with the menu shortcut hints,
//! and are identifiers, not prose. Keys land via IA-V5-1;
//! until they do, `tr` falls back to the key itself and the
//! pin below fails by design (the Q7 pattern: wiring and
//! keys land together at integration).
//!
//! Wave W (W11) doc correction, 2026-11-10: the dated
//! counts above are each refresh's own census — 54 at
//! Phase M, unchanged through the two Phase N passes,
//! then 56, 57, 58 and 61 at the UI-mission Waves 9–12
//! refreshes — so the three paragraphs still reading
//! "54 rows" state Phase M/N's figure, NOT the current
//! census. The panel now holds **61 rows**: 10 Shortcuts
//! + 8 Workspaces + 2 Posing + 5 Timeline + 36 Features,
//! Drawing none, pinned by
//! `shortcut_rows_has_exactly_61_rows` below, whose
//! comment carries the full reconciliation. The dated
//! paragraphs stand as written — their counts are the
//! evidence the later +2 / +1 / +1 / +3 arithmetic
//! stands on — but no "54 rows" above describes the
//! panel as it is today.
//!
//! Roadmap Wave 1 (RM26, PL-4 + PL-5), 2026-11-10: the
//! five key sections are no longer literal tables here —
//! they are generated from the binding registry
//! (`keymap::BINDINGS`), which the menu hints and the
//! palette chips also read, so the three surfaces share
//! one spelling by construction. PL-4's clusters added
//! 19 rows (File/Edit chords + the Backspace alias in
//! Shortcuts, the arrow-nudge row in Posing, the
//! transport + keyframing clusters in Timeline, and
//! Drawing's first four keys — its "no shortcuts yet"
//! note is retired). The panel now holds **81 rows**:
//! 17 Shortcuts + 8 Workspaces + 3 Posing + 13 Timeline
//! + 4 Drawing + 36 Features, pinned by
//! `shortcut_rows_has_exactly_81_rows` below. (As
//! landed at integration: RM26's registry brought the
//! census to 80; IA-RM28-5's F1 binding then added the
//! 81st row, Shortcuts 16 → 17.)
//!
//! Roadmap Wave 1 (RM28, PL-7 first steps), 2026-11-10:
//! the panel becomes searchable and its rows act. A
//! search field (state: [`HelpPanelState::search`], the
//! figures filter's `TextEdit` idiom) filters the
//! canonical rows: matching runs over the canonical
//! English AND its rendered translation, a section the
//! query names shows whole, and a query matching
//! nothing renders the kit's actionable empty state
//! with a Clear search button. Under the header, a
//! Getting Started block (NOT part of the
//! [`group_rows`] census) lists six steps whose
//! buttons perform the step: go to Animate, add a
//! figure, add a frame, play, save, go to Export.
//! Rows whose label names a real op now run it on
//! press: a key row runs the palette command its
//! registry binding names (PL-5's twin link — the
//! very op the key runs), with three direct arms
//! (Ctrl+K opens the palette, C flips camera mode,
//! F1 closes the panel — the toggle's reachable half
//! from inside it); every Features row except MCP
//! stdio server (an external program — nothing in-app
//! to jump to) takes the user to the surface its text
//! names (a workspace switch; Settings opens for Live
//! MCP control; the palette opens for Palette
//! recents). Rows with no honest press — the
//! palette-scope Down/Up/Enter, Escape, the Delete
//! cascade, the Arrows nudge row, the two 10-frame
//! rows — stay plain text, unpressable. The F1 row
//! itself joins the registry via IA-RM28-5 (the
//! `on_key` arm in main.rs, placed after the
//! text-field bail so the Shortcuts description's
//! typing-guard sentence stays exactly true, plus the
//! `keymap` entry that generates the row); the search
//! wiring is IA-RM28-1/-2/-3 and the new strings are
//! IA-RM28-4 — the Q7 pattern: wiring and keys land
//! together at integration. The census stands at 80
//! until IA-RM28-5 lands, at 81 with it.

use rune::tok::{type_, Theme};
use rune::{col, row, text, El, TextEdit};
use stykanimate_rs::editor::EditorCore;

use crate::chrome_view::btn;
use crate::rune_sec::command_palette;
use crate::rune_sec::design;
use crate::rune_sec::i18n_wire;
use crate::rune_sec::icons::icon_el;
use crate::rune_sec::keymap;
use crate::rune_sec::workspaces;
use crate::State;

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
/// section names from [`group_rows`]. (Drawing held no live
/// keys until Roadmap Wave 1's PL-4 cluster — RM26 — gave
/// it four; its old [`section_note`] is retired with them.)
pub fn section_rows(section: &str) -> Vec<(&'static str, &'static str)> {
    match section {
        // Roadmap Wave 1 (RM26, PL-5): every section that
        // holds bindings is generated from the binding
        // registry (`keymap::BINDINGS`) — the help rows,
        // the menu hints and the palette chips now read
        // one table, so a spelling cannot drift between
        // surfaces. The Features section below stays a
        // literal table: its rows name features, not keys.
        "Shortcuts" => crate::rune_sec::keymap::section_rows("Shortcuts"),
        "Workspaces" => crate::rune_sec::keymap::section_rows("Workspaces"),
        "Posing" => crate::rune_sec::keymap::section_rows("Posing"),
        "Timeline" => crate::rune_sec::keymap::section_rows("Timeline"),
        "Drawing" => crate::rune_sec::keymap::section_rows("Drawing"),
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
                "Project assets",
                "Scene panel — a Placed media readout counts the project's assets in use; the asset list itself is managed with the MCP tools assets_list, asset_info, asset_import, asset_rename, asset_attach and asset_remove — an asset a figure segment, a movieclip definition's segment or a sprite still uses cannot be removed, detach it first (a movieclip definition's segment media is read with clip_get_node_media and detached with clip_clear_node_media)",
            ),
            (
                "Attach asset to segment",
                "Segment inspector — attach an image from the project's assets to the selected segment, or clear the segment's image; MCP clients use asset_attach",
            ),
            (
                "Sprites",
                "Sprites panel — free-floating images from the project's assets, each placed with keyframed position, rotation and scale: add an instance from the panel, select it, and step its transform keys at the current frame (the stage draws an outline and label; exports render the real image). MCP clients use sprite_list, sprite_add, sprite_remove, sprite_set_key and sprite_delete_key. An image fixed to a segment is attached from the segment inspector instead",
            ),
            (
                "MCP stdio server",
                "The mcp_server program serves the same core tools over stdio (JSON-RPC, one message per line) to clients that launch it — it works on its own copy of a project and never sees or drives this editor window",
            ),
            (
                "Live MCP control",
                "Settings — off by default; when on, the Live MCP control toggle lets MCP clients and other programs on this computer drive this running editor in full at http://127.0.0.1:32117/mcp: the core tools edit the project, and the live-only shell_* tools drive the window itself — workspace, playback, view, selection, theme and panels (the full tool list is in docs/MCP.md). There is no password: any program on this computer can drive the editor and its window while this is on, so turn it off here when you do not want that",
            ),
        ],
        _ => vec![],
    }
}

/// The explanatory note rendered in place of rows for a
/// section that has no live shortcuts yet. `None` for
/// sections that have rows.
pub fn section_note(section: &str) -> Option<&'static str> {
    // Roadmap Wave 1 (RM26): Drawing's note ("No keyboard
    // shortcuts yet…") is retired — PL-4 gave the section
    // real keys, so no section currently needs a note. The
    // fn stays as the panel's one extension point for a
    // future row-less section.
    let _ = section;
    None
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
        // RM26: Drawing has real keys now (PL-4), routed
        // through the same text-field guard as every other
        // binding — its description says so, like the rest.
        ("Drawing", "Freehand drawing keys — paused while typing in a text field"),
        (
            "Features",
            "Features on buttons, panels and the stage — none of these is a keyboard shortcut",
        ),
    ]
}

/// The catalogue key for one canonical display string, or
/// `None` when the string is deliberately not wired (see
/// below). The canonical data ([`group_rows`],
/// [`section_rows`], [`section_note`]) stays English — the
/// command palette's drift guard and this file's row
/// census pin those pairs — so localisation resolves the
/// string to its key here, at render time, in the audio
/// panel's `health_word_tr` resolver shape. NOT wired, by
/// decision (see the module docs): the panel title "Help"
/// (U10's WIRE site for the existing `help` key, IA-U10-1)
/// and the Features label "Live MCP control" (U10's WIRE
/// candidate for the existing `live_mcp_control` key) are
/// absent from this table on purpose — both are existing
/// catalogue keys whose wiring is V11's catalogue-hygiene
/// call, and a competing `help_*` key would fork it. The
/// key-cap labels never reach this table: they name
/// physical keys and are identifiers, not prose. Keys land
/// via IA-V5-1 (the Q7 pattern).
fn help_text_key(english: &str) -> Option<&'static str> {
    Some(match english {
        // Section titles.
        "Shortcuts" => "help_section_shortcuts",
        "Workspaces" => "help_section_workspaces",
        "Posing" => "help_section_posing",
        "Timeline" => "help_section_timeline",
        "Drawing" => "help_section_drawing",
        "Features" => "help_section_features",
        // Section descriptions.
        "General editing keys — all but Ctrl+K pause while typing in a text field" => {
            "help_section_shortcuts_desc"
        }
        "Workspace switching keys — paused while typing in a text field" => {
            "help_section_workspaces_desc"
        }
        "Keys that change how stage drags behave and what the stage shows — paused while typing in a text field" => {
            "help_section_posing_desc"
        }
        "Playback and frame keys — paused while typing in a text field" => {
            "help_section_timeline_desc"
        }
        "Freehand drawing keys — paused while typing in a text field" => {
            "help_section_drawing_desc"
        }
        "Features on buttons, panels and the stage — none of these is a keyboard shortcut" => {
            "help_section_features_desc"
        }
        // (The Drawing section's note and its
        // `help_drawing_note` key retired at RM26 — the
        // section has real keys now; see `section_note`.)
        // Shortcuts actions ("Redo" serves two rows).
        "Undo" => "help_act_undo",
        "Redo" => "help_act_redo",
        "Quick save" => "help_act_quick_save",
        "Open the command palette" => "help_act_open_palette",
        "Next result in the command palette (palette open)" => "help_act_palette_next",
        "Previous result in the command palette (palette open)" => "help_act_palette_prev",
        "Run the selected command (command palette open)" => "help_act_palette_run",
        "Close the open menu or command palette" => "help_act_close_menu_or_palette",
        "Delete the selected stroke, particle event or segment" => "help_act_delete_selected",
        // Workspaces actions.
        "Switch to the Animate workspace" => "help_act_workspace_animate",
        "Switch to the Draw workspace" => "help_act_workspace_draw",
        "Switch to the Graph workspace" => "help_act_workspace_graph",
        "Switch to the Effects workspace" => "help_act_workspace_effects",
        "Switch to the Audio workspace" => "help_act_workspace_audio",
        "Switch to the 3D workspace" => "help_act_workspace_3d",
        "Switch to the Export workspace" => "help_act_workspace_export",
        "Switch to the Home workspace" => "help_act_workspace_home",
        // Posing actions.
        "Toggle camera mode" => "help_act_toggle_camera_mode",
        "Toggle onion skin" => "help_act_toggle_onion_skin",
        // Timeline actions.
        "Play / pause" => "help_act_play_pause",
        "Previous frame (wraps around)" => "help_act_prev_frame_wrap",
        "Next frame (wraps around)" => "help_act_next_frame_wrap",
        "Add a frame after the current one" => "help_act_add_frame",
        "Insert an inbetween frame" => "help_act_insert_inbetween",
        // Roadmap Wave 1 (RM26, PL-4) actions — the
        // registry's new bindings. Keys land via
        // IA-RM26-3 (the Q7 pattern: wiring and keys land
        // together at integration).
        "Open the selected project file" => "help_act_open_project_file",
        "Save a copy of the project" => "help_act_save_copy",
        "Duplicate the current frame" => "help_act_duplicate_frame",
        "Copy the current frame" => "help_act_copy_frame",
        "Paste the copied frame" => "help_act_paste_frame",
        "Nudge the selected joint by 1 (Shift 10, Alt 0.1); with no joint selected, step frames" => {
            "help_act_nudge_selected_joint"
        }
        "Go to the first frame" => "help_act_first_frame",
        "Go to the last frame" => "help_act_last_frame",
        "Back 10 frames" => "help_act_back_10_frames",
        "Forward 10 frames" => "help_act_forward_10_frames",
        "Go to the previous keyframe" => "help_act_prev_keyframe",
        "Go to the next keyframe" => "help_act_next_keyframe",
        "Key all channels at the playhead" => "help_act_key_all_channels",
        "Delete the key at the playhead" => "help_act_delete_key_here",
        "Toggle draw mode" => "help_act_toggle_draw_mode",
        "Brush size down (nearest smaller preset)" => "help_act_brush_size_down",
        "Brush size up (nearest larger preset)" => "help_act_brush_size_up",
        "Cycle brush" => "help_act_cycle_brush",
        // Features labels ("Live MCP control" is absent on
        // purpose — see this fn's docs).
        "Loop playback" => "help_feature_loop_playback",
        "Playback speed" => "help_feature_playback_speed",
        "Keyframe jump" => "help_feature_keyframe_jump",
        "Timeline ruler" => "help_feature_timeline_ruler",
        "Dopesheet" => "help_feature_dopesheet",
        "Clear keys in loop range" => "help_feature_clear_keys_in_loop_range",
        "Shift graph channel" => "help_feature_shift_graph_channel",
        "Delete graph channel" => "help_feature_delete_graph_channel",
        "Loop range steppers" => "help_feature_loop_range_steppers",
        "Camera key delete" => "help_feature_camera_key_delete",
        "Swap frames" => "help_feature_swap_frames",
        "View zoom" => "help_feature_view_zoom",
        "Stroke selection" => "help_feature_stroke_selection",
        "Right-press selects" => "help_feature_right_press_selects",
        "Stroke simplify" => "help_feature_stroke_simplify",
        "Stroke opacity" => "help_feature_stroke_opacity",
        "Particles panel" => "help_feature_particles_panel",
        "Mute audio clips" => "help_feature_mute_audio_clips",
        "Audio clip selection" => "help_feature_audio_clip_selection",
        "3D preview" => "help_feature_3d_preview",
        "3D figure on the stage" => "help_feature_3d_figure_on_stage",
        "Bend steppers" => "help_feature_bend_steppers",
        "Split FX clip" => "help_feature_split_fx_clip",
        "Hide FX clips" => "help_feature_hide_fx_clips",
        "Mirror whole figure" => "help_feature_mirror_whole_figure",
        "Head ring size" => "help_feature_head_ring_size",
        "Frame rate & stage size" => "help_feature_frame_rate_stage_size",
        "Delete project file" => "help_feature_delete_project_file",
        "Reload from disk" => "help_feature_reload_from_disk",
        "Palette recents" => "help_feature_palette_recents",
        "Range export" => "help_feature_range_export",
        "Project assets" => "help_feature_project_assets",
        "Attach asset to segment" => "help_feature_attach_asset_to_segment",
        "Sprites" => "help_feature_sprites",
        "MCP stdio server" => "help_feature_mcp_stdio_server",
        // Features actions.
        "Transport Loop button or the command palette" => "help_feature_loop_playback_desc",
        "Transport gauge button cycles 0.25×, 0.5×, 1×, 1.5×, 2×" => {
            "help_feature_playback_speed_desc"
        }
        "Prev key / Next key transport buttons — or click a timeline marker (◆ camera, ● graph)" => {
            "help_feature_keyframe_jump_desc"
        }
        "Second labels above the frame chips" => "help_feature_timeline_ruler_desc",
        "Timeline — one row per figure; ◆ marks that figure's keyframes; click a cell to go to that frame and select the figure" => {
            "help_feature_dopesheet_desc"
        }
        "Keyframes — eraser button clears the selected joint's keys inside the loop range" => {
            "help_feature_clear_keys_in_loop_range_desc"
        }
        "Graph panel — Shift −1 / +1 buttons move every key in the channel; Keyframes Fill range copies the current key across the loop range" => {
            "help_feature_shift_graph_channel_desc"
        }
        "Graph panel — Delete channel button removes the drawn channel" => {
            "help_feature_delete_graph_channel_desc"
        }
        "Timeline — the loop range's only home: Set loop button, Start / End steppers and Clear below the frame chips" => {
            "help_feature_loop_range_steppers_desc"
        }
        "Camera panel — Delete key button removes the camera key at the current frame" => {
            "help_feature_camera_key_delete_desc"
        }
        "Frame ops — Swap with previous / next buttons or the command palette; camera, particle and graph keys follow the swap" => {
            "help_feature_swap_frames_desc"
        }
        "Stage — the mouse wheel zooms the view, not the camera; the stage overlay's Fit / Centre / 1:1 and −/+ steer the same view, and the Camera panel authors the real camera" => {
            "help_feature_view_zoom_desc"
        }
        "Click a stroke on the stage to select it" => "help_feature_stroke_selection_desc",
        "Stage — right-press a figure to select it and its joint" => {
            "help_feature_right_press_selects_desc"
        }
        "Stroke inspector — Simplify button reduces the selected stroke's points" => {
            "help_feature_stroke_simplify_desc"
        }
        "Draw workspace — stroke panel opacity steppers; brushes record opacity on strokes, and the stage and exports honour it" => {
            "help_feature_stroke_opacity_desc"
        }
        "Effects workspace — add, select and delete particle events" => {
            "help_feature_particles_panel_desc"
        }
        "Audio panel — Mute button, command palette or Edit menu; muted clips are excluded from playback and exports" => {
            "help_feature_mute_audio_clips_desc"
        }
        "Click a clip row — the effects then act on that clip" => {
            "help_feature_audio_clip_selection_desc"
        }
        "3D workspace — live preview of the 3D figure; drag to orbit, Grid toggle shows the ground grid" => {
            "help_feature_3d_preview_desc"
        }
        "3D workspace — follows the open 3D session: the stage itself draws the 3D figure and its ground grid, like the panel preview" => {
            "help_feature_3d_figure_on_stage_desc"
        }
        "Segment inspector buttons bend the selected segment by −5°, −1°, +1° or +5°" => {
            "help_feature_bend_steppers_desc"
        }
        "Effects workspace — Split at playhead button or the command palette" => {
            "help_feature_split_fx_clip_desc"
        }
        "Effects workspace — FX panel Hide button keeps a clip in the project but out of the stage and exports; the timeline clips lane dims muted and hidden spans" => {
            "help_feature_hide_fx_clips_desc"
        }
        "Pose tools or command palette — Mirror figure (all frames)" => {
            "help_feature_mirror_whole_figure_desc"
        }
        "Figure inspector — −/+ steppers scale the project's head ring" => {
            "help_feature_head_ring_size_desc"
        }
        "Scene panel — Frame rate stepper and Stage size width / height steppers" => {
            "help_feature_frame_rate_stage_size_desc"
        }
        "Home workspace — Delete on a project row, then confirm" => {
            "help_feature_delete_project_file_desc"
        }
        "Project tools — Reload from disk button re-reads the saved file" => {
            "help_feature_reload_from_disk_desc"
        }
        "Command palette — recent commands are remembered between sessions" => {
            "help_feature_palette_recents_desc"
        }
        "Export panel — SVG and Spritesheet now take the loop range too" => {
            "help_feature_range_export_desc"
        }
        "Scene panel — a Placed media readout counts the project's assets in use; the asset list itself is managed with the MCP tools assets_list, asset_info, asset_import, asset_rename, asset_attach and asset_remove — an asset a figure segment, a movieclip definition's segment or a sprite still uses cannot be removed, detach it first (a movieclip definition's segment media is read with clip_get_node_media and detached with clip_clear_node_media)" => {
            "help_feature_project_assets_desc"
        }
        "Segment inspector — attach an image from the project's assets to the selected segment, or clear the segment's image; MCP clients use asset_attach" => {
            "help_feature_attach_asset_to_segment_desc"
        }
        "Sprites panel — free-floating images from the project's assets, each placed with keyframed position, rotation and scale: add an instance from the panel, select it, and step its transform keys at the current frame (the stage draws an outline and label; exports render the real image). MCP clients use sprite_list, sprite_add, sprite_remove, sprite_set_key and sprite_delete_key. An image fixed to a segment is attached from the segment inspector instead" => {
            "help_feature_sprites_desc"
        }
        "The mcp_server program serves the same core tools over stdio (JSON-RPC, one message per line) to clients that launch it — it works on its own copy of a project and never sees or drives this editor window" => {
            "help_feature_mcp_stdio_server_desc"
        }
        "Settings — off by default; when on, the Live MCP control toggle lets MCP clients and other programs on this computer drive this running editor in full at http://127.0.0.1:32117/mcp: the core tools edit the project, and the live-only shell_* tools drive the window itself — workspace, playback, view, selection, theme and panels (the full tool list is in docs/MCP.md). There is no password: any program on this computer can drive the editor and its window while this is on, so turn it off here when you do not want that" => {
            "help_feature_live_mcp_control_desc"
        }
        // Roadmap Wave 1 (RM28): the F1 row's action, the
        // Getting Started block and the search chrome.
        // Keys land via IA-RM28-4 (the Q7 pattern).
        "Open or close Help" => "help_open_or_close_help",
        "Getting Started" => "help_getting_started",
        "New here? Each step does the step for you — work through them in order." => {
            "help_start_desc"
        }
        "Go to the Animate workspace" => "help_start_go_animate",
        "Add a figure" => "help_start_add_figure",
        "Add a frame" => "help_start_add_frame",
        "Play the animation" => "help_start_play",
        "Save the project" => "help_start_save",
        "Open the Export workspace to render a video" => "help_start_open_export",
        "Search help" => "help_search_placeholder",
        "Clear search" => "help_search_clear",
        _ => return None,
    })
}

/// Render one canonical display string in the core's
/// current language: its catalogue translation when
/// [`help_text_key`] wires it, the canonical English
/// otherwise (the deliberate exclusions — see
/// [`help_text_key`]).
fn help_text(core: &EditorCore, english: &'static str) -> String {
    match help_text_key(english) {
        Some(key) => i18n_wire::tr(core, key),
        None => english.to_string(),
    }
}

/// The header meta line: the open project's name, frame
/// count and fps, composed from the `help_meta` template
/// (Wave V, V5; the camera `follow_label` shape — numeric
/// slots are substituted before `{name}`, so a project
/// name containing slot text is never re-scanned). Until
/// IA-V5-1 lands the key, `tr` falls back to the key itself
/// and the pin below fails by design (the Q7 pattern).
fn header_meta(core: &EditorCore) -> String {
    let fps = core.project.fps;
    let fps_txt = if fps.fract() == 0.0 {
        format!("{}", fps as i64)
    } else {
        format!("{fps:.1}")
    };
    i18n_wire::tr(core, "help_meta")
        .replace("{frames}", &core.frame_count().to_string())
        .replace("{fps}", &fps_txt)
        .replace("{name}", &core.project.name)
}

/// View-state for the help panel (Roadmap Wave 1,
/// RM28): the search box's text edit. Owned by `State`
/// (the wiring is IA-RM28-1/-2/-3); the derived
/// `Default` covers it (`TextEdit` defaults to empty),
/// so the shell's init is one field line. Searching
/// never touches the project.
#[derive(Clone, Debug, Default)]
pub struct HelpPanelState {
    /// Help search box contents (Rune core text edit).
    /// The sections below filter through
    /// [`section_match`] on this text every render;
    /// typing is the only writer.
    pub search: TextEdit,
}

/// True when `query` actually narrows the panel: a
/// non-blank search (the figures filter's
/// `filter_active` rule).
pub fn search_active(query: &str) -> bool {
    !query.trim().is_empty()
}

/// The search comparison: a case-insensitive substring
/// test over BOTH the canonical English string and its
/// rendered translation, so a search finds rows in the
/// language the user is reading — the canonical pairs
/// are English, the rendered text is what is on
/// screen. A blank query matches everything (callers
/// gate on [`search_active`] first).
fn text_matches(query: &str, canonical: &str, rendered: &str) -> bool {
    let q = query.trim().to_lowercase();
    q.is_empty()
        || canonical.to_lowercase().contains(&q)
        || rendered.to_lowercase().contains(&q)
}

/// One section's search result: the rows that survive
/// the query, and whether the section note shows.
struct SectionMatch {
    rows: Vec<(&'static str, &'static str)>,
    note: bool,
}

/// Filter one section by `query`. With no active
/// search the section is whole. A query naming the
/// section or its description shows the section whole;
/// otherwise rows survive individually (either column
/// may match, in either language) and the note shows
/// when its own text matches. `None` hides the
/// section from a searching panel entirely.
fn section_match(
    core: &EditorCore,
    name: &'static str,
    description: &'static str,
    query: &str,
) -> Option<SectionMatch> {
    let rows = section_rows(name);
    let note = section_note(name);
    if !search_active(query) {
        return Some(SectionMatch {
            rows,
            note: note.is_some(),
        });
    }
    if text_matches(query, name, &help_text(core, name))
        || text_matches(query, description, &help_text(core, description))
    {
        return Some(SectionMatch {
            rows,
            note: note.is_some(),
        });
    }
    let kept: Vec<(&'static str, &'static str)> = rows
        .into_iter()
        .filter(|(label, action)| {
            text_matches(query, label, &help_text(core, label))
                || text_matches(query, action, &help_text(core, action))
        })
        .collect();
    let note_hit = note.map_or(false, |n| text_matches(query, n, &help_text(core, n)));
    if kept.is_empty() && !note_hit {
        None
    } else {
        Some(SectionMatch {
            rows: kept,
            note: note_hit,
        })
    }
}

/// The palette command a key row's press runs, from
/// the binding registry's own twin link (PL-5: the
/// registry names the command whose action is exactly
/// this binding's). Running it runs the very op the
/// key runs — `cmd_undo` is `on_key`'s undo body,
/// stroke-selection clear included.
fn key_row_command(key: &str) -> Option<&'static str> {
    // The Ctrl+Y alias row: the palette lists Redo
    // once, under Ctrl+Shift+Z, so the registry leaves
    // the alias unlinked; its press runs the same
    // command.
    if key == "Ctrl+Y" {
        return Some("redo");
    }
    keymap::BINDINGS
        .iter()
        .find(|b| b.keys == key)
        .and_then(|b| b.command_key)
}

/// True when a key row's press performs its labelled
/// action. The rows left plain are the ones with no
/// honest press: the palette-scope rows (Down/Up/Enter
/// — the palette is closed while Help is open),
/// Escape and the Delete/Backspace cascade (both are
/// `on_key`'s own tier/cascade code, with no shared
/// entry point), the Arrows nudge row (its op is
/// modifier- and selection-sensitive) and the two
/// 10-frame rows (no palette twin).
fn key_row_has_action(key: &str) -> bool {
    matches!(key, "F1" | "C" | "Ctrl+K") || key_row_command(key).is_some()
}

/// A key row's press: run its labelled action. Only
/// called for rows [`key_row_has_action`] accepts.
fn key_row_press(s: &mut State, key: &str) {
    match key {
        // F1 toggles Help; from inside the open panel
        // the reachable half of the toggle is Close.
        // (The row itself lands with IA-RM28-5.)
        "F1" => s.menus.help_open = false,
        // Ctrl+K's binding IS opening the palette.
        "Ctrl+K" => command_palette::open_palette(s),
        // Camera mode has no palette twin; the binding
        // is the one-field flip `on_key` runs.
        "C" => s.core.camera_mode = !s.core.camera_mode,
        _ => {
            if let Some(cmd) = key_row_command(key) {
                if let Some(c) = command_palette::COMMANDS.iter().find(|c| c.key == cmd) {
                    (c.run)(s);
                }
            }
        }
    }
}

/// Where a Features row's press takes the user: the
/// workspace hosting the surface its text names, or
/// one of the two chrome surfaces (Settings, the
/// palette) that mount in every workspace.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FeatureAction {
    Workspace(workspaces::Workspace),
    OpenSettings,
    OpenPalette,
    /// No in-app target: the row documents an external
    /// program (the MCP stdio server).
    None,
}

/// A Features row's press target, by canonical label.
/// The default arm is Animate: the transport, the
/// timeline and its lanes, the keyframe tools, the
/// camera and scene panels, the inspectors, the pose
/// tools and the sprites panel all live there (the
/// workspaces gating table is the census this
/// follows).
fn feature_action(label: &str) -> FeatureAction {
    use workspaces::Workspace as Ws;
    match label {
        "MCP stdio server" => FeatureAction::None,
        "Live MCP control" => FeatureAction::OpenSettings,
        "Palette recents" => FeatureAction::OpenPalette,
        "Shift graph channel" | "Delete graph channel" => FeatureAction::Workspace(Ws::Graph),
        "Stroke simplify" | "Stroke opacity" => FeatureAction::Workspace(Ws::Draw),
        "Particles panel" | "Split FX clip" | "Hide FX clips" => {
            FeatureAction::Workspace(Ws::Effects)
        }
        "Mute audio clips" | "Audio clip selection" => FeatureAction::Workspace(Ws::Audio),
        "3D preview" | "3D figure on the stage" => FeatureAction::Workspace(Ws::Build3D),
        "Range export" => FeatureAction::Workspace(Ws::Export),
        "Delete project file" | "Reload from disk" => FeatureAction::Workspace(Ws::Home),
        _ => FeatureAction::Workspace(Ws::Animate),
    }
}

/// A Features row's press: go to the surface its text
/// names. The help panel itself is ungated and stays
/// open beside it.
fn feature_press(s: &mut State, label: &str) {
    match feature_action(label) {
        FeatureAction::Workspace(ws) => workspaces::select_workspace(s, ws),
        FeatureAction::OpenSettings => s.settings.open = true,
        FeatureAction::OpenPalette => command_palette::open_palette(s),
        FeatureAction::None => {}
    }
}

/// The slug inside a pressable help row's element key,
/// from its section and canonical label. The key itself
/// is composed at the two call sites
/// (`help-row-{slug}` — one `format!` template), so the
/// key census sees the dynamic pattern.
fn row_slug(section: &str, label: &str) -> String {
    format!("{section}-{label}")
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect()
}

/// The Getting Started steps, as (element key, label)
/// pairs in order. NOT part of [`group_rows`] /
/// [`section_rows`]: those are the canonical shortcut
/// census the palette drift guard pins. Each step's
/// button performs the step (see [`start_press`]).
pub fn start_steps() -> Vec<(&'static str, &'static str)> {
    vec![
        ("help-start-1", "Go to the Animate workspace"),
        ("help-start-2", "Add a figure"),
        ("help-start-3", "Add a frame"),
        ("help-start-4", "Play the animation"),
        ("help-start-5", "Save the project"),
        ("help-start-6", "Open the Export workspace to render a video"),
    ]
}

/// A Getting Started step's press: the step itself —
/// the same ops the shell's own controls run (the
/// figures panel's + Figure is `core.add_figure`, the
/// transport's Play is `chrome_view::toggle_play`,
/// quick save is `core.save`).
fn start_press(s: &mut State, step: usize) {
    match step {
        0 => workspaces::select_workspace(s, workspaces::Workspace::Animate),
        1 => s.core.add_figure(),
        2 => s.core.add_frame(),
        3 => crate::chrome_view::toggle_play(s),
        4 => s.core.save(),
        5 => workspaces::select_workspace(s, workspaces::Workspace::Export),
        _ => {}
    }
}

/// The Getting Started block: a titled section whose
/// six numbered steps are buttons that do the step.
/// Shown only while no search is active — search is a
/// reference lookup, not the starter path.
fn getting_started(core: &EditorCore, t: &Theme) -> El {
    let mut block = col()
        .key("help-getting-started")
        .gap(4.0)
        .child(
            row()
                .w_full()
                .gap(8.0)
                .center_y()
                .child(design::panel_title(
                    Some("shapes"),
                    &help_text(core, "Getting Started"),
                    None,
                    t,
                )),
        )
        .child(
            text(help_text(
                core,
                "New here? Each step does the step for you — work through them in order.",
            ))
            .font(type_::LABEL)
            .color(t.text_secondary),
        );
    for (i, (key, label)) in start_steps().into_iter().enumerate() {
        block = block.child(
            row()
                .gap(8.0)
                .center_y()
                .child(
                    text(format!("{}.", i + 1))
                        .font(type_::LABEL)
                        .color(t.text_secondary),
                )
                .child(btn(key, &help_text(core, label), false, t, move |s: &mut State| {
                    start_press(s, i)
                })),
        );
    }
    block
}

/// The no-results line for an active search: the
/// `help_no_matches` template (the [`header_meta`]
/// shape — the query substitutes into `{query}`).
/// The key lands via IA-RM28-4 (the Q7 pattern).
fn no_matches_line(core: &EditorCore, query: &str) -> String {
    i18n_wire::tr(core, "help_no_matches").replace("{query}", query.trim())
}

/// One shortcut row: a kit `kbd` chip, then the action it
/// performs. Only for rows whose first column names a real
/// key (the binding sections) — Features rows render through
/// [`feature_row`] instead. When the row's binding has an
/// honest press (see [`key_row_has_action`]) the row rides
/// the kit's list-row shell and runs the action on press;
/// otherwise it stays the plain text row it has always been.
fn shortcut_row(
    core: &EditorCore,
    section: &str,
    key: &'static str,
    action: &'static str,
    t: &Theme,
) -> El {
    if key_row_has_action(key) {
        design::row_shell(false, t)
            .key(format!("help-row-{}", row_slug(section, key)))
            .on_press(move |s: &mut State| key_row_press(s, key))
            .child(design::kbd(key, t))
            .child(text(help_text(core, action)).font(type_::LABEL).color(t.text))
    } else {
        row()
            .gap(8.0)
            .center_y()
            .child(design::kbd(key, t))
            .child(text(help_text(core, action)).font(type_::LABEL).color(t.text))
    }
}

/// One Features row: the feature name as plain text, then
/// how the feature is reached. The first column names a
/// feature, not a key, so it must not wear a `kbd` chip —
/// a chip would claim a keyboard binding that does not
/// exist (p14 §3). When the feature has an in-app home
/// (see [`feature_action`]) the row rides the kit's
/// list-row shell and takes the user there on press;
/// the one row documenting an external program stays
/// plain.
fn feature_row(core: &EditorCore, label: &'static str, action: &'static str, t: &Theme) -> El {
    if feature_action(label) != FeatureAction::None {
        design::row_shell(false, t)
            .key(format!("help-row-{}", row_slug("Features", label)))
            .on_press(move |s: &mut State| feature_press(s, label))
            .child(text(help_text(core, label)).font(type_::LABEL).color(t.text))
            .child(text(help_text(core, action)).font(type_::LABEL).color(t.text))
    } else {
        row()
            .gap(8.0)
            .center_y()
            .child(text(help_text(core, label)).font(type_::LABEL).color(t.text))
            .child(text(help_text(core, action)).font(type_::LABEL).color(t.text))
    }
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
/// Under an active search the section renders only its
/// surviving rows (see [`section_match`]); `None` hides
/// a section the search emptied.
fn help_section(
    core: &EditorCore,
    name: &'static str,
    description: &'static str,
    query: &str,
    t: &Theme,
) -> Option<El> {
    let m = section_match(core, name, description, query)?;
    // Branching, the header icon and the element key stay
    // keyed on the canonical English `name`; only the
    // displayed title and description translate.
    let header_icon = if name == "Shortcuts" { "key" } else { "list" };
    let mut section = col()
        .key(format!("help-section-{}", name.to_lowercase()))
        .gap(4.0)
        .child(
            row()
                .w_full()
                .gap(8.0)
                .center_y()
                .child(design::panel_title(
                    Some(header_icon),
                    &help_text(core, name),
                    None,
                    t,
                )),
        )
        .child(
            text(help_text(core, description))
                .font(type_::LABEL)
                .color(t.text_secondary),
        );
    if m.rows.is_empty() {
        if m.note {
            if let Some(note) = section_note(name) {
                section = section.child(
                    text(help_text(core, note))
                        .font(type_::LABEL)
                        .color(t.text_secondary),
                );
            }
        }
    } else if name == "Features" {
        for (label, action) in m.rows {
            section = section.child(feature_row(core, label, action, t));
        }
    } else {
        for (key, action) in m.rows {
            section = section.child(shortcut_row(core, name, key, action, t));
        }
    }
    Some(section)
}

/// The help panel: a header (title + the open project's
/// name, frame count and fps, read from the core), the
/// search field, the Getting Started block and the six
/// sections from [`group_rows`] (filtered while a
/// search is active — see [`section_match`]). Like
/// every other panel it builds elements from the core
/// plus its own view-state ([`HelpPanelState`]); the
/// ops its rows run live behind the presses, never in
/// the builder.
pub fn help_panel(core: &EditorCore, st: &HelpPanelState, t: &Theme) -> El {
    let meta = header_meta(core);
    let query = st.search.text.clone();
    let searching = search_active(&query);
    let mut panel = design::panel_col("help-panel", t)
        .child(
            row()
                .w_full()
                .gap(8.0)
                .center_y()
                // The title resolves through the catalogue's
                // existing `help` key (U10's WIRE site, filed
                // as IA-V11-1 and applied at Wave V
                // integration; English byte-identical).
                .child(design::panel_title(
                    Some("circle-question"),
                    &i18n_wire::tr(core, "help"),
                    Some(&meta),
                    t,
                )),
        )
        .child(design::divider(t));
    // The search field (the figures filter's
    // composition): typing is the only writer, and the
    // sections below filter on the text every render.
    let mut search_row = row()
        .key("help-search-row")
        .w_full()
        .gap(8.0)
        .center_y()
        .child(icon_el("magnifying-glass", 13.0, t.text_secondary))
        .child(
            design::text_field(&st.search, type_::BODY.lh(26.0), st.search.focused, t)
                .key("help-search")
                .single_line()
                .grow(1.0)
                .color(t.text)
                .placeholder(help_text(core, "Search help"), t.text_secondary)
                .on_edit(|s: &mut State| &mut s.help_panel.search),
        );
    if searching {
        search_row = search_row.child(btn(
            "help-search-clear",
            &help_text(core, "Clear search"),
            false,
            t,
            |s: &mut State| s.help_panel.search = TextEdit::default(),
        ));
    }
    panel = panel.child(search_row);
    if !searching {
        panel = panel.child(getting_started(core, t));
    }
    let mut any_section = false;
    for (name, description) in group_rows() {
        if let Some(section) = help_section(core, name, description, &query, t) {
            panel = panel.child(section);
            any_section = true;
        }
    }
    if searching && !any_section {
        // The search that found nothing offers its own
        // remedy (PL-7's actionable empty states, in
        // this panel first): clear the search.
        panel = panel.child(design::empty_state_action(
            "magnifying-glass",
            &no_matches_line(core, &query),
            "help-no-results-clear",
            &help_text(core, "Clear search"),
            t,
            |s: &mut State| s.help_panel.search = TextEdit::default(),
        ));
    }
    panel
}

#[cfg(test)]
mod tests {
    use super::{
        command_palette, feature_action, group_rows, header_meta, help_text_key,
        i18n_wire, key_row_command, key_row_has_action, keymap, no_matches_line,
        search_active, section_match, section_note, section_rows, shortcut_rows,
        start_steps, text_matches, workspaces, EditorCore, FeatureAction,
        HelpPanelState,
    };

    #[test]
    fn shortcut_rows_has_exactly_81_rows() {
        // 10 Shortcuts + 8 Workspaces + 2 Posing + 5 Timeline
        // + 36 Features; Drawing contributes none (see the
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
        // refresh added one more (Dopesheet). The Wave 11
        // refresh added one more (MCP stdio server).
        // The Wave 12 refresh added three more (Project
        // assets, Attach asset to segment, Sprites).
        // Roadmap Wave 1 (RM26) added 19 binding rows
        // from the keymap registry: Shortcuts 10 → 16
        // (Ctrl+O, Ctrl+Shift+S, Ctrl+D, Ctrl+C, Ctrl+V,
        // Backspace), Posing 2 → 3 (the Arrows nudge
        // row), Timeline 5 → 13 (Home, End, Shift+Left,
        // Shift+Right, comma, period, K, Alt+K) and
        // Drawing 0 → 4 (B, [, ], Shift+B).
        // IA-RM28-5 (RM28) then landed the F1 binding in
        // Shortcuts (16 → 17): the census is 81.
        assert_eq!(shortcut_rows().len(), 81);
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
        assert_eq!(section_rows("Features").len(), 36);
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
        // thirty-two stood then; the Wave 11 refresh
        // added the MCP stdio server row, so thirty-three
        // stood then; the Wave 12 refresh added the three
        // asset rows, so thirty-six stand now.
        let features = section_rows("Features");
        assert_eq!(features.len(), 36);
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
        assert_eq!(features.len(), 36);
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
        assert_eq!(features.len(), 36);
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
        assert_eq!(features.len(), 36);
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
            "Settings — off by default; when on, the Live MCP control toggle lets MCP clients and other programs on this computer drive this running editor in full at http://127.0.0.1:32117/mcp: the core tools edit the project, and the live-only shell_* tools drive the window itself — workspace, playback, view, selection, theme and panels (the full tool list is in docs/MCP.md). There is no password: any program on this computer can drive the editor and its window while this is on, so turn it off here when you do not want that"
        )));
        // Placement: View zoom sits with the stage /
        // camera rows, directly after Swap frames; Live
        // MCP control closes the Features section (the
        // Wave 11 refresh inserted the MCP stdio server
        // row directly before it and reworded this row —
        // see the Wave 11 test). No earlier phase's
        // pinned adjacency is disturbed by either.
        let features = section_rows("Features");
        assert_eq!(features.len(), 36);
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
        assert_eq!(features.len(), 36);
        let ruler_row = features
            .iter()
            .position(|(label, _)| *label == "Timeline ruler")
            .expect("Timeline ruler row");
        assert_eq!(features[ruler_row + 1].0, "Dopesheet");
    }

    #[test]
    fn wave_11_feature_rows_are_listed() {
        // The Wave 11 refresh — MCP end-to-end (see the
        // module docs). The census corrects the wave
        // brief: Help already documented the live
        // endpoint (the Wave 9 Live MCP control row);
        // what was missing is the stdio transport and
        // the default-on framing. MCP stdio server —
        // `src/bin/mcp_server.rs`: JSON-RPC 2.0 over
        // stdio, one JSON object per line, dispatching
        // through the same `stykanimate_rs::mcp::
        // McpServer` as the live bridge; its core is
        // private, so its edits never reach the open
        // editor window (`src/live_mcp.rs`' module docs
        // state the distinction). Live MCP control —
        // reworded in place, not duplicated: on by
        // default (`LayoutPrefs.live_mcp` defaults to
        // `true` in `src/shell.rs`), started at launch,
        // toggled only from Settings (no key binding
        // exists, so no shortcut row), and the security
        // truth in the bridge's own terms (no
        // authentication in v1; `GET /health` states
        // `"auth": "none"`).
        let rows = shortcut_rows();
        assert!(rows.contains(&(
            "MCP stdio server",
            "The mcp_server program serves the same core tools over stdio (JSON-RPC, one message per line) to clients that launch it — it works on its own copy of a project and never sees or drives this editor window"
        )));
        assert!(rows.contains(&(
            "Live MCP control",
            "Settings — off by default; when on, the Live MCP control toggle lets MCP clients and other programs on this computer drive this running editor in full at http://127.0.0.1:32117/mcp: the core tools edit the project, and the live-only shell_* tools drive the window itself — workspace, playback, view, selection, theme and panels (the full tool list is in docs/MCP.md). There is no password: any program on this computer can drive the editor and its window while this is on, so turn it off here when you do not want that"
        )));
        // Placement: the two MCP rows close the
        // Features section as a pair — the stdio row
        // directly before Live MCP control, which stays
        // last (the Wave 9 test's pin). No earlier
        // phase's pinned adjacency is disturbed.
        let features = section_rows("Features");
        assert_eq!(features.len(), 36);
        assert_eq!(
            features[features.len() - 2].0,
            "MCP stdio server"
        );
        assert_eq!(features.last().map(|(label, _)| *label), Some("Live MCP control"));
    }

    #[test]
    fn wave_12_feature_rows_are_listed() {
        // The Wave 12 refresh — the asset surface (see
        // the module docs). Project assets — the
        // project's asset table (`Project.assets`, named
        // PNGs in `model.rs`), managed by MCP clients
        // through the wave's media.rs tools; the Scene
        // panel's part is a read-only Placed media
        // counts readout (the wave's audit proved no
        // asset list exists there — the row says so);
        // the row states the removal refusal plainly (an
        // asset a segment's media or a sprite instance
        // still references cannot be removed). Attach
        // asset to segment — the segment inspector's
        // parity home for attaching an existing asset to
        // the selected segment (the old one-step flow,
        // `EditorCore::attach_image_to_node`, imported
        // the first import-folder PNG as a new asset
        // while attaching; clearing a segment's image is
        // the existing `clear_node_media` op). Sprites —
        // the free-standing instances (`Project.sprites`:
        // centre, rotation and scale per `InstanceKey`,
        // per `media.rs`' docs). At the Wave 12 close
        // this row said — truly, then — that sprites
        // were import-only and not editable in the app;
        // Wave 13 landed the Sprites panel, the sprite
        // engine ops and the five MCP sprite tools, so
        // the row now names the panel home and the tool
        // names, as distinct from images fixed to a
        // segment.
        let rows = shortcut_rows();
        assert!(rows.contains(&(
            "Project assets",
            "Scene panel — a Placed media readout counts the project's assets in use; the asset list itself is managed with the MCP tools assets_list, asset_info, asset_import, asset_rename, asset_attach and asset_remove — an asset a figure segment, a movieclip definition's segment or a sprite still uses cannot be removed, detach it first (a movieclip definition's segment media is read with clip_get_node_media and detached with clip_clear_node_media)"
        )));
        assert!(rows.contains(&(
            "Attach asset to segment",
            "Segment inspector — attach an image from the project's assets to the selected segment, or clear the segment's image; MCP clients use asset_attach"
        )));
        assert!(rows.contains(&(
            "Sprites",
            "Sprites panel — free-floating images from the project's assets, each placed with keyframed position, rotation and scale: add an instance from the panel, select it, and step its transform keys at the current frame (the stage draws an outline and label; exports render the real image). MCP clients use sprite_list, sprite_add, sprite_remove, sprite_set_key and sprite_delete_key. An image fixed to a segment is attached from the segment inspector instead"
        )));
        // Placement: the three asset rows sit together
        // with the project rows, directly after Range
        // export, and the MCP pair still closes the
        // section (the Wave 11 test's pin: stdio server
        // second-to-last, Live MCP control last). No
        // earlier phase's pinned adjacency is disturbed.
        let features = section_rows("Features");
        assert_eq!(features.len(), 36);
        let range_row = features
            .iter()
            .position(|(label, _)| *label == "Range export")
            .expect("Range export row");
        assert_eq!(features[range_row + 1].0, "Project assets");
        assert_eq!(features[range_row + 2].0, "Attach asset to segment");
        assert_eq!(features[range_row + 3].0, "Sprites");
        assert_eq!(features[range_row + 4].0, "MCP stdio server");
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
        // before the guard). Features alone has no keys to
        // guard — it is buttons and panels, and its
        // description says so instead. (Drawing joined the
        // guarded sections at RM26, when PL-4 gave it keys.)
        for (name, description) in group_rows() {
            if name == "Features" {
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
    fn no_section_is_empty_and_none_carries_a_note() {
        // Until RM26 this test pinned the opposite for
        // Drawing (empty rows + the "no shortcuts yet"
        // note); PL-4 gave Drawing real keys, so every
        // section now has rows and no section needs a
        // note.
        for name in ["Shortcuts", "Workspaces", "Posing", "Timeline", "Drawing", "Features"] {
            assert!(!section_rows(name).is_empty(), "{name} should have rows");
            assert!(section_note(name).is_none(), "{name} should have no note");
        }
        assert_eq!(section_rows("Drawing").len(), 4);
    }

    #[test]
    fn rm26_pl4_rows_are_listed() {
        // The PL-4 clusters (RM26), generated from the
        // keymap registry: File/Edit chords + the
        // Backspace alias, the nudge row, the transport
        // and keyframing clusters, and Drawing's first
        // keys.
        let rows = shortcut_rows();
        assert!(rows.contains(&("Ctrl+O", "Open the selected project file")));
        assert!(rows.contains(&("Ctrl+Shift+S", "Save a copy of the project")));
        assert!(rows.contains(&("Ctrl+D", "Duplicate the current frame")));
        assert!(rows.contains(&("Ctrl+C", "Copy the current frame")));
        assert!(rows.contains(&("Ctrl+V", "Paste the copied frame")));
        assert!(rows.contains(&(
            "Backspace",
            "Delete the selected stroke, particle event or segment"
        )));
        assert!(rows.contains(&(
            "Arrows",
            "Nudge the selected joint by 1 (Shift 10, Alt 0.1); with no joint selected, step frames"
        )));
        assert!(rows.contains(&("Home", "Go to the first frame")));
        assert!(rows.contains(&("End", "Go to the last frame")));
        assert!(rows.contains(&("Shift+Left", "Back 10 frames")));
        assert!(rows.contains(&("Shift+Right", "Forward 10 frames")));
        assert!(rows.contains(&(",", "Go to the previous keyframe")));
        assert!(rows.contains(&(".", "Go to the next keyframe")));
        assert!(rows.contains(&("K", "Key all channels at the playhead")));
        assert!(rows.contains(&("Alt+K", "Delete the key at the playhead")));
        assert!(rows.contains(&("B", "Toggle draw mode")));
        assert!(rows.contains(&("[", "Brush size down (nearest smaller preset)")));
        assert!(rows.contains(&("]", "Brush size up (nearest larger preset)")));
        assert!(rows.contains(&("Shift+B", "Cycle brush")));
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

    #[test]
    fn help_i18n_keys_exist_and_match_the_canonical_strings() {
        // Wave V (V5): every display-prose string the panel
        // renders resolves through `help_text_key` to a
        // `help_*` key whose catalogue English is
        // byte-identical to the canonical string. The set
        // is enumerated from the canonical data itself —
        // section names and descriptions, every row's
        // action, the Features labels, and the Drawing
        // note — so a future row added without a key fails
        // here. The key-cap labels of the true-key
        // sections are NOT in the set: they name physical
        // keys (identifiers, not prose). Exactly one
        // deliberate exclusion: the Features label "Live
        // MCP control", U10's WIRE candidate for the
        // existing `live_mcp_control` key (V11's call).
        // The keys land via IA-V5-1, so this pin passes
        // only once that IA lands — until then `tr` falls
        // back to the key itself and `key_in_table` is
        // false (the Q7 pattern: wiring and keys land
        // together at integration).
        let core = EditorCore::new();
        let mut prose: Vec<&'static str> = Vec::new();
        for (name, description) in group_rows() {
            prose.push(name);
            prose.push(description);
            for (label, action) in section_rows(name) {
                if name == "Features" {
                    prose.push(label);
                }
                prose.push(action);
            }
            if let Some(note) = section_note(name) {
                prose.push(note);
            }
        }
        let mut unresolved: Vec<&'static str> = Vec::new();
        let mut keyed: Vec<(&'static str, &'static str)> = Vec::new();
        for english in prose {
            match help_text_key(english) {
                Some(key) => {
                    // One key may not serve two different
                    // strings (the catalogue value could
                    // not equal both); the same string
                    // twice ("Redo") is fine.
                    if let Some((_, other)) =
                        keyed.iter().find(|(k, _)| *k == key)
                    {
                        assert_eq!(
                            *other, english,
                            "i18n key {key} serves two different strings"
                        );
                    }
                    keyed.push((key, english));
                    assert!(
                        i18n_wire::key_in_table(key),
                        "missing i18n key: {key}"
                    );
                    assert_eq!(i18n_wire::tr(&core, key), english);
                }
                None => {
                    if !unresolved.contains(&english) {
                        unresolved.push(english);
                    }
                }
            }
        }
        assert_eq!(unresolved, ["Live MCP control"]);
    }

    #[test]
    fn help_meta_template_exists_and_composes_byte_identically() {
        // Wave V (V5): the header meta resolves through
        // the `help_meta` template key (IA-V5-1) and
        // composing it reproduces exactly the line the
        // pre-wiring `format!` printed. Passes only once
        // IA-V5-1 lands (the Q7 pattern, as above).
        let core = EditorCore::new();
        assert!(i18n_wire::key_in_table("help_meta"), "missing i18n key: help_meta");
        assert_eq!(
            i18n_wire::tr(&core, "help_meta"),
            "{name} · {frames} frames · {fps} fps"
        );
        let fps = core.project.fps;
        let fps_txt = if fps.fract() == 0.0 {
            format!("{}", fps as i64)
        } else {
            format!("{fps:.1}")
        };
        assert_eq!(
            header_meta(&core),
            format!(
                "{} · {} frames · {} fps",
                core.project.name,
                core.frame_count(),
                fps_txt
            )
        );
    }

    #[test]
    fn rm28_search_model_filters_rows_and_sections() {
        let core = EditorCore::new();
        // Blank queries are no search at all.
        assert!(!search_active(""));
        assert!(!search_active("   "));
        assert!(search_active("undo"));
        // Case-insensitive substring over either column
        // (canonical or rendered — identical under an
        // English core).
        assert!(text_matches("UNDO", "Ctrl+Z", "Ctrl+Z"));
        assert!(text_matches("undo", "Ctrl+Z", "Undo"));
        assert!(!text_matches("xyz", "Ctrl+Z", "Undo"));
        // A row-level query keeps only the matching rows.
        let m = section_match(&core, "Shortcuts", group_rows()[0].1, "undo")
            .expect("Shortcuts survives an undo search");
        assert_eq!(m.rows, vec![("Ctrl+Z", "Undo")]);
        // A query naming the section shows it whole.
        let m = section_match(&core, "Workspaces", group_rows()[1].1, "workspaces")
            .expect("a named section shows whole");
        assert_eq!(m.rows.len(), 8);
        // A query with no match anywhere hides the section.
        assert!(section_match(&core, "Features", group_rows()[5].1, "zzz-no-match").is_none());
        // No search: every section is whole.
        for (name, description) in group_rows() {
            let m = section_match(&core, name, description, "").expect("unfiltered section");
            assert_eq!(m.rows, section_rows(name));
        }
    }

    #[test]
    fn rm28_every_actionable_key_row_names_a_real_command() {
        // The press runs the palette command the registry
        // links — the command must exist, or the press
        // would be a fake control.
        for b in keymap::BINDINGS {
            if let Some(cmd) = b.command_key {
                assert!(
                    command_palette::COMMANDS.iter().any(|c| c.key == cmd),
                    "binding {} names missing command {cmd}",
                    b.id
                );
            }
        }
        // The disposition, pinned: these rows act…
        for key in [
            "Ctrl+Z", "Ctrl+Shift+Z", "Ctrl+Y", "Ctrl+S", "Ctrl+K", "Ctrl+O",
            "Ctrl+Shift+S", "Ctrl+D", "Ctrl+C", "Ctrl+V", "Alt+1", "Alt+8",
            "C", "O", "Space", "Left", "Right", "N", "I", "Home", "End", ",",
            ".", "K", "Alt+K", "B", "[", "]", "Shift+B",
        ] {
            assert!(key_row_has_action(key), "{key} should act");
        }
        // …and these stay plain (see key_row_has_action).
        for key in [
            "Down", "Up", "Enter", "Escape", "Delete", "Backspace", "Arrows",
            "Shift+Left", "Shift+Right",
        ] {
            assert!(!key_row_has_action(key), "{key} must stay plain");
        }
        // F1's press is armed ahead of its row (the row
        // lands with IA-RM28-5).
        assert!(key_row_has_action("F1"));
        assert_eq!(key_row_command("Ctrl+Y"), Some("redo"));
    }

    #[test]
    fn rm28_every_feature_row_has_a_disposition() {
        // Every Features row either takes the user to its
        // surface on press or is the one documented
        // exception (the external stdio server).
        for (label, _) in section_rows("Features") {
            let action = feature_action(label);
            if label == "MCP stdio server" {
                assert_eq!(action, FeatureAction::None);
            } else {
                assert_ne!(action, FeatureAction::None, "{label} must act");
            }
        }
        // Spot-pin the non-Animate homes.
        assert_eq!(
            feature_action("Delete graph channel"),
            FeatureAction::Workspace(workspaces::Workspace::Graph)
        );
        assert_eq!(feature_action("Live MCP control"), FeatureAction::OpenSettings);
        assert_eq!(feature_action("Palette recents"), FeatureAction::OpenPalette);
        assert_eq!(
            feature_action("Range export"),
            FeatureAction::Workspace(workspaces::Workspace::Export)
        );
        assert_eq!(
            feature_action("Sprites"),
            FeatureAction::Workspace(workspaces::Workspace::Animate)
        );
    }

    #[test]
    fn rm28_getting_started_steps_are_six_and_wired() {
        let steps = start_steps();
        assert_eq!(steps.len(), 6);
        for (i, (key, label)) in steps.iter().enumerate() {
            assert!(
                !steps[..i].iter().any(|(k, _)| k == key),
                "duplicate step key {key}"
            );
            assert!(!label.trim().is_empty());
        }
        assert_eq!(steps[0], ("help-start-1", "Go to the Animate workspace"));
        assert_eq!(
            steps[5],
            ("help-start-6", "Open the Export workspace to render a video")
        );
    }

    #[test]
    fn rm28_new_strings_resolve_through_the_catalogue() {
        // The RM28 strings (the F1 action, the Getting
        // Started block, the search chrome) resolve
        // through `help_text_key` like every other string
        // the panel renders, to keys whose catalogue
        // English is byte-identical. The keys land via
        // IA-RM28-4, so this pin passes only once that IA
        // lands (the Q7 pattern).
        let core = EditorCore::new();
        let mut strings: Vec<&'static str> = vec![
            "Open or close Help",
            "Getting Started",
            "New here? Each step does the step for you — work through them in order.",
            "Search help",
            "Clear search",
        ];
        for (_, label) in start_steps() {
            strings.push(label);
        }
        for english in strings {
            let key =
                help_text_key(english).unwrap_or_else(|| panic!("{english} is not wired"));
            assert!(i18n_wire::key_in_table(key), "missing i18n key: {key}");
            assert_eq!(i18n_wire::tr(&core, key), english);
        }
        // The no-matches template composes like
        // header_meta (its own pin, same pattern).
        assert!(
            i18n_wire::key_in_table("help_no_matches"),
            "missing i18n key: help_no_matches"
        );
        assert_eq!(
            i18n_wire::tr(&core, "help_no_matches"),
            "No help matches \"{query}\""
        );
        assert_eq!(no_matches_line(&core, " undo "), "No help matches \"undo\"");
    }

    #[test]
    fn rm28_panel_state_defaults_to_an_empty_search() {
        let st = HelpPanelState::default();
        assert!(st.search.text.is_empty());
        assert!(!st.search.focused);
    }
}
