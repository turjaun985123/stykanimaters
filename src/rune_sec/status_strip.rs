//! Status strip for the Rune editor shell (Phase D, section
//! 27): a segmented, separator-divided replacement for the
//! one-line `chrome_view::status_text`. Pure builder — it
//! reads `EditorCore` and returns an element tree; it
//! attaches no handlers and mutates nothing.
//!
//! Every value shown is read live from the core:
//!
//! - project name — `core.project.name`
//!   (`model.rs:682`), with the project's total
//!   runtime appended (` · {runtime}`, Phase I — see
//!   [`runtime_label`]) and a ` ●` suffix while the
//!   project has edits the autosave has not captured
//!   (see [`project_text`])
//! - frame position — `core.current` + 1 over
//!   `core.frame_count()` (`editor.rs:33`, `editor.rs:192`)
//! - timecode — current frame at `core.project.fps`
//!   (`model.rs:683`), via the pure [`timecode`] fn,
//!   rounded exactly as the playback panel's readout
//!   (`rune_sec::playback_controls::format_timecode`)
//! - fps — `core.project.fps`, formatted as in chrome
//! - zoom — the EFFECTIVE zoom: `core.camera_here().2`
//!   (`editor.rs:530`), the camera key zoom at the
//!   current frame, multiplied by the shell view zoom
//!   (`crate::stage_view::view_nav().zoom`) — the same
//!   number the stage overlay's readout shows (UI
//!   mission Wave 9; see [`zoom_text`])
//! - selection — `core.selected_figure` /
//!   `core.selected_node` (`editor.rs:34-35`) with the
//!   figure's `FigureInstance::name` (`model.rs:404`)
//! - undo depth — `core.undo_depth()` /
//!   `core.redo_depth()` (`editor.rs:250`, `editor.rs:255`),
//!   the Phase E accessors over the private stacks
//!   (see [`undo_text`])
//! - play state — `core.playing` (`editor.rs:38`)
//! - audio state (Phase G, conditional) —
//!   `core.playback_audio_active()` (`audio_play.rs:211`),
//!   shown only while the playback player is live
//! - loop range (Phase G, conditional) —
//!   `core.loop_range` (`editor.rs:61`), shown only
//!   while a loop is set
//! - stroke / particle-event selection (Phase G,
//!   conditional) — the shell's `State::stroke_selected`
//!   / `State::particle_selected` (`main.rs`), visible
//!   only through [`status_strip_state`]: that state
//!   lives on `State`, not on the core
//! - autosave freshness (Phase J, conditional) — the
//!   recovery snapshot's age, from the mtime of
//!   `./projects/.recovery.json` via
//!   `stykanimate_rs::shell::recovery_modified()`,
//!   rendered as a relative age by [`autosave_segment`]
//!   and visible only through [`status_strip_state`]:
//!   the snapshot is shell support data, not core
//!   state (see the Phase J note below)
//! - playback speed (Phase L, conditional) — the
//!   shell's `State::playback_speed` (`main.rs`),
//!   shown only while it differs from normal speed
//!   (1.0×) and visible only through
//!   [`status_strip_state`]: the speed is shell
//!   state, not core state (see the Phase L note
//!   below)
//! - 3D session (Phase M, conditional) — `3D`
//!   while a fig3 session is active on the core
//!   (`core.fig3`), composed by [`fig3_segment`]
//!   and visible only through
//!   [`status_strip_state`] (see the Phase M note
//!   below)
//! - last status message — `core.status` (`editor.rs:42`),
//!   the trailing segment chrome's line ended with, kept so
//!   Save/Load feedback stays visible
//!
//! Phase E closed the two gaps Phase D had to leave open:
//! the counters behind the dirty marker and the undo
//! readout were private then. The dirty signal is the
//! public `edits_since_autosave()` (`editor.rs:1587`) —
//! the same counter the project section's autosave line
//! reads — and Phase E section 7 added the
//! `undo_depth()` / `redo_depth()` accessors. Both are
//! read live here; nothing is derived from side channels
//! such as sniffing `core.status`.
//!
//! ## Width discipline (Phase F fix)
//!
//! On a real 1440×900 screen the strip's tail painted
//! *over* the neighbouring text. Root cause, verified
//! against the pinned framework source: the old builder
//! put nine plain `text()` segments in a `w_full` row
//! with no sizing at all. Rune text defaults to one
//! unwrapped, untruncated line (`wrap: false`,
//! `truncate: false` — `crates/rune/src/el.rs:649-651`),
//! so its glyphs always paint at full natural width;
//! but taffy's flex items all default to
//! `flex_shrink: 1.0` (taffy `style/mod.rs:563`) with
//! the CSS automatic minimum size — min-content, i.e.
//! roughly the longest word (`compute/flexbox.rs`
//! §4.5). When the centre column is narrower than the
//! segments' total (at 1440 wide the column is ≈ 740 pt
//! and the initial status sentence alone is 67 chars),
//! every box shrinks toward word width while its glyphs
//! keep painting full-width — the tail worst of all,
//! spilling across the dividers and the column edge.
//!
//! The fix gives the row exactly one flexible member:
//!
//! - every fixed segment is floored at a conservative
//!   estimate of its own content width
//!   ([`est_text_width`]), so the shrink pass can never
//!   compress it below what its glyphs paint;
//! - the trailing status segment is the single
//!   absorber: `grow(1)` takes leftover width,
//!   Rune's one-line `.truncate()` elides the painted
//!   line with "…" to the box it actually gets (real
//!   font metrics, refit on resize), and [`fit_hint`]
//!   caps the string itself as a backstop so the line
//!   is bounded even before layout runs.
//!
//! All nine segments remain, in the same order, with
//! the same text sources.
//!
//! ## Phase G conditional segments
//!
//! Phase G adds information without breaking the
//! discipline above: four more *fixed* segments, each
//! present only while its condition holds, appended
//! after the play segment and before the tail — audio
//! state ([`audio_segment`]), loop range
//! ([`loop_segment`]), stroke selection
//! ([`stroke_segment`]) and particle-event selection
//! ([`particle_segment`]), composed by
//! [`extra_segments`]. Each is built by the same
//! [`segment`] helper, so each is floored at its own
//! label's [`est_text_width`]; even the worst case
//! (all four present at their longest plausible
//! labels) adds ≈ 42 characters of floored width, and
//! the tail remains the single flexible absorber, so
//! the no-overlap argument is unchanged. The original
//! nine keep their order and their text sources.
//!
//! The audio and loop segments read core state only,
//! so both builders show them. The two selection
//! segments read shell state the core-only builder
//! cannot see: [`status_strip`] delegates with no
//! selections, and the shell calls
//! [`status_strip_state`], which passes them through.
//!
//! ## Phase H — one glyph, and one refusal
//!
//! Phase H leads the loop segment with a `repeat`
//! icon ([`segment_with_icon`]): 11 pt tall, and
//! because `repeat`'s view box is square (512 × 512)
//! exactly 11 pt wide, plus a 4 pt gap — 15 pt of
//! additive width inside a segment whose floor stays
//! the text-only [`est_text_width`]. The absorption
//! check, stated as the average advance the label may
//! have before glyph + text would outpaint the floor:
//! `(floor − 15) / chars`. For the longest pinned loop
//! label, `Loop 999–1000` (floor 13 × 8.0 = 104 pt),
//! the budget is ≈ 6.85 pt/char (0.57 em). The classes
//! actually in the label run below it — letters at the
//! documented ≈ 0.5 em, digits at ≈ 0.55 em, capitals
//! ≈ 0.67 em for the one `L` — totalling ≈ 84 pt of
//! text, ≈ 99 pt with the glyph, ≈ 5 pt to spare; even
//! at a pessimistic 0.6 em tabular-digit advance the
//! total is ≈ 103 pt, still inside, and overflow
//! would need digits averaging over ≈ 0.61 em, beyond
//! common UI fonts. Longer labels only amortise
//! better: the overhead is fixed while the floor
//! grows per character.
//!
//! The audio segment gets NO glyph, deliberately.
//! `Audio on` is only 8 chars (floor 64 pt), and the
//! honest `volume-high` glyph is wider than its 11 pt
//! height suggests — its view box is 640 wide, so it
//! paints 13.75 pt wide; glyph + gap ≈ 17.75 pt over
//! 8 chars drops the budget to ≈ 5.8 pt/char
//! (0.48 em), below even the documented *typical*
//! glyph advance of 0.5 em (see [`LABEL_CHAR_W`]) —
//! the floor cannot absorb it safely, so the segment
//! stays text-only. The stroke and particle segments
//! stay text-only too: a selection index has no
//! honest glyph. Phase K re-ran this refusal at the
//! Phase J strip width — see the Phase K section
//! below; the refusal stands.
//!
//! ## Phase I — runtime inside the project segment
//!
//! The strip never showed the project's total
//! runtime. Phase I appends it to the project
//! segment's text (`{name} · {runtime}`, see
//! [`project_text`] / [`runtime_label`]) rather than
//! adding a tenth fixed segment: no new divider, no
//! new floor competing with the tail, and the width
//! cost is only the label's own characters. It is
//! free under the width discipline because
//! [`segment`] computes its floor from the final
//! label string via [`est_text_width`] — no width
//! anywhere in this module is hard-coded per segment —
//! so the project segment's floor grows by exactly
//! the runtime label's estimated width (≈ 5 chars,
//! ≈ 40 pt) and the tail remains the single absorber.
//!
//! ## Phase J — autosave freshness segment
//!
//! The strip now also says how fresh the safety net
//! is: while a recovery snapshot exists, a fixed
//! segment after the Phase G block reads `Autosaved
//! just now`, `Autosaved {n}m ago` or `Autosaved
//! {n}h ago` ([`autosave_segment`]). The snapshot is
//! written by `EditorCore::maybe_autosave`
//! (`editor.rs:1929`), which the shell's poll calls
//! every tick (`main.rs:478`); it lands as
//! `./projects/.recovery.json` and its mtime is the
//! last-autosave time.
//!
//! The label is a relative age, not a clock time,
//! deliberately: nothing in the tree converts a
//! `SystemTime` to local HH:MM (the only `SystemTime`
//! maths anywhere is `UNIX_EPOCH` duration arithmetic
//! in `audio.rs`), and a UTC clock reading would
//! mislead. The age is also the more glanceable fact
//! — it answers "how much work would I lose?" — and
//! it needs no timezone at all.
//!
//! The mtime reaches this module through one lib
//! call, `stykanimate_rs::shell::recovery_modified()
//! -> Option<SystemTime>`. (Phase J wrote this
//! module against that accessor's contract before
//! it existed; it has since landed in `shell.rs`,
//! over the same private `default_dir` / `file_mtime`
//! pair the boolean
//! `recovery_is_newer_than_last_project` query the
//! Home panel's recovery offer reads is built on.)
//! The contract is the one stated here: snapshot
//! mtime, `None` when no snapshot exists or cannot
//! be stat'ed — the same error-swallowing as
//! `file_mtime`.
//!
//! Placement and width follow the Phase G pattern:
//! the segment is composed by
//! [`status_strip_state`] (the core-only
//! [`status_strip`] passes `None`, exactly as it
//! does for the selection segments), appended after
//! the particle segment and before the tail, and
//! floored through [`segment`] at its own label's
//! [`est_text_width`] — the longest plausible label,
//! `Autosaved 999h ago`, floors at 18 × 8.0 = 144 pt,
//! and only while a snapshot exists. With no
//! snapshot the segment vanishes and the strip is
//! byte-identical to Phase I's. It is deliberately
//! NOT part of [`extra_segments`]: that composition
//! is pure core-plus-selections, and its emptiness
//! on a fresh core is pinned by test.
//!
//! ## Phase K — the audio glyph, recomputed at 744 pt
//!
//! Phase H refused the audio segment a glyph on
//! width maths from the squeezed layout, where the
//! centre column measured ≈ 461 pt. Phase J fixed
//! the layout — side columns fixed at 300 / 340 pt,
//! the centre takes the rest — and the Phase J
//! headless tree dump measures the strip itself at
//! 744 pt wide (bounds [328, 845, 744, 15] at the
//! 1440-wide window). The refusal was therefore
//! re-run at the new width, charging the glyph's
//! painted width against the strip (the additive
//! accounting, not Phase H's per-segment absorption):
//!
//! - the default state's fixed floors, each
//!   [`est_text_width`] of a fresh core's live label:
//!   project `Untitled · 0:00` 120, frame `Frame 1/5`
//!   72, timecode `0:00.0` 48, fps `12 fps` 48, zoom
//!   `Zoom 100%` 72, selection `Styk` 32, undo
//!   `Undo 0` 48, play `❚❚ Paused` 72 — 512 pt in all;
//! - the audio segment's own floor, `Audio on`: 64 pt;
//! - the `volume-high` glyph at the loop segment's
//!   size (11 pt, [`segment_with_icon`]): its view
//!   box is 640 wide, so it paints 11 × 640 / 512 =
//!   13.75 pt, plus the 4 pt icon/label gap —
//!   17.75 pt charged;
//! - total with the glyph: 512 + 64 + 17.75 =
//!   **593.75 pt**.
//!
//! The tail absorber must keep at least 160 pt of
//! the 744: 593.75 + 160 = 753.75 > 744 — **the
//! glyph still does not fit**, short by 9.75 pt.
//! Without the glyph the same state totals 576 pt
//! and passes (576 + 160 = 736 ≤ 744, 8 pt to
//! spare): the current strip is comfortable at the
//! new width, but the glyph alone tips it over.
//! (This accounting charges segment floors only;
//! the row's dividers and 8 pt gaps, uncharged here,
//! would widen the shortfall further.) Phase H's
//! per-segment budget independently agrees — it is
//! width-independent: (64 − 17.75) / 8 ≈ 5.8 pt/char
//! against a ≈ 6 pt typical advance. So the audio
//! segment stays text-only and nothing in the
//! rendering changes. The arithmetic is pinned by
//! `audio_glyph_width_recomputation_at_744pt` in
//! the tests: any future change to a default label,
//! the icon size or the strip's width fails that
//! test and re-opens the question loudly instead
//! of letting the refusal ride on stale numbers.
//!
//! ## Phase L — playback-speed segment
//!
//! One more conditional segment: while the shell's
//! playback speed (`State::playback_speed`, cycled by
//! the transport's `pb-speed` button) differs from
//! normal, a fixed segment directly after the play
//! segment reads `{x}×` — `0.25×`, `1.5×`, `2×` —
//! composed by [`speed_segment`] and passed into the
//! composer by [`status_strip_state`] exactly as the
//! Phase J autosave label is (the core-only
//! [`status_strip`] passes `None`: the speed lives on
//! `State`, not on the core). It heads the conditional
//! block, ahead of the Phase G segments, because it
//! qualifies the play segment it follows; it is
//! floored through [`segment`] at its own label's
//! [`est_text_width`], like every other fixed segment.
//!
//! The segment is deliberately **excluded from the
//! Phase K floors pinned above** (512 pt of default
//! floors, 576 pt with audio): those numbers describe
//! the *default* state, and at the default speed of
//! 1.0× the segment is absent — [`speed_segment`]
//! returns `None` — so the default strip, its floors
//! and the pinned arithmetic test are byte-identical
//! to Phase K's. At a non-default speed the fixed sum
//! can exceed the old totals by the segment's own
//! floor (at most `2×` / `0.25×`: 2–4 characters,
//! 16–32 pt); that is the designed behaviour, not an
//! overflow — the tail remains the single flexible
//! absorber and truncates, exactly as it does when
//! every Phase G segment is present at once.
//!
//! ## Phase M — 3D-session segment
//!
//! One more conditional segment: while a fig3
//! session is active on the core (`core.fig3`,
//! started by `EditorCore::enter_3d`), a fixed
//! segment directly after the speed segment reads
//! `3D` — composed by [`fig3_segment`] and passed
//! into the composer by [`status_strip_state`]
//! exactly as the Phase L speed label is (the
//! core-only [`status_strip`] passes `None`: this
//! wrapper stays the reduced view, as it does for
//! the speed segment). It sits with the speed
//! segment at the head of the conditional block,
//! ahead of the Phase G segments, because both
//! qualify the shell's current editing state rather
//! than the project; it is floored through
//! [`segment`] at its own label's
//! [`est_text_width`], like every other fixed
//! segment.
//!
//! The segment is **text only, deliberately no
//! glyph**: the icon table does carry a `cube`
//! glyph, and because the segment is absent in the
//! default state it would fit the width maths
//! pinned above — but the strip's conditional
//! state segments are text-only by the Phase H/K
//! audio decision, the loop segment's `repeat` icon
//! being the strip's only icon glyph, and two
//! characters of text name the state without
//! spending the glyph width discipline on it.
//! Consistency with the audio decision wins over
//! the available room.
//!
//! The segment is likewise **excluded from the
//! Phase K floors pinned above**: with no session
//! active — the default state — [`fig3_segment`]
//! returns `None`, so the default strip, its floors
//! and the pinned arithmetic test are byte-identical
//! to Phase K's. While a session is active the
//! fixed sum grows by the segment's own floor (2
//! characters, 16 pt) and the tail truncates — the
//! designed behaviour, as with the speed segment.
//!
//! ## UI mission Wave 3 — small-centre compression
//!
//! The audit (S03 D1) charged this strip ≈ 690 pt
//! of floored content against the 584 pt centre
//! column of the app's own minimum window (1280
//! wide): the Phase F floors are `min_w`, the
//! shrink pass cannot compress a floored segment,
//! so once the tail had absorbed what it could the
//! remaining segments painted past the column's
//! right edge — the Phase J failure mode, returning
//! at exactly the size the app declares its
//! minimum.
//!
//! The fix is a protection ladder, expressed in
//! the only currency Rune's flex model offers —
//! floors, bases, and the one-line truncating mode
//! (`.truncate()`: flex_shrink 1, minimum width 0,
//! glyphs elided to the box at real font metrics,
//! so a yielding segment can narrow without ever
//! painting over a neighbour):
//!
//! - **Protected — never yield.** The frame
//!   segment and the autosave segment keep the
//!   plain [`segment`] path (floored at
//!   [`est_text_width`], no truncation), as do
//!   timecode, selection, undo and every
//!   conditional segment. The project segment's
//!   suffix (` · {runtime}` plus the dirty ` ●`)
//!   is protected the same way, so the strip's
//!   save state — the autosave age, and the dirty
//!   dot — is fully visible at every width.
//! - **First sacrifice — the play segment.** Its
//!   floor is REMOVED: [`segment_yielding`] bases
//!   the text at its old floor value via `.w()`
//!   and truncates it, so the box may narrow all
//!   the way to nothing. It carries the largest
//!   base of the yielding set, so under pressure
//!   it yields the largest share.
//! - **Second — the project name truncates.**
//!   The project segment is now a two-part row
//!   ([`project_segment`], built from
//!   [`project_parts`]): the name is a yielding
//!   truncating text based at its own estimated
//!   width; the suffix keeps its floor. The name
//!   elides toward "…" before the runtime or the
//!   dirty dot can be touched.
//! - **Third — the fps segment** yields the same
//!   way ([`segment_yielding`]): the smallest base
//!   of the three, so it yields the smallest share
//!   at every pressure level.
//! - **Fourth — the zoom segment** (UI mission
//!   Wave 10) yields the same way
//!   ([`segment_yielding`]). Its base (72 pt at
//!   `Zoom 100%`) ties the play segment's as the
//!   largest the ladder had left protected, and
//!   the value it carries is an echo: since Wave 9
//!   it is the EFFECTIVE zoom, whose primary
//!   readout is the stage overlay — the overlay
//!   keeps the exact percentage visible at every
//!   width while the strip's copy elides. The
//!   proof that bought this rung — the deepest
//!   coincidence's residual, made exact — is the
//!   Wave 10 section below.
//!
//! The order is a sacrifice gradient, and it is
//! worth stating exactly what that means
//! mechanically. Rune exposes flex_shrink only at
//! 1.0 (the pinned rev has no numeric shrink
//! factor), so once the tail has given up its
//! grown width — a narrowing window consumes the
//! tail first, as it always has; it sits at zero
//! before any fixed segment comes under pressure
//! — a shortage distributes over the yielding
//! bases in proportion to their size:
//! play (72) and zoom (72) ahead of the name (64)
//! ahead of fps (48), in the default state's
//! estimated widths.
//! No segment is removed, no label changes, and
//! the loop segment keeps its glyph: at 1440
//! (744 pt centre) there is no shortage, nothing
//! yields, and the strip's boxes are point-for-
//! point the Phase N measurements (floors 512,
//! boundaries 136, tail 96 — pinned by
//! `strip_compression_arithmetic_at_584pt`).
//!
//! The arithmetic, in this module's estimated-
//! width accounting ([`LABEL_CHAR_W`] = 8.0
//! pt/char), with boundaries charged at the
//! MEASURED 17 pt each (8 pt row gap + the 1 pt
//! divider + 8 pt gap, read off the Phase N tree
//! dump, where the default strip's segment boxes
//! are exactly the floors below and the tail
//! measures 96):
//!
//! - default state: fixed floors 512 (the Phase K
//!   pin). The yielding bases inside that sum —
//!   play 72, project name 64 (`Untitled`), fps
//!   48 and, since Wave 10, zoom 72 — total 256
//!   of capacity, leaving 256 of protected
//!   floors.
//! - at 744: 512 + 8 boundaries × 17 = 648 ≤ 744;
//!   the tail takes the remaining 96. Nothing
//!   yields.
//! - at 584, default state: demand 648, shortage
//!   64 — inside the 256 of yield capacity, and
//!   the protected floors plus boundaries total
//!   256 + 136 = 392 ≤ 584, so frame, autosave
//!   and the project suffix are whole at the
//!   minimum size.
//! - at 584 with the audit's C1 coincidence live
//!   (audio 64 + speed `1.5×` 32 + 3D 16):
//!   floors 624; the protected part is 368 since
//!   Wave 10 (440 before it). The
//!   boundary-charged proof is the Wave 10
//!   section below.
//!
//! One honest remainder, in the Phase K tradition
//! of charging what an accounting leaves out: the
//! C1 coincidence at 584 also carries 11
//! boundaries × 17 = 187 of gap/divider overhead,
//! against which the 184 of estimated yield
//! capacity then in the ladder left a residual of
//! ≈ 43 pt in measured geometry — the failure was
//! pushed from the default state (which overflowed
//! before this change) to the deepest coincidence
//! at the minimum size, not abolished. Wave 10
//! closed exactly that residual, with the fourth
//! yielding segment this paragraph named as one of
//! the two prices; the boundary-charged proof —
//! the estimate made exact, its dirty-session
//! variant, and the wall that remains beyond the
//! bound — is the Wave 10 section below.
//!
//! ## UI mission Wave 10 — the deepest coincidence, proven
//!
//! Wave 3 left the strip's last width question as
//! an estimate: the deepest coincidence (audio
//! live + speed 1.5× + a 3D session, over the
//! default state) at the 584 pt minimum centre,
//! residual ≈ 43 pt. The ≈ had three sources: the
//! boundary overhead was charged from a tree-dump
//! measurement, the coincidence's labels were
//! charged at default-state values without an
//! audit of what each segment can actually say,
//! and the audio leg's presence depends on live
//! ffplay liveness, which no headless probe can
//! produce. This section removes the first two
//! and states the third exactly. The arithmetic
//! is now a proof inside the strip's own
//! accounting ([`est_text_width`] throughout),
//! conditional — as every conditional segment's
//! accounting in this module is — on the
//! coincidence holding; it is pinned by
//! `deepest_coincidence_proven_bound_at_584pt`.
//!
//! The boundary charge first, because it is now
//! derived, not just measured: [`build_strip`]
//! joins N elements with N−1 dividers (1 pt,
//! floored) and a uniform 8 pt row gap on both
//! sides of each, so the overhead is (N−1) × 17
//! by construction. The default state's 8 × 17 =
//! 136 is exactly what the Phase N dump measured
//! (512 + 136 + tail 96 = 744), which is the
//! formula's check against reality.
//!
//! Every segment's worst case, from its label fn,
//! in the universe the earlier sections already
//! charge — projects of at most 1000 frames,
//! durations inside the editor's 0.25..=8.0 clamp,
//! fps inside its 1.0..=60.0 clamp, counters at
//! the 999 plausible cap Phase G/J use:
//!
//! - project suffix ` · 133:20 ●` — 11 chars,
//!   88 pt (the universe's runtime edge: 1000
//!   frames × 8.0 at 1 fps = 8000 s); protected.
//!   The name itself is rung 2 — user text,
//!   unbounded, and width-neutral to every bound
//!   below: it can always elide to nothing, so a
//!   longer name only enlarges the yield pool.
//! - frame `Frame 1000/1000` — 15 chars, 120 pt;
//!   protected, never yields.
//! - timecode `16:39.0` — 7 chars, 56 pt (frame
//!   999 at the 1 fps clamp floor); protected.
//! - fps `59.9 fps` — 8 chars, 64 pt of base;
//!   rung 3, pool side only.
//! - zoom `Zoom 3200%` — 10 chars, 80 pt of base
//!   (the [`zoom_text`] corner: camera 4.0 × view
//!   8.0); rung 4, as of this section.
//! - selection `{figure} › node {n}` — no finite
//!   worst case: a figure's name is user text on
//!   a protected segment (see the wall below).
//! - undo `Undo 999 · Redo 999` — 19 chars,
//!   152 pt (the stacks are uncapped in the
//!   editor; 999 is this module's plausible cap);
//!   protected.
//! - play `▶ Playing` — 9 chars, 72 pt of base;
//!   rung 1.
//! - speed — the cycle's widest label is its
//!   SLOWEST step, `0.25×` (5 chars, 40 pt); the
//!   coincidence pins 1.5× (32 pt), and the
//!   fastest step `2×` is the narrowest (16 pt).
//! - fig3 `3D` — 2 chars, 16 pt, a constant.
//! - audio `Audio on` — 8 chars, 64 pt, and a
//!   CONSTANT: [`audio_segment`]'s text is a
//!   literal, so the segment's width was never
//!   the unprovable part of this coincidence —
//!   only its presence is, and presence is the
//!   coincidence's premise, not its arithmetic.
//! - loop `Loop 999–1000` — 13 chars, 104 pt of
//!   floor, the glyph inside it (Phase H).
//! - stroke `Stroke 1000` — 11 chars, 88 pt;
//!   particle `Particle 1000` — 13 chars, 104 pt.
//! - autosave `Autosaved 999h ago` — 18 chars,
//!   144 pt (Phase J's cap).
//!
//! The coincidence, charged in full: the eight
//! always-present segments at their default-state
//! labels (512, the Phase K pin), the three legs
//! at 64 + 32 + 16 = 112 — floors and bases 624
//! in all — the tail at its minimum of zero, and
//! all 11 boundaries at 187. Before this change
//! the protected floors were 440 (328 default +
//! 112), so the row's minimum width was 440 +
//! 187 = 627 against the 584 column: **an
//! overflow of exactly 43 pt** — the Wave 3
//! residual with its ≈ removed; every term is a
//! pinned label width or a derived constant of
//! the row's own construction. In a real editing
//! session the coincidence's project is dirty, so
//! the suffix is ` · 0:00 ●` (72, not 56): 643
//! against 584, an overflow of 59. The failure
//! sat in the protected set alone — the yield
//! pool and the tail are already at zero in both
//! variants; nothing flexible was left to take
//! it. (The brief's fourth leg, the longest
//! project name, moves neither number: the name
//! is rung 2, pure pool.)
//!
//! The fourth rung is the zoom segment, on the
//! ladder's own gradient: its 72 pt base ties the
//! play segment's as the largest left protected;
//! its value is an echo whose primary readout —
//! the stage overlay's effective-zoom percentage
//! — stays fully visible at every width; and it
//! degrades the way rungs 1 and 3 do, by
//! right-elision, never by painting over a
//! neighbour and never by stating a wrong number
//! (the ellipsis is part of the truncated
//! rendering). No other single rung closes both
//! variants: the frame readout (72) never yields,
//! the suffix (56/72) is the save state, and the
//! selection (32 at `Styk`) and undo (48 at
//! `Undo 0`) segments each free less than the
//! dirty variant's 59.
//!
//! After the rung, the coincidence's protected
//! floors are 368 and its minimum width 368 +
//! 187 = 555 ≤ 584 — **it fits the minimum
//! window with 29 pt to spare**; the dirty
//! variant fits at 571 (spare 13), and the bound
//! is robust across the whole speed cycle:
//! swapping 1.5× for the widest label (0.25×, +8)
//! leaves the dirty variant at 579 (spare 5). In
//! the default state the protected floors are now
//! 256 and the yield pool 256; at 744 nothing
//! yields in either state (in the coincidence the
//! 67 pt shortage there lands on the tail and the
//! yielding bases, every protected segment
//! whole), so the Phase N measurements stand.
//! The price, stated plainly: at 584 in the
//! default state the zoom segment now shares the
//! 64 pt shortage with rungs 1–3 — ∝ its base its
//! paper share is ≈ 18 pt, narrowing its box to
//! ≈ 54 pt, right where its label's painted width
//! sits at the documented typical advance (≈ 0.5
//! em), so its final glyphs may elide at the
//! minimum window. The exact percentage remains
//! on the stage overlay at all times.
//!
//! The wall that remains, so the next change does
//! not have to rediscover it: the bound's
//! premises are the coincidence above, and every
//! further charge lands pt-for-pt on the dirty
//! spare of 13 (5 at the widest speed), plus 17
//! for each segment that joins the row. The
//! autosave segment alone at its longest label
//! (144 + 17) exceeds it; so do the table's worst
//! cases arriving together — frame +48 over the
//! charged 72, undo +104, the zoom corner +8.
//! And the selection segment's floor is sized by
//! a figure's name: user text on a protected
//! segment, the species rung 2 exists to absorb,
//! so a long-named selected figure at the minimum
//! window can still push the row past its column.
//! It is the principled candidate if a fifth rung
//! is ever briefed. The audio leg's presence
//! remains ffplay's to give — the bound is proven
//! for the coincidence when it holds, and the
//! coincidence itself is still only producible
//! live.
//!
//! ## UI mission Wave 8 — the strip takes the app theme
//!
//! The composer built its own theme —
//! `tok::theme(true)`, upstream's DARK theme,
//! unconditionally — so when the shell switched to
//! the light appearance the strip kept painting
//! upstream's dark colours: segment text at
//! `oklch(0.955, …)` (≈ `#EFF0F2`) on the light
//! `#EDF0F5` background, near-invisible (the Wave 7
//! headless dumps show every strip text element at
//! `#eff0f2` in the light tree, byte-identical to
//! the dark tree). The strip was the last shell
//! surface still constructing a private theme.
//!
//! The fix threads the shell's theme in: both
//! wrappers resolve
//! `app_theme::app_theme(!core.light_theme)` — the
//! exact construction `main.rs::view` uses for the
//! whole view — and [`build_strip`] takes it as a
//! parameter. The role mapping is unchanged, now
//! resolved from that theme and pinned at one seam
//! ([`strip_colors`]): fixed segments and the loop
//! glyph in `text`, the status tail in
//! `text_secondary`, dividers in `separator`, and
//! the rolling play segment in the brief accent
//! ([`app_theme::accent`], the theme-aware successor
//! to the hard-coded `tok::accent(0, true)`).
//!
//! In the dark appearance the strip therefore
//! renders the brief palette the rest of the shell
//! already uses — text `#F1F3F7`, secondary
//! `#A1A9B7`, separator `#303846` @ α 0.55, accent
//! `#6C8CFF` — in place of upstream's near-identical
//! `#EFF0F2` / `#A2A5AA` / white @ α 0.055: the same
//! roles, now from the single palette, so strip and
//! shell can never drift apart again. In light the
//! strip takes the light roles (`#171A21` /
//! `#5B6577` / tinted hairline / `#4666D8`) and is
//! legible on the light background.
//!
//! ## Wave 11 — the counters under live MCP control
//!
//! Every value above is read live from the core at
//! build time — this module caches nothing, derives
//! nothing from side channels, and holds no per-frame
//! state (the module docs' first section is the
//! inventory). That is exactly what live MCP control
//! needs: `McpServer::handle_on` mutates the SAME
//! `EditorCore` the strip reads, through the same ops
//! the GUI calls, so a tool's `snapshot()` lands on
//! the same undo stack and edit counter the undo
//! segment and the dirty dot read. The strip cannot
//! say `Undo 0` after an MCP edit because there is
//! no second counter for it to read instead — the
//! claim is pinned end to end by
//! `strip_counters_follow_a_live_mcp_edit`, which
//! drives a real `tools/call` through `handle_on`
//! and reads the segments back. Wave 11 touches no
//! segment, floor or yielding rung here: the Wave 10
//! yield ladder and its 555 pt proven bound stand
//! exactly as derived.
//!
//! ## Wave 12 — the strip audited against its sources
//!
//! Wave 12 re-read every segment against the field
//! it renders, with Wave 11's rename/save-copy tools
//! and Wave 12's `project_files` refresh (B3) in
//! view. Three verdicts, all about the SOURCES, not
//! the rendering — this module still caches nothing
//! and reads every value live:
//!
//! - **The project name is the session's, and that
//!   is the truthful choice.** The segment reads
//!   `core.project.name`, never a file name, and
//!   this module never reads `core.project_files`
//!   at all — so B3's refresh of that list cannot
//!   change anything the strip shows. Wave 11's
//!   `project_rename` handler sets the session name
//!   in the same call that moves the file
//!   (`tool_project_rename`, mcp_tools.rs), so the
//!   strip follows an MCP rename immediately
//!   (pinned by
//!   `strip_project_name_follows_a_live_mcp_rename`).
//!   The divergence to know about is on the other
//!   surface: `projects::rename_project` moves the
//!   file only — the name stored INSIDE the file is
//!   unchanged until the next save (its own
//!   documented contract), and `projects::scan`
//!   lists library projects by that stored name, so
//!   Home can list the old name for the project the
//!   strip already calls by the new one. Both
//!   surfaces are faithful to their sources; the
//!   stored name's staleness is `projects.rs`'s
//!   contract to change, not this module's.
//!   `save_project_copy` is the mirror case: the
//!   session keeps editing the original, so the
//!   strip correctly keeps the original's name.
//! - **The dirty dot counts snapshots, and some
//!   real edits never snapshot.** `edit_count` —
//!   the counter behind both the dot and the undo
//!   segment — increments in exactly one place,
//!   `EditorCore::snapshot()` (editor.rs). Every
//!   snapshotting MCP tool therefore moves the dot
//!   (the Wave 11 pin above), but the sanctioned
//!   non-snapshotting writes of serialised project
//!   data move neither the counter nor the autosave
//!   cadence: `figure_rename` and `figure_set_color`
//!   (figure_ops.rs), the `layers` tool's locked /
//!   hidden / opacity writes (mcp_tools.rs),
//!   `clip_set_angle` / `clip_set_root` (editor.rs;
//!   `exit_clip` does not snapshot either), and the
//!   MCP `project_rename`'s session-name write —
//!   where the panel's own Set-name snapshots first
//!   (Wave 10), an MCP/GUI asymmetry worth the
//!   handler owner's eye. These are panel-parity
//!   behaviours, identical from the GUI, so the
//!   strip under-reports exactly the edits the
//!   counter itself cannot see, from either driver;
//!   the fix belongs to those handlers and ops
//!   (the built-ins' Wave 11 self-snapshot pattern
//!   in mcp.rs is the template), not to this module,
//!   which has no second signal to render instead.
//! - **A manual recovery write leaves the dot
//!   unstamped.** The GUI's Save
//!   (`save_named_pressed`, project_tools.rs) and
//!   Home's open (`toggle_open`, home_panel.rs)
//!   write the recovery snapshot directly, bypassing
//!   `maybe_autosave`, so `last_autosave_count` is
//!   not stamped: right after a save the autosave
//!   segment can read `Autosaved just now` (the
//!   mtime is fresh — true) while the dot still
//!   shows (the counter is unstamped — also true,
//!   of the counter). The project panel's autosave
//!   line carries the same overstatement, documented
//!   at its source (project_tools.rs). The repair
//!   is a stamp the core's owners expose for manual
//!   recovery writes; until then both of the strip's
//!   save-state signals stay exactly as sourced.
//!
//! The autosave segment itself is untouched by
//! B3: its source is the recovery file's mtime via
//! `shell::recovery_modified()`, independent of the
//! `project_files` list B3 refreshes, and
//! `maybe_autosave` still runs on the shell's poll
//! tick against the same live core the MCP queue
//! drains into — so MCP edits meet the autosave on
//! exactly the GUI's terms. No segment, floor or
//! yielding rung changes in this wave either.
//!
//! ## Wave 13 — the read path against the dirty-marking contract
//!
//! Squad B's contract this wave: every project
//! mutation either takes exactly one `snapshot()`
//! (one undo step) or calls the new
//! `EditorCore::mark_dirty()` seam — dirty and
//! autosave counting WITHOUT an undo step, for the
//! writes the Wave 12 section above caught the
//! counter missing (figure rename / recolour, the
//! layer writes, clip angle / root, the MCP
//! `project_rename` name write). This module's read
//! path was re-verified end to end against that
//! contract; the verdict is that it needed no
//! rendering change, and the reasons are now pinned
//! by test instead of resting on the module docs'
//! say-so:
//!
//! - **One counter, read live.** The dot is
//!   `project_parts`' read of
//!   `core.edits_since_autosave()` —
//!   `edit_count − last_autosave_count` in the core —
//!   taken fresh on every build. `snapshot()` bumps
//!   `edit_count` at its single increment site
//!   (editor.rs), and the seam's contract is to bump
//!   THE SAME field (B2 places `mark_dirty` beside
//!   `edits_since_autosave` / `note_recovery_written`
//!   for exactly that reason), so a marked edit and
//!   a snapshotted edit are indistinguishable to
//!   the dot — by design. The undo segment is a
//!   DIFFERENT source on purpose (the stacks, via
//!   `undo_depth()` / `redo_depth()`), so after a
//!   marked write the strip honestly shows the dot
//!   beside `Undo 0`: dirty, with nothing to undo.
//!   That coexistence is pinned by
//!   `strip_dot_lights_for_a_mark_dirty_write_with_no_undo_step`,
//!   which drives the seam through a landed B-squad
//!   op (`EditorCore::set_figure_color`, figure_ops)
//!   and asserts only through this module's fns and
//!   the counter accessor — at this audit the seam's
//!   call sites were in the tree ahead of the
//!   `editor.rs` definition, so no test here names
//!   `mark_dirty` itself; if the seam ever bumped a
//!   different counter, that pin fails loudly.
//! - **The dot is the counter, not the undo stack.**
//!   `note_recovery_written()` (the Wave 12 E8 stamp
//!   for recovery writes made outside
//!   `maybe_autosave`) sets `last_autosave_count =
//!   edit_count`, so the accessor returns 0 and the
//!   dot clears while the undo stack still holds
//!   its steps — pinned by
//!   `strip_dot_follows_the_counter_not_the_undo_stack`.
//!   A strip that sniffed the stack for dirtiness
//!   would stay lit after every stamped save; this
//!   one cannot.
//! - **Nothing here can go stale across a swap.**
//!   This module holds no state at all — no field,
//!   no static, no cache; both wrappers are pure
//!   builders over the core (and, for
//!   [`status_strip_state`], the shell's `State`)
//!   handed to them, and `main.rs`'s view rebuilds
//!   the strip from the live core on every render.
//!   A project swap replaces `core.project` and
//!   calls `note_project_swapped()`, which zeroes
//!   BOTH counters; the next build therefore reads
//!   the new project's name, runtime and a zeroed
//!   counter — there is no segment value a swap
//!   could strand. Pinned end to end, through the
//!   GUI's own New path, by
//!   `strip_shows_no_stale_dirty_state_across_a_project_swap`.
//! - **The MCP rename pin moved — deliberately.**
//!   Wave 12's `strip_project_name_follows_a_live_mcp_rename`
//!   asserted the renamed project's text WITHOUT the
//!   dot, because the rename's session-name write
//!   then counted nothing. Under the census that
//!   write marks dirty at its call site (B5's
//!   verdict, recorded in `projects.rs`:
//!   `refresh_project_files` itself never touches
//!   the counters, deliberately, so the mark is the
//!   handler's alone), so the pin now expects the
//!   dot and `Undo 0` — the strip showing exactly
//!   what the census promises, one counted edit and
//!   no undo step.
//! - **The autosave segment's source, examined for
//!   swap staleness.** Its input is the recovery
//!   FILE's mtime (`shell::recovery_modified()`),
//!   computed fresh by [`status_strip_state`] on
//!   every build — again no cache in this module.
//!   The file is the shell's safety net, not the
//!   current project's property: after a swap it
//!   still holds the previous project's snapshot
//!   until the next autosave writes, so the segment
//!   can name that snapshot's age over a fresh
//!   project. That is the file's own semantics —
//!   Home's recovery offer reads the same file the
//!   same way — not a stale read in this module,
//!   and no read-path change is made for it here.
//!
//! No segment, floor or yielding rung changes in
//! this wave either: the fixes all landed in the
//! counter's writers (Squad B), and this module's
//! job was to prove the strip already reads the
//! one counter they all now write.

