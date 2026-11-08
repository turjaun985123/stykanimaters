# Squad 06 Audit — Project Home & Dashboard

Phase 1 audit, Ultimate UI/UX Mission. Read-only inspection
2026-11-08 against current source (post-Phase N tree).
Files owned/inspected: `src/rune_sec/home_panel.rs`,
`src/rune_sec/project_tools.rs`, `src/projects.rs`,
`src/shell.rs` (recovery/recents/layout), plus
`src/export.rs` (renderer) and the Rune framework source
(`~/workspace/rune-src`) for thumbnail feasibility.

## 1. Current anatomy

There is **no dashboard or landing screen**. "Home" is a
collapsible card in the left column, toggled by a `home`
button, sitting in the shell among the editing panels. A
new user opens the app into the Animate workspace with the
demo project; Home is one panel among many, found by
scrolling the left column.

### 1.1 Home panel (`home_panel.rs`, key `home-panel`)

Kit card (`design::panel_col`). When open, top to bottom:

1. **Reopen last** (conditional) — accent `icon_btn`
   (`home-reopen-last`, `arrow-rotate-right`), label
   "Reopen {name}". Resolved from `shell::last_opened_project()`
   (recents store display *names*, capped at 10) by exact
   name match against the scan, skipping shell-owned and
   recovery entries; hidden when it names the open project.
2. **Header row** — `panel_title` "Saved projects in
   ./projects" + `home-new` ("New project", `file` glyph).
   A kit divider sits under it.
3. **Recovery banner** (conditional) — accent-tinted row
   (`home-recovery`): "Unsaved recovery snapshot is newer
   than the last save." + a "Recover" button. See §3.
4. **Search row** — magnifying-glass glyph, `home-search`
   text field (substring, case-insensitive, name only),
   a visible-count label, and the `home-sort` toggle
   ("Sort: recent" / "Sort: A–Z").
