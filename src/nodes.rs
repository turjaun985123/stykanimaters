//! Stick Nodes `.nodes` figure import.
//!
//! The byte grammar is ported from the original app's decoder
//! (`src/sticknodes/decoder/decoder.mjs`, itself based on
//! sticknodes-rs) — see `docs/NODES-SPEC.md` for the full spec with
//! source references. Nothing here is guessed: every field, gate
//! and ordering traces to `parseNodes` / `layoutFigure` in that
//! code.
//!
//! A `.nodes` file is a single figure definition (no timeline), so
//! the import produces a one-figure, one-frame project holding the
//! figure's rest pose. Polyfills and connections are parsed
//! byte-exactly but not modelled; connector nodes and angle-lock
//! solver fields are consumed and the stored rest pose is kept,
//! mirroring the original importer's stance.

use crate::model::{
    Easing, FigureDef, FigureInstance, Frame, NodeDef, Project, Shape,
};
use std::collections::HashSet;
use std::io::Read as _;

const MAX_NODES: usize = 10_000;
const MAX_DEPTH: usize = 1_024;
const MAX_INPUT: usize = 32 * 1024 * 1024;
const MAX_PAYLOAD: usize = 512 * 1024 * 1024;

// ---------- byte reader (mirrors the decoder's Reader) ----------

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
    le: bool,
}

impl<'a> Reader<'a> {
    fn new(data: &'a [u8], le: bool) -> Self {
        Reader { data, pos: 0, le }
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        if self.pos + n > self.data.len() {
            return Err(format!("unexpected end of file at {}", self.pos));
        }
        let out = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(out)
    }
    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }
    fn i8(&mut self) -> Result<i8, String> {
        Ok(self.u8()? as i8)
    }
    fn i16(&mut self) -> Result<i16, String> {
        let b = self.take(2)?;
        Ok(if self.le {
            i16::from_le_bytes([b[0], b[1]])
        } else {
            i16::from_be_bytes([b[0], b[1]])
        })
    }
    fn i32(&mut self) -> Result<i32, String> {
        let b = self.take(4)?;
        Ok(if self.le {
            i32::from_le_bytes([b[0], b[1], b[2], b[3]])
        } else {
            i32::from_be_bytes([b[0], b[1], b[2], b[3]])
        })
    }
    fn f32(&mut self) -> Result<f32, String> {
        let v = f32::from_bits(self.i32()? as u32);
        if !v.is_finite() {
            return Err(format!("non-finite float at {}", self.pos - 4));
        }
        Ok(v)
    }
    /// One byte; any nonzero value is true.
    fn flag(&mut self) -> Result<bool, String> {
        Ok(self.u8()? != 0)
    }
    /// Four bytes in fixed [alpha, blue, green, red] order,
    /// returned as 0xRRGGBB (alpha dropped, like the pivot path).
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
}

// ---------- raw figure ----------

