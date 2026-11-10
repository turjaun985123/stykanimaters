//! Bundled stress fixtures (Roadmap PL-13, Wave 2 W2-74):
//! the long-project fixture under `assets/fixtures/`,
//! embedded in the binary the same way the starter
//! templates and the sample episode are (see templates.rs
//! / samples.rs) — a fixture the app ships is a fixture
//! the headless probe can load, from any working
//! directory.
//!
//! A fixture is measurement input, never a session state:
//! the one caller is the headless `long-project` scene,
//! which installs the parsed [`Project`] into the probe's
//! fresh state exactly the way a scene plants any other
//! project data.

use crate::model::Project;

/// The parsed long-project fixture (600 frames, 12
/// figures), through the loader's own rule (the same
/// `serde_json::from_str::<Project>` that
/// `projects::load` runs).
pub fn long_project() -> Result<Project, String> {
    serde_json::from_str(include_str!(
        "../assets/fixtures/long-project.json"
    ))
    .map_err(|e| format!("long-project fixture does not parse: {e}"))
}
