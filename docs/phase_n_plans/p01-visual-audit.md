# Phase N — P01 Visual Audit

## Evidence and scope

Screenshots reviewed:

- `stykanimate-phase-m-headless-snap.png`
- `stykanimate-phase-m-3d-stage-snap.png`
- `stykanimate-phase-l-headless-snap.png` — the clearest true Animate-workspace view
- `stykanimate-phase-l-3d-preview.png`

The shell regions were cross-checked against `src/main.rs` `view()`:
the root column contains the app bar, menu bar, workspace row and
three-column main row; the left column contains Home/Project, Figures,
figure-add, Library and Layers; the right column starts with Settings and
then workspace-gated inspectors and tools; the centre contains the stage,
camera overlay row, transport, timeline and status strip.

One evidence note: despite its filename, the Phase M “default” snap
visually has the **3D** workspace selected — the 3D chip is blue and its
blurb is showing — so this audit uses the Phase L default snap for
Animate-specific observations and treats the Phase M default snap as
another Build3D/general-shell view.

Menus, the command palette and most popovers are not open in the supplied
screenshots, so this audit makes no visual claim about their interiors.
The judgements below concern visible layout, hierarchy, state expression,
spacing, containment and consistency.

What already works and should not be lost: the stage is large and its 3D
grid stays contained; the accent blue is used consistently for the
current workspace and current frame; the selected figure row’s navy tint
points in the direction of the charter’s selected-fill idea; the status
strip contains genuinely useful information.

## Ranked visual defects — worst first

### 1. Panels do not read as panels at all

- **What looks wrong:** Almost every left- and right-column section is
  black-on-black: a small heading followed by loose controls directly on
  the window background. Project, Figures, Library, Camera, Pose tools
  and Keyframes run together as one continuous control wall. The few
  exceptions — the Library block, Figure inspector and 3D block — use
  different fills and shapes, which makes the system look accidental
  rather than merely flat.
- **Where:** Both side columns, all workspaces; `side-left` and
  `side-right` in `src/main.rs`.
- **Charter rule:** #1 “Every panel is a card”; #2 “One header.”
- **Fix:** Put every panel root on `design::panel_col` — `t.card`,
  1px `t.card_border`, radius 12, padding 12, gap 8 — with no local
  background inventions.

### 2. The right column is an undifferentiated scroll wall

- **What looks wrong:** Settings is a filled strip, but Camera then
  continues underneath with no boundary; in Animate, inspector, Segment
  style, Pose tools and Keyframes follow in the same uninterrupted
  stream. The eye cannot tell where one tool ends and the next begins,
  and the column is cut off mid-panel at the bottom.
- **Where:** Right column, especially Settings → Camera in Build3D and
  Figure inspector → Segment style → Pose tools → Keyframes in Animate.
- **Charter rule:** #1 panel cards; #2 one header with a hairline under
  it.
- **Fix:** Give each right-column section its own card and header
  divider so scrolling reads as a stack of complete tools, not one wall.

### 3. Transport, timeline and frame actions are visually fragmented

- **What looks wrong:** The primary playback cluster sits under the
  stage at the left, frame creation actions sit at the right on the same
  line, the ruler and frame chips sit below as tiny isolated buttons,
  and “Set loop” is left alone on another line. Nothing groups these as
  one timeline system, while the stage takes most of the window.
- **Where:** Centre column under the stage: `transport_row`,
  `timeline_marks`, loop row and status strip.
- **Charter rule:** #13 “Timeline block” — ruler, chips, markers, clips
  lane and loop row in one card.
- **Fix:** Place the whole timeline stack in one radius-12 card with
  padding 8 and visually align the transport and frame actions with it.

### 4. Camera controls are duplicated, and neither copy sits on the stage

