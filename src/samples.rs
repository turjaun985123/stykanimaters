//! Bundled sample content (Roadmap E-2, Wave 2 W2-92): the
//! finished demo episode under `assets/samples/`, embedded
//! in the binary the same way the starter templates are
//! (see templates.rs) — a sample the app ships is a sample
//! the app can open, from any working directory.
//!
//! A sample is content to look at, never a session state:
//! callers install the parsed [`Project`] as a fresh unsaved
//! session exactly the way Home's New installs
//! `Project::new_demo`.

use crate::model::Project;

/// One bundled sample: its chooser identity and its file.
pub struct Sample {
    pub id: &'static str,
    pub name: &'static str,
    pub blurb: &'static str,
    json: &'static str,
}

pub const SAMPLES: &[Sample] = &[Sample {
    id: "the-bus-stop",
    name: "The Bus Stop",
    blurb: "A finished 4-second episode: two chapters, dialogue, a camera move.",
    json: include_str!("../assets/samples/the-bus-stop.json"),
}];

/// The parsed project for sample `id`, through the loader's
/// own rule (the same `serde_json::from_str::<Project>` that
/// `projects::load` runs).
pub fn sample_project(id: &str) -> Result<Project, String> {
    let s = SAMPLES
        .iter()
        .find(|s| s.id == id)
        .ok_or_else(|| format!("unknown sample '{id}'"))?;
    serde_json::from_str(s.json)
        .map_err(|e| format!("sample '{id}' does not parse: {e}"))
}
