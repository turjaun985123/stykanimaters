//! Wave 14, Squad E (session E10) — CROSS-CUTTING contract
//! tests for the MCP surface as Wave 14 grows it.
//!
//! The per-tool behaviour suites belong to the squads
//! that wrote the tools (E1's MCP-only build, C7/C10's
//! clip-media suites, D2's transport parity harness, and
//! each domain file's own unit tests). This file pins
//! only what cuts ACROSS all of them — the standing
//! contracts every new tool inherits, re-verified against
//! the Wave 14 additions:
//!
//!   1. an unknown tool name is a PROTOCOL error,
//!      `-32602`, with the byte-exact message
//!      `unknown tool: {name}` — including near-miss
//!      spellings of the NEW Wave 14 tool names, so a
//!      pluralised or mistyped new name can never fall
//!      into a different error shape as the registry
//!      grows;
//!   2. an unknown-tool call spends no undo step and is
//!      not answered as a tool result;
//!   3. notifications are NEVER answered — not the
//!      initialised notification, not a `tools/call`
//!      sent without an id (for a real tool or an
//!      unknown one), not an unknown method;
//!   4. a REFUSED call to a new Wave 14 mutating handler
//!      spends no undo step: bad index, out-of-range
//!      value and non-finite float (including a finite
//!      f64 that overflows the f32 cast, e.g. 1e300) are
//!      all refused BEFORE the op runs. These cases call
//!      the handlers directly because at the time of
//!      writing their registry entries are the
//!      integrator's splice — the handler is the unit
//!      the contract binds, and the splice cannot change
//!      a handler's refusal order;
//!   5. the positive control: successful calls to a
//!      sample of the new strict-family handlers spend
//!      EXACTLY one undo step each — the contract's
//!      other half, so a future "fix" that quietly drops
//!      a snapshot fails here.
//!
//! The full per-handler audit these tests sample from
//! is `docs/wave14/CONTRACT_AUDIT.md`.

use serde_json::{json, Value};

use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::mcp::McpServer;
use stykanimate_rs::model::{AudioClip, FxClipEntry};
use stykanimate_rs::vfx::{ClipKind, ParticleClip};

// ---------------------------------------------------------------------
// Dispatch helper (the house pattern, compact form)
// ---------------------------------------------------------------------

struct Dispatch {
    server: McpServer,
    next_id: u64,
}

impl Dispatch {
    fn new() -> Self {
        Dispatch {
            server: McpServer::new(),
            next_id: 1,
        }
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        let response = self
            .server
            .handle(&json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": method,
                "params": params,
            }))
            .expect("a request with an id must be answered");
        assert_eq!(response["id"], json!(id), "{method}: id not echoed");
        response
    }

    fn undo_depth(&mut self) -> u64 {
        let response = self.request(
            "tools/call",
            json!({ "name": "get_editor_status", "arguments": {} }),
        );
        let text = response["result"]["content"][0]["text"]
            .as_str()
            .expect("get_editor_status answers with one text block");
        let payload: Value =
            serde_json::from_str(text).expect("get_editor_status payload is JSON");
        payload["undo_depth"]
            .as_u64()
            .expect("get_editor_status reports undo_depth")
    }
}

// ---------------------------------------------------------------------
// 1 + 2. Unknown tool: -32602, exact message, no undo spent
// ---------------------------------------------------------------------

#[test]
fn unknown_tool_names_earn_minus_32602_with_the_exact_message() {
    let mut mcp = Dispatch::new();
    // Near-misses of NEW Wave 14 names (fx_set_seed,
    // sprite_sample, particle_set_seed, graph_key_interp,
    // shell_* family) plus plain garbage: none of these
    // is registered, before or after the Wave 14 splice,
    // so the protocol error must be the answer for each.
    for name in [
        "fx_set_seeds",
        "sprite_samples",
        "particle_set_seeds",
        "graph_key_interps",
        "shell_frobnicate",
        "definitely_not_a_tool",
    ] {
        let response = mcp.request(
            "tools/call",
            json!({ "name": name, "arguments": {} }),
        );
        assert!(
            response.get("result").is_none(),
            "{name}: an unknown tool must not produce a tool result: {response}"
        );
        let error = &response["error"];
        assert_eq!(
            error["code"],
            json!(-32602),
            "{name}: unknown tool is a -32602 protocol error: {response}"
        );
        assert_eq!(
            error["message"],
            json!(format!("unknown tool: {name}")),
            "{name}: the message is byte-exact: {response}"
        );
    }
}

#[test]
fn an_unknown_tool_call_spends_no_undo_step() {
    let mut mcp = Dispatch::new();
    let before = mcp.undo_depth();
    let response = mcp.request(
        "tools/call",
        json!({ "name": "fx_set_seeds", "arguments": {} }),
    );
    assert_eq!(response["error"]["code"], json!(-32602));
    assert_eq!(
        mcp.undo_depth(),
        before,
        "an unknown-tool call must not touch the undo history"
    );
}

