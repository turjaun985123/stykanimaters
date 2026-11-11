//! Stroke / brush inspector (Rune Phase D, section 12/40;
//! per-stroke delete added in Phase E, section 14/40).
//! Declared under `crate::rune_sec` — bin-crate code, not part
//! of the engine library.
//!
//! Detail panel for the active brush preset and the strokes
//! recorded on the current frame. Sibling section 11
//! (`draw_tools`) owns brush *selection* (one button per
//! preset, the cycle button, the eraser toggle, Clear
//! drawing); this section is the inspector view of the same
//! state: the full parameter set of the active preset, and
//! the per-frame stroke list.
//!
//! ## The honest parameter mapping
//!
//! The brief asks for size / opacity / scatter / smoothing
//! steppers and a nib segmented control. The engine has NO
//! per-parameter brush ops: a brush is a preset bundle
//! (`paint::Brush`, paint.rs:304) from the fixed 10-entry
//! `paint::brush_library()` (paint.rs:333), and the only
//! brush ops are `active_brush` (editor.rs:2562),
//! `cycle_brush` (editor.rs:2576) and writing the public
//! `brush_index` field (editor.rs:73) — the write the
//! engine's own tests use to select a brush. There is no
//! `set_brush_width` / `adjust_brush_opacity` / … anywhere
//! (grep-verified), so a stepper that edited one parameter
//! of a custom brush would be a fake control.
//!
//! What the steppers here do instead is real: stepping a
//! parameter selects the nearest preset whose value for
//! THAT parameter is strictly higher / lower (smallest
//! difference, ties to the lowest library index —
//! [`step_index_by`]). The other parameters follow the
//! chosen preset, because presets are bundles; the status
//! line names the resulting brush, exactly as `cycle_brush`
//! reports it. The nib segmented control selects the first
//! library preset carrying the chosen nib
//! ([`first_index_with_nib`]). Every press performs a real
//! selection change (or reports, via the status line, that
//! no preset steps that way).
//!
//! The panel copy says all of this out loud (UI mission
//! Wave 4, audit S08 item I5 — the integrator picked the
//! fallback (b), relabelling, because the preferred (a)
//! needs a per-parameter engine op that does not exist):
//! the section heading is `Brush preset · {name}`, every
//! parameter row is labelled `Preset size` / `Preset
//! opacity` / `Preset scatter` / `Preset smoothing`, the
//! nib row is `Preset nib` with its `Preset nib angle`
//! readout, and a hint line under the tool line states
//! that stepping a value or picking a nib selects another
//! library preset. The numbers in the rows are the active
//! preset's own values, shown beside −/+ buttons that
//! step the *selection* — the labels no longer present
//! them as editors of a working brush's values. (The
//! selected-stroke Width / Opacity rows further up ARE
//! real value editors — they rewrite a recorded stroke —
//! and keep their plain labels.)
//!
//! ## Stroke list
//!
//! One row per stroke on the current frame
//! (`Project.strokes`, model.rs:703, filtered by
//! `Stroke.frame == EditorCore.current`), with a colour
//! swatch, a data summary (points, width, fill, spray
//! mark) and a **Delete** button. The button calls the
//! per-stroke delete op `EditorCore::delete_stroke_at`
//! (added in Phase E, section 7) with the stroke's index
//! in `Project.strokes` — carried on the row data
//! ([`StrokeRow::project_index`]), never the row position,
//! because the frame filter makes the two differ whenever
//! other frames carry strokes. The frame-scoped Clear
//! (`clear_frame_drawings`, editor.rs:2391) remains as
//! the bulk removal above the list.
//!
//! ## Bulk ops (Phase F, section 26)
//!
//! A row at the panel top acts on **every** stroke in
//! `Project.strokes`, across all frames — deliberately a
//! wider scope than the frame-filtered list below it,
//! because the frame-scoped removal already exists (the
//! Clear button above). **Recolour all** rewrites every
//! stroke's base `color` to the bulk target colour
//! ([`bulk_target_color`]: the selected node's colour,
//! falling back to the selected figure's — the stage's
//! own node-over-figure resolution); a stroke's `fill`
//! and spray mark keep their own values. **Delete all**
//! empties `Project.strokes`.
//!
//! Undo composition: there is no bulk op in the core,
//! and `delete_stroke_at` snapshots **per call**, so
//! looping it would flood the undo stack with one step
//! per stroke. Both bulk ops here therefore compose the
//! core's public parts exactly as `delete_stroke_at`
//! itself is built: one `EditorCore::snapshot()`
//! (editor.rs:202 — public, serialises the whole
//! project, strokes included) followed by direct
//! `project.strokes` writes — one press, one undo step.
//! An empty stroke list takes no snapshot at all: the
//! press only reports that there are no strokes.
//!
//! ## Stroke selection (Phase G, section 7)
//!
//! A selection row under the bulk row steps a
//! **project-wide** stroke selection: `State.stroke_selected`
//! (declared in main.rs) holds an index into
//! `Project.strokes` — the same index space
//! `delete_stroke_at` and the stage's
//! `stage_layers::highlight_geometry` use, and the space
//! the stage section reads when it draws the highlight.
//! The per-frame list further down is only a view onto
//! that space (its rows carry
//! [`StrokeRow::project_index`]); the selection ignores
//! the frame filter, so Prev / Next walk every stroke in
//! the project, wrapping at both ends, and the readout
//! names the selected stroke's own frame
//! ([`stroke_frame_of`]) so the user can jump there with
//! **Go to frame** (`EditorCore::goto` — the core's
//! navigation op: no snapshot, no status of its own).
//!
//! Selection is shell view state: stepping and clearing
//! write nothing but `s.stroke_selected` — no snapshot,
//! no core mutation — so they add no undo steps and no
//! status toasts. A stale selection (index ≥ the stroke
//! count, e.g. after a bulk delete elsewhere) is treated
//! as unset everywhere: the readout falls back to "No
//! stroke selected" and stepping re-enters at the first
//! (+1) or last (−1) stroke ([`step_selection`]). On an
//! empty project the row reads "No strokes" and every
//! button is inert apart from the status line.
//!
//! ## Selected stroke measure (Phase I, section 33)
//!
//! While a valid selection exists, a muted line under
//! the selection row (key `stroke-measure`) reports the
//! selected stroke's facts ([`stroke_measure`]):
//! `Stroke {n}: {points} pts · {length} long · frame
//! {f}`. The length sums the base polyline's segment
//! lengths — a spray stroke measures by its base
//! polyline, the picking convention of Phase G section
//! 5 — and a filled stroke's closing edge back to its
//! first point is not counted. No selection (or a stale
//! one) renders no line at all, so an unselected panel
//! gains nothing.
//!
//! ## Frame-scoped bulk ops (Phase J, section 23)
//!
//! A second bulk row, directly under the project-wide
//! one, runs the same two presses against **this frame
//! only** — the strokes [`frame_stroke_indices`]
//! returns for `EditorCore::current`. The Phase F row
//! stayed project-wide because a frame-scoped *removal*
//! already existed (the list's Clear button); recolour
//! had no frame-scoped form at all, and clearing one
//! frame should not need a project-wide delete plus
//! undo archaeology. Both presses keep the Phase F
//! composition — one `snapshot()`, then direct
//! `project.strokes` writes (the delete removes the
//! highest indices first, so each removal leaves the
//! remaining targets' indices valid) — and a frame
//! carrying no strokes takes no snapshot, only a
//! status report. The statuses name the frame,
//! 1-based as everywhere in this panel:
//! `Recoloured {n} strokes on frame {f}.` /
//! `Deleted {n} strokes on frame {f}.`
//!
//! Selection hygiene differs from the project-wide
//! delete, on purpose. `delete_all_strokes` empties
//! the whole list, so any selection it orphans is
//! *stale* — and stale selections are treated as unset
//! everywhere in this panel. A frame delete instead
//! leaves survivors whose indices shift down past the
//! removed strokes, so an untouched selection would
//! silently point at a **different live stroke** and
//! the stage would highlight the wrong one. The
//! delete press therefore runs through
//! [`delete_frame_strokes_press`], which remaps the
//! selection with [`selection_after_delete`]: cleared
//! when its stroke was removed, shifted when its
//! stroke survived.
//!
//! ## Selected stroke width (Phase K, section 24)
//!
//! Until this phase a recorded stroke's `width` was
//! fixed at draw time: the brush steppers above choose
//! the preset a *future* stroke is drawn with, and no
//! op rewrote an existing stroke's width. A row under
//! the measure line (key `stroke-width-row`, rendered
//! only while a valid selection exists) closes that:
//! [`set_selected_stroke_width`] steps the selected
//! stroke's width by [`STROKE_WIDTH_STEP`] (0.5),
//! clamped to [0.5, 40.0] by the pure [`width_after`].
//! The composition is the Phase F bulk discipline —
//! one `core.snapshot()`, then a direct
//! `project.strokes` write — so a press is exactly
//! one undo step. A non-empty `widths` taper scales
//! by the same ratio (new / old), so the stroke keeps
//! its shape and only grows or shrinks; the scaling
//! is skipped when the old width is zero, negative or
//! non-finite, where a ratio would be meaningless or
//! destructive, and the base width is still written.
//! A non-finite stored width is recovered from
//! [`DEFAULT_STROKE_WIDTH`] (4.0 — the default brush's
//! width, the first `brush_library` entry, which is
//! what a fresh stroke is recorded with) before the
//! step applies. With no selection, or a stale one,
//! the press is a silent no-op — no snapshot, no
//! status — like the readout helpers: the row is not
//! even rendered then, so a press can only go stale
//! if the project changed under it.
//!
//! ## Selected stroke simplify (Phase L, section 22)
//!
//! A **Simplify** button (key `stroke-simplify`,
//! `wand-magic-sparkles` glyph — the same glyph the
//! Draw panel's frame-wide Simplify carries) under
//! the width row (Phase M's opacity row now sits
//! between them), rendered under the same
//! valid-selection condition, runs Douglas-Peucker
//! over the one selected stroke through the engine
//! op `draw_ops::simplify_stroke_at` (Phase L,
//! section 21) at [`SIMPLIFY_EPSILON`]. The op takes
//! no snapshot — the caller snapshots — so the press
//! probes first with the same algorithm's index form,
//! `draw_ops::douglas_peucker_indices`: when the
//! probe would keep every point, the stroke is
//! already simple at this epsilon and the press only
//! reports that (`Stroke {n} is already simple.`),
//! taking no snapshot and adding no undo step for a
//! change that does not exist. Otherwise exactly one
//! `core.snapshot()` precedes the op, and the status
//! reports the surviving count (`Stroke {n}
//! simplified to {m} points.`) — one press, one undo
//! step, like the width steppers. With no selection,
//! or a stale one, the press is a silent no-op, the
//! width press's discipline: the button is not even
//! rendered then.
//!
//! ## Selected stroke opacity (Phase M, section 27)
//!
//! Until this phase a recorded stroke's `opacity`
//! (`Stroke.opacity`, model.rs — recorded from the
//! brush at draw time; projects saved before the
//! field existed deserialize as 1.0) was fixed at
//! draw time, exactly as its `width` was until Phase
//! K. A row under the width row (key
//! `stroke-opacity-row`, so the Simplify button now
//! sits under this row) closes that, following the
//! width row's pattern exactly: the pure
//! [`opacity_after`] steps by [`OPACITY_STEP`] (0.1)
//! clamped to [[`MIN_STROKE_OPACITY`],
//! [`MAX_STROKE_OPACITY`]]; the press
//! ([`set_selected_stroke_opacity`]) is one
//! `core.snapshot()` then a direct `project.strokes`
//! write — one undo step — and the status names the
//! stroke and its new opacity as a percent
//! (`Stroke {n} opacity {p}.`, `{p}` rendered by
//! [`opacity_label`]). Unlike width there is no
//! per-point taper to rescale: opacity is a single
//! scalar on the stroke. With no selection, or a
//! stale one, the press is a silent no-op and the
//! row is not rendered — the width press's
//! discipline, unchanged.
//!
//! The floor is [`MIN_STROKE_OPACITY`] (0.05), not
//! 0.0: a fully invisible stroke is a trap — it
//! still exists in the project, still exports as
//! nothing visible, and cannot be seen to be
//! selected on the stage — so the stepper stops at
//! a faint-but-visible 5%, and a stroke can never
//! be faded past the point where its owner can
//! still find it.
//!
//! Spray strokes are refused. A spray stroke's
//! visible opacity lives on its spray mark
//! (`SprayMark.opacity` — the value its dots render
//! with, and the one [`stroke_summary`] reports);
//! the stroke's own `opacity` field is recorded 1.0
//! for a spray stroke and plays no part in how its
//! dots render, so stepping it here would be a fake
//! control. The row is therefore absent for a spray
//! selection — the width row's treatment of its own
//! inapplicable case: no valid selection renders no
//! row — and the press refuses silently too.
//! [`selected_stroke_opacity`] is the shared gate:
//! it yields the value only for a valid selection
//! on a non-spray stroke, and both the row and the
//! press read it (the press restates the same guard
//! inline, as the width pair does).
//!
//! ## Selected stroke scatter (Wave 12, agent D3)
//!
//! The audit that opened this section: every `Stroke`
//! field (model.rs:495) against what this panel shows.
//! Points, frame, colour, width, fill presence and the
//! spray mark were all visible (measure line, list
//! swatch, Width row, summary clauses) and the Width /
//! Opacity rows read the stroke's stored values, never
//! the brush's — but `Stroke.scatter` (model.rs:525,
//! recorded from the brush by the stamp path,
//! editor.rs:3247) appeared NOWHERE for a selected
//! stroke: no row, no summary clause. The value is
//! real — every renderer spreads an ordinary stroke's
//! dabs by it (`paint::stroke_dabs`) — and MCP could
//! already rewrite it (`draw_ops::tool_set_stroke_style`),
//! so the panel was the only surface that could neither
//! see nor change it. A row under the opacity row (key
//! `stroke-scatter-row`) closes that, following the
//! opacity row's pattern and discipline exactly: the
//! pure [`scatter_after`] steps by [`SCATTER_STEP`]
//! (1.0) clamped to [[`MIN_STROKE_SCATTER`],
//! [`MAX_STROKE_SCATTER`]]; the press
//! ([`set_selected_stroke_scatter`]) is one
//! `core.snapshot()` then a direct `project.strokes`
//! write — one undo step — and the status names the
//! stroke and its new scatter in the measure voice
//! (`Stroke {n} scatter {v}.`, `{v}` rendered by
//! [`scatter_text`], so 0 reads `Off`).
//!
//! Spray strokes are refused, on the MCP op's own
//! reasoning (`tool_set_stroke_style` refuses scatter
//! writes to spray strokes in the same words): a spray
//! stroke renders from its spray mark and never reaches
//! the scatter dab path, so its stored `scatter` —
//! whatever a loaded file carries there — moves nothing
//! on screen, and stepping it here would be a fake
//! control. The row is therefore absent for a spray
//! selection and the press refuses silently too.
//! [`selected_stroke_scatter`] is the shared gate, in
//! [`selected_stroke_opacity`]'s shape. The maximum,
//! 24.0, is twice the broadest scatter any brush records
//! (Spray, 12.0) — the width row's reasoning (its 40.0
//! cap sits above the broadest brush width) applied to
//! scatter — so an existing stroke can always be
//! scattered more broadly than any brush draws it,
//! without runaway values from a corrupt file; a
//! non-finite stored scatter recovers from
//! [`DEFAULT_STROKE_SCATTER`] (0.0, the model's serde
//! default for the field) before the step applies.
//!
//! The same audit gave the per-point `widths` taper a
//! summary clause ([`stroke_summary`] now reports
//! `taper` when the vec is non-empty): the taper is
//! real model data the nib profiling writes at draw
//! time, and until now nothing in this panel admitted
//! it existed. It stays display-only here — no op
//! edits a taper directly; the Width press rescales it
//! (Phase K) and Simplify re-gathers it (Phase L).
//!
//! `State` access: the brush and stroke data read
//! `s.core` (active brush, `brush_index`,
//! `project.strokes`, `current`) and writes go through
//! the `brush_index` field write, the frame-clear op,
//! `delete_stroke_at`, the width and opacity ops
//! below or the bulk composition above;
//! the selection row additionally reads and writes
//! `s.stroke_selected`, declared in main.rs — no field
//! is declared in this file.
//!
//! ## Icons (Phase H, section 13)
//!
//! The bulk row and the selection row render through
//! the shell's icon buttons (`rune_sec::icon_btn`):
//! Recolour all carries `palette`, Delete all
//! `trash-can`, Go to frame `crosshairs` (each with its
//! text label, as before), while Prev / Next / Clear
//! are icon-only (`chevron-left` / `chevron-right` /
//! `xmark`) — their labels were pure navigation glyphs
//! in text form. Element keys are unchanged
//! (`stroke-recolor-all`, `stroke-delete-all`,
//! `stroke-sel-prev` / `-next` / `-goto` / `-clear`),
//! and the handlers are the same presses: the selection
//! buttons still write nothing but `s.stroke_selected`
//! (plus `goto`'s navigation), exactly as documented
//! above. Phase J's frame row reuses `palette` /
//! `trash-can` under the keys `stroke-recolor-frame` /
//! `stroke-delete-frame`, in the row keyed
//! `stroke-bulk-frame-row`.
//!
//! ## Phase N adoption
//!
//! The panel root is the kit card
//! (`design::panel_col`, key `stroke-inspector`
//! unchanged) with a `design::panel_title` header
//! ("Stroke inspector" — the panel carried no title
//! before this phase) over a `design::divider`. The
//! brush parameter rows ([`param_row`]) and the
//! selected-stroke Width / Opacity rows are rebuilt in
//! the charter's stepper order: the label grows, then
//! the − button, the value as `design::stepper_value`,
//! and the + button; the brush rows' − glyph is now
//! U+2212, and the Width / Opacity rows keep their
//! chevron steppers (the F-STEP-CHEV variant) with
//! only the value readout kitted. The nib segmented
//! row takes the segmented gap (4). The two in-panel
//! section headings — `Brush preset · {name}` (the
//! `preset` wording is the Wave 4 I5(b) relabel) and the stroke
//! list's `strokes-header` text — render through
//! `design::section_label`. The stroke rows' 14×14
//! colour chips are display readouts, not swatch
//! controls (p09 §2.4), and stay as they are; the
//! bulk rows hold no swatches at all — Recolour is a
//! button press against [`bulk_target_color`]. Keys,
//! handlers and status strings are unchanged.
//!
//! ## Wave 9 polish (agent B5)
//!
//! Three fixes, all inside this file. (1) The per-row
//! Delete and the header's Clear frame were the two
//! removal paths that never remapped the selection —
//! the frame delete has since Phase J, for the reason
//! stated there — so both now run through presses
//! ([`delete_stroke_press`],
//! [`clear_frame_strokes_press`]) that apply
//! [`selection_after_delete`] after the removal. (2)
//! The selected stroke's list row renders on the
//! kit's `design::row_shell` in its selected form —
//! the list itself now shows the selection the
//! readout names. (3) Row widths re-proven against
//! the 316 pt content budget from the headless tree
//! (draw workspace): the nib row's fixed children sum
//! to 273 pt (43 pt slack), the stepper rows' fixed
//! parts to 115 pt with the label flexing, and the
//! Width / Opacity rows share that construction
//! (fixed 110 pt at the value box's 34 pt minimum).
//! The frame-strokes empty state is the kit
//! `design::empty_state` (Wave 6 conversion, verified
//! in place) and its message names the frame scope
//! the list actually filters by.
//!
//! ## Wave 10 round-2 audit (agent C6)
//!
//! A second pass over the selection's hygiene, the
//! rows' widths and the list's growth, hunting what
//! round 1 (B5, above) did not cover.
//!
//! **Widths — two growing texts did not flex.** The
//! selection readout and the per-stroke summary are
//! both `text(...).grow(1.0)` in rows with fixed
//! siblings, and in Rune only a `.truncate()`d text
//! flexes (the status strip's documented semantics:
//! flex_shrink 1, minimum width 0, elision) — a plain
//! growing text paints at full width and pushes its
//! siblings past the card's right edge. The readout's
//! row leaves it ≈101 pt of the 316 pt content
//! column (three 26 pt icon buttons, the ≈105 pt
//! Go to frame button, four 8 pt gaps), and the
//! shortest live readout — "No stroke selected ·
//! 1 stroke" — is already wider; a selected readout
//! ("Stroke 118 of 120 · frame 60") wider still. The
//! summary's room is ≈187 pt, and an ordinary filled
//! spray summary is 42 chars ("2048 pts · w 24 ·
//! filled · spray r12 · 50%"). Both texts now carry
//! `.truncate()` — the clips / audio / figures
//! panels' idiom for the same shape — so the buttons
//! keep their places and elision stays display-only:
//! [`stroke_summary`] and [`selection_readout`]
//! still return the full strings (pinned by tests).
//! The state was invisible to the overflow scan for
//! the usual reason: no headless scene gives the
//! demo project strokes, so neither row has ever
//! been in a dumped tree with live data. The panel's
//! other rows re-check clean at the same budget:
//! the bulk rows' labels are fixed literals beside
//! fixed buttons (≈297 pt worst), the header needs
//! ten combined digits of frame + count to cross,
//! and the measure / hint lines are column-flow
//! prose, which wraps.
//!
//! **Selection hygiene — every other mutation path.**
//! Reorder does not exist: the rows carry no move
//! control, and no code anywhere swaps, sorts or
//! inserts into `Project.strokes` (full-tree grep:
//! the vec's only mutations are the editor's
//! remove / retain / clear and the draw path's
//! appends, which never shift an existing index).
//! Simplify mutates the selected stroke's points in
//! place at its own index. Frame navigation
//! (`EditorCore::goto`) writes only the playhead,
//! and frame deletion (`EditorCore::delete_frame`)
//! touches only `project.frames` — the strokes vec
//! and the `Stroke.frame` values are never
//! renumbered, so an index keeps naming its stroke
//! (its strokes are not removed either; which frame
//! they render on after a deletion is the engine's
//! standing behaviour, not a selection hazard). The
//! Delete key, the Edit menu and the palette all
//! delete the selected stroke itself and clear the
//! selection; the palette's `delete-last-stroke`
//! removes only the tail index, which can stale a
//! selection (readers treat stale as unset) but
//! never mispoint it. The engine's `lasso_erase`
//! retain-removal is unreachable — no shell or MCP
//! caller exists — but any future caller must remap
//! like the presses here.
//!
//! **The one proven hole is project load, and its
//! fix does not live in this file.** Every
//! session-replacing press (Open / New / Recover /
//! Reload, in both the Project section and Home)
//! swaps `core.project` wholesale —
//! `open_project_file` assigns the loaded project,
//! Home's New installs `Project::new_demo` — while
//! `State.stroke_selected` is shell state the core
//! cannot clear. The presses curate the rest of the
//! session (`clear_stale_solo`, `sync_name_edit`,
//! the core's own figure / node / solo resets), but
//! nothing clears the stroke selection, so a
//! surviving `Some(i)` inside the new project's
//! range silently names a stroke the user never
//! selected there — the stage highlights it, the
//! Width / Opacity / Simplify presses edit it, the
//! Delete key deletes it. The B5 hazard through the
//! load door. The fix is one line per press —
//! `s.stroke_selected = None` after a successful
//! swap, the `clear_stale_solo` discipline — in
//! `project_tools` and `home_panel`; it is recorded
//! here, where the selection's contract lives, for
//! the integrator to place. Undo across a swap
//! shares the mechanism (the undo history survives
//! an open by design) and wants the same look.
//!
//! **Simplify status — verified honest.** The press
//! reports the APPLIED count: it reads
//! `project.strokes[i].points.len()` back from the
//! mutated stroke after `simplify_stroke_at`, not
//! the probe's kept count (pinned by
//! `simplify_selected_reduces_a_jittery_stroke_in_one_undo_step`).
//! Probe and op call the same deterministic
//! `douglas_peucker_indices` over unchanged points,
//! and the op refuses under exactly the probe's
//! no-op condition, so the press never snapshots a
//! change the op then declines; the op also gathers
//! an exactly-parallel `widths` taper at the kept
//! indices, so a simplified stroke's taper stays
//! parallel to its points.
//!
//! **List growth — clean.** The stroke list is the
//! panel's last block and the side columns scroll
//! whole (main.rs mounts them `.scroll(...)`), so
//! more rows only lengthen the column's scroll —
//! nothing inside the panel is displaced, the case
//! that forced the graph channel list's bound. The
//! shell's other per-item lists (figures, layers,
//! clips) are unbounded the same way.
//!
//! ## Wave U i18n (agent U1)
//!
//! Every label and composed readout in this panel
//! resolves through `i18n_wire::tr` (the subtitle /
//! camera / 3D / Wave T idiom): the panel title, the
//! bulk / frame / selection row labels and button
//! labels, the selected-stroke row captions and
//! stepper help labels, the preset rows and nib
//! segmented control, the empty state, and — as
//! `{slot}` template keys substituted with
//! `.replace()` at the composing site (the camera
//! `follow_label` precedent) — the composed readouts:
//! [`tool_text`], [`strokes_header`],
//! [`stroke_summary`] (base + clause fragments),
//! [`selection_readout`], [`measure_readout`],
//! [`spray_hint`], [`nib_text`], the `Frame {n}` row
//! label, the `Brush preset · {name}` heading and the
//! `Preset nib angle` line. The value formatters
//! [`scatter_text`] / [`smoothing_text`] /
//! [`nib_label`] gained an `&EditorCore` parameter for
//! the lookup (the T2 `mute_toggle_parts` precedent).
//! English is byte-identical once IA-U1-1 lands; until
//! then `tr` falls back to the key itself (the Q7
//! pattern — the pin test below says so). Status
//! sentences stay English (they are handler feedback,
//! not labels — including the `Stroke {n} scatter
//! {v}.` status, whose sentence frame stays English
//! while its `{v}` comes from the shared readout
//! formatter [`scatter_text`]), as do the pure number
//! formats ([`width_text`], [`opacity_text`],
//! [`opacity_label`], the `{w:.1}` stepper value) and
//! the notation glyphs (`−` / `+` steppers, the `#n`
//! row position).

