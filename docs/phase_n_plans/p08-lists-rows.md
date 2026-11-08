# P8 — Lists & Rows Spec (Phase N)

Scope: every list in the shell and the one row anatomy
(charter §5) mapped onto it. Files studied:
`figures_panel.rs`, `library_panel.rs`, `home_panel.rs`,
`clips_panel.rs`, `layers_panel.rs`, `fx_clips.rs`,
`pose_tools.rs`, `text_tool.rs`. All builders cited are
in `src/rune_sec/`. No code changes in this plan; the
work items are for the adoption (A) wave, one file each.

Kit dependency (charter "The kit"): everything below
builds on `design::list_row(key, selected, t)` and
`design::empty_state(icon, message, t)` from
`src/rune_sec/design.rs` (foundation wave). Where a row
is pressable, note the kit shell takes **no handler** —
the adoption keeps the row's existing `.on_press(...)`
attached to the returned `El`, handlers verbatim.

---

## 1. The one list-row anatomy

Every list row in the app becomes this, top to bottom
of the charter's slots:

| Slot | Spec |
|---|---|
| **Shell** | `design::list_row(key, selected, t)`: min height 28, px 8, radius 8 (`ROW_RADIUS`), hover `t.hover`. Rest fill transparent (the panel card shows through). |
| **Selected** | `design::selected_fill()` — accent at 0.16 alpha; accent border at 0.35 alpha where the row draws a border. Selection is carried by the shell fill, never by a marker dot or by brightening one text child alone. |
| **Leading slot** | Fixed-size, before the text, vertically centred: a thumbnail canvas, a 10px colour dot, or a 13px icon in `text_secondary`. One slot, one size per list — never two leading visuals. Vacant only where the list's data has no honest visual (library, §3). |
| **Primary text** | `type_::LABEL`, `t.text`, grows (`grow(1.0)`). Dimmed to `text_secondary` only for a *state* (hidden figure), never for unselected. |
| **Meta text** | `type_::LABEL`, `t.text_secondary`, after the primary text; purely numeric readouts (opacity %, segment counts) prefer `MONO_CAPTION` (charter §12 spirit). Never grows. |
| **Trailing actions** | The one button family only (`icon_btn` / `icon_only_btn` 26.0 / `btn` + inert twins), gap 6–8, in the order the row has today. No per-file button copies. |
| **Editor variant** | A rename/confirm editor renders *under* the row (library/pose convention) or *replaces* the row's content under the same key (home confirm convention). It is not a second row style. |
| **Two-line variant** | Where a row genuinely carries two control lines (figures, home, pose entries), the shell is a `col` with the same fill/hover/radius/px and py 4–6, lines at gap 2–4. This is the only sanctioned deviation from the 28px single line, and the fill/selected rules are identical. |
| **Empty state** | `design::empty_state(icon, message, t)` — centred 20px icon + LABEL text, both `text_secondary`, py 12 — replaces every bare "nothing here" text row (charter §10). **The message string stays byte-identical** to the text it replaces; only its presentation changes. |

Cross-list rules:

- **No key is renamed, ever.** One existing exception
  stays an exception: clips rows are keyed 1-based
  (`clip-row-{index + 1}`) while every other list is
  0-based. Do not "fix" it — the gate diffs key sets.
- **No new keys are needed by this spec.** Every item
  below restyles elements that already have keys, or
  adds unkeyed kit elements (`empty_state`,
  `section_label`, `divider` carry no key in the fixed
  API). One flagged exception: text *segment* rows
  (§9b) have no keys today and the kit shell requires
  one — see the blocker note there.
- Row gap inside a list container: 4 (dense lists:
  figures, fx, layers) or 6 (card lists: home,
  library) — pick the value the list uses today; do
  not invent a third.

## 2. Where the lists stand today (summary)

| List | Shell today | Hover | Selected today | Leading slot | Empty state today |
|---|---|---|---|---|---|
| Figures (`fig-row-{fi}`) | col, px 6 py 4, r 6, fill accent 0.22 or transparent | `control_track` | accent fill **0.22** (charter: 0.16) | 44px thumb canvas + 10px dot (**two** visuals) | bare text, filter-no-match only |
| Library (`library-row-{i}`) | bare `row()`, no px/py/fill/radius | none | none (rename target only implied) | **none** | bare `group_label` text ×2 |
| Home projects (`home-row-{i}`) | row, px 10 py 6, r 8, fill `t.bg` | none | **none — open project unmarked** | none | bare `section_label` text |
| Movieclips (`clip-row-{n}`, 1-based) | row, px 8 py 4, r 6, fill `control_track`/`card` | `control_track` | `control_track` fill | none | bare text |
| Layers (`layer-row-{fi}`) | bare `row()`, no fill/radius | none | only the name **button** goes accent | 10px dot (`layer-dot-{fi}`) | **none at all** |
| FX clips (`fx-clip-{i}`) | row, px 8 py 5, r 6, fill `control_track`/`card` | `control_track`/`control_off` | `control_track` fill **+ 8px accent marker dot** | 10px dot (unkeyed) | bare text |
| Pose library (`pose-lib-row-{i}`) | bare `col()`, no fill/radius | none | none (rename target unmarked) | none | **none — whole block vanishes** |
| Text instances (`text-row-{i}`) | bare `row()`, no fill/radius | none | **none — loaded instance unmarked** | none (name is a button) | bare text |
| Text segments (unkeyed, `text-list`) | bare text lines, not rows | n/a | text colour only | none | bare text |

