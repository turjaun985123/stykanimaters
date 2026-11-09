# Wave 14 — Contract re-verification across all new tools (E10)

**What this is.** The standing MCP contracts, re-verified
handler by handler against everything Wave 14 adds:

1. every NEW mutating handler spends **exactly one**
   snapshot (a double snapshot and a zero snapshot are
   both failures);
2. every float argument passes a finiteness check —
   including the f32-cast overflow case (a finite f64
   such as `1e300` casting to f32 infinity) — **before**
   the op runs;
3. refusals happen **before** any snapshot: a refused
   call spends no undo step;
4. an unknown tool name still returns the protocol
   error **−32602** with the byte-exact message
   `unknown tool: {name}`;
5. shell tools refuse on the stdio transport with the
   shape B1 specifies (`docs/wave14/SHELL_CONTROL.md`
   §5).

**Method and snapshot.** Source audit only (no cargo,
per the repo rule): each handler body was read, its
snapshot calls counted in the handler **and** in the op
it drives, its argument helpers traced to their
finiteness checks, and its refusal order compared
against the first snapshot/write. The tree was still
moving under the audit (squads landing in parallel);
this is the as-found state at the freeze,
**2026-11-09 ~10:50 +06**, with the working tree's
uncommitted squad files included. Line numbers are
from that snapshot. Handlers whose registry entries
are still the integrator's splice were audited at the
handler — the unit the contracts bind; a splice cannot
change a handler's refusal order. Items not yet in the
tree are marked **PENDING**, never passed by proxy.

**Headline verdict.** **61 handlers audited. Contracts
(1)–(4): zero violations.** Contract (5): **not yet
conformant** — B1's §5 shape and the B-squad files
diverge in six named places (F1–F6 below); all six are
**integrator fix items** for the splice, and none is a
defect in a shipped surface (the shell family is
pre-splice: today the shell names are simply absent
from `all_tools()`, so a stdio call to one earns the
ordinary −32602 — exactly what B1 §5 says must not
happen once they are listed).

The one systematic subtlety: contract (1) binds the
**strict family**. Three documented exception classes,
each precedented by registered pre-Wave-14 tools and
each named in its own spec text, take no undo step by
design — the mark-dirty panel-parity family (the
Wave 13 dirty-marking census discipline: the panel
writes the same fields with `mark_dirty()` and no
snapshot), the session/shell-state family (undo stores
the `Project` only — the Wave 11 contract; view
navigation per the Wave 5 doctrine), and the file-op
family (`project_rename` / `duplicate_project`
precedent). They are tabled separately below so the
exception is auditable rather than silent.

---

## A. Strict family — exactly one snapshot (14)