use rune::tok::{type_, Theme};
use rune::{el, row, text, El};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::model::Stroke;
use stykanimate_rs::paint::{Brush, Nib};

use crate::chrome_view::btn;
use crate::rune_sec::design;
use crate::rune_sec::i18n_wire;
use crate::rune_sec::icon_btn::{icon_btn, icon_only_btn};
use crate::stage_view::hex_color;
use crate::State;

/// The three nib kinds, in segmented-control display order.
pub const NIB_ORDER: [Nib; 3] = [Nib::Round, Nib::Square, Nib::Calligraphy];

/// A brush parameter a stepper can step through the preset
/// library by. The parameters are the preset-bundle fields
/// of `paint::Brush` (paint.rs:304).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Param {
    Size,
    Opacity,
    Scatter,
    Smoothing,
}

impl Param {
    /// This parameter's value on a preset.
    pub fn value(self, b: &Brush) -> f32 {
        match self {
            Param::Size => b.width,
            Param::Opacity => b.opacity,
            Param::Scatter => b.scatter,
            Param::Smoothing => b.smoothing as f32,
        }
    }

    /// Status text when no preset steps in direction `dir`
    /// (the stepper press is then a no-op apart from this
    /// honest report).
    pub fn no_step_message(self, dir: i32) -> &'static str {
        match (self, dir > 0) {
            (Param::Size, true) => "No wider brush in the library.",
            (Param::Size, false) => "No narrower brush in the library.",
            (Param::Opacity, true) => "No more opaque brush in the library.",
            (Param::Opacity, false) => "No fainter brush in the library.",
            (Param::Scatter, true) => "No broader-scatter brush in the library.",
            (Param::Scatter, false) => "No tighter-scatter brush in the library.",
            (Param::Smoothing, true) => "No smoother brush in the library.",
            (Param::Smoothing, false) => "No rougher brush in the library.",
        }
    }
}

/// The active brush's library index, clamped exactly as
/// `EditorCore::active_brush` clamps it, so a stale
/// `brush_index` can never point outside the library.
pub fn active_index(core: &EditorCore) -> usize {
    let len = stykanimate_rs::paint::brush_library().len();
    if len == 0 {
        return 0;
    }
    core.brush_index.min(len - 1)
}

/// Step through a preset library by one parameter: from
/// `current`, the index whose `key` value is nearest in
/// direction `dir` (+1 = strictly greater, −1 = strictly
/// smaller; smallest absolute difference wins, ties go to
/// the lowest index). `None` when no preset steps that way,
/// the library is empty, `current` is out of range, or
/// `dir` is 0.
pub fn step_index_by(
    lib: &[Brush],
    current: usize,
    dir: i32,
    key: impl Fn(&Brush) -> f32,
) -> Option<usize> {
    if lib.is_empty() || current >= lib.len() || dir == 0 {
        return None;
    }
    let cur = key(&lib[current]);
    let mut best: Option<(f32, usize)> = None;
    for (i, b) in lib.iter().enumerate() {
        if i == current {
            continue;
        }
        let v = key(b);
        if (dir > 0 && v <= cur) || (dir < 0 && v >= cur) {
            continue;
        }
        let d = (v - cur).abs();
        match best {
            Some((bd, _)) if d >= bd => {}
            _ => best = Some((d, i)),
        }
    }
    best.map(|(_, i)| i)
}

/// The first library preset carrying `nib` — the nib
/// segmented control's selection target (Round → Pen,
/// Square → Brush, Calligraphy → Calligraphy).
pub fn first_index_with_nib(lib: &[Brush], nib: Nib) -> Option<usize> {
    lib.iter().position(|b| b.nib == nib)
}

/// Select preset `i`: the public `brush_index` field write
/// (clamped) plus the exact status string `cycle_brush`
/// sets — the same selection the engine's own tests perform
/// when they write the field directly.
fn apply_preset(core: &mut EditorCore, i: usize) {
    let lib = stykanimate_rs::paint::brush_library();
    if lib.is_empty() {
        return;
    }
    core.brush_index = i.min(lib.len() - 1);
    let b = lib[core.brush_index];
    core.status = format!("Brush: {} (width {:.0}).", b.name, b.width);
}

/// Stepper press: select the nearest preset stepping
/// parameter `param` in direction `dir`. Returns whether
/// the selection moved; when it cannot, the status line
/// says so (see [`Param::no_step_message`]).
pub fn step_param(core: &mut EditorCore, param: Param, dir: i32) -> bool {
    let lib = stykanimate_rs::paint::brush_library();
    let cur = active_index(core);
    match step_index_by(&lib, cur, dir, |b| param.value(b)) {
        Some(i) => {
            apply_preset(core, i);
            true
        }
        None => {
            core.status = param.no_step_message(dir).into();
            false
        }
    }
}

/// Nib segmented press: select the first preset carrying
/// `nib`. Returns whether a preset was selected (every nib
/// in [`NIB_ORDER`] exists in the shipped library, so this
/// is always true in practice).
pub fn select_nib(core: &mut EditorCore, nib: Nib) -> bool {
    let lib = stykanimate_rs::paint::brush_library();
    match first_index_with_nib(&lib, nib) {
        Some(i) => {
            apply_preset(core, i);
            true
        }
        None => false,
    }
}

/// A number the way the panel shows sizes and radii: whole
/// values without a fraction (`4`), others with one decimal
/// (`2.5`) — the library's widths are all whole, but loaded
/// data need not be.
pub fn width_text(w: f32) -> String {
    if w.fract() == 0.0 {
        format!("{}", w as i64)
    } else {
        format!("{w:.1}")
    }
}

/// Opacity as a percent (`100%`, `50%`).
pub fn opacity_text(opacity: f32) -> String {
    format!("{}%", (opacity * 100.0).round() as i64)
}

/// Scatter radius: `Off` at 0 (marks land exactly on the
/// recorded path), otherwise the radius in stage units.
/// The `Off` word resolves through the catalogue
/// (Wave U, U1); the radius itself is notation.
pub fn scatter_text(core: &EditorCore, scatter: f32) -> String {
    if scatter <= 0.0 {
        i18n_wire::tr(core, "stroke_off")
    } else {
        width_text(scatter)
    }
}

/// Short display label for a nib kind — display only:
/// nothing parses these names back (the nib itself is
/// the `paint::Nib` enum wherever it is stored or
/// matched), so they resolve through the catalogue
/// (Wave U, U1).
pub fn nib_label(core: &EditorCore, nib: Nib) -> String {
    i18n_wire::tr(
        core,
        match nib {
            Nib::Round => "stroke_nib_round",
            Nib::Square => "stroke_nib_square",
            Nib::Calligraphy => "stroke_nib_calligraphy",
        },
    )
}

/// The segmented control's element key for a nib.
pub fn nib_key(nib: Nib) -> &'static str {
    match nib {
        Nib::Round => "brush-nib-round",
        Nib::Square => "brush-nib-square",
        Nib::Calligraphy => "brush-nib-calligraphy",
    }
}

/// A preset's nib display: the kind, plus the edge angle
/// when it is non-zero (a round nib's angle is meaningless
/// and several presets leave it 0 — `Square`, not
/// `Square · 0°`).
pub fn nib_text(core: &EditorCore, brush: &Brush) -> String {
    if brush.nib == Nib::Round || brush.nib_angle == 0.0 {
        nib_label(core, brush.nib)
    } else {
        i18n_wire::tr(core, "stroke_nib_with_angle")
            .replace("{nib}", &nib_label(core, brush.nib))
            .replace("{angle}", &format!("{:.0}", brush.nib_angle))
    }
}

/// Smoothing display: `Off` at 0 Chaikin iterations, else
/// the pass count (singular / plural catalogue forms,
/// Wave U, U1).
pub fn smoothing_text(core: &EditorCore, smoothing: usize) -> String {
    match smoothing {
        0 => i18n_wire::tr(core, "stroke_off"),
        1 => i18n_wire::tr(core, "stroke_smoothing_one"),
        n => i18n_wire::tr(core, "stroke_smoothing_many")
            .replace("{count}", &n.to_string()),
    }
}

/// One-line explainer shown when the active brush scatters
/// (today only Spray): the mark a stroke records, in the
/// model's own terms (`model::SprayMark`, model.rs:473 —
/// the seed is per-stroke, the radius and opacity come from
/// the brush, editor.rs:2303).
pub fn spray_hint(core: &EditorCore, brush: &Brush) -> Option<String> {
    if brush.scatter <= 0.0 {
        return None;
    }
    Some(
        i18n_wire::tr(core, "stroke_spray_hint")
            .replace("{radius}", &width_text(brush.scatter))
            .replace("{opacity}", &opacity_text(brush.opacity)),
    )
}

/// The draw-tool readout: which tool is active and which
/// preset sizes it (the preset "applies to whichever tool
/// is active" — it also sizes the eraser, editor.rs:2573).
pub fn tool_text(core: &EditorCore) -> String {
    let brush = core.active_brush();
    if core.draw_tool_is_eraser() {
        i18n_wire::tr(core, "stroke_tool_eraser").replace("{name}", brush.name)
    } else {
        i18n_wire::tr(core, "stroke_tool_brush").replace("{name}", brush.name)
    }
}

/// Indices into `Project.strokes` of the strokes recorded
/// on the current frame, in project order.
pub fn frame_stroke_indices(core: &EditorCore) -> Vec<usize> {
    core.project
        .strokes
        .iter()
        .enumerate()
        .filter(|(_, s)| s.frame == core.current)
        .map(|(i, _)| i)
        .collect()
}

/// How many strokes the current frame carries.
pub fn frame_stroke_count(core: &EditorCore) -> usize {
    core.project
        .strokes
        .iter()
        .filter(|s| s.frame == core.current)
        .count()
}

/// The stroke-list header: `Strokes · Frame {n} · {count}`.
pub fn strokes_header(core: &EditorCore) -> String {
    i18n_wire::tr(core, "stroke_strokes_header")
        .replace("{frame}", &(core.current + 1).to_string())
        .replace("{count}", &frame_stroke_count(core).to_string())
}

/// One stroke row's summary: point count and base width,
/// plus `taper` when the stroke carries a per-point
/// `widths` profile, `filled` when it carries a fill
/// colour, and either the spray mark's radius/opacity
/// when it carries one or — for an ordinary stroke —
/// its `scatter` radius when scatter is on (`Stroke`,
/// model.rs:495). The two radius clauses never appear
/// together: a spray stroke renders from its mark and
/// never reaches the scatter dab path, so its stored
/// `scatter` is not reported (Wave 12, agent D3 — the
/// same refusal [`selected_stroke_scatter`] applies).
pub fn stroke_summary(core: &EditorCore, stroke: &Stroke) -> String {
    let mut out = i18n_wire::tr(core, "stroke_summary_base")
        .replace("{points}", &stroke.points.len().to_string())
        .replace("{width}", &width_text(stroke.width));
    if !stroke.widths.is_empty() {
        out.push_str(&i18n_wire::tr(core, "stroke_summary_taper"));
    }
    if stroke.fill.is_some() {
        out.push_str(&i18n_wire::tr(core, "stroke_summary_filled"));
    }
    if let Some(mark) = &stroke.spray {
        out.push_str(
            &i18n_wire::tr(core, "stroke_summary_spray")
                .replace("{radius}", &width_text(mark.radius))
                .replace("{opacity}", &opacity_text(mark.opacity)),
        );
    } else if stroke.scatter > 0.0 {
        out.push_str(
            &i18n_wire::tr(core, "stroke_summary_scatter")
                .replace("{radius}", &width_text(stroke.scatter)),
        );
    }
    out
}

/// One stroke-list row's data. The list is filtered to the
/// current frame, so a row's display position and its
/// stroke's index in `Project.strokes` differ whenever
/// other frames carry strokes — the Delete button must
/// target [`StrokeRow::project_index`], never the position.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StrokeRow {
    /// Index into `Project.strokes` — the delete target
    /// and the row button's key suffix.
    pub project_index: usize,
    /// 1-based position within this frame's list (the
    /// `#n` label).
    pub position: usize,
    /// The stroke's colour (swatch fill).
    pub color: u32,
    /// The one-line data summary (see [`stroke_summary`]).
    pub summary: String,
}

/// The current frame's stroke rows, in project order,
/// built over [`frame_stroke_indices`] so every row
/// carries its stroke's real `Project.strokes` index.
pub fn stroke_rows(core: &EditorCore) -> Vec<StrokeRow> {
    frame_stroke_indices(core)
        .into_iter()
        .enumerate()
        .map(|(k, idx)| {
            let stroke = &core.project.strokes[idx];
            StrokeRow {
                project_index: idx,
                position: k + 1,
                color: stroke.color,
                summary: stroke_summary(core, stroke),
            }
        })
        .collect()
}