use std::time::{Duration, SystemTime};

use rune::tok::{self, type_, Theme};
use rune::{el, row, text, Color, El};
use stykanimate_rs::editor::EditorCore;

use crate::rune_sec::app_theme;
use crate::rune_sec::i18n_wire;
use crate::State;

/// Timecode of a frame at a project frame rate, formatted
/// `m:ss.f` (minutes unpadded, seconds zero-padded, one
/// tenths digit). Frame 0 is time zero. A non-positive or
/// non-finite fps cannot time anything and yields
/// `"0:00.0"`.
///
/// The maths is the playback panel's, ported exactly
/// (`playback_controls::format_timecode`, itself the GPUI
/// timecode chip's): the frame's time in seconds is
/// rounded once, to total tenths — half away from zero —
/// before being split into digits, so the seconds and
/// tenths digits can never disagree and a value a hair
/// under a minute (59.97 s) reads `1:00.0`, never
/// `0:60.0`. Phase D floored here instead, which made
/// the strip disagree with the playback readout by a
/// tenth on boundary frames (frame 7 at 12 fps is
/// 0.583 s: floor `0:00.5`, rounded `0:00.6`); Phase E
/// unifies on the playback rounding.
pub fn timecode(frame: usize, fps: f32) -> String {
    if !(fps > 0.0) || !fps.is_finite() {
        return "0:00.0".to_string();
    }
    let tenths = (frame as f32 / fps * 10.0).round() as u64;
    format!(
        "{}:{:02}.{}",
        tenths / 600,
        (tenths % 600) / 10,
        tenths % 10
    )
}

