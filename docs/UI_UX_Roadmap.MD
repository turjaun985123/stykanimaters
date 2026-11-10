# StykAnimate-RS — UI/UX Roadmap

## The ultimate professional 2D stickman animation studio

**Status:** Synthesis of the 30-seat UI/UX audit (findings `a1.md`–`a30.md`,
repo master @ `4edd39b`, Wave AB closed green). Written 2026-11-10.
**Scope:** product roadmap, not a wave board. Items already in flight in
Wave AC (AC0, AC1) are marked as such and never double-counted as new
proposals. Standing user decisions are listed in §8 with their evidence
and are **not** smuggled into any phase as if decided.

**Method note.** Every claim below traces to a findings seat; seat IDs
are cited per item. One evidence caveat: `a10.md` on disk is truncated —
it contains its full §1 (current state, including the P0 defect in
§1.6) but its §2/§3 were cut off mid-write. Audio roadmap items are
therefore derived from A10's §1 evidence and cited as such, rather
than carrying invented A10 item numbers.

**Effort scale (house convention):** S ≈ one wave seat / days ·
M ≈ a focused wave slice / 2–3 seats · L ≈ a dedicated wave or
multi-seat programme. Every item carries its findings seats.

---

## 1. Vision & positioning

**StykAnimate-RS is the professional studio for stickman animation:
the depth of a DCC, the honesty of an instrument, and the only
animation tool an AI agent can drive end to end.**

The audit's competitive finding (A28) is that this position is
already half-won and entirely perishable:

- **Engineering breadth no stickman rival matches.** Against Pivot
  5.2, Stick Nodes Pro, OpenToonz, Krita, Blender Grease Pencil,
  Adobe Animate, Spine and SpriteLoop, StykAnimate-RS already leads
  or matches on graph editing, dopesheet, audio DSP, subtitles
  (speaker credits, style, SRT/WebVTT), format interop
  (.piv/.stk/.nodes/.stknds), deterministic exports, and 28-language
  localisation.
- **One capability nobody else has at all.** A 392-tool MCP server
  lets an AI agent build, pose, render and export a whole scene —
  and *see* its work (`render_frame_base64`). Every rival scores ○
  on that row. In February 2026 Adobe put Animate into permanent
  maintenance mode while its owner pivoted to AI; the largest pool
  of 2D tween animators alive is looking for a living tool, and
  the "AI-native stickman studio" position is unclaimed.