/// Row Delete press: remove the stroke at
/// `project_index` in `Project.strokes` via the core's
/// per-stroke delete op (`EditorCore::delete_stroke_at`,
/// Phase E section 7 — snapshot, removal and status are
/// the op's business). Later strokes shift down one
/// index; the panel rebuilds its rows from the core on
/// the next view pass, so no row data is ever reused
/// across a deletion.
pub fn delete_stroke(core: &mut EditorCore, project_index: usize) {
    core.delete_stroke_at(project_index);
}

/// Row Delete press, as the list's button runs it:
/// [`delete_stroke`] on the core, then the selection
/// remap ([`selection_after_delete`] over the single
/// removed index). The Phase J frame delete remaps
/// for exactly this reason — a removal shifts every
/// later stroke down one index, so an untouched
/// selection would silently point at a **different
/// live stroke**: the stage would highlight the wrong
/// stroke, and the Width / Opacity rows would edit
/// it. The per-row delete (Phase E) predates that
/// discipline and never received it; this press is
/// the fix — same helper, same semantics (the
/// selected stroke itself deleted → selection
/// cleared; a survivor keeps its stroke). A stale
/// target (index past the end — the row cannot be
/// rendered then, so a press can only go stale if
/// the project changed under it) keeps the core
/// op's standing behaviour: it reports "No stroke
/// {n} to delete." and the selection is untouched.
pub fn delete_stroke_press(s: &mut State, project_index: usize) {
    if project_index >= s.core.project.strokes.len() {
        delete_stroke(&mut s.core, project_index);
        return;
    }
    delete_stroke(&mut s.core, project_index);
    s.stroke_selected = selection_after_delete(s.stroke_selected, &[project_index]);
}

/// Clear frame press, as the strokes header's button
/// runs it: `EditorCore::clear_frame_drawings` on the
/// core, then the same selection remap as
/// [`delete_stroke_press`], over the frame's removed
/// indices ([`frame_stroke_indices`], read **before**
/// the clear — the same set the op's retain filter
/// removes). The header button previously called the
/// core op bare, with the same wrong-stroke hazard
/// as the row Delete: the frame delete in this panel
/// remapped, the frame clear did not. An empty frame
/// removes nothing and the selection is left alone;
/// the op still runs, so its "Cleared 0 stroke(s)…"
/// report is unchanged.
pub fn clear_frame_strokes_press(s: &mut State) {
    let removed = frame_stroke_indices(&s.core);
    s.core.clear_frame_drawings();
    if !removed.is_empty() {
        s.stroke_selected = selection_after_delete(s.stroke_selected, &removed);
    }
}

/// Total strokes in the project, across all frames — the
/// size of the selection's index space. (The frame list
/// above counts only the current frame; the selection
/// deliberately does not.)
pub fn stroke_total(core: &EditorCore) -> usize {
    core.project.strokes.len()
}

/// The frame the stroke at project-wide `index` was
/// recorded on (`Stroke.frame`, model.rs:482), or `None`
/// when `index` is outside `Project.strokes`. This is
/// the mapping the selection readout and **Go to frame**
/// need: the selection lives in the project-wide space,
/// but a stroke is viewed on its own frame.
pub fn stroke_frame_of(core: &EditorCore, index: usize) -> Option<usize> {
    core.project.strokes.get(index).map(|s| s.frame)
}

/// Step a project-wide stroke selection by `dir`
/// (+1 = next, −1 = previous) over `total` strokes,
/// wrapping at both ends. A missing **or stale**
/// selection (`cur >= total`, e.g. after strokes were
/// deleted elsewhere) is treated as unset: +1 enters at
/// the first stroke, −1 at the last. `total == 0` never
/// selects anything; `dir == 0` holds a valid selection
/// and drops an unset or stale one.
pub fn step_selection(cur: Option<usize>, total: usize, dir: i32) -> Option<usize> {
    if total == 0 {
        return None;
    }
    let valid = cur.filter(|&i| i < total);
    match dir.signum() {
        1 => Some(match valid {
            Some(i) => (i + 1) % total,
            None => 0,
        }),
        -1 => Some(match valid {
            Some(i) => (i + total - 1) % total,
            None => total - 1,
        }),
        _ => valid,
    }
}

/// The selection row's readout. With a valid selection:
/// `Stroke {i+1} of {n} · frame {f+1}` — the stroke's
/// 1-based position in the project-wide space, the
/// project total, and the 1-based frame it lives on.
/// A missing or stale selection reads as no selection;
/// an empty project reads "No strokes".
pub fn selection_readout(core: &EditorCore, selected: Option<usize>) -> String {
    let total = core.project.strokes.len();
    if total == 0 {
        return i18n_wire::tr(core, "stroke_readout_empty");
    }
    match selected.filter(|&i| i < total) {
        Some(i) => {
            // `i < total`, so the stroke — and its frame —
            // exist; the fallback is unreachable.
            let frame = stroke_frame_of(core, i).unwrap_or(0);
            i18n_wire::tr(core, "stroke_readout_selected")
                .replace("{index}", &(i + 1).to_string())
                .replace("{total}", &total.to_string())
                .replace("{frame}", &(frame + 1).to_string())
        }
        None => i18n_wire::tr(
            core,
            if total == 1 {
                "stroke_readout_none_one"
            } else {
                "stroke_readout_none_many"
            },
        )
        .replace("{count}", &total.to_string()),
    }
}

/// Measured facts about one stroke: its polyline point
/// count, its total length in stage units, and the frame
/// it was recorded on. Produced by [`stroke_measure`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StrokeMeasure {
    /// Polyline point count (`Stroke.points.len()`).
    pub points: usize,
    /// Summed segment lengths along the base polyline, in
    /// stage units. A stroke with fewer than two points
    /// measures 0.
    pub length: f32,
    /// The frame the stroke was recorded on — the raw
    /// `Stroke.frame` value, as [`stroke_frame_of`]
    /// returns it (displays add 1).
    pub frame: usize,
}

/// Measure the stroke at project-wide `index`: point
/// count, total length and owning frame, or `None` when
/// `index` is outside `Project.strokes`. The length sums
/// the Euclidean lengths of the consecutive segments of
/// the stroke's **base polyline** — a spray stroke is
/// measured by that polyline too, never by its scattered
/// dots, matching the stage picking convention
/// (`stage_layers::stroke_at_point`, Phase G section 5).
/// A filled stroke is measured along its open polyline:
/// the closing edge back to the first point is not
/// counted. Pure: same project state, same measure.
pub fn stroke_measure(core: &EditorCore, index: usize) -> Option<StrokeMeasure> {
    let stroke = core.project.strokes.get(index)?;
    let length = stroke
        .points
        .windows(2)
        .map(|seg| {
            let d = [seg[1][0] - seg[0][0], seg[1][1] - seg[0][1]];
            (d[0] * d[0] + d[1] * d[1]).sqrt()
        })
        .sum();
    Some(StrokeMeasure {
        points: stroke.points.len(),
        length,
        frame: stroke_frame_of(core, index)?,
    })
}

/// The measure line shown under the selection row while
/// a valid selection exists: `Stroke {n}: {points} pts ·
/// {length} long · frame {f}` — the stroke's 1-based
/// project position, its point count, its total length
/// rounded to whole stage units, and its 1-based frame.
/// `None` — and no line is rendered — with no selection,
/// a stale one, or an empty project.
pub fn measure_readout(core: &EditorCore, selected: Option<usize>) -> Option<String> {
    let i = selected.filter(|&i| i < core.project.strokes.len())?;
    let m = stroke_measure(core, i)?;
    Some(
        i18n_wire::tr(core, "stroke_measure_line")
            .replace("{index}", &(i + 1).to_string())
            .replace("{points}", &m.points.to_string())
            .replace("{length}", &format!("{:.0}", m.length))
            .replace("{frame}", &(m.frame + 1).to_string()),
    )
}

/// One width-stepper press moves a stroke's width by
/// this much. Half a stage unit is fine enough to tune
/// a line and coarse enough that a few presses cross
/// the brush library's whole range (2.0–24.0).
pub const STROKE_WIDTH_STEP: f32 = 0.5;

/// The narrowest width the stepper writes: a stroke
/// never quite vanishes, and the value matches the
/// lower clamp the draw path's own thinnest recorded
/// widths stay above.
pub const MIN_STROKE_WIDTH: f32 = 0.5;

/// The widest width the stepper writes — above the
/// broadest brush preset (24.0), so an existing stroke
/// can always be made heavier than any brush draws it,
/// without runaway values from a corrupt file.
pub const MAX_STROKE_WIDTH: f32 = 40.0;

/// The width a non-finite stored width is recovered
/// to before a step applies: the default brush's
/// width (4.0, the first `paint::brush_library`
/// entry — "the editor's default", paint.rs), which
/// is what `begin_draw` records a fresh stroke with.
pub const DEFAULT_STROKE_WIDTH: f32 = 4.0;

/// Douglas-Peucker epsilon (stage units) for the
/// selected stroke's Simplify press — the same value
/// the Draw panel's frame-wide Simplify uses
/// (`draw_tools::SIMPLIFY_EPSILON`, Phase H): hand-drawn
/// strokes record points only when the pointer has
/// moved more than 2 units (`EditorCore::draw_to`), so
/// an epsilon of 2.0 removes sub-recording jitter
/// without eating deliberate corners at typical stroke
/// scales. Both panels simplifying at one tolerance
/// keeps the gesture predictable: a stroke the frame
/// press would leave alone, this press leaves alone.
pub const SIMPLIFY_EPSILON: f32 = 2.0;

/// The width after one stepper press in direction
/// `dir` (−1 / +1; larger magnitudes step further):
/// `current` moved by `dir` × [`STROKE_WIDTH_STEP`]
/// and clamped to [[`MIN_STROKE_WIDTH`],
/// [`MAX_STROKE_WIDTH`]]. A non-finite `current`
/// (a corrupt or hand-edited project file) is first
/// recovered to [`DEFAULT_STROKE_WIDTH`], so the
/// press always lands on a real, usable width. Pure.
pub fn width_after(current: f32, dir: i32) -> f32 {
    let base = if current.is_finite() {
        current
    } else {
        DEFAULT_STROKE_WIDTH
    };
    (base + dir as f32 * STROKE_WIDTH_STEP).clamp(MIN_STROKE_WIDTH, MAX_STROKE_WIDTH)
}

/// The selected stroke's current width, or `None`
/// with no (or a stale) selection — the width row's
/// render condition and value source, in the same
/// guard shape as [`measure_readout`].
pub fn selected_stroke_width(core: &EditorCore, selected: Option<usize>) -> Option<f32> {
    let i = selected.filter(|&i| i < core.project.strokes.len())?;
    Some(core.project.strokes[i].width)
}

/// Width stepper press: step the selected stroke's
/// recorded `width` by [`width_after`]. The write is
/// panel-composed in the Phase F bulk discipline —
/// exactly one `core.snapshot()`, then a direct
/// `project.strokes` write — so the press is one undo
/// step. A non-empty per-point `widths` taper scales
/// by the same ratio (new / old), preserving the
/// taper's shape; the scaling is skipped when the old
/// width is not a positive finite number (zero,
/// negative or non-finite), where the ratio would
/// divide by zero or poison every entry — the base
/// width is still written. With no selection, or a
/// stale one, the press is a silent no-op: `false`,
/// no snapshot, no status. On success the status
/// names the stroke and its new width, in the
/// measure line's voice: `Stroke {n} width {w}.`
pub fn set_selected_stroke_width(
    core: &mut EditorCore,
    selected: Option<usize>,
    dir: i32,
) -> bool {
    let Some(i) = selected.filter(|&i| i < core.project.strokes.len()) else {
        return false;
    };
    let old = core.project.strokes[i].width;
    let new = width_after(old, dir);
    core.snapshot_labeled("Stroke width");
    let stroke = &mut core.project.strokes[i];
    stroke.width = new;
    if old.is_finite() && old > 0.0 && !stroke.widths.is_empty() {
        let scale = new / old;
        for w in &mut stroke.widths {
            *w *= scale;
        }
    }
    core.status = format!("Stroke {} width {:.1}.", i + 1, new);
    true
}

/// One opacity-stepper press moves a stroke's
/// opacity by this much: a tenth of the full range —
/// fine enough to fade a stroke in stages, coarse
/// enough that the whole range is ten presses.
pub const OPACITY_STEP: f32 = 0.1;

/// The faintest opacity the stepper writes. The
/// floor is 0.05, not 0.0, on purpose: a fully
/// invisible stroke is a trap — it still exists in
/// the project, still exports as nothing visible,
/// and cannot be seen to be selected on the stage —
/// so the stepper stops at a faint-but-visible 5%,
/// and a stroke can never be faded past the point
/// where its owner can still find it.
pub const MIN_STROKE_OPACITY: f32 = 0.05;

/// The most opaque value the stepper writes: fully
/// opaque, the model's own ceiling — also the value
/// strokes recorded before the field existed carry.
pub const MAX_STROKE_OPACITY: f32 = 1.0;

/// The opacity a non-finite stored opacity is
/// recovered to before a step applies: 1.0, the
/// model's serde default for the field
/// (`default_stroke_opacity`, model.rs) — a corrupt
/// value steps from fully opaque, never from itself.
pub const DEFAULT_STROKE_OPACITY: f32 = 1.0;

/// The opacity after one stepper press in direction
/// `dir` (−1 / +1; larger magnitudes step further):
/// `current` moved by `dir` × [`OPACITY_STEP`] and
/// clamped to [[`MIN_STROKE_OPACITY`],
/// [`MAX_STROKE_OPACITY`]] — [`width_after`]'s shape
/// applied to opacity. A non-finite `current` (a
/// corrupt or hand-edited project file) is first
/// recovered to [`DEFAULT_STROKE_OPACITY`], so the
/// press always lands on a real, usable opacity.
/// Pure.
pub fn opacity_after(current: f32, dir: i32) -> f32 {
    let base = if current.is_finite() {
        current
    } else {
        DEFAULT_STROKE_OPACITY
    };
    (base + dir as f32 * OPACITY_STEP).clamp(MIN_STROKE_OPACITY, MAX_STROKE_OPACITY)
}

/// A stroke opacity as a percent (`55%`). The
/// computation is the codebase's standing one —
/// `(opacity * 100.0).round()`, as this panel's own
/// [`opacity_text`] renders brush opacities and
/// `layers_panel::opacity_label` renders layer
/// opacities — and `f32::round` breaks ties away
/// from zero, so a stored 0.575 (whose × 100 lands
/// on exactly 57.5 in f32) reads `58%`. The opacity
/// row's readout and the press's status share this
/// one formatter.
pub fn opacity_label(opacity: f32) -> String {
    format!("{}%", (opacity * 100.0).round() as i64)
}

/// The selected stroke's current opacity, or `None`
/// when there is nothing the opacity steppers may
/// act on: no (or a stale) selection — the width
/// row's render condition, in [`selected_stroke_width`]'s
/// guard shape — or the selected stroke is a **spray**
/// stroke. A spray stroke's visible opacity lives on
/// its spray mark (`SprayMark.opacity`, the value
/// its dots render with); the stroke's own `opacity`
/// is recorded 1.0 there and unused, so stepping it
/// would change nothing on screen — the steppers
/// refuse spray strokes rather than pretend. This
/// is the opacity row's render condition and value
/// source; the press restates the same guard inline,
/// as the width pair does, so the row and the press
/// can never disagree.
pub fn selected_stroke_opacity(core: &EditorCore, selected: Option<usize>) -> Option<f32> {
    let i = selected.filter(|&i| i < core.project.strokes.len())?;
    let stroke = &core.project.strokes[i];
    if stroke.spray.is_some() {
        return None;
    }
    Some(stroke.opacity)
}

/// Opacity stepper press: step the selected stroke's
/// recorded `opacity` by [`opacity_after`]. The
/// write is the width press's composition — exactly
/// one `core.snapshot()`, then a direct
/// `project.strokes` write — so the press is one
/// undo step; unlike width there is no per-point
/// taper to rescale, because opacity is a single
/// scalar on the stroke. With no selection, a stale
/// one, or a spray stroke selected, the press is a
/// silent no-op: `false`, no snapshot, no status
/// (see [`selected_stroke_opacity`] for the spray
/// rule). On success the status names the stroke and
/// its new opacity, in the measure line's voice:
/// `Stroke {n} opacity {p}.`, with `{p}` from
/// [`opacity_label`].
pub fn set_selected_stroke_opacity(
    core: &mut EditorCore,
    selected: Option<usize>,
    dir: i32,
) -> bool {
    let Some(i) = selected.filter(|&i| i < core.project.strokes.len()) else {
        return false;
    };
    if core.project.strokes[i].spray.is_some() {
        return false;
    }
    let new = opacity_after(core.project.strokes[i].opacity, dir);
    core.snapshot_labeled("Stroke opacity");
    core.project.strokes[i].opacity = new;
    core.status = format!("Stroke {} opacity {}.", i + 1, opacity_label(new));
    true
}

/// One scatter-stepper press moves a stroke's
/// scatter radius by this much: one stage unit —
/// scatter radii are coarse spatial spreads (the
/// library's only scattering brush records 12.0), so
/// the width step's half unit would make crossing
/// the useful range a chore of presses.
pub const SCATTER_STEP: f32 = 1.0;

/// The smallest scatter the stepper writes: 0.0,
/// scatter off — the stroke draws exactly on its
/// path, the model's own floor and serde default.
pub const MIN_STROKE_SCATTER: f32 = 0.0;

/// The largest scatter the stepper writes: twice the
/// broadest scatter any brush records (Spray, 12.0) —
/// the width cap's reasoning (its 40.0 sits above the
/// broadest brush width) applied to scatter — so an
/// existing stroke can always be spread more broadly
/// than any brush draws it, without runaway values
/// from a corrupt file. (The MCP op
/// `draw_ops::tool_set_stroke_style` sets scatter
/// with no upper bound; the panel's stepper range is
/// deliberately narrower, as its width range is the
/// same values the op validates against.)
pub const MAX_STROKE_SCATTER: f32 = 24.0;

/// The scatter a non-finite stored scatter is
/// recovered to before a step applies: 0.0, the
/// model's serde default for the field
/// (`Stroke.scatter`, model.rs:525) — a corrupt value
/// steps from scatter-off, never from itself.
pub const DEFAULT_STROKE_SCATTER: f32 = 0.0;

/// The scatter after one stepper press in direction
/// `dir` (−1 / +1; larger magnitudes step further):
/// `current` moved by `dir` × [`SCATTER_STEP`] and
/// clamped to [[`MIN_STROKE_SCATTER`],
/// [`MAX_STROKE_SCATTER`]] — [`opacity_after`]'s shape
/// applied to scatter. A non-finite `current` (a
/// corrupt or hand-edited project file) is first
/// recovered to [`DEFAULT_STROKE_SCATTER`], so the
/// press always lands on a real, usable radius. A
/// negative stored value clamps up to 0.0 on the
/// first press in either direction. Pure.
pub fn scatter_after(current: f32, dir: i32) -> f32 {
    let base = if current.is_finite() {
        current
    } else {
        DEFAULT_STROKE_SCATTER
    };
    (base + dir as f32 * SCATTER_STEP).clamp(MIN_STROKE_SCATTER, MAX_STROKE_SCATTER)
}