Empty-state inventory — **lists with no empty state
at all today: layers, pose library.** Lists whose
empty state is a bare text row (to be replaced by the
kit element): figures (filter case), library (×2),
home (×2 messages, one element), movieclips, FX
clips, text instances, text segments. Lists where a
zero-item state is unreachable in practice (engine
refuses to delete the last figure): figures and
layers with 0 figures — their filter/zero branches
are still specified below for completeness.

---

## 3. `figures_panel.rs` — figure rows + thumbnails

**Current structure** — `figure_row()` (two-line
variant): root `col` keyed `fig-row-{fi}`, px 6 / py 4,
radius 6, gap 2. Line 1: thumbnail canvas
`fig-thumb-{fi}` (`THUMB_SIZE` 44.0, `thumb_shape()` —
live rig silhouette, head ring at the project's
scaled radius), then a pressable dot+name cluster
`fig-sel-{fi}` (10px colour dot, name LABEL; name
dimmed when the figure is hidden), opacity readout
`"{pct}%"`, then `fig-lock-{fi}` (icon_only, lock /
lock-open), `fig-solo-{fi}` (`row_toggle` "solo"),
`fig-op-dn-{fi}` / `fig-op-up-{fi}` (`row_toggle`
−/+), `fig-eye-{fi}` (icon_only, eye / eye-slash).
Line 2, right-aligned: `fig-up-{fi}` /
`fig-dn-{fi}` (icon_only chevrons), `fig-opac-{fi}`
(`row_toggle` "opac"), `fig-del-{fi}` (icon_only
trash-can). Row data is the `FigureRow` projection
(`figure_rows()`); the list is filtered by
`visible_figures()` under the `figures-filter` field.

**Selected / hover** — selected fill is
`accent.alpha(0.22)`; hover `t.control_track` on all
rows. Closest to the charter of any list, but the
alpha and radius are off-spec, and hover should be
`t.hover`.

**Thumbnail / icon slot** — the reference
implementation for the anatomy: fixed 44×44 canvas,
pure view (no handler). Deviation: the colour dot
*also* sits in line 1 (inside `fig-sel-{fi}`), so the
row has two leading visuals. The dot is part of the
pressable select cluster and duplicates the colour
the thumbnail already draws — keep both (removing the
dot changes the select cluster's hit area and look
for no charter requirement; the anatomy is satisfied
by reading thumb = leading slot, dot = part of the
primary-text cluster).

**Trailing actions** — nine controls, the densest
row in the shell; the two-line split is why. Four of
them go through the private `row_toggle()` helper
(px 6 / py 3, radius 5, own fill/hover/press), which
is a per-file button copy the charter retires.

**Empty states** — filter-no-match renders a bare
`text("No figures match \"…\".")`. Zero figures
renders nothing special (rows simply absent; header,
filter and footer still draw). Zero is reachable
only by loading an empty project — delete refuses
the last figure.

**Work items**

- **FIG-1** `figure_row()`: rebuild the root on the
  two-line variant of the anatomy — radius 6 →
  `ROW_RADIUS` (8), px 6 → 8, selected fill
  `accent.alpha(0.22)` → `design::selected_fill()`,
  hover `t.control_track` → `t.hover`. Keys
  `fig-row-{fi}` and all child keys byte-identical;
  line split and child order unchanged.
- **FIG-2** Retire `row_toggle()` onto the button
  family: `fig-solo-{fi}`, `fig-op-dn-{fi}`,
  `fig-op-up-{fi}`, `fig-opac-{fi}` become
  `icon_btn`/`icon_only_btn` (+ inert twins where a
  state needs one) at charter metrics (px 10 / py 5,
  radius 6). Handlers verbatim (`toggle_solo`,
  `step_figure_opacity(∓0.1)`, `cycle_row_opacity`).
  Delete the helper once its four call sites move.
- **FIG-3** Opacity readout `"{pct}%"` → 
...[truncated 15821 chars]