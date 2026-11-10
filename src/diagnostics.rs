//! Diagnostics bundle (Roadmap Wave 2, W2-96 — roadmap
//! item E-8): the one-click evidence pack a user attaches
//! to a bug report.
//!
//! A bundle is a single `.zip` with three entries, always
//! in this (sorted) order:
//!
//! - `manifest.json` — machine-readable facts: bundle
//!   format version, app version, generation time,
//!   platform, UI scale (when the caller could read it),
//!   UI language + tier, the gate-relevant counts (MCP
//!   tool count, undo/redo depth) and the store-dir /
//!   autosave facts below, all in redacted form.
//! - `project-census.json` — the open project's
//!   *structural* census: counts and shape numbers only
//!   (frames, figures, nodes, cues, chapters, …).
//! - `report.txt` — the same facts rendered for a human
//!   reading the bug report, plus the recent status
//!   history (scrubbed, see the privacy contract).
//!
//! # The privacy contract (hard, test-pinned below)
//!
//! The bundle is built to be safe to attach to a public
//! issue. By construction it contains:
//!
//! - **No project content.** The census reads lengths,
//!   shapes and booleans only — never a pose, a name, a
//!   cue's text or speaker, a chapter or marker title, an
//!   asset or text instance's content. The project title
//!   is excluded too (decision: a title is user content
//!   like any other name; a bug report loses nothing by
//!   its absence, and "counts only" stays a rule with no
//!   exceptions to audit).
//! - **No absolute paths.** Store directories are
//!   reported by role (`<data>/projects`, `<config>`,
//!   `<state>`, …), a caller-supplied home prefix becomes
//!   `~/…`, and any other absolute path collapses to
//!   `<path>`. Status lines — the one place user paths
//!   legitimately appear ("Exported … to /home/…") — pass
//!   through [`scrub_status_text`], which replaces every
//!   path-like token with `<path>`.
//! - **No environment dump.** Nothing in this module
//!   reads an environment variable. Platform facts come
//!   from `std::env::consts` (compile-time target facts,
//!   not the process environment); the store dirs arrive
//!   already resolved by the caller via
//!   [`crate::shell::store_dirs`].
//!
//! # Determinism
//!
//! Given the same inputs (including `generated_at_unix`),
//! [`build_bundle`] produces byte-identical output: JSON
//! is emitted in struct-declaration order, report lines
//! in a fixed order, and the ZIP writer below uses the
//! stored (uncompressed) method with a fixed DOS
//! timestamp (1980-01-01) and entries sorted by name, so
//! two bundles of the same state diff cleanly. The ZIP
//! writer is hand-rolled on purpose: the crate has no
//! `zip` dependency, the three entries are small text,
//! and stored entries need only a CRC-32 (from the
//! already-present `flate2`). Adding the `zip` crate for
//! this is filed as a decision the integrator may take
//! (IA-W2-96-5) — it is not needed for correctness.
//!
//! Registration is the integrator's: `pub mod
//! diagnostics;` in `src/lib.rs` is IA-W2-96-1
//! (must-land-with — this module's tests compile only
//! once it lands, the wave's usual pattern). The shell
//! surface lives in `src/rune_sec/diagnostics_panel.rs`.

use std::fmt::Write as _;
use std::io;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::model::Project;
use crate::shell::{StoreDirs, RECOVERY_FILE};

/// The bundle format version written into every
/// manifest. Bump when the manifest/census shape changes
/// incompatibly; readers (humans, mostly) key off it.
pub const BUNDLE_FORMAT_VERSION: u32 = 1;

/// File stem of a written bundle: the full name is
/// `<stem>-<yyyymmdd>-<hhmmss>.zip` (UTC), so a folder of
/// bundles sorts chronologically.
pub const BUNDLE_FILE_STEM: &str = "stykanimate-diagnostics";

/// The manifest entry name (always present, always
/// first after sorting).
pub const MANIFEST_ENTRY: &str = "manifest.json";
/// The structural project census entry name.
pub const CENSUS_ENTRY: &str = "project-census.json";
/// The human-readable report entry name.
pub const REPORT_ENTRY: &str = "report.txt";

/// The longest status line the bundle keeps, in chars:
/// a pathological status (a pasted path chain, say) must
/// not bloat the pack. Truncation is marked with `…`.
const STATUS_TEXT_MAX_CHARS: usize = 200;

/// How many recovery generations the disk census looks
/// for at most. The autosave preference caps `keep` at
/// 10 (see `crate::autosave`'s `MAX_KEEP`); 64 leaves
/// generous headroom for files a larger historical keep
/// left behind without looping forever on a strange dir.
const MAX_GENERATIONS_PROBED: u32 = 64;

/// One recent-status fact as the caller hands it over:
/// a severity label (the caller's own vocabulary —
/// the shell passes its toast severities) and the raw
/// status text. The text is scrubbed at build time
/// ([`scrub_status_text`]); what lands in the bundle is
/// the scrubbed form, serialised from this same struct.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct StatusFact {
    pub severity: String,
    pub text: String,
}

