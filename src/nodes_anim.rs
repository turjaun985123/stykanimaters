//! Stick Nodes animation containers: `.stknds` projects and
//! `.nodemc` movie clips.
//!
//! The byte grammar is ported from the original app's project
//! decoder (`src/sticknodes/decoder/project.mjs` +
//! `project-layout.mjs`) — see `docs/NODES-ANIM-SPEC.md` for the
//! full spec with source references. Nothing here is guessed: every
//! field, gate and ordering traces to `parseProjectPayload` /
//! `parseMovieClipPayload` and the record-layout helpers.
//!
//! Both containers are parsed byte-exactly (same strictness as the
//! original: canonical flags, finite floats, validated references,
//! exact EOF). `.stknds` export is also provided
//! (`export_nodes_animation`, 431/117 — figures, frames, poses,
//! durations and model sprite tracks; texts/joins/clips written as
//! absent). Parsed
//! projects are mapped onto the model: figure instances become
//! project figure slots keyed by instance id, pose records become
//! model poses with the `layoutFigure` accumulation rules, and
//! single-figure movieclips become `ClipDef`s. Sprite library
//! images are extracted to PNG assets and sprite instances (main /
//! `.nodemc` timelines) become `SpriteInstance` key tracks;
//! main-frame texts become `TextInstance` key tracks. Main-frame
//! camera records are modelled as camera keys (zoom, position
//! and rotation — the fields the original adapter itself
//! imports; the records' wobble and binding blocks are consumed
//! but not modelled, mirroring the adapter, which retains them
//! as metadata). Sounds and filter blocks are consumed precisely
//! but not modelled — the same "stored poses preserved, dynamics
//! not recreated" stance the original adapter takes. Joins are the
//! exception: they are baked at import — a joined instance's
//! root is superseded by the evaluated end of its host's named
//! node (see `resolve_join_roots` and §17 of the spec).
//!
//! Figure nodes never gain `SegmentMedia` from this importer:
//! the figure grammar's node kinds are purely geometric (the
//! original's `NODE_TYPES` table — root plus eight shapes — has
//! no image or text kind, and node records carry no string or
//! image payload), so there is no node-attached media in the
//! source bytes to decode or to drop. Media enters a project
//! from these containers only as sprite-library images and
//! main-frame texts, both handled above; segment media in the
//! model comes from Pivot `.stk` figures and editor-attached
//! media instead. Imported asset indices are likewise always
//! relative to the asset table built by the same import —
//! `import_nodes_animation` returns a whole fresh project and
//! its callers replace, never merge — so no index offsetting
//! arises here.

use crate::model::{
    evaluate, norm_deg, CameraKey, ClipDef, ClipLoop, Easing, FigureDef, FigureInstance, Frame,
    ImageAsset, InstanceKey, NodeDef, Pose, Project, Shape, SpriteInstance, TextInstance,
};
use std::collections::HashMap;
use std::io::Read as _;

const MAX_INPUT: usize = 256 * 1024 * 1024; // PROJECT_LIMITS.input
const MAX_PAYLOAD: usize = 512 * 1024 * 1024; // PROJECT_LIMITS.output
const MAX_ASSETS: usize = 20_000;
const MAX_FRAMES: usize = 100_000;
const MAX_INSTANCES: usize = 20_000;
const MAX_NODES: usize = 10_000;
const MAX_DEPTH: usize = 1_024;
const DEFAULT_BG: u32 = 0x232830;
const STAGE: [f32; 2] = [960.0, 720.0];

// ---------- big-endian reader (mirrors the project BinaryReader) ----------

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Reader { data, pos: 0 }
    }
    fn at(data: &'a [u8], pos: usize) -> Self {
        Reader { data, pos }
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        if self.pos + n > self.data.len() {
            return Err(format!("unexpected end of data at {}", self.pos));
        }
        let out = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(out)
    }
    fn skip(&mut self, n: usize) -> Result<(), String> {
        self.take(n)?;
        Ok(())
    }
    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }
    fn i8(&mut self) -> Result<i8, String> {
        Ok(self.u8()? as i8)
    }
    fn i16(&mut self) -> Result<i16, String> {
        let b = self.take(2)?;
        Ok(i16::from_be_bytes([b[0], b[1]]))
    }
    fn i32(&mut self) -> Result<i32, String> {
        let b = self.take(4)?;
        Ok(i32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn f32(&mut self) -> Result<f32, String> {
        let v = f32::from_bits(self.i32()? as u32);
        if !v.is_finite() {
            return Err(format!("non-finite float at {}", self.pos - 4));
        }
        Ok(v)
    }
    /// Project-reader flag: the byte must be 0 or 1
    /// (`BinaryReader.flag` fails on anything else).
    fn flag(&mut self) -> Result<bool, String> {
        let v = self.u8()?;
        if v > 1 {
            return Err(format!("non-canonical bool {v} at {}", self.pos - 1));
        }
        Ok(v == 1)
    }
    /// Figure-reader flag (`parseNodes`' Reader): any nonzero byte
    /// is true.
    fn flag_lenient(&mut self) -> Result<bool, String> {
        Ok(self.u8()? != 0)
    }
    /// Four bytes in [alpha, blue, green, red] order, returned as
    /// 0xRRGGBB (alpha dropped, like the other importers).
    fn color(&mut self) -> Result<u32, String> {
        let b = self.take(4)?;
        let (_a, blue, green, red) = (b[0], b[1], b[2], b[3]);
        Ok(((red as u32) << 16) | ((green as u32) << 8) | blue as u32)
    }
    fn count(&mut self, max: usize) -> Result<usize, String> {
        let n = self.i32()?;
        if n < 0 || n as usize > max {
            return Err(format!("count {n} outside 0..={max}"));
        }
        Ok(n as usize)
    }
    fn string(&mut self) -> Result<String, String> {
        let n = self.count(1024 * 1024)?;
        let bytes = self.take(n)?;
        String::from_utf8(bytes.to_vec())
            .map_err(|_| format!("invalid UTF-8 string at {}", self.pos - n))
    }
}

// ---------- container unwrap (mirrors openProject) ----------

fn inflate_gzip(data: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let ok = flate2::read::GzDecoder::new(data)
        .read_to_end(&mut out)
        .is_ok();
    if ok && out.len() <= MAX_PAYLOAD {
        Some(out)
    } else {
        None
    }
}

fn unwrap_container(data: &[u8]) -> Result<Vec<u8>, String> {
    if data.len() >= 12 && data[..9] == [1, 2, 3, 4, 5, 6, 7, 8, 9] {
        if data[9] != 0x1F || data[10] != 0x8B {
            return Err("the 9-byte project prefix must be followed by gzip".into());
        }
        return inflate_gzip(&data[9..])
            .ok_or_else(|| "gzip decompression failed".to_string());
    }
    if data.len() >= 2 && data[0] == 0x1F && data[1] == 0x8B {
        return inflate_gzip(data).ok_or_else(|| "gzip decompression failed".to_string());
    }
    Ok(data.to_vec())
}

// ---------- embedded figure scan ----------
//
// Mirrors `parseNodes` in the original figure decoder (the same
// grammar `src/nodes.rs` implements for standalone files), but
// starting at an arbitrary offset and recording what the container
// layer needs: the consumed byte range, connector node indices and
// the definition fields used by pose conversion and by the fallback
// definition conversion.

struct ScanNode {
    parent: Option<usize>,
    connector: bool,
    kind: i8,
    id: i32,
    is_static: bool,
    local_angle: f32,
    length: f32,
    thickness: i32,
    scale: f32,
    use_segment_scale: bool,
    use_segment_color: bool,
    color: u32,
    num_polygon_vertices: i16,
    child_count: usize,
    child_connector_flags: Vec<i32>,
}

struct FigureScan {
    start: usize,
    end: usize,
    version: i32,
    build: i32,
    scale: f32,
    color: u32,
    nodes: Vec<ScanNode>,
}

fn scan_modern(version: i32, build: i32, b: i32) -> bool {
    version >= 403 && build >= b
}

#[allow(clippy::too_many_arguments)]
fn scan_node(
    r: &mut Reader,
    version: i32,
    build: i32,
    parent: Option<usize>,
    connector: bool,
    seq: usize,
) -> Result<ScanNode, String> {
    if seq >= MAX_NODES {
        return Err("node limit reached".into());
    }
    if connector {
        r.f32()?; // connector local_x
        r.f32()?; // connector local_y
        r.f32()?; // connector percent
        if build >= 44 {
            r.f32()?; // percent_default
        }
        r.f32()?; // connector value
        r.i32()?; // connector method
        r.flag_lenient()?; // connector reversed
        if build >= 65 {
            r.f32()?; // smart_stretch_ancestral_value
        }
    }
    let kind = r.i8()?;
    if !(-1..=7).contains(&kind) {
        return Err(format!("unknown node type {kind}"));
    }
    let id = r.i32()?;
    if !(0..=1_000_000).contains(&id) {
        return Err(format!("invalid node id {id}"));
    }
    if seq == 0 && kind != -1 {
        return Err("first record is not a root node".into());
    }
    if seq > 0 && kind == -1 {
        return Err("additional root node in hierarchy".into());
    }
    let is_static = r.flag_lenient()?;
    r.flag_lenient()?; // is_stretchy
    if scan_modern(version, build, 48) {
        r.flag_lenient()?; // is_floaty
    }
    if version >= 248 {
        r.flag_lenient()?; // is_smart_stretch
    }
    if version >= 252 {
        r.flag_lenient()?; // do_not_apply_smart_stretch
    }
    if scan_modern(version, build, 50) {
        r.flag_lenient()?; // smart_stretch_reset_impulse
    }
    let use_segment_color = r.flag_lenient()?;
    if version >= 256 {
        r.flag_lenient()?; // use_circle_outline
    }
    if scan_modern(version, build, 21) {
        r.flag_lenient()?; // circle_is_hollow
    }
    if version >= 176 {
        r.flag_lenient()?; // use_gradient
        r.flag_lenient()?; // reverse_gradient
    }
    if scan_modern(version, build, 20) {
        r.i16()?; // gradient_mode
    }
    let use_segment_scale = r.flag_lenient()?;
    r.f32()?; // local_x
    r.f32()?; // local_y
    let scale = r.f32()?;
    r.f32()?; // default_length
    let length = r.f32()?;
    r.i32()?; // default_thickness
    let thickness = r.i32()?;
    if version >= 320 {
        r.i32()?; // segment_curve_radius_and_default_curve_radius
    }
    if scan_modern(version, build, 20) {
        r.flag_lenient()?; // curve_circulization
    }
    if scan_modern(version, build, 21) {
        r.i16()?; // segment_curve_polyfill_precision
    }
    if version >= 256 {
        r.flag_lenient()?; // half_arc
        r.i16()?; // right_triangle_direction
    }
    if version >= 300 {
        r.flag_lenient()?; // triangle_upside_down
    }
    if scan_modern(version, build, 36) {
        if build < 64 {
            r.i32()?;
            r.i32()?;
        } else {
            r.f32()?;
            r.f32()?;
        }
        if build == 36 {
            r.i32()?; // reserved_build36_a
            r.i32()?; // reserved_build36_b
            r.flag_lenient()?; // use_trapezoid_thickness_start
            r.flag_lenient()?; // use_trapezoid_thickness_end
        }
    }
    if version >= 256 && build != 36 {
        r.f32()?; // trapezoid_top_thickness_ratio
    }
    if scan_modern(version, build, 36) {
        r.flag_lenient()?; // trapezoid_is_rounded_start
        r.flag_lenient()?; // trapezoid_is_rounded_end
    }
    let mut num_polygon_vertices: i16 = 5;
    if version >= 256 {
        num_polygon_vertices = r.i16()?;
    }
    if version >= 248 {
        r.f32()?; // default_local_angle
    }
    let local_angle = r.f32()?;
    if version >= 248 {
        r.f32()?; // default_angle
    }
    let color = r.color()?;
    if version >= 176 {
        r.color()?; // gradient_color
    }
    if version >= 256 {
        r.color()?; // circle_outline_color
    }
    if scan_modern(version, build, 39) {
        r.flag_lenient()?; // is_angle_locked
        if build <= 50 {
            r.f32()?; // reserved_angle_lock_39_50
        }
    }
    if scan_modern(version, build, 51) {
        r.flag_lenient()?; // angle_lock_is_main_node
        if build <= 56 {
            r.f32()?; // angle_lock_offset_minuend
            r.f32()?; // angle_lock_offset_subtrahend
        }
    }
    if scan_modern(version, build, 57) {
        r.f32()?; // angle_lock_offset
    }
    if scan_modern(version, build, 63) {
        r.f32()?; // angle_lock_relative_start
    }
    if scan_modern(version, build, 67) {
        r.f32()?; // angle_lock_stickfigure_start
    }
    if scan_modern(version, build, 63) {
        r.i8()?; // angle_lock_relative_multiplier
    }
    if scan_modern(version, build, 39) {
        if build <= 40 {
            r.i16()?; // is_drag_locked (i16 in old builds)
        } else {
            r.flag_lenient()?; // is_drag_locked
        }
    }
    if scan_modern(version, build, 41) && build <= 45 {
        r.i16()?; // reserved_drag_lock_41_45
    }
    if scan_modern(version, build, 46) {
        r.f32()?; // drag_lock_angle
    }
    if scan_modern(version, build, 41) {
        r.f32()?; // smart_stretch_multiplier
    }
    if scan_modern(version, build, 41) && build <= 45 {
        r.u8()?; // reserved_smart_stretch_41_45
    }
    let child_count = r.count(MAX_NODES)?;
    let mut child_connector_flags = Vec::new();
    if scan_modern(version, build, 38) {
        for _ in 0..child_count {
            child_connector_flags.push(r.i32()?);
        }
    }
    Ok(ScanNode {
        parent,
        connector,
        kind,
        id,
        is_static,
        local_angle,
        length,
        thickness,
        scale,
        use_segment_scale,
        use_segment_color,
        color,
        num_polygon_vertices,
        child_count,
        child_connector_flags,
    })
}

fn scan_figure(data: &[u8], start: usize) -> Result<FigureScan, String> {
    let mut r = Reader::at(data, start);
    let version = r.i32()?;
    if version < 1 {
        return Err(format!("invalid figure version {version}"));
    }
    let build = if version >= 403 { r.i32()? } else { 0 };
    if build < 0 {
        return Err(format!("invalid figure build {build}"));
    }
    let scale = r.f32()?;
    let color = r.color()?;

    let mut nodes: Vec<ScanNode> = Vec::new();
    nodes.push(scan_node(&mut r, version, build, None, false, 0)?);
    let mut stack: Vec<(usize, usize, usize)> = vec![(0, 0, 0)];
    while !stack.is_empty() {
        let (parent_idx, next, depth) = {
            let top = stack.last_mut().unwrap();
            let state = (top.0, top.1, top.2);
            top.1 += 1;
            state
        };
        if next >= nodes[parent_idx].child_count {
            stack.pop();
            continue;
        }
        if depth + 1 > MAX_DEPTH {
            return Err("hierarchy too deep".into());
        }
        let connector = nodes[parent_idx]
            .child_connector_flags
            .get(next)
            .copied()
            .unwrap_or(0)
            != 0;
        let seq = nodes.len();
        let child = scan_node(&mut r, version, build, Some(parent_idx), connector, seq)?;
        nodes.push(child);
        stack.push((nodes.len() - 1, 0, depth + 1));
    }
    let mut ids = std::collections::HashSet::new();
    for n in &nodes {
        if !ids.insert(n.id) {
            return Err(format!("duplicate node id {}", n.id));
        }
    }
    // Polyfills (version >= 230): consumed, not modelled.
    if version >= 230 {
        let count = r.count(MAX_NODES)?;
        for _ in 0..count {
            r.i32()?; // anchorId
            r.color()?;
            r.flag_lenient()?; // use_polyfill_color
            let attached = r.count(MAX_NODES)?;
            for _ in 0..attached {
                r.i32()?; // attachedId
            }
        }
    }
    // Connections (modern(38)): consumed, not modelled.
    if scan_modern(version, build, 38) {
        let count = r.count(MAX_NODES)?;
        for _ in 0..count {
            r.i32()?; // sourceId
            r.i32()?; // targetId
        }
    }
    Ok(FigureScan {
        start,
        end: r.pos,
        version,
        build,
        scale,
        color,
        nodes,
    })
}

fn scan_kind_name(kind: i8) -> &'static str {
    match kind {
        -1 => "root",
        0 => "rounded-segment",
        1 => "segment",
        2 => "circle",
        3 => "triangle",
        4 => "filled-circle",
        5 => "ellipse",
        6 => "trapezoid",
        _ => "polygon",
    }
}

/// Definition conversion replicating `nodes::convert` field for
/// field. Used only for embedded figures older than version 160,
/// whose bytes the standalone `.nodes` unwrap path (which recognises
/// raw figures from version 160 up) cannot be reused for; a test
/// pins this fallback to the primary path on a modern figure.
fn convert_fallback(scan: &FigureScan) -> (FigureDef, u32) {
    let g = scan.scale;
    let n = scan.nodes.len();
    let mut theta = vec![0.0f32; n];
    let mut eff_len = vec![0.0f32; n];
    let mut eff_thick = vec![0.0f32; n];
    for (i, node) in scan.nodes.iter().enumerate() {
        let ns = if node.use_segment_scale { node.scale } else { 1.0 };
        let raw_len = node.length * ns * g;
        eff_len[i] = raw_len.abs();
        eff_thick[i] = (node.thickness as f32).abs() * (ns * g).abs();
        let flip = if raw_len < 0.0 { 180.0 } else { 0.0 };
        if i == 0 {
            let local = if scan.build < 36 { 0.0 } else { node.local_angle };
            theta[i] = local + flip;
        } else {
            theta[i] = theta[node.parent.unwrap()] + node.local_angle + flip;
        }
    }
    let mut nodes: Vec<NodeDef> = Vec::with_capacity(n + 1);
    nodes.push(NodeDef {
        min_angle: None,
        max_angle: None,
        style: crate::model::NodeStyle::default(),
        media: None,
        name: "origin".into(),
        parent: None,
        length: 0.0,
        rest_angle: 0.0,
        head: false,
        bend: 1.0,
        thickness: 5.0,
        color: None,
        ik_end: false,
        shape: Shape::Line,
    });
    for (i, node) in scan.nodes.iter().enumerate() {
        let rest = if i == 0 {
            norm_deg(theta[0] + 90.0)
        } else {
            norm_deg(theta[i] - theta[node.parent.unwrap()])
        };
        let shape = match node.kind {
            2 | 4 => Shape::Circle,
            3 => Shape::Polygon(3),
            7 => Shape::Polygon(node.num_polygon_vertices.clamp(3, 255) as u8),
            5 => Shape::Circle, // ellipse: approximated by a circle
            _ => Shape::Line,
        };
        nodes.push(NodeDef {
            min_angle: None,
            max_angle: None,
            style: crate::model::NodeStyle::default(),
            media: None,
            name: format!("{} {}", scan_kind_name(node.kind), node.id),
            parent: Some(node.parent.map(|p| p + 1).unwrap_or(0)),
            length: if i == 0 { 0.0 } else { eff_len[i] },
            rest_angle: rest,
            head: false,
            bend: 1.0,
            thickness: eff_thick[i],
            color: if node.use_segment_color && i > 0 {
                Some(node.color)
            } else {
                None
            },
            ik_end: false,
            shape,
        });
    }
    (FigureDef { nodes }, scan.color)
}

/// The definition for an embedded figure: the established `.nodes`
/// conversion (`import_nodes_figure`) applied to the figure's byte
/// range whenever it succeeds, with the local fallback above for
/// pre-160 figures.
fn figure_def(payload: &[u8], scan: &FigureScan) -> (FigureDef, u32) {
    if scan.version >= 160 {
        if let Ok(pair) = crate::nodes::import_nodes_figure(&payload[scan.start..]) {
            if pair.0.nodes.len() == scan.nodes.len() + 1 {
                return pair;
            }
        }
    }
    convert_fallback(scan)
}

// ---------- pose records (compilePoseLayout port) ----------

#[derive(Clone, Copy, PartialEq)]
enum PKind {
    Flag,
    I8,
    I16,
    I32,
    F32,
    Color,
}