/// Total-runtime label (Phase I): the project's whole
/// playback length formatted `m:ss` — minutes
/// unpadded, seconds zero-padded, so seven seconds
/// reads `0:07` and minutes keep counting past the
/// hour (`65:00`), the same convention as
/// [`timecode`].
///
/// The length itself is the engine's own total,
/// `Project::total_seconds()` (`model.rs:855`): the
/// sum of every frame's `duration` divided by the
/// project fps. A frame's `duration` is a multiplier
/// in frame periods — `adjust_frame_duration`
/// (`editor.rs:1728`) scales it and formats it as
/// `{:.2}x`, clamped to 0.25..=8.0 — so a frame with
/// duration 2.0 at 12 fps lasts 2/12 s, and
/// `total_seconds` is exactly Σ duration / fps (with
/// fps floored at 1.0 inside the model fn). The total
/// is rounded once, half away from zero, before the
/// split into minutes and seconds — the strip's
/// established rounding (see [`timecode`]) — so a
/// 2.5 s project reads `0:03`. An empty project sums
/// to zero and reads `0:00`; a non-finite total (only
/// reachable from a hand-edited project file) also
/// reads `0:00` rather than painting a nonsense
/// number.
pub fn runtime_label(core: &EditorCore) -> String {
    let secs = core.project.total_seconds();
    if !secs.is_finite() || secs <= 0.0 {
        return "0:00".to_string();
    }
    let total = secs.round() as u64;
    format!("{}:{:02}", total / 60, total % 60)
}

/// Project segment: the name, the total runtime
/// appended as ` · {runtime}` (Phase I, see
/// [`runtime_label`]), plus a ` ●` suffix while the
/// project is dirty. The dirty signal is the core's
/// public edit counter, `edits_since_autosave()`
/// (`editor.rs:1914`) — edits counted since the core's
/// last autosave stamp (the autosave writes a recovery
/// snapshot every so many edits and re-stamps; the
/// project section's autosave line reads the same
/// counter). Dirty is exactly `edits_since_autosave() >
/// 0`; no other channel (status text, file mtimes) is
/// consulted. The counter counts `snapshot()` calls
/// and — since Wave 13's dirty-marking contract —
/// `mark_dirty()` calls: the two bump the same
/// `edit_count` in the core, so a non-undoable write
/// the census assigned to the seam is exactly as
/// visible here as a snapshotted edit, while
/// [`undo_text`] keeps reading the stacks alone (the
/// module docs' Wave 13 section has the verification;
/// the Wave 12 section records what the counter could
/// not see before the seam existed). This fn renders
/// the counter faithfully either way, and the name is
/// the session's (`core.project.name`), never the
/// file's.
///
/// The runtime rides inside this segment instead of
/// becoming a new one: [`segment`] floors the segment
/// at [`est_text_width`] of the whole label, so the
/// floor widens by the label's own width and the
/// strip gains no new fixed segment.
///
/// UI mission Wave 3: the segment is no longer
/// built from this whole string but from its two
/// halves ([`project_parts`], via
/// [`project_segment`]) — this fn remains the
/// single source of the segment's text, and the
/// parts concatenate to exactly this string.
pub fn project_text(core: &EditorCore) -> String {
    let (name, suffix) = project_parts(core);
    format!("{name}{suffix}")
}

/// The project segment's two parts (UI mission
/// Wave 3): the project name, and everything after
/// it — ` · {runtime}` plus the dirty ` ●` while
/// the project has edits the autosave has not
/// captured. [`project_text`] is exactly the two
/// concatenated. The split exists for the segment
/// builder ([`project_segment`]): under width
/// pressure the NAME is the part that truncates
/// (second in the compression order — see the
/// module docs), while the suffix, carrying the
/// runtime and the strip's save-state dot, keeps
/// its floor and stays fully visible at every
/// width.
pub fn project_parts(core: &EditorCore) -> (String, String) {
    let mut suffix = format!(" · {}", runtime_label(core));
    if core.edits_since_autosave() > 0 {
        suffix.push_str(" ●");
    }
    (core.project.name.clone(), suffix)
}

/// Undo-depth segment: `Undo {n}` from
/// `core.undo_depth()` (`editor.rs:250`). When the redo
/// stack is non-empty the redo depth is appended to the
/// same segment — `Undo {n} · Redo {m}` — rather than
/// shown as its own segment: redo is only ever non-empty
/// right after undos, and a separate segment appearing
/// and disappearing would shift every later segment in
/// the strip sideways each time.
pub fn undo_text(core: &EditorCore) -> String {
    // Wave P (P48): the two label words resolve through
    // the i18n table (keys `undo` / `redo`, genuinely
    // translated in all 28 languages) in the core's
    // current language — the shell's `i18n_wire::tr`
    // idiom. The English values are the exact literals
    // this segment carried before the wiring, so the
    // default-language text is byte-identical; the
    // depth numbers are data and stay untranslated.
    let redo = core.redo_depth();
    if redo > 0 {
        format!(
            "{} {} · {} {}",
            i18n_wire::tr(core, "undo"),
            core.undo_depth(),
            i18n_wire::tr(core, "redo"),
            redo
        )
    } else {
        format!("{} {}", i18n_wire::tr(core, "undo"), core.undo_depth())
    }
}

/// Fps readout, formatted exactly as chrome's status line
/// does it: whole rates without a fraction (`"12"`),
/// fractional rates with one decimal (`"30.0"` for 29.97).
pub fn fps_text(fps: f32) -> String {
    if fps.fract() == 0.0 {
        format!("{}", fps as i64)
    } else {
        format!("{fps:.1}")
    }
}

/// Frame position segment: `Frame {current + 1}/{total}`
/// (1-based, the convention chrome's line and the timeline
/// chips both use).
pub fn frame_text(core: &EditorCore) -> String {
    // Wave P (P48): the label word resolves through the
    // i18n table (key `frame`, genuinely translated in
    // all 28 languages) in the core's current language;
    // the English value is the literal this segment
    // carried before the wiring, so the default text is
    // byte-identical. The positions are data.
    format!(
        "{} {}/{}",
        i18n_wire::tr(core, "frame"),
        core.current + 1,
        core.frame_count()
    )
}

/// Zoom segment: `Zoom {pct}%` for the EFFECTIVE zoom —
/// the camera key zoom at the current frame multiplied
/// by the shell view zoom, the number that matches what
/// the stage actually shows. UI mission Wave 5 made the
/// wheel a view zoom and the stage overlay's readout
/// effective (`camera_panel::view_zoom_pct`) but left
/// this segment reading the authored camera alone, so
/// the two displays disagreed whenever the view was
/// zoomed; Wave 9 closes that: the expression below is
/// the overlay's own, term for term and in the same
/// order, so the percentages agree bit for bit (pinned
/// by `zoom_text_agrees_with_the_overlay_readout`).
/// At the identity view the multiplier is exactly 1.0
/// and the segment reads the authored camera zoom,
/// unchanged from before.
///
/// Width: the label keeps its old width class —
/// `Zoom {pct}%` is at most 9 characters (est. 72 pt,
/// the Phase K pinned width) for every percentage up
/// to 999, which covers every view zoom (≤ 8×) at the
/// default camera and the camera steppers' range
/// (0.3–4.0, `editor.rs`) up to a 2.49× view. Only the
/// extreme corner — camera 4.0 × view 8.0 = 3200% —
/// grows it one character (`Zoom 3200%`, est. 80 pt).
/// UI mission Wave 10: the segment is built by
/// [`segment_yielding`], based at its live label's
/// estimated width — under width pressure those
/// points come out of the segment's own box (it
/// elides; the exact percentage stays on the stage
/// overlay), never out of a neighbour, and the
/// 3200% corner funds itself the same way. The
/// Phase K default-state pins do not move: at the
/// default (identity) view the label is still
/// exactly `Zoom 100%`, and at rest the yielding
/// base reserves exactly the width the old floor
/// did. See the module docs' Wave 10 section for
/// the bound this rung closes.
pub fn zoom_text(core: &EditorCore) -> String {
    let pct =
        (core.camera_here().2 * crate::stage_view::view_nav().zoom * 100.0).round() as i64;
    // Wave P (P48): the label word resolves through the
    // i18n table (key `zoom`, genuinely translated in
    // all 28 languages) in the core's current language;
    // the English value is the literal this segment
    // carried before the wiring, so at the default
    // language the label — and the width arithmetic in
    // this fn's docs, which is stated against it — is
    // unchanged. The segment is built by
    // `segment_yielding` from the live label's estimated
    // width (see the Wave 10 note above), so a longer
    // translated word funds its own box the same way.
    format!("{} {pct}%", i18n_wire::tr(core, "zoom"))
}