/// Everything [`build_bundle`] needs that is not in the
/// project or the store dirs. Assembled by the caller
/// (the shell surface) from live state; every field that
/// a caller may be unable to supply honestly is an
/// `Option` and serialises as `null` / "not recorded"
/// rather than a guessed value.
#[derive(Clone, Debug, Default)]
pub struct DiagnosticsInput {
    /// Seconds since the Unix epoch, stamped by the
    /// caller so tests can pin it.
    pub generated_at_unix: u64,
    /// The UI scale factor, when the caller could read
    /// it. The Rune shell today never reads the host
    /// scale (W2-56's audit), so the surface passes
    /// `None` and the bundle says so — never a default
    /// dressed up as a measurement.
    pub ui_scale: Option<f32>,
    /// The UI language's code name (the shell passes its
    /// `Lang` debug name, e.g. `"En"`, `"PtBr"`).
    pub language: Option<String>,
    /// The language's coverage tier (`"full"` /
    /// `"partial"` / `"basic"`), from the same table the
    /// Settings language picker shows.
    pub language_tier: Option<String>,
    /// The MCP tool count, when the caller computed it
    /// from the real registry (`crate::mcp::all_tools`).
    pub mcp_tool_count: Option<usize>,
    /// Undo stack depth at gather time.
    pub undo_depth: usize,
    /// Redo stack depth at gather time.
    pub redo_depth: usize,
    /// Recent status history, oldest first, raw text
    /// (scrubbed during the build).
    pub recent_status: Vec<StatusFact>,
    /// The user's home directory, used ONLY as a
    /// redaction needle (paths under it render as
    /// `~/…`); the value itself is never emitted.
    pub home: Option<PathBuf>,
}

/// The structural census of one project: counts and
/// shape numbers only. This struct is the whole privacy
/// argument for the project half of the bundle — it has
/// no field that could carry content, so serialising it
/// cannot leak any.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct ProjectCensus {
    /// The project's stored format version.
    pub format_version: u32,
    /// Frames per second (a shape number, not content).
    pub fps: f32,
    /// Logical stage size `[width, height]`.
    pub stage: [f32; 2],
    /// Authored frames on the timeline.
    pub frames: usize,
    /// Total poses across all frames.
    pub poses_total: usize,
    /// Figures in the project.
    pub figures: usize,
    /// Total rig nodes across all figure definitions.
    pub figure_nodes_total: usize,
    /// Camera keys.
    pub camera_keys: usize,
    /// Subtitle cues (their text is never read).
    pub subtitle_cues: usize,
    /// Cues carrying a speaker credit (the names are
    /// never read).
    pub cues_with_speaker: usize,
    /// Episode chapters (titles never read).
    pub chapters: usize,
    /// Story-beat markers (names never read).
    pub beat_markers: usize,
    /// Figure-state clips in the project pool.
    pub clips: usize,
    /// Effects clips.
    pub fx_clips: usize,
    /// Audio clips.
    pub audio_clips: usize,
    /// Image assets (names/bytes never read).
    pub assets: usize,
    /// Sprite instances.
    pub sprites: usize,
    /// Text instances (their content is never read).
    pub texts: usize,
    /// Freehand strokes.
    pub strokes: usize,
    /// Particle events.
    pub particles: usize,
    /// Graph-editor channels.
    pub graph_channels: usize,
    /// Stored guide lines.
    pub stored_guides: usize,
    /// Whether chapter title cards are enabled.
    pub title_cards_enabled: bool,
    /// Whether a continuous weather effect is set.
    pub weather_present: bool,
}

/// Count a project's shape. Pure: reads lengths and
/// booleans only — see the module docs' privacy
/// contract.
pub fn project_census(project: &Project) -> ProjectCensus {
    ProjectCensus {
        format_version: project.format_version,
        fps: project.fps,
        stage: project.stage,
        frames: project.frames.len(),
        poses_total: project.frames.iter().map(|f| f.poses.len()).sum(),
        figures: project.figures.len(),
        figure_nodes_total: project
            .figures
            .iter()
            .map(|f| f.def.nodes.len())
            .sum(),
        camera_keys: project.camera.len(),
        subtitle_cues: project.subtitles.len(),
        cues_with_speaker: project
            .subtitles
            .iter()
            .filter(|c| c.speaker.as_deref().is_some_and(|s| !s.is_empty()))
            .count(),
        chapters: project.chapters.len(),
        beat_markers: project.beat_markers.len(),
        clips: project.clips.len(),
        fx_clips: project.fx_clips.len(),
        audio_clips: project.audio.len(),
        assets: project.assets.len(),
        sprites: project.sprites.len(),
        texts: project.texts.len(),
        strokes: project.strokes.len(),
        particles: project.particles.len(),
        graph_channels: project.channels.len(),
        stored_guides: project.stored_guides.len(),
        title_cards_enabled: project.title_cards.enabled,
        weather_present: project.weather.is_some(),
    }
}

/// Platform facts from `std::env::consts` — compile-time
/// target facts, never the process environment.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
struct PlatformFacts {
    os: &'static str,
    arch: &'static str,
    family: &'static str,
}

/// The store directories in redacted display form, one
/// field per AppDirs role (see [`redact_path`]).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
struct StoreDirFacts {
    portable: bool,
    data: String,
    projects: String,
    library: String,
    library3d: String,
    import: String,
    config: String,
    state: String,
}

/// Autosave/recovery facts read off the state dir's
/// disk shape (presence and generation count — the
/// snapshots' contents are never opened).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
struct AutosaveFacts {
    recovery_present: bool,
    generations_present: u32,
}

/// The manifest serialised into `manifest.json`.
#[derive(Serialize)]
struct Manifest {
    bundle_format: u32,
    app: &'static str,
    app_version: &'static str,
    generated_at_unix: u64,
    generated_at_utc: String,
    platform: PlatformFacts,
    ui_scale: Option<f32>,
    language: Option<String>,
    language_tier: Option<String>,
    mcp_tools: Option<usize>,
    undo_depth: usize,
    redo_depth: usize,
    store_dirs: StoreDirFacts,
    autosave: AutosaveFacts,
    project: ProjectCensus,
    recent_status: Vec<StatusFact>,
    entries: Vec<&'static str>,
    privacy: &'static str,
}

