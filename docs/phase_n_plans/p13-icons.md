# P13 — Icon usage audit (Phase N planning)

Census of every icon the shell uses, every plain-text
button that should carry one, and every glyph the
charter's design language needs that the curated set
does not yet have. Method: a brace-matching call-site
parser over `src/**/*.rs` (production code only —
everything before each file's `#[cfg(test)]` block,
plus `snap_tests.rs` excluded), every dynamic icon
source resolved by hand to its possible values, and
every proposed name checked against the files on
disk.

**Path correction first:** the brief named
`assets/icons/fontawesome/svgs/solid/`. The actual
on-disk layout is **`assets/icons/fontawesome/solid/`**
(2,001 SVGs; sibling dirs `regular/`, `brands/`).
All "exists" verdicts below are against that dir.

## Ground facts

- `src/rune_sec/icon_data.rs`: **111** names in
  `ICON_NAMES`, **111** `icon_data()` match arms —
  the two sets are identical, and **every curated
  name has a matching `solid/<name>.svg` file**.
- Icon-bearing production call sites: **182 with a
  literal icon argument** (`icon_btn`, `icon_btn_inert`,
  `icon_only_btn`, `icon_only_btn_inert`, `icon_el`,
  `icon_el_rgb`, `icon_shape`, `icon_shape_rgb`,
  `icon_data`, `has_icon`, plus the pass-through
  helpers `pibtn`, `ictl`, `danger_btn`,
  export's `range_row` / `bundle_row` / `plain_row`,
  fx/export `section_with_icon`, status_strip's
  `segment_with_icon`), and **39 with a dynamic
  argument**, each resolved in §2.
- Plain-text button call sites (§1): **253** through
  the `btn` family (`btn` 227, frame_ops' `fbtn` 15,
  pose_tools' `pbtn` 9, keyframe_tools' `kbtn` 2),
  plus **~22** through three_d_panel's local `ctl`
  helper and **1** through inspector_figure's
  text-only `danger_btn`. Five of the 253 are builder
  plumbing, not call sites (the two degrade fallbacks
  inside `icon_btn.rs`, menus' `item()`,
  figure_add's `entry_button`, export's `plain_row`
  fallback) — the format rows that flow through
  `plain_row` are counted once, in §1.
- The degrade contract (icon_btn.rs): an unknown
  icon name makes `icon_btn` silently fall back to
  the plain `btn`, and `icon_only_btn` to an empty
  square. There is no warning anywhere. That is why
  §2 matters and why §4 proposes a census gate.

---

## 1. Plain-text buttons with an obvious icon

Conversion means `btn`/`fbtn`/`pbtn`/`kbtn`/`ctl` →
`icon_btn` (and the local helper retired onto it,
per charter §4), handler and label **verbatim** —
several labels already contain a glyph character
(`"+ Add figure"`, `"< Shift"`); the charter's
"never change a string" rule wins, so the redundant
character stays unless the user approves a copy pass.
That decision is flagged once here, not per row.

**Status key:** **have** = name already in
`ICON_NAMES`, convert today. **add** = name verified
present in `solid/` but not curated — convert after
the §3 addition lands.

### Tier A — icon already curated