// ---------------------------------------------------------------------
// 3. Notification silence
// ---------------------------------------------------------------------

#[test]
fn notifications_are_never_answered() {
    let mut server = McpServer::new();
    // The initialised notification.
    assert!(
        server
            .handle(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }))
            .is_none(),
        "notifications/initialized gets no reply"
    );
    // A tools/call with no id — a real tool, notification form.
    assert!(
        server
            .handle(&json!({
                "jsonrpc": "2.0",
                "method": "tools/call",
                "params": { "name": "get_state", "arguments": {} },
            }))
            .is_none(),
        "a tools/call notification gets no reply, even for a real tool"
    );
    // A tools/call with no id — an unknown tool: still
    // silence, never a -32602 (notifications are never
    // answered, known or not).
    assert!(
        server
            .handle(&json!({
                "jsonrpc": "2.0",
                "method": "tools/call",
                "params": { "name": "fx_set_seeds", "arguments": {} },
            }))
            .is_none(),
        "a tools/call notification for an unknown tool gets no reply"
    );
    // An unknown method in notification form.
    assert!(
        server
            .handle(&json!({ "jsonrpc": "2.0", "method": "notifications/whatever" }))
            .is_none(),
        "an unknown-method notification gets no reply"
    );
}

// ---------------------------------------------------------------------
// 4. Refused calls to NEW handlers spend no undo step
// ---------------------------------------------------------------------

fn fx_core() -> EditorCore {
    let mut core = EditorCore::new();
    core.project.fx_clips.push(FxClipEntry {
        clip: ParticleClip {
            start: 0.0,
            duration: 2.0,
            kind: ClipKind::Burst,
            color: 0xFFFFFF,
            color2: None,
            seed: 7,
            angle: 0.0,
            length: 130.0,
        },
        figure: 0,
        node: 0,
        hidden: false,
    });
    core
}

fn audio_core() -> EditorCore {
    let mut core = EditorCore::new();
    core.project.audio.push(AudioClip {
        path: "a.wav".into(),
        start_frame: 0,
        gain: 1.0,
        muted: false,
    });
    core
}

/// One refused call: it must be an `Err` whose text
/// contains `needle`, and the undo depth must be
/// exactly what it was before the call. The call is a
/// closure so the depth is read BEFORE it runs.
fn assert_refused_no_spend(
    core: &mut EditorCore,
    call: impl FnOnce(&mut EditorCore) -> Result<Value, String>,
    needle: &str,
    what: &str,
) {
    let before = core.undo_depth();
    let outcome = call(core);
    let err = match outcome {
        Ok(_) => panic!("{what}: the call must be refused"),
        Err(e) => e,
    };
    assert!(
        err.contains(needle),
        "{what}: refusal must contain {needle:?}, got {err:?}"
    );
    assert_eq!(
        core.undo_depth(),
        before,
        "{what}: a refused call spends no undo step"
    );
}

#[test]
fn refused_fx_calls_spend_no_undo() {
    let mut core = fx_core();
    assert_refused_no_spend(
        &mut core,
        |core| stykanimate_rs::fx_ops::tool_fx_set_angle(
            core,
            &json!({ "index": 0, "angle": 1e300 }),
        ),
        "finite",
        "fx_set_angle with an f32-overflowing angle",
    );
    assert_refused_no_spend(
        &mut core,
        |core| stykanimate_rs::fx_ops::tool_fx_set_angle(
            core,
            &json!({ "index": 99, "angle": 0.0 }),
        ),
        "no FX clip at index 99",
        "fx_set_angle with a missing clip",
    );
    assert_refused_no_spend(
        &mut core,
        |core| stykanimate_rs::fx_ops::tool_fx_set_seed(
            core,
            &json!({ "index": 0, "seed": 4294967296u64 }),
        ),
        "too large",
        "fx_set_seed with a seed past u32",
    );
    assert_refused_no_spend(
        &mut core,
        |core| stykanimate_rs::fx_ops::tool_fx_set_length(
            core,
            &json!({ "index": 0, "length": -1e300 }),
        ),
        "finite",
        "fx_set_length with an f32-overflowing length",
    );
}

#[test]
fn refused_particle_and_audio_calls_spend_no_undo() {
    let mut core = EditorCore::new();
    assert_refused_no_spend(
        &mut core,
        |core| stykanimate_rs::particle_ops::tool_particle_set_seed(
            core,
            &json!({ "index": 0, "seed": 1 }),
        ),
        "no particle event at index 0",
        "particle_set_seed with no events",
    );

    let mut core = audio_core();
    assert_refused_no_spend(
        &mut core,
        |core| stykanimate_rs::audio_ops::tool_audio_clip_gain_step(
            core,
            &json!({ "index": 0, "delta": 1e300 }),
        ),
        "finite",
        "audio_clip_gain_step with an f32-overflowing delta",
    );
    assert_eq!(
        core.project.audio[0].gain, 1.0,
        "the refused gain step changed nothing"
    );
}