struct RawNode {
    parent: Option<usize>,
    kind: i8,
    id: i32,
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

struct RawFigure {
    version: i32,
    build: i32,
    scale: f32,
    color: u32,
    nodes: Vec<RawNode>,
}

fn modern(version: i32, build: i32, b: i32) -> bool {
    version >= 403 && build >= b
}

fn kind_name(kind: i8) -> &'static str {
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

/// Read one node record. Field order and version/build gates follow
/// `readNode` in the original decoder exactly.
#[allow(clippy::too_many_arguments)]
fn read_node(
    r: &mut Reader,
    version: i32,
    build: i32,
    parent: Option<usize>,
    connector: bool,
    seq: usize,
) -> Result<RawNode, String> {
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
        r.flag()?; // connector reversed
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
    r.flag()?; // is_static
    r.flag()?; // is_stretchy
    if modern(version, build, 48) {
        r.flag()?; // is_floaty
    }
    if version >= 248 {
        r.flag()?; // is_smart_stretch
    }
    if version >= 252 {
        r.flag()?; // do_not_apply_smart_stretch
    }
    if modern(version, build, 50) {
        r.flag()?; // smart_stretch_reset_impulse
    }
    let use_segment_color = r.flag()?;
    if version >= 256 {
        r.flag()?; // use_circle_outline
    }
    if modern(version, build, 21) {
        r.flag()?; // circle_is_hollow
    }
    if version >= 176 {
        r.flag()?; // use_gradient
        r.flag()?; // reverse_gradient
    }
    if modern(version, build, 20) {
        r.i16()?; // gradient_mode
    }
    let use_segment_scale = r.flag()?;
    r.f32()?; // local_x (not used by figure layout)
    r.f32()?; // local_y
    let scale = r.f32()?;
    r.f32()?; // default_length
    let length = r.f32()?;
    r.i32()?; // default_thickness
    let thickness = r.i32()?;
    if version >= 320 {
        r.i32()?; // segment_curve_radius_and_default_curve_radius
    }
    if modern(version, build, 20) {
        r.flag()?; // curve_circulization
    }
    if modern(version, build, 21) {
        r.i16()?; // segment_curve_polyfill_precision
    }
    if version >= 256 {
        r.flag()?; // half_arc
        r.i16()?; // right_triangle_direction
    }
    if version >= 300 {
        r.flag()?; // triangle_upside_down
    }
    if modern(version, build, 36) {
        // trapezoid thicknesses: i32 before build 64, f32 from 64
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
            r.flag()?; // use_trapezoid_thickness_start
            r.flag()?; // use_trapezoid_thickness_end
        }
    }
    if version >= 256 && build != 36 {
        r.f32()?; // trapezoid_top_thickness_ratio
    }
    if modern(version, build, 36) {
        r.flag()?; // trapezoid_is_rounded_start
        r.flag()?; // trapezoid_is_rounded_end
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
    if modern(version, build, 39) {
        r.flag()?; // is_angle_locked
        if build <= 50 {
            r.f32()?; // reserved_angle_lock_39_50
        }
    }
    if modern(version, build, 51) {
        r.flag()?; // angle_lock_is_main_node
        if build <= 56 {
            r.f32()?; // angle_lock_offset_minuend
            r.f32()?; // angle_lock_offset_subtrahend
        }
    }
    if modern(version, build, 57) {
        r.f32()?; // angle_lock_offset
    }
    if modern(version, build, 63) {
        r.f32()?; // angle_lock_relative_start
    }
    if modern(version, build, 67) {
        r.f32()?; // angle_lock_stickfigure_start
    }
    if modern(version, build, 63) {
        r.i8()?; // angle_lock_relative_multiplier
    }
    if modern(version, build, 39) {
        if build <= 40 {
            r.i16()?; // is_drag_locked (stored as i16 in old builds)
        } else {
            r.flag()?; // is_drag_locked
        }
    }
    if modern(version, build, 41) && build <= 45 {
        r.i16()?; // reserved_drag_lock_41_45
    }
    if modern(version, build, 46) {
        r.f32()?; // drag_lock_angle
    }
    if modern(version, build, 41) {
        r.f32()?; // smart_stretch_multiplier
    }
    if modern(version, build, 41) && build <= 45 {
        r.u8()?; // reserved_smart_stretch_41_45
    }
    let child_count = r.count(MAX_NODES)?;
    let mut child_connector_flags = Vec::new();
    if modern(version, build, 38) {
        for _ in 0..child_count {
            child_connector_flags.push(r.i32()?);
        }
    }
    Ok(RawNode {
        parent,
        kind,
        id,
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

fn parse_payload(data: &[u8], le: bool) -> Result<RawFigure, String> {
    let mut r = Reader::new(data, le);
    let version = r.i32()?;
    if version < 1 {
        return Err(format!("invalid figure version {version}"));
    }
    if !le && version < 160 {
        // Legacy payloads are little-endian (they arrive compressed);
        // the caller retries with le = true.
        return Err("legacy version requires little-endian reading".into());
    }
    let build = if version >= 403 { r.i32()? } else { 0 };
    if build < 0 {
        return Err(format!("invalid build {build}"));
    }
    let scale = r.f32()?;
    let color = r.color()?;

    // Preorder tree walk, mirroring the original's explicit stack.
    let mut nodes: Vec<RawNode> = Vec::new();
    let root = read_node(&mut r, version, build, None, false, 0)?;
    nodes.push(root);
    // (node index, next child to read, depth)
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
        let child = read_node(&mut r, version, build, Some(parent_idx), connector, seq)?;
        nodes.push(child);
        stack.push((nodes.len() - 1, 0, depth + 1));
    }
    let mut ids = HashSet::new();
    for n in &nodes {
        if !ids.insert(n.id) {
            return Err(format!("duplicate node id {}", n.id));
        }
    }
    // Polyfills (version >= 230): parsed, not modelled.
    if version >= 230 {
        let count = r.count(MAX_NODES)?;
        for _ in 0..count {
            r.i32()?; // anchorId
            r.color()?;
            r.flag()?; // use_polyfill_color
            let attached = r.count(MAX_NODES)?;
            for _ in 0..attached {
                r.i32()?; // attachedId
            }
        }
    }
    // Connections (modern(38)): parsed, not modelled.
    if modern(version, build, 38) {
        let count = r.count(MAX_NODES)?;
        for _ in 0..count {
            r.i32()?; // sourceId
            r.i32()?; // targetId
        }
    }
    // Trailing bytes are tolerated by the original decoder.
    Ok(RawFigure {
        version,
        build,
        scale,
        color,
        nodes,
    })
}

// ---------- container unwrap (mirrors unwrapContainer) ----------

fn zlib_header_at(b: &[u8], i: usize) -> bool {
    i + 2 <= b.len()
        && (b[i] & 15) == 8
        && (b[i] >> 4) <= 7
        && (((b[i] as u16) << 8) + b[i + 1] as u16) % 31 == 0
        && (b[i + 1] & 32) == 0
}

fn gzip_at(b: &[u8], i: usize) -> bool {
    i + 3 <= b.len() && b[i] == 0x1F && b[i + 1] == 0x8B && b[i + 2] == 8
}

fn inflate(data: &[u8], gzip: bool) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let ok = if gzip {
        flate2::read::GzDecoder::new(data)
            .read_to_end(&mut out)
            .is_ok()
    } else {
        flate2::read::ZlibDecoder::new(data)
            .read_to_end(&mut out)
            .is_ok()
    };
    if ok && out.len() <= MAX_PAYLOAD {
        Some(out)
    } else {
        None
    }
}

fn unwrap(data: &[u8]) -> Vec<u8> {
    // A recognisable raw figure is never mistaken for a stream.
    if data.len() >= 16 {
        let version = i32::from_be_bytes([data[0], data[1], data[2], data[3]]);
        let root_offset = if version >= 403 { 16 } else { 12 };
        if version >= 160
            && !zlib_header_at(data, 0)
            && !gzip_at(data, 0)
            && data.get(root_offset) == Some(&0xFF)
        {
            return data.to_vec();
        }
    }
    let scan = data.len().saturating_sub(2).min(4096);
    for i in 0..=scan {
        if gzip_at(data, i) {
            if let Some(payload) = inflate(&data[i..], true) {
                return payload;
            }
        } else if zlib_header_at(data, i) {
            if let Some(payload) = inflate(&data[i..], false) {
                return payload;
            }
        }
    }
    data.to_vec()
}

// ---------- conversion to the model ----------

fn convert(fig: &RawFigure) -> (FigureDef, u32) {
    let g = fig.scale;
    let n = fig.nodes.len();
    // Effective absolute angles in the Stick Nodes convention,
    // following layoutFigure: parent-first accumulation, the root's
    // local angle is zeroed before build 36, and a negative
    // effective length flips the angle by 180 (the flip accumulates
    // into descendants through the stored pose angle).
    let mut theta = vec![0.0f32; n];
    let mut eff_len = vec![0.0f32; n];
    let mut eff_thick = vec![0.0f32; n];
    for (i, node) in fig.nodes.iter().enumerate() {
        let ns = if node.use_segment_scale { node.scale } else { 1.0 };
        let raw_len = node.length * ns * g;
        eff_len[i] = raw_len.abs();
        eff_thick[i] = (node.thickness as f32).abs() * (ns * g).abs();
        let flip = if raw_len < 0.0 { 180.0 } else { 0.0 };
        if i == 0 {
            let local = if fig.build < 36 { 0.0 } else { node.local_angle };
            theta[i] = local + flip;
        } else {
            theta[i] = theta[node.parent.unwrap()] + node.local_angle + flip;
        }
    }

    let mut nodes: Vec<NodeDef> = Vec::with_capacity(n + 1);
    nodes.push(NodeDef {
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
    for (i, node) in fig.nodes.iter().enumerate() {
        let rest = if i == 0 {
            crate::model::norm_deg(theta[0] + 90.0)
        } else {
            crate::model::norm_deg(theta[i] - theta[node.parent.unwrap()])
        };
        let shape = match node.kind {
            2 | 4 => Shape::Circle,
            3 => Shape::Polygon(3),
            7 => Shape::Polygon(node.num_polygon_vertices.clamp(3, 255) as u8),
            5 => Shape::Circle, // ellipse: approximated by a circle
            _ => Shape::Line,
        };
        nodes.push(NodeDef {
        style: crate::model::NodeStyle::default(),
        media: None,
            name: format!("{} {}", kind_name(node.kind), node.id),
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
    (FigureDef { nodes }, fig.color)
}

fn parse_any(data: &[u8]) -> Result<RawFigure, String> {
    if data.len() > MAX_INPUT {
        return Err("file exceeds the 32 MiB input limit".into());
    }
    let payload = unwrap(data);
    // Big-endian first (modern raw files), then little-endian
    // (legacy compressed payloads) — the original tries both.
    match parse_payload(&payload, false) {
        Ok(fig) => Ok(fig),
        Err(be_err) => parse_payload(&payload, true).map_err(|le_err| {
            format!("not a readable .nodes figure ({be_err}; little-endian: {le_err})")
        }),
    }
}

/// Import a `.nodes` file as a figure definition plus the figure
/// colour (0xRRGGBB) for the instance.
pub fn import_nodes_figure(data: &[u8]) -> Result<(FigureDef, u32), String> {
    Ok(convert(&parse_any(data)?))
}

/// Import a `.nodes` file as a one-figure, one-frame project
/// holding the figure's rest pose at the stage centre.
pub fn import_nodes(data: &[u8]) -> Result<Project, String> {
    let (def, color) = import_nodes_figure(data)?;
    let stage = [960.0, 720.0];
    let pose = def.rest_pose([stage[0] / 2.0, stage[1] / 2.0]);
    Ok(Project {
        name: "Stick Nodes figure".into(),
        fps: 24.0,
        stage,
        background: 0x232830,
        figures: vec![FigureInstance {
            locked: false,
            opacity: 1.0,
            name: "Stick Nodes figure".into(),
            color,
            def,
            hidden: false,
            clip: None,
        }],
        frames: vec![Frame {
            poses: vec![pose],
            duration: 1.0,
            easing: None,
        }],
        camera: Vec::new(),
        tween_playback: false,
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
    })
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

    /// The synthetic corpus fixture shipped in the original repo
    /// (tests/sticknodes-synthetic.json, `modernNodesBase64`):
    /// version 431 / build 117, big-endian — root plus one
    /// rounded-segment child (id 7, local angle 0, length 80,
    /// thickness 4), verified against the original's own parser.
    fn fixture_bytes() -> Vec<u8> {
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

    #[test]
    fn real_corpus_fixture_imports() {
        let project = import_nodes(&fixture_bytes()).unwrap();
        assert_eq!(project.figures.len(), 1);
        assert_eq!(project.figures[0].color, 0x000000);
        let def = &project.figures[0].def;
        assert_eq!(def.nodes.len(), 3); // origin + root + segment
        let seg = &def.nodes[2];
        assert!((seg.length - 80.0).abs() < 0.01);
        assert!((seg.thickness - 4.0).abs() < 0.01);
        assert!((seg.rest_angle - 0.0).abs() < 0.01);
        // Angle 0 in Stick Nodes points right on screen.
        let pos = evaluate(def, &project.frames[0].poses[0]);
        assert!((pos[2][0] - 560.0).abs() < 0.01 && (pos[2][1] - 360.0).abs() < 0.01);
    }

    #[test]
    fn zlib_wrapped_fixture_imports() {
        use std::io::Write as _;
        let mut enc = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
        enc.write_all(&fixture_bytes()).unwrap();
        let wrapped = enc.finish().unwrap();
        let project = import_nodes(&wrapped).unwrap();
        assert_eq!(project.figures[0].def.nodes.len(), 3);
        assert!((project.figures[0].def.nodes[2].length - 80.0).abs() < 0.01);
    }

    /// Byte builder for the hand-packed version-200 profile
    /// (no build field; per node: 6 flags, 5 f32, 2 i32,
    /// local_angle f32, colour, gradient colour, child_count).
    struct B {
        v: Vec<u8>,
        le: bool,
    }
    impl B {
        fn i8(&mut self, x: i8) {
            self.v.push(x as u8);
        }
        fn i32(&mut self, x: i32) {
            self.v.extend_from_slice(&if self.le { x.to_le_bytes() } else { x.to_be_bytes() });
        }
        fn f32(&mut self, x: f32) {
            self.v.extend_from_slice(&if self.le { x.to_le_bytes() } else { x.to_be_bytes() });
        }
        fn flag(&mut self, x: bool) {
            self.v.push(x as u8);
        }
        fn color(&mut self, a: u8, b: u8, g: u8, r: u8) {
            self.v.extend_from_slice(&[a, b, g, r]);
        }
        fn node(&mut self, kind: i8, id: i32, seg_color: bool, length: f32, thick: i32, angle: f32, rgba: [u8; 4], children: i32) {
            self.i8(kind);
            self.i32(id);
            self.flag(false); // is_static
            self.flag(false); // is_stretchy
            self.flag(seg_color); // use_segment_color
            self.flag(false); // use_gradient
            self.flag(false); // reverse_gradient
            self.flag(false); // use_segment_scale
            self.f32(0.0); // local_x
            self.f32(0.0); // local_y
            self.f32(1.0); // scale
            self.f32(length); // default_length
            self.f32(length); // length
            self.i32(thick); // default_thickness
            self.i32(thick); // thickness
            self.f32(angle); // local_angle
            self.color(rgba[0], rgba[1], rgba[2], rgba[3]);
            self.color(0, 0, 0, 0); // gradient_color
            self.i32(children);
        }
    }

    fn hand_packed_v200(le: bool) -> Vec<u8> {
        let mut b = B { v: Vec::new(), le };
        b.i32(200); // version (no build field before 403)
        b.f32(1.0); // figure scale
        b.color(255, 0, 0, 255); // figure colour: red
        // Root, one child (torso up), torso has one child (arm right).
        b.node(-1, 0, false, 0.0, 0, 0.0, [0, 0, 0, 0], 1);
        b.node(1, 1, false, 50.0, 10, 90.0, [0, 0, 0, 0], 1);
        b.node(1, 2, true, 30.0, 6, -90.0, [255, 255, 128, 0], 0);
        b.v
    }

    #[test]
    fn hand_packed_v200_big_endian() {
        let project = import_nodes(&hand_packed_v200(false)).unwrap();
        assert_eq!(project.figures[0].color, 0xFF0000);
        let def = &project.figures[0].def;
        assert_eq!(def.nodes.len(), 4); // origin + root + torso + arm
        assert!((def.nodes[2].length - 50.0).abs() < 0.01);
        assert!((def.nodes[2].rest_angle - 90.0).abs() < 0.01);
        assert!((def.nodes[3].length - 30.0).abs() < 0.01);
        assert!((def.nodes[3].rest_angle + 90.0).abs() < 0.01);
        assert_eq!(def.nodes[3].color, Some(0x0080FF));
        let pos = evaluate(def, &project.frames[0].poses[0]);
        // Torso points up (Stick Nodes 90 degrees), arm points right.
        assert!((pos[2][0] - 480.0).abs() < 0.01 && (pos[2][1] - 310.0).abs() < 0.01);
        assert!((pos[3][0] - 510.0).abs() < 0.01 && (pos[3][1] - 310.0).abs() < 0.01);
    }

    #[test]
    fn hand_packed_v200_little_endian_fallback() {
        let project = import_nodes(&hand_packed_v200(true)).unwrap();
        assert_eq!(project.figures[0].def.nodes.len(), 4);
        assert!((project.figures[0].def.nodes[2].length - 50.0).abs() < 0.01);
    }

    #[test]
    fn garbage_is_rejected() {
        assert!(import_nodes(&[]).is_err());
        assert!(import_nodes(&[1, 2, 3]).is_err());
        assert!(import_nodes(&[0, 0, 0, 200, 1, 2, 3, 4]).is_err());
    }
}