/// A built bundle: the three payloads plus the stamp
/// they were built with. [`DiagnosticsBundle::zip_bytes`]
/// packs them; [`write_bundle`] persists the pack.
#[derive(Clone, Debug, PartialEq)]
pub struct DiagnosticsBundle {
    /// The `generated_at_unix` the bundle was built
    /// with (drives the written file's name).
    pub generated_at_unix: u64,
    /// Serialised `manifest.json`.
    pub manifest_json: String,
    /// Serialised `project-census.json` (the same census
    /// the manifest embeds — the file form is the one a
    /// maintainer diffs or feeds to a tool).
    pub census_json: String,
    /// The human-readable `report.txt`.
    pub report_txt: String,
}

impl DiagnosticsBundle {
    /// The bundle's entries as `(name, bytes)`, sorted
    /// by name — the ZIP's order and the manifest's
    /// `entries` list both come from here.
    pub fn entries(&self) -> Vec<(&'static str, &[u8])> {
        let mut entries: Vec<(&'static str, &[u8])> = vec![
            (MANIFEST_ENTRY, self.manifest_json.as_bytes()),
            (CENSUS_ENTRY, self.census_json.as_bytes()),
            (REPORT_ENTRY, self.report_txt.as_bytes()),
        ];
        entries.sort_by(|a, b| a.0.cmp(b.0));
        entries
    }

    /// The written file's name for this bundle:
    /// `stykanimate-diagnostics-<yyyymmdd>-<hhmmss>.zip`
    /// (UTC), so bundles sort chronologically by name.
    pub fn file_name(&self) -> String {
        format!("{}-{}.zip", BUNDLE_FILE_STEM, file_stamp(self.generated_at_unix))
    }

    /// Pack the bundle as a ZIP archive (stored entries,
    /// fixed 1980-01-01 DOS timestamp, sorted names):
    /// byte-identical for identical bundles.
    pub fn zip_bytes(&self) -> Vec<u8> {
        zip_stored(&self.entries())
    }
}

/// Where bundles are written: a `diagnostics` folder
/// under the AppDirs data root — beside the projects,
/// never inside them, and findable from the same place
/// the app keeps everything else.
pub fn diagnostics_dir(dirs: &StoreDirs) -> PathBuf {
    dirs.data_root().join("diagnostics")
}

/// Build a bundle from a project, the resolved store
/// dirs and the caller's facts. Pure apart from the
/// state-dir presence probes (file existence only).
pub fn build_bundle(
    project: &Project,
    dirs: &StoreDirs,
    input: &DiagnosticsInput,
) -> DiagnosticsBundle {
    let census = project_census(project);
    let recent_status: Vec<StatusFact> = input
        .recent_status
        .iter()
        .map(|fact| StatusFact {
            severity: fact.severity.clone(),
            text: scrub_status_text(&fact.text, input.home.as_deref()),
        })
        .collect();
    let manifest = Manifest {
        bundle_format: BUNDLE_FORMAT_VERSION,
        app: "stykanimate-rs",
        app_version: env!("CARGO_PKG_VERSION"),
        generated_at_unix: input.generated_at_unix,
        generated_at_utc: utc_stamp(input.generated_at_unix),
        platform: PlatformFacts {
            os: std::env::consts::OS,
            arch: std::env::consts::ARCH,
            family: std::env::consts::FAMILY,
        },
        ui_scale: input.ui_scale,
        language: input.language.clone(),
        language_tier: input.language_tier.clone(),
        mcp_tools: input.mcp_tool_count,
        undo_depth: input.undo_depth,
        redo_depth: input.redo_depth,
        store_dirs: StoreDirFacts {
            portable: dirs.is_portable(),
            data: redact_path(dirs.data_root(), dirs, input.home.as_deref()),
            projects: redact_path(dirs.projects(), dirs, input.home.as_deref()),
            library: redact_path(dirs.library(), dirs, input.home.as_deref()),
            library3d: redact_path(dirs.library3d(), dirs, input.home.as_deref()),
            import: redact_path(dirs.import(), dirs, input.home.as_deref()),
            config: redact_path(dirs.config(), dirs, input.home.as_deref()),
            state: redact_path(dirs.state(), dirs, input.home.as_deref()),
        },
        autosave: autosave_facts(dirs),
        project: census.clone(),
        recent_status: recent_status.clone(),
        entries: vec![MANIFEST_ENTRY, CENSUS_ENTRY, REPORT_ENTRY],
        privacy: "counts-only: no project content, names, cue text or \
                  absolute paths; see report.txt",
    };
    let manifest_json =
        serde_json::to_string_pretty(&manifest).expect("manifest serialises");
    let census_json =
        serde_json::to_string_pretty(&census).expect("census serialises");
    let report_txt = render_report(&manifest, &recent_status);
    DiagnosticsBundle {
        generated_at_unix: input.generated_at_unix,
        manifest_json,
        census_json,
        report_txt,
    }
}

/// Write a built bundle into `dir` (created if needed)
/// under its stamped file name, overwriting a bundle
/// written in the same second. Returns the path.
pub fn write_bundle(dir: &Path, bundle: &DiagnosticsBundle) -> io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(bundle.file_name());
    std::fs::write(&path, bundle.zip_bytes())?;
    Ok(path)
}