struct PoseLayout {
    fields: Vec<(PKind, &'static str)>,
}

fn compile_pose_layout(v: i32, b: i32, connector: bool) -> PoseLayout {
    let mut fields: Vec<(PKind, &'static str)> = Vec::new();
    let mut a = |name: &'static str, kind: PKind, when: bool| {
        if when {
            fields.push((kind, name));
        }
    };
    a("is_stretchy", PKind::Flag, true);
    a("is_floaty", PKind::Flag, b >= 48);
    a("is_smart_stretch", PKind::Flag, v >= 248 && b < 22);
    a("use_segment_color", PKind::Flag, true);
    a("use_circle_outline", PKind::Flag, v >= 256);
    a("circle_is_hollow", PKind::Flag, b >= 21);
    a("use_gradient", PKind::Flag, v >= 176);
    a("reverse_gradient", PKind::Flag, v >= 176);
    a("gradient_mode", PKind::I16, b >= 20);
    a("use_segment_scale", PKind::Flag, true);
    a("scale", PKind::F32, true);
    a("length", PKind::F32, true);
    a("thickness", PKind::I32, true);
    a("local_angle", PKind::F32, true);
    a("default_local_angle", PKind::F32, v >= 259);
    a("default_angle", PKind::F32, v >= 259);
    a("color", PKind::Color, true);
    a("gradient_color", PKind::Color, v >= 176);
    a("circle_outline_color", PKind::Color, v >= 256);
    a("use_polyfill_color", PKind::Flag, v >= 230);
    a("polyfill_color", PKind::Color, v >= 230);
    a("right_triangle_direction", PKind::I16, v >= 259);
    a("segment_curve_radius_and_default_curve_radius", PKind::I32, v >= 320);
    a("curve_circulization", PKind::Flag, b >= 20);
    a("segment_curve_polyfill_precision", PKind::I16, b >= 21);
    if v >= 320 {
        if b < 36 {
            a("trapezoid_top_thickness_ratio", PKind::F32, true);
        } else {
            let tk = if b < 64 { PKind::I32 } else { PKind::F32 };
            a("trapezoid_thickness_start", tk, true);
            a("trapezoid_thickness_end", tk, true);
            if b == 36 {
                a("use_trapezoid_thickness_start", PKind::Flag, true);
                a("use_trapezoid_thickness_end", PKind::Flag, true);
            } else {
                a("trapezoid_is_rounded_start", PKind::Flag, true);
                a("trapezoid_is_rounded_end", PKind::Flag, true);
                a("trapezoid_top_thickness_ratio", PKind::F32, true);
            }
        }
    }
    if b >= 41 {
        if b <= 50 {
            a("is_angle_locked", PKind::Flag, true);
            a("legacy_angle_lock_offset", PKind::F32, true);
        } else if b <= 56 {
            a("is_angle_locked", PKind::Flag, true);
            a("angle_lock_is_main_node", PKind::Flag, true);
            a("angle_lock_offset_minuend", PKind::F32, true);
            a("angle_lock_offset_subtrahend", PKind::F32, true);
        } else if b <= 66 {
            a("legacy_angle_locked", PKind::Flag, true);
            a("legacy_angle_main", PKind::Flag, true);
            a("angle_lock_offset", PKind::F32, true);
            a("legacy_angle_relative_start", PKind::F32, b >= 63);
            a("legacy_angle_relative_multiplier", PKind::I8, b >= 63);
        } else {
            a("angle_lock_offset", PKind::F32, true);
        }
        a("is_drag_locked", PKind::Flag, true);
        if b <= 45 {
            a("legacy_drag_lock_mode", PKind::I16, true);
        } else {
            a("drag_lock_angle", PKind::F32, true);
        }
    }
    if connector && b >= 43 {
        a("connector_local_x", PKind::F32, true);
        a("connector_local_y", PKind::F32, true);
        a("connector_value", PKind::F32, true);
        a("connector_percent", PKind::F32, true);
    }
    if connector && b >= 65 {
        a("connector_smart_stretch_ancestral_value", PKind::F32, true);
    }
    PoseLayout { fields }
}

#[derive(Clone, Copy, Default)]
struct PoseRec {
    local_angle: f32,
    length: f32,
    scale: f32,
    use_segment_scale: bool,
}

fn read_pose_record(r: &mut Reader, layout: &PoseLayout) -> Result<PoseRec, String> {
    let mut rec = PoseRec::default();
    for (kind, name) in &layout.fields {
        match kind {
            PKind::Flag => {
                // skipPoseRecords validation: pose flags must be 0/1.
                let byte = r.u8()?;
                if byte > 1 {
                    return Err(format!("pose flag {name} is not a boolean at {}", r.pos - 1));
                }
                if *name == "use_segment_scale" {
                    rec.use_segment_scale = byte != 0;
                }
            }
            PKind::I8 => {
                r.i8()?;
            }
            PKind::I16 => {
                r.i16()?;
            }
            PKind::I32 => {
                r.i32()?;
            }
            PKind::F32 => {
                let val = r.f32()?;
                match *name {
                    "local_angle" => rec.local_angle = val,
                    "length" => rec.length = val,
                    "scale" => rec.scale = val,
                    _ => {}
                }
            }
            PKind::Color => {
                r.color()?;
            }
        }
    }
    Ok(rec)
}

// ---------- raw container records ----------

struct LibFigure {
    name: String,
    scan: FigureScan,
    def: FigureDef,
    /// Library figure colour; placed instances override it with
    /// their own colour (see `posedFigure`), so this is retained
    /// for parity with the decoded document only.
    #[allow(dead_code)]
    color: u32,
}

/// One sprite library entry. Group members are resolved to local
/// library indices of *image* entries at parse time (for `.nodemc`
/// the file stores source-library ids; the resolution mirrors the
/// original decoder, which also fails on dangling references).
#[derive(Clone)]
enum SpriteLib {
    Image(SpriteImage),
    Group { members: Vec<usize> },
}

impl SpriteLib {
    fn kind(&self) -> u8 {
        match self {
            SpriteLib::Image(_) => 0,
            SpriteLib::Group { .. } => 1,
        }
    }
}

/// A sprite library image: raw pixels (RGB or RGBA, row-major,
/// top-down — the layout the original passes straight to its PNG
/// encoder).
#[derive(Clone)]
struct SpriteImage {
    name: String,
    width: u32,
    height: u32,
    alpha: bool,
    pixels: Vec<u8>,
}

struct RawInstance {
    ty: i32,
    asset: usize,
    id: i32,
    scale: f32,
    x: f32,
    y: f32,
    color: u32,
    poses: Vec<PoseRec>,
    /// The style block's opacity (v >= 220; 1.0 before).
    /// Modelled only on sprite keys (Wave X, P-6) — figures
    /// carry their own opacity and movieclips have no key.
    opacity: f32,
    // Sprite instances (type 2/3) only; defaults elsewhere.
    scale_x: f32,
    scale_y: f32,
    rotation: f32,
    flip_x: bool,
    flip_y: bool,
    pivot_x: f32,
    pivot_y: f32,
    group_frame: i32,
}

/// A main-frame text record (`readProjectText`). `id` is the stored
/// id when the container version has one (v >= 176); before that it
/// is the text's index within its frame — the same fallback the
/// original decoder's record reader applies.
struct RawText {
    id: i32,
    x: f32,
    y: f32,
    scale: f32,
    content: String,
    color: u32,
    align: i32,
    rotation: f32,
}

struct MainSettings {
    bg: u32,
    is_tweened: bool,
    is_delayed: bool,
    delay_seconds: f32,
    slow_motion: bool,
    slow_motion_tween_frames: i32,
    sound_index: i32,
}

/// One join record (`readJoins`): instance `child` (an index
/// into its frame's instance list) is pinned to node `node_id`
/// of the instance `parent` (also a frame instance index — the
/// file stores the parent's instance *id*; the parser resolves
/// it, mirroring the original decoder).
struct RawJoin {
    child: usize,
    parent: usize,
    node_id: i32,
    /// Stored rotation of the child relative to the parent node
    /// (build >= 53). Read for parity with the decoded document
    /// but never applied: the original adapter ignores it too —
    /// a child's orientation comes from its own stored pose
    /// records (see §17 of the spec).
    #[allow(dead_code)]
    relative_rotation: Option<f32>,
}

struct RawFrame {
    instances: Vec<RawInstance>,
    joins: Vec<RawJoin>,
    settings: Option<MainSettings>,
    /// Main-frame texts; always empty for clip frames (the format
    /// stores texts only in `.stknds` main frames).
    texts: Vec<RawText>,
    /// Main-frame camera record; `None` for clip frames (the
    /// format stores camera records only in `.stknds` main
    /// frames).
    camera: Option<RawCamera>,
}

/// A parsed library movieclip. Only name/frames feed the model
/// (ClipDef carries no timing of its own); fps is validated at
/// parse time and the tween fields are retained for parity with
/// the decoded document.
#[allow(dead_code)]
struct RawClip {
    name: String,
    fps: i32,
    tween_count: i32,
    auto_tween: bool,
    frames: Vec<RawFrame>,
}

/// The instance style block (`readStyle`): consumed, not
/// modelled — except its opacity (v >= 220), which is returned:
/// sprite keys carry it (Wave X, P-6). 1.0 before v220, where
/// the block has no opacity field.
fn read_style(r: &mut Reader, v: i32, b: i32) -> Result<f32, String> {
    let mut opacity = 1.0f32;
    if v >= 220 {
        opacity = r.f32()?; // opacity
        r.f32()?; // blur
    }
    if b >= 16 {
        r.flag()?; // motionBlur
        r.flag()?; // omnidirectionalBlur
        r.i32()?; // motionBlurAngle
    } else if v >= 220 {
        r.flag()?; // legacyHorizontalBlur
        r.flag()?; // legacyVerticalBlur
    }
    if v >= 220 {
        r.f32()?; // saturation
    }
    if b >= 16 {
        r.i32()?; // pixelation
    }
    if v >= 220 {
        r.color()?; // tintColor
        r.f32()?; // tintAmount
    }
    if v >= 303 {
        r.f32()?; // hueShift
    }
    if v >= 220 {
        r.flag()?; // invert
    }
    if v >= 230 {
        r.f32()?; // glow
        r.color()?; // glowColor
    }
    if v >= 242 {
        r.f32()?; // glowIntensity
    }
    if b >= 32 {
        r.color()?; // outlineColor
        r.f32()?; // outlineAlpha
        r.f32()?; // outlineThickness
    }
    if v >= 330 {
        r.color()?; // dropShadowColor
        r.f32()?; // dropShadowAlpha
        r.i32()?; // dropShadowDistance
        r.i32()?; // dropShadowAngle
        r.f32()?; // dropShadowBlur
    }
    Ok(opacity)
}

#[allow(clippy::too_many_arguments)]
fn read_instance(
    r: &mut Reader,
    v: i32,
    b: i32,
    clip_ctx: bool,
    figures: &[LibFigure],
    sprites: &[SpriteLib],
    expected_clips: usize,
    layouts: &(PoseLayout, PoseLayout),
) -> Result<RawInstance, String> {
    let ty = if v >= (if clip_ctx { 400 } else { 300 }) {
        r.i32()?
    } else {
        0
    };
    if !(0..=3).contains(&ty) || (clip_ctx && ty == 1) {
        return Err(format!("unsupported instance type {ty}"));
    }
    let asset_i = r.i32()?;
    if asset_i < 0 {
        return Err(format!("negative asset index {asset_i}"));
    }
    let asset = asset_i as usize;
    match ty {
        0 if asset >= figures.len() => {
            return Err(format!("figure asset {asset} does not exist"));
        }
        1 if asset >= expected_clips => {
            return Err(format!("movieclip asset {asset} does not exist"));
        }
        2 if sprites.get(asset).map(|s| s.kind()) != Some(0) => {
            return Err(format!("sprite asset {asset} is not an image"));
        }
        3 if sprites.get(asset).map(|s| s.kind()) != Some(1) => {
            return Err(format!("sprite asset {asset} is not a group"));
        }
        _ => {}
    }
    let id = r.i32()?;
    let mut scale = 1.0f32;
    if ty == 0 {
        scale = r.f32()?;
    }
    let opacity = read_style(r, v, b)?;
    let (mut x, mut y, mut color) = (0.0f32, 0.0f32, 0xFFFFFFu32);
    let mut poses: Vec<PoseRec> = Vec::new();
    // Sprite-instance fields (type 2/3); defaults for other types.
    let (mut scale_x, mut rotation, mut group_frame) = (1.0f32, 0.0f32, 0i32);
    let (mut scale_y, mut flip_x, mut flip_y) = (1.0f32, false, false);
    let (mut pivot_x, mut pivot_y) = (0.5f32, 0.5f32);
    match ty {
        0 => {
            x = r.f32()?;
            y = r.f32()?;
            color = r.color()?;
            if v >= 126 {
                r.flag()?; // isLocked
            }
            if v >= 180 {
                r.flag()?; // persistent
            }
            if b >= 15 {
                r.flag()?; // visibleInOnionSkin
            }
            if v >= 230 {
                r.flag()?; // tweenColors
                if b >= 78 {
                    r.i8()?; // tweenMode
                } else {
                    r.flag()?; // tweenMode
                }
            }
            if v >= 244 {
                r.flag()?; // scaleJoinedFigures
            }
            if v >= 400 {
                r.flag()?; // pushJoinedFigures
                r.flag()?; // setStateOfJoins
                r.i32()?; // joinsState
            }
            if v >= 310 {
                r.flag()?; // useParentFilters
            }
            if v >= 248 {
                r.i32()?; // lockedNodeId
                r.f32()?; // lockedNodeX
                r.f32()?; // lockedNodeY
            }
            let scan = &figures[asset].scan;
            for node in &scan.nodes {
                let layout = if node.connector { &layouts.1 } else { &layouts.0 };
                poses.push(read_pose_record(r, layout)?);
            }
        }
        1 => {
            if v >= 310 {
                r.flag()?; // useParentFilters
            }
            scale = r.f32()?;
            x = r.f32()?;
            y = r.f32()?;
            r.i32()?; // currentFrameIndex
            r.i32()?; // startFrameIndex
            r.i32()?; // flipX
            r.i32()?; // flipY
            r.i32()?; // playMode
            r.f32()?; // volumeScale
            r.flag()?; // playDuringDelay
            r.flag()?; // isLocked
            r.flag()?; // persistent
            if b >= 78 {
                r.i8()?; // tweenMode
            }
            if b >= 15 {
                r.flag()?; // visibleInOnionSkin
            }
            r.f32()?; // rotation
            r.f32()?; // secondaryRotation
        }
        _ => {
            // Sprite image (2) / sprite group (3) instance.
            r.flag()?; // useParentFilters (always present)
            if b < 2 {
                r.u8()?; // legacySpriteByte0
                r.u8()?; // legacySpriteByte1
            }
            r.f32()?; // scaleXYRatio (editor value; not used by the
                       // original adapter either)
            scale_x = r.f32()?;
            scale_y = r.f32()?; // scaleY (modelled on the key since
                                // Wave X, P-6 — §17's discard is over)
            if b >= 110 {
                for _ in 0..8 {
                    r.f32()?; // corner distortion Tl/Tr/Bl/Br
                }
            }
            pivot_x = r.f32()?; // pivotX (modelled on the key, P-6)
            pivot_y = r.f32()?; // pivotY
            x = r.f32()?;
            y = r.f32()?;
            rotation = r.f32()?;
            flip_x = r.flag()?; // flipX (modelled on the key, P-6)
            flip_y = r.flag()?; // flipY
            r.flag()?; // persistent
            if b >= 78 {
                r.i8()?; // tweenMode
            }
            if b >= 15 {
                r.flag()?; // visibleInOnionSkin
            }
            r.i32()?; // lockState
            let smart_stretch_state = r.i32()?;
            if smart_stretch_state != 0 {
                for _ in 0..9 {
                    r.f32()?; // smartStretchParameter[i]
                }
            }
            if ty == 3 {
                group_frame = r.i32()?;
            }
        }
    }
    Ok(RawInstance {
        ty,
        asset,
        id,
        scale,
        x,
        y,
        color,
        poses,
        opacity,
        scale_x,
        scale_y,
        rotation,
        flip_x,
        flip_y,
        pivot_x,
        pivot_y,
        group_frame,
    })
}

/// Join records (`readJoins`), with the original decoder's
/// cross-record validation: instance ids in the frame must be
/// non-negative and unique (checked whether or not any join is
/// present — the original runs this inside `readJoins` too), a
/// join's child index and parent id must resolve, an instance
/// may have at most one join parent, a figure parent must own
/// the named node, and join parent chains must be acyclic.
fn read_joins(
    r: &mut Reader,
    b: i32,
    present: bool,
    instances: &[RawInstance],
    figures: &[LibFigure],
) -> Result<Vec<RawJoin>, String> {
    let mut by_id: std::collections::HashMap<i32, usize> = std::collections::HashMap::new();
    for (k, inst) in instances.iter().enumerate() {
        if inst.id < 0 {
            return Err(format!("negative instance id {}", inst.id));
        }
        if by_id.insert(inst.id, k).is_some() {
            return Err(format!("instance id {} appears twice in one frame", inst.id));
        }
    }
    if !present {
        return Ok(Vec::new());
    }
    let n = r.count(MAX_INSTANCES)?;
    let mut joins = Vec::with_capacity(n);
    let mut children = std::collections::HashSet::new();
    for _ in 0..n {
        let child_i = r.i32()?;
        let parent_id = r.i32()?;
        let node_id = r.i32()?;
        let relative_rotation = if b >= 53 { Some(r.f32()?) } else { None };
        if child_i < 0 || child_i as usize >= instances.len() {
            return Err(format!("join child index {child_i} is absent"));
        }
        let child = child_i as usize;
        let Some(&parent) = by_id.get(&parent_id) else {
            return Err(format!("join parent id {parent_id} is absent"));
        };
        if !children.insert(child) {
            return Err("an instance cannot have multiple join parents".into());
        }
        let pi = &instances[parent];
        if pi.ty == 0
            && !figures[pi.asset]
                .scan
                .nodes
                .iter()
                .any(|nd| nd.id == node_id)
        {
            return Err(format!("join target node {node_id} is absent"));
        }
        joins.push(RawJoin {
            child,
            parent,
            node_id,
            relative_rotation,
        });
    }
    // A cyclic parent chain is not a valid rig (the original
    // rejects it rather than letting it hang rendering).
    let parents: std::collections::HashMap<usize, usize> =
        joins.iter().map(|j| (j.child, j.parent)).collect();
    let mut finished = std::collections::HashSet::new();
    for &k in parents.keys() {
        let mut visited = std::collections::HashSet::new();
        let mut x = k;
        while parents.contains_key(&x) && !finished.contains(&x) {
            if !visited.insert(x) {
                return Err("cycle in instance joins".into());
            }
            x = parents[&x];
        }
        finished.extend(visited);
    }
    Ok(joins)
}

#[allow(clippy::too_many_arguments)]
fn read_clip_frame(
    r: &mut Reader,
    v: i32,
    b: i32,
    figures: &[LibFigure],
    sprites: &[SpriteLib],
    expected_clips: usize,
    sounds_len: usize,
    layouts: &(PoseLayout, PoseLayout),
) -> Result<RawFrame, String> {
    r.f32()?; // volume
    if v >= 310 {
        r.f32()?; // pan
        r.f32()?; // pitch
    }
    let sound_index = r.i32()?;
    if sound_index < -1 || sound_index >= sounds_len as i32 {
        return Err(format!("sound index {sound_index} is absent"));
    }
    let n = r.count(MAX_INSTANCES)?;
    let mut instances = Vec::new();
    for _ in 0..n {
        instances.push(read_instance(r, v, b, true, figures, sprites, expected_clips, layouts)?);
    }
    let joins = read_joins(r, b, true, &instances, figures)?;
    Ok(RawFrame {
        instances,
        joins,
        settings: None,
        texts: Vec::new(),
        camera: None,
    })
}

fn read_main_settings(r: &mut Reader, v: i32) -> Result<MainSettings, String> {
    let is_tweened = r.flag()?;
    if v >= 140 {
        r.flag()?; // useImageBackground
    }
    if v >= 200 {
        r.flag()?; // useGradient
        r.flag()?; // expandedGradient
    }
    if v >= 160 {
        r.flag()?; // stopSounds
    }
    let is_delayed = if v >= 200 { r.flag()? } else { false };
    let delay_seconds = if v >= 200 { r.f32()? } else { 0.0 };
    let slow_motion = if v >= 310 { r.flag()? } else { false };
    let slow_motion_tween_frames = if v >= 310 { r.i32()? } else { 0 };
    if v >= 330 {
        r.flag()?; // repeating
        r.i32()?; // repeatBackFrames
        r.i32()?; // repeatLoops
        r.flag()?; // tweenWhenRepeating
    }
    if v >= 310 {
        r.flag()?; // cameraDuringDelay
        r.flag()?; // easeIn
        r.flag()?; // easeOut
        r.i16()?; // interpolation
    }
    let bg = r.color()?;
    if v >= 200 {
        r.color()?; // backgroundGradientColor
    }
    let sound_index = if v >= 160 { r.i32()? } else { -1 };
    if v >= 160 {
        r.f32()?; // soundVolume
    }
    if v >= 310 {
        r.f32()?; // soundPan
        r.f32()?; // soundPitch
    }
    Ok(MainSettings {
        bg,
        is_tweened,
        is_delayed,
        delay_seconds,
        slow_motion,
        slow_motion_tween_frames,
        sound_index,
    })
}

/// Main-frame text record (`readProjectText`). The stored id is
/// read when the version has one (v >= 176); before that, the
/// frame-local `index` is the id (the original record reader's
/// default for the field).
fn read_text(r: &mut Reader, v: i32, index: i32) -> Result<RawText, String> {
    let id = if v >= 176 { r.i32()? } else { index };
    if v >= 244 {
        r.i32()?; // fontIndex (font table not modelled, §17)
    }
    let x = r.f32()?;
    let y = r.f32()?;
    let scale = r.f32()?;
    let content = r.string()?;
    let color = r.color()?;
    let align = r.i32()?;
    let mut rotation = 0.0f32;
    if v >= 237 {
        rotation = r.f32()?;
        r.f32()?; // opacity (TextInstance has no opacity field)
    }
    if v >= 310 {
        r.flag()?; // aboveBars
        r.flag()?; // isLocked
    }
    if v >= 242 {
        r.flag()?; // hasShadow
        r.color()?; // shadowColor
    }
    Ok(RawText {
        id,
        x,
        y,
        scale,
        content,
        color,
        align,
        rotation,
    })
}

/// One frame's camera record (`readCamera`), values only. The
/// wobble and binding blocks are consumed by the reader but not
/// modelled — the original adapter takes the same stance (it
/// imports camera position, zoom and rotation, and retains
/// wobble and bindings as metadata, their parameter meanings
/// unestablished).
#[derive(Clone, Copy)]
struct RawCamera {
    zoom: f32,
    x: f32,
    y: f32,
    rotation: f32,
}

impl RawCamera {
    /// The decoder's own fallback record: zoom 1, no offset, no
    /// rotation (what an untouched camera reads as at any
    /// version, and what older versions carry no fields for).
    fn fallback() -> RawCamera {
        RawCamera {
            zoom: 1.0,
            x: 0.0,
            y: 0.0,
            rotation: 0.0,
        }
    }

