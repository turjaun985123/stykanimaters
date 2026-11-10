//! Starter templates (Roadmap E-1, Wave 2 W2-91): the three
//! bundled project files under `assets/templates/`, embedded
//! in the binary (the assets precedent: icon data is baked
//! into source the same way, so a template can never be lost
//! to a working-directory change — the failure F-8 removed
//! for project storage).
//!
//! A template is a starting point, never a session state:
//! callers install the parsed [`Project`] as a fresh unsaved
//! session exactly the way Home's New installs
//! `Project::new_demo` (see `home_panel::dispatch_new`).

use crate::model::Project;

/// One bundled starter: its chooser identity and its file.
pub struct Template {
    pub id: &'static str,
    pub name: &'static str,
    pub blurb: &'static str,
    json: &'static str,
}

pub const TEMPLATES: &[Template] = &[
    Template {
        id: "episode-blank",
        name: "Blank Episode",
        blurb: "Episode scaffold: 3 chapters, title cards, one figure.",
        json: include_str!("../assets/templates/episode-blank.json"),
    },
    Template {
        id: "dialogue-scene",
        name: "Dialogue Scene",
        blurb: "Two figures in conversation, subtitled with speakers.",
        json: include_str!("../assets/templates/dialogue-scene.json"),
    },
    Template {
        id: "acting-scene",
        name: "Two-Figure Acting Scene",
        blurb: "A walk, an exchange of gestures, a moving camera.",
        json: include_str!("../assets/templates/acting-scene.json"),
    },
];

/// The parsed project for template `id`, through the loader's
/// own rule (the same `serde_json::from_str::<Project>` that
/// `projects::load` runs), so a template the app ships is a
/// template the app can open.
pub fn template_project(id: &str) -> Result<Project, String> {
    let t = TEMPLATES
        .iter()
        .find(|t| t.id == id)
        .ok_or_else(|| format!("unknown template '{id}'"))?;
    serde_json::from_str(t.json)
        .map_err(|e| format!("template '{id}' does not parse: {e}"))
}
