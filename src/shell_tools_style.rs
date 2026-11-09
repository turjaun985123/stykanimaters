//! Shell-control MCP handler, style family (Focused Wave,
//! B2): the read-back for the style inspector's
//! recent-colours list — census gap G7, closed as a READ
//! tool. It acts on GUI *shell* state rather than the
//! project: the handler is dispatched by `shell_control`'s
//! drain on the live endpoint, it is strictly read-only (no
//! undo step, no dirty mark, no mutation of the list it
//! reports), and on stdio the registered stub refuses
//! honestly (see the seam note below).
//!
//! ---- The recent-colours list, as found in source -----
//!
//! * **Storage.** `RECENT_COLORS`
//!   (`src/rune_sec/inspector_style.rs:619`): a
//!   `thread_local! RefCell<Vec<u32>>` owned by the style
//!   panel, because — in its own doc's words — the style
//!   panel has no state struct. It survives workspace
//!   switches and panel remounts for the life of the
//!   process and is **not persisted**: not in the project,
//!   not in `LayoutPrefs`, gone on restart.
//! * **Policy.** Most recent first, no duplicates, capped
//!   at `RECENT_COLORS_CAP` = 8
//!   (`inspector_style.rs:605`), all of it in the one pure
//!   function `push_recent` (`inspector_style.rs:629`):
//!   the new colour moves to the front, any earlier
//!   occurrence is dropped, the tail is truncated.
//! * **The only writer** is `note_recent_color`
//!   (`inspector_style.rs:651`), called from exactly five
//!   sites, all of them this panel's own colour-applying
//!   handlers: a gradient stepper press
//!   (`inspector_style.rs:436`), a gradient swatch press
//!   (`:463`, colour presses only — the Off press records
//!   nothing), a preset application (`:566`), a style
//!   paste (`:599`), and a recent-chip press itself
//!   (`:671`, via `apply_recent_color`, which re-notes the
//!   colour and so moves it to the front). The
//!   primary-colour RGB steppers deliberately never record
//!   (their presses are intermediate values, not chosen
//!   colours — the panel's module doc says so). Nothing
//!   outside `inspector_style.rs` writes the list.
//! * **The only reader** before this tool is the panel
//!   itself: `recent_colors()` (`inspector_style.rs:638`,
//!   a clone) feeds the Recent row at
//!   `inspector_style.rs:1043`; the row renders one chip
//!   per colour and is hidden while the list is empty.
//!
//! ---- Why a read tool, and why no write tool ----------
//!
//! The list is shell state in exactly the sense
//! `shell_control` exists for: state the user creates by
//! working in the GUI that does not live in the
//! `EditorCore` at all. It is also **not derivable** from
//! anything else an MCP client can read — it is a history
//! (including gradient-end and pasted colours, since
//! dropped from the nodes they were applied to), not a
//! projection of the project. A client that wants to offer
//! "the colours you have been using" — the same offer the
//! panel's Recent row makes — can only get it here. That
//! is the legitimate read the census asked about, so it
//! lands.
//!
//! A write tool was considered and rejected, on evidence:
//! applying a colour to the selected node is already
//! tooled (`set_node_style` / the node colour setters in
//! the core registry), and `apply_recent_color` itself is
//! nothing but `set_node_color` + the re-note. An
//! "apply recent colour by index" tool would duplicate
//! the core tool with a more fragile address (a position
//! in a list that reorders on every press), and a
//! "push this colour into the list" tool would fabricate
//! history — list entries for colours the user never
//! applied through the panel. Both are fake in the way
//! this project refuses; neither is provided.
//!
//! ---- The shell-control seam (as landed) --------------
//!
//! The handler runs from `crate::shell_control::drain` on
//! the UI thread — the same thread the panel's handlers
//! run on, which is what makes reading the panel's
//! `thread_local` here correct rather than racy (the
//! Wave 14 view tools read the view layer's `thread_local`
//! the same way). It takes `&State` only for dispatch
//! uniformity with its sibling families; the list needs
//! neither the state nor arguments. On stdio the name is
//! registered with the shared `shell_unavailable` stub, so
//! a stdio call refuses with the live-editor wording and
//! never reaches this handler.
//!
//! Reply: the colours most recent first, each as
//! `{"hex": "#RRGGBB", "value": n}` — the hex is the
//! panel's own display form (`inspector_style::
//! color_text`), the value the `0xRRGGBB` number the core
//! colour tools take — plus `count` and the list's `cap`.
//! An empty list is a successful `count: 0` reply, not an
//! error: the GUI hides the row in exactly that case.

