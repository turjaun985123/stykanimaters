# Shell control — driving the GUI itself (live only)

> **Merge note for the integrator (B10):** this is the
> shell-tools chapter for `docs/MCP.md`, written to be
> dropped in as a new top-level section after the
> "Transport 2 — live control" section and before
> "Connect a client", with its catalogue table joining
> the Catalogue as a fourth group. It was written while
> the B squad's handlers were still landing in
> parallel: `set_workspace` and `get_shell_state` are
> the roster's own names (B3); the remaining names are
> the squad's working names in the house style and are
> E3's audit to reconcile against the landed registry
> before this text merges. The behavioural claims —
> what each tool drives, the stdio refusal, the
> security posture — are the design's contract, not
> aspirations.

Every tool in the main catalogue edits the project:
figures, frames, keys, media, files. None of them
could touch the application around the project —
which workspace is showing, whether playback is
running, what the shell has selected, how the stage
is being viewed. A user driving the editor by hand
does those things constantly, so an agent limited to
the project tools was never actually driving the
editor. The shell tools close that gap: over the
live-control transport, a client can operate the GUI
shell itself.

## How it works

Shell state lives in the shell, not in the editor
core the other tools share — the workspace is a
`State` field, the sprite selection is panel state,
the view zoom is a shell-side layer. A core tool
handler, which receives only the core, cannot reach
any of it. Shell tools therefore work in two steps:

1. The tool call, dispatched like any other, does not
   edit anything itself. It validates its arguments
   and places a **shell command** on the shell-command
   queue (`src/shell_control.rs`), taking a ticket.
2. On its next tick, the editor's own poll loop — the
   same loop that drains live project requests —
   applies the queued commands to the live shell
   state, in the order they arrived, and writes each
   command's outcome into that ticket's **result
   slot**. The tool call's reply is its own slot's
   contents: success with a small JSON summary of the
   state it applied, or a refusal quoting the
   offending argument (an unknown workspace name, a
   figure index with no figure).

Consequences worth knowing as a client:

- **A frame of latency is normal.** The reply arrives
  after the shell's next poll tick, not inside the
  dispatch. In practice this is the same heartbeat
  the live transport already runs on; a shell call is
  not noticeably slower than a project call.
- **Order is preserved.** Commands apply strictly in
  arrival order, and each result reaches its own
  caller — a refused command between two good ones
  fails alone and disturbs neither.
- **Shell actions are session state, not edits.**
  Like `set_frame` and `set_loop_range`, they are not
  undo steps and do not dirty the project: switching
  workspace, pausing playback or moving a selection
  changes nothing a save would persist, beyond what
  the shell already persists for its own layout
  (the active workspace and the theme are layout
  preferences, and shell tools persist them exactly
  as the equivalent clicks do).

## The tools

### Workspace and shell state (B3)

| Tool | What it drives in the GUI | Required | Optional |
|---|---|---|---|
| `set_workspace` | The workspace chip row: switches the active workspace (`State.workspace`), mounting that workspace's panels and unmounting the others, and persists the choice to the layout preferences — the chip press, by wire. `workspace` is one of the eight persisted names: `Animate`, `Draw`, `Graph`, `Effects`, `Audio`, `3D`, `Export`, `Home`. An unknown name is refused and quotes it. Live only (see "stdio behaviour"). | `workspace` | — |
| `get_shell_state` | Nothing — the read side of this whole section. A snapshot of the shell as it stands: the active workspace, the transport (playing, current frame, loop-playback flag, playback speed), the theme (dark/light), the side columns' and timeline's visibility, and every selection the shell keeps (figure, node, sprite, stroke, particle, FX clip). Read-only; the shell counterpart of `get_editor_status`, which reports the core's half of that picture. Live only — on stdio there is no shell to report. | — | — |

### Transport (B4)

| Tool | What it drives in the GUI | Required | Optional |
|---|---|---|---|
| `set_transport` | The transport row's transport state, held by the shell: `play` and `pause` flip the same playing flag the chrome Play/Pause button flips, which the poll loop observes — playback and its audio start and stop exactly as if the button had been pressed; `toggle` flips whichever state is current; `stop` pauses and returns the playhead to frame 0. Session state, not an undo step. (The core-level `set_playback` tool drives the same flag through the project path, including over stdio; this tool exists so live clients drive the shell's own transport surface — the one that also carries the loop and speed state below — in one place.) Live only. | `action` | — |
| `set_loop_playback` | The transport row's loop toggle (`State.loop_playback`): with looping on, the playhead wraps at the end (or at the loop range); off, poll stops playback on the last frame. Shell-only state — no project tool can reach it. Live only. | `enabled` | — |
| `set_playback_speed` | The transport row's speed control (`State.playback_speed`): a finite, positive multiplier on the project fps — 1.0 is the project's own rate, 2.0 double, 0.5 half. Non-finite or non-positive values are refused and quoted. Live only. | `speed` | — |
| `scrub` | The playhead, by shell hand: moves to a 0-based `frame` exactly as dragging the timeline playhead does (the core's `set_frame` moves the same playhead through the project path; `scrub` is the shell transport surface's own verb). An out-of-range frame is refused and quoted — a shell command must never fail silently the way a bare playhead write can. Does not change the playing state. Live only. | `frame` | — |