- **What looks wrong:** The right Camera panel has Fit / Centre / 1:1,
  a zoom stepper and angle controls; a second Fit / Centre / 1:1 / zoom
  cluster floats as a plain row beneath the stage. The user sees the
  same controls twice in different forms, and the stage itself has no
  compact camera affordance.
- **Where:** Camera panel in the right column and `stage-overlay-row`
  directly beneath the centre stage.
- **Charter rule:** #14 — the camera overlay becomes a floating card
  pill on/over the stage; #4 one button family.
- **Fix:** Make the under-stage camera cluster a single floating pill
  (`t.card`, radius 999, px 10 / py 4, `card_border`) and leave detailed
  key/follow controls to the Camera panel.

### 5. The top chrome is three stacked rows of near-identical pills

- **What looks wrong:** “Untitled” is plain text at upper left while
  Save / Load / Undo / Redo float at upper right; File / Edit / View /
  Help form another pill row; eight workspace chips form a third. All
  three rows use similar dark pills, so app actions, menus and
  navigation compete at the same visual level, and there is no divider
  separating chrome from work.
- **Where:** App bar, menu bar and workspace row at the root of
  `view()`.
- **Charter rule:** #15 app bar — TITLE project name, accent dirty dot,
  hairline divider under the bar and workspace row; #4 one button
  family.
- **Fix:** Rebuild the app bar as a titled bar with a dirty dot and
  hairline, and give menus and workspace navigation visibly different
  treatments from ordinary content buttons.

### 6. Workspace chips advertise shortcuts louder than destinations

- **What looks wrong:** Every workspace chip says “Name · Alt+N”, making
  the navigation row long, noisy and more about keyboard shortcuts than
  places. A full-sentence blurb for the selected workspace then takes
  another row — particularly heavy in Build3D, where the sentence is
  short but still consumes a separate band.
- **Where:** Workspace row and blurb line above the main row.
- **Charter rule:** #15 app/workspace hierarchy; #11 shortcut hints as
  `kbd` chips.
- **Fix:** Make the workspace name the primary label, demote “Alt+N” to
  a small `kbd` chip, and render the blurb as one muted meta line in
  the header cluster rather than a competing text band.

### 7. Button importance, danger and disabled states are inconsistent

- **What looks wrong:** Save, menus, workspace chips, transport,
  panel actions and destructive actions are almost all the same dark
  pill. “Delete key” in Camera is neutral, “Delete” under the stage is
  neutral, but “Delete figure” is a filled coral button. Some
  unavailable-looking pose actions are only slightly greyer than live
  ones, so enabled, inactive, selected and dangerous do not form a
  reliable visual language.
- **Where:** App bar, Camera panel, transport/frame row, Figure
  inspector, Pose tools and panels generally.
- **Charter rule:** #4 — one button family; active is accent fill with
  white text; inert is muted with no handler.
- **Fix:** Retire local button styles onto `btn` / `icon_btn` and their
  inert twins, with one consistent treatment each for primary, neutral,
  selected, inert and destructive actions.

### 8. The selected figure row turns into a cramped two-row toolbar

- **What looks wrong:** The Styk row combines a thumbnail, selection
  dot, name, percentage, lock, “solo”, zoom-like −/+, an eye, then a
  second wrapped row of chevrons, an “opac” abbreviation and delete.
  It is much taller than a list row, the controls have mixed sizes and
  meanings, and the second row feels like overflow rather than design.
- **Where:** Figures panel, left column, selected figure card/row.
- **Charter rule:** #5 list rows — min height 28, px 8, radius 8, hover
  and selected treatments; #6 steppers for numeric adjustment.
- **Fix:** Build the identity line on `design::list_row`, then place
  the secondary actions in one deliberate, evenly spaced control row
  using kit icon buttons and a proper opacity stepper.

### 9. The 3D panel’s preview and footer are not contained cleanly