#[test]
fn refused_graph_sprite_and_settings_calls_spend_no_undo() {
    // graph_count_keys_in_range: the from-after-to
    // refusal (needs one channel to pass the index
    // check first).
    let mut core = EditorCore::new();
    core.project.channels.push(stykanimate_rs::graph::Channel {
        figure: 0,
        target: stykanimate_rs::graph::ChannelTarget::RootX,
        keys: Vec::new(),
    });
    assert_refused_no_spend(
        &mut core,
        |core| stykanimate_rs::graph_keys::tool_graph_count_keys_in_range(
            core,
            &json!({ "channel": 0, "from": 5.0, "to": 2.0 }),
        ),
        "must not be after",
        "graph_count_keys_in_range with from after to",
    );

    // sprite_sample: no sprites at all.
    let mut core = EditorCore::new();
    assert_refused_no_spend(
        &mut core,
        |core| stykanimate_rs::media::tool_sprite_sample(
            core,
            &json!({ "sprite": 0, "frame": 0.0 }),
        ),
        "sprite",
        "sprite_sample with no sprites",
    );

    // set_weather_amount: weather off is a refusal, and
    // the mark-dirty family must not even mark.
    let mut core = EditorCore::new();
    core.project.weather = None;
    let edits_before = core.edits_since_autosave();
    assert_refused_no_spend(
        &mut core,
        |core| stykanimate_rs::editor::tool_set_weather_amount(
            core,
            &json!({ "amount": 50.0 }),
        ),
        "Weather is off",
        "set_weather_amount with the weather off",
    );
    assert_eq!(
        core.edits_since_autosave(),
        edits_before,
        "the refused weather call must not mark an edit either"
    );
    assert!(core.project.weather.is_none(), "weather stayed off");
}

#[test]
fn refused_camera_frame_and_figure_calls_spend_no_undo() {
    let mut core = EditorCore::new();
    assert_refused_no_spend(
        &mut core,
        |core| stykanimate_rs::editor::tool_camera_zoom_by(
            core,
            &json!({ "factor": -2.0 }),
        ),
        "must be positive",
        "camera_zoom_by with a negative factor",
    );
    assert_refused_no_spend(
        &mut core,
        |core| stykanimate_rs::editor::tool_camera_zoom_by(
            core,
            &json!({ "factor": 1e300 }),
        ),
        "finite",
        "camera_zoom_by with an f32-overflowing factor",
    );
    assert_refused_no_spend(
        &mut core,
        |core| stykanimate_rs::frame_swap::tool_adjust_frame_duration(
            core,
            &json!({ "factor": 0.0 }),
        ),
        "must be positive",
        "adjust_frame_duration with a zero factor",
    );
    assert_refused_no_spend(
        &mut core,
        |core| stykanimate_rs::figure_ops::tool_nudge_path_point(
            core,
            &json!({ "dx": 1e300, "dy": 0.0 }),
        ),
        "finite",
        "nudge_path_point with an f32-overflowing dx",
    );
}

// ---------------------------------------------------------------------
// 5. Positive control: success spends exactly one undo step
// ---------------------------------------------------------------------

#[test]
fn successful_new_strict_family_calls_spend_exactly_one_undo_step() {
    let mut core = fx_core();
    let before = core.undo_depth();
    stykanimate_rs::fx_ops::tool_fx_set_angle(
        &mut core,
        &json!({ "index": 0, "angle": 45.0 }),
    )
    .expect("fx_set_angle succeeds");
    assert_eq!(
        core.undo_depth(),
        before + 1,
        "fx_set_angle is exactly one undo step"
    );

    let mut core = audio_core();
    let before = core.undo_depth();
    stykanimate_rs::audio_ops::tool_audio_clip_gain_step(
        &mut core,
        &json!({ "index": 0, "delta": 0.25 }),
    )
    .expect("audio_clip_gain_step succeeds");
    assert_eq!(
        core.undo_depth(),
        before + 1,
        "audio_clip_gain_step is exactly one undo step"
    );

    let mut core = EditorCore::new();
    let before = core.undo_depth();
    stykanimate_rs::editor::tool_camera_zoom_by(&mut core, &json!({ "factor": 2.0 }))
        .expect("camera_zoom_by succeeds");
    assert_eq!(
        core.undo_depth(),
        before + 1,
        "camera_zoom_by is exactly one undo step"
    );
}