    fn is_default(&self) -> bool {
        self.zoom == 1.0 && self.x == 0.0 && self.y == 0.0 && self.rotation == 0.0
    }
}

/// Frame camera record (`readCamera`): returns the record's
/// zoom / position / rotation; wobble and bindings are consumed
/// and dropped (see [`RawCamera`]).
fn read_camera(r: &mut Reader, v: i32) -> Result<RawCamera, String> {
    let mut cam = RawCamera::fallback();
    if v >= 170 {
        cam.zoom = r.f32()?;
        cam.x = r.f32()?;
        cam.y = r.f32()?;
    }
    if v >= 230 {
        cam.rotation = r.f32()?;
    }
    if v >= 310 {
        r.flag()?; // wobbleXY
        r.flag()?; // wobbleRotation
        r.flag()?; // wobbleScale
        r.i32()?; // wobbleXYIntensity
        r.f32()?; // wobbleRotationIntensity
        r.f32()?; // wobbleSpeed
    } else if v >= 230 {
        r.flag()?; // legacyWobble
        r.f32()?; // legacyWobbleIntensity
        r.f32()?; // wobbleSpeed
    }
    if v >= 220 {
        r.flag()?; // widescreen
    }
    if v >= 220 && v < 244 {
        r.flag()?; // legacyFlag
    }
    if v >= 237 {
        let n = r.count(MAX_ASSETS)?;
        for _ in 0..n {
            r.i32()?; // instanceId
            r.i32()?; // nodeId
            if v >= 300 {
                r.i32()?; // kind
            }
            for _ in 0..9 {
                r.f32()?; // parameter[j]
            }
            r.flag()?; // enabled
        }
    }
    if v >= 310 {
        let n = r.count(MAX_ASSETS)?;
        for _ in 0..n {
            r.i32()?; // instanceId
            for _ in 0..8 {
                r.f32()?; // parameter[j]
            }
        }
    }
    Ok(cam)
}

#[allow(clippy::too_many_arguments)]
fn read_main_frame(
    r: &mut Reader,
    v: i32,
    b: i32,
    figures: &[LibFigure],
    sprites: &[SpriteLib],
    clips_len: usize,
    sounds_len: usize,
    layouts: &(PoseLayout, PoseLayout),
) -> Result<RawFrame, String> {
    let settings = read_main_settings(r, v)?;
    if settings.sound_index < -1 || settings.sound_index >= sounds_len as i32 {
        return Err(format!("sound index {} is absent", settings.sound_index));
    }
    let n = r.count(MAX_INSTANCES)?;
    let mut instances = Vec::new();
    for _ in 0..n {
        instances.push(read_instance(r, v, b, false, figures, sprites, clips_len, layouts)?);
    }
    let joins = read_joins(r, b, v >= 174, &instances, figures)?;
    let mut texts = Vec::new();
    if v >= 150 {
        let nt = r.count(10_000)?;
        for i in 0..nt {
            texts.push(read_text(r, v, i as i32)?);
        }
    }
    let camera = read_camera(r, v)?;
    Ok(RawFrame {
        instances,
        joins,
        settings: Some(settings),
        texts,
        camera: Some(camera),
    })
}

/// Project header (`readProjectHeader`): returns (fps, tweenCount,
/// autoTween); everything else is editor metadata, consumed only.
fn read_project_header(r: &mut Reader, v: i32, b: i32) -> Result<(i32, i32, bool), String> {
    if b >= 16 {
        r.flag()?; // editorFlagD
    }
    if v >= 220 {
        if v >= 244 {
            r.flag()?; // editorFlagE
        }
        r.string()?; // projectCredit
        r.color()?; // editorColor
    }
    let fps = r.i32()?;
    let tween_count = if v >= 176 { r.i32()? } else { 2 };
    if v >= 127 {
        r.flag()?; // editorFlagB
    }
    if v >= 113 {
        r.flag()?; // editorFlagZ
    }
    r.flag()?; // editorFlagA
    if v >= 220 {
        r.flag()?; // editorFlagC
    }
    let auto_tween = r.flag()?;
    if v >= 127 {
        r.flag()?; // timelineFlagC
    }
    if v >= 140 && v < 200 {
        r.flag()?; // legacyFlag
    }
    r.i32()?; // selectedFrame
    if v >= 176 {
        r.i32()?; // nextTextId
    }
    if v >= 300 {
        r.i32()?; // nextInstanceId
        r.i32()?; // nextLibraryId
    }
    if v >= 400 {
        let n = r.count(10_000)?;
        for _ in 0..n {
            r.flag()?; // fontUsage
        }
    }
    let lists = if v >= 400 {
        3
    } else if v >= 300 {
        2
    } else {
        0
    };
    for _ in 0..lists {
        let n = r.count(MAX_ASSETS)?;
        for _ in 0..n {
            r.i32()?;
        }
    }
    if v >= 300 {
        let outer = r.count(MAX_ASSETS)?;
        for _ in 0..outer {
            r.i32()?; // key
            let inner = r.count(MAX_ASSETS)?;
            for _ in 0..inner {
                r.i32()?; // innerKey
                let pairs = r.count(MAX_ASSETS)?;
                for _ in 0..pairs {
                    r.i32()?;
                    r.i32()?;
                }
            }
        }
    }
    if v >= 140 && v < 400 && r.flag()? {
        // Legacy embedded RGB background: consumed, not modelled.
        if v < 170 {
            r.flag()?; // legacyFit
        } else {
            r.f32()?; // scaleX
            r.f32()?; // scaleY
        }
        let w = r.count(32768)?;
        let h = r.count(32768)?;
        if (w as u64) * (h as u64) > 64_000_000 {
            return Err("legacy background exceeds the pixel limit".into());
        }
        r.skip(w * h * 3)?;
    }
    Ok((fps, tween_count, auto_tween))
}

/// Project trailer (`readProjectTrailer`): consumed, not modelled.
fn read_trailer(r: &mut Reader, v: i32, b: i32) -> Result<(), String> {
    if v >= 237 {
        let n = r.count(MAX_FRAMES)?;
        for _ in 0..n {
            r.i32()?; // startFrame
            r.i32()?; // endFrame
            r.flag()?; // easeIn
            r.flag()?; // easeOut
            if v >= 256 {
                r.i16()?; // interpolation
            }
        }
    }
    if v >= 251 {
        r.i32()?; // selectionMode
        for _ in 0..5 {
            r.f32()?; // parameter[i]
        }
        r.i32()?; // selectedAsset
        if v >= 300 {
            r.flag()?; // flag_i
            r.flag()?; // flag_j
        }
        r.flag()?; // flag_k
        r.flag()?; // flag_l
        if v >= 300 {
            r.flag()?; // flag_m
        }
        if v >= 310 {
            r.flag()?; // flag_n
            r.flag()?; // flag_o
        }
        r.flag()?; // flag_p
        if b >= 84 {
            r.flag()?; // flag_r
            r.flag()?; // flag_s
            r.flag()?; // flag_t
            r.flag()?; // flag_u
        }
        if b >= 79 {
            r.i8()?; // mode
        }
        if b >= 84 {
            for _ in 0..7 {
                r.color()?; // palette
            }
        }
    }
    if v >= 319 {
        let n = r.count(MAX_FRAMES)?;
        for _ in 0..n {
            r.i32()?;
            r.i32()?;
        }
    }
    Ok(())
}

/// One sprite library image body (shared by both containers):
/// returns (width, height, has-alpha, raw pixels). The library
/// pivot/fields are consumed but not retained — the model's sprite
/// instances anchor images at their centre (see §17 of the spec),
/// the same treatment the original adapter gives pivots.
fn read_sprite_image_body(r: &mut Reader, b: i32) -> Result<(u32, u32, bool, Vec<u8>), String> {
    let alpha = r.flag()?;
    r.flag()?; // flag1
    r.f32()?; // pivotX
    r.f32()?; // pivotY
    r.f32()?; // field2
    r.f32()?; // field3
    if b >= 9 {
        r.flag()?; // samplingFlag
    }
    let w = r.count(32768)?;
    let h = r.count(32768)?;
    if (w as u64) * (h as u64) > 64_000_000 {
        return Err("sprite exceeds the pixel limit".into());
    }
    let channels = if alpha { 4u64 } else { 3u64 };
    let n = r.count(MAX_PAYLOAD)?;
    if n as u64 != (w as u64) * (h as u64) * channels {
        return Err("image byte count does not match dimensions/channels".into());
    }
    let pixels = r.take(n)?.to_vec();
    Ok((w as u32, h as u32, alpha, pixels))
}

/// Validate group members against a finished sprite library:
/// every member must reference an *image* entry (both original
/// container parsers reject anything else).
fn validate_group_members(sprites: &[SpriteLib]) -> Result<(), String> {
    for s in sprites {
        if let SpriteLib::Group { members } = s {
            for &m in members {
                if !matches!(sprites.get(m), Some(SpriteLib::Image(_))) {
                    return Err(format!("sprite group references non-image {m}"));
                }
            }
        }
    }
    Ok(())
}

/// `.stknds` sprite library (v >= 400). Group members are local
/// library indices.
fn read_sprite_library(r: &mut Reader, b: i32) -> Result<Vec<SpriteLib>, String> {
    let count = r.count(MAX_ASSETS)?;
    let mut sprites = Vec::new();
    for _ in 0..count {
        let ty = r.u8()?;
        if ty > 1 {
            return Err(format!("unknown sprite type {ty}"));
        }
        let name = r.string()?;
        if ty == 1 {
            let n = r.count(MAX_ASSETS)?;
            let mut members = Vec::with_capacity(n);
            for _ in 0..n {
                let m = r.i32()?; // member index
                if m < 0 {
                    return Err(format!("negative sprite group member {m}"));
                }
                members.push(m as usize);
            }
            sprites.push(SpriteLib::Group { members });
        } else {
            let (width, height, alpha, pixels) = read_sprite_image_body(r, b)?;
            sprites.push(SpriteLib::Image(SpriteImage {
                name,
                width,
                height,
                alpha,
                pixels,
            }));
        }
    }
    validate_group_members(&sprites)?;
    Ok(sprites)
}

fn read_figure_library(
    payload: &[u8],
    r: &mut Reader,
    count: usize,
) -> Result<Vec<LibFigure>, String> {
    let mut figures = Vec::new();
    for _ in 0..count {
        let name = r.string()?;
        let scan = scan_figure(payload, r.pos)?;
        r.pos = scan.end;
        let (def, color) = figure_def(payload, &scan);
        figures.push(LibFigure {
            name,
            scan,
            def,
            color,
        });
    }
    Ok(figures)
}

// ---------- conversion to the model ----------

/// Clone `def` with lengths/thicknesses rescaled for an instance:
/// a posed figure's scale *is* the instance scale (§15 of the spec),
/// while the library def baked the figure's own scale in, so the
/// factor is |instance scale| / |figure scale|.
fn bake_instance_def(def: &FigureDef, figure_scale: f32, instance_scale: f32) -> FigureDef {
    let factor = if figure_scale.abs() > 1e-9 {
        instance_scale.abs() / figure_scale.abs()
    } else {
        instance_scale.abs()
    };
    let mut out = def.clone();
    for (i, node) in out.nodes.iter_mut().enumerate() {
        node.length *= factor;
        if i > 0 {
            node.thickness *= factor;
        }
    }
    out
}

/// Convert one figure instance's pose records into a model pose,
/// applying the `layoutFigure` accumulation: parent-first absolute
/// angles, the signed-length +180 flip (skipped for connectors),
/// static nodes taking definition length/scale, and the build <= 53
/// rule that root children store absolute angles.
fn convert_pose(inst: &RawInstance, scan: &FigureScan, container_build: i32) -> Pose {
    let n = scan.nodes.len();
    let mut theta = vec![0.0f32; n];
    let mut angles = vec![0.0f32; n + 1];
    let root_pose_angle = inst.poses.first().map(|p| p.local_angle).unwrap_or(0.0);
    for i in 0..n {
        let node = &scan.nodes[i];
        let pose = &inst.poses[i];
        let mut local = pose.local_angle;
        if container_build <= 53 && i != 0 && node.parent == Some(0) {
            local -= root_pose_angle;
        }
        let (len_v, sc_v, uss_v) = if node.is_static {
            (node.length, node.scale, node.use_segment_scale)
        } else {
            (pose.length, pose.scale, pose.use_segment_scale)
        };
        let eff = len_v * (if uss_v { sc_v } else { 1.0 }) * inst.scale;
        let flip = if !node.connector && eff < 0.0 {
            180.0
        } else {
            0.0
        };
        if i == 0 {
            theta[i] = local + flip;
            angles[1] = norm_deg(theta[i] + 90.0);
        } else {
            let p = node.parent.unwrap_or(0);
            theta[i] = theta[p] + local + flip;
            angles[i + 1] = norm_deg(local + flip);
        }
    }
    Pose {
        root: [inst.x, inst.y],
        angles,
    }
}

/// The adapter's tick rule: a stored frame lasts hold + tween ticks.
#[allow(clippy::too_many_arguments)]
fn frame_duration(
    is_delayed: bool,
    delay_seconds: f32,
    is_tweened: bool,
    is_last: bool,
    auto_tween: bool,
    slow_motion: bool,
    slow_motion_tween_frames: i32,
    tween_count: i32,
    fps: i32,
) -> f32 {
    let hold = if is_delayed {
        ((delay_seconds * fps as f32) - 1e-5).ceil().max(1.0)
    } else {
        1.0
    };
    let tween = if !is_last && auto_tween && is_tweened {
        (if slow_motion {
            slow_motion_tween_frames.max(0)
        } else {
            tween_count.max(0)
        }) as f32
    } else {
        0.0
    };
    (hold + tween).clamp(1.0, 65535.0)
}

/// World position of the end of scan node `scan_idx` of a
/// figure instance whose root sits at `root`, evaluated with
/// the model evaluator — the same point the original adapter
/// computes as `instance position + layoutFigure(posed).get(id)
/// .end` (its handle 0 — the figure's root node — lands on the
/// root itself, since model node 1 has length 0). The instance's
/// def is baked from its own scale, which equals the timeline
/// slot def whenever the instance's scale is constant across
/// frames (the modelled case; §17 of the spec).
fn node_end(
    lib: &LibFigure,
    inst: &RawInstance,
    container_build: i32,
    root: [f32; 2],
    scan_idx: usize,
) -> [f32; 2] {
    let def = bake_instance_def(&lib.def, lib.scan.scale, inst.scale);
    let mut pose = convert_pose(inst, &lib.scan, container_build);
    pose.root = root;
    evaluate(&def, &pose)[scan_idx + 1]
}

/// Resolve one raw frame's joins: for every joined instance
/// index, the root that supersedes its stored (x, y) this frame.
///
/// A join pins its child instance's root to the end of the
/// named node of its parent instance, evaluated with the
/// parent's *final* root — join chains resolve parent-first
/// (the parser has already rejected cycles and duplicate
/// parents). A parent that is not a figure instance has no
/// nodes: it pins to its own final root, which is the original
/// adapter's handle-0 rule (`pi.type === 0 ? handles.get(nodeId)
/// : 0`, attaching at the parent instance's position). Only
/// position is inherited: the child's angles are its own stored
/// pose angles, exactly as the original adapter treats them —
/// the join record's `relativeRotation` is read but not
/// applied, by either importer.
///
/// Joins that cannot resolve (a parent index or node the
/// parser's validation should have caught) are skipped, so the
/// child keeps its stored root — the documented fallback.
fn resolve_join_roots(
    frame: &RawFrame,
    figures: &[LibFigure],
    container_build: i32,
) -> HashMap<usize, [f32; 2]> {
    let mut resolved = HashMap::new();
    // Parser-validated frames always satisfy these bounds; the
    // filter keeps hand-built frames (and any future caller)
    // from panicking here — an unresolvable join simply leaves
    // the child's stored root in place.
    let joins: Vec<&RawJoin> = frame
        .joins
        .iter()
        .filter(|j| j.child < frame.instances.len() && j.parent < frame.instances.len())
        .collect();
    if joins.is_empty() {
        return resolved;
    }
    let join_of: HashMap<usize, &RawJoin> = joins.iter().map(|j| (j.child, *j)).collect();
    // Depth of each joined child = its ancestor count in the
    // join forest; processing in ascending depth order means a
    // parent's root is final before any child reads it.
    let mut depth: HashMap<usize, usize> = HashMap::new();
    for j in &joins {
        if depth.contains_key(&j.child) {
            continue;
        }
        let mut chain: Vec<usize> = Vec::new();
        let mut cur = j.child;
        let base = loop {
            if let Some(&d) = depth.get(&cur) {
                break d;
            }
            // The parser rejects cycles; this visited set keeps
            // a hand-built cyclic frame from looping forever.
            if chain.contains(&cur) {
                break 0;
            }
            match join_of.get(&cur) {
                Some(pj) => {
                    chain.push(cur);
                    cur = pj.parent;
                }
                None => {
                    depth.insert(cur, 0);
                    break 0;
                }
            }
        };
        let mut d = base;
        for node in chain.iter().rev() {
            d += 1;
            depth.insert(*node, d);
        }
    }
    let mut order: Vec<&RawJoin> = joins.clone();
    order.sort_by_key(|j| depth.get(&j.child).copied().unwrap_or(0));
    let mut roots: Vec<[f32; 2]> = frame.instances.iter().map(|i| [i.x, i.y]).collect();
    for j in order {
        let pi = &frame.instances[j.parent];
        let attach = if pi.ty == 0 {
            let lib = &figures[pi.asset];
            match lib.scan.nodes.iter().position(|nd| nd.id == j.node_id) {
                Some(s) => node_end(lib, pi, container_build, roots[j.parent], s),
                None => roots[j.parent],
            }
        } else {
            roots[j.parent]
        };
        roots[j.child] = attach;
    }
    for j in &joins {
        resolved.insert(j.child, roots[j.child]);
    }
    resolved
}

/// Encode a sprite library image as a PNG asset. The stored
/// pixels are RGB(A), row-major, top-down — the layout the
/// original passes straight to its PNG encoder — so this is a
/// lossless repackage, never a resample.
fn encode_sprite_png(img: &SpriteImage) -> Option<Vec<u8>> {
    if img.width == 0 || img.height == 0 {
        return None;
    }
    let channels = if img.alpha { 4usize } else { 3usize };
    if img.pixels.len() != (img.width as usize) * (img.height as usize) * channels {
        return None;
    }
    let mut rgba = Vec::with_capacity((img.width as usize) * (img.height as usize) * 4);
    if img.alpha {
        rgba.extend_from_slice(&img.pixels);
    } else {
        for px in img.pixels.chunks_exact(3) {
            rgba.extend_from_slice(px);
            rgba.push(255);
        }
    }
    let buf = image::RgbaImage::from_raw(img.width, img.height, rgba)?;
    let mut out = Vec::new();
    image::DynamicImage::ImageRgba8(buf)
        .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
        .ok()?;
    Some(out)
}

/// Build the project asset table from a sprite library: one asset
/// per image entry, in library order. Returns the assets and, per
/// library index, the project asset index (None for groups and for
/// images that fail to encode). After the parser's byte-count
/// validation the only un-encodable images are zero-dimension
/// ones (no pixels to display); they map to None and their
/// instances are skipped in `build_instance_tracks` — a deliberate
/// lenient outcome, pinned by
/// `zero_size_sprite_image_is_skipped_not_scrambled`.
fn sprite_assets(sprites: &[SpriteLib]) -> (Vec<ImageAsset>, Vec<Option<usize>>) {
    let mut assets = Vec::new();
    let mut map = Vec::with_capacity(sprites.len());
    for s in sprites {
        match s {
            SpriteLib::Image(img) => match encode_sprite_png(img) {
                Some(png) => {
                    map.push(Some(assets.len()));
                    assets.push(ImageAsset {
                        name: img.name.clone(),
                        png,
                    });
                }
                None => map.push(None),
            },
            SpriteLib::Group { .. } => map.push(None),
        }
    }
    (assets, map)
}

/// The library *image* a sprite instance displays: itself for
/// type 2; for a group, the member at `group_frame` (falling back
/// to the first member when the frame is out of range — the
/// original's semantic validator rejects such files; this importer
/// stays lenient there, as it does for clip cursors).
fn resolve_sprite_image(sprites: &[SpriteLib], inst: &RawInstance) -> Option<usize> {
    match inst.ty {
        2 => match sprites.get(inst.asset) {
            Some(SpriteLib::Image(_)) => Some(inst.asset),
            _ => None,
        },
        3 => match sprites.get(inst.asset) {
            Some(SpriteLib::Group { members }) => {
                if members.is_empty() {
                    None
                } else if (0..members.len() as i32).contains(&inst.group_frame) {
                    Some(members[inst.group_frame as usize])
                } else {
                    members.first().copied()
                }
            }
            _ => None,
        },
        _ => None,
    }
}

/// Base glyph height (stage units) of an unscaled project text.
/// The original adapter builds its text figures at font size 48,
/// and per-frame `scale` multiplies it — so the model's
/// `TextInstance.size` is this constant and the key scale carries
/// the file's value.
const TEXT_BASE_SIZE: f32 = 48.0;

/// Stage position of a text's centre. The stored (x, y) is an
/// anchor whose meaning follows `align` — 8: start of the text,
/// 16: end, anything else: centre — mirroring the shift the
/// original adapter applies. The width estimate uses the bitmap
/// font `media.rs` draws with: (4n − 1) cells of size/5.
fn text_centre(text: &RawText) -> [f32; 2] {
    let n = text.content.chars().count() as f32;
    let height = TEXT_BASE_SIZE * text.scale;
    let width = if n > 0.0 {
        (4.0 * n - 1.0) * height / 5.0
    } else {
        0.0
    };
    let factor = match text.align {
        8 => 0.5,
        16 => -0.5,
        _ => 0.0,
    };
    if factor == 0.0 || width == 0.0 {
        return [text.x, text.y];
    }
    let rad = text.rotation.to_radians();
    [
        text.x + factor * width * rad.cos(),
        text.y + factor * width * rad.sin(),
    ]
}

/// Build sprite/text instance tracks from parsed timeline frames
/// (`.stknds` main frames, or a `.nodemc`'s own frames — library
/// movieclip frames are *not* timelines and are not passed here).
///
/// Tracks are keyed in first-appearance order — sprites by
/// (instance type, instance id), like the figure slots, texts by
/// text id. A key is recorded at every frame the instance appears
/// in (the model's `InstanceKey::sample` then interpolates between
/// appearances and holds outside them, mirroring figure holds).
/// Sprite content/colour are fixed at first appearance: a sprite
/// whose resolved image changes later keeps its first image (the
/// figure slots' hold rule), and a text whose content changes
/// keeps its first content. A sprite instance joined to another
/// instance this frame keys at its join-resolved root instead
/// of its stored position (see `resolve_join_roots`).
fn build_instance_tracks(
    frames_raw: &[RawFrame],
    sprites: &[SpriteLib],
    sprite_asset: &[Option<usize>],
    figures: &[LibFigure],
    container_build: i32,
) -> (Vec<SpriteInstance>, Vec<TextInstance>) {
    let mut out_sprites: Vec<SpriteInstance> = Vec::new();
    let mut sprite_tracks: HashMap<(i32, i32), usize> = HashMap::new();
    let mut out_texts: Vec<TextInstance> = Vec::new();
    let mut text_tracks: HashMap<i32, usize> = HashMap::new();
    for (fi, frame) in frames_raw.iter().enumerate() {
        let join_roots = resolve_join_roots(frame, figures, container_build);
        for (ii, inst) in frame.instances.iter().enumerate() {
            if inst.ty != 2 && inst.ty != 3 {
                continue;
            }
            let Some(lib_idx) = resolve_sprite_image(sprites, inst) else {
                continue;
            };
            let Some(asset) = sprite_asset.get(lib_idx).copied().flatten() else {
                continue;
            };
            let (kx, ky) = match join_roots.get(&ii) {
                Some(&[x, y]) => (x, y),
                None => (inst.x, inst.y),
            };
            let key = InstanceKey {
                frame: fi as f32,
                x: kx,
                y: ky,
                scale: inst.scale_x.abs(),
                angle: inst.rotation,
                // The P-6 enrichment (Wave X): fields the
                // importer used to read and discard. Magnitudes
                // for the scales, as with `scale` — the flips
                // ride their own flags.
                opacity: inst.opacity,
                flip_x: inst.flip_x,
                flip_y: inst.flip_y,
                scale_y: inst.scale_y.abs(),
                pivot_x: inst.pivot_x,
                pivot_y: inst.pivot_y,
            };
            let track_key = (inst.ty, inst.id);
            match sprite_tracks.get(&track_key) {
                Some(&ti) => {
                    let track = &mut out_sprites[ti];
                    if track.asset == asset {
                        match track.keys.last_mut() {
                            Some(last) if last.frame == key.frame => *last = key,
                            _ => track.keys.push(key),
                        }
                    }
                    // else: image changed after first appearance —
                    // the first image is kept (see fn docs).
                }
                None => {
                    sprite_tracks.insert(track_key, out_sprites.len());
                    out_sprites.push(SpriteInstance {
                        asset,
                        keys: vec![key],
                    });
                }
            }
        }
        for text in &frame.texts {
            let centre = text_centre(text);
            let key = InstanceKey {
                frame: fi as f32,
                x: centre[0],
                y: centre[1],
                scale: text.scale,
                angle: text.rotation,
                // Text keys share the struct; the enrichment
                // fields stay at their defaults (the text
                // record's own fields are a separate site).
                opacity: 1.0,
                flip_x: false,
                flip_y: false,
                scale_y: 0.0,
                pivot_x: 0.5,
                pivot_y: 0.5,
            };
            match text_tracks.get(&text.id) {
                Some(&ti) => {
                    let track = &mut out_texts[ti];
                    match track.keys.last_mut() {
                        Some(last) if last.frame == key.frame => *last = key,
                        _ => track.keys.push(key),
                    }
                }
                None => {
                    text_tracks.insert(text.id, out_texts.len());
                    out_texts.push(TextInstance {
                        content: text.content.clone(),
                        size: TEXT_BASE_SIZE,
                        color: text.color,
                        keys: vec![key],
                    });
                }
            }
        }
    }
    (out_sprites, out_texts)
}

/// The one figure instance a clip frame may hold for the clip to
/// convert (sprite instances alongside it no longer block
/// conversion — they simply are not carried into the `ClipDef`;
/// see §17 of the spec).
fn clip_figure_instance(frame: &RawFrame) -> Option<&RawInstance> {
    let mut found = None;
    for inst in &frame.instances {
        if inst.ty == 0 {
            if found.is_some() {
                return None;
            }
            found = Some(inst);
        }
    }
    found
}

/// Convert a library movieclip to a `ClipDef` when every frame
/// holds exactly one figure instance of the same figure and no
/// joins — the only shape `model::ClipDef` can represent.
fn convert_clip(
    clip: &RawClip,
    figures: &[LibFigure],
    container_build: i32,
) -> Option<(ClipDef, usize, u32)> {
    let first = clip.frames.first()?;
    if !first.joins.is_empty() {
        return None;
    }
    let inst0 = clip_figure_instance(first)?;
    let asset = inst0.asset;
    let mut poses = Vec::with_capacity(clip.frames.len());
    for frame in &clip.frames {
        if !frame.joins.is_empty() {
            return None;
        }
        let inst = clip_figure_instance(frame)?;
        if inst.asset != asset {
            return None;
        }
        poses.push(convert_pose(inst, &figures[asset].scan, container_build));
    }
    let def = bake_instance_def(&figures[asset].def, figures[asset].scan.scale, inst0.scale);
    Some((
        ClipDef {
            name: clip.name.clone(),
            def,
            frames: poses,
            loop_mode: ClipLoop::Loop,
        },
        asset,
        inst0.color,
    ))
}

struct Slot {
    key: (i32, i32), // (instance type, instance id)
    asset: usize,
    def: FigureDef,
    name: String,
    color: u32,
    clip: Option<usize>,
    first_xy: [f32; 2],
}

/// Assemble figure slots (first-appearance order, keyed by instance
/// id) and per-frame poses (missing instances hold their last pose).
#[allow(clippy::too_many_arguments)]
fn build_timeline(
    frames_raw: &[RawFrame],
    durations: &[f32],
    figures: &[LibFigure],
    raw_clips: &[RawClip],
    converted_clips: &HashMap<usize, (usize, u32)>, // clip asset -> (project.clips index, slot colour)
    clips: &mut Vec<ClipDef>,
    clip_first_roots: &HashMap<usize, [f32; 2]>, // clip asset -> pre-bake first root
    container_build: i32,
) -> (Vec<FigureInstance>, Vec<Frame>) {
    let mut slots: Vec<Slot> = Vec::new();
    let mut slot_index: HashMap<(i32, i32), usize> = HashMap::new();
    let mut baked_clips: std::collections::HashSet<usize> = std::collections::HashSet::new();
    for frame in frames_raw {
        for inst in &frame.instances {
            if inst.ty != 0 && inst.ty != 1 {
                continue; // sprites become instance tracks, not figure slots
            }
            let key = (inst.ty, inst.id);
            if slot_index.contains_key(&key) {
                continue;
            }
            if inst.ty == 0 {
                let fig = &figures[inst.asset];
                let def = bake_instance_def(&fig.def, fig.scan.scale, inst.scale);
                slot_index.insert(key, slots.len());
                slots.push(Slot {
                    key,
                    asset: inst.asset,
                    def,
                    name: fig.name.clone(),
                    color: inst.color,
                    clip: None,
                    first_xy: [inst.x, inst.y],
                });
            } else {
                // Movieclip instance: only converted clips get a slot.
                let Some(&(ci, slot_color)) = converted_clips.get(&inst.asset) else {
                    continue;
                };
                if !baked_clips.contains(&inst.asset) {
                    // Offset the clip so its first-frame root lands
                    // on this (first) placement point.
                    let base = clip_first_roots
                        .get(&inst.asset)
                        .copied()
                        .unwrap_or([0.0, 0.0]);
                    let offset = [inst.x - base[0], inst.y - base[1]];
                    for pose in &mut clips[ci].frames {
                        pose.root = [pose.root[0] + offset[0], pose.root[1] + offset[1]];
                    }
                    baked_clips.insert(inst.asset);
                }
                let clip_def = clips[ci].def.clone();
                slot_index.insert(key, slots.len());
                slots.push(Slot {
                    key,
                    asset: inst.asset,
                    def: clip_def,
                    name: raw_clips[inst.asset].name.clone(),
                    color: slot_color,
                    clip: Some(ci),
                    first_xy: [inst.x, inst.y],
                });
            }
        }
    }

    let mut frames: Vec<Frame> = Vec::with_capacity(frames_raw.len());
    let mut last_poses: Vec<Pose> = slots
        .iter()
        .map(|s| s.def.rest_pose(s.first_xy))
        .collect();
    for (fi, frame) in frames_raw.iter().enumerate() {
        let mut by_key: HashMap<(i32, i32), &RawInstance> = HashMap::new();
        let mut index_of_key: HashMap<(i32, i32), usize> = HashMap::new();
        for (ii, inst) in frame.instances.iter().enumerate() {
            by_key.insert((inst.ty, inst.id), inst);
            index_of_key.insert((inst.ty, inst.id), ii);
        }
        // Joins bake at assembly: a joined instance's pose root
        // is superseded by its host's evaluated node end. Held
        // poses (instance absent this frame) are untouched —
        // there is no join record for an absent instance.
        let join_roots = resolve_join_roots(frame, figures, container_build);
        let mut poses: Vec<Pose> = Vec::with_capacity(slots.len());
        for (si, slot) in slots.iter().enumerate() {
            let (mut pose, present) = match by_key.get(&slot.key) {
                Some(inst) if slot.clip.is_none() && inst.asset == slot.asset => {
                    (convert_pose(inst, &figures[slot.asset].scan, container_build), true)
                }
                Some(inst) if slot.clip.is_some() => {
                    // Playback replaces this with the clip's pose;
                    // the rest pose at the placement is the fallback.
                    (slot.def.rest_pose([inst.x, inst.y]), true)
                }
                _ => (last_poses[si].clone(), false),
            };
            if present {
                if let Some(&ii) = index_of_key.get(&slot.key) {
                    if let Some(&root) = join_roots.get(&ii) {
                        pose.root = root;
                    }
                }
            }
            poses.push(pose);
        }
        last_poses = poses.clone();
        frames.push(Frame {
            poses,
            duration: durations.get(fi).copied().unwrap_or(1.0),
            easing: None,
            bends: Vec::new(),
        });
    }
    if frames.is_empty() {
        frames.push(Frame {
            poses: slots
                .iter()
                .map(|s| s.def.rest_pose(s.first_xy))
                .collect(),
            duration: 1.0,
            easing: None,
            bends: Vec::new(),
        });
    }
    let figures_out = slots
        .into_iter()
        .map(|s| FigureInstance {
            locked: false,
            opacity: 1.0,
            name: s.name,
            color: s.color,
            def: s.def,
            hidden: false,
            clip: s.clip,
            states: Vec::new(),
        })
        .collect();
    (figures_out, frames)
}

fn empty_project(name: String, fps: f32, auto_tween: bool) -> Project {
    Project {
        name,
        fps,
        stage: STAGE,
        background: DEFAULT_BG,
        figures: Vec::new(),
        frames: Vec::new(),
        camera: Vec::new(),
        tween_playback: auto_tween,
        easing: Easing::Linear,
        camera_follow: None,
        particles: Vec::new(),
        strokes: Vec::new(),
        audio: Vec::new(),
        clips: Vec::new(),
        fx_clips: Vec::new(),
        assets: Vec::new(),
        channels: Vec::new(),
        sprites: Vec::new(),
        texts: Vec::new(),
        weather: None,
        head_ring_scale: 1.0,
        subtitles: Vec::new(),
        subtitle_style: Default::default(),
        format_version: crate::model::FORMAT_VERSION_CURRENT,
    }
}

/// Convert the main frames' camera records to model camera
/// keys. The mapping is the original adapter's own conversion
/// (`src/sticknodes/adapter.mjs`: native camera
/// `x = c.x + 960/z − 960`, `y = 540 − c.y − 540/z`,
/// `scale = 1/z`, `angle_rad = −c.rotation°`), composed with
/// the native render transform (`cameraMatrix` in
/// `renderer-geometry.mjs`, whose effective magnification is
/// `1/scale` per `inverseCameraScale` in
/// `pivot/background.mjs`): in the format's own coordinates
/// the record's (x, y) anchors the view's corner, the view
/// centre sits at `(x + 960/z, y + 540/z)`, the magnification
/// is `z`, and `rotation` is in degrees. This importer keeps
/// file positions verbatim (figure roots, text and sprite
/// positions alike) and — like the text and sprite rotation
/// fields, which become `InstanceKey` angles verbatim — keeps
/// the rotation verbatim in degrees, so a record becomes the
/// key `(x + 960/z, y + 540/z, z, rotation)`.
///
/// A project whose every record is the default (zoom 1, no
/// offset, no rotation) gets NO keys: the model's own default
/// camera then applies, exactly as imports behaved before
/// camera records were modelled. If any record differs, keys
/// are dense (one per frame) — the format stores a record per
/// frame, and dense keys reproduce every frame's values
/// exactly under the model's interpolation.
fn camera_keys(frames_raw: &[RawFrame]) -> Vec<CameraKey> {
    if !frames_raw
        .iter()
        .any(|f| f.camera.map_or(false, |c| !c.is_default()))
    {
        return Vec::new();
    }
    frames_raw
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let c = f.camera.unwrap_or_else(RawCamera::fallback);
            // The adapter's guard: a non-positive zoom reads as 1.
            let z = if c.zoom > 0.0 { c.zoom } else { 1.0 };
            CameraKey {
                frame: i,
                x: c.x + 960.0 / z,
                y: c.y + 540.0 / z,
                zoom: z,
                angle: c.rotation,
            }
        })
        .collect()
}