| File | Line | Key | Label | Proposed icon |
|---|---|---|---|---|
| src/chrome_view.rs | 165 | dup-frame | Duplicate frame (i18n) | copy |
| src/chrome_view.rs | 172 | del-frame | Delete frame (i18n) | trash-can |
| src/rune_sec/audio_panel.rs | 821 | audio-norm-last | Normalise | volume-high |
| src/rune_sec/audio_panel.rs | 744 | audio-norm-{i} | Norm | volume-high |
| src/rune_sec/audio_panel.rs | 822 | audio-echo | Echo | repeat |
| src/rune_sec/audio_panel.rs | 824 | audio-trimsil | Trim silence | scissors |
| src/rune_sec/audio_panel.rs | 832 | audio-reverse | Reverse | left-right |
| src/rune_sec/audio_panel.rs | 853 | audio-move-earlier | Earlier | chevron-left |
| src/rune_sec/audio_panel.rs | 856 | audio-move-later | Later | chevron-right |
| src/rune_sec/audio_panel.rs | 859 | mixdown | Mixdown | layer-group |
| src/rune_sec/camera_panel.rs | 830 | cam-key-clear-{frame} | Clear | eraser |
| src/rune_sec/camera_panel.rs | 869 | cam-follow | Follow | crosshairs |
| src/rune_sec/camera_panel.rs | 925, 992 | view-fit | Fit | expand |
| src/rune_sec/camera_panel.rs | 949 | cam-rot-left | -15° | arrow-rotate-left |
| src/rune_sec/camera_panel.rs | 953 | cam-rot-right | +15° | arrow-rotate-right |
| src/rune_sec/camera_panel.rs | 962 | cam-set-key | Set key | key |
| src/rune_sec/camera_panel.rs | 963 | cam-clear-key | Clear key | eraser |
| src/rune_sec/camera_panel.rs | 966 | cam-hold | Hold camera | lock |
| src/rune_sec/clips_panel.rs | 825 | clip-loop-loop | Loop | repeat |
| src/rune_sec/clips_panel.rs | 836 | clip-loop-pingpong | Ping-pong | left-right |
| src/rune_sec/clips_panel.rs | 847 | clip-loop-once | Once | play |
| src/rune_sec/clips_panel.rs | 892 | clip-edit-prev | Prev frame | backward-step |
| src/rune_sec/clips_panel.rs | 895 | clip-edit-next | Next frame | forward-step |
| src/rune_sec/clips_panel.rs | 903 | clip-edit-add-frame | Add frame | plus |
| src/rune_sec/clips_panel.rs | 910 | clip-edit-delete-frame | Delete frame | trash-can |
| src/rune_sec/clips_panel.rs | 918 | clip-edit-exit | Exit | xmark |
| src/rune_sec/clips_panel.rs | 927 | clip-edit-enter | Edit clip | pen |
| src/rune_sec/clips_panel.rs | 985 | make-clip | Make clip | clapperboard |
| src/rune_sec/clips_panel.rs | 988 | insert-clip | Insert clip | plus |
| src/rune_sec/draw_tools.rs | 644 | mirror-strokes | Mirror strokes | arrow-right-arrow-left |
| src/rune_sec/export_panel.rs | 985 | xc-range-all | All frames | film |
| src/rune_sec/export_panel.rs | 988 | xc-range-loop | Loop range | repeat |
| src/rune_sec/export_panel.rs | 1088–1148 | xc-v2, xc-v3, xc-v4, xc-v5, xc-stk, xc-nodes, xc-stknds | .piv v2 … .stknds (icon arg is `None` today) | file (all seven) |
| src/rune_sec/figure_add.rs | 177 | figure-add-toggle | + Add figure | plus |
| src/rune_sec/frame_ops.rs | 579 | inbetween | Inbetween | plus |
| src/rune_sec/frame_ops.rs | 582 | shift-back | < Shift | chevron-left |
| src/rune_sec/frame_ops.rs | 588 | shift-fwd | Shift > | chevron-right |
| src/rune_sec/frame_ops.rs | 594 | reverse-range | Reverse | arrow-right-arrow-left |
| src/rune_sec/frame_ops.rs | 653 | smooth-path | Smooth path | bezier-curve |
| src/rune_sec/frame_ops.rs | 723 | loop-clear | Loop clear | eraser |
| src/rune_sec/frame_ops.rs | 769 | fop-dur-reset | Reset 1.0 | arrow-rotate-left |
| src/rune_sec/fx_browser.rs | 611 | fx-preset-save | Save last clip as preset | floppy-disk |
| src/rune_sec/fx_clips.rs | 1383 | fx-earlier-{i} | Earlier | chevron-left |
| src/rune_sec/fx_clips.rs | 1386 | fx-later-{i} | Later | chevron-right |
| src/rune_sec/fx_clips.rs | 1484 | fx-color2-none-{i} | Solid | circle |
| src/rune_sec/fx_clips.rs | 1510 | fx-anchor-{i} | Anchor to joint | link |
| src/rune_sec/graph_panel.rs | 1582 | graph-chan-duplicate | Duplicate channel to next figure | copy |
| src/rune_sec/graph_panel.rs | 1589 | graph-chan-delete | Delete channel | trash-can |
| src/rune_sec/graph_panel.rs | 1675 | graph-key | Key angle | key |
| src/rune_sec/graph_panel.rs | 1676 | graph-smooth | Auto tangents | wand-magic-sparkles |
| src/rune_sec/graph_panel.rs | 1679 | graph-mirror | Mirror time | arrow-right-arrow-left |
| src/rune_sec/graph_panel.rs | 1718 | graph-bake | Bake | stamp |
| src/rune_sec/graph_panel.rs | 1722 | graph-clear | Clear | eraser |
| src/rune_sec/home_panel.rs | 1031 | home-proj-del-confirm | Delete file | trash-can |
| src/rune_sec/home_panel.rs | 1040 | home-proj-del-cancel | Keep | check |
| src/rune_sec/home_panel.rs | 1086 | home-proj-dup-{i} (dup_key) | Duplicate | copy |
| src/rune_sec/home_panel.rs | 1089 | home-proj-del-{i} (del_key) | Delete | trash-can |
| src/rune_sec/home_panel.rs | 1102 | home | Home | house |
| src/rune_sec/inspector_figure.rs | 797 | fig-centre | Centre figure | crosshairs |
| src/rune_sec/inspector_figure.rs | 912 | fig-flip | Flip horizontal | left-right |
| src/rune_sec/inspector_figure.rs | 915 | fig-duplicate | Duplicate | copy |
| src/rune_sec/inspector_figure.rs | 919 | fig-del (via text `danger_btn`) | Delete figure | trash-can |
| src/rune_sec/inspector_segment.rs | 744 | seg-nav-parent | Parent joint | chevron-up |
| src/rune_sec/inspector_segment.rs | 747 | seg-nav-child | Child joint | chevron-down |
| src/rune_sec/inspector_segment.rs | 836 | seg-bend-zero | Straighten — see Tier B (ruler); Tier A fallback: minus | minus |
| src/rune_sec/inspector_segment.rs | 850 | node-shape | Cycle shape | shapes |
| src/rune_sec/inspector_segment.rs | 863 | node-color | Cycle | palette |
| src/rune_sec/inspector_segment.rs | 872 | add-seg | + Segment | plus |
| src/rune_sec/keyframe_tools.rs | 1138 | key-angle | Key angle here | key |
| src/rune_sec/keyframe_tools.rs | 1141 | key-delete | Delete key here | trash-can |
| src/rune_sec/keyframe_tools.rs | 1144 | key-auto | Auto tangents | wand-magic-sparkles |
| src/rune_sec/keyframe_tools.rs | 1152 | key-copy (via `kbtn`) | Copy keys | copy |
| src/rune_sec/keyframe_tools.rs | 1159 | key-paste (via `kbtn`) | Paste keys | clipboard |
| src/rune_sec/layers_panel.rs | 593 | layer-show-all | Show all | eye |
| src/rune_sec/library_panel.rs | 885 | library-collapse | Show / Hide (dynamic) | chevron-up / chevron-down by state, like play/pause |
| src/rune_sec/library_panel.rs | 900 | library-save | Save current figure to library | floppy-disk |
| src/rune_sec/onion_controls.rs | 123 | onion-toggle | Onion | eye |
| src/rune_sec/playback_controls.rs | 514 | pb-prev-key | Prev key | backward-step |
| src/rune_sec/playback_controls.rs | 520 | pb-next-key | Next key | forward-step |
| src/rune_sec/playback_controls.rs | 613 | tween | Tween play | play |
| src/rune_sec/playback_controls.rs | 655 | loop-clear | Loop clear | eraser |
| src/rune_sec/pose_tools.rs | 1237 | pose-mirror (via `pbtn`) | Mirror pose | arrow-right-arrow-left |
| src/rune_sec/pose_tools.rs | 1244 | pose-mirror-all (via `pbtn`) | Mirror figure (all frames) | arrow-right-arrow-left |
| src/rune_sec/pose_tools.rs | 1258 | pose-copy (via `pbtn`) | Copy pose | copy |
| src/rune_sec/pose_tools.rs | 1265 | pose-paste (via `pbtn`) | Paste pose | clipboard |
| src/rune_sec/pose_tools.rs | 1272 | pose-apply-all (via `pbtn`) | Apply pose to all frames | check |
| src/rune_sec/pose_tools.rs | 1283 | pose-reset (via `pbtn`) | Reset pose | arrow-rotate-left |
| src/rune_sec/pose_tools.rs | 1290 | pose-bake-graph (via `pbtn`) | Bake graph | chart-line |
| src/rune_sec/project_tools.rs | 840 | proj-open-{i} (open_row_key) | Open | folder-open |
| src/rune_sec/scene_panel.rs | 969 | weather | Cycle weather | dynamic: reuse the panel's own `kind_icon(kind)` (sun / cloud-rain / snowflake / star / cloud / bolt — all curated), exactly as the weather picker at scene_panel.rs:868 already does |
| src/rune_sec/stroke_inspector.rs | 1664 | strokes-clear | Clear frame | eraser |
| src/rune_sec/stroke_inspector.rs | 1700 | stroke-del-{idx} | Delete | trash-can |
| src/rune_sec/text_tool.rs | 1077 | text-del-{i} | Delete | trash-can |
| src/rune_sec/text_tool.rs | 1080 | text-dup-{i} | Duplicate | copy |
| src/rune_sec/text_tool.rs | 1127 | text-update | Update text | check |
| src/rune_sec/text_tool.rs | 1152–1161 | text-nudge-l/u/d/r | ← ↑ ↓ → | chevron-left / chevron-up / chevron-down / chevron-right (as `icon_only_btn`) |
| src/rune_sec/text_tool.rs | 1171 | text-rot-l | Rotate −15° | arrow-rotate-left |
| src/rune_sec/text_tool.rs | 1174 | text-rot-r | Rotate +15° | arrow-rotate-right |
| src/rune_sec/text_tool.rs | 1248 | text-seg | Add text | font |
| src/rune_sec/particles_panel.rs | 725–734 | particles-nudge-l/u/d/r | ← ↑ ↓ → | chevron-left / chevron-up / chevron-down / chevron-right (as `icon_only_btn`) |
| src/rune_sec/timeline_marks.rs | 933 | loop-set | Set loop | repeat |
| src/rune_sec/timeline_marks.rs | 950 | loop-clear | Clear | eraser |
| src/rune_sec/three_d_panel.rs | 828 | mode-3d (via `ctl`) | Enter/Exit 3D (toggle_label) | cube |
| src/rune_sec/three_d_panel.rs | 887 | joint-3d (via `ctl`) | Joint> | chevron-right |
| src/rune_sec/three_d_panel.rs | 899 | orbit-3d (via `ctl`) | Orbit +15 | arrow-rotate-right |
| src/rune_sec/three_d_panel.rs | 968 | cam-frame-3d (via `ctl`) | Frame camera | expand — the same panel's `d3-frame` already does this action with FRAME_ICON `expand` via `ictl`; this is a duplicate text twin |
| src/rune_sec/three_d_panel.rs | 1020 | bake-anim-3d (via `ctl`) | Bake to frames | film |
| src/rune_sec/three_d_panel.rs | 1023 | bake-3d (via `ctl`) | Bake to 2D | stamp |