/// Presence facts about the recovery snapshot family in
/// the state dir. The generation names mirror
/// `crate::autosave::generation_name` (`.recovery.json`,
/// `.recovery.1.json`, …) — re-derived here from
/// [`RECOVERY_FILE`] so this module does not depend on
/// the autosave module's registration state; only file
/// existence is probed, contents are never opened.
fn autosave_facts(dirs: &StoreDirs) -> AutosaveFacts {
    let state = dirs.state();
    let recovery_present = state.join(RECOVERY_FILE).exists();
    // Generations are contiguous by construction (the
    // rotation keeps the family packed from the live
    // file outward), so the census walks `.recovery.N`
    // names from 1 up to the first gap.
    let mut generations_present = u32::from(recovery_present);
    for index in 1..MAX_GENERATIONS_PROBED {
        if state.join(format!(".recovery.{index}.json")).exists() {
            generations_present += 1;
        } else {
            break;
        }
    }
    AutosaveFacts {
        recovery_present,
        generations_present,
    }
}

/// Render a path in its bundle-safe display form:
///
/// 1. A path at or under one of the store roots renders
///    as that root's role plus the relative remainder
///    (`<data>/projects`, `<config>/.layout.json`, …).
///    The longest matching root wins, so `projects`
///    (which lives under `data`) reports as its own
///    role, not as `<data>/projects`-the-remainder —
///    both spellings coincide textually, but the rule
///    keeps nested roles correct in general.
/// 2. Else, a path under `home` (when supplied) renders
///    as `~/…`.
/// 3. Else, a relative path renders as itself (it names
///    no location), and an absolute path collapses to
///    `<path>` — its location is exactly what the
///    contract withholds.
pub fn redact_path(path: &Path, dirs: &StoreDirs, home: Option<&Path>) -> String {
    let roles: [(&str, &Path); 7] = [
        ("<data>/projects", dirs.projects()),
        ("<data>/library3d", dirs.library3d()),
        ("<data>/library", dirs.library()),
        ("<data>/import", dirs.import()),
        ("<config>", dirs.config()),
        ("<state>", dirs.state()),
        ("<data>", dirs.data_root()),
    ];
    let mut best: Option<(&str, &Path)> = None;
    for (role, root) in roles {
        if path.starts_with(root) {
            let longer = best
                .map(|(_, best_root)| root.as_os_str().len() > best_root.as_os_str().len())
                .unwrap_or(true);
            if longer {
                best = Some((role, root));
            }
        }
    }
    if let Some((role, root)) = best {
        return match path.strip_prefix(root) {
            Ok(rest) if !rest.as_os_str().is_empty() => {
                format!("{}/{}", role, slash_join(rest))
            }
            _ => role.to_string(),
        };
    }
    if let Some(home) = home {
        if let Ok(rest) = path.strip_prefix(home) {
            if rest.as_os_str().is_empty() {
                return "~".to_string();
            }
            return format!("~/{}", slash_join(rest));
        }
    }
    if path.is_absolute() {
        "<path>".to_string()
    } else {
        slash_join(path)
    }
}

/// Join a relative path's components with `/` on every
/// platform, so bundle text is identical wherever the
/// bundle was made.
fn slash_join(path: &Path) -> String {
    let mut parts: Vec<String> = Vec::new();
    for component in path.components() {
        if let std::path::Component::Normal(part) = component {
            parts.push(part.to_string_lossy().into_owned());
        }
    }
    parts.join("/")
}

/// Scrub one status line for the bundle: every
/// path-like whitespace-separated token collapses to
/// `<path>`, where path-like means: starting with `/`
/// or `~`, containing a drive/scheme separator (`:/` or
/// `:\`), or containing the caller's home path anywhere
/// inside it. Surrounding punctuation (`"`, `'`, `(`,
/// `)`, `,`, `;`) is ignored for the test but the whole
/// token is replaced when it matches. Whitespace is
/// normalised to single spaces and the result is capped
/// at [`STATUS_TEXT_MAX_CHARS`] chars with a `…` mark.
///
/// This is deliberately blunt: a status line's
/// diagnostic value ("Exported 48-frame MP4 to …",
/// "Autosave failed: …") survives; only locations —
/// which can carry the user's name and project file
/// names — are withheld.
pub fn scrub_status_text(text: &str, home: Option<&Path>) -> String {
    let home_str = home.map(|h| h.to_string_lossy().into_owned());
    let mut out: Vec<String> = Vec::new();
    for token in text.split_whitespace() {
        let core = token.trim_matches(|c| matches!(c, '"' | '\'' | '(' | ')' | ',' | ';'));
        let path_like = core.starts_with('/')
            || core.starts_with('~')
            || core.contains(":/")
            || core.contains(":\\")
            || home_str
                .as_deref()
                .is_some_and(|h| !h.is_empty() && core.contains(h));
        out.push(if path_like {
            "<path>".to_string()
        } else {
            token.to_string()
        });
    }
    let joined = out.join(" ");
    if joined.chars().count() > STATUS_TEXT_MAX_CHARS {
        let mut capped: String = joined.chars().take(STATUS_TEXT_MAX_CHARS).collect();
        capped.push('…');
        capped
    } else {
        joined
    }
}

