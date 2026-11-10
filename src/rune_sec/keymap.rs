//! The binding registry (Roadmap Wave 1, RM26 — PL-5's
//! K11): every keyboard binding the shell owns, as data.
//!
//! Before this module, the same facts were described three
//! times by hand — the help panel's shortcut rows
//! (`help_panel::section_rows`), the menu shortcut hints
//! (`menus::shortcut_hint`) and the command palette's kbd
//! chips (`command_palette::command_hint`) — and held in
//! sync by drift-guard tests. All three surfaces now READ
//! this table, so a binding's spelling cannot drift between
//! them: there is one spelling, here. The drift guards stay
//! as tests, but they now verify structure (every hinted
//! command exists, every spelling is a help label) instead
//! of policing copies.
//!
//! What this table is NOT: the dispatch. `main.rs::on_key`
//! still matches keys imperatively (it owns the routing
//! tiers — palette claim, Escape scopes, the text-field
//! guard — that a flat table cannot express). The PL-4
//! chord clusters (RM26's K1–K4) were added to this table
//! and to all three surfaces in this wave; their `on_key`
//! arms land with IA-RM26-1 at integration, the wave's
//! land-together pattern (the Q7 precedent). Until that IA
//! is applied, the new rows describe bindings one push
//! away — the same interim state RM15's nudge op shipped
//! in, recorded here rather than hidden.
//!
//! Context scoping (A21's K5): no per-binding scope ruling
//! has been taken (§8 secondary decisions), so every
//! binding below is global in exactly the way `on_key`
//! already routes it — the table records the two scopes
//! that EXIST (global, and the palette's modal keys), and
//! invents none.
//!
//! Spelling conventions, inherited from the surfaces this
//! replaces: the platform-primary modifier is spelled
//! "Ctrl" on every platform (A21 §1.7 records the stance),
//! and plain-letter bindings name the typed character, so
//! they follow the layout's produced character rather than
//! the physical key.

/// Where a binding is live. `Global` bindings route
/// through `main.rs::on_key` under its standing guards
/// (they pause while a text field is focused, except
/// Ctrl+K). `Palette` bindings exist only while the
/// command palette is open and route through its own key
/// handlers.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BindingScope {
    Global,
    Palette,
}

/// One keyboard binding. `keys` is the display spelling
/// the help panel prints and the hint chips carry — one
/// spelling for every surface. `action` is the help
/// panel's canonical English action prose (the renderer
/// localises it through `help_panel::help_text_key`, so
/// this string must stay byte-identical to the mapped
/// string there). `section` names the help section the
/// binding's row lives in. `menu_key` / `command_key` name
/// the SAME action in the menu items' and the palette
/// commands' key namespaces when such a twin exists —
/// `None` is a positive statement (no twin, or the twin's
/// action is not exactly this binding's: the menu Delete
/// items stay hint-less because the key runs a cascade,
/// and the app-bar Load button stays hint-less because
/// Ctrl+O opens the selected project file while that
/// button quick-loads the working file).
#[derive(Clone, Copy)]
pub struct Binding {
    pub id: &'static str,
    pub keys: &'static str,
    pub action: &'static str,
    pub section: &'static str,
    pub scope: BindingScope,
    pub menu_key: Option<&'static str>,
    pub command_key: Option<&'static str>,
}

const G: BindingScope = BindingScope::Global;
const P: BindingScope = BindingScope::Palette;

