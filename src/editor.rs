//! Editor state and operations, independent of GPUI: selection,
//! dragging (root move / FK aim / two-bone IK), frame operations,
//! undo snapshots, save/load and export triggers. The GPUI layer in
//! `app.rs` only translates mouse events and buttons into these calls.

use crate::export::{
    export_gif as render_gif, export_mp4 as render_mp4, export_png_sequence,
    export_spritesheet_pack as render_sheet, export_webm as render_webm,
};
use crate::model::{
    aim_node, apply_ik, dist, evaluate, norm_deg, FigureDef, NodeDef, Project, SetRestMode, V2,
};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DragKind {
    Root,
    Ik(usize),
    Aim(usize),
    Camera,
    /// Wave Y (Y8, P-10): setup-mode drags — the Ik/Aim
    /// solvers driven against the figure's rest angles.
    SetupIk(usize),
    SetupAim(usize),
}

/// Wave 7: what a draw-mode drag does — paint a stroke with the
/// active brush preset, or erase stroke points around the pointer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DrawTool {
    Brush,
    Eraser,
}

/// Roadmap Wave 1 (RM24, P-14 — A15 H1): one undoable
/// edit, as the history stacks store it: WHAT happened
/// (the label the History panel lists and the app bar's
/// "Undo {label}" button speaks) plus the whole project
/// as it was BEFORE the edit (the payload `undo()`
/// restores — the pre-RM24 `Vec<String>` stack's strings,
/// unchanged in form and cost). Runtime only, like the
/// stacks themselves: undo never crosses a project swap
/// and nothing here is serialised.
#[derive(Clone)]
pub struct HistoryEntry {
    pub label: String,
    pub project: String,
}

/// Roadmap Wave 1 (RM24, P-14 — A15 R4): the project's
/// truth about its NAMED file, split from the recovery
/// snapshot's story (which `edits_since_autosave` keeps
/// telling on its own segment): a project can be fully
/// covered by the recovery snapshot and still never have
/// been saved anywhere the user chose.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveState {
    /// No successful named save (and no open from a
    /// named file) since this project entered the core:
    /// it exists only in the session and the recovery
    /// snapshot.
    NeverSaved,
    /// The named file holds exactly the current edit
    /// baseline — no counted edit since the save/open.
    Saved,
    /// Counted edits have landed since the last named
    /// save/open; the file on disk is older than the
    /// session.
    ModifiedSinceSave,
}

pub struct EditorCore {
    pub project: Project,
    pub current: usize,
    pub selected_figure: usize,
    pub selected_node: Option<usize>,
    /// Roadmap Wave 2 (W2-01): the marquee figure
    /// set — the figures the stage's last marquee
    /// committed, sorted figure indices. Session
    /// state like the single selection beside it:
    /// never serialised, not an edit, and every
    /// consumer re-validates indices against the
    /// live project.
    pub selected_figures: Vec<usize>,
    pub dragging: Option<DragKind>,
    pub onion: bool,
    pub playing: bool,
    /// When true, canvas drags move the camera instead of posing.
    pub camera_mode: bool,
    camera_last: Option<V2>,
    /// Wave P (P6): the pre-gesture poses of a stage pose
    /// drag — one `(frame, pose)` baseline per frame the
    /// gesture has written, the grab frame's captured in
    /// `begin_drag` before the first `drag_to` writes and
    /// any later frame's lazily in `drag_to` (Wave AC,
    /// AC4: a mid-gesture frame step — the arrow-key
    /// `goto` does not end the drag — retargets the
    /// chase, and the newly targeted frame's baseline is
    /// taken before its first write). `end_drag` compares
    /// each written frame against its OWN baseline to
    /// tell a real move from a press that changed nothing
    /// and post the gesture-end recap only for the
    /// former; comparing the release frame against the
    /// grab frame's baseline (the pre-AC4 shape) posted
    /// the recap for presses that never touched the
    /// release frame and swallowed it for real edits
    /// whenever the two frames' poses happened to agree.
    drag_poses_before: Vec<(usize, crate::model::Pose)>,
    /// Wave Y (Y8): the grabbed figure's rest angles
    /// captured at a setup drag's start, for
    /// `end_drag`'s recap decision — `drag_poses_before`'
    /// setup twin. One baseline suffices here: rest
    /// angles are frame-independent data, so a
    /// mid-gesture step cannot cross its comparison
    /// (Wave AC, AC4).
    drag_rest_before: Option<Vec<f32>>,
    /// Wave P (P6): the camera in effect when a camera drag
    /// began — `end_drag`'s comparison baseline for its pan
    /// recap, mirroring `drag_poses_before` (Wave AC,
    /// AC4: per-frame baselines for the same reason —
    /// the grab frame's `camera_at` at capture, then
    /// lazily per stepped-to frame in `drag_to`).
    drag_cameras_before: Vec<(usize, (f32, f32, f32))>,
    pub status: String,
    undo: Vec<HistoryEntry>,
    redo: Vec<HistoryEntry>,
    /// Roadmap Wave 1 (RM24, P-14): true only in the
    /// instant after a snapshot push — the licence
    /// [`EditorCore::snapshot_coalesced`] needs to fold a
    /// continuing stepper/cycler/scrub run into the entry
    /// on top of the stack instead of pushing another.
    /// Cleared by `undo`, `redo` and every wholesale
    /// project replacement, so a coalesced write can never
    /// merge across a history navigation or a swap.
    history_coalesce: bool,
    /// Roadmap Wave 1 (RM24, P-14): `edit_count` at the
    /// last successful NAMED save (or open from a named
    /// file) — the baseline [`EditorCore::save_state`]
    /// reads. `None` while this project has never met a
    /// named file in this session (fresh, recovered or
    /// imported): the strip's dot then tells the truth
    /// that the recovery snapshot alone cannot. Runtime
    /// only, like the other counters: undo snapshots
    /// cover the project, not this.
    last_save_count: Option<u64>,
    /// Wave 9: edits counted at the snapshot choke points
    /// over this core's lifetime (plus the one remaining
    /// `mark_dirty` class — see its doc). Since Roadmap
    /// Wave 1 (RM24, P-14) every project mutation snapshots,
    /// so this counts every edit; before RM24 a few
    /// appearance cyclers (segment colour/shape, figure
    /// opacity) mutated unsnapshotted by design and were
    /// invisible here.
    edit_count: u64,
    /// Wave 9: `edit_count` at the last successful autosave.
    last_autosave_count: u64,
    /// Wave 11 (E9): bumped on every wholesale project swap
    /// (`note_project_swapped` and the MCP io replace path) so
    /// the shell's poll can tell "the project under the session
    /// state changed" and clear index-based panel selections
    /// that would otherwise point into the new project.
    project_swaps: usize,
    /// Roadmap Wave 1 (RM6, F-5): the project revision — a
    /// monotone counter bumped at exactly the choke points
    /// that count an edit for autosave: the snapshot family
    /// (every undoable mutation — since RM24, P-14, that is
    /// every project mutation), `mark_dirty()` (the one
    /// remaining un-undoable class: session bookkeeping
    /// that follows an external file event), and the
    /// wholesale-replacement funnels (`undo` / `redo`
    /// restores, `note_project_swapped`,
    /// `note_project_replaced`). The house rule those sites
    /// already enforce — a mutation either snapshots exactly
    /// once or marks dirty, never neither — is what makes
    /// this counter complete: the project content cannot
    /// change without the revision moving. Unlike
    /// `edit_count` it is NEVER reset (a swap restarts the
    /// autosave baseline, but a revision that went backwards
    /// could collide with a cache entry cut against the
    /// previous project). Runtime only, like `project_swaps`:
    /// undo snapshots cover the project, not this.
    revision: u64,
    /// Roadmap Wave 1 (RM6, F-5): the evaluate-once cache —
    /// `(revision, frame bits, evaluated frame)` entries,
    /// newest last, capped at a handful of frames of the
    /// CURRENT revision (entries for older revisions are
    /// unreachable by construction and dropped on the next
    /// miss). The key is the Wave L lesson made structural:
    /// frame alone is never a valid cache key in this
    /// editor (a frame-keyed subtitle cache once served
    /// stale cues under a stationary playhead); revision +
    /// frame is, because evaluation is a pure function of
    /// (project, frame) and the revision moves on every
    /// project change. Session-only derived data: never
    /// serialised, and a miss simply re-evaluates.
    eval_cache: std::cell::RefCell<Vec<(u64, u32, std::rc::Rc<crate::graph::ChannelResult>)>>,
    /// Wave P (P14): stored paths of the current project's
    /// unmuted audio clips whose files did not exist at the
    /// last refresh — one entry per missing clip, in timeline
    /// order (the classification of
    /// [`crate::audio::missing_audio_clips`]: muted clips are
    /// intentionally silent, so their files' absence loses
    /// nothing and is not reported). Refreshed by
    /// `refresh_missing_audio` wherever
    /// the project is wholesale replaced (the swap funnels
    /// and undo/redo), so the report always describes the
    /// project actually in the core. Runtime only, like
    /// `project_swaps`: undo snapshots cover the project,
    /// not this report. It is a report, not a repair — the
    /// clips themselves are never dropped or rewritten.
    missing_audio: Vec<String>,
    /// Wave W (W3): the MCP progress sink for the
    /// `tools/call` currently being dispatched against
    /// this core, installed by the MCP router for the
    /// call's duration only and restored afterwards (see
    /// [`crate::mcp::ProgressSink`]). Runtime only, like
    /// `project_swaps`: undo snapshots cover the project,
    /// not this. `None` outside MCP dispatch — and during
    /// it too, unless the client passed a progress token
    /// AND the transport can stream notifications — so
    /// the GUI's own exports and direct handler calls in
    /// tests never see a sink.
    pub progress_sink: Option<crate::mcp::ProgressSink>,
    clipboard: Option<crate::model::Frame>,
    /// Wave 9: deep-copied frames held by `copy_range` for
    /// `paste_range`. Distinct from the single-frame `clipboard`.
    frame_clipboard: Vec<crate::model::Frame>,
    pub playhead: f32,
    /// Wave 7: inclusive frame range the playhead loops inside,
    /// when set. Runtime only, like `solo`: undo snapshots cover
    /// the project, not this.
    pub loop_range: Option<(usize, usize)>,
    /// Roadmap Wave 2 (W2-40): ping-pong travel direction —
    /// `true` while a ping-pong pass is travelling backwards.
    /// Runtime only, like `loop_range`: undo snapshots cover
    /// the project, not this. Read and written only by
    /// [`EditorCore::advance_ping_pong`]; every other playhead
    /// writer (`goto`, `advance`, seeks, frame ops) leaves it
    /// alone, so a seek mid-bounce keeps the direction of
    /// travel instead of silently restarting the pass.
    pub play_reverse: bool,
    pub show_paths: bool,
    pub light_theme: bool,
    pub library: Vec<(String, FigureDef)>,
    pub library_index: usize,
    /// Draw mode: canvas drags paint brush strokes on this frame.
    pub draw_mode: bool,
    /// Wave Y (Y8, P-10): setup (rigging) mode — while
    /// on, the stage draws every figure at its rest pose
    /// and stage drags drive the solvers against rest
    /// angles instead of the current frame's pose (see
    /// `begin_drag`'s setup branch). Runtime-only, like
    /// `draw_mode`; the figure inspector's toggle is its
    /// only writer.
    pub setup_mode: bool,
    drawing: bool,
    /// Wave 7: the active draw tool (brush or eraser).
    pub draw_tool: DrawTool,
    /// Wave 7: index into `paint::brush_library` — the active
    /// brush preset (also sizes the eraser).
    pub brush_index: usize,
    /// Strokes changed so far by the current eraser drag, for the
    /// status line when the drag ends.
    erase_changed: usize,
    /// How many previous frames the onion skin shows (1..=3).
    pub onion_frames: u8,
    pub project_files: Vec<PathBuf>,
    pub project_index: usize,
    /// UI language for toolbar labels (see i18n.rs).
    /// Defaults to English here; the shell's `init`
    /// seeds it from the persisted `LayoutPrefs.lang`
    /// and the shell's `poll` writes changes back to
    /// that field (Roadmap F-9), so a language picked
    /// in Settings survives a relaunch. The core itself
    /// never reads or writes the prefs file.
    pub lang: crate::i18n::Lang,
    /// Active 3D editing session (fig3d), if any. Runtime only:
    /// undo snapshots cover the project, not this state.
    pub fig3: Option<crate::fig3d::Fig3State>,
    /// Wave 5: solo figure (layers) — display/edit focus.
    pub solo: Option<usize>,
    /// Wave 7: active movieclip editing session, if any.
    pub clip_edit: Option<ClipEditSession>,
}

/// Runtime state of a movieclip editing session (wave 7): which
/// clip is being edited and which of its frames is current.
/// Runtime only — undo snapshots cover the project, not this
/// state (same convention as `fig3`).
///
/// Wave 10 (D5): the session also carries the clip's `name`
/// from when the session was entered. Undo/redo restore the
/// project only, so the stored index can come to name a
/// DIFFERENT clip after a history jump (undoing a clip's
/// creation, redoing a deletion); `undo`/`redo` compare the
/// name and drop a session whose clip is gone or no longer
/// the clip at its index, instead of letting the session
/// silently retarget. Two clips sharing one name remain
/// indistinguishable — the residual limit of an index-based
/// session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClipEditSession {
    pub clip: usize,
    pub frame: usize,
    pub name: String,
}

const FIGURE_COLORS: [u32; 6] = [0xE8E8EC, 0xFF5D5D, 0x5DB2FF, 0x7BE495, 0xFFC65D, 0xC792FF];

/// The opacity a newly recorded stroke carries from a brush
/// opacity: clamped to [0.0, 1.0]; a NaN brush opacity (no
/// preset produces one) falls back to fully opaque rather
/// than recording a value renderers cannot apply.
fn recorded_stroke_opacity(brush_opacity: f32) -> f32 {
    if brush_opacity.is_nan() {
        1.0
    } else {
        brush_opacity.clamp(0.0, 1.0)
    }
}

/// Where a tracked clip index (a figure's `clip` reference,
/// the clip-edit session's clip) lands when clip `deleted`
/// is removed: before the gap untouched, after it shifted
/// down by one, the deleted clip itself mapped to `None` —
/// tracking ends. The clips panel keeps the same mapping
/// for its own tracked indices (Wave 11 port, C6 §5).
fn clip_target_after_delete(target: Option<usize>, deleted: usize) -> Option<usize> {
    let t = target?;
    if t < deleted {
        Some(t)
    } else if t > deleted {
        Some(t - 1)
    } else {
        None
    }
}

/// Where a tracked clip index lands when clips `a` and `b`
/// trade places: the two swap, every other index untouched.
fn clip_target_after_swap(target: Option<usize>, a: usize, b: usize) -> Option<usize> {
    target.map(|t| {
        if t == a {
            b
        } else if t == b {
            a
        } else {
            t
        }
    })
}

/// Retarget guard (Wave X, P-1 step 2): may `clip` play on
/// `fig`? Clips were the one reuse path with no rig check —
/// `apply_clips` clones clip frame poses into the figure's
/// pose row unchecked, and `model::evaluate` indexes
/// `pose.angles[i]` directly, so a clip authored for a
/// different-sized rig corrupts the row and panics the next
/// render. The guard follows the pose paths' definition
/// (copy pose / pose apply refuse on angle-count mismatch):
/// the clip's def must carry exactly as many nodes as the
/// figure's def, and every frame pose in the clip must carry
/// exactly that many angles — a clip whose frames do not fit
/// its own def (hand-edited, mid-repair) is not retargetable
/// anywhere. An empty clip with a matching def passes: there
/// is nothing to misfit, and playback skips empty clips.
pub fn clip_compatible(
    fig: &crate::model::FigureInstance,
    clip: &crate::model::ClipDef,
) -> bool {
    let n = fig.def.nodes.len();
    clip.def.nodes.len() == n
        && clip.frames.iter().all(|p| p.angles.len() == n)
}

/// Remap a figure's state keys after clip `deleted` is
/// removed, mirroring `clip_target_after_delete` per key:
/// keys naming the deleted clip are dropped (their state is
/// gone with the clip), keys after it shift down by one.
fn state_keys_after_delete(states: &mut Vec<crate::model::StateKey>, deleted: usize) {
    states.retain(|k| k.clip != deleted);
    for k in states.iter_mut() {
        if k.clip > deleted {
            k.clip -= 1;
        }
    }
}

/// Remap a figure's state keys after clips `a` and `b` trade
/// places, mirroring `clip_target_after_swap` per key.
fn state_keys_after_swap(states: &mut Vec<crate::model::StateKey>, a: usize, b: usize) {
    for k in states.iter_mut() {
        if let Some(clip) = clip_target_after_swap(Some(k.clip), a, b) {
            k.clip = clip;
        }
    }
}

impl EditorCore {
    pub fn new() -> Self {
        EditorCore {
            project: Project::new_demo(),
            current: 0,
            selected_figure: 0,
            selected_node: None,
            selected_figures: Vec::new(),
            dragging: None,
            onion: true,
            playing: false,
            camera_mode: false,
            camera_last: None,
            drag_poses_before: Vec::new(),
            drag_rest_before: None,
            drag_cameras_before: Vec::new(),
            status: "Drag a joint to pose. Hands and feet use IK; drag the hips to move.".into(),
            undo: Vec::new(),
            redo: Vec::new(),
            history_coalesce: false,
            last_save_count: None,
            edit_count: 0,
            last_autosave_count: 0,
            project_swaps: 0,
            revision: 0,
            eval_cache: std::cell::RefCell::new(Vec::new()),
            missing_audio: Vec::new(),
            progress_sink: None,
            clipboard: None,
            frame_clipboard: Vec::new(),
            playhead: 0.0,
            loop_range: None,
            play_reverse: false,
            show_paths: false,
            light_theme: false,
            library: Self::load_library(),
            library_index: 0,
            draw_mode: false,
            setup_mode: false,
            drawing: false,
            draw_tool: DrawTool::Brush,
            brush_index: 0,
            erase_changed: 0,
            onion_frames: 1,
            project_files: Self::list_project_files(),
            project_index: 0,
            lang: crate::i18n::Lang::En,
            fig3: None,
            solo: None,
            clip_edit: None,
        }
    }

    fn list_project_files() -> Vec<PathBuf> {
        let dir = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("projects");
        // Wave 12 (B3): the listing itself lives in
        // `projects::list_project_files` (shell-owned files
        // filtered there) — one implementation, so this
        // selector listing and the refresh seam's cannot drift.
        crate::projects::list_project_files(&dir)
    }

    /// The on-disk text of a saved library file: the figure
    /// definition's own JSON plus a top-level `"name"` field
    /// holding the figure's name verbatim. The name rides
    /// INSIDE the file — the pattern the pose library
    /// (`./library/poses`) and the 3D library
    /// (`./library3d`) already use — because the file's
    /// NAME is a slug (spaces→`-`, lowercased) and cannot
    /// carry the display name: a figure saved as "Styk"
    /// used to come back listed and inserted as "styk"
    /// (Wave V, V1). The extra field is invisible to the
    /// definition reader: `FigureDef` ignores unknown
    /// fields, so the file still parses as a bare
    /// definition everywhere it did before, and an older
    /// build reading a newer file simply names it by its
    /// stem, exactly as it always did.
    fn library_file_text(def: &FigureDef, name: &str) -> Result<String, serde_json::Error> {
        let mut value = serde_json::to_value(def)?;
        if let Some(obj) = value.as_object_mut() {
            obj.insert(
                "name".to_string(),
                serde_json::Value::String(name.to_string()),
            );
        }
        serde_json::to_string_pretty(&value)
    }

    /// The display name stored inside a library file's
    /// text (see [`Self::library_file_text`]), or `None`
    /// when the file carries none — every file written
    /// before the stored name existed, or a hand-written
    /// definition. An empty stored name counts as none:
    /// it could never be displayed or addressed usefully.
    fn library_stored_name(text: &str) -> Option<String> {
        let value: serde_json::Value = serde_json::from_str(text).ok()?;
        value
            .get("name")?
            .as_str()
            .filter(|n| !n.is_empty())
            .map(|n| n.to_string())
    }

    /// The local figure library: JSON figure definitions from
    /// `./library/`, plus three built-ins so it is never empty.
    /// A saved entry's display name is the name stored
    /// inside its file (see [`Self::library_file_text`]);
    /// files written before the stored name existed carry
    /// none and are named by their file stem, as they
    /// always were — an existing library loads with the
    /// names its user already sees, nothing renamed,
    /// moved or rewritten.
    fn load_library() -> Vec<(String, FigureDef)> {
        let mut lib: Vec<(String, FigureDef)> = vec![
            ("Built-in: Humanoid".into(), FigureDef::humanoid()),
            ("Built-in: Quadruped".into(), FigureDef::quadruped()),
            ("Built-in: Mini".into(), FigureDef::simple()),
        ];
        let dir = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("library");
        if let Ok(entries) = std::fs::read_dir(&dir) {
            let mut files: Vec<PathBuf> = entries
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.extension().map(|x| x == "json").unwrap_or(false))
                .collect();
            files.sort();
            for f in files {
                if let Ok(text) = std::fs::read_to_string(&f) {
                    if let Ok(def) = serde_json::from_str::<FigureDef>(&text) {
                        let name = Self::library_stored_name(&text).unwrap_or_else(|| {
                            f.file_stem()
                                .map(|n| n.to_string_lossy().into_owned())
                                .unwrap_or_else(|| "Figure".into())
                        });
                        lib.push((name, def));
                    }
                }
            }
        }
        lib
    }

    pub fn frame_count(&self) -> usize {
        self.project.frames.len()
    }

    /// Joint positions of figure `fi` in frame `frame`.
    pub fn positions(&self, frame: usize, fi: usize) -> Vec<V2> {
        let fig = &self.project.figures[fi];
        evaluate(&fig.def, &self.project.frames[frame].poses[fi])
    }

    /// The undo choke point: every undoable project
    /// mutation records exactly one history entry here
    /// BEFORE it writes. Unlabelled by default — the
    /// generic "Edit" the History panel shows for doors
    /// that have not taken a label yet; labelled doors
    /// call [`EditorCore::snapshot_labeled`] directly.
    pub fn snapshot(&mut self) {
        self.snapshot_labeled("Edit");
    }

    /// Roadmap Wave 1 (RM24, P-14 — A15 H1): record one
    /// undoable edit under `label` — the name the History
    /// panel lists and the "Undo {label}" surfaces speak.
    /// The payload is the whole project serialised BEFORE
    /// the caller's write (the stacks' standing form);
    /// the push caps the stack at 60 (the oldest entry
    /// evicts silently, as before), clears redo, and
    /// counts the edit for the autosave cadence and the
    /// revision (the RM6 choke-point rule).
    pub fn snapshot_labeled(&mut self, label: &str) {
        if let Ok(s) = serde_json::to_string(&self.project) {
            self.undo.push(HistoryEntry {
                label: label.to_string(),
                project: s,
            });
            if self.undo.len() > 60 {
                self.undo.remove(0);
            }
            self.redo.clear();
            // Wave 9: every recorded undo step is one counted
            // edit for the autosave cadence.
            self.edit_count += 1;
            // Roadmap Wave 1 (RM6, F-5): the same choke point
            // moves the project revision — every undoable
            // mutation passes through here exactly once.
            self.revision += 1;
            self.history_coalesce = true;
        }
    }

    /// Roadmap Wave 1 (RM24, P-14 — A15 H3): record one
    /// undoable edit under `label`, folding a CONTINUING
    /// run of the same edit into the entry already on top
    /// of the stack instead of pushing another: a stepper
    /// held down, a cycler pressed repeatedly, a scrub's
    /// tenth Move — the run costs ONE undo step, which
    /// restores the state from before the run began (the
    /// board's "one snapshot per project mutation,
    /// gesture-coalesced"). The fold is licensed only in
    /// the instant after a push (`history_coalesce`) and
    /// only into a same-label top entry, so a different
    /// edit — or any undo/redo/swap — always starts a
    /// fresh entry. A folded write still counts as an
    /// edit (autosave cadence, revision): the project
    /// really changed again, it just already has its
    /// undo entry.
    pub fn snapshot_coalesced(&mut self, label: &str) {
        if self.history_coalesce
            && self.undo.last().map(|e| e.label == label).unwrap_or(false)
        {
            self.edit_count += 1;
            self.revision += 1;
            return;
        }
        self.snapshot_labeled(label);
    }

    /// Roadmap Wave 1 (RM24, P-14): take back the entry a
    /// `snapshot_coalesced` push just made — the slider
    /// gesture that ends where it began (cancelled, or
    /// released on its press value) leaves no phantom
    /// step behind: the caller restores the press value
    /// itself, this drops the entry and un-counts the
    /// edit. The revision is NOT rewound (RM6: it never
    /// goes backwards; a spare bump only costs a cache
    /// re-evaluation). A no-op unless the coalesce
    /// licence is live and the top entry carries `label`.
    pub fn rollback_coalesced(&mut self, label: &str) {
        if self.history_coalesce
            && self.undo.last().map(|e| e.label == label).unwrap_or(false)
        {
            self.undo.pop();
            self.history_coalesce = false;
            self.edit_count = self.edit_count.saturating_sub(1);
        }
    }

    pub fn redo(&mut self) {
        if let Some(entry) = self.redo.pop() {
            if let Ok(current) = serde_json::to_string(&self.project) {
                self.undo.push(HistoryEntry {
                    label: entry.label.clone(),
                    project: current,
                });
            }
            if let Ok(p) = serde_json::from_str::<Project>(&entry.project) {
                self.project = p;
                self.current = self.current.min(self.project.frames.len().saturating_sub(1));
                // Wave 13 (B2): mirror undo's clamp — a
                // redone project may hold fewer figures
                // than the selection names, and every
                // reader of `selected_figure` assumes it
                // is in bounds. (A parity bug fix only;
                // the swap semantics themselves are
                // untouched.)
                self.selected_figure = self
                    .selected_figure
                    .min(self.project.figures.len().saturating_sub(1));
                let session_dropped = self.repair_clip_session_after_history();
                self.refresh_missing_audio();
                // Roadmap Wave 1 (RM6, F-5): a restore replaces
                // the project wholesale, so the revision moves —
                // never backwards, even though the restored
                // content may equal an older state.
                self.revision += 1;
                self.history_coalesce = false;
                // Roadmap Wave 1 (RM24, P-14 — A15 H6): a
                // restore that had to drop the clip-edit
                // session says so in the recap instead of
                // the bare word.
                self.status = if session_dropped {
                    "Redone. (Clip session closed — its clip is gone.)".into()
                } else {
                    "Redone.".into()
                };
            }
        } else {
            self.status = "Nothing to redo.".into();
        }
    }

    pub fn undo(&mut self) {
        if let Some(entry) = self.undo.pop() {
            if let Ok(current) = serde_json::to_string(&self.project) {
                self.redo.push(HistoryEntry {
                    label: entry.label.clone(),
                    project: current,
                });
            }
            if let Ok(p) = serde_json::from_str::<Project>(&entry.project) {
                self.project = p;
                self.current = self.current.min(self.project.frames.len().saturating_sub(1));
                self.selected_figure = self
                    .selected_figure
                    .min(self.project.figures.len().saturating_sub(1));
                let session_dropped = self.repair_clip_session_after_history();
                self.refresh_missing_audio();
                // Roadmap Wave 1 (RM6, F-5): the revision moves
                // on a wholesale restore, as in `redo`.
                self.revision += 1;
                self.history_coalesce = false;
                // Roadmap Wave 1 (RM24, P-14 — A15 H6): as
                // `redo` — the dropped-session case speaks.
                self.status = if session_dropped {
                    "Undone. (Clip session closed — its clip is gone.)".into()
                } else {
                    "Undone.".into()
                };
            }
        } else {
            self.status = "Nothing to undo.".into();
        }
    }

    /// Wave 10 (D5): after an undo/redo project restore, a
    /// live clip-edit session survives only if the clip at
    /// its index is still the clip it was entered on (the
    /// name captured in the session). A session whose clip
    /// vanished or was replaced at its index is dropped —
    /// editing would otherwise continue into a different
    /// clip while the panel markers followed the index.
    /// The undo/redo status stands; the panel simply shows
    /// no session, and the next session op reports the
    /// no-session case as usual.
    ///
    /// Roadmap Wave 1 (RM24, P-14 — A15 H6): returns
    /// whether a session was actually dropped, so the
    /// undo/redo recaps can SAY so instead of the bare
    /// "Undone." — the swap funnels ignore the answer
    /// (their own status lines already name the bigger
    /// event).
    fn repair_clip_session_after_history(&mut self) -> bool {
        if let Some(session) = &self.clip_edit {
            let alive = self
                .project
                .clips
                .get(session.clip)
                .map(|c| c.name == session.name)
                .unwrap_or(false);
            if !alive {
                self.clip_edit = None;
                return true;
            }
        }
        false
    }

    /// Number of edits that can currently be undone (the undo
    /// stack depth, capped at 60 by `snapshot`).
    pub fn undo_depth(&self) -> usize {
        self.undo.len()
    }

    /// Number of edits that can currently be redone.
    pub fn redo_depth(&self) -> usize {
        self.redo.len()
    }

    /// Roadmap Wave 1 (RM24, P-14): the label of the edit
    /// the next `undo()` would revert — what the app bar's
    /// "Undo {label}" button and the History panel's
    /// current-position row speak. `None` at depth 0.
    pub fn undo_label(&self) -> Option<&str> {
        self.undo.last().map(|e| e.label.as_str())
    }

    /// Roadmap Wave 1 (RM24, P-14): the label of the edit
    /// the next `redo()` would re-apply. `None` when the
    /// redo stack is empty.
    pub fn redo_label(&self) -> Option<&str> {
        self.redo.last().map(|e| e.label.as_str())
    }

    /// Roadmap Wave 1 (RM24, P-14): the undo stack's
    /// labels, oldest first — the History panel's rows
    /// below the current position (it reverses them for
    /// its newest-first list).
    pub fn undo_labels(&self) -> Vec<String> {
        self.undo.iter().map(|e| e.label.clone()).collect()
    }

    /// Roadmap Wave 1 (RM24, P-14): the redo stack's
    /// labels, oldest first — the LAST entry is the edit
    /// the next `redo()` re-applies.
    pub fn redo_labels(&self) -> Vec<String> {
        self.redo.iter().map(|e| e.label.clone()).collect()
    }

    /// Roadmap Wave 1 (RM24, P-14): jump `steps` entries
    /// through history in one press (the History panel's
    /// row clicks): negative undoes, positive redoes,
    /// clamped at the stacks' ends. Each step is a full
    /// `undo()`/`redo()` restore (repairs included); only
    /// the final recap speaks, in the single-step
    /// vocabulary extended with the count.
    pub fn history_jump(&mut self, steps: i32) {
        if steps == 0 {
            return;
        }
        let mut done = 0usize;
        if steps < 0 {
            while done < steps.unsigned_abs() as usize && self.undo_depth() > 0 {
                self.undo();
                done += 1;
            }
            self.status = if done == 0 {
                "Nothing to undo.".into()
            } else if done == 1 {
                "Undone.".into()
            } else {
                format!("Undid {done} steps.")
            };
        } else {
            while done < steps as usize && self.redo_depth() > 0 {
                self.redo();
                done += 1;
            }
            self.status = if done == 0 {
                "Nothing to redo.".into()
            } else if done == 1 {
                "Redone.".into()
            } else {
                format!("Redid {done} steps.")
            };
        }
    }

    /// Roadmap Wave 1 (RM24, P-14): counted edits since
    /// the last successful NAMED save (or open from a
    /// named file) — `None` while this project has never
    /// met a named file in this session. The counter is
    /// the autosave counter's sibling, not its rival:
    /// `edits_since_autosave` answers "is the recovery
    /// snapshot current?", this answers "is MY FILE
    /// current?" — the two diverge exactly when autosave
    /// has covered work the user never saved.
    pub fn edits_since_save(&self) -> Option<u64> {
        self.last_save_count
            .map(|saved| self.edit_count.saturating_sub(saved))
    }

    /// Roadmap Wave 1 (RM24, P-14): [`SaveState`] from
    /// the counter — see the enum for the three truths.
    pub fn save_state(&self) -> SaveState {
        match self.edits_since_save() {
            None => SaveState::NeverSaved,
            Some(0) => SaveState::Saved,
            Some(_) => SaveState::ModifiedSinceSave,
        }
    }

    /// Roadmap Wave 1 (RM24, P-14): note that the project
    /// now in the core IS the content of its named file —
    /// called by the successful named save and by the
    /// file-open funnels, right after the swap funnel
    /// reset the baseline to "never saved" (the truth
    /// for New / Recover / import swaps, which reach no
    /// such note).
    fn note_save_baseline(&mut self) {
        self.last_save_count = Some(self.edit_count);
    }

    /// Hit test in stage coordinates. Returns (figure, node).
    pub fn hit_test(&self, p: V2) -> Option<(usize, usize)> {
        self.hit_test_impl(p, false, 13.0)
    }

    /// [`hit_test`] with the pick radius as a parameter
    /// (Roadmap Wave 1, IA-RM15-2): the shell passes a
    /// screen-space radius (`stage_view::pick_radius`)
    /// so the on-screen target size is zoom-independent.
    /// The setup arm is preserved: in setup mode the
    /// rest-pose positions are tested, exactly the
    /// positions the press path's setup twin tests.
    pub fn hit_test_with_radius(&self, p: V2, radius: f32) -> Option<(usize, usize)> {
        self.hit_test_impl(p, self.setup_mode, radius)
    }

    /// Wave Y (Y8, P-10): the hit test, against the current
    /// frame's positions or — in setup mode — the rest-pose
    /// positions the stage actually draws.
    fn hit_test_impl(&self, p: V2, setup: bool, radius: f32) -> Option<(usize, usize)> {
        let mut best: Option<(usize, usize, f32)> = None;
        for fi in 0..self.project.figures.len() {
            for (ni, joint) in (if setup {
                self.setup_positions(fi)
            } else {
                self.positions(self.current, fi)
            })
            .iter()
            .enumerate()
            {
                let d = dist(p, *joint);
                if d <= radius && best.map(|(_, _, bd)| d < bd).unwrap_or(true) {
                    best = Some((fi, ni, d));
                }
            }
        }
        best.map(|(fi, ni, _)| (fi, ni))
    }

    /// Joint positions of figure `fi` as setup mode
    /// draws and hit-tests them: the figure's rest pose,
    /// rooted where the current frame's pose roots it
    /// (the rest pose has no root of its own — roots
    /// live in frame poses, so the frame's root is the
    /// anchor a rigging session edits around).
    pub fn setup_positions(&self, fi: usize) -> Vec<V2> {
        let fig = &self.project.figures[fi];
        let root = self.project.frames[self.current].poses[fi].root;
        evaluate(&fig.def, &fig.def.rest_pose(root))
    }

    /// [`setup_positions`] at a fractional frame, for
    /// the stage's display pass: the root comes from
    /// the sampled pose at `frame_f`, the angles are
    /// the rest angles.
    pub fn setup_positions_f(&self, frame_f: f32, fi: usize) -> Vec<V2> {
        let fig = &self.project.figures[fi];
        let poses = self.project.poses_at(frame_f);
        evaluate(&fig.def, &fig.def.rest_pose(poses[fi].root))
    }

    pub fn begin_drag(&mut self, p: V2) -> bool {
        if self.playing {
            return false;
        }
        // Wave Y (Y8, P-10): in setup mode the stage
        // shows rest poses, so the press hit-tests those
        // and the gesture edits rest angles.
        if self.setup_mode {
            return self.begin_setup_drag(p);
        }
        let Some((fi, ni)) = self.hit_test(p) else {
            return false;
        };
        self.begin_drag_at(fi, ni, p)
    }

    /// [`begin_drag`] with the pick radius as a parameter
    /// (Roadmap Wave 1, IA-RM15-2): the shell passes a
    /// screen-space radius so the press target is the
    /// same size on screen at any view zoom. Setup mode
    /// routes to the setup twin, exactly as `begin_drag`
    /// routes to `begin_setup_drag`.
    pub fn begin_drag_with_radius(&mut self, p: V2, radius: f32) -> bool {
        if self.playing {
            return false;
        }
        if self.setup_mode {
            return self.begin_setup_drag_with_radius(p, radius);
        }
        let Some((fi, ni)) = self.hit_test_with_radius(p, radius) else {
            return false;
        };
        self.begin_drag_at(fi, ni, p)
    }

    /// The post-hit half of `begin_drag` (Roadmap Wave 1,
    /// IA-RM15-5): the guards, the gesture's one snapshot,
    /// the selection writes, the drag-kind choice and the
    /// first `drag_to` — everything after the press's hit
    /// test has named (figure, node). The shell's segment
    /// press enters here with `stage_view::segment_hit`'s
    /// answer, so a grab on a limb's middle starts the
    /// same drag a grab on its child joint starts (a
    /// root segment starts the Root drag; a mid joint's
    /// segment starts its Aim). Locked/playing behaviour
    /// is the extracted body's own.
    pub fn begin_drag_at(&mut self, fi: usize, ni: usize, p: V2) -> bool {
        if self.project.figures[fi].locked {
            self.status = "Figure is locked — unlock it in the figures list.".into();
            return false;
        }
        self.snapshot_labeled("Move joint");
        self.selected_figure = fi;
        self.selected_node = Some(ni);
        let fig = &self.project.figures[fi];
        let has_grandparent = fig.def.nodes[ni]
            .parent
            .and_then(|par| fig.def.nodes[par].parent)
            .is_some();
        let kind = if fig.def.nodes[ni].parent.is_none() {
            DragKind::Root
        } else if fig.def.nodes[ni].ik_end && has_grandparent {
            DragKind::Ik(ni)
        } else {
            DragKind::Aim(ni)
        };
        // Wave P (P6) + Wave AC (AC4): the grab frame's
        // pre-drag pose — the first of `end_drag`'s
        // per-frame recap baselines (see the field docs).
        self.drag_poses_before = vec![(
            self.current,
            self.project.frames[self.current].poses[fi].clone(),
        )];
        self.dragging = Some(kind);
        self.drag_to(p);
        true
    }

    /// Begin a setup-mode stage drag (Wave Y, Y8;
    /// SpriteLoop GAP P-10): `begin_drag` against the
    /// rest pose. The press hit-tests the setup
    /// positions of every figure and selects what it
    /// grabs, exactly like a frame drag; Aim and IK then
    /// drive the same solvers against a scratch rest
    /// pose (see `drag_to`'s setup arms), and each move
    /// writes the solved angles back as the figure's
    /// rest angles, clamped through each node's joint
    /// limits — rest data must satisfy the rig's own
    /// limits, where frame poses are performance data
    /// and stay unclamped as they have always been. One
    /// snapshot per gesture, taken only once the press
    /// is known to start a real setup drag.
    ///
    /// A press on the root joint starts no drag: the
    /// rest pose has no root position (roots live in
    /// frame poses), so there is nothing for the
    /// solvers to edit — the press is consumed (returns
    /// true, so the shell does not fall through to the
    /// sprite / stroke pick over a joint the user
    /// plainly grabbed) with a status saying so, and
    /// takes NO snapshot.
    pub fn begin_setup_drag(&mut self, p: V2) -> bool {
        self.begin_setup_drag_with_radius(p, 13.0)
    }

    /// [`begin_setup_drag`] with the pick radius as a
    /// parameter (Roadmap Wave 1, IA-RM15-2).
    pub fn begin_setup_drag_with_radius(&mut self, p: V2, radius: f32) -> bool {
        if self.playing {
            return false;
        }
        let Some((fi, ni)) = self.hit_test_impl(p, true, radius) else {
            return false;
        };
        if self.project.figures[fi].locked {
            self.status = "Figure is locked — unlock it in the figures list.".into();
            return false;
        }
        if self.project.figures[fi].def.nodes[ni].parent.is_none() {
            self.status =
                "The rest pose has no root position — roots live in frame poses.".into();
            self.selected_figure = fi;
            self.selected_node = Some(ni);
            return true;
        }
        self.snapshot_labeled("Adjust rig");
        self.selected_figure = fi;
        self.selected_node = Some(ni);
        let fig = &self.project.figures[fi];
        let has_grandparent = fig.def.nodes[ni]
            .parent
            .and_then(|par| fig.def.nodes[par].parent)
            .is_some();
        let kind = if fig.def.nodes[ni].ik_end && has_grandparent {
            DragKind::SetupIk(ni)
        } else {
            DragKind::SetupAim(ni)
        };
        self.drag_rest_before = Some(
            fig.def.nodes.iter().map(|n| n.rest_angle).collect(),
        );
        self.dragging = Some(kind);
        self.drag_to(p);
        true
    }

    /// Start a camera drag (camera mode): snapshots once, then
    /// `drag_to` pans the key at the current frame.
    pub fn begin_camera_drag(&mut self, p: V2) {
        // Wave P (P6) + Wave AC (AC4): the grab frame's
        // pre-drag camera — the first of `end_drag`'s
        // per-frame recap baselines (see the field docs).
        self.drag_cameras_before = vec![(self.current, self.camera_here())];
        self.snapshot_labeled("Camera drag");
        self.dragging = Some(DragKind::Camera);
        self.camera_last = Some(p);
    }

    pub fn drag_to(&mut self, p: V2) {
        let Some(kind) = self.dragging else { return };
        if let DragKind::Camera = kind {
            // Wave AC (AC4): a mid-gesture frame step
            // retargets the pan at a new frame — capture
            // its pre-gesture camera before the first
            // write lands on it (see the field docs).
            if !self.drag_cameras_before.iter().any(|(f, _)| *f == self.current) {
                let here = self.camera_here();
                self.drag_cameras_before.push((self.current, here));
            }
            if let Some(last) = self.camera_last {
                let (x, y, zoom) = self.camera_here();
                self.project.set_camera_key(
                    self.current,
                    x - (p[0] - last[0]),
                    y - (p[1] - last[1]),
                    zoom,
                );
            }
            self.camera_last = Some(p);
            return;
        }
        // Wave Y (Y8, P-10): setup kinds solve against a
        // scratch pose seeded from the rest angles and
        // write the solved angles back as rest angles;
        // the frame pose is never touched.
        if let DragKind::SetupIk(end) | DragKind::SetupAim(end) = kind {
            let fi = self.selected_figure;
            let root = self.project.frames[self.current].poses[fi].root;
            let mut scratch = self.project.figures[fi].def.rest_pose(root);
            {
                let def = &self.project.figures[fi].def;
                if matches!(kind, DragKind::SetupIk(_)) {
                    apply_ik(def, &mut scratch, end, p);
                } else {
                    aim_node(def, &mut scratch, end, p);
                }
            }
            let fig = &mut self.project.figures[fi];
            for (node, angle) in fig.def.nodes.iter_mut().zip(scratch.angles.iter()) {
                node.rest_angle = node.clamp_angle(*angle);
            }
            return;
        }
        let fi = self.selected_figure;
        let frame = self.current;
        // Wave AC (AC4): the camera arm's retarget rule —
        // a stepped-to frame's pre-gesture pose is
        // captured before the gesture's first write
        // lands on it (see the field docs).
        if !self.drag_poses_before.iter().any(|(f, _)| *f == frame) {
            let before = self.project.frames[frame].poses[fi].clone();
            self.drag_poses_before.push((frame, before));
        }
        let fig = &self.project.figures[fi];
        let pose = &mut self.project.frames[frame].poses[fi];
        match kind {
            DragKind::Camera => {} // handled above
            DragKind::Root => pose.root = p,
            DragKind::Ik(end) => {
                apply_ik(&fig.def, pose, end, p);
            }
            DragKind::Aim(node) => {
                aim_node(&fig.def, pose, node, p);
            }
            // Setup kinds returned above, before the
            // frame pose was borrowed.
            DragKind::SetupIk(_) | DragKind::SetupAim(_) => {}
        }
    }

    /// End a stage drag. The gesture's writes were visible
    /// throughout — the pose / camera follows the pointer —
    /// and a drag that actually moved something also leaves
    /// the status-line recap the shell's other gestures post
    /// at their end (`end_draw`'s "Stroke drawn (…)", the
    /// graph canvas's "Graph key moved: …", the rotation
    /// dial's per-write rotation line). Wave P (P6, charter
    /// §6 feedback completeness): pose and camera drags were
    /// the two stage gestures that ended silent, leaving
    /// whatever stale status preceded them standing. A press
    /// that changed nothing (a click on a joint, a camera
    /// press with no travel) posts nothing — the rotation
    /// dial's dead-zone discipline, applied at gesture end.
    pub fn end_drag(&mut self) {
        let kind = self.dragging.take();
        self.camera_last = None;
        let poses_before = std::mem::take(&mut self.drag_poses_before);
        let rest_before = self.drag_rest_before.take();
        let cameras_before = std::mem::take(&mut self.drag_cameras_before);
        match kind {
            Some(DragKind::Camera) => {
                // Wave AC (AC4): the last frame the
                // gesture actually moved, each frame
                // compared against its own baseline.
                let mut moved_at = None;
                for (frame, before) in &cameras_before {
                    let now = self.project.camera_at(*frame as f32);
                    if (now.0 - before.0).abs() > 0.01
                        || (now.1 - before.1).abs() > 0.01
                        || (now.2 - before.2).abs() > 0.0001
                    {
                        moved_at = Some(*frame);
                    }
                }
                if let Some(frame) = moved_at {
                    self.status = format!("Camera key moved at frame {}.", frame + 1);
                }
            }
            Some(DragKind::SetupIk(_)) | Some(DragKind::SetupAim(_)) => {
                if let Some(before) = rest_before {
                    let fi = self.selected_figure;
                    let changed = self
                        .project
                        .figures
                        .get(fi)
                        .map(|f| {
                            f.def.nodes.len() == before.len()
                                && f.def
                                    .nodes
                                    .iter()
                                    .zip(before.iter())
                                    .any(|(n, b)| (n.rest_angle - b).abs() > 0.01)
                        })
                        .unwrap_or(false);
                    if changed {
                        self.status =
                            format!("Rest pose edited for figure {}.", fi + 1);
                    }
                }
            }
            Some(_) => {
                let fi = self.selected_figure;
                // Wave AC (AC4): the last frame the
                // gesture actually changed, each frame
                // compared against its own baseline.
                let mut posed_at = None;
                for (frame, before) in &poses_before {
                    let changed = self
                        .project
                        .frames
                        .get(*frame)
                        .and_then(|f| f.poses.get(fi))
                        .map(|now| {
                            (now.root[0] - before.root[0]).abs() > 0.01
                                || (now.root[1] - before.root[1]).abs() > 0.01
                                || now.angles.len() != before.angles.len()
                                || now
                                    .angles
                                    .iter()
                                    .zip(before.angles.iter())
                                    .any(|(a, b)| (a - b).abs() > 0.01)
                        })
                        .unwrap_or(false);
                    if changed {
                        posed_at = Some(*frame);
                    }
                }
                if let Some(frame) = posed_at {
                    self.status =
                        format!("Posed figure {} at frame {}.", fi + 1, frame + 1);
                }
            }
            None => {}
        }
    }

    pub fn add_frame(&mut self) {
        self.snapshot_labeled("Add frame");
        // Zero-frame recovery (Wave 10, E9): a project with
        // no frames — an import or an emptied timeline —
        // has no current frame to clone, and indexing one
        // panicked. The timeline's empty state directs the
        // user here, so this path must work: append a
        // fresh frame holding every figure's rest pose at
        // the stage centre and make it current.
        if self.project.frames.is_empty() {
            let stage = self.project.stage;
            let poses = self
                .project
                .figures
                .iter()
                .map(|f| f.def.rest_pose([stage[0] / 2.0, stage[1] * 0.65]))
                .collect();
            self.project.frames.push(crate::model::Frame {
                poses,
                duration: 1.0,
                easing: None,
                bends: Vec::new(),
            });
            self.current = 0;
            self.status = "Frame 1 added.".into();
            return;
        }
        let poses = self.project.frames[self.current].poses.clone();
        let duration = self.project.frames[self.current].duration;
        self.project
            .frames
            .insert(self.current + 1, crate::model::Frame { poses, duration, easing: None, bends: Vec::new() });
        self.current += 1;
        self.status = format!("Frame {} added.", self.current + 1);
    }

    pub fn duplicate_frame(&mut self) {
        self.add_frame();
        self.status = format!("Frame {} duplicated.", self.current + 1);
    }

    pub fn delete_frame(&mut self) {
        if self.project.frames.len() <= 1 {
            self.status = "Cannot delete the only frame.".into();
            return;
        }
        self.snapshot_labeled("Delete frame");
        self.project.frames.remove(self.current);
        // Roadmap Wave 1 (RM18, P-6b): the camera key on
        // the deleted frame goes with it; later keys shift
        // one frame earlier (the delete mapping for a
        // one-frame block at the playhead).
        let removed = self.current;
        self.project.camera.retain(|k| k.frame != removed);
        for key in &mut self.project.camera {
            if key.frame > removed {
                key.frame -= 1;
            }
        }
        if self.current >= self.project.frames.len() {
            self.current = self.project.frames.len() - 1;
        }
        self.status = "Frame deleted.".into();
    }

    // ---- Wave 10 (dopesheet): per-figure pose keyframes ----------
    //
    // Poses are stored densely — every frame holds a pose per
    // figure — so a "keyframe" here is the derived notion in
    // `stats::figure_keyframes`: a frame where figure `fi`'s pose
    // first appears or changes versus the previous frame. The ops
    // below are the engine half of the dopesheet; the shell only
    // displays what these compute.

    /// Delete figure `fi`'s keyframe at `frame`: the frame stops
    /// introducing a new pose and instead holds the pose of the
    /// previous frame, so playback and tweening carry through it
    /// (the dense-model equivalent of removing a key — there is
    /// no sparse key store to remove from). The frame's bend data
    /// for the figure is held the same way, so the frame
    /// introduces no visual change for the figure at all. One
    /// undo step.
    ///
    /// Refusals (returns false, no snapshot): frame 0 — it holds
    /// the base pose and has no earlier pose to hold; a frame
    /// that is not a keyframe for the figure (out of range, the
    /// figure absent there, or its pose already equal to the
    /// previous frame's); and a keyframe where the figure
    /// *appears* (no pose in the previous frame), which has no
    /// earlier pose to hold either.
    pub fn delete_keyframe(&mut self, fi: usize, frame: usize) -> bool {
        let keys = crate::stats::figure_keyframes(&self.project, fi);
        if frame == 0 && keys.contains(&0) {
            self.status = "Frame 1 is the base pose — its keyframe cannot be deleted.".into();
            return false;
        }
        if !keys.contains(&frame) {
            self.status = format!("No keyframe for figure {} at frame {}.", fi + 1, frame + 1);
            return false;
        }
        let Some(held) = self.project.frames[frame - 1].poses.get(fi).cloned() else {
            self.status = format!(
                "Figure {} appears at frame {} — no earlier pose to hold.",
                fi + 1,
                frame + 1
            );
            return false;
        };
        self.snapshot_labeled("Delete keyframe");
        self.project.frames[frame].poses[fi] = held;
        // Hold the bends too (see the doc comment): the target
        // frame's entry, when it has one, takes the previous
        // frame's entry — or zeros, which is what `bends_at`
        // already reads a missing entry as.
        let held_bends = self.project.frames[frame - 1].bends.get(fi).cloned();
        if let Some(entry) = self.project.frames[frame].bends.get_mut(fi) {
            let mut bends = held_bends.unwrap_or_default();
            bends.resize(entry.len(), 0.0);
            *entry = bends;
        }
        self.status = format!(
            "Figure {} keyframe at frame {} deleted.",
            fi + 1,
            frame + 1
        );
        true
    }

    /// The next keyframe of figure `fi` strictly after `from`
    /// (per `stats::figure_keyframes`), or `None` when the
    /// figure has no later key — including an out-of-range `fi`,
    /// which has no keys at all.
    pub fn next_keyframe(&self, fi: usize, from: usize) -> Option<usize> {
        crate::stats::figure_keyframes(&self.project, fi)
            .into_iter()
            .find(|&f| f > from)
    }

    /// The previous keyframe of figure `fi` strictly before
    /// `from`, or `None` when the figure has no earlier key.
    pub fn prev_keyframe(&self, fi: usize, from: usize) -> Option<usize> {
        crate::stats::figure_keyframes(&self.project, fi)
            .into_iter()
            .rev()
            .find(|&f| f < from)
    }

    /// Jump the playhead to figure `fi`'s next keyframe after
    /// the current frame (via [`Self::goto`]). Returns false —
    /// and leaves the playhead where it is — when there is none.
    pub fn goto_next_keyframe(&mut self, fi: usize) -> bool {
        match self.next_keyframe(fi, self.current) {
            Some(target) => {
                self.goto(target);
                self.status = format!("Figure {} keyframe at frame {}.", fi + 1, target + 1);
                true
            }
            None => {
                self.status = format!("No later keyframe for figure {}.", fi + 1);
                false
            }
        }
    }

    /// Jump the playhead to figure `fi`'s previous keyframe
    /// before the current frame. Returns false — and leaves the
    /// playhead where it is — when there is none.
    pub fn goto_prev_keyframe(&mut self, fi: usize) -> bool {
        match self.prev_keyframe(fi, self.current) {
            Some(target) => {
                self.goto(target);
                self.status = format!("Figure {} keyframe at frame {}.", fi + 1, target + 1);
                true
            }
            None => {
                self.status = format!("No earlier keyframe for figure {}.", fi + 1);
                false
            }
        }
    }

    pub fn add_figure(&mut self) {
        self.snapshot_labeled("Add figure");
        let idx = self.project.figures.len();
        let color = FIGURE_COLORS[idx % FIGURE_COLORS.len()];
        let root = [300.0 + 90.0 * idx as f32, 470.0];
        self.project.add_figure(&format!("Styk {}", idx + 1), color, root);
        self.selected_figure = idx;
        self.status = format!("Figure {} added.", idx + 1);
    }

    /// Insert a tween frame halfway between the current frame and
    /// the next (the original's Insert Inbetween).
    pub fn insert_inbetween(&mut self) {
        if self.current + 1 >= self.project.frames.len() {
            self.status = "Inbetween needs a following frame.".into();
            return;
        }
        self.snapshot_labeled("Insert inbetween");
        let a = self.project.frames[self.current].poses.clone();
        let b = self.project.frames[self.current + 1].poses.clone();
        let poses: Vec<crate::model::Pose> = a
            .iter()
            .zip(b.iter())
            .map(|(pa, pb)| pa.lerp(pb, 0.5))
            .collect();
        self.project.frames.insert(
            self.current + 1,
            crate::model::Frame { poses, duration: 1.0, easing: None, bends: Vec::new() },
        );
        self.current += 1;
        self.status = format!("Inbetween inserted at frame {}.", self.current + 1);
    }

    // ---- Figure builder (definition edits, like the original) ----

    /// Append a segment to the selected node of the selected figure.
    pub fn add_segment(&mut self) {
        let fi = self.selected_figure;
        let def = &mut self.project.figures[fi].def;
        let parent = self
            .selected_node
            .filter(|n| *n < def.nodes.len())
            .unwrap_or(def.nodes.len() - 1);
        self.snapshot_labeled("Add segment");
        let ni = self.project.figures[fi].def.nodes.len();
        self.project.figures[fi].def.nodes.push(NodeDef {
            min_angle: None,
            max_angle: None,
            name: format!("segment {ni}"),
            parent: Some(parent),
            length: 36.0,
            rest_angle: 0.0,
            head: false,
            bend: 1.0,
            thickness: 5.0,
            color: None,
            ik_end: false,
            shape: crate::model::Shape::Line,
            style: crate::model::NodeStyle::default(),
            media: None,
        });
        for frame in &mut self.project.frames {
            // Wave 11 (E6): frames where the figure is
            // absent hold no pose for it (the row is
            // shorter) — indexing `poses[fi]` there
            // panicked when a segment was added via MCP
            // (`add_node`) on such a project. Absent rows
            // have no angle vec to grow; skip them.
            if let Some(pose) = frame.poses.get_mut(fi) {
                pose.angles.push(0.0);
            }
        }
        self.selected_node = Some(ni);
        self.status = format!("Segment {ni} added. Drag it to pose.");
    }

    /// Delete the selected leaf segment. Root and segments that
    /// still have children are refused. The index remap is the
    /// full §4.0 set (Roadmap Wave 1, RM19, IA-RM19-6): parents,
    /// pose angles, frame bends and node-indexed graph channels
    /// all move together — a channel targeting the deleted node
    /// is dropped and counted on the status line, never left
    /// dangling.
    pub fn delete_segment(&mut self) {
        let Some(ni) = self.selected_node else {
            self.status = "Select a segment first.".into();
            return;
        };
        let fi = self.selected_figure;
        let def = &self.project.figures[fi].def;
        if def.nodes[ni].parent.is_none() {
            self.status = "Cannot delete the root.".into();
            return;
        }
        if def.nodes.iter().any(|n| n.parent == Some(ni)) {
            self.status = "That segment has children; delete them first.".into();
            return;
        }
        self.snapshot_labeled("Delete segment");
        let def = &mut self.project.figures[fi].def;
        def.nodes.remove(ni);
        for node in def.nodes.iter_mut() {
            if let Some(par) = node.parent {
                if par > ni {
                    node.parent = Some(par - 1);
                }
            }
        }
        for frame in &mut self.project.frames {
            // Wave 11 (E6): same absence guard as
            // `add_segment`, plus the angle row itself: a
            // pose whose angle vec is shorter than the
            // definition (a stale or hand-authored row)
            // has no slot `ni` to remove — `Vec::remove`
            // panicked on both. The MCP `delete_node`
            // handler validates against the definition,
            // not the per-frame pose rows.
            if let Some(pose) = frame.poses.get_mut(fi) {
                if ni < pose.angles.len() {
                    pose.angles.remove(ni);
                }
            }
        }
        // Frame bends (IA-RM19-6): the per-figure bend row is
        // node-indexed exactly like the angle row — remove the
        // slot with the same absence/short-row guards, or every
        // later node's bend silently slides one segment over.
        for frame in &mut self.project.frames {
            if let Some(row) = frame.bends.get_mut(fi) {
                if ni < row.len() {
                    row.remove(ni);
                }
            }
        }
        // Graph channels (IA-RM19-6): this figure's channels
        // whose target names a node past the hole decrement; a
        // channel targeting the deleted node itself is dropped
        // and counted — the §4.0 rule (dropped with a status
        // count, never left dangling). RootX/RootY channels and
        // other figures' channels are not node-indexed here
        // and pass untouched.
        let mut dropped_channels = 0usize;
        self.project.channels.retain_mut(|ch| {
            if ch.figure != fi {
                return true;
            }
            let target = match &mut ch.target {
                crate::graph::ChannelTarget::Angle(t)
                | crate::graph::ChannelTarget::Length(t)
                | crate::graph::ChannelTarget::Bend(t) => t,
                _ => return true,
            };
            if *target == ni {
                dropped_channels += 1;
                false
            } else {
                if *target > ni {
                    *target -= 1;
                }
                true
            }
        });
        self.selected_node = None;
        self.status = if dropped_channels > 0 {
            format!(
                "Segment deleted. ({dropped_channels} graph channel{} removed)",
                if dropped_channels == 1 { "" } else { "s" }
            )
        } else {
            "Segment deleted.".into()
        };
    }

    /// Reparent node `ni` of the selected figure onto
    /// `new_parent` (Roadmap Wave 2, W2-23 — RM19 §4.1,
    /// the IA-RM19-5 signature). **No node index moves**
    /// — only `nodes[ni].parent` changes — so the §4.0
    /// remap table does not apply: pose angles, frame
    /// bends, graph channels and figure states all keep
    /// naming the same nodes, untouched.
    ///
    /// The world rest pose is preserved: with world rest
    /// orientations W(·) composed over `rest_angle`s from
    /// the root (the rest pose IS the def's FK — a root's
    /// world orientation is 0, exactly as `evaluate`
    /// seeds it), the node's new rest angle is
    /// `W(ni) − W(new_parent)`, normalised. Lengths are
    /// unchanged and children follow — their relative
    /// rests are untouched, so the whole subtree keeps
    /// its world rest pose. Preserving the *relative*
    /// angle instead would swing the subtree silently;
    /// this is the only honest default (RM19 §4.1).
    ///
    /// Refusals spend no snapshot and return false:
    /// no such figure or node, the figure is locked,
    /// `ni` is the root, `new_parent` is out of range,
    /// or `new_parent` is `ni` itself or one of its
    /// descendants (the cycle refusal). A successful
    /// reparent spends exactly one snapshot labelled
    /// "Reparent node" and returns true.
    pub fn reparent_node(&mut self, ni: usize, new_parent: usize) -> bool {
        let fi = self.selected_figure;
        let Some(fig) = self.project.figures.get(fi) else {
            self.status = "Select a figure first.".into();
            return false;
        };
        if ni >= fig.def.nodes.len() {
            self.status = "Select a segment first.".into();
            return false;
        }
        if fig.locked {
            self.status = "Figure is locked — unlock it in the figures list.".into();
            return false;
        }
        let def = &fig.def;
        if def.nodes[ni].parent.is_none() {
            self.status = "The root cannot be reparented.".into();
            return false;
        }
        if new_parent >= def.nodes.len() {
            self.status = "That parent does not exist.".into();
            return false;
        }
        if new_parent == ni || Self::subtree_contains(def, ni, new_parent) {
            self.status = "That would make the segment its own ancestor.".into();
            return false;
        }
        // World rest orientations, composed BEFORE the
        // write from the def as it stands.
        let world = Self::rest_world_angles(def);
        let new_rest = crate::model::norm_deg(world[ni] - world[new_parent]);
        let parent_name = def.nodes[new_parent].name.clone();
        let node_name = def.nodes[ni].name.clone();
        self.snapshot_labeled("Reparent node");
        let node = &mut self.project.figures[fi].def.nodes[ni];
        node.parent = Some(new_parent);
        node.rest_angle = new_rest;
        self.status = format!("{node_name} reparented to {parent_name}.");
        true
    }

    /// Cycle the selected node's parent to the next
    /// legal parent in tree order (Roadmap Wave 2,
    /// W2-23 — RM19 §3/§4.1's panel idiom: a parent
    /// picker needs no new kit, so the panel's
    /// "Reparent" button cycles). The candidate list is
    /// [`EditorCore::legal_parents`] — every node except
    /// `ni` and its subtree, in the rig tree's pre-order
    /// — and the step after the current parent wraps to
    /// the first candidate. The write itself is
    /// [`EditorCore::reparent_node`], so a successful
    /// cycle costs exactly one "Reparent node" snapshot;
    /// every refusal (no selection, locked figure, the
    /// root, no other legal parent) spends nothing.
    pub fn cycle_reparent_selected(&mut self) -> bool {
        let Some(ni) = self.selected_node else {
            self.status = "Select a segment first.".into();
            return false;
        };
        let fi = self.selected_figure;
        let Some(fig) = self.project.figures.get(fi) else {
            self.status = "Select a figure first.".into();
            return false;
        };
        if ni >= fig.def.nodes.len() {
            self.status = "Select a segment first.".into();
            return false;
        }
        if fig.locked {
            self.status = "Figure is locked — unlock it in the figures list.".into();
            return false;
        }
        if fig.def.nodes[ni].parent.is_none() {
            self.status = "The root cannot be reparented.".into();
            return false;
        }
        let Some(next) = Self::next_legal_parent(&fig.def, ni) else {
            self.status = "No other legal parent for this segment.".into();
            return false;
        };
        self.reparent_node(ni, next)
    }

    /// The legal parents for node `ni`, in tree order:
    /// every node of the def except `ni` itself and its
    /// whole subtree (parenting onto any of those would
    /// close a cycle), ordered by the rig tree's
    /// pre-order walk — the same order the panel shows.
    /// Pure over the def; malformed parent links
    /// (out-of-range, self-parented, cycles) cannot hang
    /// it: a visited set bounds the walk, as in the rig
    /// tree's own rows.
    pub fn legal_parents(def: &crate::model::FigureDef, ni: usize) -> Vec<usize> {
        if ni >= def.nodes.len() {
            return Vec::new();
        }
        let mut banned = vec![false; def.nodes.len()];
        banned[ni] = true;
        // Ban the subtree: children in node-index order,
        // transitively. The visited marks themselves
        // bound the walk on cyclic defs.
        let mut stack = vec![ni];
        while let Some(n) = stack.pop() {
            for (i, node) in def.nodes.iter().enumerate() {
                if node.parent == Some(n) && !banned[i] {
                    banned[i] = true;
                    stack.push(i);
                }
            }
        }
        Self::preorder(def)
            .into_iter()
            .filter(|&i| !banned[i])
            .collect()
    }

    /// The candidate [`EditorCore::legal_parents`]
    /// offers after `ni`'s current parent, wrapping to
    /// the first candidate at the end of the list;
    /// `None` when the list is empty (a lone root, or a
    /// root whose subtree is the whole figure). A
    /// current parent missing from the list (it cannot
    /// be — the parent is never in the subtree) falls
    /// back to the first candidate.
    pub fn next_legal_parent(def: &crate::model::FigureDef, ni: usize) -> Option<usize> {
        let legal = Self::legal_parents(def, ni);
        let current = def.nodes.get(ni)?.parent;
        match current.and_then(|p| legal.iter().position(|&c| c == p)) {
            Some(pos) => legal.get(pos + 1).or_else(|| legal.first()).copied(),
            None => legal.first().copied(),
        }
    }

    /// Whether `target` lies in the subtree rooted at
    /// `root` (root itself excluded — the caller checks
    /// identity separately): walk `target`'s parent
    /// chain looking for `root`. A visited bound keeps
    /// a cyclic def from hanging the walk.
    fn subtree_contains(def: &crate::model::FigureDef, root: usize, target: usize) -> bool {
        let mut cur = def.nodes.get(target).and_then(|n| n.parent);
        let mut steps = 0;
        while let Some(p) = cur {
            if p == root {
                return true;
            }
            steps += 1;
            if steps > def.nodes.len() {
                return false;
            }
            cur = def.nodes.get(p).and_then(|n| n.parent);
        }
        false
    }

    /// World rest orientation of every node: the
    /// running sum of `rest_angle`s from the root (a
    /// root's own world orientation is 0, matching
    /// `evaluate`'s seeding). Nodes are stored
    /// parent-before-child in every constructor the app
    /// ships; a node whose parent link is malformed
    /// (out of range, self, or not yet visited) reads as
    /// a root rather than panicking — the rig tree's
    /// rows take the same line on bad links.
    fn rest_world_angles(def: &crate::model::FigureDef) -> Vec<f32> {
        let mut world = vec![0.0f32; def.nodes.len()];
        let mut done = vec![false; def.nodes.len()];
        for (i, node) in def.nodes.iter().enumerate() {
            world[i] = match node.parent {
                Some(p) if p < def.nodes.len() && p != i && done[p] => world[p] + node.rest_angle,
                _ => 0.0,
            };
            done[i] = true;
        }
        world
    }

    /// Every node in the rig tree's display order: a
    /// pre-order walk from the roots, children in
    /// node-index order, a visited set bounding the
    /// walk; a node caught in a parent cycle shows as
    /// its own root rather than dropping out (the rig
    /// tree rows' own rule, mirrored so the cycle order
    /// and the row order can never disagree).
    fn preorder(def: &crate::model::FigureDef) -> Vec<usize> {
        fn is_root(def: &crate::model::FigureDef, ni: usize) -> bool {
            match def.nodes[ni].parent {
                None => true,
                Some(p) => p >= def.nodes.len() || p == ni,
            }
        }
        fn walk(def: &crate::model::FigureDef, ni: usize, visited: &mut Vec<bool>, out: &mut Vec<usize>) {
            if visited[ni] {
                return;
            }
            visited[ni] = true;
            out.push(ni);
            for i in 0..def.nodes.len() {
                if def.nodes[i].parent == Some(ni) {
                    walk(def, i, visited, out);
                }
            }
        }
        let mut out = Vec::with_capacity(def.nodes.len());
        let mut visited = vec![false; def.nodes.len()];
        for ni in 0..def.nodes.len() {
            if is_root(def, ni) {
                walk(def, ni, &mut visited, &mut out);
            }
        }
        for ni in 0..def.nodes.len() {
            walk(def, ni, &mut visited, &mut out);
        }
        out
    }

    /// The members of the subtree rooted at `ni`, in
    /// the rig tree's pre-order (root first, children in
    /// node-index order). The membership set is collected
    /// by a child-walk bounded by its own marks, so a
    /// cyclic hand-authored def cannot hang it; the
    /// ordering rides [`EditorCore::preorder`] so it can
    /// never disagree with the panel's rows.
    fn subtree_members(def: &crate::model::FigureDef, ni: usize) -> Vec<usize> {
        if ni >= def.nodes.len() {
            return Vec::new();
        }
        let mut member = vec![false; def.nodes.len()];
        member[ni] = true;
        let mut stack = vec![ni];
        while let Some(n) = stack.pop() {
            for (i, node) in def.nodes.iter().enumerate() {
                if node.parent == Some(n) && !member[i] {
                    member[i] = true;
                    stack.push(i);
                }
            }
        }
        Self::preorder(def)
            .into_iter()
            .filter(|&i| member[i])
            .collect()
    }

    /// Split the segment parent→`ni` of the selected
    /// figure at its midpoint with a new collinear node
    /// (Roadmap Wave 2, W2-24 — RM19 §4.2, the
    /// IA-RM19-5 signature). The new node N is
    /// **appended** to `nodes`, never spliced at `ni`,
    /// so every existing node index stays valid and the
    /// §4.0 remap table does not engage — the only
    /// writes to node-indexed stores are slot extensions
    /// (one 0.0 angle slot and one 0.0 bend slot per
    /// frame row, absence-guarded exactly like
    /// `add_segment` / `delete_segment`): parents (only
    /// `ni`'s own, rewired to N), pose angles (extended,
    /// existing values untouched), frame bends
    /// (extended), graph channels (not node-moving —
    /// untouched), figure states (frame→clip addressed,
    /// no node index — untouched).
    ///
    /// The rest pose is preserved **exactly**: N takes
    /// `ni`'s parent, half of `ni`'s length and `ni`'s
    /// rest angle; `ni` keeps the other half of its
    /// length with rest angle 0 under N, so `ni`'s joint
    /// lands where it always did and its children are
    /// untouched. N's remaining fields are
    /// `add_segment`'s defaults. Posed frames: pose
    /// angles are relative, so existing poses keep their
    /// shape — `ni`'s keyed angle stays on `ni` (the
    /// segment the user keyed), and N's fresh slot is
    /// 0.0 = rest (RM19 §4.2, documented there).
    ///
    /// Refusals spend no snapshot and return false: no
    /// such figure or node, the figure is locked, or
    /// `ni` is the root (it has no segment to split). A
    /// successful split spends exactly one snapshot
    /// labelled "Insert segment node", selects the new
    /// node (the `add_segment` precedent — the created
    /// node becomes the selection) and returns true.
    pub fn insert_segment_between(&mut self, ni: usize) -> bool {
        let fi = self.selected_figure;
        let Some(fig) = self.project.figures.get(fi) else {
            self.status = "Select a figure first.".into();
            return false;
        };
        if ni >= fig.def.nodes.len() {
            self.status = "Select a segment first.".into();
            return false;
        }
        if fig.locked {
            self.status = "Figure is locked — unlock it in the figures list.".into();
            return false;
        }
        let def = &fig.def;
        let Some(parent) = def.nodes[ni].parent else {
            self.status = "The root has no segment to split.".into();
            return false;
        };
        // Validate-first: every value the write needs is
        // read from the def as it stands, before the
        // snapshot is spent.
        let new_index = def.nodes.len();
        let half = def.nodes[ni].length / 2.0;
        let rest = def.nodes[ni].rest_angle;
        let node_name = def.nodes[ni].name.clone();
        let new_name = format!("segment {new_index}");
        self.snapshot_labeled("Insert segment node");
        let def = &mut self.project.figures[fi].def;
        def.nodes.push(NodeDef {
            min_angle: None,
            max_angle: None,
            name: new_name.clone(),
            parent: Some(parent),
            length: half,
            rest_angle: rest,
            head: false,
            bend: 1.0,
            thickness: 5.0,
            color: None,
            ik_end: false,
            shape: crate::model::Shape::Line,
            style: crate::model::NodeStyle::default(),
            media: None,
        });
        let node = &mut def.nodes[ni];
        node.parent = Some(new_index);
        node.length = half;
        node.rest_angle = 0.0;
        // Slot extension (§4.0's append case): one fresh
        // slot at the appended position per frame row —
        // angles exactly like `add_segment` (a pose row
        // that exists grows; absent figures have no row
        // to grow), bends likewise where a bend row
        // exists (reads are absence-guarded, so a row
        // that stays short still reads 0.0 = straight).
        for frame in &mut self.project.frames {
            if let Some(pose) = frame.poses.get_mut(fi) {
                pose.angles.push(0.0);
            }
            if let Some(row) = frame.bends.get_mut(fi) {
                row.push(0.0);
            }
        }
        self.selected_node = Some(new_index);
        self.status = format!("Segment split — {new_name} inserted above {node_name}.");
        true
    }

    /// Deep-copy the subtree rooted at `ni` of the
    /// selected figure, appended in pre-order with fresh
    /// indices (Roadmap Wave 2, W2-24 — RM19 §4.3, the
    /// IA-RM19-5 signature). Appending moves no existing
    /// index, so the §4.0 remap is the identity on the
    /// old figure: parents, pose angles, frame bends and
    /// graph channels of existing nodes are byte-untouched.
    /// Inside the clone, parent links remap through the
    /// old→new map; the clone root's parent is `ni`'s own
    /// parent — an external reference, deliberately NOT
    /// remapped — so the copy lands as a sibling subtree.
    /// The clone root is named `"{name} copy"`; descendants
    /// keep their names (name uniqueness is not an engine
    /// invariant, RM19 §4.3). Node defs are deep-cloned
    /// whole — segment media, joint limits, shapes and
    /// styles ride along; a media entry names a project
    /// asset by index, and that external reference stays
    /// pointing at the same asset.
    ///
    /// The clone starts at rest: each frame row gains
    /// one 0.0 angle slot and one 0.0 bend slot per
    /// cloned node, absence-guarded like
    /// [`EditorCore::insert_segment_between`].
    /// Duplicating posed angles was considered and
    /// rejected by the design — a duplicated rig part
    /// starts at rest; duplicating animation is the
    /// pose/clip tools' job. Graph channels are NOT
    /// cloned (§4.0/§4.3): a channel keyed on the
    /// original keeps driving the original. Figure
    /// states carry no node index (frame→clip) and are
    /// untouched.
    ///
    /// Refusals spend no snapshot and return false: no
    /// such figure or node, the figure is locked, or
    /// `ni` is the root (its parent — the clone root's
    /// anchor — does not exist, and a second root in one
    /// def is not a legal rig). A successful duplicate
    /// spends exactly one snapshot labelled "Duplicate
    /// subtree", moves the selection to the clone root
    /// (RM19 §4.3) and returns true.
    pub fn duplicate_subtree(&mut self, ni: usize) -> bool {
        let fi = self.selected_figure;
        let Some(fig) = self.project.figures.get(fi) else {
            self.status = "Select a figure first.".into();
            return false;
        };
        if ni >= fig.def.nodes.len() {
            self.status = "Select a segment first.".into();
            return false;
        }
        if fig.locked {
            self.status = "Figure is locked — unlock it in the figures list.".into();
            return false;
        }
        let def = &fig.def;
        if def.nodes[ni].parent.is_none() {
            self.status = "The root cannot be duplicated.".into();
            return false;
        }
        // Validate-first: the clone set, the old→new map
        // and the cloned defs are all computed from the
        // def as it stands, before the snapshot is spent.
        let members = Self::subtree_members(def, ni);
        let base = def.nodes.len();
        let map: std::collections::HashMap<usize, usize> = members
            .iter()
            .enumerate()
            .map(|(k, &old)| (old, base + k))
            .collect();
        let mut clones: Vec<NodeDef> = Vec::with_capacity(members.len());
        for &old in &members {
            let mut node = def.nodes[old].clone();
            if old == ni {
                // The clone root: anchored on the
                // original's parent (external, unmapped)
                // and named as a copy.
                node.name = format!("{} copy", node.name);
            } else {
                // An internal link: the parent is always
                // another member (a subtree is closed
                // under children), so the map always hits.
                node.parent = node.parent.and_then(|p| map.get(&p).copied());
            }
            clones.push(node);
        }
        let count = clones.len();
        let clone_root = base; // `ni` is first in pre-order.
        self.snapshot_labeled("Duplicate subtree");
        self.project.figures[fi].def.nodes.extend(clones);
        for frame in &mut self.project.frames {
            if let Some(pose) = frame.poses.get_mut(fi) {
                let len = pose.angles.len();
                pose.angles.resize(len + count, 0.0);
            }
            if let Some(row) = frame.bends.get_mut(fi) {
                let len = row.len();
                row.resize(len + count, 0.0);
            }
        }
        self.selected_node = Some(clone_root);
        self.status = format!(
            "Subtree duplicated ({count} segment{}).",
            if count == 1 { "" } else { "s" }
        );
        true
    }

    /// Remap every node-indexed store of figure `fi`
    /// through `map` (Roadmap Wave 2, W2-25 — RM19
    /// §4.0's shared helper for the ops that REMOVE or
    /// REORDER nodes): `map[old]` is the node's new
    /// index, or `None` when the node is being removed.
    /// The caller owns `def.nodes` itself (the physical
    /// removal and the parent rewrite — parents are
    /// definition data, removed in the same breath);
    /// this helper owns everything that POINTS at
    /// nodes, so the stores can never drift apart
    /// again:
    ///
    /// - pose angles (`frames[].poses[fi].angles`):
    ///   rebuilt through the map — slots naming removed
    ///   nodes drop out, survivors land at their new
    ///   positions, and a short/stale row stays aligned
    ///   because a missing slot rebuilds as 0.0 only
    ///   where a surviving node needs one (the read side
    ///   treats a missing slot as rest, the same value —
    ///   see `bends_at`'s convention);
    /// - frame bends (`frames[].bends[fi]`): rebuilt
    ///   identically (a missing slot reads as 0.0 =
    ///   straight);
    /// - graph channels of THIS figure: Angle/Length/
    ///   Bend targets remap through the map; a channel
    ///   targeting a removed node, or one whose stored
    ///   index is out of range entirely (stale data),
    ///   is DROPPED and counted — never left dangling
    ///   (§4.0's rule). RootX/RootY channels and other
    ///   figures' channels pass untouched.
    ///
    /// Figure states (`FigureInstance::states`) carry
    /// no node index — `StateKey` is frame→clip
    /// addressed (verified at build, W2-24 §2) — and a
    /// figure's IK ends ride `NodeDef::ik_end` in the
    /// def itself, so neither needs a row here.
    /// `core.selected_node` is the caller's to remap
    /// (it lives on the core, not the project). Returns
    /// the number of dropped channels, for the status
    /// count.
    fn remap_node_indices(
        project: &mut crate::model::Project,
        fi: usize,
        map: &[Option<usize>],
    ) -> usize {
        // Rebuild one node-indexed row through the map:
        // position `new` takes old slot `map`⁻¹(new);
        // an old slot past the row's end (a short/stale
        // row) or past the map leaves the value 0.0,
        // which every reader of these rows already
        // treats as the slot's neutral value.
        fn rebuild(row: &[f32], map: &[Option<usize>]) -> Vec<f32> {
            let new_len = map.iter().filter(|m| m.is_some()).count();
            let mut out = vec![0.0f32; new_len];
            for (old, m) in map.iter().enumerate() {
                if let (Some(new), Some(v)) = (m, row.get(old)) {
                    out[*new] = *v;
                }
            }
            out
        }
        for frame in &mut project.frames {
            if let Some(pose) = frame.poses.get_mut(fi) {
                pose.angles = rebuild(&pose.angles, map);
            }
            if let Some(row) = frame.bends.get_mut(fi) {
                let rebuilt = rebuild(row, map);
                *row = rebuilt;
            }
        }
        let mut dropped = 0usize;
        project.channels.retain_mut(|ch| {
            if ch.figure != fi {
                return true;
            }
            let target = match &mut ch.target {
                crate::graph::ChannelTarget::Angle(t)
                | crate::graph::ChannelTarget::Length(t)
                | crate::graph::ChannelTarget::Bend(t) => t,
                _ => return true,
            };
            match map.get(*target).copied().flatten() {
                Some(new) => {
                    *target = new;
                    true
                }
                None => {
                    dropped += 1;
                    false
                }
            }
        });
        dropped
    }

    /// Delete the subtree rooted at `ni` of the
    /// selected figure (Roadmap Wave 2, W2-25 — RM19
    /// §4.4, the IA-RM19-5 signature). This is the
    /// index-shifting op: removing nodes moves every
    /// later index down, so the removal runs the full
    /// §4.0 table through the one shared helper
    /// [`EditorCore::remap_node_indices`] — parents
    /// (rewritten as the nodes are removed), pose
    /// angles, frame bends and this figure's graph
    /// channels all move together; channels into the
    /// deleted subtree are dropped and counted on the
    /// status line, never left dangling. Segment media
    /// ride inside the removed `NodeDef`s; media on
    /// surviving segments keep pointing at their
    /// segments because they ARE their segments' data.
    /// Figure states carry no node index (frame→clip)
    /// and pass untouched. The selection remaps with
    /// everything else: a surviving selection follows
    /// its node, a deleted one clears. This op
    /// supersedes `delete_segment`'s leaf-only refusal
    /// for panel use; `delete_segment` itself stays for
    /// MCP compat.
    ///
    /// Refusals spend no snapshot and return false: no
    /// such figure or node, the figure is locked, or
    /// `ni` is the root (deleting the root would clear
    /// the whole figure — that is the figure list's
    /// delete, not a rig op). A successful delete
    /// spends exactly one snapshot labelled "Delete
    /// subtree" and returns true.
    pub fn delete_subtree(&mut self, ni: usize) -> bool {
        let fi = self.selected_figure;
        let Some(fig) = self.project.figures.get(fi) else {
            self.status = "Select a figure first.".into();
            return false;
        };
        if ni >= fig.def.nodes.len() {
            self.status = "Select a segment first.".into();
            return false;
        }
        if fig.locked {
            self.status = "Figure is locked — unlock it in the figures list.".into();
            return false;
        }
        if fig.def.nodes[ni].parent.is_none() {
            self.status = "The root cannot be deleted.".into();
            return false;
        }
        // Validate-first: the member set and the
        // old→new map are computed from the def as it
        // stands, before the snapshot is spent.
        let members = Self::subtree_members(&fig.def, ni);
        let old_len = fig.def.nodes.len();
        let mut removed = vec![false; old_len];
        for &m in &members {
            removed[m] = true;
        }
        let mut map: Vec<Option<usize>> = vec![None; old_len];
        let mut next = 0usize;
        for old in 0..old_len {
            if !removed[old] {
                map[old] = Some(next);
                next += 1;
            }
        }
        let count = members.len();
        self.snapshot_labeled("Delete subtree");
        // Physical removal, highest index first so the
        // lower indices the map was built against stay
        // valid until their own removal; then every
        // surviving parent rewrites through the map (a
        // survivor's parent is always a survivor — a
        // subtree is closed under children — and a
        // malformed out-of-range parent degrades to a
        // root rather than dangling).
        let mut descending = members.clone();
        descending.sort_unstable_by(|a, b| b.cmp(a));
        let def = &mut self.project.figures[fi].def;
        for &old in &descending {
            def.nodes.remove(old);
        }
        for node in def.nodes.iter_mut() {
            if let Some(p) = node.parent {
                node.parent = map.get(p).copied().flatten();
            }
        }
        let dropped_channels = Self::remap_node_indices(&mut self.project, fi, &map);
        self.selected_node = self
            .selected_node
            .and_then(|sel| map.get(sel).copied().flatten());
        self.status = if dropped_channels > 0 {
            format!(
                "Subtree deleted ({count} segment{}, {dropped_channels} graph channel{} removed).",
                if count == 1 { "" } else { "s" },
                if dropped_channels == 1 { "" } else { "s" }
            )
        } else {
            format!(
                "Subtree deleted ({count} segment{}).",
                if count == 1 { "" } else { "s" }
            )
        };
        true
    }

    /// The mirror image of a node name under RM19
    /// §4.5's token-exact swap: whole space-separated
    /// tokens `left ↔ right`, `Left ↔ Right`,
    /// `LEFT ↔ RIGHT` and the single letters
    /// `L ↔ R` / `l ↔ r` swap; a token ENDING in
    /// `_l ↔ _r`, `_L ↔ _R` or `-l ↔ -r` swaps its
    /// suffix. Anything else is left alone — a node
    /// named "elbow" or "flare" must not change.
    fn mirror_node_name(name: &str) -> String {
        name.split(' ')
            .map(|tok| match tok {
                "left" => "right".to_string(),
                "right" => "left".to_string(),
                "Left" => "Right".to_string(),
                "Right" => "Left".to_string(),
                "LEFT" => "RIGHT".to_string(),
                "RIGHT" => "LEFT".to_string(),
                "L" => "R".to_string(),
                "R" => "L".to_string(),
                "l" => "r".to_string(),
                "r" => "l".to_string(),
                _ => {
                    let mut out = tok.to_string();
                    for (suffix, swapped) in [
                        ("_l", "_r"),
                        ("_r", "_l"),
                        ("_L", "_R"),
                        ("_R", "_L"),
                        ("-l", "-r"),
                        ("-r", "-l"),
                    ] {
                        if let Some(stem) = tok.strip_suffix(suffix) {
                            out = format!("{stem}{swapped}");
                            break;
                        }
                    }
                    out
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Mirror the subtree rooted at `ni` of the
    /// selected figure across the figure's vertical
    /// axis (Roadmap Wave 2, W2-25 — RM19 §4.5, the
    /// IA-RM19-5 signature). **No node index moves**
    /// — like reparent, the op edits in place, so the
    /// §4.0 remap table does not engage; what changes
    /// is the mirrored content of the subtree's own
    /// slots:
    ///
    /// - geometry, per node in the subtree:
    ///   `rest_angle = norm_deg(−rest_angle)` — the
    ///   model's angle convention is relative and
    ///   signed, so negation IS reflection across the
    ///   vertical through the subtree root's parent
    ///   joint, composed: negating every relative
    ///   angle in the subtree mirrors the world pose
    ///   about the vertical line through the subtree
    ///   root's position, and the subtree root's own
    ///   negation swings it to the mirrored side of
    ///   its parent. Rest angles are re-solved in the
    ///   only sense a reflection needs — each relative
    ///   rest negates, so the FK composition lands the
    ///   subtree's rest pose mirrored exactly;
    /// - `NodeDef::bend = −bend` (the IK bend-side
    ///   sign flips with the limb), joint limits swap
    ///   (`min ↔ −max`: the arc reflects with the
    ///   joint);
    /// - animation, for the subtree's nodes only:
    ///   pose angles negated in every frame
    ///   (absence-guarded), frame bends negated
    ///   (absence-guarded), Angle channels' key values
    ///   AND tangents negated (a mirrored curve is the
    ///   honest mirror); Length/Bend channels
    ///   untouched (a length has no side; frame bends
    ///   themselves carry the bend mirror);
    /// - names swap token-exactly (`left ↔ right`,
    ///   `_l ↔ _r`, … — see
    ///   [`EditorCore::mirror_node_name`]); unmatched
    ///   names are left alone, the geometry still
    ///   mirrors;
    /// - a subtree segment's image media has its
    ///   `flip` toggled (a mirrored segment's image
    ///   must mirror too — Wave Y media semantics);
    ///   text media and the image's remaining
    ///   transform ride unchanged.
    ///
    /// Refusals spend no snapshot and return false:
    /// no such figure or node, the figure is locked,
    /// or `ni` is the root (the root is the figure's
    /// mirror axis — mirroring it would mirror the
    /// whole figure, a figure-level op this is not).
    /// A successful mirror spends exactly one snapshot
    /// labelled "Mirror subtree" and returns true.
    pub fn mirror_subtree(&mut self, ni: usize) -> bool {
        let fi = self.selected_figure;
        let Some(fig) = self.project.figures.get(fi) else {
            self.status = "Select a figure first.".into();
            return false;
        };
        if ni >= fig.def.nodes.len() {
            self.status = "Select a segment first.".into();
            return false;
        }
        if fig.locked {
            self.status = "Figure is locked — unlock it in the figures list.".into();
            return false;
        }
        if fig.def.nodes[ni].parent.is_none() {
            self.status = "The root cannot be mirrored.".into();
            return false;
        }
        // Validate-first: membership and the mirrored
        // names are computed from the def as it stands,
        // before the snapshot is spent.
        let members = Self::subtree_members(&fig.def, ni);
        let count = members.len();
        let mut in_subtree = vec![false; fig.def.nodes.len()];
        for &m in &members {
            in_subtree[m] = true;
        }
        let names: Vec<String> = members
            .iter()
            .map(|&m| Self::mirror_node_name(&fig.def.nodes[m].name))
            .collect();
        self.snapshot_labeled("Mirror subtree");
        let def = &mut self.project.figures[fi].def;
        for (k, &m) in members.iter().enumerate() {
            let node = &mut def.nodes[m];
            node.rest_angle = crate::model::norm_deg(-node.rest_angle);
            node.bend = -node.bend;
            // The limit arc reflects with the joint:
            // the old ceiling becomes the mirrored
            // floor.
            let (min, max) = (node.min_angle, node.max_angle);
            node.min_angle = max.map(|v| -v);
            node.max_angle = min.map(|v| -v);
            node.name = names[k].clone();
            if let Some(crate::model::SegmentMedia::Image { flip, .. }) = &mut node.media {
                *flip = !*flip;
            }
        }
        for frame in &mut self.project.frames {
            if let Some(pose) = frame.poses.get_mut(fi) {
                for &m in &members {
                    if let Some(a) = pose.angles.get_mut(m) {
                        *a = -*a;
                    }
                }
            }
            if let Some(row) = frame.bends.get_mut(fi) {
                for &m in &members {
                    if let Some(b) = row.get_mut(m) {
                        *b = -*b;
                    }
                }
            }
        }
        for ch in &mut self.project.channels {
            if ch.figure != fi {
                continue;
            }
            if let crate::graph::ChannelTarget::Angle(t) = ch.target {
                if in_subtree.get(t) == Some(&true) {
                    for key in &mut ch.keys {
                        key.value = -key.value;
                        key.in_tan = -key.in_tan;
                        key.out_tan = -key.out_tan;
                    }
                }
            }
        }
        self.status = format!(
            "Subtree mirrored ({count} segment{}).",
            if count == 1 { "" } else { "s" }
        );
        true
    }

    /// The shared validation head of the four node-flag
    /// setters below (Roadmap Wave 2, W2-26 — RM19 §5 /
    /// IA-RM19-7): figure present, node present, figure
    /// unlocked, in the sibling rig ops' refusal order
    /// (W2-23/24/25). Returns the node's name for the
    /// caller's status, or posts the refusal and returns
    /// `None` — a refusal spends no snapshot.
    fn node_flag_target(&mut self, fi: usize, ni: usize) -> Option<String> {
        let Some(fig) = self.project.figures.get(fi) else {
            self.status = "Select a figure first.".into();
            return None;
        };
        let Some(node) = fig.def.nodes.get(ni) else {
            self.status = "Select a segment first.".into();
            return None;
        };
        if fig.locked {
            self.status = "Figure is locked — unlock it in the figures list.".into();
            return None;
        }
        Some(node.name.clone())
    }

    /// Rename node `ni` of figure `fi` (Roadmap Wave 2,
    /// W2-26 — RM19 §5 / IA-RM19-7): the segment
    /// inspector's Node name field. The raw text is
    /// trimmed, as the figure rename trims; an empty
    /// result is refused ("Segment name cannot be
    /// empty." — the figure rename's sentence, segment
    /// form). A real change spends exactly one snapshot
    /// labelled "Rename segment"; renaming to the name
    /// the node already carries spends nothing and still
    /// reports success (the `rename_selected_figure`
    /// discipline). Refusals (no such figure or node,
    /// locked figure, empty name) spend nothing and
    /// return false.
    pub fn set_node_name(&mut self, fi: usize, ni: usize, raw: &str) -> bool {
        let Some(_) = self.node_flag_target(fi, ni) else {
            return false;
        };
        let name = raw.trim();
        if name.is_empty() {
            self.status = "Segment name cannot be empty.".into();
            return false;
        }
        if self.project.figures[fi].def.nodes[ni].name != name {
            self.snapshot_labeled("Rename segment");
            self.project.figures[fi].def.nodes[ni].name = name.to_string();
        }
        self.status = format!("Segment {ni} renamed to \"{name}\".");
        true
    }

    /// Set or clear node `ni`'s head flag on figure `fi`
    /// (Roadmap Wave 2, W2-26 — RM19 §5 / IA-RM19-7):
    /// the segment inspector's Head toggle. Multiple
    /// heads are legal data (the constructors make one;
    /// this setter does not police it — RM19 §5), so
    /// there is no op-specific refusal. A real change
    /// spends exactly one snapshot labelled "Segment
    /// head"; writing the value the flag already carries
    /// spends nothing and still reports the state.
    /// Refusals (no such figure or node, locked figure)
    /// spend nothing and return false.
    pub fn set_node_head(&mut self, fi: usize, ni: usize, on: bool) -> bool {
        let Some(name) = self.node_flag_target(fi, ni) else {
            return false;
        };
        if self.project.figures[fi].def.nodes[ni].head != on {
            self.snapshot_labeled("Segment head");
            self.project.figures[fi].def.nodes[ni].head = on;
        }
        self.status = if on {
            format!("{name} is the head.")
        } else {
            format!("{name} is not the head.")
        };
        true
    }

    /// Set or clear node `ni`'s IK-end flag on figure
    /// `fi` (Roadmap Wave 2, W2-26 — RM19 §5 /
    /// IA-RM19-7): the segment inspector's IK end
    /// toggle. Turning the flag ON refuses when the node
    /// has no grandparent — the two-bone solver's actual
    /// precondition, the same `has_grandparent` test
    /// `begin_drag_at` uses to pick the IK drag kind —
    /// with the standing sentence "IK needs a chain of
    /// two segments above this joint." Turning it OFF is
    /// always allowed. A real change spends exactly one
    /// snapshot labelled "Segment IK end"; writing the
    /// value the flag already carries spends nothing and
    /// still reports the state. Refusals (no such figure
    /// or node, locked figure, the chain refusal) spend
    /// nothing and return false.
    pub fn set_node_ik_end(&mut self, fi: usize, ni: usize, on: bool) -> bool {
        let Some(name) = self.node_flag_target(fi, ni) else {
            return false;
        };
        if on {
            let fig = &self.project.figures[fi];
            let has_grandparent = fig.def.nodes[ni]
                .parent
                .and_then(|par| fig.def.nodes.get(par))
                .and_then(|par| par.parent)
                .is_some();
            if !has_grandparent {
                self.status = "IK needs a chain of two segments above this joint.".into();
                return false;
            }
        }
        if self.project.figures[fi].def.nodes[ni].ik_end != on {
            self.snapshot_labeled("Segment IK end");
            self.project.figures[fi].def.nodes[ni].ik_end = on;
        }
        self.status = if on {
            format!("{name} is an IK end.")
        } else {
            format!("{name} is not an IK end.")
        };
        true
    }

    /// Set node `ni`'s IK bend side on figure `fi`
    /// (Roadmap Wave 2, W2-26 — RM19 §5 / IA-RM19-7):
    /// the segment inspector's Bend side −/+ control.
    /// `NodeDef::bend` is a sign by convention (model.rs:
    /// "+1 / -1"), so the write is exactly ±1.0, taken
    /// from `sign`'s sign; a zero or non-finite sign is
    /// refused ("Bend side must be − or +."). A real
    /// change spends exactly one snapshot labelled
    /// "Segment bend side"; writing the side the node
    /// already carries spends nothing and still reports
    /// the state. Refusals (no such figure or node,
    /// locked figure, bad sign) spend nothing and return
    /// false. This writes the DEF's preferred bend side
    /// only — no frame's stored bend is touched (that is
    /// the Bend stepper row's `Frame.bends` quantity).
    pub fn set_node_bend_side(&mut self, fi: usize, ni: usize, sign: f32) -> bool {
        let Some(name) = self.node_flag_target(fi, ni) else {
            return false;
        };
        if !sign.is_finite() || sign == 0.0 {
            self.status = "Bend side must be − or +.".into();
            return false;
        }
        let target = if sign < 0.0 { -1.0 } else { 1.0 };
        if self.project.figures[fi].def.nodes[ni].bend != target {
            self.snapshot_labeled("Segment bend side");
            self.project.figures[fi].def.nodes[ni].bend = target;
        }
        self.status = format!(
            "{name} bend side set to {}.",
            if target > 0.0 { "+1" } else { "-1" }
        );
        true
    }

    /// The segment length range every length door clamps
    /// with (Roadmap Wave 1, RM3 — F-3): these bounds used
    /// to live as inline literals inside `adjust_length`,
    /// so a typed value (the kit `NumberField`) had no one
    /// home to take its form from. The consts and the pure
    /// clamps below are that home: `adjust_length`,
    /// `set_length` and the panels' field specs all read
    /// the same numbers, so a stepper's form, the op's
    /// clamp and the field's validation cannot disagree
    /// (the StepForm discipline, made literal).
    pub const LENGTH_MIN: f32 = 4.0;
    /// See [`EditorCore::LENGTH_MIN`].
    pub const LENGTH_MAX: f32 = 220.0;
    /// The segment thickness range — see
    /// [`EditorCore::LENGTH_MIN`].
    pub const THICKNESS_MIN: f32 = 1.0;
    /// See [`EditorCore::THICKNESS_MIN`].
    pub const THICKNESS_MAX: f32 = 18.0;
    /// The taper range — see [`EditorCore::LENGTH_MIN`].
    /// (The write itself is `style::set_node_taper`.)
    pub const TAPER_MIN: f32 = 0.0;
    /// See [`EditorCore::TAPER_MIN`].
    pub const TAPER_MAX: f32 = 1.0;

    /// Clamp a segment length into
    /// [`EditorCore::LENGTH_MIN`]..=[`EditorCore::LENGTH_MAX`]
    /// — the one clamp every length write applies.
    pub fn length_clamped(value: f32) -> f32 {
        value.clamp(Self::LENGTH_MIN, Self::LENGTH_MAX)
    }

    /// Clamp a segment thickness — see
    /// [`EditorCore::length_clamped`].
    pub fn thickness_clamped(value: f32) -> f32 {
        value.clamp(Self::THICKNESS_MIN, Self::THICKNESS_MAX)
    }

    /// Clamp a taper — see [`EditorCore::length_clamped`].
    pub fn taper_clamped(value: f32) -> f32 {
        value.clamp(Self::TAPER_MIN, Self::TAPER_MAX)
    }

    /// Whether a length write of `value` through the
    /// doors below would actually change the selected
    /// segment (selection present, parented node, the
    /// clamped value different) — the doors snapshot on
    /// exactly this condition, so a clamped-to-same press
    /// spends no undo step.
    fn length_write_changes(&self, value: f32) -> bool {
        let Some(ni) = self.selected_node else {
            return false;
        };
        let Some(node) = self
            .project
            .figures
            .get(self.selected_figure)
            .and_then(|f| f.def.nodes.get(ni))
        else {
            return false;
        };
        node.parent.is_some() && Self::length_clamped(value) != node.length
    }

    pub fn adjust_length(&mut self, delta: f32) {
        // Roadmap Wave 1 (RM24, P-14 — A15 H3): the Wave 13
        // B2 stepper exemption is closed — a real change
        // takes an undo step, coalesced so a held stepper
        // run costs one step, not one per press.
        let cur = self.selected_node.and_then(|ni| {
            self.project
                .figures
                .get(self.selected_figure)?
                .def
                .nodes
                .get(ni)
                .filter(|n| n.parent.is_some())
                .map(|n| n.length)
        });
        if let Some(cur) = cur {
            if Self::length_clamped(cur + delta) != cur {
                self.snapshot_coalesced("Segment length");
            }
        }
        self.adjust_length_raw(delta);
    }

    /// The length step's write itself — status recap
    /// included, NO undo step and no dirty mark of its
    /// own: composed ops that record their own single
    /// snapshot (`scale_selected_figure` in the shell)
    /// call this per node, under their step.
    pub fn adjust_length_raw(&mut self, delta: f32) {
        let Some(ni) = self.selected_node else { return };
        let (name, length) = {
            let node = &mut self.project.figures[self.selected_figure].def.nodes[ni];
            if node.parent.is_none() {
                return;
            }
            node.length = Self::length_clamped(node.length + delta);
            (node.name.clone(), node.length)
        };
        self.status = format!("{} length {:.0}", name, length);
    }

    /// Set the selected segment's length to an exact value
    /// (Roadmap Wave 1, RM3 — F-3): the typed-entry
    /// counterpart of `adjust_length`, which is delta-only.
    /// Same selection rules (no selection, or the origin
    /// node, writes nothing and says nothing), the same
    /// clamp ([`EditorCore::length_clamped`]) and the same
    /// status wording, so a typed commit reads exactly
    /// like the steps that would have reached the same
    /// value. Discipline since RM24 (P-14, A15 H3): a real
    /// change takes a coalesced undo step (see
    /// `adjust_length`) — RM3's B2 mark-only discipline is
    /// superseded. The write itself is
    /// [`EditorCore::write_length`]; this door announces.
    pub fn set_length(&mut self, value: f32) {
        if self.length_write_changes(value) {
            self.snapshot_coalesced("Segment length");
        }
        self.write_length(value, true);
    }

    /// [`EditorCore::set_length`] without the status
    /// recap: the write a drag-scrub performs mid-gesture
    /// (mid-drag writes stay silent so a scrub does not
    /// spam the status line or the toasts fed from it; the
    /// gesture's release announces once, through
    /// `set_length`). The coalesced step makes the whole
    /// scrub one undo step, exactly like the stepper run.
    pub fn set_length_quiet(&mut self, value: f32) {
        if self.length_write_changes(value) {
            self.snapshot_coalesced("Segment length");
        }
        self.write_length(value, false);
    }

    /// The one length write behind `set_length` /
    /// `set_length_quiet`: `announce` gates only the
    /// status recap — the clamp and the selection rules
    /// are identical either way. Records nothing itself:
    /// the doors above own the undo step (and composed
    /// callers their own). Returns whether the value
    /// changed.
    fn write_length(&mut self, value: f32, announce: bool) -> bool {
        let Some(ni) = self.selected_node else {
            return false;
        };
        let (name, length, changed) = {
            let node = &mut self.project.figures[self.selected_figure].def.nodes[ni];
            if node.parent.is_none() {
                return false;
            }
            let next = Self::length_clamped(value);
            let changed = next != node.length;
            node.length = next;
            (node.name.clone(), node.length, changed)
        };
        if announce {
            self.status = format!("{} length {:.0}", name, length);
        }
        changed
    }

    /// Whether a thickness write of `value` would change
    /// the selected segment — the thickness doors'
    /// snapshot condition, as
    /// [`EditorCore::length_write_changes`] (no origin
    /// refusal here: thickness is a property of the node
    /// itself, not of a segment running from a parent).
    fn thickness_write_changes(&self, value: f32) -> bool {
        let Some(ni) = self.selected_node else {
            return false;
        };
        let Some(node) = self
            .project
            .figures
            .get(self.selected_figure)
            .and_then(|f| f.def.nodes.get(ni))
        else {
            return false;
        };
        Self::thickness_clamped(value) != node.thickness
    }

    pub fn adjust_thickness(&mut self, delta: f32) {
        // Roadmap Wave 1 (RM24, P-14 — A15 H3): as
        // `adjust_length` — a real change takes a
        // coalesced undo step.
        let cur = self.selected_node.and_then(|ni| {
            self.project
                .figures
                .get(self.selected_figure)?
                .def
                .nodes
                .get(ni)
                .map(|n| n.thickness)
        });
        if let Some(cur) = cur {
            if Self::thickness_clamped(cur + delta) != cur {
                self.snapshot_coalesced("Segment thickness");
            }
        }
        let Some(ni) = self.selected_node else { return };
        let (name, thickness) = {
            let node = &mut self.project.figures[self.selected_figure].def.nodes[ni];
            node.thickness = Self::thickness_clamped(node.thickness + delta);
            (node.name.clone(), node.thickness)
        };
        self.status = format!("{} thickness {:.0}", name, thickness);
    }

    /// Set the selected segment's thickness to an exact
    /// value (Roadmap Wave 1, RM3 — F-3): the typed-entry
    /// counterpart of `adjust_thickness`, under the same
    /// rules as [`EditorCore::set_length`] — the shared
    /// clamp ([`EditorCore::thickness_clamped`]), a
    /// coalesced undo step on a real change (RM24, P-14 —
    /// RM3's mark-only discipline superseded), the op's
    /// own status wording. (No origin refusal:
    /// `adjust_thickness` has none — thickness is a
    /// property of the node itself, not of a segment
    /// running from a parent.) The write itself is
    /// [`EditorCore::write_thickness`]; this door
    /// announces, [`EditorCore::set_thickness_quiet`]
    /// is the scrub's silent form.
    pub fn set_thickness(&mut self, value: f32) {
        if self.thickness_write_changes(value) {
            self.snapshot_coalesced("Segment thickness");
        }
        self.write_thickness(value, true);
    }

    /// [`EditorCore::set_thickness`] without the status
    /// recap — the drag-scrub form, as
    /// [`EditorCore::set_length_quiet`].
    pub fn set_thickness_quiet(&mut self, value: f32) {
        if self.thickness_write_changes(value) {
            self.snapshot_coalesced("Segment thickness");
        }
        self.write_thickness(value, false);
    }

    /// The one thickness write behind `set_thickness` /
    /// `set_thickness_quiet` — see
    /// [`EditorCore::write_length`] for the shape (the
    /// doors own the undo step; this records nothing).
    /// Returns whether the value changed.
    fn write_thickness(&mut self, value: f32, announce: bool) -> bool {
        let Some(ni) = self.selected_node else {
            return false;
        };
        let (name, thickness, changed) = {
            let node = &mut self.project.figures[self.selected_figure].def.nodes[ni];
            let next = Self::thickness_clamped(value);
            let changed = next != node.thickness;
            node.thickness = next;
            (node.name.clone(), node.thickness, changed)
        };
        if announce {
            self.status = format!("{} thickness {:.0}", name, thickness);
        }
        changed
    }

    const NODE_COLORS: [Option<u32>; 7] = [
        None,
        Some(0xFF5D5D),
        Some(0x5DB2FF),
        Some(0x7BE495),
        Some(0xFFC65D),
        Some(0xC792FF),
        Some(0x232830),
    ];

    /// Cycle the selected segment's shape: line -> circle -> triangle
    /// -> square -> pentagon -> line.
    pub fn cycle_shape(&mut self) {
        let Some(ni) = self.selected_node else { return };
        // Roadmap Wave 1 (RM24, P-14 — A15 H3): the B2
        // cycler exemption is closed — the shape always
        // changes, so the step is taken up front, coalesced
        // so a run of presses costs one undo step.
        self.snapshot_coalesced("Segment shape");
        let (name, shape) = {
            let node = &mut self.project.figures[self.selected_figure].def.nodes[ni];
            node.shape = match node.shape {
                crate::model::Shape::Line => crate::model::Shape::Circle,
                crate::model::Shape::Circle => crate::model::Shape::Polygon(3),
                crate::model::Shape::Polygon(n) if n < 6 => crate::model::Shape::Polygon(n + 1),
                crate::model::Shape::Polygon(_) => crate::model::Shape::Line,
            };
            (node.name.clone(), node.shape)
        };
        self.status = format!("{} shape: {:?}.", name, shape);
    }

    pub fn cycle_node_color(&mut self) {
        let Some(ni) = self.selected_node else { return };
        // Roadmap Wave 1 (RM24, P-14): as `cycle_shape`.
        self.snapshot_coalesced("Segment colour");
        let name = {
            let node = &mut self.project.figures[self.selected_figure].def.nodes[ni];
            let idx = Self::NODE_COLORS
                .iter()
                .position(|c| *c == node.color)
                .unwrap_or(0);
            node.color = Self::NODE_COLORS[(idx + 1) % Self::NODE_COLORS.len()];
            node.name.clone()
        };
        self.status = format!("{} colour changed.", name);
    }

    /// Set the selected segment's colour directly (the colour
    /// picker panel). Writes the same `NodeDef.color` the cycle
    /// op steps through, as `Some(rgb)`; since Roadmap Wave 1
    /// (RM24, P-14 — A15 H3) a write that actually changes the
    /// colour takes a coalesced undo step first, so a picker
    /// drag is one step, not a stack of them. (The shared
    /// `write_node_color` itself stays unrecorded: its other
    /// caller, `set_node_style_atomic`, snapshots.)
    pub fn set_node_color(&mut self, rgb: u32) {
        let before = self.selected_node.and_then(|ni| {
            self.project
                .figures
                .get(self.selected_figure)?
                .def
                .nodes
                .get(ni)
                .map(|n| n.color)
        });
        if let Some(before) = before {
            if before != Some(rgb) {
                self.snapshot_coalesced("Segment colour");
            }
        }
        self.write_node_color(rgb);
    }

    /// The colour write itself, without `set_node_color`'s
    /// (absent) snapshot: shared with `set_node_style_atomic`,
    /// which snapshots once for the whole style. Silent when
    /// nothing is selected, exactly like `set_node_color`.
    fn write_node_color(&mut self, rgb: u32) {
        let Some(ni) = self.selected_node else { return };
        let node = &mut self.project.figures[self.selected_figure].def.nodes[ni];
        node.color = Some(rgb);
        self.status = format!("{} colour changed.", node.name);
    }

    // ---- Camera ----

    pub fn camera_here(&self) -> (f32, f32, f32) {
        self.project.camera_at(self.current as f32)
    }

    pub fn camera_zoom_by(&mut self, factor: f32) {
        self.snapshot_labeled("Camera zoom");
        let (x, y, zoom) = self.camera_here();
        let zoom = (zoom * factor).clamp(0.3, 4.0);
        self.project.set_camera_key(self.current, x, y, zoom);
        self.status = format!("Camera zoom {zoom:.2} at frame {}.", self.current + 1);
    }

    pub fn camera_clear_key(&mut self) {
        // Roadmap Wave 1 (RM24, P-14 — A15 H3): the Wave 13
        // census A6 exemption is closed — a real removal
        // takes exactly one undo step, like the
        // `delete_camera_key` sibling always did. (The
        // panel button's "Clear key (no undo)" label is
        // retired with it — see IA-RM24-4; the panel's MCP
        // twin `clear_camera_key` moved in the same wave.)
        if self.project.camera.iter().any(|k| k.frame == self.current) {
            self.snapshot_labeled("Clear camera key");
        }
        if self.project.remove_camera_key(self.current) {
            self.status = "Camera key removed.".into();
        } else {
            self.status = "No camera key on this frame.".into();
        }
    }

    /// Upsert a camera key at the current frame (exactly one
    /// snapshot), preserving the camera angle in effect at this
    /// frame: `set_camera_key` alone would give a fresh key
    /// angle 0, snapping any interpolated rotation — the same
    /// fix-up `rotate_camera` applies.
    fn set_camera_key_here(&mut self, x: f32, y: f32, zoom: f32) {
        self.snapshot_labeled("Camera key");
        let frame = self.current;
        let angle = self.project.camera_angle_at(frame as f32);
        self.project.set_camera_key(frame, x, y, zoom);
        if let Some(k) = self.project.camera.iter_mut().find(|k| k.frame == frame) {
            k.angle = angle;
        }
    }

    /// Fit the camera to every visible figure at the current
    /// frame (the canvas Fit button): one camera key at the
    /// current frame, centred on the figures' joint bounds with a
    /// zoom that fits those bounds into the stage with a 10%
    /// margin, angle preserved.
    ///
    /// Camera convention (from the rasterizer's view closure in
    /// export.rs `render_frame_at`, mirrored by the canvas map in
    /// app.rs `paint_stage`): the key's (x, y) is the stage point
    /// shown at the centre of the view and a stage span of length
    /// L occupies L * zoom view units, so the view shows
    /// stage_w / zoom by stage_h / zoom stage units around the
    /// centre. Fitting bounds of bw x bh therefore needs
    /// zoom <= stage_w / bw and zoom <= stage_h / bh; the 10%
    /// margin divides that fit zoom by 1.1. A zero-size bounds
    /// dimension (every joint on one line) imposes no constraint,
    /// and a single-point bounds keeps the current zoom.
    pub fn frame_all_figures(&mut self) {
        let poses = self.project.poses_at(self.current as f32);
        let mut min = [f32::INFINITY; 2];
        let mut max = [f32::NEG_INFINITY; 2];
        let mut any = false;
        for (fi, fig) in self.project.figures.iter().enumerate() {
            if fig.hidden {
                continue;
            }
            let Some(pose) = poses.get(fi) else { continue };
            for p in evaluate(&fig.def, pose) {
                any = true;
                min[0] = min[0].min(p[0]);
                min[1] = min[1].min(p[1]);
                max[0] = max[0].max(p[0]);
                max[1] = max[1].max(p[1]);
            }
        }
        if !any {
            self.status = "No figures to frame.".into();
            return;
        }
        let centre = [(min[0] + max[0]) / 2.0, (min[1] + max[1]) / 2.0];
        let (bw, bh) = (max[0] - min[0], max[1] - min[1]);
        let mut fit = f32::INFINITY;
        if bw > 0.0 {
            fit = fit.min(self.project.stage[0] / bw);
        }
        if bh > 0.0 {
            fit = fit.min(self.project.stage[1] / bh);
        }
        let zoom = if fit.is_finite() {
            fit / 1.1
        } else {
            self.camera_here().2
        };
        self.set_camera_key_here(centre[0], centre[1], zoom);
        self.status = format!(
            "Camera fit all figures: zoom {zoom:.2} at frame {}.",
            self.current + 1
        );
    }

    /// Centre the camera on the selected figure's root at the
    /// current frame (the canvas Centre button): one camera key
    /// at the current frame with the current zoom and angle
    /// unchanged, only the centre moved to the root.
    pub fn centre_on_selection(&mut self) {
        if self.selected_figure >= self.project.figures.len() {
            self.status = "No figures to centre on.".into();
            return;
        }
        let poses_here = self.project.poses_at(self.current as f32);
        let Some(pose) = poses_here.get(self.selected_figure) else {
            self.status = "No figures to centre on.".into();
            return;
        };
        let root = pose.root;
        let zoom = self.camera_here().2;
        let name = self.project.figures[self.selected_figure].name.clone();
        self.set_camera_key_here(root[0], root[1], zoom);
        self.status = format!("Camera centred on {name} at frame {}.", self.current + 1);
    }

    pub fn add_quadruped(&mut self) {
        self.snapshot_labeled("Add quadruped");
        let idx = self.project.figures.len();
        let color = FIGURE_COLORS[idx % FIGURE_COLORS.len()];
        let root = [260.0 + 60.0 * idx as f32, 430.0];
        self.project
            .add_figure_with(FigureDef::quadruped(), &format!("Quad {}", idx + 1), color, root);
        self.selected_figure = idx;
        self.selected_node = None;
        self.status = "Quadruped added.".into();
    }

    pub fn add_simple(&mut self) {
        self.snapshot_labeled("Add figure");
        let idx = self.project.figures.len();
        let color = FIGURE_COLORS[(idx + 2) % FIGURE_COLORS.len()];
        let root = [340.0 + 70.0 * idx as f32, 470.0];
        self.project
            .add_figure_with(FigureDef::simple(), &format!("Mini {}", idx + 1), color, root);
        self.selected_figure = idx;
        self.selected_node = None;
        self.status = "Mini figure added.".into();
    }

    pub fn goto(&mut self, frame: usize) {
        if frame < self.project.frames.len() {
            self.current = frame;
            self.playhead = frame as f32;
        }
    }

    /// Seek the playhead to a FRACTIONAL frame time — the
    /// sub-frame scrub (Roadmap Wave 2, W2-40). The playhead
    /// lands exactly where the scrub dropped it, inside the
    /// frame `current` names: **`current` stays the authoring
    /// frame, `current == floor(playhead)`, the same relation
    /// `advance_play` maintains** — every authoring op in this
    /// core keys on `current`, so an edit made mid-scrub lands
    /// on the frame the playhead is inside, never on a
    /// fractional frame of its own. Like `goto` this is
    /// session state: no snapshot, no status line.
    ///
    /// The domain is `[0, n)`: frame time `n` is the end of
    /// the timeline (the beat walk's codomain tops out below
    /// it), so the clamp stops a hair short of it; the final
    /// frame's interior stays reachable. A non-finite time or
    /// an empty project refuses silently, like a `goto` past
    /// the end. Whether the stage actually SHOWS the
    /// in-between pose at this position is `display_frame`'s
    /// decision (the project's tween-playback flag), not the
    /// seek's: the playhead is fractional after a scrub
    /// regardless of that view preference.
    pub fn seek_fractional(&mut self, frame_time: f32) {
        let n = self.project.frames.len();
        if n == 0 || !frame_time.is_finite() {
            return;
        }
        let t = frame_time.clamp(0.0, n as f32 - 0.001);
        self.playhead = t;
        self.current = (t.floor() as usize).min(n - 1);
    }

    pub fn advance(&mut self) {
        if !self.project.frames.is_empty() {
            self.current = (self.current + 1) % self.project.frames.len();
            self.playhead = self.current as f32;
        }
    }

    /// Advance the playhead by wall-clock seconds, honouring fps and
    /// per-frame durations. Loops at the end — or, when a loop range
    /// is set, at the end of the range (see `set_loop_range`).
    ///
    /// Roadmap Wave 1 (RM7 / F-6): the advance walks the timeline
    /// on the project's shared duration-weighted clock
    /// (`Project::beats_at_frame_time` / `frame_time_at_beats`):
    /// the tick's beats are spent frame by frame, each frame
    /// charged at its own duration, so a tick that crosses a
    /// boundary lands part-way through the next frame at THAT
    /// frame's rate. The pre-RM7 shape charged the whole tick at
    /// the rate of the frame current when the tick began, which
    /// mis-timed every boundary crossing — the disagreement the
    /// audio side's `expected_frame_walk` documented against it.
    /// Folding happens in beats: the loop window is the range's
    /// beat span (or the whole timeline's), and overshoot folds
    /// back by that span, preserving the fractional remainder.
    pub fn advance_play(&mut self, dt_secs: f32) {
        let n = self.project.frames.len();
        if n == 0 {
            return;
        }
        let mut beats = self.project.beats_at_frame_time(self.playhead);
        if dt_secs.is_finite() && dt_secs > 0.0 {
            beats += dt_secs * self.project.fps.max(1.0);
        }
        let (fold_start, fold_end) = match self.loop_range {
            Some((a, b)) => {
                // Clamp defensively: frames may have been deleted
                // since the range was set. The range is inclusive,
                // so the window ends at the end of frame b; a
                // playhead below a (e.g. after a `goto` outside
                // the range) plays forward into the range and
                // only folds once it reaches that end.
                let b = b.min(n - 1);
                let a = a.min(b);
                (
                    self.project.beats_at_frame_time(a as f32),
                    self.project.beats_at_frame_time((b + 1) as f32),
                )
            }
            None => (0.0, self.project.timeline_beats()),
        };
        let span = fold_end - fold_start;
        if span > 0.0 && beats >= fold_end {
            beats = fold_start + (beats - fold_start).rem_euclid(span);
        }
        self.playhead = self.project.frame_time_at_beats(beats);
        self.current = (self.playhead.floor() as usize).min(n - 1);
    }

    /// Advance the playhead by wall-clock seconds, ping-pong
    /// style (Roadmap Wave 2, W2-40): travel runs forward to
    /// the end of the loop window, REVERSES, travels back to
    /// its start, reverses again — continuously, never
    /// wrapping and never stopping. The shell's poll calls
    /// this instead of [`EditorCore::advance_play`] while the
    /// ping-pong loop mode is on; the mode itself is shell
    /// state (see `rune_sec::playback_controls`), exactly as
    /// the loop on/off policy is — this op is only the
    /// mechanics, beside the forward op it mirrors.
    ///
    /// The walk is the same one clock `advance_play` walks
    /// (RM7 / F-6): the tick's beats are measured on the
    /// project's duration-weighted beat line, inside the same
    /// loop window (the `loop_range`'s beat span, clamped as
    /// `advance_play` clamps it, or the whole timeline's).
    /// Reflection happens in beats: the position is unfolded
    /// onto a triangle wave of period `2 × span` — phase
    /// `[0, span]` travels forward from the window's start,
    /// `(span, 2·span)` travels back from its end — stepped
    /// by the tick, then folded back into the window, which
    /// sets both the new playhead and [`EditorCore::
    /// play_reverse`]. A tick larger than the window (a
    /// one-frame loop at a coarse rate) folds correctly in
    /// one step; no iteration is involved. Overshoot past a
    /// bound is preserved as travel back into the window —
    /// the mirror of `advance_play`'s fold preserving the
    /// fractional remainder — so the bounce loses no time.
    ///
    /// A playhead parked OUTSIDE the window (a `goto` beyond
    /// the range) is first driven toward it: forward when
    /// below the window, backward when above, entering
    /// without a teleport; only travel inside the window
    /// reflects. Direction survives seeks (see
    /// `play_reverse`), so bouncing resumes where it left
    /// off. A tick that is not finite and positive moves
    /// nothing, the forward op's guard. An empty project, or
    /// a window with no beat span, stands still.
    pub fn advance_ping_pong(&mut self, dt_secs: f32) {
        let n = self.project.frames.len();
        if n == 0 {
            return;
        }
        if !dt_secs.is_finite() || dt_secs <= 0.0 {
            return;
        }
        let (fold_start, fold_end) = match self.loop_range {
            Some((a, b)) => {
                // The same defensive clamp `advance_play`
                // applies: frames may have been deleted since
                // the range was set.
                let b = b.min(n - 1);
                let a = a.min(b);
                (
                    self.project.beats_at_frame_time(a as f32),
                    self.project.beats_at_frame_time((b + 1) as f32),
                )
            }
            None => (0.0, self.project.timeline_beats()),
        };
        let span = fold_end - fold_start;
        if span <= 0.0 {
            return;
        }
        let step = dt_secs * self.project.fps.max(1.0);
        let mut beats = self.project.beats_at_frame_time(self.playhead);
        // Outside the window: aim at it first, entering
        // monotonically this tick; a step that reaches into
        // the window falls through to the fold below with
        // the direction of entry (and is not stepped twice).
        let mut stepped = false;
        if beats < fold_start {
            self.play_reverse = false;
            beats += step;
            stepped = true;
            if beats < fold_start {
                self.playhead = self.project.frame_time_at_beats(beats);
                self.current = (self.playhead.floor() as usize).min(n - 1);
                return;
            }
        } else if beats > fold_end {
            self.play_reverse = true;
            beats -= step;
            stepped = true;
            if beats > fold_end {
                self.playhead = self.project.frame_time_at_beats(beats);
                self.current = (self.playhead.floor() as usize).min(n - 1);
                return;
            }
        }
        // Unfold onto the triangle wave: forward travel is
        // phase [0, span], backward travel is (span, 2·span)
        // measured back from the window's end.
        let rel = beats - fold_start;
        let mut phase = if self.play_reverse {
            2.0 * span - rel
        } else {
            rel
        };
        if !stepped {
            phase += step;
        }
        phase = phase.rem_euclid(2.0 * span);
        let (offset, reverse) = if phase <= span {
            (phase, false)
        } else {
            (2.0 * span - phase, true)
        };
        self.play_reverse = reverse;
        // A landing exactly on the far bound commits a hair
        // inside it: `frame_time_at_beats(fold_end)` is the
        // START of the frame after a loop range (and clamps
        // to the last frame's start on a whole timeline), so
        // the un-nudged value would flash a frame outside the
        // window for that one tick. The nudge is in beats
        // (a ten-thousandth of a frame period at duration
        // 1.0) — no visible position changes.
        let commit = if offset >= span {
            fold_end - 0.0001
        } else {
            fold_start + offset
        };
        self.playhead = self.project.frame_time_at_beats(commit);
        self.current = (self.playhead.floor() as usize).min(n - 1);
    }

    /// Frame time the canvas should draw: the fractional
    /// playhead while playing with tween playback on,
    /// otherwise the authored frame.
    ///
    /// Roadmap Wave 2 (W2-40): "while playing" is now "while
    /// the playhead sits between frames" — with tween
    /// playback on, a fractional playhead shows its
    /// interpolated pose whether playback put it there (a
    /// pause mid-frame now HOLDS the in-between instead of
    /// snapping back to the authored frame) or a sub-frame
    /// scrub did ([`EditorCore::seek_fractional`]). That is
    /// the one honest rule: the tween flag governs whether
    /// in-betweens render at all, and every fractional
    /// position renders the same way. With tween off the
    /// authored frame shows, exactly as before, however the
    /// playhead got between frames. Every playhead writer
    /// keeps `current == floor(playhead)`, so an integer
    /// playhead renders identically under both branches.
    pub fn display_frame(&self) -> f32 {
        if self.project.tween_playback && (self.playing || self.playhead.fract() != 0.0) {
            self.playhead
        } else {
            self.current as f32
        }
    }

    // ---- Wave 7: timeline depth --------------------------------------

    /// Set the inclusive loop range [a, b] the playhead wraps inside
    /// during playback (see `advance_play`). The endpoints are
    /// ordered and clamped to the timeline; a == b is a valid
    /// single-frame loop. This is editor runtime state, not project
    /// data, so it is not snapshotted for undo.
    pub fn set_loop_range(&mut self, a: usize, b: usize) {
        let n = self.project.frames.len();
        if n == 0 {
            self.loop_range = None;
            self.status = "No frames to loop.".into();
            return;
        }
        let (a, b) = if a <= b { (a, b) } else { (b, a) };
        let b = b.min(n - 1);
        let a = a.min(b);
        self.loop_range = Some((a, b));
        self.status = format!("Looping frames {}–{}.", a + 1, b + 1);
    }

    pub fn clear_loop_range(&mut self) {
        self.loop_range = None;
        self.status = "Loop range cleared.".into();
    }

    /// Where an index lands after the block [from, from+count) is
    /// moved to start at `target`: indices inside the block move
    /// with it, indices the block jumps over shift by `count`
    /// against the move, everything else stays.
    fn remap_after_shift(idx: usize, from: usize, count: usize, target: usize) -> usize {
        if idx >= from && idx < from + count {
            idx - from + target
        } else if target > from && idx >= from + count && idx < target + count {
            idx - count
        } else if target < from && idx >= target && idx < from {
            idx + count
        } else {
            idx
        }
    }

    /// After a frame-block move, keep `current` and the loop range
    /// pointing at the same frames they pointed at before.
    fn remap_after_frame_move(&mut self, from: usize, count: usize, target: usize) {
        self.current = Self::remap_after_shift(self.current, from, count, target);
        self.playhead = self.current as f32;
        if let Some((a, b)) = self.loop_range {
            let a = Self::remap_after_shift(a, from, count, target);
            let b = Self::remap_after_shift(b, from, count, target);
            self.loop_range = Some(if a <= b { (a, b) } else { (b, a) });
        }
    }

    /// Move the block of `count` frames starting at `from` by
    /// `delta` positions (negative = earlier, positive = later).
    /// Frames are values — poses, bends, durations and easings all
    /// travel with their frame automatically. Returns false and
    /// changes nothing when the block or its destination would fall
    /// outside the timeline (or the move is empty).
    pub fn shift_frames(&mut self, from: usize, count: usize, delta: i32) -> bool {
        let n = self.project.frames.len();
        // Wave 11 (E6): `from + count` must not be computed
        // unchecked — `count` arrives from MCP arguments
        // (`shift_frames`), where a huge count wrapped the
        // sum in release builds, passed this guard, and
        // panicked in the `drain` below (start > end). A
        // sum that cannot be represented is an out-of-range
        // block: refuse it like any other.
        let block_fits = from
            .checked_add(count)
            .map(|end| end <= n)
            .unwrap_or(false);
        if count == 0 || delta == 0 || from >= n || !block_fits {
            return false;
        }
        let target = from as i32 + delta;
        if target < 0 || target as usize + count > n {
            return false;
        }
        let target = target as usize;
        self.snapshot_labeled("Shift frames");
        // Drain the block, then re-insert it at `target`. The
        // insert index is the same in the drained vec for both
        // directions: exactly the frames that must precede the
        // block (everything before it originally, plus the frames
        // it jumps over) sit in front of `target` after the drain.
        let block: Vec<_> = self.project.frames.drain(from..from + count).collect();
        for (i, frame) in block.into_iter().enumerate() {
            self.project.frames.insert(target + i, frame);
        }
        self.remap_after_frame_move(from, count, target);
        // Roadmap Wave 1 (RM18, P-6b): camera keys ride the
        // same permutation as the playhead — a key on a moved
        // frame travels with it, a key the block jumps over
        // shifts against the move. Re-sorted afterwards:
        // `set_camera_key`'s invariant is an ascending list.
        for key in &mut self.project.camera {
            key.frame = Self::remap_after_shift(key.frame, from, count, target);
        }
        self.project.camera.sort_by_key(|k| k.frame);
        self.status = format!(
            "Moved {count} frame(s) to position {}.",
            target + 1
        );
        true
    }

    /// Reverse the block of `count` frames starting at `from`, in
    /// place — poses, bends, durations and easings reverse with
    /// their frames. Out-of-range blocks are clamped; a block of
    /// fewer than two frames is a no-op.
    pub fn reverse_range(&mut self, from: usize, count: usize) {
        let n = self.project.frames.len();
        if from >= n {
            return;
        }
        let count = count.min(n - from);
        if count < 2 {
            return;
        }
        self.snapshot_labeled("Reverse frames");
        self.project.frames[from..from + count].reverse();
        let last = from + count - 1;
        let flip = |idx: usize| {
            if idx >= from && idx <= last {
                from + (last - idx)
            } else {
                idx
            }
        };
        self.current = flip(self.current);
        self.playhead = self.current as f32;
        if let Some((a, b)) = self.loop_range {
            let (a, b) = (flip(a), flip(b));
            self.loop_range = Some(if a <= b { (a, b) } else { (b, a) });
        }
        // Roadmap Wave 1 (RM18, P-6b): camera keys inside
        // the reversed block flip with their frames.
        for key in &mut self.project.camera {
            key.frame = flip(key.frame);
        }
        self.project.camera.sort_by_key(|k| k.frame);
        self.status = format!("Reversed frames {}–{}.", from + 1, from + count);
    }

    /// After frames [from, from+count) are deleted, keep `current`
    /// and the loop range attached to surviving frames where they
    /// can. A current frame inside the deleted block lands on the
    /// next survivor (or the last survivor when the tail was cut);
    /// a loop range that only overlapped the block shrinks to its
    /// surviving part, and one wholly inside it is cleared.
    fn remap_after_frame_delete(&mut self, from: usize, count: usize) {
        let end = from + count;
        let new_len = self.project.frames.len();
        if self.current >= from {
            self.current = if self.current < end {
                from.min(new_len - 1)
            } else {
                self.current - count
            };
        }
        self.playhead = self.current as f32;
        if let Some((a, b)) = self.loop_range {
            let has_prefix = a < from;
            let has_suffix = b >= end;
            self.loop_range = match (has_prefix, has_suffix) {
                (false, false) => None,
                (true, true) => Some((a, b - count)),
                (true, false) => Some((a, b.min(from - 1))),
                (false, true) => {
                    let a = if a >= end { a - count } else { from };
                    Some((a, b - count))
                }
            };
        }
        // Roadmap Wave 1 (RM18, P-6b): camera keys follow
        // the delete mapping — a key on a deleted frame is
        // removed with its content, a key after the block
        // shifts earlier by `count`. Keys before the block
        // are untouched; ascending order is preserved, so
        // no re-sort is needed.
        self.project.camera.retain(|k| k.frame < from || k.frame >= end);
        for key in &mut self.project.camera {
            if key.frame >= end {
                key.frame -= count;
            }
        }
    }

    /// Delete the block of `count` frames starting at `from`.
    /// Poses, bends, durations and easings all live in `Frame`, so
    /// removal is structural. The count is clamped to the timeline
    /// end, like `reverse_range`; an empty or out-of-range block is
    /// refused, as is deleting every frame — a project always keeps
    /// at least one frame. Camera keys ARE remapped (RM18,
    /// P-6b: keys on deleted frames are removed with their
    /// content, later keys shift earlier); other
    /// frame-indexed project data outside `Frame` (FX clip
    /// start frames) is NOT remapped.
    pub fn delete_range(&mut self, from: usize, count: usize) -> bool {
        let n = self.project.frames.len();
        if count == 0 || from >= n {
            return false;
        }
        let count = count.min(n - from);
        if count == n {
            return false;
        }
        self.snapshot_labeled("Delete frames");
        self.project.frames.drain(from..from + count);
        self.remap_after_frame_delete(from, count);
        self.status = format!(
            "Deleted {count} frame(s) (frames {}–{}).",
            from + 1,
            from + count
        );
        true
    }

    /// Copy the block of `count` frames starting at `from` into the
    /// range clipboard as deep clones, ready for `paste_range`. The
    /// count is clamped to the timeline end; an empty or
    /// out-of-range block is refused and leaves any previous range
    /// clipboard untouched. This does not use the single-frame
    /// clipboard and, like `copy_frame`, is not an undoable edit.
    pub fn copy_range(&mut self, from: usize, count: usize) -> bool {
        let n = self.project.frames.len();
        if count == 0 || from >= n {
            return false;
        }
        let count = count.min(n - from);
        self.frame_clipboard = self.project.frames[from..from + count].to_vec();
        self.status = format!(
            "Copied {count} frame(s) (frames {}–{}).",
            from + 1,
            from + count
        );
        true
    }

    /// After frames are inserted at `at`, keep `current` and the
    /// loop range pointing at the same frames they pointed at
    /// before: indices at or after the insertion point shift later.
    fn remap_after_frame_insert(&mut self, at: usize, count: usize) {
        if self.current >= at {
            self.current += count;
        }
        self.playhead = self.current as f32;
        if let Some((a, b)) = self.loop_range {
            let a = if a >= at { a + count } else { a };
            let b = if b >= at { b + count } else { b };
            self.loop_range = Some((a, b));
        }
        // Roadmap Wave 1 (RM18, P-6b): camera keys at or
        // after the paste point shift later with the frames
        // they were authored against (the same rule the
        // playhead follows above). Ascending order is
        // preserved, so no re-sort is needed.
        for key in &mut self.project.camera {
            if key.frame >= at {
                key.frame += count;
            }
        }
    }

    /// Insert the range clipboard's frames at index `at` (clamped
    /// to the timeline, so a large `at` appends), shifting later
    /// frames. Refused when the range clipboard is empty, or when a
    /// copied frame's figure count no longer matches the project
    /// (the same guard as `paste_frame`). Frame-indexed project
    /// data outside `Frame`. Camera keys ARE remapped (RM18,
    /// P-6b: keys at or after the paste point shift later
    /// with their frames); other frame-indexed project data
    /// (FX clip start frames) is NOT remapped.
    pub fn paste_range(&mut self, at: usize) -> bool {
        if self.frame_clipboard.is_empty() {
            return false;
        }
        if self
            .frame_clipboard
            .iter()
            .any(|frame| frame.poses.len() != self.project.figures.len())
        {
            self.status = "Copied frames have a different figure count.".into();
            return false;
        }
        // Wave 11 (E6): the same stale-rig refusal as
        // `paste_frame` — a clipboard frame copied before
        // a segment edit carries fewer angles than the
        // grown definition, and pasting it would plant a
        // render panic in `model::evaluate`.
        if self
            .frame_clipboard
            .iter()
            .any(|frame| !self.frame_poses_match(frame))
        {
            self.status = "Copied frames' poses no longer match the figures' rigs.".into();
            return false;
        }
        let at = at.min(self.project.frames.len());
        let block = self.frame_clipboard.clone();
        let count = block.len();
        self.snapshot_labeled("Paste frames");
        for (i, frame) in block.into_iter().enumerate() {
            self.project.frames.insert(at + i, frame);
        }
        self.remap_after_frame_insert(at, count);
        self.status = format!("Pasted {count} frame(s) at position {}.", at + 1);
        true
    }

    /// Motion-path editing primitive: move the ROOT of figure `fi`'s
    /// pose at frame `frame` by `delta`. The roots across frames
    /// are what the motion path draws, so this bends the path at
    /// exactly one point; every other frame, and this frame's
    /// angles, are untouched.
    pub fn nudge_path_point(&mut self, frame: usize, fi: usize, delta: V2) {
        let valid = self
            .project
            .frames
            .get(frame)
            .map(|f| fi < f.poses.len())
            .unwrap_or(false);
        if !valid {
            self.status = "No such frame or figure to nudge.".into();
            return;
        }
        self.snapshot_labeled("Nudge motion path");
        let root = &mut self.project.frames[frame].poses[fi].root;
        root[0] += delta[0];
        root[1] += delta[1];
        self.status = format!(
            "Nudged path point at frame {} by ({:.0}, {:.0}).",
            frame + 1,
            delta[0],
            delta[1]
        );
    }

    /// One smoothing pass over figure `fi`'s motion path: each
    /// interior frame's root becomes the midpoint of its two
    /// neighbours' roots, computed from the pre-pass values (a
    /// single spike collapses to zero). The first and last frames
    /// keep their roots, and pose angles are never touched.
    pub fn smooth_path_roots(&mut self, fi: usize) {
        let n = self.project.frames.len();
        if n < 3 || fi >= self.project.figures.len() {
            self.status = "Nothing to smooth.".into();
            return;
        }
        // Compute the whole pass from the original roots first, so
        // a smoothed point never feeds its own neighbours.
        let roots: Vec<Option<V2>> = self
            .project
            .frames
            .iter()
            .map(|f| f.poses.get(fi).map(|p| p.root))
            .collect();
        let mut updates: Vec<(usize, V2)> = Vec::new();
        for f in 1..n - 1 {
            if let (Some(prev), Some(cur), Some(next)) = (roots[f - 1], roots[f], roots[f + 1]) {
                let mid = [(prev[0] + next[0]) / 2.0, (prev[1] + next[1]) / 2.0];
                if mid != cur {
                    updates.push((f, mid));
                }
            }
        }
        if updates.is_empty() {
            self.status = "Motion path already smooth.".into();
            return;
        }
        self.snapshot_labeled("Smooth motion path");
        let changed = updates.len();
        for (f, root) in updates {
            self.project.frames[f].poses[fi].root = root;
        }
        self.status = format!(
            "Smoothed motion path of figure {} ({changed} point(s)).",
            fi + 1
        );
    }

    /// How far (stage units) a path point may sit from the
    /// straight line between its neighbours and still count
    /// as lying ON that line (Roadmap Wave 2, W2-36): below
    /// this, deleting the point would change nothing the eye
    /// can see, so [`Self::delete_path_point`] refuses
    /// instead of spending an undo step on a no-op. Sub-unit
    /// on purpose: a genuine corner, however small, is a
    /// point the user authored.
    pub const PATH_POINT_STRAIGHT_EPS: f32 = 0.05;

    /// The root the path would pass through at `frame` if
    /// figure `fi`'s point there were deleted: the midpoint
    /// of the neighbouring frames' roots (a frame's two
    /// neighbours are equidistant in time, so the
    /// re-interpolation is the plain midpoint). `None` when
    /// the point has no pair of neighbours to interpolate
    /// between — the first or last frame, an out-of-range
    /// frame or figure, or a frame where the figure has no
    /// pose. Pure: the read-side half of
    /// [`Self::delete_path_point`], shared with the stage
    /// and panel guards that mirror it.
    pub fn path_point_reinterpolation(&self, frame: usize, fi: usize) -> Option<V2> {
        if frame == 0 {
            return None;
        }
        let prev = self.project.frames.get(frame - 1)?.poses.get(fi)?.root;
        // The point itself must exist to be deletable.
        self.project.frames.get(frame)?.poses.get(fi)?;
        let next = self.project.frames.get(frame + 1)?.poses.get(fi)?.root;
        Some([(prev[0] + next[0]) / 2.0, (prev[1] + next[1]) / 2.0])
    }

    /// Insert a motion-path point for figure `fi` in the
    /// segment after frame `after` (Roadmap Wave 2, W2-36):
    /// a NEW frame lands at `after + 1`, splitting the
    /// segment — the dense model's only way to add a point
    /// between two frames. Every figure's pose in the new
    /// frame is the `t`-lerp of its poses in the two split
    /// frames (a figure present on only one side is carried
    /// from that side), and figure `fi`'s root is then set
    /// to `root` — the point on the segment the user
    /// pressed, so the split itself does not move the path;
    /// the new point is there to be dragged afterwards.
    /// The new frame copies frame `after`'s duration and
    /// carries no bends (the `add_frame` /
    /// `insert_inbetween` precedents). Frame-indexed data
    /// remaps through [`Self::remap_after_frame_insert`]
    /// (playhead, loop range, camera keys — the paste
    /// rule); collections that remap does not cover follow
    /// the standing paste precedent and are not shifted.
    /// Exactly one undo step. The playhead lands on the
    /// new frame, whose point the pose drag then owns (the
    /// current-frame rule). Refusals (false, no snapshot):
    /// no segment after `after`, `t` outside (0, 1) or
    /// non-finite, a non-finite `root`, or figure `fi`
    /// missing from either split frame.
    pub fn insert_path_point(&mut self, after: usize, fi: usize, t: f32, root: V2) -> bool {
        let valid = after + 1 < self.project.frames.len()
            && t.is_finite()
            && t > 0.0
            && t < 1.0
            && root.iter().all(|v| v.is_finite())
            && self
                .project
                .frames
                .get(after)
                .and_then(|f| f.poses.get(fi))
                .is_some()
            && self
                .project
                .frames
                .get(after + 1)
                .and_then(|f| f.poses.get(fi))
                .is_some();
        if !valid {
            self.status = "No path segment there to split.".into();
            return false;
        }
        let (a, b) = (&self.project.frames[after], &self.project.frames[after + 1]);
        let len = a.poses.len().max(b.poses.len());
        let mut poses: Vec<crate::model::Pose> = Vec::with_capacity(len);
        for g in 0..len {
            match (a.poses.get(g), b.poses.get(g)) {
                (Some(pa), Some(pb)) => poses.push(pa.lerp(pb, t)),
                (Some(pa), None) => poses.push(pa.clone()),
                (None, Some(pb)) => poses.push(pb.clone()),
                (None, None) => unreachable!("g is below the longer pose vec"),
            }
        }
        poses[fi].root = root;
        let duration = a.duration;
        self.snapshot_labeled("Insert path point");
        self.project.frames.insert(
            after + 1,
            crate::model::Frame { poses, duration, easing: None, bends: Vec::new() },
        );
        self.remap_after_frame_insert(after + 1, 1);
        self.current = after + 1;
        self.playhead = self.current as f32;
        self.status = format!("Inserted path point at frame {}.", after + 2);
        true
    }

    /// Delete figure `fi`'s motion-path point at `frame`
    /// (Roadmap Wave 2, W2-36): the frame's root becomes
    /// [`Self::path_point_reinterpolation`] — the path
    /// re-interpolates straight through where the point
    /// was, and the point stops deflecting it. Only the
    /// root is written: angles, squash and bends at the
    /// frame are the pose's, not the path point's, and the
    /// frame itself stays (this is a point delete, not a
    /// frame delete). Exactly one undo step. Refusals
    /// (false, no snapshot): the first or last frame (the
    /// endpoints anchor the path and have no neighbour
    /// pair), a missing pose, and a point already lying on
    /// the line between its neighbours within
    /// [`Self::PATH_POINT_STRAIGHT_EPS`] (deleting it would
    /// change nothing).
    pub fn delete_path_point(&mut self, frame: usize, fi: usize) -> bool {
        if frame == 0 || frame + 1 >= self.project.frames.len() {
            self.status =
                "The first and last path points anchor the path and cannot be deleted.".into();
            return false;
        }
        let Some(mid) = self.path_point_reinterpolation(frame, fi) else {
            self.status = format!("No path point for figure {} at frame {}.", fi + 1, frame + 1);
            return false;
        };
        let cur = self.project.frames[frame].poses[fi].root;
        let deflection = (cur[0] - mid[0]).hypot(cur[1] - mid[1]);
        if deflection <= Self::PATH_POINT_STRAIGHT_EPS {
            self.status = format!(
                "Frame {}'s path point already lies on the path between its neighbours.",
                frame + 1
            );
            return false;
        }
        self.snapshot_labeled("Delete path point");
        self.project.frames[frame].poses[fi].root = mid;
        self.status = format!(
            "Deleted path point at frame {} — the path runs straight through it.",
            frame + 1
        );
        true
    }

    /// Roadmap Wave 1 (RM6, F-5): the current project
    /// revision (see the `revision` field). Paired with a
    /// frame it is the only honest cache key for anything
    /// derived from the project — the Wave L lesson.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Roadmap Wave 1 (RM6, F-5): the display frame's full
    /// channel evaluation — poses, defs and bend rows —
    /// computed ONCE per (revision, frame) and shared by
    /// every consumer of this redraw. Before this funnel the
    /// stage paid the evaluation per figure (`positions_f`
    /// ran `poses_at` or `apply_channels` for every figure,
    /// and the bend fetch ran `apply_channels` a second time
    /// per figure: O(figures²) clone work per redraw).
    ///
    /// The evaluation itself is exactly
    /// [`crate::graph::apply_channels`] on the project's own
    /// channels, in both branches: with no channels its poses
    /// are `poses_at` verbatim and its defs are the figures'
    /// own defs (cloned, read identically), so
    /// [`Self::positions_f`] and the stage's bend rows return
    /// byte-identical results to the per-figure paths they
    /// replace — the only change is how often the work runs.
    pub fn evaluated_frame(
        &self,
        frame_f: f32,
    ) -> std::rc::Rc<crate::graph::ChannelResult> {
        let bits = frame_f.to_bits();
        {
            let cache = self.eval_cache.borrow();
            if let Some(hit) = cache
                .iter()
                .find(|(r, b, _)| *r == self.revision && *b == bits)
                .map(|(_, _, v)| v.clone())
            {
                return hit;
            }
        }
        let fresh = std::rc::Rc::new(crate::graph::apply_channels(
            &self.project,
            &self.project.channels,
            frame_f,
        ));
        let mut cache = self.eval_cache.borrow_mut();
        // Entries cut against older revisions can never be
        // served again (the counter is monotone); drop them
        // here so the cache only ever holds this revision's
        // frames, a handful at most (the display frame plus
        // scrub/onion neighbours).
        cache.retain(|(r, _, _)| *r == self.revision);
        if cache.len() >= 8 {
            cache.remove(0);
        }
        cache.push((self.revision, bits, fresh.clone()));
        fresh
    }

    pub fn positions_f(&self, frame_f: f32, fi: usize) -> Vec<V2> {
        // Roadmap Wave 1 (RM6, F-5): read through the one
        // shared evaluation (see `evaluated_frame`) instead
        // of re-running `poses_at` / `apply_channels` per
        // figure. Both former branches are the same values:
        // with channels, `apply_channels`' poses and defs
        // verbatim — the historical Wave 5 path; without,
        // `poses_at` poses against the figure's own def.
        let ev = self.evaluated_frame(frame_f);
        evaluate(&ev.defs[fi], &ev.poses[fi])
    }

    // ---- Wave 5 integration ops -------------------------------------
    /// Reorder the selected figure with the full layers engine:
    /// poses, graph channels, FX clips, particle refs and camera
    /// follow all travel with the figure.
    pub fn move_figure_full(&mut self, dir: i32) {
        let fi = self.selected_figure;
        let n = self.project.figures.len();
        if n == 0 {
            return;
        }
        let to = (fi as i32 + dir).clamp(0, n as i32 - 1) as usize;
        if to == fi {
            return;
        }
        self.snapshot_labeled("Reorder figure");
        if crate::layers::move_figure(&mut self.project, fi, dir) {
            self.selected_figure = crate::layers::remapped_index(self.selected_figure, fi, to);
            if let Some(solo) = self.solo {
                self.solo = Some(crate::layers::remapped_index(solo, fi, to));
            }
            self.status = format!("Figure moved to position {}.", to + 1);
        }
    }



    // ---- Camera preset moves (Roadmap Wave 2, W2-39) ----
    //
    // A preset move is ONE press = ONE labelled undo step:
    // a start key pinned with the camera exactly as it stands
    // at the current frame, and an end key `frames` frames
    // later carrying the preset's target. The start key's
    // `easing` carries the segment's timing (W2-32 — the
    // easing lives on the key the segment LEAVES), so a
    // preset's smoothness is data, set once here, and every
    // evaluator (stage, raster, SVG) times the move the same
    // way. Every value is computed from the camera in effect
    // and validated BEFORE the snapshot: a refusal spends
    // nothing. Zoom targets ride the same clamp every other
    // camera write here uses (0.3..=4.0 — `camera_zoom_by`).

    /// The maximum preset move length, in frames (2 s at
    /// 24 fps): past this a "preset" is really a hand-built
    /// shot, and the key list stays readable.
    pub const CAMERA_PRESET_MAX_FRAMES: usize = 48;

    /// The preset easing vocabulary: the segment timing a
    /// preset stamps on its start key. `smooth` is the
    /// default for panel presses (the brief's call — a
    /// one-press move should not jar); the cycle-button
    /// vocabulary (`CameraEase::label`) names the rest.
    /// `hold` is a legal choice — the move then cuts at the
    /// end frame instead of gliding.
    pub fn camera_preset_easing(label: &str) -> Option<crate::model::CameraEase> {
        crate::model::CameraEase::from_label(label)
    }

    /// Validate a preset request's shared parameters and
    /// resolve the move's geometry. On `Err` the returned
    /// string is the user-facing refusal, posted verbatim
    /// by the callers; nothing has been written and no
    /// snapshot taken. On `Ok`: `(start_frame, end_frame,
    /// start(x, y, zoom, angle), end(x, y, zoom, angle))`
    /// with every number finite and the end zoom clamped.
    fn camera_preset_span(
        &self,
        frames: usize,
        target: (f32, f32, f32),
    ) -> Result<(usize, usize, (f32, f32, f32, f32), (f32, f32, f32, f32)), String> {
        let count = self.project.frames.len();
        if count == 0 {
            return Err("No frames to move the camera over.".into());
        }
        if frames == 0 {
            return Err("A camera move needs at least 1 frame.".into());
        }
        if frames > Self::CAMERA_PRESET_MAX_FRAMES {
            return Err(format!(
                "Camera moves are at most {} frames.",
                Self::CAMERA_PRESET_MAX_FRAMES
            ));
        }
        let start = self.current;
        let end = start + frames;
        if end >= count {
            return Err(format!(
                "No room for a {frames}-frame camera move: only {} frame(s) remain.",
                count - 1 - start
            ));
        }
        let (sx, sy, szoom) = self.camera_here();
        let sangle = crate::model::norm_deg(self.project.camera_angle_at(start as f32));
        let (tx, ty, tzoom) = target;
        if ![sx, sy, szoom, sangle, tx, ty, tzoom].iter().all(|v| v.is_finite()) {
            return Err("The camera move would not be finite; nothing was written.".into());
        }
        let tzoom = tzoom.clamp(0.3, 4.0);
        Ok((start, end, (sx, sy, szoom, sangle), (tx, ty, tzoom, sangle)))
    }

    /// Write a validated preset span: ONE labelled snapshot,
    /// the start key pinned with the camera as it stands
    /// (its easing becomes `easing` — the segment's timing),
    /// and the end key carrying the target with the angle
    /// unchanged and a fresh key's Linear easing (its own
    /// following segment is untouched business). An existing
    /// key at either frame is upserted in place — pan/zoom
    /// replaced, angle fixed up by the explicit stamp (the
    /// `hold_camera` discipline).
    fn write_camera_preset(
        &mut self,
        span: (usize, usize, (f32, f32, f32, f32), (f32, f32, f32, f32)),
        easing: crate::model::CameraEase,
        label: &str,
        status: String,
    ) {
        let (start, end, s, e) = span;
        self.snapshot_labeled(label);
        self.project.set_camera_key(start, s.0, s.1, s.2);
        if let Some(k) = self.project.camera.iter_mut().find(|k| k.frame == start) {
            k.angle = s.3;
            k.easing = easing;
        }
        self.project.set_camera_key(end, e.0, e.1, e.2);
        if let Some(k) = self.project.camera.iter_mut().find(|k| k.frame == end) {
            k.angle = e.3;
        }
        self.status = status;
    }

    /// Push-in preset: zoom ×[`Self::CAMERA_PRESET_ZOOM_FACTOR`]
    /// over `frames` frames toward the point the camera's centre
    /// converges on —
    /// the selected figure's root when a figure is selected
    /// AND follow is off (under follow the root IS the
    /// centre every frame, so there is nothing to converge
    /// on), else the camera centre already in effect. The
    /// centre moves 3/4 of the way to that target; at the
    /// full zoom factor the target would leave the frame's
    /// centre, which reads as a miss, not a push-in. The
    /// zoom factor is the panel steppers' own `ZOOM_STEP`
    /// squared (1.25² ≈ 1.56× — two stepper presses of
    /// travel), stated so the number is a convention of the
    /// app, not a new magic constant.
    pub fn camera_push_in(&mut self, frames: usize, easing: crate::model::CameraEase) {
        let (cx, cy, zoom) = self.camera_here();
        let (tx, ty) = self.camera_preset_focus().unwrap_or((cx, cy));
        let target = (
            cx + (tx - cx) * 0.75,
            cy + (ty - cy) * 0.75,
            zoom * Self::CAMERA_PRESET_ZOOM_FACTOR,
        );
        match self.camera_preset_span(frames, target) {
            Ok(span) => {
                let end = span.1;
                self.write_camera_preset(
                    span,
                    easing,
                    "Camera push-in",
                    format!(
                        "Push-in: zoom {:.2} by frame {}.",
                        target.2.clamp(0.3, 4.0),
                        end + 1
                    ),
                );
            }
            Err(msg) => self.status = msg,
        }
    }

    /// Pull-out preset: the push-in's mirror — zoom
    /// ÷`CAMERA_PRESET_ZOOM_FACTOR`, centre easing 3/4 of
    /// the way back toward the stage centre (the wide
    /// shot's home). From a camera already centred on the
    /// stage this is a pure zoom-out.
    pub fn camera_pull_out(&mut self, frames: usize, easing: crate::model::CameraEase) {
        let (cx, cy, zoom) = self.camera_here();
        let (sx, sy) = (self.project.stage[0] / 2.0, self.project.stage[1] / 2.0);
        let target = (
            cx + (sx - cx) * 0.75,
            cy + (sy - cy) * 0.75,
            zoom / Self::CAMERA_PRESET_ZOOM_FACTOR,
        );
        match self.camera_preset_span(frames, target) {
            Ok(span) => {
                let end = span.1;
                self.write_camera_preset(
                    span,
                    easing,
                    "Camera pull-out",
                    format!(
                        "Pull-out: zoom {:.2} by frame {}.",
                        target.2.clamp(0.3, 4.0),
                        end + 1
                    ),
                );
            }
            Err(msg) => self.status = msg,
        }
    }

    /// Pan preset: the centre travels `dir` stage-widths
    /// horizontally (`dir` ±1; the distance is the visible
    /// width at the current zoom — stage_w / zoom — so a
    /// pan always crosses exactly one screenful, at any
    /// zoom), zoom and angle unchanged. `dir` is taken by
    /// its sign; 0 refuses.
    pub fn camera_pan(&mut self, dir: i32, frames: usize, easing: crate::model::CameraEase) {
        if dir == 0 {
            self.status = "A pan needs a direction.".into();
            return;
        }
        let (cx, cy, zoom) = self.camera_here();
        let step = self.project.stage[0] / zoom.max(0.05) * dir.signum() as f32;
        let target = (cx + step, cy, zoom);
        match self.camera_preset_span(frames, target) {
            Ok(span) => {
                let end = span.1;
                let name = if dir > 0 { "Pan right" } else { "Pan left" };
                self.write_camera_preset(
                    span,
                    easing,
                    name,
                    format!("{name} by frame {}.", end + 1),
                );
            }
            Err(msg) => self.status = msg,
        }
    }

    /// Truck preset: the centre travels `dir` stage-heights
    /// vertically (the visible height at the current zoom,
    /// stage_h / zoom — one screenful), zoom and angle
    /// unchanged. In a 2.5D stage this is the honest
    /// vertical dolly: the background and figures translate
    /// together, with no parallax claim. `dir` +1 moves the
    /// centre DOWN-stage (the view travels up); −1 up-stage.
    pub fn camera_truck(&mut self, dir: i32, frames: usize, easing: crate::model::CameraEase) {
        if dir == 0 {
            self.status = "A truck needs a direction.".into();
            return;
        }
        let (cx, cy, zoom) = self.camera_here();
        let step = self.project.stage[1] / zoom.max(0.05) * dir.signum() as f32;
        let target = (cx, cy + step, zoom);
        match self.camera_preset_span(frames, target) {
            Ok(span) => {
                let end = span.1;
                let name = if dir > 0 { "Truck down" } else { "Truck up" };
                self.write_camera_preset(
                    span,
                    easing,
                    name,
                    format!("{name} by frame {}.", end + 1),
                );
            }
            Err(msg) => self.status = msg,
        }
    }

    /// Arc preset — the honest 2D orbit: the centre swings
    /// around the focus point (see
    /// [`EditorCore::camera_preset_focus`]) by `dir` × 30°,
    /// keeping the centre-to-focus distance, so the subject
    /// stays framed while the background sweeps — the move
    /// a 2D camera can truthfully call an arc. It is NOT a
    /// 3D orbit: no viewpoint orbits the subject, nothing
    /// parallaxes, and the figures do not turn. The camera
    /// ANGLE is deliberately untouched: rotating the frame
    /// as well would make it a roll, a different move.
    /// Refuses (status-only, no snapshot) when there is no
    /// usable focus — follow off and no valid selection, or
    /// the camera already centred on the focus (a zero
    /// radius has no arc).
    pub fn camera_arc(&mut self, dir: i32, frames: usize, easing: crate::model::CameraEase) {
        if dir == 0 {
            self.status = "An arc needs a direction.".into();
            return;
        }
        let Some((fx, fy)) = self.camera_preset_focus() else {
            self.status = "Select a figure to arc around first.".into();
            return;
        };
        let (cx, cy, zoom) = self.camera_here();
        let (dx, dy) = (cx - fx, cy - fy);
        if dx * dx + dy * dy < 1.0 {
            self.status = "The camera is already centred on the figure; an arc needs an offset.".into();
            return;
        }
        let a = (30.0f32 * dir.signum() as f32).to_radians();
        let (sin, cos) = a.sin_cos();
        let target = (fx + dx * cos - dy * sin, fy + dx * sin + dy * cos, zoom);
        match self.camera_preset_span(frames, target) {
            Ok(span) => {
                let end = span.1;
                let name = if dir > 0 { "Arc right" } else { "Arc left" };
                self.write_camera_preset(
                    span,
                    easing,
                    name,
                    format!("{name} around the selected figure by frame {}.", end + 1),
                );
            }
            Err(msg) => self.status = msg,
        }
    }

    /// Handheld-shake preset (the board's "Camera shake"):
    /// a deterministic damped jitter AROUND the camera in
    /// effect, authored as real keys — a start key plus one
    /// key every 2 frames over `frames` frames at the
    /// current frame, offsets from the closed-form
    /// `camera_shake_offset` (seeded by the start frame, so
    /// the same press at the same frame reproduces the same
    /// shake, and re-pressing REPLACES rather than stacks).
    /// Amplitude `amp` is in stage units (the panel offers
    /// 6 ≈ a subtle hand-held tremor at stage scale). The
    /// move ends at rest (the final key is the unshaken
    /// camera), every key is Linear — the jitter is in the
    /// keys, not the timing — and the whole burst is ONE
    /// labelled undo step. Zoom and angle never move.
    /// Existing keys INSIDE the span are replaced (that is
    /// what authoring a shake over them means); keys
    /// outside are untouched.
    pub fn camera_shake(&mut self, frames: usize, amp: f32) {
        let count = self.project.frames.len();
        if count == 0 {
            self.status = "No frames to shake the camera over.".into();
            return;
        }
        if frames < 4 {
            self.status = "A shake needs at least 4 frames.".into();
            return;
        }
        if frames > Self::CAMERA_PRESET_MAX_FRAMES {
            self.status = format!(
                "Camera moves are at most {} frames.",
                Self::CAMERA_PRESET_MAX_FRAMES
            );
            return;
        }
        if !amp.is_finite() || amp <= 0.0 {
            self.status = "Shake amplitude must be a positive number of stage units.".into();
            return;
        }
        let start = self.current;
        let end = start + frames;
        if end >= count {
            self.status = format!(
                "No room for a {frames}-frame shake: only {} frame(s) remain.",
                count - 1 - start
            );
            return;
        }
        let (cx, cy, zoom) = self.camera_here();
        let angle = crate::model::norm_deg(self.project.camera_angle_at(start as f32));
        self.snapshot_labeled("Camera shake");
        let seed = start as u32;
        let mut f = start;
        loop {
            let (ox, oy) = Self::camera_shake_offset(seed, f - start, frames, amp);
            self.project.set_camera_key(f, cx + ox, cy + oy, zoom);
            if let Some(k) = self.project.camera.iter_mut().find(|k| k.frame == f) {
                k.angle = angle;
                k.easing = crate::model::CameraEase::Linear;
            }
            if f >= end {
                break;
            }
            f = (f + 2).min(end);
        }
        self.status = format!("Handheld shake over frames {}–{}.", start + 1, end + 1);
    }

    /// The shake's offset at `step` frames into a
    /// `frames`-long burst (closed form, the vfx.rs
    /// determinism discipline — the same inputs always give
    /// the same offset, on every surface): two incommensurate
    /// sine pairs phased by a per-burst seed, damped by
    /// (1 − step/frames)² so the burst settles, and exactly
    /// (0, 0) at both ends — the camera starts and lands at
    /// rest. Pure — unit-testable without a core.
    pub fn camera_shake_offset(seed: u32, step: usize, frames: usize, amp: f32) -> (f32, f32) {
        if frames == 0 || step == 0 || step >= frames {
            return (0.0, 0.0);
        }
        let t = step as f32 / frames as f32;
        let damp = (1.0 - t) * (1.0 - t);
        let ph = (seed % 360) as f32;
        let ox = amp
            * damp
            * (0.7 * (t * 25.13 + ph).sin() + 0.3 * (t * 61.0 + ph * 1.7).sin());
        let oy = amp * 0.7 * damp * (0.7 * (t * 21.99 + ph * 0.6).sin() + 0.3 * (t * 53.0 + ph).sin());
        (ox, oy)
    }

    /// The point preset moves converge on / swing around:
    /// the selected figure's root at the current frame —
    /// `centre_on_selection`'s target — or `None` when the
    /// selection is not a real figure OR follow is on (under
    /// follow the camera already tracks the followed figure;
    /// a preset focus would fight it). Pure read.
    pub fn camera_preset_focus(&self) -> Option<(f32, f32)> {
        if self.project.camera_follow.is_some() {
            return None;
        }
        let poses = self.project.poses_at(self.current as f32);
        let pose = poses.get(self.selected_figure)?;
        Some((pose.root[0], pose.root[1]))
    }

    /// The push-in / pull-out zoom travel, as a factor: the
    /// panel steppers' `ZOOM_STEP` (1.25) squared ≈ 1.5625×.
    pub const CAMERA_PRESET_ZOOM_FACTOR: f32 = 1.5625;

    /// The panel's standing preset length, in frames
    /// (1 s at 24 fps; ≈2 s at the demo's 12 fps).
    pub const CAMERA_PRESET_DEFAULT_FRAMES: usize = 24;

    /// Rotate the camera at the current frame (wave 6): upserts a
    /// camera key carrying the existing pan/zoom plus the new angle.
    pub fn rotate_camera(&mut self, delta_deg: f32) {
        self.snapshot_labeled("Rotate camera");
        let frame = self.current;
        let (x, y, zoom) = self.project.camera_at(frame as f32);
        let angle = crate::model::norm_deg(self.project.camera_angle_at(frame as f32) + delta_deg);
        self.project.set_camera_key(frame, x, y, zoom);
        if let Some(k) = self.project.camera.iter_mut().find(|k| k.frame == frame) {
            k.angle = angle;
        }
        self.status = format!("Camera rotation {angle:.0}° at frame {}.", frame + 1);
    }

    /// Cycle the selected node's bend on the current frame
    /// (wave 7): 0 → +0.35 → +0.7 → −0.35 → −0.7 → 0 radians.
    /// Off-cycle values (e.g. imported bends) snap to the nearest
    /// step, then advance. Bends live in `Frame.bends[figure]
    /// [node]`; the origin has no segment and cannot bend.
    pub fn cycle_bend(&mut self) {
        let Some(ni) = self.selected_node else {
            self.status = "Select a joint first, then cycle its bend.".into();
            return;
        };
        if ni == 0 {
            self.status = "The origin has no segment to bend.".into();
            return;
        }
        let fi = self.selected_figure;
        if fi >= self.project.figures.len() || self.current >= self.project.frames.len() {
            return;
        }
        const STEPS: [f32; 5] = [0.0, 0.35, 0.7, -0.35, -0.7];
        let cur = self.project.frames[self.current]
            .bends
            .get(fi)
            .and_then(|b| b.get(ni))
            .copied()
            .unwrap_or(0.0);
        let nearest = STEPS
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                (cur - **a)
                    .abs()
                    .partial_cmp(&(cur - **b).abs())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(i, _)| i)
            .unwrap_or(0);
        let next = STEPS[(nearest + 1) % STEPS.len()];
        self.snapshot_labeled("Bend");
        let nodes = self.project.figures[fi].def.nodes.len();
        let frame = &mut self.project.frames[self.current];
        if frame.bends.len() <= fi {
            frame.bends.resize(fi + 1, Vec::new());
        }
        if frame.bends[fi].len() < nodes {
            frame.bends[fi].resize(nodes, 0.0);
        }
        frame.bends[fi][ni] = next;
        self.status = format!(
            "Bend: figure {} node {} \u{2192} {:.0}\u{00b0}.",
            fi + 1,
            ni,
            next.to_degrees()
        );
    }

    pub fn toggle_figure_lock(&mut self, fi: usize) {
        // Roadmap Wave 1 (RM24, P-14 — A15 H3): the Wave 13
        // B2 layers exemption is closed — the toggle takes
        // exactly one undo step (peeked first, so a missing
        // figure spends nothing).
        if self.project.figures.get(fi).is_some() {
            self.snapshot_labeled("Figure lock");
        }
        match crate::layers::toggle_lock(&mut self.project, fi) {
            Some(locked) => {
                self.status = format!("Figure {} {}.", fi + 1, if locked { "locked" } else { "unlocked" })
            }
            None => self.status = "No such figure.".into(),
        }
    }

    pub fn cycle_figure_opacity(&mut self, fi: usize) {
        // Roadmap Wave 1 (RM24, P-14): as `toggle_figure_lock`,
        // coalesced — a run of cycle presses is one step.
        if self.project.figures.get(fi).is_some() {
            self.snapshot_coalesced("Figure opacity");
        }
        match crate::layers::cycle_opacity(&mut self.project, fi) {
            Some(o) => {
                self.status = format!("Figure {} opacity {:.0}%.", fi + 1, o * 100.0)
            }
            None => self.status = "No such figure.".into(),
        }
    }

    /// Step a figure's opacity by `delta` (the sidebar's −/+ steppers),
    /// via the layers engine's clamped setter — so the bounds and NaN
    /// handling match every other opacity write. The target is rounded
    /// to two decimals so repeated steps land on clean percentages.
    /// Since Roadmap Wave 1 (RM24, P-14 — A15 H3) a step that
    /// actually changes the value takes a coalesced undo step
    /// (a held stepper is one step), where Wave 13 (B2) only
    /// marked it dirty.
    pub fn step_figure_opacity(&mut self, fi: usize, delta: f32) {
        let Some(fig) = self.project.figures.get(fi) else {
            self.status = "No such figure.".into();
            return;
        };
        let before = fig.opacity;
        let next = ((fig.opacity + delta) * 100.0).round() / 100.0;
        // The engine's clamped result, peeked so a step
        // clamped onto the current value spends no step
        // (and a NaN step, which the engine refuses
        // outright, spends nothing either).
        let effective = next.clamp(crate::layers::MIN_OPACITY, crate::layers::MAX_OPACITY);
        if effective.is_finite() && effective != before {
            self.snapshot_coalesced("Figure opacity");
        }
        if crate::layers::set_opacity(&mut self.project, fi, next) {
            let o = self.project.figures[fi].opacity;
            self.status = format!("Figure {} opacity {:.0}%.", fi + 1, o * 100.0);
        } else {
            self.status = "No such figure.".into();
        }
    }

    pub fn toggle_solo(&mut self, fi: usize) {
        if self.solo == Some(fi) {
            self.solo = None;
            self.status = "Solo off.".into();
        } else if fi < self.project.figures.len() {
            self.solo = Some(fi);
            self.status = format!("Solo: figure {}.", fi + 1);
        }
    }

    /// Key the selected node's angle on a graph channel at the current
    /// frame, using its current keyframed value.
    ///
    /// The origin (node 0) is refused, before any snapshot
    /// (Wave AB, AB4 — AA5's Bend(0) call extended to the
    /// angle door): `model::evaluate` gives a parentless
    /// node `abs_angle = 0.0` and never reads its stored
    /// angle, so an Angle(0) channel's value lands in a
    /// pose slot nothing renders. The refusal blocks
    /// AUTHORING only: an Angle(0) channel a saved project
    /// already carries stays selectable, its keys stay
    /// editable and deletable through the transform tools,
    /// and it round-trips untouched. Every caller of this
    /// op (the graph panel's "Key angle", the command
    /// palette, the keyframe tools' "Key angle here", MCP
    /// `graph_key_angle`) inherits the refusal from here.
    pub fn key_graph_angle(&mut self) {
        let Some(ni) = self.selected_node else {
            self.status = "Select a joint first, then Key graph.".into();
            return;
        };
        if ni == 0 {
            self.status = "The origin has no angle to key.".into();
            return;
        }
        let fi = self.selected_figure;
        let frame = self.current as f32;
        let value = self
            .project
            .frames
            .get(self.current)
            .and_then(|f| f.poses.get(fi))
            .and_then(|p| p.angles.get(ni).copied())
            .unwrap_or(0.0);
        // Wave 11 (C3): this is the graph panel's primary Key
        // button — it must be an undo step like every other
        // mutation; previously it neither snapshotted nor
        // counted toward the autosave cadence.
        self.snapshot_labeled("Key angle");
        let ch = crate::graph::channel_mut(
            &mut self.project.channels,
            fi,
            crate::graph::ChannelTarget::Angle(ni),
        );
        ch.add_key(frame, value);
        self.status = format!("Graph key: figure {} joint {} = {value:.0}° @ frame {}.", fi + 1, ni, self.current + 1);
    }

    /// Remove all graph channels (the Graph panel edits them in a
    /// later pass; clearing must always be possible).
    pub fn clear_graph(&mut self) {
        // Wave 11 (C3): an undo step like every other mutation
        // (previously not undoable at all), with the house
        // empty-guard so an empty clear records nothing.
        if self.project.channels.is_empty() {
            self.status = "No graph channels to clear.".into();
            return;
        }
        self.snapshot_labeled("Clear graph channels");
        self.project.channels.clear();
        self.status = "Graph channels cleared.".into();
    }

    /// Wave 10: smooth every graph channel's key tangents in one
    /// undo step. `graph::smooth_channel` fits automatic
    /// Catmull-Rom-style tangents through each channel's keys so
    /// Hermite segments flow instead of cornering at flat tangents;
    /// it derives from the channel's own target whether the values
    /// are angles (smoothed the short way around the circle) or
    /// plain scalars.
    pub fn smooth_graph_tangents(&mut self) {
        if self.project.channels.is_empty() {
            self.status = "No graph channels to smooth.".into();
            return;
        }
        let n = self.project.channels.len();
        self.snapshot_labeled("Auto tangents");
        for ch in &mut self.project.channels {
            crate::graph::smooth_channel(ch);
        }
        self.status = format!("Smoothed tangents on {n} graph channel(s).");
    }

    /// Wave 12: apply a named tangent preset
    /// (`crate::graph::TangentPreset`) to every graph channel
    /// belonging to the selected figure, as one undo step. Angle
    /// channels are shaped with shortest-path (wrapping) deltas
    /// and every other target with plain deltas — the same
    /// `angular` derivation `graph::smooth_channel` uses — and
    /// each channel's keys are sorted first, as that wrapper
    /// does. Channels belonging to other figures are untouched;
    /// a selected figure with no channels is a no-op (no
    /// snapshot).
    pub fn apply_graph_ease_preset(&mut self, preset: crate::graph::TangentPreset) {
        let fi = self.selected_figure;
        let n = self
            .project
            .channels
            .iter()
            .filter(|ch| ch.figure == fi)
            .count();
        if n == 0 {
            self.status = format!("No graph channels for figure {}.", fi + 1);
            return;
        }
        self.snapshot_labeled("Tangent preset");
        for ch in &mut self.project.channels {
            if ch.figure != fi {
                continue;
            }
            ch.sort_keys();
            let angular = matches!(ch.target, crate::graph::ChannelTarget::Angle(_));
            crate::graph::apply_tangent_preset(&mut ch.keys, preset, angular);
        }
        self.status = format!(
            "Applied {preset:?} preset to {n} graph channel(s) for figure {}.",
            fi + 1
        );
    }

    /// Wave 15: set the interpolation mode (`crate::graph::Interp`)
    /// of one graph key, as one undo step.
    ///
    /// Which key: `EditorCore` has no graph key selection of its
    /// own — the graph panel's channel selector and its key
    /// selection (`graph_sel`) live in the view, and the core-side
    /// graph ops scope by figure and joint instead (the tangent
    /// presets apply to every channel of the selected figure).
    /// So this op targets the same channel `key_graph_angle`
    /// authors — the selected figure's `Angle(selected_node)`
    /// channel — and within it the key nearest the current frame
    /// (an exact hit wins; ties go to the earlier key). The mode
    /// governs the segment STARTING at that key (see
    /// `graph::Interp`). No selected joint, or a channel with no
    /// keys, is a no-op (no snapshot).
    pub fn set_graph_key_interp(&mut self, mode: crate::graph::Interp) {
        let Some(ni) = self.selected_node else {
            self.status = "Select a joint first, then set graph interp.".into();
            return;
        };
        let fi = self.selected_figure;
        let target = crate::graph::ChannelTarget::Angle(ni);
        let Some(ci) = self
            .project
            .channels
            .iter()
            .position(|ch| ch.figure == fi && ch.target == target)
        else {
            self.status = format!("No graph keys for figure {} joint {}.", fi + 1, ni);
            return;
        };
        let now = self.current as f32;
        let mut best: Option<usize> = None;
        let mut best_dist = f32::INFINITY;
        for (ki, key) in self.project.channels[ci].keys.iter().enumerate() {
            let dist = (key.frame - now).abs();
            if dist < best_dist {
                best_dist = dist;
                best = Some(ki);
            }
        }
        let Some(ki) = best else {
            self.status = format!("No graph keys for figure {} joint {}.", fi + 1, ni);
            return;
        };
        self.snapshot_labeled("Key interpolation");
        let key = &mut self.project.channels[ci].keys[ki];
        key.interp = mode;
        let frame = key.frame;
        self.status = format!(
            "Interp {mode:?} set on key at frame {}.",
            frame.round() as i64 + 1
        );
    }

    /// Wave 11: bake every graph channel into real frame poses
    /// for frames `from..=to` (the end clamped to the timeline),
    /// as one undo step.
    ///
    /// Each frame's poses are computed with
    /// `crate::graph::apply_channels` — the same evaluation the
    /// renderer uses — so the baked frames hold exactly what the
    /// channels produced, and the channels can then be edited or
    /// cleared without changing those frames. Every frame is
    /// evaluated BEFORE the snapshot and the writes, so a bake
    /// never leaves a half-baked range behind. Stated plainly:
    /// length channels are not baked — `apply_channels` reports
    /// them through the returned figure defs, and frame poses
    /// carry no lengths, so only root and angle channels land in
    /// the frames. The status counts every channel either way.
    ///
    /// Wave Z (Z11): bend channels DO bake — frames carry a
    /// bend home (`Frame::bends`), unlike lengths. Each baked
    /// frame's evaluated bend row (the same
    /// `apply_channels` result, seeded from `bends_at` with
    /// the bend channels applied) is written into the frame
    /// for every figure a bend channel targets, so clearing
    /// the channels after a bake no longer drops the bend
    /// motion the way it keeps the baked poses. Figures no
    /// bend channel targets keep their stored bend tables
    /// untouched — a pose-only bake never materialises them.
    pub fn bake_graph_channels(&mut self, from: usize, to: usize) {
        if self.project.channels.is_empty() {
            self.status = "No graph channels to bake.".into();
            return;
        }
        if self.project.frames.is_empty() || from > to || from >= self.project.frames.len() {
            self.status = "Graph bake failed: invalid frame range.".into();
            return;
        }
        let lo = from;
        let hi = to.min(self.project.frames.len() - 1);
        let n_channels = self.project.channels.len();
        // Figures a bend channel targets (Wave Z, Z11): only
        // their bend rows are written by the bake.
        let bend_figures: Vec<bool> = (0..self.project.figures.len())
            .map(|fi| {
                self.project
                    .channels
                    .iter()
                    .any(|ch| {
                        ch.figure == fi
                            && matches!(ch.target, crate::graph::ChannelTarget::Bend(_))
                    })
            })
            .collect();
        let baked: Vec<(
            Vec<crate::model::Pose>,
            Vec<Vec<f32>>,
        )> = (lo..=hi)
            .map(|f| {
                let out = crate::graph::apply_channels(
                    &self.project,
                    &self.project.channels,
                    f as f32,
                );
                (out.poses, out.bends)
            })
            .collect();
        self.snapshot_labeled("Bake graph channels");
        for ((poses, bends), f) in baked.into_iter().zip(lo..=hi) {
            self.project.frames[f].poses = poses;
            for (fi, row) in bends.into_iter().enumerate() {
                if !bend_figures.get(fi).copied().unwrap_or(false) {
                    continue;
                }
                let frame = &mut self.project.frames[f];
                if frame.bends.len() <= fi {
                    frame.bends.resize(fi + 1, Vec::new());
                }
                frame.bends[fi] = row;
            }
        }
        self.status = format!(
            "Baked {n_channels} graph channel(s) into frames {}–{} ({} frame(s)).",
            lo + 1,
            hi + 1,
            hi - lo + 1
        );
    }

    /// Wave 10: mirror the selected figure's pose on the current
    /// frame, in place, as one undo step.
    ///
    /// Pose angles are RELATIVE joint angles in degrees, so
    /// negating every angle reflects the pose about the vertical
    /// axis through the root: each segment leaves its parent at
    /// the mirrored heading, and the root position is unchanged.
    /// Approximation, stated honestly: this is exact for the angle
    /// data itself and for bilaterally symmetric rigs (the
    /// built-in presets) — mirroring twice returns the original
    /// pose exactly. It does not touch anything outside `Pose`:
    /// per-node bends, segment lengths and graph channels keep
    /// their values, so a mirrored pose driven by a graph channel
    /// or carrying bends can differ slightly from a true
    /// geometric mirror of the rendered figure.
    pub fn mirror_selected_pose(&mut self) {
        let fi = self.selected_figure;
        if fi >= self.project.figures.len() {
            self.status = "No such figure.".into();
            return;
        }
        let has_pose = self
            .project
            .frames
            .get(self.current)
            .and_then(|f| f.poses.get(fi))
            .is_some();
        if !has_pose {
            self.status = format!(
                "Figure {} has no pose on frame {}.",
                fi + 1,
                self.current + 1
            );
            return;
        }
        self.snapshot_labeled("Mirror pose");
        let pose = &mut self.project.frames[self.current].poses[fi];
        for a in &mut pose.angles {
            *a = -*a;
        }
        self.status = format!("Mirrored figure {} on frame {}.", fi + 1, self.current + 1);
    }

    /// Wave 11: copy the current frame's pose of figure
    /// `from_fig` onto figure `to_fig` (same frame), as one undo
    /// step. The pose is copied exactly — root and every angle —
    /// so the target figure adopts the source's posture while
    /// keeping its own definition, bends and lengths.
    pub fn copy_pose_between(&mut self, from_fig: usize, to_fig: usize) {
        if from_fig >= self.project.figures.len() || to_fig >= self.project.figures.len() {
            self.status = "Copy pose failed: figure index out of range.".into();
            return;
        }
        if from_fig == to_fig {
            self.status = "Copy pose failed: source and target are the same figure.".into();
            return;
        }
        let cur = self.current;
        let Some(pose) = self
            .project
            .frames
            .get(cur)
            .and_then(|f| f.poses.get(from_fig))
            .cloned()
        else {
            self.status = format!("Figure {} has no pose on frame {}.", from_fig + 1, cur + 1);
            return;
        };
        if self.project.frames[cur].poses.get(to_fig).is_none() {
            self.status = format!("Figure {} has no pose on frame {}.", to_fig + 1, cur + 1);
            return;
        }
        // Rig-size guard (Wave 10, D7): `model::evaluate`
        // indexes `pose.angles[i]` directly, so copying a
        // pose onto a figure whose pose carries a
        // different angle count corrupts the row and
        // panics the next render. The pose panel has
        // always guarded its own presses; the core op now
        // refuses too, so MCP and palette callers cannot
        // reach the panic either. No snapshot on refusal.
        let target_angles = self.project.frames[cur].poses[to_fig].angles.len();
        if pose.angles.len() != target_angles {
            self.status = format!(
                "Copy pose failed: figure {} has {} angles but figure {} has {} — poses only copy between figures with the same rig size.",
                from_fig + 1,
                pose.angles.len(),
                to_fig + 1,
                target_angles
            );
            return;
        }
        self.snapshot_labeled("Copy pose");
        self.project.frames[cur].poses[to_fig] = pose;
        self.status = format!(
            "Copied figure {}'s pose to figure {} on frame {}.",
            from_fig + 1,
            to_fig + 1,
            cur + 1
        );
    }

    pub fn open_via_dialog(&mut self) {
        if crate::dialogs::available_tool().is_none() {
            self.status =
                "No dialog tool (zenity/kdialog) on this system — use the project list instead."
                    .into();
            return;
        }
        let filters = vec![crate::dialogs::FileFilter::new(
            "StykAnimate project",
            &["*.json"],
        )];
        let Some(path) = crate::dialogs::open_file("Open project", &filters) else {
            self.status = "Open cancelled.".into();
            return;
        };
        match std::fs::read_to_string(&path)
            .ok()
            .and_then(|txt| serde_json::from_str::<Project>(&txt).ok())
        {
            Some(p) => {
                self.project = p;
                self.current = 0;
                self.selected_figure = 0;
                self.selected_node = None;
                self.solo = None;
                self.note_project_swapped();
                self.note_save_baseline();
                self.status =
                    format!("Opened {}{}", path.display(), self.missing_audio_note());
            }
            None => self.status = format!("Could not open {} as a project.", path.display()),
        }
    }

    pub fn save_via_dialog(&mut self) {
        if crate::dialogs::available_tool().is_none() {
            self.status =
                "No dialog tool (zenity/kdialog) on this system — use Save project instead."
                    .into();
            return;
        }
        let filters = vec![crate::dialogs::FileFilter::new(
            "StykAnimate project",
            &["*.json"],
        )];
        let Some(path) = crate::dialogs::save_file("Save project", &filters, "project.styk.json")
        else {
            self.status = "Save cancelled.".into();
            return;
        };
        match serde_json::to_string_pretty(&self.project)
            .map_err(std::io::Error::other)
            .and_then(|txt| std::fs::write(&path, txt))
        {
            Ok(()) => {
                // Wave 12 (B4/B3): a dialog save that lands in
                // ./projects creates a file the cached selector
                // list has never heard of; refresh it (the seam
                // preserves the selection by identity and is a
                // no-op for the list when the path is elsewhere).
                crate::projects::refresh_project_files(self);
                self.status = format!("Saved to {}", path.display());
            }
            Err(e) => self.status = format!("Save failed: {e}"),
        }
    }

    pub fn save_path() -> PathBuf {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("project.styk.json")
    }

    pub fn save(&mut self) {
        let path = Self::save_path();
        match serde_json::to_string_pretty(&self.project)
            .map_err(std::io::Error::other)
            .and_then(|s| std::fs::write(&path, s))
        {
            Ok(()) => {
                // Roadmap Wave 1 (RM24, P-14): this IS a
                // write of the project to a named file, so
                // the save-state baseline stamps here too.
                self.note_save_baseline();
                self.status = format!("Saved to {}", path.display())
            }
            Err(e) => self.status = format!("Save failed: {e}"),
        }
    }

    pub fn load(&mut self) {
        let path = Self::save_path();
        match std::fs::read_to_string(&path)
            .map_err(std::io::Error::other)
            .and_then(|s| serde_json::from_str::<Project>(&s).map_err(std::io::Error::other))
        {
            Ok(p) => {
                self.project = p;
                self.current = 0;
                self.selected_figure = 0;
                self.selected_node = None;
                self.undo.clear();
                self.note_project_swapped();
                self.note_save_baseline();
                self.status =
                    format!("Loaded {}{}", path.display(), self.missing_audio_note());
            }
            Err(e) => self.status = format!("Load failed: {e}"),
        }
    }

    /// Wave 9: autosave the recovery snapshot after this many
    /// snapshot-counted edits (see `maybe_autosave`).
    pub const AUTOSAVE_EVERY: u64 = 25;

    /// Wave 9: the pure autosave decision, factored out of
    /// `maybe_autosave` so the threshold logic is unit tested
    /// without touching the filesystem.
    fn autosave_due(edit_count: u64, last_autosave_count: u64) -> bool {
        edit_count.saturating_sub(last_autosave_count) >= Self::AUTOSAVE_EVERY
    }

    /// Wave 9: snapshot-counted edits not yet covered by an
    /// autosave (0 right after a successful one).
    pub fn edits_since_autosave(&self) -> u64 {
        self.edit_count.saturating_sub(self.last_autosave_count)
    }

    /// Wave 12 (E8): note that a recovery snapshot of the
    /// current state was just written OUTSIDE `maybe_autosave`
    /// (the GUI save paths write one as a follow-up). Without
    /// this, the status strip's autosave segment kept counting
    /// edits the snapshot already covers while its dirty dot
    /// read the same counter differently — the two segments
    /// contradicted each other after every GUI save. (MCP
    /// saves skip `write_recovery` by standing product call,
    /// so no MCP path needs this.)
    pub fn note_recovery_written(&mut self) {
        self.last_autosave_count = self.edit_count;
    }

    /// Wave 13 (B2): count one edit for the dirty dot and
    /// the autosave cadence WITHOUT pushing an undo
    /// snapshot. This bumps the same `edit_count` that
    /// `snapshot()` bumps — the counter
    /// `edits_since_autosave` reads, and the one the
    /// status strip derives its dirty dot from — so a
    /// marked edit is indistinguishable from a
    /// snapshot-counted one for both readers, and a later
    /// `note_recovery_written` covers it exactly the same
    /// way.
    ///
    /// History: the seam existed for the core ops that
    /// mutated the project with undoability deliberately
    /// OFF (the layers family — lock / hide / opacity —
    /// and the project-settings and segment tweak cyclers
    /// and steppers), whose pre-seam writes were invisible
    /// to the dirty dot and could slip a whole session
    /// past the 25-edit autosave cadence (the Wave 12 E8
    /// remainder).
    ///
    /// Roadmap Wave 1 (RM24, P-14 — A15 H3) closed that
    /// census: every one of those ops now snapshots (the
    /// stepper/cycler/scrub families coalesced, the rest
    /// labelled), so a Project mutation NEVER routes here
    /// any more. What remains is the one class whose undo
    /// would desync state outside the project: session
    /// bookkeeping that follows an external event (the
    /// project-file rename handler re-pointing the session
    /// name at the file that now exists on disk — undoing
    /// it would restore a name whose file is gone). The
    /// house rule stands, narrowed: a mutation either
    /// snapshots exactly once or — only for that class —
    /// calls this; never neither. Callers mark only when
    /// a value actually changed; a refused or
    /// clamped-to-same-value call marks nothing.
    pub fn mark_dirty(&mut self) {
        self.edit_count += 1;
        // Roadmap Wave 1 (RM6, F-5): a marked edit is a real
        // project mutation, so the revision moves here too —
        // the two counters share their choke points by design.
        self.revision += 1;
    }

    /// Wave 9: if at least `AUTOSAVE_EVERY` edits have
    /// accumulated since the last autosave, write the recovery
    /// snapshot (`./projects/.recovery.json`, via shell.rs — the
    /// same file the Home panel writes and the Recover button
    /// reads). Silent on success: autosave must never spam the
    /// status line. On failure the status reports it and the
    /// last-autosave counter is left alone, so the next call
    /// retries instead of waiting for another AUTOSAVE_EVERY
    /// edits. The app layer calls this on every poll tick
    /// (main.rs) — the cadence decision lives there; this is
    /// only the mechanism.
    pub fn maybe_autosave(&mut self) {
        if !Self::autosave_due(self.edit_count, self.last_autosave_count) {
            return;
        }
        self.write_recovery_snapshot();
    }

    /// Wave P (P19): the window-close flush. The Rune
    /// framework offers no close veto (proven from the
    /// pinned framework source: `Window::close` runs the
    /// `on_close` hook, then closes unconditionally — see
    /// docs in wave-p/findings/p19.md), so a dirty window
    /// close cannot be confirmed or refused; the fail-closed
    /// behaviour available is that it must lose NOTHING.
    /// `maybe_autosave` only writes every `AUTOSAVE_EVERY`
    /// edits, so up to 24 counted edits could sit outside
    /// the recovery snapshot when the window closes. This
    /// flush closes that gap: while any counted edit is not
    /// yet covered by the snapshot (the shell's dirty truth,
    /// `edits_since_autosave() > 0` — the same counter the
    /// dirty dot and `session_dirty` read), write the
    /// snapshot NOW, whatever the cadence says. A clean
    /// session writes nothing, so the snapshot's mtime keeps
    /// meaning "unsaved work" to the startup recovery offer
    /// (`shell::recovery_is_newer_than_last_project`) instead
    /// of being refreshed by every clean exit. Called from
    /// the shell's `on_close` hook (main.rs, integrator
    /// wiring); silent on success and honest on failure
    /// exactly like `maybe_autosave`, whose write body this
    /// shares.
    pub fn flush_recovery_on_close(&mut self) {
        if self.edits_since_autosave() == 0 {
            return;
        }
        self.write_recovery_snapshot();
    }

    /// The one recovery-snapshot write both autosave paths
    /// share: on success the last-autosave counter catches
    /// up with the edit counter (the session reads clean);
    /// on failure the status reports it and the counter is
    /// left alone, so the next call retries.
    fn write_recovery_snapshot(&mut self) {
        match crate::shell::write_recovery(&self.project) {
            Ok(_) => self.last_autosave_count = self.edit_count,
            Err(e) => self.status = format!("Autosave failed: {e}"),
        }
    }

    // ---- Figure / layer management ----

    pub fn delete_figure(&mut self) {
        if self.project.figures.len() <= 1 {
            self.status = "Keep at least one figure.".into();
            return;
        }
        self.snapshot_labeled("Delete figure");
        let fi = self.selected_figure.min(self.project.figures.len() - 1);
        self.project.figures.remove(fi);
        for frame in &mut self.project.frames {
            // Wave 11 (E6): a frame where the figure never
            // appeared holds a shorter pose row (no slot for
            // `fi`) — `Vec::remove` there panicked. The MCP
            // `delete_figure` handler validates the figure
            // index, not per-frame pose presence.
            if fi < frame.poses.len() {
                frame.poses.remove(fi);
            }
        }
        // Every figure-indexed reference follows the removal
        // (Phase F figure_ops census: particles, FX clips,
        // graph channels, camera follow) — refs to the removed
        // figure drop, refs above it shift down.
        crate::figure_ops::remap_figures_after_remove(&mut self.project, fi);
        if self.solo == Some(fi) {
            self.solo = None;
        } else if let Some(s) = self.solo {
            if s > fi {
                self.solo = Some(s - 1);
            }
        }
        self.selected_figure = fi.min(self.project.figures.len() - 1);
        self.selected_node = None;
        self.status = "Figure deleted.".into();
    }

    /// Move the selected figure later (drawn on top) or earlier.
    pub fn move_figure(&mut self, dir: i32) {
        let fi = self.selected_figure;
        let target = fi as i32 + dir;
        if target < 0 || target as usize >= self.project.figures.len() {
            return;
        }
        self.snapshot_labeled("Reorder figure");
        let target = target as usize;
        let figure_count = self.project.figures.len();
        self.project.figures.swap(fi, target);
        for frame in &mut self.project.frames {
            // Wave 11 (E6 residual): swap pose rows only where
            // both exist — Vec::swap panics on a short row (a
            // frame missing a figure's pose), the same absence
            // class as the bend rows guarded below.
            if fi < frame.poses.len() && target < frame.poses.len() {
                frame.poses.swap(fi, target);
            }
            // Bend rows are per-figure like pose rows, but
            // the both-rows guard alone strands a row at the
            // padded-prefix boundary (Wave P, P46 — the
            // single-step form of the corruption P13 fixed
            // in layers::move_figure): the bend writer pads
            // the table only up to the highest figure edited
            // in the frame, so with a length-1 table the
            // 0<->1 swap never fires and figure 0's row stays
            // behind on figure 1. Pad-then-swap, mirroring
            // layers: a non-empty table shorter than the
            // figure count is padded with empty rows first
            // (an absent row and an empty row both read as
            // straight in `Project::bends_at`), then the
            // guarded swap runs. An empty table stays empty:
            // a bend-free frame gains no rows from a reorder.
            if !frame.bends.is_empty() && frame.bends.len() < figure_count {
                frame.bends.resize(figure_count, Vec::new());
            }
            if fi < frame.bends.len() && target < frame.bends.len() {
                frame.bends.swap(fi, target);
            }
        }
        // Figure-indexed references ride the swap
        // (Phase G): particle anchors, FX clips, graph
        // channels and the camera-follow target.
        crate::figure_ops::remap_figures_after_move(&mut self.project, fi, target);
        if let Some(s) = self.solo {
            self.solo = Some(crate::figure_ops::remap_index_after_move(s, fi, target));
        }
        self.selected_figure = target;
        self.status = "Figure order changed.".into();
    }

    pub fn toggle_figure_hidden(&mut self) {
        // Roadmap Wave 1 (RM24, P-14 — A15 H3): the Wave 13
        // B2 layers exemption is closed — the flip takes
        // exactly one undo step.
        self.snapshot_labeled("Figure visibility");
        let fi = self.selected_figure;
        let (name, hidden) = {
            let fig = &mut self.project.figures[fi];
            fig.hidden = !fig.hidden;
            (fig.name.clone(), fig.hidden)
        };
        self.status = format!("{} {}.", name, if hidden { "hidden" } else { "shown" });
    }

    pub fn select_figure(&mut self, fi: usize) {
        if fi < self.project.figures.len() {
            self.selected_figure = fi;
            self.selected_node = None;
        }
    }

    // ---- Frame clipboard, timing, project settings ----

    /// True when every pose row in `frame` lines up with
    /// the project's figures: one pose per figure, and
    /// each pose's angle count equal to its figure's node
    /// count — the alignment `model::evaluate` assumes
    /// when it indexes `pose.angles[i]`. The paste ops
    /// refuse clipboard frames that fail this (a frame
    /// copied before a figure or segment edit), instead
    /// of planting a pose the next render would panic on
    /// (Wave 11, E6).
    fn frame_poses_match(&self, frame: &crate::model::Frame) -> bool {
        frame.poses.len() == self.project.figures.len()
            && frame
                .poses
                .iter()
                .zip(self.project.figures.iter())
                .all(|(pose, fig)| pose.angles.len() == fig.def.nodes.len())
    }

    pub fn copy_frame(&mut self) {
        // Wave 11 (E6): a project can legitimately hold
        // zero frames (it loads that way via MCP
        // `load_project`), and indexing `frames[current]`
        // then panicked — the MCP `copy_frame` tool takes
        // no arguments, so no handler check can cover it.
        let Some(frame) = self.project.frames.get(self.current).cloned() else {
            self.status = "No frame to copy.".into();
            return;
        };
        self.clipboard = Some(frame);
        self.status = format!("Frame {} copied.", self.current + 1);
    }

    pub fn paste_frame(&mut self) {
        let Some(frame) = self.clipboard.clone() else {
            self.status = "Nothing copied yet.".into();
            return;
        };
        if frame.poses.len() != self.project.figures.len() {
            self.status = "Copied frame has a different figure count.".into();
            return;
        }
        // Wave 11 (E6): the copied frame's poses must also
        // still match the figures' rigs — a segment added
        // after the copy grew the definition and every live
        // frame's angle vec, but not the clipboard's, and
        // `model::evaluate` indexes `pose.angles[i]`
        // directly, so pasting the stale frame would plant
        // a render panic (the same class the Wave 10 D7
        // guard closed in `copy_pose_between`).
        if !self.frame_poses_match(&frame) {
            self.status = "Copied frame's poses no longer match the figures' rigs.".into();
            return;
        }
        self.snapshot_labeled("Paste frame");
        // Wave 11 (E6): an empty timeline has no current
        // frame to insert after — `insert(current + 1)`
        // panicked on it — so the pasted frame becomes
        // frame 1 there.
        let at = if self.project.frames.is_empty() {
            0
        } else {
            self.current + 1
        };
        self.project.frames.insert(at, frame);
        self.current = at;
        self.status = format!("Frame pasted at {}.", at + 1);
    }

    pub fn adjust_frame_duration(&mut self, factor: f32) {
        // Roadmap Wave 1 (RM24, P-14 — A15 H3): a real
        // change takes a coalesced undo step (peeked so a
        // factor clamped onto the current value spends
        // none), where Wave 13 (B2) only marked it dirty.
        let peek_before = self.project.frames[self.current].duration;
        if (peek_before * factor).clamp(0.25, 8.0) != peek_before {
            self.snapshot_coalesced("Frame duration");
        }
        let after = {
            let frame = &mut self.project.frames[self.current];
            frame.duration = (frame.duration * factor).clamp(0.25, 8.0);
            frame.duration
        };
        self.status = format!("Frame {} duration {:.2}x.", self.current + 1, after);
    }

    pub fn adjust_fps(&mut self, delta: f32) {
        let before = self.project.fps;
        // Roadmap Wave 1 (RM24, P-14 — A15 H3): as
        // `adjust_frame_duration` — a real change takes a
        // coalesced undo step. (The deliberate setter
        // `set_project_fps` snapshots, as it always did.)
        if (before + delta).clamp(1.0, 60.0) != before {
            self.snapshot_coalesced("Frame rate");
        }
        self.project.fps = (self.project.fps + delta).clamp(1.0, 60.0);
        self.status = format!("{} fps.", self.project.fps as i32);
    }

    /// Trim-friendly number formatting for the
    /// settings setters below: whole numbers print with
    /// no decimal point ("24"), fractional values with
    /// one decimal ("29.9"). `adjust_fps`'s `as i32`
    /// truncation is fine for a stepper that moves in
    /// whole steps, but a setter handed 29.97 must not
    /// report "29".
    fn setting_number(v: f32) -> String {
        if v == v.trunc() {
            format!("{}", v as i64)
        } else {
            format!("{v:.1}")
        }
    }

    /// Set the project's frame rate directly (the
    /// settings panel's fps stepper, Phase K). Unlike
    /// the steppers above, a discrete settings choice IS
    /// an undoable edit here: a real change takes
    /// exactly one snapshot. A non-finite value, or one
    /// that clamps to the current rate, changes nothing
    /// and takes none.
    pub fn set_project_fps(&mut self, fps: f32) {
        if !fps.is_finite() {
            self.status = "Frame rate unchanged.".into();
            return;
        }
        let fps = fps.clamp(1.0, 60.0);
        if fps == self.project.fps {
            self.status = "Frame rate unchanged.".into();
            return;
        }
        self.snapshot_labeled("Frame rate");
        self.project.fps = fps;
        self.status = format!("Frame rate set to {} fps.", Self::setting_number(fps));
    }

    /// Set the stage size directly (the scene panel's
    /// custom-size steppers, Phase K). Same discipline
    /// as [`EditorCore::set_project_fps`]: non-finite on
    /// either axis, or a size that clamps to the current
    /// one, is a status-only refusal; a real change
    /// takes exactly one snapshot.
    pub fn set_stage_size(&mut self, w: f32, h: f32) {
        if !w.is_finite() || !h.is_finite() {
            self.status = "Stage size unchanged.".into();
            return;
        }
        let stage = [w.clamp(64.0, 4096.0), h.clamp(64.0, 4096.0)];
        if stage == self.project.stage {
            self.status = "Stage size unchanged.".into();
            return;
        }
        self.snapshot_labeled("Stage size");
        self.project.stage = stage;
        self.status = format!(
            "Stage size set to {}×{}.",
            Self::setting_number(stage[0]),
            Self::setting_number(stage[1])
        );
    }

    /// Set the head-ring scale directly (Phase L; the
    /// field itself is `Project::head_ring_scale`, and
    /// the ring every renderer draws is
    /// `Project::head_ring_radius` — the base
    /// `HEAD_RING_RADIUS` times this scale). Same
    /// discipline as [`EditorCore::set_project_fps`]: a
    /// non-finite value is a status-only refusal
    /// ("Head ring size unchanged."); a real change
    /// takes exactly one snapshot. One wording
    /// difference, deliberate: a value that clamps to
    /// the current scale still reports the set wording
    /// ("Head ring size set to {x}×.") — the size IS
    /// the requested size, only the snapshot is
    /// skipped — so the set wording is written once
    /// and used by both arms.
    pub fn set_head_ring_scale(&mut self, scale: f32) {
        if !scale.is_finite() {
            self.status = "Head ring size unchanged.".into();
            return;
        }
        let scale = scale.clamp(0.5, 2.0);
        if scale == self.project.head_ring_scale {
            self.status = format!(
                "Head ring size set to {}×.",
                Self::setting_number(scale)
            );
            return;
        }
        self.snapshot_labeled("Head ring size");
        self.project.head_ring_scale = scale;
        self.status = format!(
            "Head ring size set to {}×.",
            Self::setting_number(scale)
        );
    }

    pub fn cycle_easing(&mut self) {
        // Roadmap Wave 1 (RM24, P-14 — A15 H3): the B2
        // settings exemption is closed — the value always
        // changes, so the coalesced step is taken up front.
        self.snapshot_coalesced("Easing");
        self.project.easing = self.project.easing.next();
        self.status = format!("Easing: {}.", self.project.easing.label());
    }

    /// Set the project easing directly (the easing picker's
    /// preset buttons). Since Roadmap Wave 1 (RM24, P-14 —
    /// A15 H3) a set that changes the value takes exactly
    /// one undo step, like the MCP twin always should have
    /// (Wave 13 (B2) only marked it dirty).
    pub fn set_project_easing(&mut self, easing: crate::model::Easing) {
        if self.project.easing != easing {
            self.snapshot_labeled("Easing");
        }
        self.project.easing = easing;
        self.status = format!("Easing: {}.", self.project.easing.label());
    }

    pub fn toggle_tween_playback(&mut self) {
        // Roadmap Wave 1 (RM24, P-14 — A15 H3): the flip
        // always changes the serialized flag, so it takes
        // exactly one undo step.
        self.snapshot_labeled("Tween playback");
        self.project.tween_playback = !self.project.tween_playback;
        self.status = format!(
            "Tween playback {}.",
            if self.project.tween_playback { "on" } else { "off" }
        );
    }

    pub fn toggle_camera_follow(&mut self) {
        // Roadmap Wave 1 (RM24, P-14 — A15 H3): as
        // `toggle_tween_playback`.
        self.snapshot_labeled("Camera follow");
        self.project.camera_follow = if self.project.camera_follow == Some(self.selected_figure) {
            None
        } else {
            Some(self.selected_figure)
        };
        self.status = match self.project.camera_follow {
            // Wave 11 (C2 flag): resolve the name fallibly — a
            // stale selection must not panic the status line.
            Some(fi) => match self.project.figures.get(fi) {
                Some(f) => format!("Camera follows {}.", f.name),
                None => format!("Camera follows figure {}.", fi + 1),
            },
            None => "Camera follow off.".into(),
        };
    }

    // ---- Particles ----

    const PARTICLE_KINDS: [crate::model::ParticleKind; 3] = [
        crate::model::ParticleKind::Burst,
        crate::model::ParticleKind::Sparkle,
        crate::model::ParticleKind::Smoke,
    ];

    pub fn add_particle(&mut self, kind_idx: usize) {
        self.snapshot_labeled("Add particle event");
        let fi = self.selected_figure;
        let (figure, node, x, y) = match self.selected_node {
            Some(ni) => {
                let pos = self.positions(self.current, fi);
                (Some(fi), Some(ni), pos[ni][0], pos[ni][1])
            }
            None => {
                let pos = self.positions(self.current, fi);
                (Some(fi), Some(0), pos[0][0], pos[0][1])
            }
        };
        let color = self.project.figures[fi].color;
        let seed = (self.project.particles.len() as u32)
            .wrapping_mul(2654435761)
            .wrapping_add(12345);
        self.project.particles.push(crate::model::ParticleEvent {
            frame: self.current,
            figure,
            node,
            x,
            y,
            kind: Self::PARTICLE_KINDS[kind_idx % Self::PARTICLE_KINDS.len()],
            color,
            seed,
        });
        self.status = "Particle effect added on this frame.".into();
    }

    pub fn clear_particles(&mut self) {
        self.snapshot_labeled("Clear particles");
        self.project.particles.clear();
        self.project.fx_clips.clear();
        self.status = "Particle effects cleared.".into();
    }

    // ---- Wave-4 integrations: nodes animation, styles, media, mixdown ----

    pub fn import_nodes_anim_file(&mut self) {
        let Some(path) = self.first_file_with("import", &["nodemc", "stknds"]) else {
            self.status = "Put a .nodemc or .stknds file in the import folder first.".into();
            return;
        };
        match std::fs::read(&path) {
            Ok(bytes) => match crate::nodes_anim::import_nodes_animation(&bytes) {
                Ok(project) => {
                    self.snapshot_labeled("Import animation");
                    self.project = project;
                    self.current = 0;
                    self.playhead = 0.0;
                    self.selected_figure = 0;
                    self.selected_node = None;
                    // Wave P (P14): this replace bypasses the
                    // swap funnels (it is undoable), so refresh
                    // the missing-audio report directly — it
                    // must describe the project now in the core.
                    self.refresh_missing_audio();
                    self.status = format!("Imported {}.", path.display());
                }
                Err(e) => self.status = format!("Stick Nodes animation import failed: {e}"),
            },
            Err(e) => self.status = format!("Could not read {}: {e}", path.display()),
        }
    }

    fn selected_node_mut(&mut self) -> Option<&mut crate::model::NodeDef> {
        let ni = self.selected_node?;
        self.project.figures.get_mut(self.selected_figure)?.def.nodes.get_mut(ni)
    }

    pub fn cycle_node_cap(&mut self) {
        self.snapshot_labeled("Segment cap");
        match self.selected_node_mut() {
            Some(node) => {
                node.style.cap = crate::style::cycle_cap(node.style.cap);
                self.status = format!("Segment cap: {:?}", node.style.cap);
            }
            None => self.status = "Select a joint first.".into(),
        }
    }

    pub fn cycle_node_taper(&mut self) {
        self.snapshot_labeled("Segment taper");
        match self.selected_node_mut() {
            Some(node) => {
                node.style.taper = match node.style.taper {
                    t if t > 0.9 => 0.6,
                    t if t > 0.5 => 0.3,
                    t if t > 0.1 => 0.0,
                    _ => 1.0,
                };
                self.status = format!("Segment {}", crate::style::taper_label(node.style.taper));
            }
            None => self.status = "Select a joint first.".into(),
        }
    }

    /// Set the selected segment's taper to an exact value
    /// (Roadmap Wave 1, RM3 — F-3): the panel door for the
    /// direct write that has existed since Wave 11 only as
    /// the MCP free function `style::set_node_taper` — the
    /// style panel could only cycle, because the write had
    /// no `EditorCore` method. The value is clamped here
    /// ([`EditorCore::taper_clamped`] — the validation the
    /// free function asks its caller to have done) and the
    /// write itself is that function's: exactly one
    /// snapshot, the cycle op's status wording, the
    /// no-selection refusal in the same voice.
    pub fn set_node_taper(&mut self, taper: f32) {
        crate::style::set_node_taper(self, Self::taper_clamped(taper));
    }

    pub fn cycle_node_gradient(&mut self) {
        self.snapshot_labeled("Segment gradient");
        match self.selected_node_mut() {
            Some(node) => {
                node.style.color2 = match node.style.color2 {
                    None => Some(0xFFFFFF),
                    Some(0xFFFFFF) => Some(0x222222),
                    _ => None,
                };
                self.status = match node.style.color2 {
                    Some(c) => format!("Gradient to #{c:06X}."),
                    None => "Gradient off (solid colour).".into(),
                };
            }
            None => self.status = "Select a joint first.".into(),
        }
    }

    /// Set the selected segment's cap directly (the style
    /// panel's segmented control) instead of cycling it.
    /// Snapshots and reports exactly like `cycle_node_cap`.
    pub fn set_node_cap(&mut self, cap: crate::model::CapKind) {
        self.snapshot_labeled("Segment cap");
        self.write_node_cap(cap);
    }

    /// The cap write itself, without `set_node_cap`'s
    /// snapshot: shared with `set_node_style_atomic`.
    fn write_node_cap(&mut self, cap: crate::model::CapKind) {
        match self.selected_node_mut() {
            Some(node) => {
                node.style.cap = cap;
                self.status = format!("Segment cap: {:?}", node.style.cap);
            }
            None => self.status = "Select a joint first.".into(),
        }
    }

    /// Set the selected segment's gradient end colour
    /// directly, or clear it with `None` (the style panel's
    /// swatch row). Snapshots and reports exactly like
    /// `cycle_node_gradient`.
    pub fn set_node_color2(&mut self, color2: Option<u32>) {
        self.snapshot_labeled("Segment gradient");
        self.write_node_color2(color2);
    }

    /// The gradient-colour write itself, without
    /// `set_node_color2`'s snapshot: shared with
    /// `set_node_style_atomic`. `None` clears the gradient,
    /// exactly like the public setter.
    fn write_node_color2(&mut self, color2: Option<u32>) {
        match self.selected_node_mut() {
            Some(node) => {
                node.style.color2 = color2;
                self.status = match node.style.color2 {
                    Some(c) => format!("Gradient to #{c:06X}."),
                    None => "Gradient off (solid colour).".into(),
                };
            }
            None => self.status = "Select a joint first.".into(),
        }
    }

    /// Set the selected segment's opacity directly (the
    /// style panel's stepper), clamped to 0.05..=1.0 so a
    /// segment can never be made fully invisible by accident.
    /// Snapshots and reports like the other style setters.
    pub fn set_node_opacity(&mut self, opacity: f32) {
        self.snapshot_labeled("Segment opacity");
        self.write_node_opacity(opacity);
    }

    /// The opacity write itself, without `set_node_opacity`'s
    /// snapshot: shared with `set_node_style_atomic`. Keeps
    /// the public setter's 0.05..=1.0 clamp.
    fn write_node_opacity(&mut self, opacity: f32) {
        match self.selected_node_mut() {
            Some(node) => {
                node.style.opacity = opacity.clamp(0.05, 1.0);
                self.status = format!("Segment opacity {:.0}%.", node.style.opacity * 100.0);
            }
            None => self.status = "Select a joint first.".into(),
        }
    }

    /// Apply a complete style to the selected segment in
    /// exactly ONE undo step: the four writes of
    /// `set_node_color` / `set_node_color2` / `set_node_cap` /
    /// `set_node_opacity` (same fields, same clamps, same
    /// per-write status wording — the last write's status is
    /// the one left standing), but with a single snapshot
    /// taken here instead of the three the snapshotting
    /// setters would take between them. This is the write
    /// the style panel's paste path uses, so pasting a style
    /// is one undo, not three. With no segment selected it
    /// refuses in the setters' voice and snapshots nothing.
    pub fn set_node_style_atomic(
        &mut self,
        color: u32,
        color2: Option<u32>,
        cap: crate::model::CapKind,
        opacity: f32,
    ) {
        if self.selected_node_mut().is_none() {
            self.status = "Select a joint first.".into();
            return;
        }
        self.snapshot_labeled("Segment style");
        self.write_node_color(color);
        self.write_node_color2(color2);
        self.write_node_cap(cap);
        self.write_node_opacity(opacity);
    }

    pub fn attach_image_to_node(&mut self) {
        let Some(node_idx) = self.selected_node else {
            self.status = "Select a joint first.".into();
            return;
        };
        // Wave 12 (B4): validate the target BEFORE the
        // snapshot and the asset push. The selection can be
        // stale in one specific way — `redo` restores the
        // project without clamping `selected_figure` (undo
        // does clamp it) — and the old order pushed the
        // asset first and looked the node up afterwards, so
        // a stale figure index silently appended an
        // unreferenced asset to the table, took an undo
        // snapshot for an edit that never landed, and left
        // the previous status standing. The asset table is
        // append-only (nothing in this core removes or
        // reorders entries), so an orphan is never
        // reclaimed; refusing here keeps the table and its
        // references in agreement by construction.
        let target_exists = self
            .project
            .figures
            .get(self.selected_figure)
            .and_then(|f| f.def.nodes.get(node_idx))
            .is_some();
        if !target_exists {
            self.status = "Select a joint first.".into();
            return;
        }
        let Some(path) = self.first_file_with("import", &["png", "jpg", "jpeg"]) else {
            self.status = "Put a PNG or JPEG in the import folder first.".into();
            return;
        };
        match std::fs::read(&path) {
            Ok(bytes) => {
                self.snapshot_labeled("Attach image to segment");
                let asset = self.project.assets.len();
                self.project.assets.push(crate::model::ImageAsset {
                    name: path.file_name().unwrap_or_default().to_string_lossy().into_owned(),
                    png: bytes,
                });
                if let Some(node) = self
                    .project
                    .figures
                    .get_mut(self.selected_figure)
                    .and_then(|f| f.def.nodes.get_mut(node_idx))
                {
                    node.media = Some(crate::model::SegmentMedia::Image { asset, offset: [0.0, 0.0], scale: 1.0, angle_offset: 0.0, flip: false, opacity: 1.0 });
                    self.status = format!("Sprite attached (asset {}).", asset + 1);
                }
            }
            Err(e) => self.status = format!("Could not read {}: {e}", path.display()),
        }
    }

    pub fn clear_node_media(&mut self) {
        // Wave 12 (B2, the B4 pattern): resolve the selection
        // BEFORE the snapshot — the refusal path used to spend
        // a phantom undo step on a change that never happens.
        // The check mirrors `selected_node_mut`'s own
        // resolution exactly.
        let resolves = self.selected_node.is_some_and(|ni| {
            self.project
                .figures
                .get(self.selected_figure)
                .is_some_and(|f| f.def.nodes.get(ni).is_some())
        });
        if !resolves {
            self.status = "Select a joint first.".into();
            return;
        }
        self.snapshot_labeled("Clear segment media");
        if let Some(node) = self.selected_node_mut() {
            node.media = None;
            self.status = "Segment media cleared.".into();
        }
    }

    pub fn mixdown_audio(&mut self) {
        if self.project.audio.is_empty() {
            self.status = "No audio clips to mix.".into();
            return;
        }
        let fps = self.project.fps.max(1.0);
        let clips: Vec<crate::audio_edit::MixClip> = self
            .project
            .audio
            .iter()
            // Wave 11 (B6 B-A): muted clips are silent in
            // playback and in the export mux — the mixdown
            // must exclude them too, or a muted clip sounds
            // in the mix the user renders from here.
            .filter(|c| !c.muted)
            .map(|c| crate::audio_edit::MixClip {
                path: c.path.clone(),
                start_secs: c.start_frame as f32 / fps,
                gain: c.gain,
                fade_in: 0.0,
                fade_out: 0.0,
            })
            .collect();
        match crate::audio_edit::mix(&clips) {
            Ok(wav) => {
                let bytes = crate::audio_edit::write_wav(&wav);
                let dir = PathBuf::from("out");
                let _ = std::fs::create_dir_all(&dir);
                match std::fs::write(dir.join("mixdown.wav"), bytes) {
                    Ok(()) => self.status = "Mixed audio to out/mixdown.wav.".into(),
                    Err(e) => self.status = format!("Mixdown write failed: {e}"),
                }
            }
            Err(e) => self.status = format!("Mixdown failed: {e}"),
        }
    }

    // ---- Wave-10 integrations: formats, language, easing, VFX clips ----

    pub fn cycle_lang(&mut self) {
        // Cycle through every language the i18n tables know about, so
        // this keeps working as the language list grows.
        let all = crate::i18n::Lang::all();
        let idx = all.iter().position(|l| *l == self.lang).unwrap_or(0);
        self.lang = all[(idx + 1) % all.len()];
        self.status = format!("Language: {}", self.lang.label());
    }

    pub fn export_v5_file(&mut self) {
        // Wave X (X7, IA-X7-1): v5 export can refuse (an
        // undecodable image asset) — the refusal lands in
        // the op's existing failure voice and no file is
        // written.
        let bytes = match crate::pivot::export_piv_v5(&self.project) {
            Ok(bytes) => bytes,
            Err(e) => {
                self.status = format!("v5 export failed: {e}");
                return;
            }
        };
        let dir = PathBuf::from("out");
        let _ = std::fs::create_dir_all(&dir);
        match std::fs::write(dir.join("animation-v5.piv"), &bytes) {
            Ok(()) => self.status = "Exported out/animation-v5.piv (Pivot v5).".into(),
            Err(e) => self.status = format!("v5 export failed: {e}"),
        }
    }

    /// The file stem this core's own export ops name
    /// their single-file destinations from (Roadmap
    /// Wave 1, RM23 — IA-RM23-1): the project name,
    /// lowercased, with every run of non-alphanumeric
    /// characters collapsed to one `-` and no
    /// leading/trailing dash; underscores are kept; a
    /// name with no alphanumeric characters at all
    /// falls back to `animation`. This is the
    /// library-side mirror of the naming home — the
    /// bin's `export_panel::project_stem`, whose
    /// `ExportOp::dest_named` builds the same file
    /// names from the same stem; the two must agree.
    fn export_stem(&self) -> String {
        let mut stem = String::new();
        let mut pending_dash = false;
        for ch in self.project.name.chars() {
            if ch.is_alphanumeric() || ch == '_' {
                if pending_dash && !stem.is_empty() {
                    stem.push('-');
                }
                pending_dash = false;
                stem.extend(ch.to_lowercase());
            } else {
                pending_dash = true;
            }
        }
        if stem.is_empty() {
            "animation".to_string()
        } else {
            stem
        }
    }

    pub fn export_apng_file(&mut self) {
        let stem = self.export_stem();
        match crate::export::export_apng(
            &self.project,
            std::path::Path::new(&format!("out/{stem}.apng")),
        ) {
            Ok(_) => self.status = format!("Exported out/{stem}.apng."),
            Err(e) => self.status = format!("APNG export failed: {e}"),
        }
    }

    pub fn export_jpeg_dir(&mut self) {
        match crate::export::export_jpeg_sequence(&self.project, std::path::Path::new("out/jpeg")) {
            Ok(_) => self.status = "Exported JPEG frames to out/jpeg/.".into(),
            Err(e) => self.status = format!("JPEG export failed: {e}"),
        }
    }

    pub fn import_nodes_file(&mut self) {
        let Some(path) = self.first_file_with("import", &["nodes"]) else {
            self.status = "Put a .nodes file in the import folder first.".into();
            return;
        };
        match std::fs::read(&path) {
            Ok(bytes) => match crate::nodes::import_nodes(&bytes) {
                Ok(project) => {
                    self.snapshot_labeled("Import figure");
                    self.project = project;
                    self.current = 0;
                    self.selected_figure = 0;
                    self.selected_node = None;
                    // Wave P (P14): same funnel bypass as the
                    // nodes-animation import — refresh the
                    // missing-audio report for the new project.
                    self.refresh_missing_audio();
                    self.status = format!("Imported {} as a figure.", path.display());
                }
                Err(e) => self.status = format!("Stick Nodes import failed: {e}"),
            },
            Err(e) => self.status = format!("Could not read {}: {e}", path.display()),
        }
    }

    pub fn cycle_frame_easing(&mut self) {
        use crate::model::Easing;
        self.snapshot_labeled("Frame easing");
        let cur = self.current.min(self.project.frames.len().saturating_sub(1));
        if let Some(frame) = self.project.frames.get_mut(cur) {
            frame.easing = match frame.easing {
                None => Some(Easing::Linear),
                Some(Easing::Linear) => Some(Easing::Smooth),
                Some(Easing::Smooth) => Some(Easing::EaseIn),
                Some(Easing::EaseIn) => Some(Easing::EaseOut),
                Some(Easing::EaseOut) => None,
            };
            self.status = format!(
                "Frame {} easing: {}",
                cur + 1,
                frame.easing.map(|e| e.label()).unwrap_or("project default")
            );
        }
    }

    /// Set the CURRENT frame's easing directly (the easing
    /// picker's Apply / None buttons), with the same snapshot +
    /// status behaviour as `cycle_frame_easing`.
    pub fn set_frame_easing(&mut self, easing: Option<crate::model::Easing>) {
        self.snapshot_labeled("Frame easing");
        let cur = self.current.min(self.project.frames.len().saturating_sub(1));
        if let Some(frame) = self.project.frames.get_mut(cur) {
            frame.easing = easing;
            self.status = format!(
                "Frame {} easing: {}",
                cur + 1,
                frame.easing.map(|e| e.label()).unwrap_or("project default")
            );
        }
    }

    pub fn frame_easing_label(&self) -> String {
        match self.project.frames.get(self.current).and_then(|f| f.easing) {
            Some(e) => format!("F-ease: {}", e.label()),
            None => "F-ease: project".into(),
        }
    }

    /// Add a timed VFX clip of `kind` anchored at the selected node,
    /// starting at the current frame.
    pub fn add_fx_clip(&mut self, kind: crate::vfx::ClipKind) {
        use crate::vfx::{ClipKind, ParticleClip};
        let Some(node) = self.selected_node else {
            self.status = "Select a joint first, then add the FX clip.".into();
            return;
        };
        self.snapshot_labeled("Add FX clip");
        let color = self
            .project
            .figures
            .get(self.selected_figure)
            .map(|f| f.color)
            .unwrap_or(0xFFFFFF);
        self.project.fx_clips.push(crate::model::FxClipEntry {
            clip: ParticleClip {
                start: self.current as f32,
                duration: 16.0,
                kind,
                color,
                seed: 11 + self.project.fx_clips.len() as u32 * 7,
                angle: 0.0,
                length: 130.0,
                color2: None,
            },
            figure: self.selected_figure,
            node,
            hidden: false,
        });
        let _ = ClipKind::Burst;
        self.status = format!("FX clip ({kind:?}) added at frame {}.", self.current + 1);
    }

    // ---- FX presets (wave 13) ----

    /// Wave 13: save FX clip `index` as a preset in
    /// `./fx-presets/`, auto-named `<kind>-<index+1>` (for
    /// example `sparkle-2`). Saving a preset does not change
    /// the project, so no snapshot is taken. A missing index or
    /// a failed write sets an error status.
    pub fn fx_save_preset(&mut self, index: usize) {
        let Some(entry) = self.project.fx_clips.get(index) else {
            self.status = format!("FX preset save failed: no FX clip {}.", index + 1);
            return;
        };
        let name = format!(
            "{}-{}",
            crate::fx_edit::kind_name(entry.clip.kind),
            index + 1
        );
        match crate::fx_edit::save_clip_preset(&entry.clip, &name) {
            Ok(path) => {
                // Wave 10 (C7): the engine never overwrites
                // a different preset — when the positional
                // name was taken it writes a suffixed file
                // instead, and the status names the file
                // actually written.
                let written = path
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| name.clone());
                if written == name {
                    self.status = format!("FX preset '{name}' saved to {}.", path.display());
                } else {
                    self.status = format!(
                        "FX preset '{name}' saved to {} — '{name}' already holds a different preset.",
                        path.display()
                    );
                }
            }
            Err(e) => self.status = format!("FX preset save failed: {e}"),
        }
    }

    /// Wave 13: add the preset `name` from `./fx-presets/` as a
    /// new FX clip on the selected figure (node 0), starting at
    /// the current frame — the placement shape of `add_fx_clip`,
    /// as one undo step. A preset that cannot be loaded sets an
    /// error status and changes nothing.
    pub fn fx_apply_preset(&mut self, name: &str) {
        let mut clip = match crate::fx_edit::load_clip_preset(name) {
            Ok(clip) => clip,
            Err(e) => {
                self.status = format!("FX preset apply failed: {e}");
                return;
            }
        };
        clip.start = self.current as f32;
        self.snapshot_labeled("FX preset");
        self.project.fx_clips.push(crate::model::FxClipEntry {
            clip,
            figure: self.selected_figure,
            node: 0,
            hidden: false,
        });
        self.status = format!("FX preset '{name}' added at frame {}.", self.current + 1);
    }

    /// Wave 13: the slugs of the FX presets available in
    /// `./fx-presets/` (see `fx_edit::list_clip_presets`).
    pub fn fx_preset_names(&self) -> Vec<String> {
        crate::fx_edit::list_clip_presets()
    }


    // ---- Library ----

    pub fn cycle_library(&mut self) {
        if !self.library.is_empty() {
            self.library_index = (self.library_index + 1) % self.library.len();
            self.status = format!("Library: {}", self.library[self.library_index].0);
        }
    }

    pub fn insert_library_figure(&mut self) {
        if self.library.is_empty() {
            return;
        }
        self.snapshot_labeled("Insert library figure");
        let (name, mut def) = self.library[self.library_index].clone();
        // Wave 12 (B7): strip segment IMAGE media on insert.
        // A library file stores the FigureDef only — no asset
        // bytes — so a stored image index would resolve
        // against THIS project's asset table: the wrong image
        // drawn, or a silent skip when the index is out of
        // range. Text media is self-contained and survives.
        // (The library format carrying referenced PNGs would
        // be a format change; unbuilt — see §6.7.)
        let mut dropped = 0usize;
        for node in &mut def.nodes {
            if matches!(node.media, Some(crate::model::SegmentMedia::Image { .. })) {
                node.media = None;
                dropped += 1;
            }
        }
        let idx = self.project.figures.len();
        let color = FIGURE_COLORS[idx % FIGURE_COLORS.len()];
        let root = [300.0 + 80.0 * idx as f32, 450.0];
        self.project.add_figure_with(def, &name, color, root);
        self.selected_figure = idx;
        self.selected_node = None;
        self.status = if dropped > 0 {
            format!(
                "Inserted {name} from the library (segment images dropped: the library stores no image data)."
            )
        } else {
            format!("Inserted {name} from the library.")
        };
    }

    /// Save the selected figure's definition into
    /// `./library` and reload the library. The FILE keeps
    /// the historical slug name (spaces→`-`, lowercased),
    /// so existing libraries and the rename op's stem
    /// arithmetic are untouched; the figure's exact name
    /// is stored inside the file (see
    /// [`Self::library_file_text`]) and becomes the
    /// entry's display name on load — the name now
    /// round-trips exactly (Wave V, V1).
    pub fn save_figure_to_library(&mut self) {
        let fig = &self.project.figures[self.selected_figure];
        let dir = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("library");
        let file = dir.join(format!("{}.json", fig.name.replace(' ', "-").to_lowercase()));
        let result = std::fs::create_dir_all(&dir)
            .and_then(|_| {
                Self::library_file_text(&fig.def, &fig.name).map_err(std::io::Error::other)
            })
            .and_then(|text| std::fs::write(&file, text));
        match result {
            Ok(()) => {
                self.library = Self::load_library();
                self.status = format!("Saved {} to the library.", fig.name);
            }
            Err(e) => self.status = format!("Library save failed: {e}"),
        }
    }

    pub fn export_gif(&mut self) {
        let path = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("export")
            .join(format!("{}.gif", self.export_stem()));
        match render_gif(&self.project, &path) {
            Ok(n) => self.status = format!("Exported {n}-frame GIF to {}", path.display()),
            Err(e) => self.status = format!("GIF export failed: {e}"),
        }
    }

    // ---- Pivot formats ----

    fn first_file_with(&self, dir: &str, exts: &[&str]) -> Option<PathBuf> {
        let dir = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(dir);
        let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
            .map(|entries| {
                entries
                    .filter_map(|e| e.ok().map(|e| e.path()))
                    .filter(|p| {
                        p.extension()
                            .map(|x| exts.iter().any(|e| x.eq_ignore_ascii_case(e)))
                            .unwrap_or(false)
                    })
                    .collect()
            })
            .unwrap_or_default();
        files.sort();
        files.into_iter().next()
    }

    pub fn import_piv(&mut self) {
        let Some(path) = self.first_file_with("import", &["piv"]) else {
            self.status = "Put a .piv file in ./import/ first.".into();
            return;
        };
        match std::fs::read(&path).map_err(|e| e.to_string()).and_then(|b| crate::pivot::import_piv(&b)) {
            Ok(project) => {
                self.project = project;
                self.current = 0;
                self.playhead = 0.0;
                self.selected_figure = 0;
                self.selected_node = None;
                self.undo.clear();
                self.redo.clear();
                self.note_project_swapped();
                self.status = format!("Imported {}", path.display());
            }
            Err(e) => self.status = format!("Import failed: {e}"),
        }
    }

    pub fn import_stk(&mut self) {
        let Some(path) = self.first_file_with("import", &["stk"]) else {
            self.status = "Put a .stk file in ./import/ first.".into();
            return;
        };
        match std::fs::read(&path).map_err(|e| e.to_string()).and_then(|b| crate::pivot::import_stk(&b)) {
            Ok((def, name)) => {
                self.snapshot_labeled("Import figure");
                let idx = self.project.figures.len();
                let color = FIGURE_COLORS[idx % FIGURE_COLORS.len()];
                let label = if name.is_empty() { "Imported figure".to_string() } else { name };
                self.project.add_figure_with(def, &label, color, [320.0, 450.0]);
                self.selected_figure = idx;
                self.status = format!("Imported figure from {}", path.display());
            }
            Err(e) => self.status = format!("STK import failed: {e}"),
        }
    }

    pub fn export_piv(&mut self) {
        let dir = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("export");
        let path = dir.join("animation.piv");
        let bytes = crate::pivot::export_piv_v3(&self.project);
        match std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(&path, bytes)) {
            Ok(()) => self.status = format!("Exported Pivot .piv (v3) to {}", path.display()),
            Err(e) => self.status = format!("PIV export failed: {e}"),
        }
    }

    pub fn export_stk(&mut self) {
        let fig = &self.project.figures[self.selected_figure];
        let dir = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("export");
        let path = dir.join(format!("{}.stk", fig.name.replace(' ', "-").to_lowercase()));
        let bytes = crate::pivot::export_stk(&fig.def, &fig.name);
        match std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(&path, bytes)) {
            Ok(()) => self.status = format!("Exported {}.stk", fig.name),
            Err(e) => self.status = format!("STK export failed: {e}"),
        }
    }

    pub fn export_piv2(&mut self) {
        let dir = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("export");
        let path = dir.join("animation-v2.piv");
        let bytes = crate::pivot::export_piv_v2(&self.project);
        match std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(&path, bytes)) {
            Ok(()) => self.status = format!("Exported Pivot .piv (v2) to {}", path.display()),
            Err(e) => self.status = format!("PIV v2 export failed: {e}"),
        }
    }

    pub fn export_piv4(&mut self) {
        let dir = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("export");
        let path = dir.join("animation-v4.piv");
        let bytes = crate::pivot::export_piv_v4(&self.project);
        match std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(&path, bytes)) {
            Ok(()) => self.status = format!("Exported Pivot .piv (v4) to {}", path.display()),
            Err(e) => self.status = format!("PIV v4 export failed: {e}"),
        }
    }

    pub fn export_nodes(&mut self) {
        let fig = &self.project.figures[self.selected_figure];
        let dir = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("export");
        let path = dir.join(format!("{}.nodes", fig.name.replace(' ', "-").to_lowercase()));
        let bytes = crate::nodes::export_nodes_figure(&fig.def, &fig.name);
        match std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(&path, bytes)) {
            Ok(()) => self.status = format!("Exported {}.nodes", fig.name),
            Err(e) => self.status = format!("Nodes export failed: {e}"),
        }
    }

    pub fn export_stknds(&mut self) {
        let dir = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("export");
        let path = dir.join("animation.stknds");
        let bytes = crate::nodes_anim::export_nodes_animation(&self.project);
        match std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(&path, bytes)) {
            Ok(()) => self.status = format!("Exported Stick Nodes .stknds to {}", path.display()),
            Err(e) => self.status = format!("STKNDS export failed: {e}"),
        }
    }

    // ---- Drawing ----

    pub fn begin_draw(&mut self, p: V2) {
        if self.draw_tool == DrawTool::Eraser {
            // An eraser drag erases around the pointer instead of
            // recording a stroke; it continues in `draw_to`. One
            // snapshot covers the whole drag, like `lasso_erase`.
            self.snapshot_labeled("Draw stroke");
            let frame = self.current;
            let radius = self.eraser_radius();
            self.erase_changed =
                crate::draw_ops::erase(&mut self.project, frame, p, radius);
            self.drawing = true;
            return;
        }
        let brush = self.active_brush();
        let color = self.project.figures[self.selected_figure].color;
        // Wave 13 (census A18): one snapshot for the whole
        // stroke gesture, taken here before the push —
        // every other stroke-creation path snapshots
        // (the eraser branch above, stamps, the MCP draw
        // tools); before this, a hand-drawn stroke was
        // neither undoable nor counted. Never in
        // `draw_to`: per-move snapshots would flood the
        // undo stack.
        self.snapshot_labeled("Draw stroke");
        // Wave 13: a scatter brush (scatter > 0) records a spray
        // mark on the stroke. The seed is derived from the frame
        // and the stroke's own index in `project.strokes`, so the
        // scatter a renderer expands from the mark
        // (`paint::spray_dots`) is deterministic per stroke. The
        // recorded points stay the path the user drew; plain
        // brushes (scatter 0) record no mark.
        let spray = if brush.scatter > 0.0 {
            Some(crate::model::SprayMark {
                seed: 0x5EED_0000u64
                    + (self.current as u64) * 131
                    + self.project.strokes.len() as u64,
                radius: brush.scatter,
                opacity: brush.opacity,
            })
        } else {
            None
        };
        self.project.strokes.push(crate::model::Stroke {
            frame: self.current,
            points: vec![p],
            color,
            width: brush.width,
            widths: vec![brush.width],
            fill: None,
            spray,
            // A spray stroke records 1.0 here on purpose: its
            // dots already carry the brush's opacity in the
            // SprayMark, and a renderer applying both would
            // double-apply it. A plain stroke records the
            // brush's opacity itself.
            opacity: if spray.is_some() {
                1.0
            } else {
                recorded_stroke_opacity(brush.opacity)
            },
            // The brush's scatter is carried by the
            // SprayMark above — scatter > 0.0 is exactly
            // what made this a spray stroke — so the
            // stroke's own scatter stays 0.0 here. A
            // non-spray stroke's scatter arrives via
            // project files or MCP and renders through
            // paint::stroke_dabs.
            scatter: 0.0,
        });
        self.drawing = true;
    }

    pub fn draw_to(&mut self, p: V2) {
        if !self.drawing {
            return;
        }
        if self.draw_tool == DrawTool::Eraser {
            let frame = self.current;
            let radius = self.eraser_radius();
            self.erase_changed +=
                crate::draw_ops::erase(&mut self.project, frame, p, radius);
            return;
        }
        if let Some(stroke) = self.project.strokes.last_mut() {
            let last = *stroke.points.last().unwrap();
            let d = dist(last, p);
            if d > 2.0 {
                stroke.points.push(p);
                // Pressure stroke: the recorded per-point width follows
                // pointer speed — slow movement lays down up to 1.8x the
                // base width, a fast flick tapers to 0.45x. Smoothed
                // against the previous width so the taper doesn't
                // jitter. `width` stays the base; `widths` stays
                // parallel to `points`.
                let base = stroke.width;
                while stroke.widths.len() < stroke.points.len() - 1 {
                    stroke.widths.push(base);
                }
                let prev = stroke.widths.last().copied().unwrap_or(base);
                let t = ((d - 2.0) / 28.0).clamp(0.0, 1.0);
                let target = base * (1.8 - t * (1.8 - 0.45));
                let w = (prev + (target - prev) * 0.5).clamp(base * 0.45, base * 1.8);
                stroke.widths.push(w);
            }
        }
    }

    pub fn end_draw(&mut self) {
        if !self.drawing {
            return;
        }
        self.drawing = false;
        if self.draw_tool == DrawTool::Eraser {
            self.status = format!("Eraser: {} stroke(s) changed.", self.erase_changed);
            self.erase_changed = 0;
            return;
        }
        // Finalise the stroke with the active brush's pipeline, in
        // this order: Chaikin smoothing first (it repositions the
        // points), then the nib profile (computed from the smoothed
        // path, replacing the recorded pressure widths). The
        // default Pen brush has smoothing 0 and a round nib, so
        // neither step runs for it and the recorded stroke is left
        // exactly as drawn.
        let brush = self.active_brush();
        if let Some(stroke) = self.project.strokes.last_mut() {
            if brush.smoothing > 0 {
                crate::draw_ops::smooth_stroke(stroke, brush.smoothing);
            }
            if brush.nib != crate::paint::Nib::Round {
                crate::draw_ops::apply_nib(stroke, brush.nib, brush.nib_angle);
            }
            let n = stroke.points.len();
            self.status = format!("Stroke drawn ({n} points).");
        }
    }

    /// Delete the stroke at `index` in `project.strokes`
    /// (the stroke inspector's per-stroke delete), with a
    /// snapshot so the delete is undoable. Out of range only
    /// reports — nothing is removed or snapshotted.
    pub fn delete_stroke_at(&mut self, index: usize) {
        if index >= self.project.strokes.len() {
            self.status = format!("No stroke {} to delete.", index + 1);
            return;
        }
        self.snapshot_labeled("Delete stroke");
        self.project.strokes.remove(index);
        self.status = format!("Stroke {} deleted.", index + 1);
    }

    pub fn clear_frame_drawings(&mut self) {
        let before = self.project.strokes.len();
        let on_frame = self.project.strokes.iter().filter(|s| s.frame == self.current).count();
        // Wave 13 (census A17, overruling B2's landed
        // mark): stroke destruction snapshots like every
        // peer op (`delete_stroke_at`, `fill_current_stroke`,
        // `lasso_erase`) — the op owns the one undo step,
        // so the MCP handler takes none of its own. An
        // empty frame spends nothing.
        if on_frame > 0 {
            self.snapshot_labeled("Clear frame drawings");
        }
        self.project.strokes.retain(|s| s.frame != self.current);
        let removed = before - self.project.strokes.len();
        self.status = format!("Cleared {} stroke(s) on this frame.", removed);
    }

    /// Fill the most recent stroke on this frame: the stroke's outline
    /// is treated as a closed shape, its `fill` is set to its own
    /// colour, and a serpentine scanline fill (see `paint`) is
    /// appended as a separate stroke on the same frame.
    pub fn fill_current_stroke(&mut self) {
        let frame = self.current;
        let Some(src) = self
            .project
            .strokes
            .iter()
            .rev()
            .find(|s| s.frame == frame)
        else {
            self.status = "Nothing to fill: draw a closed shape on this frame first.".into();
            return;
        };
        if src.points.len() < 3 {
            self.status = "Nothing to fill: the last stroke needs at least 3 points to make a shape.".into();
            return;
        }
        let spacing = src.width;
        let path = crate::paint::polyline_fill_path(&src.points, spacing);
        if path.len() < 2 {
            self.status = "Nothing to fill: the shape is too small or flat to hold a fill.".into();
            return;
        }
        let color = src.color;
        let n_points = path.len();
        self.snapshot_labeled("Fill stroke");
        if let Some(s) = self
            .project
            .strokes
            .iter_mut()
            .rev()
            .find(|s| s.frame == frame)
        {
            s.fill = Some(color);
        }
        self.project.strokes.push(crate::model::Stroke {
            frame,
            points: path,
            color,
            width: spacing,
            widths: Vec::new(),
            fill: Some(color),
            spray: None,
            // Generated from the source stroke, not drawn with
            // the brush — its colour and width come from that
            // stroke too — so it records opaque rather than
            // borrowing the active brush's opacity.
            opacity: 1.0,
            scatter: 0.0,
        });
        self.status = format!("Shape filled with a scanline fill ({n_points} points).");
    }

    /// Erase strokes on this frame that lie mostly (a strict majority
    /// of their points) inside the lasso polygon `poly`.
    pub fn lasso_erase(&mut self, poly: &[V2]) {
        if poly.len() < 3 {
            self.status = "Lasso needs at least 3 points; nothing erased.".into();
            return;
        }
        let frame = self.current;
        let mostly_inside = |s: &crate::model::Stroke| {
            s.frame == frame
                && !s.points.is_empty()
                && s.points
                    .iter()
                    .filter(|p| crate::paint::point_in_polygon(**p, poly))
                    .count()
                    * 2
                    > s.points.len()
        };
        let n = self.project.strokes.iter().filter(|s| mostly_inside(s)).count();
        if n == 0 {
            self.status = "Lasso erased nothing: no stroke lies mostly inside it.".into();
            return;
        }
        self.snapshot_labeled("Lasso erase");
        self.project.strokes.retain(|s| !mostly_inside(s));
        self.status = format!("Lasso erased {n} stroke(s).");
    }

    /// Wave 12: mirror every stroke on the current frame across
    /// the stage's vertical centre line (x = stage width / 2), as
    /// one undo step. Only the points change — colour, width,
    /// per-point widths and fill stay as drawn — and strokes on
    /// other frames are left alone. Mirroring twice restores the
    /// original drawing exactly. A frame with no strokes is a
    /// no-op (no snapshot).
    pub fn mirror_strokes_x(&mut self) {
        let frame = self.current;
        let n = self
            .project
            .strokes
            .iter()
            .filter(|s| s.frame == frame)
            .count();
        if n == 0 {
            self.status = format!("No strokes to mirror on frame {}.", frame + 1);
            return;
        }
        let cx = self.project.stage[0] / 2.0;
        self.snapshot_labeled("Mirror strokes");
        for s in &mut self.project.strokes {
            if s.frame == frame {
                s.points = crate::paint::mirror_points_x(&s.points, cx);
            }
        }
        self.status = format!(
            "Mirrored {n} stroke(s) on frame {} across the stage centre.",
            frame + 1
        );
    }

    /// Shared stamp construction: a generated outline becomes
    /// ONE stroke on the current frame, built exactly like a
    /// drawn stroke (`begin_draw`): the selected figure's
    /// colour, the active brush's width recorded per point (a
    /// drawn stroke carries a width for every point), the
    /// active brush's opacity, and no fill. One snapshot per
    /// stamp.
    fn stamp_points(&mut self, points: Vec<V2>, what: &str) {
        let brush = self.active_brush();
        let color = self.project.figures[self.selected_figure].color;
        let n = points.len();
        self.snapshot_labeled(&format!("Stamp {what}"));
        self.project.strokes.push(crate::model::Stroke {
            frame: self.current,
            points,
            color,
            width: brush.width,
            widths: vec![brush.width; n],
            fill: None,
            spray: None,
            opacity: recorded_stroke_opacity(brush.opacity),
            // A stamp is never a spray stroke, so the
            // brush's scatter lives on the stroke itself
            // and the renderers jitter its dabs.
            scatter: brush.scatter,
        });
        self.status = format!("Stamped {what} on frame {} ({n} points).", self.current + 1);
    }

    /// Wave 12: stamp a regular pentagon (radius 60, one vertex
    /// pointing up) at the stage centre on the current frame.
    pub fn stamp_polygon(&mut self) {
        let c = [self.project.stage[0] / 2.0, self.project.stage[1] / 2.0];
        let points = crate::paint::regular_polygon_points(c, 60.0, 5, -90.0);
        self.stamp_points(points, "a pentagon");
    }

    /// Wave 12: stamp a five-pointed star (outer radius 60,
    /// inner radius 26, one point up) at the stage centre on
    /// the current frame.
    pub fn stamp_star(&mut self) {
        let c = [self.project.stage[0] / 2.0, self.project.stage[1] / 2.0];
        let points = crate::paint::star_points(c, 60.0, 26.0, 5, -90.0);
        self.stamp_points(points, "a star");
    }

    /// Wave 12: stamp a horizontal arrow at the stage centre on
    /// the current frame, pointing right: the shaft runs from
    /// 70 units left of centre to 70 units right, with a
    /// 22-long head.
    pub fn stamp_arrow(&mut self) {
        let cx = self.project.stage[0] / 2.0;
        let cy = self.project.stage[1] / 2.0;
        let points = crate::paint::arrow_points([cx - 70.0, cy], [cx + 70.0, cy], 22.0);
        self.stamp_points(points, "an arrow");
    }

    // ---- Draw tools (wave 7) ----

    /// The active brush preset from `paint::brush_library`
    /// (`brush_index` clamped, so a stale index can never panic).
    pub fn active_brush(&self) -> crate::paint::Brush {
        let lib = crate::paint::brush_library();
        lib[self.brush_index.min(lib.len() - 1)]
    }

    /// Eraser radius for the active brush: twice its width, so a
    /// fatter brush also erases a fatter swath.
    fn eraser_radius(&self) -> f32 {
        self.active_brush().width * 2.0
    }

    /// Cycle to the next brush preset in the library (wrapping).
    /// Applies to whichever tool is active: the brush paints with
    /// it, the eraser is sized by it.
    pub fn cycle_brush(&mut self) {
        let lib = crate::paint::brush_library();
        self.brush_index = (self.brush_index + 1) % lib.len();
        let b = self.active_brush();
        self.status = format!("Brush: {} (width {:.0}).", b.name, b.width);
    }

    /// True when the eraser is the active draw tool (the app's
    /// tool-toggle button highlights and labels itself from this,
    /// so it never has to name `DrawTool` itself).
    pub fn draw_tool_is_eraser(&self) -> bool {
        self.draw_tool == DrawTool::Eraser
    }

    /// Switch the draw tool between brush and eraser.
    pub fn toggle_draw_tool(&mut self) {
        self.draw_tool = match self.draw_tool {
            DrawTool::Brush => DrawTool::Eraser,
            DrawTool::Eraser => DrawTool::Brush,
        };
        self.status = match self.draw_tool {
            DrawTool::Eraser => "Draw tool: Eraser.".into(),
            DrawTool::Brush => format!("Draw tool: Brush ({}).", self.active_brush().name),
        };
    }

    // ---- Audio ----

    pub fn add_audio_clip(&mut self) {
        let Some(path) = self.first_file_with("audio", &["wav", "mp3", "ogg"]) else {
            self.status = "Put an audio file (.wav/.mp3/.ogg) in ./audio/ first.".into();
            return;
        };
        self.snapshot_labeled("Add audio clip");
        self.project.audio.push(crate::model::AudioClip {
            path: path.to_string_lossy().into_owned(),
            start_frame: self.current,
            gain: 1.0,
            muted: false,
        });
        self.status = format!("Audio clip added at frame {}: {}", self.current + 1, path.display());
    }

    // ---- Wave 6 audio editing ----

    /// Path for an audio edit product: same directory as `source`,
    /// with the source file stem plus `suffix` (for example
    /// `.norm.wav`). The suffix always changes the file name, so an
    /// edit product cannot accidentally be its own source.
    fn audio_derived_path(source: &PathBuf, suffix: &str) -> PathBuf {
        let stem = source
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "audio".into());
        let mut out = source.clone();
        out.set_file_name(format!("{stem}{suffix}"));
        out
    }

    /// Normalise audio clip `clip_idx` to a peak of 0.9.
    ///
    /// The normalised audio is written beside the source as
    /// `<stem>.norm.wav`; the source file is never overwritten.
    /// Only after that file has been written does the clip point to
    /// it. A missing index or a source that cannot be read as a WAV
    /// sets an error status and leaves the project unchanged.
    pub fn audio_normalize(&mut self, clip_idx: usize) {
        let Some(clip) = self.project.audio.get(clip_idx).cloned() else {
            self.status = format!("Audio normalise failed: no clip {}.", clip_idx + 1);
            return;
        };
        let mut wav = match crate::audio_edit::read_wav(&clip.path) {
            Ok(wav) => wav,
            Err(e) => {
                self.status = format!("Audio normalise failed: {e}");
                return;
            }
        };
        let gain = crate::audio_edit::normalize(&mut wav, 0.9);
        let source = PathBuf::from(&clip.path);
        let out = Self::audio_derived_path(&source, ".norm.wav");
        if out == source {
            self.status = "Audio normalise failed: output would overwrite the source.".into();
            return;
        }
        if let Err(e) = std::fs::write(&out, crate::audio_edit::write_wav(&wav)) {
            self.status = format!(
                "Audio normalise failed: could not write {}: {e}",
                out.display()
            );
            return;
        }
        self.snapshot_labeled("Normalize audio");
        self.project.audio[clip_idx].path = out.to_string_lossy().into_owned();
        self.status = format!(
            "Audio clip {} normalised to peak 0.90 (gain {gain:.2}x): {}.",
            clip_idx + 1,
            out.display()
        );
    }

    /// Trim audio clip `clip_idx` to `[start_secs, end_secs)` and
    /// write the result beside the source as `<stem>.trim.wav`.
    ///
    /// When `start_secs` is positive, the clip's `start_frame`
    /// advances by `round(start_secs * fps)`. The trimmed head held
    /// the clip's earliest audible content, so the remaining content
    /// originally sounded that much later; shifting the placement by
    /// the same amount preserves its position on the timeline. A
    /// non-positive start does not move the clip. An empty trim
    /// range is an error and leaves the project unchanged.
    pub fn audio_trim(&mut self, clip_idx: usize, start_secs: f32, end_secs: f32) {
        let Some(clip) = self.project.audio.get(clip_idx).cloned() else {
            self.status = format!("Audio trim failed: no clip {}.", clip_idx + 1);
            return;
        };
        if !start_secs.is_finite() || !end_secs.is_finite() {
            self.status = "Audio trim failed: trim times must be finite.".into();
            return;
        }
        let wav = match crate::audio_edit::read_wav(&clip.path) {
            Ok(wav) => wav,
            Err(e) => {
                self.status = format!("Audio trim failed: {e}");
                return;
            }
        };
        let trimmed = crate::audio_edit::trim(&wav, start_secs, end_secs);
        if trimmed.frames() == 0 {
            self.status = "Audio trim failed: the trim range is empty.".into();
            return;
        }
        let source = PathBuf::from(&clip.path);
        let out = Self::audio_derived_path(&source, ".trim.wav");
        if out == source {
            self.status = "Audio trim failed: output would overwrite the source.".into();
            return;
        }
        if let Err(e) = std::fs::write(&out, crate::audio_edit::write_wav(&trimmed)) {
            self.status = format!("Audio trim failed: could not write {}: {e}", out.display());
            return;
        }
        let fps = self.project.fps.max(1.0);
        let shift = if start_secs > 0.0 {
            (start_secs * fps).round().max(0.0) as usize
        } else {
            0
        };
        self.snapshot_labeled("Trim audio clip");
        let clip = &mut self.project.audio[clip_idx];
        clip.path = out.to_string_lossy().into_owned();
        clip.start_frame = clip.start_frame.saturating_add(shift);
        self.status = format!(
            "Audio clip {} trimmed to {:.2}s; starts at frame {}.",
            clip_idx + 1,
            trimmed.duration_secs(),
            clip.start_frame + 1
        );
    }

    /// Split audio clip `clip_idx` at timeline frame `at_frame`.
    ///
    /// `crate::audio_edit::split_clip` splits the timeline placement
    /// and `crate::audio_edit::split_wav` cuts the actual audio.
    /// Both halves are materialised as real files, `<stem>.a.wav`
    /// and `<stem>.b.wav`, and the second clip points at the `.b`
    /// file. Materialising the tail is what makes playback of the
    /// second clip correct: `AudioClip` has no source-offset field,
    /// so a second clip pointing at the original file would restart
    /// the audio from its beginning. The second clip is inserted
    /// immediately after the first. A split point that is not
    /// strictly inside the clip is an error and leaves the project
    /// unchanged.
    pub fn audio_split(&mut self, clip_idx: usize, at_frame: usize) {
        let Some(clip) = self.project.audio.get(clip_idx).cloned() else {
            self.status = format!("Audio split failed: no clip {}.", clip_idx + 1);
            return;
        };
        let wav = match crate::audio_edit::read_wav(&clip.path) {
            Ok(wav) => wav,
            Err(e) => {
                self.status = format!("Audio split failed: {e}");
                return;
            }
        };
        let fps = self.project.fps.max(1.0);
        let dur_frames = (wav.duration_secs() * fps).round() as usize;
        let Some((mut first, mut second)) =
            crate::audio_edit::split_clip(&clip, at_frame, dur_frames)
        else {
            self.status = format!(
                "Audio split failed: frame {} is not inside clip {}.",
                at_frame + 1,
                clip_idx + 1
            );
            return;
        };
        let split_secs = (at_frame - clip.start_frame) as f32 / fps;
        let (first_wav, second_wav) = crate::audio_edit::split_wav(&wav, split_secs);
        if first_wav.frames() == 0 || second_wav.frames() == 0 {
            self.status = "Audio split failed: the split point leaves an empty half.".into();
            return;
        }
        let source = PathBuf::from(&clip.path);
        let first_path = Self::audio_derived_path(&source, ".a.wav");
        let second_path = Self::audio_derived_path(&source, ".b.wav");
        if let Err(e) = std::fs::write(&first_path, crate::audio_edit::write_wav(&first_wav)) {
            self.status = format!(
                "Audio split failed: could not write {}: {e}",
                first_path.display()
            );
            return;
        }
        if let Err(e) = std::fs::write(&second_path, crate::audio_edit::write_wav(&second_wav)) {
            self.status = format!(
                "Audio split failed: could not write {}: {e}",
                second_path.display()
            );
            return;
        }
        first.path = first_path.to_string_lossy().into_owned();
        second.path = second_path.to_string_lossy().into_owned();
        self.snapshot_labeled("Split audio clip");
        self.project.audio[clip_idx] = first;
        self.project.audio.insert(clip_idx + 1, second);
        self.status = format!(
            "Audio clip {} split at frame {} into {} and {}.",
            clip_idx + 1,
            at_frame + 1,
            first_path.display(),
            second_path.display()
        );
    }

    /// Crossfade audio clip `clip_idx` into the following clip and
    /// replace the pair with one clip at the first clip's start.
    ///
    /// If the clips overlap on the timeline, the fade lasts for the
    /// overlap, preserving the second clip's timeline position in
    /// the joined file. If they are merely adjacent, the fade is
    /// 1.0 seconds, capped by the shorter clip's duration; creating
    /// that overlap necessarily makes the joined file shorter than
    /// the two clips laid end to end. Clips separated by a gap, an
    /// overlap too long for either source, or a source that cannot
    /// be read as a WAV are errors and leave the project unchanged.
    /// The joined audio is written beside the first source as
    /// `<stem>.xfade.wav`.
    pub fn audio_crossfade(&mut self, clip_idx: usize) {
        if clip_idx + 1 >= self.project.audio.len() {
            self.status = format!(
                "Audio crossfade failed: clip {} has no following clip.",
                clip_idx + 1
            );
            return;
        }
        let first = self.project.audio[clip_idx].clone();
        let second = self.project.audio[clip_idx + 1].clone();
        let first_wav = match crate::audio_edit::read_wav(&first.path) {
            Ok(wav) => wav,
            Err(e) => {
                self.status = format!("Audio crossfade failed: {e}");
                return;
            }
        };
        let second_wav = match crate::audio_edit::read_wav(&second.path) {
            Ok(wav) => wav,
            Err(e) => {
                self.status = format!("Audio crossfade failed: {e}");
                return;
            }
        };
        if first_wav.frames() == 0 || second_wav.frames() == 0 {
            self.status = "Audio crossfade failed: a clip is empty.".into();
            return;
        }
        let fps = self.project.fps.max(1.0);
        let first_duration = first_wav.duration_secs();
        let second_duration = second_wav.duration_secs();
        let first_start = first.start_frame as f32 / fps;
        let second_start = second.start_frame as f32 / fps;
        let overlap = first_start + first_duration - second_start;
        // Starts and ends are frame-quantised; half a frame of
        // slack keeps an exactly adjacent pair from looking gapped
        // after floating-point conversion.
        let adjacency_tolerance = 0.5 / fps;
        let fade = if overlap > adjacency_tolerance {
            overlap
        } else if overlap >= -adjacency_tolerance {
            1.0_f32.min(first_duration).min(second_duration)
        } else {
            self.status = format!(
                "Audio crossfade failed: clips {} and {} are separated by a {:.2}s gap.",
                clip_idx + 1,
                clip_idx + 2,
                -overlap
            );
            return;
        };
        if fade <= 0.0 || fade > first_duration || fade > second_duration {
            self.status =
                "Audio crossfade failed: the clips do not overlap by a usable amount.".into();
            return;
        }
        let mixed = match crate::audio_edit::crossfade_mix(&first_wav, &second_wav, fade) {
            Ok(wav) => wav,
            Err(e) => {
                self.status = format!("Audio crossfade failed: {e}");
                return;
            }
        };
        let source = PathBuf::from(&first.path);
        let out = Self::audio_derived_path(&source, ".xfade.wav");
        if let Err(e) = std::fs::write(&out, crate::audio_edit::write_wav(&mixed)) {
            self.status = format!(
                "Audio crossfade failed: could not write {}: {e}",
                out.display()
            );
            return;
        }
        self.snapshot_labeled("Crossfade audio");
        self.project.audio[clip_idx].path = out.to_string_lossy().into_owned();
        self.project.audio.remove(clip_idx + 1);
        self.status = format!(
            "Audio clips {} and {} crossfaded ({fade:.2}s) into {}.",
            clip_idx + 1,
            clip_idx + 2,
            out.display()
        );
    }

    // ---- Wave 11 audio effects (last clip) ----

    /// Wave 11: apply a feedback echo (0.18 s delay, 0.35
    /// feedback) to the LAST audio clip. Follows the
    /// `audio_normalize` pattern: the processed audio is written
    /// beside the source as `<stem>.echo.wav`, the source file is
    /// never overwritten, and only after the write does the clip
    /// point to the new file (behind one undo snapshot).
    pub fn audio_echo_last(&mut self) {
        let Some(clip_idx) = self.project.audio.len().checked_sub(1) else {
            self.status = "Audio echo failed: there are no audio clips.".into();
            return;
        };
        let clip = self.project.audio[clip_idx].clone();
        let mut wav = match crate::audio_edit::read_wav(&clip.path) {
            Ok(wav) => wav,
            Err(e) => {
                self.status = format!("Audio echo failed: {e}");
                return;
            }
        };
        crate::audio_edit::echo(&mut wav, 0.18, 0.35);
        let source = PathBuf::from(&clip.path);
        let out = Self::audio_derived_path(&source, ".echo.wav");
        if out == source {
            self.status = "Audio echo failed: output would overwrite the source.".into();
            return;
        }
        if let Err(e) = std::fs::write(&out, crate::audio_edit::write_wav(&wav)) {
            self.status = format!("Audio echo failed: could not write {}: {e}", out.display());
            return;
        }
        self.snapshot_labeled("Echo audio");
        self.project.audio[clip_idx].path = out.to_string_lossy().into_owned();
        self.status = format!(
            "Echo applied to audio clip {} (0.18s delay, 35% feedback): {}.",
            clip_idx + 1,
            out.display()
        );
    }

    /// Wave 11: apply a 1200 Hz low-pass filter to the LAST
    /// audio clip. Same materialise-and-repoint pattern as
    /// `audio_echo_last`; the product is `<stem>.lowpass.wav`.
    pub fn audio_lowpass_last(&mut self) {
        let Some(clip_idx) = self.project.audio.len().checked_sub(1) else {
            self.status = "Audio low-pass failed: there are no audio clips.".into();
            return;
        };
        let clip = self.project.audio[clip_idx].clone();
        let mut wav = match crate::audio_edit::read_wav(&clip.path) {
            Ok(wav) => wav,
            Err(e) => {
                self.status = format!("Audio low-pass failed: {e}");
                return;
            }
        };
        crate::audio_edit::lowpass(&mut wav, 1200.0);
        let source = PathBuf::from(&clip.path);
        let out = Self::audio_derived_path(&source, ".lowpass.wav");
        if out == source {
            self.status = "Audio low-pass failed: output would overwrite the source.".into();
            return;
        }
        if let Err(e) = std::fs::write(&out, crate::audio_edit::write_wav(&wav)) {
            self.status = format!(
                "Audio low-pass failed: could not write {}: {e}",
                out.display()
            );
            return;
        }
        self.snapshot_labeled("Lowpass audio");
        self.project.audio[clip_idx].path = out.to_string_lossy().into_owned();
        self.status = format!(
            "Low-pass filter (1200 Hz) applied to audio clip {}: {}.",
            clip_idx + 1,
            out.display()
        );
    }

    /// Wave 11: trim leading and trailing silence (samples below
    /// 0.02) from the LAST audio clip. Same materialise-and-
    /// repoint pattern as `audio_echo_last`; the product is
    /// `<stem>.silence.wav`. When the clip has no silence at
    /// either edge there is nothing to write: no file is created,
    /// no snapshot is taken, and the status says so.
    pub fn audio_trim_silence_last(&mut self) {
        let Some(clip_idx) = self.project.audio.len().checked_sub(1) else {
            self.status = "Audio trim silence failed: there are no audio clips.".into();
            return;
        };
        let clip = self.project.audio[clip_idx].clone();
        let mut wav = match crate::audio_edit::read_wav(&clip.path) {
            Ok(wav) => wav,
            Err(e) => {
                self.status = format!("Audio trim silence failed: {e}");
                return;
            }
        };
        let (lead, trail) = crate::audio_edit::trim_silence(&mut wav, 0.02);
        if lead + trail == 0 {
            self.status = format!("No silence to trim from audio clip {}.", clip_idx + 1);
            return;
        }
        let source = PathBuf::from(&clip.path);
        let out = Self::audio_derived_path(&source, ".silence.wav");
        if out == source {
            self.status = "Audio trim silence failed: output would overwrite the source.".into();
            return;
        }
        if let Err(e) = std::fs::write(&out, crate::audio_edit::write_wav(&wav)) {
            self.status = format!(
                "Audio trim silence failed: could not write {}: {e}",
                out.display()
            );
            return;
        }
        self.snapshot_labeled("Trim silence");
        self.project.audio[clip_idx].path = out.to_string_lossy().into_owned();
        self.status = format!(
            "Trimmed {} silent frame(s) ({} at the start, {} at the end) from audio clip {}: {}.",
            lead + trail,
            lead,
            trail,
            clip_idx + 1,
            out.display()
        );
    }

    /// Wave 12: pitch-shift the LAST audio clip up 2 semitones
    /// (a rate shift — pitch and duration change together, the
    /// stored sample rate is untouched). Same materialise-and-
    /// repoint pattern as `audio_echo_last`; the product is
    /// `<stem>.pitch.wav` and the source file is never
    /// overwritten.
    pub fn audio_pitch_last(&mut self) {
        let Some(clip_idx) = self.project.audio.len().checked_sub(1) else {
            self.status = "Audio pitch shift failed: there are no audio clips.".into();
            return;
        };
        let clip = self.project.audio[clip_idx].clone();
        let mut wav = match crate::audio_edit::read_wav(&clip.path) {
            Ok(wav) => wav,
            Err(e) => {
                self.status = format!("Audio pitch shift failed: {e}");
                return;
            }
        };
        crate::audio_edit::pitch_shift(&mut wav, 2.0);
        let source = PathBuf::from(&clip.path);
        let out = Self::audio_derived_path(&source, ".pitch.wav");
        if out == source {
            self.status = "Audio pitch shift failed: output would overwrite the source.".into();
            return;
        }
        if let Err(e) = std::fs::write(&out, crate::audio_edit::write_wav(&wav)) {
            self.status = format!(
                "Audio pitch shift failed: could not write {}: {e}",
                out.display()
            );
            return;
        }
        self.snapshot_labeled("Pitch audio");
        self.project.audio[clip_idx].path = out.to_string_lossy().into_owned();
        self.status = format!(
            "Pitch shift (+2 semitones) applied to audio clip {}: {}.",
            clip_idx + 1,
            out.display()
        );
    }

    /// Wave 12: run a compressor (threshold 0.5, ratio 4.0)
    /// over the LAST audio clip. Same materialise-and-repoint
    /// pattern as `audio_echo_last`; the product is
    /// `<stem>.comp.wav` and the source file is never
    /// overwritten.
    pub fn audio_compress_last(&mut self) {
        let Some(clip_idx) = self.project.audio.len().checked_sub(1) else {
            self.status = "Audio compress failed: there are no audio clips.".into();
            return;
        };
        let clip = self.project.audio[clip_idx].clone();
        let mut wav = match crate::audio_edit::read_wav(&clip.path) {
            Ok(wav) => wav,
            Err(e) => {
                self.status = format!("Audio compress failed: {e}");
                return;
            }
        };
        crate::audio_edit::compress(&mut wav, 0.5, 4.0);
        let source = PathBuf::from(&clip.path);
        let out = Self::audio_derived_path(&source, ".comp.wav");
        if out == source {
            self.status = "Audio compress failed: output would overwrite the source.".into();
            return;
        }
        if let Err(e) = std::fs::write(&out, crate::audio_edit::write_wav(&wav)) {
            self.status = format!(
                "Audio compress failed: could not write {}: {e}",
                out.display()
            );
            return;
        }
        self.snapshot_labeled("Compress audio");
        self.project.audio[clip_idx].path = out.to_string_lossy().into_owned();
        self.status = format!(
            "Compressor (threshold 0.5, ratio 4.0) applied to audio clip {}: {}.",
            clip_idx + 1,
            out.display()
        );
    }

    /// Wave 13: split the LAST audio clip wherever silence
    /// (below 0.02) lasts at least a quarter second, using
    /// `audio_edit::split_at_silences` to find the sounding
    /// ranges. Each range is materialised beside the source as
    /// `<stem>.seg1.wav`, `<stem>.seg2.wav`, ... — a segment
    /// clip pointing at the original file would restart the
    /// audio from its beginning, because `AudioClip` has no
    /// source-offset field (the same reason `audio_split`
    /// materialises its tail). The first segment replaces the
    /// last clip and the remaining segments are inserted
    /// immediately after it, each starting at the original
    /// start plus `round(range_start_secs * fps)` — the shift
    /// convention `audio_trim` uses for a trimmed head — and
    /// keeping the original clip's gain. One snapshot, taken
    /// only after every segment file is written. Fewer than
    /// two ranges (no qualifying silence) is a no-op: no files
    /// are written and no snapshot is taken.
    pub fn audio_split_at_silences_last(&mut self) {
        let Some(clip_idx) = self.project.audio.len().checked_sub(1) else {
            self.status = "Audio split at silences failed: there are no audio clips.".into();
            return;
        };
        let clip = self.project.audio[clip_idx].clone();
        let wav = match crate::audio_edit::read_wav(&clip.path) {
            Ok(wav) => wav,
            Err(e) => {
                self.status = format!("Audio split at silences failed: {e}");
                return;
            }
        };
        let rate = wav.sample_rate;
        let min_silence = (rate as f32 * 0.25) as usize;
        let ranges = crate::audio_edit::split_at_silences(&wav, 0.02, min_silence);
        if ranges.len() < 2 {
            self.status = format!(
                "Audio clip {} has no silence of 0.25s or more to split at.",
                clip_idx + 1
            );
            return;
        }
        let source = PathBuf::from(&clip.path);
        let ch = wav.channels as usize;
        let fps = self.project.fps.max(1.0);
        let mut segments: Vec<crate::model::AudioClip> =
            Vec::with_capacity(ranges.len());
        for (i, &(start, end)) in ranges.iter().enumerate() {
            let seg = crate::audio_edit::WavData {
                sample_rate: wav.sample_rate,
                channels: wav.channels,
                samples: wav.samples[start * ch..end * ch].to_vec(),
            };
            let out = Self::audio_derived_path(&source, &format!(".seg{}.wav", i + 1));
            if let Err(e) = std::fs::write(&out, crate::audio_edit::write_wav(&seg)) {
                self.status = format!(
                    "Audio split at silences failed: could not write {}: {e}",
                    out.display()
                );
                return;
            }
            let start_secs = start as f32 / rate.max(1) as f32;
            let shift = (start_secs * fps).round().max(0.0) as usize;
            segments.push(crate::model::AudioClip {
                path: out.to_string_lossy().into_owned(),
                start_frame: clip.start_frame.saturating_add(shift),
                gain: clip.gain,
                // Wave 11 (B6 B-B): a split must not unmute —
                // the segments carry the source clip's flag.
                muted: clip.muted,
            });
        }
        let n = segments.len();
        self.snapshot_labeled("Split audio at silences");
        // The first segment replaces the split clip; the rest
        // are inserted immediately after it, in order (the
        // placement `audio_split` uses for its second half).
        let mut segments = segments.into_iter();
        self.project.audio[clip_idx] =
            segments.next().expect("at least two ranges were checked");
        for (j, seg) in segments.enumerate() {
            self.project.audio.insert(clip_idx + 1 + j, seg);
        }
        self.status = format!(
            "Audio clip {} split at silences into {n} clips.",
            clip_idx + 1
        );
    }

    // ---- Movieclips ----

    pub fn make_clip_from_figure(&mut self) {
        let fi = self.selected_figure;
        // Wave 11 (E6 residual): guard the selection and the
        // per-frame pose reads — on a project where a frame
        // lacks this figure's pose row (an import mid-repair)
        // the direct indexing panicked. The fallback chain is
        // create_clip_from_figure's idiom.
        if fi >= self.project.figures.len() {
            self.status = "No figure selected to make a clip from.".into();
            return;
        }
        self.snapshot_labeled("Make movieclip");
        let fig = &self.project.figures[fi];
        let mut fallback =
            fig.def.rest_pose([self.project.stage[0] / 2.0, self.project.stage[1] / 2.0]);
        let frames: Vec<crate::model::Pose> = self
            .project
            .frames
            .iter()
            .map(|f| {
                let pose = f.poses.get(fi).cloned().unwrap_or_else(|| fallback.clone());
                fallback = pose.clone();
                pose
            })
            .collect();
        let name = format!("{} clip", fig.name);
        self.project.clips.push(crate::model::ClipDef {
            name: name.clone(),
            def: fig.def.clone(),
            frames,
            loop_mode: crate::model::ClipLoop::Loop,
        });
        let ci = self.project.clips.len() - 1;
        self.project.figures[fi].clip = Some(ci);
        self.status = format!("{name} created; this figure now loops its own timeline.");
    }

    pub fn insert_clip_figure(&mut self) {
        if self.project.clips.is_empty() {
            self.status = "No movieclips yet — select a figure and use Make clip.".into();
            return;
        }
        self.snapshot_labeled("Insert clip figure");
        let ci = 0;
        let clip = self.project.clips[ci].clone();
        let idx = self.project.figures.len();
        let color = FIGURE_COLORS[(idx + 3) % FIGURE_COLORS.len()];
        self.project
            .add_figure_with(clip.def.clone(), &clip.name, color, [380.0 + 40.0 * idx as f32, 450.0]);
        self.project.figures[idx].clip = Some(ci);
        self.selected_figure = idx;
        self.status = format!("Inserted movieclip {}.", clip.name);
    }

    // ---- Wave 7: movieclip editing session ----
    //
    // A session scopes the ops below to one ClipDef's own frames
    // instead of the project timeline: they mutate
    // `project.clips[session.clip]` only, never the project frames.
    // Structural ops (add/delete frame, creating a clip) snapshot
    // for undo; pose tweaks (angle/root/goto) do not — the same
    // rule as dragging, where the gesture snapshots, not each move.

    /// Enter the editing session for clip `clip_idx`, starting at
    /// its first frame. Returns false (with a status message) when
    /// the index is invalid or the clip has no frames to edit.
    pub fn enter_clip(&mut self, clip_idx: usize) -> bool {
        let Some(clip) = self.project.clips.get(clip_idx) else {
            self.status = format!("No movieclip {} to edit.", clip_idx + 1);
            return false;
        };
        if clip.frames.is_empty() {
            self.status = format!("Movieclip {} has no frames to edit.", clip.name);
            return false;
        }
        let name = clip.name.clone();
        let count = clip.frames.len();
        self.clip_edit = Some(ClipEditSession {
            clip: clip_idx,
            frame: 0,
            name: name.clone(),
        });
        self.status = format!("Editing movieclip {name} ({count} frames).");
        true
    }

    /// Leave the editing session. The clip keeps its edits.
    pub fn exit_clip(&mut self) {
        if self.clip_edit.take().is_some() {
            self.status = "Movieclip editing closed.".into();
        } else {
            self.status = "Not editing a movieclip.".into();
        }
    }

    /// Validate the live session: returns (clip index, frame index)
    /// with the frame clamped into range. A session whose clip has
    /// vanished (e.g. after an undo) is dropped with a status
    /// message. Callers first check `clip_edit.is_none()` to report
    /// the no-session case themselves.
    fn clip_session(&mut self) -> Option<(usize, usize)> {
        let session = self.clip_edit.clone()?;
        let Some(clip) = self.project.clips.get(session.clip) else {
            self.clip_edit = None;
            self.status = "Movieclip is gone — editing session closed.".into();
            return None;
        };
        if clip.frames.is_empty() {
            self.status = "Movieclip has no frames to edit.".into();
            return None;
        }
        let frame = session.frame.min(clip.frames.len() - 1);
        if frame != session.frame {
            if let Some(s) = self.clip_edit.as_mut() {
                s.frame = frame;
            }
        }
        Some((session.clip, frame))
    }

    /// Keep clip frame `fi` of clip `ci` node-count wide: angle
    /// vecs are truncated or padded with rest angles.
    fn clip_fit_frame(&mut self, ci: usize, fi: usize) {
        let clip = &mut self.project.clips[ci];
        let n = clip.def.nodes.len();
        let cur = clip.frames[fi].angles.len();
        if cur < n {
            let extra: Vec<f32> = clip.def.nodes[cur..]
                .iter()
                .map(|node| node.rest_angle)
                .collect();
            clip.frames[fi].angles.extend(extra);
        }
        clip.frames[fi].angles.truncate(n);
    }

    /// Append a frame to the edited clip, duplicating its last
    /// pose, and move the session onto the new frame.
    pub fn clip_add_frame(&mut self) {
        if self.clip_edit.is_none() {
            self.status = "Not editing a movieclip.".into();
            return;
        }
        let Some((ci, _)) = self.clip_session() else { return };
        self.snapshot_labeled("Add clip frame");
        let pose = self.project.clips[ci]
            .frames
            .last()
            .expect("clip_session guarantees frames")
            .clone();
        self.project.clips[ci].frames.push(pose);
        let idx = self.project.clips[ci].frames.len() - 1;
        self.clip_fit_frame(ci, idx);
        if let Some(s) = self.clip_edit.as_mut() {
            s.frame = idx;
        }
        self.status = format!("Movieclip frame {} added.", idx + 1);
    }

    /// Delete the session's current clip frame. Refused when it is
    /// the clip's only frame.
    pub fn clip_delete_frame(&mut self) {
        if self.clip_edit.is_none() {
            self.status = "Not editing a movieclip.".into();
            return;
        }
        let Some((ci, fi)) = self.clip_session() else { return };
        if self.project.clips[ci].frames.len() <= 1 {
            self.status = "A movieclip needs at least one frame.".into();
            return;
        }
        self.snapshot_labeled("Delete clip frame");
        self.project.clips[ci].frames.remove(fi);
        let last = self.project.clips[ci].frames.len() - 1;
        if let Some(s) = self.clip_edit.as_mut() {
            s.frame = s.frame.min(last);
        }
        self.status = format!("Movieclip frame {} deleted.", fi + 1);
    }

    /// Move the session to clip frame `idx` (clamped to the
    /// clip's last frame).
    pub fn clip_goto(&mut self, idx: usize) {
        if self.clip_edit.is_none() {
            self.status = "Not editing a movieclip.".into();
            return;
        }
        let Some((ci, _)) = self.clip_session() else { return };
        let last = self.project.clips[ci].frames.len() - 1;
        let target = idx.min(last);
        if let Some(s) = self.clip_edit.as_mut() {
            s.frame = target;
        }
        self.status = format!("Movieclip frame {}.", target + 1);
    }

    /// Set joint `node`'s angle on the session frame's pose.
    /// Out-of-range joints are refused with a status message.
    pub fn clip_set_angle(&mut self, node: usize, degrees: f32) {
        if self.clip_edit.is_none() {
            self.status = "Not editing a movieclip.".into();
            return;
        }
        let Some((ci, fi)) = self.clip_session() else { return };
        if node >= self.project.clips[ci].def.nodes.len() {
            self.status = format!("Movieclip has no joint {}.", node + 1);
            return;
        }
        self.clip_fit_frame(ci, fi);
        // Roadmap Wave 1 (RM24, P-14 — A15 H3): the Wave 13
        // census A19 exemption is closed — a real change
        // takes a coalesced undo step, so a clip-pose drag
        // costs one step, not one per move (and not zero,
        // as the old pose-tweak rule had it). The fit above
        // is an invariant repair and rides the same step.
        if self.project.clips[ci].frames[fi].angles[node] != degrees {
            self.snapshot_coalesced("Movieclip frame");
            self.project.clips[ci].frames[fi].angles[node] = degrees;
        }
    }

    /// Set the root position on the session frame's pose.
    pub fn clip_set_root(&mut self, root: V2) {
        if self.clip_edit.is_none() {
            self.status = "Not editing a movieclip.".into();
            return;
        }
        let Some((ci, fi)) = self.clip_session() else { return };
        // Roadmap Wave 1 (RM24, P-14 — A15 H3): as
        // `clip_set_angle` — the A20 exemption is closed.
        if self.project.clips[ci].frames[fi].root != root {
            self.snapshot_coalesced("Movieclip frame");
            self.project.clips[ci].frames[fi].root = root;
        }
    }

    /// Joint positions of the edited clip at the session frame —
    /// what a canvas view of the session would draw. Empty when
    /// no session is active.
    pub fn clip_positions(&self) -> Vec<V2> {
        let Some(session) = self.clip_edit.clone() else { return Vec::new() };
        let Some(clip) = self.project.clips.get(session.clip) else {
            return Vec::new();
        };
        let frame = session.frame.min(clip.frames.len().saturating_sub(1));
        let Some(pose) = clip.frames.get(frame) else {
            return Vec::new();
        };
        evaluate(&clip.def, pose)
    }

    /// Build a movieclip from figure `fi`: the clip takes the
    /// figure's definition and its pose from every project frame,
    /// is appended to `project.clips`, and the instance is pointed
    /// at it, so the figure loops the clip from now on. Because
    /// the clip frames ARE the timeline poses, the first loop pass
    /// is visually identical to the timeline it replaces (after
    /// that the loop repeats with the clip's length, independent
    /// of the timeline). Returns the new clip's index; this is
    /// the parameterised sibling of `make_clip_from_figure`.
    pub fn create_clip_from_figure(&mut self, fi: usize) -> Option<usize> {
        if fi >= self.project.figures.len() {
            self.status = format!("No figure {} to make a clip from.", fi + 1);
            return None;
        }
        if self.project.frames.is_empty() {
            self.status = "No frames to build a movieclip from.".into();
            return None;
        }
        self.snapshot_labeled("Create movieclip");
        let (name, def) = {
            let fig = &self.project.figures[fi];
            (format!("{} clip", fig.name), fig.def.clone())
        };
        // Poses for `fi` across the timeline; a frame missing the
        // pose (should not happen — the editor keeps pose vecs
        // figure-wide) falls back to the previous pose, or to the
        // rest pose at stage centre for the first frame.
        let mut fallback = def.rest_pose([self.project.stage[0] / 2.0, self.project.stage[1] / 2.0]);
        let mut frames: Vec<crate::model::Pose> = Vec::with_capacity(self.project.frames.len());
        for frame in &self.project.frames {
            let pose = frame.poses.get(fi).cloned().unwrap_or_else(|| fallback.clone());
            fallback = pose.clone();
            frames.push(pose);
        }
        self.project.clips.push(crate::model::ClipDef {
            name: name.clone(),
            def,
            frames,
            loop_mode: crate::model::ClipLoop::Loop,
        });
        let ci = self.project.clips.len() - 1;
        for f in 0..self.project.clips[ci].frames.len() {
            self.clip_fit_frame(ci, f);
        }
        self.project.figures[fi].clip = Some(ci);
        self.status = format!("{name} created; figure {} now loops it.", fi + 1);
        Some(ci)
    }

    /// Create a clip from figure `fi`'s poses over the
    /// half-open frame range `[start, end)` only — the ranged
    /// sibling of `create_clip_from_figure` (Wave X, P-1
    /// step 2), for building a walk cycle out of frames 5–12
    /// without baking the whole timeline. The pose fallback
    /// chain threads from frame 0, so a frame inside the
    /// range that lacks the figure's pose falls back to the
    /// pose just before it (the whole-timeline op's rule),
    /// not to the rest pose. The new clip is fitted and the
    /// figure linked to it, exactly like the whole-timeline
    /// op; it is named "{figure} clip {start+1}-{end}" so a
    /// ranged clip is distinguishable in the clip list from
    /// the whole-timeline "{figure} clip". One snapshot;
    /// `None` (with a status) when the figure is invalid, the
    /// project has no frames, or the range is empty or
    /// outside the timeline.
    pub fn create_clip_from_figure_range(
        &mut self,
        fi: usize,
        start: usize,
        end: usize,
    ) -> Option<usize> {
        if fi >= self.project.figures.len() {
            self.status = format!("No figure {} to make a clip from.", fi + 1);
            return None;
        }
        if self.project.frames.is_empty() {
            self.status = "No frames to build a movieclip from.".into();
            return None;
        }
        if start >= end || end > self.project.frames.len() {
            self.status = format!(
                "No frames in range {}-{} to build a movieclip from.",
                start + 1,
                end
            );
            return None;
        }
        self.snapshot_labeled("Create movieclip");
        let (name, def) = {
            let fig = &self.project.figures[fi];
            (
                format!("{} clip {}-{}", fig.name, start + 1, end),
                fig.def.clone(),
            )
        };
        let mut fallback =
            def.rest_pose([self.project.stage[0] / 2.0, self.project.stage[1] / 2.0]);
        let mut frames: Vec<crate::model::Pose> = Vec::with_capacity(end - start);
        for (i, frame) in self.project.frames.iter().enumerate() {
            let pose = frame.poses.get(fi).cloned().unwrap_or_else(|| fallback.clone());
            fallback = pose.clone();
            if i >= start && i < end {
                frames.push(pose);
            }
        }
        self.project.clips.push(crate::model::ClipDef {
            name: name.clone(),
            def,
            frames,
            loop_mode: crate::model::ClipLoop::Loop,
        });
        let ci = self.project.clips.len() - 1;
        for f in 0..self.project.clips[ci].frames.len() {
            self.clip_fit_frame(ci, f);
        }
        self.project.figures[fi].clip = Some(ci);
        self.status = format!("{name} created; figure {} now loops it.", fi + 1);
        Some(ci)
    }

    // ---- Wave X (P-1 step 2): clip linking + state authoring ----
    //
    // The ops below are the parameterised linking surface the
    // hard-coded paths above never exposed: assigning an
    // existing clip to an existing figure (guarded by
    // `clip_compatible`), unlinking, and authoring the
    // animation-state keys X1's resolution reads. Every
    // mutating op is exactly one snapshot; refusals snapshot
    // nothing, in the clip family's established pattern.

    /// May clip `ci` be assigned to figure `fi`? The
    /// index-level form of [`clip_compatible`]; false when
    /// either index is out of range.
    pub fn clip_compatible(&self, fi: usize, ci: usize) -> bool {
        let (Some(fig), Some(clip)) = (
            self.project.figures.get(fi),
            self.project.clips.get(ci),
        ) else {
            return false;
        };
        clip_compatible(fig, clip)
    }

    /// The refusal status for an assignment the rig guard
    /// turned away, in the copy-pose sentence's shape. Two
    /// cases: the rigs differ in size, or the clip's frames
    /// do not fit the clip's own rig.
    fn clip_guard_status(&self, fi: usize, ci: usize) -> String {
        let fig = &self.project.figures[fi];
        let clip = &self.project.clips[ci];
        if clip.def.nodes.len() != fig.def.nodes.len() {
            format!(
                "clip {} ({}) is built for {} segments but figure {} has {} — clips only play on figures with the same rig size.",
                ci + 1,
                clip.name,
                clip.def.nodes.len(),
                fi + 1,
                fig.def.nodes.len()
            )
        } else {
            format!(
                "clip {} ({}) has a frame that does not fit its own {}-segment rig — repair or re-make the clip first.",
                ci + 1,
                clip.name,
                clip.def.nodes.len()
            )
        }
    }

    /// Assign existing clip `ci` to existing figure `fi` —
    /// the link `make_clip_from_figure` and
    /// `insert_clip_figure` set implicitly, as a guarded op
    /// of its own. The retarget guard refuses a clip whose
    /// rig does not match the figure's (no snapshot, status
    /// explains). Assigning the clip already linked is a
    /// no-op (false, no status churn — the `rename_clip`
    /// precedent). One snapshot on success. Note the link
    /// governs only when no state key is active on the
    /// figure (see `FigureInstance::states`): before the
    /// first key, and whenever the figure has no keys, this
    /// is the clip that plays.
    pub fn assign_clip_to_figure(&mut self, fi: usize, ci: usize) -> bool {
        if fi >= self.project.figures.len() {
            self.status = format!("No figure {} to assign a clip to.", fi + 1);
            return false;
        }
        if ci >= self.project.clips.len() {
            self.status = "No such clip.".into();
            return false;
        }
        if self.project.figures[fi].clip == Some(ci) {
            return false;
        }
        if !self.clip_compatible(fi, ci) {
            self.status = format!("Assign clip failed: {}", self.clip_guard_status(fi, ci));
            return false;
        }
        self.snapshot_labeled("Assign clip");
        self.project.figures[fi].clip = Some(ci);
        let name = self.project.clips[ci].name.clone();
        self.status = format!("Figure {} now loops clip {} ({name}).", fi + 1, ci + 1);
        true
    }

    /// Unlink figure `fi` from its movieclip: the inverse
    /// `make_clip_from_figure` never had. The figure falls
    /// back to its timeline poses — unless it carries state
    /// keys, which are a separate authoring surface and are
    /// NOT cleared here (the status says they still apply).
    /// Unlinking an unlinked figure is a no-op. One snapshot
    /// on success.
    pub fn unlink_figure(&mut self, fi: usize) -> bool {
        let Some(fig) = self.project.figures.get(fi) else {
            self.status = format!("No figure {} to unlink.", fi + 1);
            return false;
        };
        let Some(ci) = fig.clip else {
            return false;
        };
        let has_states = !fig.states.is_empty();
        let name = self
            .project
            .clips
            .get(ci)
            .map(|c| c.name.clone())
            .unwrap_or_else(|| format!("clip {}", ci + 1));
        self.snapshot_labeled("Unlink figure");
        self.project.figures[fi].clip = None;
        self.status = if has_states {
            format!(
                "Figure {} unlinked from clip {} ({name}); its state keys still apply.",
                fi + 1,
                ci + 1
            )
        } else {
            format!(
                "Figure {} unlinked from clip {} ({name}); it uses its timeline poses again.",
                fi + 1,
                ci + 1
            )
        };
        true
    }

    /// Author (or replace) figure `fi`'s animation-state key
    /// at `key.frame`: from that timeline frame the figure
    /// plays `key.clip`, starting `key.offset` frames in, at
    /// `key.speed`× (resolution: X1's `active_state_key` /
    /// `state_local_time`). A key already sitting at that
    /// frame is replaced, and the vec is kept sorted by
    /// frame. The clip must pass the same rig guard as
    /// assignment — a state key is just a scheduled
    /// assignment — and a non-finite speed is refused at
    /// authoring (playback sanitises corrupt speeds at read;
    /// this op does not write one). One snapshot on success.
    pub fn set_state_key(&mut self, fi: usize, key: crate::model::StateKey) -> bool {
        if fi >= self.project.figures.len() {
            self.status = format!("No figure {} to set a state key on.", fi + 1);
            return false;
        }
        if key.clip >= self.project.clips.len() {
            self.status = "No such clip.".into();
            return false;
        }
        if !key.speed.is_finite() {
            self.status = "Set state key failed: speed must be finite.".into();
            return false;
        }
        if !self.clip_compatible(fi, key.clip) {
            self.status = format!(
                "Set state key failed: {}",
                self.clip_guard_status(fi, key.clip)
            );
            return false;
        }
        self.snapshot_labeled("Set state key");
        let fig = &mut self.project.figures[fi];
        fig.states.retain(|k| k.frame != key.frame);
        fig.states.push(key);
        fig.states.sort_by_key(|k| k.frame);
        let name = self.project.clips[key.clip].name.clone();
        self.status = format!(
            "Figure {} state at frame {}: clip {} ({name}).",
            fi + 1,
            key.frame + 1,
            key.clip + 1
        );
        true
    }

    /// Remove figure `fi`'s state key at timeline frame
    /// `frame` (0-based, the stored key frame). Refuses —
    /// no snapshot — when the figure has no key there. One
    /// snapshot on success.
    pub fn remove_state_key(&mut self, fi: usize, frame: usize) -> bool {
        let Some(fig) = self.project.figures.get(fi) else {
            self.status = format!("No figure {} to remove a state key from.", fi + 1);
            return false;
        };
        if !fig.states.iter().any(|k| k.frame == frame) {
            self.status = format!("Figure {} has no state key at frame {}.", fi + 1, frame + 1);
            return false;
        }
        self.snapshot_labeled("Remove state key");
        self.project.figures[fi].states.retain(|k| k.frame != frame);
        self.status = format!("Figure {} state key at frame {} removed.", fi + 1, frame + 1);
        true
    }

    /// Wave Y (Y7, P-10): set figure `fi`'s rest (setup)
    /// pose from the pose the stage currently shows for
    /// it — the timeline pose at the current frame (clips
    /// and state keys applied), or the rest pose itself
    /// when the timeline carries no pose for the figure.
    /// With `rebase`, every authored angle store shifts
    /// by the rest delta so the animation keeps its
    /// content against the new rig; without it, only the
    /// rest angles change (absolute). One snapshot on a
    /// real change; refusals (unknown figure, a malformed
    /// timeline pose) and the already-rest no-op take
    /// none. The math is [`Project::set_rest_from_pose`].
    pub fn set_rest_from_pose(&mut self, fi: usize, rebase: bool) -> bool {
        if fi >= self.project.figures.len() {
            self.status = format!("No figure {} to set the rest pose of.", fi + 1);
            return false;
        }
        let poses = self.project.poses_at(self.current as f32);
        let pose = match poses.get(fi) {
            Some(p) => p.clone(),
            None => self.project.figures[fi].def.rest_pose([0.0, 0.0]),
        };
        let n = self.project.figures[fi].def.nodes.len();
        if pose.angles.len() != n || !pose.angles.iter().all(|a| a.is_finite()) {
            self.status = "Set rest pose failed: the current pose is malformed.".into();
            return false;
        }
        let rest = self.project.figures[fi].def.rest_angles();
        let already_rest = pose
            .angles
            .iter()
            .zip(rest.iter())
            .all(|(a, b)| (norm_deg(*a) - b).abs() < 1e-4);
        if already_rest {
            return false;
        }
        self.snapshot_labeled("Set rest pose");
        let mode = if rebase {
            SetRestMode::Rebase
        } else {
            SetRestMode::Absolute
        };
        let _ = self.project.set_rest_from_pose(fi, &pose, mode);
        self.status = if rebase {
            format!(
                "Figure {} rest pose set from frame {}; the animation was rebased onto the new rest.",
                fi + 1,
                self.current + 1
            )
        } else {
            format!(
                "Figure {} rest pose set from frame {} (absolute — the timeline is unchanged).",
                fi + 1,
                self.current + 1
            )
        };
        true
    }

    /// Wave 2 (W2-30, P-10): restore figure `fi`'s pose on
    /// timeline frame `frame` to its rest (setup) pose —
    /// the restore half of the setup-pose round trip whose
    /// capture half is [`EditorCore::set_rest_from_pose`].
    /// The root position is kept; the written pose is
    /// `FigureDef::rest_pose(current root)` exactly — the
    /// stored rest angles verbatim and the squash back at
    /// (1, 1) — so a capture → pose-freely → restore trip
    /// returns the frame pose to the setup pose
    /// field-for-field, never an approximation. One
    /// labelled snapshot ("Reset pose") on a real change;
    /// refusals (unknown figure, no pose on the frame)
    /// and the already-rest no-op take none and the no-op
    /// leaves the status line untouched — the capture
    /// op's discipline, so the round trip's two halves
    /// spend undo steps identically. The pose panel's
    /// Reset pose, the figure inspector's Setup-section
    /// reset row and the MCP `reset_pose` tool all ride
    /// this op, so the surfaces cannot drift apart.
    pub fn reset_pose_to_rest(&mut self, fi: usize, frame: usize) -> bool {
        if fi >= self.project.figures.len() {
            self.status = "No such figure.".into();
            return false;
        }
        let rest = {
            let Some(cur) = self
                .project
                .frames
                .get(frame)
                .and_then(|f| f.poses.get(fi))
            else {
                self.status = format!(
                    "Figure {} has no pose on frame {}.",
                    fi + 1,
                    frame + 1
                );
                return false;
            };
            let rest = self.project.figures[fi].def.rest_pose(cur.root);
            if cur.angles == rest.angles && cur.sx == rest.sx && cur.sy == rest.sy {
                return false;
            }
            rest
        };
        self.snapshot_labeled("Reset pose");
        self.project.frames[frame].poses[fi] = rest;
        self.status = format!(
            "Figure {} reset to its rest pose on frame {}.",
            fi + 1,
            frame + 1
        );
        true
    }

    /// Wave 10: cycle a clip's loop mode — Loop → PingPong →
    /// Once → Loop — as one undo step. `model::poses_at` honours
    /// the mode when a figure plays the clip back.
    pub fn cycle_clip_loop(&mut self, index: usize) {
        if index >= self.project.clips.len() {
            self.status = "No such clip.".into();
            return;
        }
        self.snapshot_labeled("Clip loop mode");
        let clip = &mut self.project.clips[index];
        clip.loop_mode = match clip.loop_mode {
            crate::model::ClipLoop::Loop => crate::model::ClipLoop::PingPong,
            crate::model::ClipLoop::PingPong => crate::model::ClipLoop::Once,
            crate::model::ClipLoop::Once => crate::model::ClipLoop::Loop,
        };
        self.status = format!(
            "Clip loop: {}.",
            match clip.loop_mode {
                crate::model::ClipLoop::Loop => "loop",
                crate::model::ClipLoop::PingPong => "ping-pong",
                crate::model::ClipLoop::Once => "once",
            }
        );
    }

    /// Wave 11 (C6): set a clip's loop mode directly — one
    /// undo step, the direct setter `cycle_clip_loop` never
    /// had (the panel reaches a target in up to two cycles
    /// via `apply_loop_mode`). Setting the current mode is a
    /// no-op in the `apply_loop_mode` discipline: no snapshot,
    /// no status churn.
    pub fn set_clip_loop_mode(&mut self, index: usize, mode: crate::model::ClipLoop) {
        let Some(clip) = self.project.clips.get(index) else {
            self.status = "No such clip.".into();
            return;
        };
        if clip.loop_mode == mode {
            return;
        }
        self.snapshot_labeled("Clip loop mode");
        self.project.clips[index].loop_mode = mode;
        self.status = format!(
            "Clip loop: {}.",
            match mode {
                crate::model::ClipLoop::Loop => "loop",
                crate::model::ClipLoop::PingPong => "ping-pong",
                crate::model::ClipLoop::Once => "once",
            }
        );
    }

    /// Wave 11 (C2 GAP-3): hold the camera over frames
    /// `from..=to` — the camera panel's Hold, lifted from
    /// clips of panel code into the core so the panel and MCP
    /// run the identical write: the camera in effect at the
    /// current frame (position/zoom via `camera_here`, the
    /// angle wrapped via `norm_deg`) is stamped onto every
    /// frame of the span under ONE snapshot.
    pub fn hold_camera(&mut self, from: usize, to: usize) {
        if from > to || to >= self.project.frames.len() {
            self.status = "No frames to hold the camera over.".into();
            return;
        }
        let (x, y, zoom) = self.camera_here();
        // The held angle is stamped wrapped, like a paste:
        // `camera_angle_at` can return an unwrapped stored
        // angle at the end keys (see `angle_deg`), and the
        // hold writes it onto every frame of the span.
        let angle = crate::model::norm_deg(self.project.camera_angle_at(self.current as f32));
        self.snapshot_labeled("Hold camera");
        for frame in from..=to {
            self.project.set_camera_key(frame, x, y, zoom);
            if let Some(k) = self.project.camera.iter_mut().find(|k| k.frame == frame) {
                k.angle = angle;
            }
        }
        self.status = format!("Camera held over frames {}–{}.", from + 1, to + 1);
    }

    // ---- Movieclip list ops (Wave 11 port of the clips
    // panel's field-write dispatches, C6 §5: the panel fns in
    // clips_panel.rs now delegate here so the panel and MCP
    // run one implementation. Discipline is the panel's own:
    // an out-of-range index only sets the status ("No such
    // clip.", no snapshot); a real change is exactly one
    // snapshot, the write, and the panel's status line.)

    /// Duplicate clip `clip_idx`: the clone is APPENDED
    /// (named `"<name> copy"`), so no stored clip index
    /// shifts. Returns the copy's index.
    pub fn duplicate_clip(&mut self, clip_idx: usize) -> Option<usize> {
        let Some(clip) = self.project.clips.get(clip_idx) else {
            self.status = "No such clip.".into();
            return None;
        };
        let mut copy = clip.clone();
        copy.name = format!("{} copy", copy.name);
        self.snapshot_labeled("Duplicate clip");
        self.project.clips.push(copy);
        let new_index = self.project.clips.len() - 1;
        self.status = format!(
            "Duplicated clip {} as clip {}.",
            clip_idx + 1,
            new_index + 1
        );
        Some(new_index)
    }

    /// Rename clip `clip_idx` to the resolved field text
    /// (trimmed; the current name when nothing usable was
    /// typed). A rename resolving to the current name is a
    /// no-op — no snapshot, no status churn. A live session's
    /// identity follows its clip. Returns true on a change.
    pub fn rename_clip(&mut self, clip_idx: usize, field_text: &str) -> bool {
        let Some(current) = self.project.clips.get(clip_idx).map(|c| c.name.clone()) else {
            self.status = "No such clip.".into();
            return false;
        };
        let trimmed = field_text.trim();
        let name = if trimmed.is_empty() {
            current.clone()
        } else {
            trimmed.to_string()
        };
        if name == current {
            return false;
        }
        self.snapshot_labeled("Rename clip");
        self.project.clips[clip_idx].name = name.clone();
        if let Some(session) = self.clip_edit.as_mut() {
            if session.clip == clip_idx {
                session.name = name.clone();
            }
        }
        self.status = format!("Clip {} renamed to {name}.", clip_idx + 1);
        true
    }

    /// Delete clip `clip_idx`: removal shifts later indices,
    /// so figure `clip` references and the clip-edit session
    /// are remapped through the delete mapping in the same
    /// write (a figure whose clip is deleted falls back to
    /// its timeline poses; a session whose clip is deleted
    /// ends). Returns true when a clip was removed.
    pub fn delete_clip(&mut self, clip_idx: usize) -> bool {
        let Some(name) = self.project.clips.get(clip_idx).map(|c| c.name.clone()) else {
            self.status = "No such clip.".into();
            return false;
        };
        self.snapshot_labeled("Delete clip");
        self.project.clips.remove(clip_idx);
        for fig in &mut self.project.figures {
            fig.clip = clip_target_after_delete(fig.clip, clip_idx);
            // Wave X (X2): state keys track their clips through
            // the same mapping — a key left pointing past the
            // gap would name the wrong clip, not a dangling one.
            state_keys_after_delete(&mut fig.states, clip_idx);
        }
        if let Some(session) = self.clip_edit.clone() {
            self.clip_edit = clip_target_after_delete(Some(session.clip), clip_idx)
                .map(|clip| ClipEditSession { clip, ..session });
        }
        self.status = format!("Clip {} ({name}) deleted.", clip_idx + 1);
        true
    }

    /// Move clip `clip_idx` one slot up or down; stored clip
    /// indices (figures, the session) follow through the swap
    /// mapping. Returns true when the swap happened.
    pub fn move_clip(&mut self, clip_idx: usize, up: bool) -> bool {
        if clip_idx >= self.project.clips.len() {
            self.status = "No such clip.".into();
            return false;
        }
        let other = if up {
            if clip_idx == 0 {
                self.status = format!("Clip {} is already first.", clip_idx + 1);
                return false;
            }
            clip_idx - 1
        } else if clip_idx + 1 >= self.project.clips.len() {
            self.status = format!("Clip {} is already last.", clip_idx + 1);
            return false;
        } else {
            clip_idx + 1
        };
        self.snapshot_labeled("Reorder clip");
        self.project.clips.swap(clip_idx, other);
        for fig in &mut self.project.figures {
            fig.clip = clip_target_after_swap(fig.clip, clip_idx, other);
            // Wave X (X2): state keys follow the swap too.
            state_keys_after_swap(&mut fig.states, clip_idx, other);
        }
        if let Some(session) = self.clip_edit.clone() {
            if let Some(clip) = clip_target_after_swap(Some(session.clip), clip_idx, other) {
                self.clip_edit = Some(ClipEditSession { clip, ..session });
            }
        }
        let name = self.project.clips[other].name.clone();
        self.status = format!(
            "Clip {} ({name}) moved {}.",
            other + 1,
            if up { "up" } else { "down" }
        );
        true
    }

    /// Insert a figure playing clip `clip_idx` — the
    /// parameterised form of `insert_clip_figure` (which
    /// always inserts the FIRST clip): a figure built from
    /// the clip's def and name, coloured
    /// `FIGURE_COLORS[(idx + 3) % 6]` where `idx` is the
    /// pre-insert figure count, pointed at the clip and
    /// selected. Returns true when a figure was inserted.
    pub fn insert_clip_figure_at(&mut self, clip_idx: usize) -> bool {
        let Some(clip) = self.project.clips.get(clip_idx).cloned() else {
            self.status = "No such clip.".into();
            return false;
        };
        self.snapshot_labeled("Insert clip figure");
        let idx = self.project.figures.len();
        let color = FIGURE_COLORS[(idx + 3) % FIGURE_COLORS.len()];
        self.project.add_figure_with(
            clip.def.clone(),
            &clip.name,
            color,
            [380.0 + 40.0 * idx as f32, 450.0],
        );
        self.project.figures[idx].clip = Some(clip_idx);
        self.selected_figure = idx;
        self.status = format!("Inserted movieclip {}.", clip.name);
        true
    }

    // ---- Named projects ----

    /// Save the project under its (slugified) name in
    /// `./projects/`. Returns true iff the file was actually
    /// written — callers that sync side state (recovery
    /// snapshot, recents) gate on it, so a failed save no
    /// longer plants a phantom recent. The status line
    /// reports the outcome either way, as it always has.
    pub fn save_named_project(&mut self) -> bool {
        let dir = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("projects");
        self.save_named_project_to(&dir)
    }

    /// Wave 10 (D10 I1): restart the dirty baseline after a
    /// project swap. The edit counters are lifetime counters
    /// with `snapshot()` as their only writer, so without
    /// this a freshly opened or created project inherits the
    /// previous project's unsaved-edit count — the dirty dot
    /// stays lit and discard confirms arm over a session
    /// with nothing unsaved. Every swap path calls this:
    /// `open_project_file` below, and the shell's raw swaps
    /// (project tools' New / Recover, Home's New / Recover).
    pub fn note_project_swapped(&mut self) {
        self.edit_count = 0;
        self.last_autosave_count = 0;
        // Roadmap Wave 1 (RM24, P-14): a swapped-in
        // project starts as "never saved" — the file-open
        // funnels re-stamp the baseline right after this
        // (see `note_save_baseline`); New / Recover /
        // import swaps never do, which is their truth.
        self.last_save_count = None;
        // A coalesced stepper run can never merge across
        // a swap either (the stacks are cleared by the
        // callers, but the licence dies here regardless).
        self.history_coalesce = false;
        // Roadmap Wave 1 (RM6, F-5): the revision deliberately
        // does NOT restart with the autosave counters — it
        // moves forward by one, so no cache entry cut against
        // the previous project can ever be served for this one.
        self.revision += 1;
        // Wave 11 (E9): record the swap for the shell's poll.
        self.project_swaps += 1;
        // Wave 11 (C6): a clip-edit session must settle against
        // the NEW project exactly as it does after undo/redo —
        // every swap path funnels through here, so the session
        // can never keep editing a same-index different clip.
        self.repair_clip_session_after_history();
        // Wave P (P14): the missing-audio report settles
        // against the new project here too, so every swap
        // path (opens, New, Recover) reports the project it
        // actually landed on.
        self.refresh_missing_audio();
    }

    /// Wave 11 (E9): how many wholesale project swaps this core
    /// has seen; the shell snapshots it around the live-MCP
    /// drain and clears stale panel selections when it moves.
    pub fn project_swaps(&self) -> usize {
        self.project_swaps
    }

    /// Wave 11 (E9): record a wholesale project replacement
    /// that is NOT a baseline restart (the MCP io Nodes/.stk
    /// import path swaps the project snapshot-first, so undo
    /// survives — but panel selections must still settle).
    pub fn note_project_replaced(&mut self) {
        self.project_swaps += 1;
        // Roadmap Wave 1 (RM24, P-14): replaced content
        // arrived from a tool payload, not a named file —
        // the save baseline resets to "never saved", and
        // no coalesced run folds across the replacement.
        self.last_save_count = None;
        self.history_coalesce = false;
        // Roadmap Wave 1 (RM6, F-5): a snapshot-first replace
        // is still new project content under the session.
        self.revision += 1;
        self.repair_clip_session_after_history();
        // Wave P (P14): a snapshot-first replace is still a
        // new project under the session — refresh the
        // missing-audio report with the same funnel rule.
        self.refresh_missing_audio();
    }

    /// Wave P (P14): recompute the missing-audio report
    /// against the project now in the core. Called wherever
    /// the project is wholesale replaced — the swap funnels
    /// (`note_project_swapped`, `note_project_replaced`)
    /// and undo/redo — so the report can neither name clips
    /// of a project the session has already moved away from
    /// nor miss the clips of the one just landed.
    fn refresh_missing_audio(&mut self) {
        self.missing_audio = crate::audio::missing_audio_clips(&self.project.audio);
    }

    /// Wave P (P14): the current project's audio clips
    /// whose files were missing at the last refresh (see
    /// the field note on `missing_audio`): their stored
    /// paths, one per clip in timeline order, classified by
    /// the tree's one missing-file query
    /// ([`crate::audio::missing_audio_clips`] — the same
    /// classification the MP4 export outcome reports, and
    /// the same unmuted-and-present test playback planning
    /// and the export mix apply). Empty when every unmuted
    /// clip's file is present.
    pub fn missing_audio_clips(&self) -> &[String] {
        &self.missing_audio
    }

    /// The status-line suffix a load appends when the
    /// missing-audio report is non-empty; the empty string
    /// when nothing is missing, so a clean load's status
    /// stays byte-identical to before this report existed.
    fn missing_audio_note(&self) -> String {
        match self.missing_audio.len() {
            0 => String::new(),
            1 => format!(
                " — 1 audio clip is missing its file: {}",
                self.missing_audio[0]
            ),
            n => format!(
                " — {n} audio clips are missing their files: {}",
                self.missing_audio.join(", ")
            ),
        }
    }

    /// Wave 11 (A5): drop both history stacks without touching
    /// the project — the `.piv` import contract (`import_piv`
    /// clears both), exposed so the MCP io import path can
    /// mirror the GUI exactly.
    pub fn clear_history(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }

    /// Wave 11 (E9): after an MCP-driven open or import, point
    /// the project-file selector at the loaded file — the
    /// insert-or-select rule the Home panel's open applies —
    /// so the selector, Home's "current project" marking and
    /// Reopen suppression name the project actually open
    /// (previously they kept naming the previous file).
    pub fn note_project_loaded(&mut self, path: &std::path::Path) {
        if self.project_files.is_empty() {
            self.project_files = Self::list_project_files();
        }
        if let Some(pos) = self.project_files.iter().position(|p| p == path) {
            self.project_index = pos;
        } else {
            self.project_files.push(path.to_path_buf());
            self.project_index = self.project_files.len() - 1;
        }
    }

    /// `save_named_project` against an explicit projects
    /// directory (the codebase's `_in`-style split, so tests
    /// can aim the write at a temp dir). Same slug, same
    /// status strings, same `project_files` refresh.
    fn save_named_project_to(&mut self, dir: &std::path::Path) -> bool {
        // Wave 10 (B7): refuse a name that cannot name a
        // file — blank or punctuation-only names slugged
        // to a hidden `.json` or a bare `---.json`. The
        // rule is the library's own (`projects::
        // file_stem_for`, the same one save-a-copy uses).
        let Some(slug) = crate::projects::file_stem_for(&self.project.name) else {
            self.status =
                "Project save failed: the project name cannot name a file — give it a name with at least one letter or digit."
                    .into();
            return false;
        };
        let path = dir.join(format!("{slug}.json"));
        // Wave 11 (A10): write atomically (temp sibling +
        // rename) like projects::save — a crash mid-write must
        // not tear the only copy of the project.
        let result = std::fs::create_dir_all(dir)
            .and_then(|_| serde_json::to_string_pretty(&self.project).map_err(std::io::Error::other))
            .and_then(|text| crate::projects::write_file_atomic(&path, &text));
        match result {
            Ok(()) => {
                self.project_files = Self::list_project_files();
                // Wave 11 (E9): the refresh replaces the list
                // wholesale; re-resolve the index onto the file
                // just saved, or the selector can silently land
                // on a different project after a slug change.
                if let Some(pos) = self.project_files.iter().position(|p| p == &path) {
                    self.project_index = pos;
                } else if self.project_index >= self.project_files.len() {
                    self.project_index = self.project_files.len().saturating_sub(1);
                }
                // Wave 10 (D10 I1): a successful save IS
                // the baseline — the session no longer
                // reads dirty for work that is on disk
                // (the shell refreshes the recovery
                // snapshot with this same state next).
                self.last_autosave_count = self.edit_count;
                // Roadmap Wave 1 (RM24, P-14): and it is
                // the NAMED-save baseline in its own right
                // — the save-state the strip now speaks
                // separately from the recovery story.
                self.note_save_baseline();
                self.status = format!("Project saved to {}", path.display());
                true
            }
            Err(e) => {
                self.status = format!("Project save failed: {e}");
                false
            }
        }
    }

    pub fn cycle_project_file(&mut self) {
        if self.project_files.is_empty() {
            self.project_files = Self::list_project_files();
        }
        if !self.project_files.is_empty() {
            self.project_index = (self.project_index + 1) % self.project_files.len();
            self.status = format!(
                "Project: {}",
                self.project_files[self.project_index].display()
            );
        } else {
            self.status = "No saved projects in ./projects/ yet.".into();
        }
    }

    pub fn open_project_file(&mut self) {
        let Some(path) = self.project_files.get(self.project_index).cloned() else {
            self.status = "No project selected.".into();
            return;
        };
        match std::fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|s| serde_json::from_str::<Project>(&s).map_err(|e| e.to_string()))
        {
            Ok(project) => {
                self.project = project;
                self.current = 0;
                self.playhead = 0.0;
                self.selected_figure = 0;
                self.selected_node = None;
                // Mirror the dialog-open op: a solo index
                // from the previous project must not hide
                // every figure in the newly opened one.
                self.solo = None;
                self.note_project_swapped();
                self.note_save_baseline();
                self.status =
                    format!("Opened {}{}", path.display(), self.missing_audio_note());
            }
            Err(e) => self.status = format!("Open failed: {e}"),
        }
    }

    pub fn cycle_onion_frames(&mut self) {
        self.onion_frames = self.onion_frames % 3 + 1;
        self.status = format!("Onion skin: {} previous frame(s).", self.onion_frames);
    }

    pub fn export_mp4(&mut self) {
        let path = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("export")
            .join("animation.mp4");
        match render_mp4(&self.project, &path) {
            Ok(outcome) => {
                // Wave P (P15): the export succeeded, but any
                // clips whose files are missing were left out
                // — the status names them instead of the mux
                // dropping them silently. With every clip
                // present the status is the sentence it
                // always was.
                self.status = match outcome.missing_audio_note() {
                    Some(note) => format!(
                        "Exported {}-frame MP4 to {}. {note}",
                        outcome.frames,
                        path.display()
                    ),
                    None => format!(
                        "Exported {}-frame MP4 to {}",
                        outcome.frames,
                        path.display()
                    ),
                }
            }
            Err(e) => self.status = format!("MP4 export failed: {e}"),
        }
    }

    pub fn export_webm(&mut self) {
        let path = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("export")
            .join("animation.webm");
        match render_webm(&self.project, &path) {
            Ok(n) => self.status = format!("Exported {n}-frame WebM."),
            Err(e) => self.status = format!("WebM export failed: {e}"),
        }
    }

    pub fn export_svg(&mut self) {
        let dir = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("export")
            .join("svg");
        match crate::svg::export_svg(&self.project, &dir) {
            Ok(n) => self.status = format!("Exported {n} SVG frames to {}", dir.display()),
            Err(e) => self.status = format!("SVG export failed: {e}"),
        }
    }

    pub fn export_sheet(&mut self) {
        let path = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("export")
            .join(format!("{}-spritesheet.png", self.export_stem()));
        match render_sheet(&self.project, &path) {
            Ok(n) => self.status = format!("Exported {n}-frame spritesheet."),
            Err(e) => self.status = format!("Spritesheet failed: {e}"),
        }
    }

    pub fn export_png(&mut self) {
        let dir = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("export");
        match export_png_sequence(&self.project, &dir) {
            Ok(n) => self.status = format!("Exported {n} PNG frames to {}", dir.display()),
            Err(e) => self.status = format!("Export failed: {e}"),
        }
    }

    // ---- 3D figures (fig3d session, manipulation, bake to 2D) ----

    /// Enter 3D mode: load the 3D humanoid at rest with the default
    /// orbit camera. If a 3D session is already active it is kept
    /// as-is (pose and camera preserved).
    pub fn enter_3d(&mut self) {
        if self.fig3.is_some() {
            self.status = "3D mode is already active.".into();
            return;
        }
        let figure = crate::fig3d::humanoid3();
        let pose = crate::fig3d::Pose3::rest(&figure, [0.0, 0.0, 0.0]);
        self.fig3 = Some(crate::fig3d::Fig3State {
            figure,
            pose,
            camera: crate::fig3d::default_camera(),
            selected: 0,
            pose_b: None,
        });
        self.status = "3D mode: posing the 3D humanoid. Bake turns it into a 2D figure.".into();
    }

    /// Leave 3D mode, discarding the 3D session. Figures already
    /// baked into the project stay.
    pub fn exit_3d(&mut self) {
        if self.fig3.take().is_some() {
            self.status = "Exited 3D mode.".into();
        } else {
            self.status = "Not in 3D mode.".into();
        }
    }

    /// Cycle the selected 3D joint to the next node.
    pub fn fig3_select_next(&mut self) {
        let msg = match &mut self.fig3 {
            Some(st) if !st.figure.nodes.is_empty() => {
                st.selected = (st.selected + 1) % st.figure.nodes.len();
                format!("3D joint: {}.", st.figure.nodes[st.selected].name)
            }
            _ => "Enter 3D mode first.".into(),
        };
        self.status = msg;
    }

    /// Rotate the selected 3D joint by `delta_deg` around one Euler
    /// axis (0 = X, 1 = Y, 2 = Z; order documented in fig3d).
    pub fn fig3_rotate(&mut self, axis: usize, delta_deg: f32) {
        let msg = match &mut self.fig3 {
            Some(st) if axis < 3 && !st.figure.nodes.is_empty() => {
                let sel = st.selected.min(st.figure.nodes.len() - 1);
                while st.pose.rotations.len() < st.figure.nodes.len() {
                    st.pose.rotations.push([0.0; 3]);
                }
                st.pose.rotations[sel][axis] += delta_deg;
                format!(
                    "3D {} rotated to {:.0} deg on axis {}.",
                    st.figure.nodes[sel].name,
                    st.pose.rotations[sel][axis],
                    "XYZ".chars().nth(axis).unwrap()
                )
            }
            Some(_) => "Rotation axis must be 0 (X), 1 (Y) or 2 (Z).".into(),
            None => "Enter 3D mode first.".into(),
        };
        self.status = msg;
    }

    /// Orbit the 3D camera by the given yaw/pitch deltas in degrees.
    /// Pitch clamps to +-89.9, the same clamp the projector applies,
    /// so the stored camera never degenerates.
    pub fn fig3_orbit(&mut self, dyaw: f32, dpitch: f32) {
        let msg = match &mut self.fig3 {
            Some(st) => {
                st.camera.yaw += dyaw;
                st.camera.pitch = (st.camera.pitch + dpitch).clamp(-89.9, 89.9);
                format!(
                    "3D camera: yaw {:.0}, pitch {:.0}.",
                    crate::model::norm_deg(st.camera.yaw),
                    st.camera.pitch
                )
            }
            None => "Enter 3D mode first.".into(),
        };
        self.status = msg;
    }

    /// Relative 2D angle of every node in one frame of 3D bake
    /// nodes: each node's baked absolute direction minus its
    /// parent's (0 for the root). Shared by the figure-definition
    /// build and the per-frame pose build so the two can never
    /// drift apart.
    fn fig3_relative_angles(
        fig: &crate::fig3d::Figure3,
        baked: &[crate::fig3d::BakeNode],
    ) -> Vec<f32> {
        baked
            .iter()
            .enumerate()
            .map(|(i, bn)| {
                let parent_abs = fig.nodes[i]
                    .parent
                    .map(|p| baked[p].abs_angle)
                    .unwrap_or(0.0);
                crate::model::norm_deg(bn.abs_angle - parent_abs)
            })
            .collect()
    }

    /// Build the 2D figure definition for one frame of 3D bake
    /// nodes (origin node + one node per 3D joint, parent structure
    /// preserved, thickness scaled by projected scale — the same
    /// conversion the file importers use).
    fn fig3_bake_def(
        fig: &crate::fig3d::Figure3,
        baked: &[crate::fig3d::BakeNode],
    ) -> FigureDef {
        let rel = Self::fig3_relative_angles(fig, baked);
        let nodes = fig
            .nodes
            .iter()
            .zip(baked.iter().zip(rel.iter()))
            .map(|(n3, (bn, &rest_angle))| NodeDef {
                min_angle: None,
                max_angle: None,
                name: n3.name.clone(),
                parent: n3.parent,
                length: bn.length,
                rest_angle,
                head: n3.head,
                bend: 1.0,
                thickness: bn.thickness,
                color: None,
                ik_end: false,
                shape: crate::model::Shape::Line,
                style: crate::model::NodeStyle::default(),
                media: None,
            })
            .collect();
        FigureDef { nodes }
    }

    /// The 2D pose one frame of 3D bake nodes represents in a
    /// figure whose definition came from [`Self::fig3_bake_def`]:
    /// root at the baked root position, relative angles exactly as
    /// in the definition. Because the 2D evaluator accumulates
    /// relative angles along the tree, the evaluated joints
    /// reproduce the baked points whenever this frame's projected
    /// segment lengths equal the definition's — exactly true for
    /// the bake that built the definition, and whenever the motion
    /// stays in the projection plane.
    fn fig3_bake_pose(
        fig: &crate::fig3d::Figure3,
        baked: &[crate::fig3d::BakeNode],
    ) -> crate::model::Pose {
        crate::model::Pose {
            root: baked.first().map(|b| b.xy).unwrap_or([0.0, 0.0]),
            angles: Self::fig3_relative_angles(fig, baked),
            sx: 1.0,
            sy: 1.0,
        }
    }

    /// Bake the current 3D pose into a new 2D figure in the project.
    /// Projected segment lengths and directions become the figure
    /// definition (see [`Self::fig3_bake_def`]). The pose is
    /// stored as the definition's rest pose, so the held-pose
    /// convention of `add_figure_with` puts it on every frame.
    pub fn fig3_bake(&mut self) {
        let built = self.fig3.as_ref().map(|st| {
            let baked = crate::fig3d::bake_nodes(
                &st.figure,
                &st.pose,
                &st.camera,
                self.project.stage,
            );
            let root = baked.first().map(|b| b.xy).unwrap_or([0.0, 0.0]);
            (
                Self::fig3_bake_def(&st.figure, &baked),
                format!("{} (3D)", st.figure.name),
                st.figure.color,
                root,
            )
        });
        let Some((def, name, color, root)) = built else {
            self.status = "Enter 3D mode first.".into();
            return;
        };
        self.snapshot_labeled("Bake 3D pose");
        let idx = self.project.figures.len();
        self.project.add_figure_with(def, &name, color, root);
        self.selected_figure = idx;
        self.selected_node = None;
        self.status = format!(
            "Baked 3D pose into 2D figure {} ({name}), held on all frames.",
            idx + 1
        );
    }

    /// Mark the current 3D pose as "pose B": the target
    /// [`Self::fig3_bake_animation`] interpolates to from whatever
    /// pose is current when it runs. Pose the figure, mark B, pose
    /// it back, then bake the animation between the two.
    pub fn fig3_mark_pose_b(&mut self) {
        let msg = match &mut self.fig3 {
            Some(st) => {
                st.pose_b = Some(st.pose.clone());
                format!("3D pose B marked for {}.", st.figure.name)
            }
            None => "Enter 3D mode first.".into(),
        };
        self.status = msg;
    }

    /// The directory holding the 3D figure library (`./library3d`,
    /// alongside the 2D `./library`).
    fn fig3_library_dir() -> PathBuf {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("library3d")
    }

    /// Save the session's 3D figure, in its current pose, to the
    /// 3D library so it survives the session.
    pub fn fig3_save_to_library(&mut self) {
        let msg = match &self.fig3 {
            Some(st) => match crate::fig3d::save_library_figure(
                &Self::fig3_library_dir(),
                &st.figure.name,
                &st.figure,
                &st.pose,
            ) {
                Ok(path) => {
                    format!("Saved 3D figure {} to {}.", st.figure.name, path.display())
                }
                Err(e) => format!("Could not save 3D figure: {e}"),
            },
            None => "Enter 3D mode first.".into(),
        };
        self.status = msg;
    }

    /// Load a 3D library figure by name, replacing the current
    /// session (or starting one). The camera is kept when a
    /// session is already active; pose B is cleared, as it
    /// belonged to the previous figure.
    pub fn fig3_load_from_library(&mut self, name: &str) {
        // Resolve through the stored name, not the slug: the
        // listing (`fig3_library_list`) reads names out of
        // file contents, so a listed entry whose stem is not
        // its name's slug must still load (Wave W, W9).
        let path = crate::fig3d::library_entry_file(&Self::fig3_library_dir(), name);
        let msg = match crate::fig3d::load_library_figure(&path) {
            Some((figure, pose)) => {
                let camera = self
                    .fig3
                    .as_ref()
                    .map(|st| st.camera.clone())
                    .unwrap_or_else(crate::fig3d::default_camera);
                let loaded_name = figure.name.clone();
                self.fig3 = Some(crate::fig3d::Fig3State {
                    figure,
                    pose,
                    camera,
                    selected: 0,
                    pose_b: None,
                });
                format!("Loaded 3D figure {loaded_name} from the library.")
            }
            None => format!("No 3D library figure named '{name}'."),
        };
        self.status = msg;
    }

    /// The display names of every figure in the 3D library,
    /// sorted (empty when the library directory is missing or
    /// holds no readable entries).
    pub fn fig3_library_list(&self) -> Vec<String> {
        crate::fig3d::scan_library(&Self::fig3_library_dir())
    }

    /// Load the next 3D library figure, cycling through
    /// [`Self::fig3_library_list`] the way [`Self::cycle_library`]
    /// walks the 2D library: with a session active, the entry
    /// after the current figure's name (wrapping); otherwise the
    /// first entry. An empty library only sets a status message.
    pub fn fig3_cycle_library(&mut self) {
        let names = self.fig3_library_list();
        if names.is_empty() {
            self.status = "3D library is empty — save a 3D figure first.".into();
            return;
        }
        let current = self.fig3.as_ref().map(|st| st.figure.name.clone());
        let next = match current {
            Some(name) => match names.iter().position(|n| *n == name) {
                Some(i) => names[(i + 1) % names.len()].clone(),
                None => names[0].clone(),
            },
            None => names[0].clone(),
        };
        self.fig3_load_from_library(&next);
    }

    /// Bake an ANIMATION: interpolate the session's current 3D
    /// pose to the marked pose B over `frames` frames and write
    /// the result as a new 2D figure whose frame poses follow the
    /// interpolation. The figure's definition is built from the
    /// first frame's projection (like [`Self::fig3_bake`]); each
    /// following frame's pose is that frame's projection
    /// recomputed against the definition (see
    /// [`Self::fig3_bake_pose`]). If the project has fewer than
    /// `frames` frames it is extended with held frames first.
    /// One snapshot covers the whole bake. Refuses — with a status
    /// message and no changes — without a session, without a
    /// marked pose B, or for `frames < 2`.
    pub fn fig3_bake_animation(&mut self, frames: usize) {
        if frames < 2 {
            self.status = "Bake animation needs at least 2 frames.".into();
            return;
        }
        let built = self.fig3.as_ref().and_then(|st| {
            let pose_b = st.pose_b.as_ref()?;
            let range = crate::fig3d::bake_range_staged(
                &st.figure,
                &st.pose,
                pose_b,
                &st.camera,
                self.project.stage,
                frames,
            );
            let poses: Vec<crate::model::Pose> = range
                .iter()
                .map(|b| Self::fig3_bake_pose(&st.figure, b))
                .collect();
            let root = range[0].first().map(|b| b.xy).unwrap_or([0.0, 0.0]);
            Some((
                Self::fig3_bake_def(&st.figure, &range[0]),
                format!("{} (3D anim)", st.figure.name),
                st.figure.color,
                root,
                poses,
            ))
        });
        let Some((def, name, color, root, poses)) = built else {
            self.status = if self.fig3.is_none() {
                "Enter 3D mode first.".into()
            } else {
                "Mark a pose B first, then bake the animation.".into()
            };
            return;
        };
        self.snapshot_labeled("Bake 3D animation");
        let idx = self.project.figures.len();
        self.project.add_figure_with(def, &name, color, root);
        // Grow the timeline if the baked range is longer than the
        // project: appended frames hold the previous frame's poses
        // (the new figure included); the range overwrites the
        // first `frames` poses of the new figure below.
        while self.project.frames.len() < frames {
            let held = match self.project.frames.last() {
                Some(f) => f.poses.clone(),
                None => self
                    .project
                    .figures
                    .iter()
                    .map(|f| f.def.rest_pose([0.0, 0.0]))
                    .collect(),
            };
            self.project.frames.push(crate::model::Frame {
                poses: held,
                duration: 1.0,
                easing: None,
                bends: Vec::new(),
            });
        }
        for (f, pose) in poses.into_iter().enumerate() {
            // Wave 11 (E6): `add_figure_with` APPENDS the
            // new figure's held pose to each frame's pose
            // row, so on a frame whose row was short (a
            // figure absent there — pose rows may be
            // truncated) the appended slot is the row's
            // last entry, not `idx`, and indexing
            // `poses[idx]` panicked. Write the slot that
            // was actually appended: `idx` when the row is
            // figure-wide (the normal case, unchanged),
            // the last entry otherwise.
            let row = &mut self.project.frames[f].poses;
            let slot = if idx < row.len() {
                row.get_mut(idx)
            } else {
                row.last_mut()
            };
            if let Some(slot) = slot {
                *slot = pose;
            }
        }
        self.selected_figure = idx;
        self.selected_node = None;
        self.status = format!(
            "Baked 3D animation into 2D figure {} ({name}) over {frames} frames.",
            idx + 1
        );
    }
}

impl Default for EditorCore {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------
// Wave 14 (A9): MCP parity handlers for the editor-settings
// surface — scene weather, camera keys, onion skin, and the
// consolidated settings read.
//
// The audit behind this section (against the entering
// registry, `mcp_tools::tools()` + the `mcp.rs` built-ins):
//
// - Weather: `set_weather` (registry) sets kind + amount +
//   wind in one call, but it is NOT the panel's semantics —
//   an omitted amount/wind falls back to ITS defaults
//   (90/20; storm 140/30), so it cannot switch kinds while
//   keeping tuned values (the Scene panel's
//   `set_weather_kind`), cannot cycle kinds (the panel's
//   `cycle_weather`), and cannot set amount or wind alone
//   without also naming a kind and resending the other
//   value (the panel's steppers/sliders). Those three are
//   the first three handlers below.
// - Camera keys: `set_camera` (registry), `get_camera`,
//   `delete_camera_key`, `set_camera_key_here`,
//   `hold_camera`, `frame_all_figures`,
//   `centre_on_selection` and `set_camera_follow`
//   (built-ins) cover most of the panel, but the panel's
//   "Clear key (no undo)" button (`camera_clear_key` —
//   mark_dirty-only, deliberately NOT undoable), its zoom
//   buttons (`camera_zoom_by`) and its rotate buttons /
//   dial (`rotate_camera`) had no tool. Those are the
//   next three handlers.
// - Onion skin: NO tool touched `core.onion` at all. The
//   master toggle is an `EditorCore` field, so it is
//   handled here; the per-side depths are SHELL state
//   (`State::onion_ui` in the bin crate) and cannot be
//   reached from a library handler — they belong to
//   Wave 14 Squad B's shell control, and the legacy
//   `core.onion_frames` count is deliberately NOT
//   exposed: the stage no longer reads it, so a tool for
//   it would be a fake control.
// - Loop range: VERIFIED covered — `set_loop_range` /
//   `clear_loop_range` built-ins write it and
//   `get_editor_status` reads it. No handler owed.
// - Tween/easing defaults: VERIFIED covered —
//   `set_project_easing` (registry; the op marks dirty),
//   `set_frame_easing` and `set_tween_playback`
//   (built-ins). The cycle forms (`cycle_easing`,
//   `cycle_frame_easing`) are palette commands whose
//   capability the absolute setters cover (Squad A10's
//   map records them).
// - Background: VERIFIED covered —
//   `set_project_background` (registry, one undo step;
//   the panel's own swatch/stepper path marks dirty
//   instead — the Wave 13 census recorded the asymmetry)
//   and `get_state` reads it back.
//
// Discipline rule for every handler below: match the
// PANEL's exact discipline for the same write, and say
// which was chosen in the handler's own comment —
// mark_dirty-only where the panel only marks dirty,
// exactly one undo step where the backing op snapshots,
// neither where the state is session-only view state.
// All float arguments pass the Wave 11 finiteness rule
// (a finite f64 that casts to f32 infinity is refused).
//
// These are `pub fn tool_*` handlers in the exact
// registry shape for the integrator to register in
// `mcp_tools::tools()`; each carries its
// `// TOOL ENTRY SPEC:` (the registry entry to add) and
// its MCP.md catalogue row. Argument helpers are LOCAL
// to this section with `eds_`-prefixed names —
// `mcp_tools`' helpers are private to that module (the
// `aop_` / `ast_` precedent in audio_ops.rs / media.rs).
// ---------------------------------------------------------------------

use crate::vfx::{Weather, WeatherKind};
use serde_json::{json, Value};

/// Local arg accessor (the `mcp_tools::get` shape): the
/// named member of the arguments object, if present and
/// not null.
fn eds_get<'a>(args: &'a Value, key: &str) -> Option<&'a Value> {
    args.get(key).filter(|v| !v.is_null())
}

/// The Wave 11 finiteness rule (see the section header):
/// refuse a non-finite f64 and a finite f64 whose f32
/// cast overflows.
fn eds_finite_f32(f: f64, key: &str) -> Result<f32, String> {
    let v = f as f32;
    if f.is_finite() && v.is_finite() {
        Ok(v)
    } else {
        Err(format!("argument '{key}' must be a finite number"))
    }
}

fn eds_req_f32(args: &Value, key: &str) -> Result<f32, String> {
    match eds_get(args, key) {
        Some(v) => match v.as_f64() {
            Some(f) => eds_finite_f32(f, key),
            None => Err(format!("argument '{key}' must be a number")),
        },
        None => Err(format!("missing required argument '{key}'")),
    }
}

fn eds_req_str<'a>(args: &'a Value, key: &str) -> Result<&'a str, String> {
    match eds_get(args, key) {
        Some(v) => v
            .as_str()
            .ok_or_else(|| format!("argument '{key}' must be a string")),
        None => Err(format!("missing required argument '{key}'")),
    }
}

fn eds_req_bool(args: &Value, key: &str) -> Result<bool, String> {
    match eds_get(args, key) {
        Some(v) => v
            .as_bool()
            .ok_or_else(|| format!("argument '{key}' must be a boolean")),
        None => Err(format!("missing required argument '{key}'")),
    }
}

/// The weather kind's wire name (the `get_state` /
/// `set_weather` spelling).
fn eds_weather_kind_name(kind: WeatherKind) -> &'static str {
    match kind {
        WeatherKind::Rain => "rain",
        WeatherKind::Snow => "snow",
        WeatherKind::Fireflies => "fireflies",
        WeatherKind::Fog => "fog",
        WeatherKind::Storm => "storm",
    }
}

/// Parse a weather kind argument: the five kinds, or
/// `"off"` for no weather. The error wording is
/// `tool_set_weather`'s own.
fn eds_parse_weather_kind(name: &str) -> Result<Option<WeatherKind>, String> {
    match name {
        "off" => Ok(None),
        "rain" => Ok(Some(WeatherKind::Rain)),
        "snow" => Ok(Some(WeatherKind::Snow)),
        "fireflies" => Ok(Some(WeatherKind::Fireflies)),
        "fog" => Ok(Some(WeatherKind::Fog)),
        "storm" => Ok(Some(WeatherKind::Storm)),
        other => Err(format!(
            "unknown weather kind '{other}' (expected \"rain\", \"snow\", \"fireflies\", \"fog\", \"storm\" or \"off\")"
        )),
    }
}

/// The preset applied when a kind is enabled from off —
/// the Scene panel's `kind_preset` values, duplicated
/// here because the panel lives in the bin crate and a
/// library handler cannot call it. This table is the
/// ONE source for per-kind weather defaults: the MCP
/// `set_weather` tool in `mcp_tools.rs` also takes its
/// omitted amount/wind defaults from here (Focused
/// Wave, C1 — that tool used to carry its own table,
/// which had drifted: every non-storm kind defaulted
/// to wind 20 and fireflies/fog to amount 90).
pub(crate) fn eds_kind_preset(kind: WeatherKind) -> Weather {
    match kind {
        WeatherKind::Rain => Weather { kind, amount: 90.0, wind: 25.0 },
        WeatherKind::Snow => Weather { kind, amount: 90.0, wind: 12.0 },
        WeatherKind::Fireflies => Weather { kind, amount: 60.0, wind: 4.0 },
        WeatherKind::Fog => Weather { kind, amount: 60.0, wind: 0.0 },
        WeatherKind::Storm => Weather { kind, amount: 140.0, wind: 30.0 },
    }
}

/// The weather as this section's handlers report it:
/// `"off"`, or the stored `{kind, amount, wind}` object
/// (the `tool_set_weather` reply shape).
fn eds_weather_json(weather: Option<Weather>) -> Value {
    match weather {
        None => json!("off"),
        Some(w) => json!({
            "kind": eds_weather_kind_name(w.kind),
            "amount": w.amount,
            "wind": w.wind,
        }),
    }
}

// TOOL ENTRY SPEC (integrator: register in mcp_tools::tools()):
//   Tool {
//       name: "set_weather_kind",
//       description: "Set the scene weather kind (the Scene panel's kind picker): switching kinds keeps the tuned amount/wind, and enabling a kind from off applies the panel's per-kind preset. \"off\" turns weather off. One undo step on a real change, exactly like the panel.",
//       schema: obj_schema(
//           json!({
//               "kind": { "type": "string", "enum": ["rain", "snow", "fireflies", "fog", "storm", "off"] }
//           }),
//           &["kind"],
//       ),
//       handler: crate::editor::tool_set_weather_kind,
//   }
// MCP.md row (registry catalogue, beside `set_weather`):
//   | `set_weather_kind` | Set the scene weather kind (the Scene panel's kind picker): switching kinds keeps the tuned amount/wind; enabling from off applies the panel's per-kind preset. One undo step on a real change. | `kind` | — |
/// MCP handler: set the weather kind, the Scene panel
/// picker's exact semantics (its `set_weather_kind` in
/// `rune_sec/scene_panel.rs`, re-implemented here
/// because the panel is bin-crate code): switching
/// kinds keeps the tuned amount/wind, enabling from
/// off applies the kind's preset, and the status
/// strings are the panel's own.
///
/// Discipline: exactly one undo step on a real change
/// (Roadmap Wave 1, RM24, P-14 — A15 H3 closed the
/// Wave 13 mark_dirty-only census for the whole
/// weather family, panel and tools together).
pub fn tool_set_weather_kind(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let kind = eds_parse_weather_kind(eds_req_str(args, "kind")?)?;
    let before = core.project.weather;
    let (next, label): (Option<Weather>, &'static str) = match kind {
        None => (None, "Weather: off."),
        Some(kind) => {
            let next = Some(match before {
                Some(mut w) => {
                    w.kind = kind;
                    w
                }
                None => eds_kind_preset(kind),
            });
            let label: &'static str = match kind {
                WeatherKind::Rain => "Weather: rain.",
                WeatherKind::Snow => "Weather: snow.",
                WeatherKind::Fireflies => "Weather: fireflies.",
                WeatherKind::Fog => "Weather: fog.",
                WeatherKind::Storm => "Weather: storm.",
            };
            (next, label)
        }
    };
    if next != before {
        core.snapshot_labeled("Weather");
        core.project.weather = next;
    }
    core.status = label.into();
    Ok(json!({
        "ok": true,
        "weather": eds_weather_json(core.project.weather),
        "status": core.status,
    }))
}

// TOOL ENTRY SPEC (integrator: register in mcp_tools::tools()):
//   Tool {
//       name: "cycle_weather",
//       description: "Cycle the scene weather like the Scene group's Cycle weather button: off -> rain -> snow -> fireflies -> off, applying each kind's preset wholesale (fog and storm are not in the cycle; cycling from either turns weather off). One undo step, exactly like the panel.",
//       schema: obj_schema(json!({}), &[]),
//       handler: crate::editor::tool_cycle_weather,
//   }
// MCP.md row (registry catalogue, beside `set_weather`):
//   | `cycle_weather` | Cycle the scene weather (the Scene group's Cycle weather button): off → rain → snow → fireflies → off, each kind's preset applied wholesale; fog and storm are not in the cycle (cycling from either turns weather off). One undo step. | — | — |
/// MCP handler: cycle the weather kinds, the Scene
/// panel's `cycle_weather` semantics (re-implemented
/// here; the panel is bin-crate code): the cycle is
/// Off → Rain → Snow → Fireflies → Off with each
/// kind's preset applied wholesale — unlike the
/// picker, cycling does NOT keep tuned amount/wind.
///
/// Discipline: exactly one undo step — the cycle
/// always changes the weather (Roadmap Wave 1, RM24,
/// P-14; the Wave 13 mark_dirty-only rule is closed).
pub fn tool_cycle_weather(core: &mut EditorCore, _args: &Value) -> Result<Value, String> {
    let (next, label) = match core.project.weather {
        None => (Some(eds_kind_preset(WeatherKind::Rain)), "Weather: rain."),
        Some(w) => match w.kind {
            WeatherKind::Rain => {
                (Some(eds_kind_preset(WeatherKind::Snow)), "Weather: snow.")
            }
            WeatherKind::Snow => (
                Some(eds_kind_preset(WeatherKind::Fireflies)),
                "Weather: fireflies.",
            ),
            WeatherKind::Fireflies | WeatherKind::Fog | WeatherKind::Storm => {
                (None, "Weather: off.")
            }
        },
    };
    if core.project.weather != next {
        core.snapshot_labeled("Weather");
        core.project.weather = next;
    }
    core.status = label.into();
    Ok(json!({
        "ok": true,
        "weather": eds_weather_json(core.project.weather),
        "status": core.status,
    }))
}

// TOOL ENTRY SPEC (integrator: register in mcp_tools::tools()):
//   Tool {
//       name: "set_weather_amount",
//       description: "Set the weather's particle amount absolutely (the Scene panel's Amount stepper/slider): clamped to 0..=200. Errors when the weather is off — pick a kind first (set_weather_kind). One undo step on a real change, exactly like the panel's steppers.",
//       schema: obj_schema(
//           json!({
//               "amount": { "type": "number", "description": "Particles per frame at steady state, 0..=200" }
//           }),
//           &["amount"],
//       ),
//       handler: crate::editor::tool_set_weather_amount,
//   }
// MCP.md row (registry catalogue, beside `set_weather`):
//   | `set_weather_amount` | Set the weather's particle amount absolutely (the Scene panel's Amount stepper/slider), clamped to 0..=200. Errors when the weather is off. One undo step on a real change. | `amount` | — |
/// MCP handler: set the weather amount absolutely —
/// the amount half of the Scene panel's stepper/slider
/// pair, which `set_weather` cannot express without
/// also naming a kind and resending the wind.
///
/// Discipline: exactly one undo step on a real change
/// (Roadmap Wave 1, RM24, P-14 — the panel's steppers
/// take a coalesced step for a run; a single MCP call
/// is one discrete change, so it takes one labelled
/// step; a fully-clamped call changes nothing and
/// spends none). With weather off the panel is
/// status-only; an MCP call refuses instead, with the
/// panel's own wording, so no edit is spent.
pub fn tool_set_weather_amount(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let value = eds_req_f32(args, "amount")?;
    let Some(mut w) = core.project.weather else {
        return Err("Weather is off — pick a kind first.".to_string());
    };
    let amount = value.clamp(0.0, 200.0);
    if amount != w.amount {
        core.snapshot_labeled("Weather amount");
        w.amount = amount;
        core.project.weather = Some(w);
    }
    core.status = format!("Weather amount: {:.0}.", w.amount);
    Ok(json!({
        "ok": true,
        "weather": eds_weather_json(core.project.weather),
        "status": core.status,
    }))
}

// TOOL ENTRY SPEC (integrator: register in mcp_tools::tools()):
//   Tool {
//       name: "set_weather_wind",
//       description: "Set the weather's wind (horizontal drift, stage units per second) absolutely (the Scene panel's Wind stepper/slider): clamped to -200..=200. Errors when the weather is off — pick a kind first (set_weather_kind). One undo step on a real change, exactly like the panel's steppers.",
//       schema: obj_schema(
//           json!({
//               "wind": { "type": "number", "description": "Horizontal drift in stage units per second, -200..=200" }
//           }),
//           &["wind"],
//       ),
//       handler: crate::editor::tool_set_weather_wind,
//   }
// MCP.md row (registry catalogue, beside `set_weather`):
//   | `set_weather_wind` | Set the weather's wind (horizontal drift) absolutely (the Scene panel's Wind stepper/slider), clamped to -200..=200. Errors when the weather is off. One undo step on a real change. | `wind` | — |
/// MCP handler: set the weather wind absolutely — the
/// wind half of the Scene panel's stepper/slider pair.
/// Discipline: exactly one undo step on a real change,
/// exactly as [`tool_set_weather_amount`]; the clamp is
/// the panel's −200..=200. (The registry's `set_weather`
/// deliberately does not clamp wind; this handler
/// follows the PANEL, not that tool.)
pub fn tool_set_weather_wind(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let value = eds_req_f32(args, "wind")?;
    let Some(mut w) = core.project.weather else {
        return Err("Weather is off — pick a kind first.".to_string());
    };
    let wind = value.clamp(-200.0, 200.0);
    if wind != w.wind {
        core.snapshot_labeled("Weather wind");
        w.wind = wind;
        core.project.weather = Some(w);
    }
    core.status = format!("Weather wind: {}.", w.wind);
    Ok(json!({
        "ok": true,
        "weather": eds_weather_json(core.project.weather),
        "status": core.status,
    }))
}

// TOOL ENTRY SPEC (integrator: register in mcp_tools::tools()):
//   Tool {
//       name: "clear_camera_key",
//       description: "Remove the camera key at the CURRENT frame with one undo step (the camera panel's Clear key button). For a delete at any frame, delete_camera_key also works. Errors when the current frame holds no camera key.",
//       schema: obj_schema(json!({}), &[]),
//       handler: crate::editor::tool_clear_camera_key,
//   }
// MCP.md row (registry catalogue, beside `set_camera`):
//   | `clear_camera_key` | Remove the camera key at the current frame. One undo step. Errors when the current frame holds no camera key. | — | — |
/// MCP handler: the camera panel's Clear key button —
/// [`EditorCore::camera_clear_key`] at the current
/// frame. (The `delete_camera_key` built-in deletes at
/// any frame; this is the current-frame removal the
/// panel offers.)
///
/// Discipline: exactly one undo step, taken inside
/// the backing op since Roadmap Wave 1 (RM24, P-14 —
/// A15 H3 closed the old "Clear key (no undo)"
/// exemption; the panel button's label was retired
/// with it, IA-RM24-4). The existence check runs first
/// so a keyless frame refuses with the op's own status
/// wording and spends nothing.
pub fn tool_clear_camera_key(core: &mut EditorCore, _args: &Value) -> Result<Value, String> {
    if !core.project.camera.iter().any(|k| k.frame == core.current) {
        return Err("No camera key on this frame.".to_string());
    }
    let frame = core.current;
    core.camera_clear_key();
    Ok(json!({
        "ok": true,
        "frame": frame,
        "status": core.status,
    }))
}

// TOOL ENTRY SPEC (integrator: register in mcp_tools::tools()):
//   Tool {
//       name: "camera_zoom_by",
//       description: "Multiply the camera zoom at the current frame by 'factor' (the camera panel's zoom in/out buttons): upserts a camera key at the current frame with the zoom clamped to 0.3..=4.0, preserving the angle in effect. One undo step.",
//       schema: obj_schema(
//           json!({
//               "factor": { "type": "number", "description": "Zoom multiplier (must be positive; 2.0 doubles, 0.5 halves)" }
//           }),
//           &["factor"],
//       ),
//       handler: crate::editor::tool_camera_zoom_by,
//   }
// MCP.md row (registry catalogue, beside `set_camera`):
//   | `camera_zoom_by` | Multiply the camera zoom at the current frame by a factor (the camera panel's zoom buttons): upserts a key there, zoom clamped to 0.3..=4.0, angle preserved. One undo step. | `factor` | — |
/// MCP handler: the camera panel's zoom step buttons —
/// [`EditorCore::camera_zoom_by`] at the current
/// frame. (`set_camera` can set an absolute zoom at
/// any frame, but the panel's relative zoom — and its
/// angle-preserving upsert — had no tool.)
///
/// Discipline chosen: exactly one undo step, taken
/// inside the backing op (it snapshots once). The
/// factor is validated first (finite per the section
/// rule, and positive — a non-positive multiplier is
/// meaningless and the op would silently clamp the
/// resulting zoom instead of refusing).
pub fn tool_camera_zoom_by(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let factor = eds_req_f32(args, "factor")?;
    if factor <= 0.0 {
        return Err(format!("argument 'factor' must be positive (got {factor})"));
    }
    core.camera_zoom_by(factor);
    Ok(json!({
        "ok": true,
        "frame": core.current,
        "zoom": core.camera_here().2,
        "status": core.status,
    }))
}

// TOOL ENTRY SPEC (integrator: register in mcp_tools::tools()):
//   Tool {
//       name: "rotate_camera",
//       description: "Rotate the camera at the current frame by 'degrees' (the camera panel's rotate buttons): upserts a camera key carrying the pan/zoom in effect plus the new angle, wrapped to -180..180. One undo step. For an absolute angle at any frame, use set_camera with 'angle'.",
//       schema: obj_schema(
//           json!({
//               "degrees": { "type": "number", "description": "Rotation delta in degrees (positive = clockwise on stage)" }
//           }),
//           &["degrees"],
//       ),
//       handler: crate::editor::tool_rotate_camera,
//   }
// MCP.md row (registry catalogue, beside `set_camera`):
//   | `rotate_camera` | Rotate the camera at the current frame by a degree delta (the camera panel's rotate buttons): upserts a key with the pan/zoom in effect and the angle wrapped to -180..180. One undo step. | `degrees` | — |
/// MCP handler: the camera panel's rotate step
/// buttons — [`EditorCore::rotate_camera`] at the
/// current frame. (The panel's rotation DIAL composes
/// the same write per gesture with one snapshot of
/// its own; a discrete MCP call maps to the op's
/// press form.)
///
/// Discipline chosen: exactly one undo step, taken
/// inside the backing op. The reply's angle is the
/// stored convention — wrapped via `norm_deg`, as
/// `get_camera` documents.
pub fn tool_rotate_camera(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let degrees = eds_req_f32(args, "degrees")?;
    core.rotate_camera(degrees);
    Ok(json!({
        "ok": true,
        "frame": core.current,
        "angle": crate::model::norm_deg(
            core.project.camera_angle_at(core.current as f32)
        ),
        "status": core.status,
    }))
}

// TOOL ENTRY SPEC (integrator: register in mcp_tools::tools()):
//   Tool {
//       name: "get_onion_skin",
//       description: "Read the onion-skin master switch (the Onion panel's On/Off toggle): 'enabled' is the stage's master switch. The per-side depths are GUI shell state and are NOT reported here — over the live endpoint they are Squad B shell state; 'legacy_prev_frames' is the core's legacy single-side count, which the stage no longer reads. Read-only.",
//       schema: obj_schema(json!({}), &[]),
//       handler: crate::editor::tool_get_onion_skin,
//   }
// MCP.md row (registry catalogue, beside `set_weather`):
//   | `get_onion_skin` | Read the onion-skin master switch ('enabled'). The per-side depths are GUI shell state, not core state, and are not reported; 'legacy_prev_frames' is the core's legacy count the stage no longer reads. Read-only. | — | — |
/// MCP handler: read the onion-skin master switch,
/// `core.onion` — the field the stage's master switch
/// reads and the Onion panel's toggle flips. No tool
/// could see it before this one.
///
/// Honest scope: the per-side depths the panel's
/// steppers write are shell state (`State::onion_ui`,
/// bin crate) — a library handler cannot read them,
/// so they are not reported (Wave 14 Squad B's shell
/// control owns them). `legacy_prev_frames` reports
/// the core's legacy `onion_frames` count for what it
/// is: a value the stage no longer reads, exposed
/// read-only so a client is not left guessing — and
/// deliberately given NO setter (see the section
/// header: a setter would be a fake control).
///
/// Discipline chosen: read-only — no snapshot, no
/// dirty mark, no status write.
pub fn tool_get_onion_skin(core: &mut EditorCore, _args: &Value) -> Result<Value, String> {
    Ok(json!({
        "ok": true,
        "enabled": core.onion,
        "legacy_prev_frames": core.onion_frames,
    }))
}

// TOOL ENTRY SPEC (integrator: register in mcp_tools::tools()):
//   Tool {
//       name: "set_onion_skin",
//       description: "Turn the onion-skin master switch on or off (the Onion panel's On/Off toggle). Session view state, not project data: no undo step and no dirty mark, exactly like the panel toggle. The per-side depths are GUI shell state and cannot be set from the core surface.",
//       schema: obj_schema(
//           json!({
//               "enabled": { "type": "boolean", "description": "True to show onion skins, false to hide them" }
//           }),
//           &["enabled"],
//       ),
//       handler: crate::editor::tool_set_onion_skin,
//   }
// MCP.md row (registry catalogue, beside `set_weather`):
//   | `set_onion_skin` | Turn the onion-skin master switch on or off (the Onion panel's toggle). Session view state: no undo step, no dirty mark — exactly like the panel toggle. | `enabled` | — |
/// MCP handler: the Onion panel's On/Off toggle —
/// a bare write of `core.onion`, exactly the panel's
/// own handler body (`s.core.onion = !s.core.onion`).
///
/// Discipline chosen: NEITHER an undo step NOR a
/// dirty mark. `onion` is editor session state, not
/// project data — it is not serialised, and the panel
/// toggle takes no snapshot and marks nothing. Over
/// the live endpoint this flips the switch on the
/// session the user is looking at.
pub fn tool_set_onion_skin(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let enabled = eds_req_bool(args, "enabled")?;
    core.onion = enabled;
    Ok(json!({
        "ok": true,
        "enabled": core.onion,
    }))
}

// TOOL ENTRY SPEC (integrator: register in mcp_tools::tools()):
//   Tool {
//       name: "get_editor_settings",
//       description: "Read the editor-settings surface in one call: weather (kind/amount/wind, or \"off\"), background colour (0xRRGGBB), project easing, tween playback, camera follow target, loop range, onion master switch, fps, stage size and head-ring scale. The consolidated read behind the set_weather_kind / set_project_easing / set_project_background family — several of these values previously had no read at all (project easing, tween playback, head-ring scale). Read-only.",
//       schema: obj_schema(json!({}), &[]),
//       handler: crate::editor::tool_get_editor_settings,
//   }
// MCP.md row (registry catalogue, beside `set_weather`):
//   | `get_editor_settings` | Read the editor-settings surface in one call: weather, background, project easing, tween playback, camera follow, loop range, onion switch, fps, stage size and head-ring scale. Read-only. | — | — |
/// MCP handler: the consolidated read for this
/// section's whole domain. `get_state` reports some
/// of these (weather, background, fps, stage) and
/// `get_editor_status` the loop range, but the
/// project easing, tween playback and head-ring
/// scale had NO read anywhere — a client could set
/// them only blind. This handler closes that without
/// touching either built-in.
///
/// Discipline chosen: read-only — no snapshot, no
/// dirty mark, no status write.
pub fn tool_get_editor_settings(core: &mut EditorCore, _args: &Value) -> Result<Value, String> {
    let loop_range = match core.loop_range {
        Some((a, b)) => json!([a, b]),
        None => Value::Null,
    };
    Ok(json!({
        "ok": true,
        "weather": eds_weather_json(core.project.weather),
        "background": core.project.background,
        "easing": core.project.easing.label(),
        "tween_playback": core.project.tween_playback,
        "camera_follow": core.project.camera_follow,
        "loop_range": loop_range,
        "onion_enabled": core.onion,
        "fps": core.project.fps,
        "stage": core.project.stage,
        "head_ring_scale": core.project.head_ring_scale,
    }))
}

// TOOL ENTRY SPEC (integrator: register in mcp_tools::tools()):
//   Tool {
//       name: "camera_move_preset",
//       description: "Author a preset camera move at the current frame (the camera panel's Moves row): 'preset' is one of push-in, pull-out, pan-left, pan-right, truck-up, truck-down, arc-left, arc-right, shake. Non-shake presets write a start key (the camera as it stands, stamped with 'easing') and an end key 'frames' frames later; shake writes a deterministic damped key burst and ignores 'easing'. frames defaults to 24, range 1..=48 (shake needs >= 4); easing is one of linear | smooth | ease-in | ease-out | hold, default smooth. Exactly one undo step; a refused move (no room, bad arguments) spends nothing.",
//       schema: obj_schema(
//           json!({
//               "preset": { "type": "string", "enum": ["push-in", "pull-out", "pan-left", "pan-right", "truck-up", "truck-down", "arc-left", "arc-right", "shake"] },
//               "frames": { "type": "integer" },
//               "easing": { "type": "string", "enum": ["linear", "smooth", "ease-in", "ease-out", "hold"] }
//           }),
//           &["preset"],
//       ),
//       handler: crate::editor::tool_camera_move_preset,
//   }
// MCP.md row (registry catalogue, beside `camera_zoom_by`):
//   | `camera_move_preset` | Author a preset camera move at the current frame (the camera panel's Moves row): 'preset' is push-in, pull-out, pan-left, pan-right, truck-up, truck-down, arc-left, arc-right or shake. Non-shake presets write a start key (the camera as it stands, stamped with 'easing') and an end key 'frames' frames later; shake writes a deterministic damped key burst and ignores 'easing'. frames defaults to 24, range 1..=48 (shake >= 4); easing linear | smooth | ease-in | ease-out | hold, default smooth. One undo step; refusals spend nothing. | `preset` | `frames`, `easing` |
/// MCP handler: a camera-panel preset move (Roadmap
/// Wave 2, W2-39 — the preset core ops live with the
/// camera ops above). The handler is a thin dispatch:
/// every argument is parsed and validated BEFORE the
/// core op runs, and the op itself validates before its
/// snapshot, so a refusal — unknown preset, bad easing
/// label, out-of-range frames, no room on the timeline —
/// is an Err with no undo step and no write.
///
/// Discipline: exactly one undo step per applied move
/// (the core op's own), matching the panel press.
pub fn tool_camera_move_preset(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let preset = eds_req_str(args, "preset")?;
    let frames = match eds_get(args, "frames") {
        None => EditorCore::CAMERA_PRESET_DEFAULT_FRAMES,
        Some(v) => {
            let f = eds_req_f32(args, "frames")?;
            if f < 0.0 || f.fract() != 0.0 || f > usize::MAX as f32 {
                return Err("argument 'frames' must be a non-negative integer".into());
            }
            f as usize
        }
    };
    let easing = match eds_get(args, "easing") {
        None => crate::model::CameraEase::Smooth,
        Some(_) => {
            let label = eds_req_str(args, "easing")?;
            EditorCore::camera_preset_easing(label).ok_or_else(|| {
                "argument 'easing' must be one of: linear, smooth, ease-in, ease-out, hold"
                    .to_string()
            })?
        }
    };
    let depth = core.undo_depth();
    match preset {
        "push-in" => core.camera_push_in(frames, easing),
        "pull-out" => core.camera_pull_out(frames, easing),
        "pan-left" => core.camera_pan(-1, frames, easing),
        "pan-right" => core.camera_pan(1, frames, easing),
        "truck-up" => core.camera_truck(-1, frames, easing),
        "truck-down" => core.camera_truck(1, frames, easing),
        "arc-left" => core.camera_arc(-1, frames, easing),
        "arc-right" => core.camera_arc(1, frames, easing),
        "shake" => core.camera_shake(frames, 6.0),
        other => return Err(format!("unknown camera preset '{other}'")),
    }
    if core.undo_depth() == depth {
        // The op refused and said why in the status line
        // (the `tool_delete_node` precedent).
        return Err(core.status.clone());
    }
    Ok(json!({
        "ok": true,
        "preset": preset,
        "from_frame": core.current,
        "frames": frames,
        "status": core.status,
    }))
}

// ---------------------------------------------------------------------
// Wave P (P23): subtitle ops — the EditorCore half of board
// contract 1. The data, the style, the layout maths and the
// SRT/WebVTT converters live in `subtitles.rs`; these are the
// editing ops over `Project::subtitles` /
// `Project::subtitle_style`, in the house op style:
//
// * A refused op (bad range, empty text, no such cue,
//   non-finite font scale) changes nothing and takes NO
//   snapshot — it only sets `status`, in the failure voice
//   ("… failed: …").
// * A successful mutation snapshots exactly once (one undo
//   step via `snapshot()`), then writes.
// * The cue list is kept sorted by `(start_frame, end_frame)`
//   at all times; inserts are stable, so cues with equal
//   ranges keep their authoring order. Frame numbers are NOT
//   clamped to the timeline.
// * A cue's speaker rides the add/update ops as an optional
//   name, normalised by `subtitles::normalise_speaker`
//   (trimmed; empty stores none; a name holding `<`, `>` or
//   a newline refuses in the same failure voice, before any
//   snapshot). Update is a full replace: the stored speaker
//   becomes the normalised argument, so `None` clears it.
// * Success is reported on `status` with 1-based cue and
//   frame numbers.
// ---------------------------------------------------------------------

impl EditorCore {
    /// Add a cue over `[start_frame, end_frame)` with
    /// `text` and an optional `speaker` name, keeping the
    /// list sorted. One undo step.
    /// Overlapping an existing cue is allowed — stacking
    /// is a legitimate authoring choice (the one rule set
    /// in `subtitles.rs`'s module docs).
    /// Returns the new cue's index in the sorted list, or
    /// `None` when the cue is refused (invalid range,
    /// empty text, or a speaker name
    /// [`crate::subtitles::normalise_speaker`] refuses) —
    /// a refusal changes nothing and takes no snapshot.
    pub fn subtitle_add(
        &mut self,
        start_frame: usize,
        end_frame: usize,
        text: &str,
        speaker: Option<&str>,
    ) -> Option<usize> {
        if let Err(reason) = crate::subtitles::SubtitleCue::validate(start_frame, end_frame, text) {
            self.status = format!("Subtitle add failed: {reason}.");
            return None;
        }
        let speaker = match crate::subtitles::normalise_speaker(speaker) {
            Ok(speaker) => speaker,
            Err(reason) => {
                self.status = format!("Subtitle add failed: {reason}.");
                return None;
            }
        };
        self.snapshot_labeled("Add subtitle");
        let cue = crate::subtitles::SubtitleCue {
            start_frame,
            end_frame,
            text: text.to_string(),
            speaker,
        };
        // Insert after every cue whose key is <= the new
        // key: the list stays sorted and the insert is
        // stable among equal ranges.
        let key = (cue.start_frame, cue.end_frame);
        let index = self
            .project
            .subtitles
            .partition_point(|c| (c.start_frame, c.end_frame) <= key);
        self.project.subtitles.insert(index, cue);
        self.status = format!(
            "Subtitle {} added at frame {}.",
            index + 1,
            start_frame + 1
        );
        Some(index)
    }

    /// Replace cue `index` (an index into the sorted list —
    /// validated before anything is taken or changed) with a
    /// new range, text and speaker, re-sorting afterwards.
    /// A full replace: the stored speaker becomes the
    /// normalised `speaker` argument, so `None` (or an
    /// empty name) clears it. One undo step. Returns the
    /// cue's index after the re-sort, or
    /// `None` when refused (no such cue, or the new values
    /// fail validation) — a refusal changes nothing and
    /// takes no snapshot.
    pub fn subtitle_update(
        &mut self,
        index: usize,
        start_frame: usize,
        end_frame: usize,
        text: &str,
        speaker: Option<&str>,
    ) -> Option<usize> {
        if self.project.subtitles.get(index).is_none() {
            self.status = format!("Subtitle update failed: no subtitle {}.", index + 1);
            return None;
        }
        if let Err(reason) = crate::subtitles::SubtitleCue::validate(start_frame, end_frame, text) {
            self.status = format!("Subtitle update failed: {reason}.");
            return None;
        }
        let speaker = match crate::subtitles::normalise_speaker(speaker) {
            Ok(speaker) => speaker,
            Err(reason) => {
                self.status = format!("Subtitle update failed: {reason}.");
                return None;
            }
        };
        self.snapshot_labeled("Edit subtitle");
        self.project.subtitles.remove(index);
        let cue = crate::subtitles::SubtitleCue {
            start_frame,
            end_frame,
            text: text.to_string(),
            speaker,
        };
        let key = (cue.start_frame, cue.end_frame);
        let new_index = self
            .project
            .subtitles
            .partition_point(|c| (c.start_frame, c.end_frame) <= key);
        self.project.subtitles.insert(new_index, cue);
        self.status = format!("Subtitle {} updated.", new_index + 1);
        Some(new_index)
    }

    /// Delete cue `index` (an index into the sorted list,
    /// validated first). One undo step. Returns false —
    /// changing nothing and taking no snapshot — when there
    /// is no such cue.
    pub fn subtitle_delete(&mut self, index: usize) -> bool {
        if self.project.subtitles.get(index).is_none() {
            self.status = format!("Subtitle delete failed: no subtitle {}.", index + 1);
            return false;
        }
        self.snapshot_labeled("Delete subtitle");
        self.project.subtitles.remove(index);
        self.status = format!("Subtitle {} deleted.", index + 1);
        true
    }

    /// Shift EVERY cue by `ms` milliseconds (Roadmap
    /// Wave 1, RM21's core half, IA-RM21-3): the op under
    /// the Subtitles panel's Shift-all and the MCP
    /// `subtitle_shift_all` tool — one home for the bulk
    /// discipline (clone, shift, re-sort, one snapshot),
    /// so the panel and the tool cannot drift. The
    /// millisecond delta converts at the project rate
    /// ([`crate::subtitles::nudge_delta_frames`]: rounded,
    /// never zero for a non-zero shift) and each cue
    /// clamps at frame 0 individually, duration preserved
    /// ([`crate::subtitles::shifted_window`]). Returns how
    /// many cues actually moved; a shift in which no cue
    /// moves (or an empty list) reports on the status
    /// line and spends no undo step. The panel stands
    /// its selection and edit session down after a
    /// successful call — shell state this op never sees.
    pub fn subtitle_shift_all(&mut self, ms: i32) -> usize {
        let delta = crate::subtitles::nudge_delta_frames(ms, self.project.fps);
        let mut cues = self.project.subtitles.clone();
        let mut moved = 0usize;
        for cue in cues.iter_mut() {
            if let Some((start, end)) = crate::subtitles::shifted_window(cue, delta) {
                cue.start_frame = start;
                cue.end_frame = end;
                moved += 1;
            }
        }
        if moved == 0 {
            self.status = "Shift all subtitles: no cue moved.".to_string();
            return 0;
        }
        crate::subtitles::sort_subtitle_cues(&mut cues);
        self.snapshot_labeled("Shift all subtitles");
        self.project.subtitles = cues;
        self.status = format!("Shifted {moved} subtitle cue(s) by {ms} ms.");
        moved
    }

    /// Retime EVERY cue from `from_fps` to the project's
    /// current rate (Roadmap Wave 1, RM21's core half,
    /// IA-RM21-3): the op under the panel's Retime cues
    /// press and the MCP `subtitle_retime` tool. Each
    /// cue's window rescales by the rate ratio, the
    /// half-open minimum re-established
    /// ([`crate::subtitles::retimed_window`]), and the
    /// status line names BOTH rates — retiming is never
    /// silent. Returns the number of cues in the retimed
    /// list; an equal-rate call, or a rescale that lands
    /// every cue where it already sits, reports and
    /// spends no undo step. `from_fps` is the caller's
    /// validated rate (the window math itself refuses
    /// unusable rates by leaving windows unchanged,
    /// which lands here as the no-change report).
    /// Apply dialogue-timing fixes (Roadmap Wave 2,
    /// W2-16): the op under the Subtitles panel's Timing
    /// assistant Apply / Apply-all and the MCP
    /// `subtitle_timing_apply` tool — one home for the
    /// bulk discipline (compute whole, swap whole, one
    /// snapshot), so the panel and the tool cannot
    /// drift. The pure engine
    /// (`crate::subtitle_timing::timing_report`)
    /// measures every cue at the project's rate under
    /// `params` — the author's target pace and minimum
    /// duration, model parameters, NOT a warning
    /// threshold — and proposes one fix per needy cue;
    /// `only` applies the single proposal at that index
    /// of the report, `None` applies them all. Returns
    /// how many proposals were applied; a report with
    /// nothing applicable (or an `only` index the
    /// report no longer holds) reports on the status
    /// line and spends no undo step. The panel stands
    /// its selection and edit session down after a
    /// landed fix — shell state this op never sees.
    pub fn subtitle_timing_apply(
        &mut self,
        params: &crate::subtitle_timing::TimingParams,
        only: Option<usize>,
    ) -> usize {
        let report = crate::subtitle_timing::timing_report(
            &self.project.subtitles,
            self.project.fps,
            params,
        );
        let chosen: Vec<crate::subtitle_timing::Proposal> = match only {
            Some(k) => match report.proposals.get(k) {
                Some(p) => vec![p.clone()],
                None => {
                    self.status =
                        "Subtitle timing: that suggestion no longer applies.".to_string();
                    return 0;
                }
            },
            None => report.proposals,
        };
        if chosen.is_empty() {
            self.status = "Subtitle timing: nothing to change.".to_string();
            return 0;
        }
        let applied = chosen.len();
        let cues =
            crate::subtitle_timing::apply_proposals(&self.project.subtitles, &chosen);
        if cues == self.project.subtitles {
            self.status = "Subtitle timing: nothing to change.".to_string();
            return 0;
        }
        self.snapshot_labeled("Fix subtitle timing");
        self.project.subtitles = cues;
        self.status = format!(
            "Applied {applied} timing suggestion(s); {} subtitle cue(s) now.",
            self.project.subtitles.len()
        );
        applied
    }

    pub fn subtitle_retime(&mut self, from_fps: f32) -> usize {
        let to = self.project.fps;
        if (from_fps - to).abs() < f32::EPSILON {
            self.status =
                "Subtitle retime: cues already match the project frame rate.".to_string();
            return 0;
        }
        let mut cues = self.project.subtitles.clone();
        for cue in cues.iter_mut() {
            let (start, end) = crate::subtitles::retimed_window(cue, from_fps, to);
            cue.start_frame = start;
            cue.end_frame = end;
        }
        if cues == self.project.subtitles {
            self.status = "Subtitle retime: no cue window changed.".to_string();
            return 0;
        }
        let count = cues.len();
        crate::subtitles::sort_subtitle_cues(&mut cues);
        self.snapshot_labeled("Retime subtitles");
        self.project.subtitles = cues;
        self.status = format!("Retimed {count} subtitle cue(s) from {from_fps} fps to {to} fps.");
        count
    }

    /// All cues, in the list's kept order: sorted by
    /// `(start_frame, end_frame)`, stable among equals.
    pub fn subtitle_cues(&self) -> &[crate::subtitles::SubtitleCue] {
        &self.project.subtitles
    }

    /// How many cues the project carries.
    pub fn subtitle_count(&self) -> usize {
        self.project.subtitles.len()
    }

    /// The first cue (in stored order) live at `frame`,
    /// when one is. Overlapping cues resolve to the
    /// earliest-stored one.
    pub fn subtitle_at(&self, frame: usize) -> Option<&crate::subtitles::SubtitleCue> {
        self.project.subtitles.iter().find(|c| c.is_live_at(frame))
    }

    /// ALL cues live at `frame`, in the list's kept
    /// (stored) order. Overlapping cues are all returned:
    /// the renderers stack them (see
    /// `subtitles::subtitle_stack_line_offset`).
    /// `subtitle_at` above keeps its first-cue contract
    /// unchanged — it is exactly this list's first element.
    pub fn subtitles_at(&self, frame: usize) -> Vec<&crate::subtitles::SubtitleCue> {
        crate::subtitles::live_subtitle_cues(&self.project, frame)
    }

    /// The project's current subtitle style (the stored
    /// value verbatim; renderers sanitise at read via
    /// `SubtitleStyle::sanitised`).
    pub fn subtitle_style(&self) -> crate::subtitles::SubtitleStyle {
        self.project.subtitle_style
    }

    /// Replace the project's subtitle style. Exactly ONE
    /// undo step on success. The style is validated first
    /// (see `SubtitleStyle::validated`): a non-finite font
    /// scale is refused — the error is returned and mirrored
    /// on `status` in the ops' failure voice, nothing
    /// changes and no snapshot is taken. A finite
    /// out-of-range scale is clamped, not refused.
    pub fn set_subtitle_style(
        &mut self,
        style: crate::subtitles::SubtitleStyle,
    ) -> Result<(), String> {
        let style = match style.validated() {
            Ok(style) => style,
            Err(reason) => {
                self.status = format!("Subtitle style set failed: {reason}.");
                return Err(reason);
            }
        };
        self.snapshot_labeled("Subtitle style");
        self.project.subtitle_style = style;
        self.status = "Subtitle style updated.".to_string();
        Ok(())
    }
}

// ---------------------------------------------------------------------
// Roadmap Wave 2 (W2-11): chapter ops — the EditorCore half
// of P-15's episode structure. The data and its one
// validation rule live in `model.rs` (`Chapter`); these are
// the editing ops over `Project::chapters`, in the house op
// style the subtitle ops above established:
//
// * A refused op (bad range, empty title, no such chapter,
//   a move to the chapter's own position) changes nothing
//   and takes NO snapshot — it only sets `status`, in the
//   failure voice ("… failed: …").
// * A successful mutation snapshots exactly once (one undo
//   step, labelled for the History panel), then writes.
// * The list is NEVER re-sorted: its order is the episode
//   order. `chapter_add` appends; `chapter_move` is the
//   reorder. Ranges are attributes, not sort keys.
// * Frame numbers are NOT clamped to the timeline (the cue
//   posture): a chapter may be authored past the last
//   frame, and `end_frame` may equal the frame count.
// * Deleting a chapter removes the chapter ONLY — frames,
//   cues and every other collection are untouched. A
//   chapter is structure, not content.
// * Success is reported on `status` with 1-based chapter
//   and frame numbers.
// ---------------------------------------------------------------------

impl EditorCore {
    /// All chapters, in episode order.
    pub fn chapters(&self) -> &[crate::model::Chapter] {
        &self.project.chapters
    }

    /// How many chapters the project carries.
    pub fn chapter_count(&self) -> usize {
        self.project.chapters.len()
    }

    /// The chapter covering `frame` (an authored frame
    /// index): the first in episode order whose half-open
    /// range holds it, or `None` in a gap.
    pub fn chapter_at(&self, frame: usize) -> Option<&crate::model::Chapter> {
        self.project.chapter_at(frame)
    }

    /// The episode-order index of the chapter covering
    /// `frame`, when one does.
    pub fn chapter_index_at(&self, frame: usize) -> Option<usize> {
        self.project.chapter_index_at(frame)
    }

    /// Append a chapter titled `title` over
    /// `[start_frame, end_frame)` at the END of the episode
    /// order. One undo step. The stored title is the
    /// trimmed argument. Returns the new chapter's index
    /// (always the last position), or `None` when refused
    /// (invalid range or empty title) — a refusal changes
    /// nothing and takes no snapshot.
    pub fn chapter_add(
        &mut self,
        start_frame: usize,
        end_frame: usize,
        title: &str,
    ) -> Option<usize> {
        if let Err(reason) = crate::model::Chapter::validate(start_frame, end_frame, title) {
            self.status = format!("Chapter add failed: {reason}.");
            return None;
        }
        self.snapshot_labeled("Add chapter");
        self.project.chapters.push(crate::model::Chapter {
            title: title.trim().to_string(),
            start_frame,
            end_frame,
        });
        let index = self.project.chapters.len() - 1;
        self.status = format!(
            "Chapter {} added at frame {}.",
            index + 1,
            start_frame + 1
        );
        Some(index)
    }

    /// Rename chapter `index` (validated first) to the
    /// trimmed `title`. One undo step. Returns false —
    /// changing nothing and taking no snapshot — when there
    /// is no such chapter or the title is empty.
    pub fn chapter_rename(&mut self, index: usize, title: &str) -> bool {
        if self.project.chapters.get(index).is_none() {
            self.status = format!("Chapter rename failed: no chapter {}.", index + 1);
            return false;
        }
        if title.trim().is_empty() {
            self.status = "Chapter rename failed: title is empty.".to_string();
            return false;
        }
        let title = title.trim().to_string();
        self.snapshot_labeled("Rename chapter");
        self.project.chapters[index].title = title.clone();
        self.status = format!("Chapter {} renamed to {title}.", index + 1);
        true
    }

    /// Assign chapter `index` a new frame range
    /// `[start_frame, end_frame)` (validated first, in the
    /// chapter's own rule). The chapter keeps its position
    /// in the episode order — ranges are attributes, never
    /// sort keys. One undo step. Returns false — changing
    /// nothing and taking no snapshot — when there is no
    /// such chapter or the range is invalid.
    pub fn chapter_set_range(
        &mut self,
        index: usize,
        start_frame: usize,
        end_frame: usize,
    ) -> bool {
        let Some(title) = self.project.chapters.get(index).map(|c| c.title.clone()) else {
            self.status = format!("Chapter range failed: no chapter {}.", index + 1);
            return false;
        };
        if let Err(reason) = crate::model::Chapter::validate(start_frame, end_frame, &title) {
            self.status = format!("Chapter range failed: {reason}.");
            return false;
        }
        self.snapshot_labeled("Set chapter range");
        self.project.chapters[index].start_frame = start_frame;
        self.project.chapters[index].end_frame = end_frame;
        self.status = format!(
            "Chapter {} now spans frames {}-{}.",
            index + 1,
            start_frame + 1,
            end_frame
        );
        true
    }

    /// Move chapter `index` to position `to_index` in the
    /// episode order (both validated first) — the reorder
    /// op. One undo step. A move to the chapter's own
    /// position is a no-op: it reports, returns false and
    /// spends no snapshot (the bulk ops' no-change
    /// discipline). Returns false likewise when either
    /// index names no chapter.
    pub fn chapter_move(&mut self, index: usize, to_index: usize) -> bool {
        if self.project.chapters.get(index).is_none() {
            self.status = format!("Chapter move failed: no chapter {}.", index + 1);
            return false;
        }
        if self.project.chapters.get(to_index).is_none() {
            self.status = format!("Chapter move failed: no chapter {}.", to_index + 1);
            return false;
        }
        if index == to_index {
            self.status = format!(
                "Chapter {} is already at position {}.",
                index + 1,
                to_index + 1
            );
            return false;
        }
        self.snapshot_labeled("Move chapter");
        let chapter = self.project.chapters.remove(index);
        self.project.chapters.insert(to_index, chapter);
        self.status = format!(
            "Chapter moved to position {}.",
            to_index + 1
        );
        true
    }

    /// Delete chapter `index` (validated first). One undo
    /// step. The chapter ONLY is removed: frames, cues and
    /// all other content are untouched — a chapter is
    /// structure, not content. Returns false — changing
    /// nothing and taking no snapshot — when there is no
    /// such chapter.
    pub fn chapter_delete(&mut self, index: usize) -> bool {
        if self.project.chapters.get(index).is_none() {
            self.status = format!("Chapter delete failed: no chapter {}.", index + 1);
            return false;
        }
        self.snapshot_labeled("Delete chapter");
        self.project.chapters.remove(index);
        self.status = format!("Chapter {} deleted.", index + 1);
        true
    }
}

// ---------------------------------------------------------------------
// Roadmap Wave 2 (W2-05): stored-guide ops — the EditorCore
// half of rulers + stored guides. The data and its one
// validation rule live in `model.rs` (`StoredGuide`); these
// are the editing ops over `Project::stored_guides`, in the
// house op style the subtitle/chapter ops established:
//
// * A refused op (non-finite position, no such guide)
//   changes nothing and takes NO snapshot — it only sets
//   `status`, in the failure voice ("… failed: …").
// * A successful mutation snapshots exactly once (one undo
//   step, labelled for the History panel), then writes.
//   The stage's guide drag session obeys the same rule by
//   construction: its moves write nothing, and its release
//   lands exactly one of these ops.
// * The list is NEVER re-sorted: its order is creation
//   order. `stored_guide_add` appends; positions are
//   attributes, never sort keys (the chapter rule).
// * Positions are NOT clamped to the stage (the chapter /
//   cue posture): a guide may sit outside the stage rect.
// * Deleting a guide removes the guide ONLY — a guide is
//   annotation, not content.
// * Success is reported on `status` with the 1-based guide
//   number and the stored coordinate.
// ---------------------------------------------------------------------

impl EditorCore {
    /// All stored guides, in creation order.
    pub fn stored_guides(&self) -> &[crate::model::StoredGuide] {
        &self.project.stored_guides
    }

    /// How many stored guides the project carries.
    pub fn stored_guide_count(&self) -> usize {
        self.project.stored_guides.len()
    }

    /// Append a stored guide on `axis` at stage coordinate
    /// `position` at the END of the creation order. One
    /// undo step. Returns the new guide's index (always
    /// the last position), or `None` when refused
    /// (non-finite position) — a refusal changes nothing
    /// and takes no snapshot.
    pub fn stored_guide_add(
        &mut self,
        axis: crate::model::GuideAxis,
        position: f32,
    ) -> Option<usize> {
        if let Err(reason) = crate::model::StoredGuide::validate(position) {
            self.status = format!("Guide add failed: {reason}.");
            return None;
        }
        self.snapshot_labeled("Add guide");
        self.project.stored_guides.push(crate::model::StoredGuide {
            axis,
            position,
        });
        let index = self.project.stored_guides.len() - 1;
        self.status = format!("Guide {} added at {position}.", index + 1);
        Some(index)
    }

    /// Move guide `index` to stage coordinate `position`
    /// (validated first, in the guide's own rule). The
    /// guide keeps its axis and its creation-order
    /// position — only the coordinate changes. One undo
    /// step. A move to the guide's CURRENT coordinate is
    /// a no-op: it reports, returns false and spends no
    /// snapshot (the chapter-move discipline — this is
    /// what makes a click on a guide, or a drag that
    /// returns to its start, free). Returns false likewise
    /// when there is no such guide or the position is
    /// non-finite.
    pub fn stored_guide_move(&mut self, index: usize, position: f32) -> bool {
        let Some(guide) = self.project.stored_guides.get(index) else {
            self.status = format!("Guide move failed: no guide {}.", index + 1);
            return false;
        };
        if let Err(reason) = crate::model::StoredGuide::validate(position) {
            self.status = format!("Guide move failed: {reason}.");
            return false;
        }
        if guide.position == position {
            self.status = format!("Guide {} is already at {position}.", index + 1);
            return false;
        }
        self.snapshot_labeled("Move guide");
        self.project.stored_guides[index].position = position;
        self.status = format!("Guide {} moved to {position}.", index + 1);
        true
    }

    /// Delete guide `index` (validated first). One undo
    /// step. The guide ONLY is removed: figures, frames
    /// and every other collection are untouched — a guide
    /// is annotation, not content. Returns false —
    /// changing nothing and taking no snapshot — when
    /// there is no such guide.
    pub fn stored_guide_delete(&mut self, index: usize) -> bool {
        if self.project.stored_guides.get(index).is_none() {
            self.status = format!("Guide delete failed: no guide {}.", index + 1);
            return false;
        }
        self.snapshot_labeled("Delete guide");
        self.project.stored_guides.remove(index);
        self.status = format!("Guide {} deleted.", index + 1);
        true
    }

    /// Delete EVERY stored guide. One undo step however
    /// many guides there are (the bulk-op shape: the
    /// gesture is one action). Returns the number removed.
    /// With no guides stored this is a reported no-op:
    /// it returns 0 and spends no snapshot.
    pub fn stored_guide_clear(&mut self) -> usize {
        let n = self.project.stored_guides.len();
        if n == 0 {
            self.status = "No guides to clear.".to_string();
            return 0;
        }
        self.snapshot_labeled("Clear guides");
        self.project.stored_guides.clear();
        self.status = if n == 1 {
            "Cleared 1 guide.".to_string()
        } else {
            format!("Cleared {n} guides.")
        };
        n
    }
}

#[cfg(test)]
mod stored_guide_ops_tests {
    use super::*;

    fn guide(
        axis: crate::model::GuideAxis,
        position: f32,
    ) -> crate::model::StoredGuide {
        crate::model::StoredGuide { axis, position }
    }

    /// The whole project as its snapshot JSON — the exact
    /// bytes `snapshot()` would store. `Project` has no
    /// `PartialEq`, so this is how "untouched" is asserted
    /// (the house pattern, from the chapter ops tests).
    fn project_json(core: &EditorCore) -> String {
        serde_json::to_string(&core.project).unwrap()
    }

    #[test]
    fn add_appends_in_creation_order_and_never_sorts() {
        use crate::model::GuideAxis::*;
        let mut core = EditorCore::new();
        assert_eq!(core.stored_guide_add(Vertical, 480.0), Some(0));
        assert_eq!(core.stored_guide_add(Horizontal, 100.0), Some(1));
        // A smaller coordinate does NOT jump the queue:
        // positions are attributes, not sort keys.
        assert_eq!(core.stored_guide_add(Vertical, 40.0), Some(2));
        assert_eq!(
            core.stored_guides(),
            &[
                guide(Vertical, 480.0),
                guide(Horizontal, 100.0),
                guide(Vertical, 40.0),
            ]
        );
        assert_eq!(core.stored_guide_count(), 3);
    }

    #[test]
    fn each_successful_mutation_takes_exactly_one_undo_step() {
        use crate::model::GuideAxis::*;
        let mut core = EditorCore::new();
        let depth = core.undo_depth();
        core.stored_guide_add(Vertical, 480.0);
        core.stored_guide_add(Horizontal, 200.0);
        assert!(core.stored_guide_move(0, 512.0));
        assert!(core.stored_guide_delete(1));
        assert_eq!(core.undo_depth(), depth + 4, "one step per mutation");
        // Undo walks the mutations back one at a time and
        // restores the pre-add list verbatim.
        core.undo();
        assert_eq!(core.stored_guide_count(), 2, "delete undone");
        core.undo();
        assert_eq!(core.stored_guides()[0].position, 480.0, "move undone");
    }

    #[test]
    fn refused_ops_change_nothing_and_take_no_snapshot() {
        use crate::model::GuideAxis::*;
        let mut core = EditorCore::new();
        core.stored_guide_add(Vertical, 480.0);
        let before = project_json(&core);
        let depth = core.undo_depth();
        assert_eq!(core.stored_guide_add(Vertical, f32::NAN), None);
        assert_eq!(core.stored_guide_add(Horizontal, f32::INFINITY), None);
        assert!(!core.stored_guide_move(0, f32::NAN));
        assert!(!core.stored_guide_move(7, 10.0), "no such guide");
        assert!(!core.stored_guide_delete(7), "no such guide");
        assert_eq!(project_json(&core), before, "refusals write nothing");
        assert_eq!(core.undo_depth(), depth, "refusals spend nothing");
        assert!(core.status.contains("failed"), "status: {}", core.status);
    }

    #[test]
    fn move_to_the_current_coordinate_spends_nothing() {
        use crate::model::GuideAxis::*;
        let mut core = EditorCore::new();
        core.stored_guide_add(Vertical, 480.0);
        let depth = core.undo_depth();
        assert!(!core.stored_guide_move(0, 480.0));
        assert_eq!(core.undo_depth(), depth);
        assert!(core.status.contains("already at"), "status: {}", core.status);
    }

    #[test]
    fn positions_are_not_clamped_to_the_stage() {
        use crate::model::GuideAxis::*;
        let mut core = EditorCore::new();
        // Off-stage coordinates are stored verbatim (a
        // margin guide), on either side of either axis.
        assert_eq!(core.stored_guide_add(Vertical, -40.0), Some(0));
        assert_eq!(core.stored_guide_add(Horizontal, 5000.0), Some(1));
        assert_eq!(core.stored_guides()[0].position, -40.0);
        assert_eq!(core.stored_guides()[1].position, 5000.0);
    }

    #[test]
    fn delete_removes_the_guide_and_nothing_else() {
        use crate::model::GuideAxis::*;
        let mut core = EditorCore::new();
        core.stored_guide_add(Vertical, 480.0);
        let frames = core.project.frames.len();
        let figures = core.project.figures.len();
        assert!(core.stored_guide_delete(0));
        assert_eq!(core.stored_guide_count(), 0);
        assert_eq!(core.project.frames.len(), frames);
        assert_eq!(core.project.figures.len(), figures);
    }

    #[test]
    fn clear_is_one_step_and_an_empty_clear_spends_nothing() {
        use crate::model::GuideAxis::*;
        let mut core = EditorCore::new();
        let depth = core.undo_depth();
        assert_eq!(core.stored_guide_clear(), 0);
        assert_eq!(core.undo_depth(), depth, "empty clear is free");
        core.stored_guide_add(Vertical, 1.0);
        core.stored_guide_add(Vertical, 2.0);
        core.stored_guide_add(Horizontal, 3.0);
        let depth = core.undo_depth();
        assert_eq!(core.stored_guide_clear(), 3);
        assert_eq!(core.undo_depth(), depth + 1, "one step for the bulk delete");
        assert_eq!(core.stored_guide_count(), 0);
    }

    #[test]
    fn nearest_guide_measures_on_the_perpendicular_axis() {
        use crate::model::GuideAxis::*;
        let mut core = EditorCore::new();
        core.stored_guide_add(Vertical, 480.0);
        core.stored_guide_add(Horizontal, 200.0);
        core.stored_guide_add(Vertical, 500.0);
        let p = &core.project;
        // A vertical guide's distance ignores y entirely.
        assert_eq!(p.nearest_stored_guide([484.0, 9999.0], 13.0), Some(0));
        // The nearer of two same-axis guides wins.
        assert_eq!(p.nearest_stored_guide([497.0, 0.0], 13.0), Some(2));
        // A horizontal guide answers on y.
        assert_eq!(p.nearest_stored_guide([0.0, 190.0], 13.0), Some(1));
        // Outside tolerance: nothing.
        assert_eq!(p.nearest_stored_guide([300.0, 300.0], 13.0), None);
    }

    #[test]
    fn old_format_project_loads_with_no_guides() {
        // A full current serialisation with the
        // `stored_guides` key removed loads identically
        // apart from the empty list (the chapter pin's
        // shape): the serde default absorbs the field, no
        // format bump.
        let mut core = EditorCore::new();
        core.stored_guide_add(crate::model::GuideAxis::Vertical, 480.0);
        let mut value: serde_json::Value =
            serde_json::from_str(&project_json(&core)).unwrap();
        value
            .as_object_mut()
            .unwrap()
            .remove("stored_guides")
            .expect("the key was written");
        let loaded: crate::model::Project =
            serde_json::from_value(value).unwrap();
        assert!(loaded.stored_guides.is_empty());
        assert_eq!(loaded.format_version, core.project.format_version);
        // The minimal file loads too.
        let minimal: crate::model::Project =
            serde_json::from_str("{}").unwrap();
        assert!(minimal.stored_guides.is_empty());
    }

    #[test]
    fn guides_round_trip_and_guide_fields_default() {
        use crate::model::GuideAxis::*;
        let mut core = EditorCore::new();
        core.stored_guide_add(Vertical, 480.0);
        core.stored_guide_add(Horizontal, -12.5);
        let json = project_json(&core);
        let loaded: crate::model::Project =
            serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.stored_guides, core.project.stored_guides);
        // Byte-exact re-serialisation.
        assert_eq!(serde_json::to_string(&loaded).unwrap(), json);
        // A `{}` guide loads on per-field defaults:
        // vertical at 0 (the Default axis/coordinate).
        let g: crate::model::StoredGuide =
            serde_json::from_str("{}").unwrap();
        assert_eq!(g, guide(Vertical, 0.0));
    }

    #[test]
    fn validate_refuses_only_non_finite_positions() {
        assert!(crate::model::StoredGuide::validate(0.0).is_ok());
        assert!(crate::model::StoredGuide::validate(-1.0e6).is_ok());
        assert!(crate::model::StoredGuide::validate(f32::NAN).is_err());
        assert!(crate::model::StoredGuide::validate(f32::INFINITY).is_err());
        assert!(crate::model::StoredGuide::validate(f32::NEG_INFINITY).is_err());
    }

    #[test]
    fn endpoints_span_the_stage_and_handles_sit_on_the_ruler_edges() {
        use crate::model::GuideAxis::*;
        let stage = [960.0, 720.0];
        let v = guide(Vertical, 480.0);
        assert_eq!(v.endpoints(stage), Some(([480.0, 0.0], [480.0, 720.0])));
        assert_eq!(v.handle(stage), Some([480.0, 0.0]), "top edge");
        let h = guide(Horizontal, 200.0);
        assert_eq!(h.endpoints(stage), Some(([0.0, 200.0], [960.0, 200.0])));
        assert_eq!(h.handle(stage), Some([0.0, 200.0]), "left edge");
        // A non-finite stored guide is inert at read.
        let bad = guide(Vertical, f32::NAN);
        assert_eq!(bad.endpoints(stage), None);
        assert_eq!(bad.handle(stage), None);
    }
}

#[cfg(test)]
mod chapter_ops_tests {
    use super::*;

    fn chapter(start_frame: usize, end_frame: usize, title: &str) -> crate::model::Chapter {
        crate::model::Chapter {
            title: title.to_string(),
            start_frame,
            end_frame,
        }
    }

    /// The whole project as its snapshot JSON — the exact
    /// bytes `snapshot()` would store. `Project` has no
    /// `PartialEq`, so this is how "untouched" is asserted
    /// (the house pattern, from the subtitle ops tests).
    fn project_json(core: &EditorCore) -> String {
        serde_json::to_string(&core.project).unwrap()
    }

    fn titles(core: &EditorCore) -> Vec<&str> {
        core.chapters().iter().map(|c| c.title.as_str()).collect()
    }

    #[test]
    fn add_appends_in_episode_order_and_never_sorts() {
        let mut core = EditorCore::new();
        // Ranges deliberately added out of frame order: the
        // list keeps AUTHORING order — order is narrative,
        // ranges are attributes.
        assert_eq!(core.chapter_add(24, 48, "Finale"), Some(0));
        assert_eq!(core.chapter_add(0, 12, "Cold open"), Some(1));
        assert_eq!(core.chapter_add(12, 24, "Middle"), Some(2));
        assert_eq!(titles(&core), ["Finale", "Cold open", "Middle"]);
        assert_eq!(core.chapter_count(), 3);
        // Titles are stored trimmed.
        assert_eq!(core.chapter_add(48, 60, "  Coda  "), Some(3));
        assert_eq!(core.chapters()[3].title, "Coda");
        assert!(core.status.contains("added at frame 49"), "{}", core.status);
    }

    #[test]
    fn each_successful_mutation_takes_exactly_one_undo_step() {
        let mut core = EditorCore::new();
        assert_eq!(core.undo_depth(), 0);
        core.chapter_add(0, 12, "One");
        assert_eq!(core.undo_depth(), 1);
        core.chapter_add(12, 24, "Two");
        assert_eq!(core.undo_depth(), 2);
        core.chapter_rename(0, "Opening");
        assert_eq!(core.undo_depth(), 3);
        core.chapter_set_range(1, 12, 30);
        assert_eq!(core.undo_depth(), 4);
        core.chapter_move(1, 0);
        assert_eq!(core.undo_depth(), 5);
        core.chapter_delete(0);
        assert_eq!(core.undo_depth(), 6);
        // Undo restores the pre-delete list verbatim,
        // ranges and order alike.
        core.undo();
        assert_eq!(
            core.chapters(),
            [chapter(12, 30, "Two"), chapter(0, 12, "Opening")]
        );
    }

    #[test]
    fn refused_ops_change_nothing_and_take_no_snapshot() {
        let mut core = EditorCore::new();
        core.chapter_add(0, 12, "Kept");
        let before = project_json(&core);
        let depth = core.undo_depth();

        assert_eq!(core.chapter_add(12, 12, "empty range"), None);
        assert_eq!(core.chapter_add(13, 12, "inverted"), None);
        assert_eq!(core.chapter_add(12, 24, "   "), None);
        assert!(!core.chapter_rename(9, "ghost"));
        assert!(!core.chapter_rename(0, "  "));
        assert!(!core.chapter_set_range(9, 0, 4));
        assert!(!core.chapter_set_range(0, 4, 4));
        assert!(!core.chapter_move(9, 0));
        assert!(!core.chapter_move(0, 9));
        assert!(!core.chapter_delete(9));

        assert_eq!(project_json(&core), before);
        assert_eq!(core.undo_depth(), depth);
        assert!(core.status.contains("failed"), "{}", core.status);
    }

    #[test]
    fn rename_trims_and_reports_the_new_title() {
        let mut core = EditorCore::new();
        core.chapter_add(0, 12, "Old");
        assert!(core.chapter_rename(0, "  New name  "));
        assert_eq!(core.chapters()[0].title, "New name");
        assert_eq!(core.status, "Chapter 1 renamed to New name.");
    }

    #[test]
    fn set_range_keeps_the_chapters_episode_position() {
        let mut core = EditorCore::new();
        core.chapter_add(0, 12, "A");
        core.chapter_add(12, 24, "B");
        // Widen A past B's whole range: A stays FIRST —
        // the range is an attribute, never a sort key.
        assert!(core.chapter_set_range(0, 0, 30));
        assert_eq!(titles(&core), ["A", "B"]);
        assert_eq!(core.chapters()[0], chapter(0, 30, "A"));
        assert_eq!(core.status, "Chapter 1 now spans frames 1-30.");
    }

    #[test]
    fn move_reorders_and_a_self_move_spends_nothing() {
        let mut core = EditorCore::new();
        core.chapter_add(0, 12, "A");
        core.chapter_add(12, 24, "B");
        core.chapter_add(24, 36, "C");
        let depth = core.undo_depth();
        // Self-move: a reported no-op, no snapshot.
        assert!(!core.chapter_move(1, 1));
        assert_eq!(core.undo_depth(), depth);
        assert!(core.status.contains("already at position 2"), "{}", core.status);
        // Real moves land exactly where told, both ways.
        assert!(core.chapter_move(0, 2));
        assert_eq!(titles(&core), ["B", "C", "A"]);
        assert!(core.chapter_move(2, 0));
        assert_eq!(titles(&core), ["A", "B", "C"]);
        assert_eq!(core.undo_depth(), depth + 2);
    }

    #[test]
    fn delete_removes_the_chapter_and_nothing_else() {
        let mut core = EditorCore::new();
        core.subtitle_add(0, 6, "a line", None);
        let frames_before = core.frame_count();
        core.chapter_add(0, 12, "A");
        core.chapter_add(12, 24, "B");
        assert!(core.chapter_delete(0));
        assert_eq!(core.chapters(), [chapter(12, 24, "B")]);
        // Structure, not content: frames and cues untouched.
        assert_eq!(core.frame_count(), frames_before);
        assert_eq!(core.subtitle_count(), 1);
    }

    #[test]
    fn chapter_at_uses_the_half_open_range_and_episode_order() {
        let mut core = EditorCore::new();
        core.chapter_add(0, 12, "A");
        core.chapter_add(24, 36, "C");
        // Half-open: live at start, gone at end; the gap
        // 12..24 belongs to no chapter.
        assert_eq!(core.chapter_at(0).map(|c| c.title.as_str()), Some("A"));
        assert_eq!(core.chapter_at(11).map(|c| c.title.as_str()), Some("A"));
        assert_eq!(core.chapter_at(12), None);
        assert_eq!(core.chapter_at(23), None);
        assert_eq!(core.chapter_index_at(24), Some(1));
        assert_eq!(core.chapter_index_at(35), Some(1));
        assert_eq!(core.chapter_index_at(36), None);
        // Overlap resolves to the FIRST chapter in episode
        // order, whatever its range.
        core.chapter_add(6, 30, "Overlap");
        assert_eq!(core.chapter_at(8).map(|c| c.title.as_str()), Some("A"));
        assert_eq!(core.chapter_at(28).map(|c| c.title.as_str()), Some("C"));
    }

    #[test]
    fn ranges_are_not_clamped_to_the_timeline() {
        let mut core = EditorCore::new();
        // A chapter authored wholly past the demo timeline
        // is stored verbatim; it simply never shows until
        // the timeline grows into it (the cue posture).
        let frames = core.frame_count();
        assert_eq!(core.chapter_add(frames + 10, frames + 22, "Future"), Some(0));
        assert_eq!(core.chapters()[0], chapter(frames + 10, frames + 22, "Future"));
        assert_eq!(core.chapter_at(frames + 10).map(|c| c.title.as_str()), Some("Future"));
        // And an end AT the frame count — "to the end" —
        // is a valid range.
        assert_eq!(core.chapter_add(0, frames, "Whole"), Some(1));
    }

    #[test]
    fn validate_speaks_the_shared_refusal_shapes() {
        assert_eq!(
            crate::model::Chapter::validate(4, 4, "x"),
            Err("end frame 5 must be after start frame 5".to_string())
        );
        assert_eq!(
            crate::model::Chapter::validate(0, 4, "  "),
            Err("title is empty".to_string())
        );
        assert!(crate::model::Chapter::validate(0, 1, "x").is_ok());
    }

    #[test]
    fn old_format_project_loads_with_no_chapters() {
        // The pre-chapters format: a full current
        // serialisation with the `chapters` key REMOVED
        // must load identically apart from the empty list —
        // the serde default absorbs the field, so
        // format_version stays 1.
        let mut project = Project::new_demo();
        project.chapters.push(chapter(0, 2, "One"));
        let mut value = serde_json::to_value(&project).unwrap();
        let object = value.as_object_mut().unwrap();
        assert!(object.remove("chapters").is_some());
        let text = serde_json::to_string(&value).unwrap();
        let loaded: Project = serde_json::from_str(&text).unwrap();
        assert!(loaded.chapters.is_empty());
        assert_eq!(loaded.name, project.name);
        assert_eq!(loaded.fps, project.fps);
        assert_eq!(loaded.frames.len(), project.frames.len());
        assert_eq!(loaded.format_version, crate::model::FORMAT_VERSION_CURRENT);
        // The strongest form: a file naming almost nothing
        // at all still loads, chapters empty.
        let minimal: Project = serde_json::from_str("{}").unwrap();
        assert!(minimal.chapters.is_empty());
    }

    #[test]
    fn chapters_round_trip_and_chapter_fields_default() {
        let mut project = Project::new_demo();
        project.chapters = vec![chapter(0, 12, "Opening"), chapter(12, 24, "Finale")];
        let text = serde_json::to_string(&project).unwrap();
        let loaded: Project = serde_json::from_str(&text).unwrap();
        assert_eq!(loaded.chapters, project.chapters);
        // Re-serialising the load reproduces the bytes:
        // nothing is dropped, renamed or re-defaulted.
        assert_eq!(serde_json::to_string(&loaded).unwrap(), text);
        // A chapter object missing fields loads on the
        // per-field defaults (hand-trimmed files stay
        // loadable, the nested-struct census posture).
        let bare: crate::model::Chapter = serde_json::from_str("{}").unwrap();
        assert_eq!(bare, chapter(0, 0, ""));
    }
}

#[cfg(test)]
mod subtitle_ops_tests {
    use super::*;
    use crate::subtitles::{SubtitleCue, SubtitlePosition, SubtitleStyle};

    fn cue(start_frame: usize, end_frame: usize, text: &str) -> SubtitleCue {
        SubtitleCue {
            start_frame,
            end_frame,
            text: text.to_string(),
            speaker: None,
        }
    }

    /// The whole project as its snapshot JSON — the exact
    /// bytes `snapshot()` would store. `Project` has no
    /// `PartialEq`, so this is how "untouched" is asserted
    /// (the house pattern).
    fn project_json(core: &EditorCore) -> String {
        serde_json::to_string(&core.project).unwrap()
    }

    #[test]
    fn add_keeps_the_list_sorted_and_reports_the_index() {
        let mut core = EditorCore::new();
        assert_eq!(core.subtitle_add(10, 14, "last", None), Some(0));
        assert_eq!(core.subtitle_add(0, 4, "first", None), Some(0));
        assert_eq!(core.subtitle_add(4, 10, "middle", None), Some(1));
        // Equal ranges keep authoring order (stable insert).
        assert_eq!(core.subtitle_add(4, 10, "middle two", None), Some(2));
        let texts: Vec<&str> = core.subtitle_cues().iter().map(|c| c.text.as_str()).collect();
        assert_eq!(texts, ["first", "middle", "middle two", "last"]);
        assert_eq!(core.subtitle_count(), 4);
    }

    #[test]
    fn each_successful_mutation_takes_exactly_one_undo_step() {
        let mut core = EditorCore::new();
        assert_eq!(core.undo_depth(), 0);
        core.subtitle_add(0, 4, "one", None);
        assert_eq!(core.undo_depth(), 1);
        core.subtitle_add(4, 8, "two", None);
        assert_eq!(core.undo_depth(), 2);
        core.subtitle_update(0, 8, 12, "one moved", None);
        assert_eq!(core.undo_depth(), 3);
        core.subtitle_delete(0);
        assert_eq!(core.undo_depth(), 4);
        // Undo restores the pre-delete list verbatim.
        core.undo();
        let texts: Vec<&str> = core.subtitle_cues().iter().map(|c| c.text.as_str()).collect();
        assert_eq!(texts, ["two", "one moved"]);
    }

    #[test]
    fn refused_ops_change_nothing_and_take_no_snapshot() {
        let mut core = EditorCore::new();
        core.subtitle_add(0, 4, "kept", None);
        let before = project_json(&core);
        let depth = core.undo_depth();

        assert_eq!(core.subtitle_add(4, 4, "empty range", None), None);
        assert_eq!(core.subtitle_add(5, 4, "inverted", None), None);
        assert_eq!(core.subtitle_add(4, 8, "  ", None), None);
        assert_eq!(core.subtitle_update(9, 0, 4, "ghost", None), None);
        assert_eq!(core.subtitle_update(0, 3, 3, "bad range", None), None);
        assert_eq!(core.subtitle_update(0, 0, 4, "", None), None);
        assert!(!core.subtitle_delete(9));

        assert_eq!(project_json(&core), before);
        assert_eq!(core.undo_depth(), depth);
        assert!(core.status.contains("failed"), "{}", core.status);
    }

    #[test]
    fn update_replaces_and_resorts() {
        let mut core = EditorCore::new();
        core.subtitle_add(0, 4, "a", None);
        core.subtitle_add(4, 8, "b", None);
        core.subtitle_add(8, 12, "c", None);
        // Move "a" past "c": it lands at the end, and the
        // returned index is its new position.
        assert_eq!(core.subtitle_update(0, 12, 16, "a2", None), Some(2));
        let texts: Vec<&str> = core.subtitle_cues().iter().map(|c| c.text.as_str()).collect();
        assert_eq!(texts, ["b", "c", "a2"]);
        assert_eq!(core.subtitle_cues()[2], cue(12, 16, "a2"));
    }

    #[test]
    fn delete_removes_the_cue() {
        let mut core = EditorCore::new();
        core.subtitle_add(0, 4, "a", None);
        core.subtitle_add(4, 8, "b", None);
        assert!(core.subtitle_delete(0));
        assert_eq!(core.subtitle_cues(), [cue(4, 8, "b")]);
    }

    #[test]
    fn subtitle_at_uses_the_half_open_range() {
        let mut core = EditorCore::new();
        core.subtitle_add(2, 5, "cue", None);
        assert_eq!(core.subtitle_at(1), None);
        assert_eq!(core.subtitle_at(2).map(|c| c.text.as_str()), Some("cue"));
        assert_eq!(core.subtitle_at(4).map(|c| c.text.as_str()), Some("cue"));
        // end_frame is exclusive.
        assert_eq!(core.subtitle_at(5), None);
    }

    #[test]
    fn subtitles_at_returns_every_live_cue_in_stored_order() {
        let mut core = EditorCore::new();
        core.subtitle_add(0, 10, "under", None);
        core.subtitle_add(2, 6, "over", None);
        let live: Vec<&str> = core.subtitles_at(3).iter().map(|c| c.text.as_str()).collect();
        assert_eq!(live, ["under", "over"]);
        // subtitle_at is exactly the first of subtitles_at.
        assert_eq!(core.subtitle_at(3).map(|c| c.text.as_str()), Some("under"));
        assert!(core.subtitles_at(10).is_empty());
    }

    #[test]
    fn core_queries_follow_stored_order_on_a_foreign_list() {
        // Wave R (R2): the queries never re-sort at read.
        // A list written directly into the project (the
        // hand-edited-file shape — every op keeps the list
        // sorted, so only foreign state looks like this)
        // is followed in its stored order: `subtitle_at`
        // is the earliest-STORED live cue, exactly the
        // first element of `subtitles_at`. An invalid
        // stored cue (inverted here) is inert — the
        // half-open liveness test can never hold for it.
        let mut core = EditorCore::new();
        core.project.subtitles = vec![
            cue(8, 20, "stored first"),
            cue(0, 10, "stored second"),
            cue(6, 3, "inverted"),
        ];
        assert_eq!(core.subtitle_at(9).map(|c| c.text.as_str()), Some("stored first"));
        let live: Vec<&str> = core.subtitles_at(9).iter().map(|c| c.text.as_str()).collect();
        assert_eq!(live, ["stored first", "stored second"]);
        for frame in 0..24 {
            assert_ne!(core.subtitle_at(frame).map(|c| c.text.as_str()), Some("inverted"));
        }
    }

    #[test]
    fn set_subtitle_style_is_one_step_and_validates() {
        let mut core = EditorCore::new();
        let depth = core.undo_depth();
        let style = SubtitleStyle {
            position: SubtitlePosition::Top,
            font_scale: 2.0,
            text_color: 0xFF_FF_00,
            box_color: 0x10_20_30,
            font: Default::default(),
        };
        core.set_subtitle_style(style).expect("a valid style sets");
        assert_eq!(core.subtitle_style(), style);
        assert_eq!(core.undo_depth(), depth + 1);
        // A finite out-of-range scale clamps, not refuses.
        core.set_subtitle_style(SubtitleStyle {
            font_scale: 99.0,
            ..SubtitleStyle::default()
        })
        .expect("a finite scale clamps");
        assert_eq!(core.subtitle_style().font_scale, crate::subtitles::SUBTITLE_FONT_SCALE_MAX);
        // A non-finite scale refuses: no step, style kept.
        let depth = core.undo_depth();
        let kept = core.subtitle_style();
        let err = core
            .set_subtitle_style(SubtitleStyle {
                font_scale: f32::NAN,
                ..SubtitleStyle::default()
            })
            .expect_err("a non-finite scale refuses");
        assert!(err.contains("not a finite number"), "{err}");
        assert_eq!(core.undo_depth(), depth);
        assert_eq!(core.subtitle_style(), kept);
        assert!(core.status.contains("failed"), "{}", core.status);
    }

    // -----------------------------------------------------------------
    // Wave R (R21): the speaker rides the ops — normalised
    // at the write path, a full-replace field on update,
    // and never a second undo step.
    // -----------------------------------------------------------------

    #[test]
    fn add_and_update_carry_the_normalised_speaker() {
        let mut core = EditorCore::new();
        // The name is trimmed at the write path; the cue
        // stores the trimmed form. One undo step.
        assert_eq!(
            core.subtitle_add(0, 4, "line", Some("  Mary Jane  ")),
            Some(0)
        );
        assert_eq!(core.subtitle_cues()[0].speaker.as_deref(), Some("Mary Jane"));
        assert_eq!(core.undo_depth(), 1);
        // Changing only the speaker is a real change: one
        // step, and undo restores the earlier name.
        assert_eq!(core.subtitle_update(0, 0, 4, "line", Some("John")), Some(0));
        assert_eq!(core.subtitle_cues()[0].speaker.as_deref(), Some("John"));
        assert_eq!(core.undo_depth(), 2);
        core.undo();
        assert_eq!(core.subtitle_cues()[0].speaker.as_deref(), Some("Mary Jane"));
        assert_eq!(core.undo_depth(), 1);
        // A whitespace-only name clears the speaker:
        // update is a full replace and the empty name
        // normalises to none.
        assert_eq!(core.subtitle_update(0, 0, 4, "line", Some("   ")), Some(0));
        assert_eq!(core.subtitle_cues()[0].speaker, None);
        // And `None` simply means no speaker.
        assert_eq!(core.subtitle_add(4, 8, "plain", None), Some(1));
        assert_eq!(core.subtitle_cues()[1].speaker, None);
    }

    #[test]
    fn speaker_refusals_change_nothing_and_take_no_snapshot() {
        let mut core = EditorCore::new();
        core.subtitle_add(0, 4, "kept", Some("Mary"));
        let before = project_json(&core);
        let depth = core.undo_depth();
        // A name that cannot survive a WebVTT voice tag
        // refuses at the op, in the failure voice.
        assert_eq!(core.subtitle_add(4, 8, "new", Some("A>B")), None);
        assert_eq!(core.subtitle_update(0, 0, 4, "kept", Some("A<B")), None);
        assert_eq!(core.subtitle_update(0, 0, 4, "kept", Some("A\nB")), None);
        // The speaker is metadata, not text: whitespace
        // TEXT with a perfectly good speaker still
        // refuses — on the text rule.
        assert_eq!(core.subtitle_add(4, 8, "  ", Some("Mary")), None);
        assert_eq!(project_json(&core), before);
        assert_eq!(core.undo_depth(), depth);
        assert!(core.status.contains("failed"), "{}", core.status);
    }
}

#[cfg(test)]
mod wave14_settings_mcp_tests {
    use super::*;
    use serde_json::json;

    /// A core whose weather starts off and whose edit
    /// counters start clean, whatever the demo project
    /// carries.
    fn clean_core() -> EditorCore {
        let mut core = EditorCore::new();
        core.project.weather = None;
        core
    }

    #[test]
    fn set_weather_kind_from_off_applies_the_panel_preset() {
        let mut core = clean_core();
        let edits = core.edits_since_autosave();
        let out = tool_set_weather_kind(&mut core, &json!({ "kind": "rain" })).unwrap();
        let w = core.project.weather.expect("weather is on");
        assert_eq!(w.kind, WeatherKind::Rain);
        // The PANEL preset (wind 25), not tool_set_weather's 20.
        assert_eq!((w.amount, w.wind), (90.0, 25.0));
        assert_eq!(out["weather"]["kind"], "rain");
        assert_eq!(core.status, "Weather: rain.");
        // RM24: a counted edit AND exactly one undo step.
        assert_eq!(core.undo_depth(), 1, "one undo step");
        assert_eq!(core.undo_label(), Some("Weather"));
        assert_eq!(core.edits_since_autosave(), edits + 1, "one counted edit");
    }

    #[test]
    fn set_weather_kind_switching_keeps_tuned_values() {
        let mut core = clean_core();
        core.project.weather = Some(Weather {
            kind: WeatherKind::Rain,
            amount: 33.0,
            wind: -7.0,
        });
        tool_set_weather_kind(&mut core, &json!({ "kind": "snow" })).unwrap();
        let w = core.project.weather.expect("weather is on");
        assert_eq!(w.kind, WeatherKind::Snow);
        assert_eq!((w.amount, w.wind), (33.0, -7.0), "tuned values kept");
        // Re-setting the same kind changes nothing and marks nothing.
        let edits = core.edits_since_autosave();
        tool_set_weather_kind(&mut core, &json!({ "kind": "snow" })).unwrap();
        assert_eq!(core.edits_since_autosave(), edits, "no change, no mark");
    }

    #[test]
    fn set_weather_kind_off_and_unknown_kind() {
        let mut core = clean_core();
        tool_set_weather_kind(&mut core, &json!({ "kind": "fog" })).unwrap();
        let out = tool_set_weather_kind(&mut core, &json!({ "kind": "off" })).unwrap();
        assert_eq!(core.project.weather, None);
        assert_eq!(out["weather"], "off");
        let err = tool_set_weather_kind(&mut core, &json!({ "kind": "hail" }))
            .expect_err("an unknown kind refuses");
        assert!(err.contains("unknown weather kind 'hail'"), "err: {err}");
    }

    #[test]
    fn cycle_weather_walks_the_panel_cycle() {
        let mut core = clean_core();
        tool_cycle_weather(&mut core, &json!({})).unwrap();
        assert_eq!(core.project.weather.map(|w| w.kind), Some(WeatherKind::Rain));
        tool_cycle_weather(&mut core, &json!({})).unwrap();
        assert_eq!(core.project.weather.map(|w| w.kind), Some(WeatherKind::Snow));
        tool_cycle_weather(&mut core, &json!({})).unwrap();
        assert_eq!(
            core.project.weather.map(|w| w.kind),
            Some(WeatherKind::Fireflies)
        );
        tool_cycle_weather(&mut core, &json!({})).unwrap();
        assert_eq!(core.project.weather, None);
        // Fog is not in the cycle: cycling from fog turns off.
        core.project.weather = Some(eds_kind_preset(WeatherKind::Fog));
        tool_cycle_weather(&mut core, &json!({})).unwrap();
        assert_eq!(core.project.weather, None);
        // RM24: every cycle is a real change, so every
        // cycle took its undo step (5 in this walk).
        assert_eq!(core.undo_depth(), 5, "each cycle is one undo step");
    }

    #[test]
    fn set_weather_amount_and_wind_clamp_and_refuse_when_off() {
        let mut core = clean_core();
        let err = tool_set_weather_amount(&mut core, &json!({ "amount": 10.0 }))
            .expect_err("amount with weather off refuses");
        assert!(err.contains("pick a kind first"), "err: {err}");
        core.project.weather = Some(eds_kind_preset(WeatherKind::Rain));
        tool_set_weather_amount(&mut core, &json!({ "amount": 500.0 })).unwrap();
        assert_eq!(core.project.weather.map(|w| w.amount), Some(200.0));
        tool_set_weather_wind(&mut core, &json!({ "wind": -999.0 })).unwrap();
        assert_eq!(core.project.weather.map(|w| w.wind), Some(-200.0));
        assert_eq!(core.status, "Weather wind: -200.");
        // RM24: the amount and wind writes took one step each.
        assert_eq!(core.undo_depth(), 2, "one undo step per settings write");
        // A non-finite amount is refused by the finiteness rule.
        assert!(tool_set_weather_amount(&mut core, &json!({ "amount": 1e300 })).is_err());
    }

    #[test]
    fn clear_camera_key_takes_one_undo_step() {
        // (Wave 14 pinned this as mark-only; RM24 closed
        // the "no undo" exemption — see IA-RM24-4.)
        let mut core = clean_core();
        // No key anywhere: refused, nothing spent.
        let err = tool_clear_camera_key(&mut core, &json!({}))
            .expect_err("no key on this frame refuses");
        assert!(err.contains("No camera key on this frame."), "err: {err}");
        core.project.set_camera_key(core.current, 10.0, 20.0, 1.0);
        let edits = core.edits_since_autosave();
        tool_clear_camera_key(&mut core, &json!({})).unwrap();
        assert!(core.project.camera.is_empty(), "the key is gone");
        assert_eq!(core.undo_depth(), 1, "clear takes one undo step");
        assert_eq!(core.undo_label(), Some("Clear camera key"));
        assert_eq!(core.edits_since_autosave(), edits + 1, "and it counts one edit");
        core.undo();
        assert_eq!(core.project.camera.len(), 1, "undo restores the cleared key");
    }

    #[test]
    fn camera_zoom_by_and_rotate_camera_take_one_undo_step_each() {
        let mut core = clean_core();
        let depth = core.undo_depth();
        let out = tool_camera_zoom_by(&mut core, &json!({ "factor": 2.0 })).unwrap();
        assert_eq!(core.undo_depth(), depth + 1, "zoom is one undo step");
        assert_eq!(out["zoom"], 2.0);
        let out = tool_rotate_camera(&mut core, &json!({ "degrees": 30.0 })).unwrap();
        assert_eq!(core.undo_depth(), depth + 2, "rotate is one undo step");
        assert_eq!(out["angle"], 30.0);
        // Refusals spend nothing.
        let depth = core.undo_depth();
        assert!(tool_camera_zoom_by(&mut core, &json!({ "factor": 0.0 })).is_err());
        assert!(tool_camera_zoom_by(&mut core, &json!({ "factor": 1e300 })).is_err());
        assert!(tool_rotate_camera(&mut core, &json!({ "degrees": 1e300 })).is_err());
        assert_eq!(core.undo_depth(), depth, "refusals take no snapshot");
    }

    // ---- Camera preset moves (Roadmap Wave 2, W2-39) ----

    /// A demo core stretched to `n` frames (the demo's own
    /// 5 looped) and optionally given a first camera key, so
    /// a preset move has room and a known start camera.
    fn preset_core(n: usize, keyed: bool) -> EditorCore {
        let mut core = clean_core();
        while core.project.frames.len() < n {
            let frame = core.project.frames.last().unwrap().clone();
            core.project.frames.push(frame);
        }
        if keyed {
            core.project
                .set_camera_key(0, core.project.stage[0] / 2.0, core.project.stage[1] / 2.0, 1.0);
        }
        core
    }

    fn key_at(core: &EditorCore, frame: usize) -> crate::model::CameraKey {
        core.project
            .camera
            .iter()
            .find(|k| k.frame == frame)
            .cloned()
            .unwrap_or_else(|| panic!("no camera key at frame {frame}"))
    }

    #[test]
    fn camera_push_in_writes_two_keys_and_one_undo_step() {
        use crate::model::CameraEase;
        let mut core = preset_core(60, true);
        let depth = core.undo_depth();
        core.camera_push_in(24, CameraEase::Smooth);
        assert_eq!(core.undo_depth(), depth + 1, "one press, one undo step");
        assert_eq!(core.undo_label(), Some("Camera push-in"));
        let a = key_at(&core, 0);
        let b = key_at(&core, 24);
        assert_eq!(a.easing, CameraEase::Smooth, "the start key carries the segment timing");
        assert_eq!(a.zoom, 1.0);
        assert!((b.zoom - EditorCore::CAMERA_PRESET_ZOOM_FACTOR).abs() < 1e-6);
        assert_eq!(b.easing, CameraEase::Linear, "the end key keeps a fresh key's easing");
        // The camera converges toward the demo figure's
        // root (480, 470): 3/4 of the way from (480, 360).
        assert!((b.x - 480.0).abs() < 1e-6);
        assert!((b.y - 442.5).abs() < 0.01, "end y: {}", b.y);
        // Evaluation rides the start key's easing and the
        // log-space zoom: the Smooth midpoint is t = 0.5, so
        // the mid-move zoom is the geometric mean, sqrt(1.5625).
        let (_, _, mid_zoom) = core.project.camera_at(12.0);
        assert!((mid_zoom - EditorCore::CAMERA_PRESET_ZOOM_FACTOR.sqrt()).abs() < 0.01);
        core.undo();
        assert_eq!(core.project.camera.len(), 1, "undo removes the whole move");
    }

    #[test]
    fn camera_preset_refusals_spend_nothing() {
        use crate::model::CameraEase;
        let mut core = preset_core(30, true);
        let depth = core.undo_depth();
        core.camera_push_in(0, CameraEase::Smooth);
        assert!(core.status.contains("at least 1 frame"), "status: {}", core.status);
        core.camera_push_in(49, CameraEase::Smooth);
        assert!(core.status.contains("at most"), "status: {}", core.status);
        core.camera_push_in(30, CameraEase::Smooth);
        assert!(core.status.contains("No room"), "status: {}", core.status);
        core.camera_pan(0, 12, CameraEase::Smooth);
        core.camera_arc(0, 12, CameraEase::Smooth);
        assert_eq!(core.undo_depth(), depth, "refusals take no snapshot");
        assert_eq!(core.project.camera.len(), 1, "and write no keys");
    }

    #[test]
    fn camera_pan_and_truck_move_one_screenful() {
        use crate::model::CameraEase;
        let mut core = preset_core(60, true);
        core.camera_pan(1, 12, CameraEase::Linear);
        let b = key_at(&core, 12);
        assert!((b.x - (480.0 + 960.0)).abs() < 0.01, "one stage width at zoom 1: {}", b.x);
        assert!((b.y - 360.0).abs() < 1e-6, "a pan never moves y");
        assert!((b.zoom - 1.0).abs() < 1e-6);
        core.goto(12);
        core.camera_truck(-1, 12, CameraEase::Linear);
        let c = key_at(&core, 24);
        assert!((c.y - (360.0 - 720.0)).abs() < 0.01, "one stage height up: {}", c.y);
        assert!((c.x - b.x).abs() < 1e-6, "a truck never moves x");
    }

    #[test]
    fn camera_pull_out_homes_to_the_stage_centre() {
        use crate::model::CameraEase;
        let mut core = preset_core(60, false);
        core.project.set_camera_key(0, 200.0, 200.0, 2.0);
        core.camera_pull_out(24, CameraEase::Smooth);
        let b = key_at(&core, 24);
        // 3/4 of the way from (200, 200) to the stage centre.
        assert!((b.x - 410.0).abs() < 0.01, "end x: {}", b.x);
        assert!((b.y - 320.0).abs() < 0.01, "end y: {}", b.y);
        assert!((b.zoom - 2.0 / EditorCore::CAMERA_PRESET_ZOOM_FACTOR).abs() < 1e-6);
    }

    #[test]
    fn camera_arc_keeps_the_focus_distance() {
        use crate::model::CameraEase;
        let mut core = preset_core(60, true);
        // Offset the camera from the demo figure's root (480, 470).
        core.project.set_camera_key(0, 280.0, 470.0, 1.0);
        core.camera_arc(1, 24, CameraEase::Smooth);
        let b = key_at(&core, 24);
        let before = ((280.0f32 - 480.0).powi(2) + 0.0f32.powi(2)).sqrt();
        let after = ((b.x - 480.0).powi(2) + (b.y - 470.0).powi(2)).sqrt();
        assert!((before - after).abs() < 0.5, "radius kept: {before} vs {after}");
        assert!((b.x - 280.0).abs() > 1.0, "the centre actually swung");
        // Dead-centre: no radius, no arc, no snapshot.
        let mut centred = preset_core(60, true);
        centred.project.set_camera_key(0, 480.0, 470.0, 1.0);
        let depth = centred.undo_depth();
        centred.camera_arc(1, 24, CameraEase::Smooth);
        assert!(centred.status.contains("already centred"), "status: {}", centred.status);
        assert_eq!(centred.undo_depth(), depth);
    }

    #[test]
    fn camera_shake_is_deterministic_bounded_and_one_step() {
        let mut core = preset_core(60, true);
        let depth = core.undo_depth();
        core.camera_shake(24, 6.0);
        assert_eq!(core.undo_depth(), depth + 1, "the whole burst is one undo step");
        assert_eq!(core.project.camera.len(), 13, "keys every 2 frames over 24, plus the start");
        let a = key_at(&core, 0);
        let z = key_at(&core, 24);
        assert!((a.x - 480.0).abs() < 1e-6 && (a.y - 360.0).abs() < 1e-6, "starts at rest");
        assert!((z.x - 480.0).abs() < 1e-6 && (z.y - 360.0).abs() < 1e-6, "lands at rest");
        for k in &core.project.camera {
            assert!((k.x - 480.0).abs() <= 6.0 && (k.y - 360.0).abs() <= 6.0, "bounded by amp");
            assert!((k.zoom - 1.0).abs() < 1e-6, "a shake never zooms");
            assert_eq!(k.easing, crate::model::CameraEase::Linear);
        }
        // Deterministic: the same press at the same frame
        // reproduces the same keys (closed form, seeded by
        // the start frame).
        let first: Vec<(f32, f32)> = core.project.camera.iter().map(|k| (k.x, k.y)).collect();
        core.camera_shake(24, 6.0);
        let second: Vec<(f32, f32)> = core.project.camera.iter().map(|k| (k.x, k.y)).collect();
        assert_eq!(first, second, "re-pressing replaces, never stacks");
        // The offset curve itself: rest at both ends, live
        // in between, and seed-sensitive.
        assert_eq!(EditorCore::camera_shake_offset(3, 0, 24, 6.0), (0.0, 0.0));
        assert_eq!(EditorCore::camera_shake_offset(3, 24, 24, 6.0), (0.0, 0.0));
        let mid = EditorCore::camera_shake_offset(3, 6, 24, 6.0);
        assert!(mid.0.abs() + mid.1.abs() > 0.1, "the middle of the burst moves: {mid:?}");
        assert_ne!(
            EditorCore::camera_shake_offset(3, 6, 24, 6.0),
            EditorCore::camera_shake_offset(9, 6, 24, 6.0),
            "a different start frame seeds a different shake"
        );
    }

    #[test]
    fn camera_move_preset_tool_dispatches_and_refuses() {
        let mut core = preset_core(60, true);
        let out = tool_camera_move_preset(
            &mut core,
            &json!({ "preset": "push-in", "frames": 12 }),
        )
        .unwrap();
        assert_eq!(out["preset"], "push-in");
        let start = key_at(&core, 0);
        assert_eq!(
            start.easing,
            crate::model::CameraEase::Smooth,
            "the tool's default easing is the panel's Smooth"
        );
        assert!((key_at(&core, 12).zoom - EditorCore::CAMERA_PRESET_ZOOM_FACTOR).abs() < 1e-6);
        // Omitted frames = the standing default.
        let out = tool_camera_move_preset(&mut core, &json!({ "preset": "shake" })).unwrap();
        assert_eq!(out["frames"], EditorCore::CAMERA_PRESET_DEFAULT_FRAMES);
        // Refusals are errors and spend nothing.
        let depth = core.undo_depth();
        assert!(tool_camera_move_preset(&mut core, &json!({ "preset": "orbit" })).is_err());
        assert!(tool_camera_move_preset(
            &mut core,
            &json!({ "preset": "pan-left", "easing": "bounce" })
        )
        .is_err());
        assert!(tool_camera_move_preset(
            &mut core,
            &json!({ "preset": "pan-left", "frames": 99 })
        )
        .is_err());
        assert_eq!(core.undo_depth(), depth, "tool refusals take no snapshot");
    }

    #[test]
    fn onion_skin_set_is_session_only_and_reads_back() {
        let mut core = clean_core();
        let edits = core.edits_since_autosave();
        let out = tool_set_onion_skin(&mut core, &json!({ "enabled": false })).unwrap();
        assert_eq!(out["enabled"], false);
        assert!(!core.onion);
        assert_eq!(core.undo_depth(), 0, "no undo step");
        assert_eq!(core.edits_since_autosave(), edits, "no dirty mark");
        let out = tool_get_onion_skin(&mut core, &json!({})).unwrap();
        assert_eq!(out["enabled"], false);
        assert_eq!(out["legacy_prev_frames"], core.onion_frames);
        assert!(tool_set_onion_skin(&mut core, &json!({ "enabled": 1 })).is_err());
    }

    #[test]
    fn get_editor_settings_reports_the_whole_surface() {
        let mut core = clean_core();
        core.project.weather = Some(eds_kind_preset(WeatherKind::Storm));
        core.set_loop_range(0, 0);
        let out = tool_get_editor_settings(&mut core, &json!({})).unwrap();
        assert_eq!(out["weather"]["kind"], "storm");
        assert_eq!(out["weather"]["amount"], 140.0);
        assert_eq!(out["easing"], core.project.easing.label());
        assert_eq!(out["tween_playback"], core.project.tween_playback);
        assert_eq!(out["loop_range"], json!([0, 0]));
        assert_eq!(out["onion_enabled"], core.onion);
        assert_eq!(out["background"], core.project.background);
    }
}

// ---------------------------------------------------------------------
// Roadmap Wave 2 (W2-15): title-card settings ops — the
// EditorCore half of chapter title cards. The settings
// data, the card windows/layout and the burn-in
// composite live in `title_cards.rs`; these are the two
// editing ops over `Project::title_cards`, in the house
// op style the chapter ops established:
//
// * A no-change call (the switch already in the asked
//   state, the duration already the sanitised asked
//   value) reports, returns false and takes NO snapshot
//   (the chapter-move no-change discipline).
// * A successful change snapshots exactly once (one
//   undo step, labelled for the History panel), then
//   writes, and reports on `status`.
// * A non-finite duration is refused in the failure
//   voice ("… failed: …"), changing nothing — the
//   subtitle-style rule. A finite out-of-range duration
//   is clamped, not refused (the same rule).
// ---------------------------------------------------------------------

impl EditorCore {
    /// The project's title-card settings, as stored.
    pub fn title_card_settings(&self) -> crate::title_cards::TitleCardSettings {
        self.project.title_cards
    }

    /// Turn chapter title cards on or off for exports.
    /// One undo step on a real change. Returns false —
    /// changing nothing and taking no snapshot — when
    /// the switch already reads `enabled`.
    pub fn title_cards_set_enabled(&mut self, enabled: bool) -> bool {
        if self.project.title_cards.enabled == enabled {
            self.status = if enabled {
                "Title cards are already on.".to_string()
            } else {
                "Title cards are already off.".to_string()
            };
            return false;
        }
        self.snapshot_labeled(if enabled {
            "Enable title cards"
        } else {
            "Disable title cards"
        });
        self.project.title_cards.enabled = enabled;
        self.status = if enabled {
            "Title cards on: each chapter opens with its card.".to_string()
        } else {
            "Title cards off.".to_string()
        };
        true
    }

    /// Set how long each chapter's title card holds, in
    /// seconds (converted through the project fps at
    /// read, so the on-screen length survives a
    /// retime). The stored value is the sanitised one:
    /// a finite value clamps into the kept range, a
    /// non-finite one refuses — changing nothing, no
    /// snapshot. One undo step on a real change.
    pub fn title_cards_set_duration(&mut self, seconds: f32) -> bool {
        if !seconds.is_finite() {
            self.status = "Title card duration failed: duration must be a number.".to_string();
            return false;
        }
        let next = crate::title_cards::TitleCardSettings {
            duration_seconds: seconds,
            ..self.project.title_cards
        }
        .sanitised();
        if next == self.project.title_cards {
            self.status = format!(
                "Title card duration is already {} s.",
                next.duration_seconds
            );
            return false;
        }
        self.snapshot_labeled("Set title card duration");
        self.project.title_cards = next;
        self.status = format!("Title card duration: {} s.", next.duration_seconds);
        true
    }
}

#[cfg(test)]
mod title_card_ops_tests {
    use super::*;

    fn core_with_chapters() -> EditorCore {
        let mut core = EditorCore::new();
        core.chapter_add(24, 96, "The Deep Forest");
        core.chapter_add(96, 240, "Finale");
        core
    }

    #[test]
    fn enable_and_disable_each_take_exactly_one_undo_step() {
        let mut core = core_with_chapters();
        let depth = core.undo_depth();
        assert!(core.title_cards_set_enabled(true));
        assert_eq!(core.undo_depth(), depth + 1);
        assert!(core.project.title_cards.enabled);
        assert!(core.title_cards_set_enabled(false));
        assert_eq!(core.undo_depth(), depth + 2);
        assert!(!core.project.title_cards.enabled);
        // Undo restores the pre-toggle settings verbatim.
        core.undo();
        assert!(core.project.title_cards.enabled);
    }

    #[test]
    fn no_change_calls_spend_nothing() {
        let mut core = core_with_chapters();
        let depth = core.undo_depth();
        assert!(!core.title_cards_set_enabled(false), "already off");
        assert!(!core.title_cards_set_duration(
            crate::title_cards::TITLE_CARD_DEFAULT_SECONDS
        ));
        assert_eq!(core.undo_depth(), depth);
    }

    #[test]
    fn duration_clamps_and_a_nonfinite_refuses() {
        let mut core = core_with_chapters();
        assert!(core.title_cards_set_duration(100.0));
        assert_eq!(
            core.project.title_cards.duration_seconds,
            crate::title_cards::TITLE_CARD_MAX_SECONDS
        );
        let depth = core.undo_depth();
        assert!(!core.title_cards_set_duration(f32::NAN));
        assert_eq!(core.undo_depth(), depth, "a refusal spends nothing");
        assert!(core.status.contains("failed"), "status: {}", core.status);
    }
}

// ---------------------------------------------------------------------
// Session-read MCP handlers (Wave 14, A8)
//
// One editor-session fact the MCP surface could not see
// entering this wave: what the frame clipboards currently
// hold (the copy/paste tools are built-ins in `mcp.rs`
// and report only through the status line, so a client
// could not tell whether a paste would succeed before
// spending the call). The handler is read-only — no undo
// step, no status change. (The onion-skin read/write
// pair was A9's settings remit and landed in A9's own
// editor.rs section: `tool_get_onion_skin` /
// `tool_set_onion_skin` — A8's duplicate read was
// removed in favour of A9's, which also reports the
// legacy-frame-count caveat.)
// ---------------------------------------------------------------------

// TOOL ENTRY SPEC: add to tools() in src/mcp_tools.rs,
// in the Wave 14 domain-file block.
//   Tool {
//       name: "get_clipboard_state",
//       description: "Read what the frame clipboards hold: 'frame' is null when the single-frame clipboard is empty, else the copied frame's figure count and duration; 'range_frames' is the range clipboard's frame count. A paste succeeds only against a non-empty matching clipboard, and refuses when the rig changed since the copy. Read-only; no undo step.",
//       schema: obj_schema(json!({}), &[]),
//       handler: crate::editor::tool_clipboard_state,
//   }
// MCP.md row:
//   | `get_clipboard_state` | Read what the frame clipboards hold: 'frame' is null when the single-frame clipboard is empty, else the copied frame's figure count and duration; 'range_frames' is the range clipboard's frame count. A paste refuses when the clipboard is empty or the rig changed since the copy. Read-only; no undo step. | — | — |
/// MCP handler: report the frame clipboards' contents —
/// the single-frame clipboard as an object (figure count
/// + duration) or null, and the range clipboard's frame
/// count. Read-only; no undo step.
pub fn tool_clipboard_state(
    core: &mut EditorCore,
    _args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let frame = core.clipboard.as_ref().map(|f| {
        serde_json::json!({
            "figures": f.poses.len(),
            "duration": f.duration,
        })
    });
    Ok(serde_json::json!({
        "ok": true,
        "frame": frame,
        "range_frames": core.frame_clipboard.len(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    // --------------------------------------------------
    // Reparent (Roadmap Wave 2, W2-23 — RM19 §4.1)
    // --------------------------------------------------

    /// World rest orientation of node `ni`, recomputed
    /// test-side straight from the parent chain (a walk,
    /// not the op's index-order composition), so the
    /// pins below check the op against an independent
    /// reading of the same definition.
    fn chain_world(def: &crate::model::FigureDef, ni: usize) -> f32 {
        let mut acc = 0.0f32;
        let mut cur = Some(ni);
        let mut first = true;
        while let Some(n) = cur {
            if first {
                // A node's own world orientation includes
                // its own rest angle; the root's is 0
                // (`evaluate` seeds it so, and the root's
                // rest angle is inert).
                first = false;
            }
            acc += def.nodes[n].rest_angle;
            cur = def.nodes[n].parent;
        }
        // The root contributed its inert rest angle; the
        // engine's world orientation for the root is 0.
        acc - def.nodes[0].rest_angle
    }

    #[test]
    fn reparent_refuses_cycles_and_preserves_world_rest() {
        // RM19 §8's named pin for §4.1.
        let mut core = EditorCore::new();
        let before: Vec<f32> = (0..11).map(|i| chain_world(&core.project.figures[0].def, i)).collect();
        // forearm L (4) onto the chest (1): world rest
        // re-solves — 302 − 180 = 122 — and the subtree
        // (just node 4) keeps its world orientation.
        assert!(core.reparent_node(4, 1));
        {
            let def = &core.project.figures[0].def;
            assert_eq!(def.nodes[4].parent, Some(1));
            assert!((def.nodes[4].rest_angle - 122.0).abs() < 1e-4);
            assert!((chain_world(def, 4) - before[4]).abs() < 1e-4);
        }
        assert_eq!(core.undo_depth(), 1);
        // Cycles refuse and spend nothing: onto itself,
        // and onto a descendant (head is not node 1's
        // descendant after the move above — node 2 is).
        assert!(!core.reparent_node(4, 4));
        assert_eq!(core.status, "That would make the segment its own ancestor.");
        assert!(!core.reparent_node(1, 2));
        assert_eq!(core.status, "That would make the segment its own ancestor.");
        // The root refuses; an out-of-range parent refuses.
        assert!(!core.reparent_node(0, 1));
        assert_eq!(core.status, "The root cannot be reparented.");
        assert!(!core.reparent_node(4, 999));
        assert_eq!(core.undo_depth(), 1, "refusals spend no snapshot");
        // Undo restores the pre-reparent def exactly.
        core.undo();
        let def = &core.project.figures[0].def;
        assert_eq!(def.nodes[4].parent, Some(3));
        assert!((def.nodes[4].rest_angle - 4.0).abs() < 1e-4);
    }

    #[test]
    fn reparent_keeps_the_whole_subtrees_world_rest() {
        let mut core = EditorCore::new();
        let before: Vec<f32> = (0..11).map(|i| chain_world(&core.project.figures[0].def, i)).collect();
        // upper-arm L (3, subtree 3+4) onto thigh R (9):
        // both nodes keep their world rest orientations,
        // lengths never move, and no other node's parent
        // or rest changes.
        assert!(core.reparent_node(3, 9));
        let def = &core.project.figures[0].def;
        for ni in 0..11 {
            assert!(
                (chain_world(def, ni) - before[ni]).abs() < 1e-4,
                "node {ni} kept its world rest orientation"
            );
        }
        assert_eq!(def.nodes[3].parent, Some(9));
        assert_eq!(def.nodes[4].parent, Some(3), "children follow untouched");
        assert!((def.nodes[3].length - 38.0).abs() < 1e-4);
        assert!((def.nodes[4].length - 34.0).abs() < 1e-4);
    }

    #[test]
    fn reparent_moves_no_indices_and_no_node_indexed_store() {
        let mut core = EditorCore::new();
        let names_before: Vec<String> = core.project.figures[0]
            .def
            .nodes
            .iter()
            .map(|n| n.name.clone())
            .collect();
        let angles_before: Vec<Vec<f32>> = core
            .project
            .frames
            .iter()
            .map(|f| f.poses.first().map(|p| p.angles.clone()).unwrap_or_default())
            .collect();
        assert!(core.reparent_node(7, 1));
        let def = &core.project.figures[0].def;
        let names_after: Vec<String> = def.nodes.iter().map(|n| n.name.clone()).collect();
        assert_eq!(names_before, names_after, "no index moved");
        let angles_after: Vec<Vec<f32>> = core
            .project
            .frames
            .iter()
            .map(|f| f.poses.first().map(|p| p.angles.clone()).unwrap_or_default())
            .collect();
        assert_eq!(angles_before, angles_after, "poses untouched — no remap needed");
    }

    #[test]
    fn legal_parents_exclude_self_and_subtree_in_tree_order() {
        let def = crate::model::FigureDef::humanoid();
        // Chest (1): its subtree is 1..=6; the legs and
        // the root remain, in the tree's pre-order.
        assert_eq!(EditorCore::legal_parents(&def, 1), vec![0, 7, 8, 9, 10]);
        // A leaf excludes only itself.
        assert_eq!(
            EditorCore::legal_parents(&def, 4),
            vec![0, 1, 2, 3, 5, 6, 7, 8, 9, 10]
        );
        // The root's subtree is the whole figure.
        assert!(EditorCore::legal_parents(&def, 0).is_empty());
        assert_eq!(EditorCore::next_legal_parent(&def, 0), None);
        // A stale index yields nothing, never a panic.
        assert!(EditorCore::legal_parents(&def, 999).is_empty());
    }

    #[test]
    fn cycle_reparent_steps_and_wraps_with_one_snapshot_each() {
        let mut core = EditorCore::new();
        core.selected_node = Some(4); // forearm L, parent 3
        // Candidates for 4: [0,1,2,3,5,6,7,8,9,10] —
        // after 3 comes 5.
        assert!(core.cycle_reparent_selected());
        assert_eq!(core.project.figures[0].def.nodes[4].parent, Some(5));
        assert_eq!(core.undo_depth(), 1);
        // Nine more cycles tour the rest of the list —
        // 6, 7, 8, 9, 10, wrapping to 0, then 1, 2 — and
        // land back on the starting parent (3): a full
        // tour of the ten candidates, ten snapshots total.
        for _ in 0..9 {
            assert!(core.cycle_reparent_selected());
        }
        assert_eq!(core.project.figures[0].def.nodes[4].parent, Some(3));
        assert_eq!(core.undo_depth(), 10, "one snapshot per reparent");
        // The tour preserved the world rest orientation
        // at every stop.
        let w = chain_world(&core.project.figures[0].def, 4);
        assert!((w - 302.0).abs() < 1e-3, "world rest held across the tour: {w}");
    }

    #[test]
    fn cycle_reparent_refusals_spend_nothing() {
        let mut core = EditorCore::new();
        // No selection.
        assert!(!core.cycle_reparent_selected());
        assert_eq!(core.status, "Select a segment first.");
        // The root.
        core.selected_node = Some(0);
        assert!(!core.cycle_reparent_selected());
        assert_eq!(core.status, "The root cannot be reparented.");
        // A locked figure refuses both doors.
        core.selected_node = Some(4);
        core.project.figures[0].locked = true;
        assert!(!core.cycle_reparent_selected());
        assert_eq!(
            core.status,
            "Figure is locked — unlock it in the figures list."
        );
        assert!(!core.reparent_node(4, 1));
        assert_eq!(core.undo_depth(), 0, "no refusal spent a snapshot");
    }

    // --------------------------------------------------
    // Structural ops (Roadmap Wave 2, W2-24 — RM19
    // §4.2 insert-between + §4.3 duplicate-subtree)
    // --------------------------------------------------

    /// Joint positions of the rest pose, computed
    /// test-side from the def alone: the root sits at
    /// the origin and each joint lands at its parent's
    /// position plus its segment, directed by the
    /// chain-composed world rest orientation
    /// (`chain_world`). An independent reading of the
    /// same definition `evaluate` uses, so the pins
    /// below check the ops against the def, not against
    /// the ops' own helpers.
    fn rest_positions(def: &crate::model::FigureDef) -> Vec<[f32; 2]> {
        let mut pos = vec![[0.0f32; 2]; def.nodes.len()];
        for (i, node) in def.nodes.iter().enumerate() {
            if let Some(p) = node.parent {
                let d = crate::model::dir(chain_world(def, i));
                pos[i] = [
                    pos[p][0] + d[0] * node.length,
                    pos[p][1] + d[1] * node.length,
                ];
            }
        }
        pos
    }

    #[test]
    fn insert_segment_between_preserves_the_rest_pose() {
        // RM19 §8's named pin for §4.2.
        let mut core = EditorCore::new();
        let before = rest_positions(&core.project.figures[0].def);
        // Plant a keyed angle on the segment being split
        // and a full bend row: the angle must stay on
        // node 4 (the segment the user keyed), the fresh
        // slots read as rest/straight.
        core.project.frames[0].poses[0].angles[4] = 30.0;
        core.project.frames[0].bends = vec![vec![0.25; 11]];
        // Split forearm L (4, length 34, rest 4°) under
        // upper-arm L (3).
        assert!(core.insert_segment_between(4));
        assert_eq!(core.undo_depth(), 1, "exactly one snapshot");
        {
            let def = &core.project.figures[0].def;
            assert_eq!(def.nodes.len(), 12);
            let n = &def.nodes[11];
            assert_eq!(n.parent, Some(3), "the new node takes ni's parent");
            assert!((n.length - 17.0).abs() < 1e-4, "half the length");
            assert!((n.rest_angle - 4.0).abs() < 1e-4, "ni's rest angle");
            let ni = &def.nodes[4];
            assert_eq!(ni.parent, Some(11), "ni hangs under the new node");
            assert!((ni.length - 17.0).abs() < 1e-4, "the other half");
            assert_eq!(ni.rest_angle, 0.0, "ni continues collinearly");
        }
        // Every original joint is exactly where it was;
        // the new joint sits at the old segment's
        // midpoint.
        let after = rest_positions(&core.project.figures[0].def);
        for i in 0..11 {
            assert!(
                (after[i][0] - before[i][0]).abs() < 1e-3
                    && (after[i][1] - before[i][1]).abs() < 1e-3,
                "joint {i} kept its rest position"
            );
        }
        let mid = [
            (before[3][0] + before[4][0]) / 2.0,
            (before[3][1] + before[4][1]) / 2.0,
        ];
        assert!(
            (after[11][0] - mid[0]).abs() < 1e-3 && (after[11][1] - mid[1]).abs() < 1e-3,
            "the new joint is the midpoint"
        );
        // Slot extension: existing values untouched, the
        // appended slots are rest (0.0 angle, 0.0 bend).
        let angles = &core.project.frames[0].poses[0].angles;
        assert_eq!(angles.len(), 12);
        assert_eq!(angles[4], 30.0, "the keyed angle stays on ni");
        assert_eq!(angles[11], 0.0);
        let bends = &core.project.frames[0].bends[0];
        assert_eq!(bends.len(), 12);
        assert_eq!(bends[4], 0.25);
        assert_eq!(bends[11], 0.0);
        // The created node is the selection (the
        // `add_segment` precedent).
        assert_eq!(core.selected_node, Some(11));
        // Undo restores the pre-split def exactly.
        core.undo();
        let def = &core.project.figures[0].def;
        assert_eq!(def.nodes.len(), 11);
        assert_eq!(def.nodes[4].parent, Some(3));
        assert!((def.nodes[4].length - 34.0).abs() < 1e-4);
    }

    #[test]
    fn insert_segment_between_refusals_spend_nothing() {
        let mut core = EditorCore::new();
        // The root has no segment above it to split.
        assert!(!core.insert_segment_between(0));
        assert_eq!(core.status, "The root has no segment to split.");
        // A stale index refuses.
        assert!(!core.insert_segment_between(999));
        assert_eq!(core.status, "Select a segment first.");
        // A locked figure refuses.
        core.project.figures[0].locked = true;
        assert!(!core.insert_segment_between(4));
        assert_eq!(
            core.status,
            "Figure is locked — unlock it in the figures list."
        );
        // No figure at all refuses.
        core.project.figures.clear();
        assert!(!core.insert_segment_between(4));
        assert_eq!(core.status, "Select a figure first.");
        assert_eq!(core.undo_depth(), 0, "no refusal spent a snapshot");
    }

    #[test]
    fn duplicate_subtree_appends_without_moving_existing_indices() {
        // RM19 §8's named pin for §4.3.
        let mut core = EditorCore::new();
        let before: Vec<(String, Option<usize>, f32, f32)> = core.project.figures[0]
            .def
            .nodes
            .iter()
            .map(|n| (n.name.clone(), n.parent, n.length, n.rest_angle))
            .collect();
        // Plant keyed angles inside the subtree, a full
        // bend row, and a graph channel keyed on node 4:
        // existing stores must come through untouched,
        // and the channel must NOT be cloned.
        core.project.frames[0].poses[0].angles[3] = 10.0;
        core.project.frames[0].poses[0].angles[4] = 20.0;
        core.project.frames[0].bends = vec![vec![0.5; 11]];
        core.project.channels.push(crate::graph::Channel {
            figure: 0,
            target: crate::graph::ChannelTarget::Angle(4),
            keys: vec![crate::graph::Key::flat(0.0, 42.0)],
        });
        // Duplicate upper-arm L (3): subtree {3, 4}.
        assert!(core.duplicate_subtree(3));
        assert_eq!(core.undo_depth(), 1, "exactly one snapshot");
        assert_eq!(core.status, "Subtree duplicated (2 segments).");
        {
            let def = &core.project.figures[0].def;
            assert_eq!(def.nodes.len(), 13);
            // The identity remap on the old figure: no
            // existing node moved or changed.
            for (i, b) in before.iter().enumerate() {
                let n = &def.nodes[i];
                assert_eq!(
                    &(n.name.clone(), n.parent, n.length, n.rest_angle),
                    b,
                    "existing node {i} untouched"
                );
            }
            // The clone, appended in pre-order: its root
            // is a sibling of the original (same parent —
            // the external reference is NOT remapped) and
            // carries the copy name; the descendant keeps
            // its name and hangs under the clone root.
            let root = &def.nodes[11];
            assert_eq!(root.name, format!("{} copy", before[3].0));
            assert_eq!(root.parent, before[3].1, "clone root is a sibling");
            assert_eq!(root.length, before[3].2);
            assert_eq!(root.rest_angle, before[3].3);
            let child = &def.nodes[12];
            assert_eq!(child.name, before[4].0, "descendants keep names");
            assert_eq!(child.parent, Some(11), "internal link remapped");
            assert_eq!(child.length, before[4].2);
        }
        // The clone starts at rest: existing angle/bend
        // values untouched, fresh slots 0.0.
        let angles = &core.project.frames[0].poses[0].angles;
        assert_eq!(angles.len(), 13);
        assert_eq!(angles[3], 10.0);
        assert_eq!(angles[4], 20.0);
        assert_eq!(angles[11], 0.0);
        assert_eq!(angles[12], 0.0);
        let bends = &core.project.frames[0].bends[0];
        assert_eq!(bends.len(), 13);
        assert_eq!(bends[11], 0.0);
        assert_eq!(bends[12], 0.0);
        // Channels are not cloned (§4.3): the one
        // channel still targets the ORIGINAL node 4.
        assert_eq!(core.project.channels.len(), 1);
        assert_eq!(
            core.project.channels[0].target,
            crate::graph::ChannelTarget::Angle(4)
        );
        // Selection moves to the clone root (§4.3).
        assert_eq!(core.selected_node, Some(11));
        // Undo restores the pre-duplicate def exactly.
        core.undo();
        assert_eq!(core.project.figures[0].def.nodes.len(), 11);
    }

    #[test]
    fn duplicate_subtree_refusals_spend_nothing_and_leaf_clones_one() {
        let mut core = EditorCore::new();
        // The root has no parent to anchor a sibling
        // clone on.
        assert!(!core.duplicate_subtree(0));
        assert_eq!(core.status, "The root cannot be duplicated.");
        // A stale index refuses.
        assert!(!core.duplicate_subtree(999));
        assert_eq!(core.status, "Select a segment first.");
        // A locked figure refuses.
        core.project.figures[0].locked = true;
        assert!(!core.duplicate_subtree(4));
        assert_eq!(
            core.status,
            "Figure is locked — unlock it in the figures list."
        );
        assert_eq!(core.undo_depth(), 0, "no refusal spent a snapshot");
        // A leaf duplicates as a one-node subtree.
        core.project.figures[0].locked = false;
        assert!(core.duplicate_subtree(2)); // head, parent chest (1)
        assert_eq!(core.status, "Subtree duplicated (1 segment).");
        let def = &core.project.figures[0].def;
        assert_eq!(def.nodes.len(), 12);
        assert_eq!(def.nodes[11].name, "head copy");
        assert_eq!(def.nodes[11].parent, Some(1));
        assert_eq!(core.selected_node, Some(11));
    }

    // --------------------------------------------------
    // Structural ops (Roadmap Wave 2, W2-25 — RM19
    // §4.4 delete-subtree + §4.5 mirror-subtree, over
    // the §4.0 shared remap helper)
    // --------------------------------------------------

    #[test]
    fn delete_subtree_remaps_bends_and_channels() {
        // RM19 §8's named pin for §4.4, extended to the
        // full §4.0 table. The fixture: the demo
        // humanoid, posed/channelled/stated, deleting
        // the chest subtree {1,2,3,4,5,6} — a mid-tree
        // subtree. Survivors: 0 (hips), 7..10 (legs),
        // which become 0..4.
        let mut core = EditorCore::new();
        // Posed, distinct per node, so a mis-remap
        // cannot hide behind equal values.
        for (i, a) in core.project.frames[0].poses[0].angles.iter_mut().enumerate() {
            *a = i as f32;
        }
        core.project.frames[0].bends = vec![(0..11).map(|i| i as f32 * 0.1).collect()];
        // A short/stale frame: rows cut to 5 slots.
        // Survivors past the cut rebuild as 0.0 (the
        // rows' neutral value).
        core.project.frames[1].poses[0].angles.truncate(5);
        core.project.frames[1].bends = vec![vec![0.7; 5]];
        // Channels: one into the subtree (dropped),
        // Angle/Length/Bend past the hole (remapped),
        // a RootY (not node-indexed), and one on a
        // SECOND figure past its own hole (untouched —
        // the remap is per-figure).
        core.project.channels = vec![
            crate::graph::Channel {
                figure: 0,
                target: crate::graph::ChannelTarget::Angle(4),
                keys: vec![crate::graph::Key::flat(0.0, 1.0)],
            },
            crate::graph::Channel {
                figure: 0,
                target: crate::graph::ChannelTarget::Angle(9),
                keys: vec![crate::graph::Key::flat(0.0, 2.0)],
            },
            crate::graph::Channel {
                figure: 0,
                target: crate::graph::ChannelTarget::Length(8),
                keys: vec![crate::graph::Key::flat(0.0, 3.0)],
            },
            crate::graph::Channel {
                figure: 0,
                target: crate::graph::ChannelTarget::Bend(10),
                keys: vec![crate::graph::Key::flat(0.0, 4.0)],
            },
            crate::graph::Channel {
                figure: 0,
                target: crate::graph::ChannelTarget::RootY,
                keys: vec![crate::graph::Key::flat(0.0, 5.0)],
            },
            crate::graph::Channel {
                figure: 1,
                target: crate::graph::ChannelTarget::Angle(9),
                keys: vec![crate::graph::Key::flat(0.0, 6.0)],
            },
        ];
        // "Stated": a state key rides the figure —
        // frame→clip addressed, no node index, so it
        // must pass through byte-identical.
        core.project.figures[0].states = vec![crate::model::StateKey {
            frame: 2,
            clip: 0,
            offset: 1,
            speed: 1.5,
        }];
        // A second figure whose def must not move.
        core.project.figures.push(crate::model::FigureInstance {
            name: "Beta".into(),
            color: 0xFFFFFF,
            def: crate::model::FigureDef::humanoid(),
            hidden: false,
            clip: None,
            states: Vec::new(),
            locked: false,
            opacity: 1.0,
        });
        // Segment media on a survivor (thigh R) and
        // inside the subtree (forearm L).
        core.project.figures[0].def.nodes[9].media =
            Some(crate::model::SegmentMedia::Text { content: "R".into(), size: 12.0 });
        core.project.figures[0].def.nodes[4].media =
            Some(crate::model::SegmentMedia::Text { content: "L".into(), size: 12.0 });
        // The selection sits on a survivor past the
        // hole: it must follow its node (9 → 3).
        core.selected_node = Some(9);

        assert!(core.delete_subtree(1));
        assert_eq!(core.undo_depth(), 1, "exactly one snapshot");
        assert_eq!(
            core.status,
            "Subtree deleted (6 segments, 1 graph channel removed)."
        );
        {
            let def = &core.project.figures[0].def;
            assert_eq!(def.nodes.len(), 5);
            let names: Vec<&str> = def.nodes.iter().map(|n| n.name.as_str()).collect();
            assert_eq!(names, ["hips", "thigh L", "shin L", "thigh R", "shin R"]);
            // Parents: the legs hung on the root and
            // still do; the shins follow their thighs
            // to the remapped indices.
            assert_eq!(def.nodes[0].parent, None);
            assert_eq!(def.nodes[1].parent, Some(0));
            assert_eq!(def.nodes[2].parent, Some(1));
            assert_eq!(def.nodes[3].parent, Some(0));
            assert_eq!(def.nodes[4].parent, Some(3));
            // Media rides its segment: thigh R's label
            // is at its new index; the deleted label is
            // gone with its node.
            assert!(matches!(
                &def.nodes[3].media,
                Some(crate::model::SegmentMedia::Text { content, .. }) if content == "R"
            ));
            assert!(def.nodes.iter().all(|n| !matches!(
                &n.media,
                Some(crate::model::SegmentMedia::Text { content, .. }) if content == "L"
            )));
        }
        // Angles and bends: the survivors' distinct
        // values, at their new positions.
        assert_eq!(
            core.project.frames[0].poses[0].angles,
            vec![0.0, 7.0, 8.0, 9.0, 10.0]
        );
        let bends = &core.project.frames[0].bends[0];
        assert_eq!(bends.len(), 5);
        for (new, old) in [(0usize, 0usize), (1, 7), (2, 8), (3, 9), (4, 10)] {
            assert!(
                (bends[new] - old as f32 * 0.1).abs() < 1e-6,
                "bend slot {new} holds old node {old}'s value"
            );
        }
        // The short frame: survivors 0..=4 were inside
        // the 5 slots; the legs were past the cut and
        // rebuild as the neutral 0.0.
        assert_eq!(
            core.project.frames[1].poses[0].angles,
            vec![0.0, 0.0, 0.0, 0.0, 0.0]
        );
        assert_eq!(core.project.frames[1].bends[0], vec![0.7, 0.0, 0.0, 0.0, 0.0]);
        // Channels: the into-subtree one is gone; the
        // rest of figure 0's remapped (9→3, 8→2, 10→4),
        // RootY untouched, figure 1's untouched.
        let targets: Vec<crate::graph::ChannelTarget> =
            core.project.channels.iter().map(|c| c.target).collect();
        assert_eq!(
            targets,
            vec![
                crate::graph::ChannelTarget::Angle(3),
                crate::graph::ChannelTarget::Length(2),
                crate::graph::ChannelTarget::Bend(4),
                crate::graph::ChannelTarget::RootY,
                crate::graph::ChannelTarget::Angle(9),
            ]
        );
        assert_eq!(core.project.channels[4].figure, 1);
        // States untouched; the second figure's def
        // untouched; the selection followed its node.
        assert_eq!(core.project.figures[0].states.len(), 1);
        assert_eq!(core.project.figures[0].states[0].frame, 2);
        assert_eq!(core.project.figures[1].def.nodes.len(), 11);
        assert_eq!(core.selected_node, Some(3));
        // Undo restores the whole pre-delete figure.
        core.undo();
        assert_eq!(core.project.figures[0].def.nodes.len(), 11);
        assert_eq!(core.project.channels.len(), 6);
        assert_eq!(core.project.frames[0].poses[0].angles.len(), 11);
    }

    #[test]
    fn delete_subtree_leaf_clears_an_inside_selection() {
        let mut core = EditorCore::new();
        for (i, a) in core.project.frames[0].poses[0].angles.iter_mut().enumerate() {
            *a = i as f32;
        }
        core.project.channels.push(crate::graph::Channel {
            figure: 0,
            target: crate::graph::ChannelTarget::Angle(10),
            keys: vec![crate::graph::Key::flat(0.0, 5.0)],
        });
        core.selected_node = Some(8);
        // Delete shin L (8) — a leaf: the selection
        // was inside the subtree, so it clears.
        assert!(core.delete_subtree(8));
        assert_eq!(core.status, "Subtree deleted (1 segment).");
        assert_eq!(core.selected_node, None);
        let def = &core.project.figures[0].def;
        assert_eq!(def.nodes.len(), 10);
        assert_eq!(def.nodes[8].name, "thigh R");
        assert_eq!(def.nodes[8].parent, Some(0));
        assert_eq!(def.nodes[9].name, "shin R");
        assert_eq!(def.nodes[9].parent, Some(8));
        // The channel past the hole followed its node.
        assert_eq!(
            core.project.channels[0].target,
            crate::graph::ChannelTarget::Angle(9)
        );
        assert_eq!(
            core.project.frames[0].poses[0].angles,
            vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 9.0, 10.0]
        );
        assert_eq!(core.undo_depth(), 1);
    }

    #[test]
    fn delete_subtree_refusals_spend_nothing() {
        let mut core = EditorCore::new();
        // The root: deleting it would clear the whole
        // figure — refused.
        assert!(!core.delete_subtree(0));
        assert_eq!(core.status, "The root cannot be deleted.");
        // A stale index refuses.
        assert!(!core.delete_subtree(999));
        assert_eq!(core.status, "Select a segment first.");
        // A locked figure refuses.
        core.project.figures[0].locked = true;
        assert!(!core.delete_subtree(1));
        assert_eq!(
            core.status,
            "Figure is locked — unlock it in the figures list."
        );
        // No figure at all refuses.
        core.project.figures.clear();
        assert!(!core.delete_subtree(1));
        assert_eq!(core.status, "Select a figure first.");
        assert_eq!(core.undo_depth(), 0, "no refusal spent a snapshot");
    }

    #[test]
    fn remap_node_indices_rebuilds_rows_and_drops_dangling_channels() {
        // The shared §4.0 helper, exercised directly:
        // a map removing only node 4.
        let mut core = EditorCore::new();
        for (i, a) in core.project.frames[0].poses[0].angles.iter_mut().enumerate() {
            *a = i as f32;
        }
        core.project.frames[0].bends = vec![(0..11).map(|i| i as f32).collect()];
        core.project.channels = vec![
            crate::graph::Channel {
                figure: 0,
                target: crate::graph::ChannelTarget::Angle(4),
                keys: vec![crate::graph::Key::flat(0.0, 1.0)],
            },
            crate::graph::Channel {
                figure: 0,
                target: crate::graph::ChannelTarget::Angle(9),
                keys: vec![crate::graph::Key::flat(0.0, 2.0)],
            },
            // A stale target past the def entirely:
            // dropped like a removed node's, never left
            // dangling.
            crate::graph::Channel {
                figure: 0,
                target: crate::graph::ChannelTarget::Angle(999),
                keys: vec![crate::graph::Key::flat(0.0, 3.0)],
            },
        ];
        let mut map: Vec<Option<usize>> = (0..11).map(Some).collect();
        map[4] = None;
        let mut next = 0usize;
        for m in map.iter_mut() {
            if m.is_some() {
                *m = Some(next);
                next += 1;
            }
        }
        let dropped = EditorCore::remap_node_indices(&mut core.project, 0, &map);
        assert_eq!(dropped, 2, "the removed node's channel and the stale one");
        assert_eq!(
            core.project.frames[0].poses[0].angles,
            vec![0.0, 1.0, 2.0, 3.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0]
        );
        assert_eq!(
            core.project.frames[0].bends[0],
            vec![0.0, 1.0, 2.0, 3.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0]
        );
        assert_eq!(core.project.channels.len(), 1);
        assert_eq!(
            core.project.channels[0].target,
            crate::graph::ChannelTarget::Angle(8)
        );
        // The helper never touches the def itself —
        // the physical removal is the caller's half of
        // the contract.
        assert_eq!(core.project.figures[0].def.nodes.len(), 11);
    }

    #[test]
    fn mirror_subtree_flips_bends_angles_and_names() {
        // RM19 §8's named pin for §4.5.
        let mut core = EditorCore::new();
        // Planted state: a keyed angle inside the
        // subtree and one outside, a bend row, an
        // Angle channel (mirrored), a Length channel on
        // the same node (untouched), an Angle channel
        // outside (untouched), image media inside.
        core.project.frames[0].poses[0].angles[3] = 25.0;
        core.project.frames[0].poses[0].angles[7] = 33.0;
        core.project.frames[0].bends = vec![vec![
            0.0, 0.0, 0.0, 0.4, 0.0, 0.0, 0.0, 0.2, 0.0, 0.0, 0.0,
        ]];
        core.project.channels = vec![
            crate::graph::Channel {
                figure: 0,
                target: crate::graph::ChannelTarget::Angle(3),
                keys: vec![crate::graph::Key {
                    frame: 0.0,
                    value: 30.0,
                    in_tan: 2.0,
                    out_tan: -3.0,
                    interp: crate::graph::Interp::Hermite,
                }],
            },
            crate::graph::Channel {
                figure: 0,
                target: crate::graph::ChannelTarget::Length(3),
                keys: vec![crate::graph::Key::flat(0.0, 38.0)],
            },
            crate::graph::Channel {
                figure: 0,
                target: crate::graph::ChannelTarget::Angle(7),
                keys: vec![crate::graph::Key::flat(0.0, 11.0)],
            },
        ];
        core.project.figures[0].def.nodes[4].media =
            Some(crate::model::SegmentMedia::Image {
                asset: 0,
                offset: [0.0, 0.0],
                scale: 1.0,
                angle_offset: 0.0,
                flip: false,
                opacity: 1.0,
            });
        // A joint limit inside the subtree: the arc
        // reflects with the joint (min −10/max 40
        // becomes min −40/max 10).
        core.project.figures[0].def.nodes[3].min_angle = Some(-10.0);
        core.project.figures[0].def.nodes[3].max_angle = Some(40.0);

        // Mirror the chest subtree {1,2,3,4,5,6}.
        assert!(core.mirror_subtree(1));
        assert_eq!(core.undo_depth(), 1, "exactly one snapshot");
        assert_eq!(core.status, "Subtree mirrored (6 segments).");
        {
            let def = &core.project.figures[0].def;
            // Names swap token-exactly; unmatched names
            // (chest, head) are left alone.
            assert_eq!(def.nodes[1].name, "chest");
            assert_eq!(def.nodes[2].name, "head");
            assert_eq!(def.nodes[3].name, "upper-arm R");
            assert_eq!(def.nodes[4].name, "forearm R");
            assert_eq!(def.nodes[5].name, "upper-arm L");
            assert_eq!(def.nodes[6].name, "forearm L");
            // Rests negate (normalised): 180 → −180,
            // 118 → −118, 4 → −4, −118 → 118, −4 → 4.
            assert_eq!(def.nodes[1].rest_angle, -180.0);
            assert_eq!(def.nodes[3].rest_angle, -118.0);
            assert_eq!(def.nodes[4].rest_angle, -4.0);
            assert_eq!(def.nodes[5].rest_angle, 118.0);
            assert_eq!(def.nodes[6].rest_angle, 4.0);
            // The def bend-side sign flips with the
            // limb; nodes outside keep theirs.
            assert_eq!(def.nodes[4].bend, 1.0);
            assert_eq!(def.nodes[6].bend, 1.0);
            assert_eq!(def.nodes[8].bend, 1.0, "outside the subtree: untouched");
            // The limit arc reflected.
            assert_eq!(def.nodes[3].min_angle, Some(-40.0));
            assert_eq!(def.nodes[3].max_angle, Some(10.0));
            // Other def fields ride unchanged.
            assert!(def.nodes[2].head);
            assert!(def.nodes[4].ik_end);
            // The image media's flip toggled.
            assert!(matches!(
                &def.nodes[4].media,
                Some(crate::model::SegmentMedia::Image { flip: true, .. })
            ));
        }
        // Posed angles and frame bends negate inside
        // the subtree only.
        assert_eq!(core.project.frames[0].poses[0].angles[3], -25.0);
        assert_eq!(core.project.frames[0].poses[0].angles[7], 33.0);
        assert_eq!(core.project.frames[0].bends[0][3], -0.4);
        assert_eq!(core.project.frames[0].bends[0][7], 0.2);
        // The Angle channel's curve mirrored (value
        // and tangents); Length and outside channels
        // untouched.
        let key = &core.project.channels[0].keys[0];
        assert_eq!(key.value, -30.0);
        assert_eq!(key.in_tan, -2.0);
        assert_eq!(key.out_tan, 3.0);
        assert_eq!(core.project.channels[1].keys[0].value, 38.0);
        assert_eq!(core.project.channels[2].keys[0].value, 11.0);
        // Mirroring again restores the original rig:
        // the op is an involution on this data.
        assert!(core.mirror_subtree(1));
        let def = &core.project.figures[0].def;
        assert_eq!(def.nodes[3].name, "upper-arm L");
        assert_eq!(def.nodes[3].rest_angle, 118.0);
        assert_eq!(def.nodes[4].bend, -1.0);
        assert!(matches!(
            &def.nodes[4].media,
            Some(crate::model::SegmentMedia::Image { flip: false, .. })
        ));
        assert_eq!(core.project.frames[0].poses[0].angles[3], 25.0);
        // Undo steps back over the second mirror only.
        core.undo();
        assert_eq!(core.project.figures[0].def.nodes[3].name, "upper-arm R");
    }

    #[test]
    fn mirror_subtree_leaf_and_refusals() {
        let mut core = EditorCore::new();
        // A leaf mirrors as a one-node subtree.
        assert!(core.mirror_subtree(8)); // shin L
        assert_eq!(core.status, "Subtree mirrored (1 segment).");
        let def = &core.project.figures[0].def;
        assert_eq!(def.nodes[8].name, "shin R");
        assert_eq!(def.nodes[8].rest_angle, -5.0);
        assert_eq!(def.nodes[8].bend, -1.0);
        assert_eq!(core.undo_depth(), 1);
        // Refusals spend nothing: the root (the
        // figure's mirror axis), a stale index, a
        // locked figure, no figure.
        assert!(!core.mirror_subtree(0));
        assert_eq!(core.status, "The root cannot be mirrored.");
        assert!(!core.mirror_subtree(999));
        assert_eq!(core.status, "Select a segment first.");
        core.project.figures[0].locked = true;
        assert!(!core.mirror_subtree(7));
        assert_eq!(
            core.status,
            "Figure is locked — unlock it in the figures list."
        );
        core.project.figures.clear();
        assert!(!core.mirror_subtree(7));
        assert_eq!(core.status, "Select a figure first.");
        assert_eq!(core.undo_depth(), 1, "no refusal spent a snapshot");
    }

    #[test]
    fn mirror_node_name_swaps_tokens_only() {
        // Whole-token and suffix swaps; a name that
        // merely CONTAINS a side word never changes.
        assert_eq!(EditorCore::mirror_node_name("upper-arm L"), "upper-arm R");
        assert_eq!(EditorCore::mirror_node_name("forearm R"), "forearm L");
        assert_eq!(EditorCore::mirror_node_name("hand left"), "hand right");
        assert_eq!(EditorCore::mirror_node_name("Left foot"), "Right foot");
        assert_eq!(EditorCore::mirror_node_name("arm_l"), "arm_r");
        assert_eq!(EditorCore::mirror_node_name("arm_R"), "arm_L");
        assert_eq!(EditorCore::mirror_node_name("leg-l"), "leg-r");
        assert_eq!(EditorCore::mirror_node_name("elbow"), "elbow");
        assert_eq!(EditorCore::mirror_node_name("flare"), "flare");
        assert_eq!(EditorCore::mirror_node_name("chest"), "chest");
        assert_eq!(EditorCore::mirror_node_name("segment 4"), "segment 4");
    }

    #[test]
    fn node_name_setter_trims_writes_and_spends_once() {
        // W2-26 (RM19 §5): the Node name setter trims,
        // writes under exactly one "Rename segment"
        // snapshot, and spends nothing on a no-change
        // rename or an empty name.
        let mut core = EditorCore::new();
        assert!(core.set_node_name(0, 3, "  bicep L  "));
        assert_eq!(core.project.figures[0].def.nodes[3].name, "bicep L");
        assert_eq!(core.status, "Segment 3 renamed to \"bicep L\".");
        assert_eq!(core.undo_depth(), 1);
        assert_eq!(core.undo_label(), Some("Rename segment"));
        // The same name again: success, no second step.
        assert!(core.set_node_name(0, 3, "bicep L"));
        assert_eq!(core.undo_depth(), 1, "no-change rename spends nothing");
        // Empty after trimming refuses and spends nothing.
        assert!(!core.set_node_name(0, 3, "   "));
        assert_eq!(core.status, "Segment name cannot be empty.");
        assert_eq!(core.project.figures[0].def.nodes[3].name, "bicep L");
        assert_eq!(core.undo_depth(), 1);
        // Undo restores the constructor's name.
        core.undo();
        assert_eq!(core.project.figures[0].def.nodes[3].name, "upper-arm L");
    }

    #[test]
    fn node_head_toggle_writes_and_spends_once() {
        // W2-26 (RM19 §5): the Head toggle writes the
        // NodeDef flag under one "Segment head" snapshot
        // per real change; multiple heads are legal data
        // and the toggle does not police them.
        let mut core = EditorCore::new();
        assert!(core.set_node_head(0, 3, true));
        assert!(core.project.figures[0].def.nodes[3].head);
        assert_eq!(core.status, "upper-arm L is the head.");
        assert_eq!(core.undo_depth(), 1);
        assert_eq!(core.undo_label(), Some("Segment head"));
        // Node 2 is already the constructor's head: the
        // figure now carries two heads, which is legal.
        assert!(core.project.figures[0].def.nodes[2].head);
        // Writing the carried value spends nothing.
        assert!(core.set_node_head(0, 3, true));
        assert_eq!(core.undo_depth(), 1);
        assert!(core.set_node_head(0, 3, false));
        assert!(!core.project.figures[0].def.nodes[3].head);
        assert_eq!(core.status, "upper-arm L is not the head.");
        assert_eq!(core.undo_depth(), 2);
    }

    #[test]
    fn ik_end_toggle_refuses_without_a_grandparent() {
        // RM19 §8's named pin for §5 (W2-26): turning IK
        // end ON requires a chain of two segments above
        // the joint — the chest (parent = the root) and
        // the root itself refuse with the standing
        // sentence and spend nothing.
        let mut core = EditorCore::new();
        assert!(!core.set_node_ik_end(0, 1, true));
        assert_eq!(
            core.status,
            "IK needs a chain of two segments above this joint."
        );
        assert!(!core.set_node_ik_end(0, 0, true));
        assert_eq!(
            core.status,
            "IK needs a chain of two segments above this joint."
        );
        assert_eq!(core.undo_depth(), 0, "refusals spend nothing");
        // upper-arm L (parent chest, grandparent hips)
        // qualifies: one "Segment IK end" step per change.
        assert!(core.set_node_ik_end(0, 3, true));
        assert!(core.project.figures[0].def.nodes[3].ik_end);
        assert_eq!(core.status, "upper-arm L is an IK end.");
        assert_eq!(core.undo_depth(), 1);
        assert_eq!(core.undo_label(), Some("Segment IK end"));
        assert!(core.set_node_ik_end(0, 3, true));
        assert_eq!(core.undo_depth(), 1, "no-change write spends nothing");
        // Turning OFF never needs the chain.
        assert!(core.set_node_ik_end(0, 3, false));
        assert!(!core.project.figures[0].def.nodes[3].ik_end);
        assert_eq!(core.undo_depth(), 2);
    }

    #[test]
    fn bend_side_setter_writes_the_sign() {
        // RM19 §8's named pin for §5 (W2-26): the setter
        // writes exactly ±1.0 (the field is a sign by
        // convention), under one "Segment bend side"
        // snapshot per real change, and never touches a
        // frame's stored bend.
        let mut core = EditorCore::new();
        assert_eq!(core.project.figures[0].def.nodes[3].bend, 1.0);
        assert!(core.set_node_bend_side(0, 3, -1.0));
        assert_eq!(core.project.figures[0].def.nodes[3].bend, -1.0);
        assert_eq!(core.status, "upper-arm L bend side set to -1.");
        assert_eq!(core.undo_depth(), 1);
        assert_eq!(core.undo_label(), Some("Segment bend side"));
        // Any negative sign lands on the same −1.0: no
        // second step. A zero or non-finite sign refuses.
        assert!(core.set_node_bend_side(0, 3, -0.25));
        assert_eq!(core.undo_depth(), 1);
        assert!(!core.set_node_bend_side(0, 3, 0.0));
        assert_eq!(core.status, "Bend side must be − or +.");
        assert!(!core.set_node_bend_side(0, 3, f32::NAN));
        assert_eq!(core.undo_depth(), 1);
        assert!(core.set_node_bend_side(0, 3, 1.0));
        assert_eq!(core.project.figures[0].def.nodes[3].bend, 1.0);
        assert_eq!(core.undo_depth(), 2);
        // The def write is not a frame-bend write.
        assert!(
            core.project.frames[core.current].bends.is_empty(),
            "no frame bend table was grown by the def setter"
        );
    }

    #[test]
    fn node_flag_setters_refuse_locked_and_stale() {
        // W2-26: the four setters share one refusal head
        // — no figure, no node, locked figure — and a
        // refusal never spends a snapshot.
        let mut core = EditorCore::new();
        assert!(!core.set_node_name(99, 0, "x"));
        assert_eq!(core.status, "Select a figure first.");
        assert!(!core.set_node_head(0, 999, true));
        assert_eq!(core.status, "Select a segment first.");
        core.project.figures[0].locked = true;
        assert!(!core.set_node_name(0, 3, "bicep L"));
        assert!(!core.set_node_head(0, 3, true));
        assert!(!core.set_node_ik_end(0, 3, true));
        assert!(!core.set_node_bend_side(0, 3, -1.0));
        assert_eq!(
            core.status,
            "Figure is locked — unlock it in the figures list."
        );
        assert_eq!(core.undo_depth(), 0, "locked refusals spend nothing");
        assert_eq!(core.project.figures[0].def.nodes[3].name, "upper-arm L");
        assert!(!core.project.figures[0].def.nodes[3].head);
    }

    #[test]
    fn autosave_counter_counts_snapshots_only() {
        // RENAMED in Wave 13 (B2) — see
        // `autosave_counter_counts_marked_edits_too` below:
        // this test's name and second half pinned the
        // pre-Wave-13 contract, under which the deliberate
        // non-snapshot mutators did not move the counter
        // at all. That invisibility was the Wave 12 E8
        // bug; the counter now counts marked edits too.
        // What survives of the old test is its first half:
        // snapshots count, one each.
        let mut core = EditorCore::new();
        assert_eq!(core.edits_since_autosave(), 0);
        core.snapshot();
        assert_eq!(core.edits_since_autosave(), 1);
        core.snapshot();
        core.snapshot();
        assert_eq!(core.edits_since_autosave(), 3);
        assert_eq!(core.undo_depth(), 3);
    }

    #[test]
    fn autosave_counter_counts_marked_edits_too() {
        // Wave 13 (B2): the deliberate non-snapshot
        // mutators (non-undoable appearance cyclers and
        // the layers family) now move the counter through
        // `mark_dirty` — one per real change — while the
        // undo stack stays exactly where the snapshots
        // left it.
        //
        // Roadmap Wave 1 (RM24, P-14): those mutators
        // snapshot now (coalesced), so the counter moves
        // the same way — one per real change — and the
        // undo stack grows with them; `mark_dirty` itself
        // remains for the one session-bookkeeping class
        // (see its doc and
        // `mark_dirty_advances_the_counter_without_an_undo_step`).
        let mut core = EditorCore::new();
        core.selected_node = Some(1);
        core.snapshot();
        core.snapshot();
        core.snapshot();
        assert_eq!(core.edits_since_autosave(), 3);
        core.cycle_node_color();
        assert_eq!(core.edits_since_autosave(), 4);
        core.cycle_figure_opacity(0);
        assert_eq!(core.edits_since_autosave(), 5);
        assert_eq!(core.undo_depth(), 5, "the cyclers are undo steps since RM24");
    }

    #[test]
    fn mark_dirty_advances_the_counter_without_an_undo_step() {
        // Wave 13 (B2): the seam itself — the autosave
        // counter (and with it the strip's dirty dot,
        // which reads this counter) advances; neither
        // history stack moves; and a recovery write
        // covers marked edits exactly like snapshot ones.
        let mut core = EditorCore::new();
        assert_eq!(core.edits_since_autosave(), 0);
        core.mark_dirty();
        core.mark_dirty();
        core.mark_dirty();
        assert_eq!(core.edits_since_autosave(), 3);
        assert_eq!(core.undo_depth(), 0);
        assert_eq!(core.redo_depth(), 0);
        core.note_recovery_written();
        assert_eq!(core.edits_since_autosave(), 0);
    }

    // -----------------------------------------------------------------
    // Roadmap Wave 1 (RM24, P-14): labelled history, coalescing,
    // the History-panel jump, the named-save state, and the H6
    // dropped-session recap.
    // -----------------------------------------------------------------

    #[test]
    fn rm24_labels_ride_the_stacks() {
        let mut core = EditorCore::new();
        assert_eq!(core.undo_label(), None);
        assert_eq!(core.redo_label(), None);
        core.set_figure_color(0, 0x112233);
        assert_eq!(core.undo_label(), Some("Figure colour"));
        assert_eq!(core.undo_labels(), vec!["Figure colour".to_string()]);
        core.undo();
        assert_eq!(core.undo_label(), None);
        assert_eq!(core.redo_label(), Some("Figure colour"));
        assert_eq!(core.redo_labels(), vec!["Figure colour".to_string()]);
        core.redo();
        assert_eq!(core.undo_label(), Some("Figure colour"));
        assert_eq!(core.project.figures[0].color, 0x112233);
    }

    #[test]
    fn rm24_coalesced_run_is_one_step_restoring_the_start() {
        let mut core = EditorCore::new();
        let start = core.project.figures[0].color;
        // A colour-picker run: three picks, one gesture.
        core.set_figure_color(0, 0x111111);
        core.set_figure_color(0, 0x222222);
        core.set_figure_color(0, 0x333333);
        assert_eq!(core.undo_depth(), 1, "the run folds into one step");
        assert_eq!(core.edits_since_autosave(), 3, "every pick still counts");
        core.undo();
        assert_eq!(core.project.figures[0].color, start, "one undo reverts the run");
    }

    #[test]
    fn rm24_coalescing_licence_dies_on_history_navigation() {
        let mut core = EditorCore::new();
        core.set_figure_color(0, 0x111111);
        core.set_figure_color(0, 0x222222);
        assert_eq!(core.undo_depth(), 1);
        core.undo();
        core.redo();
        // Same label, but the run was interrupted by history
        // navigation: the next pick starts a NEW step.
        core.set_figure_color(0, 0x333333);
        assert_eq!(core.undo_depth(), 2, "no fold across an undo/redo");
    }

    #[test]
    fn rm24_rollback_coalesced_pops_only_its_own_entry() {
        let mut core = EditorCore::new();
        core.snapshot_coalesced("Test gesture");
        assert_eq!(core.undo_depth(), 1);
        let edits = core.edits_since_autosave();
        // A rollback naming another gesture is a no-op.
        core.rollback_coalesced("Other gesture");
        assert_eq!(core.undo_depth(), 1);
        assert_eq!(core.edits_since_autosave(), edits);
        core.rollback_coalesced("Test gesture");
        assert_eq!(core.undo_depth(), 0, "the gesture's own entry pops");
        assert_eq!(
            core.edits_since_autosave(),
            edits - 1,
            "and its counted edit is given back"
        );
    }

    #[test]
    fn rm24_history_jump_walks_and_clamps() {
        let mut core = EditorCore::new();
        core.add_frame();
        core.add_frame();
        core.add_frame();
        assert_eq!(core.undo_depth(), 3);
        core.history_jump(-2);
        assert_eq!(core.undo_depth(), 1);
        assert_eq!(core.redo_depth(), 2);
        assert_eq!(core.status, "Undid 2 steps.");
        core.history_jump(1);
        assert_eq!(core.redo_depth(), 1);
        assert_eq!(core.status, "Redone.");
        // Clamped at the end of the redo stack.
        core.history_jump(99);
        assert_eq!(core.redo_depth(), 0);
        assert_eq!(core.undo_depth(), 3);
        // Zero is a no-op that leaves the recap alone.
        core.history_jump(0);
        assert_eq!(core.status, "Redid 2 steps.");
        // And at the bottom of the undo stack.
        core.history_jump(-99);
        assert_eq!(core.undo_depth(), 0);
        core.history_jump(-1);
        assert_eq!(core.status, "Nothing to undo.");
    }

    #[test]
    fn rm24_save_state_follows_the_named_baseline() {
        let mut core = EditorCore::new();
        // A fresh core has never met a named file.
        assert_eq!(core.save_state(), SaveState::NeverSaved);
        assert_eq!(core.edits_since_save(), None);
        // A file open / named save stamps the baseline.
        core.note_save_baseline();
        assert_eq!(core.save_state(), SaveState::Saved);
        assert_eq!(core.edits_since_save(), Some(0));
        core.add_frame();
        assert_eq!(core.save_state(), SaveState::ModifiedSinceSave);
        assert_eq!(core.edits_since_save(), Some(1));
        // A swap forgets the baseline entirely.
        core.note_project_swapped();
        assert_eq!(core.save_state(), SaveState::NeverSaved);
        assert_eq!(core.edits_since_save(), None);
    }

    #[test]
    fn rm24_undo_names_a_dropped_clip_session() {
        let mut core = EditorCore::new();
        let ci = core.create_clip_from_figure(0).unwrap();
        assert!(core.enter_clip(ci));
        // Undoing the create removes the clip under the live
        // session: the recap must say the session closed.
        core.undo();
        assert!(core.clip_edit.is_none(), "the orphaned session is dropped");
        assert_eq!(
            core.status, "Undone. (Clip session closed — its clip is gone.)",
            "H6: the recap names the side effect"
        );
        // A plain undo keeps the bare historical word (the
        // exact string older suites pin).
        let mut core = EditorCore::new();
        core.add_frame();
        core.undo();
        assert_eq!(core.status, "Undone.");
    }

    #[test]
    fn set_length_writes_the_exact_value_through_the_shared_clamp() {
        // Roadmap Wave 1 (RM3, F-3): the typed-entry
        // counterpart of `adjust_length` — the value lands
        // exactly, out-of-range input clamps to the same
        // bounds the stepper op uses, and the write counts
        // one edit on a real change, with the op's status
        // wording. Discipline since RM24 (P-14): the set
        // takes a coalesced undo step (RM3 shipped it as a
        // Wave 13 B2 mark-only tweak).
        let mut core = EditorCore::new();
        core.selected_node = Some(1);
        let edits = core.edits_since_autosave();
        core.set_length(120.0);
        assert_eq!(core.project.figures[0].def.nodes[1].length, 120.0);
        assert_eq!(core.edits_since_autosave(), edits + 1);
        assert_eq!(core.undo_depth(), 1, "a set is one coalesced undo step");
        assert!(
            core.status.ends_with("length 120"),
            "status: {}",
            core.status
        );
        // Clamp parity with `adjust_length`, both ends.
        core.set_length(999.0);
        assert_eq!(
            core.project.figures[0].def.nodes[1].length,
            EditorCore::LENGTH_MAX
        );
        core.set_length(0.0);
        assert_eq!(
            core.project.figures[0].def.nodes[1].length,
            EditorCore::LENGTH_MIN
        );
        // A no-change set moves nothing.
        let edits = core.edits_since_autosave();
        core.set_length(EditorCore::LENGTH_MIN);
        assert_eq!(core.edits_since_autosave(), edits);
        // The origin has no segment to lengthen: node 0
        // refuses silently, exactly as `adjust_length`.
        core.selected_node = Some(0);
        let before = core.project.figures[0].def.nodes[0].length;
        core.set_length(100.0);
        assert_eq!(core.project.figures[0].def.nodes[0].length, before);
        // And with no selection at all: no write, no panic.
        core.selected_node = None;
        core.set_length(100.0);
    }

    #[test]
    fn set_thickness_writes_the_exact_value_through_the_shared_clamp() {
        // Roadmap Wave 1 (RM3, F-3): as `set_length`,
        // against `adjust_thickness`'s bounds and wording.
        let mut core = EditorCore::new();
        core.selected_node = Some(1);
        core.set_thickness(9.0);
        assert_eq!(core.project.figures[0].def.nodes[1].thickness, 9.0);
        assert!(
            core.status.ends_with("thickness 9"),
            "status: {}",
            core.status
        );
        assert_eq!(core.undo_depth(), 1, "one coalesced step (RM24)");
        core.set_thickness(99.0);
        assert_eq!(
            core.project.figures[0].def.nodes[1].thickness,
            EditorCore::THICKNESS_MAX
        );
        core.set_thickness(0.0);
        assert_eq!(
            core.project.figures[0].def.nodes[1].thickness,
            EditorCore::THICKNESS_MIN
        );
    }

    #[test]
    fn quiet_setters_write_without_a_recap() {
        // Roadmap Wave 1 (RM3, F-3): the scrub doors —
        // same write, same dirty discipline as the
        // announcing setters, but the status line is left
        // exactly as it was (mid-drag silence; the
        // gesture's release announces once).
        let mut core = EditorCore::new();
        core.selected_node = Some(1);
        core.status = "Standing line.".into();
        let edits = core.edits_since_autosave();
        core.set_length_quiet(88.0);
        assert_eq!(core.project.figures[0].def.nodes[1].length, 88.0);
        assert_eq!(core.edits_since_autosave(), edits + 1);
        assert_eq!(core.status, "Standing line.");
        core.set_thickness_quiet(7.0);
        assert_eq!(core.project.figures[0].def.nodes[1].thickness, 7.0);
        assert_eq!(core.status, "Standing line.");
        // RM24: each scrub door took its own coalesced
        // step (different labels, so no fold).
        assert_eq!(core.undo_depth(), 2);
    }

    #[test]
    fn set_node_taper_clamps_and_takes_the_family_snapshot() {
        // Roadmap Wave 1 (RM3, F-3): the panel door for
        // `style::set_node_taper` — the value is clamped
        // to 0..=1 here, the write is the free function's:
        // exactly one snapshot and the cycle op's status
        // wording ("Segment Taper …").
        let mut core = EditorCore::new();
        core.selected_node = Some(1);
        core.set_node_taper(0.6);
        assert_eq!(core.project.figures[0].def.nodes[1].style.taper, 0.6);
        assert_eq!(core.undo_depth(), 1, "one snapshot, like the cycle");
        assert!(
            core.status.starts_with("Segment Taper"),
            "status: {}",
            core.status
        );
        core.set_node_taper(2.5);
        assert_eq!(core.project.figures[0].def.nodes[1].style.taper, 1.0);
        core.set_node_taper(-1.0);
        assert_eq!(core.project.figures[0].def.nodes[1].style.taper, 0.0);
        // No selection: the free function's refusal, and
        // no snapshot taken for it.
        core.selected_node = None;
        let depth = core.undo_depth();
        core.set_node_taper(0.5);
        assert_eq!(core.undo_depth(), depth);
        assert_eq!(core.status, "Select a joint first.");
    }

    #[test]
    fn shared_clamps_are_the_stepper_ops_bounds() {
        // Roadmap Wave 1 (RM3, F-3): the pure helpers the
        // kit's field specs read — pinned at the consts so
        // a future bounds change moves the ops, the
        // setters and the fields together, or fails here.
        assert_eq!(EditorCore::length_clamped(2.0), EditorCore::LENGTH_MIN);
        assert_eq!(EditorCore::length_clamped(120.0), 120.0);
        assert_eq!(EditorCore::length_clamped(500.0), EditorCore::LENGTH_MAX);
        assert_eq!(
            EditorCore::thickness_clamped(0.5),
            EditorCore::THICKNESS_MIN
        );
        assert_eq!(
            EditorCore::thickness_clamped(30.0),
            EditorCore::THICKNESS_MAX
        );
        assert_eq!(EditorCore::taper_clamped(-0.25), 0.0);
        assert_eq!(EditorCore::taper_clamped(0.25), 0.25);
        assert_eq!(EditorCore::taper_clamped(1.25), 1.0);
    }

    #[test]
    fn revision_moves_at_the_edit_choke_points_only() {
        // Roadmap Wave 1 (RM6, F-5): the revision shares
        // edit_count's choke points — snapshot, mark_dirty,
        // the undo/redo restores and the swap funnels move
        // it; pure navigation does not (the frame is the
        // cache key's other half, not a project change).
        let mut core = EditorCore::new();
        let r0 = core.revision();
        core.goto(1.min(core.frame_count() - 1));
        assert_eq!(core.revision(), r0, "navigation is not a project change");
        core.snapshot();
        assert_eq!(core.revision(), r0 + 1, "snapshot bumps");
        core.snapshot();
        core.mark_dirty();
        assert_eq!(core.revision(), r0 + 3, "mark_dirty bumps");
        core.undo();
        assert_eq!(core.revision(), r0 + 4, "an undo restore bumps");
        core.redo();
        assert_eq!(core.revision(), r0 + 5, "a redo restore bumps");
        core.note_project_swapped();
        assert_eq!(
            core.revision(),
            r0 + 6,
            "a swap bumps forward — it never restarts the counter"
        );
        core.note_project_replaced();
        assert_eq!(core.revision(), r0 + 7, "a replace bumps");
    }

    #[test]
    fn revision_never_moves_backwards_across_undo() {
        // Roadmap Wave 1 (RM6, F-5): undo restores earlier
        // CONTENT; the revision must still move forward, or
        // a cache entry cut before the edit could be served
        // for the restored project.
        let mut core = EditorCore::new();
        core.snapshot();
        let r_before_edit = core.revision();
        core.project.background = 0x123456;
        core.snapshot();
        assert!(core.revision() > r_before_edit);
        core.undo();
        assert_eq!(core.project.background, 0x123456, "undo spent the edit snapshot");
        core.undo();
        assert_ne!(core.project.background, 0x123456, "content went back");
        assert!(
            core.revision() > r_before_edit,
            "but the revision only ever moves forward"
        );
    }

    #[test]
    fn evaluated_frame_is_shared_per_revision_and_frame() {
        // Roadmap Wave 1 (RM6, F-5): evaluate-once — two
        // reads of the same (revision, frame) return the
        // SAME allocation; a different frame evaluates
        // separately; any project mutation (here a snapshot
        // with no content change — the gesture-start shape)
        // invalidates by revision.
        let core = EditorCore::new();
        let a = core.evaluated_frame(0.0);
        let b = core.evaluated_frame(0.0);
        assert!(std::rc::Rc::ptr_eq(&a, &b), "one evaluation per (revision, frame)");
        let c = core.evaluated_frame(1.0);
        assert!(!std::rc::Rc::ptr_eq(&a, &c), "a different frame evaluates separately");
        let a2 = core.evaluated_frame(0.0);
        assert!(std::rc::Rc::ptr_eq(&a, &a2), "the first frame stays cached");

        let mut core = EditorCore::new();
        let before = core.evaluated_frame(0.0);
        core.snapshot();
        let after = core.evaluated_frame(0.0);
        assert!(
            !std::rc::Rc::ptr_eq(&before, &after),
            "a mutation invalidates by revision even at the same frame"
        );
        assert_eq!(before.poses.len(), after.poses.len());
    }

    #[test]
    fn positions_f_reads_the_shared_evaluation_in_both_branches() {
        // Roadmap Wave 1 (RM6, F-5): positions_f through
        // the evaluated frame returns exactly the values
        // the per-figure paths returned — raw poses_at
        // without channels, the apply_channels result with.
        let mut core = EditorCore::new();
        for fi in 0..core.project.figures.len() {
            let want = {
                let poses = core.project.poses_at(1.0);
                evaluate(&core.project.figures[fi].def, &poses[fi])
            };
            assert_eq!(core.positions_f(1.0, fi), want, "no-channel figure {fi}");
        }
        core.project.channels.push(crate::graph::Channel {
            figure: 0,
            target: crate::graph::ChannelTarget::RootX,
            keys: vec![crate::graph::Key::flat(0.0, 123.5)],
        });
        let r = crate::graph::apply_channels(&core.project, &core.project.channels, 1.0);
        for fi in 0..core.project.figures.len() {
            assert_eq!(
                core.positions_f(1.0, fi),
                evaluate(&r.defs[fi], &r.poses[fi]),
                "channelled figure {fi}"
            );
            assert_eq!(
                core.evaluated_frame(1.0).bends[fi],
                r.bends[fi],
                "the stage's bend rows are the same evaluation's, figure {fi}"
            );
        }
    }

    #[test]
    fn layer_ops_spend_one_step_per_real_change() {
        // Wave 13 (B2): the E8 family that lives in this
        // file — lock, opacity cycle/step, hide — each
        // real change counts once and a refusal or a
        // clamped no-change step counts nothing.
        // Roadmap Wave 1 (RM24, P-14): the changes are
        // undo steps now — the opacity step FOLDS into
        // the cycle's entry (same label, one gesture
        // family), so four counted edits are three steps.
        let mut core = EditorCore::new();
        let edits = core.edits_since_autosave();
        let depth = core.undo_depth();
        core.toggle_figure_lock(0);
        assert_eq!(core.edits_since_autosave(), edits + 1);
        core.cycle_figure_opacity(0);
        assert_eq!(core.edits_since_autosave(), edits + 2);
        core.step_figure_opacity(0, -0.1);
        assert_eq!(core.edits_since_autosave(), edits + 3);
        core.toggle_figure_hidden();
        assert_eq!(core.edits_since_autosave(), edits + 4);
        assert_eq!(core.undo_depth(), depth + 3, "lock + folded opacity run + hide");
        // Refusals spend nothing.
        core.toggle_figure_lock(99);
        core.cycle_figure_opacity(99);
        core.step_figure_opacity(99, -0.1);
        assert_eq!(core.edits_since_autosave(), edits + 4);
        // A step clamped to the current value is not a
        // change and does not mark: park the opacity at
        // the layers floor, then step down again.
        core.project.figures[0].opacity = 0.05;
        core.step_figure_opacity(0, -0.1);
        assert_eq!(core.edits_since_autosave(), edits + 4);
    }

    #[test]
    fn settings_and_tweak_ops_mark_dirty_on_real_changes_only() {
        // Wave 13 (B2): a representative pass over the
        // wider non-snapshot family in this file —
        // settings setters/cyclers, segment tweaks and
        // the frame-clear — each real change counts
        // once and no-change calls count nothing.
        // Roadmap Wave 1 (RM24, P-14): each real change
        // is an undo step now; the pass below spends
        // exactly seven (easing cycle, easing set, fps,
        // tween, follow, shape, colour).
        let mut core = EditorCore::new();
        core.selected_node = Some(1);
        let depth = core.undo_depth();
        let mut edits = core.edits_since_autosave();
        core.cycle_easing();
        edits += 1;
        assert_eq!(core.edits_since_autosave(), edits);
        // Re-setting the current easing changes nothing.
        let current = core.project.easing;
        core.set_project_easing(current);
        assert_eq!(core.edits_since_autosave(), edits);
        core.set_project_easing(crate::model::Easing::EaseIn);
        edits += 1;
        assert_eq!(core.edits_since_autosave(), edits);
        core.adjust_fps(1.0);
        edits += 1;
        assert_eq!(core.edits_since_autosave(), edits);
        core.toggle_tween_playback();
        edits += 1;
        assert_eq!(core.edits_since_autosave(), edits);
        core.toggle_camera_follow();
        edits += 1;
        assert_eq!(core.edits_since_autosave(), edits);
        core.cycle_shape();
        edits += 1;
        assert_eq!(core.edits_since_autosave(), edits);
        // Segment colour: a change marks, the same
        // colour again does not.
        core.set_node_color(0x112233);
        edits += 1;
        assert_eq!(core.edits_since_autosave(), edits);
        core.set_node_color(0x112233);
        assert_eq!(core.edits_since_autosave(), edits);
        assert_eq!(core.undo_depth(), depth + 7, "one undo step per real change (RM24)");
        // Clearing an empty frame spends nothing. Plant a
        // stroke through the draw path (which snapshots
        // at the gesture start — census A18) and
        // re-baseline: the clear itself must then count
        // once and spend exactly one undo step (census
        // A17 overruled the mark-only pin this test first
        // carried: the op owns the snapshot, like every
        // peer stroke-destruction op).
        core.clear_frame_drawings();
        assert_eq!(core.edits_since_autosave(), edits);
        assert_eq!(core.undo_depth(), depth + 7, "an empty clear spends no undo step");
        core.begin_draw([100.0, 100.0]);
        core.draw_to([140.0, 120.0]);
        core.end_draw();
        assert_eq!(core.project.strokes.len(), 1);
        let edits = core.edits_since_autosave();
        let depth = core.undo_depth();
        core.clear_frame_drawings();
        assert_eq!(core.edits_since_autosave(), edits + 1);
        assert_eq!(core.undo_depth(), depth + 1, "the clear's one undo step");
        assert!(core.project.strokes.is_empty());
        core.undo();
        assert_eq!(core.project.strokes.len(), 1, "one undo restores the cleared stroke");
    }

    #[test]
    fn redo_clamps_selected_figure_like_undo() {
        // Wave 13 (B2): undo has always clamped the
        // selection into the restored project; redo did
        // not — restoring a project with fewer figures
        // left `selected_figure` out of bounds. Drive
        // redo from a hand-built stack entry (the test
        // module can touch the stacks directly).
        let mut core = EditorCore::new();
        core.add_figure();
        assert_eq!(core.project.figures.len(), 2);
        core.selected_figure = 1;
        let mut one_fig = core.project.clone();
        one_fig.figures.pop();
        core.redo
            .push(serde_json::to_string(&one_fig).unwrap());
        core.redo();
        assert_eq!(core.project.figures.len(), 1);
        assert_eq!(
            core.selected_figure, 0,
            "redo clamps the selection into the restored project, like undo"
        );
    }

    #[test]
    fn undo_and_redo_depth_track_the_stacks() {
        let mut core = EditorCore::new();
        assert_eq!(core.undo_depth(), 0);
        assert_eq!(core.redo_depth(), 0);
        core.add_frame();
        core.add_frame();
        assert_eq!(core.undo_depth(), 2);
        core.undo();
        assert_eq!(core.undo_depth(), 1);
        assert_eq!(core.redo_depth(), 1);
        core.redo();
        assert_eq!(core.undo_depth(), 2);
        assert_eq!(core.redo_depth(), 0);
        // A fresh edit clears the redo stack.
        core.undo();
        assert_eq!(core.redo_depth(), 1);
        core.add_frame();
        assert_eq!(core.redo_depth(), 0);
    }

    #[test]
    fn node_style_setters_write_the_selected_node() {
        let mut core = EditorCore::new();
        core.selected_node = Some(1);
        core.set_node_cap(crate::model::CapKind::Square);
        assert_eq!(
            core.project.figures[0].def.nodes[1].style.cap,
            crate::model::CapKind::Square
        );
        // Same status wording as cycle_node_cap.
        assert_eq!(core.status, "Segment cap: Square");
        core.set_node_color2(Some(0x112233));
        assert_eq!(
            core.project.figures[0].def.nodes[1].style.color2,
            Some(0x112233)
        );
        assert_eq!(core.status, "Gradient to #112233.");
        core.set_node_color2(None);
        assert_eq!(core.project.figures[0].def.nodes[1].style.color2, None);
        assert_eq!(core.status, "Gradient off (solid colour).");
        core.set_node_opacity(0.5);
        assert_eq!(core.project.figures[0].def.nodes[1].style.opacity, 0.5);
        // Opacity is clamped to 0.05..=1.0.
        core.set_node_opacity(2.0);
        assert_eq!(core.project.figures[0].def.nodes[1].style.opacity, 1.0);
        core.set_node_opacity(0.0);
        assert_eq!(core.project.figures[0].def.nodes[1].style.opacity, 0.05);
    }

    #[test]
    fn node_style_setters_no_op_without_a_selection() {
        let mut core = EditorCore::new();
        assert_eq!(core.selected_node, None);
        let before = core.project.figures[0].def.nodes[1].style;
        core.set_node_cap(crate::model::CapKind::Point);
        assert_eq!(core.status, "Select a joint first.");
        core.set_node_color2(Some(0xFFFFFF));
        assert_eq!(core.status, "Select a joint first.");
        core.set_node_opacity(0.25);
        assert_eq!(core.status, "Select a joint first.");
        assert_eq!(core.project.figures[0].def.nodes[1].style, before);
    }

    #[test]
    fn node_style_atomic_applies_all_four_in_one_undo_step() {
        let mut core = EditorCore::new();
        core.selected_node = Some(1);
        let before_color = core.project.figures[0].def.nodes[1].color;
        let before_style = core.project.figures[0].def.nodes[1].style;
        let depth = core.undo_depth();
        core.set_node_style_atomic(
            0x334455,
            Some(0x112233),
            crate::model::CapKind::Square,
            0.5,
        );
        // Exactly one snapshot for the whole style.
        assert_eq!(core.undo_depth(), depth + 1);
        let node = &core.project.figures[0].def.nodes[1];
        assert_eq!(node.color, Some(0x334455));
        assert_eq!(node.style.color2, Some(0x112233));
        assert_eq!(node.style.cap, crate::model::CapKind::Square);
        assert_eq!(node.style.opacity, 0.5);
        // The last write's status is the one left standing,
        // in the opacity setter's own wording.
        assert_eq!(core.status, "Segment opacity 50%.");
        // And that one step undoes the whole style.
        core.undo();
        let node = &core.project.figures[0].def.nodes[1];
        assert_eq!(node.color, before_color);
        assert_eq!(node.style, before_style);
        assert_eq!(core.undo_depth(), depth);
    }

    #[test]
    fn node_style_atomic_honours_the_setter_clamps_and_none() {
        let mut core = EditorCore::new();
        core.selected_node = Some(1);
        // Opacity clamps to 1.0, exactly like set_node_opacity.
        core.set_node_style_atomic(0x000000, Some(0xABCDEF), crate::model::CapKind::Round, 2.0);
        assert_eq!(core.project.figures[0].def.nodes[1].style.opacity, 1.0);
        // …and to 0.05; a None gradient clears, like
        // set_node_color2(None).
        core.set_node_style_atomic(0x000000, None, crate::model::CapKind::Round, 0.0);
        let node = &core.project.figures[0].def.nodes[1];
        assert_eq!(node.style.opacity, 0.05);
        assert_eq!(node.style.color2, None);
    }

    #[test]
    fn node_style_atomic_refuses_without_a_selection() {
        let mut core = EditorCore::new();
        assert_eq!(core.selected_node, None);
        let before = core.project.figures[0].def.nodes[1].style;
        let depth = core.undo_depth();
        core.set_node_style_atomic(
            0x334455,
            Some(0x112233),
            crate::model::CapKind::Square,
            0.5,
        );
        assert_eq!(core.status, "Select a joint first.");
        // The refusal snapshots nothing.
        assert_eq!(core.undo_depth(), depth);
        assert_eq!(core.project.figures[0].def.nodes[1].style, before);
    }

    fn save_temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "styk-editor-save-{}-{}",
            std::process::id(),
            name
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn save_named_project_to_reports_true_and_writes_the_file() {
        let mut core = EditorCore::new();
        core.project.name = "Bool Save Test".into();
        // A projects dir that does not exist yet — the op
        // creates it, as it does for ./projects.
        let dir = save_temp_dir("ok").join("projects");
        assert!(core.save_named_project_to(&dir));
        assert!(core.status.starts_with("Project saved to "));
        let written = std::fs::read_to_string(dir.join("bool-save-test.json"))
            .expect("the slugified project file exists");
        let parsed: crate::model::Project =
            serde_json::from_str(&written).expect("the written file parses back");
        assert_eq!(parsed.name, "Bool Save Test");
    }

    #[test]
    fn save_named_project_to_reports_false_when_the_write_cannot_succeed() {
        let mut core = EditorCore::new();
        core.project.name = "Bool Save Test".into();
        // A regular file where the projects dir would have
        // to be created: create_dir_all cannot succeed.
        let base = save_temp_dir("blocked");
        let blocker = base.join("blocker");
        std::fs::write(&blocker, b"not a directory").unwrap();
        let dir = blocker.join("projects");
        assert!(!core.save_named_project_to(&dir));
        assert!(core.status.starts_with("Project save failed: "));
    }

    #[test]
    fn set_project_easing_writes_without_snapshotting() {
        let mut core = EditorCore::new();
        let depth = core.undo_depth();
        core.set_project_easing(crate::model::Easing::EaseIn);
        assert_eq!(core.project.easing, crate::model::Easing::EaseIn);
        assert!(core.status.starts_with("Easing: "));
        // Mirrors cycle_easing: project-settings tweaks are
        // not undoable edits, so the undo stack is untouched.
        assert_eq!(core.undo_depth(), depth);
    }

    #[test]
    fn set_project_fps_writes_and_snapshots_once() {
        let mut core = EditorCore::new();
        assert_eq!(core.project.fps, 12.0, "demo rate");
        let depth = core.undo_depth();
        core.set_project_fps(24.0);
        assert_eq!(core.project.fps, 24.0);
        assert_eq!(core.status, "Frame rate set to 24 fps.");
        assert_eq!(core.undo_depth(), depth + 1);
    }

    #[test]
    fn set_project_fps_clamps_into_range() {
        let mut core = EditorCore::new();
        let depth = core.undo_depth();
        core.set_project_fps(0.0);
        assert_eq!(core.project.fps, 1.0);
        assert_eq!(core.status, "Frame rate set to 1 fps.");
        core.set_project_fps(500.0);
        assert_eq!(core.project.fps, 60.0);
        assert_eq!(core.undo_depth(), depth + 2, "one snapshot per real change");
    }

    #[test]
    fn set_project_fps_refuses_non_finite_without_snapshotting() {
        let mut core = EditorCore::new();
        let depth = core.undo_depth();
        core.set_project_fps(f32::NAN);
        assert_eq!(core.project.fps, 12.0);
        assert_eq!(core.status, "Frame rate unchanged.");
        core.set_project_fps(f32::INFINITY);
        assert_eq!(core.project.fps, 12.0);
        assert_eq!(core.undo_depth(), depth);
    }

    #[test]
    fn set_project_fps_refuses_the_current_rate() {
        let mut core = EditorCore::new();
        let depth = core.undo_depth();
        core.set_project_fps(12.0);
        assert_eq!(core.project.fps, 12.0);
        assert_eq!(core.status, "Frame rate unchanged.");
        assert_eq!(core.undo_depth(), depth);
    }

    #[test]
    fn set_stage_size_writes_and_snapshots_once() {
        let mut core = EditorCore::new();
        assert_eq!(core.project.stage, [960.0, 720.0], "demo stage");
        let depth = core.undo_depth();
        core.set_stage_size(1280.0, 960.0);
        assert_eq!(core.project.stage, [1280.0, 960.0]);
        assert_eq!(core.status, "Stage size set to 1280×960.");
        assert_eq!(core.undo_depth(), depth + 1);
    }

    #[test]
    fn set_stage_size_clamps_each_axis() {
        let mut core = EditorCore::new();
        let depth = core.undo_depth();
        core.set_stage_size(5000.0, 10.0);
        assert_eq!(core.project.stage, [4096.0, 64.0]);
        assert_eq!(core.status, "Stage size set to 4096×64.");
        assert_eq!(core.undo_depth(), depth + 1);
    }

    #[test]
    fn set_stage_size_refuses_non_finite_without_snapshotting() {
        let mut core = EditorCore::new();
        let depth = core.undo_depth();
        core.set_stage_size(f32::NAN, 720.0);
        assert_eq!(core.project.stage, [960.0, 720.0]);
        assert_eq!(core.status, "Stage size unchanged.");
        core.set_stage_size(960.0, f32::INFINITY);
        assert_eq!(core.project.stage, [960.0, 720.0]);
        assert_eq!(core.undo_depth(), depth);
    }

    #[test]
    fn set_stage_size_refuses_the_current_size() {
        let mut core = EditorCore::new();
        let depth = core.undo_depth();
        core.set_stage_size(960.0, 720.0);
        assert_eq!(core.project.stage, [960.0, 720.0]);
        assert_eq!(core.status, "Stage size unchanged.");
        assert_eq!(core.undo_depth(), depth);
    }

    #[test]
    fn set_head_ring_scale_writes_and_snapshots_once() {
        let mut core = EditorCore::new();
        assert_eq!(core.project.head_ring_scale, 1.0, "default scale");
        let depth = core.undo_depth();
        core.set_head_ring_scale(1.5);
        assert_eq!(core.project.head_ring_scale, 1.5);
        assert_eq!(core.status, "Head ring size set to 1.5×.");
        assert_eq!(core.undo_depth(), depth + 1);
        core.undo();
        assert_eq!(core.project.head_ring_scale, 1.0, "one undo restores the default");
    }

    #[test]
    fn set_head_ring_scale_clamps_into_range() {
        let mut core = EditorCore::new();
        let depth = core.undo_depth();
        core.set_head_ring_scale(99.0);
        assert_eq!(core.project.head_ring_scale, 2.0);
        assert_eq!(core.status, "Head ring size set to 2×.");
        assert_eq!(core.undo_depth(), depth + 1, "one snapshot for the clamped change");
    }

    #[test]
    fn set_head_ring_scale_refuses_non_finite_without_snapshotting() {
        let mut core = EditorCore::new();
        let depth = core.undo_depth();
        core.set_head_ring_scale(f32::NAN);
        assert_eq!(core.project.head_ring_scale, 1.0);
        assert_eq!(core.status, "Head ring size unchanged.");
        core.set_head_ring_scale(f32::INFINITY);
        assert_eq!(core.project.head_ring_scale, 1.0);
        assert_eq!(core.undo_depth(), depth);
    }

    #[test]
    fn set_head_ring_scale_refuses_the_current_scale() {
        let mut core = EditorCore::new();
        let depth = core.undo_depth();
        core.set_head_ring_scale(1.0);
        assert_eq!(core.project.head_ring_scale, 1.0);
        // The set wording is used even though nothing
        // changed — the size IS the requested size; only
        // the snapshot is skipped (see the setter's doc).
        assert_eq!(core.status, "Head ring size set to 1×.");
        assert_eq!(core.undo_depth(), depth);
    }

    #[test]
    fn delete_stroke_at_removes_the_indexed_stroke() {
        let mut core = EditorCore::new();
        let mk = |color| crate::model::Stroke {
            frame: 0,
            points: vec![[0.0, 0.0], [10.0, 10.0]],
            color,
            width: 3.0,
            widths: Vec::new(),
            fill: None,
            spray: None,
            opacity: 1.0,
            scatter: 0.0,
        };
        core.project.strokes = vec![mk(0x111111), mk(0x222222), mk(0x333333)];
        core.delete_stroke_at(1);
        let colors: Vec<u32> = core.project.strokes.iter().map(|s| s.color).collect();
        assert_eq!(colors, vec![0x111111, 0x333333]);
        assert_eq!(core.status, "Stroke 2 deleted.");
        // Out of range: status only, list untouched.
        core.delete_stroke_at(9);
        assert_eq!(core.project.strokes.len(), 2);
        assert_eq!(core.status, "No stroke 10 to delete.");
    }

    #[test]
    fn autosave_due_respects_the_threshold() {
        let every = EditorCore::AUTOSAVE_EVERY;
        assert_eq!(every, 25);
        assert!(!EditorCore::autosave_due(0, 0));
        assert!(!EditorCore::autosave_due(every - 1, 0));
        assert!(EditorCore::autosave_due(every, 0));
        assert!(EditorCore::autosave_due(every * 2, every));
        assert!(!EditorCore::autosave_due(every * 2 - 1, every));
        // A last-autosave count ahead of the edit count (cannot
        // happen in practice) saturates instead of underflowing.
        assert!(!EditorCore::autosave_due(0, every));
    }

    #[test]
    fn maybe_autosave_noops_before_the_threshold() {
        let mut core = EditorCore::new();
        core.snapshot();
        let status_before = core.status.clone();
        core.maybe_autosave();
        // Below the threshold: no write, no status change (the
        // success path is silent by design), counter untouched.
        assert_eq!(core.edits_since_autosave(), 1);
        assert_eq!(core.status, status_before);
    }

    #[test]
    fn fig3_enter_exit_and_noop_bake() {
        let mut core = EditorCore::new();
        assert!(core.fig3.is_none());
        let figures_before = core.project.figures.len();
        core.fig3_bake();
        assert_eq!(
            core.project.figures.len(),
            figures_before,
            "bake without a 3D session is a no-op"
        );
        core.enter_3d();
        assert!(core.fig3.is_some());
        // Re-entering keeps the live session: a rotation survives it.
        core.fig3_rotate(2, 30.0);
        core.enter_3d();
        let st = core.fig3.as_ref().unwrap();
        assert_eq!(st.pose.rotations[st.selected][2], 30.0);
        core.exit_3d();
        assert!(core.fig3.is_none());
        core.exit_3d(); // a second exit is a harmless no-op
        assert!(core.fig3.is_none());
    }

    #[test]
    fn fig3_bake_joints_match_projection() {
        let mut core = EditorCore::new();
        core.enter_3d();
        // Expected stage points straight from the fig3d pipeline.
        let expected = {
            let st = core.fig3.as_ref().unwrap();
            let world = crate::fig3d::evaluate3(&st.figure, &st.pose);
            crate::fig3d::project(&world, &st.camera, core.project.stage)
                .iter()
                .map(|p| p.xy)
                .collect::<Vec<_>>()
        };
        let figures_before = core.project.figures.len();
        core.fig3_bake();
        assert_eq!(core.project.figures.len(), figures_before + 1);
        let fi = core.project.figures.len() - 1;
        assert_eq!(core.selected_figure, fi);
        assert_eq!(core.project.figures[fi].def.nodes.len(), expected.len());
        for frame in &core.project.frames {
            assert_eq!(frame.poses.len(), core.project.figures.len());
        }
        // The baked figure's evaluated joints equal the projected 3D
        // points in every frame (the bake is held as the rest pose).
        for f in 0..core.frame_count() {
            let joints = core.positions(f, fi);
            assert_eq!(joints.len(), expected.len());
            for (j, e) in joints.iter().zip(&expected) {
                assert!(
                    (j[0] - e[0]).abs() < 0.5 && (j[1] - e[1]).abs() < 0.5,
                    "frame {f}: joint {j:?} != projected {e:?}"
                );
            }
        }
        // Undo removes the baked figure again.
        core.undo();
        assert_eq!(core.project.figures.len(), figures_before);
    }

    #[test]
    fn fig3_rotate_and_orbit_change_the_bake() {
        let mut core = EditorCore::new();
        core.enter_3d();
        core.fig3_bake();
        let first = core.positions(0, core.project.figures.len() - 1);
        // Swing the left upper arm about Z: the left hand must move.
        core.fig3.as_mut().unwrap().selected = 3; // upperArm.L
        core.fig3_rotate(2, 90.0);
        core.fig3_bake();
        let second = core.positions(0, core.project.figures.len() - 1);
        let hand_l = 5;
        assert!(
            dist(first[hand_l], second[hand_l]) > 5.0,
            "hand.L should move: {:?} -> {:?}",
            first[hand_l],
            second[hand_l]
        );
        // Orbiting 180 degrees mirrors the figure on stage: the
        // right hand flips to the other side of the root.
        core.fig3_orbit(180.0, 0.0);
        assert_eq!(core.fig3.as_ref().unwrap().camera.yaw, 180.0);
        core.fig3_bake();
        let third = core.positions(0, core.project.figures.len() - 1);
        let root_x = third[0][0];
        let hand_r = 8;
        assert!(
            (second[hand_r][0] - root_x) * (third[hand_r][0] - root_x) < 0.0,
            "hand.R should mirror: {} vs {} around root {}",
            second[hand_r][0],
            third[hand_r][0],
            root_x
        );
    }

    #[test]
    fn fig3_bake_animation_requires_pose_b() {
        let mut core = EditorCore::new();
        let figures_before = core.project.figures.len();
        // No session at all.
        core.fig3_bake_animation(6);
        assert_eq!(core.project.figures.len(), figures_before);
        assert!(core.status.contains("3D mode"), "status: {}", core.status);
        // Session but no pose B marked.
        core.enter_3d();
        core.fig3_bake_animation(6);
        assert_eq!(core.project.figures.len(), figures_before);
        assert!(core.status.contains("pose B"), "status: {}", core.status);
        // Fewer than two frames is refused even with pose B marked.
        core.fig3_mark_pose_b();
        assert!(core.fig3.as_ref().unwrap().pose_b.is_some());
        core.fig3_bake_animation(1);
        assert_eq!(core.project.figures.len(), figures_before);
    }

    #[test]
    fn fig3_bake_animation_endpoints_match_projections() {
        let mut core = EditorCore::new();
        core.enter_3d();
        let project_frames = |core: &EditorCore| {
            let st = core.fig3.as_ref().unwrap();
            crate::fig3d::project(
                &crate::fig3d::evaluate3(&st.figure, &st.pose),
                &st.camera,
                core.project.stage,
            )
            .iter()
            .map(|p| p.xy)
            .collect::<Vec<_>>()
        };
        // Pose A = the rest pose the session starts in.
        let expected_a = project_frames(&core);
        // Pose B: swing the left upper arm 60 deg about Z (an
        // in-plane rotation, so projected segment lengths are
        // preserved and every frame can match its projection).
        core.fig3.as_mut().unwrap().selected = 3; // upperArm.L
        core.fig3_rotate(2, 60.0);
        let expected_b = project_frames(&core);
        core.fig3_mark_pose_b();
        // Pose back to A; the animation runs A -> B.
        core.fig3_rotate(2, -60.0);
        let figures_before = core.project.figures.len();
        core.fig3_bake_animation(6);
        assert_eq!(core.project.figures.len(), figures_before + 1);
        assert!(core.frame_count() >= 6, "timeline extended to the range");
        let fi = core.project.figures.len() - 1;
        assert_eq!(core.selected_figure, fi);
        for (f, expected) in [(0usize, &expected_a), (5usize, &expected_b)] {
            let joints = core.positions(f, fi);
            assert_eq!(joints.len(), expected.len());
            for (j, e) in joints.iter().zip(expected) {
                assert!(
                    (j[0] - e[0]).abs() < 0.5 && (j[1] - e[1]).abs() < 0.5,
                    "frame {f}: joint {j:?} != projected {e:?}"
                );
            }
        }
        // The middle of the range really moves: hand.L is well away
        // from its frame-0 spot by frame 3.
        let hand_l = 5;
        assert!(
            dist(core.positions(0, fi)[hand_l], core.positions(3, fi)[hand_l]) > 1.0,
            "hand.L should travel during the baked animation"
        );
        // One undo step removes the whole baked figure.
        core.undo();
        assert_eq!(core.project.figures.len(), figures_before);
    }

    #[test]
    fn fig3_library_save_and_load_via_editor() {
        let mut core = EditorCore::new();
        core.enter_3d();
        core.fig3_save_to_library();
        assert!(
            core.status.starts_with("Saved 3D figure"),
            "status: {}",
            core.status
        );
        // Loading a name that was never saved changes nothing.
        core.fig3_load_from_library("No Such Figure");
        assert!(core.status.contains("No 3D library figure"));
        assert!(core.fig3.is_some(), "session survives a failed load");
        // Round trip: drop the session, load the figure back.
        core.exit_3d();
        assert!(core.fig3.is_none());
        core.fig3_load_from_library("Humanoid3");
        let st = core.fig3.as_ref().expect("library load starts a session");
        assert_eq!(st.figure.name, "Humanoid3");
        assert_eq!(st.figure.nodes.len(), 15);
        assert!(st.pose_b.is_none());
        // Wave W, W9: an externally renamed library file
        // (stem no longer the slug of its stored name) is
        // still listed — and still loads, because load
        // resolves through the stored name, not the slug.
        let lib_dir = EditorCore::fig3_library_dir();
        let placed = lib_dir.join("fig3d-renamed.json");
        std::fs::rename(
            lib_dir.join(crate::fig3d::library_file_name("Humanoid3")),
            &placed,
        )
        .expect("rename the library file");
        assert!(core.fig3_library_list().iter().any(|n| n == "Humanoid3"));
        core.exit_3d();
        core.fig3_load_from_library("Humanoid3");
        let st = core
            .fig3
            .as_ref()
            .expect("hand-placed entry loads by its stored name");
        assert_eq!(st.figure.name, "Humanoid3");
        let _ = std::fs::remove_file(&placed);
        // Clean up the file this test wrote into ./library3d.
        let dir = EditorCore::fig3_library_dir();
        let _ = std::fs::remove_file(
            dir.join(crate::fig3d::library_file_name("Humanoid3")),
        );
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn library_file_text_stores_the_name_verbatim() {
        // Wave V (V1): the saved file carries the figure's
        // exact name in a top-level "name" field, and still
        // parses as a bare FigureDef — the stored name is
        // an extra field the definition reader ignores.
        let def = FigureDef::simple();
        let text = EditorCore::library_file_text(&def, "Styk The Great")
            .expect("serialise the library file");
        assert_eq!(
            EditorCore::library_stored_name(&text).as_deref(),
            Some("Styk The Great")
        );
        let back: FigureDef =
            serde_json::from_str(&text).expect("the file still parses as a bare FigureDef");
        assert_eq!(back.nodes.len(), def.nodes.len());
    }

    #[test]
    fn library_stored_name_absent_empty_or_mistyped_reads_as_none() {
        // The fallback contract load_library relies on for
        // every pre-stored-name file: no field, an empty
        // field, a mistyped field, or unparseable text all
        // read as "no stored name" — the stem names it.
        let bare = serde_json::to_string(&FigureDef::simple()).expect("serialise a bare def");
        assert_eq!(EditorCore::library_stored_name(&bare), None);
        assert_eq!(EditorCore::library_stored_name("{\"name\": \"\"}"), None);
        assert_eq!(EditorCore::library_stored_name("{\"name\": 3}"), None);
        assert_eq!(EditorCore::library_stored_name("not json"), None);
    }

    #[test]
    fn figure_library_round_trips_the_exact_name() {
        // Wave V (V1), the fig3 library test's pattern
        // (write into the real ./library, clean up after):
        // the demo figure is "Styk"; its file is the slug
        // styk.json, but the reloaded entry — and a figure
        // inserted from it — carry the exact name.
        let mut core = EditorCore::new();
        assert_eq!(core.project.figures[core.selected_figure].name, "Styk");
        core.save_figure_to_library();
        let dir = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("library");
        let file = dir.join("styk.json");
        // Capture everything before asserting, so the
        // cleanup below runs whatever the outcome.
        let status = core.status.clone();
        let listed = core.library.iter().any(|(n, _)| n == "Styk");
        let stored = std::fs::read_to_string(&file)
            .ok()
            .and_then(|t| EditorCore::library_stored_name(&t));
        let idx = core.library.iter().position(|(n, _)| n == "Styk");
        let mut inserted_name = None;
        if let Some(idx) = idx {
            core.library_index = idx;
            let before = core.project.figures.len();
            core.insert_library_figure();
            inserted_name = core.project.figures.get(before).map(|f| f.name.clone());
        }
        let _ = std::fs::remove_file(&file);
        let _ = std::fs::remove_dir(&dir);
        assert_eq!(status, "Saved Styk to the library.");
        assert!(listed, "the entry is listed as 'Styk', not its stem");
        assert_eq!(
            stored.as_deref(),
            Some("Styk"),
            "the name is stored in the file"
        );
        assert_eq!(
            inserted_name.as_deref(),
            Some("Styk"),
            "the inserted figure takes the exact name"
        );
    }

    #[test]
    fn library_file_text_preserves_joint_limits_verbatim() {
        // Wave Z (Z9, library state round-trip audit):
        // joint limits (Wave X, P-4) live ON NodeDef, so
        // they are part of the FigureDef the figure library
        // stores — they must ride the library file text
        // exactly, past the extra top-level "name" field
        // `library_file_text` adds. The def below carries
        // every limit shape beside the humanoid presets:
        // a wrap arc (min > max), a single bound, and a
        // pinned joint (min == max).
        let mut def = FigureDef::humanoid();
        def.nodes[0].min_angle = Some(170.0);
        def.nodes[0].max_angle = Some(-170.0);
        def.nodes[1].min_angle = None;
        def.nodes[1].max_angle = Some(-30.0);
        def.nodes[2].min_angle = Some(45.0);
        def.nodes[2].max_angle = Some(45.0);
        let text = EditorCore::library_file_text(&def, "Limited")
            .expect("serialise the library file");
        // The limits are IN the file, not reconstructed
        // from presets on load.
        let value: serde_json::Value =
            serde_json::from_str(&text).expect("the library file is JSON");
        assert_eq!(value["nodes"][0]["min_angle"], serde_json::json!(170.0));
        assert_eq!(value["nodes"][0]["max_angle"], serde_json::json!(-170.0));
        assert_eq!(value["nodes"][1]["min_angle"], serde_json::Value::Null);
        assert_eq!(value["nodes"][1]["max_angle"], serde_json::json!(-30.0));
        assert_eq!(value["nodes"][4]["min_angle"], serde_json::json!(-10.0));
        assert_eq!(value["nodes"][4]["max_angle"], serde_json::json!(160.0));
        // And the file parses back into the identical def,
        // node for node — the read `load_library` performs.
        let back: FigureDef =
            serde_json::from_str(&text).expect("the file parses as a bare FigureDef");
        assert_eq!(back.nodes.len(), def.nodes.len());
        for (want, got) in def.nodes.iter().zip(back.nodes.iter()) {
            assert_eq!(
                (want.min_angle, want.max_angle),
                (got.min_angle, got.max_angle),
                "limits of node '{}' round-trip verbatim",
                want.name
            );
        }
    }

    #[test]
    fn figure_library_round_trip_preserves_limits_and_starts_stateless() {
        // Wave Z (Z9, library state round-trip audit), the
        // V1 name pin's pattern (write into the real
        // ./library under this figure's own slug, clean up
        // after): the full save -> reload -> insert path.
        // Joint limits ride it verbatim (they are def data);
        // animation states (Wave X, P-1) do NOT — they live
        // on FigureInstance, the library file stores the
        // FigureDef only, and a state key's clip index names
        // a PROJECT clip the library does not carry, so an
        // inserted figure starts stateless by format. This
        // pin holds both halves of that audit verdict
        // consciously, so a future format change must move
        // them deliberately, never silently.
        let mut core = EditorCore::new();
        core.project.figures[0].name = "Limited Styk".into();
        core.project.figures[0].def.nodes[1].min_angle = Some(12.5);
        core.project.figures[0].def.nodes[1].max_angle = Some(34.25);
        // Assigned directly, not via the authoring op: the
        // audit question is what the SAVE does with a state
        // key that exists, not what authoring allows.
        core.project.figures[0].states = vec![crate::model::StateKey {
            frame: 2,
            clip: 0,
            offset: 1,
            speed: 1.5,
        }];
        core.save_figure_to_library();
        let dir = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("library");
        let file = dir.join("limited-styk.json");
        // Capture everything before asserting, so the
        // cleanup below runs whatever the outcome.
        let status = core.status.clone();
        let text = std::fs::read_to_string(&file).unwrap_or_default();
        let value: serde_json::Value =
            serde_json::from_str(&text).unwrap_or(serde_json::Value::Null);
        let listed_limits = core
            .library
            .iter()
            .find(|(n, _)| n == "Limited Styk")
            .map(|(_, d)| {
                (
                    (d.nodes[1].min_angle, d.nodes[1].max_angle),
                    (d.nodes[4].min_angle, d.nodes[4].max_angle),
                )
            });
        let idx = core.library.iter().position(|(n, _)| n == "Limited Styk");
        let mut inserted: Option<((Option<f32>, Option<f32>), bool)> = None;
        if let Some(idx) = idx {
            core.library_index = idx;
            let before = core.project.figures.len();
            core.insert_library_figure();
            if let Some(fig) = core.project.figures.get(before) {
                inserted = Some((
                    (fig.def.nodes[1].min_angle, fig.def.nodes[1].max_angle),
                    fig.states.is_empty(),
                ));
            }
        }
        let _ = std::fs::remove_file(&file);
        let _ = std::fs::remove_dir(&dir);
        assert_eq!(status, "Saved Limited Styk to the library.");
        // Limits are in the file itself...
        assert_eq!(value["nodes"][1]["min_angle"], serde_json::json!(12.5));
        assert_eq!(value["nodes"][1]["max_angle"], serde_json::json!(34.25));
        assert_eq!(value["nodes"][4]["min_angle"], serde_json::json!(-10.0));
        // ...the reloaded entry carries them...
        assert_eq!(
            listed_limits,
            Some(((Some(12.5), Some(34.25)), (Some(-10.0), Some(160.0)))),
            "the reloaded library entry keeps the saved limits"
        );
        // ...and the inserted figure keeps them too, while
        // starting with no state keys: the file holds no
        // "states" field at all (the format boundary).
        assert_eq!(value.get("states"), None, "no states ride the library file");
        assert_eq!(
            inserted,
            Some(((Some(12.5), Some(34.25)), true)),
            "insert keeps the limits and starts stateless"
        );
    }

    #[test]
    fn builder_add_and_delete_segment_stays_consistent() {
        let mut core = EditorCore::new();
        core.selected_node = Some(2); // head: has no children after we add one
        let nodes_before = core.project.figures[0].def.nodes.len();
        core.add_segment();
        assert_eq!(core.project.figures[0].def.nodes.len(), nodes_before + 1);
        for frame in &core.project.frames {
            assert_eq!(frame.poses[0].angles.len(), nodes_before + 1);
        }
        // The new node is selected and is a leaf: deletion succeeds.
        core.delete_segment();
        assert_eq!(core.project.figures[0].def.nodes.len(), nodes_before);
        for frame in &core.project.frames {
            assert_eq!(frame.poses[0].angles.len(), nodes_before);
        }
        // A node with children refuses deletion.
        core.selected_node = Some(1); // chest has children
        core.delete_segment();
        assert_eq!(core.project.figures[0].def.nodes.len(), nodes_before);
    }

    /// IA-RM19-6 (RM19 SS4.0): the delete remap covers bends
    /// and node-indexed graph channels, not just parents and
    /// pose angles — both misalignments reproduced here.
    #[test]
    fn delete_segment_remaps_bends_and_channels() {
        let mut core = EditorCore::new();
        let fi = 0usize;
        let n = core.project.figures[fi].def.nodes.len();
        // The first leaf past the root: later nodes exist after
        // it, so the decrement half is exercised too.
        let leaf = (1..n)
            .find(|&i| {
                core.project.figures[fi]
                    .def
                    .nodes
                    .iter()
                    .all(|nd| nd.parent != Some(i))
            })
            .expect("the demo figure has a leaf segment");
        assert!(leaf + 1 < n, "a later node must exist");
        // Distinct bend per node on frame 0, so a slid row is
        // visible value-by-value.
        let bends0: Vec<f32> = (0..n).map(|i| i as f32 * 0.1).collect();
        core.project.frames[0].bends = vec![bends0.clone()];
        let chan = |figure: usize, target: crate::graph::ChannelTarget| crate::graph::Channel {
            figure,
            target,
            keys: vec![crate::graph::Key::flat(0.0, 1.0)],
        };
        core.project.channels = vec![
            chan(fi, crate::graph::ChannelTarget::Angle(leaf)),
            chan(fi, crate::graph::ChannelTarget::Bend(leaf + 1)),
            chan(fi, crate::graph::ChannelTarget::RootX),
            chan(1, crate::graph::ChannelTarget::Angle(leaf)),
        ];
        let depth_before = core.undo_depth();
        core.selected_node = Some(leaf);
        core.delete_segment();
        assert_eq!(core.project.figures[fi].def.nodes.len(), n - 1);
        assert_eq!(core.undo_depth(), depth_before + 1, "one snapshot");
        // Bends: slot `leaf` gone, later values unmoved.
        let mut expected = bends0;
        expected.remove(leaf);
        assert_eq!(core.project.frames[0].bends[fi], expected);
        // Channels: the Angle(leaf) channel dropped + counted,
        // Bend(leaf + 1) decremented onto the same node, the
        // root channel and the other figure's channel intact.
        assert_eq!(core.project.channels.len(), 3);
        assert!(core
            .project
            .channels
            .iter()
            .any(|c| c.figure == fi
                && c.target == crate::graph::ChannelTarget::Bend(leaf)));
        assert!(core
            .project
            .channels
            .iter()
            .any(|c| c.figure == fi
                && c.target == crate::graph::ChannelTarget::RootX));
        assert!(core
            .project
            .channels
            .iter()
            .any(|c| c.figure == 1
                && c.target == crate::graph::ChannelTarget::Angle(leaf)));
        assert_eq!(core.status, "Segment deleted. (1 graph channel removed)");
    }

    #[test]
    fn redo_restores_an_undone_frame_add() {
        let mut core = EditorCore::new();
        let n = core.frame_count();
        core.add_frame();
        assert_eq!(core.frame_count(), n + 1);
        core.undo();
        assert_eq!(core.frame_count(), n);
        core.redo();
        assert_eq!(core.frame_count(), n + 1);
    }

    #[test]
    fn figure_delete_and_reorder_keep_poses_aligned() {
        let mut core = EditorCore::new();
        core.add_figure();
        core.add_quadruped();
        assert_eq!(core.project.figures.len(), 3);
        core.select_figure(0);
        core.move_figure(1);
        assert_eq!(core.selected_figure, 1);
        for frame in &core.project.frames {
            assert_eq!(frame.poses.len(), 3);
        }
        core.delete_figure();
        assert_eq!(core.project.figures.len(), 2);
        for frame in &core.project.frames {
            assert_eq!(frame.poses.len(), 2);
        }
    }

    #[test]
    fn frame_clipboard_roundtrip() {
        let mut core = EditorCore::new();
        core.copy_frame();
        core.paste_frame();
        assert_eq!(core.frame_count(), 6);
    }

    #[test]
    fn wave14_clipboard_state_read_reports_both_clipboards() {
        // Wave 14 (A8): the MCP clipboard read starts
        // empty, then tracks the real clipboard fields
        // through the copy ops themselves.
        let mut core = EditorCore::new();
        let out = tool_clipboard_state(&mut core, &serde_json::json!({})).unwrap();
        assert_eq!(out["frame"], serde_json::Value::Null);
        assert_eq!(out["range_frames"], 0);

        core.copy_frame();
        let out = tool_clipboard_state(&mut core, &serde_json::json!({})).unwrap();
        assert_eq!(out["frame"]["figures"], 1, "the demo project has one figure");
        assert_eq!(out["range_frames"], 0, "the single-frame copy leaves the range clipboard alone");

        assert!(core.copy_range(0, 2));
        let out = tool_clipboard_state(&mut core, &serde_json::json!({})).unwrap();
        assert_eq!(out["range_frames"], 2);
    }

    #[test]
    fn wave14_onion_skin_read_reports_the_core_fields() {
        // A9's `tool_get_onion_skin` (its own editor.rs
        // section) is the wave's onion read; this pins
        // the two fields A8's audit confirmed it reports:
        // the master switch, and the legacy count under
        // its honest name.
        let mut core = EditorCore::new();
        let out = tool_get_onion_skin(&mut core, &serde_json::json!({})).unwrap();
        assert_eq!(out["enabled"], true);
        assert_eq!(out["legacy_prev_frames"], 1);

        core.cycle_onion_frames();
        let out = tool_get_onion_skin(&mut core, &serde_json::json!({})).unwrap();
        assert_eq!(out["legacy_prev_frames"], 2);
    }

    // ---- Wave 11 (E6): MCP-reachability panic regressions ----
    //
    // Each test below drives an EditorCore op with an
    // adversarial-but-type-valid state or argument that an
    // MCP client can produce (a loaded project with
    // figure-absent pose rows, a zero-frame project, a huge
    // range count, a clipboard frame that predates a rig
    // edit). Every one of these panicked before the fix;
    // each now refuses or skips and leaves the state
    // consistent.

    #[test]
    fn wave11_shift_frames_huge_count_is_refused_not_a_panic() {
        // MCP `shift_frames` (mcp.rs): `count` is a raw
        // usize argument; usize::MAX wrapped `from + count`
        // in release builds, passed the guard, and the
        // frame drain panicked on the inverted range.
        let mut core = EditorCore::new();
        assert!(!core.shift_frames(1, usize::MAX, 1));
        assert_eq!(core.frame_count(), 5, "nothing moved");
        // A merely-too-big count is refused the same way.
        assert!(!core.shift_frames(1, 999, 1));
        assert_eq!(core.frame_count(), 5);
    }

    #[test]
    fn wave11_delete_figure_skips_frames_where_it_is_absent() {
        // MCP `delete_figure` (mcp.rs): a loaded project
        // may hold frames whose pose row is truncated
        // (the figure appears later); `Vec::remove(fi)`
        // on those rows panicked.
        let mut core = EditorCore::new();
        core.add_figure();
        assert_eq!(core.project.figures.len(), 2);
        core.project.frames[0].poses.truncate(1);
        core.selected_figure = 1;
        core.delete_figure();
        assert_eq!(core.project.figures.len(), 1);
        assert_eq!(core.project.frames[0].poses.len(), 1, "absent row untouched");
        assert_eq!(core.project.frames[1].poses.len(), 1, "present row removed");
    }

    #[test]
    fn wave11_add_segment_skips_frames_where_the_figure_is_absent() {
        // MCP `add_node` (mcp_tools.rs) -> add_segment:
        // the handler validates the figure's definition,
        // not per-frame pose presence; indexing
        // `poses[fi]` on an absent row panicked.
        let mut core = EditorCore::new();
        core.add_figure();
        core.project.frames[0].poses.truncate(1);
        core.selected_figure = 1;
        core.selected_node = Some(0);
        let nodes_before = core.project.figures[1].def.nodes.len();
        core.add_segment();
        assert_eq!(core.project.figures[1].def.nodes.len(), nodes_before + 1);
        assert_eq!(core.project.frames[0].poses.len(), 1, "absent row skipped");
        assert_eq!(
            core.project.frames[1].poses[1].angles.len(),
            nodes_before + 1,
            "present rows grew with the definition"
        );
    }

    #[test]
    fn wave11_delete_segment_skips_absent_and_short_pose_rows() {
        // MCP `delete_node` (mcp_tools.rs) ->
        // delete_segment: both an absent pose row and a
        // pose whose angle vec is shorter than the
        // definition panicked in the per-frame cleanup.
        let mut core = EditorCore::new();
        core.add_figure();
        core.selected_figure = 1;
        core.selected_node = Some(0);
        core.add_segment(); // appends a leaf node to figure 1
        let nodes_before = core.project.figures[1].def.nodes.len();
        core.project.frames[0].poses.truncate(1);
        core.project.frames[1].poses[1].angles.truncate(1);
        // add_segment left the new leaf selected.
        core.delete_segment();
        assert_eq!(core.project.figures[1].def.nodes.len(), nodes_before - 1);
        assert_eq!(
            core.project.frames[1].poses[1].angles.len(),
            1,
            "short row skipped, not removed from"
        );
        assert_eq!(
            core.project.frames[2].poses[1].angles.len(),
            nodes_before - 1,
            "well-formed rows lost the angle"
        );
    }

    #[test]
    fn wave11_copy_and_paste_frame_on_a_zero_frame_project() {
        // MCP `copy_frame` / `paste_frame` (mcp.rs) take
        // no index arguments, so no handler check exists:
        // a zero-frame project (loadable via
        // `load_project`) panicked both — copy indexed
        // `frames[current]`, paste inserted at
        // `current + 1` into an empty vec.
        let mut core = EditorCore::new();
        core.project.frames.clear();
        core.current = 0;
        core.copy_frame();
        assert!(core.clipboard.is_none(), "nothing to copy");
        assert_eq!(core.status, "No frame to copy.");

        let mut donor = EditorCore::new();
        donor.copy_frame();
        core.clipboard = donor.clipboard.clone();
        core.paste_frame();
        assert_eq!(core.frame_count(), 1, "the paste became frame 1");
        assert_eq!(core.current, 0);
        assert_eq!(core.status, "Frame pasted at 1.");
    }

    #[test]
    fn wave11_paste_frame_refuses_a_clipboard_frame_with_stale_angles() {
        // MCP sequence copy_frame -> add_node ->
        // paste_frame: the segment edit grows the rig and
        // every live frame, but not the clipboard frame;
        // pasting it would plant a pose whose angle count
        // the renderer indexes past.
        let mut core = EditorCore::new();
        core.copy_frame();
        core.selected_node = Some(0);
        core.add_segment();
        let before = core.frame_count();
        core.paste_frame();
        assert_eq!(core.frame_count(), before, "stale frame refused");
        assert!(core.status.contains("no longer match"), "status: {}", core.status);
    }

    #[test]
    fn wave11_paste_range_refuses_a_stale_angle_clipboard() {
        // The range twin of the paste_frame refusal:
        // copy_range -> add_node -> paste_range.
        let mut core = EditorCore::new();
        assert!(core.copy_range(0, 1));
        core.selected_node = Some(0);
        core.add_segment();
        assert!(!core.paste_range(0), "stale range refused");
        assert_eq!(core.frame_count(), 5, "nothing inserted");
        assert!(core.status.contains("no longer match"), "status: {}", core.status);
    }

    #[test]
    fn wave11_fig3_bake_animation_writes_appended_slots_on_short_pose_rows() {
        // MCP `fig3_bake_animation` (mcp_tools.rs): on a
        // project with a figure-absent (truncated) pose
        // row, `add_figure_with` appends the baked
        // figure's pose at the row's end — indexing it
        // back at the figure index panicked.
        let mut core = EditorCore::new();
        core.add_figure();
        core.project.frames[0].poses.truncate(1);
        core.enter_3d();
        core.fig3_mark_pose_b();
        core.fig3_bake_animation(3);
        assert_eq!(core.project.figures.len(), 3, "the bake landed");
        assert_eq!(
            core.project.frames[0].poses.len(),
            2,
            "short row keeps its shape; the appended slot took the pose"
        );
        assert_eq!(
            core.project.frames[1].poses.len(),
            3,
            "full rows are figure-wide"
        );
    }

    #[test]
    fn drawing_strokes_attach_to_the_frame() {
        let mut core = EditorCore::new();
        core.begin_draw([100.0, 100.0]);
        core.draw_to([140.0, 120.0]);
        core.draw_to([180.0, 110.0]);
        core.end_draw();
        assert_eq!(core.project.strokes.len(), 1);
        assert_eq!(core.project.strokes[0].points.len(), 3);
        core.clear_frame_drawings();
        assert!(core.project.strokes.is_empty());
    }

    #[test]
    fn pressure_widths_follow_pointer_speed() {
        let mut core = EditorCore::new();
        core.begin_draw([0.0, 0.0]);
        core.draw_to([3.0, 0.0]); // slow 3px step
        core.draw_to([120.0, 0.0]); // fast flick
        core.end_draw();
        let s = &core.project.strokes[0];
        assert_eq!(s.widths.len(), s.points.len(), "widths stay parallel to points");
        assert!((s.width - 4.0).abs() < f32::EPSILON, "base width unchanged");
        for w in &s.widths {
            assert!(
                *w >= 4.0 * 0.45 - 0.01 && *w <= 4.0 * 1.8 + 0.01,
                "width {w} within the pressure range"
            );
        }
        assert!(
            s.widths[1] > s.widths[2],
            "slow point wider than the fast flick: {:?}",
            s.widths
        );
    }

    fn set_brush(core: &mut EditorCore, name: &str) {
        core.brush_index = crate::paint::brush_library()
            .iter()
            .position(|b| b.name == name)
            .unwrap_or_else(|| panic!("no brush named {name}"));
    }

    #[test]
    fn default_pen_brush_leaves_the_recorded_stroke_untouched() {
        // The Pen preset (index 0, the default) must reproduce the
        // pre-library behaviour exactly: no smoothing, no nib
        // profile, base width 4.0.
        let mut core = EditorCore::new();
        assert_eq!(core.brush_index, 0);
        assert_eq!(core.active_brush().name, "Pen");
        core.begin_draw([0.0, 0.0]);
        core.draw_to([10.0, 5.0]);
        core.draw_to([25.0, 5.0]);
        core.end_draw();
        let s = &core.project.strokes[0];
        assert_eq!(s.points, vec![[0.0, 0.0], [10.0, 5.0], [25.0, 5.0]]);
        assert_eq!(s.width, 4.0);
    }

    #[test]
    fn smoothing_brush_adds_points_at_stroke_end() {
        let mut core = EditorCore::new();
        set_brush(&mut core, "Marker"); // width 8, smoothing 1
        core.begin_draw([0.0, 0.0]);
        core.draw_to([30.0, 0.0]);
        core.draw_to([60.0, 30.0]);
        // Before the stroke ends, only the 3 recorded points exist.
        assert_eq!(core.project.strokes[0].points.len(), 3);
        core.end_draw();
        let s = &core.project.strokes[0];
        assert_eq!(s.points.len(), 6, "3 recorded points, Chaikin x2: {:?}", s.points);
        assert_eq!(s.widths.len(), s.points.len(), "widths stay parallel");
        assert_eq!(s.width, 8.0, "the brush sets the stroke's base width");
        // Smoothing pins the endpoints.
        assert_eq!(s.points.first(), Some(&[0.0, 0.0]));
        assert_eq!(s.points.last(), Some(&[60.0, 30.0]));
    }

    #[test]
    fn calligraphy_brush_profiles_widths_at_stroke_end() {
        let mut core = EditorCore::new();
        set_brush(&mut core, "Calligraphy"); // width 7, nib 30 degrees
        core.begin_draw([0.0, 0.0]);
        core.draw_to([40.0, 0.0]);
        core.draw_to([40.0, 40.0]);
        core.end_draw();
        let s = &core.project.strokes[0];
        assert_eq!(s.widths.len(), s.points.len(), "profile is parallel");
        let max = s.widths.iter().copied().fold(f32::MIN, f32::max);
        let min = s.widths.iter().copied().fold(f32::MAX, f32::min);
        assert!(max > min, "widths vary with travel direction: {:?}", s.widths);
        assert!(
            s.widths.iter().all(|&w| w >= 0.15 * 7.0 - 0.01 && w <= 7.0 + 0.01),
            "widths within the nib's range: {:?}",
            s.widths
        );
    }

    #[test]
    fn eraser_tool_splits_strokes_on_the_current_frame() {
        let mut core = EditorCore::new();
        core.project.strokes.push(crate::model::Stroke {
            frame: 0,
            points: (0..=10).map(|i| [i as f32 * 10.0, 0.0]).collect(),
            color: 0xFF0000,
            width: 4.0,
            widths: Vec::new(),
            fill: None,
            spray: None,
            opacity: 1.0,
            scatter: 0.0,
        });
        core.toggle_draw_tool();
        assert_eq!(core.draw_tool, DrawTool::Eraser);
        // Pen width 4 -> eraser radius 8: the press at x=50 removes
        // only that point and splits the stroke; dragging to x=90
        // removes that point too (x=100 is left a singleton and
        // vanishes with it).
        core.begin_draw([50.0, 0.0]);
        assert_eq!(core.project.strokes.len(), 2, "split by the press");
        core.draw_to([90.0, 0.0]);
        core.end_draw();
        assert_eq!(core.project.strokes.len(), 2);
        assert_eq!(core.project.strokes[0].points.last(), Some(&[40.0, 0.0]));
        assert_eq!(
            core.project.strokes[1].points,
            vec![[60.0, 0.0], [70.0, 0.0], [80.0, 0.0]]
        );
        assert!(
            core.status.contains("Eraser: 2 stroke(s) changed"),
            "status reports both erase steps: {}",
            core.status
        );
        // Toggling back restores the brush tool.
        core.toggle_draw_tool();
        assert_eq!(core.draw_tool, DrawTool::Brush);
    }

    #[test]
    fn cycle_brush_walks_the_library_and_wraps() {
        let mut core = EditorCore::new();
        let n = crate::paint::brush_library().len();
        assert_eq!(core.active_brush().name, "Pen");
        for _ in 0..n {
            core.cycle_brush();
        }
        assert_eq!(core.brush_index, 0, "a full cycle wraps to the start");
        assert_eq!(core.active_brush().name, "Pen");
        core.cycle_brush();
        assert_eq!(core.active_brush().name, "Pencil");
        assert!(core.status.contains("Pencil"), "status names the brush: {}", core.status);
    }

    #[test]
    fn fill_appends_scanline_stroke_and_marks_outline() {
        let mut core = EditorCore::new();
        core.begin_draw([100.0, 100.0]);
        core.draw_to([200.0, 100.0]);
        core.draw_to([200.0, 200.0]);
        core.draw_to([100.0, 200.0]);
        core.end_draw();
        let color = core.project.strokes[0].color;
        core.fill_current_stroke();
        assert_eq!(core.project.strokes.len(), 2);
        assert_eq!(core.project.strokes[0].fill, Some(color));
        let fill = &core.project.strokes[1];
        assert_eq!(fill.color, color);
        assert_eq!(fill.frame, 0);
        assert!(fill.points.len() > 8, "fill has many scan points: {}", fill.points.len());
        for p in &fill.points {
            assert!(
                p[0] >= 99.9 && p[0] <= 200.1 && p[1] >= 99.9 && p[1] <= 200.1,
                "fill point within the shape bounds: {p:?}"
            );
        }
        core.undo();
        assert_eq!(core.project.strokes.len(), 1);
        assert_eq!(core.project.strokes[0].fill, None);
    }

    #[test]
    fn fill_without_a_shape_is_a_no_op() {
        let mut core = EditorCore::new();
        core.fill_current_stroke();
        assert!(core.project.strokes.is_empty());
        core.begin_draw([10.0, 10.0]);
        core.draw_to([60.0, 10.0]);
        core.end_draw();
        core.fill_current_stroke(); // only 2 points: not a shape
        assert_eq!(core.project.strokes.len(), 1);
        assert_eq!(core.project.strokes[0].fill, None);
    }

    #[test]
    fn lasso_erases_only_mostly_inside_strokes() {
        let mut core = EditorCore::new();
        // Stroke fully inside the lasso box.
        core.begin_draw([10.0, 10.0]);
        core.draw_to([20.0, 20.0]);
        core.draw_to([30.0, 10.0]);
        core.end_draw();
        // Stroke fully outside.
        core.begin_draw([400.0, 400.0]);
        core.draw_to([430.0, 420.0]);
        core.draw_to([460.0, 400.0]);
        core.end_draw();
        // Straddling stroke: 1 of 3 points inside -> survives.
        core.begin_draw([20.0, 20.0]);
        core.draw_to([400.0, 20.0]);
        core.draw_to([20.0, 400.0]);
        core.end_draw();
        let lasso = vec![[0.0, 0.0], [50.0, 0.0], [50.0, 50.0], [0.0, 50.0]];
        core.lasso_erase(&lasso);
        assert_eq!(core.project.strokes.len(), 2);
        core.undo();
        assert_eq!(core.project.strokes.len(), 3);
        // A degenerate lasso erases nothing.
        core.lasso_erase(&[[0.0, 0.0], [10.0, 10.0]]);
        assert_eq!(core.project.strokes.len(), 3);
    }

    #[test]
    fn movieclip_loops_independently_of_the_timeline() {
        let mut core = EditorCore::new();
        core.make_clip_from_figure();
        assert_eq!(core.project.clips.len(), 1);
        assert_eq!(core.project.figures[0].clip, Some(0));
        // Corrupt the timeline pose; the clip pose must not follow.
        core.project.frames[0].poses[0].root = [0.0, 0.0];
        let poses = core.project.poses_at(0.0);
        assert!(poses[0].root[0] > 400.0, "clip kept its own root, got {:?}", poses[0].root);
    }

    #[test]
    fn inbetween_lands_halfway() {
        let mut core = EditorCore::new();
        core.goto(0);
        core.insert_inbetween();
        assert_eq!(core.frame_count(), 6);
        // Demo frames 0 and 1 have root_x 480 and 494: midpoint 487.
        let root = core.project.frames[1].poses[0].root;
        assert!((root[0] - 487.0).abs() < 0.01, "root {root:?}");
    }

    #[test]
    fn wave10_delete_keyframe_holds_previous_pose_and_undoes() {
        let mut c = EditorCore::new();
        // The demo animates every frame, so frame 2 is a key.
        assert!(crate::stats::figure_keyframes(&c.project, 0).contains(&2));
        let authored = c.project.frames[2].poses[0].clone();
        let depth = c.undo_depth();
        assert!(c.delete_keyframe(0, 2));
        assert_eq!(c.undo_depth(), depth + 1, "one snapshot for the delete");
        // The frame now holds frame 1's pose exactly...
        assert_eq!(c.project.frames[2].poses[0].root, c.project.frames[1].poses[0].root);
        assert_eq!(c.project.frames[2].poses[0].angles, c.project.frames[1].poses[0].angles);
        // ...so the keyed set no longer lists it, and the pose
        // the renderer samples at frame 2 is the held one.
        assert!(!crate::stats::figure_keyframes(&c.project, 0).contains(&2));
        assert_eq!(c.project.poses_at(2.0)[0].root, c.project.frames[1].poses[0].root);
        // Undo restores the authored pose and the key.
        c.undo();
        assert_eq!(c.project.frames[2].poses[0].root, authored.root);
        assert!(crate::stats::figure_keyframes(&c.project, 0).contains(&2));
    }

    #[test]
    fn wave10_delete_keyframe_refuses_frame_zero() {
        let mut c = EditorCore::new();
        assert!(crate::stats::figure_keyframes(&c.project, 0).contains(&0));
        let depth = c.undo_depth();
        assert!(!c.delete_keyframe(0, 0));
        assert_eq!(c.undo_depth(), depth, "a refusal takes no snapshot");
        assert!(c.status.contains("base pose"), "status: {}", c.status);
        assert!(crate::stats::figure_keyframes(&c.project, 0).contains(&0));
    }

    #[test]
    fn wave10_delete_keyframe_refuses_non_keys() {
        let mut c = EditorCore::new();
        // Hold frame 2 at frame 1's pose: frame 2 stops being a key.
        c.project.frames[2].poses[0] = c.project.frames[1].poses[0].clone();
        assert!(!crate::stats::figure_keyframes(&c.project, 0).contains(&2));
        let depth = c.undo_depth();
        assert!(!c.delete_keyframe(0, 2));
        assert!(c.status.contains("No keyframe"), "status: {}", c.status);
        // Out-of-range figure / frame are refusals too, not panics.
        assert!(!c.delete_keyframe(9, 1));
        assert!(!c.delete_keyframe(0, 99));
        assert_eq!(c.undo_depth(), depth, "refusals take no snapshot");
        // A figure that appears mid-timeline — absent at frame 1,
        // present at frame 2 — keys there, but that keyframe has
        // no earlier pose to hold and is refused as well.
        let mut c = EditorCore::new();
        c.project.frames[1].poses.clear();
        assert!(crate::stats::figure_keyframes(&c.project, 0).contains(&2));
        assert!(!c.delete_keyframe(0, 2));
        assert!(c.status.contains("appears at frame"), "status: {}", c.status);
    }

    #[test]
    fn wave10_keyframe_navigation_boundaries() {
        let mut c = EditorCore::new();
        // Reshape the demo into sparse keys {0, 2, 4}: hold frame 1
        // at frame 0's pose and frame 3 at frame 2's pose.
        c.project.frames[1].poses[0] = c.project.frames[0].poses[0].clone();
        c.project.frames[3].poses[0] = c.project.frames[2].poses[0].clone();
        assert_eq!(crate::stats::figure_keyframes(&c.project, 0), vec![0, 2, 4]);
        // Strictly-after / strictly-before stepping.
        assert_eq!(c.next_keyframe(0, 0), Some(2));
        assert_eq!(c.next_keyframe(0, 1), Some(2));
        assert_eq!(c.next_keyframe(0, 2), Some(4));
        assert_eq!(c.prev_keyframe(0, 4), Some(2));
        assert_eq!(c.prev_keyframe(0, 3), Some(2));
        assert_eq!(c.prev_keyframe(0, 2), Some(0));
        // Boundaries: before the first key and after the last.
        assert_eq!(c.prev_keyframe(0, 0), None);
        assert_eq!(c.next_keyframe(0, 4), None);
        assert_eq!(c.next_keyframe(0, 99), None);
        // Empty: an out-of-range figure has no keys at all.
        assert_eq!(c.next_keyframe(9, 0), None);
        assert_eq!(c.prev_keyframe(9, 4), None);
        // The goto wrappers move the playhead and stop at the ends.
        c.goto(0);
        assert!(c.goto_next_keyframe(0));
        assert_eq!(c.current, 2);
        assert!(c.goto_next_keyframe(0));
        assert_eq!(c.current, 4);
        assert!(!c.goto_next_keyframe(0), "no key after the last");
        assert_eq!(c.current, 4, "a failed jump leaves the playhead");
        assert!(c.goto_prev_keyframe(0));
        assert_eq!(c.current, 2);
        c.goto(0);
        assert!(!c.goto_prev_keyframe(0));
        assert_eq!(c.current, 0);
    }

    #[test]
    fn wave7_cycle_bend_steps_and_storage() {
        let mut c = EditorCore::new();
        let nodes = c.project.figures[0].def.nodes.len();
        // No joint selected: explains itself, changes nothing.
        c.cycle_bend();
        assert!(c.project.frames[0].bends.is_empty());
        assert!(c.status.contains("Select a joint"), "status: {}", c.status);
        // The origin has no segment to bend.
        c.selected_node = Some(0);
        c.cycle_bend();
        assert!(c.project.frames[0].bends.is_empty());
        assert!(c.status.contains("origin"), "status: {}", c.status);
        // First step lands and is visible through bends_at.
        c.selected_node = Some(2);
        c.cycle_bend();
        {
            let b = &c.project.frames[0].bends;
            assert_eq!(b.len(), 1, "one figure entry");
            assert_eq!(b[0].len(), nodes, "per-node storage shape");
            assert!((b[0][2] - 0.35).abs() < 1e-6);
        }
        assert!((c.project.bends_at(0.0, 0)[2] - 0.35).abs() < 1e-6);
        // The rest of the cycle: +0.7, -0.35, -0.7, back to 0.
        for w in [0.7f32, -0.35, -0.7, 0.0] {
            c.cycle_bend();
            let got = c.project.frames[0].bends[0][2];
            assert!((got - w).abs() < 1e-6, "bend {got} vs {w}");
        }
        // All-zero again: bends_at reports "no bends" (empty).
        assert!(c.project.bends_at(0.0, 0).is_empty());
    }

    #[test]
    fn wave5_locked_figure_refuses_drag() {
        let mut c = EditorCore::new();
        let pos = c.positions(0, 0);
        let hand = pos[pos.len() - 1];
        c.project.figures[0].locked = true;
        assert!(!c.begin_drag(hand), "locked figure must refuse drags");
        c.toggle_figure_lock(0);
        assert!(c.begin_drag(hand), "unlocked figure drags again");
        c.end_drag();
    }

    #[test]
    fn wave_p_end_drag_recaps_only_real_moves() {
        // Wave P (P6, feedback completeness): a pose drag
        // that moved the pose ends with the status recap;
        // a press that changed nothing posts none.
        let mut c = EditorCore::new();
        let root = c.positions(0, 0)[0];
        let before = c.status.clone();
        assert!(c.begin_drag(root));
        c.end_drag();
        assert_eq!(c.status, before, "an unmoved press posts no recap");
        assert!(c.begin_drag(root));
        c.drag_to([root[0] + 40.0, root[1] + 25.0]);
        c.end_drag();
        assert_eq!(c.status, "Posed figure 1 at frame 1.");
        // Camera drags follow the same discipline.
        let mut c = EditorCore::new();
        let before = c.status.clone();
        c.begin_camera_drag([5.0, 5.0]);
        c.end_drag();
        assert_eq!(c.status, before, "an untravelled camera press posts no recap");
        c.begin_camera_drag([5.0, 5.0]);
        c.drag_to([35.0, 15.0]);
        c.end_drag();
        assert_eq!(c.status, "Camera key moved at frame 1.");
    }

    #[test]
    fn wave_ac_end_drag_recap_compares_each_frame_to_its_own_baseline() {
        // Wave AC (AC4): the arrow-key `goto` does not end
        // a live pose drag — the drag chases `current`, so
        // a step retargets the gesture's writes mid-flight.
        // The gesture-end recap must therefore compare
        // every frame the gesture wrote against THAT
        // frame's pre-gesture pose. The pre-AC4 shape
        // compared the release frame against the grab
        // frame's baseline, which posted the recap for
        // presses that never touched the release frame
        // and swallowed it for real edits whenever the
        // two frames' poses happened to agree. The write
        // chase itself is unchanged (AB9's census: the
        // pose drag carries no grab-time payload, so
        // chasing is its honest shape) — only the recap's
        // comparison crossed frames.
        //
        // (1) An unmoved press, stepped, released: silent,
        // even though the stepped-to frame's pose differs
        // from the grab frame's.
        let mut c = EditorCore::new();
        let root = c.positions(0, 0)[0];
        c.project.frames[2].poses[0].root = [root[0] + 120.0, root[1] + 60.0];
        let before = c.status.clone();
        assert!(c.begin_drag(root));
        c.goto(2);
        c.end_drag();
        assert_eq!(c.status, before, "an unmoved press posts no recap, stepped or not");
        //
        // (2) A real move at the grab frame, stepped to a
        // frame holding the SAME pose, released unmoved:
        // the recap fires and names the grab frame.
        let mut c = EditorCore::new();
        let root = c.positions(0, 0)[0];
        let grab_pose = c.project.frames[0].poses[0].clone();
        c.project.frames[2].poses[0] = grab_pose;
        assert!(c.begin_drag(root));
        c.drag_to([root[0] + 40.0, root[1] + 25.0]);
        c.goto(2);
        c.end_drag();
        assert_eq!(c.status, "Posed figure 1 at frame 1.");
        //
        // (3) Press, step, THEN the first move lands on
        // the stepped-to frame: the recap names that
        // frame and the grab frame stays byte-untouched.
        // Frame 2 starts displaced; the move drags its
        // root exactly onto the grab frame's root — equal
        // to the OLD baseline, different from its own.
        let mut c = EditorCore::new();
        let root = c.positions(0, 0)[0];
        let grab_pose = c.project.frames[0].poses[0].clone();
        c.project.frames[2].poses[0].root = [root[0] + 120.0, root[1] + 60.0];
        assert!(c.begin_drag(root));
        c.goto(2);
        c.drag_to(root);
        c.end_drag();
        assert_eq!(c.status, "Posed figure 1 at frame 3.");
        let f0 = &c.project.frames[0].poses[0];
        assert_eq!(f0.root, grab_pose.root);
        assert_eq!(f0.angles, grab_pose.angles);
        //
        // (4) Step-and-continue: both frames keep their
        // writes, the recap names the last changed frame,
        // and the gesture's ONE snapshot (taken at the
        // grab) undoes both frames' writes together —
        // the commit never crossed frames, only the
        // recap did.
        let mut c = EditorCore::new();
        let root = c.positions(0, 0)[0];
        let pre0 = c.project.frames[0].poses[0].clone();
        let pre2 = c.project.frames[2].poses[0].clone();
        assert!(c.begin_drag(root));
        c.drag_to([root[0] + 40.0, root[1]]);
        c.goto(2);
        let root2 = c.project.frames[2].poses[0].root;
        c.drag_to([root2[0] - 30.0, root2[1]]);
        c.end_drag();
        assert_eq!(c.status, "Posed figure 1 at frame 3.");
        c.undo();
        let f0 = &c.project.frames[0].poses[0];
        assert_eq!(f0.root, pre0.root);
        assert_eq!(f0.angles, pre0.angles);
        let f2 = &c.project.frames[2].poses[0];
        assert_eq!(f2.root, pre2.root);
        assert_eq!(f2.angles, pre2.angles);
    }

    #[test]
    fn wave_ac_camera_drag_recap_compares_each_frame_to_its_own_baseline() {
        // Wave AC (AC4): the camera drag is the pose
        // drag's P6 twin (same end_drag charter, same
        // chase, baselines documented as mirroring), and
        // it carried the identical crossed comparison —
        // `camera_here()` at the release frame against
        // the grab frame's camera. Same repair: per-frame
        // baselines via `camera_at`.
        //
        // An untravelled camera press stepped to a frame
        // whose evaluated camera differs: silent.
        let mut c = EditorCore::new();
        c.project.set_camera_key(0, 0.0, 0.0, 1.0);
        c.project.set_camera_key(2, 100.0, 0.0, 1.0);
        let before = c.status.clone();
        c.begin_camera_drag([5.0, 5.0]);
        c.goto(2);
        c.end_drag();
        assert_eq!(c.status, before, "an untravelled camera press posts no recap, stepped or not");
        //
        // A real pan at the grab frame, stepped to a frame
        // whose camera equals the grab baseline: the recap
        // fires and names the grab frame.
        let mut c = EditorCore::new();
        c.project.set_camera_key(0, 0.0, 0.0, 1.0);
        c.project.set_camera_key(2, 0.0, 0.0, 1.0);
        c.begin_camera_drag([5.0, 5.0]);
        c.drag_to([35.0, 15.0]);
        c.goto(2);
        c.end_drag();
        assert_eq!(c.status, "Camera key moved at frame 1.");
    }

    #[test]
    fn wave5_graph_key_drives_positions() {
        let mut c = EditorCore::new();
        c.selected_node = Some(2);
        c.key_graph_angle();
        assert_eq!(c.project.channels.len(), 1);
        let base = c.positions_f(0.0, 0);
        // Rewrite the channel to a very different angle at frame 0 and
        // confirm positions_f (the editor's sampling path) follows it.
        c.project.channels[0].keys[0].value += 120.0;
        // Hermite with a single key holds its value everywhere.
        let moved = c.positions_f(0.0, 0);
        assert!(
            (moved[2][0] - base[2][0]).abs() + (moved[2][1] - base[2][1]).abs() > 1.0,
            "graph channel must change the sampled pose"
        );
        c.clear_graph();
        assert!(c.project.channels.is_empty());
    }

    #[test]
    fn key_graph_angle_refuses_the_origin_without_a_snapshot() {
        // Wave AB (AB4): evaluate never reads the origin's
        // stored angle, so an Angle(0) channel is inert at
        // render; Key angle refuses it before any snapshot —
        // AA5's Bend(0) call, extended to the angle door.
        let mut c = EditorCore::new();
        c.selected_node = Some(0);
        let depth = c.undo_depth();
        c.key_graph_angle();
        assert_eq!(c.status.as_str(), "The origin has no angle to key.");
        assert!(c.project.channels.is_empty(), "no channel authored");
        assert_eq!(c.undo_depth(), depth, "the refusal spends no undo step");
        // An Angle(0) channel a saved project already
        // carries gains no key and keeps the one it has:
        // the refusal blocks authoring, never stored data.
        c.project.channels = vec![crate::graph::Channel {
            figure: 0,
            target: crate::graph::ChannelTarget::Angle(0),
            keys: vec![crate::graph::Key::flat(0.0, 33.0)],
        }];
        c.key_graph_angle();
        assert_eq!(c.project.channels[0].keys.len(), 1, "the stored key is untouched");
        assert_eq!(c.project.channels[0].keys[0].value, 33.0);
        assert_eq!(c.undo_depth(), depth, "still no snapshot");
        // A real joint still keys.
        c.selected_node = Some(1);
        c.key_graph_angle();
        assert_eq!(c.project.channels.len(), 2, "joint 1 authored its channel");
        assert_eq!(
            c.project.channels[1].target,
            crate::graph::ChannelTarget::Angle(1)
        );
    }

    #[test]
    fn wave5_move_figure_full_remaps_selection_and_solo() {
        let mut c = EditorCore::new();
        c.add_figure(); // two figures
        assert_eq!(c.project.figures.len(), 2);
        c.selected_figure = 0;
        c.solo = Some(0);
        c.move_figure_full(1);
        assert_eq!(c.selected_figure, 1);
        assert_eq!(c.solo, Some(1));
    }

    pub(super) fn audio_temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "styk-editor-audio-{}-{}",
            std::process::id(),
            name
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    pub(super) fn write_editor_wav(
        dir: &PathBuf,
        name: &str,
        data: &crate::audio_edit::WavData,
    ) -> String {
        let path = dir.join(name);
        std::fs::write(&path, crate::audio_edit::write_wav(data)).unwrap();
        path.to_string_lossy().into_owned()
    }

    pub(super) fn const_editor_wav(
        rate: u32,
        frames: usize,
        value: f32,
    ) -> crate::audio_edit::WavData {
        crate::audio_edit::WavData {
            sample_rate: rate,
            channels: 1,
            samples: vec![value; frames],
        }
    }

    fn audio_editor_core(fps: f32) -> EditorCore {
        let mut core = EditorCore::new();
        core.project.fps = fps;
        core.project.audio.clear();
        core
    }

    fn assert_audio_clip(
        core: &EditorCore,
        idx: usize,
        path: &str,
        start_frame: usize,
    ) {
        assert_eq!(core.project.audio[idx].path, path);
        assert_eq!(core.project.audio[idx].start_frame, start_frame);
    }

    #[test]
    fn wave6_audio_normalize_writes_norm_file_and_repoints_clip() {
        let dir = audio_temp_dir("normalize");
        let source_data = crate::audio_edit::WavData {
            sample_rate: 8000,
            channels: 1,
            samples: vec![0.0, 0.25, -0.125, 0.05],
        };
        let source = write_editor_wav(&dir, "voice.wav", &source_data);
        let mut core = audio_editor_core(12.0);
        core.project.audio.push(crate::model::AudioClip {
            path: source.clone(),
            start_frame: 3,
            gain: 1.0,
            muted: false,
        });

        core.audio_normalize(0);

        let expected = dir.join("voice.norm.wav");
        assert_audio_clip(&core, 0, &expected.to_string_lossy(), 3);
        assert!(expected.exists(), "normalised file should be written");
        let out = crate::audio_edit::read_wav(&expected.to_string_lossy()).unwrap();
        assert!(
            (crate::audio_edit::peak(&out) - 0.9).abs() < 0.001,
            "normalised peak {}",
            crate::audio_edit::peak(&out)
        );
        let original = crate::audio_edit::read_wav(&source).unwrap();
        assert!((crate::audio_edit::peak(&original) - 0.25).abs() < 0.001);
        assert!(core.status.contains("gain 3.60x"), "status: {}", core.status);

        // A bad index changes neither the clip list nor its entries.
        core.audio_normalize(99);
        assert_eq!(core.project.audio.len(), 1);
        assert_audio_clip(&core, 0, &expected.to_string_lossy(), 3);
        assert!(core.status.contains("failed"), "status: {}", core.status);
    }

    #[test]
    fn wave6_audio_normalize_rejects_non_wav_without_changes() {
        let dir = audio_temp_dir("normalize-bad");
        let bad_path = dir.join("bad.wav");
        std::fs::write(&bad_path, b"this is not a wav").unwrap();
        let source = bad_path.to_string_lossy().into_owned();
        let mut core = audio_editor_core(12.0);
        core.project.audio.push(crate::model::AudioClip {
            path: source.clone(),
            start_frame: 4,
            gain: 1.0,
            muted: false,
        });

        core.audio_normalize(0);

        assert_eq!(core.project.audio.len(), 1);
        assert_audio_clip(&core, 0, &source, 4);
        assert!(!dir.join("bad.norm.wav").exists());
        assert!(core.status.contains("Audio normalise failed"), "status: {}", core.status);
    }

    #[test]
    fn wave6_audio_trim_writes_trim_file_and_shifts_start_frame() {
        let dir = audio_temp_dir("trim");
        let samples: Vec<f32> = (0..8000)
            .map(|i| {
                if i < 2000 {
                    0.1
                } else if i < 4000 {
                    0.2
                } else {
                    0.3
                }
            })
            .collect();
        let source_data = crate::audio_edit::WavData {
            sample_rate: 8000,
            channels: 1,
            samples,
        };
        let source = write_editor_wav(&dir, "clip.wav", &source_data);
        let mut core = audio_editor_core(12.0);
        core.project.audio.push(crate::model::AudioClip {
            path: source,
            start_frame: 7,
            gain: 1.0,
            muted: false,
        });

        core.audio_trim(0, 0.25, 0.75);

        let expected = dir.join("clip.trim.wav");
        // At 12 fps, trimming a 0.25s head shifts frame 7 by 3 to 10.
        assert_audio_clip(&core, 0, &expected.to_string_lossy(), 10);
        let trimmed = crate::audio_edit::read_wav(&expected.to_string_lossy()).unwrap();
        assert_eq!(trimmed.frames(), 4000);
        assert!((trimmed.samples[0] - 0.2).abs() < 0.001);
        assert!((trimmed.samples[3999] - 0.3).abs() < 0.001);
    }

    #[test]
    fn wave6_audio_trim_rejects_empty_range_without_changes() {
        let dir = audio_temp_dir("trim-empty");
        let source = write_editor_wav(&dir, "range.wav", &const_editor_wav(8000, 8000, 0.25));
        let mut core = audio_editor_core(12.0);
        core.project.audio.push(crate::model::AudioClip {
            path: source.clone(),
            start_frame: 9,
            gain: 1.0,
            muted: false,
        });

        core.audio_trim(0, 0.75, 0.25);

        assert_eq!(core.project.audio.len(), 1);
        assert_audio_clip(&core, 0, &source, 9);
        assert!(!dir.join("range.trim.wav").exists());
        assert!(core.status.contains("Audio trim failed"), "status: {}", core.status);
    }

    #[test]
    fn wave6_audio_split_materialises_both_halves_and_inserts_second_clip() {
        let dir = audio_temp_dir("split");
        let mut samples = vec![0.25; 4000];
        samples.extend(vec![-0.5; 4000]);
        let source_data = crate::audio_edit::WavData {
            sample_rate: 8000,
            channels: 1,
            samples,
        };
        let source = write_editor_wav(&dir, "take.wav", &source_data);
        let other = write_editor_wav(&dir, "other.wav", &const_editor_wav(8000, 800, 0.1));
        let mut core = audio_editor_core(12.0);
        core.project.audio.push(crate::model::AudioClip {
            path: source,
            start_frame: 12,
            gain: 1.0,
            muted: false,
        });
        core.project.audio.push(crate::model::AudioClip {
            path: other.clone(),
            start_frame: 30,
            gain: 1.0,
            muted: false,
        });

        // The source lasts 12 timeline frames (1s at 12 fps), so
        // frame 18 is its midpoint: 0.5s / 4000 sample frames in.
        core.audio_split(0, 18);

        let first_path = dir.join("take.a.wav");
        let second_path = dir.join("take.b.wav");
        assert_eq!(core.project.audio.len(), 3);
        assert_audio_clip(&core, 0, &first_path.to_string_lossy(), 12);
        assert_audio_clip(&core, 1, &second_path.to_string_lossy(), 18);
        assert_audio_clip(&core, 2, &other, 30);
        let first = crate::audio_edit::read_wav(&first_path.to_string_lossy()).unwrap();
        let second = crate::audio_edit::read_wav(&second_path.to_string_lossy()).unwrap();
        assert_eq!(first.frames() + second.frames(), source_data.frames());
        assert_eq!(first.frames(), 4000);
        assert_eq!(second.frames(), 4000);
        assert!(first.samples.iter().all(|s| (*s - 0.25).abs() < 0.001));
        assert!(second.samples.iter().all(|s| (*s + 0.5).abs() < 0.001));
    }

    #[test]
    fn wave6_audio_split_rejects_boundaries_without_changes() {
        let dir = audio_temp_dir("split-boundary");
        let source = write_editor_wav(&dir, "edge.wav", &const_editor_wav(8000, 8000, 0.25));
        let mut core = audio_editor_core(12.0);
        core.project.audio.push(crate::model::AudioClip {
            path: source.clone(),
            start_frame: 12,
            gain: 1.0,
            muted: false,
        });

        // The clip spans timeline frames 12..24; neither endpoint is
        // strictly inside it.
        core.audio_split(0, 12);
        assert_eq!(core.project.audio.len(), 1);
        assert_audio_clip(&core, 0, &source, 12);
        core.audio_split(0, 24);
        assert_eq!(core.project.audio.len(), 1);
        assert_audio_clip(&core, 0, &source, 12);
        assert!(!dir.join("edge.a.wav").exists());
        assert!(!dir.join("edge.b.wav").exists());
        assert!(core.status.contains("Audio split failed"), "status: {}", core.status);
    }

    #[test]
    fn wave6_audio_crossfade_uses_overlap_and_replaces_pair() {
        let dir = audio_temp_dir("crossfade-overlap");
        let first = write_editor_wav(&dir, "a.wav", &const_editor_wav(8000, 4000, 0.4));
        let second = write_editor_wav(&dir, "b.wav", &const_editor_wav(8000, 4000, 0.8));
        let mut core = audio_editor_core(10.0);
        core.project.audio.push(crate::model::AudioClip {
            path: first,
            start_frame: 0,
            gain: 1.0,
            muted: false,
        });
        core.project.audio.push(crate::model::AudioClip {
            path: second,
            start_frame: 3,
            gain: 1.0,
            muted: false,
        });

        // Each clip lasts 0.5s. The second starts at 0.3s, so the
        // overlap (and therefore the fade) is 0.2s / 1600 frames.
        core.audio_crossfade(0);

        let expected = dir.join("a.xfade.wav");
        assert_eq!(core.project.audio.len(), 1);
        assert_audio_clip(&core, 0, &expected.to_string_lossy(), 0);
        let mixed = crate::audio_edit::read_wav(&expected.to_string_lossy()).unwrap();
        assert_eq!(mixed.frames(), 4000 + 4000 - 1600);
        assert!((mixed.samples[0] - 0.4).abs() < 0.001);
        assert!(
            (mixed.samples[3200] - 0.6).abs() < 0.001,
            "fade midpoint {}",
            mixed.samples[3200]
        );
        assert!((mixed.samples[4000] - 0.8).abs() < 0.001);
        assert!(core.status.contains("0.20s"), "status: {}", core.status);
    }

    #[test]
    fn wave6_audio_crossfade_adjacent_pair_uses_one_second_fade() {
        let dir = audio_temp_dir("crossfade-adjacent");
        let first = write_editor_wav(&dir, "a.wav", &const_editor_wav(8000, 12000, 0.4));
        let second = write_editor_wav(&dir, "b.wav", &const_editor_wav(8000, 12000, 0.8));
        let mut core = audio_editor_core(10.0);
        core.project.audio.push(crate::model::AudioClip {
            path: first,
            start_frame: 0,
            gain: 1.0,
            muted: false,
        });
        core.project.audio.push(crate::model::AudioClip {
            path: second,
            start_frame: 15,
            gain: 1.0,
            muted: false,
        });

        // Two adjacent 1.5s clips: the default fade is 1.0s, so
        // the joined file lasts 1.5 + 1.5 - 1.0 = 2.0s.
        core.audio_crossfade(0);

        let expected = dir.join("a.xfade.wav");
        assert_eq!(core.project.audio.len(), 1);
        assert_audio_clip(&core, 0, &expected.to_string_lossy(), 0);
        let mixed = crate::audio_edit::read_wav(&expected.to_string_lossy()).unwrap();
        assert_eq!(mixed.frames(), 16000);
        assert!((mixed.samples[0] - 0.4).abs() < 0.001);
        assert!((mixed.samples[8000] - 0.6).abs() < 0.001);
        assert!((mixed.samples[15999] - 0.8).abs() < 0.001);
    }

    #[test]
    fn wave6_audio_crossfade_rejects_gap_without_changes() {
        let dir = audio_temp_dir("crossfade-gap");
        let first = write_editor_wav(&dir, "a.wav", &const_editor_wav(8000, 4000, 0.4));
        let second = write_editor_wav(&dir, "b.wav", &const_editor_wav(8000, 4000, 0.8));
        let mut core = audio_editor_core(10.0);
        core.project.audio.push(crate::model::AudioClip {
            path: first.clone(),
            start_frame: 0,
            gain: 1.0,
            muted: false,
        });
        core.project.audio.push(crate::model::AudioClip {
            path: second.clone(),
            start_frame: 10,
            gain: 1.0,
            muted: false,
        });

        // The first 0.5s clip ends at frame 5; the second starts at
        // frame 10, leaving a 0.5s gap.
        core.audio_crossfade(0);

        assert_eq!(core.project.audio.len(), 2);
        assert_audio_clip(&core, 0, &first, 0);
        assert_audio_clip(&core, 1, &second, 10);
        assert!(!dir.join("a.xfade.wav").exists());
        assert!(core.status.contains("Audio crossfade failed"), "status: {}", core.status);
    }

    // ---- Wave 7: timeline depth -------------------------------------

    /// Tag every demo frame with a unique root x, duration and
    /// bend so frame order is directly observable.
    fn tag_frames(core: &mut EditorCore) {
        for (i, frame) in core.project.frames.iter_mut().enumerate() {
            frame.poses[0].root = [i as f32 * 10.0, 100.0];
            frame.duration = (i + 1) as f32;
            frame.bends = vec![vec![i as f32]];
        }
        core.project.frames[2].easing = Some(crate::model::Easing::EaseIn);
    }

    fn root_xs(core: &EditorCore) -> Vec<f32> {
        core.project
            .frames
            .iter()
            .map(|f| f.poses[0].root[0])
            .collect()
    }

    #[test]
    fn wave7_loop_range_orders_and_clamps() {
        let mut core = EditorCore::new(); // 5 demo frames
        core.set_loop_range(3, 1);
        assert_eq!(core.loop_range, Some((1, 3)), "endpoints are ordered");
        core.set_loop_range(0, 99);
        assert_eq!(core.loop_range, Some((0, 4)), "clamped to the timeline");
        core.set_loop_range(2, 2);
        assert_eq!(core.loop_range, Some((2, 2)), "a single-frame loop is valid");
        core.clear_loop_range();
        assert_eq!(core.loop_range, None);
    }

    #[test]
    fn wave7_loop_range_wraps_playback_at_its_end() {
        let mut core = EditorCore::new(); // fps 12, all durations 1.0
        core.set_loop_range(1, 3);
        // One authored frame of playback per 1/12 s.
        let step = 1.0 / core.project.fps;
        core.goto(1);
        core.advance_play(step);
        assert_eq!(core.current, 2, "inside the range: no wrap");
        core.goto(3);
        core.advance_play(step);
        assert_eq!(core.current, 1, "past frame 3 the playhead wraps to the range start");
        assert!((core.playhead - 1.0).abs() < 1e-4, "playhead: {}", core.playhead);
        // Fractional overshoot survives the wrap.
        core.goto(3);
        core.advance_play(1.5 * step);
        assert_eq!(core.current, 1);
        assert!((core.playhead - 1.5).abs() < 1e-4, "playhead: {}", core.playhead);
        // A single-frame loop pins playback to that frame.
        core.set_loop_range(2, 2);
        core.goto(2);
        core.advance_play(step);
        assert_eq!(core.current, 2);
        core.advance_play(step);
        assert_eq!(core.current, 2);
    }

    #[test]
    fn wave7_loop_range_survives_goto_outside_it() {
        let mut core = EditorCore::new();
        core.set_loop_range(2, 3);
        core.goto(0); // before the range
        assert_eq!(core.loop_range, Some((2, 3)), "goto leaves the range intact");
        // Playing forward from before the range enters it, then wraps.
        let step = 1.0 / core.project.fps;
        core.advance_play(step);
        assert_eq!(core.current, 1);
        core.advance_play(step);
        assert_eq!(core.current, 2);
        core.advance_play(step);
        assert_eq!(core.current, 3);
        core.advance_play(step);
        assert_eq!(core.current, 2, "wrapped at the range end");
        assert_eq!(core.loop_range, Some((2, 3)), "range intact after playback");
    }

    #[test]
    fn wave7_no_loop_range_playback_unchanged() {
        let mut core = EditorCore::new();
        assert_eq!(core.loop_range, None);
        let step = 1.0 / core.project.fps;
        core.goto(4);
        core.advance_play(step);
        assert_eq!(core.current, 0, "wraps around the whole timeline, as before");
        assert!(core.playhead.abs() < 1e-4, "playhead: {}", core.playhead);
        core.advance_play(step);
        assert_eq!(core.current, 1);
    }

    #[test]
    fn roadmap_f6_advance_play_walks_durations_across_boundaries() {
        // Roadmap Wave 1 (RM7 / F-6): a tick is charged frame
        // by frame at each frame's own duration. Frame 0 lasts
        // 1 beat and frame 1 lasts 4, so a 2-beat tick from the
        // top crosses frame 0 (1 beat) and spends its second
        // beat inside frame 1 — a quarter of that frame — for
        // a playhead of 1.25. The pre-RM7 integrator charged
        // both beats at frame 0's rate and landed on frame 2.
        let mut core = EditorCore::new();
        core.project.fps = 24.0;
        core.project.frames[0].duration = 1.0;
        core.project.frames[1].duration = 4.0;
        core.project.frames[2].duration = 1.5;
        core.project.frames[3].duration = 0.5;
        core.project.frames[4].duration = 1.0;
        core.goto(0);
        core.advance_play(2.0 / 24.0);
        assert_eq!(core.current, 1, "crossed into the held frame");
        assert!(
            (core.playhead - 1.25).abs() < 1e-4,
            "playhead: {}",
            core.playhead
        );
        // Three more beats finish frame 1 exactly (1 + 3 of its
        // 4 beats spent), landing on frame 2's first instant.
        core.advance_play(3.0 / 24.0);
        assert_eq!(core.current, 2);
        assert!((core.playhead - 2.0).abs() < 1e-4, "playhead: {}", core.playhead);
        // Fractional holds are charged as authored while they
        // are left: 1.5 beats leaves frame 2, 0.5 leaves frame 3.
        core.advance_play(1.5 / 24.0);
        assert_eq!(core.current, 3, "the 1.5-beat hold lasted 1.5 beats");
        core.advance_play(0.5 / 24.0);
        assert_eq!(core.current, 4, "the 0.5-beat hold lasted 0.5 beats");
        // One more beat wraps the whole 8-beat timeline.
        core.advance_play(1.0 / 24.0);
        assert_eq!(core.current, 0);
        assert!(core.playhead.abs() < 1e-4, "playhead: {}", core.playhead);
    }

    #[test]
    fn roadmap_f6_advance_play_loop_range_folds_in_beats() {
        // The loop fold is measured in beats, not frame counts:
        // frames 1..=2 below span 1.5 + 0.5 = 2 beats, so a
        // 2-beat tick from the range's start returns exactly to
        // its start, and a 3-beat tick lands one beat into it.
        let mut core = EditorCore::new();
        core.project.fps = 24.0;
        core.project.frames[0].duration = 1.0;
        core.project.frames[1].duration = 1.5;
        core.project.frames[2].duration = 0.5;
        core.project.frames[3].duration = 1.0;
        core.project.frames[4].duration = 1.0;
        core.set_loop_range(1, 2);
        core.goto(1);
        core.advance_play(2.0 / 24.0);
        assert_eq!(core.current, 1, "one full beat-span loop returns to the start");
        assert!((core.playhead - 1.0).abs() < 1e-4, "playhead: {}", core.playhead);
        core.advance_play(3.0 / 24.0);
        assert_eq!(core.current, 1);
        assert!(
            (core.playhead - (1.0 + 1.0 / 1.5)).abs() < 1e-4,
            "one beat into frame 1's 1.5-beat hold: {}",
            core.playhead
        );
    }

    #[test]
    fn w2_40_ping_pong_bounces_at_the_timeline_ends() {
        // Roadmap Wave 2 (W2-40): five uniform frames at
        // 24 fps — one beat per 1/24 s tick, a 5-beat window.
        // Steps of 1.5 beats keep every landing off the exact
        // bounds: forward 0 → 1.5 → 3.0 → 4.5, then the next
        // step crosses the end (5) and reflects to 4.0
        // travelling backward, on to 2.5 and 1.0, then the
        // step crosses the start and reflects to 0.5 forward.
        let mut core = EditorCore::new();
        core.project.fps = 24.0;
        let step = 1.5 / 24.0;
        core.goto(0);
        assert!(!core.play_reverse, "a fresh pass travels forward");
        core.advance_ping_pong(step);
        assert!((core.playhead - 1.5).abs() < 1e-4, "playhead: {}", core.playhead);
        assert_eq!(core.current, 1);
        core.advance_ping_pong(step);
        assert!((core.playhead - 3.0).abs() < 1e-4, "playhead: {}", core.playhead);
        core.advance_ping_pong(step);
        assert!((core.playhead - 4.5).abs() < 1e-4, "playhead: {}", core.playhead);
        assert!(!core.play_reverse, "still forward inside the last frame");
        core.advance_ping_pong(step);
        assert!(
            (core.playhead - 4.0).abs() < 1e-4,
            "the overshoot reflects back into the window: {}",
            core.playhead
        );
        assert!(core.play_reverse, "the bounce flips the direction");
        assert_eq!(core.current, 4);
        core.advance_ping_pong(step);
        assert!((core.playhead - 2.5).abs() < 1e-4, "playhead: {}", core.playhead);
        core.advance_ping_pong(step);
        assert!((core.playhead - 1.0).abs() < 1e-4, "playhead: {}", core.playhead);
        core.advance_ping_pong(step);
        assert!(
            (core.playhead - 0.5).abs() < 1e-4,
            "the start bounce reflects the overshoot too: {}",
            core.playhead
        );
        assert!(!core.play_reverse, "forward again after the start bounce");
        assert_eq!(core.current, 0);
    }

    #[test]
    fn w2_40_ping_pong_full_period_returns_to_the_start() {
        // One full triangle period (2 × the 5-beat window)
        // returns the playhead to the start travelling
        // forward — no time is lost at either bounce.
        let mut core = EditorCore::new();
        core.project.fps = 24.0;
        core.goto(0);
        core.advance_ping_pong(10.0 / 24.0);
        assert!(core.playhead.abs() < 1e-4, "playhead: {}", core.playhead);
        assert_eq!(core.current, 0);
        assert!(!core.play_reverse);
        // Two periods land the same way.
        core.advance_ping_pong(20.0 / 24.0);
        assert!(core.playhead.abs() < 1e-4, "playhead: {}", core.playhead);
        assert!(!core.play_reverse);
    }

    #[test]
    fn w2_40_ping_pong_bounces_inside_the_loop_range() {
        // The window is the loop range's beat span (frames
        // 1..=2 = beats 1..3 at uniform durations), not the
        // whole timeline's.
        let mut core = EditorCore::new();
        core.project.fps = 24.0;
        core.set_loop_range(1, 2);
        core.goto(1);
        core.advance_ping_pong(1.5 / 24.0);
        assert!((core.playhead - 2.5).abs() < 1e-4, "playhead: {}", core.playhead);
        assert!(!core.play_reverse);
        core.advance_ping_pong(1.5 / 24.0);
        // 2.5 + 1.5 crosses the range end (beat 3): reflects
        // to beat 2.0, travelling backward.
        assert!((core.playhead - 2.0).abs() < 1e-4, "playhead: {}", core.playhead);
        assert!(core.play_reverse);
        assert_eq!(core.current, 2);
        core.advance_ping_pong(1.5 / 24.0);
        // Back across the range start (beat 1): reflects to
        // beat 1.5, forward again.
        assert!((core.playhead - 1.5).abs() < 1e-4, "playhead: {}", core.playhead);
        assert!(!core.play_reverse);
        assert_eq!(core.current, 1);
    }

    #[test]
    fn w2_40_ping_pong_walks_durations_on_the_shared_clock() {
        // The bounce is measured in beats, not frames: with
        // frame 0 held 4 beats the 5-frame timeline spans
        // 8 beats, so a 6-beat tick from the top lands 2
        // beats into the held frame (playhead 0.5), and the
        // next 6-beat tick crosses the end and reflects to
        // 4 beats back — 3 beats into the hold (0.75).
        let mut core = EditorCore::new();
        core.project.fps = 24.0;
        core.project.frames[0].duration = 4.0;
        core.goto(0);
        core.advance_ping_pong(6.0 / 24.0);
        assert!((core.playhead - 0.5).abs() < 1e-4, "playhead: {}", core.playhead);
        assert!(!core.play_reverse);
        core.advance_ping_pong(6.0 / 24.0);
        assert!((core.playhead - 0.75).abs() < 1e-4, "playhead: {}", core.playhead);
        assert!(core.play_reverse, "reflected at the 8-beat end");
        assert_eq!(core.current, 0);
    }

    #[test]
    fn w2_40_ping_pong_enters_the_window_from_outside() {
        // A playhead parked before the range walks forward
        // into it (the forward op's behaviour, mirrored);
        // parked past it, backward. No teleport either way.
        let mut core = EditorCore::new();
        core.project.fps = 24.0;
        core.set_loop_range(2, 3);
        core.goto(0);
        core.advance_ping_pong(1.0 / 24.0);
        assert!((core.playhead - 1.0).abs() < 1e-4, "playhead: {}", core.playhead);
        assert!(!core.play_reverse);
        core.advance_ping_pong(1.0 / 24.0);
        assert!((core.playhead - 2.0).abs() < 1e-4, "entered at the range start");
        // From past the range: one beat back lands on the
        // range's last frame, travelling backward.
        core.goto(4);
        core.advance_ping_pong(1.0 / 24.0);
        assert!((core.playhead - 3.0).abs() < 1e-4, "playhead: {}", core.playhead);
        assert!(core.play_reverse);
        assert_eq!(core.current, 3);
    }

    #[test]
    fn w2_40_ping_pong_stands_still_when_it_cannot_move() {
        // A non-positive or non-finite tick moves nothing
        // (the forward op's guard), an empty project is a
        // no-op, and a single-frame project stays on its
        // only frame however long it bounces.
        let mut core = EditorCore::new();
        core.project.fps = 24.0;
        core.goto(2);
        for dt in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            core.advance_ping_pong(dt);
            assert_eq!(core.current, 2, "dt {dt} must not move the playhead");
            assert!((core.playhead - 2.0).abs() < 1e-4);
        }
        core.project.frames.clear();
        core.advance_ping_pong(1.0);
        let mut single = EditorCore::new();
        single.project.fps = 24.0;
        single.project.frames.truncate(1);
        single.advance_ping_pong(3.0 / 24.0);
        assert_eq!(single.current, 0);
        single.advance_ping_pong(3.0 / 24.0);
        assert_eq!(single.current, 0);
    }

    #[test]
    fn w2_40_ping_pong_keeps_direction_across_a_seek() {
        // `goto` does not touch the direction: a seek
        // mid-bounce keeps travelling the way it was.
        let mut core = EditorCore::new();
        core.project.fps = 24.0;
        core.goto(4);
        core.advance_ping_pong(1.5 / 24.0);
        assert!(core.play_reverse, "bounced off the end");
        core.goto(2);
        assert!(core.play_reverse, "the seek leaves the direction alone");
        core.advance_ping_pong(1.0 / 24.0);
        assert!(
            (core.playhead - 1.0).abs() < 1e-4,
            "travel continues backward from the seek: {}",
            core.playhead
        );
    }

    #[test]
    fn w2_40_seek_fractional_lands_inside_the_authoring_frame() {
        // The sub-frame scrub's contract: the playhead is
        // fractional, `current` is its floor — the frame
        // every authoring op edits.
        let mut core = EditorCore::new();
        core.project.fps = 24.0;
        core.seek_fractional(2.5);
        assert!((core.playhead - 2.5).abs() < 1e-6, "playhead: {}", core.playhead);
        assert_eq!(core.current, 2, "current is the frame the playhead is inside");
        core.seek_fractional(2.999);
        assert_eq!(core.current, 2);
        core.seek_fractional(3.0);
        assert_eq!(core.current, 3, "an exact boundary is the next frame");
        assert!((core.playhead - 3.0).abs() < 1e-6);
        // Clamps: before the top to frame 0, past the end to
        // a hair inside the last frame (frame time n is the
        // timeline's end, outside the playhead's domain).
        core.seek_fractional(-4.0);
        assert_eq!(core.current, 0);
        assert!(core.playhead.abs() < 1e-6);
        core.seek_fractional(99.0);
        assert_eq!(core.current, 4);
        assert!(core.playhead > 4.9 && core.playhead < 5.0, "playhead: {}", core.playhead);
        // Refusals: non-finite times and an empty project
        // leave the playhead where it was.
        core.seek_fractional(f32::NAN);
        assert!(core.playhead > 4.9, "a NaN seek is refused");
        core.project.frames.clear();
        core.seek_fractional(1.0);
        assert!(core.playhead > 4.9, "an empty project refuses the seek");
    }

    #[test]
    fn w2_40_display_frame_shows_the_in_between_while_parked() {
        // Tween on + a fractional playhead: the stage shows
        // the interpolated position, playing or parked
        // mid-scrub. Tween off: the authored frame, always.
        let mut core = EditorCore::new();
        core.project.fps = 24.0;
        core.project.tween_playback = true;
        core.seek_fractional(2.5);
        assert!(!core.playing);
        assert!(
            (core.display_frame() - 2.5).abs() < 1e-6,
            "a parked scrub shows the in-between: {}",
            core.display_frame()
        );
        // An integer playhead renders the same either way.
        core.goto(2);
        assert!((core.display_frame() - 2.0).abs() < 1e-6);
        // Tween off: the authored frame shows even parked
        // between frames — and the playhead stays fractional
        // (the flag governs the view, not the position).
        core.project.tween_playback = false;
        core.seek_fractional(2.5);
        assert!((core.display_frame() - 2.0).abs() < 1e-6);
        assert!((core.playhead - 2.5).abs() < 1e-6);
        // A pose authored mid-scrub lands on `current`:
        // the op writes the frame the playhead is inside.
        let before = core.project.frames[2].poses[0].root;
        core.project.tween_playback = true;
        core.seek_fractional(2.5);
        core.snapshot_labeled("Pose");
        core.project.frames[core.current].poses[0].root = [before[0] + 10.0, before[1]];
        assert_eq!(core.current, 2);
        assert_eq!(core.project.frames[2].poses[0].root[0], before[0] + 10.0);
        assert_ne!(
            core.project.frames[3].poses[0].root[0],
            before[0] + 10.0,
            "the next frame is untouched by the mid-scrub edit"
        );
    }

    #[test]
    fn wave7_shift_frames_moves_block_with_all_frame_data() {
        let mut core = EditorCore::new();
        tag_frames(&mut core);
        core.goto(0);
        assert!(core.shift_frames(0, 2, 2), "move frames 1-2 later by 2");
        assert_eq!(root_xs(&core), vec![20.0, 30.0, 0.0, 10.0, 40.0]);
        let durations: Vec<f32> = core.project.frames.iter().map(|f| f.duration).collect();
        assert_eq!(durations, vec![3.0, 4.0, 1.0, 2.0, 5.0], "durations travel");
        let bends: Vec<f32> = core.project.frames.iter().map(|f| f.bends[0][0]).collect();
        assert_eq!(bends, vec![2.0, 3.0, 0.0, 1.0, 4.0], "bends travel");
        assert_eq!(
            core.project.frames[0].easing,
            Some(crate::model::Easing::EaseIn),
            "easing travels (tagged frame 2 is now first)"
        );
        assert_eq!(core.current, 2, "current follows its frame");
        // Undo restores the original order.
        core.undo();
        assert_eq!(root_xs(&core), vec![0.0, 10.0, 20.0, 30.0, 40.0]);
    }

    #[test]
    fn wave7_shift_frames_earlier_and_refusals() {
        let mut core = EditorCore::new();
        tag_frames(&mut core);
        assert!(core.shift_frames(3, 2, -2), "move frames 4-5 earlier by 2");
        assert_eq!(root_xs(&core), vec![0.0, 30.0, 40.0, 10.0, 20.0]);
        // Refusals change nothing.
        let before = root_xs(&core);
        assert!(!core.shift_frames(3, 2, 1), "destination past the end");
        assert!(!core.shift_frames(0, 2, -1), "destination before the start");
        assert!(!core.shift_frames(4, 2, -1), "block past the end");
        assert!(!core.shift_frames(0, 0, 1), "empty block");
        assert!(!core.shift_frames(0, 2, 0), "zero delta");
        assert_eq!(root_xs(&core), before, "refused shifts are no-ops");
    }

    #[test]
    fn wave7_reverse_range_reverses_frames() {
        let mut core = EditorCore::new();
        tag_frames(&mut core);
        core.goto(1);
        core.reverse_range(1, 3);
        assert_eq!(root_xs(&core), vec![0.0, 30.0, 20.0, 10.0, 40.0]);
        let durations: Vec<f32> = core.project.frames.iter().map(|f| f.duration).collect();
        assert_eq!(durations, vec![1.0, 4.0, 3.0, 2.0, 5.0], "durations reverse too");
        assert_eq!(core.current, 3, "current flips inside the reversed block");
        core.undo();
        assert_eq!(root_xs(&core), vec![0.0, 10.0, 20.0, 30.0, 40.0]);
    }

    #[test]
    fn wave9_delete_range_removes_middle_and_remaps_state() {
        let mut core = EditorCore::new();
        tag_frames(&mut core);
        core.set_loop_range(0, 4);
        core.goto(2); // inside the block being deleted
        assert!(core.delete_range(1, 2));
        assert_eq!(root_xs(&core), vec![0.0, 30.0, 40.0]);
        let durations: Vec<f32> = core.project.frames.iter().map(|f| f.duration).collect();
        assert_eq!(durations, vec![1.0, 4.0, 5.0], "surviving durations keep their frames");
        let bends: Vec<f32> = core.project.frames.iter().map(|f| f.bends[0][0]).collect();
        assert_eq!(bends, vec![0.0, 3.0, 4.0], "surviving bends keep their frames");
        assert_eq!(core.current, 1, "current lands on the next surviving frame");
        assert_eq!(core.project.frames[core.current].poses[0].root[0], 30.0);
        assert_eq!(
            core.loop_range,
            Some((0, 2)),
            "loop range follows its surviving first and last frames"
        );
        core.undo();
        assert_eq!(root_xs(&core), vec![0.0, 10.0, 20.0, 30.0, 40.0]);
    }

    #[test]
    fn wave9_delete_range_refuses_every_frame_and_clamps_tail() {
        let mut core = EditorCore::new();
        tag_frames(&mut core);
        let before = root_xs(&core);
        assert!(!core.delete_range(0, 99), "deleting every frame is refused");
        assert!(!core.delete_range(5, 1), "start past the end is refused");
        assert!(!core.delete_range(0, 0), "an empty range is refused");
        assert_eq!(root_xs(&core), before, "refused deletions are no-ops");

        core.set_loop_range(3, 4);
        core.goto(4);
        assert!(core.delete_range(3, 99), "count clamps to the timeline end");
        assert_eq!(root_xs(&core), vec![0.0, 10.0, 20.0]);
        assert_eq!(core.current, 2, "current lands on the last surviving frame");
        assert_eq!(core.loop_range, None, "a loop range inside the deleted tail is cleared");
    }

    #[test]
    fn wave9_copy_paste_range_round_trip_preserves_poses() {
        let mut core = EditorCore::new();
        tag_frames(&mut core);
        assert!(core.copy_range(1, 2));
        assert_eq!(core.frame_clipboard.len(), 2);
        assert!(core.clipboard.is_none(), "the single-frame clipboard is separate");
        core.project.frames[1].poses[0].root[0] = 999.0;
        assert_eq!(
            core.frame_clipboard[0].poses[0].root[0], 10.0,
            "the range clipboard holds deep clones"
        );
        assert!(!core.copy_range(5, 1), "copy start past the end is refused");
        assert!(!core.copy_range(0, 0), "an empty copy is refused");
        assert_eq!(core.frame_clipboard.len(), 2, "refused copies keep the clipboard");

        core.set_loop_range(3, 4);
        core.goto(4);
        assert!(core.paste_range(4));
        assert_eq!(root_xs(&core), vec![0.0, 999.0, 20.0, 30.0, 10.0, 20.0, 40.0]);
        let durations: Vec<f32> = core.project.frames.iter().map(|f| f.duration).collect();
        assert_eq!(durations, vec![1.0, 2.0, 3.0, 4.0, 2.0, 3.0, 5.0]);
        let bends: Vec<f32> = core.project.frames.iter().map(|f| f.bends[0][0]).collect();
        assert_eq!(bends, vec![0.0, 1.0, 2.0, 3.0, 1.0, 2.0, 4.0]);
        assert_eq!(
            core.project.frames[5].easing,
            Some(crate::model::Easing::EaseIn),
            "the copied frame's easing is preserved"
        );
        assert_eq!(core.current, 6, "current follows its original frame past the insertion");
        assert_eq!(core.loop_range, Some((3, 6)), "loop endpoints follow their frames");
        assert_eq!(core.frame_clipboard.len(), 2, "paste does not consume the clipboard");
    }

    #[test]
    fn wave9_paste_range_at_end_appends_and_empty_clipboard_refuses() {
        let mut core = EditorCore::new();
        assert!(!core.paste_range(0), "an empty range clipboard is refused");
        assert_eq!(core.project.frames.len(), 5);
        tag_frames(&mut core);
        assert!(core.copy_range(3, 2));
        core.goto(0);
        assert!(core.paste_range(999), "the paste index clamps to the end");
        assert_eq!(root_xs(&core), vec![0.0, 10.0, 20.0, 30.0, 40.0, 30.0, 40.0]);
        assert_eq!(core.current, 0, "appending does not move an earlier current frame");
    }

    #[test]
    fn wave7_nudge_path_point_moves_only_the_target_root() {
        let mut core = EditorCore::new();
        let angles_before = core.project.frames[2].poses[0].angles.clone();
        let roots_before = root_xs(&core);
        core.nudge_path_point(2, 0, [5.0, -3.0]);
        let root = core.project.frames[2].poses[0].root;
        assert_eq!(root, [roots_before[2] + 5.0, 470.0 - 3.0]);
        assert_eq!(core.project.frames[1].poses[0].root[0], roots_before[1]);
        assert_eq!(core.project.frames[3].poses[0].root[0], roots_before[3]);
        assert_eq!(
            core.project.frames[2].poses[0].angles, angles_before,
            "angles untouched"
        );
        // Out-of-range targets are reported, not panics.
        core.nudge_path_point(99, 0, [1.0, 1.0]);
        assert!(core.status.contains("No such frame"), "status: {}", core.status);
        assert_eq!(root_xs(&core)[2], roots_before[2] + 5.0);
    }

    #[test]
    fn wave7_smooth_path_roots_averages_a_spike() {
        let mut core = EditorCore::new();
        // Roots on x: still, still, SPIKE, still, still. y constant.
        for (i, frame) in core.project.frames.iter_mut().enumerate() {
            frame.poses[0].root = [if i == 2 { 100.0 } else { 0.0 }, 50.0];
        }
        let angles_before = core.project.frames[2].poses[0].angles.clone();
        core.smooth_path_roots(0);
        assert_eq!(
            root_xs(&core),
            vec![0.0, 50.0, 0.0, 50.0, 0.0],
            "one pass over the original values: the spike collapses, \
             its neighbours take the midpoint"
        );
        for frame in &core.project.frames {
            assert_eq!(frame.poses[0].root[1], 50.0, "y untouched");
        }
        assert_eq!(
            core.project.frames[2].poses[0].angles, angles_before,
            "angles untouched"
        );
        assert_eq!(core.project.frames[0].poses[0].root, [0.0, 50.0], "first endpoint kept");
        assert_eq!(core.project.frames[4].poses[0].root, [0.0, 50.0], "last endpoint kept");
    }

    #[test]
    fn w2_36_insert_path_point_splits_the_segment_and_remaps() {
        let mut core = EditorCore::new();
        for (i, frame) in core.project.frames.iter_mut().enumerate() {
            frame.poses[0].root = [40.0 * i as f32, 100.0];
        }
        core.project.camera = vec![crate::model::CameraKey {
            frame: 4,
            x: 0.0,
            y: 0.0,
            zoom: 1.0,
            angle: 0.0,
            easing: crate::model::CameraEase::Linear,
        }];
        let depth = core.undo_depth();
        // Split segment 1 (frames 2→3 in 1-based terms) at
        // its midpoint: a new frame lands between them with
        // the pressed point as its root, one undo step, the
        // playhead on the new frame, the camera key after
        // the split shifted with its frame.
        assert!(core.insert_path_point(1, 0, 0.5, [60.0, 100.0]));
        assert_eq!(core.project.frames.len(), 6);
        assert_eq!(core.project.frames[2].poses[0].root, [60.0, 100.0]);
        assert_eq!(core.project.frames[1].poses[0].root, [40.0, 100.0]);
        assert_eq!(core.project.frames[3].poses[0].root, [80.0, 100.0]);
        assert_eq!(core.current, 2, "the playhead lands on the new point");
        assert_eq!(core.undo_depth(), depth + 1, "one insert, one undo step");
        assert_eq!(core.project.camera[0].frame, 5, "camera keys remap on insert");
        // Refusals spend nothing: no segment after the last
        // frame, and t at a segment end is a point, not a split.
        assert!(!core.insert_path_point(5, 0, 0.5, [0.0, 0.0]));
        assert!(!core.insert_path_point(0, 0, 0.0, [0.0, 0.0]));
        assert!(!core.insert_path_point(0, 99, 0.5, [0.0, 0.0]));
        assert_eq!(core.project.frames.len(), 6);
        assert_eq!(core.undo_depth(), depth + 1);
    }

    #[test]
    fn w2_36_delete_path_point_reinterpolates_and_refuses_straight_points() {
        let mut core = EditorCore::new();
        for (i, frame) in core.project.frames.iter_mut().enumerate() {
            frame.poses[0].root = [40.0 * i as f32, 100.0];
        }
        // A spike at frame 3 (index 2): the point deflects
        // the path, so it can be deleted — and deleting it
        // re-interpolates the root to the neighbours'
        // midpoint, angles untouched, one undo step.
        core.project.frames[2].poses[0].root = [80.0, 200.0];
        let angles_before = core.project.frames[2].poses[0].angles.clone();
        let depth = core.undo_depth();
        assert!(core.delete_path_point(2, 0));
        assert_eq!(core.project.frames[2].poses[0].root, [80.0, 100.0]);
        assert_eq!(core.project.frames[2].poses[0].angles, angles_before);
        assert_eq!(core.undo_depth(), depth + 1, "one delete, one undo step");
        // Straight now: deleting again would change nothing
        // and refuses without spending.
        assert!(!core.delete_path_point(2, 0));
        assert!(core.status.contains("already lies on the path"), "{}", core.status);
        // The endpoints anchor the path and refuse.
        assert!(!core.delete_path_point(0, 0));
        assert!(!core.delete_path_point(4, 0));
        assert_eq!(core.undo_depth(), depth + 1);
    }

    #[test]
    fn wave6_audio_crossfade_rejects_unreadable_pair_without_changes() {
        let dir = audio_temp_dir("crossfade-bad");
        let first = write_editor_wav(&dir, "a.wav", &const_editor_wav(8000, 4000, 0.4));
        let bad_path = dir.join("b.wav");
        std::fs::write(&bad_path, b"not a wav").unwrap();
        let second = bad_path.to_string_lossy().into_owned();
        let mut core = audio_editor_core(10.0);
        core.project.audio.push(crate::model::AudioClip {
            path: first.clone(),
            start_frame: 0,
            gain: 1.0,
            muted: false,
        });
        core.project.audio.push(crate::model::AudioClip {
            path: second.clone(),
            start_frame: 3,
            gain: 1.0,
            muted: false,
        });

        core.audio_crossfade(0);

        assert_eq!(core.project.audio.len(), 2);
        assert_audio_clip(&core, 0, &first, 0);
        assert_audio_clip(&core, 1, &second, 3);
        assert!(!dir.join("a.xfade.wav").exists());
        assert!(core.status.contains("Audio crossfade failed"), "status: {}", core.status);
    }

    #[test]
    fn wave7_clip_create_from_figure_round_trip() {
        let mut core = EditorCore::new();
        core.project.tween_playback = true;
        let times = [0.0f32, 1.0, 1.5, 2.0, 3.0, 4.0];
        let before: Vec<Vec<V2>> = times.iter().map(|&f| core.positions_f(f, 0)).collect();
        let ci = core.create_clip_from_figure(0).expect("clip created");
        assert_eq!(core.project.figures[0].clip, Some(ci));
        assert_eq!(core.project.clips[ci].frames.len(), core.project.frames.len());
        assert_eq!(
            core.project.clips[ci].def.nodes.len(),
            core.project.figures[0].def.nodes.len()
        );
        for (k, &f) in times.iter().enumerate() {
            assert_eq!(
                core.positions_f(f, 0),
                before[k],
                "frame {f} changed after clip conversion"
            );
        }
        // The conversion snapshot: undo detaches the instance again.
        core.undo();
        assert_eq!(core.project.figures[0].clip, None);
    }

    #[test]
    fn wave7_clip_session_edits_only_the_clip() {
        let mut core = EditorCore::new();
        let ci = core.create_clip_from_figure(0).unwrap();
        let timeline_before = serde_json::to_string(&core.project.frames).unwrap();
        assert!(core.enter_clip(ci));
        assert_eq!(
            core.clip_edit,
            Some(ClipEditSession {
                clip: ci,
                frame: 0,
                name: core.project.clips[ci].name.clone(),
            })
        );
        core.clip_goto(1);
        core.clip_set_angle(3, 42.0);
        core.clip_set_root([10.0, 20.0]);
        assert_eq!(core.project.clips[ci].frames[1].angles[3], 42.0);
        assert_eq!(core.project.clips[ci].frames[1].root, [10.0, 20.0]);
        // Positions evaluate the clip's own def and session pose.
        let pos = core.clip_positions();
        assert_eq!(pos.len(), core.project.clips[ci].def.nodes.len());
        assert_eq!(pos[0], [10.0, 20.0]);
        // Add duplicates the last pose and moves the session onto
        // it; delete removes the session frame again.
        let n = core.project.clips[ci].frames.len();
        let last = core.project.clips[ci].frames[n - 1].clone();
        core.clip_add_frame();
        assert_eq!(core.project.clips[ci].frames.len(), n + 1);
        assert_eq!(core.project.clips[ci].frames[n].root, last.root);
        assert_eq!(core.project.clips[ci].frames[n].angles, last.angles);
        assert_eq!(core.clip_edit.as_ref().unwrap().frame, n);
        core.clip_delete_frame();
        assert_eq!(core.project.clips[ci].frames.len(), n);
        // The project timeline was never touched.
        assert_eq!(
            serde_json::to_string(&core.project.frames).unwrap(),
            timeline_before
        );
        core.exit_clip();
        assert!(core.clip_edit.is_none());
    }

    #[test]
    fn wave7_clip_session_boundaries() {
        let mut core = EditorCore::new();
        // No session: every session op is a no-op.
        core.clip_add_frame();
        core.clip_delete_frame();
        core.clip_goto(2);
        core.clip_set_angle(0, 10.0);
        core.clip_set_root([0.0, 0.0]);
        assert!(core.project.clips.is_empty());
        assert!(core.clip_positions().is_empty());
        assert!(core.status.contains("Not editing"));
        // Invalid entry points.
        assert!(!core.enter_clip(0));
        assert!(core.create_clip_from_figure(99).is_none());
        // A one-frame clip refuses deletion of its only frame.
        let ci = core.create_clip_from_figure(0).unwrap();
        core.project.clips[ci].frames.truncate(1);
        assert!(core.enter_clip(ci));
        core.clip_delete_frame();
        assert_eq!(core.project.clips[ci].frames.len(), 1);
        assert!(core.status.contains("at least one frame"));
        // Out-of-range joint is refused without changes.
        let angles_before = core.project.clips[ci].frames[0].angles.clone();
        core.clip_set_angle(999, 5.0);
        assert_eq!(core.project.clips[ci].frames[0].angles, angles_before);
        // Goto clamps to the final frame (frame 0 of one).
        core.clip_goto(500);
        assert_eq!(core.clip_edit.as_ref().unwrap().frame, 0);
        core.exit_clip();
        core.exit_clip(); // a second exit is a harmless no-op
        assert!(core.clip_edit.is_none());
    }

    // ---- Wave X (P-1 step 2): linking + state authoring ----

    #[test]
    fn wave_x_clip_compatible_predicate() {
        let core = EditorCore::new();
        let fig = core.project.figures[0].clone();
        let n = fig.def.nodes.len();
        let good = crate::model::ClipDef {
            name: "ok".into(),
            def: fig.def.clone(),
            frames: vec![fig.def.rest_pose([0.0, 0.0])],
            loop_mode: crate::model::ClipLoop::Loop,
        };
        assert!(clip_compatible(&fig, &good));
        // A clip built for a different rig (quadruped: 13
        // nodes vs the humanoid's 11) is not retargetable.
        let quad = FigureDef::quadruped();
        assert_ne!(quad.nodes.len(), n);
        let foreign = crate::model::ClipDef {
            name: "dog".into(),
            def: quad.clone(),
            frames: vec![quad.rest_pose([0.0, 0.0])],
            loop_mode: crate::model::ClipLoop::Loop,
        };
        assert!(!clip_compatible(&fig, &foreign));
        // Matching defs but a frame that does not fit its own
        // rig: still refused — playback would misfit the row.
        let mut broken = good.clone();
        broken.frames[0].angles.truncate(3);
        assert!(!clip_compatible(&fig, &broken));
        // An empty clip with a matching def passes: nothing
        // to misfit, and playback skips empty clips.
        let mut empty = good.clone();
        empty.frames.clear();
        assert!(clip_compatible(&fig, &empty));
    }

    #[test]
    fn wave_x_assign_clip_respects_the_rig_guard() {
        let mut core = EditorCore::new();
        let ci = core.create_clip_from_figure(0).expect("clip created");
        // A quadruped figure cannot take the humanoid clip.
        core.project
            .add_figure_with(FigureDef::quadruped(), "Dog", 0xA0A0A0, [120.0, 120.0]);
        let dog = core.project.figures.len() - 1;
        let depth = core.undo_depth();
        assert!(!core.assign_clip_to_figure(dog, ci));
        assert!(core.status.contains("same rig size"), "status: {}", core.status);
        assert_eq!(core.project.figures[dog].clip, None);
        assert_eq!(core.undo_depth(), depth, "a refusal snapshots nothing");
        // Bad indices refuse with the family wordings.
        assert!(!core.assign_clip_to_figure(0, 99));
        assert_eq!(core.status, "No such clip.");
        assert!(!core.assign_clip_to_figure(99, ci));
        assert!(core.status.contains("No figure 100"), "status: {}", core.status);
        assert_eq!(core.undo_depth(), depth);
        // The index-level predicate agrees with the op.
        assert!(core.clip_compatible(0, ci));
        assert!(!core.clip_compatible(dog, ci));
        assert!(!core.clip_compatible(0, 99));
        // A good assign is exactly one undo step; re-assigning
        // the same clip is a no-op; undo detaches again.
        assert!(core.unlink_figure(0));
        let depth = core.undo_depth();
        assert!(core.assign_clip_to_figure(0, ci));
        assert_eq!(core.undo_depth(), depth + 1);
        assert_eq!(core.project.figures[0].clip, Some(ci));
        assert!(core.status.contains("now loops clip"), "status: {}", core.status);
        assert!(!core.assign_clip_to_figure(0, ci));
        assert_eq!(core.undo_depth(), depth + 1);
        core.undo();
        assert_eq!(core.project.figures[0].clip, None);
    }

    #[test]
    fn wave_x_unlink_figure_round_trip() {
        let mut core = EditorCore::new();
        let ci = core.create_clip_from_figure(0).expect("clip created");
        let depth = core.undo_depth();
        assert!(core.unlink_figure(0));
        assert_eq!(core.undo_depth(), depth + 1);
        assert_eq!(core.project.figures[0].clip, None);
        assert!(core.status.contains("timeline poses"), "status: {}", core.status);
        // A second unlink is a no-op; a bad figure refuses.
        assert!(!core.unlink_figure(0));
        assert_eq!(core.undo_depth(), depth + 1);
        assert!(!core.unlink_figure(42));
        assert!(core.status.contains("No figure 43"), "status: {}", core.status);
        // Undo re-links.
        core.undo();
        assert_eq!(core.project.figures[0].clip, Some(ci));
        // With state keys authored, unlink leaves them in
        // force and the status says so.
        assert!(core.set_state_key(
            0,
            crate::model::StateKey { frame: 1, clip: ci, offset: 0, speed: 1.0 },
        ));
        assert!(core.unlink_figure(0));
        assert_eq!(core.project.figures[0].clip, None);
        assert_eq!(core.project.figures[0].states.len(), 1);
        assert!(core.status.contains("state keys still apply"), "status: {}", core.status);
    }

    #[test]
    fn wave_x_state_key_authoring() {
        let mut core = EditorCore::new();
        let ci_a = core.create_clip_from_figure(0).expect("clip created");
        let ci_b = core.duplicate_clip(ci_a).expect("clip duplicated");
        let key = crate::model::StateKey { frame: 2, clip: ci_b, offset: 1, speed: 2.0 };
        let depth = core.undo_depth();
        assert!(core.set_state_key(0, key));
        assert_eq!(core.undo_depth(), depth + 1);
        assert_eq!(core.project.figures[0].states, vec![key]);
        assert!(core.status.contains("state at frame 3"), "status: {}", core.status);
        // Authoring an earlier frame keeps the vec sorted.
        let first = crate::model::StateKey { frame: 0, clip: ci_a, offset: 0, speed: 1.0 };
        assert!(core.set_state_key(0, first));
        assert_eq!(core.project.figures[0].states, vec![first, key]);
        // Re-setting the same frame replaces, never stacks.
        let replaced = crate::model::StateKey { frame: 2, clip: ci_a, offset: 0, speed: 1.0 };
        assert!(core.set_state_key(0, replaced));
        assert_eq!(core.project.figures[0].states, vec![first, replaced]);
        // Refusals snapshot nothing: bad clip, bad figure,
        // rig mismatch, non-finite speed.
        core.project
            .add_figure_with(FigureDef::quadruped(), "Dog", 0xA0A0A0, [120.0, 120.0]);
        let dog = core.project.figures.len() - 1;
        let depth = core.undo_depth();
        assert!(!core.set_state_key(0, crate::model::StateKey { clip: 99, ..key }));
        assert_eq!(core.status, "No such clip.");
        assert!(!core.set_state_key(99, key));
        assert!(!core.set_state_key(dog, key));
        assert!(core.status.contains("same rig size"), "status: {}", core.status);
        assert!(!core.set_state_key(
            0,
            crate::model::StateKey { speed: f32::NAN, ..key },
        ));
        assert!(core.status.contains("speed must be finite"), "status: {}", core.status);
        assert_eq!(core.undo_depth(), depth);
        // Removal: one step, then the repeat refusal.
        assert!(core.remove_state_key(0, 2));
        assert_eq!(core.undo_depth(), depth + 1);
        assert_eq!(core.project.figures[0].states, vec![first]);
        assert!(!core.remove_state_key(0, 2));
        assert!(core.status.contains("no state key at frame 3"), "status: {}", core.status);
        assert_eq!(core.undo_depth(), depth + 1);
        core.undo();
        assert_eq!(core.project.figures[0].states, vec![first, replaced]);
    }

    #[test]
    fn wave_x_state_key_switches_the_evaluated_pose() {
        let mut core = EditorCore::new();
        let ci_a = core.create_clip_from_figure(0).expect("clip created");
        let ci_b = core.duplicate_clip(ci_a).expect("clip duplicated");
        for p in &mut core.project.clips[ci_b].frames {
            p.root = [100.0, 100.0];
        }
        assert!(core.set_state_key(
            0,
            crate::model::StateKey { frame: 2, clip: ci_b, offset: 0, speed: 1.0 },
        ));
        // Before the key the legacy link plays (the demo
        // timeline's own roots, baked into clip A); at the
        // key frame the state clip takes over.
        assert_eq!(core.positions_f(0.0, 0)[0], [480.0, 470.0]);
        assert_eq!(core.positions_f(1.0, 0)[0], [494.0, 470.0]);
        assert_eq!(core.positions_f(2.0, 0)[0], [100.0, 100.0]);
        assert_eq!(core.positions_f(4.0, 0)[0], [100.0, 100.0]);
    }

    #[test]
    fn wave_x_create_clip_from_figure_range() {
        let mut core = EditorCore::new();
        let total = core.project.frames.len();
        assert_eq!(total, 5, "the demo timeline's shape");
        // Refusals first: empty, inverted, out-of-timeline
        // and bad-figure ranges snapshot nothing.
        let depth = core.undo_depth();
        assert!(core.create_clip_from_figure_range(0, 2, 2).is_none());
        assert!(core.create_clip_from_figure_range(0, 3, 2).is_none());
        assert!(core.create_clip_from_figure_range(0, 0, total + 1).is_none());
        assert!(core.status.contains("No frames in range"), "status: {}", core.status);
        assert!(core.create_clip_from_figure_range(99, 0, 1).is_none());
        assert!(core.status.contains("No figure 100"), "status: {}", core.status);
        assert_eq!(core.undo_depth(), depth);
        assert!(core.project.clips.is_empty());
        // [1, 3) bakes exactly timeline frames 1 and 2.
        let ci = core.create_clip_from_figure_range(0, 1, 3).expect("ranged clip created");
        assert_eq!(core.undo_depth(), depth + 1);
        assert_eq!(core.project.clips[ci].name, "Styk clip 2-3");
        assert_eq!(core.project.clips[ci].frames.len(), 2);
        for (k, src) in core.project.frames[1..3].iter().enumerate() {
            assert_eq!(core.project.clips[ci].frames[k].angles, src.poses[0].angles);
            assert_eq!(core.project.clips[ci].frames[k].root, src.poses[0].root);
        }
        assert_eq!(core.project.figures[0].clip, Some(ci));
        assert!(core.clip_compatible(0, ci));
        // One undo removes both the clip and the link.
        core.undo();
        assert!(core.project.clips.is_empty());
        assert_eq!(core.project.figures[0].clip, None);
    }

    #[test]
    fn wave_x_delete_and_move_clip_remap_state_keys() {
        let mut core = EditorCore::new();
        let ci_a = core.create_clip_from_figure(0).expect("clip created");
        let ci_b = core.duplicate_clip(ci_a).expect("first duplicate");
        let ci_c = core.duplicate_clip(ci_b).expect("second duplicate");
        let name_b = core.project.clips[ci_b].name.clone();
        let name_c = core.project.clips[ci_c].name.clone();
        assert!(core.set_state_key(
            0,
            crate::model::StateKey { frame: 0, clip: ci_c, offset: 0, speed: 1.0 },
        ));
        assert!(core.set_state_key(
            0,
            crate::model::StateKey { frame: 2, clip: ci_b, offset: 0, speed: 1.0 },
        ));
        let keyed_name = |core: &EditorCore, frame: usize| {
            let key = core.project.figures[0]
                .states
                .iter()
                .find(|k| k.frame == frame)
                .expect("key present");
            core.project.clips[key.clip].name.clone()
        };
        // Deleting a clip BEFORE the keyed ones shifts the
        // keys down; they still name the same clips.
        assert!(core.delete_clip(ci_a));
        assert_eq!(keyed_name(&core, 0), name_c);
        assert_eq!(keyed_name(&core, 2), name_b);
        // Deleting a keyed clip removes its key; the other
        // key shifts and still names its clip.
        let doomed = core.project.figures[0]
            .states
            .iter()
            .find(|k| k.frame == 2)
            .map(|k| k.clip)
            .expect("key present");
        assert!(core.delete_clip(doomed));
        assert_eq!(core.project.figures[0].states.len(), 1);
        assert_eq!(keyed_name(&core, 0), name_c);
        // A swap exchanges the keys' clip indices.
        let ci_d = core.duplicate_clip(0).expect("third duplicate");
        let name_d = core.project.clips[ci_d].name.clone();
        assert!(core.set_state_key(
            0,
            crate::model::StateKey { frame: 4, clip: ci_d, offset: 0, speed: 1.0 },
        ));
        assert!(core.move_clip(0, false));
        assert_eq!(keyed_name(&core, 0), name_c);
        assert_eq!(keyed_name(&core, 4), name_d);
    }
}

#[cfg(test)]
mod wave10_tests {
    use crate::editor::EditorCore;
    use crate::graph::{Channel, ChannelTarget, Key};
    use crate::model::{ClipDef, ClipLoop, FigureDef};

    #[test]
    fn smooth_graph_tangents_rewrites_flat_tangents() {
        let mut core = EditorCore::new();
        // The demo project has no channels: a clear no-op status,
        // and no undo step is recorded for it.
        core.smooth_graph_tangents();
        assert!(
            core.status.contains("No graph channels"),
            "status: {}",
            core.status
        );

        core.project.channels.push(Channel {
            figure: 0,
            target: ChannelTarget::Angle(1),
            keys: vec![
                Key::flat(0.0, 0.0),
                Key::flat(10.0, 50.0),
                Key::flat(20.0, 10.0),
            ],
        });
        core.smooth_graph_tangents();
        assert!(core.status.contains("Smoothed"), "status: {}", core.status);
        let ch = &core.project.channels[0];
        // Frames and values are untouched; only tangents move.
        assert_eq!(ch.keys.len(), 3);
        assert_eq!(ch.keys[1].frame, 10.0);
        assert_eq!(ch.keys[1].value, 50.0);
        assert!(
            ch.keys.iter().any(|k| k.in_tan != 0.0 || k.out_tan != 0.0),
            "smoothing should give at least one key a non-flat tangent: {:?}",
            ch.keys
        );
    }

    #[test]
    fn mirror_pose_negates_angles_and_is_its_own_inverse() {
        let mut core = EditorCore::new();
        let before = core.project.frames[0].poses[0].clone();
        assert!(!before.angles.is_empty());
        core.mirror_selected_pose();
        assert!(core.status.contains("Mirrored"), "status: {}", core.status);
        let mirrored = &core.project.frames[0].poses[0];
        assert_eq!(mirrored.root, before.root, "the root must not move");
        for (m, b) in mirrored.angles.iter().zip(&before.angles) {
            assert_eq!(*m, -*b);
        }
        // Mirroring a second time returns the exact original pose.
        core.mirror_selected_pose();
        assert_eq!(core.project.frames[0].poses[0].angles, before.angles);
        assert_eq!(core.project.frames[0].poses[0].root, before.root);
    }

    #[test]
    fn mirror_pose_without_pose_reports_and_changes_nothing() {
        let mut core = EditorCore::new();
        core.project.frames[0].poses.clear();
        core.mirror_selected_pose();
        assert!(core.status.contains("no pose"), "status: {}", core.status);
        assert!(core.project.frames[0].poses.is_empty());
    }

    #[test]
    fn cycle_clip_loop_rotates_through_all_modes() {
        let mut core = EditorCore::new();
        core.project.clips.push(ClipDef {
            name: "T".into(),
            def: FigureDef::simple(),
            frames: Vec::new(),
            loop_mode: ClipLoop::Loop,
        });
        core.cycle_clip_loop(0);
        assert!(matches!(
            core.project.clips[0].loop_mode,
            ClipLoop::PingPong
        ));
        assert!(core.status.contains("ping-pong"), "status: {}", core.status);
        core.cycle_clip_loop(0);
        assert!(matches!(core.project.clips[0].loop_mode, ClipLoop::Once));
        assert!(core.status.contains("once"), "status: {}", core.status);
        core.cycle_clip_loop(0);
        assert!(matches!(core.project.clips[0].loop_mode, ClipLoop::Loop));
        assert!(core.status.contains("loop"), "status: {}", core.status);
        // Out of range: quiet no-op with a status, mode unchanged.
        core.cycle_clip_loop(9);
        assert!(core.status.contains("No such clip"), "status: {}", core.status);
        assert!(matches!(core.project.clips[0].loop_mode, ClipLoop::Loop));
    }
}

#[cfg(test)]
mod wave11_tests {
    use super::tests::{audio_temp_dir, const_editor_wav, write_editor_wav};
    use crate::audio_edit::WavData;
    use crate::editor::EditorCore;
    use crate::graph::{Channel, ChannelTarget, Key};
    use crate::model::{AudioClip, Project};
    use std::path::PathBuf;

    /// A one-key RootX channel: `value_at` returns the key value
    /// verbatim at every frame, so baked roots are exact.
    fn rootx_channel(value: f32) -> Channel {
        Channel {
            figure: 0,
            target: ChannelTarget::RootX,
            keys: vec![Key::flat(0.0, value)],
        }
    }

    #[test]
    fn bake_graph_channels_writes_channel_poses() {
        let mut core = EditorCore::new();
        let n_frames = core.project.frames.len();
        assert!(n_frames >= 4, "demo project should have several frames");
        let pre_root = core.project.frames[1].poses[0].root;
        core.project.channels.push(rootx_channel(123.5));
        // The expected bake, computed on the pre-bake project.
        let expected: Vec<(f32, Vec<f32>)> = (0..=2)
            .map(|f| {
                let p = crate::graph::apply_channels(&core.project, &core.project.channels, f as f32)
                    .poses[0]
                    .clone();
                (p.root[0], p.angles)
            })
            .collect();
        core.bake_graph_channels(0, 2);
        assert!(core.status.contains("Baked"), "status: {}", core.status);
        for (f, (root_x, angles)) in expected.iter().enumerate() {
            let pose = &core.project.frames[f].poses[0];
            assert_eq!(pose.root[0], *root_x, "frame {f} root x should be baked");
            assert_eq!(pose.root[0], 123.5, "frame {f} root x is the channel value");
            assert_eq!(&pose.angles, angles, "frame {f} angles should be baked");
        }
        assert_ne!(pre_root[0], 123.5, "the channel value must differ from the stored root");
        // Frames outside the range keep their stored poses.
        assert_ne!(core.project.frames[3].poses[0].root[0], 123.5);
    }

    #[test]
    fn bake_graph_channels_writes_channel_bends() {
        // Wave Z (Z11): bend channels bake into Frame::bends —
        // frames carry a bend home, unlike lengths. A constant
        // Bend(2) channel at 0.5 rad lands in every baked frame,
        // and the bend survives clearing the channels (the bake
        // promise root/angle already kept).
        let mut core = EditorCore::new();
        let frame3_bends_before = core.project.frames[3].bends.clone();
        core.project.channels.push(Channel {
            figure: 0,
            target: ChannelTarget::Bend(2),
            keys: vec![Key::flat(0.0, 0.5)],
        });
        core.bake_graph_channels(0, 2);
        assert!(core.status.contains("Baked"), "status: {}", core.status);
        for f in 0..=2 {
            let row = &core.project.frames[f].bends[0];
            assert!(
                (row[2] - 0.5).abs() < 1e-6,
                "frame {f} bend baked, got {row:?}"
            );
            assert!(
                row.iter().enumerate().all(|(i, b)| i == 2 || *b == 0.0),
                "frame {f} unbend nodes stay straight: {row:?}"
            );
        }
        // Frames outside the range keep their bend tables.
        assert_eq!(core.project.frames[3].bends, frame3_bends_before);
        // Clearing the channels changes nothing now: the baked
        // bends read back through bends_at exactly.
        core.project.channels.clear();
        assert!((core.project.bends_at(1.0, 0)[2] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn bake_without_bend_channels_leaves_bend_tables_alone() {
        // Wave Z (Z11): a pose-only bake must not materialise
        // bend tables — figures no bend channel targets keep
        // their sparse stored shape byte-for-byte.
        let mut core = EditorCore::new();
        let before: Vec<Vec<Vec<f32>>> = core
            .project
            .frames
            .iter()
            .map(|f| f.bends.clone())
            .collect();
        core.project.channels.push(rootx_channel(123.5));
        core.bake_graph_channels(0, 2);
        assert!(core.status.contains("Baked"), "status: {}", core.status);
        for (f, frame) in core.project.frames.iter().enumerate() {
            assert_eq!(
                frame.bends, before[f],
                "frame {f} bend table untouched by a pose-only bake"
            );
        }
    }

    #[test]
    fn bake_graph_channels_clamps_the_range_end() {
        let mut core = EditorCore::new();
        let last = core.project.frames.len() - 1;
        core.project.channels.push(rootx_channel(123.5));
        core.bake_graph_channels(0, usize::MAX);
        assert!(core.status.contains("Baked"), "status: {}", core.status);
        assert_eq!(core.project.frames[last].poses[0].root[0], 123.5);
    }

    #[test]
    fn bake_graph_channels_noops_without_channels_or_range() {
        let mut core = EditorCore::new();
        let edits = core.edits_since_autosave();
        let before = core.project.frames[0].poses[0].clone();
        core.bake_graph_channels(0, 2);
        assert!(
            core.status.contains("No graph channels"),
            "status: {}",
            core.status
        );
        assert_eq!(core.edits_since_autosave(), edits, "no snapshot on a no-op");
        assert_eq!(core.project.frames[0].poses[0].root, before.root);
        assert_eq!(core.project.frames[0].poses[0].angles, before.angles);

        // With a channel but an invalid range: still no snapshot.
        core.project.channels.push(rootx_channel(123.5));
        core.bake_graph_channels(3, 1);
        assert!(core.status.contains("invalid"), "status: {}", core.status);
        assert_eq!(core.edits_since_autosave(), edits, "no snapshot on a bad range");
        assert_ne!(core.project.frames[0].poses[0].root[0], 123.5);
    }

    /// Give the demo project a second figure whose frame poses
    /// start as copies of figure 0's.
    fn add_second_figure(core: &mut EditorCore) {
        let fig = core.project.figures[0].clone();
        core.project.figures.push(fig);
        for frame in &mut core.project.frames {
            let pose = frame.poses[0].clone();
            frame.poses.push(pose);
        }
    }

    #[test]
    fn copy_pose_between_copies_root_and_angles_exactly() {
        let mut core = EditorCore::new();
        add_second_figure(&mut core);
        // Make the target pose unmistakably different.
        core.project.frames[0].poses[1].root = [1.0, 2.0];
        for a in &mut core.project.frames[0].poses[1].angles {
            *a = 7.0;
        }
        let source = core.project.frames[0].poses[0].clone();
        core.copy_pose_between(0, 1);
        assert!(core.status.contains("Copied"), "status: {}", core.status);
        let copied = &core.project.frames[0].poses[1];
        assert_eq!(copied.root, source.root);
        assert_eq!(copied.angles, source.angles);
    }

    #[test]
    fn copy_pose_between_rejects_bad_indices() {
        let mut core = EditorCore::new();
        add_second_figure(&mut core);
        let edits = core.edits_since_autosave();
        let before = core.project.frames[0].poses[1].clone();

        core.copy_pose_between(0, 0);
        assert!(core.status.contains("same figure"), "status: {}", core.status);
        core.copy_pose_between(0, 9);
        assert!(core.status.contains("out of range"), "status: {}", core.status);
        core.copy_pose_between(9, 1);
        assert!(core.status.contains("out of range"), "status: {}", core.status);

        assert_eq!(core.edits_since_autosave(), edits, "no snapshot on rejected copies");
        assert_eq!(core.project.frames[0].poses[1].root, before.root);
        assert_eq!(core.project.frames[0].poses[1].angles, before.angles);
    }

    /// Write a tiny mono WAV into a pid-unique temp path and
    /// return the path as a String.
    fn temp_wav(tag: &str, samples: Vec<f32>) -> String {
        let path = std::env::temp_dir().join(format!(
            "styk-wave11-{tag}-{}.wav",
            std::process::id()
        ));
        let wav = WavData {
            sample_rate: 8000,
            channels: 1,
            samples,
        };
        std::fs::write(&path, crate::audio_edit::write_wav(&wav)).unwrap();
        path.to_string_lossy().into_owned()
    }

    /// A fresh core whose only audio clip points at `path`.
    fn core_with_clip(path: String) -> EditorCore {
        let mut core = EditorCore::new();
        core.project.audio.clear();
        core.project.audio.push(AudioClip {
            path,
            start_frame: 0,
            gain: 1.0,
            muted: false,
        });
        core
    }

    /// P14 fixture: a demo project whose audio is exactly
    /// `clips`, in order.
    fn project_with_audio(clips: Vec<crate::model::AudioClip>) -> Project {
        let mut project = Project::new_demo();
        project.audio = clips;
        project
    }

    /// P14 fixture: an unmuted unity-gain clip at
    /// `start_frame` pointing at `path`.
    fn audio_clip_at(path: String, start_frame: usize) -> crate::model::AudioClip {
        crate::model::AudioClip {
            path,
            start_frame,
            gain: 1.0,
            muted: false,
        }
    }

    /// P14 fixture: write `project` into `dir` as a project
    /// file and return its path, ready to be opened through
    /// `open_project_file` via the `project_files` selector.
    fn write_project_file(dir: &PathBuf, project: &Project) -> PathBuf {
        let file = dir.join("proj.styk.json");
        std::fs::write(&file, serde_json::to_string_pretty(project).unwrap()).unwrap();
        file
    }

    #[test]
    fn open_project_file_reports_a_missing_audio_clip() {
        let dir = audio_temp_dir("p14-missing-open");
        let present = dir.join("present.wav");
        std::fs::write(&present, b"").unwrap();
        let present_s = present.to_string_lossy().into_owned();
        let missing_s = dir.join("gone.wav").to_string_lossy().into_owned();
        let project = project_with_audio(vec![
            audio_clip_at(present_s.clone(), 0),
            audio_clip_at(missing_s.clone(), 3),
        ]);
        let file = write_project_file(&dir, &project);

        let mut core = EditorCore::new();
        core.project_files = vec![file];
        core.project_index = 0;
        core.open_project_file();

        // The missing clip is reported by its stored path;
        // the present clip is not in the report.
        assert_eq!(core.missing_audio_clips().len(), 1);
        assert_eq!(core.missing_audio_clips()[0], missing_s);
        // The load dropped nothing: both clips survive, the
        // present one exactly as saved.
        assert_eq!(core.project.audio.len(), 2);
        assert_eq!(core.project.audio[0].path, present_s);
        assert_eq!(core.project.audio[0].start_frame, 0);
        assert_eq!(core.project.audio[1].path, missing_s);
        assert_eq!(core.project.audio[1].start_frame, 3);
        // The load status carries the finding and names the
        // missing file (and not the present one).
        assert!(core.status.starts_with("Opened "), "status: {}", core.status);
        assert!(core.status.contains(&missing_s), "status: {}", core.status);
        assert!(
            !core.status.contains("present.wav"),
            "status: {}",
            core.status
        );
    }

    #[test]
    fn open_project_file_with_all_audio_present_reports_nothing() {
        let dir = audio_temp_dir("p14-all-present");
        let a = write_editor_wav(&dir, "a.wav", &const_editor_wav(8000, 8, 0.0));
        let b = write_editor_wav(&dir, "b.wav", &const_editor_wav(8000, 8, 0.0));
        let project = project_with_audio(vec![audio_clip_at(a, 0), audio_clip_at(b, 5)]);
        let file = write_project_file(&dir, &project);

        let mut core = EditorCore::new();
        core.project_files = vec![file.clone()];
        core.project_index = 0;
        core.open_project_file();

        assert!(core.missing_audio_clips().is_empty());
        // A clean load's status is byte-identical to before
        // the report existed.
        assert_eq!(core.status, format!("Opened {}", file.display()));
    }

    #[test]
    fn missing_audio_report_classifies_an_empty_path_as_missing() {
        // An empty path names no file at all, so nothing can
        // ever be found there: the clip is reported, under
        // its stored (empty) path.
        let mut core = core_with_clip(String::new());
        core.note_project_swapped();
        assert_eq!(core.missing_audio_clips().len(), 1);
        assert_eq!(core.missing_audio_clips()[0], "");
    }

    #[test]
    fn missing_audio_report_excludes_muted_clips() {
        // The report shares the tree's one classification
        // (`crate::audio::missing_audio_clips`, also consumed
        // by the MP4 export outcome): a muted clip is
        // intentionally silent, so its file's absence loses
        // nothing and is not reported — the same exclusion
        // playback planning and the export mix apply.
        let mut core = EditorCore::new();
        core.project.audio.clear();
        core.project.audio.push(crate::model::AudioClip {
            path: "/nonexistent/p14-muted-gone.wav".to_string(),
            start_frame: 0,
            gain: 1.0,
            muted: true,
        });
        core.note_project_swapped();
        assert!(core.missing_audio_clips().is_empty());
        // Unmute the same clip and the loss becomes real:
        // the next refresh names it.
        core.project.audio[0].muted = false;
        core.note_project_swapped();
        assert_eq!(core.missing_audio_clips().len(), 1);
        assert_eq!(
            core.missing_audio_clips()[0],
            "/nonexistent/p14-muted-gone.wav"
        );
    }

    #[test]
    fn missing_audio_report_refreshes_on_the_next_swap() {
        let mut core = core_with_clip("/nonexistent/p14-definitely-gone.wav".to_string());
        core.note_project_swapped();
        assert_eq!(core.missing_audio_clips().len(), 1);

        // Point the same clip at a file that exists and swap
        // again: the stale report clears instead of naming a
        // clip whose file is now present.
        let dir = audio_temp_dir("p14-refresh");
        let present = write_editor_wav(&dir, "here.wav", &const_editor_wav(8000, 8, 0.0));
        core.project.audio[0].path = present;
        core.note_project_swapped();
        assert!(core.missing_audio_clips().is_empty());
    }

    #[test]
    fn audio_echo_last_repoints_clip_to_a_new_file() {
        let src = temp_wav("echo", vec![0.5; 800]);
        let mut core = core_with_clip(src.clone());
        core.audio_echo_last();
        assert!(core.status.contains("Echo applied"), "status: {}", core.status);
        let new_path = core.project.audio[0].path.clone();
        assert_ne!(new_path, src, "the clip must point at the echo product");
        let back = crate::audio_edit::read_wav(&new_path).expect("echo product reads back");
        assert!(back.frames() > 0);
        // The original file is untouched and still reads back.
        assert!(crate::audio_edit::read_wav(&src).is_ok());
        let _ = std::fs::remove_file(&src);
        let _ = std::fs::remove_file(&new_path);
    }

    #[test]
    fn audio_lowpass_last_repoints_clip_to_a_new_file() {
        let src = temp_wav("lowpass", vec![0.5; 800]);
        let mut core = core_with_clip(src.clone());
        core.audio_lowpass_last();
        assert!(
            core.status.contains("Low-pass filter"),
            "status: {}",
            core.status
        );
        let new_path = core.project.audio[0].path.clone();
        assert_ne!(new_path, src, "the clip must point at the low-pass product");
        let back = crate::audio_edit::read_wav(&new_path).expect("low-pass product reads back");
        assert!(back.frames() > 0);
        let _ = std::fs::remove_file(&src);
        let _ = std::fs::remove_file(&new_path);
    }

    #[test]
    fn audio_trim_silence_last_trims_and_reports() {
        let mut samples = vec![0.0; 400];
        samples.extend(vec![0.5; 800]);
        samples.extend(vec![0.0; 400]);
        let src = temp_wav("silence", samples);
        let mut core = core_with_clip(src.clone());
        core.audio_trim_silence_last();
        assert!(core.status.contains("Trimmed"), "status: {}", core.status);
        assert!(
            core.status.contains("800 silent frame(s)"),
            "status: {}",
            core.status
        );
        let new_path = core.project.audio[0].path.clone();
        assert_ne!(new_path, src, "the clip must point at the trimmed product");
        let back = crate::audio_edit::read_wav(&new_path).expect("trimmed product reads back");
        assert_eq!(back.frames(), 800, "only the audible middle remains");
        let _ = std::fs::remove_file(&src);
        let _ = std::fs::remove_file(&new_path);
    }

    #[test]
    fn audio_trim_silence_last_noops_when_nothing_to_trim() {
        let src = temp_wav("nosilence", vec![0.5; 800]);
        let mut core = core_with_clip(src.clone());
        let edits = core.edits_since_autosave();
        core.audio_trim_silence_last();
        assert!(
            core.status.contains("No silence to trim"),
            "status: {}",
            core.status
        );
        assert_eq!(core.project.audio[0].path, src, "the clip keeps its file");
        assert_eq!(core.edits_since_autosave(), edits, "no snapshot when nothing changed");
        let _ = std::fs::remove_file(&src);
    }

    #[test]
    fn audio_effects_report_when_there_are_no_clips() {
        let mut core = EditorCore::new();
        core.project.audio.clear();
        core.audio_echo_last();
        assert!(core.status.contains("no audio clips"), "status: {}", core.status);
        core.audio_lowpass_last();
        assert!(core.status.contains("no audio clips"), "status: {}", core.status);
        core.audio_trim_silence_last();
        assert!(core.status.contains("no audio clips"), "status: {}", core.status);
    }
}
#[cfg(test)]
mod wave12_tests {
    use crate::audio_edit::WavData;
    use crate::editor::EditorCore;
    use crate::graph::{Channel, ChannelTarget, Key, TangentPreset};
    use crate::model::AudioClip;

    /// Write a tiny mono WAV into a pid-unique temp path and
    /// return the path as a String. The `wave12` tag keeps these
    /// files distinct from the wave11 tests' (same pid — one
    /// test binary).
    fn temp_wav(tag: &str, samples: Vec<f32>) -> String {
        let path = std::env::temp_dir().join(format!(
            "styk-wave12-{tag}-{}.wav",
            std::process::id()
        ));
        let wav = WavData {
            sample_rate: 8000,
            channels: 1,
            samples,
        };
        std::fs::write(&path, crate::audio_edit::write_wav(&wav)).unwrap();
        path.to_string_lossy().into_owned()
    }

    /// A fresh core whose only audio clip points at `path`.
    fn core_with_clip(path: String) -> EditorCore {
        let mut core = EditorCore::new();
        core.project.audio.clear();
        core.project.audio.push(AudioClip {
            path,
            start_frame: 0,
            gain: 1.0,
            muted: false,
        });
        core
    }

    #[test]
    fn audio_pitch_last_repoints_clip_to_a_new_file() {
        let src = temp_wav("pitch", vec![0.5; 800]);
        let mut core = core_with_clip(src.clone());
        let edits = core.edits_since_autosave();
        core.audio_pitch_last();
        assert!(core.status.contains("Pitch shift"), "status: {}", core.status);
        assert_eq!(core.edits_since_autosave(), edits + 1, "exactly one snapshot");
        let new_path = core.project.audio[0].path.clone();
        assert_ne!(new_path, src, "the clip must point at the pitch product");
        assert!(new_path.ends_with(".pitch.wav"), "path: {new_path}");
        let back = crate::audio_edit::read_wav(&new_path).expect("pitch product reads back");
        // +2 semitones resamples by 2^(2/12): 800 frames become
        // ~713 (pitch and duration change together).
        assert!(
            (700..800).contains(&back.frames()),
            "frames: {}",
            back.frames()
        );
        // The original file is untouched and still reads back.
        assert!(crate::audio_edit::read_wav(&src).is_ok());
        let _ = std::fs::remove_file(&src);
        let _ = std::fs::remove_file(&new_path);
    }

    #[test]
    fn audio_compress_last_repoints_clip_to_a_new_file() {
        let src = temp_wav("comp", vec![0.5; 800]);
        let mut core = core_with_clip(src.clone());
        let edits = core.edits_since_autosave();
        core.audio_compress_last();
        assert!(core.status.contains("Compressor"), "status: {}", core.status);
        assert_eq!(core.edits_since_autosave(), edits + 1, "exactly one snapshot");
        let new_path = core.project.audio[0].path.clone();
        assert_ne!(new_path, src, "the clip must point at the compressed product");
        assert!(new_path.ends_with(".comp.wav"), "path: {new_path}");
        let back = crate::audio_edit::read_wav(&new_path).expect("compressed product reads back");
        assert_eq!(back.frames(), 800, "compression keeps the frame count");
        assert!(crate::audio_edit::read_wav(&src).is_ok());
        let _ = std::fs::remove_file(&src);
        let _ = std::fs::remove_file(&new_path);
    }

    #[test]
    fn audio_pitch_and_compress_report_when_there_are_no_clips() {
        let mut core = EditorCore::new();
        core.project.audio.clear();
        let edits = core.edits_since_autosave();
        core.audio_pitch_last();
        assert!(core.status.contains("no audio clips"), "status: {}", core.status);
        core.audio_compress_last();
        assert!(core.status.contains("no audio clips"), "status: {}", core.status);
        assert_eq!(core.edits_since_autosave(), edits, "no snapshot without clips");
    }

    /// A stroke literal in the shape `begin_draw` records.
    fn stroke(frame: usize, points: Vec<[f32; 2]>) -> crate::model::Stroke {
        crate::model::Stroke {
            frame,
            points,
            color: 0x112233,
            width: 3.0,
            widths: Vec::new(),
            fill: None,
            spray: None,
            opacity: 1.0,
            scatter: 0.0,
        }
    }

    #[test]
    fn mirror_strokes_x_mirrors_current_frame_only() {
        let mut core = EditorCore::new();
        core.project.strokes.clear();
        let cur = core.current;
        let w = core.project.stage[0];
        core.project.strokes.push(stroke(cur, vec![[10.0, 20.0], [30.0, 40.0]]));
        core.project.strokes.push(stroke(cur + 1, vec![[10.0, 20.0]]));
        let edits = core.edits_since_autosave();
        core.mirror_strokes_x();
        assert!(
            core.status.contains("Mirrored 1 stroke(s)"),
            "status: {}",
            core.status
        );
        assert_eq!(core.edits_since_autosave(), edits + 1, "exactly one snapshot");
        // x' = stage width - x; y unchanged; order preserved.
        assert_eq!(
            core.project.strokes[0].points,
            vec![[w - 10.0, 20.0], [w - 30.0, 40.0]]
        );
        // The other frame's stroke is untouched.
        assert_eq!(core.project.strokes[1].points, vec![[10.0, 20.0]]);
        // Mirroring again restores the original points exactly.
        core.mirror_strokes_x();
        assert_eq!(core.project.strokes[0].points, vec![[10.0, 20.0], [30.0, 40.0]]);
    }

    #[test]
    fn mirror_strokes_x_noops_without_strokes_on_the_frame() {
        let mut core = EditorCore::new();
        core.project.strokes.clear();
        // A stroke on ANOTHER frame does not count.
        core.project.strokes.push(stroke(core.current + 1, vec![[1.0, 2.0]]));
        let edits = core.edits_since_autosave();
        core.mirror_strokes_x();
        assert!(
            core.status.contains("No strokes to mirror"),
            "status: {}",
            core.status
        );
        assert_eq!(core.edits_since_autosave(), edits, "no snapshot on a no-op");
    }

    #[test]
    fn apply_graph_ease_preset_shapes_selected_figure_only() {
        let mut core = EditorCore::new();
        let fi = core.selected_figure;
        let keys = || {
            vec![
                Key::flat(0.0, 0.0),
                Key::flat(10.0, 50.0),
                Key::flat(20.0, 0.0),
            ]
        };
        core.project.channels.push(Channel {
            figure: fi,
            target: ChannelTarget::Angle(0),
            keys: keys(),
        });
        core.project.channels.push(Channel {
            figure: fi + 1,
            target: ChannelTarget::Angle(0),
            keys: keys(),
        });
        let edits = core.edits_since_autosave();
        core.apply_graph_ease_preset(TangentPreset::Smooth);
        assert!(core.status.contains("Smooth"), "status: {}", core.status);
        assert!(
            core.status.contains("1 graph channel(s)"),
            "status: {}",
            core.status
        );
        assert_eq!(core.edits_since_autosave(), edits + 1, "exactly one snapshot");
        // Smooth is Catmull-Rom: the first key's tangents are the
        // one-sided secant (50 - 0) / (10 - 0) = 5.
        assert_eq!(core.project.channels[0].keys[0].out_tan, 5.0);
        assert_eq!(core.project.channels[0].keys[0].in_tan, 5.0);
        // The other figure's channel keeps its flat tangents.
        assert_eq!(core.project.channels[1].keys[0].out_tan, 0.0);
        assert_eq!(core.project.channels[1].keys[0].in_tan, 0.0);
    }

    #[test]
    fn apply_graph_ease_preset_noops_without_channels_for_the_figure() {
        let mut core = EditorCore::new();
        core.project.channels.clear();
        let edits = core.edits_since_autosave();
        core.apply_graph_ease_preset(TangentPreset::EaseInOut);
        assert!(
            core.status.contains("No graph channels"),
            "status: {}",
            core.status
        );
        assert_eq!(core.edits_since_autosave(), edits, "no snapshot on a no-op");
        // A channel belonging to another figure does not count.
        core.project.channels.push(Channel {
            figure: core.selected_figure + 1,
            target: ChannelTarget::RootX,
            keys: vec![Key::flat(0.0, 0.0), Key::flat(10.0, 10.0)],
        });
        core.apply_graph_ease_preset(TangentPreset::EaseInOut);
        assert!(
            core.status.contains("No graph channels"),
            "status: {}",
            core.status
        );
        assert_eq!(core.edits_since_autosave(), edits, "still no snapshot");
    }

    /// Assert the shared stamp state on the last stroke of a
    /// core whose strokes were cleared: current frame, selected
    /// figure's colour, active brush width (per point too), no
    /// fill.
    fn check_stamp(core: &EditorCore) -> &crate::model::Stroke {
        assert_eq!(core.project.strokes.len(), 1, "exactly one stroke stamped");
        let s = core.project.strokes.last().unwrap();
        assert_eq!(s.frame, core.current);
        assert_eq!(s.color, core.project.figures[core.selected_figure].color);
        assert_eq!(s.width, core.active_brush().width);
        assert_eq!(s.fill, None);
        assert_eq!(s.widths.len(), s.points.len(), "a width per point");
        assert!(s.widths.iter().all(|&x| x == s.width));
        s
    }

    #[test]
    fn stamp_polygon_appends_a_pentagon_at_stage_centre() {
        let mut core = EditorCore::new();
        core.project.strokes.clear();
        let (w, h) = (core.project.stage[0], core.project.stage[1]);
        let edits = core.edits_since_autosave();
        core.stamp_polygon();
        assert!(
            core.status.contains("Stamped a pentagon"),
            "status: {}",
            core.status
        );
        assert_eq!(core.edits_since_autosave(), edits + 1, "exactly one snapshot");
        let s = check_stamp(&core);
        // Closed pentagon: 5 vertices + the closing repeat, the
        // first vertex pointing up from the centre (radius 60).
        assert_eq!(s.points.len(), 6);
        let first = s.points[0];
        assert!((first[0] - w / 2.0).abs() < 0.001, "first x: {}", first[0]);
        assert!(
            (first[1] - (h / 2.0 - 60.0)).abs() < 0.001,
            "first y: {}",
            first[1]
        );
        assert_eq!(s.points[5], s.points[0], "closed: last repeats first");
    }

    #[test]
    fn stamp_star_appends_a_five_pointed_star_at_stage_centre() {
        let mut core = EditorCore::new();
        core.project.strokes.clear();
        let (cx, cy) = (core.project.stage[0] / 2.0, core.project.stage[1] / 2.0);
        let edits = core.edits_since_autosave();
        core.stamp_star();
        assert!(core.status.contains("Stamped a star"), "status: {}", core.status);
        assert_eq!(core.edits_since_autosave(), edits + 1, "exactly one snapshot");
        let s = check_stamp(&core);
        // 2 * 5 alternating outer/inner vertices + closing repeat.
        assert_eq!(s.points.len(), 11);
        assert_eq!(s.points[10], s.points[0], "closed: last repeats first");
        // Every vertex sits on the outer (60) or inner (26) ring.
        for p in &s.points {
            let d = ((p[0] - cx) * (p[0] - cx) + (p[1] - cy) * (p[1] - cy)).sqrt();
            assert!(
                (d - 60.0).abs() < 0.01 || (d - 26.0).abs() < 0.01,
                "vertex distance from centre: {d}"
            );
        }
    }

    #[test]
    fn stamp_arrow_appends_a_right_pointing_arrow_at_stage_centre() {
        let mut core = EditorCore::new();
        core.project.strokes.clear();
        let (cx, cy) = (core.project.stage[0] / 2.0, core.project.stage[1] / 2.0);
        let edits = core.edits_since_autosave();
        core.stamp_arrow();
        assert!(
            core.status.contains("Stamped an arrow"),
            "status: {}",
            core.status
        );
        assert_eq!(core.edits_since_autosave(), edits + 1, "exactly one snapshot");
        let s = check_stamp(&core);
        // [tail, tip, wing, tip, wing] — the shaft endpoints are
        // the caller's points verbatim.
        assert_eq!(s.points.len(), 5);
        assert_eq!(s.points[0], [cx - 70.0, cy]);
        assert_eq!(s.points[1], [cx + 70.0, cy]);
        assert_eq!(s.points[3], s.points[1], "the polyline revisits the tip");
    }
}
#[cfg(test)]
mod wave13_tests {
    use crate::audio_edit::WavData;
    use crate::editor::EditorCore;
    use crate::model::AudioClip;

    /// Select a brush preset by name (the draw tests in
    /// `mod tests` pick brushes the same way).
    fn set_brush(core: &mut EditorCore, name: &str) {
        core.brush_index = crate::paint::brush_library()
            .iter()
            .position(|b| b.name == name)
            .unwrap_or_else(|| panic!("no brush named {name}"));
    }

    #[test]
    fn spray_brush_records_a_deterministic_spray_mark() {
        let mut core = EditorCore::new();
        core.project.strokes.clear();
        set_brush(&mut core, "Spray");
        let brush = core.active_brush();
        assert!(brush.scatter > 0.0, "the Spray preset must scatter");
        core.begin_draw([0.0, 0.0]);
        core.draw_to([30.0, 0.0]);
        core.end_draw();
        assert_eq!(core.project.strokes.len(), 1);
        let s = &core.project.strokes[0];
        // The recorded points stay the drawn path — the Spray
        // preset has no smoothing and a round nib, so neither
        // end-of-stroke step rewrites them; the mark is what
        // tells renderers to scatter dots along it.
        assert_eq!(s.points, vec![[0.0, 0.0], [30.0, 0.0]]);
        let mark = s.spray.expect("a scatter brush records a spray mark");
        assert_eq!(mark.seed, 0x5EED_0000u64, "frame 0, stroke index 0");
        assert_eq!(mark.radius, brush.scatter);
        assert_eq!(mark.opacity, brush.opacity);

        // A second spray stroke on a later frame: the seed
        // moves with both the frame (x131) and the stroke's
        // own index (+1).
        core.current = 3;
        core.begin_draw([10.0, 10.0]);
        core.end_draw();
        assert_eq!(core.project.strokes.len(), 2);
        let mark = core.project.strokes[1]
            .spray
            .expect("the second spray stroke is marked too");
        assert_eq!(mark.seed, 0x5EED_0000u64 + 3 * 131 + 1);
    }

    #[test]
    fn plain_brush_records_no_spray_mark() {
        let mut core = EditorCore::new();
        core.project.strokes.clear();
        assert_eq!(core.active_brush().name, "Pen");
        assert_eq!(core.active_brush().scatter, 0.0);
        core.begin_draw([0.0, 0.0]);
        core.draw_to([30.0, 0.0]);
        core.end_draw();
        assert_eq!(core.project.strokes.len(), 1);
        assert!(core.project.strokes[0].spray.is_none());
    }

    #[test]
    fn spray_stroke_keeps_its_opacity_on_the_mark_not_the_stroke() {
        // The Spray preset is the library's one translucent
        // brush, and its opacity lives on the SprayMark — the
        // scattered dots carry it. The stroke itself must
        // record 1.0: a renderer applying both would
        // double-apply the opacity.
        let mut core = EditorCore::new();
        core.project.strokes.clear();
        set_brush(&mut core, "Spray");
        let brush_opacity = core.active_brush().opacity;
        assert!(
            brush_opacity != 1.0,
            "the Spray preset is the translucent one"
        );
        core.begin_draw([0.0, 0.0]);
        core.draw_to([30.0, 0.0]);
        core.end_draw();
        let s = &core.project.strokes[0];
        assert_eq!(
            s.spray.expect("a scatter brush records a spray mark").opacity,
            brush_opacity,
            "the mark carries the brush's opacity"
        );
        assert_eq!(s.opacity, 1.0, "the stroke itself stays opaque");
    }

    #[test]
    fn drawn_and_stamped_strokes_record_the_active_brush_opacity() {
        // With the default Pen (opacity 1.0) both recording
        // paths — a drawn stroke and a shape stamp — carry
        // the active brush's opacity onto the stroke. No
        // non-spray preset in the library is translucent yet,
        // so a translucent freehand stroke cannot be drawn
        // through a preset here; the spray test above covers
        // the one brush whose opacity is not 1.0.
        let mut core = EditorCore::new();
        core.project.strokes.clear();
        let brush_opacity = core.active_brush().opacity;
        core.begin_draw([0.0, 0.0]);
        core.draw_to([30.0, 0.0]);
        core.end_draw();
        assert_eq!(core.project.strokes[0].opacity, brush_opacity);
        core.stamp_polygon();
        assert_eq!(core.project.strokes.len(), 2);
        assert_eq!(core.project.strokes[1].opacity, brush_opacity);
    }

    fn fx_entry(kind: crate::vfx::ClipKind, seed: u32) -> crate::model::FxClipEntry {
        crate::model::FxClipEntry {
            clip: crate::vfx::ParticleClip {
                start: 0.0,
                duration: 16.0,
                kind,
                color: 0xFFCC00,
                seed,
                angle: 0.0,
                length: 130.0,
                color2: None,
            },
            figure: 0,
            node: 0,
            hidden: false,
        }
    }

    #[test]
    fn fx_preset_save_then_apply_round_trips_the_clip() {
        let mut core = EditorCore::new();
        core.project.fx_clips.clear();
        core.project
            .fx_clips
            .push(fx_entry(crate::vfx::ClipKind::Sparkle, 42));
        let edits = core.edits_since_autosave();

        // Saving writes the preset file but is not an undoable
        // project change: no snapshot.
        core.fx_save_preset(0);
        assert!(
            core.status.contains("FX preset 'sparkle-1' saved"),
            "status: {}",
            core.status
        );
        assert_eq!(core.edits_since_autosave(), edits, "save takes no snapshot");
        assert!(
            core.fx_preset_names().iter().any(|n| n == "sparkle-1"),
            "the saved preset is listed"
        );

        // Applying adds a fresh entry at the current frame on
        // the selected figure (node 0), as one undo step.
        core.current = 4;
        core.fx_apply_preset("sparkle-1");
        assert!(
            core.status.contains("added at frame 5"),
            "status: {}",
            core.status
        );
        assert_eq!(core.edits_since_autosave(), edits + 1, "exactly one snapshot");
        assert_eq!(core.project.fx_clips.len(), 2);
        let added = &core.project.fx_clips[1];
        assert!(matches!(added.clip.kind, crate::vfx::ClipKind::Sparkle));
        assert_eq!(added.clip.seed, 42, "the preset's clip data round-trips");
        assert_eq!(added.clip.start, 4.0);
        assert_eq!(added.figure, core.selected_figure);
        assert_eq!(added.node, 0);

        let _ = std::fs::remove_file(
            std::path::PathBuf::from("fx-presets").join("sparkle-1.json"),
        );
    }

    #[test]
    fn fx_preset_failures_change_nothing() {
        let mut core = EditorCore::new();
        core.project.fx_clips.clear();
        let edits = core.edits_since_autosave();
        core.fx_save_preset(0);
        assert!(
            core.status.contains("no FX clip 1"),
            "status: {}",
            core.status
        );
        core.fx_apply_preset("wave13-no-such-preset");
        assert!(
            core.status.contains("FX preset apply failed"),
            "status: {}",
            core.status
        );
        assert!(core.project.fx_clips.is_empty());
        assert_eq!(core.edits_since_autosave(), edits, "no snapshot on failures");
    }

    /// Write a tiny mono WAV into a pid-unique temp path and
    /// return the path as a String. The `wave13` tag keeps these
    /// files distinct from the wave11/12 tests' (same pid — one
    /// test binary).
    fn temp_wav(tag: &str, samples: Vec<f32>) -> String {
        let path = std::env::temp_dir().join(format!(
            "styk-wave13-{tag}-{}.wav",
            std::process::id()
        ));
        let wav = WavData {
            sample_rate: 8000,
            channels: 1,
            samples,
        };
        std::fs::write(&path, crate::audio_edit::write_wav(&wav)).unwrap();
        path.to_string_lossy().into_owned()
    }

    /// A fresh core whose only audio clip points at `path`,
    /// starting at `start_frame`, with the project at `fps`.
    fn core_with_clip(path: String, start_frame: usize, fps: f32) -> EditorCore {
        let mut core = EditorCore::new();
        core.project.fps = fps;
        core.project.audio.clear();
        core.project.audio.push(AudioClip {
            path,
            start_frame,
            gain: 1.0,
            muted: false,
        });
        core
    }

    #[test]
    fn audio_split_at_silences_last_splits_two_bursts() {
        // 0.5s burst, 0.5s silence, 0.5s burst at 8 kHz: the
        // gap (4000 frames) clears the 0.25s minimum (2000
        // frames), so the sounding ranges are exactly the two
        // bursts.
        let mut samples = vec![0.5; 4000];
        samples.extend(vec![0.0; 4000]);
        samples.extend(vec![0.5; 4000]);
        let src = temp_wav("bursts", samples);
        let mut core = core_with_clip(src.clone(), 10, 30.0);
        let edits = core.edits_since_autosave();
        core.audio_split_at_silences_last();
        assert!(
            core.status.contains("split at silences into 2 clips"),
            "status: {}",
            core.status
        );
        assert_eq!(core.edits_since_autosave(), edits + 1, "exactly one snapshot");
        assert_eq!(core.project.audio.len(), 2);
        let (first, second) = (&core.project.audio[0], &core.project.audio[1]);
        assert!(first.path.ends_with(".seg1.wav"), "path: {}", first.path);
        assert!(second.path.ends_with(".seg2.wav"), "path: {}", second.path);
        // Segment 1 starts where the clip started; segment 2 is
        // offset by its range start (8000 frames = 1.0s = 30
        // frames at 30 fps).
        assert_eq!(first.start_frame, 10);
        assert_eq!(second.start_frame, 40);
        assert!(first.start_frame < second.start_frame, "starts are ordered");
        for clip in [first, second] {
            let back =
                crate::audio_edit::read_wav(&clip.path).expect("segment file reads back");
            assert_eq!(back.frames(), 4000, "each burst is one segment");
        }
        // The original file is untouched.
        assert_eq!(crate::audio_edit::read_wav(&src).unwrap().frames(), 12000);
        let _ = std::fs::remove_file(&src);
        let _ = std::fs::remove_file(&first.path);
        let _ = std::fs::remove_file(&second.path);
    }

    #[test]
    fn audio_split_at_silences_last_noops_without_a_long_gap() {
        // Continuous sound is a single range: nothing to split,
        // no files written, no snapshot.
        let src = temp_wav("steady", vec![0.5; 8000]);
        let mut core = core_with_clip(src.clone(), 7, 30.0);
        let edits = core.edits_since_autosave();
        core.audio_split_at_silences_last();
        assert!(
            core.status.contains("no silence of 0.25s or more"),
            "status: {}",
            core.status
        );
        assert_eq!(core.edits_since_autosave(), edits, "no snapshot on a no-op");
        assert_eq!(core.project.audio.len(), 1);
        assert_eq!(core.project.audio[0].path, src);
        assert_eq!(core.project.audio[0].start_frame, 7);
        let source_path = std::path::PathBuf::from(&src);
        let stem = source_path
            .file_stem()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let seg1 = source_path.with_file_name(format!("{stem}.seg1.wav"));
        assert!(!seg1.exists(), "no segment file is written on a no-op");
        let _ = std::fs::remove_file(&src);
    }

    #[test]
    fn audio_split_at_silences_last_reports_when_there_are_no_clips() {
        let mut core = EditorCore::new();
        core.project.audio.clear();
        let edits = core.edits_since_autosave();
        core.audio_split_at_silences_last();
        assert!(
            core.status.contains("no audio clips"),
            "status: {}",
            core.status
        );
        assert_eq!(core.edits_since_autosave(), edits, "no snapshot without clips");
    }
}

#[cfg(test)]
mod wave14_camera_tests {
    use crate::editor::EditorCore;
    use crate::model::{
        CameraKey, FigureDef, FigureInstance, Frame, NodeDef, NodeStyle, Pose, Shape,
    };

    /// Two `FigureDef::simple()` figures with all-zero pose
    /// angles. With every angle 0, FK points every segment
    /// straight down (model::dir(0) = [0, 1]), so each figure's
    /// joints lie on the vertical line through its root, spanning
    /// root.y ..= root.y + 118 (the simple rig's longest
    /// root-to-node chain: hips -> chest 52 -> arm 66 = 118).
    /// Alpha's root is [100, 100], Beta's is [500, 300]; the
    /// demo stage (960 x 720) is kept.
    fn two_figure_core(frames: usize) -> EditorCore {
        let mut core = EditorCore::new();
        let def = FigureDef::simple();
        let fig = |name: &str| FigureInstance {
            name: name.into(),
            color: 0xFFFFFF,
            def: def.clone(),
            hidden: false,
            clip: None,
            states: Vec::new(),
            locked: false,
            opacity: 1.0,
        };
        core.project.figures = vec![fig("Alpha"), fig("Beta")];
        let frame = || Frame {
            poses: vec![
                Pose {
                    root: [100.0, 100.0],
                    angles: vec![0.0; 7],
                    sx: 1.0,
                    sy: 1.0,
                },
                Pose {
                    root: [500.0, 300.0],
                    angles: vec![0.0; 7],
                    sx: 1.0,
                    sy: 1.0,
                },
            ],
            duration: 1.0,
            easing: None,
            bends: Vec::new(),
        };
        core.project.frames = (0..frames).map(|_| frame()).collect();
        core.project.camera = Vec::new();
        core.project.camera_follow = None;
        core.current = 0;
        core.selected_figure = 0;
        core
    }

    #[test]
    fn frame_all_fits_two_figures_with_ten_percent_margin() {
        let mut core = two_figure_core(1);
        let edits = core.edits_since_autosave();
        core.frame_all_figures();
        assert_eq!(core.edits_since_autosave(), edits + 1, "exactly one snapshot");
        assert_eq!(core.project.camera.len(), 1);
        let k = &core.project.camera[0];
        assert_eq!(k.frame, 0);
        // Joint bounds: x 100..=500, y 100..=418 -> centre (300, 259).
        assert!((k.x - 300.0).abs() < 1e-3, "x: {}", k.x);
        assert!((k.y - 259.0).abs() < 1e-3, "y: {}", k.y);
        // Derivation: the view shows stage/zoom stage units around
        // the camera centre (the view closure in export.rs
        // render_frame_at; the canvas map in app.rs paint_stage
        // agrees), so fitting bounds bw x bh needs
        // zoom <= stage_w / bw and zoom <= stage_h / bh. Here
        // 960/400 = 2.4 and 720/318 = 2.26415 — the height binds —
        // and the 10% margin divides by 1.1.
        let expected = (720.0f32 / 318.0) / 1.1;
        assert!((k.zoom - expected).abs() < 1e-4, "zoom {} vs {expected}", k.zoom);
        assert_eq!(k.angle, 0.0);
        // The evaluated camera at frame 0 is the key itself.
        assert_eq!(core.project.camera_at(0.0), (k.x, k.y, k.zoom));
    }

    #[test]
    fn frame_all_ignores_hidden_figures() {
        let mut core = two_figure_core(1);
        core.project.figures[1].hidden = true;
        core.frame_all_figures();
        let k = &core.project.camera[0];
        // Only Alpha counts: bounds x = {100} (zero width imposes
        // no constraint), y 100..=218 -> centre (100, 159), zoom
        // from the height alone.
        assert!((k.x - 100.0).abs() < 1e-3, "x: {}", k.x);
        assert!((k.y - 159.0).abs() < 1e-3, "y: {}", k.y);
        let expected = (720.0f32 / 118.0) / 1.1;
        assert!((k.zoom - expected).abs() < 1e-4, "zoom {} vs {expected}", k.zoom);
    }

    #[test]
    fn frame_all_preserves_the_current_angle_when_inserting_a_key() {
        let mut core = two_figure_core(3);
        core.project.camera = vec![CameraKey {
            frame: 0,
            x: 480.0,
            y: 360.0,
            zoom: 1.0,
            angle: 30.0,
            easing: crate::model::CameraEase::Linear,
        }];
        core.current = 2;
        core.frame_all_figures();
        // The fresh key at frame 2 must carry the angle in effect
        // there (30, held from frame 0's key), not the 0.0 that
        // set_camera_key gives a new key.
        let k = core
            .project
            .camera
            .iter()
            .find(|k| k.frame == 2)
            .expect("key at frame 2");
        assert_eq!(k.angle, 30.0);
        assert_eq!(core.project.camera_angle_at(2.0), 30.0);
    }

    #[test]
    fn frame_all_noops_without_figures() {
        let mut core = two_figure_core(1);
        core.project.figures = Vec::new();
        let edits = core.edits_since_autosave();
        core.frame_all_figures();
        assert_eq!(core.status, "No figures to frame.");
        assert!(core.project.camera.is_empty());
        assert_eq!(core.edits_since_autosave(), edits, "no snapshot");
    }

    #[test]
    fn frame_all_single_point_bounds_keeps_current_zoom() {
        let mut core = two_figure_core(1);
        core.project.figures = vec![FigureInstance {
            name: "Dot".into(),
            color: 0xFFFFFF,
            def: FigureDef {
                nodes: vec![NodeDef {
                    min_angle: None,
                    max_angle: None,
                    name: "origin".into(),
                    parent: None,
                    length: 0.0,
                    rest_angle: 0.0,
                    head: false,
                    bend: 1.0,
                    thickness: 6.0,
                    color: None,
                    ik_end: false,
                    shape: Shape::Line,
                    style: NodeStyle::default(),
                    media: None,
                }],
            },
            hidden: false,
            clip: None,
            states: Vec::new(),
            locked: false,
            opacity: 1.0,
        }];
        core.project.frames[0].poses = vec![Pose {
            root: [200.0, 200.0],
            angles: vec![0.0],
            sx: 1.0,
            sy: 1.0,
        }];
        core.frame_all_figures();
        let k = &core.project.camera[0];
        assert_eq!((k.x, k.y), (200.0, 200.0));
        // No camera keys existed, so the current zoom is the
        // no-keys default 1.0, and a point bounds must keep it.
        assert_eq!(k.zoom, 1.0);
    }

    #[test]
    fn centre_on_selection_centres_on_the_selected_root_and_keeps_zoom() {
        let mut core = two_figure_core(1);
        core.project.camera = vec![CameraKey {
            frame: 0,
            x: 480.0,
            y: 360.0,
            zoom: 2.0,
            angle: 0.0,
            easing: crate::model::CameraEase::Linear,
        }];
        core.selected_figure = 1;
        let edits = core.edits_since_autosave();
        core.centre_on_selection();
        assert_eq!(core.edits_since_autosave(), edits + 1, "exactly one snapshot");
        assert_eq!(
            core.project.camera.len(),
            1,
            "the key at frame 0 is replaced, not duplicated"
        );
        let k = &core.project.camera[0];
        // Beta's root, with the pre-existing zoom untouched.
        assert_eq!((k.x, k.y, k.zoom), (500.0, 300.0, 2.0));
        assert!(core.status.contains("Beta"), "status: {}", core.status);
    }

    #[test]
    fn centre_on_selection_noops_without_figures() {
        let mut core = two_figure_core(1);
        core.project.figures = Vec::new();
        let edits = core.edits_since_autosave();
        core.centre_on_selection();
        assert_eq!(core.status, "No figures to centre on.");
        assert!(core.project.camera.is_empty());
        assert_eq!(core.edits_since_autosave(), edits, "no snapshot");
    }
}

#[cfg(test)]
mod wave15_graph_interp_tests {
    use super::EditorCore;
    use crate::graph::{Channel, ChannelTarget, Interp, Key};

    /// A core whose selected figure has one angle channel for
    /// joint 0 with Hermite keys at frames 0, 10 and 20, and
    /// joint 0 selected.
    fn core_with_angle_keys() -> EditorCore {
        let mut core = EditorCore::new();
        let fi = core.selected_figure;
        core.selected_node = Some(0);
        core.project.channels.clear();
        core.project.channels.push(Channel {
            figure: fi,
            target: ChannelTarget::Angle(0),
            keys: vec![
                Key::flat(0.0, 0.0),
                Key::flat(10.0, 50.0),
                Key::flat(20.0, 0.0),
            ],
        });
        core
    }

    #[test]
    fn set_graph_key_interp_changes_only_the_key_at_the_current_frame() {
        let mut core = core_with_angle_keys();
        core.current = 10;
        let edits = core.edits_since_autosave();
        core.set_graph_key_interp(Interp::Linear);
        assert!(core.status.contains("Linear"), "status: {}", core.status);
        assert!(core.status.contains("frame 11"), "status: {}", core.status);
        assert_eq!(core.edits_since_autosave(), edits + 1, "exactly one snapshot");
        let keys = &core.project.channels[0].keys;
        assert_eq!(keys[0].interp, Interp::Hermite, "earlier key untouched");
        assert_eq!(keys[1].interp, Interp::Linear, "the key at the current frame changed");
        assert_eq!(keys[2].interp, Interp::Hermite, "later key untouched");
    }

    #[test]
    fn set_graph_key_interp_picks_the_nearest_key_to_the_current_frame() {
        let mut core = core_with_angle_keys();
        // 17 is 3 frames from the key at 20 and 7 from the key at 10.
        core.current = 17;
        core.set_graph_key_interp(Interp::Step);
        assert!(core.status.contains("frame 21"), "status: {}", core.status);
        let keys = &core.project.channels[0].keys;
        assert_eq!(keys[2].interp, Interp::Step, "nearest key changed");
        assert_eq!(keys[1].interp, Interp::Hermite, "farther key untouched");
        assert_eq!(keys[0].interp, Interp::Hermite, "farthest key untouched");
        // A tie goes to the earlier key: 15 is equidistant from
        // the keys at 10 and 20.
        core.current = 15;
        core.set_graph_key_interp(Interp::Linear);
        let keys = &core.project.channels[0].keys;
        assert_eq!(keys[1].interp, Interp::Linear, "earlier key wins the tie");
        assert_eq!(keys[2].interp, Interp::Step, "later key keeps its mode");
    }

    #[test]
    fn set_graph_key_interp_noops_without_a_joint_or_keys() {
        let mut core = EditorCore::new();
        core.project.channels.clear();
        core.selected_node = None;
        let edits = core.edits_since_autosave();
        core.set_graph_key_interp(Interp::Linear);
        assert!(
            core.status.contains("Select a joint"),
            "status: {}",
            core.status
        );
        assert_eq!(core.edits_since_autosave(), edits, "no snapshot on a no-op");
        // Joint selected but no channel for it.
        core.selected_node = Some(0);
        core.set_graph_key_interp(Interp::Linear);
        assert!(
            core.status.contains("No graph keys"),
            "status: {}",
            core.status
        );
        assert_eq!(core.edits_since_autosave(), edits, "still no snapshot");
        // A channel for another target does not count.
        core.project.channels.push(Channel {
            figure: core.selected_figure,
            target: ChannelTarget::RootX,
            keys: vec![Key::flat(0.0, 0.0)],
        });
        core.set_graph_key_interp(Interp::Linear);
        assert!(
            core.status.contains("No graph keys"),
            "status: {}",
            core.status
        );
        assert_eq!(core.edits_since_autosave(), edits, "still no snapshot");
    }
}

#[cfg(test)]
mod wave_y_p10_tests {
    use super::*;

    fn project_json(core: &EditorCore) -> String {
        serde_json::to_string(&core.project).expect("project serialises")
    }

    /// Move the current frame's pose off the rest pose
    /// without spending an undo step (a direct write, as
    /// a corrupt or hand-edited project would carry).
    fn displace_current_pose(core: &mut EditorCore) {
        core.project.frames[0].poses[0].angles[5] += 30.0;
    }

    #[test]
    fn set_rest_from_pose_takes_exactly_one_undo_step() {
        let mut core = EditorCore::new();
        displace_current_pose(&mut core);
        let expected_rest = core.project.frames[0].poses[0].angles.clone();
        let before = project_json(&core);
        assert_eq!(core.undo_depth(), 0);
        assert!(core.set_rest_from_pose(0, true));
        assert_eq!(core.undo_depth(), 1, "one snapshot for the whole rebase");
        assert!(core.status.contains("rebased"), "status: {}", core.status);
        let stored = core.project.figures[0].def.rest_angles();
        assert!(
            stored
                .iter()
                .zip(expected_rest.iter())
                .all(|(a, b)| (norm_deg(*a) - norm_deg(*b)).abs() < 1e-3),
            "the stored rest is the current frame pose's angles"
        );
        core.undo();
        assert_eq!(project_json(&core), before, "one undo restores everything");
    }

    #[test]
    fn set_rest_absolute_mode_leaves_the_timeline_untouched() {
        let mut core = EditorCore::new();
        displace_current_pose(&mut core);
        let frames_before =
            serde_json::to_string(&core.project.frames).expect("frames serialise");
        let rest_before = core.project.figures[0].def.rest_angles();
        assert!(core.set_rest_from_pose(0, false));
        assert_eq!(core.undo_depth(), 1);
        assert!(core.status.contains("absolute"), "status: {}", core.status);
        assert_eq!(
            serde_json::to_string(&core.project.frames).expect("frames serialise"),
            frames_before,
            "absolute mode never touches the timeline"
        );
        let rest_after = core.project.figures[0].def.rest_angles();
        assert!(
            (rest_after[5] - rest_before[5]).abs() > 1.0,
            "the rest itself moved"
        );
    }

    #[test]
    fn set_rest_refusals_and_no_op_take_no_snapshot() {
        let mut core = EditorCore::new();
        // Unknown figure: refusal with a status, no step.
        assert!(!core.set_rest_from_pose(9, true));
        assert_eq!(core.undo_depth(), 0);
        assert!(core.status.contains("No figure 10"), "status: {}", core.status);
        // Already-rest: the current frame pose IS the rest
        // pose — a silent no-op.
        let root = core.project.frames[0].poses[0].root;
        core.project.frames[0].poses[0] = core.project.figures[0].def.rest_pose(root);
        let status_before = core.status.clone();
        assert!(!core.set_rest_from_pose(0, true));
        assert_eq!(core.undo_depth(), 0);
        assert_eq!(core.status, status_before, "no status churn on a no-op");
        // Malformed poses refuse at the Project level (the
        // editor op reads its pose from the timeline, so a
        // short or non-finite pose can only arrive through
        // a corrupt project — drive the core op directly).
        let short = crate::model::Pose {
            root: [0.0, 0.0],
            angles: vec![0.0; 2],
            sx: 1.0,
            sy: 1.0,
        };
        assert!(core
            .project
            .set_rest_from_pose(0, &short, SetRestMode::Rebase)
            .is_none());
        let mut nan = core.project.figures[0].def.rest_pose([0.0, 0.0]);
        nan.angles[0] = f32::NAN;
        assert!(core
            .project
            .set_rest_from_pose(0, &nan, SetRestMode::Absolute)
            .is_none());
        assert_eq!(core.undo_depth(), 0);
    }

    #[test]
    fn reset_pose_to_rest_is_one_labelled_step_and_exact() {
        let mut core = EditorCore::new();
        // Pose freely: angles off rest AND a squash the
        // rest pose does not carry.
        displace_current_pose(&mut core);
        core.project.frames[0].poses[0].sx = 0.8;
        core.project.frames[0].poses[0].sy = 1.25;
        let root = core.project.frames[0].poses[0].root;
        assert!(core.reset_pose_to_rest(0, 0));
        assert_eq!(core.undo_depth(), 1, "one snapshot for the restore");
        assert_eq!(core.undo_label(), Some("Reset pose"));
        assert!(
            core.status.contains("reset to its rest pose"),
            "status: {}",
            core.status
        );
        // Full-structure equality with the setup pose:
        // the JSON form compares root, angles and scale
        // together (Pose carries no PartialEq).
        let stored =
            serde_json::to_string(&core.project.frames[0].poses[0]).expect("pose serialises");
        let rest = serde_json::to_string(&core.project.figures[0].def.rest_pose(root))
            .expect("rest serialises");
        assert_eq!(stored, rest, "the restore is exact, not approximate");
        assert_eq!(core.project.frames[0].poses[0].root, root, "the root is kept");
        core.undo();
        assert_eq!(
            core.project.frames[0].poses[0].sx, 0.8,
            "one undo returns the freed pose"
        );
        assert_eq!(core.project.frames[0].poses[0].sy, 1.25);
    }

    #[test]
    fn reset_pose_to_rest_refusals_and_no_op_take_no_snapshot() {
        let mut core = EditorCore::new();
        // Unknown figure: refusal with a status, no step.
        assert!(!core.reset_pose_to_rest(9, 0));
        assert_eq!(core.status, "No such figure.");
        // No pose on the frame: a frame past the end, and
        // a figure whose pose row the frame lacks.
        assert!(!core.reset_pose_to_rest(0, 99));
        assert!(
            core.status.contains("has no pose on frame 100"),
            "status: {}",
            core.status
        );
        core.project.frames[0].poses.clear();
        assert!(!core.reset_pose_to_rest(0, 0));
        assert!(
            core.status.contains("has no pose on frame 1"),
            "status: {}",
            core.status
        );
        assert_eq!(core.undo_depth(), 0, "refusals spend no step");
        // Already rest: a silent no-op — no step, no
        // status churn (the capture op's discipline).
        let root = core.project.frames[0].poses[0].root;
        core.project.frames[0].poses[0] = core.project.figures[0].def.rest_pose(root);
        let status_before = core.status.clone();
        assert!(!core.reset_pose_to_rest(0, 0));
        assert_eq!(core.undo_depth(), 0);
        assert_eq!(core.status, status_before, "no status churn on a no-op");
    }

    #[test]
    fn setup_pose_round_trip_capture_free_restore_is_exact() {
        let mut core = EditorCore::new();
        let pristine = project_json(&core);
        // 1. Author a pose and CAPTURE it as the setup
        // pose (absolute: the timeline keeps the pose,
        // the rest becomes it).
        displace_current_pose(&mut core);
        let captured = core.project.frames[0].poses[0].angles.clone();
        assert!(core.set_rest_from_pose(0, false));
        assert_eq!(core.undo_depth(), 1);
        let rest = core.project.figures[0].def.rest_angles();
        assert!(
            rest.iter()
                .zip(captured.iter())
                .all(|(a, b)| (norm_deg(*a) - norm_deg(*b)).abs() < 1e-4),
            "the setup pose is the captured pose, normalised"
        );
        // 2. Pose freely — a different pose on the frame.
        core.project.frames[0].poses[0].angles[2] += 45.0;
        core.project.frames[0].poses[0].sx = 0.5;
        let freed =
            serde_json::to_string(&core.project.frames[0].poses[0]).expect("pose serialises");
        // 3. RESTORE: the frame pose becomes the setup
        // pose exactly — full-structure equality.
        let root = core.project.frames[0].poses[0].root;
        assert!(core.reset_pose_to_rest(0, 0));
        assert_eq!(core.undo_depth(), 2, "one step per half of the trip");
        assert_eq!(
            serde_json::to_string(&core.project.frames[0].poses[0])
                .expect("pose serialises"),
            serde_json::to_string(&core.project.figures[0].def.rest_pose(root))
                .expect("rest serialises"),
            "restore returns the captured setup pose exactly"
        );
        // 4. The whole trip unwinds through undo: the
        // freed pose returns, then the pre-capture
        // project — rest angles included — is identical.
        core.undo();
        assert_eq!(
            serde_json::to_string(&core.project.frames[0].poses[0])
                .expect("pose serialises"),
            freed,
            "undoing the restore returns the freed pose exactly"
        );
        core.undo();
        assert_eq!(
            project_json(&core),
            pristine,
            "undoing the capture restores the old setup pose"
        );
    }

    #[test]
    fn captured_setup_pose_survives_the_save_format_and_restores_after_load() {
        let mut core = EditorCore::new();
        displace_current_pose(&mut core);
        assert!(core.set_rest_from_pose(0, false));
        let rest = core.project.figures[0].def.rest_angles();
        // The on-disk form: projects::save writes
        // to_string_pretty, projects::load reads it back.
        let text = serde_json::to_string_pretty(&core.project).expect("project serialises");
        assert!(
            text.contains("\"rest_angle\""),
            "the setup angles are on the wire, not derived"
        );
        let back: crate::model::Project =
            serde_json::from_str(&text).expect("project parses back");
        assert_eq!(
            back.figures[0].def.rest_angles(),
            rest,
            "the captured setup pose is bit-exact through save/load"
        );
        // A never-captured project loads with the
        // constructor's rest untouched — no capture, no
        // behaviour change.
        let fresh: crate::model::Project =
            serde_json::from_str(&serde_json::to_string_pretty(&Project::new_demo()).unwrap())
                .unwrap();
        assert_eq!(
            fresh.figures[0].def.rest_angles(),
            Project::new_demo().figures[0].def.rest_angles()
        );
        // And the loaded project restores exactly.
        let mut core2 = EditorCore::new();
        core2.project = back;
        core2.project.frames[0].poses[0].angles[2] += 45.0;
        let root = core2.project.frames[0].poses[0].root;
        assert!(core2.reset_pose_to_rest(0, 0));
        assert_eq!(
            serde_json::to_string(&core2.project.frames[0].poses[0])
                .expect("pose serialises"),
            serde_json::to_string(&core2.project.figures[0].def.rest_pose(root))
                .expect("rest serialises"),
            "restore after load is the loaded setup pose exactly"
        );
    }

    #[test]
    fn setup_drag_edits_rest_angles_not_the_frame_pose() {
        let mut core = EditorCore::new();
        core.setup_mode = true;
        let frame_before =
            serde_json::to_string(&core.project.frames[0].poses[0]).expect("pose serialises");
        let rest_before = core.project.figures[0].def.rest_angles();
        // Grab the right-hand joint (node 6) where setup
        // mode draws it, then pull it away.
        let hand = core.setup_positions(0)[6];
        assert!(core.begin_drag(hand), "the press starts a setup drag");
        assert_eq!(core.undo_depth(), 1, "one snapshot per setup gesture");
        core.drag_to([hand[0] + 45.0, hand[1] + 30.0]);
        core.end_drag();
        let rest_after = core.project.figures[0].def.rest_angles();
        assert!(
            rest_before
                .iter()
                .zip(rest_after.iter())
                .any(|(a, b)| (a - b).abs() > 0.01),
            "the drag wrote rest angles"
        );
        assert_eq!(
            serde_json::to_string(&core.project.frames[0].poses[0]).expect("pose serialises"),
            frame_before,
            "the frame pose is never touched by a setup drag"
        );
        assert!(
            core.status.contains("Rest pose edited"),
            "status: {}",
            core.status
        );
    }

    #[test]
    fn setup_press_on_the_root_is_consumed_without_a_snapshot() {
        let mut core = EditorCore::new();
        core.setup_mode = true;
        let root_joint = core.setup_positions(0)[0];
        assert!(core.begin_drag(root_joint), "the root press is consumed");
        assert_eq!(core.undo_depth(), 0, "no snapshot for a root press");
        assert!(core.dragging.is_none(), "no drag begins");
        assert!(
            core.status.contains("no root position"),
            "status: {}",
            core.status
        );
    }
}