5. **Rows** (`home-rows`) — one kit `list_row` per project:
   display name (`TITLE`), one metadata line
   (`home-proj-meta-{i}`: "{n} figures · {m} frames ·
   {fps} fps · {size} · modified {YYYY-MM-DD HH:MM}"),
   and Open (`icon_btn`, `folder-open`) / Duplicate /
   Delete buttons. The open project's row carries the kit
   selected fill. Metadata comes from a light per-file
   read (top-level array counts only) cached on mtime,
   falling back to the full scan's numbers.
6. **Totals footer** (`home-totals`, non-empty list only):
   "{n} projects · {f} figures · {m} frames" over the
   *visible* (filtered/sorted) rows.
7. **Empty states** — done properly since Phase N:
   kit `empty_state` with glyph + line for both cases
   ("No saved projects yet — use \"Save project\" to add
   one." / "No projects match the search.").

The scan itself is mtime-cached (`HomePanelState::projects`)
because `projects::scan` fully parses every file; shell-owned
files (`.recovery.json`, `.layout.json`, `recents.json`,
any dotfile) are filtered at one choke point
(`visible_projects`) after the Phase E incident where a
stray ".layout" row shipped.

### 1.2 Project section (`project_tools.rs`, key `project-tools`)

A **second, overlapping project interface** in the same
shell: name field + "Set name", conditional "Current file:"
line + "{size} · modified {time}" line, "Reload from disk",
New / Save project / Recover buttons, an autosave line
("Recovery snapshot up to date." / "{n} edits since the
last recovery snapshot."), and its own selector list
(`proj-list`) — label-only rows (the file *stem*, e.g.
`my-film`, not the display name) with an Open button each.
Its empty list is still a **bare text line** ("No saved
projects in ./projects/ yet."), not the kit empty state —
one of the lines the Phase O charter seeded.

### 1.3 Backend (`projects.rs`, `shell.rs`)

- `projects::scan` — full `Project` parse per file, newest
  first, silent skip of unparseable files.
- `save` / `rename_project` / `duplicate_project`
  (`<stem>-copy.json`, incrementing) / `filter_projects` /
  `load`. **`rename_project` has no UI caller anywhere** —
  the only rename path is editing the name field and
  re-saving (which writes a *new* file and orphans the old).
- `shell.rs` — recovery snapshot (`.recovery.json`),
  recents (names, cap 10), layout prefs, and
  `delete_project_file`, which refuses shell-owned names.

## 2. Flows

- **Open** — Home row and project-tools row both route to
  `EditorCore::open_project_file` (index-based), record a
  recent, re-sync the name field. No unsaved-work prompt on
  any open path (the recovery snapshot is the safety net,
  and undo survives the swap by design).
- **New** — two buttons, **two different projects** (the
  standing disagreement, user's product call — flagged,
  not silently fixable): Home's `home-new` installs
  `Project::new_demo()` (the 5-frame demo); project-tools'
  `new-proj` installs `project_tools::blank_project`
  (960×720, 1 frame, 1 figure). Neither confirms; both drop
  unsaved work instantly.
- **Duplicate** — Home only. File copy with identical bytes,
  so the copy carries the **same internal display name**:
  the list can show two indistinguishable "My Film" rows
  differing only in metadata/size.
- **Delete** — Home only, and the best flow in the area:
  a row's Delete only *arms*; the row re-renders in place
  as "Delete {name}?" with `danger_btn` "Delete file" /
  "Keep". `shell::delete_project_file` refuses shell-owned
  files as a backstop; success invalidates both caches;
  failure keeps the confirm armed with the error on the
  status line. **No undo** — deletion is final, and the
  confirm line does not say so. Deleting the currently
  open project's file silently leaves the in-memory copy
  editing as an unsaved project — deliberate (documented
  in the source), but the confirm copy never tells the user.
- **Recover** — three entry points (Home banner,
  project-tools "Recover", startup auto-offer) all replace
  `core.project` immediately, **with no confirmation** and
  no snapshot of the work being replaced beyond the
  periodic autosave. The project-tools Recover is always
  visible and ungated — it offers to overwrite the current
  project with the snapshot even when the snapshot is
  *older* than the current work.

## 3. Recovery presentation

Genuinely careful engineering, thin presentation:

- The newer-than verdict
  (`shell::recovery_is_newer_than_last_project`) is
  metadata-only and is captured at the right moments —
  at boot (`initial_state`, performing **no writes**, so
  judging never corrupts the evidence) and in
  `toggle_open` *before* the panel's own snapshot write.
- At boot an offer starts Home open with the banner up and
  the status line "A newer recovery file exists — open
  Home to restore it."
- But the banner itself is one sentence + Recover. It
  never says **what** the snapshot holds (name, figures,
  frames, when it was taken) — the user must choose blind
  between "last save" and "snapshot" with no way to know
  how much work either side represents.
- Once Home is open the verdict is frozen at open time;
  the autosave line in project-tools ("{n} edits since…")
  is the only live freshness signal, and it lives in a
  different panel from the banner it qualifies.
- The banner occupies the same visual weight whether the
  snapshot is seconds or weeks newer.

## 4. Thumbnail feasibility — VERDICT: FEASIBLE, genuinely

**A real rendering path exists at every layer. No fake is
needed — but it is real work, not a switch.**

Evidence, layer by layer:

1. **Renderer exists and is tested.**
   `projects::thumbnail(project, frame, out)` (projects.rs)
   renders via `export::render_frame_at` — the same
   supersampled CPU rasteriser the export pipeline and PNG
   exports use — and saves a PNG. Its test
   (`thumbnail_writes_a_stage_sized_png`) proves a real
   stage-sized PNG comes out. **It has zero callers** in
   the app or MCP today (grep: no `::thumbnail(` outside
   its own file and test).
2. **Downscale precedent exists.** `export::render_contact_sheet`
   already renders frames with `render_frame_at` and shrinks
   them with `image::imageops::resize` (Triangle filter) to
   a thumbnail width — the exact recipe a library thumbnail
   needs, proven in production code and exposed via MCP.
3. **Display path exists in the framework.** Rune has a
   content-addressed image system (`crates/rune/src/image.rs`):
   `ImageCache::rgba(width, height, bytes)` builds an image
   from an in-memory RGBA buffer (no file round-trip), an
   `Image` exposes `pixels() -> &Pixels`, and
   `El::image(pixels)` (el/pills.rs) is the element
   constructor. **The app uses none of it today** — every
   visual is canvas geometry or SVG-path icons — so this is
   a first use, not an established pattern.
4. **The headless snap machinery is NOT the path.** It
   renders whole app scenes for QA; it plays no role here.

Costs and constraints a plan must respect:

- `render_frame_at` renders at stage size ×2 supersample.
  Per project, per frame 0, that is a full export-grade
  render. In the Home builder (which runs per render) it
  must sit behind the existing cache discipline — mtime-keyed
  like `metadata_for`, rendered once per file version,
  ideally downscaled immediately and cached as small RGBA
  (e.g. ~112 px wide). Rendering inline on first paint of
  a large library will stall; a lazy/background fill or a
  "render on scan, once" step is the honest design point
  to decide in Phase 2.
- `projects::scan` currently discards the parsed `Project`;
  thumbnails need the parse (or the light read extended) —
  the scan already pays the parse cost, so generation
  belongs at scan/cache time, not in the row builder.
- Row geometry changes: a thumbnail breaks the current
  276 pt left-column row budget if placed naïvely; the
  redesign must state its row anatomy and width math
  (charter rule: budgets stated, not hoped).

**Recommendation:** plan thumbnails as a foundation item
(scan-time, mtime-cached RGBA at fixed small width via
`render_frame_at` + `imageops::resize`, displayed via
Rune's image element), sized as its own coder task with a
cache test — not folded into a row-restyle item.

## 5. Defects (prioritised)

**D1 — Two project libraries, diverging.** Home and the
Project section both list/open projects with different
row content (display name + rich metadata vs bare file
stems, no metadata), different empty-state treatment
(kit vs bare line), and different feature sets (search/
sort/duplicate/delete only in Home; rename/reload only
reachable via Project). A beginner cannot tell which is
"the" project place. *(Mission §5/§8: no confusing
duplicate controls.)*

**D2 — Recovery is a blind, unconfirmed overwrite.**
Banner offers no snapshot metadata (name/size/age/
contents); Recover (banner and project-tools) replaces
the current project instantly with no confirm; the
project-tools Recover is ungated by the newer-than
verdict, so it can silently replace newer work with an
older snapshot. The verdict/freshness signals that exist
(banner, autosave counter) are split across two panels.

**D3 — No rename in the UI.** `projects::rename_project`
is implemented, tested, and unreachable. Renaming via the
name field + Save creates a second file and orphans the
first; the library then shows both under confusingly
related names, and recents (name-keyed) can point at the
orphan.

**D4 — No thumbnails despite a proven renderer** (§4).
Rows are text-only; a library of similarly named projects
is indistinguishable at a glance. This is the single
biggest gap between Home and a "premium project-library
experience" — and it is closable with existing machinery.

**D5 — Delete confirm undersells its finality.**
Two-step arm/confirm is right, but the confirm line
doesn't state that deletion is permanent (no undo, no
trash), and doesn't warn when the file being deleted is
the currently open project (the session silently becomes
unsaved work).

**D6 — Duplicate rows are indistinguishable.** The copy
keeps the source's internal name; two rows can render the
same display name with no distinguishing mark beyond
metadata numbers the user must read closely.

**D7 — The two "New project" results disagree.**
`home-new` → 5-frame demo project; `new-proj` → 1-frame
blank. Recorded standing issue; it is the **user's
product call** — this audit flags it, and the mission
must not silently unify them without that decision.

**D8 — Timestamps are UTC.** `format_modified` renders UTC
("YYYY-MM-DD HH:MM") and project-tools' `modified_label`
compares UTC days for its "today" rule. A user at UTC+6
sees modification times six hours off their wall clock —
in the panel whose whole job is "find the project I was
just working on".

**D9 — Home is buried.** The library is a collapsible
left-column card in the editing shell, closed by default
unless a recovery is offered. First-run (empty library)
is only ever seen inside that card; there is no moment
where the app presents itself as a studio entrance
(mission §12 vision) — no recent-project emphasis, no
"start here" hierarchy beyond Reopen-last.

**D10 — Minor cluster.** Search matches name only (not
file stem); no project file path is ever shown in Home
(only the "./projects" header implies it); the totals
footer counts *visible* rows, which reads as a library
total while a search is active; Home has no keyboard
story of its own (rows are buttons only).

### What is already good (do not redesign away)

Two-step delete with shell-owned refusal backstop;
verdict-capture-before-write recovery timing; write-free
boot decision; mtime-cached scan + light metadata with
honest fallbacks (a vanished file degrades to the scan
line, never a blank or a guess); shell-owned filtering at
one choke point; both Home empty states on the kit;
current-project row marking; totals that never guess a
failed read (counts it as 0 and says why in the source).

## 6. Coder-sized items

1. **S06-1 — Snapshot facts in the banner.** Extend the
   recovery banner to show the snapshot's name/figures/
   frames/modified time from one light read
   (`project_metadata` on `.recovery.json`, shell.rs
   path constant), keys/handlers unchanged. *Files:
   home_panel.rs only. Proof: Runtime test on the banner
   contents + settings/home scene snap.*
2. **S06-2 — Gate project-tools Recover.** Hide or
   disable-with-reason the `recover` button when no
   snapshot exists or it is not newer (reuse
   `recovery_is_newer_than_last_project` at panel build,
   the banner's discipline); keep the Home banner path
   unchanged. *Files: project_tools.rs only. Proof:
   unit tests on the gating decision.*
3. **S06-3 — Confirm before Recover.** Route both Recover
   buttons through the Home delete pattern (arm → inline
   confirm line) before `dispatch_recover` /
   `recover_snapshot` runs. *Files: home_panel.rs,
   project_tools.rs (one coder, pattern copied, not
   abstracted). Proof: snap_tests for the armed state.*
4. **S06-4 — Rename row action.** Add a rename affordance
   to Home rows wired to the existing
   `projects::rename_project`, with the file-level status
   reporting duplicate code already has; recents
   re-touch on success. *Files: home_panel.rs only;
   backend exists. Proof: dispatch unit tests incl. the
   AlreadyExists refusal.*
5. **S06-5 — Delete confirm copy.** Confirm line gains
   "This cannot be undone." and, when the armed file is
   the open project, "The open project will stay in the
   editor as unsaved work." — copy only, no behaviour
   change. *Files: home_panel.rs only. Proof: existing
   confirm tests extended.*
6. **S06-6 — Duplicate naming.** After duplicate, offer
   the copy under a distinguished display (status names
   the new file — exists; add: the copy's row is the
   selected/flashed row on next render, or pre-fill a
   rename). Smallest honest version: status + sort
   keeps the copy adjacent. *Files: home_panel.rs only.
   Decision point for Phase 2 — do not invent a file
   format change.*
7. **S06-7 — project-tools parity pass.** Kit
   `empty_state` for the bare empty-list line;
   selector rows show display names via the scan the
   panel already can reach, or the section is formally
   slimmed to name/save/reload only (D1 decision).
   *Files: project_tools.rs only. Blocked on the D1
   direction from Squad 20/integrator.*
8. **S06-8 — Thumbnail foundation (see §4).**
   Scan-time, mtime-cached thumbnail RGBA
   (`render_frame_at` frame 0 → `imageops::resize` to a
   fixed width), a `HomePanelState` cache beside
   `meta_cache`, and one row-anatomy change displaying it
   via Rune's `El::image`. *Files: home_panel.rs +
   projects.rs (cache helper). Proof: cache unit tests
   (mtime hit/miss), headless home snap pixel-read.
   Largest item in this squad — split display from
   generation if needed.*
9. **S06-9 — Local-time rendering.** `format_modified`
   gains a local-offset variant or the panels render
   relative labels ("2 h ago") from the mtimes they
   already hold. *Files: home_panel.rs + project_tools.rs
   call sites. Note: no tz database dependency exists in
   the tree today — relative labels are the dependency-
   free option; flag for Squad 02.*

## 7. Open decisions for the integrator / user

- D1: one library or two? (Squad 20 call — S06-7 waits.)
- D7: the two "New project" behaviours — **user's
  product call, standing.** Not this mission's to settle
  silently.
- S06-8 thumbnail size/placement vs the 276 pt column
  budget — needs Squad 02/03 agreement before coding.