/// The selected stroke's current scatter, or `None`
/// when there is nothing the scatter steppers may act
/// on: no (or a stale) selection — the width row's
/// render condition, in [`selected_stroke_width`]'s
/// guard shape — or the selected stroke is a **spray**
/// stroke. A spray stroke renders from its spray mark
/// and never reaches the scatter dab path
/// (`paint::stroke_dabs` — spray precedence is the
/// caller's, and every renderer replaces a marked
/// stroke with its spray dots), so its stored
/// `scatter` moves nothing on screen and stepping it
/// would be a fake control — the refusal the MCP op
/// `draw_ops::tool_set_stroke_style` makes in the same
/// case. This is the scatter row's render condition
/// and value source; the press restates the same
/// guard inline, as the opacity pair does, so the row
/// and the press can never disagree.
pub fn selected_stroke_scatter(core: &EditorCore, selected: Option<usize>) -> Option<f32> {
    let i = selected.filter(|&i| i < core.project.strokes.len())?;
    let stroke = &core.project.strokes[i];
    if stroke.spray.is_some() {
        return None;
    }
    Some(stroke.scatter)
}

/// Scatter stepper press: step the selected stroke's
/// recorded `scatter` by [`scatter_after`]. The write
/// is the opacity press's composition — exactly one
/// `core.snapshot()`, then a direct `project.strokes`
/// write — so the press is one undo step; like
/// opacity, scatter is a single scalar on the stroke,
/// so there is no taper to rescale. With no selection,
/// a stale one, or a spray stroke selected, the press
/// is a silent no-op: `false`, no snapshot, no status
/// (see [`selected_stroke_scatter`] for the spray
/// rule). On success the status names the stroke and
/// its new scatter, in the measure line's voice:
/// `Stroke {n} scatter {v}.`, with `{v}` from
/// [`scatter_text`] (0 reads `Off`, as the Preset
/// scatter row renders it).
pub fn set_selected_stroke_scatter(
    core: &mut EditorCore,
    selected: Option<usize>,
    dir: i32,
) -> bool {
    let Some(i) = selected.filter(|&i| i < core.project.strokes.len()) else {
        return false;
    };
    if core.project.strokes[i].spray.is_some() {
        return false;
    }
    let new = scatter_after(core.project.strokes[i].scatter, dir);
    core.snapshot_labeled("Stroke scatter");
    core.project.strokes[i].scatter = new;
    core.status = format!("Stroke {} scatter {}.", i + 1, scatter_text(core, new));
    true
}

/// Simplify press: Douglas-Peucker the selected
/// stroke at [`SIMPLIFY_EPSILON`] through the engine
/// op `draw_ops::simplify_stroke_at` (Phase L,
/// section 21). The op mutates in place and takes no
/// snapshot — the caller snapshots — so the press
/// first probes with the op's own index form,
/// `draw_ops::douglas_peucker_indices`, over the
/// stroke's points: if the probe keeps every point,
/// the stroke is already simple at this epsilon and
/// the press only reports that (`Stroke {n} is
/// already simple.`), taking NO snapshot — an undo
/// step for a change that does not exist would be a
/// lie in the undo history. Otherwise exactly one
/// `core.snapshot()` precedes the op, so the press is
/// one undo step, and the status names the surviving
/// point count read back from the stroke itself
/// (`Stroke {n} simplified to {m} points.`). With no
/// selection, or a stale one, the press is a silent
/// no-op — no snapshot, no status — the width press's
/// discipline; the button is not rendered then, so a
/// press can only go stale if the project changed
/// under it.
pub fn simplify_selected(s: &mut State) {
    let Some(i) = s
        .stroke_selected
        .filter(|&i| i < s.core.project.strokes.len())
    else {
        return;
    };
    let kept = stykanimate_rs::draw_ops::douglas_peucker_indices(
        &s.core.project.strokes[i].points,
        SIMPLIFY_EPSILON,
    );
    if kept.len() == s.core.project.strokes[i].points.len() {
        s.core.status = format!("Stroke {} is already simple.", i + 1);
        return;
    }
    s.core.snapshot_labeled("Simplify stroke");
    if stykanimate_rs::draw_ops::simplify_stroke_at(
        &mut s.core.project,
        i,
        SIMPLIFY_EPSILON,
    ) {
        let m = s.core.project.strokes[i].points.len();
        s.core.status = format!("Stroke {} simplified to {} points.", i + 1, m);
    }
}

/// Prev / Next press: step `s.stroke_selected` through
/// the project-wide stroke space ([`step_selection`]).
/// View state only — no snapshot, no status, no core
/// write. On an empty project the press only reports
/// that there are no strokes (and drops any stale
/// selection).
pub fn step_stroke_selection(s: &mut State, dir: i32) {
    let total = s.core.project.strokes.len();
    if total == 0 {
        s.stroke_selected = None;
        s.core.status = "No strokes to select.".into();
        return;
    }
    s.stroke_selected = step_selection(s.stroke_selected, total, dir);
}

/// Clear press: drop the selection. View state only,
/// like stepping; on an empty project the press only
/// reports that there are no strokes.
pub fn clear_stroke_selection(s: &mut State) {
    if s.core.project.strokes.is_empty() {
        s.core.status = "No strokes to select.".into();
        return;
    }
    s.stroke_selected = None;
}

/// Go to frame press: navigate to the selected stroke's
/// frame via `EditorCore::goto` — navigation, the one
/// core write the selection row performs (no snapshot:
/// `goto` is the same op the timeline chips call).
/// Returns whether it navigated; with no (or a stale)
/// selection, or on an empty project, the press only
/// reports why via the status line.
pub fn goto_selected_stroke_frame(s: &mut State) -> bool {
    let total = s.core.project.strokes.len();
    if total == 0 {
        s.core.status = "No strokes to select.".into();
        return false;
    }
    match s.stroke_selected.filter(|&i| i < total) {
        Some(i) => {
            let frame = stroke_frame_of(&s.core, i).unwrap_or(0);
            s.core.goto(frame);
            true
        }
        None => {
            s.core.status = "No stroke selected.".into();
            false
        }
    }
}

/// The bulk Recolour's target colour: the selected
/// node's own colour when a node is selected and
/// carries one, else the selected figure's colour —
/// the same node-over-figure resolution the stage
/// draws with (`stage_view`'s node-colour closure
/// reads `node.color` over the figure's base colour).
pub fn bulk_target_color(core: &EditorCore) -> u32 {
    let fig = &core.project.figures[core.selected_figure];
    match core.selected_node {
        Some(ni) => fig
            .def
            .nodes
            .get(ni)
            .and_then(|node| node.color)
            .unwrap_or(fig.color),
        None => fig.color,
    }
}

/// Bulk Recolour press: rewrite every stroke's base
/// `color` (all frames — see the module docs for the
/// scope choice) to [`bulk_target_color`]. Exactly one
/// snapshot, taken only when there is something to
/// recolour; on an empty stroke list the press only
/// reports that there are no strokes. Returns the
/// number of strokes recoloured.
pub fn recolor_all_strokes(core: &mut EditorCore) -> usize {
    let n = core.project.strokes.len();
    if n == 0 {
        core.status = "No strokes to recolour.".into();
        return 0;
    }
    let color = bulk_target_color(core);
    // One undo step for the whole press: `snapshot()`
    // is the same public call `delete_stroke_at`
    // makes per stroke; composing it once here and
    // writing the field directly is what keeps the
    // bulk op to a single step.
    core.snapshot_labeled("Recolour all strokes");
    for stroke in &mut core.project.strokes {
        stroke.color = color;
    }
    core.status = format!(
        "Recoloured {n} stroke{}.",
        if n == 1 { "" } else { "s" }
    );
    n
}

/// Bulk Delete press: remove every stroke in
/// `Project.strokes` (all frames). Same one-snapshot
/// composition as [`recolor_all_strokes`]; looping
/// `delete_stroke_at` instead would cost one undo
/// step per stroke. On an empty stroke list the
/// press only reports that there are no strokes.
/// Returns the number of strokes removed.
pub fn delete_all_strokes(core: &mut EditorCore) -> usize {
    let n = core.project.strokes.len();
    if n == 0 {
        core.status = "No strokes to delete.".into();
        return 0;
    }
    core.snapshot_labeled("Delete all strokes");
    core.project.strokes.clear();
    core.status = format!(
        "Deleted {n} stroke{}.",
        if n == 1 { "" } else { "s" }
    );
    n
}

/// Where a project-wide stroke selection lands after
/// the strokes at `removed` (ascending
/// `Project.strokes` indices) were deleted: `None`
/// when the selected stroke itself was removed,
/// otherwise the selection shifted down by the number
/// of removed strokes that sat before it, so it keeps
/// pointing at the same stroke. An unset selection
/// stays unset. A *stale* selection (past the end of
/// the list) is shifted arithmetically and stays
/// stale — harmless, because every reader of the
/// selection treats stale as unset (see the Phase G
/// module docs); what this guards against is a valid
/// selection silently landing on a different live
/// stroke.
pub fn selection_after_delete(
    selected: Option<usize>,
    removed: &[usize],
) -> Option<usize> {
    let sel = selected?;
    if removed.contains(&sel) {
        return None;
    }
    let shift = removed.iter().filter(|&&i| i < sel).count();
    Some(sel - shift)
}

/// Frame Recolour press (Phase J): rewrite the base
/// `color` of the **current frame's** strokes only —
/// [`frame_stroke_indices`], which is
/// `stroke_frame_of(..) == core.current` in index
/// form — to [`bulk_target_color`]. Same composition
/// as [`recolor_all_strokes`]: exactly one snapshot,
/// taken only when the frame carries strokes; on an
/// empty frame the press only reports so. Returns the
/// number of strokes recoloured.
pub fn recolor_frame_strokes(core: &mut EditorCore) -> usize {
    let frame = core.current;
    let targets = frame_stroke_indices(core);
    if targets.is_empty() {
        core.status = format!("No strokes on frame {} to recolour.", frame + 1);
        return 0;
    }
    let color = bulk_target_color(core);
    core.snapshot_labeled("Recolour strokes");
    let n = targets.len();
    for i in targets {
        core.project.strokes[i].color = color;
    }
    core.status = format!(
        "Recoloured {n} stroke{} on frame {}.",
        if n == 1 { "" } else { "s" },
        frame + 1
    );
    n
}

/// Frame Delete press, core half (Phase J): remove
/// the current frame's strokes from `Project.strokes`,
/// leaving every other frame's strokes in place and
/// in order. Same one-snapshot composition as
/// [`delete_all_strokes`] — looping
/// `delete_stroke_at` would cost one undo step per
/// stroke. Removals run highest index first: each
/// `Vec::remove` shifts only the strokes after it,
/// which have already been removed, so every target
/// index stays valid until its turn. On an empty
/// frame the press only reports so. Returns the
/// number of strokes removed. Callers holding a
/// stroke selection should go through
/// [`delete_frame_strokes_press`], which also remaps
/// the selection.
pub fn delete_frame_strokes(core: &mut EditorCore) -> usize {
    let frame = core.current;
    let targets = frame_stroke_indices(core);
    if targets.is_empty() {
        core.status = format!("No strokes on frame {} to delete.", frame + 1);
        return 0;
    }
    core.snapshot_labeled("Delete strokes");
    let n = targets.len();
    for i in targets.into_iter().rev() {
        core.project.strokes.remove(i);
    }
    core.status = format!(
        "Deleted {n} stroke{} on frame {}.",
        if n == 1 { "" } else { "s" },
        frame + 1
    );
    n
}

/// Frame Delete press, as the panel's button runs it:
/// [`delete_frame_strokes`] on the core, then the
/// selection remap ([`selection_after_delete`]).
/// Unlike the project-wide Delete all — which leaves
/// any orphaned selection stale, where readers treat
/// it as unset — a frame delete leaves live survivors
/// whose indices move, so the selection is fixed up
/// here rather than allowed to drift onto a different
/// stroke. When the frame carried no strokes nothing
/// was removed and the selection is left untouched.
/// Returns the number of strokes removed.
pub fn delete_frame_strokes_press(s: &mut State) -> usize {
    let removed = frame_stroke_indices(&s.core);
    let n = delete_frame_strokes(&mut s.core);
    if n > 0 {
        s.stroke_selected = selection_after_delete(s.stroke_selected, &removed);
    }
    n
}

/// One parameter row in the charter's stepper order
/// (Phase N): the label grows, then the − button, the
/// value as the kit's centred mono readout
/// (`design::stepper_value`), and the + button — the
/// pair steps that parameter through the preset
/// library ([`step_param`]). Callers pass labels with
/// the `Preset ` prefix (`Preset size`, …): the value
/// is the active preset's, and the pair steps the
/// preset selection, not the value (audit I5(b)).
fn param_row(
    label: &str,
    value: String,
    dec_key: &'static str,
    inc_key: &'static str,
    param: Param,
    t: &Theme,
) -> El {
    row()
        .gap(8.0)
        .center_y()
        .child(
            text(label)
                .font(type_::LABEL)
                .color(t.text_secondary)
                .grow(1.0),
        )
        .child(btn(dec_key, "−", false, t, move |s: &mut State| {
            step_param(&mut s.core, param, -1);
        }))
        .child(design::stepper_value(&value, t))
        .child(btn(inc_key, "+", false, t, move |s: &mut State| {
            step_param(&mut s.core, param, 1);
        }))
}

