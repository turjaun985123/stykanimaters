//! Native file chooser dialogs, without a GUI toolkit dependency.
//!
//! Linux desktops ship two scriptable file choosers: `zenity`
//! (GNOME/GTK) and `kdialog` (KDE). Both are external programs that
//! print the chosen path on stdout and exit non-zero when the user
//! cancels, so this module needs nothing but `std::process` — no
//! new crates, no linking against GTK/Qt.
//!
//! # Fallback contract
//!
//! Headless machines, minimal VMs and containers usually have
//! NEITHER tool installed (this project's build VM is one). In
//! that case [`available_tool`] returns `None` and [`open_file`] /
//! [`save_file`] return `None` immediately, without spawning
//! anything. Callers must therefore treat a `None` from
//! `open_file` / `save_file` as "no path was chosen" — that covers
//! both *user cancelled* and *no dialog tool available* — and fall
//! back to the existing folder-based flow (the `import/` folder and
//! the projects directory). Callers that need to distinguish the
//! two cases can check [`available_tool`] first: `None` there
//! means dialogs are unavailable on this machine at all.
//!
//! # Structure
//!
//! The argv construction ([`build_open_args`], [`build_save_args`])
//! and the output parsing ([`parse_choice`]) are pure functions,
//! so the exact command lines for both tools are fully testable on
//! machines where neither tool exists. Only the thin spawn layer
//! ([`open_file`], [`save_file`], [`available_tool`]) touches the
//! system.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// A desktop file-chooser program this module knows how to drive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DialogTool {
    /// GNOME/GTK chooser (`zenity`).
    Zenity,
    /// KDE chooser (`kdialog`).
    KDialog,
}

impl DialogTool {
    /// The program name to spawn for this tool.
    pub fn program(self) -> &'static str {
        match self {
            DialogTool::Zenity => "zenity",
            DialogTool::KDialog => "kdialog",
        }
    }
}

/// One named file-type filter for the chooser, e.g. label
/// `"Pivot"` with patterns `["*.piv", "*.stk"]`. Patterns are
/// shell-style globs, the syntax both tools expect.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileFilter {
    pub label: String,
    pub patterns: Vec<String>,
}

impl FileFilter {
    pub fn new(label: &str, patterns: &[&str]) -> Self {
        FileFilter {
            label: label.to_string(),
            patterns: patterns.iter().map(|p| p.to_string()).collect(),
        }
    }
}

/// Which tool to try first for a given `XDG_CURRENT_DESKTOP`
/// value: KDE desktops get kdialog first (it matches the running
/// desktop's look), everything else gets zenity first. Pure, so
/// the preference order is testable without a desktop.
fn preferred_order(desktop: Option<&str>) -> [DialogTool; 2] {
    match desktop {
        Some(d) if d.split(':').any(|part| part.to_ascii_lowercase().contains("kde")) => {
            [DialogTool::KDialog, DialogTool::Zenity]
        }
        _ => [DialogTool::Zenity, DialogTool::KDialog],
    }
}