// ---------- .stknds ----------

fn parse_stknds(payload: &[u8]) -> Result<Project, String> {
    let mut r = Reader::new(payload);
    let version = r.i32()?;
    if version < 1 {
        return Err(format!("invalid project version {version}"));
    }
    let build = if version >= 401 { r.i32()? } else { 1 };
    if build < 0 {
        return Err(format!("invalid build {build}"));
    }
    let name = r.string()?;
    let (fps, tween_count, auto_tween) = read_project_header(&mut r, version, build)?;
    if !(1..=1000).contains(&fps) {
        return Err(format!("project fps {fps} outside 1..=1000"));
    }
    let layouts = (
        compile_pose_layout(version, build, false),
        compile_pose_layout(version, build, true),
    );

    let figure_count = r.count(MAX_ASSETS)?;
    let figures = read_figure_library(payload, &mut r, figure_count)?;

    let mut sounds_len = 0usize;
    if version >= 160 {
        sounds_len = r.count(MAX_ASSETS)?;
        for _ in 0..sounds_len {
            r.string()?; // sound name (external reference only)
        }
    }

    let sprites = if version >= 400 {
        read_sprite_library(&mut r, build)?
    } else {
        Vec::new()
    };

    let mut raw_clips: Vec<RawClip> = Vec::new();
    if version >= 300 {
        let clip_count = r.count(MAX_ASSETS)?;
        for _ in 0..clip_count {
            let clip_name = r.string()?;
            let frame_count = r.count(MAX_FRAMES)?;
            let clip_fps = r.count(1000)? as i32;
            if clip_fps < 1 {
                return Err(format!("movieclip fps {clip_fps} outside 1..=1000"));
            }
            let clip_tween = r.count(10_000)? as i32;
            let clip_auto = r.flag()?;
            r.flag()?; // flags[1]
            r.i32()?; // nextInstanceId
            let mut frames = Vec::with_capacity(frame_count);
            for _ in 0..frame_count {
                frames.push(read_clip_frame(
                    &mut r,
                    version,
                    build,
                    &figures,
                    &sprites,
                    clip_count,
                    sounds_len,
                    &layouts,
                )?);
            }
            r.i32()?; // libraryId
            raw_clips.push(RawClip {
                name: clip_name,
                fps: clip_fps,
                tween_count: clip_tween,
                auto_tween: clip_auto,
                frames,
            });
        }
    }

    let frame_count = r.count(MAX_FRAMES)?;
    let mut frames_raw = Vec::with_capacity(frame_count);
    for _ in 0..frame_count {
        frames_raw.push(read_main_frame(
            &mut r,
            version,
            build,
            &figures,
            &sprites,
            raw_clips.len(),
            sounds_len,
            &layouts,
        )?);
    }
    read_trailer(&mut r, version, build)?;
    if r.pos != payload.len() {
        return Err(format!(
            "{} trailing bytes after the project trailer",
            payload.len() - r.pos
        ));
    }

    // Convert single-figure movieclips to ClipDefs.
    let mut clips: Vec<ClipDef> = Vec::new();
    let mut converted: HashMap<usize, (usize, u32)> = HashMap::new();
    let mut clip_first_roots: HashMap<usize, [f32; 2]> = HashMap::new();
    for (asset, clip) in raw_clips.iter().enumerate() {
        if let Some((def, _asset, color)) = convert_clip(clip, &figures, build) {
            if let Some(first) = def.frames.first() {
                clip_first_roots.insert(asset, first.root);
            }
            converted.insert(asset, (clips.len(), color));
            clips.push(def);
        }
    }

    let durations: Vec<f32> = frames_raw
        .iter()
        .enumerate()
        .map(|(i, fr)| {
            let s = fr.settings.as_ref().unwrap();
            frame_duration(
                s.is_delayed,
                s.delay_seconds,
                s.is_tweened,
                i + 1 == frames_raw.len(),
                auto_tween,
                s.slow_motion,
                s.slow_motion_tween_frames,
                tween_count,
                fps,
            )
        })
        .collect();
    let background = frames_raw
        .first()
        .and_then(|f| f.settings.as_ref())
        .map(|s| s.bg)
        .unwrap_or(DEFAULT_BG);
    let (figures_out, frames) = build_timeline(
        &frames_raw,
        &durations,
        &figures,
        &raw_clips,
        &converted,
        &mut clips,
        &clip_first_roots,
        build,
    );
    let (assets, sprite_asset) = sprite_assets(&sprites);
    let (sprites_out, texts_out) =
        build_instance_tracks(&frames_raw, &sprites, &sprite_asset, &figures, build);
    let mut project = empty_project(name, fps as f32, auto_tween);
    project.background = background;
    project.figures = figures_out;
    project.frames = frames;
    project.clips = clips;
    project.assets = assets;
    project.sprites = sprites_out;
    project.texts = texts_out;
    project.camera = camera_keys(&frames_raw);
    Ok(project)
}

// ---------- .nodemc ----------

fn parse_nodemc(payload: &[u8]) -> Result<Project, String> {
    let mut r = Reader::new(payload);
    let version = r.i32()?;
    if version < 1 {
        return Err(format!("invalid movieclip version {version}"));
    }
    let build = if version >= 402 { r.i32()? } else { 1 };
    if build < 0 {
        return Err(format!("invalid build {build}"));
    }
    let name = r.string()?;
    let frame_count = r.count(MAX_FRAMES)?;
    let figure_count = r.count(MAX_ASSETS)?;
    let sprite_count = if version >= 400 {
        r.count(MAX_ASSETS)?
    } else {
        0
    };
    let sound_count = r.count(MAX_ASSETS)?;
    let thumb_count = r.count(MAX_FRAMES)?;
    let thumb_w = r.count(32768)?;
    let thumb_h = r.count(32768)?;
    let thumb_bytes = (thumb_count as u64) * (thumb_w as u64) * (thumb_h as u64) * 4;
    if thumb_bytes > MAX_PAYLOAD as u64 {
        return Err("movieclip thumbnails exceed the byte budget".into());
    }
    r.skip(thumb_bytes as usize)?; // RGBA thumbnails: consumed, not modelled
    let fps = r.i32()?;
    if !(1..=1000).contains(&fps) {
        return Err(format!("movieclip fps {fps} outside 1..=1000"));
    }
    let tween_count = r.i32()?;
    let auto_tween = r.flag()?;
    r.flag()?; // clipFlag
    r.i32()?; // nextInstanceId
    let layouts = (
        compile_pose_layout(version, build, false),
        compile_pose_layout(version, build, true),
    );

    let figures = read_figure_library(payload, &mut r, figure_count)?;
    // Byte order after the figure library: sprites, then sounds.
    // (The sprite *count* was read in the header block above.)
    let sprites = read_sprite_library_mc_count(&mut r, build, sprite_count)?;
    for _ in 0..sound_count {
        r.string()?; // sound name (external reference only)
    }

    let mut frames_raw = Vec::with_capacity(frame_count);
    for _ in 0..frame_count {
        frames_raw.push(read_clip_frame(
            &mut r,
            version,
            build,
            &figures,
            &sprites,
            0,
            sound_count,
            &layouts,
        )?);
    }
    if r.pos != payload.len() {
        return Err(format!(
            "{} trailing bytes after the movie clip",
            payload.len() - r.pos
        ));
    }

    let durations: Vec<f32> = frames_raw
        .iter()
        .enumerate()
        .map(|(i, _)| {
            frame_duration(
                false,
                0.0,
                true,
                i + 1 == frames_raw.len(),
                auto_tween,
                false,
                0,
                tween_count,
                fps,
            )
        })
        .collect();
    let (figures_out, frames) = build_timeline(
        &frames_raw,
        &durations,
        &figures,
        &[],
        &HashMap::new(),
        &mut Vec::new(),
        &HashMap::new(),
        build,
    );
    let (assets, sprite_asset) = sprite_assets(&sprites);
    // A .nodemc's frames are its own timeline, so its sprite
    // instances model directly (clip frames carry no texts).
    let (sprites_out, texts_out) =
        build_instance_tracks(&frames_raw, &sprites, &sprite_asset, &figures, build);
    let mut project = empty_project(name, fps as f32, auto_tween);
    project.figures = figures_out;
    project.frames = frames;
    project.assets = assets;
    project.sprites = sprites_out;
    project.texts = texts_out;
    Ok(project)
}

/// `.nodemc` sprite entries when the count was read earlier in the
/// header (the count precedes the thumbnails/settings in the byte
/// stream, but the entries sit after the figure library). Group
/// members are *source-library ids*, resolved here through the
/// entries' own source ids — the original decoder's rule, including
/// its duplicate-id and dangling-reference errors.
fn read_sprite_library_mc_count(
    r: &mut Reader,
    b: i32,
    count: usize,
) -> Result<Vec<SpriteLib>, String> {
    enum Tmp {
        Image(SpriteImage),
        Group { source_members: Vec<i32> },
    }
    let mut source_ids: HashMap<i32, usize> = HashMap::new();
    let mut tmp: Vec<Tmp> = Vec::with_capacity(count);
    for _ in 0..count {
        let ty = r.u8()?;
        if ty > 1 {
            return Err(format!("unknown sprite type {ty}"));
        }
        let name = r.string()?;
        let source_id = r.i32()?;
        if source_ids.insert(source_id, tmp.len()).is_some() {
            return Err(format!("duplicate movie-clip sprite library id {source_id}"));
        }
        if ty == 1 {
            let n = r.count(MAX_ASSETS)?;
            let mut source_members = Vec::with_capacity(n);
            for _ in 0..n {
                source_members.push(r.i32()?); // source member id
            }
            tmp.push(Tmp::Group { source_members });
        } else {
            let (width, height, alpha, pixels) = read_sprite_image_body(r, b)?;
            tmp.push(Tmp::Image(SpriteImage {
                name,
                width,
                height,
                alpha,
                pixels,
            }));
        }
    }
    let mut sprites = Vec::with_capacity(tmp.len());
    for entry in tmp {
        match entry {
            Tmp::Image(img) => sprites.push(SpriteLib::Image(img)),
            Tmp::Group { source_members } => {
                let mut members = Vec::with_capacity(source_members.len());
                for id in source_members {
                    match source_ids.get(&id) {
                        Some(&idx) => members.push(idx),
                        None => {
                            return Err(format!(
                                "sprite group references missing source id {id}"
                            ));
                        }
                    }
                }
                sprites.push(SpriteLib::Group { members });
            }
        }
    }
    validate_group_members(&sprites)?;
    Ok(sprites)
}

/// Import a Stick Nodes animation container (`.stknds` project or
/// `.nodemc` movie clip) as a model project.
///
/// The original decoder dispatches on filename; with bytes only,
/// the `.stknds` grammar is attempted first and the `.nodemc`
/// grammar second — both demand exact EOF, so a wrong attempt
/// cannot silently succeed.
pub fn import_nodes_animation(data: &[u8]) -> Result<Project, String> {
    if data.len() > MAX_INPUT {
        return Err("file exceeds the 256 MiB input limit".into());
    }
    let payload = unwrap_container(data)?;
    if payload.len() > MAX_PAYLOAD {
        return Err("payload exceeds the 512 MiB limit".into());
    }
    match parse_stknds(&payload) {
        Ok(project) => Ok(project),
        Err(stk_err) => match parse_nodemc(&payload) {
            Ok(project) => Ok(project),
            Err(mc_err) => Err(format!(
                "not a readable Stick Nodes animation container (.stknds: {stk_err}; .nodemc: {mc_err})"
            )),
        },
    }
}

// ---------- .stknds export ----------

/// Big-endian byte writer for the export profile below.
struct BeWriter {
    v: Vec<u8>,
}

impl BeWriter {
    fn i8(&mut self, x: i8) {
        self.v.push(x as u8);
    }
    fn i16(&mut self, x: i16) {
        self.v.extend_from_slice(&x.to_be_bytes());
    }
    fn i32(&mut self, x: i32) {
        self.v.extend_from_slice(&x.to_be_bytes());
    }
    fn f32(&mut self, x: f32) {
        self.v.extend_from_slice(&x.to_be_bytes());
    }
    fn flag(&mut self, b: bool) {
        self.v.push(b as u8);
    }
    /// [alpha, blue, green, red] — the wire order `Reader::color`
    /// reads back as 0xRRGGBB.
    fn color(&mut self, rgb: u32) {
        self.v.push(255);
        self.v.push((rgb & 0xFF) as u8);
        self.v.push(((rgb >> 8) & 0xFF) as u8);
        self.v.push(((rgb >> 16) & 0xFF) as u8);
    }
    /// A fully transparent colour (unused editor colours).
    fn color0(&mut self) {
        self.v.extend_from_slice(&[0, 0, 0, 0]);
    }
    fn string(&mut self, s: &str) {
        self.i32(s.len() as i32);
        self.v.extend_from_slice(s.as_bytes());
    }
}