| Handler (registered name) | Handler / op snapshot evidence | Finiteness | Refusal before spend |
|---|---|---|---|
| `fx_set_seed` | `fx_ops.rs:676`; op `fx_set_seed` snapshot `fx_ops.rs:461` (sole) | integer only; `fx_arg_u32` (`:586`) range-refuses past u32 | index pre-check `:678` before op |
| `fx_set_angle` | `fx_ops.rs:709`; op snapshot `:482` (sole) | `fx_arg_f32` `:566` — f64 finite **and** cast finite | index pre-check before arg read |
| `fx_set_length` | `fx_ops.rs:742`; op snapshot `:505` (sole) | same helper | same |
| `particle_set_seed` | `particle_ops.rs:745`; op snapshot `:356` (sole) | integer only; `particle_mcp_usize` + `u32::try_from` | `particle_mcp_index` (`:468`) + seed range before op |
| `audio_clip_gain_step` | `audio_ops.rs:1448`; chain `audio_clip_gain_step_at` `:509` → `audio_set_gain_at` snapshot `:487` (sole) | `aop_req_f32` is a bare cast, **and** the handler checks the cast value `delta.is_finite()` at `:1451` before the op — compliant (see O1) | `aop_check_index` (`:1098`) before the op |
| `apply_nib_stroke` | `draw_ops.rs:1589`; the handler itself snapshots once, then the free fn `apply_nib` rewrites in place | `angle` via `dops_opt_f32` `:461` (f64 + cast finite) | index + nib-enum refusals before the snapshot |
| `smooth_stroke` | `draw_ops.rs:1653`; handler snapshots once when it smooths | `subdiv` via the new `dops_opt_i32` (finite + whole + range) | subdiv range refusal before; a <2-point stroke spends **nothing** and reports `smoothed: false` (documented conditional step) |
| `mirror_all_strokes` | `draw_ops.rs:1719`; handler snapshots once | `cx` via `dops_opt_f32` | empty-project refusal before the snapshot |
| `set_stroke_spray` | `draw_ops.rs:1781`; snapshots once per change path | `radius`/`opacity` via `dops_opt_f32`, then range refusals (>0, 0..=1) | conflict / at-least-one / range refusals all before; clearing a markless stroke spends nothing, `changed: false` |
| `clear_strokes_on_frame` | `draw_ops.rs:1896`; snapshots only when the frame carries strokes | integer frame via `dops_check_frame` | frame check first; 0 strokes → no spend |
| `nudge_path_point` | `figure_ops.rs:1503`; op `editor.rs:1333` snapshot `:1344` (sole) | `fop_req_f32` + explicit `is_finite` re-check `:1508` | both-zero refusal and the pose-existence pre-check both before the op (the handler exists partly so the op's snapshot is never spent on an unmoving target) |
| `camera_zoom_by` | `editor.rs:5732`; op `:855` snapshot `:856` (sole) | `eds_req_f32` → `eds_finite_f32` `:5375` (f64 + cast finite) | positive-factor refusal, quoting the value, before the op |
| `rotate_camera` | `editor.rs:5771`; op `:1439` snapshot `:1440` (sole) | `eds_req_f32` | — (no other refusal class) |
| `graph_set_interp_at_playhead` | `graph_keys.rs:1576`; op `set_graph_key_interp` `editor.rs:1683`, snapshot `:1713` after all no-op returns | no float args (interp is an enum string) | `gk_parse_interp` refusal before the op; the op's status-only no-ops (no joint / no channel / no keys) return before its snapshot — "one undo step **when it sets**", per its spec |

## B. Read-only (13) — no snapshot, no spend, by construction

| Handler | Evidence |
|---|---|
| `graph_key_interp` | `graph_keys.rs:1475`; frame via `gk_req_frame` `:816` (`gk_opt_f32` checks the cast value's finiteness `:796`, plus non-negative) |
| `graph_count_keys_in_range` | `graph_keys.rs:1518`; `from > to` refused, quoting the rule |
| `sprite_sample` | `media.rs:1009`; frame via `ast_req_f32` → `ast_as_f32` `:778` (post-cast finite check); negative refused quoting the value; keyless sprite refused |
| `get_onion_skin` | `editor.rs:5811` |
| `get_editor_settings` | `editor.rs:5872` |
| `clipboard_state` | `editor.rs:6086` |
| `get_brush` | `draw_ops.rs:1289` |
| `shell_get_state` | `shell_tools_state.rs:296` (State-based; drain-dispatched) |
| `shell_get_view` | `shell_tools_view.rs:402` |
| `shell_get_transport` | `shell_tools_transport.rs:264` |
| `shell_get_selection` | `shell_tools_selection.rs:392` |
| `shell_get_layout` | `shell_tools_layout.rs:430` (answers via `shell_control::submit` — plumbing, see P2) |
| `shell_get_settings` (exec form) | `shell_tools_settings.rs:591` |

## C. Session / shell-state family (24) — no undo step by the house contract

Undo snapshots store the `Project` only (Wave 11);
everything here writes session fields, shell `State`,
or the view-navigation layer, which a GUI press also
changes without an undo step. Zero `snapshot()` calls
exist in any `shell_tools_*.rs` production code
(grep-verified). All refusals precede all writes.

| Handler | File:line | Float / refusal notes |
|---|---|---|
| `set_draw_mode` | `draw_ops.rs:1341` | bool only; writes `core.draw_mode` (session) |
| `step_brush_param` | `draw_ops.rs:1429` | enum + `direction` ∈ {1, −1} refusals (value quoted) before the write; no-preset-step is a `changed: false` success, the panel's own behaviour |
| `set_brush_nib` | `draw_ops.rs:1529` | enum refusal before the write |
| `set_onion_skin` | `editor.rs:5843` | bool only; `core.onion` is a session field, not project state |
| `figure_toggle_solo` | `figure_ops.rs:1466` | index pre-checked via `fop_target_figure`; solo is session state |
| `fig3_set_camera` (**amended**, A7) | `fig3d.rs:914` | session refusal first; yaw/pitch/distance/fov via `fig3mcp_opt_f32` `:855` (f64 + cast finite); `target[ i ]` components check **both** at `:933`; at-least-one-field refusal before any write; no snapshot — the session-camera class Wave 11 already documents for orbit/zoom |
| `shell_play` | `shell_tools_transport.rs:155` | no args; sets the core's playback flag (see F1 for its stdio divergence) |
| `shell_pause` | `shell_tools_transport.rs:187` | no args |
| `shell_set_playhead` | `shell_tools_transport.rs:228` | whole-frame arg via `sht_req_frame`; out-of-range refusal quoting frame + count before `goto` |
| `shell_select_figure` | `shell_tools_selection.rs:220` | `check_index` quotes index + valid range before the write |
| `shell_select_node` | `shell_tools_selection.rs:246` | figure then node range-checked before either write |
| `shell_select_sprite` | `shell_tools_selection.rs:275` | targets-based signature (drain-dispatched); range check before the write |
| `shell_select_fx_clip` | `shell_tools_selection.rs:302` | same pattern |
| `shell_select_audio_clip` | `shell_tools_selection.rs:329` | same pattern |
| `shell_select_frame` | `shell_tools_selection.rs:361` | range check before `goto` + range-selection clear |
| `shell_view_zoom` | `shell_tools_view.rs:204` | `finite_f32` `:89` (f64 + cast); factor/percent exclusivity and positivity refusals (values quoted) before `set_view_nav`; view-only per the Wave 5 doctrine |
| `shell_view_fit` | `shell_tools_view.rs:273` | no args |
| `shell_view_centre` | `shell_tools_view.rs:305` | x/y via `opt_f32`; pair-together, no-figures and no-stage-layout refusals before any write |
| `shell_view_reset` | `shell_tools_view.rs:367` | non-positive camera zoom refused, quoting frame + value |
| `shell_set_workspace` | `shell_tools_state.rs:139` | `parse_workspace` refusal lists the valid names before `select_workspace` runs |
| `shell_set_section` | `shell_tools_layout.rs:358` | section-name and collapsible refusals (vocabulary quoted) before `submit` |
| `shell_set_column_widths` | `shell_tools_layout.rs:393` | `lay_opt_f32` `:284` (post-cast finite); at-least-one refusal before `submit` |
| `shell_set_theme` (exec form) | `shell_tools_settings.rs:553` | `parse_theme_arg` `:312` validates first |
| `shell_set_setting` (exec form) | `shell_tools_settings.rs:700` | `parse_setting_patch` `:334` validates the whole patch first (finiteness at `:475`, both f64 and cast); apply is a separate fn |

## D. Mark-dirty panel-parity exception family (9)

Project writes that take **no undo step** — deliberately,
matching the GUI panel that owns the same fields (the
Wave 13 dirty-marking census discipline: a real change
calls `mark_dirty()` exactly once; a no-change call marks
nothing). Each handler's spec text states the exception
and its reason; A9's and A2's own tests pin
`undo_depth() == 0` across these writes. This family is
the documented exception to contract (1), tabled here
so it is a decision on record, not a silent pass.

| Handler | File:line | Notes |
|---|---|---|
| `set_weather_kind` | `editor.rs:5500` | kind enum parsed first; `mark_dirty()` only when the stored weather changed |
| `cycle_weather` | `editor.rs:5554` | no args; marks only on change |
| `set_weather_amount` | `editor.rs:5611` | `eds_req_f32` first; weather-off refusal **before** any write or mark |
| `set_weather_wind` | `editor.rs:5652` | same shape |
| `clear_camera_key` | `editor.rs:5693` | the backing op is the camera panel's own "Clear key (no undo)" button; existence refusal before the op; the op marks dirty internally |
| `figure_step_opacity` | `figure_ops.rs:1359` | layers-family parity (the registered `layers` tool and `figure_cycle_opacity` predate it); delta finite-checked twice over, zero refused, both before the op |
| `figure_toggle_hidden` | `figure_ops.rs:1401` | layers-family; index pre-checked |
| `figure_toggle_lock` | `figure_ops.rs:1434` | layers-family; index pre-checked |
| `adjust_frame_duration` | `frame_swap.rs:512` | timing-tweak parity; `fs_arg_f32` `:355` (post-cast finite), positive + frame-range refusals before the op; op `editor.rs:2214` is mark-dirty-only per its Wave 13 note |

## E. File-op family (1)

| Handler | File:line | Notes |
|---|---|---|
| `delete_project_file` | `projects.rs:681` | no undo step — a file operation, the `project_rename` / `duplicate_project` family. Every refusal (missing / blank / not-in-the-library-listing / shell-owned name) resolves **before** the delete runs, with the argument quoted; the tool cannot be aimed outside `./projects`. No float args. |

---

## Contract (4) — unknown tool −32602: PASS

The fallthrough arm is intact in the current tree at
`src/mcp.rs:326`:

```rust
return error_response(id, -32602, &format!("unknown tool: {name}"));
```

— inside `tools_call_result`, reached only after the
`all_tools()` lookup misses, and undisturbed by the
D-squad robustness edits (which added their own
−32602-shape pin at `src/mcp.rs:3777`). The e2e half
is pinned in `tests/wave14_e2e_contracts.rs`:
near-miss spellings of the new names (`fx_set_seeds`,
`sprite_samples`, `particle_set_seeds`,
`graph_key_interps`, `shell_frobnicate`) all earn
−32602 with the byte-exact message and spend no undo.

## Contract (5) — shell tools on stdio: NOT YET CONFORMANT (integrator fix items)

**B1's specified shape** (`docs/wave14/SHELL_CONTROL.md`
§5): one registry on both transports — shell tools
**appear in the stdio `tools/list`**; on stdio their
registered handler is a shared stub
`shell_unavailable` returning an **`isError` tool
result** (never −32602) whose text interpolates the
tool name:

> `{name} needs the live editor: shell tools drive the
> running GUI's shell (workspace, transport, view,
> panels), and the stdio server has no GUI shell.
> Start the editor and connect to its live endpoint
> http://127.0.0.1:32117/mcp instead.`

**As found, no stub exists in `src/`** (zero hits for
`shell_unavailable` / `SHELL_UNAVAILABLE_TEXT`), and
the B-squad files diverge from §5 — and from each
other — in six places. All six are **integrator fix
items for the splice**; B1 §5 is the arbiter this
audit recommends, since it is the only position
written as a transport-level design.

- **F1 — B4 transport tools would EXECUTE on stdio.**
  `shell_tools_transport.rs`'s entry specs register
  real handlers in `mcp_tools::tools()` ("On the stdio
  server the flag is set but nothing advances it",
  `:147`/`:154`). §5 requires the stub for **every**
  shell tool. Fix: register the stub for
  `shell_play` / `shell_pause` / `shell_set_playhead`
  / `shell_get_transport`, or amend §5 — one of the
  two, on record.
- **F2 — B3's refusal wording differs.**
  `shell_tools_state.rs:122-126` proposes
  `"{name} is live-only: it drives the running
  editor's shell and has no meaning for the stdio
  server — call it through the editor's live endpoint
  instead."` — a different text from §5's, and B3's
  registration route (a live shell-command table, not
  `all_tools()`) also differs from §5's one-registry
  rule.
- **F3 — B8's refusal wording differs.**
  `shell_tools_layout.rs:153-154`:
  `SHELL_UNAVAILABLE = "shell control requires the
  live editor endpoint (no GUI shell is attached to
  this transport)"` — the isError **shape** matches
  §5; the text does not.
- **F4 — B5's route contradicts §5's list rule.**
  `shell_tools_view.rs:57-62`: "The stdio server must
  NOT dispatch these tools … Do not add these tools
  to the stdio `all_tools()` registry", with the
  refusal paraphrased as "shell tools need the live
  editor". §5 deliberately lists them on stdio (D2's
  parity diff and client discoverability depend on
  it).