/// Every binding, in help-panel order: section by section
/// (Shortcuts, Workspaces, Posing, Timeline, Drawing —
/// the Features section holds no bindings and stays a
/// literal table in the help panel), and within a section
/// in the order the rows have always printed, with the
/// PL-4 clusters appended to their sections.
pub static BINDINGS: &[Binding] = &[
    // -- Shortcuts (general editing) ----------------------
    Binding { id: "undo", keys: "Ctrl+Z", action: "Undo", section: "Shortcuts", scope: G, menu_key: Some("undo"), command_key: Some("undo") },
    Binding { id: "redo", keys: "Ctrl+Shift+Z", action: "Redo", section: "Shortcuts", scope: G, menu_key: Some("redo"), command_key: Some("redo") },
    Binding { id: "redo-alt", keys: "Ctrl+Y", action: "Redo", section: "Shortcuts", scope: G, menu_key: None, command_key: None },
    Binding { id: "quick-save", keys: "Ctrl+S", action: "Quick save", section: "Shortcuts", scope: G, menu_key: Some("save"), command_key: Some("save") },
    Binding { id: "open-palette", keys: "Ctrl+K", action: "Open the command palette", section: "Shortcuts", scope: G, menu_key: Some("menu-palette"), command_key: None },
    // IA-RM28-5 (RM28): F1 toggles Help — a direct arm
    // in main.rs's `on_key` and in the panel's own
    // `key_row_press`, so there is no menu or palette
    // twin to name.
    Binding { id: "help-toggle", keys: "F1", action: "Open or close Help", section: "Shortcuts", scope: G, menu_key: None, command_key: None },
    Binding { id: "palette-next", keys: "Down", action: "Next result in the command palette (palette open)", section: "Shortcuts", scope: P, menu_key: None, command_key: None },
    Binding { id: "palette-prev", keys: "Up", action: "Previous result in the command palette (palette open)", section: "Shortcuts", scope: P, menu_key: None, command_key: None },
    Binding { id: "palette-run", keys: "Enter", action: "Run the selected command (command palette open)", section: "Shortcuts", scope: P, menu_key: None, command_key: None },
    Binding { id: "close-scope", keys: "Escape", action: "Close the open menu or command palette", section: "Shortcuts", scope: G, menu_key: None, command_key: None },
    Binding { id: "delete", keys: "Delete", action: "Delete the selected stroke, particle event or segment", section: "Shortcuts", scope: G, menu_key: None, command_key: None },
    // PL-4 (RM26): the Delete alias and the File/Edit
    // chord cluster (A21 K1/K2). The Delete row order in
    // the action prose IS the cascade priority — stroke,
    // then particle event, then segment (A21 K6).
    Binding { id: "delete-alias", keys: "Backspace", action: "Delete the selected stroke, particle event or segment", section: "Shortcuts", scope: G, menu_key: None, command_key: None },
    Binding { id: "open-project", keys: "Ctrl+O", action: "Open the selected project file", section: "Shortcuts", scope: G, menu_key: None, command_key: Some("open-project") },
    Binding { id: "save-copy", keys: "Ctrl+Shift+S", action: "Save a copy of the project", section: "Shortcuts", scope: G, menu_key: None, command_key: Some("save-copy") },
    Binding { id: "duplicate-frame", keys: "Ctrl+D", action: "Duplicate the current frame", section: "Shortcuts", scope: G, menu_key: None, command_key: Some("dup-frame") },
    Binding { id: "copy-frame", keys: "Ctrl+C", action: "Copy the current frame", section: "Shortcuts", scope: G, menu_key: Some("copy-frame"), command_key: Some("copy-frame") },
    Binding { id: "paste-frame", keys: "Ctrl+V", action: "Paste the copied frame", section: "Shortcuts", scope: G, menu_key: Some("paste-frame"), command_key: Some("paste-frame") },
    // -- Workspaces ---------------------------------------
    Binding { id: "workspace-animate", keys: "Alt+1", action: "Switch to the Animate workspace", section: "Workspaces", scope: G, menu_key: None, command_key: Some("go-animate") },
    Binding { id: "workspace-draw", keys: "Alt+2", action: "Switch to the Draw workspace", section: "Workspaces", scope: G, menu_key: None, command_key: Some("go-draw") },
    Binding { id: "workspace-graph", keys: "Alt+3", action: "Switch to the Graph workspace", section: "Workspaces", scope: G, menu_key: None, command_key: Some("go-graph") },
    Binding { id: "workspace-effects", keys: "Alt+4", action: "Switch to the Effects workspace", section: "Workspaces", scope: G, menu_key: None, command_key: Some("go-effects") },
    Binding { id: "workspace-audio", keys: "Alt+5", action: "Switch to the Audio workspace", section: "Workspaces", scope: G, menu_key: None, command_key: Some("go-audio") },
    Binding { id: "workspace-3d", keys: "Alt+6", action: "Switch to the 3D workspace", section: "Workspaces", scope: G, menu_key: None, command_key: Some("go-build3d") },
    Binding { id: "workspace-export", keys: "Alt+7", action: "Switch to the Export workspace", section: "Workspaces", scope: G, menu_key: None, command_key: Some("go-export") },
    Binding { id: "workspace-home", keys: "Alt+8", action: "Switch to the Home workspace", section: "Workspaces", scope: G, menu_key: None, command_key: Some("go-home") },
    // -- Posing -------------------------------------------
    Binding { id: "camera-mode", keys: "C", action: "Toggle camera mode", section: "Posing", scope: G, menu_key: None, command_key: None },
    Binding { id: "onion-skin", keys: "O", action: "Toggle onion skin", section: "Posing", scope: G, menu_key: Some("onion"), command_key: Some("onion") },
    // PL-4 (RM26) carrying RM15's nudge (A21 K3): the
    // arrows nudge while a joint is selected and step
    // frames otherwise — one row, because the help panel
    // forbids two rows sharing a key label, and because
    // it is one physical idiom with a selection gate
    // (IA-RM15-4's rule, composed into IA-RM26-1).
    Binding { id: "nudge-joint", keys: "Arrows", action: "Nudge the selected joint by 1 (Shift 10, Alt 0.1); with no joint selected, step frames", section: "Posing", scope: G, menu_key: None, command_key: None },
    // -- Timeline -----------------------------------------
    Binding { id: "play-pause", keys: "Space", action: "Play / pause", section: "Timeline", scope: G, menu_key: None, command_key: Some("play") },
    Binding { id: "prev-frame", keys: "Left", action: "Previous frame (wraps around)", section: "Timeline", scope: G, menu_key: None, command_key: Some("prev") },
    Binding { id: "next-frame", keys: "Right", action: "Next frame (wraps around)", section: "Timeline", scope: G, menu_key: None, command_key: Some("next") },
    Binding { id: "add-frame", keys: "N", action: "Add a frame after the current one", section: "Timeline", scope: G, menu_key: None, command_key: Some("add-frame") },
    Binding { id: "insert-inbetween", keys: "I", action: "Insert an inbetween frame", section: "Timeline", scope: G, menu_key: None, command_key: Some("inbetween") },
    // PL-4 (RM26): the transport cluster (A21 K1) and
    // the keyframing cluster (A21 K3).
    Binding { id: "first-frame", keys: "Home", action: "Go to the first frame", section: "Timeline", scope: G, menu_key: None, command_key: Some("first-frame") },
    Binding { id: "last-frame", keys: "End", action: "Go to the last frame", section: "Timeline", scope: G, menu_key: None, command_key: Some("last-frame") },
    Binding { id: "back-10-frames", keys: "Shift+Left", action: "Back 10 frames", section: "Timeline", scope: G, menu_key: None, command_key: None },
    Binding { id: "forward-10-frames", keys: "Shift+Right", action: "Forward 10 frames", section: "Timeline", scope: G, menu_key: None, command_key: None },
    Binding { id: "prev-keyframe", keys: ",", action: "Go to the previous keyframe", section: "Timeline", scope: G, menu_key: None, command_key: Some("prev-keyframe") },
    Binding { id: "next-keyframe", keys: ".", action: "Go to the next keyframe", section: "Timeline", scope: G, menu_key: None, command_key: Some("next-keyframe") },
    Binding { id: "key-all-channels", keys: "K", action: "Key all channels at the playhead", section: "Timeline", scope: G, menu_key: None, command_key: Some("key-all-channels") },
    Binding { id: "delete-key-here", keys: "Alt+K", action: "Delete the key at the playhead", section: "Timeline", scope: G, menu_key: None, command_key: Some("delete-key-here") },
    // -- Drawing (PL-4, RM26 — A21 K4: the section's
    // first real keys) ------------------------------------
    Binding { id: "draw-mode", keys: "B", action: "Toggle draw mode", section: "Drawing", scope: G, menu_key: None, command_key: Some("draw-mode") },
    Binding { id: "brush-size-down", keys: "[", action: "Brush size down (nearest smaller preset)", section: "Drawing", scope: G, menu_key: None, command_key: Some("brush-size-down") },
    Binding { id: "brush-size-up", keys: "]", action: "Brush size up (nearest larger preset)", section: "Drawing", scope: G, menu_key: None, command_key: Some("brush-size-up") },
    Binding { id: "cycle-brush", keys: "Shift+B", action: "Cycle brush", section: "Drawing", scope: G, menu_key: None, command_key: Some("cycle-brush") },
];