- **The blockers are not features.** They are, in A28's order:
  **reach** (the stickman audience lives on Windows and Android;
  the only shipped artefact is a Linux CI binary), **ecosystem**
  (Stick Nodes' 30,000+ downloadable community figures are the
  niche's real moat; our library is a local folder), and
  **production reuse** (movieclips are flat pose lists — no
  nesting, no reusable cycles — while every serious rival's
  workflow is built on reusable symbols/clips).

The vision this roadmap serves, in one paragraph: a stickman
animator — or an AI agent working for one — can take an episode
from story to published video inside one tool: cast characters
whose identity is locked and portable, pose with professional
direct-manipulation feel, time it on a dopesheet that tells the
truth about time, give it a camera with intent, finish its sound
and subtitles in the user's own language, and export a master
that is *provably* what the editor showed — on the platform the
audience actually uses. The studio's existing character —
determinism, no fake controls, one undo step per mutation,
verdicts read from artefacts never conclusions — is not overhead
on that vision; it is the brand.

**Positioning statements the roadmap protects:**

1. *Stickman-only, studio-grade.* The stickman rule is a feature:
   characters stay recognisable stickmen with individual colour,
   head style, proportions, clothing and accessories. Cutout rigs
   and mesh character systems stay rejected (standing exclusion).
2. *The agent-drivable studio.* MCP parity is a product surface,
   not an API afterthought (T4, §4).
3. *Output you can trust.* What the stage shows is what the file
   contains — the two P0 defects in §3 are P0 precisely because
   they break this sentence today.

---

## 2. Audit summary

One verdict per area. Detail, measurements and item lists live in
the findings files; this section carries the headline and the
strongest single piece of evidence.

### A1 — Shell & workspaces
Genuinely good bones: 8 workspaces, 30 render-gated sections, real
unmounting. But the stage rect at 1440×900 is 52.8% of the main-row
width (~30% of window area) against the 2.0 directive's 65–80%
band; one global layout serves all workspaces with no per-workspace
memory; the Graph editor is crammed into a 340 pt column; subtitles
have no Section of their own. REDESIGN_PROGRESS §1/§4 are two
generations stale — a documentation trap for every future reader.

### A2 — Stage & canvas
The transform core is excellent (one ViewTransform family, pinned
round-trip exactness, export parity). The interaction layer is a
generation behind: no hover feedback, no cursor shapes, no live
drag readout — posing is blind until release. Picking tolerance is
13.0 in *stage units*, not screen pixels (≈1 px at 0.1× zoom,
~100 px magnet at 8×). Segments aren't clickable, sprites have no
rotate/scale handles, the motion path is invisible despite the
core flag and data existing, and the stage does not show real
media artwork (accent quads and 3×5 glyph boxes stand in).

### A3 — Rigging & figure building
The engine is professional (limit-aware FK/IK with wrap-arc
clamping, set-rest with Rebase/Absolute, per-segment style, media
funnel). The building UX is not: there is no Build workspace
(merged into Build3D, the 3D workspace), construction is
stepper-only add/leaf-delete with no reparent, insert, duplicate-
subtree or mirror, and several rig properties (IK-end, bend side,
head toggle) are constructor-only — custom rigs can never acquire
IK. Identity is critically thin: figure colour is a 16-swatch
palette with no custom colour, and head style is a project-global
boolean edited from a per-figure panel, so two characters cannot
differ in head size or style. There is no character entity: each
instance owns a cloned FigureDef, so rigs diverge silently.

### A4 — Posing & animation model
Two animation systems that never meet — dense per-frame poses and
sparse graph channels, with bake as the only bridge. Squash &
stretch is *structurally impossible* (Pose = {root, angles}; no
scale). Root motion lerps linearly; arcs exist only via segment
bend. The motion-path overlay exists in core (`show_paths`,
`nudge_path_point`, `smooth_path_roots`) with no consumer in the
shell. Defaults fight quality: 12 fps, tween off, Linear easing;
`insert_inbetween` is a fixed t=0.5 linear split with no
breakdown/favour control; joint limits are never applied to
stored poses, so a pose silently snaps on first drag.

### A5 — Timeline & dopesheet
No timeline zoom at all: chip width is a fixed 30 pt, so a 90 s
episode at 24 fps is ~73,000 pt of scroll. The dopesheet is
read-only — cells select and seek, but no key can be moved,
copied or deleted there; rows cap at 4 with a non-interactive
"+N more". Three conflicting definitions of "keyframe" coexist
(dopesheet, Playback, goto). The ruler and timecode are not
duration-weighted, so held frames falsify both. Strengths are
real: ruler-drag scrub, chip range-select, draggable loop edges,
and a lossless four-kind mark composition.

### A6 — Graph editor
A pro-grade engine (cubic Hermite, broken tangents, three interp
modes, angle-aware auto-tangents, ~47 graph MCP tools at parity)
behind a surface a generation behind it: there is **no
key-selection model** — the "selected" key is implicitly the
playhead-nearest, clicking a key does not select it, and there is
no delete-key control in the panel at all. Auto-fit only, no
zoom/pan; a fixed 170 pt canvas in a sidebar; no numeric entry,
no live drag readout, no snapping (keys land on fractional
frames). The engine's multi-channel `overlay_curves` view is used
by no shell surface.

### A7 — Inspectors & forms
Phase N's kit is a real form kit and the stepper doctrine is
exemplary (form computed from the same pure helper the op clamps
with; one press = one undo). The headline gap: **no direct
numeric entry anywhere in the app** — all 21 text fields are word
fields, and the worst cases are ~54 presses for a length, ~96
presses per colour, up to 500 per media-offset axis. Real sliders
exist for exactly three values shell-wide. Opacity step is 0.05 in
one panel and 0.1 in another; `opacity_text` is defined twice;
no panel has collapsible sections, so a ~40-row segment inspector
is always fully expanded with four values shown twice.

### A8 — Sprites & media
The render core is strong: one funnel (`media::media_items`) for
stage, raster and SVG; segment media follows bend arcs; layered
import validates before writing and degrades never drops; the MCP
asset table is professional (8 MiB gate, reference census, refusal
+ remap). The shell lags: **no GUI asset manager** (import, rename,
remove and inspection are MCP-only; GUI pickers are text buttons
truncated to 18 chars plus an archaic `./import` folder flow).
Direct manipulation is move-only. Z-order is four fragmented
worlds — a sprite can never go behind or between figures — and
sprites/texts have no name, hidden or locked state. Text instances
provably ignore key opacity (alpha hard-coded 1.0).

### A9 — Subtitles & text
The data layer is genuinely professional: half-open frame ranges,
one validation rule at every boundary, speaker normalised at write
paths, loss *counted* when SRT drops speakers, frame-exact round
trips. The cliff is at the raster surface: burn-in renders through
the 3×5 bitmap font — ~68 glyphs, lowercase folded to uppercase,
**everything else renders as `?`** — so Bengali, accented or CJK
dialogue is destroyed in the delivered MP4 while the SRT sidecar
and SVG export of the same cue are perfect UTF-8 (§3, P0-2). The
panel is buried at the foot of the Audio panel; there is no
timecode anywhere in the subtitle UI; fps changes silently retime
every cue.

### A10 — Audio (see method note: §1 evidence only)
The engine is real DSP with a genuine mixer, but the model is
four fields (path/start/gain/muted — edits materialise destructive
WAV forks with constant parameters: fade 0.25 s, echo 0.18 s/35%,
pitch +2 semitones only). There is no trim control in the panel at
all, gain is a linear stepper with no dB readout or meter, add-clip
is a typed filename, and the 2,376-line waveform view shows one
clip stretched across the strip regardless of its project span.
**§1.6 documents the audit's worst defect: full-length MP4 export
can silently lose audio** (§3, P0-1). And there is no recording or
voiceover path anywhere — for a dialogue-driven channel, lines are
recorded in another app, exported as WAV, copied to `./audio/` and
typed in by name.

### A11 — Camera
A real keyframed track with rotation and editor/export parity
pinned by tests; a broad panel; a working perspective Camera3 in
Build3D. But **every camera move is linear** — CameraKey has no
easing or interp field while poses and graph keys have both, and
zoom lerps the factor so dollies perceptually accelerate. Frame
ops never remap camera keys, so deleting or shifting a frame
range silently desyncs every camera move. Composition guides
describe the stage rectangle, not the shot — they lie the moment
the camera moves. There is no parallax anywhere in the codebase
(grep: zero hits).

### A12 — Export & publishing
A dedicated workspace, 16 format rows, severity-coloured class-C
recaps, and honesty notes that warn *before* the press — a genuine
strength. But three engine capabilities have zero panel presence
(export scale, alpha exports, encoder quality: MP4 ships with no
CRF at all — ffmpeg's default 23; WebM is a fixed 2 Mbps).
Destinations are hard-coded CWD-relative names with `ffmpeg -y`:
**episode 2 silently overwrites episode 1's master.** No caption
sidecar in the export flow, no loudness step, no thumbnail export,
no cancel. Recaps are machine-read data (class-C) and stay frozen.

### A13 — Home & projects
The backend is unusually honest (mtime-keyed scan caches,
content-compared recovery gating, pixel-proven thumbnails with an
ink-mask pair gate: identical = 0, different = 48–51, floor 12).
The surface is a collapsible card in a ~300 pt sidebar. Corrupt
project files vanish silently — the scan lists only files that
fully parse. Recovery is one global `.recovery.json` whose banner
names no project, time or divergence, and any project's autosave
overwrites every other's. Delete is permanent `remove_file`, no
trash. The two-"New project" disagreement (demo vs blank) stands
open (§8).

### A14 — Formats & interop
The native format is plain pretty JSON with serde-default
absorption but no migration mechanism and no published schema —
and it is the least portable home: image assets serialise as
number arrays (~3–4 bytes of text per image byte, duplicated into
every undo snapshot), and audio clips are bare file paths that
silently detach when a project moves. There is no packaged
format. Pivot v5 is the flagship interop but asymmetric: export
writes image side tables, import walks them to `media: None` —
only the AB6 flip pair crosses back. `.stknds` export has the
widest drop list and *cannot report it* (returns `Vec<u8>`, not a
Result). Subtitles are the model citizen: SRT export counts its
speaker loss. The roadmap generalises that pattern (R14.7).

### A15 — Undo, autosave & recovery
Safety engineering is strong (atomic writes, content-compare
recovery gate, close-flush) but history UX is a generation
behind: undo entries are bare serialised Projects, cap 60,
linear, **unlabelled, with no history panel**. A documented census
of non-undoable project mutations exists (layer lock/hide/opacity,
settings steppers, segment colour/shape, figure opacity, camera
"Clear key") — experienced by users as "undo is broken". Autosave
is edit-count-only (25 counted edits), one global slot,
cwd-relative; the dirty dot means "edits since autosave", not
"modified since named save". The GUI binary installs no panic
hook and keeps no crash log.

### A16 — Playback & performance
Playback is poll-driven with no deadline anchoring, no vsync and
no drop policy: the real frame period is work + interval, so
playback silently runs slow under load, and `advance_play` charges
the whole tick at the current frame's duration, mis-timing holds.
Evaluation is O(figures²) — `poses_at` clones all poses per call
and channel application runs ~2× per figure per redraw; the
export funnel evaluates once, the editor doesn't. Media is
re-decoded on the hot path (a full PNG decode just for
dimensions). Export's subtitle overlay clones the entire Project,
asset bytes included, per rendered frame. Editor and export
disagree about time: fractional holds play as authored but export
rounds them. Zero instrumentation: no benchmarks, no fps meter,
no perf CI.

### A17 — Rendering quality
Geometry and determinism are strong (analytic taper/gradient
styling, premultiplied-correct alpha funnel, true re-render export
scale). Sampling caps the quality: anti-aliasing is a fixed 2×
binary supersample; the vector pass samples at pixel corners while
media samples at centres — a permanent 0.25 px misregistration;
all main-funnel colour math is 8-bit sRGB-space with truncating
division, while the newer alpha funnel rounds properly in
premultiplied f32 (the funnel producing 99% of delivered frames
is the less accurate one). Media composites after downsampling,
nearest-neighbour, single-tap, re-decoded every frame. Encoders
are uncontrolled (no CRF, no bt709 tags, WebM flat 2 Mbps), odd
stage dimensions fail video encode outright, and ffmpeg framerate
is truncated to integer (29.97 → 29) causing A/V drift.
Determinism is a strength to protect: quality upgrades must land
as new modes, not silent changes to the default stream.

### A18 — MCP & automation
392 tools with the undo census holding, and a live bridge that is
explicit-enable, default-off, loopback-only with DNS-rebinding
guards. The agent can see its work (`render_frame_base64`,
`render_contact_sheet`) — a real differentiator. But there are no
batch transactions (JSON-RPC batches are refused; an episode
build is thousands of `set_pose` calls, each costing a
full-project serialisation against a 60-step undo cap), renders
are synchronous, uncancellable and editor-freezing on the live
bridge, security is all-or-nothing (`"auth": "none"` — no token,
no read-only mode, no audit), and there is no headless CLI
one-shot mode. The protocol is pinned at 2024-11-05 with free-text
errors that agents must string-match.

### A19 — Internationalisation
A measured 9/19 split: nine languages are genuine (4.5–10.5% of
values identical to English); the other nineteen are 95.2–96.4%
byte-identical English placeholders — everything from Wave R
onward landed as English there. The es-419 trap: fallback goes
only to English, never to the complete Spanish table in the same
file. Whole surfaces are literal English by convention (graph
panel, keyframe tools, layers, sprites, command palette); **no
status or error sentence is localised in any language**; language
choice does not persist across launches. Layout proof is
English-only — genuine-tier strings run 21–28% longer, yet no
headless scene sets a language, so every CLEAN overflow verdict
in project history is an English verdict. Bengali, Arabic, Hebrew
and Chinese depend entirely on OS fonts with no coverage probe
and no pixel proof — silent tofu risk, on top of the §3 font
cliff in the raster path.

### A20 — Accessibility
The measured centre: white-on-accent is exactly **3.14:1**,
computed *and* rendered — and the fill carrying white text is
upstream `tok::accent` = `#4392F7`, not the brief accent
`#6C8CFF` (which carries no white text, so editing app constants
alone fixes nothing). This is a standing user decision (§8) with
pre-computed options. Misses needing nobody's permission: plain
`design::kbd` at 2.88:1 everywhere while the purpose-built
`kbd_on_accent` has **zero production call sites** (palette
selected row ≈1.18:1 — the app's worst pair); menu labels rendered
in "decorative-only" muted ink at ~2.0–2.6:1 in light. Keyboard:
a strong shortcut layer over **no focus model** —
`tab_visits_controls` is unset (standing decision), Rune's focus
builders have zero uses in `src/`, and core posing cannot be
completed keyboard-only. There is no text-scaling lever at all.
Colour-blind safety of the mark system is sound by encoding
design (shape/position channels, not hue) — but no CVD simulation
has ever been run. Rune emits a real accessibility tree; the app
wires **zero** a11y API call sites into it.

### A21 — Keyboard, shortcuts & palette
~21 global bindings and 135 palette commands against 392 MCP
tools: entire pro clusters are unbound (File chords, clipboard,
duplicate, keyframe jump, first/last frame, ±10 stepping,
insert/delete keyframe, nudge, zoom/fit — and the Draw workspace
has *zero* bindings). Bindings are global-or-typing with no
context scoping (N on Home adds a frame; Space plays behind
Export). The palette caps at 12 results with no scroll, matches
no synonyms ("render", "fit", "new project" find nothing), and is
English-only. The honesty discipline is best-in-class — no
invented hints anywhere, drift guards across all three hint
surfaces — and Delete is a hidden three-target cascade whose
priority nothing states at press time.

### A22 — Design system & theming
A real design system: one theme seam, a charter-fixed kit with
broad adoption, WCAG audits recomputed twice and pinned by tests.
One structural incoherence at its centre: **three blues claim to
be "the accent"** — the `#4392F7` fill users see, the `#6C8CFF`
brief accent that carries no white text, and the stage's
`0x6D8CFF` dialect, one digit from the brief value. The light
theme is a derived mirror of the dark one, not a design: four
measured misses, `elevated == card`, zero `.shadow(` uses in the
shell. Icons (163 curated) have never been pixel-verified —
compound-path holes are an assumption. Seven fragmented content
palettes disagree with each other (the same scene-background name
exists at two values) — for a product where *character colour is
identity*, that is a product bug, not a code smell.

### A23 — Onboarding & help
An unusually honest help surface (61 rows, every one verified
real, translated ×28) and an unusually absent onboarding surface:
**no first-run experience at all** — no welcome, tour, tutorial,
sample gallery or persisted "has onboarded" flag. A new user
lands in the richest workspace (Animate, 17 panels) with one
transient status sentence as their only teacher; the demo is a
5-frame arm wave teaching none of the eight workspaces' reasons
to exist. Help is unsearchable, inert (zero press handlers), and
taxonomy-ordered rather than task-ordered. `design::empty_state`
speaks in next actions across ~30 panels but has no action slot —
every teachable moment ends in a dead end. The canvas gestures
that *are* the product (IK/FK/root/wheel semantics) have no
persistent hint anywhere.

### A24 — Errors, status & feedback
The conversational surface of the core is one mutable string
written by ~1,139 sites; severity is **inferred from English
string shapes** by a classifier with live counterexamples (failed
imports — "Could not read…" — classify as neutral Info; origin
refusals are indistinguishable from confirmations). Toasts die in
5 s; the persistent strip tail is uncoloured and capped at 96
chars; there is no message history anywhere. Export feedback is
the strongest subsystem (persistent severity-aware recaps, honest
phase-separated progress) inside its borders: outside them, GIF/
APNG/SVG/project formats freeze the UI synchronously with no
progress and no signal, and **there is no cancel anywhere** — the
only escape from a mistaken 4,000-frame bundle is killing the
app. Silent clamps are a principled doctrine whose mitigation is
visual-only and per-panel hand-mirrored. Class-C sentences are
machine-read data throughout: detectors may migrate, wording
never does.

### A25 — Settings & preferences
One scrolling card, eight persisted fields, high-quality
persistence machinery (atomic writes, field-by-field salvage).
The sharpest finding is a correctness bug: **language does not
persist** — session-only despite the module docs and an MCP reply
both claiming it persists with the project; every non-English
user re-picks their language every launch. Prefs live in
`./projects/` relative to CWD, so launching from another
directory silently resets *all* preferences. The card interleaves
global and per-project settings with no distinction (two controls
silently dirty the current project). Guides, loop, speed, right
sidebar and onion depths all reset every launch — asymmetrically
(left sidebar persists, right doesn't). Autosave has no controls
at all. No window-geometry memory, no new-project defaults, and
the Settings card itself is mostly English literals — the place
where you choose the language is the least translated surface.

### A26 — Testing & verification infrastructure
Headless verification at a standard few commercial tools attempt:
the real binary renders 35 scenes to trees and pixels in CI, a
10-check gate censuses every key/control/colour, and two bespoke
pixel censuses prove canvas-painted features actually paint. The
missing layer is *bindingness*: the headless run is advisory
(manual dispatch, every probe fail-soft), **the overflow scan —
its most important verdict — never runs in CI at all**, only 2 of
34 snapshots are machine-judged, and there are no performance
budgets, no locale matrix and no accessibility automation. Wave X
is the standing lesson: reported CLEAN from workflow conclusions
while the artefact held 40 overflow hits.

### A27 — Platform & distribution
Framing correction: the app is not on GPUI any more; every
platform statement has a historical layer and a Rune layer.
Rune is a git-pinned, *unreleased* framework (rev-pinned, version
0.0.1, multiple upstream commits a day). Exactly one platform has
ever compiled: CI is a single Ubuntu x86-64 job; Windows and
macOS have never been built, anywhere. "Distribution" is a CI
artefact with 90-day retention: no installers, no app icon, **no
LICENSE file in a GPL product**, no signing, no releases.
Storage is CWD-relative `./projects/` while Rune already ships
`AppDirs` unused. Runtime deps (ffmpeg/ffplay, zenity/kdialog)
are unbundled and unchecked at startup. The app ignores its own
argv and implements no drag-and-drop despite Rune having a
complete tested drop API. Touch: proven impossible on the deleted
GPUI shell, unproven-and-unwired on Rune; the tested gesture
recogniser survives orphaned in the library, referenced by zero
shell code.

### A28 — Competitive landscape
Covered as the opening of §1. The three ranked gaps: platform
reach, the invisible MCP moat, and the missing community figure
ecosystem — followed by non-nesting movieclips, Stick Nodes
fidelity leftovers (smart-stretch read-not-modelled, joins baked,
filters parsed-not-simulated), and no lip-sync/dialogue staging.
Posing aids are thinner than Pivot's (onion skins 1–3 vs 0–9 plus
background skin; no colour/style channels). One A28 proposal is
**not** carried into this roadmap: `.spla`/`.spriteloop` package
read/write (its R9) contradicts A14's record that the SpriteLoop
RE workstream's "do not build" verdict stands; it is flagged as a
tension in §4, not scheduled.

### A29 — The creator workflow, end to end
Two production systems exist and have never met. The harness
covers story, storyboard, casting, sound and subtitles
automatically with a quality gate per stage; the app is a deep
animation/subtitle/export studio with nothing at story, casting,
sound generation or publishing. Every real episode so far was
produced harness-only; **the app has never carried an episode end
to end**. The seams are concrete: no `story.json` bridge in either
direction; no episode structure inside a project (one flat frame
vector — no chapters, shots or markers); dialogue is a production
source in the harness and a subtitle-only afterthought in the
app, with word-timing knowledge thrown away at the boundary;
character identity is gate-enforced in one system and memory in
the other; the channel is 9:16 @ 30 fps while the app defaults to
960×720 @ 12 with no Shorts preset and no warning; the app runs
no verification on its own exports; publishing is re-invented per
episode in a loose markdown file. App figures cannot change
expression at all — the deepest capability gap for an emotional
channel.

### A30 — Coherence synthesis
The workspace IA is disciplined (one gating table, argued
placements, tested counts); selection is the app's spine; the
no-fake-controls rule and the one-undo-step contract are coherence
guarantees users can feel. Its eight cross-cutting themes are
adopted as §4's skeleton. Its §3.4 *predicted* collisions —
written when 19 seats were still missing — are resolved against
the now-complete findings in §4.3.

---

## 3. P0 defect register — output that is WRONG today

A P0 here means one thing: the file the user takes away does not
contain what the editor showed and the user made, and nothing
tells them. Two defects qualify, plus a watch list.

### P0-1 — Full-length MP4 export can silently lose audio (A10 §1.6)

**Status: fix already boarded as Wave AC seat AC0 — in flight,
not a new proposal.**

`mux_audio` (export.rs) mixes through the WAV-only mixer. A single
unity-gain clip passes to ffmpeg directly; a gained lone clip
renders a one-clip mix — except a **gained MP3 falls through to
the direct path with its gain silently unapplied**. With two or
more clips, if *any* audible clip is non-WAV, the mix fails and
the code falls back to **the earliest audible clip alone**,
dropping every other clip and every gain — and returns success.
The `missing_audio` note covers missing *files* only; nothing
reports this drop. The range-export path already does it correctly
(half-open clip windows, ffprobe durations, decode-to-WAV
segments, explicit refusal on unprobeable clips); AC0 ports that
machinery to the full-length path.

### P0-2 — Burn-in font cliff destroys non-ASCII dialogue (A9 G1, A19, A17)

Subtitle burn-in, stage subtitle rendering, text instances and
text segment media all stamp through the 3×5 bitmap font
(`glyph_rows`, media.rs): ~68 glyphs, lowercase folded to
uppercase, **every other character rendered as `?`**. Bengali,
accented Latin, CJK and Arabic dialogue is destroyed in the
delivered MP4. Storage, editing, SRT/WebVTT sidecars and SVG
export are all Unicode-correct — the raster font is the sole
failure point. For a dialogue-driven stickman studio whose
primary user's language is Bengali, this is the single most
damaging defect in the product after P0-1. Fix = real font
rasterisation for the raster path (§5, Foundation F-2); it is
one item, merged across A9 R9-1, A17 R17-3, A19 R6/R8 and
A27 R14 — counted once.

### P0 watch list (wrong-output risks, one tier down)

- **Editor/export time disagreement** (A16): fractional holds
  play as authored but export rounds them (a 1.5-beat hold exports
  as 2); ffmpeg framerate truncates to integer (29.97 → 29), a
  standing A/V drift source. Foundation F-6.
- **Odd stage dimensions fail video encode outright** (A17) with
  no pre-flight guard. Foundation-adjacent quick fix in Phase 3
  rendering item, flagged here so it is not lost.
- **WebM advertises alpha it never has** (A17): fed from the RGB
  funnel, its alpha is always opaque.
- **Text instances ignore key opacity** (A8): alpha hard-coded
  1.0 where sprites honour it — authored fades silently flat.
- **Camera desync on frame ops** (A11 R2): deleting/shifting
  frames silently strands every camera move.
- **`.stknds` export cannot report its own losses** (A14):
  returns bytes, not a Result; out-of-timeline keys omitted
  silently.

---

## 4. Cross-cutting themes

Built on A30's eight themes, reconciled against all 30 files.
§4.1 keeps A30's themes with their final evidence; §4.2 is the
merge register (duplicates counted once); §4.3 resolves A30's
§3.4 predictions and its open tensions against the seats that
have since landed.

### 4.1 The eight themes, final form

**T1 — Verification is visual-blind in one direction.** The
headless suite proves layout, presence and overflow, but sound is
user-verified only, most snapshots are judged by no machine, no
scene renders a non-English UI, and no CVD simulation exists.
Every visual proposal inherits this blind spot; the roadmap funds
verification infrastructure as its own line (A26) instead of
attaching 29 separate caveats. *Strengthened by A26 (the net is
advisory — §7) and A19 (layout proof is English-only).*

**T2 — The stepper ceiling.** Numeric precision work is done in
fixed steps everywhere because the kit lacks a committed
numeric-entry / drag-scrub primitive (A7 is the definitive
statement: no direct numeric entry *anywhere*; sliders for
exactly three values shell-wide). One kit primitive plus per-panel
adoptions — never fifteen local inventions. Foundation F-3.

**T3 — Space negotiation between chrome and canvas.** The stage
is at 52.8% of the main row against a 65–80% directive (A1);
timeline lanes are data-gated and shift layout as projects grow
(A30); the dopesheet, subtitle lane, waveform and transport all
compete for the same vertical budget. One timeline-space model
and per-workspace layout memory own this — not per-panel claims.

**T4 — Two doors, one truth.** Panel, 392-tool MCP surface, docs
and catalogue must never diverge. AB1/AB2 proved the audit works
(zero drift across 392 rows, by hand); A26 R9 makes it a script
so it stops costing two seats per run. Every capability item in
§5 carries its MCP/docs shadow explicitly or rides the standing
parity audit.

**T5 — Language completeness is a system, not a pile of keys.**
695 × 28 with a 9-genuine / 19-placeholder split (A19), English
islands by family convention (graph, keyframe tools, sprites,
palette, settings card, all status prose), a fallback chain that
strands es-419 at ~5%, and no rendered-locale proof. One
completeness programme with a coverage instrument — tranches,
fallback, honest tier labels, locale scenes — not per-panel
translation favours.

**T6 — Persistence is thin, CWD-relative and partly false.**
Prefs, recents, recovery and library all live under
`./projects/` relative to the working directory (A15, A25, A27
converge — merge X-12 below); language doesn't persist while two
in-repo texts claim it does (A25 G1); layout memory is one global
slot for eight different rooms (A1, A25). Professionals trust an
app that remembers; this layer is Foundation work, not polish.

**T7 — Accessibility is unowned.** No focus model (decision-gated,
§8), zero a11y API wiring into a tree Rune already emits, no text
scaling, sub-threshold pairs that need nobody's permission to fix
(kbd 2.88 / ~1.18; muted menu text ~2.0–2.6 light), target heights
of 14–20 pt on precision rows, and one measured brand-level
contrast decision standing open. One named theme, staged exactly
as A20/A21 priced it: unblocked fixes first, decision package
second, programme third.

**T8 — The engine is ahead of the shell.** Bend with no stage
gesture, export scale/alpha/quality with no panel presence,
audio trim with no panel control, `overlay_curves` with no
consumer, asset management MCP-only, `show_paths` with no
drawing. The standing parity audit (T4) is the instrument; the
Production phase is largely the shell collecting debts the
engine already paid.

### 4.2 Merge register — duplicates counted once

A30's X-register, extended to the full 30 seats. Each merged item
appears **once** in §5, at the location named.

| ID | Merged item | Seats merged | Lives at |
|----|-------------|--------------|----------|
| X1 | **One shared key-selection model** for dopesheet, timeline and graph | A5 R2, A6 R1/R17 | Foundation F-4 |
| X2 | **One motion-path item** (overlay on evaluated trajectory) | A2 R-A2-12, A4 R2 | Production P-4 |
| X3 | Dopesheet key editing (move/copy/delete) sequenced after X1 | A4 R5, A5 R2/R3 | Production P-2 |
| X4 | **Drawn-target mode counted once** — Wave AC seat **AC1**, already in flight on AB5's Variant D | A6 R8–R10 (restating AB5) | In flight; follow-ons only in Production P-7 |
| X5 | Small targets are T7 instances, with A5's measurements (marker 14 pt, rows 20 pt) | A5, A20-14 | Polish PL-6 |
| X6 | i18n islands are one completeness programme, AB16 partition as baseline | A5, A6 R19, A19, A23 R9, A25 R14 | Polish PL-9 |
| X7 | Subtitles need a Section/gating answer before any subtitle UI grows | A1, A9 R9-5 | Production P-11 |
| X10 | **Real text rendering for raster surfaces** (burn-in, stage texts, text instances) — the P0-2 fix | A9 R9-1, A17 R17-3, A19 R6/R8, A27 R14 | Foundation F-2 |
| X11 | **Decoded-asset cache** (stage hot path, media compositing, subtitle overlay clone) | A8, A16 P2, A17 R17-4 | Foundation F-5 |
| X12 | **Per-user data directories** replacing CWD-relative `./projects/` | A15 R8, A25 R2, A27 R1 | Foundation F-8 |
| X13 | Language persistence (one serde-defaulted field, established pattern) | A19, A25 R1 | Foundation F-9 |
| X14 | Export scale / alpha panel presence — **standing user decisions**, not items | A12, A17, A25 | §8 only |
| X15 | History is two different items — undo history (A15) and message history (A24) — kept separate deliberately | A15 H1/H2, A24 R2 | Production P-14 / Polish PL-11 |
| X16 | Nested clips / reusable cycles — one production-reuse programme | A28 R4, A30 Phase-2 evaluation note | Ecosystem E-3 |
| X17 | Keymap editor — one item, registry first | A21 K11/K12, A25 R13 | Polish PL-8 (registry) / Ecosystem E-6 (editor) |
| X18 | Export cancellation — one item, decision-gated on the class-C ruling | A12, A24 R5 | §8 decision; build slot reserved in Production P-13 note |
| X19 | Onboarding is one programme (first-run, samples, actionable Help, empty states, coach hints) | A23 R1–R5, A28 R12 | Polish PL-7 |
| X20 | Post-export verification (app-side QA probe + partial-artifact honesty) | A29 R6, A24 R6 | Production P-16 |

### 4.3 A30's predictions, resolved

A30 §3.4 was written when 19 seats were missing. Verdicts:

- **D1 (timeline real estate claimed by subtitle/waveform/
  transport seats) — CONFIRMED.** A9's lane exists only when cues
  exist and is panel-less; A10's waveform is clip-time stretched
  over a project-time strip and its clips lane is a passive canvas
  with no handlers. Resolution: the timeline-space model (T3)
  owns lane allocation; A5's items are the dopesheet/chips part,
  A9 P-11 and A10 P-10 are lane citizens, not lane owners.
- **D2 (steppers recur in camera/subtitle/export seats) —
  CONFIRMED and closed by construction.** A11 (camera steppers),
  A9 (subtitle style steppers, ±1-frame timing), A12/A17 (absent
  quality controls) all reduce to T2. A7 R1 is the one primitive;
  §5 lists adoptions, not inventions. A6 R5's numeric inspector
  is an adoption of F-3, as predicted.
- **D3 (empty states / first-run recur) — CONFIRMED, worse than
  predicted.** A23 shows the pattern is structural: one shared
  builder with no action slot, used ~30 times. Consolidated as
  X19: one builder change + one adoption sweep + one first-run
  programme.
- **D4 (MCP parity re-proposed per seat) — CONFIRMED.** A14, A18
  and A29 each restate parity needs. Resolution stands: the
  standing parity audit (T4) plus automation of the schema↔
  catalogue audit (A26 R9); §5 does not carry per-feature parity
  items.
- **D6 (export panel re-described five ways; class-C frozen) —
  CONFIRMED.** A12 (panel), A17 (encoders), A24 (recaps/progress)
  and A29 (QA/publish) all touch the same panel. Resolution: A12
  owns the panel items; A24's constraint is binding on all of
  them — class-C recap/status sentences are machine-read data,
  detectors and carriers may migrate, wording never does.
- **X8 (a4's model changes vs a2's gizmos) — HELD and extended.**
  Pose squash & stretch (model) and stage gizmos (interaction)
  remain separate items (P-5 vs P-3). The same split now also
  governs camera easing (model: A11 R1) vs composition guides
  (surface: A11 R3), and nested clips (model: E-3) vs clip-lane
  editing (surface).
- **X9 (new panels pay a gating-table price) — HELD and
  generalised.** The History panel (A15), Asset Manager (A8),
  subtitles' own Section (X7), the AI panel (A28 R2) and the
  animatic board (A29 R8) each owe a `Section` + workspace-home
  decision, priced in their §5 entries.
- **New tension — A28 R9 (.spla packages) vs A14 (RE verdict
  stands).** Not merged, not scheduled. A14 records the
  SpriteLoop RE workstream's "do not build" verdict as standing;
  A28 proposes package read/write. This roadmap follows the
  standing verdict and flags the disagreement for the user
  rather than resolving it silently (§8, secondary list).
- **New tension — A27 R14 vs A9 R9-1.** Same item (real text),
  two scopes: A27 frames text instances, A9 frames burn-in.
  Merged as X10 with burn-in first (it is the P0).
- **Confirmation — A16 vs A17 on cost.** A17 notes its coverage-AA
  + linear-light pipeline is *faster* than the current 2× SSAA,
  which funds A16's performance concerns rather than competing
  with them. Sequenced together in PL-12/F-5's slipstream.

### 4.4 Standing exclusions (respected, not re-proposed)

- **Cutout rigs and full mesh character deformation** — rejected
  under the stickman rule (SpriteLoop RE mission). Strip-warp
  along bend arcs, as landed, is the boundary.
- **Class-C status/recap sentences are machine-read data.**
  No item in §5 rewords them; A24 R1 migrates *detectors and
  carriers* only.
- **Serialised vocabularies are data** — fx `kind_name`, particle
  kind names, `shape_label` and their kin are file-format
  vocabulary, never "cleanup" targets.
- **Sound is user-verified only** until a harness exists; no
  phase claims audio *quality* verification it cannot perform
  (§7).

---

## 5. Phased roadmap

Sequencing logic (A30 §4, held): platform primitives before panel
adoptions; parity and verification infrastructure before feature
depth; user decisions never smuggled in; every phase closes with
the standing gates green (key census zero lost, overflow scan,
MCP census, i18n parity ×28) — and, once F-7 lands, those gates
are CI properties, not rituals.

**Already in flight (Wave AC, not counted in any phase):**
**AC0** — the P0-1 full-export audio fix (A10 §1.6); **AC1** —
drawn-target mode, Variant D (A6 R8–R10 via AB5; X4).

---

### Phase 1 — Foundation: make everything after this cheap

**F-1 — Close P0-1's verification tail.** AC0 lands the fix; this
item lands its *proof*: a headless/export test family covering
multi-clip mixes with non-WAV members, gained MP3s and gain
preservation, so the silent-drop class cannot regress.
*Why:* a P0 fixed without a regression net is a P0 scheduled to
return. *Effort:* S. *Deps:* AC0. *Seats:* A10 §1.6, A26.

**F-2 — Real font rasterisation for burn-in and raster text
(the P0-2 fix; X10).** Replace the 3×5 stamp path for subtitle
burn-in first, then stage subtitle rendering, text instances and
text segment media, with shaped text from the Rune font stack
(Parley/fontique/swash, embedded Inter + bundled coverage for
Bn/Ar/He/ZhHans per F-10's font work) — keeping the bitmap font
as an explicit pixel-style option, and SVG output unchanged.
*Why:* §3 P0-2. *Effort:* L. *Deps:* none technical; font
bundling decision rides A19 R6. *Seats:* A9 R9-1, A17 R17-3,
A19 R6/R8, A27 R14.

**F-3 — The numeric primitive (T2).** Kit `NumberField`:
click-to-type on the StepForm discipline (form computed from the
same pure helper the op clamps with; inline validation; Esc
reverts), engine exact setters where ops are delta-only today,
drag-scrub, and the unified slider/field/stepper composite.
Adopt first in graph (A6 R5), camera and the segment inspector.
*Why:* A7's headline gap — no direct numeric entry anywhere; the
primitive every later phase spends. *Effort:* L (primitive +
setters) + M (first adoptions). *Deps:* none. *Seats:* A7 R1–R3,
A30 T2, A6 R5.

**F-4 — One shared key-selection model (X1).** Selected-key
state with multi/range selection, one idiom consumed by
dopesheet, timeline and graph; delete-key in the graph panel
rides it. *Why:* A5 and A6 independently name this their #1
missing platform concept; built twice, the app gets two
selection idioms for the same keys. *Effort:* M–L. *Deps:* none.
*Seats:* A5 R2, A6 R1/R2/R17.

**F-5 — Evaluate once + decoded-asset cache (X11).** A project
revision counter (revision + frame is the only honest cache key —
the standing Wave L lesson); stage evaluation computed once per
redraw instead of O(figures²); decoded-asset cache behind
`media_items`; subtitle overlay stops cloning the Project per
frame. *Why:* the editor's felt speed on real projects, and the
substrate for Phase 3's performance budgets. *Effort:* M.
*Deps:* none. *Seats:* A16 P1/P2, A8, A17 R17-4 (cache half).

**F-6 — Timing integrity.** Deadline-anchored playback clock;
duration-walking playhead (holds charged correctly); one
duration-weighted definition of time shared by editor timecode,
total duration and export expansion (fractional holds export as
authored, never rounded); ffmpeg framerate passed exactly.
*Why:* the P0 watch list's time cluster — editor and export
currently disagree about the project's own duration. *Effort:*
M. *Deps:* F-5's revision counter helps. *Seats:* A16 P3/P4,
A17 (framerate), A5 R5 (time-true ruler rides this).

**F-7 — Make the verification net binding (A26 R1).**
`phase_j_checks.py` as a build-workflow job; overflow scan as a
step over the just-produced headless artefact in the same job;
the three instruments' `--self-test`s in CI; a verdict aggregator
that parses every transcript and emits one machine-readable
verdict, with a separate non-fail-soft required check whose
conclusion *is* that verdict. *Why:* §7's central risk — every
instrument exists and passes; what failed historically was their
optionality. *Effort:* M. *Deps:* a maintainer decision to spend
the CI minutes and make the check required. *Seats:* A26 R1.

**F-8 — Per-user data directories (X12).** Adopt Rune `AppDirs`
(projects/library → data, prefs/recents → config/state, recovery
→ state), one-time migration from `./projects/` when the data
dir is empty, and a portable-mode override so the headless
probes' wipe-and-run workflow keeps working. *Why:* the blocker
under all distribution (§7, A27 G1) and under T6. *Effort:* M.
*Deps:* none; precedes F-10's additions to the prefs file and
all Phase 3 packaging. *Seats:* A27 R1, A25 R2, A15 R8 (whose
"confirm, don't silently fix" caution is honoured by the
migration prompt).

**F-9 — Persist the language (X13).** One serde-defaulted `lang`
field in `LayoutPrefs`, seeded at init, written back on change —
the `dark` field's exact precedent — and correction of the two
false persistence claims (settings module docs, MCP reply) in
the same change. *Why:* a live correctness bug hitting every
non-English user every launch. *Effort:* S. *Deps:* none.
*Seats:* A25 R1, A19.

**F-10 — Standing parity audit, automated.** The AB1/AB2
schema↔catalogue audit becomes a script (closing the census's
stated blind spot: argument schemas); i18n key counts and
help-row counts join the gate; the MCP probe's stale
`TOOL_FLOOR` derives from the gate's constant. Plus one manual
parity pass (engine ↔ panel ↔ MCP ↔ docs) to set the baseline.
*Why:* T4/T8 as a repeatable instrument instead of two seats per
run. *Effort:* S–M. *Deps:* none. *Seats:* A26 R9, A30 T4/T8,
A18.

**F-11 — Timeline space model (T3).** Per-lane visibility/
collapse for the timeline card (marker row, clips, dopesheet,
subtitle lane), the C4 dopesheet-gating documentation
reconciled, and per-workspace layout memory designed against
F-8's store (build lands in Polish PL-10 with A25 R10).
*Why:* one owner for the real estate D1's claimants share.
*Effort:* M. *Deps:* none. *Seats:* A30 T3, A1 R1/R3/R7
(stage-share recovery design rides this item's layout work).

**Phase 1 acceptance criteria (measurable):**
- A Bengali subtitle cue burns into an exported MP4 frame
  legibly (pixel census leg added beside the existing anchor/
  speaker legs; no `?` substitution for Bn/De/Zh test strings).
- The F-1 test family fails on the pre-AC0 code path and passes
  after; gained-MP3 and 3-clip mixed-format exports match the
  range-export result bit-for-bit in clip membership and gain.
- CI: gate 10/10 and overflow scan run as required checks; a
  planted overflow in a test branch fails the required check.
- Selection: the same key-selection state is observable from
  dopesheet and graph in a snap test (select in one, highlighted
  in the other).
- Numeric entry: length, an RGB colour and a media offset can
  each be set exactly by typing, in ≤1 field interaction each,
  in the three adoption panels.
- Performance baseline measured and recorded (stress scene from
  PL-13 may land early to serve as the fixture): stage redraw
  evaluation count per frame is O(figures), not O(figures²), in
  the instrumented build.
- Language survives relaunch (snap test: set Bn, re-init, UI
  language is Bn); prefs load identically from two different
  working directories after migration.

---

### Phase 2 — Production depth: the working animator's day

**P-1 — Timeline zoom + fit/overview (A5 R1).** Zoomable chip
scale, fit-to-project, and an overview strip. *Why:* 73,000 pt
of scroll for one episode is the timeline's defining absurdity.
*Effort:* M. *Deps:* F-11. *Seats:* A5 R1.

**P-2 — Dopesheet key editing + exposure spans (X3).** Move/
copy/delete keys from the lane on the F-4 selection model;
exposure spans that show holds as durations (the xsheet lesson,
A28) on F-6's time-true ruler; the 4-row cap becomes scrollable
rows. *Effort:* L. *Deps:* F-4, F-6. *Seats:* A5 R2/R3/R6,
A4 R5.

**P-3 — Stage feedback & precision (A2).** Pixel-true picking
(screen-space tolerance), hover highlighting, cursor shapes,
live drag HUD/readout, segment clicking, snapping, keyboard
nudge, coordinate readout. *Why:* posing blind until release is
the stage's core feel defect. *Effort:* M–L. *Deps:* F-3
(readout reuses the numeric idiom). *Seats:* A2 R-A2-1…R-A2-11
(top-3 cluster).

**P-4 — Motion-path overlay (X2, one item).** The evaluated
trajectory drawn on stage from the existing `show_paths` data,
with draggable path points (`nudge_path_point`,
`smooth_path_roots` gain their consumer). *Effort:* M. *Deps:*
none. *Seats:* A4 R2 (framing), A2 R-A2-12.

**P-5 — Posing depth: breakdowns, then squash & stretch.**
(a) Breakdown insert with favour slider replacing fixed t=0.5
linear `insert_inbetween` (M). (b) Pose-level scale (sx/sy) —
a **data-model change**, priced and sequenced separately per X8:
format/compat treatment, MCP parity, limits applied to stored
poses in the same pass (the silent-snap fix). *Effort:* (a) M,
(b) L. *Deps:* (b) F-10 parity baseline. *Seats:* A4 R1, R3, R9.

**P-6 — Camera truth.** (a) Camera easing/interp on CameraKey
(log-zoom lerp so dollies stop accelerating) — model change,
L. (b) Frame ops remap camera keys (delete/shift/reverse) — the
runner-up correctness fix, M, *queued ahead of (a)*. (c)
Composition guides drawn inside the camera frustum — M.
*Seats:* A11 R1, R2, R3.

**P-7 — Graph follow-ons after AC1.** Zoom/pan with rulers and
gridlines on a taller canvas; the engine's `overlay_curves`
multi-channel view surfaced; per-key tangent modes. *Effort:*
M. *Deps:* AC1 (in flight), F-4. *Seats:* A6 R3/R4, R- overlay,
R- tangent cluster (excluding R8–R10 = AC1).

**P-8 — A real Build workspace + rig construction (A3).**
Build gets its own workspace home (resolving A30 C3's naming
debt in the same move); a rig tree with reparent, insert,
duplicate-subtree and mirror; constructor-only properties
(IK-end, bend side, head toggle, rename) become editable;
joint-limit arcs visible in setup. *Effort:* L. *Deps:* X9
gating price included; F-3 for numeric rig fields. *Seats:*
A3 phase 1–2 items. (IA-AA9-1's set-rest clamp is **not** in
this item — it is a §8 decision.)

**P-9 — Character identity (A3 + A29 R3 foundation).** Per-figure
head style/size and custom colour (beyond the 16 swatches);
a Character entity above FigureDef so instances stop diverging
silently; library entries gain identity metadata. The
Bible-import half lands in Ecosystem E-4 on this foundation.
*Effort:* L. *Deps:* P-8's rig surface. *Seats:* A3 (identity
items), A22 DS-6 (palette of record feeds custom colour).

**P-10 — Audio panel depth (from A10 §1 evidence).** Trim
control in the panel (engine + MCP already have it); file-picker
add flow replacing the typed filename; dB gain with slider and
level meters (peak/rms already computed); waveform reconciled
to project time (span bracket promoted from divergence note to
primary view); clip parameters (fade/echo/pitch) exposed where
the ops can honour them — or the ops learn parameters first.
*Effort:* M–L. *Deps:* F-3. *Seats:* A10 §1.2–§1.5, §1.8
(recording/voiceover explicitly **not** promised here — see
P-15's dialogue lane for the placement seam).

**P-11 — Subtitles as citizens (X7).** Subtitles gain their own
Section and gating answer; timeline-first cue creation (the lane
stops being invisible until the first cue); time-domain timing
(timecode display, nudge-by-time, shift-all; fps-change retiming
made explicit, never silent); CPS/line-length authoring feedback
(A9 R9-2 — feedback only; the stack cap/wrap question stays a
§8 decision). Sidecar bundling into video export rides P-13.
*Effort:* M. *Deps:* F-6 (time model), F-11 (lane space).
*Seats:* A9 R9-5, R9-3, R9-2.

**P-12 — Sprites & media depth (A8).** GUI Asset Manager
(import/rename/remove/inspect with thumbnails, dimensions,
refcounts) on the F-5 decoded cache; real bitmap rendering on
stage (spike first — Home's `rune::Image` thumbnails are the
existence proof); sprite rotate/scale/pivot handles; unified
Outliner with cross-class draw order and named/hideable/
lockable instances; text instances honour key opacity (P0 watch
list). *Effort:* L (manager + outliner), M (stage bitmaps after
spike). *Deps:* F-5; X9 gating price for the manager's home.
*Seats:* A8 R1/R2, R5, R9 + opacity fix.

**P-13 — Export delivery (A12).** Delivery presets (YouTube
1080p/4K/Shorts/GIF) over encoder quality in core (CRF/preset,
WebM bitrate ladder, bt709 tags via PL-12); **named destinations
+ overwrite guard** — the only item preventing outright loss of
a finished master, queued first in this item; "Publish package"
bundle (MP4 + SRT/VTT sidecars + thumbnail still). *Effort:* M.
*Deps:* P-11 (sidecars), PL-12 (encoder tags) — presets may ship
on current encoders first. *Seats:* A12 R12.1–R12.5.
*Cancellation is not in this item:* it waits on the §8 class-C
ruling (X18); the build slot is reserved here so the ruling,
once taken, lands in one wave.

**P-14 — Trust: history & honest undo (A15).** Labelled undo
entries + History panel (X9: Animate home, by the playhead-
family precedent); the non-undoable-mutation census closed
(H3) so "undo is broken" stops being true; honest save-state
vocabulary in the strip (dirty = since named save; autosave age
shown separately). Message history is PL-11, not this item
(X15). *Effort:* M–L. *Deps:* none. *Seats:* A15 H1–H3.

**P-15 — Episode structure + dialogue lane (A29).** Chapters
with title cards and shot markers in the Project model;
timeline/dopesheet navigation by chapter; a dialogue list
(speaker, line, frame range) that generates subtitle cues,
imports word-timing files, and auto-places externally generated
per-line WAVs on the audio lane. *Why:* the app-side foundation
of the creator workflow (S2/S3 seams) — and the precondition
for E-4's bridge. *Effort:* L. *Deps:* P-11 (cues), P-10 (lane).
*Seats:* A29 R1, R4.

**P-16 — Post-export QA probe + partial-artifact honesty
(X20).** After MP4/WebM/bundle exports: probe the produced file
(duration vs project, frame count, audio peak/RMS, black-frame
scan on sampled frames, burn-in spot-check) and show a pass/fail
recap in the export panel — detectors only; class-C sentences
untouched. Sequence failures state their residue; Recent exports
marks incomplete sequences. *Effort:* M. *Deps:* F-1's test
family shares the probe machinery. *Seats:* A29 R6, A24 R6.

**P-17 — MCP throughput (A18).** `apply_ops` transaction tool
(one call, one undo step, all-or-nothing); async render jobs
with progress on the live bridge (cancel rides the §8 ruling);
structured error kinds replacing free-text matching for new
tools. *Effort:* M–L. *Deps:* F-10. *Seats:* A18 R1, R4, +
protocol items.

**Phase 2 acceptance criteria (measurable):**
- A 90 s / 24 fps project fits the timeline in one view (fit
  action) and any key can be moved in the dopesheet with the
  graph and timeline agreeing on its frame (snap test).
- Camera: a two-key dolly exports with eased, log-zoom motion
  (frame-sampled zoom ratios monotonic and non-accelerating);
  deleting frames 10–20 moves affected camera keys by exactly
  −10 (round-trip test).
- One new character can be built, given a custom colour and a
  distinct head size, saved to the library, and instanced twice
  with identity intact (tree + render proof).
- An MP4 exported over an existing master prompts instead of
  overwriting (UI test); the Publish package produces MP4 +
  .srt + .vtt + thumbnail in one job.
- Post-export QA recap appears for every video export and
  *fails loudly* on a planted silent-audio fixture (the P0-1
  regression fixture reused).
- Episode: a 3-chapter project navigates by chapter from the
  timeline; dialogue-lane lines generate cues whose frames match
  the imported word-timing file exactly.
- MCP: a 500-op episode build executes as ≤5 `apply_ops` calls
  and exactly ≤5 undo steps.

---

### Phase 3 — Polish & platform: designed in both themes, on every desktop

**PL-1 — Accent-adjacent contrast fixes (no decision needed).**
`kbd` ink fix + `kbd_on_accent` adoption (2.88 / ~1.18 → ≥4.5);
muted ink retired as menu text; the marginal-pair sweep
(status-strip play ink, dopesheet glyph-on-tint, splitter grip
accept-or-fix recorded). *Effort:* S ×3. *Deps:* none — these
must not wait for the §8 accent decision. *Seats:* A20-1/2/3.

**PL-2 — Design-system completion (A22).** Hard-coded dark
surfaces themed (toasts, camera pill — S–M); light theme given
its own surface ladder and its four measured misses fixed
(M); spacing/radius tokens with a gate check (M); icon set
pixel-verified at SM/MD in both themes, counters fixed or glyphs
replaced (M); one studio content palette consumed by all seven
swatch sets (M); type-scale extension — display + true caption
tiers (S); elevation ladder with applied shadows (S–M); graph
`preset_chip` merged into the kit (S); `docs/DESIGN_TOKENS.md`
generated as the palette of record (S). *Deps:* DS-1's canonical
blue waits on §8; everything else proceeds. *Seats:* A22
DS-2…DS-9, DS-11; A20-15.

**PL-3 — Rendering quality (A17).** Coverage-based AA and the
linear-light/rounded main funnel as a **new quality mode**
(determinism of the default stream untouched); filtered,
supersampled media compositing on the F-5 cache; encoder pass
(CRF/preset, bt709, WebM ladder) shared with P-13; even-dimension
guard and exact-framerate fixes (from F-6) verified here;
centre/corner sampling misregistration eliminated in the new
mode. *Effort:* L. *Deps:* F-5. *Seats:* A17 R17-1/2/4/5.

**PL-4 — Keyboard coverage (A21).** The pro clusters: transport
(Home/End, ±10, keyframe jump, Backspace alias), File/Edit
chords, keyframing cluster with arrow nudge, the Draw cluster —
all shovel-ready on today's `on_key`; Delete's cascade target
named in the status line at selection time; context-scoping
rulings (§8 secondary) applied as decided. *Effort:* S–M.
*Deps:* none. *Seats:* A21 K1–K6.

**PL-5 — Palette v2 + binding registry (A21).** Scrollable/
browsable results, synonym keywords, argument commands, pinnable
favourites; bindings extracted into a data registry that
*generates* help rows, menu hints and palette chips (drift
guards become structural). The keymap *editor* itself is E-6.
*Effort:* M ×2. *Deps:* PL-4 first (registry before the binding
count doubles). *Seats:* A21 K9/K10/K11.

**PL-6 — Accessibility programme (A20, staged).** AT semantics
wired through the kit builders (`label`/`checked`/`range_value`/
`disabled` — the labels already flow to every builder as
tooltips; M); target-height floor for the marker row and
dopesheet rows (S–M, A20-14/X5); UI scale setting 100/125/150%
with the overflow gate extended to scaled scenes (L, DPI
investigation first); reduced-motion audit of the spring pairs
+ in-app preference (S–M); flashing-content caution for FX
previews (S); CVD simulation pass over the mark-system snaps
(A20-10, M). *The focus programme (A20-6/7/8) is staged behind
the §8 `tab_visits_controls` decision — priced there, not here.*
*Seats:* A20-9…A20-14.

**PL-7 — Onboarding programme (X19).** In order: searchable,
actionable Help with a Getting Started section and F1 (M — the
content exists, verified and translated); actionable empty
states (builder gains an action slot; adoption sweep; bare-noun
states rewritten; M); stage & timeline coach hints (M); a
teaching sample family — Wave kept, **Walk cycle** (the
community's lesson 1), a two-figure scene exercising camera/FX/
text/subtitle — with Home's Samples section and open-a-copy
semantics (M); the first-run welcome + 4-step First Animation
coach on a persisted `onboarded` flag (L); `docs/USER-GUIDE.md`
with headless-generated screenshots (M). *Deps:* samples before
the coach; PL-9 localises the new strings as they land.
*Seats:* A23 R1–R8, A28 R12 (merged).

**PL-8 — Settings completion (A25).** Persist the session-only
view state (guides trio, loop, speed, right sidebar, onion
depths; M); split the card by scope — Application vs This
project, with fps/background given a settings home (M);
new-project defaults (fps/stage/background/easing; M); export
choice memory (last format/scale/directory — memory of chosen
values, *not* the §8 panel-control question; S–M); window
geometry persistence (M, scoped to what Rune reports);
configurable MCP port + undo depth (S ×2); preferences
management — reset-all with confirm, prefs path readout,
export/import (S–M). *Deps:* F-8 (paths stable first).
*Seats:* A25 R3–R12.

**PL-9 — Language completeness programme (X6/T5).** Base-language
fallback chain (es-419 → es: ~5% → ~95% for one step; S);
honest tier labels in the language picker (S); the coverage
instrument (per-language identical-to-EN ratio, gate-visible;
S–M); the 19 placeholder languages translated in tranches,
genuine-tier first by user population (L, human-review flagged —
machine drafts with native review, per A28 R11's pipeline);
locale rendering scenes (longest-expansion language + one CJK)
promoted into SCAN_SET (M, with A26 R4); status/feedback
localisation on PL-11's typed carriers (L). *Seats:* A19
R1–R3, R7, R10; A26 R4; A23 R9.

**PL-10 — Per-workspace layout memory (A1/A25).** Column widths,
sidebar and timeline state remembered per workspace; startup
workspace separable from last-used workspace. *Effort:* L.
*Deps:* F-8, F-11, PL-8. *Seats:* A25 R10, A1 R1.

**PL-11 — Feedback completion (A24).** Typed status (kind as
data) — *promoted here from A24 R1 with its L effort stated
plainly; F-6/F-10 do not block it, and PL-9's status
localisation hard-depends on it*; message history with full-text
detail (M, X15); severity in the persistent strip + data-guard
escalation for autosave/save failures (S ×2); every remaining
export format async with honest working lines (L); partial-
artifact honesty already in P-16; clamp-reason tooltips on inert
steppers (M); actionable refusals for the curated high-frequency
families (M). *Class-C sentences untouched throughout.*
*Seats:* A24 R1–R4, R7, R9–R11.

**PL-12 — Snapshot diff harness (A26 R2).** Per-scene baselines
for the 34 snaps with the codebase's proven ink-mask/band
method (never raw pixel equality — refuted on record), per-scene
budgets from measured lavapipe noise, reviewed baseline updates
at docs-close only. Starts with the 12 highest-value snaps.
*Effort:* L. *Deps:* F-7 (to enforce). *Seats:* A26 R2.

**PL-13 — Performance & scale budgets (A26 R3).** The stress
scene (12+ figures, 240 frames, planted subtitles/strokes/
sprites); timings for cold start, tree dumps, snap renders and
MCP round-trips emitted per run; budgets report-only for 3–5
runs, then enforced through F-7's aggregator. *Effort:* M.
*Deps:* F-5 (the fixes this measures), F-7. *Seats:* A26 R3,
A16 P- (instrumentation items).

**PL-14 — Platform reach (A27 + A28 R1).** Windows + macOS CI
build jobs with a burn-down list (M); LICENSE file, app icon,
About surface (S); Linux packaging — oldest-LTS floor, tarball
+ AppImage + .deb (M); Windows portable zip → installer with
ffmpeg story (M; signing cert is an owner procurement, flagged
in §8 secondary); macOS .app/DMG (M; Developer ID is a paid
owner decision — unsigned-with-instructions interim stated
honestly if declined); native dialogs via Rune's rfd/portal
(M); argv/open-with + drag-and-drop in (M — deletes most
`import/`-folder flows); runtime-dependency probe at startup
(S–M); versioned GitHub Releases + in-app update *check*
(notify + link; no silent install) (M). *Deps:* F-8 throughout;
PL-12/PL-13 absorb the new verdicts. *Seats:* A27 R2–R10,
A28 R1.

**Phase 3 acceptance criteria (measurable):**
- Contrast: every text pair in the kit census ≥4.5:1 in both
  themes *except* the §8 accent pair, grandfathered by name;
  kbd on the palette selected row ≥4.5:1.
- AT census: unnamed interactive elements in the default scene
  tree at 0 (S18's 47/229 method re-run); toggle states exposed.
- Locale: settings card + one dense panel render in the longest
  language and one CJK language with the overflow scan CLEAN on
  those trees; es-419 resolves ≥90% of keys from the es table.
- Onboarding: a timed first-session test (new user script) goes
  install → posed figure → second frame → play → GIF export in
  under 5 minutes using only in-app guidance.
- Platform: Windows and macOS CI jobs green for build + tests;
  one signed-or-honestly-labelled installable package per
  desktop attached to a versioned Release; a project opens by
  double-click and by drag-drop on all three desktops.
- Visual: snapshot-diff verdicts exist for ≥12 scenes and a
  planted colour-token regression is caught by them.
- Performance: budgets enforced; the stress scene's tree+snap
  inside budget on the CI runner.

---

### Phase 4 — Ecosystem: the moat, made visible

**E-1 — The AI surface as a product (A28 R2, A18).** An in-app
agent panel (X9 gating price included): live-bridge status,
one-click agent tasks (storyboard a scene, pose a cycle, render
a contact sheet), 6–10 canned MCP recipes, and the tool
catalogue published as the product's front door. Live-bridge
token auth + read-only mode land with it (A18 R6) — a visible
agent surface on an `"auth": "none"` bridge is not shippable.
*Effort:* M–L. *Deps:* P-17. *Seats:* A28 R2, A18 R6.

**E-2 — Community figure library (A28 R3).** In-app catalogue
over a static index (GitHub-hosted JSON + figure files),
search/preview/one-click install into the library, format
badges (.stk/.nodes/.piv), licence note per pack; seeded with
the built-ins and the original StykAnimate community packs.
*Effort:* L. *Deps:* P-9 (identity metadata), PL-14 (the
audience). *Content curation/moderation policy is a human
decision — flagged, not assumed.* *Seats:* A28 R3.

**E-3 — Nested movieclips + cycle library (X16).** Clips contain
figure tracks *and* clip instances with per-instance
offset/speed; a cycles panel ships walk/run/idle/fight as
retargetable clips; flat clips migrate. *Effort:* L. *Deps:*
P-9, F-10 (format treatment). *Seats:* A28 R4.

**E-4 — The harness bridge (A29).** `story.json` importer: beats
→ frames, shot size/angle/move → camera keys, chapters →
P-15 chapters + title cards, dialogue → dialogue lane + cues,
cast → library figures via **Character Bible import** (Bibles
read from `harness/characters/*.json`; drift warned, never
silently overwritten). The import report states exactly what
was mapped and what was dropped (expressions — until E-5 —
named in the report, never silently). *Effort:* L (importer)
+ M (Bible import). *Deps:* P-15, P-9. *Seats:* A29 R2, R3.

**E-5 — Expression & dialogue kit (A28 R6, A29 R12).** A face
layer on the figure head: a closed expression set (mirroring
the harness's 23 names where sensible), keyable as a channel,
rendered in stage + all exporters, MCP-addressable; viseme
mouth shapes with phoneme timing generated from an audio clip
+ its cues; a speech-bubble text primitive that follows a head
node. Scoped as its own mission, not a wave seat. *Effort:* L.
*Deps:* E-4 sharpens it; technically independent. *Seats:*
A29 R12, A28 R6.

**E-6 — Keymap editor + presets (X17).** Over PL-5's registry:
searchable rebind surface, conflict flagging (never silent
override), reset-to-default, import/export JSON, Default /
Blender-like / Toon Boom-like presets. *Effort:* L. *Deps:*
PL-5. *Seats:* A21 K12.

**E-7 — Stick Nodes fidelity round (A28 R5).** Simulate
smart-stretch; model instance filters (blur/glow/tint) at the
rasteriser; keep joins live where the spec allows. Each sub-item
is a priced data-model call under T4 parity. *Effort:* M–L.
*Deps:* PL-3 (filter support). *Seats:* A28 R5, A14 (fidelity
items).

**E-8 — Publishing completion (A29).** Publish-pack generator:
project metadata (title/description/tags) edited in Export, a
thumbnail picker over the contact sheet with overlay text, one
button writing the pack beside the export; animatic board
(shots as cards: thumbnail, duration, dialogue, camera "why")
on P-15's structure; per-chapter look presets; chapter-range
render queue on the bundle machinery. *Effort:* M ×3.
*Deps:* P-15, P-13, P-16. *Seats:* A29 R7, R8, R9, R11.

**E-9 — Format ecosystem completion (A14).** Packaged project
format (zip: project + assets + audio) ending the
number-array/path-detachment era, with base64 asset
serialisation as its in-file representation; Pivot v5 image
side-table import (images attach; flip read back) closing the
flagship asymmetry; per-format export loss reports everywhere,
generalising the SRT counted-loss pattern — `.stknds` first
(its exporter learns to return a Result). *Effort:* L.
*Deps:* F-8 (storage), F-10. *Seats:* A14 R14.1/2/5/7.

**E-10 — Touch, decided by evidence (A27 R11, A28 R7).** Audit
the pinned Rune rev for a desktop touch feed (added to the pin-
upgrade checklist permanently); wire trackpad `on_magnify`
pinch-zoom meanwhile; reconnect or retire the orphaned
`src/touch.rs` recogniser — dead capability in the lib is a
maintenance lie either way. A build phase follows only if the
audit's verdict supports it. *Effort:* S (audit) → M (magnify +
decision). *Seats:* A27 R11, A28 R7.

**Phase 4 acceptance criteria (measurable):**
- A canned recipe, run from the in-app panel, produces a posed,
  subtitled 10 s scene and its contact sheet with zero terminal
  use; the bridge refuses unauthenticated writes in read-only
  mode (probe test).
- A community pack installs in one click and its figures pose
  identically to their source-format originals (round-trip pins).
- One harness `story.json` (the showcase story) imports to a
  project whose chapter boundaries, camera keys, cues and cast
  match the import report line-for-line; the report names every
  dropped element.
- A walk cycle defined once plays, nested, inside two different
  scenes at different speeds (render proof).
- A project saved in the packaged format opens on a second
  machine (CI job, different CWD) with audio and images intact —
  zero detached references.

---

## 6. Effort rollup

Counts use the §5 items; split items (a)/(b) counted once at
their larger effort. In-flight Wave AC work (AC0, AC1) excluded.

| Phase | Items | S | M | L | Centre of gravity |
|-------|-------|---|---|---|-------------------|
| 1 — Foundation | 11 (F-1…F-11) | 2 | 5 | 4 | Two P0 fixes + the primitives (numeric, selection, cache, time) + making QA binding |
| 2 — Production depth | 17 (P-1…P-17) | 0 | 8 | 9 | The shell collecting engine debts: dopesheet, camera, rigging, assets, audio, export delivery, episode structure |
| 3 — Polish & platform | 14 (PL-1…PL-14) | 3* | 7 | 4 | Both themes designed, onboarding, language completeness, packages for three desktops |
| 4 — Ecosystem | 10 (E-1…E-10) | 1 | 3 | 6 | The moat productised: AI surface, community library, nested clips, the harness bridge, expressions |
| **Total** | **52** | **6** | **23** | **23** | |

*PL-1 is three S items counted as one line.

Reading of the rollup: the programme is L-heavy at both ends —
Foundation because primitives and P0 fixes are cross-cutting by
nature, Ecosystem because model changes (nesting, expressions,
packaged format) cannot be sliced smaller honestly. Production
depth is the broadest phase and the most parallelisable: its
items sit in different panels over Foundation's shared
primitives, which is exactly the wave shape this project already
runs well. Polish & platform is the cheapest phase per item and
the one whose deferral is most visible to users — it should not
be raided to fund Phase 2 overruns.

---

## 7. Risks & verification gaps

**R-1 — The advisory net (A26).** The project's strongest checks
are optional: the gate runs in no workflow, the overflow scan
runs in no workflow, and every headless probe is fail-soft, so
green conclusions have hidden red transcripts at least three
times on record, and Wave X was reported CLEAN while its
artefact held 40 overflow hits. Until F-7 lands, *every*
verdict in every phase inherits this risk. This is the
roadmap's first infrastructure priority for a reason.

**R-2 — English-only layout proof (A19, A26).** No headless
scene sets a non-English language. Genuine-tier strings run
21–28% longer than English; RTL is entirely unscoped in layout;
every CLEAN overflow verdict in project history is an English
verdict. Any phase that changes layout before PL-9's locale
scenes land is shipping unverified geometry for 27 languages.

**R-3 — Sound is user-verified only.** No harness can hear
playback or export audio. F-1/P-16 probe *files* (peaks, RMS,
durations, clip membership) — that closes the silent-drop and
truncation classes, but mix quality, sync feel and the drift-
restart seam in playback (A10: the correction is an audible
restart) remain human checks. No phase claims otherwise.

**R-4 — No performance evidence exists.** There are no
benchmarks, budgets or scale fixtures today (A16, A26). F-5
fixes the known algorithmic debts, but until PL-13 measures,
"feels professional at production size" is an assertion, not a
fact — and Phase 2's depth items all add per-frame cost.

**R-5 — Pixel proof covers 2 of 34 snapshots.** Token, theme,
icon and z-order regressions are currently invisible to machines
(A26 G3). PL-2's design-system work — the phase most likely to
*cause* such regressions — lands before PL-12's diff harness
unless the order inside Phase 3 is deliberately swapped.
Recommended: pull PL-12's first 12 scenes ahead of PL-2.

**R-6 — Framework risk (A27 G13).** Rune is unreleased,
rev-pinned and fast-moving, macOS-first with named IME and
platform-feel risks. The pin-upgrade discipline (§8 of
RUNE_MIGRATION) mitigates but cannot eliminate this; the
engine/MCP/formats must stay framework-free so the exit cost
remains the shell layer only. PL-14's cross-platform burn-down
will price this risk concretely for the first time.

**R-7 — Unverified rendering assumptions.** Compound-path icon
holes have never been seen on screen (A22 G5); CVD safety is
argued from encoding design, never simulated (A20); Bn/Ar/He/
ZhHans UI rendering rides on OS fonts with no coverage probe
(A19); software-Vulkan interactive performance is unmeasured
(A27). Each has a named item (PL-2, PL-6, PL-9, PL-13/PL-14) —
until they land, these are open risks, not known goods.

**R-8 — Translation is a people pipeline.** 19 languages × ~670
placeholder keys is not agent work at professional quality
(A19, A28). PL-9's tranches need native review; machine drafts
without review would convert an honest placeholder into a
plausible wrong string — a worse state.

**R-9 — Decision debt compounds.** §8 lists eleven standing
decisions. Several phases are shaped around their absence
(cancel, focus, alpha/scale controls, chain-spanning media);
the longer they stand, the more adoption work must route around
the gaps. They are cheap to take and expensive to defer.

**R-10 — Audit completeness.** A10's findings file is truncated
on disk (§2 method note): its §2/§3 never reached the file, so
audio gaps beyond its §1 evidence (already substantial) may be
under-represented here. Before P-10 is boarded as a wave, the
audio seat should be re-run or its missing sections recovered.

---

## 8. Open user decisions

Each decision is stated with its evidence and its price. None of
these appears in §5 as a build item; where a phase reserves a
slot, it says so. "Defer" is a legitimate answer to each — but
it should be a *recorded* answer, because several are currently
blocking staged engineering.

### D-1 — Accent contrast: white-on-accent at 3.14:1
**Evidence (A20 §1.1, A22 G2):** the fill carrying white text is
upstream `#4392F7` at exactly 3.14:1, computed and rendered; the
brief accent `#6C8CFF` carries no white text. Worst instance:
the palette selected row at α 0.85 = 3.55 dark / **2.38 light** —
and darkening the accent alone does not fix that row in light.
**Options (pre-computed):** (a) keep 3.14 as a documented brand
choice; (b) darken the white-text fill to `#2072D5` (4.72:1) or
`#1C6FD2` (4.94:1); (c) migrate fills to the brief family
`#556FCD` (4.62:1). In all non-(a) cases the palette-row alpha
and the menu hint's white@0.75 are separate sub-choices.
**Price:** decision S; implementation M (one accent token, ~28
call sites, both themes re-measured). **Blocked on it:** A22
DS-1/DS-10/DS-12 (accent unification, theme customisation,
stage↔shell colour bridge). **Not blocked:** PL-1's kbd/muted
fixes, which proceed regardless.

### D-2 — `tab_visits_controls` + the keyboard-operability bundle
**Evidence (A20 §1.4, A21 §1.6):** the flag is unset in AppSpec;
Rune's focus builders have zero uses in `src/`; Tab reaches text
fields only; menus are pointer-only; core posing cannot be
completed keyboard-only. P14 hazard-mapped the staging,
including the Space-on-focused-button conflict with Play.
**The bundle (one recorded decision):** (a) the flag itself +
the Space rule; (b) Settings-card Esc arm (two audits read the
card as outside the overlay rule — if the user rules it an
overlay, the join is one arm); (c) menu-open non-Esc key swallow
(today every binding fires behind an open menu).
**Price:** decision S; then A20-6 kit focus adoption M, A20-7
menus/lists M–L, A20-8 stage keyboard posing L (own brief).
Menu keyboard navigation (A21 K13, M) needs no ruling and is in
PL-4's family regardless.

### D-3 — The two "New project" results
**Evidence (A13):** Home's New installs `Project::new_demo()`
(5-frame waving figure); Project tools'/MCP's New installs
`blank_project` (single rest frame). QA_RUNE §16 records it as
an open user call; it still stands. **Options:** demo everywhere,
blank everywhere, or demo-on-first-run/blank-after (rides PL-7's
`onboarded` flag). **Price:** decision S; unification M (A13 R3,
including the creation setup sheet it enables). **Blocked:** the
"New project" half of PL-7's samples story.

### D-4 — Bundle-cancel class-C ruling
**Evidence (A12, A15/A24, Wave AA seat AA2):** there is no
UI→worker signal path at all today; the prerequisites are a core
stop signal plus a ruling on what a *cancelled* recap says —
class-C sentences are machine-read, so the cancelled shape must
be ruled, not improvised. A24 R5 proposes the shape for decision:
a distinct terminal recap ("Export cancelled — {done} of
{total}…"; bundles gain a `; cancelled:` arm mirroring
`; failed:`), detectors updated in the same change.
**Price:** decision S; build M–L (slot reserved at P-13).
**Blocked:** cancellation for every long export and bundle;
meanwhile the only escape from a mistaken 4,000-frame job is
killing the app.

### D-5 — Chain-spanning media, questions Q1–Q9
**Evidence:** Wave Z seat Z7's design (span_next + 9 questions)
awaits the user's answers; it has stood since Wave Z closed.
**Price:** the answers are the price; the build is unpriced
until Q1–Q9 fix the semantics. **Blocked:** any media spanning
behaviour across segment chains.

### D-6 — IA-AA9-1: set-rest clamp through joint limits
**Evidence (A3):** `set_rest_from_pose` ignores joint limits
while setup drags and steppers clamp — the same operation obeys
two rules depending on the door. AA9 priced the one-site fix and
recorded the drift as a user ruling. **Options:** clamp (one
rule everywhere; rest poses near limits change behaviour) or
document the exception as intended. **Price:** decision S; fix
S (one site). **Blocked:** nothing else — which is why it should
not still be open.

### D-7 — Panel alpha toggle
**Evidence:** core + MCP support alpha exports (PNG-alpha
sequence, APNG-alpha); the panel has none (`grep alpha` in the
export panel: nothing); flagged as a pending user decision in
Waves X and Z and deliberately not built. **Price:** decision S;
control S–M once ruled. **Related but separate:** D-8.

### D-8 — Panel export-scale control
**Evidence (A12, A17):** export scale is a true re-render in
core + MCP; the panel's own note admits "output size IS
project.stage". A17 records it as a surface gap under a standing
decision, not an oversight. **Price:** decision S; control S–M.
**Not this decision:** PL-8's export *memory* (remembering values
chosen through existing controls), which proceeds independently.

### D-9 — Subtitle stack cap / wrap behaviour
**Evidence (A9 G2):** overlapping cues are legal and stack on
stage/burn-in with no cap and no wrap guidance; storage and
layout maths would support either behaviour. A9 explicitly does
not re-propose it — it is the user's authoring-model call.
**Price:** decision S; implementation S–M either way.
**Not blocked:** P-11's CPS/line-length *feedback* (information,
not enforcement).

### D-10 — Figure-library states format brief
**Evidence (A3, A14; Wave Z seat Z9):** the library stores
FigureDef only; figure states/clips do not travel with a library
figure — a format boundary, not a bug. Carrying them needs a
user brief defining the format contract. **Price:** the brief;
build unpriced until scoped. **Blocked:** portable characters
with their state sets — which E-2/E-4 will want.

### D-11 — Joint-role metadata
**Evidence (A3; Wave Z seat Z10):** Z10's limit-sweep audit was
negative — node names cannot drive whole-figure operations;
joint-role metadata (which node is a hand, a foot, a head) is
the honest precondition for role-aware tooling (and would serve
E-5's visemes and A3's role presets). Needs a user call on
whether roles enter the figure format. **Price:** decision S;
metadata + migration M if approved.

### Secondary decisions (priced briefly, same rule)
- **Panel splitter (drag resizing)** and **File-menu Save As** —
  standing user decisions from the UI mission; A30 T3 carries
  them. Splitter: M once ruled.
- **GIF import at the asset gate** — explicitly refused today;
  A14 records it as an undecided user question. S either way.
- **Context scoping of global bindings** (A21 K5): per-binding
  ruling on intended scope; S–M to apply.
- **Windows code-signing certificate / Apple Developer account**
  (A27 R5/R6): owner procurements with cost and lead time; the
  honest interim is unsigned-with-instructions, stated as such.
  Raised early because PL-14 cannot finish without the answer.
- **`.spla`/`.spriteloop` packages** (A28 R9 vs A14's standing
  RE verdict): the roadmap follows "do not build"; reversing it
  is a user call.
- **Community-library moderation policy** (E-2): a human policy
  decision that precedes any public catalogue.
- **`status_ready` wire-or-remove** (IA-AA16-1 option C stands:
  reserved, never consumed): PL-9/PL-11's status localisation is
  the natural moment to revisit — explicitly, not by drift.

---

## Standing rules carried across all phases

CI-only verification (no local cargo builds without the user's
explicit approval for that run); every CI round's verdict
reported the moment it is checked; no fake controls — everything
shown calls a real engine op; element keys zero lost; i18n
parity ×28 for any new key; class-C sentences frozen;
serialised vocabularies are data; determinism of the default
export stream is protected (quality lands as new modes);
characters remain recognisable stickmen; final full-resolution
renders for the channel remain on the user's own PC.

*End of roadmap. Findings: `~/workspace/uiux-analysis/findings/a1.md`–`a30.md`.
In-flight context: Wave AC board `~/workspace/wave-ac/BOARD.md`.*