- **F5 — B7 carries a fourth wording.**
  `shell_tools_settings.rs:171-172`:
  `STDIO_SHELL_REFUSAL = "shell tools need the
  editor's live MCP endpoint — the stdio server has
  no GUI shell to drive"`. Right shape, fourth text.
- **F6 — the shell name set itself is unreconciled.**
  B2's `SHELL_TOOL_NAMES` (`src/shell_control.rs:357`)
  proposes seven consolidated names (`shell_transport`,
  `shell_set_view`, `shell_set_selection`,
  `shell_set_theme`, `shell_set_panels`, …) while
  B3–B8 landed 20+ granular names (`shell_play`,
  `shell_view_zoom`, `shell_select_figure`,
  `shell_set_section`, …). B2's own doc names the
  duty: the integrator renames in `shell_control.rs`
  and the `mcp.rs` list **together, and nowhere
  else**. Until that is done, any census of "the
  shell tools" (E9's included) has two answers.

## Observations (not violations)

- **O1 — bare-cast float helpers rely on caller
  discipline.** `aop_req_f32` (`audio_ops.rs:1084`)
  casts without checking; every Wave 14 caller
  audited adds the post-cast `is_finite` check, so no
  handler is non-compliant — but the helper makes the
  next caller one forgotten check away from a
  violation. The sound pattern used elsewhere
  (`ast_as_f32`, `gk_opt_f32`, `fs_arg_f32`,
  `lay_opt_f32`, `finite_f32` in the view file) checks
  the **cast value**, which is what makes the
  `1e300` case refuse. Integrator may want the audio
  helper brought into that shape while the file is
  open.