/// The stroke inspector panel: the bulk stroke row and
/// the project-wide selection row, then the active
/// brush's parameter set (steppers + nib segmented
/// control, all stepping the preset selection — see the
/// module docs), then the current frame's stroke list
/// with per-row Delete buttons and the frame Clear
/// button.
pub fn stroke_panel(s: &State, t: &Theme) -> El {
    let core = &s.core;
    let brush = core.active_brush();
    let mut panel = design::panel_col("stroke-inspector", t);

    // Panel header (Phase N): the kit title in a plain
    // row — this panel's header never carried a key —
    // over the hairline divider.
    panel = panel.child(
        row()
            .w_full()
            .gap(8.0)
            .center_y()
            .child(design::panel_title(
                None,
                &i18n_wire::tr(core, "stroke_title"),
                None,
                t,
            )),
    );
    panel = panel.child(design::divider(t));

    // Bulk stroke ops, at the panel top per the brief:
    // both act on every stroke in the project (all
    // frames) as a single undo step — see the module
    // docs. Always rendered: on an empty stroke list a
    // press reports "no strokes" and changes nothing.
    panel = panel.child(
        row()
            .key("stroke-bulk-row")
            .gap(8.0)
            .center_y()
            .child(
                text(i18n_wire::tr(core, "stroke_all_strokes"))
                    .font(type_::LABEL)
                    .color(t.text)
                    .grow(1.0),
            )
            .child(icon_btn(
                "stroke-recolor-all",
                "palette",
                &i18n_wire::tr(core, "stroke_recolour_all"),
                false,
                t,
                |s| {
                    recolor_all_strokes(&mut s.core);
                },
            ))
            .child(icon_btn(
                "stroke-delete-all",
                "trash-can",
                &i18n_wire::tr(core, "stroke_delete_all"),
                false,
                t,
                |s| {
                    delete_all_strokes(&mut s.core);
                },
            )),
    );

    // Frame-scoped bulk ops (Phase J): the same two
    // presses restricted to the current frame's
    // strokes, directly under the project-wide row —
    // one snapshot per press, and the delete goes
    // through the press helper so the selection is
    // remapped (see the module docs). Always rendered,
    // like the row above: on a frame with no strokes a
    // press reports so and changes nothing.
    panel = panel.child(
        row()
            .key("stroke-bulk-frame-row")
            .w_full()
            .min_w(0.0)
            .flex_wrap()
            .gap(8.0)
            .center_y()
            .child(
                text(
                    i18n_wire::tr(core, "stroke_frame_row")
                        .replace("{frame}", &(core.current + 1).to_string()),
                )
                .font(type_::LABEL)
                .color(t.text)
                .grow(1.0),
            )
            .child(icon_btn(
                "stroke-recolor-frame",
                "palette",
                &i18n_wire::tr(core, "stroke_frame_colour"),
                false,
                t,
                |s| {
                    recolor_frame_strokes(&mut s.core);
                },
            ))
            .child(icon_btn(
                "stroke-delete-frame",
                "trash-can",
                &i18n_wire::tr(core, "stroke_delete_frame"),
                false,
                t,
                |s| {
                    delete_frame_strokes_press(s);
                },
            )),
    );

    // Stroke selection (Phase G): project-wide stepping
    // through `Project.strokes` — the stage draws the
    // highlight for `s.stroke_selected`. View state
    // only (see the module docs); always rendered, and
    // on an empty project the readout says "No strokes"
    // and the buttons only report via the status line.
    panel = panel.child(
        row()
            .key("stroke-sel-row")
            .gap(8.0)
            .center_y()
            .child(
                // `.truncate()` (Wave 10 round 2): the
                // readout is the row's only flexible
                // child, and a plain growing text does
                // not shrink in Rune — only a truncating
                // one flexes (the status strip's
                // documented semantics). Without it the
                // readout paints at full width and pushes
                // Prev / Next / Go to frame / Clear past
                // the card's right edge: the row's fixed
                // parts (three 26 pt icon buttons, the
                // Go to frame icon button, four 8 pt
                // gaps) leave ≈101 pt of the 316 pt
                // content column, and even the shortest
                // live readout ("No stroke selected ·
                // 1 stroke", 29 chars) is wider than
                // that. The string itself is unchanged —
                // elision is display-only, and the
                // measure line below repeats the facts.
                text(selection_readout(core, s.stroke_selected))
                    .font(type_::LABEL)
                    .color(t.text)
                    .grow(1.0)
                    .truncate(),
            )
            .child(icon_only_btn(
                "stroke-sel-prev",
                "chevron-left",
                &i18n_wire::tr(core, "stroke_prev"),
                26.0, false,
                t,
                |s| {
                    step_stroke_selection(s, -1);
                },
            ))
            .child(icon_only_btn(
                "stroke-sel-next",
                "chevron-right",
                &i18n_wire::tr(core, "stroke_next"),
                26.0, false,
                t,
                |s| {
                    step_stroke_selection(s, 1);
                },
            ))
            .child(icon_btn(
                "stroke-sel-goto",
                "crosshairs",
                &i18n_wire::tr(core, "stroke_goto_frame"),
                false,
                t,
                |s| {
                    goto_selected_stroke_frame(s);
                },
            ))
            .child(icon_only_btn("stroke-sel-clear", "xmark", &i18n_wire::tr(core, "stroke_clear_selection"), 26.0, false, t, |s| {
                clear_stroke_selection(s);
            })),
    );

    // Selected stroke measure (Phase I): a muted facts
    // line under the selection row, rendered only while
    // a valid selection exists — no selection, no line.
    if let Some(line) = measure_readout(core, s.stroke_selected) {
        panel = panel.child(
            text(line)
                .key("stroke-measure")
                .font(type_::LABEL)
                .color(t.text_secondary),
        );
    }

    // Selected stroke width (Phase K): steppers under
    // the measure line, rendered under the same valid-
    // selection condition — the recorded width can now
    // be changed after drawing, not only chosen before
    // it. Each press is one undo step (see
    // `set_selected_stroke_width`).
    if let Some(w) = selected_stroke_width(core, s.stroke_selected) {
        panel = panel.child(
            row()
                .key("stroke-width-row")
                .gap(8.0)
                .center_y()
                .child(
                    text(i18n_wire::tr(core, "stroke_width"))
                        .font(type_::LABEL)
                        .color(t.text_secondary)
                        .grow(1.0),
                )
                .child(icon_only_btn(
                    "stroke-width-dn",
                    "chevron-down",
                    &i18n_wire::tr(core, "stroke_width_dec"),
                    26.0, false,
                    t,
                    |s| {
                        let sel = s.stroke_selected;
                        set_selected_stroke_width(&mut s.core, sel, -1);
                    },
                ))
                .child(design::stepper_value(&format!("{w:.1}"), t))
                .child(icon_only_btn(
                    "stroke-width-up",
                    "chevron-up",
                    &i18n_wire::tr(core, "stroke_width_inc"),
                    26.0, false,
                    t,
                    |s| {
                        let sel = s.stroke_selected;
                        set_selected_stroke_width(&mut s.core, sel, 1);
                    },
                )),
        );
    }

    // Selected stroke opacity (Phase M): steppers
    // under the width row, rendered while the
    // selection is valid and the stroke is not a
    // spray stroke (`selected_stroke_opacity` is the
    // gate — a spray stroke's opacity lives on its
    // mark, not this field; see the module docs).
    // Each press is one undo step (see
    // `set_selected_stroke_opacity`).
    if let Some(o) = selected_stroke_opacity(core, s.stroke_selected) {
        panel = panel.child(
            row()
                .key("stroke-opacity-row")
                .gap(8.0)
                .center_y()
                .child(
                    text(i18n_wire::tr(core, "stroke_opacity"))
                        .font(type_::LABEL)
                        .color(t.text_secondary)
                        .grow(1.0),
                )
                .child(icon_only_btn(
                    "stroke-opacity-dn",
                    "chevron-down",
                    &i18n_wire::tr(core, "stroke_opacity_dec"),
                    26.0, false,
                    t,
                    |s| {
                        let sel = s.stroke_selected;
                        set_selected_stroke_opacity(&mut s.core, sel, -1);
                    },
                ))
                .child(design::stepper_value(&opacity_label(o), t))
                .child(icon_only_btn(
                    "stroke-opacity-up",
                    "chevron-up",
                    &i18n_wire::tr(core, "stroke_opacity_inc"),
                    26.0, false,
                    t,
                    |s| {
                        let sel = s.stroke_selected;
                        set_selected_stroke_opacity(&mut s.core, sel, 1);
                    },
                )),
        );
    }

    // Selected stroke scatter (Wave 12, agent D3):
    // steppers under the opacity row, rendered while
    // the selection is valid and the stroke is not a
    // spray stroke (`selected_stroke_scatter` is the
    // gate — a spray stroke never reaches the scatter
    // dab path, so its stored scatter moves nothing;
    // see the module docs). The value renders through
    // `scatter_text`, the Preset scatter row's own
    // formatter (0 reads "Off"). Each press is one
    // undo step (see `set_selected_stroke_scatter`).
    // Same construction as the Width / Opacity rows,
    // so the fixed parts stay at the same ≈110 pt.
    if let Some(sc) = selected_stroke_scatter(core, s.stroke_selected) {
        panel = panel.child(
            row()
                .key("stroke-scatter-row")
                .gap(8.0)
                .center_y()
                .child(
                    text(i18n_wire::tr(core, "stroke_scatter"))
                        .font(type_::LABEL)
                        .color(t.text_secondary)
                        .grow(1.0),
                )
                .child(icon_only_btn(
                    "stroke-scatter-dn",
                    "chevron-down",
                    &i18n_wire::tr(core, "stroke_scatter_dec"),
                    26.0, false,
                    t,
                    |s| {
                        let sel = s.stroke_selected;
                        set_selected_stroke_scatter(&mut s.core, sel, -1);
                    },
                ))
                .child(design::stepper_value(&scatter_text(core, sc), t))
                .child(icon_only_btn(
                    "stroke-scatter-up",
                    "chevron-up",
                    &i18n_wire::tr(core, "stroke_scatter_inc"),
                    26.0, false,
                    t,
                    |s| {
                        let sel = s.stroke_selected;
                        set_selected_stroke_scatter(&mut s.core, sel, 1);
                    },
                )),
        );
    }

    // Selected stroke simplify (Phase L): a Simplify
    // button under the scatter row, under the same
    // valid-selection condition — Douglas-Peucker over
    // the one selected stroke, one undo step when it
    // changes anything (see `simplify_selected`).
    if selected_stroke_width(core, s.stroke_selected).is_some() {
        panel = panel.child(icon_btn(
            "stroke-simplify",
            "wand-magic-sparkles",
            &i18n_wire::tr(core, "stroke_simplify"),
            false,
            t,
            |s| simplify_selected(s),
        ));
    }

    panel = panel.child(design::section_label(
        &i18n_wire::tr(core, "stroke_preset_heading").replace("{name}", brush.name),
        t,
    ));
    panel = panel.child(
        text(tool_text(core))
            .font(type_::LABEL)
            .color(t.text_secondary),
    );
    // Preset-semantics hint (UI mission Wave 4, audit
    // I5(b)): the rows below look like value steppers
    // but step the preset selection — the line says so
    // before the user presses anything.
    panel = panel.child(
        text(i18n_wire::tr(core, "stroke_preset_hint"))
            .font(type_::LABEL)
            .color(t.text_secondary),
    );

    panel = panel.child(param_row(
        &i18n_wire::tr(core, "stroke_preset_size"),
        width_text(brush.width),
        "brush-size-dec",
        "brush-size-inc",
        Param::Size,
        t,
    ));
    panel = panel.child(param_row(
        &i18n_wire::tr(core, "stroke_preset_opacity"),
        opacity_text(brush.opacity),
        "brush-opacity-dec",
        "brush-opacity-inc",
        Param::Opacity,
        t,
    ));
    panel = panel.child(param_row(
        &i18n_wire::tr(core, "stroke_preset_scatter"),
        scatter_text(core, brush.scatter),
        "brush-scatter-dec",
        "brush-scatter-inc",
        Param::Scatter,
        t,
    ));

    // Preset nib: segmented control of the three kinds;
    // a chip press selects the first preset carrying
    // that nib (a preset swap, like the steppers above —
    // hence the "Preset" prefix on the row label).
    let mut nib_row = row()
        .key("brush-nib-row")
        .gap(4.0)
        .center_y()
        .child(text(i18n_wire::tr(core, "stroke_preset_nib")).font(type_::LABEL).color(t.text));
    for nib in NIB_ORDER {
        nib_row = nib_row.child(btn(
            nib_key(nib),
            &nib_label(core, nib),
            brush.nib == nib,
            t,
            move |s: &mut State| {
                select_nib(&mut s.core, nib);
            },
        ));
    }
    panel = panel.child(nib_row);
    panel = panel.child(
        text(
            i18n_wire::tr(core, "stroke_preset_nib_angle")
                .replace("{angle}", &format!("{:.0}", brush.nib_angle)),
        )
        .font(type_::LABEL)
        .color(t.text_secondary),
    );

    panel = panel.child(param_row(
        &i18n_wire::tr(core, "stroke_preset_smoothing"),
        smoothing_text(core, brush.smoothing),
        "brush-smoothing-dec",
        "brush-smoothing-inc",
        Param::Smoothing,
        t,
    ));

    if let Some(hint) = spray_hint(core, &brush) {
        panel = panel.child(text(hint).font(type_::LABEL).color(t.text_secondary));
    }

    // ---- Current frame's strokes ----
    let rows = stroke_rows(core);
    let mut header = row()
        .key("strokes-header")
        .gap(8.0)
        .center_y()
        .child(design::section_label(&strokes_header(core), t).grow(1.0));
    if !rows.is_empty() {
        header = header.child(btn(
            "strokes-clear",
            &i18n_wire::tr(core, "stroke_clear_frame"),
            false,
            t,
            |s| clear_frame_strokes_press(s),
        ));
    }
    panel = panel.child(header);

    if rows.is_empty() {
        panel = panel.child(design::empty_state(
            "paintbrush",
            &i18n_wire::tr(core, "stroke_none_on_frame"),
            t,
        ));
    } else {
        for srow in rows {
            let idx = srow.project_index;
            // Selection feedback (Wave 9): the row of
            // the stroke `s.stroke_selected` names —
            // when that stroke is on this frame — is
            // built on the kit's row shell in its
            // selected form, so the list shows WHICH
            // stroke the readout, the measure line and
            // the Width / Opacity rows are talking
            // about. Until now the rows were plain and
            // indistinguishable, and the readout's
            // number is the project-wide index while
            // the row label is the frame-local
            // position, so nothing on screen connected
            // the two. The shell is the kit's unkeyed
            // list-row form (these rows never carried
            // a key of their own); its gap (PANEL_GAP,
            // the 8.0 this row used) and centre
            // alignment come with it. The row stays
            // display-only — no press handler is added.
            let selected = s.stroke_selected == Some(idx);
            panel = panel.child(
                design::row_shell(selected, t)
                    .child(
                        el()
                            .size(14.0, 14.0)
                            .radius(4.0)
                            .fill(hex_color(srow.color).unwrap_or(t.text)),
                    )
                    .child(
                        text(format!("#{}", srow.position))
                            .font(type_::LABEL)
                            .color(t.text_secondary),
                    )
                    .child(
                        // `.truncate()` (Wave 10 round 2):
                        // the summary's length is unbounded
                        // data (a point count with any
                        // number of digits, the optional
                        // fill and spray clauses) and a
                        // plain growing text does not
                        // shrink in Rune — only a
                        // truncating one flexes. The row's
                        // other parts (shell padding 16,
                        // swatch 14, the `#n` label, three
                        // 8 pt gaps, the Delete button)
                        // leave ≈187 pt, which an ordinary
                        // filled spray summary ("2048 pts ·
                        // w 24 · filled · spray r12 · 50%",
                        // 42 chars) already exceeds — the
                        // Delete button was pushed past the
                        // card edge. Elision is display-
                        // only: `StrokeRow::summary` keeps
                        // the full string (pinned by test).
                        text(srow.summary)
                            .font(type_::LABEL)
                            .color(t.text)
                            .grow(1.0)
                            .truncate(),
                    )
                    .child(btn(
                        &format!("stroke-del-{idx}"),
                        &i18n_wire::tr(core, "stroke_delete"),
                        false,
                        t,
                        move |s: &mut State| {
                            delete_stroke_press(s, idx);
                        },
                    )),
            );
        }
    }

    panel
}

#[cfg(test)]
mod tests {
    use super::*;
    use stykanimate_rs::model::SprayMark;

    fn lib() -> Vec<Brush> {
        stykanimate_rs::paint::brush_library()
    }

    fn plain_stroke(frame: usize, points: usize) -> Stroke {
        Stroke {
            frame,
            points: vec![[0.0, 0.0]; points],
            color: 0xFF_0000,
            width: 4.0,
            widths: Vec::new(),
            fill: None,
            spray: None,
            opacity: 1.0,
            scatter: 0.0,
        }
    }

    #[test]
    fn number_and_percent_formats() {
        assert_eq!(width_text(4.0), "4");
        assert_eq!(width_text(20.0), "20");
        assert_eq!(width_text(2.5), "2.5");
        assert_eq!(opacity_text(1.0), "100%");
        assert_eq!(opacity_text(0.5), "50%");
        let core = EditorCore::new();
        assert_eq!(scatter_text(&core, 0.0), "Off");
        assert_eq!(scatter_text(&core, 12.0), "12");
        assert_eq!(smoothing_text(&core, 0), "Off");
        assert_eq!(smoothing_text(&core, 1), "1 pass");
        assert_eq!(smoothing_text(&core, 3), "3 passes");
    }

    #[test]
    fn stroke_i18n_keys_exist_and_match_the_english_labels() {
        // Wave U (U1): every label and composed readout
        // in this panel resolves through `i18n_wire::tr`
        // under a `stroke_*` key (IA-U1-1). This pin
        // passes only after IA-U1-1 lands — until then
        // `tr` falls back to the key itself (the Q7
        // pattern) — and the pre-existing content tests
        // of the wired helpers above and below share
        // that condition: wiring + keys land in one
        // integration push. Template keys are asserted
        // in their stored `{slot}` form, like the
        // camera pin's `camera_following_figure`.
        let core = EditorCore::new();
        for (key, english) in [
            ("stroke_off", "Off"),
            ("stroke_nib_round", "Round"),
            ("stroke_nib_square", "Square"),
            ("stroke_nib_calligraphy", "Calligraphy"),
            ("stroke_nib_with_angle", "{nib} · {angle}°"),
            ("stroke_smoothing_one", "1 pass"),
            ("stroke_smoothing_many", "{count} passes"),
            (
                "stroke_spray_hint",
                "Spray brush: strokes record a spray mark (radius {radius}, opacity {opacity}).",
            ),
            ("stroke_tool_brush", "Tool: Brush ({name})"),
            ("stroke_tool_eraser", "Tool: Eraser (sized by {name})"),
            ("stroke_strokes_header", "Strokes · Frame {frame} · {count}"),
            ("stroke_summary_base", "{points} pts · w {width}"),
            ("stroke_summary_taper", " · taper"),
            ("stroke_summary_filled", " · filled"),
            ("stroke_summary_spray", " · spray r{radius} · {opacity}"),
            ("stroke_summary_scatter", " · scatter r{radius}"),
            ("stroke_readout_empty", "No strokes"),
            (
                "stroke_readout_selected",
                "Stroke {index} of {total} · frame {frame}",
            ),
            (
                "stroke_readout_none_one",
                "No stroke selected · {count} stroke",
            ),
            (
                "stroke_readout_none_many",
                "No stroke selected · {count} strokes",
            ),
            (
                "stroke_measure_line",
                "Stroke {index}: {points} pts · {length} long · frame {frame}",
            ),
            ("stroke_title", "Stroke inspector"),
            ("stroke_all_strokes", "All strokes"),
            ("stroke_recolour_all", "Recolour all"),
            ("stroke_delete_all", "Delete all"),
            ("stroke_frame_row", "Frame {frame}"),
            ("stroke_frame_colour", "Frame colour"),
            ("stroke_delete_frame", "Delete frame"),
            ("stroke_prev", "Previous stroke"),
            ("stroke_next", "Next stroke"),
            ("stroke_goto_frame", "Go to frame"),
            ("stroke_clear_selection", "Clear selection"),
            ("stroke_width", "Width"),
            ("stroke_width_dec", "Decrease width"),
            ("stroke_width_inc", "Increase width"),
            ("stroke_opacity", "Opacity"),
            ("stroke_opacity_dec", "Decrease opacity"),
            ("stroke_opacity_inc", "Increase opacity"),
            ("stroke_scatter", "Scatter"),
            ("stroke_scatter_dec", "Decrease scatter"),
            ("stroke_scatter_inc", "Increase scatter"),
            ("stroke_simplify", "Simplify"),
            ("stroke_preset_heading", "Brush preset · {name}"),
            (
                "stroke_preset_hint",
                "Stepping a value or picking a nib selects another library preset — presets are bundles, so every value follows the preset.",
            ),
            ("stroke_preset_size", "Preset size"),
            ("stroke_preset_opacity", "Preset opacity"),
            ("stroke_preset_scatter", "Preset scatter"),
            ("stroke_preset_nib", "Preset nib"),
            ("stroke_preset_nib_angle", "Preset nib angle · {angle}°"),
            ("stroke_preset_smoothing", "Preset smoothing"),
            ("stroke_clear_frame", "Clear frame"),
            ("stroke_none_on_frame", "No strokes on this frame."),
            ("stroke_delete", "Delete"),
        ] {
            assert!(i18n_wire::key_in_table(key), "missing i18n key: {key}");
            assert_eq!(i18n_wire::tr(&core, key), english);
        }
    }

    #[test]
    fn nib_text_shows_angle_only_when_it_matters() {
        let lib = lib();
        let by_name = |name: &str| lib.iter().find(|b| b.name == name).copied().unwrap();
        let core = EditorCore::new();
        assert_eq!(nib_text(&core, &by_name("Pen")), "Round");
        // Square at 0°: no angle shown.
        assert_eq!(nib_text(&core, &by_name("Brush")), "Square");
        assert_eq!(nib_text(&core, &by_name("Paint Brush")), "Square · 45°");
        assert_eq!(nib_text(&core, &by_name("Calligraphy")), "Calligraphy · 30°");
        assert_eq!(nib_text(&core, &by_name("Calligraphy L")), "Calligraphy · 315°");
    }

    #[test]
    fn step_by_size_walks_to_the_nearest_width() {
        let lib = lib();
        // Pen (idx 0, width 4): next wider is Chisel Marker
        // (idx 7, width 5); next narrower is Pencil (idx 1).
        assert_eq!(step_index_by(&lib, 0, 1, |b| b.width), Some(7));
        assert_eq!(step_index_by(&lib, 0, -1, |b| b.width), Some(1));
        // Broad Brush (idx 8, width 20) is the widest; Pencil
        // (idx 1, width 2) the narrowest.
        assert_eq!(step_index_by(&lib, 8, 1, |b| b.width), None);
        assert_eq!(step_index_by(&lib, 1, -1, |b| b.width), None);
        // Degenerate inputs never step.
        assert_eq!(step_index_by(&lib, 0, 0, |b| b.width), None);
        assert_eq!(step_index_by(&lib, 99, 1, |b| b.width), None);
    }

    #[test]
    fn step_by_opacity_and_scatter_reach_spray() {
        let lib = lib();
        // Two presets are below full opacity since
        // Phase M converted Pencil: Pencil (idx 1,
        // 0.55) and Spray (idx 9, 0.5). Stepping down
        // from Pen walks them nearest-first; stepping
        // up from Spray reaches Pencil first, then the
        // full-opacity tie resolves to the lowest
        // index (Pen). Scatter is still Spray-only.
        assert_eq!(step_index_by(&lib, 0, -1, |b| b.opacity), Some(1));
        assert_eq!(step_index_by(&lib, 1, -1, |b| b.opacity), Some(9));
        assert_eq!(step_index_by(&lib, 9, 1, |b| b.opacity), Some(1));
        assert_eq!(step_index_by(&lib, 1, 1, |b| b.opacity), Some(0));
        assert_eq!(step_index_by(&lib, 0, 1, |b| b.scatter), Some(9));
        assert_eq!(step_index_by(&lib, 9, -1, |b| b.scatter), Some(0));
    }

    #[test]
    fn step_by_smoothing_ties_to_the_lowest_index() {
        let lib = lib();
        // Four presets smooth once; from Pen (0) the nearest
        // step up is the lowest-indexed of them: Marker (2).
        assert_eq!(step_index_by(&lib, 0, 1, |b| b.smoothing as f32), Some(2));
        // Paint Brush (idx 5, 2 passes) steps up to Broad
        // Brush (idx 8, 3 passes — the library maximum),
        // which itself cannot step up.
        assert_eq!(step_index_by(&lib, 5, 1, |b| b.smoothing as f32), Some(8));
        assert_eq!(step_index_by(&lib, 8, 1, |b| b.smoothing as f32), None);
    }

    #[test]
    fn first_preset_per_nib() {
        let lib = lib();
        assert_eq!(first_index_with_nib(&lib, Nib::Round), Some(0));
        assert_eq!(first_index_with_nib(&lib, Nib::Calligraphy), Some(3));
        assert_eq!(first_index_with_nib(&lib, Nib::Square), Some(4));
        let core = EditorCore::new();
        for nib in NIB_ORDER {
            assert!(!nib_label(&core, nib).is_empty());
            assert!(nib_key(nib).starts_with("brush-nib-"));
        }
    }