/// Selection summary. With a node selected:
/// `"{figure} › node {n}"` — the node index as stored,
/// matching the core's own status phrasing
/// (`"figure {} node {}"`, `editor.rs:1149`). With only a
/// figure selected, the figure name alone. With no figure
/// at all (empty project or a stale index), `"No
/// selection"`.
pub fn selection_text(core: &EditorCore) -> String {
    let Some(fig) = core.project.figures.get(core.selected_figure) else {
        return "No selection".to_string();
    };
    match core.selected_node {
        Some(n) => format!("{} › node {}", fig.name, n),
        None => fig.name.clone(),
    }
}

/// Play-state segment: a state glyph plus the word, so the
/// meaning survives even if a glyph falls back in the
/// embedded font. `▶` while playing (media rolling),
/// `❚❚` while paused (bars held).
pub fn play_text(playing: bool) -> &'static str {
    if playing {
        "▶ Playing"
    } else {
        "❚❚ Paused"
    }
}

/// Audio segment (Phase G): `Audio on` while the
/// shell's playback player is live — the caller reads
/// `core.playback_audio_active()` (`audio_play.rs:211`),
/// the ffplay-process liveness the Phase F glue
/// exposes. `None` while inactive: silence is the
/// common case, and a permanent "Audio off" segment
/// would spend floored width on nothing.
pub fn audio_segment(active: bool) -> Option<String> {
    if active {
        Some("Audio on".to_string())
    } else {
        None
    }
}

/// Loop segment (Phase G): `Loop {a}–{b}` for the
/// core's loop range (`core.loop_range`,
/// `editor.rs:61`) — the 0-based inclusive frame
/// indices `set_loop_range` stores, displayed 1-based
/// (`a + 1`, `b + 1`), the same convention as
/// [`frame_text`] and the timeline chips, joined by
/// an en dash. `None` while no loop is set.
pub fn loop_segment(range: Option<(usize, usize)>) -> Option<String> {
    range.map(|(a, b)| format!("Loop {}–{}", a + 1, b + 1))
}

/// Stroke-selection segment (Phase G): `Stroke {i + 1}`
/// while the shell has a stroke selected
/// (`State::stroke_selected`), the index displayed
/// 1-based like every other position in the strip.
/// `None` with no selection.
pub fn stroke_segment(selected: Option<usize>) -> Option<String> {
    selected.map(|i| format!("Stroke {}", i + 1))
}

/// Particle-event-selection segment (Phase G):
/// `Particle {i + 1}` while the shell has a particle
/// event selected (`State::particle_selected`),
/// displayed 1-based. `None` with no selection.
pub fn particle_segment(selected: Option<usize>) -> Option<String> {
    selected.map(|i| format!("Particle {}", i + 1))
}

/// Autosave segment (Phase J): how long ago the
/// recovery snapshot was written, as a relative age —
/// `Autosaved just now` under a minute, `Autosaved
/// {n}m ago` under an hour, `Autosaved {n}h ago`
/// beyond that (whole units, floored: 59 minutes
/// reads `59m`, never `1h`). `modified` is the
/// snapshot's mtime (`shell::recovery_modified()`);
/// `None` — no snapshot on disk — yields `None`, and
/// the segment vanishes. A snapshot stamped in the
/// future (clock skew between the filesystem and
/// the process) clamps to zero age, `just now`:
/// the snapshot is certainly not stale, and a
/// negative age would be nonsense. Pure: both
/// instants are parameters, so the age table is
/// testable without touching the disk or the clock.
/// See the module docs for why the label is an age
/// rather than a clock time.
pub fn autosave_segment(modified: Option<SystemTime>, now: SystemTime) -> Option<String> {
    let modified = modified?;
    let age = now.duration_since(modified).unwrap_or(Duration::ZERO);
    let secs = age.as_secs();
    if secs < 60 {
        Some("Autosaved just now".to_string())
    } else if secs < 3_600 {
        Some(format!("Autosaved {}m ago", secs / 60))
    } else {
        Some(format!("Autosaved {}h ago", secs / 3_600))
    }
}

/// The number inside a speed label, formatted exactly
/// as `playback_controls::speed_label` formats it
/// (Phase K): two decimals, trailing zeroes trimmed,
/// then a bare trailing point trimmed — `1.5`, `0.25`,
/// `2`. That formatter is private to `speed_label`'s
/// body, so the rule is replicated here; the two must
/// not drift — `speed_segment_numbers_match_speed_label`
/// in the tests pins this helper's output against
/// `speed_label`'s for the whole speed table.
fn speed_factor(speed: f32) -> String {
    let digits = format!("{speed:.2}");
    digits.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// Playback-speed segment (Phase L): `{x}×` while the
/// shell's playback speed (`State::playback_speed`)
/// differs from normal — `0.25×`, `1.5×`, `2×`, the
/// number formatted by [`speed_factor`], the same rule
/// the transport button's `Speed {x}×` label uses.
/// `None` at exactly 1.0 (normal speed is the default
/// state and spends no floored width — see the module
/// docs on the pinned Phase K floors) and `None` for a
/// non-finite speed (a NaN or infinite rate names no
/// speed at all; the cycle maps NaN back to 1.0 before
/// the field can persist, so this is belt-and-braces).
pub fn speed_segment(speed: f32) -> Option<String> {
    if !speed.is_finite() || speed == 1.0 {
        return None;
    }
    Some(format!("{}×", speed_factor(speed)))
}

/// 3D-session segment (Phase M): `3D` while a fig3
/// session is active on the core — the caller reads
/// `core.fig3.is_some()` (the session
/// `EditorCore::enter_3d` starts). The session's
/// presence is the whole signal: which joint is
/// selected and where the 3D camera sits are the 3D
/// panel's business, and the session's own actions
/// already land in the strip's status tail. `None`
/// with no session, so the default strip stays
/// byte-identical to Phase K's (see the module docs'
/// Phase M note on the pinned floors). Text only,
/// deliberately no glyph: the icon table carries a
/// `cube` glyph and it would fit the width maths —
/// the segment is absent by default — but the
/// strip's conditional state segments are text-only
/// by the Phase H/K audio decision, and consistency
/// with that decision wins (see the module docs).
pub fn fig3_segment(active: bool) -> Option<&'static str> {
    if active {
        Some("3D")
    } else {
        None
    }
}

/// The Phase G conditional segment labels, in strip
/// order (audio, loop, stroke, particle), each present
/// only while its condition holds. Pure composition of
/// the four helpers above over the live core plus the
/// shell's two selection fields; the builder turns
/// each label into a floored fixed segment through the
/// same [`segment`] path as the original nine.
pub fn extra_segments(
    core: &EditorCore,
    stroke_selected: Option<usize>,
    particle_selected: Option<usize>,
) -> Vec<String> {
    [
        audio_segment(core.playback_audio_active()),
        loop_segment(core.loop_range),
        stroke_segment(stroke_selected),
        particle_segment(particle_selected),
    ]
    .into_iter()
    .flatten()
    .collect()
}

/// Conservative per-character advance (points) assumed
/// for the strip's label font: `type_::LABEL` is 12 pt
/// at weight 500 (`crates/rune/src/tok.rs:139`), where a
/// typical glyph advances ≈ 0.5 em (6 pt) and digits
/// ≈ 0.55 em. 8.0 pt (0.67 em) sits above the average
/// for mixed prose, digits and the strip's symbol
/// glyphs (`●`, `›`, `❚❚`), so a width estimated with it
/// overstates the true painted width — the safe
/// direction for a floor (see [`est_text_width`]). Only
/// a string made almost entirely of the widest glyphs
/// (capital `W`, `m`) could beat it, and then only under
/// simultaneous width pressure. Phase N sets two wholly
/// numeric segments (timecode, speed) in a mono style at
/// the same 12 pt / weight 500 (see [`numeric_font`]):
/// a mono advance there is ≈ 0.6 em ≈ 7.2 pt, still
/// under this 8.0 estimate, so the estimate covers them
/// unchanged and no floor derived from it moves.
pub const LABEL_CHAR_W: f32 = 8.0;

/// Estimated painted width of `s` in the strip's label
/// font: character count × [`LABEL_CHAR_W`]. Deliberately
/// conservative (an overestimate) — it is used as a
/// minimum-width floor for the fixed segments, and a
/// floor only protects against overlap while it is at
/// least the true content width. The slack it adds is
/// absorbed by the flexible tail segment, never painted
/// over a neighbour.
pub fn est_text_width(s: &str) -> f32 {
    s.chars().count() as f32 * LABEL_CHAR_W
}

/// Cap applied to the status string itself (see
/// [`fit_hint`]). Generous on purpose — the framework's
/// own ellipsis does the precise fitting to the real
/// box; this only bounds pathological strings (long
/// file paths in error messages) before layout. The
/// core's initial hint is 67 chars, a typical status
/// far fewer.
pub const MAX_STATUS_CHARS: usize = 96;

/// Fit `hint` into at most `max_chars` characters,
/// ending with "…" when anything was cut. Pure and
/// total: the result never exceeds `max_chars`
/// characters (the ellipsis counts as one), is always
/// valid UTF-8 (cuts fall between characters), and a
/// hint that already fits is returned unchanged.
///
/// The cut prefers a word boundary: if the last space
/// inside the budget falls in the budget's second half,
/// the string is cut there instead of mid-word (a space
/// earlier than that would waste most of the budget, so
/// a hard character cut wins). Trailing spaces are
/// trimmed before the ellipsis. Degenerate budgets:
/// `max_chars == 0` yields an empty string and
/// `max_chars == 1` yields just "…".
pub fn fit_hint(hint: &str, max_chars: usize) -> String {
    let chars: Vec<char> = hint.chars().collect();
    if chars.len() <= max_chars {
        return hint.to_string();
    }
    if max_chars == 0 {
        return String::new();
    }
    if max_chars == 1 {
        return "…".to_string();
    }
    // One character of the budget is the ellipsis.
    let budget = max_chars - 1;
    let mut keep = budget;
    if let Some(pos) = chars[..budget].iter().rposition(|c| *c == ' ') {
        if pos >= budget / 2 {
            keep = pos;
        }
    }
    let mut out: String = chars[..keep].iter().collect();
    while out.ends_with(' ') {
        out.pop();
    }
    out.push('…');
    out
}

/// The strip's four paint colours, resolved from the
/// shell's theme at one seam (UI mission Wave 8 — see
/// the module docs): the role mapping the strip has
/// always used, now taken from the caller's theme
/// instead of a privately constructed dark one.
/// Fixed segments and the loop glyph paint in
/// `text`, the status tail in `text_secondary`, the
/// dividers in `separator`, and the play segment —
/// while playback is rolling — in `playing` (the
/// brief accent). The seam exists so the mapping is
/// testable: a built `El` is opaque to a unit test,
/// but these are the exact values every builder
/// below receives.
#[derive(Clone, Copy)]
struct StripColors {
    text: Color,
    tail: Color,
    divider: Color,
    playing: Color,
}

/// Resolve the strip's colours from a theme: primary
/// segments take the theme's `text` role, the tail
/// its `text_secondary`, the dividers its
/// `separator`, and the rolling play segment the
/// theme-aware brief accent ([`app_theme::accent`]).
fn strip_colors(t: &Theme) -> StripColors {
    StripColors {
        text: t.text,
        tail: t.text_secondary,
        divider: t.separator,
        playing: app_theme::accent(t),
    }
}

/// One fixed strip segment: a single label-styled text,
/// floored at its estimated content width so the row's
/// shrink pass can never compress its box below the
/// width its glyphs paint (see the module docs).
fn segment(label: String, color: Color) -> El {
    let floor = est_text_width(&label);
    text(label).font(type_::LABEL).color(color).min_w(floor)
}

/// The font of the strip's two wholly numeric segments —
/// the timecode and the playback-speed readout (Phase N,
/// charter §12): `type_::MONO_CAPTION` with the Mono
/// family applied (`.mono()` — the token alone keeps the
/// Sans family in the pinned Rune rev). Same 12 pt /
/// weight 500 as LABEL, and a 12 pt mono advance
/// (≈ 0.6 em ≈ 7.2 pt) sits under the 8.0 pt
/// [`LABEL_CHAR_W`] estimate, so the floors — computed
/// from the label text alone — still cover the painted
/// width and no floor in this module moves. Every other
/// segment stays LABEL: the word+number labels
/// ("Frame 1/5", "12 fps", "Zoom 100%", "Undo 0") are
/// labels, not numeric segments.
fn numeric_font() -> tok::TextStyle {
    type_::MONO_CAPTION.mono()
}

/// One fixed strip segment set in [`numeric_font`] —
/// the mono sibling of [`segment`], with identical
/// floor discipline. Used for the timecode and speed
/// segments only.
fn segment_mono(label: String, color: Color) -> El {
    let floor = est_text_width(&label);
    text(label).font(numeric_font()).color(color).min_w(floor)
}

/// One YIELDING strip segment (UI mission Wave 3):
/// a label-styled text in Rune's truncating
/// one-line mode, based at the SAME
/// [`est_text_width`] a floored [`segment`] would
/// reserve. The explicit `.w()` makes the estimate
/// the flex base; `.truncate()` sets flex_shrink 1
/// with minimum width 0, so under width pressure
/// the box narrows from that base and the glyphs
/// elide to the box with "…" — a yielding segment
/// can never paint over a neighbour, at any width.
/// At rest (no pressure) the box is exactly the
/// old floor, so the strip at 1440 is unchanged.
/// Only the play, fps and zoom segments use this —
/// the first, third and fourth sacrifices in the
/// compression order (see the module docs); every
/// other segment keeps the floored [`segment`]
/// path.
fn segment_yielding(label: String, color: Color) -> El {
    let base = est_text_width(&label);
    text(label).font(type_::LABEL).color(color).w(base).truncate()
}

/// One fixed strip segment with a leading icon glyph
/// (Phase H): a centred row of the glyph — 11 pt tall,
/// width from the icon's own aspect — a 4 pt gap, and
/// the label text, floored at the SAME
/// [`est_text_width`] a plain [`segment`] would use.
/// The glyph is painted in the theme's text colour —
/// the same `color` as the label beside it (Phase N
/// retired the hardcoded hex that used to duplicate
/// that colour for the glyph).
/// The floor stays computed from the text alone; the
/// glyph is additive width inside the floored segment,
/// absorbed by the floor's conservatism. Only a
/// segment whose label is long enough to amortise the
/// glyph may use this — the loop segment, per the
/// width check in the module docs (the audio segment
/// fails that check and stays text-only).
fn segment_with_icon(label: String, icon: &str, color: Color) -> El {
    let floor = est_text_width(&label);
    row()
        .gap(4.0)
        .center_y()
        .min_w(floor)
        .child(crate::rune_sec::icons::icon_el(icon, 11.0, color))
        .child(text(label).font(type_::LABEL).color(color))
}

/// The project segment (UI mission Wave 3): a
/// two-part row built from [`project_parts`] — the
/// name as a yielding truncating text based at its
/// own estimated width (the compression order's
/// second sacrifice), the suffix (` · {runtime}`
/// plus the dirty dot) as a floored plain text
/// that never yields, so the runtime and the
/// strip's save-state signal stay fully visible at
/// every width. The row itself shrinks
/// (`.shrink()`: flex_shrink 1) but is floored at
/// the suffix's estimated width, so the name
/// absorbs the whole narrowing — down to nothing —
/// before the suffix can be squeezed. The parts
/// sit adjacent with no gap: the suffix string
/// carries its own leading separator. At rest the
/// parts sit at their estimated widths and the
/// row measures exactly the old floored segment's
/// [`est_text_width`] of [`project_text`].
fn project_segment(core: &EditorCore, color: Color) -> El {
    let (name, suffix) = project_parts(core);
    let name_base = est_text_width(&name);
    let suffix_floor = est_text_width(&suffix);
    row()
        .gap(0.0)
        .center_y()
        .shrink()
        .min_w(suffix_floor)
        .child(
            text(name)
                .font(type_::LABEL)
                .color(color)
                .w(name_base)
                .truncate(),
        )
        .child(
            text(suffix)
                .font(type_::LABEL)
                .color(color)
                .min_w(suffix_floor),
        )
}