/// True when `program` spawns and answers `--version` with a
/// success exit code. Both zenity and kdialog handle `--version`
/// before opening any window, so this probe never shows UI; a
/// spawn error (tool absent) or a failed run counts as missing.
fn tool_runs(program: &str) -> bool {
    Command::new(program)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// The dialog tool to use on this machine, or `None` when neither
/// zenity nor kdialog is installed (headless boxes, minimal VMs).
/// Probes by running each candidate with `--version`; nothing is
/// cached, every call re-checks, so a tool installed mid-session
/// is picked up.
pub fn available_tool() -> Option<DialogTool> {
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").ok();
    preferred_order(desktop.as_deref())
        .into_iter()
        .find(|tool| tool_runs(tool.program()))
}

/// zenity's `--file-filter` value for one filter:
/// `Label | *.piv *.stk`.
fn zenity_filter(filter: &FileFilter) -> String {
    format!("{} | {}", filter.label, filter.patterns.join(" "))
}

/// kdialog's filter argument: one `patterns|Label` line per
/// filter, newline-separated.
fn kdialog_filter(filters: &[FileFilter]) -> String {
    filters
        .iter()
        .filter(|f| !f.patterns.is_empty())
        .map(|f| format!("{}|{}", f.patterns.join(" "), f.label))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Filters with no patterns are dropped by both builders: an
/// empty pattern list is not a valid filter for either tool.
fn usable(filters: &[FileFilter]) -> Vec<&FileFilter> {
    filters.iter().filter(|f| !f.patterns.is_empty()).collect()
}

/// zenity's `--filename=` value for an open dialog: the start
/// directory with a trailing slash, which is how zenity knows to
/// open *in* that directory rather than preselect a file.
fn zenity_start_dir(dir: &Path) -> String {
    let s = dir.to_string_lossy();
    if s.ends_with('/') {
        s.into_owned()
    } else {
        format!("{s}/")
    }
}

/// Build the exact argv (program name first) for an open dialog.
///
/// zenity: `zenity --file-selection --title=T [--filename=DIR/]
/// [--file-filter=Label | pats]...`
///
/// kdialog: `kdialog --title T --getopenfilename START [FILTER]`
/// where START is the start directory (or `.` when none was
/// given — kdialog needs the positional slot filled before the
/// filter) and FILTER is the newline-joined `pats|Label` string,
/// omitted entirely when there are no usable filters.
pub fn build_open_args(
    tool: DialogTool,
    title: &str,
    filters: &[FileFilter],
    start_dir: Option<&Path>,
) -> Vec<String> {
    let filters = usable(filters);
    match tool {
        DialogTool::Zenity => {
            let mut args = vec![
                tool.program().to_string(),
                "--file-selection".to_string(),
                format!("--title={title}"),
            ];
            if let Some(dir) = start_dir {
                args.push(format!("--filename={}", zenity_start_dir(dir)));
            }
            for f in filters {
                args.push(format!("--file-filter={}", zenity_filter(f)));
            }
            args
        }
        DialogTool::KDialog => {
            let mut args = vec![
                tool.program().to_string(),
                "--title".to_string(),
                title.to_string(),
                "--getopenfilename".to_string(),
                start_dir
                    .map(|d| d.to_string_lossy().into_owned())
                    .unwrap_or_else(|| ".".to_string()),
            ];
            let filter_refs: Vec<FileFilter> =
                filters.iter().map(|f| (*f).clone()).collect();
            if !filter_refs.is_empty() {
                args.push(kdialog_filter(&filter_refs));
            }
            args
        }
    }
}

/// Build the exact argv (program name first) for a save dialog.
/// `default_name` is the suggested file name; with `start_dir` it
/// becomes the full suggested path.
///
/// zenity: `zenity --file-selection --save --confirm-overwrite
/// --title=T --filename=[DIR/]NAME [--file-filter=...]...`
///
/// kdialog: `kdialog --title T --getsavefilename [DIR/]NAME
/// [FILTER]` (kdialog asks about overwriting itself).
pub fn build_save_args(
    tool: DialogTool,
    title: &str,
    filters: &[FileFilter],
    start_dir: Option<&Path>,
    default_name: &str,
) -> Vec<String> {
    let filters = usable(filters);
    let suggested: PathBuf = match start_dir {
        Some(dir) => dir.join(default_name),
        None => PathBuf::from(default_name),
    };
    match tool {
        DialogTool::Zenity => {
            let mut args = vec![
                tool.program().to_string(),
                "--file-selection".to_string(),
                "--save".to_string(),
                "--confirm-overwrite".to_string(),
                format!("--title={title}"),
                format!("--filename={}", suggested.to_string_lossy()),
            ];
            for f in filters {
                args.push(format!("--file-filter={}", zenity_filter(f)));
            }
            args
        }
        DialogTool::KDialog => {
            let mut args = vec![
                tool.program().to_string(),
                "--title".to_string(),
                title.to_string(),
                "--getsavefilename".to_string(),
                suggested.to_string_lossy().into_owned(),
            ];
            let filter_refs: Vec<FileFilter> =
                filters.iter().map(|f| (*f).clone()).collect();
            if !filter_refs.is_empty() {
                args.push(kdialog_filter(&filter_refs));
            }
            args
        }
    }
}

/// Parse a chooser's stdout into the chosen path.
///
/// Both tools print exactly one line — the path — followed by a
/// newline; empty or whitespace-only output (and any output at
/// all when the run failed, which the spawn layer filters out
/// first) means "no choice" and yields `None`.
///
/// kdialog usually prints a plain path, but in some modes it
/// prints a `file://` URL instead; that form is converted back to
/// a plain path, including percent-decoding (`%20` etc.) and
/// dropping a `localhost` host part. Plain paths are returned
/// untouched, so a literal `%` in a real file name survives.
pub fn parse_choice(tool: DialogTool, stdout: &str) -> Option<PathBuf> {
    let text = stdout.trim();
    if text.is_empty() {
        return None;
    }
    // One path per invocation; take the first line defensively.
    let line = text.lines().next().unwrap_or("");
    if line.is_empty() {
        return None;
    }
    if tool == DialogTool::KDialog {
        if let Some(rest) = line.strip_prefix("file://") {
            let path = rest.strip_prefix("localhost").unwrap_or(rest);
            if !path.starts_with('/') {
                return None;
            }
            return Some(PathBuf::from(percent_decode(path)));
        }
    }
    Some(PathBuf::from(line))
}

/// Decode `%XX` escapes in a `file://` URL path. Invalid or
/// truncated escapes are left as literal text.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = |b: u8| -> Option<u8> {
                match b {
                    b'0'..=b'9' => Some(b - b'0'),
                    b'a'..=b'f' => Some(b - b'a' + 10),
                    b'A'..=b'F' => Some(b - b'A' + 10),
                    _ => None,
                }
            };
            if let (Some(hi), Some(lo)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                out.push(hi * 16 + lo);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Spawn a built argv and interpret the result: non-zero exit
/// (cancel), a spawn failure, or empty output all become `None`.
fn run_dialog(tool: DialogTool, args: &[String]) -> Option<PathBuf> {
    let (program, rest) = args.split_first()?;
    let output = Command::new(program)
        .args(rest)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    parse_choice(tool, &String::from_utf8_lossy(&output.stdout))
}

/// Show a native open dialog and return the chosen path.
///
/// Returns `None` when no dialog tool is installed (use the
/// folder-based fallback — see the module docs), when the user
/// cancels, or when the dialog fails for any other reason.
pub fn open_file(title: &str, filters: &[FileFilter]) -> Option<PathBuf> {
    let tool = available_tool()?;
    let args = build_open_args(tool, title, filters, None);
    run_dialog(tool, &args)
}

/// Show a native save dialog (starting from `default_name` as
/// the suggested file name) and return the chosen path. Same
/// `None` contract as [`open_file`].
pub fn save_file(title: &str, filters: &[FileFilter], default_name: &str) -> Option<PathBuf> {
    let tool = available_tool()?;
    let args = build_save_args(tool, title, filters, None, default_name);
    run_dialog(tool, &args)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pivot_filter() -> FileFilter {
        FileFilter::new("Pivot", &["*.piv", "*.stk"])
    }

    fn argv(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|s| s.to_string()).collect()
    }

    // ---- zenity argv ----

    #[test]
    fn zenity_open_bare() {
        let args = build_open_args(DialogTool::Zenity, "Open Project", &[], None);
        assert_eq!(
            args,
            argv(&["zenity", "--file-selection", "--title=Open Project"])
        );
    }

    #[test]
    fn zenity_open_one_filter() {
        let args = build_open_args(
            DialogTool::Zenity,
            "Import",
            &[pivot_filter()],
            None,
        );
        assert_eq!(
            args,
            argv(&[
                "zenity",
                "--file-selection",
                "--title=Import",
                "--file-filter=Pivot | *.piv *.stk",
            ])
        );
    }

    #[test]
    fn zenity_open_multiple_filters_keep_order() {
        let filters = [
            pivot_filter(),
            FileFilter::new("Stick Nodes", &["*.nodes", "*.nodemc"]),
            FileFilter::new("All files", &["*"]),
        ];
        let args = build_open_args(DialogTool::Zenity, "Import", &filters, None);
        assert_eq!(
            args,
            argv(&[
                "zenity",
                "--file-selection",
                "--title=Import",
                "--file-filter=Pivot | *.piv *.stk",
                "--file-filter=Stick Nodes | *.nodes *.nodemc",
                "--file-filter=All files | *",
            ])
        );
    }

    #[test]
    fn zenity_open_start_dir_gets_trailing_slash() {
        let args = build_open_args(
            DialogTool::Zenity,
            "Import",
            &[pivot_filter()],
            Some(Path::new("/home/u/anims")),
        );
        assert_eq!(
            args,
            argv(&[
                "zenity",
                "--file-selection",
                "--title=Import",
                "--filename=/home/u/anims/",
                "--file-filter=Pivot | *.piv *.stk",
            ])
        );
        // A start dir that already ends in '/' is left alone.
        let args = build_open_args(
            DialogTool::Zenity,
            "Import",
            &[],
            Some(Path::new("/home/u/anims/")),
        );
        assert!(args.contains(&"--filename=/home/u/anims/".to_string()));
    }

    #[test]
    fn zenity_save_with_dir_and_name() {
        let args = build_save_args(
            DialogTool::Zenity,
            "Export",
            &[pivot_filter()],
            Some(Path::new("/tmp/out")),
            "anim.piv",
        );
        assert_eq!(
            args,
            argv(&[
                "zenity",
                "--file-selection",
                "--save",
                "--confirm-overwrite",
                "--title=Export",
                "--filename=/tmp/out/anim.piv",
                "--file-filter=Pivot | *.piv *.stk",
            ])
        );
    }

    #[test]
    fn zenity_save_without_dir_uses_bare_name() {
        let args = build_save_args(DialogTool::Zenity, "Export", &[], None, "anim.piv");
        assert_eq!(
            args,
            argv(&[
                "zenity",
                "--file-selection",
                "--save",
                "--confirm-overwrite",
                "--title=Export",
                "--filename=anim.piv",
            ])
        );
    }

    // ---- kdialog argv ----

    #[test]
    fn kdialog_open_with_filter_and_start() {
        let args = build_open_args(
            DialogTool::KDialog,
            "Import",
            &[pivot_filter()],
            Some(Path::new("/home/u/anims")),
        );
        assert_eq!(
            args,
            argv(&[
                "kdialog",
                "--title",
                "Import",
                "--getopenfilename",
                "/home/u/anims",
                "*.piv *.stk|Pivot",
            ])
        );
    }

    #[test]
    fn kdialog_open_no_start_uses_dot_and_joins_filters_with_newlines() {
        let filters = [
            pivot_filter(),
            FileFilter::new("Stick Nodes", &["*.nodes"]),
        ];
        let args = build_open_args(DialogTool::KDialog, "Import", &filters, None);
        assert_eq!(
            args,
            argv(&[
                "kdialog",
                "--title",
                "Import",
                "--getopenfilename",
                ".",
                "*.piv *.stk|Pivot\n*.nodes|Stick Nodes",
            ])
        );
    }

    #[test]
    fn kdialog_open_without_filters_omits_filter_arg() {
        let args = build_open_args(DialogTool::KDialog, "Open", &[], None);
        assert_eq!(
            args,
            argv(&["kdialog", "--title", "Open", "--getopenfilename", "."])
        );
    }

    #[test]
    fn kdialog_save_combines_dir_and_name() {
        let args = build_save_args(
            DialogTool::KDialog,
            "Export",
            &[pivot_filter()],
            Some(Path::new("/tmp/out")),
            "anim.piv",
        );
        assert_eq!(
            args,
            argv(&[
                "kdialog",
                "--title",
                "Export",
                "--getsavefilename",
                "/tmp/out/anim.piv",
                "*.piv *.stk|Pivot",
            ])
        );
        let args = build_save_args(DialogTool::KDialog, "Export", &[], None, "anim.piv");
        assert_eq!(
            args,
            argv(&[
                "kdialog",
                "--title",
                "Export",
                "--getsavefilename",
                "anim.piv",
            ])
        );
    }

    // ---- filter edge cases ----

    #[test]
    fn empty_pattern_filters_are_dropped_everywhere() {
        let filters = [FileFilter::new("Broken", &[]), pivot_filter()];
        let z = build_open_args(DialogTool::Zenity, "T", &filters, None);
        assert_eq!(z.len(), 4, "only the usable filter survives: {z:?}");
        assert!(z.iter().all(|a| !a.contains("Broken")));
        let k = build_open_args(DialogTool::KDialog, "T", &filters, None);
        assert_eq!(k.last().map(String::as_str), Some("*.piv *.stk|Pivot"));
        // And when every filter is broken, kdialog omits the arg.
        let k = build_open_args(
            DialogTool::KDialog,
            "T",
            &[FileFilter::new("Broken", &[])],
            None,
        );
        assert_eq!(k.len(), 5);
    }

    #[test]
    fn file_filter_constructor() {
        let f = FileFilter::new("Images", &["*.png", "*.jpg"]);
        assert_eq!(f.label, "Images");
        assert_eq!(f.patterns, vec!["*.png".to_string(), "*.jpg".to_string()]);
    }

    // ---- parse_choice ----

    #[test]
    fn parse_plain_paths() {
        for tool in [DialogTool::Zenity, DialogTool::KDialog] {
            assert_eq!(
                parse_choice(tool, "/home/u/anim.piv\n"),
                Some(PathBuf::from("/home/u/anim.piv"))
            );
            // Surrounding whitespace and CRLF are trimmed.
            assert_eq!(
                parse_choice(tool, "  /tmp/a b.stk\r\n"),
                Some(PathBuf::from("/tmp/a b.stk"))
            );
        }
    }

    #[test]
    fn parse_empty_means_no_choice() {
        for tool in [DialogTool::Zenity, DialogTool::KDialog] {
            assert_eq!(parse_choice(tool, ""), None);
            assert_eq!(parse_choice(tool, "\n"), None);
            assert_eq!(parse_choice(tool, "   \n"), None);
        }
    }

    #[test]
    fn parse_kdialog_file_url() {
        assert_eq!(
            parse_choice(DialogTool::KDialog, "file:///home/u/my%20anim.piv\n"),
            Some(PathBuf::from("/home/u/my anim.piv"))
        );
        assert_eq!(
            parse_choice(DialogTool::KDialog, "file://localhost/tmp/x.stk\n"),
            Some(PathBuf::from("/tmp/x.stk"))
        );
        // zenity never emits URLs: a file:// string stays a literal
        // (nonexistent) path rather than being rewritten.
        assert_eq!(
            parse_choice(DialogTool::Zenity, "file:///tmp/x\n"),
            Some(PathBuf::from("file:///tmp/x"))
        );
        // A literal '%' in a plain path is NOT percent-decoded.
        assert_eq!(
            parse_choice(DialogTool::KDialog, "/tmp/100%20legit.piv\n"),
            Some(PathBuf::from("/tmp/100%20legit.piv"))
        );
    }

    #[test]
    fn percent_decode_edges() {
        assert_eq!(percent_decode("/a%20b/%41"), "/a b/A");
        assert_eq!(percent_decode("/a%zz"), "/a%zz", "bad escape stays literal");
        assert_eq!(percent_decode("/a%2"), "/a%2", "truncated escape stays literal");
        assert_eq!(percent_decode(""), "");
    }

    // ---- probing ----

    #[test]
    fn preferred_order_follows_desktop() {
        assert_eq!(
            preferred_order(Some("KDE")),
            [DialogTool::KDialog, DialogTool::Zenity]
        );
        assert_eq!(
            preferred_order(Some("kde-plasma")),
            [DialogTool::KDialog, DialogTool::Zenity]
        );
        assert_eq!(
            preferred_order(Some("GNOME")),
            [DialogTool::Zenity, DialogTool::KDialog]
        );
        assert_eq!(
            preferred_order(None),
            [DialogTool::Zenity, DialogTool::KDialog]
        );
        // Colon-separated session lists are honoured too.
        assert_eq!(
            preferred_order(Some("ubuntu:KDE")),
            [DialogTool::KDialog, DialogTool::Zenity]
        );
    }

    #[test]
    fn available_tool_is_consistent_and_honest() {
        // The build VM has neither tool, a desktop may have one:
        // either answer is acceptable, but repeated calls within
        // one environment must agree, and a reported tool must
        // have a real program name that actually runs.
        let first = available_tool();
        assert_eq!(first, available_tool());
        if let Some(tool) = first {
            assert!(tool_runs(tool.program()));
            assert!(matches!(tool.program(), "zenity" | "kdialog"));
        }
    }

    #[test]
    fn program_names() {
        assert_eq!(DialogTool::Zenity.program(), "zenity");
        assert_eq!(DialogTool::KDialog.program(), "kdialog");
    }
}