Notes on Tier A choices, so the coding wave does
not relitigate them:

- **One glyph per verb, shell-wide:** copy =
  Duplicate, trash-can = Delete, eraser = Clear,
  floppy-disk = Save, repeat = Loop, key = Set key,
  check = Apply/Keep/Update (the pose3d library's
  Apply already uses `check`). Deviations need a
  reason in the adopter's report.
- `fig-duplicate` exists twice: as this text `btn`
  (inspector_figure.rs:915) and as an `icon_btn`
  with `copy` in figures_panel (Phase M §21).
  Converting the inspector one makes the two
  identical — and surfaces the duplicate key for
  whoever owns the gate that phase.
- fx_browser's kind buttons (fx_browser.rs:536,
  566, 599) are content-named (the kind IS the
  label) and stay text, but they can *lead* with
  the existing `category_icon(entry.category)`
  glyph (fire / bolt / wand-magic-sparkles / leaf
  / shapes — all curated) at zero new-icon cost.
  Optional polish, adopter's call.

### Tier B — obvious icon, glyph must be added first (§3)

| File | Line | Key | Label | Proposed icon (add) |
|---|---|---|---|---|
| src/rune_sec/audio_panel.rs | 823 | audio-lowpass | Lowpass | filter |
| src/rune_sec/audio_panel.rs | 825 | audio-split-sil | Split silences | wave-square |
| src/rune_sec/audio_panel.rs | 826 | audio-pitch | Pitch +2 | arrow-trend-up |
| src/rune_sec/audio_panel.rs | 827 | audio-comp | Compress | compress |
| src/rune_sec/audio_panel.rs | 759 | audio-xfade-{i} | Xfade | shuffle |
| src/rune_sec/camera_panel.rs | 928, 995 | view-centre | Centre | bullseye |
| src/rune_sec/camera_panel.rs | 937–1003 | cam-zoom-out / cam-zoom-in | - / + | magnifying-glass-minus / magnifying-glass-plus |
| src/rune_sec/draw_tools.rs | 688 | stamp-arrow | Arrow | arrow-right |
| src/rune_sec/graph_panel.rs | 1709 | graph-reduce | Reduce keys | filter |
| src/rune_sec/home_panel.rs | 971 | recover | Recover | clock-rotate-left |
| src/rune_sec/home_panel.rs | 1197 | home-sort | Sort: A–Z / Sort: added | arrow-down-a-z |
| src/rune_sec/inspector_figure.rs | 800 | fig-ground | Ground figure | arrow-down |
| src/rune_sec/inspector_segment.rs | 836 | seg-bend-zero | Straighten | ruler |
| src/rune_sec/layers_panel.rs | 509 | layer-bottom-{fi} | Bottom | angles-down |
| src/rune_sec/layers_panel.rs | 530 | layer-top-{fi} | Top | angles-up |
| src/rune_sec/layers_panel.rs | 600 | layer-solo-selected | Solo selected | bullseye |
| src/rune_sec/library_panel.rs | 923 | lib-sort | Sort: A–Z / Sort: added | arrow-down-a-z |
| src/rune_sec/project_tools.rs | 806 | proj-reload | Reload from disk | rotate-right (not arrow-rotate-right — that is Redo's glyph in the app bar) |
| src/rune_sec/text_tool.rs | 1137 | text-front | Bring to front | angles-up |
| src/rune_sec/text_tool.rs | 1140 | text-back | Send to back | angles-down |
| src/rune_sec/three_d_panel.rs | 1017 | mark-b-3d (via `ctl`) | Mark pose B | bookmark |

### Tier C — the stepper / nudge class (charter §6 kit stepper)

These are not "buttons missing an icon" so much as
hand-rolled steppers the kit `stepper_value` +
`icon_only_btn` pair replaces wholesale. Proposed
glyphs throughout: **minus / plus** (both curated)
for −/+ steppers, **chevron-down / chevron-up**
where the pair is a value spinner. Grouped by file
with key patterns; every site is a `btn`/`fbtn`
call in the census:

| File | Keys (patterns) | Labels |
|---|---|---|
| src/rune_sec/audio_panel.rs | audio-gain-dn, audio-gain-up (842, 850) | Gain - / Gain + |
| src/rune_sec/draw_tools.rs | draw-sides-dn/up, draw-size-dn/up, draw-rot-dn/up (702–720) | - / + |
| src/rune_sec/frame_ops.rs | dur-dn, dur-up (731, 734); fop-ins-n-dn, fop-ins-n-up (794, 809) | Dur - / Dur + ; - / + |
| src/rune_sec/fx_clips.rs | fx-nudge-l10, fx-nudge-r10 (1225, 1234); fx-dur-dn, fx-dur-up (1242, 1245); fx-start-dec/inc-{i}, fx-dur-dec/inc-{i} (1342–1373) | -10 / +10 ; -0.1s / +0.1s ; - / + |
| src/rune_sec/inspector_figure.rs | insp-fop-dn/up (678, 681); fig-ring-dn/up (778, 781) | − / + |
| src/rune_sec/inspector_segment.rs | seg steppers via dn_key/up_key (624, 625); seg-rot-m5/m1/p1/p5 (792–801); seg-bend-m5/m1/p1/p5 (824–833) | − / + ; −5°…+5° (degree labels are content — keep the text, the kit stepper carries them as its value readout) |
| src/rune_sec/inspector_style.rs | style-{lower}-dn/up (589, 592); style-{stem}-dn/up (633, 636); style-opacity-dn/up (938, 941) | − / + |
| src/rune_sec/layers_panel.rs | layer-op-dec-{fi}, layer-op-inc-{fi} (490, 502) | - / + |
| src/rune_sec/onion_controls.rs | onion-prev-dn/up, onion-next-dn/up (130–154) | - / + |
| src/rune_sec/particles_panel.rs | particles-frame-dn/up (649, 657) | - / + |
| src/rune_sec/playback_controls.rs | fps-dn, fps-up (582, 585); dur-dn, dur-up (677, 680) | FPS − / FPS + ; Dur − / Dur + |
| src/rune_sec/scene_panel.rs | stage steppers via dn_key/up_key (693, 695; 774, 777); scene-bg-{lower}-dn/up (717, 720); scene-fps-dn/up (742, 745) | − / + |
| src/rune_sec/settings_popover.rs | stage axis steppers via down_key/up_key (601, 608) | - / + |
| src/rune_sec/stroke_inspector.rs | width steppers via dec_key/inc_key (1314, 1317) | - / + |
| src/rune_sec/text_tool.rs | text-isize-dn/up (1185, 1193); text-size-down/up (1249, 1257) | Size − / Size + |
| src/rune_sec/timeline_marks.rs | loop-a-dn/up, loop-b-dn/up (938–947) | Start - / Start + ; End - / End + |
| src/rune_sec/three_d_panel.rs | d3-zoom-dn, d3-zoom-up via `ctl` (870, 873) | − / + |

### Keep text — no icon proposed (and why)

The label *is* the content; a glyph adds noise:

- **Named choices:** easing chips (easing_panel
  ease-frame-*/ease-project-*/ease-graph-*),
  graph interp chips (graph-interp-hermite/linear/
  step), tangent preset chips (key-preset-{suffix},
  graph preset_chip), nib chips (stroke_inspector
  nib_key), speed chips (settings-speed-{i},
  playback preset_key), stage-size chips
  (scene-stage-{w}x{h}), language buttons
  (settings language_button), figure-add entries
  (figure_add entry_button), pose-copy-to-{j} rows,
  3D camera-view buttons (cam-front/side/
  three-quarter/top-3d), IK chain buttons
  (ik-arm/leg-l/r-3d), rotx/roty/rotz-3d (axis +
  angle is the content).
- **Ratios and readouts as buttons:** view-actual
  "1:1", fop-dur-half "×0.5", fop-dur-double "×2",
  graph-scale-up/down "Scale ×2 / ÷2".
- **List rows** (Phase N styles them as `list_row`,
  not icon buttons): cam-key-{frame},
  layer-name-{fi}, text-load-{i}, joint-sel-3d-{i},
  graph channel selector chips, fx kind/recent/
  preset rows, workspace chips — *except* the
  workspace chips get per-workspace glyphs as a
  header-level decision in §3, not per-row here.
- **Loop endpoints:** loop-start / loop-end
  (frame_ops 713, 718; playback_controls
  pb-loop-start/end 558, 563 and loop-start/end
  633, 644). No distinct FA glyph reads as "loop
  endpoint"; the loop as a whole already gets
  `repeat` via loop-set / xc-range-loop. Keep.

---

## 2. Icon names that silently degrade — the audit

**Verdict: zero real cases in production code.**
Every literal icon argument and every dynamic icon
source resolves to a name in `ICON_NAMES`. The
detail matters, because the near-misses show where
the next one will come from.

### 2a. Literal call sites

182 literal icon arguments parsed across all
icon-bearing helpers. **One** fell outside
`ICON_NAMES`, and it is a parser artefact that
points at a real hazard:

- `src/rune_sec/inspector_figure.rs:919` —
  `danger_btn("fig-del", "Delete figure", …)`. The
  string in the icon slot is a *label*, because
  inspector_figure's local `danger_btn` (line 807)
  has signature `(key, label, f)` — **no icon
  parameter**. Its sibling in
  `src/rune_sec/inspector_segment.rs:635` is also
  named `danger_btn` but has signature
  `(key, icon, label, f)` and renders the glyph
  (guarded by `has_icon`). Same helper name, two
  arities, two files: exactly the setup for a
  future caller pasting the segment form into the
  figure panel and shipping a button whose "icon"
  is silently treated as a label (or vice versa).
  **Recommendation:** Phase N unifies the two onto
  the segment form (icon + label, error fill) and
  converts fig-del per §1 Tier A.

### 2b. Dynamic icon sources — all resolved, all clean

39 call sites take a non-literal icon. Each source
was traced to its full value set:

| Source (file:line) | Possible values | In set? |
|---|---|---|
| chrome_view.rs:135 play/pause conditional | play, pause | ✓ |
| audio_panel `mute_toggle_parts` → audio-mute (833) and menus' `mute_item_parts` | volume-xmark, volume-high | ✓ |
| draw_tools `brush_icon(name)` (164) | pen, pencil, paintbrush (catch-all) | ✓ |
| scene_panel `kind_icon(kind)` (167) | sun, cloud-rain, snowflake, star, cloud, bolt | ✓ |
| fx_browser `category_icon(category)` (257) | fire, bolt, wand-magic-sparkles, leaf, shapes | ✓ — and the code carries the scar of a caught case: the comment at fx_browser.rs:257 records that **"bomb" is NOT in the set**, so Explosion uses fire. The only documented near-miss in the tree. |
| menus `menu_icon(key)` map (286) | clock, folder-open, floppy-disk, download, arrow-rotate-left, arrow-rotate-right, copy, clipboard, trash-can, eye, crosshairs, gauge, list, magnifying-glass, circle-question | ✓ (and `item()` re-checks `has_icon` before drawing — belt and braces) |
| toasts `Severity::icon()` (132) | circle-info, check, triangle-exclamation, xmark | ✓ |
| pose_tools `APPLY_MIRRORED_ICON` (1179), three_d_panel `MIRROR_ICON` (142), `FRAME_ICON` (151) | arrow-right-arrow-left, expand | ✓ (each pinned by a `has_icon` unit test) |
| figures_panel row conditionals (549, 577) | lock / lock-open, eye / eye-slash | ✓ |
| export_panel rows: `range_row` / `bundle_row` icon args; `section_with_icon` (868) | image, film, video, table-cells-large, clock | ✓; `plain_row` format rows pass `None` deliberately (they are §1 Tier A candidates, not degrades) |
| help_panel `header_icon` (456) | key, list | ✓ |
| status_strip `segment_with_icon` caller (957) | repeat | ✓ |
| inspector_segment `danger_btn` (873) | trash-can | ✓ |

The remaining non-literal sites are plumbing
inside `icons.rs` / `icon_btn.rs` themselves
(parameter forwarding), not value sources.

### 2c. Test-only intentional unknowns (not defects)

`icons.rs:195` asserts `!has_icon("no-such-icon")`;
`icon_btn.rs:316–317` asserts
`!use_icon("definitely-not-an-icon")` and
`!use_icon("")`. These are the degrade path's own
negative tests — leave them alone.

### 2d. The standing risk, stated plainly

The degrade is *silent by design* in three of the
four builders. Phase H–M avoided a bad name only
by author discipline plus two local pins (menus'
`menu_icons_name_real_glyphs…` test, the 3D
constants' `has_icon` tests). Nothing shell-wide
stops the next `"bomb"`. §4 turns this audit's
method into a gate.

### 2e. Curated but unused — the free headroom

53 distinct names are used as literal icon
arguments (the 54th distinct string in that slot
is §2a's "Delete figure" artefact); the dynamic
sources of §2b add 17 more, so **70 of the 111
curated glyphs are in use and 41 have no
production icon use at all**. Of those 41, 33
have zero references anywhere in production code
(verified by tree-wide grep) and 8 more
(`arrows-up-down-left-right`, `forward-step`,
`hand`, `minus`, `shield`, `stop`,
`up-down-left-right`, `wind`) appear only as
non-icon strings — a fig3d node name, a test
comment, weather field names — or as text labels:

arrow-pointer, arrows-up-down-left-right, bars,
bug, city, clapperboard, crown, droplet,
feather, font, forward-step, gem, ghost, hand,
heart, house, i-cursor, layer-group, left-right,
lightbulb, link, magnet, minimize, minus,
mountain, photo-film, right-left, rocket,
seedling, shield, sliders, stamp, stop, tree,
umbrella, up-down, up-down-left-right, upload,
water, wind, wrench.

Phase N should spend these before adding new
names — §1 already assigns clapperboard, font,
forward-step, house, layer-group, left-right,
link, minus (Tier C), sliders (§3c), stamp and
upload's sibling download.
They are also the natural pool for panel-header
glyphs (§3b).

---

## 3. Icons the charter design needs that `icon_data` lacks

Every name below was verified **present** in
`assets/icons/fontawesome/solid/<name>.svg` and
**absent** from `ICON_NAMES`. Adding a name means
adding its `(view_box_width, path)` entry to
`icon_data()` + the `ICON_NAMES` list, following
the file's existing generated format.

### 3a. Required additions (each feeds a §1 Tier B row or a charter item)

| Icon | Charter / §1 need |
|---|---|
| grip | Drag handles: timeline loop edges, slider thumbs, list-row reorder grips (§5/§8 surfaces). grip-vertical is curated; the horizontal-dot form is the one panels reach for. |
| grip-lines | Alt drag handle (two-line form) for narrow rows. |
| ellipsis-vertical | Overflow / "more actions" menus on list rows (figures, layers, library, clips rows all grow trailing controls in the card design). |
| keyboard | Help ▸ Shortcuts section header (today it borrows `key`, a skeleton-key glyph that reads as "password"); command-palette footer hint glyph (§9). |
| swatchbook | Colour-preset section glyph: inspector_style presets, stroke_inspector colour row, fx colour rows (§2 headers). |
| eye-dropper | Colour *picking* actions (sample-from-stage / pick colour) in style + stroke panels; pairs with swatchbook. |
| fill-drip | Fill-colour actions: stroke fill, fx color2 gradient end, figure fill — today indistinguishable from line colour. |
| person | Panel header: Figures panel and Inspector (figure) — the figure glyph the set somehow lacks (only `user`-family alternatives exist, see 3c). |
| bone | Panel header: Inspector (segment) — a segment *is* a bone in every stick-figure editor's vocabulary. |
| book-open | Panel header: Library (a library of figures/poses reads as an open book; folder-open is already spent on Load). |
| diamond | Keyframe glyph: keyframe_tools header and any key marker that wants the industry-standard diamond instead of `key`. |
| wave-square | Waveform panel header (§3b) and audio-split actions (§1). The literal names `waveform` / `audio-waveform` do **not** exist in FA Free solid — wave-square is the waveform-shaped glyph that does. `file-waveform` also exists and suits audio *file* rows. |
| file-audio | Audio panel header alt / audio clip list rows (audio rows today are bare text). |
| volume-low | Completes the volume set: mute parts use volume-xmark (muted) and volume-high (audible); a mid-gain state has no honest glyph. (volume-off also exists if a fourth state is ever needed.) |
| arrow-down-a-z | Sort buttons: home-sort and lib-sort (§1) — their label is literally "Sort: A–Z". Caution: `arrow-down-wide-narrow` does **not** exist in this set; do not propose it. |
| rotate-right | Reload/refresh actions that must not reuse Redo's arrow-rotate-right (proj-reload, §1). `arrows-rotate` also exists as the two-arrow form. |
| angles-up / angles-down | "Move to the very top/bottom" pair: layers Top/Bottom, text Bring to front / Send to back (§1). FA Free has no bring-forward/send-back — these are the honest substitutes. |
| arrow-down | fig-ground (§1): ground = straight down. (arrow-up, arrow-left, arrow-right also exist and arrow-right feeds §1's stamp-arrow; the four cardinal arrows are conspicuously absent from a set that has four chevrons.) |
| ruler | Straighten (seg-bend-zero, §1) and any measure action; also the natural header glyph if a measure surface appears. |
| filter | graph-reduce and audio-lowpass (§1): both *are* filters. |
| compress | audio-comp (§1) — the glyph is the verb. |
| shuffle | audio-xfade (§1): crossfade = two streams interleaved. |
| chart-simple | Stats panel header alt; per-row Norm alt (§1 chose volume-high; chart-simple is here if the adopter prefers a levels reading). |
| arrow-trend-up | audio-pitch (§1): pitch rising. |
| bullseye | view-centre and layer-solo (§1): centre-of-target and isolate-one both read as a bullseye; crosshairs is already spent on Follow. |
| bookmark | mark-b-3d (§1): "Mark pose B" is a bookmark in everything but name. |
| table-columns | Panel/section glyph for the two side columns themselves (View ▸ toggle-left/right menu items today borrow `list`); also the layout/settings "columns" concept. |
| rectangle-list | List-section glyph for panel headers built around rows (layers, clips) if layer-group/film read wrong in situ. |
| window-maximize | Stage pop-out / focus-stage affordance in the §14 stage pill family (expand covers "frame this"; window-maximize is the window-level verb). |
| timeline | The §13 timeline card's header glyph — the name is the thing; a bars-with-markers glyph that exists in solid. |
| person-walking | Pose tools header / empty state: a figure *in a pose*, distinct from the static `person`. |
| universal-access | Inspector (figure) header alt and pose empty states — the classic articulated-figure pictogram. |
| pen-nib | Draw tools header alt (paintbrush is spent on brush presets; pen-nib separates "tools" from "preset"). |
| brush | Stroke inspector header alt: the set curates `paintbrush` but not plain `brush`, which is the narrower stroke-shaped glyph. |
| circle-half-stroke | Settings ▸ dark/light theme toggle glyph (sun/moon are spent on weather/scene meanings). |
| language | Settings ▸ Language section header (the 28 language buttons themselves stay text — §1 keep-list). |
| file-lines | Project tools header / project-file rows (file alone is spent as the generic format-row glyph in §1). |
| pen-to-square | Rename actions (figure rename, clip rename, library rename) — today renames hide behind row presses with no glyph. |
| right-from-bracket | Exit/quit-style actions if the File menu grows one; also "exit clip edit" alt (clip-edit-exit took xmark in §1 — this is the door-shaped alternative). |
| folder-plus / file-circle-plus | New-project actions (File menu / Home) if Phase N gives them menu presence; plus alone undersells "create a file". |
| share-from-square / arrow-up-from-bracket | Export *share/send* affordance if §14/Export wants one distinct from download (which means "save a copy" in the menus map). Both exist; pick one. |
| chart-column | Stats header alt to chart-simple/gauge. |

### 3b. Panel-header glyph map (charter §2 — one header per panel)

Have = curated today (many from the §2e unused
pool). Add = §3a.

| Panel (file) | Glyph | Status |
|---|---|---|
| Figures (figures_panel.rs) | person | add |
| Add figure (figure_add.rs) | user-plus — verified in solid | add |
| Inspector: segment (inspector_segment.rs) | bone | add |
| Inspector: figure (inspector_figure.rs) | universal-access | add |
| Inspector: style (inspector_style.rs) | palette | have |
| Camera (camera_panel.rs) | camera (video as alt) | have |
| Scene (scene_panel.rs) | image (mountain as alt) | have |
| Frames (frame_ops.rs) | film | have |
| Graph (graph_panel.rs) | chart-line | have |
| Easing (easing_panel.rs) | bezier-curve | have |
| Draw tools (draw_tools.rs) | paintbrush (pen-nib as alt) | have |
| Stroke inspector (stroke_inspector.rs) | pen (brush as alt) | have |
| FX browser (fx_browser.rs) | wand-magic-sparkles | have |
| FX clips (fx_clips.rs) | shapes | have |
| Audio (audio_panel.rs) | music (file-audio as alt) | have |
| Waveform (waveform_view.rs) | wave-square | add |
| Movieclips (clips_panel.rs) | clapperboard | have (unused pool) |
| Library (library_panel.rs) | book-open | add |
| 3D (three_d_panel.rs) | cube | have |
| Export (export_panel.rs) | download | have |
| Home (home_panel.rs) | house | have (unused pool) |
| Pose tools (pose_tools.rs) | person-walking | add |
| Keyframe tools (keyframe_tools.rs) | diamond (key as alt) | add |
| Layers (layers_panel.rs) | layer-group | have (unused pool) |
| Text (text_tool.rs) | font | have (unused pool) |
| Project tools (project_tools.rs) | folder-open (file-lines as alt) | have |
| Stats (stats_view.rs) | chart-simple (gauge as alt) | add |
| Particles (particles_panel.rs) | star (shapes as alt) | have |
| Onion (onion_controls.rs) | eye | have |
| Playback (playback_controls.rs) | play (gauge for the speed subgroup) | have |
| Settings (settings_popover.rs) | gear (sliders for the adjustments subgroup — both from the pool/curated) | have |
| Help (help_panel.rs) | circle-question; Shortcuts subsection: keyboard | have / add |
| Command palette (command_palette.rs) | magnifying-glass; footer: keyboard | have / add |

### 3c. Workspace chips (charter §15 row) — no additions needed

All eight resolve from the curated set, six of
them from the unused pool: Animate = play (or
film), Draw = pencil, Graph = chart-line,
Effects = wand-magic-sparkles, Audio = music,
3D = cube, Export = download, Home = house.
`workspaces.rs` today renders chips as bare `btn`
text (workspaces.rs:451) — the cheapest
high-visibility icon win in the phase.

### 3d. Names that do NOT exist — do not propose them

Checked against solid/ and absent (several are
FA Pro or later-version names — plausible-sounding
traps for the coding wave):

grip-dots, grip-dots-vertical, panel-left,
panel-top, border-top/left/right, waveform,
audio-waveform, audio-lines, waveform-lines,
list-checks, repeat-1, arrow-down-wide-narrow,
arrow-up-wide-narrow, bring-forward, bring-front,
send-back, send-backward, play-pause, command,
option, layer-plus, layer-minus, sparkles (Pro),
sliders-vertical, double-check, chevron-grab,
move, spline, vector-square, accessibility
(universal-access is the existing form),
file-plus, file-music, droplets (droplet exists),
disc, timer, list-tree, printer, zoom,
key-skeleton, arrows-expand, case-sensitive,
figure.

---

## 4. Work list for the coding wave (file-scoped)

1. **icon_data.rs** — add the §3a names
   (path data copied from the solid SVGs,
   following the file's generated format);
   extend `ICON_NAMES`; the file's own invariant
   (names == match arms) is test-pinned already.
2. **Tier A conversions** (§1) — one panel file
   per adopter, matching the charter's one-file
   rule; `fbtn` (frame_ops), `pbtn` (pose_tools),
   `kbtn` (keyframe_tools) and `ctl`
   (three_d_panel) retire onto `icon_btn` /
   `icon_btn_inert` as converted.
3. **danger_btn unification** (§2a) —
   inspector_figure adopts inspector_segment's
   (key, icon, label, f) form; fig-del gains
   trash-can.
4. **Tier B conversions** (§1) — after step 1.
5. **Tier C steppers** (§1) — with the kit
   stepper adoption, not as icon_btn one-offs.
6. **Headers** (§3b/§3c) — with each panel's
   card adoption; workspace chips in the chrome
   foundation wave.
7. **Census gate** — extend
   `scripts/phase_j_checks.py` (or the icon
   sections' tests) with this audit's check:
   every literal icon argument in production
   code ∈ `ICON_NAMES`, and every dynamic source
   in §2b re-verified when touched. Menus'
   `menu_icons_name_real_glyphs…` test is the
   local precedent; the gate makes it shell-wide
   so §2's verdict stays true by construction.

*Scope honesty:* this audit covers the shell
(`src/rune_sec/`, `src/chrome_view.rs`,
`src/stage_view.rs`, `src/main.rs`). Engine-side
files (mcp_tools, export, fig3d …) were scanned
only to distinguish icon strings from data
strings — their literals (weather kinds, MCP
parameter names, FX kind names like
burst/meteor/spiral in fx_edit/mcp_tools) are
data, not icons, and were excluded on that basis.