    #[test]
    fn step_param_moves_the_selection_and_reports_like_cycle_brush() {
        let mut core = EditorCore::new();
        assert_eq!(core.active_brush().name, "Pen");
        assert!(step_param(&mut core, Param::Size, 1));
        assert_eq!(core.brush_index, 7);
        assert_eq!(core.active_brush().name, "Chisel Marker");
        assert_eq!(core.status, "Brush: Chisel Marker (width 5).");
        // The widest brush cannot step up; the status says so
        // and the selection stays.
        core.brush_index = 8;
        assert!(!step_param(&mut core, Param::Size, 1));
        assert_eq!(core.brush_index, 8);
        assert_eq!(core.status, "No wider brush in the library.");
    }

    #[test]
    fn select_nib_picks_the_first_preset_with_that_nib() {
        let mut core = EditorCore::new();
        assert!(select_nib(&mut core, Nib::Square));
        assert_eq!(core.active_brush().name, "Brush");
        assert!(select_nib(&mut core, Nib::Calligraphy));
        assert_eq!(core.active_brush().name, "Calligraphy");
        assert!(select_nib(&mut core, Nib::Round));
        assert_eq!(core.active_brush().name, "Pen");
    }

    #[test]
    fn spray_hint_only_for_scatter_brushes() {
        let lib = lib();
        let spray = lib.iter().find(|b| b.name == "Spray").unwrap();
        let core = EditorCore::new();
        assert_eq!(
            spray_hint(&core, spray),
            Some("Spray brush: strokes record a spray mark (radius 12, opacity 50%).".to_string())
        );
        assert_eq!(spray_hint(&core, &lib[0]), None);
    }

    #[test]
    fn tool_text_names_the_tool_and_its_sizing_brush() {
        let mut core = EditorCore::new();
        assert_eq!(tool_text(&core), "Tool: Brush (Pen)");
        core.toggle_draw_tool();
        assert_eq!(tool_text(&core), "Tool: Eraser (sized by Pen)");
    }

    #[test]
    fn frame_stroke_list_filters_by_current_frame() {
        let mut core = EditorCore::new();
        assert_eq!(frame_stroke_count(&core), 0);
        assert_eq!(strokes_header(&core), "Strokes · Frame 1 · 0");
        core.project.strokes.push(plain_stroke(0, 3));
        core.project.strokes.push(plain_stroke(1, 5));
        core.project.strokes.push(plain_stroke(0, 7));
        assert_eq!(frame_stroke_indices(&core), vec![0, 2]);
        assert_eq!(frame_stroke_count(&core), 2);
        assert_eq!(strokes_header(&core), "Strokes · Frame 1 · 2");
        core.goto(1);
        assert_eq!(frame_stroke_indices(&core), vec![1]);
        assert_eq!(strokes_header(&core), "Strokes · Frame 2 · 1");
    }

    #[test]
    fn stroke_summary_reports_points_width_fill_and_spray() {
        let core = EditorCore::new();
        let plain = plain_stroke(0, 3);
        assert_eq!(stroke_summary(&core, &plain), "3 pts · w 4");
        let mut filled = plain_stroke(0, 12);
        filled.fill = Some(0xFF_0000);
        assert_eq!(stroke_summary(&core, &filled), "12 pts · w 4 · filled");
        let mut sprayed = plain_stroke(0, 20);
        sprayed.spray = Some(SprayMark {
            seed: 42,
            radius: 12.0,
            opacity: 0.5,
        });
        assert_eq!(stroke_summary(&core, &sprayed), "20 pts · w 4 · spray r12 · 50%");
    }