- **What looks wrong:** In the Phase M default snap the 3D panel runs
  to the bottom edge and its “Preview / Grid / Frame / − / +” footer is
  sliced by the viewport; in the stage snap a toast covers that footer.
  The preview itself is a large grey rectangle with no clear inner
  frame, and “Enter 3D” / “Exit 3D” is a bright blue button unlike the
  surrounding panel controls.
- **Where:** 3D panel at the bottom of the right column.
- **Charter rule:** #1 panel card; #2 one header; #4 one button family.
- **Fix:** Contain header, preview and footer inside one `panel_col`
  card with the preview inset by the panel padding and the footer kept
  as a complete final row.

### 10. The status strip is cramped, plain and partly truncated

- **What looks wrong:** Useful state is packed into one small line —
  project/time, frame, timecode, fps, zoom, figure, undo, play state
  and a hint — separated only by spaces. In the supplied snaps the tail
  is cut to “Drag a joint to…” or “3D joint:…”, and a toast can cover
  it. Numbers do not stand apart from labels.
- **Where:** Bottom of the centre column, `status_strip_state`.
- **Charter rule:** #12 — hairline tick separators; numeric segments in
  MONO_CAPTION.
- **Fix:** Rebuild the strip with 1×10 separator ticks and mono numeric
  segments, keeping the flexible hint as the only truncating segment.

### 11. The toast is oversized and occludes working controls

- **What looks wrong:** The “3D joint: chest.” toast is a large black
  card floating over the bottom of the 3D panel, covering controls and
  the status area. For a minor selection message it has the visual
  weight of a modal dialog, and its blue dot gives no clear severity
  meaning at a glance.
- **Where:** Bottom-right overlay in the Phase L 3D and Phase M
  3D-stage snaps.
- **Charter rule:** #9 floating surfaces use the shared card treatment;
  indirectly #12, because it currently competes with the status strip.
- **Fix:** Use the shared compact floating-card style and anchor toasts
  clear of panel footers and the status strip’s information tail.

### 12. Panel headings and section labels are reinvented everywhere

- **What looks wrong:** “Project”, “Figures” and “Library” are plain
  white labels; Settings is a full-width filled strip with an icon;
  Camera is a small grey label with an icon; Figure inspector uses a
  larger card title; “Segment style”, “Pose tools”, “Keys” and
  “Follow” each have another weight or colour. Counts and trailing
  actions are sometimes inline, sometimes absent, never systematic.
- **Where:** Every panel header and sub-section label in both columns.
- **Charter rule:** #2 one header; #3 section labels from the kit in
  `type_::SECTION` / `text_secondary`.
- **Fix:** Replace local headings with `design::panel_title` in a
  `header_row`, and all internal headings with `design::section_label`.

### 13. Steppers do not share a recognisable shape

- **What looks wrong:** Camera zoom is “−  100%  +” with the value as
  ordinary text; camera angle is three equal buttons (“−15° / 0° /
  +15°”); Figure opacity puts −/+ at the far edge; Scale presents
  “90%” and “110%” as decorated action buttons; Head ring uses still
  another arrangement. None gives the value a stable centred readout.
- **Where:** Camera panel and Figure inspector primarily; the same ad
  hoc pattern appears across panels.
- **Charter rule:** #6 — label grows; − button; value in MONO_CAPTION,
  centred, min-width 34; + button.
- **Fix:** Rebuild every numeric stepper around
  `design::stepper_value` with the same icon-only −/+ controls.

### 14. The camera rotation slider is nearly invisible

- **What looks wrong:** The rotation control is a very thin dark line
  with faint ticks and a small blue diamond that reads more like a
  separator or decoration than a draggable slider. There is no visible
  filled portion showing the current value’s progress.
- **Where:** Camera panel, beneath the −15° / 0° / +15° row.
- **Charter rule:** #8 — slider track from the kit: `control_track`,
  accent fill, knob dot at the value.
- **Fix:** Keep the existing drag logic but render it with
  `design::slider_track` so track, fill and knob are unmistakable.