/// Project header at 431/117 (field order per
/// `read_project_header`), editor metadata at defaults. The
/// header's auto-tween flag is written false: with it set, the
/// importer adds tween ticks to every non-final tweened frame,
/// which would corrupt the stored durations.
fn write_project_header(
    w: &mut BeWriter,
    fps: i32,
    figure_count: usize,
    sprite_library_count: usize,
    sprite_track_count: usize,
) {
    w.flag(false); // editorFlagD (build >= 16)
    w.flag(false); // editorFlagE (v >= 244)
    w.string(""); // projectCredit
    w.color0(); // editorColor
    w.i32(fps);
    w.i32(2); // tweenCount (unused: autoTween is false)
    w.flag(false); // editorFlagB
    w.flag(false); // editorFlagZ
    w.flag(false); // editorFlagA
    w.flag(false); // editorFlagC
    w.flag(false); // autoTween
    w.flag(false); // timelineFlagC
    w.i32(0); // selectedFrame
    w.i32(1); // nextTextId
    // Instance ids written by the exporter run 1..=figure_count
    // for figures, then one per exported sprite track.
    w.i32((figure_count + sprite_track_count) as i32 + 1); // nextInstanceId
    w.i32((figure_count + sprite_library_count) as i32); // nextLibraryId
    w.i32(0); // fontUsage count (v >= 400)
    w.i32(0); // editor list 1
    w.i32(0); // editor list 2
    w.i32(0); // editor list 3
    w.i32(0); // outer map count
}

/// Main-frame settings at 431/117 (field order per
/// `read_main_settings`). The frame's duration is encoded as a
/// delay: with auto-tween off, the importer's tick rule reduces
/// to `ceil(delay_seconds * fps - 1e-5)`, so an integer duration
/// `d` is stored exactly as `delay_seconds = d / fps`.
fn write_main_settings(w: &mut BeWriter, bg: u32, duration: f32, fps: f32) {
    let delayed = duration != 1.0;
    w.flag(false); // isTweened
    w.flag(false); // useImageBackground
    w.flag(false); // useGradient
    w.flag(false); // expandedGradient
    w.flag(false); // stopSounds
    w.flag(delayed); // isDelayed
    w.f32(if delayed { duration / fps } else { 0.0 }); // delaySeconds
    w.flag(false); // slowMotion
    w.i32(0); // slowMotionTweenFrames
    w.flag(false); // repeating
    w.i32(0); // repeatBackFrames
    w.i32(0); // repeatLoops
    w.flag(false); // tweenWhenRepeating
    w.flag(false); // cameraDuringDelay
    w.flag(false); // easeIn
    w.flag(false); // easeOut
    w.i16(0); // interpolation
    w.color(bg); // background colour
    w.color0(); // backgroundGradientColor
    w.i32(-1); // soundIndex: none
    w.f32(1.0); // soundVolume
    w.f32(0.0); // soundPan
    w.f32(1.0); // soundPitch
}

/// Instance style block at 431/117 (field order per
/// `read_style`), all filters at their neutral defaults;
/// the opacity is the caller's (sprite keys carry theirs,
/// Wave X P-6; figure instances pass 1.0, as before).
fn write_style(w: &mut BeWriter, opacity: f32) {
    w.f32(opacity); // opacity
    w.f32(0.0); // blur
    w.flag(false); // motionBlur
    w.flag(false); // omnidirectionalBlur
    w.i32(0); // motionBlurAngle
    w.f32(1.0); // saturation
    w.i32(0); // pixelation
    w.color0(); // tintColor
    w.f32(0.0); // tintAmount
    w.f32(0.0); // hueShift
    w.flag(false); // invert
    w.f32(0.0); // glow
    w.color0(); // glowColor
    w.f32(0.0); // glowIntensity
    w.color0(); // outlineColor
    w.f32(1.0); // outlineAlpha
    w.f32(1.0); // outlineThickness
    w.color0(); // dropShadowColor
    w.f32(0.0); // dropShadowAlpha
    w.i32(0); // dropShadowDistance
    w.i32(0); // dropShadowAngle
    w.f32(0.0); // dropShadowBlur
}

/// One pose record, driven by the reader's own compiled layout
/// table at 431/117 so writer and reader can never drift apart:
/// every field is written at its neutral default except the
/// angle, the segment length, and the thickness. The record's
/// `use_segment_scale` flag is false, so the effective length
/// the importer computes is exactly `length` (times the instance
/// scale, written as 1.0).
fn write_pose_record(
    w: &mut BeWriter,
    layout: &PoseLayout,
    local_angle: f32,
    length: f32,
    thickness: i32,
) {
    for (kind, name) in &layout.fields {
        match kind {
            PKind::Flag => w.flag(false),
            PKind::I8 => w.i8(0),
            PKind::I16 => w.i16(0),
            PKind::I32 => w.i32(if *name == "thickness" { thickness } else { 0 }),
            PKind::F32 => w.f32(match *name {
                "local_angle" | "default_local_angle" => local_angle,
                "length" => length,
                "scale" => 1.0,
                _ => 0.0,
            }),
            PKind::Color => w.color0(),
        }
    }
}

/// Frame camera at 431/117 (field order per `read_camera`).
/// With no camera data (`cam` is `None`) the record is the
/// default — zoom 1, no offset, no wobble, no bindings —
/// byte-identical to what the writer emitted before camera
/// export existed. With camera data, `cam` is the frame's
/// evaluated model camera `(x, y, zoom, angle)` and the
/// record is written via the inverse of the import mapping
/// (see `camera_keys`): zoom verbatim, the record's anchor is
/// the view centre minus `(960/z, 540/z)`, and the rotation
/// is the angle in degrees verbatim. Wobble and bindings are
/// written absent: the model has no source for them.
fn write_camera(w: &mut BeWriter, cam: Option<(f32, f32, f32, f32)>) {
    let (zoom, x, y, rotation) = match cam {
        Some((cx, cy, cz, angle)) => {
            let z = if cz > 0.0 { cz } else { 1.0 };
            (z, cx - 960.0 / z, cy - 540.0 / z, angle)
        }
        None => (1.0, 0.0, 0.0, 0.0),
    };
    w.f32(zoom); // zoom
    w.f32(x); // x
    w.f32(y); // y
    w.f32(rotation); // rotation
    w.flag(false); // wobbleXY
    w.flag(false); // wobbleRotation
    w.flag(false); // wobbleScale
    w.i32(0); // wobbleXYIntensity
    w.f32(0.0); // wobbleRotationIntensity
    w.f32(0.0); // wobbleSpeed
    w.flag(false); // widescreen
    w.i32(0); // bindings count
    w.i32(0); // secondary bindings count
}

/// Project trailer at 431/117 (field order per `read_trailer`),
/// all editor state at defaults.
fn write_trailer(w: &mut BeWriter) {
    w.i32(0); // tweenLinks count
    w.i32(0); // selectionMode
    for _ in 0..5 {
        w.f32(0.0); // parameter[i]
    }
    w.i32(0); // selectedAsset
    w.flag(false); // flag_i
    w.flag(false); // flag_j
    w.flag(false); // flag_k
    w.flag(false); // flag_l
    w.flag(false); // flag_m
    w.flag(false); // flag_n
    w.flag(false); // flag_o
    w.flag(false); // flag_p
    w.flag(false); // flag_r (build >= 84)
    w.flag(false); // flag_s
    w.flag(false); // flag_t
    w.flag(false); // flag_u
    w.i8(0); // mode (build >= 79)
    for _ in 0..7 {
        w.color0(); // palette
    }
    w.i32(0); // editorPairs count
}

