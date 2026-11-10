//! Wave P (P16) — format_version behaviour pins.
//!
//! Board contract 2 (Wave P): `Project` carries
//! `format_version: u32` with serde default =
//! `FORMAT_VERSION_CURRENT` (1) so legacy files load; the
//! constant and its doc rule live in `src/model.rs` beside
//! the field (P23 owns the field; these pins own the
//! behaviour). The rule pinned here, as written in
//! model.rs:
//!
//!   - a file WITHOUT the field (every project saved
//!     before the field existed) loads and reads as the
//!     current version — the field is absorbed by its
//!     serde default, with no format_version bump and no
//!     migration machinery;
//!   - a file stamped with a version NEWER than
//!     `FORMAT_VERSION_CURRENT` still loads (unknown-field
//!     forward compatibility: the loader never refuses on
//!     version alone) and the stored stamp survives a
//!     save→load round trip untouched.
//!
//! Serialisation in these tests is the production pair:
//! `serde_json::to_string_pretty` out (what
//! `projects::save` writes, src/projects.rs) and
//! `serde_json::from_str::<Project>` in (what
//! `projects::load` reads).
//!
//! INTEGRATOR SEQUENCING NOTE: these pins compile only
//! once P23's `format_version` field + `FORMAT_VERSION_
//! CURRENT` constant land in src/model.rs. P16 must not
//! edit model.rs, so if CI reaches this file before P23,
//! the failure is a sequencing error, not a behaviour
//! regression.

use serde_json::Value;
use stykanimate_rs::model::{Project, FORMAT_VERSION_CURRENT};

/// The documented current format version. Pinned as a
/// literal so a silent change to the constant's VALUE
/// fails here even if every use site moves together.
const CURRENT: u32 = 1;

/// Serialise exactly as `projects::save` does.
fn saved_text(project: &Project) -> String {
    serde_json::to_string_pretty(project).expect("a project always serialises")
}

/// Parse exactly as `projects::load` does.
fn load_text(text: &str) -> Project {
    serde_json::from_str(text).expect("project text must load")
}

/// A small project with non-default content in several
/// field families, so the round-trip pin compares more
/// than the constructor's demo values. fps is set
/// explicitly (never assumed from the demo).
fn fixture() -> Project {
    let mut project = Project::new_demo();
    project.name = "Format Version Fixture".to_string();
    project.fps = 24.0;
    project.background = 0x102030;
    project.head_ring_scale = 1.25;
    project
}

#[test]
fn the_current_version_constant_is_one() {
    assert_eq!(
        FORMAT_VERSION_CURRENT, CURRENT,
        "FORMAT_VERSION_CURRENT is the documented current version; \
         changing it is a format decision, not a refactor"
    );
}

#[test]
fn a_fresh_project_reports_the_current_version() {
    assert_eq!(Project::new_demo().format_version, CURRENT);
    assert_eq!(fixture().format_version, CURRENT);
}

#[test]
fn a_saved_project_carries_the_format_version_field() {
    let text = saved_text(&fixture());
    assert!(
        text.contains("\"format_version\": 1"),
        "saved project JSON must carry the version stamp:\n{text}"
    );
    let value: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(
        value.get("format_version").and_then(Value::as_u64),
        Some(CURRENT as u64),
        "the stamp is the JSON number 1, not a string or a float"
    );
}

#[test]
fn a_legacy_file_without_the_field_loads_as_current() {
    // Every project file written before the field existed
    // looks like this: the full serialised project with
    // the `format_version` key removed by hand.
    let mut value: Value = serde_json::from_str(&saved_text(&fixture())).unwrap();
    let object = value.as_object_mut().expect("a project is a JSON object");
    assert!(
        object.remove("format_version").is_some(),
        "the fixture must actually start from a stamped file"
    );
    let legacy_text = serde_json::to_string_pretty(&value).unwrap();
    assert!(
        !legacy_text.contains("format_version"),
        "the hand-written legacy file carries no version field"
    );

    let loaded = load_text(&legacy_text);
    assert_eq!(
        loaded.format_version, CURRENT,
        "a missing field is absorbed by the serde default and \
         reads as the current version"
    );
}

#[test]
fn save_load_round_trip_preserves_every_field() {
    let project = fixture();
    let before = serde_json::to_value(&project).unwrap();
    let loaded = load_text(&saved_text(&project));
    let after = serde_json::to_value(&loaded).unwrap();
    assert_eq!(
        before, after,
        "save→load must preserve every field — format_version's \
         arrival changes no other field's meaning"
    );
}

#[test]
fn a_file_stamped_zero_loads_with_its_stamp_untouched() {
    // 0 reads as an older-than-current marker. The doc
    // rule admits it (serde has no range validation) and
    // pins no migration: the file loads and the stored
    // stamp is reported as stored until a save rewrites
    // the project.
    let mut value: Value = serde_json::from_str(&saved_text(&fixture())).unwrap();
    value["format_version"] = Value::from(0u32);
    let loaded = load_text(&serde_json::to_string_pretty(&value).unwrap());
    assert_eq!(
        loaded.format_version, 0,
        "an older stamp is neither refused nor silently rewritten at load"
    );
}

#[test]
fn a_file_from_a_newer_version_still_loads() {
    // The doc rule's forward-compatibility half: the
    // loader never refuses on version alone, so a file
    // stamped by a newer build loads and its stamp
    // survives the round trip — refusing here would make
    // every future format bump a load-breaking change.
    let newer = CURRENT + 1;
    let mut value: Value = serde_json::from_str(&saved_text(&fixture())).unwrap();
    value["format_version"] = Value::from(newer);
    let loaded = load_text(&serde_json::to_string_pretty(&value).unwrap());
    assert_eq!(
        loaded.format_version, newer,
        "a newer stamp loads and round-trips untouched"
    );
}