### 15. Boolean states are disguised as ordinary buttons and chips

- **What looks wrong:** Hide, solo, the figure eye, Follow / Selected
  and the 3D Grid control all express on/off state through generic
  buttons or icons. “Following: off” is stated in text while Follow and
  Selected still look like two equivalent actions; Grid is a blue
  button rather than a switch state. State has to be read, not seen.
- **Where:** Figures rows, Camera Follow block, 3D preview footer and
  toggle-like controls generally.
- **Charter rule:** #7 — one pill switch: 28×16 track, accent when on,
  `control_off` when off, 12px `t.knob`.
- **Fix:** Use `design::toggle` / `toggle_row` wherever the control is
  genuinely binary, keeping the existing key on the pressable root.

### 16. Search and name fields do not clearly read as editable

- **What looks wrong:** “Filter figures” and “Search library” are
  mostly a magnifier and muted text floating on black; they look like
  labels or placeholders for a list, not input fields. The Project name
  field, by contrast, has a visible box, proving the inconsistency.
- **Where:** Figures filter, Library search and panel text fields
  generally.
- **Charter rule:** #1’s card/control framing and the charter’s rule
  that panels style text-field frames rather than their kit internals.
- **Fix:** Give every panel text field the same bordered control frame,
  radius and padding, with its icon in `text_secondary`.

### 17. Empty and helper states are bare text — or a one-off filled bar

- **What looks wrong:** “No saved projects in ./projects/ yet.” and
  “No camera keys yet.” are small grey sentences with no visual state.
  Elsewhere, “Select a segment on the stage to edit it.” becomes a
  full-width filled rectangle. The same kind of message therefore looks
  like body text in one panel and a panel inside a panel in another.
- **Where:** Project panel, Camera Keys block, segment inspector.
- **Charter rule:** #10 — kit `empty_state`: centred 20px icon plus
  LABEL text in `text_secondary`, py 12.
- **Fix:** Replace every bare or one-off empty/helper message with
  `design::empty_state` using a relevant existing icon.

### 18. The Figure inspector’s label/value layout wastes its centre

- **What looks wrong:** Name, Colour, Segments, Opacity, Scale and
  Head ring stretch labels to the left and values to the extreme
  right across mostly empty space. “Colour” appears twice — once as a
  metadata row and once as the swatch heading — and unrelated control
  types all share the same loose row rhythm.
- **Where:** Figure inspector, right column in Animate.
- **Charter rule:** #1 panel padding/gap; #3 one section-label system;
  #6 stepper pattern.
- **Fix:** Use compact label/value rows inside the card, remove the
  duplicated Colour label visually by making the swatch block a single
  labelled section, and align all steppers to the same right edge.

### 19. Colour swatches have no visible selection or edge treatment

- **What looks wrong:** The sixteen swatches are plain squares in two
  rows; the white swatch reads like a button, the near-black swatch
  almost disappears into the background, and no ring or check makes
  the current colour obvious.
- **Where:** Figure inspector Colour block.
- **Charter rule:** #5’s selected-state language, applied to swatches;
  the charter also names swatches among the shell’s ad hoc controls.
- **Fix:** Give swatches one radius and hairline edge, with the current
  colour shown by an accent/contrast ring rather than size or fill
  changes.

### 20. Library rows are text lines, not list rows

- **What looks wrong:** Built-in Humanoid / Quadruped / Mini are loose
  text lines with segment counts and “+ Insert” text actions at the
  right. There is no row surface, hover area or selected treatment, so
  the list feels like documentation rather than insertable content.
- **Where:** Library panel, left column in Animate.
- **Charter rule:** #5 list rows — min height 28, px 8, radius 8, hover
  `t.hover`, selected accent treatment.
- **Fix:** Put each library entry on `design::list_row` and make
  Insert a standard icon button at the row’s trailing edge.

### 21. Pose tools form an undifferentiated button cloud