## PENDING at the freeze (not landed — not audited, not passed)

- **P1 — B9** file/menu shell actions: no file or
  handlers in the tree.
- **P2 — B2's splice itself**: `src/shell_control.rs`
  landed during this audit (queue + `submit` +
  `SHELL_TOOL_NAMES`), but the `main.rs` drain and
  the `mcp.rs` registry stubs are integrator splices;
  every `submit`-based handler (layout family) is
  therefore pre-plumbing.
- **P3 — C5's clip-def media authoring tool**: no
  handler in `src/` (C7's suite funnels the candidate
  names `clip_asset_attach` / `asset_attach_clip`;
  neither exists). C1's `media_items` clip-def
  fallback and C6's asset-census update **did** land
  in `media.rs` and are render-side changes, not
  handlers.
- **P4 — A7's fig3d remainder**: only the
  `fig3_set_camera` amendment landed (audited above);
  no further fig3 handlers.
- **P5 — D2's stdio↔live parity harness**: not in
  `tests/` at the freeze (E10's contract suite does
  not duplicate it; the two files test different
  things — contracts vs transport diffing).
- **P6 — every registry `Tool` entry** for the 61
  handlers above: integrator splice, by the wave's
  hot-file rule. The TOOL ENTRY SPECs were verified
  present beside each handler while auditing.

## Companion tests

`tests/wave14_e2e_contracts.rs` (E10): the unknown-
tool −32602 shape against near-miss spellings of the
new names; unknown-tool calls spend no undo;
notification silence across four message forms;
refused calls to a sample of the new handlers
(fx ×4, particle, audio, graph, sprite, weather,
camera ×2, frame-duration, nudge) spend no undo —
including three `1e300` f32-overflow refusals; and
the positive control that successful strict-family
calls spend exactly one step. Per-tool behaviour
stays with the owning squads' suites.
