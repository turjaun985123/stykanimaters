//! The shortcut poster (Roadmap Wave 2, W2-97 — E-9):
//! a printable, shareable HTML page listing every
//! keyboard binding the shell owns, generated from the
//! binding registry ([`keymap::BINDINGS`]) and from
//! nothing else.
//!
//! Why the registry is the only source: a poster drawn
//! from any second table would drift the day a binding
//! changes — the registry is the one table the help
//! rows, the menu hints and the palette chips all read,
//! so a poster fed by the same table cannot disagree
//! with the app it describes. The converse also holds:
//! a binding that exists only as an `on_key` arm in
//! `main.rs`, with no registry row, is a REGISTRY GAP —
//! it is reported in the seat's findings, never invented
//! as poster content here.
//!
//! String provenance, exactly: the key spellings are
//! `Binding::keys`, the action prose is `Binding::action`
//! (the help panel's canonical English — the same
//! strings `help_panel::help_text_key` maps to catalogue
//! keys), the section headings are `Binding::section`
//! (the same names `help_panel::group_rows` prints), and
//! the "Palette only" tag derives from `Binding::scope`.
//! The only strings this module owns are the poster
//! chrome literals in [`poster_html`] (title, intro,
//! footer) — presentation furniture for a standalone
//! file, listed in the findings so no second naming
//! source hides here.
//!
//! Determinism contract: [`poster_html`] is a pure
//! function of its slice — sections in first-appearance
//! (registry) order, rows in registry order, no clock,
//! no environment, no timestamps anywhere in the
//! artefact. Same registry in, same bytes out; the
//! determinism and planted-change pins below hold that.
//!
//! Known gap, stated rather than papered over: the
//! registry holds eight Workspaces bindings (Alt+1…8).
//! The ninth workspace, Build, has no keymap binding and
//! no `go-build` palette command (W2-58's flag), so the
//! poster omits Build's switching key — because the
//! registry does. The fix is the integrator's keymap
//! IA, not a poster edit.
//!
//! The poster is English, like the registry prose it
//! renders; the panel button that writes it localises
//! through the catalogue (IA-W2-97-1), the file content
//! does not — a translated poster would need the
//! registry's prose in the catalogue per binding, which
//! is a different programme.
//!
//! Registration is IA-W2-97-2 (must-land-with):
//! `pub mod shortcut_poster;` in `src/rune_sec/mod.rs`.

use std::path::PathBuf;

use crate::rune_sec::keymap::{Binding, BindingScope};

/// The file name the poster is written under, inside
/// the shell's `./export` directory — the destination
/// every panel export uses (`export_panel::export_dir`'s
/// idiom, mirrored: current directory + `export`).
pub const POSTER_FILE_NAME: &str = "shortcut-poster.html";