/// The human-readable report. Line order is fixed and
/// mirrors the manifest, so two reports diff cleanly.
fn render_report(manifest: &Manifest, recent_status: &[StatusFact]) -> String {
    let mut r = String::new();
    let _ = writeln!(r, "StykAnimate-RS diagnostics bundle");
    let _ = writeln!(r, "Bundle format: {}", manifest.bundle_format);
    let _ = writeln!(
        r,
        "Generated: {} (unix {})",
        manifest.generated_at_utc, manifest.generated_at_unix
    );
    let _ = writeln!(
        r,
        "App: {} {}",
        manifest.app, manifest.app_version
    );
    let _ = writeln!(
        r,
        "Platform: {} / {} ({})",
        manifest.platform.os, manifest.platform.arch, manifest.platform.family
    );
    match manifest.ui_scale {
        Some(scale) => {
            let _ = writeln!(r, "UI scale: {scale}");
        }
        None => {
            let _ = writeln!(r, "UI scale: not recorded");
        }
    }
    match (&manifest.language, &manifest.language_tier) {
        (Some(lang), Some(tier)) => {
            let _ = writeln!(r, "UI language: {lang} ({tier})");
        }
        (Some(lang), None) => {
            let _ = writeln!(r, "UI language: {lang}");
        }
        _ => {
            let _ = writeln!(r, "UI language: not recorded");
        }
    }
    let _ = writeln!(r);
    let _ = writeln!(r, "Store directories (roles, not paths):");
    let _ = writeln!(r, "  portable layout: {}", manifest.store_dirs.portable);
    let _ = writeln!(r, "  data: {}", manifest.store_dirs.data);
    let _ = writeln!(r, "  projects: {}", manifest.store_dirs.projects);
    let _ = writeln!(r, "  library: {}", manifest.store_dirs.library);
    let _ = writeln!(r, "  library3d: {}", manifest.store_dirs.library3d);
    let _ = writeln!(r, "  import: {}", manifest.store_dirs.import);
    let _ = writeln!(r, "  config: {}", manifest.store_dirs.config);
    let _ = writeln!(r, "  state: {}", manifest.store_dirs.state);
    let _ = writeln!(
        r,
        "Recovery snapshot present: {}",
        manifest.autosave.recovery_present
    );
    let _ = writeln!(
        r,
        "Recovery generations present: {}",
        manifest.autosave.generations_present
    );
    let _ = writeln!(r);
    let _ = writeln!(r, "Counts:");
    match manifest.mcp_tools {
        Some(n) => {
            let _ = writeln!(r, "  MCP tools: {n}");
        }
        None => {
            let _ = writeln!(r, "  MCP tools: not recorded");
        }
    }
    let _ = writeln!(r, "  Undo depth: {}", manifest.undo_depth);
    let _ = writeln!(r, "  Redo depth: {}", manifest.redo_depth);
    let _ = writeln!(r);
    let _ = writeln!(r, "Project census (counts only):");
    let c = &manifest.project;
    let _ = writeln!(r, "  format version: {}", c.format_version);
    let _ = writeln!(r, "  fps: {}", c.fps);
    let _ = writeln!(r, "  stage: {} x {}", c.stage[0], c.stage[1]);
    let _ = writeln!(r, "  frames: {}", c.frames);
    let _ = writeln!(r, "  poses total: {}", c.poses_total);
    let _ = writeln!(r, "  figures: {}", c.figures);
    let _ = writeln!(r, "  figure nodes total: {}", c.figure_nodes_total);
    let _ = writeln!(r, "  camera keys: {}", c.camera_keys);
    let _ = writeln!(r, "  subtitle cues: {}", c.subtitle_cues);
    let _ = writeln!(r, "  cues with speaker: {}", c.cues_with_speaker);
    let _ = writeln!(r, "  chapters: {}", c.chapters);
    let _ = writeln!(r, "  beat markers: {}", c.beat_markers);
    let _ = writeln!(r, "  clips: {}", c.clips);
    let _ = writeln!(r, "  fx clips: {}", c.fx_clips);
    let _ = writeln!(r, "  audio clips: {}", c.audio_clips);
    let _ = writeln!(r, "  image assets: {}", c.assets);
    let _ = writeln!(r, "  sprites: {}", c.sprites);
    let _ = writeln!(r, "  text instances: {}", c.texts);
    let _ = writeln!(r, "  strokes: {}", c.strokes);
    let _ = writeln!(r, "  particle events: {}", c.particles);
    let _ = writeln!(r, "  graph channels: {}", c.graph_channels);
    let _ = writeln!(r, "  stored guides: {}", c.stored_guides);
    let _ = writeln!(r, "  title cards enabled: {}", c.title_cards_enabled);
    let _ = writeln!(r, "  weather present: {}", c.weather_present);
    let _ = writeln!(r);
    let _ = writeln!(r, "Recent status (paths withheld):");
    if recent_status.is_empty() {
        let _ = writeln!(r, "  (none recorded)");
    } else {
        for fact in recent_status {
            let _ = writeln!(r, "  [{}] {}", fact.severity, fact.text);
        }
    }
    let _ = writeln!(r);
    let _ = writeln!(
        r,
        "Privacy: this bundle carries counts and shape numbers only — \
         no project content, names, cue text or absolute paths."
    );
    r
}

/// Format unix seconds as `YYYY-MM-DD HH:MM:SS UTC`.
fn utc_stamp(unix: u64) -> String {
    let (y, mo, d, h, mi, s) = utc_parts(unix);
    format!("{y:04}-{mo:02}-{d:02} {h:02}:{mi:02}:{s:02} UTC")
}

/// Format unix seconds as `YYYYMMDD-HHMMSS` (UTC), the
/// file-name stamp.
fn file_stamp(unix: u64) -> String {
    let (y, mo, d, h, mi, s) = utc_parts(unix);
    format!("{y:04}{mo:02}{d:02}-{h:02}{mi:02}{s:02}")
}