    #[test]
    fn stroke_rows_keep_the_full_summary_for_long_metadata() {
        // Wave 10 round 2: the row's summary text
        // elides on screen (`.truncate()` — the data
        // is unbounded and the row's room is ≈187 pt),
        // but elision is display-only: the row DATA
        // keeps the full string, so nothing the panel
        // reports is falsified. A filled spray stroke
        // with a four-digit point count is the longest
        // ordinary summary shape there is.
        let mut core = EditorCore::new();
        let mut big = plain_stroke(0, 2048);
        big.width = 24.0;
        big.fill = Some(0x00_FF00);
        big.spray = Some(SprayMark {
            seed: 7,
            radius: 12.0,
            opacity: 0.5,
        });
        core.project.strokes.push(big);
        let rows = stroke_rows(&core);
        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0].summary, "2048 pts · w 24 · filled · spray r12 · 50%",
            "the full summary survives in the row data"
        );
        assert_eq!(rows[0].summary.chars().count(), 42);
    }

    #[test]
    fn selection_readout_names_multi_digit_positions_and_frames() {
        // Wave 10 round 2: the readout strings whose
        // widths ground the selection row's
        // `.truncate()` — the row leaves the readout
        // ≈101 pt, and these ordinary-project forms
        // are far wider (29+ chars in LABEL). The
        // strings themselves are the contract; the
        // view only elides their painting.
        let mut core = EditorCore::new();
        for _ in 0..120 {
            core.project.strokes.push(plain_stroke(0, 1));
        }
        core.project.strokes[117].frame = 59;
        assert_eq!(
            selection_readout(&core, Some(117)),
            "Stroke 118 of 120 · frame 60"
        );
        assert_eq!(
            selection_readout(&core, None),
            "No stroke selected · 120 strokes"
        );
        // The shortest live form, for the record: even
        // a one-stroke project overflows the room
        // without truncation.
        let mut core = EditorCore::new();
        core.project.strokes.push(plain_stroke(0, 1));
        assert_eq!(
            selection_readout(&core, None),
            "No stroke selected · 1 stroke"
        );
    }

    #[test]
    fn stroke_rows_carry_project_indices_across_interleaved_frames() {
        let mut core = EditorCore::new();
        // Interleave frames 0, 1, 0, 1, 0; distinct point
        // counts make each stroke identifiable by summary.
        for (frame, pts) in [(0, 1), (1, 2), (0, 3), (1, 4), (0, 5)] {
            core.project.strokes.push(plain_stroke(frame, pts));
        }
        // Frame 0's rows: project indices 0, 2, 4 shown at
        // display positions 1, 2, 3 — the delete target is
        // the project index, never the row position.
        let rows = stroke_rows(&core);
        assert_eq!(
            rows.iter().map(|r| r.project_index).collect::<Vec<_>>(),
            vec![0, 2, 4]
        );
        assert_eq!(
            rows.iter().map(|r| r.position).collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        assert_eq!(rows[1].summary, "3 pts · w 4");
        assert_eq!(rows[1].color, 0xFF_0000);
        core.goto(1);
        let rows = stroke_rows(&core);
        assert_eq!(
            rows.iter().map(|r| r.project_index).collect::<Vec<_>>(),
            vec![1, 3]
        );
        assert_eq!(rows[0].summary, "2 pts · w 4");
    }

    #[test]
    fn delete_stroke_removes_the_row_s_project_stroke() {
        let mut core = EditorCore::new();
        for (frame, pts) in [(0, 1), (1, 2), (0, 3), (1, 4), (0, 5)] {
            core.project.strokes.push(plain_stroke(frame, pts));
        }
        // Delete frame 0's middle row: its project index is
        // 2 (the 3-point stroke). Deleting by row position
        // instead would kill project index 1 — a frame-1
        // stroke — which must survive.
        let rows = stroke_rows(&core);
        delete_stroke(&mut core, rows[1].project_index);
        let remaining: Vec<usize> = core
            .project
            .strokes
            .iter()
            .map(|s| s.points.len())
            .collect();
        assert_eq!(remaining, vec![1, 2, 4, 5]);
        // Frame 0's rebuilt rows renumber over the
        // survivors and still point at the right strokes:
        // the 5-point stroke moved from project index 4
        // to index 3.
        let rows = stroke_rows(&core);
        assert_eq!(
            rows.iter().map(|r| r.project_index).collect::<Vec<_>>(),
            vec![0, 3]
        );
        assert_eq!(
            rows.iter().map(|r| r.position).collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert_eq!(rows[1].summary, "5 pts · w 4");
    }

    fn colored_stroke(frame: usize, color: u32) -> Stroke {
        let mut s = plain_stroke(frame, 2);
        s.color = color;
        s
    }

    #[test]
    fn bulk_target_color_prefers_node_colour_then_figure() {
        let mut core = EditorCore::new();
        let fi = core.selected_figure;
        core.project.figures[fi].color = 0x00_00FF;
        // No node selected: the figure's colour.
        core.selected_node = None;
        assert_eq!(bulk_target_color(&core), 0x00_00FF);
        // Node selected but carrying no colour of its
        // own: still the figure's colour.
        core.selected_node = Some(0);
        core.project.figures[fi].def.nodes[0].color = None;
        assert_eq!(bulk_target_color(&core), 0x00_00FF);
        // Node selected with its own colour: the node's.
        core.project.figures[fi].def.nodes[0].color = Some(0x12_3456);
        assert_eq!(bulk_target_color(&core), 0x12_3456);
    }

    #[test]
    fn recolor_all_strokes_recolors_all_frames_as_one_undo_step() {
        let mut core = EditorCore::new();
        core.project
            .strokes
            .push(colored_stroke(0, 0xAA_0000));
        core.project
            .strokes
            .push(colored_stroke(1, 0x00_BB00));
        core.project
            .strokes
            .push(colored_stroke(0, 0x00_00CC));
        let fi = core.selected_figure;
        core.project.figures[fi].color = 0x77_7777;
        core.selected_node = None;
        let depth = core.undo_depth();
        assert_eq!(recolor_all_strokes(&mut core), 3);
        assert!(
            core.project.strokes.iter().all(|s| s.color == 0x77_7777),
            "every stroke, on every frame, takes the target colour"
        );
        assert_eq!(core.status, "Recoloured 3 strokes.");
        // One press = one undo step, and that one undo
        // restores every original colour.
        assert_eq!(core.undo_depth(), depth + 1);
        core.undo();
        let colours: Vec<u32> =
            core.project.strokes.iter().map(|s| s.color).collect();
        assert_eq!(colours, vec![0xAA_0000, 0x00_BB00, 0x00_00CC]);
    }

    #[test]
    fn delete_all_strokes_empties_all_frames_as_one_undo_step() {
        let mut core = EditorCore::new();
        for frame in [0, 1, 0] {
            core.project.strokes.push(plain_stroke(frame, 2));
        }
        let depth = core.undo_depth();
        assert_eq!(delete_all_strokes(&mut core), 3);
        assert!(core.project.strokes.is_empty());
        assert_eq!(core.status, "Deleted 3 strokes.");
        assert_eq!(core.undo_depth(), depth + 1);
        core.undo();
        let frames: Vec<usize> =
            core.project.strokes.iter().map(|s| s.frame).collect();
        assert_eq!(frames, vec![0, 1, 0], "one undo restores all frames' strokes");
    }

    #[test]
    fn bulk_ops_report_singular_counts() {
        let mut core = EditorCore::new();
        core.project.strokes.push(plain_stroke(0, 2));
        assert_eq!(recolor_all_strokes(&mut core), 1);
        assert_eq!(core.status, "Recoloured 1 stroke.");
        assert_eq!(delete_all_strokes(&mut core), 1);
        assert_eq!(core.status, "Deleted 1 stroke.");
    }

    #[test]
    fn bulk_ops_on_an_empty_list_take_no_snapshot() {
        let mut core = EditorCore::new();
        core.project.strokes.clear();
        let depth = core.undo_depth();
        assert_eq!(recolor_all_strokes(&mut core), 0);
        assert_eq!(core.status, "No strokes to recolour.");
        assert_eq!(delete_all_strokes(&mut core), 0);
        assert_eq!(core.status, "No strokes to delete.");
        assert_eq!(
            core.undo_depth(),
            depth,
            "an empty press must not add an undo step"
        );
    }

    #[test]
    fn recolor_frame_strokes_touches_only_the_current_frame() {
        let mut core = EditorCore::new();
        core.project
            .strokes
            .push(colored_stroke(0, 0xAA_0000));
        core.project
            .strokes
            .push(colored_stroke(1, 0x00_BB00));
        core.project
            .strokes
            .push(colored_stroke(0, 0x00_00CC));
        let fi = core.selected_figure;
        core.project.figures[fi].color = 0x77_7777;
        core.selected_node = None;
        let depth = core.undo_depth();
        assert_eq!(recolor_frame_strokes(&mut core), 2);
        let colours: Vec<u32> =
            core.project.strokes.iter().map(|s| s.color).collect();
        assert_eq!(
            colours,
            vec![0x77_7777, 0x00_BB00, 0x77_7777],
            "frame 2's stroke keeps its own colour"
        );
        assert_eq!(core.status, "Recoloured 2 strokes on frame 1.");
        // One press = one undo step, restoring both
        // recoloured strokes at once.
        assert_eq!(core.undo_depth(), depth + 1);
        core.undo();
        let colours: Vec<u32> =
            core.project.strokes.iter().map(|s| s.color).collect();
        assert_eq!(colours, vec![0xAA_0000, 0x00_BB00, 0x00_00CC]);
        // From frame 2 the press recolours only that
        // frame's stroke, names frame 2, and uses the
        // singular.
        core.goto(1);
        assert_eq!(recolor_frame_strokes(&mut core), 1);
        assert_eq!(core.status, "Recoloured 1 stroke on frame 2.");
        let colours: Vec<u32> =
            core.project.strokes.iter().map(|s| s.color).collect();
        assert_eq!(colours, vec![0xAA_0000, 0x77_7777, 0x00_00CC]);
    }

    #[test]
    fn delete_frame_strokes_removes_only_the_current_frame() {
        let mut core = EditorCore::new();
        // Interleaved frames; point counts identify each
        // stroke after the removal.
        for (frame, pts) in [(0, 1), (1, 2), (0, 3), (1, 4), (0, 5)] {
            core.project.strokes.push(plain_stroke(frame, pts));
        }
        let depth = core.undo_depth();
        assert_eq!(delete_frame_strokes(&mut core), 3);
        // The survivors are frame 2's two strokes, in
        // their original order.
        let remaining: Vec<(usize, usize)> = core
            .project
            .strokes
            .iter()
            .map(|s| (s.frame, s.points.len()))
            .collect();
        assert_eq!(remaining, vec![(1, 2), (1, 4)]);
        assert_eq!(core.status, "Deleted 3 strokes on frame 1.");
        assert_eq!(core.undo_depth(), depth + 1);
        core.undo();
        assert_eq!(
            core.project.strokes.len(),
            5,
            "one undo restores the whole frame"
        );
    }

    #[test]
    fn frame_bulk_ops_on_an_empty_frame_take_no_snapshot() {
        let mut core = EditorCore::new();
        // A stroke exists — but on frame 2, while the
        // current frame is 1.
        core.project.strokes.push(plain_stroke(1, 2));
        let depth = core.undo_depth();
        assert_eq!(recolor_frame_strokes(&mut core), 0);
        assert_eq!(core.status, "No strokes on frame 1 to recolour.");
        assert_eq!(delete_frame_strokes(&mut core), 0);
        assert_eq!(core.status, "No strokes on frame 1 to delete.");
        assert_eq!(
            core.undo_depth(),
            depth,
            "an empty-frame press must not add an undo step"
        );
        assert_eq!(
            core.project.strokes.len(),
            1,
            "the other frame's stroke is untouched"
        );
    }

    #[test]
    fn selection_after_delete_clears_removed_and_shifts_survivors() {
        // Removed indices 0, 2, 4 of a six-stroke list.
        let removed = vec![0, 2, 4];
        assert_eq!(selection_after_delete(None, &removed), None);
        // The selected stroke itself was removed: clear.
        assert_eq!(selection_after_delete(Some(0), &removed), None);
        assert_eq!(selection_after_delete(Some(2), &removed), None);
        assert_eq!(selection_after_delete(Some(4), &removed), None);
        // Survivors shift down by the removed count
        // before them, still naming the same stroke.
        assert_eq!(selection_after_delete(Some(1), &removed), Some(0));
        assert_eq!(selection_after_delete(Some(3), &removed), Some(1));
        assert_eq!(selection_after_delete(Some(5), &removed), Some(2));
        // Nothing removed: the selection is untouched.
        assert_eq!(selection_after_delete(Some(3), &[]), Some(3));
    }

    #[test]
    fn delete_frame_press_clears_a_removed_selection_and_keeps_a_survivor() {
        // Frames 0, 1, 0, 1; the current frame is 0
        // (shown as frame 1), so the press removes the
        // strokes at project indices 0 and 2.
        let mut core = EditorCore::new();
        for frame in [0, 1, 0, 1] {
            core.project.strokes.push(plain_stroke(frame, 2));
        }
        // Selection on a stroke the press removes: it is
        // cleared, never left pointing at a neighbour.
        let mut s = state_with(core, Some(2));
        assert_eq!(delete_frame_strokes_press(&mut s), 2);
        assert_eq!(s.stroke_selected, None);
        assert_eq!(s.core.status, "Deleted 2 strokes on frame 1.");

        // Selection on a surviving stroke: it shifts
        // down past the two removals and still names
        // the same (frame-index-1) stroke.
        let mut core = EditorCore::new();
        for frame in [0, 1, 0, 1] {
            core.project.strokes.push(plain_stroke(frame, 2));
        }
        let mut s = state_with(core, Some(3));
        assert_eq!(delete_frame_strokes_press(&mut s), 2);
        assert_eq!(s.stroke_selected, Some(1));
        assert_eq!(stroke_frame_of(&s.core, 1), Some(1));
    }

    #[test]
    fn delete_stroke_press_remaps_the_selection() {
        // Frames 0, 1, 0 at project indices 0, 1, 2 —
        // point counts identify the strokes.
        // Deleting a stroke BEFORE the selection: the
        // selection shifts down one and still names
        // the same (3-point, frame-0) stroke.
        let mut core = EditorCore::new();
        for (frame, pts) in [(0, 1), (1, 2), (0, 3)] {
            core.project.strokes.push(plain_stroke(frame, pts));
        }
        let mut s = state_with(core, Some(2));
        delete_stroke_press(&mut s, 0);
        assert_eq!(s.stroke_selected, Some(1));
        assert_eq!(s.core.project.strokes[1].points.len(), 3);
        assert_eq!(s.core.status, "Stroke 1 deleted.");

        // Deleting the selected stroke itself: the
        // selection is cleared, never left pointing at
        // the stroke that slid into its index.
        let mut core = EditorCore::new();
        for (frame, pts) in [(0, 1), (1, 2), (0, 3)] {
            core.project.strokes.push(plain_stroke(frame, pts));
        }
        let mut s = state_with(core, Some(1));
        delete_stroke_press(&mut s, 1);
        assert_eq!(s.stroke_selected, None);

        // Deleting a stroke AFTER the selection: the
        // selection is untouched.
        let mut core = EditorCore::new();
        for (frame, pts) in [(0, 1), (1, 2), (0, 3)] {
            core.project.strokes.push(plain_stroke(frame, pts));
        }
        let mut s = state_with(core, Some(0));
        delete_stroke_press(&mut s, 2);
        assert_eq!(s.stroke_selected, Some(0));

        // A stale target: the core op reports and
        // nothing else changes — no removal, no remap,
        // no snapshot.
        let mut core = EditorCore::new();
        for (frame, pts) in [(0, 1), (1, 2)] {
            core.project.strokes.push(plain_stroke(frame, pts));
        }
        let depth = core.undo_depth();
        let mut s = state_with(core, Some(1));
        delete_stroke_press(&mut s, 9);
        assert_eq!(s.stroke_selected, Some(1));
        assert_eq!(s.core.project.strokes.len(), 2);
        assert_eq!(s.core.status, "No stroke 10 to delete.");
        assert_eq!(s.core.undo_depth(), depth);
    }

    #[test]
    fn clear_frame_press_remaps_the_selection() {
        // Frames 0, 1, 0, 1; the current frame is 0,
        // so the press removes project indices 0 and 2.
        // A selection on a removed stroke clears.
        let mut core = EditorCore::new();
        for frame in [0, 1, 0, 1] {
            core.project.strokes.push(plain_stroke(frame, 2));
        }
        let mut s = state_with(core, Some(2));
        clear_frame_strokes_press(&mut s);
        assert_eq!(s.stroke_selected, None);
        assert_eq!(s.core.project.strokes.len(), 2);
        assert_eq!(s.core.status, "Cleared 2 stroke(s) on this frame.");

        // A selection on a survivor after both removed
        // indices shifts down by two, still naming the
        // same (frame-1) stroke.
        let mut core = EditorCore::new();
        for frame in [0, 1, 0, 1] {
            core.project.strokes.push(plain_stroke(frame, 2));
        }
        let mut s = state_with(core, Some(3));
        clear_frame_strokes_press(&mut s);
        assert_eq!(s.stroke_selected, Some(1));
        assert_eq!(stroke_frame_of(&s.core, 1), Some(1));

        // A selection on a survivor with exactly one
        // removed index before it shifts down by one.
        let mut core = EditorCore::new();
        for frame in [0, 1, 0, 1] {
            core.project.strokes.push(plain_stroke(frame, 2));
        }
        let mut s = state_with(core, Some(1));
        clear_frame_strokes_press(&mut s);
        assert_eq!(s.stroke_selected, Some(0));
        assert_eq!(stroke_frame_of(&s.core, 0), Some(1));

        // An empty current frame removes nothing and
        // the selection is left exactly as it was.
        let mut core = EditorCore::new();
        core.project.strokes.push(plain_stroke(1, 2));
        let mut s = state_with(core, Some(0));
        clear_frame_strokes_press(&mut s);
        assert_eq!(s.stroke_selected, Some(0));
        assert_eq!(s.core.status, "Cleared 0 stroke(s) on this frame.");
    }

    #[test]
    fn stroke_panel_builds_with_rows_and_a_selection() {
        // Build smoke for the Wave 9 row-shell change:
        // a current frame carrying strokes, one of them
        // selected, must build the whole panel (bulk
        // rows, selection row, width/opacity rows, list
        // rows on the kit shell) without panicking.
        let mut core = EditorCore::new();
        core.project.strokes.push(plain_stroke(0, 3));
        core.project.strokes.push(plain_stroke(0, 5));
        let s = state_with(core, Some(1));
        let t = crate::rune_sec::app_theme::app_theme(true);
        let _ = stroke_panel(&s, &t);
        // And with no strokes at all: the kit empty
        // state branch builds too.
        let s = state_with(EditorCore::new(), None);
        let _ = stroke_panel(&s, &t);
    }

    #[test]
    fn step_selection_wraps_and_enters_from_unset() {
        // Empty project: nothing to select, either way —
        // not even from a (necessarily stale) Some.
        assert_eq!(step_selection(None, 0, 1), None);
        assert_eq!(step_selection(None, 0, -1), None);
        assert_eq!(step_selection(Some(0), 0, 1), None);
        // Single stroke: entering lands on it, and
        // stepping wraps back onto it from both sides.
        assert_eq!(step_selection(None, 1, 1), Some(0));
        assert_eq!(step_selection(None, 1, -1), Some(0));
        assert_eq!(step_selection(Some(0), 1, 1), Some(0));
        assert_eq!(step_selection(Some(0), 1, -1), Some(0));
        // Walking five strokes, entering from unset at
        // both ends and wrapping past both ends.
        assert_eq!(step_selection(None, 5, 1), Some(0));
        assert_eq!(step_selection(None, 5, -1), Some(4));
        assert_eq!(step_selection(Some(2), 5, 1), Some(3));
        assert_eq!(step_selection(Some(2), 5, -1), Some(1));
        assert_eq!(step_selection(Some(4), 5, 1), Some(0), "next wraps");
        assert_eq!(step_selection(Some(0), 5, -1), Some(4), "prev wraps");
        // A stale selection is treated as unset: +1
        // re-enters at the first stroke, −1 at the last —
        // it never steps relative to the stale index.
        assert_eq!(step_selection(Some(5), 5, 1), Some(0));
        assert_eq!(step_selection(Some(5), 5, -1), Some(4));
        assert_eq!(step_selection(Some(99), 5, 1), Some(0));
        assert_eq!(step_selection(Some(99), 5, -1), Some(4));
        // No direction: a valid selection holds; an
        // unset or stale one stays unset.
        assert_eq!(step_selection(Some(2), 5, 0), Some(2));
        assert_eq!(step_selection(None, 5, 0), None);
        assert_eq!(step_selection(Some(9), 5, 0), None);
    }

    #[test]
    fn stroke_frame_of_maps_project_indices_to_frames() {
        let mut core = EditorCore::new();
        for frame in [0, 2, 0, 1, 2] {
            core.project.strokes.push(plain_stroke(frame, 2));
        }
        // The project-wide index space interleaves frames;
        // each index maps to its own stroke's frame.
        assert_eq!(stroke_frame_of(&core, 0), Some(0));
        assert_eq!(stroke_frame_of(&core, 1), Some(2));
        assert_eq!(stroke_frame_of(&core, 2), Some(0));
        assert_eq!(stroke_frame_of(&core, 3), Some(1));
        assert_eq!(stroke_frame_of(&core, 4), Some(2));
        assert_eq!(stroke_frame_of(&core, 5), None);
        assert_eq!(stroke_total(&core), 5);
    }

    #[test]
    fn selection_readout_names_position_total_and_frame() {
        let mut core = EditorCore::new();
        core.project.strokes.clear();
        assert_eq!(selection_readout(&core, None), "No strokes");
        assert_eq!(selection_readout(&core, Some(0)), "No strokes");
        for frame in [0, 2, 0] {
            core.project.strokes.push(plain_stroke(frame, 2));
        }
        assert_eq!(
            selection_readout(&core, None),
            "No stroke selected · 3 strokes"
        );
        // Index 1 is the stroke recorded on frame 2,
        // shown 1-based as frame 3.
        assert_eq!(
            selection_readout(&core, Some(1)),
            "Stroke 2 of 3 · frame 3"
        );
        assert_eq!(
            selection_readout(&core, Some(0)),
            "Stroke 1 of 3 · frame 1"
        );
        // A stale selection reads as no selection.
        assert_eq!(
            selection_readout(&core, Some(3)),
            "No stroke selected · 3 strokes"
        );
    }

    #[test]
    fn selection_readout_uses_the_singular_for_one_stroke() {
        let mut core = EditorCore::new();
        core.project.strokes.push(plain_stroke(0, 2));
        assert_eq!(
            selection_readout(&core, None),
            "No stroke selected · 1 stroke"
        );
        assert_eq!(
            selection_readout(&core, Some(0)),
            "Stroke 1 of 1 · frame 1"
        );
    }

    /// A core whose project carries strokes on frames
    /// 0, 2, 0 (project indices 0, 1, 2). The demo
    /// project has 5 frames, so frame 2 is a real
    /// `goto` target.
    fn core_with_strokes() -> EditorCore {
        let mut core = EditorCore::new();
        for frame in [0, 2, 0] {
            core.project.strokes.push(plain_stroke(frame, 2));
        }
        core
    }

    /// A shell `State` over the given core with the
    /// given stroke selection — field-for-field the
    /// literal the other sections' State tests use,
    /// with the Phase G fields included.
    fn state_with(core: EditorCore, sel: Option<usize>) -> State {
        State {
            core,
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
            settings: crate::rune_sec::settings_popover::SettingsState::default(),
            toasts: crate::rune_sec::toasts::ToastsState::new(),
            text_tool: crate::rune_sec::text_tool::TextToolState::new(),
            name_edit: rune::TextEdit::new(""),
            export_use_range: false,
            fx_selected: None,
            subtitle_selected: None,
            pose_clipboard: None,
            workspace: crate::rune_sec::workspaces::Workspace::Animate,
            setup_mode_before_build: false,
            guides: false,
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
            stroke_selected: sel,
            particle_selected: None,
            guides_frustum: true,
            guides_safe: true,
            guides_golden: false,
            guides_diagonals: false,
            guides_stored: true,
            rulers: false,
            limit_arcs: false,
            onion_ui: Default::default(),
            last_frame: 0,
            pose_rename: crate::rune_sec::pose_tools::PoseRenameState::new(),
            pose_library: crate::rune_sec::pose_tools::PoseLibraryState::new(),
            loop_playback: true,
            playback_speed: 1.0,
            was_playing: false,
            last_status: String::new(),
            toast_now: 0.0,
            last_dark: true,
            confirm_action: None,
            home_rename: crate::rune_sec::home_panel::HomeRenameState::new(),
        }
    }

    #[test]
    fn selection_presses_step_wrap_and_clear_as_view_state_only() {
        let mut s = state_with(core_with_strokes(), None);
        let depth = s.core.undo_depth();
        let status = s.core.status.clone();
        // Stepping enters at the first stroke, walks the
        // project-wide space (frames are irrelevant to
        // the order) and wraps at both ends.
        step_stroke_selection(&mut s, 1);
        assert_eq!(s.stroke_selected, Some(0));
        step_stroke_selection(&mut s, 1);
        assert_eq!(s.stroke_selected, Some(1));
        step_stroke_selection(&mut s, 1);
        assert_eq!(s.stroke_selected, Some(2));
        step_stroke_selection(&mut s, 1);
        assert_eq!(s.stroke_selected, Some(0), "next wraps to the first");
        step_stroke_selection(&mut s, -1);
        assert_eq!(s.stroke_selected, Some(2), "prev wraps to the last");
        // View state only: no undo step was added and the
        // status line was never touched (no toast spam).
        assert_eq!(s.core.undo_depth(), depth);
        assert_eq!(s.core.status, status);
        // A stale selection re-enters instead of stepping
        // relative to the dead index.
        s.stroke_selected = Some(9);
        step_stroke_selection(&mut s, 1);
        assert_eq!(s.stroke_selected, Some(0));
        // Clear drops the selection, still silently.
        clear_stroke_selection(&mut s);
        assert_eq!(s.stroke_selected, None);
        assert_eq!(s.core.status, status);
        assert_eq!(s.core.undo_depth(), depth);
    }

    #[test]
    fn goto_press_navigates_to_the_selected_strokes_frame() {
        // Index 1 lives on frame 2, index 2 on frame 0.
        let mut s = state_with(core_with_strokes(), Some(1));
        assert!(goto_selected_stroke_frame(&mut s));
        assert_eq!(s.core.current, 2);
        s.stroke_selected = Some(2);
        assert!(goto_selected_stroke_frame(&mut s));
        assert_eq!(s.core.current, 0);
        // No selection: the press refuses with a status
        // and does not navigate.
        let mut s = state_with(core_with_strokes(), None);
        assert!(!goto_selected_stroke_frame(&mut s));
        assert_eq!(s.core.status, "No stroke selected.");
        assert_eq!(s.core.current, 0);
        // A stale selection refuses the same way.
        let mut s = state_with(core_with_strokes(), Some(7));
        assert!(!goto_selected_stroke_frame(&mut s));
        assert_eq!(s.core.status, "No stroke selected.");
    }

    #[test]
    fn selection_presses_on_an_empty_project_only_report() {
        let core = EditorCore::new();
        assert!(core.project.strokes.is_empty());
        // Even a stale selection is dropped, never
        // stepped, and nothing panics.
        let mut s = state_with(core, Some(4));
        step_stroke_selection(&mut s, 1);
        assert_eq!(s.stroke_selected, None);
        assert_eq!(s.core.status, "No strokes to select.");
        step_stroke_selection(&mut s, -1);
        assert_eq!(s.stroke_selected, None);
        assert_eq!(s.core.status, "No strokes to select.");
        clear_stroke_selection(&mut s);
        assert_eq!(s.core.status, "No strokes to select.");
        assert!(!goto_selected_stroke_frame(&mut s));
        assert_eq!(s.core.status, "No strokes to select.");
        assert_eq!(s.stroke_selected, None);
    }

    /// A stroke on `frame` whose polyline is exactly
    /// `points` — the measure tests need known geometry,
    /// which the repeated-point `plain_stroke` cannot
    /// give.
    fn polyline_stroke(frame: usize, points: &[[f32; 2]]) -> Stroke {
        let mut s = plain_stroke(frame, 0);
        s.points = points.to_vec();
        s
    }

    #[test]
    fn stroke_measure_sums_the_base_polyline_segments() {
        let mut core = EditorCore::new();
        // Right angle: (0,0) → (3,0) → (3,4) is 3 + 4 = 7
        // stage units over 3 points, recorded on frame 2.
        core.project
            .strokes
            .push(polyline_stroke(2, &[[0.0, 0.0], [3.0, 0.0], [3.0, 4.0]]));
        let m = stroke_measure(&core, 0).expect("index 0 exists");
        assert_eq!(m.points, 3);
        assert_eq!(m.length, 7.0);
        assert_eq!(m.frame, 2, "the frame is stroke_frame_of's raw value");
        // Out of range: no measure.
        assert_eq!(stroke_measure(&core, 1), None);
    }

    #[test]
    fn stroke_measure_handles_degenerate_polylines() {
        let mut core = EditorCore::new();
        core.project.strokes.push(polyline_stroke(0, &[]));
        core.project.strokes.push(polyline_stroke(1, &[[5.0, 5.0]]));
        assert_eq!(
            stroke_measure(&core, 0),
            Some(StrokeMeasure {
                points: 0,
                length: 0.0,
                frame: 0
            })
        );
        assert_eq!(
            stroke_measure(&core, 1),
            Some(StrokeMeasure {
                points: 1,
                length: 0.0,
                frame: 1
            })
        );
    }

    #[test]
    fn stroke_measure_uses_the_base_polyline_for_spray_strokes() {
        let mut core = EditorCore::new();
        let mut s = polyline_stroke(0, &[[0.0, 0.0], [6.0, 8.0]]);
        s.spray = Some(SprayMark {
            seed: 7,
            radius: 12.0,
            opacity: 0.5,
        });
        core.project.strokes.push(s);
        let m = stroke_measure(&core, 0).expect("index 0 exists");
        assert_eq!(m.points, 2);
        // The 6-8-10 hypotenuse of the base polyline — the
        // scattered spray dots play no part in the measure.
        assert_eq!(m.length, 10.0);
        assert_eq!(m.frame, 0);
    }

    #[test]
    fn measure_readout_formats_the_selected_strokes_facts() {
        let mut core = EditorCore::new();
        assert_eq!(measure_readout(&core, None), None);
        assert_eq!(measure_readout(&core, Some(0)), None, "empty project");
        core.project
            .strokes
            .push(polyline_stroke(2, &[[0.0, 0.0], [3.0, 0.0], [3.0, 4.0]]));
        // 1-based position and frame, whole-unit length.
        assert_eq!(
            measure_readout(&core, Some(0)).as_deref(),
            Some("Stroke 1: 3 pts · 7 long · frame 3")
        );
        // No selection or a stale one: no line at all.
        assert_eq!(measure_readout(&core, None), None);
        assert_eq!(measure_readout(&core, Some(1)), None);
    }

    #[test]
    fn width_after_steps_by_half_and_clamps() {
        // Mid-range: exactly one step of 0.5 each way,
        // and a magnitude-2 dir steps twice.
        assert_eq!(width_after(4.0, 1), 4.5);
        assert_eq!(width_after(4.0, -1), 3.5);
        assert_eq!(width_after(3.0, 2), 4.0);
        // Both clamps hold at and beyond the edges.
        assert_eq!(width_after(0.5, -1), 0.5);
        assert_eq!(width_after(0.4, -1), 0.5);
        assert_eq!(width_after(40.0, 1), 40.0);
        assert_eq!(width_after(39.9, 1), 40.0);
    }

    #[test]
    fn width_after_recovers_a_non_finite_width_from_the_default() {
        // A corrupt stored width steps from the default
        // brush width (4.0), never from the corrupt value.
        assert_eq!(width_after(f32::NAN, 1), 4.5);
        assert_eq!(width_after(f32::NAN, -1), 3.5);
        assert_eq!(width_after(f32::INFINITY, 1), 4.5);
        assert_eq!(width_after(f32::NEG_INFINITY, -1), 3.5);
    }

    #[test]
    fn set_selected_stroke_width_writes_and_scales_the_taper() {
        let mut core = EditorCore::new();
        let mut s = plain_stroke(0, 3);
        s.widths = vec![2.0, 4.0, 6.0];
        core.project.strokes.push(s);
        let depth = core.undo_depth();
        assert!(set_selected_stroke_width(&mut core, Some(0), 1));
        let stroke = &core.project.strokes[0];
        assert_eq!(stroke.width, 4.5);
        // Scale is 4.5 / 4.0 = 1.125: the taper keeps
        // its shape, grown by the same ratio.
        assert_eq!(stroke.widths, vec![2.25, 4.5, 6.75]);
        assert_eq!(core.undo_depth(), depth + 1, "exactly one snapshot");
        assert_eq!(core.status, "Stroke 1 width 4.5.");
    }

    #[test]
    fn set_selected_stroke_width_uniform_stroke_keeps_empty_widths() {
        let mut core = EditorCore::new();
        // plain_stroke is the uniform form: width 4.0,
        // no per-point widths — the empty vec must stay
        // empty, not be fabricated into a taper.
        core.project.strokes.push(plain_stroke(0, 2));
        let depth = core.undo_depth();
        assert!(set_selected_stroke_width(&mut core, Some(0), -1));
        let stroke = &core.project.strokes[0];
        assert_eq!(stroke.width, 3.5);
        assert!(stroke.widths.is_empty());
        assert_eq!(core.undo_depth(), depth + 1);
        assert_eq!(core.status, "Stroke 1 width 3.5.");
    }

    #[test]
    fn set_selected_stroke_width_zero_old_width_leaves_the_taper_untouched() {
        let mut core = EditorCore::new();
        let mut s = plain_stroke(0, 2);
        s.width = 0.0;
        s.widths = vec![1.0, 2.0];
        core.project.strokes.push(s);
        assert!(set_selected_stroke_width(&mut core, Some(0), 1));
        let stroke = &core.project.strokes[0];
        // The base width still steps (0.0 + 0.5, at the
        // clamp floor), but no ratio is taken from a
        // zero old width — the taper is left as stored.
        assert_eq!(stroke.width, 0.5);
        assert_eq!(stroke.widths, vec![1.0, 2.0]);
    }

    #[test]
    fn set_selected_stroke_width_without_a_selection_is_a_silent_no_op() {
        let mut core = core_with_strokes();
        let depth = core.undo_depth();
        let status_before = core.status.clone();
        // No selection, and a stale one past the end:
        // both refuse silently — no write, no snapshot,
        // no status.
        assert!(!set_selected_stroke_width(&mut core, None, 1));
        assert!(!set_selected_stroke_width(&mut core, Some(99), -1));
        assert_eq!(core.undo_depth(), depth, "no snapshot taken");
        assert_eq!(core.status, status_before, "no status written");
        assert_eq!(core.project.strokes[0].width, 4.0, "width untouched");
    }

    #[test]
    fn selected_stroke_width_reads_only_a_valid_selection() {
        let mut core = core_with_strokes();
        assert_eq!(selected_stroke_width(&core, None), None);
        assert_eq!(selected_stroke_width(&core, Some(0)), Some(4.0));
        assert_eq!(selected_stroke_width(&core, Some(99)), None);
        core.project.strokes.clear();
        assert_eq!(selected_stroke_width(&core, Some(0)), None);
    }

    /// f32 step arithmetic at a 0.1 step is not exact
    /// (0.5 + 0.1 lands a hair above 0.6), so opacity
    /// steps compare within a hair, never exactly —
    /// unlike the width tests above, whose 0.5 steps
    /// are exact in binary.
    fn near(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-6
    }

    #[test]
    fn opacity_after_steps_by_a_tenth() {
        assert!(near(opacity_after(0.5, 1), 0.6));
        assert!(near(opacity_after(0.5, -1), 0.4));
        // A magnitude-2 dir steps twice, as width does.
        assert!(near(opacity_after(0.5, 2), 0.7));
        // No direction: the value holds.
        assert!(near(opacity_after(0.5, 0), 0.5));
    }

    #[test]
    fn opacity_after_clamps_at_full_and_at_the_faint_floor() {
        // At and past the top the value is exactly the
        // clamp constant, 1.0.
        assert_eq!(opacity_after(1.0, 1), 1.0);
        assert_eq!(opacity_after(0.98, 1), 1.0);
        // The floor is 0.05 — faint but visible — never
        // 0.0, however far down the press starts.
        assert_eq!(opacity_after(0.05, -1), 0.05);
        assert_eq!(opacity_after(0.1, -1), 0.05);
        assert_eq!(opacity_after(0.0, -1), 0.05);
    }

    #[test]
    fn opacity_after_recovers_a_non_finite_opacity_to_full() {
        // A corrupt stored opacity steps from fully
        // opaque (the model's default), never from the
        // corrupt value itself.
        assert!(near(opacity_after(f32::NAN, -1), 0.9));
        assert_eq!(opacity_after(f32::NAN, 1), 1.0);
        assert!(near(opacity_after(f32::INFINITY, -1), 0.9));
        assert!(near(opacity_after(f32::NEG_INFINITY, -1), 0.9));
    }

    #[test]
    fn opacity_label_formats_percents_with_half_away_rounding() {
        assert_eq!(opacity_label(0.0), "0%");
        assert_eq!(opacity_label(0.05), "5%");
        assert_eq!(opacity_label(0.5), "50%");
        assert_eq!(opacity_label(0.55), "55%");
        assert_eq!(opacity_label(1.0), "100%");
        // 0.575 × 100 lands on exactly 57.5 in f32 and
        // the round breaks the tie away from zero:
        // 58%, not 57%.
        assert_eq!(opacity_label(0.575), "58%");
    }

    #[test]
    fn selected_stroke_opacity_reads_only_a_steppable_selection() {
        let mut core = core_with_strokes();
        core.project.strokes[0].opacity = 0.5;
        assert_eq!(selected_stroke_opacity(&core, None), None);
        assert_eq!(selected_stroke_opacity(&core, Some(0)), Some(0.5));
        assert_eq!(selected_stroke_opacity(&core, Some(99)), None);
        // A spray stroke yields nothing: its visible
        // opacity lives on the mark, not this field.
        core.project.strokes[1].spray = Some(SprayMark {
            seed: 7,
            radius: 12.0,
            opacity: 0.5,
        });
        assert_eq!(selected_stroke_opacity(&core, Some(1)), None);
        core.project.strokes.clear();
        assert_eq!(selected_stroke_opacity(&core, Some(0)), None);
    }

    #[test]
    fn set_selected_stroke_opacity_writes_the_field_as_one_undo_step() {
        let mut core = EditorCore::new();
        let mut s = plain_stroke(0, 3);
        s.opacity = 0.5;
        core.project.strokes.push(s);
        let depth = core.undo_depth();
        assert!(set_selected_stroke_opacity(&mut core, Some(0), 1));
        assert!(near(core.project.strokes[0].opacity, 0.6));
        assert_eq!(core.undo_depth(), depth + 1, "exactly one snapshot");
        assert_eq!(core.status, "Stroke 1 opacity 60%.");
        // The one press is exactly one undo step.
        core.undo();
        assert_eq!(core.project.strokes[0].opacity, 0.5);
    }

    #[test]
    fn set_selected_stroke_opacity_without_a_selection_is_a_silent_no_op() {
        let mut core = core_with_strokes();
        core.project.strokes[0].opacity = 0.5;
        let depth = core.undo_depth();
        let status_before = core.status.clone();
        // No selection, and a stale one past the end:
        // both refuse silently — no write, no snapshot,
        // no status.
        assert!(!set_selected_stroke_opacity(&mut core, None, 1));
        assert!(!set_selected_stroke_opacity(&mut core, Some(99), -1));
        assert_eq!(core.undo_depth(), depth, "no snapshot taken");
        assert_eq!(core.status, status_before, "no status written");
        assert_eq!(core.project.strokes[0].opacity, 0.5, "opacity untouched");
    }

    #[test]
    fn set_selected_stroke_opacity_refuses_a_spray_stroke() {
        let mut core = EditorCore::new();
        let mut s = plain_stroke(0, 3);
        // The stroke's own field carries the default
        // 1.0; the mark carries the opacity the dots
        // actually render with.
        s.spray = Some(SprayMark {
            seed: 7,
            radius: 12.0,
            opacity: 0.5,
        });
        core.project.strokes.push(s);
        let depth = core.undo_depth();
        let status_before = core.status.clone();
        // Both directions refuse silently: neither
        // opacity is written, no snapshot, no status.
        assert!(!set_selected_stroke_opacity(&mut core, Some(0), -1));
        assert!(!set_selected_stroke_opacity(&mut core, Some(0), 1));
        assert_eq!(core.undo_depth(), depth, "no snapshot taken");
        assert_eq!(core.status, status_before, "no status written");
        let stroke = &core.project.strokes[0];
        assert_eq!(stroke.opacity, 1.0, "the stroke field is not written");
        assert_eq!(
            stroke.spray.as_ref().map(|m| m.opacity),
            Some(0.5),
            "the mark's opacity is not written either"
        );
    }

    #[test]
    fn stroke_panel_builds_with_spray_and_plain_selections() {
        let t = rune::tok::theme(true);
        // Spray selection: the opacity row's render
        // branch (the `selected_stroke_opacity` gate)
        // is not taken, so the row is absent — the
        // panel builds without it.
        let mut core = core_with_strokes();
        core.project.strokes[0].spray = Some(SprayMark {
            seed: 7,
            radius: 12.0,
            opacity: 0.5,
        });
        let s = state_with(core, Some(0));
        let _ = stroke_panel(&s, &t);
        // Plain selection: the branch is taken and the
        // panel builds with the row.
        let s = state_with(core_with_strokes(), Some(0));
        let _ = stroke_panel(&s, &t);
    }

    #[test]
    fn stroke_panel_builds_with_and_without_strokes() {
        let t = rune::tok::theme(true);
        // Empty project: the selection row renders its
        // "No strokes" readout alongside everything else.
        let s = state_with(EditorCore::new(), None);
        let _ = stroke_panel(&s, &t);
        // Strokes present, one selected: the row renders
        // the position/frame readout.
        let s = state_with(core_with_strokes(), Some(1));
        let _ = stroke_panel(&s, &t);
    }

    /// A jittery near-straight stroke: seven points
    /// alternating ±1.0 around the x-axis. Every
    /// interior point sits within 1.0 of the chord, so
    /// Douglas-Peucker at [`SIMPLIFY_EPSILON`] (2.0)
    /// drops them all and keeps only the endpoints.
    fn jitter_stroke() -> Stroke {
        let mut s = plain_stroke(0, 0);
        s.points = vec![
            [0.0, 0.0],
            [8.0, 1.0],
            [16.0, 0.0],
            [24.0, -1.0],
            [32.0, 0.0],
            [40.0, 1.0],
            [48.0, 0.0],
        ];
        s
    }

    /// A stroke with one deliberate corner: the middle
    /// point stands 50 units off the chord, far beyond
    /// [`SIMPLIFY_EPSILON`], so the probe keeps all
    /// three points — already simple at this epsilon.
    fn corner_stroke() -> Stroke {
        let mut s = plain_stroke(0, 0);
        s.points = vec![[0.0, 0.0], [50.0, 50.0], [100.0, 0.0]];
        s
    }

    #[test]
    fn simplify_epsilon_matches_the_draw_panels_value() {
        // The two Simplify presses (this panel's
        // per-stroke one, the Draw panel's frame-wide
        // one) share one tolerance — pinned against the
        // sibling module's constant, not just the
        // number, so the two cannot drift apart.
        assert_eq!(SIMPLIFY_EPSILON, 2.0);
        assert_eq!(
            SIMPLIFY_EPSILON,
            crate::rune_sec::draw_tools::SIMPLIFY_EPSILON
        );
    }

    #[test]
    fn simplify_selected_reduces_a_jittery_stroke_in_one_undo_step() {
        let mut core = EditorCore::new();
        core.project.strokes.push(jitter_stroke());
        let mut s = state_with(core, Some(0));
        let depth = s.core.undo_depth();
        simplify_selected(&mut s);
        // Every jitter point is gone; the endpoints
        // survive verbatim (simplification never moves
        // a kept point).
        assert_eq!(
            s.core.project.strokes[0].points,
            vec![[0.0, 0.0], [48.0, 0.0]]
        );
        assert_eq!(s.core.status, "Stroke 1 simplified to 2 points.");
        assert_eq!(s.core.undo_depth(), depth + 1, "exactly one snapshot");
        // And the one press is exactly one undo step.
        s.core.undo();
        assert_eq!(
            s.core.project.strokes[0].points.len(),
            7,
            "one undo restores every point"
        );
    }

    #[test]
    fn simplify_selected_on_an_already_simple_stroke_takes_no_snapshot() {
        let mut core = EditorCore::new();
        core.project.strokes.push(corner_stroke());
        let mut s = state_with(core, Some(0));
        let depth = s.core.undo_depth();
        simplify_selected(&mut s);
        // The probe kept every point, so the press only
        // reports — no snapshot, no undo step, no write.
        assert_eq!(s.core.status, "Stroke 1 is already simple.");
        assert_eq!(s.core.undo_depth(), depth, "no snapshot taken");
        assert_eq!(
            s.core.project.strokes[0].points,
            vec![[0.0, 0.0], [50.0, 50.0], [100.0, 0.0]],
            "points untouched"
        );
    }

    #[test]
    fn simplify_selected_without_a_selection_is_a_silent_no_op() {
        let mut s = state_with(core_with_strokes(), None);
        let depth = s.core.undo_depth();
        let status_before = s.core.status.clone();
        simplify_selected(&mut s);
        assert_eq!(s.core.undo_depth(), depth, "no snapshot taken");
        assert_eq!(s.core.status, status_before, "no status written");
        // A stale selection past the end: same silence.
        s.stroke_selected = Some(99);
        simplify_selected(&mut s);
        assert_eq!(s.core.undo_depth(), depth, "no snapshot taken");
        assert_eq!(s.core.status, status_before, "no status written");
    }

    #[test]
    fn stroke_panel_builds_for_the_simplify_button_with_and_without_a_selection() {
        let t = rune::tok::theme(true);
        // No selection: the Simplify button's render
        // branch (the width row's valid-selection
        // condition) is not taken, so the button is
        // absent — the panel builds without it.
        let s = state_with(core_with_strokes(), None);
        let _ = stroke_panel(&s, &t);
        // Valid selection: the branch is taken and the
        // panel builds with the button.
        let s = state_with(core_with_strokes(), Some(0));
        let _ = stroke_panel(&s, &t);
    }

    // ---- Wave 12 (agent D3): selected stroke scatter ----

    #[test]
    fn scatter_after_steps_by_one_and_clamps() {
        // Mid-range: exactly one step of 1.0 each way,
        // and a magnitude-2 dir steps twice.
        assert_eq!(scatter_after(5.0, 1), 6.0);
        assert_eq!(scatter_after(5.0, -1), 4.0);
        assert_eq!(scatter_after(5.0, 2), 7.0);
        // The floor is 0.0 — scatter off — reachable
        // from the smallest positive value and held.
        assert_eq!(scatter_after(0.0, -1), 0.0);
        assert_eq!(scatter_after(0.5, -1), 0.0);
        // The cap is 24.0, twice the Spray preset's
        // 12.0; both edges hold at and beyond it.
        assert_eq!(scatter_after(24.0, 1), 24.0);
        assert_eq!(scatter_after(23.5, 1), 24.0);
        // A negative stored value (a corrupt file)
        // clamps up to the floor on the first press.
        assert_eq!(scatter_after(-2.0, -1), 0.0);
    }

    #[test]
    fn scatter_after_recovers_a_non_finite_scatter_to_off() {
        // A corrupt stored scatter steps from 0.0 (the
        // model's serde default), never from itself.
        assert_eq!(scatter_after(f32::NAN, 1), 1.0);
        assert_eq!(scatter_after(f32::NAN, -1), 0.0);
        assert_eq!(scatter_after(f32::INFINITY, 1), 1.0);
        assert_eq!(scatter_after(f32::NEG_INFINITY, -1), 0.0);
    }

    #[test]
    fn selected_stroke_scatter_reads_only_a_steppable_selection() {
        let mut core = core_with_strokes();
        core.project.strokes[0].scatter = 5.0;
        assert_eq!(selected_stroke_scatter(&core, None), None);
        assert_eq!(selected_stroke_scatter(&core, Some(0)), Some(5.0));
        assert_eq!(selected_stroke_scatter(&core, Some(99)), None);
        // A spray stroke yields nothing: it renders
        // from its mark and never reaches the scatter
        // dab path, so its stored scatter — even a
        // non-zero one from a loaded file — is not a
        // value the steppers may present as live.
        core.project.strokes[1].spray = Some(SprayMark {
            seed: 7,
            radius: 12.0,
            opacity: 0.5,
        });
        core.project.strokes[1].scatter = 9.0;
        assert_eq!(selected_stroke_scatter(&core, Some(1)), None);
        core.project.strokes.clear();
        assert_eq!(selected_stroke_scatter(&core, Some(0)), None);
    }

    #[test]
    fn set_selected_stroke_scatter_writes_the_field_as_one_undo_step() {
        let mut core = EditorCore::new();
        // plain_stroke carries scatter 0.0: the first
        // press turns scatter on at radius 1.
        core.project.strokes.push(plain_stroke(0, 3));
        let depth = core.undo_depth();
        assert!(set_selected_stroke_scatter(&mut core, Some(0), 1));
        assert_eq!(core.project.strokes[0].scatter, 1.0);
        assert_eq!(core.undo_depth(), depth + 1, "exactly one snapshot");
        assert_eq!(core.status, "Stroke 1 scatter 1.");
        // Stepping back down to 0 renders "Off" in the
        // status, the Preset scatter row's vocabulary.
        assert!(set_selected_stroke_scatter(&mut core, Some(0), -1));
        assert_eq!(core.project.strokes[0].scatter, 0.0);
        assert_eq!(core.status, "Stroke 1 scatter Off.");
        // Each press was exactly one undo step.
        core.undo();
        assert_eq!(core.project.strokes[0].scatter, 1.0);
        core.undo();
        assert_eq!(core.project.strokes[0].scatter, 0.0);
    }

    #[test]
    fn set_selected_stroke_scatter_without_a_selection_is_a_silent_no_op() {
        let mut core = core_with_strokes();
        core.project.strokes[0].scatter = 5.0;
        let depth = core.undo_depth();
        let status_before = core.status.clone();
        // No selection, and a stale one past the end:
        // both refuse silently — no write, no snapshot,
        // no status.
        assert!(!set_selected_stroke_scatter(&mut core, None, 1));
        assert!(!set_selected_stroke_scatter(&mut core, Some(99), -1));
        assert_eq!(core.undo_depth(), depth, "no snapshot taken");
        assert_eq!(core.status, status_before, "no status written");
        assert_eq!(core.project.strokes[0].scatter, 5.0, "scatter untouched");
    }

    #[test]
    fn set_selected_stroke_scatter_refuses_a_spray_stroke() {
        let mut core = EditorCore::new();
        let mut s = plain_stroke(0, 3);
        s.spray = Some(SprayMark {
            seed: 7,
            radius: 12.0,
            opacity: 0.5,
        });
        core.project.strokes.push(s);
        let depth = core.undo_depth();
        let status_before = core.status.clone();
        // Both directions refuse silently: the stored
        // scatter is not written, no snapshot, no
        // status — the MCP op's refusal, in panel form.
        assert!(!set_selected_stroke_scatter(&mut core, Some(0), 1));
        assert!(!set_selected_stroke_scatter(&mut core, Some(0), -1));
        assert_eq!(core.undo_depth(), depth, "no snapshot taken");
        assert_eq!(core.status, status_before, "no status written");
        assert_eq!(
            core.project.strokes[0].scatter, 0.0,
            "the stored scatter is not written"
        );
    }

    #[test]
    fn stroke_summary_reports_taper_and_scatter() {
        // A per-point taper is admitted in the summary
        // (Wave 12, D3): before this, nothing in the
        // panel showed the `widths` vec existed.
        let core = EditorCore::new();
        let mut tapered = plain_stroke(0, 3);
        tapered.widths = vec![2.0, 4.0, 2.0];
        assert_eq!(stroke_summary(&core, &tapered), "3 pts · w 4 · taper");
        // An ordinary stroke with scatter on reports
        // its radius, in the spray clause's format.
        let mut scattered = plain_stroke(0, 6);
        scattered.scatter = 5.0;
        assert_eq!(stroke_summary(&core, &scattered), "6 pts · w 4 · scatter r5");
        // Scatter off (the serde default): no clause.
        assert_eq!(stroke_summary(&core, &plain_stroke(0, 6)), "6 pts · w 4");
        // A spray stroke reports its mark, never its
        // stored scatter — even a non-zero one.
        let mut sprayed = plain_stroke(0, 20);
        sprayed.scatter = 9.0;
        sprayed.spray = Some(SprayMark {
            seed: 42,
            radius: 12.0,
            opacity: 0.5,
        });
        assert_eq!(
            stroke_summary(&core, &sprayed),
            "20 pts · w 4 · spray r12 · 50%"
        );
    }

    #[test]
    fn stroke_panel_builds_with_the_scatter_row_branches() {
        let t = rune::tok::theme(true);
        // Plain selection: the scatter row's render
        // branch (the `selected_stroke_scatter` gate)
        // is taken and the panel builds with the row.
        let s = state_with(core_with_strokes(), Some(0));
        let _ = stroke_panel(&s, &t);
        // Spray selection: the branch is not taken, so
        // the row is absent — the panel builds without
        // it, like the opacity row's spray branch.
        let mut core = core_with_strokes();
        core.project.strokes[0].spray = Some(SprayMark {
            seed: 7,
            radius: 12.0,
            opacity: 0.5,
        });
        let s = state_with(core, Some(0));
        let _ = stroke_panel(&s, &t);
        // No selection: neither selected-stroke row
        // renders — the panel builds in its compact
        // unselected form.
        let s = state_with(core_with_strokes(), None);
        let _ = stroke_panel(&s, &t);
    }
}