/// Escape the five HTML-significant characters. Binding
/// prose is app-owned data, but the poster is a file
/// meant to be shared and opened anywhere, so registry
/// text is escaped on principle, not on suspicion.
fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// The poster's stylesheet: one embedded `<style>`
/// block, system font stack, key-cap chips, and a print
/// stylesheet (`@page` margins, black-on-white, sections
/// kept whole where they fit). No external assets — the
/// file is self-contained by construction.
const POSTER_CSS: &str = "\
:root { color-scheme: light; }
* { box-sizing: border-box; }
body { margin: 2rem auto; max-width: 52rem; padding: 0 1rem;
  font-family: system-ui, \"Segoe UI\", Roboto, Helvetica, Arial, sans-serif;
  color: #1a1d21; background: #ffffff; line-height: 1.45; }
h1 { font-size: 1.6rem; margin: 0 0 0.25rem; }
.sub { color: #4a4f55; margin: 0 0 1.5rem; }
section { margin: 0 0 1.5rem; }
h2 { font-size: 1.1rem; margin: 0 0 0.5rem; padding-bottom: 0.25rem;
  border-bottom: 2px solid #1a1d21; }
table { border-collapse: collapse; width: 100%; }
td { padding: 0.3rem 0.5rem 0.3rem 0; vertical-align: top; }
td.keys { white-space: nowrap; width: 11rem; }
kbd { font-family: ui-monospace, \"Cascadia Mono\", Consolas, monospace;
  font-size: 0.85em; background: #f0f1f3; border: 1px solid #c6c9cd;
  border-bottom-width: 2px; border-radius: 0.3rem; padding: 0.1rem 0.4rem; }
.scope { font-size: 0.8em; color: #4a4f55; border: 1px solid #c6c9cd;
  border-radius: 0.6rem; padding: 0.05rem 0.45rem; margin-left: 0.5rem;
  white-space: nowrap; }
footer { color: #4a4f55; font-size: 0.85rem; border-top: 1px solid #c6c9cd;
  padding-top: 0.5rem; }
@page { size: A4; margin: 14mm; }
@media print {
  body { margin: 0; max-width: none; }
  section { break-inside: avoid; }
  kbd { background: #ffffff; }
}
";

/// Render the poster for a binding table. Production
/// passes [`keymap::BINDINGS`]; tests pass fixtures —
/// the planted-change pin depends on this function
/// reading ONLY its argument, never the static.
pub fn poster_html(bindings: &[Binding]) -> String {
    // Sections in first-appearance order: for the real
    // registry that is the help panel's own order
    // (Shortcuts, Workspaces, Posing, Timeline, Drawing).
    let mut sections: Vec<&str> = Vec::new();
    for b in bindings {
        if !sections.contains(&b.section) {
            sections.push(b.section);
        }
    }
    let mut h = String::new();
    h.push_str("<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n");
    h.push_str("<meta charset=\"utf-8\">\n");
    h.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
    h.push_str("<title>StykAnimate — Keyboard Shortcuts</title>\n<style>\n");
    h.push_str(POSTER_CSS);
    h.push_str("</style>\n</head>\n<body>\n");
    h.push_str("<header>\n<h1>StykAnimate — Keyboard Shortcuts</h1>\n");
    h.push_str(
        "<p class=\"sub\">Every keyboard binding in the editor, \
         generated from the same binding registry the Help panel, \
         the menus and the command palette read.</p>\n</header>\n",
    );
    for section in sections {
        h.push_str("<section>\n<h2>");
        h.push_str(&escape_html(section));
        h.push_str("</h2>\n<table>\n<tbody>\n");
        for b in bindings.iter().filter(|b| b.section == section) {
            h.push_str("<tr><td class=\"keys\"><kbd>");
            h.push_str(&escape_html(b.keys));
            h.push_str("</kbd></td><td>");
            h.push_str(&escape_html(b.action));
            if b.scope == BindingScope::Palette {
                h.push_str("<span class=\"scope\">Palette only</span>");
            }
            h.push_str("</td></tr>\n");
        }
        h.push_str("</tbody>\n</table>\n</section>\n");
    }
    h.push_str(
        "<footer>Sections and rows follow the Help panel's order. \
         Regenerate from the Help panel's “Export shortcut poster” \
         button — the poster is rebuilt from the registry on every \
         press, so it cannot go stale.</footer>\n",
    );
    h.push_str("</body>\n</html>\n");
    h
}

/// The `./export` directory under the working
/// directory — the destination the `EditorCore` media
/// ops build their paths from, mirrored from
/// `export_panel::export_dir`.
fn poster_dir() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("export")
}

/// The path the poster button writes to.
pub fn poster_path() -> PathBuf {
    poster_dir().join(POSTER_FILE_NAME)
}

/// Write the poster for the real registry, creating
/// `./export` when needed. Errors name their reason
/// (the io error's own text) — the caller reports it
/// verbatim on the status line.
pub fn write_poster() -> Result<PathBuf, String> {
    let dir = poster_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(POSTER_FILE_NAME);
    std::fs::write(&path, poster_html(crate::rune_sec::keymap::BINDINGS))
        .map_err(|e| e.to_string())?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rune_sec::keymap;

    /// A planted binding no real registry will ever
    /// hold — the fixture the change pin is built from.
    fn planted() -> Binding {
        Binding {
            id: "w2-97-planted",
            keys: "Ctrl+Alt+Q",
            action: "Planted test action (W2-97 fixture)",
            section: "Shortcuts",
            scope: BindingScope::Global,
            menu_key: None,
            command_key: None,
        }
    }

    #[test]
    fn poster_lists_every_registry_binding() {
        let html = poster_html(keymap::BINDINGS);
        // One key-cap per binding — no row dropped, none
        // invented.
        assert_eq!(
            html.matches("<kbd>").count(),
            keymap::BINDINGS.len(),
            "poster row count must equal the registry size"
        );
        for b in keymap::BINDINGS {
            let cap = format!("<kbd>{}</kbd>", escape_html(b.keys));
            assert!(html.contains(&cap), "poster missing key cap for {}", b.id);
            assert!(
                html.contains(&escape_html(b.action)),
                "poster missing action prose for {}",
                b.id
            );
        }
    }

    #[test]
    fn planted_registry_change_changes_the_poster() {
        let real = poster_html(keymap::BINDINGS);
        assert!(!real.contains("Planted test action"));
        // Addition: a registry fixture carrying one extra
        // binding yields a different poster that names it.
        let mut fixture: Vec<Binding> = keymap::BINDINGS.to_vec();
        fixture.push(planted());
        let grown = poster_html(&fixture);
        assert_ne!(grown, real);
        assert!(grown.contains("Planted test action (W2-97 fixture)"));
        assert!(grown.contains("<kbd>Ctrl+Alt+Q</kbd>"));
        // Removal: dropping the last binding changes the
        // bytes too (the poster is not a fixed template).
        let shrunk = poster_html(&keymap::BINDINGS[..keymap::BINDINGS.len() - 1]);
        assert_ne!(shrunk, real);
    }

    #[test]
    fn generation_is_byte_deterministic() {
        // Same registry in, same bytes out — twice is the
        // whole pin: the generator reads no clock, no
        // environment and no filesystem.
        assert_eq!(poster_html(keymap::BINDINGS), poster_html(keymap::BINDINGS));
        let fixture = vec![planted()];
        assert_eq!(poster_html(&fixture), poster_html(&fixture));
    }

    #[test]
    fn sections_follow_registry_order() {
        let html = poster_html(keymap::BINDINGS);
        let order = ["Shortcuts", "Workspaces", "Posing", "Timeline", "Drawing"];
        let mut last = 0;
        for name in order {
            let at = html
                .find(&format!("<h2>{name}</h2>"))
                .unwrap_or_else(|| panic!("poster missing section {name}"));
            assert!(at > last, "section {name} out of registry order");
            last = at;
        }
        // The Features section holds no bindings (the
        // help panel's own module docs say so), so it is
        // not a poster section.
        assert!(!html.contains("<h2>Features</h2>"));
    }

    #[test]
    fn palette_scope_bindings_are_tagged() {
        let html = poster_html(keymap::BINDINGS);
        let palette_count = keymap::BINDINGS
            .iter()
            .filter(|b| b.scope == BindingScope::Palette)
            .count();
        assert_eq!(
            html.matches("Palette only").count(),
            palette_count,
            "every palette-scoped binding carries the tag, and no other row does"
        );
    }

    #[test]
    fn registry_text_is_html_escaped() {
        let hostile = Binding {
            action: "Fish & <Chips> \"quoted\"",
            ..planted()
        };
        let html = poster_html(&[hostile]);
        assert!(html.contains("Fish &amp; &lt;Chips&gt; &quot;quoted&quot;"));
        assert!(!html.contains("Fish & <Chips>"));
    }

    #[test]
    fn poster_is_a_self_contained_document() {
        let html = poster_html(keymap::BINDINGS);
        assert!(html.starts_with("<!DOCTYPE html>"));
        assert!(html.trim_end().ends_with("</html>"));
        // No external assets: nothing fetched, ever.
        assert!(!html.contains("http://"));
        assert!(!html.contains("https://"));
        assert!(!html.contains("<link"));
        assert!(!html.contains("<script"));
        // The print stylesheet rides along.
        assert!(html.contains("@media print"));
        assert!(html.contains("@page"));
    }
}