/// Hairline divider between segments. Floored at its
/// own 1 pt so width pressure cannot shrink it away to
/// nothing (the tail segment absorbs the shortage).
/// Phase N (charter §12): a 1×10 tick in the theme's
/// `separator` tone (was 1×12 in `control_track`).
fn divider(color: Color) -> El {
    el().size(1.0, 10.0).min_w(1.0).fill(color)
}

/// The status strip from the core alone: every segment
/// the core can describe, including the Phase G audio
/// and loop segments, but without the shell's stroke /
/// particle-event selection segments (that state lives
/// on `State`, not on the core), without the Phase J
/// autosave segment (the snapshot's mtime is shell
/// support data, read by [`status_strip_state`] through
/// the lib), without the Phase L speed segment (the
/// playback speed likewise lives on `State`) and
/// without the Phase M 3D-session segment (composed
/// by [`status_strip_state`] — this wrapper stays the
/// reduced view, as it does for the speed segment).
/// The delegating wrapper over the shared composer;
/// the shell itself calls [`status_strip_state`].
/// The theme is the shell's, resolved from the
/// core's appearance flag exactly as `main.rs::view`
/// resolves it (UI mission Wave 8).
pub fn status_strip(core: &EditorCore) -> El {
    let t = app_theme::app_theme(!core.light_theme);
    build_strip(core, &t, None, None, None, None, None)
}

/// The status strip: project name (with the dirty dot),
/// frame position, timecode, fps, zoom, selection, undo
/// depth, play state, the Phase L speed segment (while
/// the playback speed differs from normal), the Phase M
/// 3D-session segment (while a fig3 session is active),
/// the Phase G conditional segments (audio, loop range,
/// stroke selection, particle-event selection), the Phase J
/// autosave segment (while a recovery snapshot exists)
/// and the core's current status message, divided by
/// hairlines. Width discipline (see the module docs):
/// the protected segments are width-floored and never
/// yield; a narrowing centre consumes the tail first,
/// then the play segment, the project name, the
/// fps segment and the zoom segment yield in that
/// order — every yielding segment truncates, so the
/// segments can never overlap however narrow the
/// strip gets.
/// Keyed `status-strip` so headless `--tree` checks can
/// find it. Theme is the shell's current app theme —
/// `app_theme(!core.light_theme)`, the exact
/// construction `main.rs::view` builds the whole view
/// with (UI mission Wave 8; the strip previously
/// substituted a private dark theme and stayed
/// dark-coloured in the light appearance).
pub fn status_strip_state(s: &State) -> El {
    let autosave = autosave_segment(
        stykanimate_rs::shell::recovery_modified(),
        SystemTime::now(),
    );
    let t = app_theme::app_theme(!s.core.light_theme);
    build_strip(
        &s.core,
        &t,
        s.stroke_selected,
        s.particle_selected,
        speed_segment(s.playback_speed),
        fig3_segment(s.core.fig3.is_some()),
        autosave,
    )
}