/// Split unix seconds into civil UTC parts via Howard
/// Hinnant's `civil_from_days` (public-domain
/// algorithm); no date crate is a dependency.
fn utc_parts(unix: u64) -> (i64, u32, u32, u32, u32, u32) {
    let days = (unix / 86_400) as i64;
    let secs = unix % 86_400;
    let (y, m, d) = civil_from_days(days);
    (
        y,
        m,
        d,
        (secs / 3600) as u32,
        ((secs % 3600) / 60) as u32,
        (secs % 60) as u32,
    )
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m as u32, d)
}

/// A minimal ZIP writer: stored (method 0) entries, a
/// fixed DOS timestamp (1980-01-01 00:00:00 — the ZIP
/// epoch) so output is byte-deterministic, no extra
/// fields, no comment. CRC-32 comes from `flate2`
/// (already a dependency for the Pivot formats).
fn zip_stored(entries: &[(&str, &[u8])]) -> Vec<u8> {
    const DOS_DATE_1980_01_01: u16 = 0x0021;
    let mut out: Vec<u8> = Vec::new();
    let mut central: Vec<u8> = Vec::new();
    for (name, data) in entries {
        let name_bytes = name.as_bytes();
        let crc = {
            let mut c = flate2::Crc::new();
            c.update(data);
            c.sum()
        };
        let size = data.len() as u32;
        let local_offset = out.len() as u32;
        // Local file header.
        out.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        out.extend_from_slice(&20u16.to_le_bytes()); // version needed
        out.extend_from_slice(&0u16.to_le_bytes()); // flags
        out.extend_from_slice(&0u16.to_le_bytes()); // method: stored
        out.extend_from_slice(&0u16.to_le_bytes()); // mod time
        out.extend_from_slice(&DOS_DATE_1980_01_01.to_le_bytes());
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&size.to_le_bytes()); // compressed
        out.extend_from_slice(&size.to_le_bytes()); // uncompressed
        out.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes()); // extra len
        out.extend_from_slice(name_bytes);
        out.extend_from_slice(data);
        // Central directory record.
        central.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes()); // version made by
        central.extend_from_slice(&20u16.to_le_bytes()); // version needed
        central.extend_from_slice(&0u16.to_le_bytes()); // flags
        central.extend_from_slice(&0u16.to_le_bytes()); // method
        central.extend_from_slice(&0u16.to_le_bytes()); // mod time
        central.extend_from_slice(&DOS_DATE_1980_01_01.to_le_bytes());
        central.extend_from_slice(&crc.to_le_bytes());
        central.extend_from_slice(&size.to_le_bytes());
        central.extend_from_slice(&size.to_le_bytes());
        central.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes()); // extra
        central.extend_from_slice(&0u16.to_le_bytes()); // comment
        central.extend_from_slice(&0u16.to_le_bytes()); // disk number
        central.extend_from_slice(&0u16.to_le_bytes()); // internal attrs
        central.extend_from_slice(&0u32.to_le_bytes()); // external attrs
        central.extend_from_slice(&local_offset.to_le_bytes());
        central.extend_from_slice(name_bytes);
    }
    let central_offset = out.len() as u32;
    let central_size = central.len() as u32;
    out.extend_from_slice(&central);
    // End of central directory.
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes()); // disk
    out.extend_from_slice(&0u16.to_le_bytes()); // cd disk
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&central_size.to_le_bytes());
    out.extend_from_slice(&central_offset.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes()); // comment len
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::TextInstance;
    use crate::subtitles::SubtitleCue;

    /// The fixed input most tests build with: a pinned
    /// stamp (2023-11-14 22:13:20 UTC) so every byte of
    /// the output is reproducible.
    fn sample_input() -> DiagnosticsInput {
        DiagnosticsInput {
            generated_at_unix: 1_700_000_000,
            ui_scale: None,
            language: Some("En".to_string()),
            language_tier: Some("full".to_string()),
            mcp_tool_count: Some(396),
            undo_depth: 3,
            redo_depth: 1,
            recent_status: Vec::new(),
            home: None,
        }
    }

    #[test]
    fn utc_stamps_match_known_vectors() {
        assert_eq!(utc_stamp(0), "1970-01-01 00:00:00 UTC");
        assert_eq!(utc_stamp(1_700_000_000), "2023-11-14 22:13:20 UTC");
        assert_eq!(file_stamp(0), "19700101-000000");
        assert_eq!(file_stamp(1_700_000_000), "20231114-221320");
        // A leap day: 2024-02-29 12:00:00 UTC.
        assert_eq!(utc_stamp(1_709_208_000), "2024-02-29 12:00:00 UTC");
    }

    #[test]
    fn census_counts_the_project_shape() {
        let project = Project::new_demo();
        let census = project_census(&project);
        assert_eq!(census.frames, project.frames.len());
        assert!(census.frames > 0);
        assert_eq!(
            census.poses_total,
            project.frames.iter().map(|f| f.poses.len()).sum::<usize>()
        );
        assert_eq!(census.figures, project.figures.len());
        assert_eq!(
            census.figure_nodes_total,
            project
                .figures
                .iter()
                .map(|f| f.def.nodes.len())
                .sum::<usize>()
        );
        assert_eq!(census.subtitle_cues, project.subtitles.len());
        assert_eq!(census.format_version, project.format_version);
    }

    #[test]
    fn privacy_contract_holds_for_a_loaded_project() {
        // A project stuffed with distinctive user
        // content in every content-bearing field the
        // census walks past: title, figure name, cue
        // text + speaker, text-instance content. None of
        // it may appear anywhere in the bundle — and
        // neither may the home path inside a status line.
        let dirs = crate::shell::store_dirs();
        let mut project = Project::new_demo();
        project.name = "ZQXSECRET project title".to_string();
        if let Some(figure) = project.figures.first_mut() {
            figure.name = "ZQXSECRET figure name".to_string();
        }
        project.subtitles.push(SubtitleCue {
            start_frame: 0,
            end_frame: 12,
            text: "ZQXSECRET cue dialogue".to_string(),
            speaker: Some("ZQXSECRET speaker".to_string()),
        });
        project.texts.push(TextInstance {
            content: "ZQXSECRET overlay words".to_string(),
            size: 24.0,
            color: 0xFFFF_FFFF,
            keys: Vec::new(),
        });
        let mut input = sample_input();
        input.home = Some(PathBuf::from("/home/testuser"));
        input.recent_status = vec![
            StatusFact {
                severity: "success".to_string(),
                text: "Exported 48-frame MP4 to /home/testuser/ZQXSECRET-movie.mp4"
                    .to_string(),
            },
            StatusFact {
                severity: "error".to_string(),
                text: "Autosave failed: permission denied".to_string(),
            },
        ];
        let bundle = build_bundle(&project, &dirs, &input);
        let zip_text = String::from_utf8_lossy(&bundle.zip_bytes()).into_owned();
        for payload in [
            &bundle.manifest_json,
            &bundle.census_json,
            &bundle.report_txt,
            &zip_text,
        ] {
            assert!(
                !payload.contains("ZQXSECRET"),
                "user content leaked into the bundle"
            );
            assert!(
                !payload.contains("/home/testuser"),
                "home path leaked into the bundle"
            );
        }
        // The status history is present, scrubbed: the
        // export line keeps its words and loses its path;
        // the path-free failure line survives verbatim.
        assert!(bundle
            .report_txt
            .contains("Exported 48-frame MP4 to <path>"));
        assert!(bundle
            .report_txt
            .contains("Autosave failed: permission denied"));
        // The manifest parses, carries the format + app
        // version, the sorted entry list, and counts that
        // match the project it was built from.
        let manifest: serde_json::Value =
            serde_json::from_str(&bundle.manifest_json).expect("manifest parses");
        assert_eq!(manifest["bundle_format"], 1u64);
        assert_eq!(manifest["app_version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(manifest["generated_at_unix"], 1_700_000_000u64);
        assert_eq!(
            manifest["entries"],
            serde_json::json!(["manifest.json", "project-census.json", "report.txt"])
        );
        assert_eq!(
            manifest["project"]["frames"],
            project.frames.len() as u64
        );
        assert_eq!(
            manifest["project"]["subtitle_cues"],
            project.subtitles.len() as u64
        );
        assert_eq!(
            manifest["project"]["cues_with_speaker"],
            project
                .subtitles
                .iter()
                .filter(|c| c.speaker.as_deref().is_some_and(|s| !s.is_empty()))
                .count() as u64
        );
        assert_eq!(manifest["project"]["texts"], project.texts.len() as u64);
        assert_eq!(manifest["mcp_tools"], 396u64);
        // The standalone census file agrees with the
        // manifest's embedded census.
        let census: serde_json::Value =
            serde_json::from_str(&bundle.census_json).expect("census parses");
        assert_eq!(census["frames"], manifest["project"]["frames"]);
        assert_eq!(census["figures"], manifest["project"]["figures"]);
        assert_eq!(census["subtitle_cues"], manifest["project"]["subtitle_cues"]);
    }

    #[test]
    fn bundle_build_is_byte_deterministic() {
        let dirs = crate::shell::store_dirs();
        let project = Project::new_demo();
        let input = sample_input();
        let a = build_bundle(&project, &dirs, &input);
        let b = build_bundle(&project, &dirs, &input);
        assert_eq!(a, b);
        assert_eq!(a.zip_bytes(), b.zip_bytes());
        assert_eq!(
            a.file_name(),
            "stykanimate-diagnostics-20231114-221320.zip"
        );
    }

    /// A minimal stored-ZIP reader for the round-trip
    /// test: walks the central directory, extracts each
    /// entry's bytes from its local header and verifies
    /// the stored CRC. Enough to prove the writer's
    /// output is a real ZIP, not to replace one.
    fn unzip_stored(bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
        let mut eocd = None;
        for i in (0..bytes.len().saturating_sub(3)).rev() {
            if bytes[i..i + 4] == [0x50, 0x4b, 0x05, 0x06] {
                eocd = Some(i);
                break;
            }
        }
        let eocd = eocd.expect("end of central directory");
        let count = u16::from_le_bytes([bytes[eocd + 10], bytes[eocd + 11]]) as usize;
        let cd_offset =
            u32::from_le_bytes(bytes[eocd + 16..eocd + 20].try_into().unwrap()) as usize;
        let mut out = Vec::new();
        let mut p = cd_offset;
        for _ in 0..count {
            assert_eq!(&bytes[p..p + 4], &[0x50, 0x4b, 0x01, 0x02]);
            let method = u16::from_le_bytes([bytes[p + 10], bytes[p + 11]]);
            assert_eq!(method, 0, "stored entries only");
            let crc = u32::from_le_bytes(bytes[p + 16..p + 20].try_into().unwrap());
            let size =
                u32::from_le_bytes(bytes[p + 20..p + 24].try_into().unwrap()) as usize;
            let name_len = u16::from_le_bytes([bytes[p + 28], bytes[p + 29]]) as usize;
            let extra_len = u16::from_le_bytes([bytes[p + 30], bytes[p + 31]]) as usize;
            let comment_len =
                u16::from_le_bytes([bytes[p + 32], bytes[p + 33]]) as usize;
            let local =
                u32::from_le_bytes(bytes[p + 42..p + 46].try_into().unwrap()) as usize;
            let name =
                String::from_utf8(bytes[p + 46..p + 46 + name_len].to_vec()).unwrap();
            assert_eq!(&bytes[local..local + 4], &[0x50, 0x4b, 0x03, 0x04]);
            let lname_len = u16::from_le_bytes([bytes[local + 26], bytes[local + 27]]);
            let lextra_len = u16::from_le_bytes([bytes[local + 28], bytes[local + 29]]);
            let data_start = local + 30 + lname_len as usize + lextra_len as usize;
            let data = bytes[data_start..data_start + size].to_vec();
            let mut check = flate2::Crc::new();
            check.update(&data);
            assert_eq!(check.sum(), crc, "CRC mismatch for {name}");
            out.push((name, data));
            p += 46 + name_len + extra_len + comment_len;
        }
        out
    }

    #[test]
    fn zip_round_trips_the_three_entries_sorted() {
        let dirs = crate::shell::store_dirs();
        let bundle = build_bundle(&Project::new_demo(), &dirs, &sample_input());
        let extracted = unzip_stored(&bundle.zip_bytes());
        let expected: Vec<(String, Vec<u8>)> = bundle
            .entries()
            .iter()
            .map(|(name, bytes)| (name.to_string(), bytes.to_vec()))
            .collect();
        assert_eq!(extracted, expected);
        let names: Vec<&str> = extracted.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(
            names,
            ["manifest.json", "project-census.json", "report.txt"]
        );
    }

    #[test]
    fn redaction_uses_roles_then_home_then_placeholder() {
        let dirs = crate::shell::store_dirs();
        // Store roles, in either layout (AppDirs or
        // portable): projects and the data root itself
        // have layout-stable role forms.
        assert_eq!(
            redact_path(&dirs.projects().join("whatever.json"), &dirs, None),
            "<data>/projects/whatever.json"
        );
        assert_eq!(redact_path(dirs.data_root(), &dirs, None), "<data>");
        assert_eq!(
            redact_path(&dirs.library3d().join("fig.json"), &dirs, None),
            "<data>/library3d/fig.json"
        );
        // The state role: its label differs between
        // layouts (portable folds state into projects),
        // so pin the contract's shape — a role in angle
        // brackets plus the relative remainder, never the
        // raw root.
        let state_form = redact_path(&dirs.state().join(".recovery.json"), &dirs, None);
        assert!(state_form.starts_with('<'), "state path not role-redacted: {state_form}");
        assert!(state_form.ends_with("/.recovery.json"));
        let raw_state = dirs.state().to_string_lossy().into_owned();
        assert!(!state_form.contains(raw_state.as_str()));
        // Home prefix becomes ~/…; other absolute paths
        // collapse; relative paths pass through.
        let home = PathBuf::from("/home/testuser");
        assert_eq!(
            redact_path(Path::new("/home/testuser/notes/x.txt"), &dirs, Some(&home)),
            "~/notes/x.txt"
        );
        assert_eq!(
            redact_path(Path::new("/etc/hostname"), &dirs, Some(&home)),
            "<path>"
        );
        assert_eq!(
            redact_path(Path::new("relative/file.json"), &dirs, None),
            "relative/file.json"
        );
    }

    #[test]
    fn scrub_replaces_path_tokens_and_keeps_words() {
        assert_eq!(
            scrub_status_text("Exported 48-frame MP4 to /home/testuser/x.mp4", None),
            "Exported 48-frame MP4 to <path>"
        );
        assert_eq!(
            scrub_status_text("Saved \"C:\\Users\\test\\a.json\" ok", None),
            "Saved <path> ok"
        );
        assert_eq!(
            scrub_status_text("Autosave failed: permission denied", None),
            "Autosave failed: permission denied"
        );
        // A home path glued inside a larger token still
        // marks the whole token as a path.
        let home = PathBuf::from("/home/testuser");
        assert_eq!(
            scrub_status_text("see prefix/home/testuser/x now", Some(&home)),
            "see <path> now"
        );
        // The cap: 200 chars plus the ellipsis mark.
        let capped = scrub_status_text(&"a".repeat(300), None);
        assert_eq!(capped.chars().count(), STATUS_TEXT_MAX_CHARS + 1);
        assert!(capped.ends_with('…'));
    }

    #[test]
    fn write_bundle_writes_a_readable_zip() {
        let dirs = crate::shell::store_dirs();
        let bundle = build_bundle(&Project::new_demo(), &dirs, &sample_input());
        let dir = std::env::temp_dir().join(format!(
            "stykanimate-diagnostics-test-{}",
            std::process::id()
        ));
        let path = write_bundle(&dir, &bundle).expect("bundle writes");
        assert_eq!(
            path.file_name().unwrap().to_string_lossy(),
            "stykanimate-diagnostics-20231114-221320.zip"
        );
        let bytes = std::fs::read(&path).expect("bundle reads back");
        assert_eq!(bytes, bundle.zip_bytes());
        assert_eq!(unzip_stored(&bytes).len(), 3);
        assert_eq!(
            diagnostics_dir(&dirs),
            dirs.data_root().join("diagnostics")
        );
        std::fs::remove_dir_all(&dir).ok();
    }
}