- **What looks wrong:** Mirror pose, Mirror figure, Copy, Paste, Apply
  to all frames, Reset, Bake graph and Save pose wrap as many
  same-weight pills with no grouping or hierarchy. The panel is hard
  to scan, and potentially destructive or project-wide actions look
  identical to harmless clipboard actions.
- **Where:** Pose tools section, right column in Animate.
- **Charter rule:** #1 panel card; #4 one button family and consistent
  state/importance treatments.
- **Fix:** Keep the same actions but group them inside the panel card
  by clipboard / mirror / apply, using the one button family and
  consistent spacing.

### 22. Icon size, colour and rendering weight drift

- **What looks wrong:** The Settings header glyph reads as a large
  white oval/egg rather than a 13px secondary icon; the figure-row eye
  is similarly large and white; the Follow “Selected” target is a
  heavy white blob; other icons, such as Camera’s, are small and grey.
  Icons therefore compete with text instead of supporting it.
- **Where:** Settings header, Figures rows, Camera Follow block and
  panel headers generally.
- **Charter rule:** #2 — header icon 13px in `text_secondary`; #4
  shared button/icon metrics.
- **Fix:** Standardise header and button glyphs through the existing
  icon-button sizes and secondary/active colour rules, with no local
  oversized white glyphs.

### 23. Side-column scrolling cuts tools off without containment

- **What looks wrong:** At the bottom of Animate, “Layers” is sliced
  at the left edge and “Keyframes” begins at the right edge with its
  content below the fold; in Build3D the 3D panel footer is similarly
  clipped. Because panels have no cards or bottom padding of their
  own, normal scrolling looks like broken layout.
- **Where:** Bottoms of both side columns in the 1440×900 snaps.
- **Charter rule:** #1 — panel roots with their own padding and gap;
  cards make a scroll boundary read as intentional.
- **Fix:** Keep the existing column scroll, but ensure every child is a
  complete card with panel padding so a partially visible next tool
  reads as scrollable content, not a sliced control.

### 24. The stage surface is close, but not yet the charter’s stage

- **What looks wrong:** The stage is the strongest surface in the app,
  yet it uses radius 8 in `src/main.rs`, has no distinct border against
  similarly dark surroundings in some states, and its project
  background can read as a giant empty panel floating in the shell.
  This is a finish defect, not a functional one.
- **Where:** Centre stage canvas.
- **Charter rule:** #14 — stage radius 12.
- **Fix:** Change the stage canvas radius to 12 and pair it with the
  floating camera pill in #4 above; do not change its geometry or
  camera behaviour.

## Top 10 fixes by user impact

1. **Card every panel with one header** — defects #1, #2, #12.
   This is the single change that makes the whole app read as a
   designed product instead of controls on a background.
2. **Make the timeline one card** — defect #3. Playback and frame work
   are the centre of the app and are used constantly.
3. **Unify buttons and their states** — defect #7. Users should not
   have to guess what is live, selected, inert or destructive.
4. **Turn the stage camera cluster into the floating pill and remove
   the visual duplication** — defect #4. It cleans the stage area and
   gives camera framing a permanent, findable home.
5. **Rebuild the top chrome and workspace row** — defects #5, #6.
   Clear navigation and a proper app bar improve every session from
   the first second.
6. **Fix the Figures list row** — defect #8. Figures are selected and
   adjusted constantly; the current row is the most crowded control in
   the shell.
7. **Standardise steppers and the slider** — defects #13, #14. Numeric
   camera and figure adjustments become faster to scan and harder to
   misread.
8. **Contain the 3D panel and its footer** — defect #9. The newest
   workspace currently looks clipped at a common window size.
9. **Repair the status strip** — defect #10. It is always visible, and
   its current truncation hides exactly the context it exists to show.
10. **Use real toggles for binary state** — defect #15. Follow, Grid,
    visibility and solo should be understandable at a glance without
    reading status text.