/// The binding with this registry id, if any.
pub fn binding(id: &str) -> Option<&'static Binding> {
    BINDINGS.iter().find(|b| b.id == id)
}

/// The (key label, action) rows of one help section, in
/// panel order — the generator behind
/// `help_panel::section_rows` for every section that
/// holds bindings.
pub fn section_rows(section: &str) -> Vec<(&'static str, &'static str)> {
    BINDINGS
        .iter()
        .filter(|b| b.section == section)
        .map(|b| (b.keys, b.action))
        .collect()
}

/// The shortcut spelling for a menu item key, when the
/// item's action is exactly a registered binding's —
/// the generator behind `menus::shortcut_hint`.
pub fn menu_hint(menu_key: &str) -> Option<&'static str> {
    BINDINGS
        .iter()
        .find(|b| b.menu_key == Some(menu_key))
        .map(|b| b.keys)
}

/// The shortcut spelling for a palette command key, when
/// the command's action is also a registered binding —
/// the generator behind the palette's kbd chips.
pub fn command_hint(command_key: &str) -> Option<&'static str> {
    BINDINGS
        .iter()
        .find(|b| b.command_key == Some(command_key))
        .map(|b| b.keys)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_holds_45_bindings_with_unique_ids_and_labels() {
        // 25 pre-PL-4 bindings (10 Shortcuts + 8
        // Workspaces + 2 Posing + 5 Timeline) + 19 PL-4
        // additions (RM26: 6 Shortcuts, 1 Posing, 8
        // Timeline, 4 Drawing) + 1 IA-RM28-5 addition
        // (F1 in Shortcuts).
        assert_eq!(BINDINGS.len(), 45);
        for (i, b) in BINDINGS.iter().enumerate() {
            assert!(
                !BINDINGS[..i].iter().any(|o| o.id == b.id),
                "duplicate binding id: {}",
                b.id
            );
            // The help panel forbids two rows sharing a
            // key label; the registry keeps that law at
            // the source.
            assert!(
                !BINDINGS[..i].iter().any(|o| o.keys == b.keys),
                "duplicate key label: {}",
                b.keys
            );
            assert!(!b.action.is_empty(), "binding {} has no action", b.id);
        }
    }

    #[test]
    fn section_counts_match_the_help_panel_census() {
        assert_eq!(section_rows("Shortcuts").len(), 16);
        assert_eq!(section_rows("Workspaces").len(), 8);
        assert_eq!(section_rows("Posing").len(), 3);
        assert_eq!(section_rows("Timeline").len(), 13);
        assert_eq!(section_rows("Drawing").len(), 4);
        // Features holds no bindings, by design.
        assert_eq!(section_rows("Features").len(), 0);
        assert_eq!(section_rows("No such section").len(), 0);
    }

    #[test]
    fn palette_scope_is_exactly_the_three_modal_keys() {
        let modal: Vec<&str> = BINDINGS
            .iter()
            .filter(|b| b.scope == BindingScope::Palette)
            .map(|b| b.id)
            .collect();
        assert_eq!(modal, vec!["palette-next", "palette-prev", "palette-run"]);
    }

    #[test]
    fn menu_hints_cover_exactly_the_seven_true_twins() {
        assert_eq!(menu_hint("undo"), Some("Ctrl+Z"));
        assert_eq!(menu_hint("redo"), Some("Ctrl+Shift+Z"));
        assert_eq!(menu_hint("save"), Some("Ctrl+S"));
        assert_eq!(menu_hint("menu-palette"), Some("Ctrl+K"));
        assert_eq!(menu_hint("onion"), Some("O"));
        assert_eq!(menu_hint("copy-frame"), Some("Ctrl+C"));
        assert_eq!(menu_hint("paste-frame"), Some("Ctrl+V"));
        // The honesty cases stay hint-less: the app-bar
        // Load button quick-loads the working file while
        // Ctrl+O opens the selected project file, and the
        // Edit menu's Delete items name single actions
        // while the key runs the priority cascade.
        assert_eq!(menu_hint("load"), None);
        assert_eq!(menu_hint("edit-delete-stroke"), None);
        assert_eq!(menu_hint("edit-delete-particle"), None);
        let hinted = BINDINGS.iter().filter(|b| b.menu_key.is_some()).count();
        assert_eq!(hinted, 7);
    }

    #[test]
    fn command_hints_cover_32_commands() {
        let hinted = BINDINGS.iter().filter(|b| b.command_key.is_some()).count();
        assert_eq!(hinted, 32);
        assert_eq!(command_hint("play"), Some("Space"));
        assert_eq!(command_hint("first-frame"), Some("Home"));
        assert_eq!(command_hint("last-frame"), Some("End"));
        assert_eq!(command_hint("prev-keyframe"), Some(","));
        assert_eq!(command_hint("next-keyframe"), Some("."));
        assert_eq!(command_hint("key-all-channels"), Some("K"));
        assert_eq!(command_hint("delete-key-here"), Some("Alt+K"));
        assert_eq!(command_hint("cycle-brush"), Some("Shift+B"));
        // No command twin: the camera-mode key (C) has no
        // palette command, and the nudge has none either.
        assert_eq!(command_hint("toggle-camera-mode"), None);
    }
}