/// Shared composer behind [`status_strip`] and
/// [`status_strip_state`]. The Phase L speed label
/// (`speed`, already composed by [`status_strip_state`]
/// from `State::playback_speed`) heads the conditional
/// block, directly after the play segment; the Phase M
/// 3D-session label (`fig3`, likewise composed by
/// [`status_strip_state`], from the core's fig3
/// session) follows it; the Phase G segments follow
/// those and the Phase J autosave label
/// (`autosave`, already composed by
/// [`status_strip_state`] from the snapshot's mtime)
/// closes the block — each only while its condition
/// holds, each floored through the same [`segment`] /
/// [`est_text_width`] path as the original nine, all
/// before the tail. The tail stays the row's single
/// flexible member.
fn build_strip(
    core: &EditorCore,
    t: &Theme,
    stroke_selected: Option<usize>,
    particle_selected: Option<usize>,
    speed: Option<String>,
    fig3: Option<&'static str>,
    autosave: Option<String>,
) -> El {
    let colors = strip_colors(t);
    let mut segments: Vec<El> = vec![
        // UI mission Wave 3: the project segment is
        // the two-part `project_segment` — the name
        // yields (truncates), the runtime + dirty-dot
        // suffix keeps its floor. Same text, same
        // width at rest.
        project_segment(core, colors.text),
        segment(frame_text(core), colors.text),
        segment_mono(timecode(core.current, core.project.fps), colors.text),
        // UI mission Wave 3: the fps segment yields
        // third in the compression order — truncating,
        // based at its estimated width, no floor.
        segment_yielding(format!("{} fps", fps_text(core.project.fps)), colors.text),
        // UI mission Wave 10: the zoom segment yields
        // fourth in the compression order — truncating,
        // based at its estimated width, no floor. The
        // proof that bought the rung is the module
        // docs' Wave 10 section (the deepest
        // coincidence's proven bound).
        segment_yielding(zoom_text(core), colors.text),
        segment(selection_text(core), colors.text),
        segment(undo_text(core), colors.text),
        // The play segment lights up in the accent colour
        // while playback is rolling. UI mission Wave 3:
        // it is the compression order's first sacrifice
        // — built by `segment_yielding` (truncating,
        // based at its estimated width, no floor)
        // instead of the floored `segment` path. The
        // accent colour while rolling is unchanged in
        // role: the theme's brief accent (Wave 8 made
        // it the app theme's, not upstream's).
        segment_yielding(
            play_text(core.playing).to_string(),
            if core.playing {
                colors.playing
            } else {
                colors.text
            },
        ),
    ];
    // Phase L: the speed segment heads the conditional
    // block — directly after the play segment it
    // qualifies, ahead of the Phase G extras — floored
    // through the same `segment` path, and absent
    // entirely at normal speed, leaving the strip
    // byte-identical to Phase K's (see the module
    // docs' Phase L note on the pinned floors).
    // Phase N: it is built by `segment_mono` instead —
    // the speed readout is wholly numeric, so it takes
    // the mono style; the floor path is unchanged.
    if let Some(label) = speed {
        segments.push(segment_mono(label, colors.text));
    }
    // Phase M: the 3D-session segment follows the
    // speed segment in the conditional block — both
    // qualify the shell's editing state rather than
    // the project — ahead of the Phase G extras,
    // floored through the same `segment` path, and
    // absent entirely with no session, leaving the
    // strip byte-identical to Phase K's (see the
    // module docs' Phase M note on the pinned
    // floors).
    if let Some(label) = fig3 {
        segments.push(segment(label.to_string(), colors.text));
    }
    // Phase G conditional segments: after the original
    // fixed segments, before the tail, in audio / loop /
    // stroke / particle order. Each is floored at its own
    // label's estimated width exactly like the original
    // fixed segments; each appears only while its
    // condition holds, so the strip's usual width is
    // unchanged. Phase H: the loop segment is built by
    // `segment_with_icon` instead — same label, same
    // floor, plus the leading `repeat` glyph whose
    // absorption the module docs verify. The branch is
    // chosen by comparing against this module's own
    // `loop_segment` output, so it tracks any future
    // formatting change; no other extra label can
    // collide ("Audio on", "Stroke {n}", "Particle {n}").
    let loop_label = loop_segment(core.loop_range);
    for label in extra_segments(core, stroke_selected, particle_selected) {
        if Some(&label) == loop_label.as_ref() {
            segments.push(segment_with_icon(label, "repeat", colors.text));
        } else {
            segments.push(segment(label, colors.text));
        }
    }
    // Phase J: the autosave segment closes the fixed
    // block — after the Phase G conditionals, before
    // the tail — floored through the same `segment`
    // path, and absent entirely when no recovery
    // snapshot exists, leaving the strip byte-identical
    // to Phase I's.
    if let Some(label) = autosave {
        segments.push(segment(label, colors.text));
    }
    // Chrome's line ended with the core's status
    // message; keep it, dimmed, as the tail segment —
    // and make it the row's single flexible member.
    // `grow(1)` gives it whatever width the fixed
    // segments leave; `.truncate()` puts it in
    // Rune's one-line mode (flex_shrink 1, minimum
    // width 0 — `crates/rune/src/el/text.rs`,
    // `one_line`), so under shortage its box, alone,
    // gives way and the painted line is elided with
    // "…" to that box at real font metrics. The
    // string is pre-capped by `fit_hint` as a
    // backstop. The protected segments sit on their
    // floors and cannot be compressed; the four
    // yielding segments (play, project name, fps,
    // zoom — see the module docs' Wave 3 and Wave 10
    // sections) narrow only through Rune's truncating
    // mode, which elides glyphs to the box. Either
    // way, no segment's glyphs can ever reach a
    // neighbour's box.
    segments.push(
        text(fit_hint(&core.status, MAX_STATUS_CHARS))
            .font(type_::LABEL)
            .color(colors.tail)
            .grow(1.0)
            .truncate(),
    );
    let mut bar = row().key("status-strip").w_full().gap(8.0).center_y();
    for (i, seg) in segments.into_iter().enumerate() {
        if i > 0 {
            bar = bar.child(divider(colors.divider));
        }
        bar = bar.child(seg);
    }
    bar
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timecode_frame_zero_is_zero() {
        assert_eq!(timecode(0, 12.0), "0:00.0");
        assert_eq!(timecode(0, 60.0), "0:00.0");
    }

    #[test]
    fn timecode_whole_seconds() {
        assert_eq!(timecode(12, 12.0), "0:01.0");
        assert_eq!(timecode(60, 30.0), "0:02.0");
    }

    #[test]
    fn timecode_tenths_digit() {
        // 150 frames at 12 fps = 12.5 s exactly.
        assert_eq!(timecode(150, 12.0), "0:12.5");
        // 29 frames at 10 fps = 2.9 s; the division must
        // not land a hair under the whole tenth.
        assert_eq!(timecode(29, 10.0), "0:02.9");
        // 7 frames at 30 fps = 0.233 s -> 2.33 tenths,
        // rounds down to 2.
        assert_eq!(timecode(7, 30.0), "0:00.2");
    }

    #[test]
    fn timecode_rounds_half_away_from_zero_like_playback() {
        // The Phase E unification cases: under the old
        // floor maths each of these read one tenth lower.
        // Frame 7 at 12 fps = 0.583 s -> 5.83 tenths -> 6.
        assert_eq!(timecode(7, 12.0), "0:00.6");
        // Frame 8 at 30 fps = 0.267 s -> 2.67 tenths -> 3.
        assert_eq!(timecode(8, 30.0), "0:00.3");
        // 3723 frames at 60 fps = 62.05 s -> exactly
        // 620.5 tenths in f32, and the exact half rounds
        // away from zero (the playback chip semantics),
        // so this is 1:02.1, not the floored 1:02.0.
        assert_eq!(timecode(3723, 60.0), "1:02.1");
        // Rounding carries across the minute boundary:
        // 1799 frames at 30 fps = 59.97 s -> 599.7
        // tenths -> 600 -> 1:00.0, never 0:60.0.
        assert_eq!(timecode(1799, 30.0), "1:00.0");
    }

    #[test]
    fn timecode_rolls_into_minutes() {
        // A full hour of 10 fps frames keeps counting
        // minutes rather than rolling to hours.
        assert_eq!(timecode(36_000, 10.0), "60:00.0");
    }

    #[test]
    fn timecode_unusable_fps_is_zero() {
        assert_eq!(timecode(100, 0.0), "0:00.0");
        assert_eq!(timecode(100, -12.0), "0:00.0");
        assert_eq!(timecode(100, f32::NAN), "0:00.0");
    }

    #[test]
    fn fps_text_whole_and_fractional() {
        assert_eq!(fps_text(12.0), "12");
        assert_eq!(fps_text(29.97), "30.0");
        assert_eq!(fps_text(23.976), "24.0");
    }

    #[test]
    fn frame_text_is_one_based() {
        let core = EditorCore::new();
        let txt = frame_text(&core);
        assert!(txt.starts_with("Frame 1/"), "unexpected: {txt}");
        assert!(txt.ends_with(&core.frame_count().to_string()));
    }

    /// Wave P (P48): the strip's three fully-keyed
    /// readout segments — frame ([`frame_text`]), zoom
    /// ([`zoom_text`]) and undo/redo ([`undo_text`]) —
    /// render their label words from the i18n table in
    /// the core's current language. Switching
    /// `core.lang` (the settings popover's
    /// `set_language` write) changes the rendered
    /// strings; the English default stays byte-identical
    /// to the literals the segments carried before the
    /// wiring. (Zoom percentages read the shell's global
    /// view nav, so the identity view is reset first —
    /// the `zoom_text_agrees_with_the_overlay_readout`
    /// idiom.)
    #[test]
    fn strip_readout_labels_follow_the_core_language() {
        use stykanimate_rs::i18n::Lang;
        crate::stage_view::reset_view_nav();
        let mut core = EditorCore::new();
        let count = core.frame_count();
        // English default: the historical literals.
        assert_eq!(frame_text(&core), format!("Frame 1/{count}"));
        assert_eq!(zoom_text(&core), "Zoom 100%");
        assert_eq!(undo_text(&core), "Undo 0");
        // Bengali: every label word moves (the keys'
        // Bengali values are genuine translations, not
        // placeholders), the numbers do not.
        core.lang = Lang::Bn;
        assert_eq!(frame_text(&core), format!("ফ্রেম 1/{count}"));
        assert_eq!(zoom_text(&core), "জুম 100%");
        assert_eq!(undo_text(&core), "আনডু 0");
        // One snapshot-counted edit, then an undo: the
        // redo half of the segment localises too.
        core.snapshot();
        core.undo();
        assert_eq!(undo_text(&core), "আনডু 0 · রিডু 1");
        // Chinese (Simplified): a second genuine table,
        // so the pin cannot pass on a one-language fluke.
        core.lang = Lang::ZhHans;
        assert_eq!(frame_text(&core), format!("帧 1/{count}"));
        assert_eq!(zoom_text(&core), "缩放 100%");
        assert_eq!(undo_text(&core), "撤销 0 · 重做 1");
        crate::stage_view::reset_view_nav();
    }

    #[test]
    fn zoom_text_defaults_to_100_percent() {
        let core = EditorCore::new();
        assert_eq!(zoom_text(&core), "Zoom 100%");
    }

    #[test]
    fn zoom_text_agrees_with_the_overlay_readout() {
        // UI mission Wave 9: the strip's zoom segment and
        // the stage overlay's readout
        // (`camera_panel::view_zoom_pct`) are the same
        // effective percentage. `zoom_text` replicates
        // the overlay's expression (the speed_label
        // precedent in this module: replicate, then pin
        // the pair together so a change to either side
        // breaks here instead of letting the two
        // displays drift apart).
        let core = EditorCore::new();
        // Identity view: effective == authored (100%).
        crate::stage_view::reset_view_nav();
        assert_eq!(zoom_text(&core), "Zoom 100%");
        assert_eq!(crate::rune_sec::camera_panel::view_zoom_pct(&core), 100);
        // View zoomed to 150%: both displays move together.
        crate::stage_view::set_view_nav(crate::stage_view::ViewNav {
            zoom: 1.5,
            pan: [0.0, 0.0],
        });
        assert_eq!(zoom_text(&core), "Zoom 150%");
        assert_eq!(crate::rune_sec::camera_panel::view_zoom_pct(&core), 150);
        // View zoomed out to 50%, then back to identity.
        crate::stage_view::set_view_nav(crate::stage_view::ViewNav {
            zoom: 0.5,
            pan: [0.0, 0.0],
        });
        assert_eq!(zoom_text(&core), "Zoom 50%");
        assert_eq!(crate::rune_sec::camera_panel::view_zoom_pct(&core), 50);
        crate::stage_view::reset_view_nav();
        assert_eq!(zoom_text(&core), "Zoom 100%");
    }

    #[test]
    fn selection_text_figure_only() {
        let core = EditorCore::new();
        assert_eq!(core.selected_node, None);
        assert_eq!(selection_text(&core), core.project.figures[0].name);
    }

    #[test]
    fn selection_text_figure_and_node() {
        let mut core = EditorCore::new();
        core.selected_node = Some(3);
        let name = core.project.figures[0].name.clone();
        assert_eq!(selection_text(&core), format!("{name} › node 3"));
    }

    #[test]
    fn selection_text_no_figures() {
        let mut core = EditorCore::new();
        core.project.figures.clear();
        assert_eq!(selection_text(&core), "No selection");
    }

    #[test]
    fn play_text_reflects_state() {
        assert_eq!(play_text(true), "▶ Playing");
        assert_eq!(play_text(false), "❚❚ Paused");
    }

    #[test]
    fn project_text_has_no_dot_on_a_fresh_core() {
        // A fresh core has counted no edits, so the
        // autosave-relative dirty signal reads clean.
        // Phase I: the segment now carries the runtime
        // too — the demo is 5 frames of duration 1.0 at
        // 12 fps, 5/12 s, which rounds to 0:00 — so the
        // pin is the name plus ` · 0:00`, no dot.
        let core = EditorCore::new();
        assert_eq!(core.edits_since_autosave(), 0);
        assert_eq!(
            project_text(&core),
            format!("{} · 0:00", core.project.name)
        );
    }

    #[test]
    fn project_text_gains_the_dot_after_an_edit() {
        // add_frame snapshots, which is an undoable edit
        // and bumps the counter the marker reads. Phase
        // I: the runtime sits between the name and the
        // dot — 6 frames of duration 1.0 at 12 fps is
        // exactly 0.5 s, rounding half away from zero
        // to 0:01.
        let mut core = EditorCore::new();
        core.add_frame();
        assert!(core.edits_since_autosave() > 0);
        assert_eq!(
            project_text(&core),
            format!("{} · 0:01 ●", core.project.name)
        );
    }

    // ---- Phase I runtime label ----

    /// A core whose project runs at `fps` with the
    /// given per-frame durations (the demo's frames,
    /// truncated to the list's length — the runtime
    /// only reads durations and fps).
    fn core_with_runtime(fps: f32, durations: &[f32]) -> EditorCore {
        let mut core = EditorCore::new();
        core.project.fps = fps;
        core.project.frames.truncate(durations.len());
        for (frame, d) in core.project.frames.iter_mut().zip(durations) {
            frame.duration = *d;
        }
        core
    }

    #[test]
    fn runtime_label_empty_project_is_zero() {
        let mut core = EditorCore::new();
        core.project.frames.clear();
        assert_eq!(runtime_label(&core), "0:00");
    }

    #[test]
    fn runtime_label_zero_pads_seconds_under_a_minute() {
        // 5 × 14.0 frame periods at 10 fps = 7.0 s.
        let core = core_with_runtime(10.0, &[14.0, 14.0, 14.0, 14.0, 14.0]);
        assert_eq!(runtime_label(&core), "0:07");
    }

    #[test]
    fn runtime_label_counts_duration_as_frame_periods() {
        // The model's own example (model.rs,
        // durations_expand_and_total_time): durations
        // 2 + 3 + 1 + 1 + 1 at 12 fps total 8/12 s,
        // which rounds to one second.
        let core = core_with_runtime(12.0, &[2.0, 3.0, 1.0, 1.0, 1.0]);
        assert_eq!(runtime_label(&core), "0:01");
        // A single frame held for 24 periods at 12 fps
        // lasts exactly two seconds: duration is a
        // multiplier on the frame period, not a count
        // of frames.
        let core = core_with_runtime(12.0, &[24.0]);
        assert_eq!(runtime_label(&core), "0:02");
    }

    #[test]
    fn runtime_label_fractional_durations_round_half_away_from_zero() {
        // 25 periods at 10 fps = exactly 2.5 s, and the
        // exact half rounds away from zero (the strip's
        // timecode rounding), not down.
        let core = core_with_runtime(10.0, &[25.0]);
        assert_eq!(runtime_label(&core), "0:03");
        // Two quarter-speed halves: 2.5 + 2.5 periods
        // at 10 fps = exactly 0.5 s -> 0:01.
        let core = core_with_runtime(10.0, &[2.5, 2.5]);
        assert_eq!(runtime_label(&core), "0:01");
        // Just under the half rounds down: 24 periods
        // at 10 fps = 2.4 s.
        let core = core_with_runtime(10.0, &[24.0]);
        assert_eq!(runtime_label(&core), "0:02");
    }

    #[test]
    fn runtime_label_minutes_unpadded_seconds_padded() {
        // 7540 periods at 10 fps = 754 s = 12 min 34 s.
        let core = core_with_runtime(10.0, &[7540.0]);
        assert_eq!(runtime_label(&core), "12:34");
    }

    #[test]
    fn undo_text_starts_at_zero_without_redo() {
        let core = EditorCore::new();
        assert_eq!(undo_text(&core), "Undo 0");
    }

    #[test]
    fn undo_text_counts_edits_and_shows_redo_after_undo() {
        let mut core = EditorCore::new();
        core.add_frame();
        assert_eq!(undo_text(&core), "Undo 1");
        // Undoing moves the snapshot to the redo stack;
        // the redo depth joins the same segment.
        core.undo();
        assert_eq!(undo_text(&core), "Undo 0 · Redo 1");
    }

    #[test]
    fn strip_counters_follow_a_live_mcp_edit() {
        // Wave 11 (MCP end-to-end): the strip's undo
        // and dirty readings must follow an edit made
        // through the LIVE MCP path, not only GUI ops.
        // This drives the real route the shell's poll
        // hook drains — `McpServer::handle_on` against
        // a caller-owned core (the router's own core is
        // not the target) — with a real `tools/call`
        // for the registry's `add_frame` tool, whose
        // handler calls the same `EditorCore::add_frame`
        // the timeline's + Frame button calls. The
        // segments are then read back through this
        // module's own fns: there is no second counter
        // for the strip to be stale on.
        let mut core = EditorCore::new();
        assert_eq!(undo_text(&core), "Undo 0");
        assert_eq!(core.edits_since_autosave(), 0);
        let (name, suffix) = project_parts(&core);
        assert_eq!(format!("{name}{suffix}"), project_text(&core));
        assert!(!suffix.contains('●'), "a fresh core is not dirty");

        let mut router = stykanimate_rs::mcp::McpServer::new();
        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": { "name": "add_frame", "arguments": {} }
        });
        let reply = router
            .handle_on(&mut core, &msg)
            .expect("a tools/call carrying an id always replies");
        assert_eq!(
            reply["result"]["isError"], false,
            "the add_frame call succeeded: {reply}"
        );
        // The op's own status is the one the poll hook
        // toasts and the strip's tail shows.
        assert_eq!(core.status, "Frame 2 added.");
        assert_eq!(frame_text(&core), "Frame 2/6");
        // The counters the strip renders moved with
        // the MCP edit: one undo step deep, and the
        // project segment's suffix carries the dirty
        // dot (the runtime ticks to 0:01 at six
        // frames, as in the GUI edit test above).
        assert_eq!(undo_text(&core), "Undo 1");
        let (_, suffix) = project_parts(&core);
        assert_eq!(suffix, " · 0:01 ●");
        // And the strip builds whole in the post-MCP
        // state, tail included.
        let _ = status_strip(&core);
    }

    #[test]
    fn strip_project_name_follows_a_live_mcp_rename() {
        // Wave 12 (E8 audit): the project segment reads
        // the SESSION name (`core.project.name`), never
        // the file name — and Wave 11's `project_rename`
        // handler sets that name in the same call that
        // moves the file (`tool_project_rename`,
        // mcp_tools.rs), so the strip follows an MCP
        // rename in the same tick, showing the typed
        // name rather than the moved file's slug. (The
        // name stored INSIDE the moved file stays the
        // old one until the next save —
        // `projects::rename_project`'s documented
        // contract — so the Home library, which
        // `projects::scan` names from the stored name,
        // can still list the old name; that divergence
        // is the file layer's, and the module docs'
        // Wave 12 section records it. This pin covers
        // the strip's half: the session name, at once.)
        let dir = std::env::temp_dir().join(format!(
            "stykanimate-strip-rename-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut core = EditorCore::new();
        let old_path = dir.join("untitled.json");
        std::fs::write(
            &old_path,
            serde_json::to_string(&core.project).unwrap(),
        )
        .unwrap();
        core.project_files = vec![old_path];
        core.project_index = 0;
        assert_eq!(project_text(&core), "Untitled · 0:00");

        let mut router = stykanimate_rs::mcp::McpServer::new();
        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "project_rename",
                "arguments": { "new_name": "Renamed Project" }
            }
        });
        let reply = router
            .handle_on(&mut core, &msg)
            .expect("a tools/call carrying an id always replies");
        assert_eq!(
            reply["result"]["isError"], false,
            "the project_rename call succeeded: {reply}"
        );
        // The segment names the session's new name at
        // once — the typed name, not the file's slug
        // ("renamed-project").
        //
        // Wave 13: the text now carries the dirty dot.
        // Wave 12's pin expected no dot because the
        // handler's session-name write counted nothing
        // then; the dirty-marking census assigns that
        // write a `mark_dirty` at its call site (B5's
        // verdict in `projects.rs` — the refresh seam
        // in the same handler never touches the
        // counters, so this one counted edit is the
        // name write's alone), and the strip must show
        // what the census promises: one counted edit,
        // and — a mark, not a snapshot — no undo step.
        assert_eq!(core.edits_since_autosave(), 1);
        assert_eq!(undo_text(&core), "Undo 0");
        assert_eq!(project_text(&core), "Renamed Project · 0:00 ●");
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ---- Wave 13 dirty-marking read path ----

    #[test]
    fn strip_dot_follows_the_counter_not_the_undo_stack() {
        // Wave 13: the dot renders
        // `edits_since_autosave()` — the counter — and
        // nothing else. A snapshotted edit lights it;
        // stamping the counter as autosaved
        // (`note_recovery_written`, the core's seam
        // for recovery writes made outside
        // `maybe_autosave`) clears it again, while the
        // undo stack keeps its step the whole time. A
        // strip that inferred dirtiness from the stack
        // would stay lit after the stamp; this one
        // follows the counter down as faithfully as up.
        let mut core = EditorCore::new();
        assert_eq!(core.edits_since_autosave(), 0);
        let (_, suffix) = project_parts(&core);
        assert!(!suffix.contains('●'), "a fresh core is not dirty");

        core.add_frame();
        assert_eq!(core.edits_since_autosave(), 1);
        assert_eq!(undo_text(&core), "Undo 1");
        let (_, suffix) = project_parts(&core);
        assert_eq!(suffix, " · 0:01 ●");

        core.note_recovery_written();
        assert_eq!(
            core.edits_since_autosave(),
            0,
            "the stamp zeroes the counter the dot reads"
        );
        assert_eq!(
            undo_text(&core),
            "Undo 1",
            "the undo stack is untouched by the stamp"
        );
        let (_, suffix) = project_parts(&core);
        assert_eq!(suffix, " · 0:01", "the dot cleared with the counter");
        assert_eq!(project_text(&core), "Untitled · 0:01");
        // And the strip builds whole in the stamped
        // state.
        let _ = status_strip(&core);
    }

    #[test]
    fn strip_dot_lights_for_a_mark_dirty_write_with_no_undo_step() {
        // Wave 13 (Squad B contract): a non-undoable
        // project write the census assigned to the
        // `mark_dirty` seam must light the dot WITHOUT
        // moving the undo segment — the two segments
        // read different sources (counter vs stacks),
        // and this is the state where they honestly
        // disagree. The seam is driven through a
        // landed B-squad op, `set_figure_color`
        // (figure_ops: no snapshot; an actual colour
        // change marks dirty exactly once), and every
        // assertion goes through this module's fns or
        // the counter accessor — no test in this file
        // names `mark_dirty` itself (at this audit
        // its call sites were in the tree ahead of
        // the editor.rs definition), so the pin fails
        // loudly if the seam ever bumps a counter
        // other than the one `snapshot()` bumps.
        let mut core = EditorCore::new();
        let stored = core.project.figures[0].color;
        // Re-selecting the colour already stored
        // changes nothing and marks nothing: the
        // strip stays clean and the stacks stay empty.
        core.set_figure_color(0, stored);
        assert_eq!(core.edits_since_autosave(), 0);
        let (_, suffix) = project_parts(&core);
        assert!(!suffix.contains('●'), "a no-op write is not dirty");

        // An actual colour change: one counted edit,
        // no undo step, and the dot lit beside
        // `Undo 0`.
        let changed = if stored == 0xFF5D5D { 0x5DB2FF } else { 0xFF5D5D };
        core.set_figure_color(0, changed);
        assert_eq!(core.project.figures[0].color, changed);
        assert_eq!(
            core.edits_since_autosave(),
            1,
            "the marked write counts exactly one edit"
        );
        assert_eq!(
            undo_text(&core),
            "Undo 0",
            "a marked write is not an undo step"
        );
        let (name, suffix) = project_parts(&core);
        assert_eq!(name, core.project.name);
        assert_eq!(suffix, " · 0:00 ●");
        assert_eq!(
            project_text(&core),
            format!("{} · 0:00 ●", core.project.name)
        );
        // A second marked write counts a second edit
        // — the cadence the autosave threshold reads
        // advances per write, not per snapshot.
        core.set_figure_color(0, stored);
        assert_eq!(core.edits_since_autosave(), 2);
        assert_eq!(undo_text(&core), "Undo 0");
        let _ = status_strip(&core);
    }

    #[test]
    fn strip_shows_no_stale_dirty_state_across_a_project_swap() {
        // Wave 13: this module caches nothing, so a
        // project swap cannot strand the previous
        // project's dirty state on the strip. The
        // swap is driven through the GUI's own New
        // path (`project_tools::new_project`: replace
        // the project, `note_project_swapped` zeroes
        // both edit counters), and the segment is
        // read back on the other side: new name, new
        // runtime, no dot — nothing of the dirty
        // project survives in anything the strip
        // renders from the counter.
        let mut core = EditorCore::new();
        core.project.name = "Before Swap".to_string();
        core.add_frame();
        assert_eq!(core.edits_since_autosave(), 1);
        assert_eq!(project_text(&core), "Before Swap · 0:01 ●");

        crate::rune_sec::project_tools::new_project(&mut core);

        assert_eq!(
            core.edits_since_autosave(),
            0,
            "the swap restarts the dirty baseline"
        );
        assert_eq!(
            project_text(&core),
            "Untitled · 0:00",
            "the strip reads the swapped-in project, clean"
        );
        let (_, suffix) = project_parts(&core);
        assert!(!suffix.contains('●'));
        // And the strip builds whole over the swapped
        // core.
        let _ = status_strip(&core);
    }

    // ---- Width discipline (Phase F) ----

    #[test]
    fn fit_hint_returns_a_fitting_hint_unchanged() {
        assert_eq!(fit_hint("Saved.", 96), "Saved.");
        assert_eq!(fit_hint("short", 5), "short");
        // Exactly at the limit is a fit, not a cut.
        assert_eq!(fit_hint("12345", 5), "12345");
        // The core's initial hint (67 chars) passes the
        // production cap untouched.
        let core = EditorCore::new();
        assert_eq!(core.status.chars().count(), 67);
        assert_eq!(fit_hint(&core.status, MAX_STATUS_CHARS), core.status);
    }

    #[test]
    fn fit_hint_truncates_with_an_ellipsis_at_a_word_boundary() {
        // Budget 19 chars + ellipsis: the kept prefix is
        // "Drag a joint to pos" and its last space (char
        // 15, before "pose") sits in the budget's second
        // half, so the cut lands there — "Drag a joint
        // to…" — rather than mid-word at "pos".
        let hint = "Drag a joint to pose. Hands and feet use IK; drag the hips to move.";
        let fitted = fit_hint(hint, 20);
        assert_eq!(fitted, "Drag a joint to…");
        assert!(fitted.chars().count() <= 20);
    }

    #[test]
    fn fit_hint_hard_cuts_when_no_usable_space() {
        // No space at all: cut at the budget.
        assert_eq!(fit_hint("abcdefghij", 5), "abcd…");
        // A space in the first half of the budget would
        // waste it, so the hard cut wins.
        assert_eq!(fit_hint("a bcdefghij", 6), "a bcd…");
    }

    #[test]
    fn fit_hint_empty_and_tiny_budgets() {
        assert_eq!(fit_hint("", 10), "");
        assert_eq!(fit_hint("", 0), "");
        assert_eq!(fit_hint("anything", 0), "");
        assert_eq!(fit_hint("anything", 1), "…");
        assert_eq!(fit_hint("abcdef", 2), "a…");
    }

    #[test]
    fn fit_hint_counts_characters_not_bytes() {
        // `❚` is three bytes in UTF-8; a byte-based cut
        // would split it or misjudge the length.
        let fitted = fit_hint("❚❚ Paused ❚❚ Paused", 6);
        assert_eq!(fitted, "❚❚…");
        assert_eq!(fitted.chars().count(), 3);
        // Any result respects the cap, multibyte or not.
        for max in 0..12 {
            let out = fit_hint("●●●● ●●●● ●●●●", max);
            assert!(out.chars().count() <= max, "max {max}: {out}");
        }
    }

    #[test]
    fn est_text_width_scales_with_character_count() {
        assert_eq!(est_text_width(""), 0.0);
        assert_eq!(est_text_width("Undo 0"), 6.0 * LABEL_CHAR_W);
        // Multibyte glyphs count once each.
        assert_eq!(est_text_width("❚❚ Paused"), 9.0 * LABEL_CHAR_W);
        // The floor for a plausible longest fixed segment
        // stays well under the strip's real width, so the
        // floors only bite under genuine pressure.
        assert!(est_text_width("Undo 0 · Redo 1") < 130.0);
    }

    #[test]
    fn strip_builds_with_a_pathologically_long_status() {
        // A status far past the cap must not panic the
        // builder; the tail is capped before layout.
        let mut core = EditorCore::new();
        core.status = "x".repeat(400);
        let _ = status_strip(&core);
        assert_eq!(
            fit_hint(&core.status, MAX_STATUS_CHARS).chars().count(),
            MAX_STATUS_CHARS
        );
    }

    // ---- Phase G conditional segments ----

    #[test]
    fn audio_segment_only_while_active() {
        assert_eq!(audio_segment(true), Some("Audio on".to_string()));
        assert_eq!(audio_segment(false), None);
    }

    #[test]
    fn loop_segment_is_one_based_with_an_en_dash() {
        assert_eq!(loop_segment(None), None);
        assert_eq!(loop_segment(Some((0, 4))), Some("Loop 1–5".to_string()));
        // A single-frame loop names the same frame twice.
        assert_eq!(loop_segment(Some((2, 2))), Some("Loop 3–3".to_string()));
    }

    #[test]
    fn stroke_segment_is_one_based() {
        assert_eq!(stroke_segment(None), None);
        assert_eq!(stroke_segment(Some(0)), Some("Stroke 1".to_string()));
        assert_eq!(stroke_segment(Some(41)), Some("Stroke 42".to_string()));
    }

    #[test]
    fn particle_segment_is_one_based() {
        assert_eq!(particle_segment(None), None);
        assert_eq!(particle_segment(Some(0)), Some("Particle 1".to_string()));
        assert_eq!(
            particle_segment(Some(998)),
            Some("Particle 999".to_string())
        );
    }

    #[test]
    fn extra_segments_empty_on_a_fresh_core() {
        // No audio playing, no loop set, no selections:
        // the Phase G block contributes nothing and the
        // strip is exactly the Phase F nine plus tail.
        let core = EditorCore::new();
        assert!(!core.playback_audio_active());
        assert_eq!(core.loop_range, None);
        assert!(extra_segments(&core, None, None).is_empty());
    }

    #[test]
    fn extra_segments_come_in_strip_order() {
        // Audio is inactive on a fresh core (it cannot
        // be started without a player), so the order is
        // proven over the three remaining conditions:
        // loop, then stroke, then particle.
        let mut core = EditorCore::new();
        core.loop_range = Some((0, 4));
        assert_eq!(
            extra_segments(&core, Some(2), Some(0)),
            vec![
                "Loop 1–5".to_string(),
                "Stroke 3".to_string(),
                "Particle 1".to_string(),
            ]
        );
    }

    #[test]
    fn extra_segment_floors_use_the_same_est_text_width_path() {
        // The builder floors every extra through
        // `segment()`, i.e. at `est_text_width` of its
        // own label. The longest plausible particle
        // label must therefore floor at exactly
        // `est_text_width` of that label — the same
        // path, so an extra can never be compressed
        // below its painted width either.
        let label = particle_segment(Some(998)).unwrap();
        assert_eq!(label, "Particle 999");
        assert_eq!(est_text_width(&label), est_text_width("Particle 999"));
        assert_eq!(est_text_width(&label), 12.0 * LABEL_CHAR_W);
        // The widest loop label shape floors the same
        // way: 13 characters including the en dash.
        let label = loop_segment(Some((998, 999))).unwrap();
        assert_eq!(label, "Loop 999–1000");
        assert_eq!(est_text_width(&label), 13.0 * LABEL_CHAR_W);
    }

    #[test]
    fn strip_builds_with_core_extras_present() {
        // The core-only wrapper shows the core-readable
        // extras (here a loop range) without panicking.
        let mut core = EditorCore::new();
        core.loop_range = Some((1, 3));
        let _ = status_strip(&core);
        assert_eq!(
            extra_segments(&core, None, None),
            vec!["Loop 2–4".to_string()]
        );
    }

    #[test]
    fn strip_builds_with_the_longest_loop_label() {
        // Phase H: the loop segment's icon branch must
        // build at the longest pinned label too — the
        // label whose floor absorption the module docs
        // verify — and the glyph leaves the label string
        // itself unchanged.
        let mut core = EditorCore::new();
        core.loop_range = Some((998, 999));
        let _ = status_strip(&core);
        assert_eq!(
            extra_segments(&core, None, None),
            vec!["Loop 999–1000".to_string()]
        );
        // The absorption budget the module docs state:
        // the text-only floor (104 pt) still covers the
        // glyph (11 pt) + its gap (4 pt) + the label's
        // painted width at the documented advances
        // (≈ 84 pt for this label's glyph classes).
        assert!(est_text_width("Loop 999–1000") > 11.0 + 4.0 + 84.0);
    }

    // ---- Phase J autosave segment ----

    /// A fixed reference instant for the age table
    /// (the same arbitrary epoch the shell tests use,
    /// shell.rs): the ages below are offsets from it,
    /// so the table never depends on the wall clock.
    fn ref_now() -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000)
    }

    #[test]
    fn autosave_segment_none_without_a_snapshot() {
        assert_eq!(autosave_segment(None, ref_now()), None);
    }

    #[test]
    fn autosave_segment_age_table() {
        let now = ref_now();
        let ago = |secs: u64| Some(now - Duration::from_secs(secs));
        // Under a minute is "just now", boundary incl.:
        // 59 s is the last whole second of the band.
        assert_eq!(
            autosave_segment(ago(10), now),
            Some("Autosaved just now".to_string())
        );
        assert_eq!(
            autosave_segment(ago(59), now),
            Some("Autosaved just now".to_string())
        );
        // Whole minutes, floored: 60 s opens the band,
        // 59 whole minutes is its last reading.
        assert_eq!(
            autosave_segment(ago(60), now),
            Some("Autosaved 1m ago".to_string())
        );
        assert_eq!(
            autosave_segment(ago(300), now),
            Some("Autosaved 5m ago".to_string())
        );
        assert_eq!(
            autosave_segment(ago(3_599), now),
            Some("Autosaved 59m ago".to_string())
        );
        // Whole hours from one hour up, unbounded:
        // a day-old snapshot honestly reads 25h.
        assert_eq!(
            autosave_segment(ago(3_600), now),
            Some("Autosaved 1h ago".to_string())
        );
        assert_eq!(
            autosave_segment(ago(7_200), now),
            Some("Autosaved 2h ago".to_string())
        );
        assert_eq!(
            autosave_segment(ago(90_000), now),
            Some("Autosaved 25h ago".to_string())
        );
    }

    #[test]
    fn autosave_segment_future_mtime_clamps_to_just_now() {
        // A snapshot stamped ahead of the process clock
        // (clock skew) is zero-age, never a negative or
        // nonsense reading.
        let now = ref_now();
        let future = now + Duration::from_secs(120);
        assert_eq!(
            autosave_segment(Some(future), now),
            Some("Autosaved just now".to_string())
        );
    }

    #[test]
    fn strip_builds_with_and_without_the_autosave_segment() {
        // The composer takes the composed label as an
        // `Option`, the selection-segment pattern: with
        // `None` the strip builds exactly as Phase I's
        // did, and with a label it builds with the
        // segment appended — neither path panics.
        let core = EditorCore::new();
        let t = app_theme::app_theme(true);
        let _ = build_strip(&core, &t, None, None, None, None, None);
        let now = ref_now();
        let label = autosave_segment(Some(now - Duration::from_secs(300)), now);
        assert_eq!(label, Some("Autosaved 5m ago".to_string()));
        let _ = build_strip(&core, &t, None, None, None, None, label);
        // The builder floors the segment through
        // `segment()`, i.e. at `est_text_width` of its
        // own label — the same path as every other
        // fixed segment, so it can never be compressed
        // below its painted width either.
        assert_eq!(est_text_width("Autosaved 5m ago"), 16.0 * LABEL_CHAR_W);
        assert_eq!(est_text_width("Autosaved just now"), 18.0 * LABEL_CHAR_W);
    }

    // ---- Phase K audio-glyph width recomputation ----

    #[test]
    fn audio_glyph_width_recomputation_at_744pt() {
        // Phase K: Phase H refused the audio segment a
        // `volume-high` glyph on width maths from the
        // ≈ 461 pt column; Phase J widened the strip to
        // 744 pt (headless tree dump: status-strip
        // bounds [328, 845, 744, 15]), so the question
        // was re-run with the glyph's painted width
        // charged against the strip. Every number of
        // that computation is pinned here (see the
        // module docs): if a default label, the icon
        // size or the strip width ever changes, this
        // test fails and the question must be re-asked
        // on fresh numbers, not inherited silently.
        let core = EditorCore::new();
        // The default state's fixed floors — no stroke
        // or particle selection, no loop — each through
        // the file's own `est_text_width` of the live
        // label a fresh demo core produces.
        let floors = [
            est_text_width(&project_text(&core)),
            est_text_width(&frame_text(&core)),
            est_text_width(&timecode(core.current, core.project.fps)),
            est_text_width(&format!("{} fps", fps_text(core.project.fps))),
            est_text_width(&zoom_text(&core)),
            est_text_width(&selection_text(&core)),
            est_text_width(&undo_text(&core)),
            est_text_width(play_text(core.playing)),
        ];
        assert_eq!(
            floors,
            [120.0, 72.0, 48.0, 48.0, 72.0, 32.0, 48.0, 72.0],
            "project / frame / timecode / fps / zoom / selection / undo / play"
        );
        // The audio segment's own floor.
        let audio_floor = est_text_width(&audio_segment(true).unwrap());
        assert_eq!(audio_floor, 64.0);
        // The glyph at the loop segment's construction
        // (`segment_with_icon`: 11 pt tall, 4 pt gap),
        // its width through the icon table's own
        // aspect — `volume-high`'s view box is 640
        // wide against the 512 height.
        let glyph_w = crate::rune_sec::icons::icon_width("volume-high", 11.0)
            .expect("volume-high is in the curated icon set");
        assert_eq!(glyph_w, 13.75);
        let glyph_total = glyph_w + 4.0;
        // The verdict: the strip minus a 160 pt
        // minimum for the tail absorber. Without the
        // glyph the default state fits with 8 pt to
        // spare; with the glyph charged it overflows
        // by 9.75 pt — so the audio segment stays
        // text-only (the module docs carry the full
        // argument).
        let base: f32 = floors.iter().sum::<f32>() + audio_floor;
        assert_eq!(base, 576.0);
        assert!(base + 160.0 <= 744.0, "the glyph-less strip fits");
        let total = base + glyph_total;
        assert_eq!(total, 593.75);
        assert!(
            total + 160.0 > 744.0,
            "with the glyph charged, the tail would fall below 160 pt"
        );
    }

    // ---- UI mission Wave 3 compression ----

    #[test]
    fn project_parts_split_name_from_suffix() {
        // Clean core: the suffix is the runtime
        // alone, and the parts concatenate to
        // `project_text` byte-for-byte (the pinned
        // default-strip test below holds the whole
        // string).
        let core = EditorCore::new();
        let (name, suffix) = project_parts(&core);
        assert_eq!(name, core.project.name);
        assert_eq!(suffix, " · 0:00");
        assert_eq!(format!("{name}{suffix}"), project_text(&core));
        // Dirty core: the dot rides in the SUFFIX —
        // the protected half — never in the name:
        // the strip's save-state signal lives in the
        // part that never truncates.
        let mut core = EditorCore::new();
        core.add_frame();
        let (name, suffix) = project_parts(&core);
        assert_eq!(suffix, " · 0:01 ●");
        assert_eq!(format!("{name}{suffix}"), project_text(&core));
        assert!(!name.contains('●'));
    }

    #[test]
    fn project_segment_and_yielding_segments_build() {
        // The new constructions build in every state
        // the strip meets: fresh, dirty, and with a
        // name far longer than the default (the case
        // the name's truncation exists for).
        let colors = strip_colors(&app_theme::app_theme(true));
        let mut core = EditorCore::new();
        let _ = project_segment(&core, colors.text);
        core.add_frame();
        let _ = project_segment(&core, colors.text);
        core.project.name = "A very long project name indeed".to_string();
        let _ = project_segment(&core, colors.text);
        let _ = segment_yielding(play_text(false).to_string(), colors.text);
        let _ = segment_yielding("12 fps".to_string(), colors.text);
        // The whole strip still builds through both
        // wrappers with the new segment constructions
        // in place.
        let _ = status_strip(&core);
    }

    #[test]
    fn strip_compression_arithmetic_at_584pt() {
        // UI mission Wave 3 (audit S03 D1/C1): the
        // strip must fit the 584 pt centre column of
        // the app's minimum window (1280 wide)
        // without any protected segment yielding.
        // Every number is in this module's estimated-
        // width accounting (`est_text_width` of live
        // labels — the Phase K tradition), with
        // boundaries charged at the MEASURED 17 pt
        // each (8 pt row gap + 1 pt divider + 8 pt
        // gap, read off the Phase N tree dump, where
        // the default strip's segment boxes are
        // exactly the floors pinned here and the
        // tail measures 96). If a default label, a
        // yielding base or the segment set changes,
        // this test fails and the compression
        // question must be re-asked on fresh numbers.
        let core = EditorCore::new();
        let floors = [
            est_text_width(&project_text(&core)),
            est_text_width(&frame_text(&core)),
            est_text_width(&timecode(core.current, core.project.fps)),
            est_text_width(&format!("{} fps", fps_text(core.project.fps))),
            est_text_width(&zoom_text(&core)),
            est_text_width(&selection_text(&core)),
            est_text_width(&undo_text(&core)),
            est_text_width(play_text(core.playing)),
        ];
        assert_eq!(floors.iter().sum::<f32>(), 512.0);
        // The yielding bases inside that sum: the
        // play segment, the project NAME (the rest
        // of the project floor is the protected
        // suffix), the fps segment, and — since
        // Wave 10 — the zoom segment.
        let (name, suffix) = project_parts(&core);
        let yield_capacity = est_text_width(play_text(core.playing))
            + est_text_width(&name)
            + est_text_width(&format!("{} fps", fps_text(core.project.fps)))
            + est_text_width(&zoom_text(&core));
        assert_eq!(yield_capacity, 72.0 + 64.0 + 48.0 + 72.0);
        let protected = 512.0 - yield_capacity;
        assert_eq!(protected, 256.0);
        // The protected set keeps the two segments
        // the wave brief names as always fully
        // visible whole: the frame readout, and the
        // save state — the suffix's dirty dot,
        // pinned here, and the autosave segment at
        // its longest plausible label (Phase J),
        // which is built by the floored `segment`
        // path like the frame segment.
        assert_eq!(est_text_width(&frame_text(&core)), 72.0);
        assert_eq!(est_text_width(&suffix), 56.0);
        assert_eq!(est_text_width("Autosaved 999h ago"), 144.0);
        // At 744 (the reference centre): floors plus
        // the 8 measured boundaries leave the tail
        // exactly the 96 pt the dump shows — no
        // shortage, nothing yields, nothing removed.
        assert_eq!(512.0 + 8.0 * 17.0 + 96.0, 744.0);
        // At 584, default state: the shortage is
        // 512 + 136 − 584 = 64, inside the yield
        // capacity; the protected floors plus
        // boundaries fit whole.
        assert!(512.0 + 8.0 * 17.0 - 584.0 <= yield_capacity);
        assert!(protected + 8.0 * 17.0 <= 584.0);
        // The audit's C1 coincidence (audio + speed
        // 1.5× + 3D live): floors 624 in this
        // accounting; the protected part fits 584
        // whole and the yielding set covers the
        // rest. The boundary-charged form of this
        // coincidence — minimum widths against the
        // column, clean and dirty — is the Wave 10
        // proof in
        // `deepest_coincidence_proven_bound_at_584pt`
        // below.
        let c1_floors = 512.0
            + est_text_width(&audio_segment(true).unwrap())
            + est_text_width(&speed_segment(1.5).unwrap())
            + est_text_width(fig3_segment(true).unwrap());
        assert_eq!(c1_floors, 624.0);
        assert!(c1_floors - yield_capacity <= 584.0);
    }

    #[test]
    fn deepest_coincidence_proven_bound_at_584pt() {
        // UI mission Wave 10: the deepest coincidence
        // — audio live, speed 1.5×, a 3D session, over
        // the default state — at the 584 pt minimum
        // centre, with EVERY charge carried: each
        // present segment at its label's
        // `est_text_width`, the tail at its minimum
        // of zero, and all 11 boundaries at the
        // measured 17 pt (derived in the module docs:
        // (N−1) × (8 gap + 1 divider + 8 gap) for the
        // row's N = 12 elements). Wave 3 left this as
        // a ≈ 43 pt residual estimate; here it is
        // exact, before and after the fourth rung
        // (the zoom segment yielding) that closes it.
        // If a coincidence label, the ladder's
        // membership or the boundary constant changes,
        // this test fails and the bound must be
        // re-proven, not inherited.
        let core = EditorCore::new();
        // The legs, each a pinned literal of its
        // label fn. The audio label is a constant
        // string — its width is exact; only its
        // presence depends on ffplay liveness, which
        // is the coincidence's premise.
        let audio = est_text_width(&audio_segment(true).unwrap());
        assert_eq!(audio_segment(true).unwrap(), "Audio on");
        assert_eq!(audio, 64.0);
        // The speed leg at the coincidence's 1.5×,
        // and the cycle's extremes: the widest label
        // is the SLOWEST step, the narrowest the
        // fastest.
        assert_eq!(est_text_width(&speed_segment(1.5).unwrap()), 32.0);
        assert_eq!(est_text_width(&speed_segment(0.25).unwrap()), 40.0);
        assert_eq!(est_text_width(&speed_segment(2.0).unwrap()), 16.0);
        let legs = audio
            + est_text_width(&speed_segment(1.5).unwrap())
            + est_text_width(fig3_segment(true).unwrap());
        assert_eq!(legs, 64.0 + 32.0 + 16.0);
        // The yield pool with the fourth rung: play,
        // project name, fps, zoom.
        let (name, clean_suffix) = project_parts(&core);
        let pool = est_text_width(play_text(core.playing))
            + est_text_width(&name)
            + est_text_width(&format!("{} fps", fps_text(core.project.fps)))
            + est_text_width(&zoom_text(&core));
        assert_eq!(pool, 256.0);
        // Protected floors in the coincidence: the
        // default 512 minus the pool, plus the legs.
        let protected = 512.0 - pool + legs;
        assert_eq!(protected, 368.0);
        // 12 elements in the row -> 11 boundaries.
        let boundaries = 11.0 * 17.0;
        assert_eq!(boundaries, 187.0);
        // BEFORE the rung, the zoom floor (72) sat in
        // the protected set: minimum width 627
        // against the 584 column — an overflow of
        // exactly 43, the Wave 3 residual made exact.
        assert_eq!(protected + 72.0 + boundaries, 627.0);
        assert_eq!(protected + 72.0 + boundaries - 584.0, 43.0);
        // AFTER the rung: minimum width 555 — the
        // coincidence fits with 29 pt to spare.
        assert_eq!(protected + boundaries, 555.0);
        assert!(protected + boundaries <= 584.0);
        // The dirty variant: a real editing session's
        // suffix carries the dirty dot, 16 pt wider
        // than the clean one (` · 0:00` -> ` · 0:01 ●`
        // at the same runtime width). Before the
        // rung it overflowed by 59; after, it fits
        // with 13 to spare.
        let mut dirty = EditorCore::new();
        dirty.add_frame();
        let (_, dirty_suffix) = project_parts(&dirty);
        let dirty_delta =
            est_text_width(&dirty_suffix) - est_text_width(&clean_suffix);
        assert_eq!(dirty_delta, 16.0);
        assert_eq!(protected + 72.0 + dirty_delta + boundaries - 584.0, 59.0);
        assert_eq!(protected + dirty_delta + boundaries, 571.0);
        assert!(protected + dirty_delta + boundaries <= 584.0);
        // Robust across the speed cycle: the widest
        // speed label (+8 over 1.5×) still fits dirty.
        assert!(protected + dirty_delta + 8.0 + boundaries <= 584.0);
        // The demand side agrees: total floors/bases
        // (624) plus boundaries, minus the column, is
        // the shortage, and the pool covers it in
        // every variant (the longest project name
        // only enlarges the pool — it is rung 2).
        assert_eq!(624.0 + boundaries - 584.0, 227.0);
        assert!(624.0 + boundaries - 584.0 <= pool);
        assert!(624.0 + dirty_delta + 8.0 + boundaries - 584.0 <= pool);
        // The strip builds in the coincidence state
        // (the audio leg cannot be forced without a
        // player; the speed and 3D legs go through
        // the composer exactly as
        // `status_strip_state` passes them).
        let t = app_theme::app_theme(true);
        let _ = build_strip(
            &core,
            &t,
            None,
            None,
            speed_segment(1.5),
            fig3_segment(true),
            None,
        );
    }

    #[test]
    fn worst_case_segment_widths_from_source() {
        // UI mission Wave 10: the per-segment
        // worst-case audit behind the module docs'
        // table — each label produced by its own fn
        // at the edge of the stated universe (at most
        // 1000 frames, durations in the editor's
        // 0.25..=8.0 clamp, fps in its 1.0..=60.0
        // clamp, counters at the 999 plausible cap),
        // or pinned as a literal where the value
        // lives outside this module (the zoom
        // corner's camera × view product, the frame
        // readout at the frame cap).
        // Timecode: the last frame at the slowest
        // fps reads 999 s.
        assert_eq!(timecode(999, 1.0), "16:39.0");
        assert_eq!(est_text_width("16:39.0"), 56.0);
        // Runtime: the universe's edge is 1000
        // frames x 8.0 at 1 fps = 8000 s = 133:20,
        // and the dirty suffix around it is 11
        // characters.
        let core = core_with_runtime(1.0, &[8000.0]);
        assert_eq!(runtime_label(&core), "133:20");
        assert_eq!(est_text_width(" · 133:20 ●"), 88.0);
        // Frame readout at the frame cap.
        assert_eq!(est_text_width("Frame 1000/1000"), 120.0);
        // Zoom at its documented corner (camera
        // 4.0 x view 8.0 = 3200%).
        assert_eq!(est_text_width("Zoom 3200%"), 80.0);
        // Undo at the plausible cap, redo live.
        assert_eq!(est_text_width("Undo 999 · Redo 999"), 152.0);
        // The selection-index conditionals at the
        // cap, through their own fns.
        assert_eq!(stroke_segment(Some(999)).unwrap(), "Stroke 1000");
        assert_eq!(est_text_width("Stroke 1000"), 88.0);
        assert_eq!(particle_segment(Some(999)).unwrap(), "Particle 1000");
        assert_eq!(est_text_width("Particle 1000"), 104.0);
        // The fps segment's widest base in range:
        // a fractional rate just under the 60 clamp.
        assert_eq!(fps_text(59.94), "59.9");
        assert_eq!(est_text_width("59.9 fps"), 64.0);
    }

    // ---- Phase L speed segment ----

    #[test]
    fn speed_segment_truth_table() {
        // Normal speed is the default state: no segment,
        // so the pinned Phase K floors above describe
        // the strip exactly as built.
        assert_eq!(speed_segment(1.0), None);
        // The off-normal steps of the cycle, formatted
        // with the transport label's trim rule.
        assert_eq!(speed_segment(1.5), Some("1.5×".to_string()));
        assert_eq!(speed_segment(0.25), Some("0.25×".to_string()));
        assert_eq!(speed_segment(0.5), Some("0.5×".to_string()));
        assert_eq!(speed_segment(2.0), Some("2×".to_string()));
        // A non-finite speed names no speed at all.
        assert_eq!(speed_segment(f32::NAN), None);
        assert_eq!(speed_segment(f32::INFINITY), None);
        assert_eq!(speed_segment(f32::NEG_INFINITY), None);
    }

    #[test]
    fn speed_segment_numbers_match_speed_label() {
        // `speed_factor` replicates the number rule
        // private to `playback_controls::speed_label`;
        // pin the two together over the whole cycle
        // table plus an off-table value, so a change to
        // either formatting breaks here instead of
        // letting the strip and the button drift apart.
        for speed in crate::rune_sec::playback_controls::SPEEDS {
            let button = crate::rune_sec::playback_controls::speed_label(speed);
            let number = button
                .strip_prefix("Speed ")
                .expect("the button label keeps its prefix");
            if speed == 1.0 {
                assert_eq!(speed_segment(speed), None);
            } else {
                assert_eq!(speed_segment(speed), Some(number.to_string()));
            }
        }
        assert_eq!(speed_segment(0.75), Some("0.75×".to_string()));
        assert_eq!(
            crate::rune_sec::playback_controls::speed_label(0.75),
            "Speed 0.75×"
        );
    }

    #[test]
    fn strip_speed_segment_only_off_normal_speed() {
        // Segment-list level, through the same
        // composition `status_strip_state` performs: a
        // fresh shell state sits at normal speed, so the
        // composed label is `None` and the strip builds
        // exactly as Phase K's did; at double speed the
        // label is present and the strip builds with the
        // segment appended after the play segment.
        let mut s = crate::init();
        assert_eq!(s.playback_speed, 1.0);
        assert_eq!(speed_segment(s.playback_speed), None);
        let _ = status_strip_state(&s);
        s.playback_speed = 2.0;
        assert_eq!(speed_segment(s.playback_speed), Some("2×".to_string()));
        let _ = status_strip_state(&s);
        // The composer takes the composed label as an
        // `Option`, the autosave pattern: both paths
        // build without panicking, and the segment is
        // floored through `segment()` at its own label's
        // `est_text_width` — 2 characters for `2×`.
        let core = EditorCore::new();
        let t = app_theme::app_theme(true);
        let _ = build_strip(&core, &t, None, None, speed_segment(1.0), None, None);
        let _ = build_strip(&core, &t, None, None, speed_segment(2.0), None, None);
        assert_eq!(est_text_width("2×"), 2.0 * LABEL_CHAR_W);
        assert_eq!(est_text_width("0.25×"), 5.0 * LABEL_CHAR_W);
    }

    // ---- Phase M 3D-session segment ----

    #[test]
    fn fig3_segment_truth_table() {
        // No session is the default state: no segment,
        // so the pinned Phase K floors above describe
        // the strip exactly as built.
        assert_eq!(fig3_segment(false), None);
        // A live session names itself with two
        // characters of text — and no glyph (see the
        // module docs' Phase M note).
        assert_eq!(fig3_segment(true), Some("3D"));
        assert_eq!(est_text_width("3D"), 2.0 * LABEL_CHAR_W);
    }

    #[test]
    fn strip_fig3_segment_only_with_a_session() {
        // Composed-strip level, through the same
        // composition `status_strip_state` performs: a
        // fresh shell state has no fig3 session, so the
        // composed label is `None` and the strip builds
        // exactly as Phase L's did; once the session
        // starts, the composed label is `Some("3D")`
        // and the strip builds with the segment in the
        // conditional block.
        let mut s = crate::init();
        assert!(s.core.fig3.is_none());
        assert_eq!(fig3_segment(s.core.fig3.is_some()), None);
        let _ = status_strip_state(&s);
        s.core.enter_3d();
        assert!(s.core.fig3.is_some());
        assert_eq!(fig3_segment(s.core.fig3.is_some()), Some("3D"));
        let _ = status_strip_state(&s);
        // The composer takes the composed label as an
        // `Option`, the autosave/speed pattern: both
        // paths build without panicking, and the
        // segment is floored through `segment()` at
        // its own label's `est_text_width` — 2
        // characters for `3D` (pinned above).
        let core = EditorCore::new();
        let t = app_theme::app_theme(true);
        let _ = build_strip(&core, &t, None, None, None, fig3_segment(false), None);
        let _ = build_strip(&core, &t, None, None, None, fig3_segment(true), None);
    }

    #[test]
    fn default_strip_segment_texts_unchanged() {
        // Byte-identity of the default strip, pinned at
        // the composition level: on a fresh shell state
        // every conditional label is absent — speed
        // `None` (Phase L), 3D `None` (Phase M), the
        // Phase G extras empty — and the eight fixed
        // labels are exactly the texts the Phase K
        // floor arithmetic charges (512 pt of floors in
        // all). Any change to a default label breaks
        // here and re-opens the pinned arithmetic
        // loudly, next to the test that owns it.
        let s = crate::init();
        assert_eq!(speed_segment(s.playback_speed), None);
        assert_eq!(fig3_segment(s.core.fig3.is_some()), None);
        assert!(
            extra_segments(&s.core, s.stroke_selected, s.particle_selected).is_empty()
        );
        let core = &s.core;
        assert_eq!(project_text(core), "Untitled · 0:00");
        assert_eq!(frame_text(core), "Frame 1/5");
        assert_eq!(timecode(core.current, core.project.fps), "0:00.0");
        assert_eq!(format!("{} fps", fps_text(core.project.fps)), "12 fps");
        assert_eq!(zoom_text(core), "Zoom 100%");
        assert_eq!(selection_text(core), "Styk");
        assert_eq!(undo_text(core), "Undo 0");
        assert_eq!(play_text(core.playing), "❚❚ Paused");
    }

    // ---- Phase N numeric-segment mono style ----

    #[test]
    fn numeric_segments_use_the_mono_caption_style() {
        // Phase N (charter §12): the strip's two wholly
        // numeric segments — timecode and speed — are
        // set in MONO_CAPTION with the Mono family
        // applied. A built `El` is opaque to a unit
        // test, so the pin lands on the style seam both
        // call sites share: `segment_mono` fonts its
        // text with `numeric_font()`, and no other
        // builder in the module does.
        assert_eq!(numeric_font(), type_::MONO_CAPTION.mono());
        // `.mono()` really applied: the bare token keeps
        // the Sans family, so the two styles differ.
        assert_ne!(numeric_font(), type_::MONO_CAPTION);
        // Same metrics as LABEL — 12 pt, weight 500 —
        // so the LABEL_CHAR_W floors (computed from the
        // label text alone) still cover the painted
        // width, and the Phase K width proof above is
        // untouched by the font change.
        assert_eq!(numeric_font().size, type_::LABEL.size);
        assert_eq!(numeric_font().weight, type_::LABEL.weight);
        // Both mono call sites construct. (The timecode
        // path is also exercised by every strip build
        // above; the speed path by the Phase L tests.)
        let colors = strip_colors(&app_theme::app_theme(true));
        let _ = segment_mono(timecode(150, 12.0), colors.text);
        let _ = segment_mono(speed_segment(1.5).unwrap(), colors.text);
        // The plain segment path still constructs too —
        // it stays LABEL for every other segment.
        let _ = segment("Frame 1/5".to_string(), colors.text);
    }

    // ---- UI mission Wave 8 app-theme colours ----

    #[test]
    fn strip_colours_follow_the_app_theme() {
        // UI mission Wave 8: the strip previously built
        // its segments from a privately constructed
        // `tok::theme(true)`, so in the light appearance
        // its text painted upstream's dark near-white
        // (≈ #EFF0F2) on the light background — the
        // Wave 7 dumps show #eff0f2 in BOTH trees. The
        // colours now resolve from the caller's app
        // theme at the `strip_colors` seam (a built
        // `El` is opaque, so the pin lands on the seam
        // every builder receives — the `numeric_font`
        // precedent above).
        let dark = app_theme::app_theme(true);
        let light = app_theme::app_theme(false);
        let d = strip_colors(&dark);
        let l = strip_colors(&light);
        // The role mapping, in both appearances: fixed
        // segments take `text`, the tail takes
        // `text_secondary`, the dividers take
        // `separator`, and the rolling play segment
        // takes the theme's brief accent.
        assert_eq!(d.text, dark.text);
        assert_eq!(d.tail, dark.text_secondary);
        assert_eq!(d.divider, dark.separator);
        assert_eq!(d.playing, app_theme::accent(&dark));
        assert_eq!(l.text, light.text);
        assert_eq!(l.tail, light.text_secondary);
        assert_eq!(l.divider, light.separator);
        assert_eq!(l.playing, app_theme::accent(&light));
        // Every role actually differs across the two
        // appearances — the regression this test guards
        // is a role silently pinned to one theme.
        assert_ne!(d.text, l.text);
        assert_ne!(d.tail, l.tail);
        assert_ne!(d.divider, l.divider);
        assert_ne!(d.playing, l.playing);
        // The pinned values: dark is the brief palette
        // the whole shell renders with, light its
        // mirrored ink on the light background (never
        // the near-white that was invisible there).
        assert_eq!(d.text.hex(), "#f1f3f7ff");
        assert_eq!(d.tail.hex(), "#a1a9b7ff");
        assert_eq!(d.playing.hex(), "#6c8cffff");
        assert_eq!(l.text.hex(), "#171a21ff");
        assert_eq!(l.tail.hex(), "#5b6577ff");
        assert_eq!(l.playing.hex(), "#4666d8ff");
        // The whole strip builds under both themes,
        // through the composer and through the
        // core-only wrapper (which resolves the theme
        // from the core's own appearance flag).
        let core = EditorCore::new();
        let _ = build_strip(&core, &dark, None, None, None, None, None);
        let _ = build_strip(&core, &light, None, None, None, None, None);
        let _ = status_strip(&core);
        let mut light_core = EditorCore::new();
        light_core.light_theme = true;
        let _ = status_strip(&light_core);
    }
}