/// One project asset decoded for the `.stknds` sprite library.
/// The library stores raw pixels; the model stores PNGs, so
/// export decodes the PNG back to RGBA rows (the layout
/// `encode_sprite_png` produced on import — a sprite that came
/// from this format therefore returns with identical pixels).
struct ExportSpriteImage {
    name: String,
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

/// The sprite half of an `.stknds` export, planned up front:
/// the library images to write (project assets referenced by
/// at least one exportable sprite track, in asset order), the
/// project-asset → library-index map, and the per-track
/// instance id to write (tracks are re-ided after the figure
/// slots, exactly as the importer keys figure slots by id).
struct SpriteExportPlan {
    library: Vec<ExportSpriteImage>,
    asset_map: Vec<Option<usize>>,
    track_ids: Vec<Option<i32>>,
}

/// The key a sprite track contributes to main frame `frame`,
/// if it has one the format can carry: the `.stknds` timeline
/// is a sequence of whole frames, so a key must sit exactly on
/// an integer frame inside the timeline and carry finite
/// values (the reader rejects non-finite floats). A track
/// with duplicate keys on one frame contributes the last one,
/// mirroring the importer's same-frame replacement rule.
fn sprite_key_for_frame(track: &SpriteInstance, frame: usize) -> Option<InstanceKey> {
    track.keys.iter().rev().copied().find(|k| {
        k.frame.is_finite()
            && k.frame >= 0.0
            && k.frame.fract() == 0.0
            && k.frame <= usize::MAX as f32
            && k.frame as usize == frame
            && k.x.is_finite()
            && k.y.is_finite()
            && k.scale.is_finite()
            && k.angle.is_finite()
            && k.opacity.is_finite()
            && k.scale_y.is_finite()
            && k.pivot_x.is_finite()
            && k.pivot_y.is_finite()
    })
}

/// Decode one project asset's PNG for the sprite library.
/// `None` when the bytes are not a decodable image, or the
/// image exceeds the library's own limits (the reader caps
/// each dimension at 32768 and the pixel count at 64M, and
/// names at the string reader's 1M count) — such an asset has
/// no valid library entry, so its tracks are omitted rather
/// than written as a file the importer would reject.
fn decode_export_sprite_image(asset: &ImageAsset) -> Option<ExportSpriteImage> {
    if asset.name.len() > 1024 * 1024 {
        return None;
    }
    let reader = image::ImageReader::new(std::io::Cursor::new(&asset.png[..]))
        .with_guessed_format()
        .ok()?;
    let (width, height) = reader.into_dimensions().ok()?;
    if width == 0 || height == 0 || width > 32768 || height > 32768 {
        return None;
    }
    if (width as u64) * (height as u64) > 64_000_000 {
        return None;
    }
    let rgba = image::load_from_memory(&asset.png).ok()?.to_rgba8();
    if rgba.dimensions() != (width, height) {
        return None;
    }
    let pixels = rgba.into_raw();
    if pixels.len() > MAX_PAYLOAD {
        return None;
    }
    Some(ExportSpriteImage {
        name: asset.name.clone(),
        width,
        height,
        pixels,
    })
}

/// Plan the sprite export: which assets enter the library and
/// which tracks get instance ids. A track is exportable when
/// its asset is in range and enters the library, and it has at
/// least one key [`sprite_key_for_frame`] accepts.
fn sprite_export_plan(project: &Project) -> SpriteExportPlan {
    let mut referenced = vec![false; project.assets.len()];
    let mut has_key = vec![false; project.sprites.len()];
    for (ti, track) in project.sprites.iter().enumerate() {
        if track.asset >= project.assets.len() {
            continue;
        }
        if (0..project.frames.len()).any(|f| sprite_key_for_frame(track, f).is_some()) {
            has_key[ti] = true;
            referenced[track.asset] = true;
        }
    }
    let mut library = Vec::new();
    let mut asset_map = vec![None; project.assets.len()];
    for (ai, asset) in project.assets.iter().enumerate() {
        if !referenced[ai] || library.len() >= MAX_ASSETS {
            continue;
        }
        if let Some(img) = decode_export_sprite_image(asset) {
            asset_map[ai] = Some(library.len());
            library.push(img);
        }
    }
    let mut track_ids = vec![None; project.sprites.len()];
    let mut next_id = project.figures.len() as i64 + 1;
    for (ti, track) in project.sprites.iter().enumerate() {
        if !has_key[ti] || asset_map[track.asset].is_none() || next_id > i32::MAX as i64 {
            continue;
        }
        track_ids[ti] = Some(next_id as i32);
        next_id += 1;
    }
    SpriteExportPlan {
        library,
        asset_map,
        track_ids,
    }
}

/// Sprite library image body at 431/117 (field order per
/// `read_sprite_image_body`). Exported images are always RGBA;
/// the LIBRARY pivot is written centred because the importer
/// does not retain library pivots — the per-key anchor rides
/// on the instance records (`write_sprite_instance`).
fn write_sprite_image_body(w: &mut BeWriter, img: &ExportSpriteImage) {
    w.flag(true); // hasAlpha (pixels are RGBA)
    w.flag(false); // flag1
    w.f32(0.5); // pivotX
    w.f32(0.5); // pivotY
    w.f32(0.0); // field2
    w.f32(0.0); // field3
    w.flag(true); // samplingFlag (build >= 9)
    w.i32(img.width as i32);
    w.i32(img.height as i32);
    w.i32(img.pixels.len() as i32); // pixelBytes
    w.v.extend_from_slice(&img.pixels);
}

/// One type-2 (sprite image) instance record at 431/117
/// (field order per `read_instance`'s sprite branch), carrying
/// one model key: position and rotation verbatim; scaleX
/// `|key.scale|` and scaleY the key's effective second axis
/// (`eff_scale_y` — a follow-`scale` key writes its uniform
/// value, as before); the key's pivot and flips written back
/// (Wave X, P-6 — the fields the importer used to discard);
/// the style block carries the key's opacity; every still-
/// unmodelled field at its neutral default.
fn write_sprite_instance(w: &mut BeWriter, library_index: usize, id: i32, key: &InstanceKey) {
    w.i32(2); // instance type: sprite image
    w.i32(library_index as i32); // asset index
    w.i32(id); // instance id (importer track key)
    write_style(w, key.opacity);
    w.flag(false); // useParentFilters
    w.f32(1.0); // scaleXYRatio (editor value; the reader ignores it)
    w.f32(key.scale.abs()); // scaleX
    w.f32(key.eff_scale_y().abs()); // scaleY
    for _ in 0..8 {
        w.f32(0.0); // corner distortions (build >= 110)
    }
    w.f32(key.pivot_x); // pivotX (the key's anchor)
    w.f32(key.pivot_y); // pivotY
    w.f32(key.x);
    w.f32(key.y);
    w.f32(key.angle); // rotation
    w.flag(key.flip_x); // flipX
    w.flag(key.flip_y); // flipY
    w.flag(false); // persistent
    w.i8(0); // tweenMode (build >= 78)
    w.flag(true); // visibleInOnionSkin (build >= 15)
    w.i32(0); // lockState
    w.i32(0); // smartStretchState (0: no parameter block)
}

/// Export the project as a Stick Nodes `.stknds` file: version
/// 431 / build 117 (the newest layout the original's decoder
/// knows — `KNOWN_PROJECT_LAYOUT` tops out at exactly this
/// profile), raw (uncompressed) payload.
///
/// The figure library embeds each project figure via
/// `crate::nodes::export_nodes_figure`; every frame then holds
/// one type-0 instance per figure — stable instance id
/// (figure index + 1, which is what the importer keys timeline
/// slots by), scale 1.0, the figure's colour, and one pose
/// record per segment, inverting `convert_pose` (the root
/// record's local angle is the pose's first angle minus 90;
/// other records carry the pose angles verbatim, with the
/// definition's lengths and thicknesses). Frame durations are
/// encoded exactly for integer values via the settings delay
/// (see `write_main_settings`).
///
/// **Sprites are written** (Wave 13 A5): the assets referenced
/// by sprite tracks are decoded back to raw pixels and written
/// as sprite-library images, and every track key that sits on
/// an integer frame inside the timeline is written as a type-2
/// instance on that frame (see `sprite_export_plan`). A
/// reimport therefore restores the model tracks — asset and
/// keys — but not the native detail the importer already
/// collapsed: group instances return as plain image instances
/// of their resolved member, and corner distortions and
/// smart-stretch parameters have no model source and are
/// written at neutral defaults (flips, the second scale
/// axis, pivots and style opacity DO have a model source
/// since Wave X (P-6) — the keys carry them and
/// `write_sprite_instance` writes them back).
/// Keys the whole-frame grammar cannot address (fractional,
/// out-of-timeline or non-finite) and assets it cannot hold
/// (undecodable or over its size limits) are omitted — the
/// exporter returns bytes, not a `Result`, so it cannot
/// report them.
///
/// Written as absent, because the model-to-format mapping for
/// them is either unmodelled by the importer or has no faithful
/// source in a `Project` (each is a real loss on reimport):
///
/// * **Texts** — the per-frame counts are written empty, so
///   text tracks are dropped.
/// * **Joins** — written as none. (Poses that were join-baked
///   at import are ordinary poses by now and survive as such.)
/// * **Movieclips** — the clip library is empty; a figure with
///   clip linkage exports as a plain posed figure and the
///   `ClipDef` content is dropped.
/// * **Camera** — each frame's record carries the frame's
///   evaluated camera (sparse keys and any follow target are
///   baked to per-frame values; see `write_camera`). A project
///   with no camera data keeps the default record per frame,
///   byte-identical to before. Camera wobble and tracking
///   bindings have no model source and are written absent.
/// * Sounds, weather, particles, FX clips, audio clips,
///   strokes, graph channels, per-frame easing and bends, the
///   stage size (the grammar has no stage field; the importer
///   always uses 960×720), and figure hidden/locked/opacity
///   state have no slots and are dropped.
///
/// Further rounding: the fps is stored as an integer (rounded),
/// fractional frame durations round UP to whole ticks (the
/// format counts frames), and figure definitions inherit the
/// documented losses of `export_nodes_figure` (node-1 length,
/// origin-parented nodes, fractional thickness).
pub fn export_nodes_animation(project: &Project) -> Vec<u8> {
    let fps = project.fps.round().clamp(1.0, 1000.0);
    let layout = compile_pose_layout(431, 117, false);
    let sprites = sprite_export_plan(project);
    let sprite_track_count = sprites.track_ids.iter().flatten().count();
    let mut w = BeWriter { v: Vec::new() };
    w.i32(431); // version
    w.i32(117); // build
    w.string(&project.name);
    write_project_header(
        &mut w,
        fps as i32,
        project.figures.len(),
        sprites.library.len(),
        sprite_track_count,
    );
    // Figure library.
    w.i32(project.figures.len() as i32);
    for fig in &project.figures {
        w.string(&fig.name);
        // The embedded figure's own header colour is the
        // instance colour (that is where convert() sources it).
        let mut def = fig.def.clone();
        if let Some(node) = def.nodes.get_mut(1) {
            if node.color.is_none() {
                node.color = Some(fig.color);
            }
        }
        w.v.extend_from_slice(&crate::nodes::export_nodes_figure(&def, &fig.name));
    }
    w.i32(0); // sound library (v >= 160): none
    // Sprite library (v >= 400): the images the exported
    // sprite tracks reference, in project-asset order.
    w.i32(sprites.library.len() as i32);
    for img in &sprites.library {
        w.v.push(0); // type: image
        w.string(&img.name);
        write_sprite_image_body(&mut w, img);
    }
    w.i32(0); // movieclip library (v >= 300): none
    // Pose records are stored per node in the embedded figure's
    // scan order (tree preorder), which the per-figure traversal
    // shared with the `.nodes` writer provides.
    let preorders: Vec<Vec<usize>> = project
        .figures
        .iter()
        .map(|f| crate::nodes::nodes_preorder(&f.def))
        .collect();
    // Main timeline.
    w.i32(project.frames.len() as i32);
    for (fi, frame) in project.frames.iter().enumerate() {
        write_main_settings(&mut w, project.background, frame.duration, fps);
        // This frame's sprite appearances: one type-2 record
        // per exported track holding a key on exactly this
        // frame, in track order (the order the importer keys
        // first appearances by).
        let sprite_here: Vec<(usize, i32, InstanceKey)> = project
            .sprites
            .iter()
            .enumerate()
            .filter_map(|(ti, track)| {
                let id = sprites.track_ids[ti]?;
                let key = sprite_key_for_frame(track, fi)?;
                let library_index = sprites.asset_map[track.asset]?;
                Some((library_index, id, key))
            })
            .collect();
        w.i32((project.figures.len() + sprite_here.len()) as i32); // instance count
        for (fi, fig) in project.figures.iter().enumerate() {
            let pose = frame
                .poses
                .get(fi)
                .cloned()
                .unwrap_or_else(|| fig.def.rest_pose([0.0, 0.0]));
            w.i32(0); // instance type: figure
            w.i32(fi as i32); // asset index
            w.i32(fi as i32 + 1); // stable instance id (slot key)
            w.f32(1.0); // instance scale
            write_style(&mut w, 1.0);
            w.f32(pose.root[0]);
            w.f32(pose.root[1]);
            w.color(fig.color);
            w.flag(false); // isLocked
            w.flag(false); // persistent
            w.flag(true); // visibleInOnionSkin
            w.flag(false); // tweenColors
            w.i8(0); // tweenMode (build >= 78)
            w.flag(false); // scaleJoinedFigures
            w.flag(false); // pushJoinedFigures
            w.flag(false); // setStateOfJoins
            w.i32(0); // joinsState
            w.flag(false); // useParentFilters
            w.i32(-1); // lockedNodeId
            w.f32(0.0); // lockedNodeX
            w.f32(0.0); // lockedNodeY
            // One pose record per library segment, in scan
            // order, inverting convert_pose: the raw root's
            // local angle is the pose's first angle minus 90,
            // every other node's is its pose angle verbatim.
            for &k in &preorders[fi] {
                let node = &fig.def.nodes[k];
                let local = if k == 1 {
                    pose.angles.get(1).copied().unwrap_or(0.0) - 90.0
                } else {
                    pose.angles.get(k).copied().unwrap_or(0.0)
                };
                write_pose_record(
                    &mut w,
                    &layout,
                    local,
                    node.length,
                    node.thickness.round() as i32,
                );
            }
        }
        for (library_index, id, key) in &sprite_here {
            write_sprite_instance(&mut w, *library_index, *id, key);
        }
        w.i32(0); // joins: none
        w.i32(0); // texts: none
        // The frame's evaluated camera, baked per frame (the
        // format stores a record per frame); a project with no
        // camera keys and no follow target writes the default
        // record, byte-identical to earlier exports.
        let cam = if project.camera.is_empty() && project.camera_follow.is_none() {
            None
        } else {
            let (x, y, zoom) = project.camera_at(fi as f32);
            Some((x, y, zoom, project.camera_angle_at(fi as f32)))
        };
        write_camera(&mut w, cam);
    }
    write_trailer(&mut w);
    w.v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::evaluate;

    fn hex_decode(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }

    /// The original repo's synthetic corpus figure (431/117):
    /// root + one rounded segment (id 7, length 80, thickness 4).
    /// Same bytes as the `.nodes` importer's corpus fixture.
    fn modern_nodes() -> Vec<u8> {
        let hex: String = [
            "000001af000000753f80000000000000ff000000000000000000000000000000",
            "00000000000000000000003f8000000000000000000000000000000000000000",
            "0000000000000000000000000000000000000000000000000000000000000000",
            "0000000000000000000000000000000000000000000000000000000000000000",
            "0000000000000000000000000001000000000000000007000000000000000000",
            "000000000000000000000000003f80000042a0000042a0000000000004000000",
            "0400000000000000000000000000000000000000000000000000000000000000",
            "0000000000000000000000000000000000000000000000000000000000000000",
            "000000000000000000000000000000000000000000000000",
        ]
        .concat();
        hex_decode(&hex)
    }

    /// Big-endian byte builder mirroring the packers used by the
    /// original's own test suite.
    #[derive(Default)]
    struct B {
        v: Vec<u8>,
    }
    impl B {
        fn i8(&mut self, x: i8) {
            self.v.push(x as u8);
        }
        fn i16(&mut self, x: i16) {
            self.v.extend_from_slice(&x.to_be_bytes());
        }
        fn i32(&mut self, x: i32) {
            self.v.extend_from_slice(&x.to_be_bytes());
        }
        fn f32(&mut self, x: f32) {
            self.v.extend_from_slice(&x.to_be_bytes());
        }
        fn flag(&mut self, x: bool) {
            self.v.push(x as u8);
        }
        fn color(&mut self, a: u8, bl: u8, g: u8, r: u8) {
            self.v.extend_from_slice(&[a, bl, g, r]);
        }
        fn string(&mut self, s: &str) {
            self.i32(s.len() as i32);
            self.v.extend_from_slice(s.as_bytes());
        }
        fn raw(&mut self, bytes: &[u8]) {
            self.v.extend_from_slice(bytes);
        }
    }

    /// Instance style block at 431/117 (79 bytes of defaults),
    /// field order per `readStyle`.
    fn style_431_117(b: &mut B) {
        style_431_117_with_opacity(b, 1.0);
    }

    /// `style_431_117` with a caller-chosen opacity (the one
    /// style field the model retains, Wave X P-6).
    fn style_431_117_with_opacity(b: &mut B, opacity: f32) {
        b.f32(opacity); // opacity
        b.f32(0.0); // blur
        b.flag(false); // motionBlur
        b.flag(false); // omnidirectionalBlur
        b.i32(0); // motionBlurAngle
        b.f32(1.0); // saturation
        b.i32(0); // pixelation
        b.color(0, 0, 0, 0); // tintColor
        b.f32(0.0); // tintAmount
        b.f32(0.0); // hueShift
        b.flag(false); // invert
        b.f32(0.0); // glow
        b.color(0, 0, 0, 0); // glowColor
        b.f32(1.0); // glowIntensity
        b.color(0, 0, 0, 0); // outlineColor
        b.f32(0.0); // outlineAlpha
        b.f32(0.0); // outlineThickness
        b.color(0, 0, 0, 0); // dropShadowColor
        b.f32(0.0); // dropShadowAlpha
        b.i32(0); // dropShadowDistance
        b.i32(0); // dropShadowAngle
        b.f32(0.0); // dropShadowBlur
    }

    /// One pose record at 431/117 (83 bytes), field order per
    /// `compilePoseLayout(431, 117, false)`.
    fn pose_431_117(b: &mut B, local_angle: f32, length: f32, thickness: i32) {
        let start = b.v.len();
        b.flag(false); // is_stretchy
        b.flag(false); // is_floaty
        b.flag(false); // use_segment_color
        b.flag(false); // use_circle_outline
        b.flag(false); // circle_is_hollow
        b.flag(false); // use_gradient
        b.flag(false); // reverse_gradient
        b.i16(0); // gradient_mode
        b.flag(false); // use_segment_scale
        b.f32(1.0); // scale
        b.f32(length);
        b.i32(thickness);
        b.f32(local_angle);
        b.f32(local_angle); // default_local_angle
        b.f32(0.0); // default_angle
        b.color(255, 0, 0, 0);
        b.color(255, 0, 0, 0); // gradient_color
        b.color(255, 0, 0, 0); // circle_outline_color
        b.flag(false); // use_polyfill_color
        b.color(255, 0, 0, 0); // polyfill_color
        b.i16(0); // right_triangle_direction
        b.i32(0); // segment_curve_radius
        b.flag(false); // curve_circulization
        b.i16(0); // segment_curve_polyfill_precision
        b.f32(0.0); // trapezoid_thickness_start
        b.f32(0.0); // trapezoid_thickness_end
        b.flag(false); // trapezoid_is_rounded_start
        b.flag(false); // trapezoid_is_rounded_end
        b.f32(0.5); // trapezoid_top_thickness_ratio
        b.f32(0.0); // angle_lock_offset
        b.flag(false); // is_drag_locked
        b.f32(0.0); // drag_lock_angle
        assert_eq!(b.v.len() - start, 83, "pose record size at 431/117");
    }

    /// A `.nodemc` packed byte-by-byte from the grammar: one
    /// figure, one clip frame, one figure instance (id 5) at
    /// (300, 200) with the segment posed at 45 degrees. The
    /// identical byte stream was independently accepted by the
    /// original's own decoder (679 bytes, exact EOF, poses
    /// [0, 45] / lengths [0, 80]).
    fn hand_packed_clip() -> Vec<u8> {
        let mut b = B::default();
        b.i32(431); // version
        b.i32(117); // build (read when version >= 402)
        b.string("Clip A");
        b.i32(1); // frameCount
        b.i32(1); // figureCount
        b.i32(0); // spriteCount (v >= 400)
        b.i32(0); // soundCount
        b.i32(0); // thumbnailCount
        b.i32(0); // thumbnailWidth
        b.i32(0); // thumbnailHeight
        b.i32(24); // settings.fps
        b.i32(2); // settings.tweenCount
        b.flag(false); // settings.autoTween
        b.flag(false); // settings.clipFlag
        b.i32(6); // settings.nextInstanceId
        b.string("Synthetic rig");
        b.raw(&modern_nodes());
        // One clip frame (readClipFrame).
        b.f32(1.0); // volume
        b.f32(0.0); // pan (v >= 310)
        b.f32(1.0); // pitch
        b.i32(-1); // soundIndex
        b.i32(1); // instance count
        b.i32(0); // instance type (read when v >= 400 in clip context)
        b.i32(0); // assetIndex
        b.i32(5); // header id
        b.f32(1.0); // header scale (type 0)
        style_431_117(&mut b);
        b.f32(300.0); // x
        b.f32(200.0); // y
        b.color(255, 255, 255, 255); // white
        b.flag(false); // isLocked
        b.flag(false); // persistent
        b.flag(true); // visibleInOnionSkin
        b.flag(false); // tweenColors
        b.i8(0); // tweenMode (i8 when build >= 78)
        b.flag(false); // scaleJoinedFigures
        b.flag(false); // pushJoinedFigures
        b.flag(false); // setStateOfJoins
        b.i32(0); // joinsState
        b.flag(false); // useParentFilters
        b.i32(-1); // lockedNodeId
        b.f32(0.0); // lockedNodeX
        b.f32(0.0); // lockedNodeY
        pose_431_117(&mut b, 0.0, 0.0, 4); // root pose
        pose_431_117(&mut b, 45.0, 80.0, 4); // segment pose
        b.i32(0); // joins count
        b.v
    }

    #[test]
    fn hand_packed_nodemc_imports_with_pose() {
        let bytes = hand_packed_clip();
        assert_eq!(bytes.len(), 679);
        let project = import_nodes_animation(&bytes).unwrap();
        assert_eq!(project.name, "Clip A");
        assert_eq!(project.fps, 24.0);
        assert_eq!(project.frames.len(), 1);
        assert_eq!(project.frames[0].duration, 1.0);
        assert_eq!(project.figures.len(), 1);
        let fig = &project.figures[0];
        assert_eq!(fig.name, "Synthetic rig");
        assert_eq!(fig.color, 0xFFFFFF);
        assert_eq!(fig.def.nodes.len(), 3); // origin + root + segment
        assert!((fig.def.nodes[2].length - 80.0).abs() < 0.01);
        let pose = &project.frames[0].poses[0];
        assert_eq!(pose.root, [300.0, 200.0]);
        // Stick Nodes absolute angle 45: endpoint = root + 80*(cos45, -sin45).
        let pos = evaluate(&fig.def, pose);
        assert!((pos[2][0] - 356.5685).abs() < 0.01, "x {}", pos[2][0]);
        assert!((pos[2][1] - 143.4315).abs() < 0.01, "y {}", pos[2][1]);
    }

    /// The exact construction of the original test suite's
    /// `syntheticClip` helper (tests/sticknodes-import.test.mjs):
    /// a 431/117 clip whose single frame holds no instances.
    fn their_synthetic_clip() -> Vec<u8> {
        let mut b = B::default();
        b.i32(431);
        b.i32(117);
        b.string("Synthetic clip");
        b.i32(1); // frameCount
        b.i32(1); // figureCount
        b.i32(0); // spriteCount
        b.i32(0); // soundCount
        b.i32(0); // thumbnailCount
        b.i32(0); // thumbnailWidth
        b.i32(0); // thumbnailHeight
        b.i32(24); // fps
        b.i32(2); // tweenCount
        b.flag(false); // autoTween
        b.flag(false); // clipFlag
        b.i32(1); // nextInstanceId
        b.string("Synthetic rig");
        b.raw(&modern_nodes());
        b.f32(1.0); // volume
        b.f32(0.0); // pan
        b.f32(1.0); // pitch
        b.i32(-1); // soundIndex
        b.i32(0); // instance count
        b.i32(0); // joins count
        b.v
    }

    #[test]
    fn original_test_suite_clip_imports() {
        let bytes = their_synthetic_clip();
        assert_eq!(bytes.len(), 389); // length reported by their decoder run
        let project = import_nodes_animation(&bytes).unwrap();
        assert_eq!(project.name, "Synthetic clip");
        assert_eq!(project.fps, 24.0);
        assert_eq!(project.frames.len(), 1);
        assert_eq!(project.figures.len(), 0); // the frame holds no instances
        assert!(project.frames[0].poses.is_empty());
    }

    #[test]
    fn gzip_wrapped_clip_imports() {
        use std::io::Write as _;
        let raw = hand_packed_clip();
        let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        enc.write_all(&raw).unwrap();
        let gz = enc.finish().unwrap();
        // Plain gzip container.
        let project = import_nodes_animation(&gz).unwrap();
        assert_eq!(project.name, "Clip A");
        assert_eq!(project.figures.len(), 1);
        // 9-byte project prefix + gzip container.
        let mut prefixed = vec![1, 2, 3, 4, 5, 6, 7, 8, 9];
        prefixed.extend_from_slice(&gz);
        let project = import_nodes_animation(&prefixed).unwrap();
        assert_eq!(project.name, "Clip A");
        assert_eq!(project.frames[0].poses[0].root, [300.0, 200.0]);
    }

    #[test]
    fn fallback_convert_matches_nodes_importer() {
        let bytes = modern_nodes();
        let scan = scan_figure(&bytes, 0).unwrap();
        assert_eq!(scan.end, bytes.len());
        let (primary, primary_color) = crate::nodes::import_nodes_figure(&bytes).unwrap();
        let (fallback, fallback_color) = convert_fallback(&scan);
        assert_eq!(primary_color, fallback_color);
        assert_eq!(primary.nodes.len(), fallback.nodes.len());
        for (a, b) in primary.nodes.iter().zip(fallback.nodes.iter()) {
            assert_eq!(a.name, b.name);
            assert_eq!(a.parent, b.parent);
            assert!((a.length - b.length).abs() < 1e-4);
            assert!((a.rest_angle - b.rest_angle).abs() < 1e-4);
            assert!((a.thickness - b.thickness).abs() < 1e-4);
            assert_eq!(a.color, b.color);
            assert_eq!(a.shape, b.shape);
            assert_eq!(a.media, b.media);
        }
    }

    fn base64_decode(s: &str) -> Vec<u8> {
        let mut table = [255u8; 256];
        for (i, c) in b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"
            .iter()
            .enumerate()
        {
            table[*c as usize] = i as u8;
        }
        let mut out = Vec::new();
        let mut acc: u32 = 0;
        let mut nbits = 0;
        for c in s.bytes() {
            if c == b'=' {
                break;
            }
            let v = table[c as usize];
            if v == 255 {
                continue;
            }
            acc = (acc << 6) | v as u32;
            nbits += 6;
            if nbits >= 8 {
                nbits -= 8;
                out.push((acc >> nbits) as u8);
            }
        }
        out
    }

    /// The original repo's synthetic `.stknds` corpus
    /// (tests/sticknodes-synthetic.json). Ground truth below was
    /// produced by running the original's own decoder over these
    /// exact bytes: 417/35 (`base64`), 431/117 (`latestBase64`) and
    /// 300/1 with a legacy background (`legacyBase64`) projects,
    /// all named "Different project — λ", fps 24, two 2-node
    /// figures, one main frame, background #785a3c, figure
    /// instances at (100, 100) and (100, 140).
    #[test]
    fn corpus_stknds_projects_import() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../StykAnimate-App/tests/sticknodes-synthetic.json"
        );
        let Ok(text) = std::fs::read_to_string(path) else {
            return; // corpus not present (skip gracefully)
        };
        let fixture: serde_json::Value = serde_json::from_str(&text).unwrap();
        // (key, expected segment endpoint for slot 0 in frame 0)
        let cases = [
            ("base64", [169.282, 60.0]),
            ("legacyBase64", [169.282, 60.0]),
            ("latestBase64", [180.0, 100.0]),
        ];
        for (key, want_end) in cases {
            let bytes = base64_decode(fixture[key].as_str().unwrap());
            let project = import_nodes_animation(&bytes)
                .unwrap_or_else(|e| panic!("{key} failed to import: {e}"));
            assert_eq!(project.name, "Different project — λ", "{key}");
            assert_eq!(project.fps, 24.0, "{key}");
            assert_eq!(project.background, 0x785A3C, "{key}");
            assert_eq!(project.frames.len(), 1, "{key}");
            assert_eq!(project.figures.len(), 2, "{key}");
            assert_eq!(project.figures[0].name, "Renamed asset 0 — è", "{key}");
            assert_eq!(project.figures[1].name, "Renamed asset 1 — è", "{key}");
            assert_eq!(project.figures[0].color, 0xFFFFFF, "{key}");
            // Both library clips hold two figures + sprites, so
            // neither converts to a single-figure ClipDef.
            assert_eq!(project.clips.len(), 0, "{key}");
            let pose0 = &project.frames[0].poses[0];
            let pose1 = &project.frames[0].poses[1];
            assert_eq!(pose0.root, [100.0, 100.0], "{key}");
            // Instance 1 is JOINED to instance 0 at node 7 — the
            // original's own decoder reports the join record
            // {child: 1, parentId: 100, nodeId: 7} in all three
            // variants (relativeRotation 0 in the 431/117 one) —
            // so the bake supersedes its stored root (100, 140)
            // with the host's segment end, the same point as
            // slot 0's endpoint below. (The original *adapter*
            // would keep the stored root here: at ~106 units it
            // counts as an offset attachment. This importer bakes
            // the pin; see §17 of the spec.)
            assert!(
                (pose1.root[0] - want_end[0]).abs() < 0.05
                    && (pose1.root[1] - want_end[1]).abs() < 0.05,
                "{key}: joined child root {:?}, want host node end {:?}",
                pose1.root,
                want_end
            );
            let pos = evaluate(&project.figures[0].def, pose0);
            assert!(
                (pos[2][0] - want_end[0]).abs() < 0.05 && (pos[2][1] - want_end[1]).abs() < 0.05,
                "{key}: segment endpoint {:?}, want {:?}",
                pos[2],
                want_end
            );
        }
    }

    /// Sprite library image body at 431/117 (field order per
    /// `read_sprite_image_body`).
    fn sprite_image_body_431_117(b: &mut B, alpha: bool, w: i32, h: i32, pixels: &[u8]) {
        b.flag(alpha);
        b.flag(false); // flag1
        b.f32(0.5); // pivotX
        b.f32(0.5); // pivotY
        b.f32(0.0); // field2
        b.f32(0.0); // field3
        b.flag(true); // samplingFlag (build >= 9)
        b.i32(w);
        b.i32(h);
        b.i32(pixels.len() as i32); // pixelBytes
        b.raw(pixels);
    }

    /// Sprite instance record at 431/117 (field order per
    /// `read_instance`'s sprite branch).
    #[allow(clippy::too_many_arguments)]
    fn sprite_instance_431_117(
        b: &mut B,
        ty: i32,
        asset: i32,
        id: i32,
        scale_x: f32,
        scale_y: f32,
        x: f32,
        y: f32,
        rotation: f32,
        group_frame: Option<i32>,
    ) {
        b.i32(ty);
        b.i32(asset);
        b.i32(id);
        style_431_117(b);
        b.flag(false); // useParentFilters
        b.f32(1.0); // scaleXYRatio
        b.f32(scale_x);
        b.f32(scale_y);
        for _ in 0..8 {
            b.f32(0.0); // corner distortions (build >= 110)
        }
        b.f32(0.5); // pivotX
        b.f32(0.5); // pivotY
        b.f32(x);
        b.f32(y);
        b.f32(rotation);
        b.flag(false); // flipX
        b.flag(false); // flipY
        b.flag(false); // persistent
        b.i8(0); // tweenMode (build >= 78)
        b.flag(true); // visibleInOnionSkin (build >= 15)
        b.i32(0); // lockState
        b.i32(0); // smartStretchState
        if let Some(gf) = group_frame {
            b.i32(gf); // groupFrame (type 3 only)
        }
    }

    /// Sprite instance record at 431/117 carrying the P-6
    /// enrichment: a non-default style opacity, a non-centre
    /// pivot and the flip flags set — the fields the importer
    /// used to read and discard.
    #[allow(clippy::too_many_arguments)]
    fn sprite_instance_enriched_431_117(
        b: &mut B,
        asset: i32,
        id: i32,
        scale_x: f32,
        scale_y: f32,
        x: f32,
        y: f32,
        rotation: f32,
        opacity: f32,
        flip_x: bool,
        flip_y: bool,
        pivot_x: f32,
        pivot_y: f32,
    ) {
        b.i32(2); // type: sprite image
        b.i32(asset);
        b.i32(id);
        style_431_117_with_opacity(b, opacity);
        b.flag(false); // useParentFilters
        b.f32(1.0); // scaleXYRatio
        b.f32(scale_x);
        b.f32(scale_y);
        for _ in 0..8 {
            b.f32(0.0); // corner distortions (build >= 110)
        }
        b.f32(pivot_x);
        b.f32(pivot_y);
        b.f32(x);
        b.f32(y);
        b.f32(rotation);
        b.flag(flip_x);
        b.flag(flip_y);
        b.flag(false); // persistent
        b.i8(0); // tweenMode (build >= 78)
        b.flag(true); // visibleInOnionSkin (build >= 15)
        b.i32(0); // lockState
        b.i32(0); // smartStretchState
    }

    /// A `.stknds` project at 431/117 whose single frame holds
    /// one enriched sprite instance: image "Px" (2x1), id 7,
    /// scale (2.0, 0.5), at (400, 300), opacity 0.5, flipX set,
    /// pivot (0.25, 0.75).
    fn hand_packed_project_with_enriched_sprite() -> Vec<u8> {
        let mut b = B::default();
        b.i32(431); // version
        b.i32(117); // build (v >= 401)
        b.string("Proj E");
        project_header_431_117(&mut b);
        b.i32(0); // figure count
        b.i32(0); // sound count (v >= 160)
        b.i32(1); // sprite library count (v >= 400)
        b.raw(&[0]); // image
        b.string("Px");
        sprite_image_body_431_117(&mut b, false, 2, 1, &[255, 0, 0, 0, 0, 255]);
        b.i32(0); // clip count (v >= 300)
        b.i32(1); // frame count
        main_settings_431_117(&mut b);
        b.i32(1); // instance count
        sprite_instance_enriched_431_117(
            &mut b, 0, 7, 2.0, 0.5, 400.0, 300.0, 0.0, 0.5, true, false, 0.25, 0.75,
        );
        b.i32(0); // joins count
        b.i32(0); // text count
        camera_431_117(&mut b);
        trailer_431_117(&mut b);
        b.v
    }

    /// Wave X (P-6): the four .stknds sprite-key losses GAP
    /// documented — flipX/flipY, scaleY, pivotX/pivotY and the
    /// style opacity — are modelled on `InstanceKey` now: the
    /// enriched record imports with every field on its key,
    /// and the exporter writes them back, so a re-imported
    /// export carries the identical key.
    #[test]
    fn stknds_enriched_sprite_key_imports_and_roundtrips() {
        let project = import_nodes_animation(&hand_packed_project_with_enriched_sprite())
            .expect("enriched project imports");
        assert_eq!(project.sprites.len(), 1);
        assert_eq!(project.sprites[0].keys.len(), 1);
        let k = &project.sprites[0].keys[0];
        key_eq(k, 0.0, 400.0, 300.0, 2.0, 0.0);
        assert!((k.scale_y - 0.5).abs() < 0.01, "key scale_y {}", k.scale_y);
        assert!((k.opacity - 0.5).abs() < 0.01, "key opacity {}", k.opacity);
        assert!(k.flip_x, "flipX retained");
        assert!(!k.flip_y, "flipY retained (unset)");
        assert!(
            (k.pivot_x - 0.25).abs() < 0.01 && (k.pivot_y - 0.75).abs() < 0.01,
            "key pivot ({}, {})",
            k.pivot_x,
            k.pivot_y
        );
        // And back out: the exported file re-imports with the
        // identical enriched key — the round trip is closed.
        let back = import_nodes_animation(&export_nodes_animation(&project))
            .expect("exported project re-imports");
        assert_eq!(back.sprites.len(), 1);
        assert_eq!(back.sprites[0].keys.len(), 1);
        let k2 = &back.sprites[0].keys[0];
        key_eq(k2, 0.0, 400.0, 300.0, 2.0, 0.0);
        assert!((k2.scale_y - 0.5).abs() < 0.01, "round-tripped scale_y {}", k2.scale_y);
        assert!((k2.opacity - 0.5).abs() < 0.01, "round-tripped opacity {}", k2.opacity);
        assert!(k2.flip_x && !k2.flip_y, "round-tripped flips");
        assert!(
            (k2.pivot_x - 0.25).abs() < 0.01 && (k2.pivot_y - 0.75).abs() < 0.01,
            "round-tripped pivot ({}, {})",
            k2.pivot_x,
            k2.pivot_y
        );
    }

    /// `hand_packed_clip` plus a sprite: the library holds one 2x2
    /// RGBA image "Dot" (source id 42) and the single clip frame
    /// holds the figure instance and a sprite instance (id 9,
    /// scale 3, at (500, 120), rotated 30 degrees).
    fn hand_packed_clip_with_sprite() -> Vec<u8> {
        let mut b = B::default();
        b.i32(431); // version
        b.i32(117); // build
        b.string("Clip S");
        b.i32(1); // frameCount
        b.i32(1); // figureCount
        b.i32(1); // spriteCount (v >= 400)
        b.i32(0); // soundCount
        b.i32(0); // thumbnailCount
        b.i32(0); // thumbnailWidth
        b.i32(0); // thumbnailHeight
        b.i32(24); // settings.fps
        b.i32(2); // settings.tweenCount
        b.flag(false); // settings.autoTween
        b.flag(false); // settings.clipFlag
        b.i32(10); // settings.nextInstanceId
        b.string("Synthetic rig");
        b.raw(&modern_nodes());
        // Sprite library entry (after the figure library).
        b.raw(&[0]); // type: image
        b.string("Dot");
        b.i32(42); // sourceLibraryId
        sprite_image_body_431_117(
            &mut b,
            true,
            2,
            2,
            &[
                255, 0, 0, 255, // red
                0, 255, 0, 255, // green
                0, 0, 255, 255, // blue
                255, 255, 0, 255, // yellow
            ],
        );
        // One clip frame (readClipFrame).
        b.f32(1.0); // volume
        b.f32(0.0); // pan
        b.f32(1.0); // pitch
        b.i32(-1); // soundIndex
        b.i32(2); // instance count
        // Figure instance (same fields as hand_packed_clip).
        b.i32(0); // type
        b.i32(0); // assetIndex
        b.i32(5); // header id
        b.f32(1.0); // header scale
        style_431_117(&mut b);
        b.f32(300.0); // x
        b.f32(200.0); // y
        b.color(255, 255, 255, 255); // white
        b.flag(false); // isLocked
        b.flag(false); // persistent
        b.flag(true); // visibleInOnionSkin
        b.flag(false); // tweenColors
        b.i8(0); // tweenMode
        b.flag(false); // scaleJoinedFigures
        b.flag(false); // pushJoinedFigures
        b.flag(false); // setStateOfJoins
        b.i32(0); // joinsState
        b.flag(false); // useParentFilters
        b.i32(-1); // lockedNodeId
        b.f32(0.0); // lockedNodeX
        b.f32(0.0); // lockedNodeY
        pose_431_117(&mut b, 0.0, 0.0, 4); // root pose
        pose_431_117(&mut b, 45.0, 80.0, 4); // segment pose
        // Sprite instance.
        sprite_instance_431_117(&mut b, 2, 0, 9, 3.0, 3.0, 500.0, 120.0, 30.0, None);
        b.i32(0); // joins count
        b.v
    }

    #[test]
    fn nodemc_sprite_instance_imports_as_asset_and_track() {
        let project = import_nodes_animation(&hand_packed_clip_with_sprite()).unwrap();
        assert_eq!(project.figures.len(), 1, "figure instance still imports");
        // The sprite's raw RGBA pixels became a PNG asset, losslessly.
        assert_eq!(project.assets.len(), 1);
        assert_eq!(project.assets[0].name, "Dot");
        let img = image::load_from_memory(&project.assets[0].png)
            .unwrap()
            .to_rgba8();
        assert_eq!((img.width(), img.height()), (2, 2));
        assert_eq!(*img.get_pixel(0, 0), image::Rgba([255, 0, 0, 255]));
        assert_eq!(*img.get_pixel(1, 0), image::Rgba([0, 255, 0, 255]));
        assert_eq!(*img.get_pixel(0, 1), image::Rgba([0, 0, 255, 255]));
        assert_eq!(*img.get_pixel(1, 1), image::Rgba([255, 255, 0, 255]));
        // And its per-frame transform became a SpriteInstance track.
        assert_eq!(project.sprites.len(), 1);
        let track = &project.sprites[0];
        assert_eq!(track.asset, 0);
        assert_eq!(track.keys.len(), 1);
        let k = track.keys[0];
        assert_eq!(k.frame, 0.0);
        assert_eq!((k.x, k.y), (500.0, 120.0));
        assert_eq!(k.scale, 3.0);
        assert_eq!(k.angle, 30.0);
        assert!(project.texts.is_empty());
    }

    /// Project header at 431/117 (field order per
    /// `read_project_header`).
    fn project_header_431_117(b: &mut B) {
        b.flag(false); // editorFlagD (build >= 16)
        b.flag(false); // editorFlagE (v >= 244)
        b.string(""); // projectCredit
        b.color(0, 0, 0, 0); // editorColor
        b.i32(24); // fps
        b.i32(2); // tweenCount
        b.flag(false); // editorFlagB
        b.flag(false); // editorFlagZ
        b.flag(false); // editorFlagA
        b.flag(false); // editorFlagC
        b.flag(false); // autoTween
        b.flag(false); // timelineFlagC
        b.i32(0); // selectedFrame
        b.i32(5); // nextTextId
        b.i32(10); // nextInstanceId
        b.i32(3); // nextLibraryId
        b.i32(0); // fontUsage count (v >= 400)
        b.i32(0); // editor list 1
        b.i32(0); // editor list 2
        b.i32(0); // editor list 3
        b.i32(0); // outer map count
    }

    /// Main-frame settings at 431/117 (field order per
    /// `read_main_settings`); background #785a3c.
    fn main_settings_431_117(b: &mut B) {
        b.flag(false); // isTweened
        b.flag(false); // useImageBackground
        b.flag(false); // useGradient
        b.flag(false); // expandedGradient
        b.flag(false); // stopSounds
        b.flag(false); // isDelayed
        b.f32(0.0); // delaySeconds (present whenever v >= 200)
        b.flag(false); // slowMotion
        b.i32(0); // slowMotionTweenFrames
        b.flag(false); // repeating
        b.i32(0); // repeatBackFrames
        b.i32(0); // repeatLoops
        b.flag(false); // tweenWhenRepeating
        b.flag(false); // cameraDuringDelay
        b.flag(false); // easeIn
        b.flag(false); // easeOut
        b.i16(0); // interpolation
        b.color(255, 0x3C, 0x5A, 0x78); // background -> 0x785A3C
        b.color(255, 0, 0, 0); // backgroundGradientColor
        b.i32(-1); // soundIndex
        b.f32(1.0); // soundVolume
        b.f32(0.0); // soundPan
        b.f32(1.0); // soundPitch
    }

    /// Frame camera at 431/117 (field order per `read_camera`).
    fn camera_431_117(b: &mut B) {
        b.f32(1.0); // zoom
        b.f32(0.0); // x
        b.f32(0.0); // y
        b.f32(0.0); // rotation
        b.flag(false); // wobbleXY
        b.flag(false); // wobbleRotation
        b.flag(false); // wobbleScale
        b.i32(0); // wobbleXYIntensity
        b.f32(0.0); // wobbleRotationIntensity
        b.f32(0.0); // wobbleSpeed
        b.flag(false); // widescreen
        b.i32(0); // bindings count
        b.i32(0); // secondary bindings count
    }

    /// Frame camera at 431/117 with explicit values (field
    /// order per `read_camera`).
    fn camera_431_117_with(b: &mut B, zoom: f32, x: f32, y: f32, rotation: f32) {
        b.f32(zoom); // zoom
        b.f32(x); // x
        b.f32(y); // y
        b.f32(rotation); // rotation
        b.flag(false); // wobbleXY
        b.flag(false); // wobbleRotation
        b.flag(false); // wobbleScale
        b.i32(0); // wobbleXYIntensity
        b.f32(0.0); // wobbleRotationIntensity
        b.f32(0.0); // wobbleSpeed
        b.flag(false); // widescreen
        b.i32(0); // bindings count
        b.i32(0); // secondary bindings count
    }

    /// Main-frame text at 431/117 (field order per `read_text`),
    /// red, unrotated, no shadow.
    fn text_431_117(b: &mut B, id: i32, x: f32, y: f32, scale: f32, text: &str, align: i32) {
        b.i32(id); // id (v >= 176)
        b.i32(0); // fontIndex (v >= 244)
        b.f32(x);
        b.f32(y);
        b.f32(scale);
        b.string(text);
        b.color(255, 0, 0, 255); // red -> 0xFF0000
        b.i32(align);
        b.f32(0.0); // rotation (v >= 237)
        b.f32(1.0); // opacity
        b.flag(false); // aboveBars (v >= 310)
        b.flag(false); // isLocked
        b.flag(false); // hasShadow (v >= 242)
        b.color(0, 0, 0, 0); // shadowColor
    }

    /// Project trailer at 431/117 (field order per `read_trailer`).
    fn trailer_431_117(b: &mut B) {
        b.i32(0); // tweenLinks count (v >= 237)
        b.i32(0); // selectionMode (v >= 251)
        for _ in 0..5 {
            b.f32(0.0); // parameter[i]
        }
        b.i32(0); // selectedAsset
        b.flag(false); // flag_i
        b.flag(false); // flag_j
        b.flag(false); // flag_k
        b.flag(false); // flag_l
        b.flag(false); // flag_m
        b.flag(false); // flag_n
        b.flag(false); // flag_o
        b.flag(false); // flag_p
        b.flag(false); // flag_r (build >= 84)
        b.flag(false); // flag_s
        b.flag(false); // flag_t
        b.flag(false); // flag_u
        b.i8(0); // mode (build >= 79)
        for _ in 0..7 {
            b.color(0, 0, 0, 0); // palette
        }
        b.i32(0); // editorPairs count (v >= 319)
    }

    /// A `.stknds` project packed byte-by-byte at 431/117: no
    /// figures; a sprite library with one 2x1 RGB image "Px"
    /// (red, blue) and a group wrapping it; two main frames.
    /// Frame 0 holds an image sprite (id 7), a group sprite
    /// (id 8), and two texts (id 3 centred anchor, id 4
    /// start-anchored); frame 1 moves sprite 7 and text 3.
    fn hand_packed_project_with_sprites_and_text() -> Vec<u8> {
        let mut b = B::default();
        b.i32(431); // version
        b.i32(117); // build (v >= 401)
        b.string("Proj S");
        project_header_431_117(&mut b);
        b.i32(0); // figure count
        b.i32(0); // sound count (v >= 160)
        b.i32(2); // sprite library count (v >= 400)
        b.raw(&[0]); // image
        b.string("Px");
        sprite_image_body_431_117(&mut b, false, 2, 1, &[255, 0, 0, 0, 0, 255]);
        b.raw(&[1]); // group
        b.string("Grp");
        b.i32(1); // member count
        b.i32(0); // member: image 0
        b.i32(0); // clip count (v >= 300)
        b.i32(2); // frame count
        // Frame 0.
        main_settings_431_117(&mut b);
        b.i32(2); // instance count
        sprite_instance_431_117(&mut b, 2, 0, 7, 2.0, 2.0, 400.0, 300.0, 0.0, None);
        sprite_instance_431_117(&mut b, 3, 1, 8, 1.0, 1.0, 100.0, 100.0, 45.0, Some(0));
        b.i32(0); // joins count
        b.i32(2); // text count
        text_431_117(&mut b, 3, 200.0, 500.0, 2.0, "HI", 0);
        text_431_117(&mut b, 4, 200.0, 560.0, 2.0, "HI", 8);
        camera_431_117(&mut b);
        // Frame 1.
        main_settings_431_117(&mut b);
        b.i32(1); // instance count
        sprite_instance_431_117(&mut b, 2, 0, 7, 4.0, 4.0, 440.0, 300.0, 90.0, None);
        b.i32(0); // joins count
        b.i32(1); // text count
        text_431_117(&mut b, 3, 260.0, 500.0, 2.0, "HI", 0);
        camera_431_117(&mut b);
        trailer_431_117(&mut b);
        b.v
    }

    /// A `.stknds` project packed byte-by-byte at 431/117: no
    /// figures, no sprites, two main frames — frame 0 carries a
    /// moved camera (zoom 2, anchor (100, 50), rotation 30),
    /// frame 1 the default camera record.
    fn hand_packed_project_with_camera() -> Vec<u8> {
        let mut b = B::default();
        b.i32(431); // version
        b.i32(117); // build (v >= 401)
        b.string("Proj C");
        project_header_431_117(&mut b);
        b.i32(0); // figure count
        b.i32(0); // sound count (v >= 160)
        b.i32(0); // sprite library count (v >= 400)
        b.i32(0); // clip count (v >= 300)
        b.i32(2); // frame count
        // Frame 0.
        main_settings_431_117(&mut b);
        b.i32(0); // instance count
        b.i32(0); // joins count
        b.i32(0); // text count
        camera_431_117_with(&mut b, 2.0, 100.0, 50.0, 30.0);
        // Frame 1.
        main_settings_431_117(&mut b);
        b.i32(0); // instance count
        b.i32(0); // joins count
        b.i32(0); // text count
        camera_431_117(&mut b);
        trailer_431_117(&mut b);
        b.v
    }

    fn key_eq(k: &InstanceKey, frame: f32, x: f32, y: f32, scale: f32, angle: f32) {
        assert_eq!(k.frame, frame, "key frame");
        assert!((k.x - x).abs() < 0.01, "key x {} want {x}", k.x);
        assert!((k.y - y).abs() < 0.01, "key y {} want {y}", k.y);
        assert!((k.scale - scale).abs() < 0.01, "key scale {}", k.scale);
        assert!((k.angle - angle).abs() < 0.01, "key angle {}", k.angle);
    }

    #[test]
    fn stknds_sprites_and_texts_import_as_tracks() {
        let project = import_nodes_animation(&hand_packed_project_with_sprites_and_text())
            .expect("hand-packed project imports");
        assert_eq!(project.name, "Proj S");
        assert_eq!(project.background, 0x785A3C);
        assert_eq!(project.frames.len(), 2);
        assert!(project.figures.is_empty());
        // One image in the library -> one PNG asset (RGB expanded
        // to opaque RGBA): red, blue.
        assert_eq!(project.assets.len(), 1);
        assert_eq!(project.assets[0].name, "Px");
        let img = image::load_from_memory(&project.assets[0].png)
            .unwrap()
            .to_rgba8();
        assert_eq!((img.width(), img.height()), (2, 1));
        assert_eq!(*img.get_pixel(0, 0), image::Rgba([255, 0, 0, 255]));
        assert_eq!(*img.get_pixel(1, 0), image::Rgba([0, 0, 255, 255]));
        // Two sprite tracks, first-appearance order. The group
        // instance resolves through groupFrame 0 to the image.
        assert_eq!(project.sprites.len(), 2);
        assert_eq!(project.sprites[0].asset, 0);
        assert_eq!(project.sprites[0].keys.len(), 2);
        key_eq(&project.sprites[0].keys[0], 0.0, 400.0, 300.0, 2.0, 0.0);
        key_eq(&project.sprites[0].keys[1], 1.0, 440.0, 300.0, 4.0, 90.0);
        assert_eq!(project.sprites[1].asset, 0);
        assert_eq!(project.sprites[1].keys.len(), 1);
        key_eq(&project.sprites[1].keys[0], 0.0, 100.0, 100.0, 1.0, 45.0);
        // Two text tracks. Base size 48 (the adapter's font size);
        // the per-frame scale rides on the keys.
        assert_eq!(project.texts.len(), 2);
        let t0 = &project.texts[0];
        assert_eq!(t0.content, "HI");
        assert_eq!(t0.size, 48.0);
        assert_eq!(t0.color, 0xFF0000);
        assert_eq!(t0.keys.len(), 2);
        key_eq(&t0.keys[0], 0.0, 200.0, 500.0, 2.0, 0.0);
        key_eq(&t0.keys[1], 1.0, 260.0, 500.0, 2.0, 0.0);
        // align 8 anchors at the text start: "HI" at size 96 is
        // (4*2-1) * 96/5 = 134.4 wide, so the centre sits half a
        // width to the right of the anchor.
        let t1 = &project.texts[1];
        assert_eq!(t1.keys.len(), 1);
        key_eq(&t1.keys[0], 0.0, 267.2, 560.0, 2.0, 0.0);
    }

    /// Wave 13 A5 verdict, pinned: the `.stknds` grammar DOES
    /// carry sprite instances — the importer above proves the
    /// library + type-2/3 instance records exist — so the
    /// exporter writes model sprite tracks back (it previously
    /// wrote the sprite library as empty and dropped every
    /// track). What round-trips is the MODEL: the asset table
    /// entry (name and pixels; the PNG bytes too, both sides
    /// being `encode_sprite_png` over the same RGBA rows) and
    /// each track's keys. What does NOT round-trip is native
    /// detail the importer already collapsed before export
    /// ever sees it: the group instance below returns as a
    /// plain image instance of its resolved member. Texts
    /// remain dropped (the exporter's documented stance).
    #[test]
    fn stknds_sprite_tracks_roundtrip_through_export() {
        let project = import_nodes_animation(&hand_packed_project_with_sprites_and_text())
            .expect("hand-packed project imports");
        assert_eq!(project.sprites.len(), 2);
        let back = import_nodes_animation(&export_nodes_animation(&project))
            .expect("exported project re-imports");
        // The asset survives: same entry, same name, same PNG
        // bytes (identical pixels through the shared encoder).
        assert_eq!(back.assets.len(), 1);
        assert_eq!(back.assets, project.assets);
        // Both tracks survive with their keys exact: the
        // two-key image track, and the group track — now an
        // image track of the same resolved member, same key.
        assert_eq!(back.sprites.len(), 2);
        assert_eq!(back.sprites[0].asset, 0);
        assert_eq!(back.sprites[0].keys.len(), 2);
        key_eq(&back.sprites[0].keys[0], 0.0, 400.0, 300.0, 2.0, 0.0);
        key_eq(&back.sprites[0].keys[1], 1.0, 440.0, 300.0, 4.0, 90.0);
        assert_eq!(back.sprites[1].asset, 0);
        assert_eq!(back.sprites[1].keys.len(), 1);
        key_eq(&back.sprites[1].keys[0], 0.0, 100.0, 100.0, 1.0, 45.0);
        // The boundary of the fix: texts are still written as
        // absent, so they do not come back.
        assert!(back.texts.is_empty());
    }

    /// Wave 13 A5, the format-limit half of the verdict: the
    /// `.stknds` timeline addresses whole frames only, so a
    /// sprite key on a fractional frame — or past the end of
    /// the timeline — has no record to ride on and is omitted
    /// on export (the exporter returns bytes, not a `Result`,
    /// so the omission is silent; it is pinned here so it is
    /// at least a known, tested limit rather than a surprise).
    #[test]
    fn stknds_sprite_export_omits_fractional_and_out_of_timeline_keys() {
        let mut project = import_nodes_animation(&hand_packed_project_with_sprites_and_text())
            .expect("hand-packed project imports");
        project.sprites[0].keys.push(InstanceKey {
            frame: 0.5,
            x: 1.0,
            y: 2.0,
            scale: 1.0,
            angle: 0.0,
            opacity: 1.0,
            flip_x: false,
            flip_y: false,
            scale_y: 0.0,
            pivot_x: 0.5,
            pivot_y: 0.5,
        });
        project.sprites[0].keys.push(InstanceKey {
            frame: 9.0,
            x: 3.0,
            y: 4.0,
            scale: 1.0,
            angle: 0.0,
            opacity: 1.0,
            flip_x: false,
            flip_y: false,
            scale_y: 0.0,
            pivot_x: 0.5,
            pivot_y: 0.5,
        });
        let back = import_nodes_animation(&export_nodes_animation(&project))
            .expect("exported project re-imports");
        assert_eq!(back.sprites.len(), 2);
        let frames: Vec<f32> = back.sprites[0].keys.iter().map(|k| k.frame).collect();
        assert_eq!(frames, vec![0.0, 1.0]);
        // The surviving keys are the originals, unmoved.
        key_eq(&back.sprites[0].keys[0], 0.0, 400.0, 300.0, 2.0, 0.0);
        key_eq(&back.sprites[0].keys[1], 1.0, 440.0, 300.0, 4.0, 90.0);
    }

    #[test]
    fn clip_with_sprite_sibling_still_converts() {
        let bytes = modern_nodes();
        let scan = scan_figure(&bytes, 0).unwrap();
        let (def, color) = figure_def(&bytes, &scan);
        let figures = vec![LibFigure {
            name: "Rig".into(),
            scan,
            def,
            color,
        }];
        let fig_inst = |x: f32| RawInstance {
            ty: 0,
            asset: 0,
            id: 5,
            scale: 1.0,
            x,
            y: 200.0,
            color: 0xFFFFFF,
            poses: vec![
                PoseRec {
                    local_angle: 0.0,
                    length: 0.0,
                    scale: 1.0,
                    use_segment_scale: false,
                },
                PoseRec {
                    local_angle: 45.0,
                    length: 80.0,
                    scale: 1.0,
                    use_segment_scale: false,
                },
            ],
            scale_x: 1.0,
            opacity: 1.0,
            scale_y: 1.0,
            rotation: 0.0,
            flip_x: false,
            flip_y: false,
            pivot_x: 0.5,
            pivot_y: 0.5,
            group_frame: 0,
        };
        let sprite_inst = || RawInstance {
            ty: 2,
            asset: 0,
            id: 9,
            scale: 1.0,
            x: 10.0,
            y: 20.0,
            color: 0xFFFFFF,
            poses: Vec::new(),
            scale_x: 2.0,
            opacity: 1.0,
            scale_y: 2.0,
            rotation: 30.0,
            flip_x: false,
            flip_y: false,
            pivot_x: 0.5,
            pivot_y: 0.5,
            group_frame: 0,
        };
        let frame = |instances: Vec<RawInstance>| RawFrame {
            instances,
            joins: Vec::new(),
            settings: None,
            texts: Vec::new(),
            camera: None,
        };
        // One figure + one sprite per frame: converts (the sprite
        // is not carried into the ClipDef).
        let clip = RawClip {
            name: "C".into(),
            fps: 24,
            tween_count: 2,
            auto_tween: false,
            frames: vec![
                frame(vec![fig_inst(300.0), sprite_inst()]),
                frame(vec![fig_inst(320.0), sprite_inst()]),
            ],
        };
        let (def, asset, _) = convert_clip(&clip, &figures, 117).expect("clip converts");
        assert_eq!(asset, 0);
        assert_eq!(def.frames.len(), 2);
        // Two figure instances in a frame still block conversion.
        let clip2 = RawClip {
            frames: vec![frame(vec![fig_inst(300.0), fig_inst(310.0)])],
            ..clip
        };
        assert!(convert_clip(&clip2, &figures, 117).is_none());
    }

    /// One type-0 instance record at 431/117 for the Synthetic
    /// rig (root pose + segment pose), field order per
    /// `read_instance`.
    fn figure_instance_431_117(b: &mut B, id: i32, x: f32, y: f32, seg_angle: f32) {
        b.i32(0); // type (read when v >= 400 in clip context)
        b.i32(0); // assetIndex
        b.i32(id); // header id
        b.f32(1.0); // header scale (type 0)
        style_431_117(b);
        b.f32(x);
        b.f32(y);
        b.color(255, 255, 255, 255); // white
        b.flag(false); // isLocked
        b.flag(false); // persistent
        b.flag(true); // visibleInOnionSkin
        b.flag(false); // tweenColors
        b.i8(0); // tweenMode (build >= 78)
        b.flag(false); // scaleJoinedFigures
        b.flag(false); // pushJoinedFigures
        b.flag(false); // setStateOfJoins
        b.i32(0); // joinsState
        b.flag(false); // useParentFilters
        b.i32(-1); // lockedNodeId
        b.f32(0.0); // lockedNodeX
        b.f32(0.0); // lockedNodeY
        pose_431_117(b, 0.0, 0.0, 4); // root pose
        pose_431_117(b, seg_angle, 80.0, 4); // segment pose
    }

    /// A `.nodemc` whose single frame holds two figure instances
    /// — a host (id 5) at (300, 200) with its segment at 45
    /// degrees, and a child (id 6) stored far away at (50, 50) —
    /// plus one join record pinning the child to the host's
    /// segment node.
    fn hand_packed_clip_with_join() -> Vec<u8> {
        let scan = scan_figure(&modern_nodes(), 0).unwrap();
        let seg_id = scan.nodes[1].id;
        assert_eq!(seg_id, 7, "corpus rig segment id");
        let mut b = B::default();
        b.i32(431); // version
        b.i32(117); // build
        b.string("Clip J");
        b.i32(1); // frameCount
        b.i32(1); // figureCount
        b.i32(0); // spriteCount
        b.i32(0); // soundCount
        b.i32(0); // thumbnailCount
        b.i32(0); // thumbnailWidth
        b.i32(0); // thumbnailHeight
        b.i32(24); // fps
        b.i32(2); // tweenCount
        b.flag(false); // autoTween
        b.flag(false); // clipFlag
        b.i32(7); // nextInstanceId
        b.string("Synthetic rig");
        b.raw(&modern_nodes());
        // One clip frame (readClipFrame).
        b.f32(1.0); // volume
        b.f32(0.0); // pan
        b.f32(1.0); // pitch
        b.i32(-1); // soundIndex
        b.i32(2); // instance count
        figure_instance_431_117(&mut b, 5, 300.0, 200.0, 45.0);
        figure_instance_431_117(&mut b, 6, 50.0, 50.0, 0.0);
        b.i32(1); // joins count
        b.i32(1); // child: frame instance index 1
        b.i32(5); // parentId: the host's instance id
        b.i32(seg_id); // nodeId
        b.f32(0.0); // relativeRotation (build >= 53)
        b.v
    }

    #[test]
    fn nodemc_join_bakes_child_root_to_host_node_end() {
        let project = import_nodes_animation(&hand_packed_clip_with_join()).unwrap();
        assert_eq!(project.figures.len(), 2);
        let host_pose = &project.frames[0].poses[0];
        let child_pose = &project.frames[0].poses[1];
        assert_eq!(host_pose.root, [300.0, 200.0]);
        // The child's stored root (50, 50) is superseded by the
        // host's segment end: (300, 200) + 80*(cos45, -sin45).
        assert!(
            (child_pose.root[0] - 356.5685).abs() < 0.01,
            "x {}",
            child_pose.root[0]
        );
        assert!(
            (child_pose.root[1] - 143.4315).abs() < 0.01,
            "y {}",
            child_pose.root[1]
        );
        // Only the root moved: the child's own segment (stored
        // angle 0) still extends along +x from the baked root.
        let pos = evaluate(&project.figures[1].def, child_pose);
        assert!((pos[2][0] - 436.5685).abs() < 0.01, "x {}", pos[2][0]);
        assert!((pos[2][1] - 143.4315).abs() < 0.01, "y {}", pos[2][1]);
    }

    fn raw_fig_inst(id: i32, x: f32, y: f32, seg_angle: f32) -> RawInstance {
        RawInstance {
            ty: 0,
            asset: 0,
            id,
            scale: 1.0,
            x,
            y,
            color: 0xFFFFFF,
            poses: vec![
                PoseRec {
                    local_angle: 0.0,
                    length: 0.0,
                    scale: 1.0,
                    use_segment_scale: false,
                },
                PoseRec {
                    local_angle: seg_angle,
                    length: 80.0,
                    scale: 1.0,
                    use_segment_scale: false,
                },
            ],
            scale_x: 1.0,
            opacity: 1.0,
            scale_y: 1.0,
            rotation: 0.0,
            flip_x: false,
            flip_y: false,
            pivot_x: 0.5,
            pivot_y: 0.5,
            group_frame: 0,
        }
    }

    fn raw_rig() -> Vec<LibFigure> {
        let bytes = modern_nodes();
        let scan = scan_figure(&bytes, 0).unwrap();
        let (def, color) = figure_def(&bytes, &scan);
        vec![LibFigure {
            name: "Rig".into(),
            scan,
            def,
            color,
        }]
    }

    fn join(child: usize, parent: usize, node_id: i32) -> RawJoin {
        RawJoin {
            child,
            parent,
            node_id,
            relative_rotation: None,
        }
    }

    #[test]
    fn join_chains_root_joins_and_fallbacks() {
        let figures = raw_rig();
        let root_id = figures[0].scan.nodes[0].id;
        let seg_id = figures[0].scan.nodes[1].id;
        // A at the origin aiming up (segment end (0, -80)); B
        // joined to A's segment, aiming +x (its segment end is
        // B's root + (80, 0)); C joined to B's segment; D joined
        // to A's root node.
        let frame = RawFrame {
            instances: vec![
                raw_fig_inst(1, 0.0, 0.0, 90.0),
                raw_fig_inst(2, 500.0, 500.0, 0.0),
                raw_fig_inst(3, 900.0, 900.0, 0.0),
                raw_fig_inst(4, 700.0, 700.0, 0.0),
            ],
            joins: vec![join(1, 0, seg_id), join(2, 1, seg_id), join(3, 0, root_id)],
            settings: None,
            texts: Vec::new(),
            camera: None,
        };
        let (figs, frames) = build_timeline(
            &[frame],
            &[1.0],
            &figures,
            &[],
            &HashMap::new(),
            &mut Vec::new(),
            &HashMap::new(),
            117,
        );
        assert_eq!(figs.len(), 4);
        let roots: Vec<[f32; 2]> = frames[0].poses.iter().map(|p| p.root).collect();
        assert_eq!(roots[0], [0.0, 0.0]);
        assert!(
            roots[1][0].abs() < 0.01 && (roots[1][1] + 80.0).abs() < 0.01,
            "B root {:?}",
            roots[1]
        );
        assert!(
            (roots[2][0] - 80.0).abs() < 0.01 && (roots[2][1] + 80.0).abs() < 0.01,
            "C root {:?}",
            roots[2]
        );
        assert_eq!(roots[3], [0.0, 0.0], "root-node join pins to the host root");

        // Unresolvable join (parent index out of range in a
        // hand-built frame): the child keeps its stored root.
        let frame = RawFrame {
            instances: vec![
                raw_fig_inst(1, 10.0, 10.0, 0.0),
                raw_fig_inst(2, 500.0, 500.0, 0.0),
            ],
            joins: vec![join(1, 99, seg_id)],
            settings: None,
            texts: Vec::new(),
            camera: None,
        };
        let (_, frames) = build_timeline(
            &[frame],
            &[1.0],
            &figures,
            &[],
            &HashMap::new(),
            &mut Vec::new(),
            &HashMap::new(),
            117,
        );
        assert_eq!(frames[0].poses[1].root, [500.0, 500.0]);
    }

    #[test]
    fn join_to_sprite_parent_and_sprite_child() {
        let figures = raw_rig();
        let seg_id = figures[0].scan.nodes[1].id;
        let sprite_inst = |id: i32, x: f32, y: f32| RawInstance {
            ty: 2,
            asset: 0,
            id,
            scale: 1.0,
            x,
            y,
            color: 0xFFFFFF,
            poses: Vec::new(),
            scale_x: 1.0,
            opacity: 1.0,
            scale_y: 1.0,
            rotation: 0.0,
            flip_x: false,
            flip_y: false,
            pivot_x: 0.5,
            pivot_y: 0.5,
            group_frame: 0,
        };
        // Figure child pinned to a sprite parent: the handle-0
        // rule (non-figure parents pin to their own position).
        let frame = RawFrame {
            instances: vec![sprite_inst(9, 33.0, 44.0), raw_fig_inst(2, 500.0, 500.0, 0.0)],
            joins: vec![join(1, 0, seg_id)],
            settings: None,
            texts: Vec::new(),
            camera: None,
        };
        let (_, frames) = build_timeline(
            &[frame],
            &[1.0],
            &figures,
            &[],
            &HashMap::new(),
            &mut Vec::new(),
            &HashMap::new(),
            117,
        );
        assert_eq!(frames[0].poses[0].root, [33.0, 44.0]);

        // Sprite child pinned to a figure host's segment end:
        // the sprite's key position is the baked root.
        let sprites = vec![SpriteLib::Image(SpriteImage {
            name: "Dot".into(),
            width: 1,
            height: 1,
            alpha: false,
            pixels: vec![255, 0, 0],
        })];
        let frame = RawFrame {
            instances: vec![raw_fig_inst(1, 0.0, 0.0, 90.0), sprite_inst(9, 7.0, 7.0)],
            joins: vec![join(1, 0, seg_id)],
            settings: None,
            texts: Vec::new(),
            camera: None,
        };
        let (tracks, _) = build_instance_tracks(&[frame], &sprites, &[Some(0)], &figures, 117);
        assert_eq!(tracks.len(), 1);
        let k = tracks[0].keys[0];
        assert!(
            k.x.abs() < 0.01 && (k.y + 80.0).abs() < 0.01,
            "sprite key ({}, {})",
            k.x,
            k.y
        );
    }

    #[test]
    fn joined_clip_still_does_not_convert() {
        let figures = raw_rig();
        let seg_id = figures[0].scan.nodes[1].id;
        let clip = RawClip {
            name: "J".into(),
            fps: 24,
            tween_count: 2,
            auto_tween: false,
            frames: vec![RawFrame {
                instances: vec![raw_fig_inst(5, 300.0, 200.0, 45.0)],
                joins: vec![join(0, 0, seg_id)],
                settings: None,
                texts: Vec::new(),
                camera: None,
            }],
        };
        assert!(convert_clip(&clip, &figures, 117).is_none());
    }

    #[test]
    fn read_joins_validates_like_the_original() {
        let figures = raw_rig();
        let seg_id = figures[0].scan.nodes[1].id;
        let insts = vec![
            raw_fig_inst(5, 0.0, 0.0, 0.0),
            raw_fig_inst(6, 9.0, 9.0, 0.0),
        ];
        let pack = |child: i32, parent: i32, node: i32| {
            let mut b = B::default();
            b.i32(1); // joins count
            b.i32(child);
            b.i32(parent);
            b.i32(node);
            b.f32(0.0); // relativeRotation at build 117
            b.v
        };
        // Happy path: the parent id resolves to an index.
        let bytes = pack(1, 5, seg_id);
        let mut r = Reader::new(&bytes);
        let joins = read_joins(&mut r, 117, true, &insts, &figures).unwrap();
        assert_eq!(joins.len(), 1);
        assert_eq!(
            (joins[0].child, joins[0].parent, joins[0].node_id),
            (1, 0, seg_id)
        );
        // Dangling parent id.
        let bytes = pack(1, 999, seg_id);
        let mut r = Reader::new(&bytes);
        assert!(read_joins(&mut r, 117, true, &insts, &figures).is_err());
        // Unknown node on a figure parent.
        let bytes = pack(1, 5, 4242);
        let mut r = Reader::new(&bytes);
        assert!(read_joins(&mut r, 117, true, &insts, &figures).is_err());
        // A cycle: instance 5 joins to 6 and 6 joins to 5.
        let mut b = B::default();
        b.i32(2); // joins count
        b.i32(0);
        b.i32(6);
        b.i32(seg_id);
        b.f32(0.0);
        b.i32(1);
        b.i32(5);
        b.i32(seg_id);
        b.f32(0.0);
        let mut r = Reader::new(&b.v);
        assert!(read_joins(&mut r, 117, true, &insts, &figures).is_err());
    }

    #[test]
    fn garbage_and_truncation_rejected() {
        assert!(import_nodes_animation(&[]).is_err());
        assert!(import_nodes_animation(&[1, 2, 3, 4]).is_err());
        assert!(import_nodes_animation(b"not a stick nodes file at all").is_err());
        let bytes = hand_packed_clip();
        assert!(import_nodes_animation(&bytes[..bytes.len() / 2]).is_err());
        let mut trailed = bytes.clone();
        trailed.push(7); // exact-EOF: one extra byte must fail
        assert!(import_nodes_animation(&trailed).is_err());
    }

    // ---- Wave 7: .stknds export ----

    fn assert_projects_match(a: &Project, b: &Project) {
        assert_eq!(a.name, b.name);
        assert_eq!(a.background, b.background);
        assert!((a.fps - b.fps).abs() < 0.01, "fps {} vs {}", a.fps, b.fps);
        assert_eq!(a.figures.len(), b.figures.len());
        for (fa, fb) in a.figures.iter().zip(b.figures.iter()) {
            assert_eq!(fa.name, fb.name);
            assert_eq!(fa.color, fb.color);
            assert_eq!(fa.def.nodes.len(), fb.def.nodes.len());
        }
        assert_eq!(a.frames.len(), b.frames.len());
        for (fa, fb) in a.frames.iter().zip(b.frames.iter()) {
            assert_eq!(fa.duration, fb.duration, "frame duration");
            assert_eq!(fa.poses.len(), fb.poses.len());
            for (pa, pb) in fa.poses.iter().zip(fb.poses.iter()) {
                assert!(
                    (pa.root[0] - pb.root[0]).abs() < 0.5,
                    "root x {} vs {}",
                    pa.root[0],
                    pb.root[0]
                );
                assert!(
                    (pa.root[1] - pb.root[1]).abs() < 0.5,
                    "root y {} vs {}",
                    pa.root[1],
                    pb.root[1]
                );
                assert_eq!(pa.angles.len(), pb.angles.len());
                for (x, y) in pa.angles.iter().zip(pb.angles.iter()) {
                    assert!(
                        (norm_deg(x - y)).abs() < 0.1,
                        "angle {x} vs {y}"
                    );
                }
            }
        }
    }

    #[test]
    fn stknds_export_roundtrips_poses_and_durations() {
        let mut project = Project::new_demo();
        project.name = "Wave 7".into();
        project.background = 0x334455;
        project.add_figure_with(FigureDef::quadruped(), "Quad", 0x5DB2FF, [300.0, 400.0]);
        project.frames[1].duration = 3.0;
        project.frames[3].duration = 2.0;
        let bytes = export_nodes_animation(&project);
        // Raw 431/117 payload.
        assert_eq!(&bytes[0..4], &431i32.to_be_bytes());
        assert_eq!(&bytes[4..8], &117i32.to_be_bytes());
        // Kept on disk for cross-verification with the original
        // app's JS project decoder (openProject).
        std::fs::write("/tmp/styk-export.stknds", &bytes).unwrap();
        let back = import_nodes_animation(&bytes).unwrap();
        assert_projects_match(&project, &back);
        // The delay-based duration encoding is exact for
        // integers, not approximate.
        let durations: Vec<f32> = back.frames.iter().map(|f| f.duration).collect();
        assert_eq!(durations, vec![1.0, 3.0, 1.0, 2.0, 1.0]);
        // A second round trip is a fixed point: the reimported
        // project's defs are in scan order already, so poses map
        // identically again.
        let back2 = import_nodes_animation(&export_nodes_animation(&back)).unwrap();
        assert_projects_match(&back, &back2);
    }

    #[test]
    fn stknds_export_empty_project_roundtrips() {
        let mut project = Project::new_demo();
        project.figures.clear();
        for frame in &mut project.frames {
            frame.poses.clear();
        }
        let back = import_nodes_animation(&export_nodes_animation(&project)).unwrap();
        assert!(back.figures.is_empty());
        assert_eq!(back.frames.len(), project.frames.len());
    }

    #[test]
    fn stknds_camera_records_import_as_dense_keys() {
        let project = import_nodes_animation(&hand_packed_project_with_camera())
            .expect("hand-packed camera project imports");
        // Frame 0's record is non-default, so keys are dense:
        // frame 0 is (100 + 960/2, 50 + 540/2, 2, 30) and the
        // default record on frame 1 is (0 + 960, 0 + 540, 1, 0).
        assert_eq!(project.camera.len(), 2);
        let k0 = &project.camera[0];
        assert_eq!(k0.frame, 0);
        assert!((k0.x - 580.0).abs() < 0.01, "key0 x {}", k0.x);
        assert!((k0.y - 320.0).abs() < 0.01, "key0 y {}", k0.y);
        assert!((k0.zoom - 2.0).abs() < 0.001, "key0 zoom {}", k0.zoom);
        assert!((k0.angle - 30.0).abs() < 0.001, "key0 angle {}", k0.angle);
        let k1 = &project.camera[1];
        assert_eq!(k1.frame, 1);
        assert!((k1.x - 960.0).abs() < 0.01, "key1 x {}", k1.x);
        assert!((k1.y - 540.0).abs() < 0.01, "key1 y {}", k1.y);
        assert!((k1.zoom - 1.0).abs() < 0.001, "key1 zoom {}", k1.zoom);
        assert!((k1.angle - 0.0).abs() < 0.001, "key1 angle {}", k1.angle);
    }

    #[test]
    fn stknds_default_camera_records_import_without_keys() {
        // Both frames of the sprites/texts project carry the
        // default camera record: no keys are created, exactly
        // as imports behaved before camera was modelled.
        let project = import_nodes_animation(&hand_packed_project_with_sprites_and_text())
            .expect("hand-packed project imports");
        assert!(project.camera.is_empty());
    }

    #[test]
    fn stknds_export_import_camera_round_trip() {
        let mut project = Project::new_demo();
        project.camera = vec![
            CameraKey {
                frame: 0,
                x: 480.0,
                y: 360.0,
                zoom: 1.0,
                angle: 0.0,
            },
            CameraKey {
                frame: 4,
                x: 672.0,
                y: 396.0,
                zoom: 2.0,
                angle: 30.0,
            },
        ];
        let back = import_nodes_animation(&export_nodes_animation(&project)).unwrap();
        // Keys come back dense (the format stores a record per
        // frame) and reproduce the evaluated camera at every
        // frame, including the interpolated middle frames.
        assert_eq!(back.camera.len(), project.frames.len());
        for f in 0..project.frames.len() {
            let (ax, ay, az) = project.camera_at(f as f32);
            let (bx, by, bz) = back.camera_at(f as f32);
            assert!((ax - bx).abs() < 0.05, "frame {f} x {ax} vs {bx}");
            assert!((ay - by).abs() < 0.05, "frame {f} y {ay} vs {by}");
            assert!((az - bz).abs() < 0.001, "frame {f} zoom {az} vs {bz}");
            let aa = project.camera_angle_at(f as f32);
            let ba = back.camera_angle_at(f as f32);
            assert!((aa - ba).abs() < 0.05, "frame {f} angle {aa} vs {ba}");
        }
    }

    #[test]
    fn stknds_export_without_camera_imports_without_keys() {
        let project = Project::new_demo();
        assert!(project.camera.is_empty());
        let back = import_nodes_animation(&export_nodes_animation(&project)).unwrap();
        assert!(back.camera.is_empty());
    }

    // ---- Wave 12 B7: media/asset truth on import ----

    /// A `.stknds` project packed byte-by-byte at 431/117 whose
    /// sprite library holds TWO images ("First" red, "Second"
    /// green) and a group wrapping the second; the single main
    /// frame instances the second image directly and the group.
    fn hand_packed_project_two_images_group() -> Vec<u8> {
        let mut b = B::default();
        b.i32(431); // version
        b.i32(117); // build (v >= 401)
        b.string("Proj M");
        project_header_431_117(&mut b);
        b.i32(0); // figure count
        b.i32(0); // sound count (v >= 160)
        b.i32(3); // sprite library count (v >= 400)
        b.raw(&[0]); // image
        b.string("First");
        sprite_image_body_431_117(&mut b, false, 1, 1, &[255, 0, 0]);
        b.raw(&[0]); // image
        b.string("Second");
        sprite_image_body_431_117(&mut b, false, 1, 1, &[0, 255, 0]);
        b.raw(&[1]); // group
        b.string("G");
        b.i32(1); // member count
        b.i32(1); // member: image 1 ("Second")
        b.i32(0); // clip count (v >= 300)
        b.i32(1); // frame count
        main_settings_431_117(&mut b);
        b.i32(2); // instance count
        // Direct reference to the SECOND library image.
        sprite_instance_431_117(&mut b, 2, 1, 11, 1.0, 1.0, 10.0, 20.0, 0.0, None);
        // Group instance: groupFrame 0 displays "Second".
        sprite_instance_431_117(&mut b, 3, 2, 12, 1.0, 1.0, 30.0, 40.0, 0.0, Some(0));
        b.i32(0); // joins count
        b.i32(0); // text count
        camera_431_117(&mut b);
        trailer_431_117(&mut b);
        b.v
    }

    #[test]
    fn stknds_asset_indices_follow_library_order() {
        let project = import_nodes_animation(&hand_packed_project_two_images_group())
            .expect("hand-packed two-image project imports");
        // Imported asset count matches the decoded media: one
        // asset per image entry, in library order; the group
        // contributes none.
        assert_eq!(project.assets.len(), 2);
        assert_eq!(project.assets[0].name, "First");
        assert_eq!(project.assets[1].name, "Second");
        let first = image::load_from_memory(&project.assets[0].png)
            .unwrap()
            .to_rgba8();
        assert_eq!(*first.get_pixel(0, 0), image::Rgba([255, 0, 0, 255]));
        let second = image::load_from_memory(&project.assets[1].png)
            .unwrap()
            .to_rgba8();
        assert_eq!(*second.get_pixel(0, 0), image::Rgba([0, 255, 0, 255]));
        // Both tracks — the direct reference and the one resolved
        // through the group — point at asset 1, the image their
        // library entries actually name. Every track index is
        // valid against the imported table.
        assert_eq!(project.sprites.len(), 2);
        for track in &project.sprites {
            assert!(track.asset < project.assets.len());
            assert_eq!(track.asset, 1);
        }
        key_eq(&project.sprites[0].keys[0], 0.0, 10.0, 20.0, 1.0, 0.0);
        key_eq(&project.sprites[1].keys[0], 0.0, 30.0, 40.0, 1.0, 0.0);
    }

    #[test]
    fn imported_figure_nodes_never_carry_media() {
        // The figure grammar encodes no node-attached media (see
        // the module docs), so every conversion path — the
        // primary `.nodes` conversion, the local fallback for
        // pre-160 figures, and a full container import — must
        // yield media-free nodes rather than inventing or
        // dropping attachments.
        let bytes = modern_nodes();
        let scan = scan_figure(&bytes, 0).unwrap();
        let (primary, _) = figure_def(&bytes, &scan);
        assert!(primary.nodes.iter().all(|n| n.media.is_none()));
        let (fallback, _) = convert_fallback(&scan);
        assert!(fallback.nodes.iter().all(|n| n.media.is_none()));
        let project = import_nodes_animation(&hand_packed_clip()).unwrap();
        assert_eq!(project.figures.len(), 1);
        for fig in &project.figures {
            assert!(fig.def.nodes.iter().all(|n| n.media.is_none()));
        }
    }

    /// A `.stknds` project whose only sprite library entry is a
    /// zero-width image (0 pixels), instanced once in frame 0.
    fn hand_packed_project_zero_size_sprite() -> Vec<u8> {
        let mut b = B::default();
        b.i32(431); // version
        b.i32(117); // build (v >= 401)
        b.string("Proj Z");
        project_header_431_117(&mut b);
        b.i32(0); // figure count
        b.i32(0); // sound count (v >= 160)
        b.i32(1); // sprite library count (v >= 400)
        b.raw(&[0]); // image
        b.string("Empty");
        sprite_image_body_431_117(&mut b, false, 0, 2, &[]);
        b.i32(0); // clip count (v >= 300)
        b.i32(1); // frame count
        main_settings_431_117(&mut b);
        b.i32(1); // instance count
        sprite_instance_431_117(&mut b, 2, 0, 3, 1.0, 1.0, 50.0, 60.0, 0.0, None);
        b.i32(0); // joins count
        b.i32(0); // text count
        camera_431_117(&mut b);
        trailer_431_117(&mut b);
        b.v
    }

    #[test]
    fn zero_size_sprite_image_is_skipped_not_scrambled() {
        // A zero-dimension image parses (its byte count, 0,
        // matches its dimensions) but cannot be PNG-encoded and
        // has no pixels to display: the import stays lenient —
        // no asset, no track, and no other index disturbed —
        // rather than failing the whole project or pointing a
        // track at a different image.
        let project = import_nodes_animation(&hand_packed_project_zero_size_sprite())
            .expect("zero-size sprite project imports");
        assert!(project.assets.is_empty());
        assert!(project.sprites.is_empty());
        assert_eq!(project.frames.len(), 1);
    }
}