### View (B5)

| Tool | What it drives in the GUI | Required | Optional |
|---|---|---|---|
| `set_view` | The stage's view layer only: `zoom_in` / `zoom_out` step the view zoom, `fit` frames the stage content, `centre` centres it, `actual_size` returns to 1:1. This is the mouse wheel's layer (the shell's view transform) — it never authors a camera key, never writes the project's camera, and is not an undo step, the Wave 5 doctrine that wheel zoom is view navigation while the Camera panel authors. For camera moves that belong in the project, use `set_camera_key_here` and its family. Live only. | `action` | `factor` |

### Selection (B6)

| Tool | What it drives in the GUI | Required | Optional |
|---|---|---|---|
| `set_shell_selection` | The shell's selection fields, in one partial update — pass only the selections to change. `figure` / `node` write the core's selection (the same fields `set_selection` writes: selecting a figure clears the node selection); `sprite` writes the Sprites panel's selection, which no core tool can reach; `stroke`, `particle` and `fx_clip` write the shell's stage/panel selections of the same names. Indices are 0-based; `null` clears that selection. An out-of-range index is refused, quotes the index, and leaves every selection untouched. Not an undo step. Live only. | — | `figure`, `node`, `sprite`, `stroke`, `particle`, `fx_clip` |

### Theme and settings (B7)

| Tool | What it drives in the GUI | Required | Optional |
|---|---|---|---|
| `set_theme` | The Settings card's theme switch: `dark` or `light`, applied to the running shell and persisted to the layout preferences exactly as the switch persists it (the poll loop's existing save path). Live only. | `theme` | — |
| `get_shell_settings` | Nothing — reads the shell's persisted layout preferences as the shell holds them (active workspace, theme, timeline collapsed, sidebars, column widths, whether live MCP starts with the editor), so a client can see the settings surface whole before changing any of it. Read-only. Live only. | — | — |
| `set_shell_setting` | One named layout-preference-backed shell setting by key, with the same clamping and validation the GUI applies (column widths are clamped to their allowed range; non-finite values are refused and quoted). Settings the shell owns outside the layout preferences are not this tool's to write; an unknown key is refused and quoted. Live only. | `key`, `value` | — |

### Panels (B8)

| Tool | What it drives in the GUI | Required | Optional |
|---|---|---|---|
| `set_panels` | Panel visibility the way the shell itself controls it: `left` / `right` show or hide the side columns (the View menu's toggles), `timeline_collapsed` collapses or restores the bottom timeline strip, and `section` with `collapsed` folds or unfolds one named panel section. Visibility still obeys the workspace rules — a section its workspace does not mount stays unmounted whatever this tool says, and the tool reports that rather than pretending. Live only. | — | `left`, `right`, `timeline_collapsed`, `section`, `collapsed` |

### File and menu actions (B9)

| Tool | What it drives in the GUI | Required | Optional |
|---|---|---|---|
| `open_recent_project` | The File menu's recent-projects list: opens a recent project by its position in the list (`index`, 0-based) or by `path`, through the shell's own open path — including its unsaved-work confirmation behaviour, so an open that the GUI would ask about is refused with the reason rather than silently discarding work. Replaces the session's project the way the menu item does. Live only. | — | `index`, `path` |

Actions whose GUI form is a **system file dialog**
(the Open and Save As pickers) deliberately have no
shell tool: a dialog is a conversation with the
person at the machine, and faking one by wire would
be a false control. Their by-path equivalents
already exist and work on both transports —
`load_project` / `import_*` to bring a file in,
`save_project` and the export tools to write one out
(see the File IO catalogue). A shell client that
needs a dialog's outcome should use those and say
so.

## stdio behaviour

Shell tools are registered on both transports, so a
stdio client's `tools/list` shows the same catalogue
as a live one. Calling one over stdio is a different
matter: the stdio server hosts a bare editor core
and no GUI — there is no workspace to switch, no
transport row, no shell selection. The call therefore
fails honestly, as a **tool result with `isError`
set** and text saying the tool needs the live editor
("…shell control requires the live editor…"), never
as a protocol error: the tool exists, and `-32602`
stays reserved for names that do not. Nothing is
queued for a shell that might appear later; the
command simply does not run. Everything else about
the stdio server is unchanged — the project tools
serve exactly as before, including `set_playback`
and `set_selection`, which act on the hosted core.

## Security posture

Shell control changes what the live endpoint can
reach, not how it is reached, and this chapter keeps
the posture stated as plainly as the transport
chapter does. The live endpoint listens on the
**loopback interface only** (`127.0.0.1`, port
32117): nothing off the machine can connect to it.
It has **no password and no other authentication** —
that was true of the project tools and remains true
now. The trust boundary is therefore the machine
itself: any process running as the user on their own
computer can already read the user's files, and this
endpoint asks no more trust than that; it exists so
the user's own agent, on the user's own machine, can
operate the editor the user is looking at. It is
only listening at all while the editor is running
with Live MCP enabled (the Settings toggle, persisted
as a layout preference). Shell tools add no new
network surface, no new listener and no new thread:
commands execute on the editor's UI thread, inside
the poll loop that already drains live requests. A
user who does not want their editor driven by wire
turns Live MCP off, and the shell tools — like every
live-only capability — cease to exist with it.