use serde_json::{json, Value};

use crate::rune_sec::inspector_style;
use crate::State;

/// `shell_get_recent_colours`: the style inspector's
/// recent-colours list, most recent first. Read-only —
/// the snapshot clones the panel's list, so reading it
/// neither reorders nor drains it.
pub fn tool_shell_get_recent_colours(_state: &State, _args: &Value) -> Result<Value, String> {
    Ok(recent_colours_snapshot())
}

/// The snapshot [`tool_shell_get_recent_colours`]
/// returns. Split from the executor (the
/// `shell_tools_settings::settings_snapshot` pattern) so
/// its shape has one definition and the tests can pin it
/// without standing up a `State`.
pub fn recent_colours_snapshot() -> Value {
    let colours: Vec<Value> = inspector_style::recent_colors()
        .iter()
        .map(|&value| {
            json!({
                "hex": inspector_style::color_text(value),
                "value": value,
            })
        })
        .collect();
    json!({
        "colors": colours,
        "count": colours.len(),
        "cap": inspector_style::RECENT_COLORS_CAP,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // `RECENT_COLORS` is a per-thread `thread_local` and
    // the test harness runs each test on its own thread,
    // so every test below starts from the panel's empty
    // list — the same isolation `inspector_style`'s own
    // tests rely on.

    #[test]
    fn empty_list_reads_as_count_zero_not_an_error() {
        let snap = recent_colours_snapshot();
        assert_eq!(snap["count"], json!(0));
        assert_eq!(snap["colors"], json!([]));
        assert_eq!(
            snap["cap"],
            json!(inspector_style::RECENT_COLORS_CAP),
            "the cap reported is the panel's own constant"
        );
    }

    #[test]
    fn noted_colours_read_back_most_recent_first_in_both_forms() {
        inspector_style::note_recent_color(0x11_1111);
        inspector_style::note_recent_color(0xE8_E8EC);
        let snap = recent_colours_snapshot();
        assert_eq!(snap["count"], json!(2));
        assert_eq!(
            snap["colors"],
            json!([
                { "hex": "#E8E8EC", "value": 0xE8_E8EC },
                { "hex": "#111111", "value": 0x11_1111 },
            ]),
            "front of the list is the most recently noted colour"
        );
    }

    #[test]
    fn snapshot_reflects_the_panels_dedupe_and_cap_policy() {
        // Nine distinct colours into a cap-8 list: the
        // oldest is gone; re-noting a survivor fronts it
        // without duplicating it.
        for i in 0..9u32 {
            inspector_style::note_recent_color(0x10_0000 + i);
        }
        inspector_style::note_recent_color(0x10_0005);
        let snap = recent_colours_snapshot();
        assert_eq!(snap["count"], json!(8));
        let colours = snap["colors"].as_array().expect("colors is an array");
        assert_eq!(colours[0]["value"], json!(0x10_0005), "re-noted colour fronts");
        assert!(
            colours.iter().all(|c| c["value"] != json!(0x10_0000)),
            "the oldest colour was truncated away"
        );
        let mut values: Vec<Value> = colours.iter().map(|c| c["value"].clone()).collect();
        values.sort_by_key(|v| v.as_u64().unwrap_or(0));
        values.dedup();
        assert_eq!(values.len(), 8, "no duplicates in the reported list");
    }

    #[test]
    fn reading_neither_reorders_nor_drains_the_list() {
        inspector_style::note_recent_color(0x22_2222);
        inspector_style::note_recent_color(0x33_3333);
        let first = recent_colours_snapshot();
        let second = recent_colours_snapshot();
        assert_eq!(first, second, "two reads of an untouched list agree");
        assert_eq!(
            inspector_style::recent_colors(),
            vec![0x33_3333, 0x22_2222],
            "the panel's own list is exactly as the reads found it"
        );
    }
}
