//! Stick Nodes `.nodes` figure import and export.
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
//! mirroring the original importer's stance. Export writes the
//! 431/117 profile as the direct inverse of the import conversion
//! (see `export_nodes_figure` for exactly what the grammar cannot
//! carry back).

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
    use_gradient: bool,
    gradient_color: u32,
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
    let mut use_gradient = false;
    if version >= 176 {
        use_gradient = r.flag()?;
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
    let mut gradient_color = 0;
    if version >= 176 {
        gradient_color = r.color()?;
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
        use_gradient,
        gradient_color,
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
        min_angle: None,
        max_angle: None,
        style: crate::model::NodeStyle {
            // The gradient end colour is a real grammar field
            // (use_gradient + gradient_color, version >= 176)
            // and round-trips through export. The raw root's is
            // dropped with its colour, by the same design.
            color2: if node.use_gradient && i > 0 {
                Some(node.gradient_color)
            } else {
                None
            },
            ..Default::default()
        },
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
    // IK end-effectors, derived (Wave X, P-4 rider): the grammar
    // carries no ik_end field, so import derives it — every chain
    // leaf (a node no other node parents to) becomes an end
    // effector, restoring the hands/feet a Stick Nodes figure
    // would otherwise lose and have to re-rig by hand. The
    // synthetic origin (index 0) is never an end. Drag dispatch
    // still requires a grandparent before it solves IK
    // (editor.rs), so a depth-1 leaf's flag is inert: it aims
    // exactly as before. Heads and tails are leaves too and the
    // grammar carries nothing that distinguishes them from
    // hands/feet, so they derive ends as well. `bend` stays 1.0:
    // no grammar field and no derivable signal.
    let mut has_child = vec![false; nodes.len()];
    for node in &nodes {
        if let Some(p) = node.parent {
            has_child[p] = true;
        }
    }
    for (i, node) in nodes.iter_mut().enumerate() {
        if i > 0 && !has_child[i] {
            node.ik_end = true;
        }
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

// ---------- export ----------

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
}

/// One node record at version 431 / build 117, in exactly the
/// field order `read_node` consumes at that profile. Every field
/// the model cannot express is written at its neutral default
/// (no curve, no locks, no smart stretch). The gradient end
/// colour is not a default: it is written from the model.
#[allow(clippy::too_many_arguments)]
fn write_node_record(
    w: &mut BeWriter,
    kind: i8,
    id: i32,
    local_angle: f32,
    length: f32,
    thickness: i32,
    color: Option<u32>,
    gradient: Option<u32>,
    polygon_vertices: i16,
    child_count: usize,
) {
    w.i8(kind);
    w.i32(id);
    w.flag(false); // is_static
    w.flag(false); // is_stretchy
    w.flag(false); // is_floaty
    w.flag(false); // is_smart_stretch
    w.flag(false); // do_not_apply_smart_stretch
    w.flag(false); // smart_stretch_reset_impulse
    w.flag(color.is_some()); // use_segment_color
    w.flag(false); // use_circle_outline
    w.flag(false); // circle_is_hollow
    w.flag(gradient.is_some()); // use_gradient
    w.flag(false); // reverse_gradient
    w.i16(0); // gradient_mode
    w.flag(false); // use_segment_scale
    w.f32(0.0); // local_x
    w.f32(0.0); // local_y
    w.f32(1.0); // scale
    w.f32(length); // default_length
    w.f32(length); // length
    w.i32(thickness); // default_thickness
    w.i32(thickness); // thickness
    w.i32(0); // segment_curve_radius
    w.flag(false); // curve_circulization
    w.i16(0); // segment_curve_polyfill_precision
    w.flag(false); // half_arc
    w.i16(0); // right_triangle_direction
    w.flag(false); // triangle_upside_down
    w.f32(0.0); // trapezoid_thickness_start
    w.f32(0.0); // trapezoid_thickness_end
    w.f32(0.5); // trapezoid_top_thickness_ratio
    w.flag(false); // trapezoid_is_rounded_start
    w.flag(false); // trapezoid_is_rounded_end
    w.i16(polygon_vertices);
    w.f32(local_angle); // default_local_angle
    w.f32(local_angle);
    w.f32(0.0); // default_angle (absolute; readers recompute it)
    w.color(color.unwrap_or(0));
    w.color(gradient.unwrap_or(0)); // gradient_color
    w.color(0); // circle_outline_color
    w.flag(false); // is_angle_locked
    w.flag(false); // angle_lock_is_main_node
    w.f32(0.0); // angle_lock_offset
    w.f32(0.0); // angle_lock_relative_start
    w.f32(0.0); // angle_lock_stickfigure_start
    w.i8(0); // angle_lock_relative_multiplier
    w.flag(false); // is_drag_locked
    w.f32(0.0); // drag_lock_angle
    w.f32(1.0); // smart_stretch_multiplier
    w.i32(child_count as i32);
    for _ in 0..child_count {
        w.i32(0); // child connector flag: plain child
    }
}

/// The children map `export_nodes_figure` writes: children of
/// each model node in node order, with a parent of 0 (the
/// origin) — or a stale out-of-range parent — collapsed onto
/// node 1, the raw root.
fn nodes_children(def: &FigureDef) -> Vec<Vec<usize>> {
    let n = def.nodes.len();
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); n];
    for k in 2..n {
        let p = def.nodes[k].parent.unwrap_or(0);
        let rp = if p == 0 || p >= n { 1 } else { p };
        children[rp].push(k);
    }
    children
}

/// Model node indices in the order `export_nodes_figure` writes
/// them (and the order `import_nodes_figure` reads them back):
/// node 1 first, then a preorder walk of the children map. A
/// def whose storage order is not already this preorder reads
/// back with its nodes reordered — same tree, same per-node
/// values, same evaluated geometry. Shared with the `.stknds`
/// writer, whose pose records are stored per node in this
/// order.
pub(crate) fn nodes_preorder(def: &FigureDef) -> Vec<usize> {
    let children = nodes_children(def);
    let mut order = Vec::with_capacity(def.nodes.len().saturating_sub(1));
    fn walk(children: &[Vec<usize>], k: usize, order: &mut Vec<usize>) {
        order.push(k);
        for &c in &children[k] {
            walk(children, c, order);
        }
    }
    if def.nodes.len() >= 2 {
        walk(&children, 1, &mut order);
    }
    order
}

/// Export a figure definition as a `.nodes` file: version 431 /
/// build 117 (the newest layout the original's native loader
/// reads, and the profile this module's importer is verified
/// against), big-endian, raw (uncompressed).
///
/// This is the direct inverse of [`convert`]: model node 1
/// becomes the raw root (kind −1), model node k becomes raw node
/// k−1 with its id, and raw parents are the model parents
/// shifted by one. `name` exists only for symmetry with the
/// other exporters — the `.nodes` grammar stores no figure
/// name. The def must be a tree over its parent links, as every
/// constructor and importer in this project produces.
///
/// What the format cannot carry, and therefore does not survive
/// a round trip (each loss is forced by the grammar or by
/// `convert`'s documented semantics, not by this writer):
///
/// * **Node 1's length.** The raw root is a pure anchor in
///   Stick Nodes (the original's `layoutFigure` always places
///   the root's end at the origin), so `convert` forces it to
///   0 on import. The value is written but reads back as 0.
///   Figures in the importer's own image — node 1 a zero-length
///   anchor, segments hanging off it — round-trip exactly.
/// * **Origin-parented nodes beyond node 1.** A model node
///   whose parent is the origin (e.g. Pivot-style legs) has no
///   distinct raw parent; it is written as a child of the raw
///   root and reads back with parent 1. Its rest angle is
///   written verbatim and survives as a value.
/// * **Node order.** Nodes are stored in tree preorder (see
///   `nodes_preorder`); a def whose storage order differs reads
///   back reordered — same tree, same per-node values, same
///   evaluated geometry, different indices.
/// * **Node 1's per-node colour, and the figure colour.**
///   `convert` drops the raw root's colour by design, and
///   `FigureDef` carries no figure colour (it lives on
///   instances): the header colour is node 1's colour when set,
///   else black, and returns as the instance colour from
///   `import_nodes_figure`. Every other node's colour survives
///   via `use_segment_color`, and its gradient end colour
///   (`NodeStyle.color2`) via `use_gradient` +
///   `gradient_color`; node 1's gradient end colour is
///   dropped with its colour.
/// * **Fractional thickness** (the format stores i32;
///   values are rounded) and **negative lengths** (written as
///   magnitude plus the format's own 180° local-angle flip, so
///   the evaluated geometry is identical but the length reads
///   back positive).
/// * **Node names**: the figure grammar contains no string
///   fields at all (strings exist only in project
///   containers), so names are dropped.
/// * **Segment taper** (`NodeStyle.taper`): the only
///   start/end thickness fields in the record are the
///   trapezoid node's own, and the original renderer
///   applies them only to kind 6; a segment's drawn width
///   comes from the i32 thickness alone. Kind 6 is never
///   written (it would read back as a plain segment and
///   render as a different shape in Stick Nodes), so taper
///   is dropped.
/// * **Segment cap** (`NodeStyle.cap`): the record has no
///   cap field, so the cap kind is dropped.
/// * **Segment opacity** (`NodeStyle.opacity`): the record
///   has no opacity field; the colour alpha byte is dropped
///   on read by design and defines no opacity, so opacity
///   is dropped.
/// * **Media** (sprites/text on segments): `.nodes` node
///   kinds are purely geometric — there are no image/text
///   kinds and no side tables — so media is dropped.
/// * The head flag and bend direction have no fields in
///   the grammar and are dropped. ik_end has no field
///   either and is not stored — but import derives it for
///   chain leaves (see `convert`), so the round trip loses
///   it only on a non-leaf node whose flag was set by hand.
///   Ellipses read back as
///   circles (there is no ellipse in the model; kind 5
///   writes as kind 2), and a 4-sided polygon writes as
///   kind 7 with 4 vertices.
pub fn export_nodes_figure(def: &FigureDef, _name: &str) -> Vec<u8> {
    let mut w = BeWriter { v: Vec::new() };
    w.i32(431); // version
    w.i32(117); // build
    w.f32(1.0); // figure scale
    let header_color = def.nodes.get(1).and_then(|n| n.color).unwrap_or(0);
    w.color(header_color);
    let n = def.nodes.len();
    if n < 2 {
        // Degenerate def (origin only): a lone raw root still
        // parses as a figure.
        write_node_record(&mut w, -1, 0, 0.0, 0.0, 0, None, None, 5, 0);
        w.i32(0); // polyfills
        w.i32(0); // connections
        return w.v;
    }
    let children = nodes_children(def);
    fn emit(w: &mut BeWriter, def: &FigureDef, children: &[Vec<usize>], k: usize) {
        let node = &def.nodes[k];
        let kind: i8 = if k == 1 {
            -1 // raw root
        } else {
            match node.shape {
                Shape::Circle => 2,
                Shape::Polygon(3) => 3,
                Shape::Polygon(_) => 7,
                Shape::Line => 1,
            }
        };
        let polygon_vertices = match node.shape {
            Shape::Polygon(s) => s as i16,
            _ => 5, // the reader's own default
        };
        // convert() accumulates raw local angles parent-first and
        // offsets only the root by 90 degrees, so node 1's local
        // angle is its rest angle minus 90 and every other node's
        // is its rest angle. A negative length folds into the
        // format's 180-degree flip convention.
        let mut local = if k == 1 {
            node.rest_angle - 90.0
        } else {
            node.rest_angle
        };
        let mut length = node.length;
        if length < 0.0 {
            length = -length;
            local += 180.0;
        }
        // The raw root's colour never surfaces in the model
        // (convert drops it), so it is written without the
        // per-segment colour flag. The same holds for its
        // gradient end colour.
        let color = if k == 1 { None } else { node.color };
        let gradient = if k == 1 { None } else { node.style.color2 };
        write_node_record(
            w,
            kind,
            (k - 1) as i32,
            local,
            length,
            node.thickness.round() as i32,
            color,
            gradient,
            polygon_vertices,
            children[k].len(),
        );
        for &c in &children[k] {
            emit(w, def, children, c);
        }
    }
    emit(&mut w, def, &children, 1);
    w.i32(0); // polyfills: none
    w.i32(0); // connections: none
    w.v
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
            states: Vec::new(),
        }],
        frames: vec![Frame {
            poses: vec![pose],
            duration: 1.0,
            easing: None,
            bends: Vec::new(),
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
        head_ring_scale: 1.0,
        subtitles: Vec::new(),
        subtitle_style: Default::default(),
        format_version: crate::model::FORMAT_VERSION_CURRENT,
        chapters: Vec::new(),
        stored_guides: Vec::new(),
        title_cards: Default::default(),
        beat_markers: Vec::new(),
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

    // ---- Wave 7: .nodes export ----

    fn nd(
        parent: Option<usize>,
        length: f32,
        rest: f32,
        thickness: f32,
        color: Option<u32>,
        shape: Shape,
    ) -> NodeDef {
        NodeDef {
            min_angle: None,
            max_angle: None,
            name: "n".into(),
            parent,
            length,
            rest_angle: rest,
            head: false,
            bend: 1.0,
            thickness,
            color,
            ik_end: false,
            shape,
            style: crate::model::NodeStyle::default(),
            media: None,
        }
    }

    /// A branched, coloured, multi-shape def in the importer's
    /// own image: node 1 is a zero-length anchor (rest 100, so
    /// the root's −90 local-angle offset is exercised), segments
    /// hang off it and off each other.
    fn branched_def() -> FigureDef {
        FigureDef {
            nodes: vec![
                nd(None, 0.0, 0.0, 5.0, None, Shape::Line),
                nd(Some(0), 0.0, 100.0, 6.0, Some(0x112233), Shape::Line),
                nd(Some(1), 80.0, 0.0, 4.0, Some(0xFF0000), Shape::Line),
                nd(Some(1), 50.0, -90.0, 3.0, None, Shape::Circle),
                nd(Some(2), 30.0, 45.0, 2.0, Some(0x00FF00), Shape::Polygon(3)),
                nd(Some(2), 25.0, -30.0, 2.0, None, Shape::Polygon(5)),
                nd(Some(1), 40.0, 170.0, 7.0, Some(0x0000FF), Shape::Polygon(4)),
            ],
        }
    }

    #[test]
    fn nodes_export_roundtrips_branched_def() {
        let def = branched_def();
        let bytes = export_nodes_figure(&def, "Branch");
        // Raw 431/117: version + build up front, root kind −1 at
        // offset 16 (the raw-detection byte).
        assert_eq!(&bytes[0..4], &431i32.to_be_bytes());
        assert_eq!(&bytes[4..8], &117i32.to_be_bytes());
        assert_eq!(bytes[16], 0xFF);
        // Kept on disk for cross-verification with the original
        // app's JS parser (parseNodes).
        std::fs::write("/tmp/styk-export.nodes", &bytes).unwrap();
        let (back, color) = import_nodes_figure(&bytes).unwrap();
        assert_eq!(color, 0x112233, "header colour from node 1");
        assert_eq!(back.nodes.len(), def.nodes.len());
        // The stream is tree preorder, which for this def is NOT
        // storage order ([1, 2, 4, 5, 3, 6]): compare per
        // corresponding node, not per index.
        let order = nodes_preorder(&def);
        assert_eq!(order, vec![1, 2, 4, 5, 3, 6]);
        let pa = evaluate(&def, &def.rest_pose([100.0, 100.0]));
        let pb = evaluate(&back, &back.rest_pose([100.0, 100.0]));
        for (j, &k) in order.iter().enumerate() {
            let (a, b) = (&def.nodes[k], &back.nodes[j + 1]);
            assert!((a.thickness - b.thickness).abs() < 0.01, "node {k} thickness");
            assert!(
                (crate::model::norm_deg(a.rest_angle - b.rest_angle)).abs() < 0.05,
                "node {k} rest {} vs {}",
                a.rest_angle,
                b.rest_angle
            );
            assert_eq!(a.shape, b.shape, "node {k} shape");
            if k == 1 {
                // Documented losses on the raw root only.
                assert_eq!(b.length, 0.0);
                assert_eq!(b.color, None);
                assert_eq!(b.parent, Some(0));
            } else {
                assert!((a.length - b.length).abs() < 0.01, "node {k} length");
                assert_eq!(a.color, b.color, "node {k} colour");
                assert_eq!(
                    a.style.color2, b.style.color2,
                    "node {k} gradient end colour"
                );
                // The new parent is the raw parent's new index.
                let rp = if a.parent == Some(0) { 1 } else { a.parent.unwrap() };
                let expect = order.iter().position(|&x| x == rp).unwrap() + 1;
                assert_eq!(b.parent, Some(expect), "node {k} parent");
            }
            // Evaluated rest geometry is identical per node.
            assert!(
                (pa[k][0] - pb[j + 1][0]).abs() < 0.05 && (pa[k][1] - pb[j + 1][1]).abs() < 0.05,
                "node {k} pos {:?} vs {:?}",
                pa[k],
                pb[j + 1]
            );
        }
    }

    #[test]
    fn nodes_export_of_imported_fixture_roundtrips() {
        let (def, _) = import_nodes_figure(&fixture_bytes()).unwrap();
        let bytes = export_nodes_figure(&def, "Fixture");
        let (back, _) = import_nodes_figure(&bytes).unwrap();
        assert_eq!(back.nodes.len(), def.nodes.len());
        assert!((back.nodes[2].length - 80.0).abs() < 0.01);
        assert!((back.nodes[2].thickness - 4.0).abs() < 0.01);
        assert!((back.nodes[2].rest_angle - 0.0).abs() < 0.05);
    }

    #[test]
    fn nodes_export_humanoid_documents_its_losses() {
        // A Pivot-style def: node 1 is a real 46-long segment and
        // the thighs parent the origin. The .nodes tree cannot
        // express either; the writer's documented behaviour is
        // asserted here so it cannot drift silently.
        let def = FigureDef::humanoid();
        let bytes = export_nodes_figure(&def, "Humanoid");
        let (back, _) = import_nodes_figure(&bytes).unwrap();
        assert_eq!(back.nodes.len(), def.nodes.len(), "node count survives");
        assert_eq!(back.nodes[1].length, 0.0, "raw root length is not representable");
        assert_eq!(back.nodes[7].parent, Some(1), "origin-parented thigh re-parents to the root");
        assert_eq!(back.nodes[9].parent, Some(1));
        for k in 2..def.nodes.len() {
            assert!(
                (def.nodes[k].length - back.nodes[k].length).abs() < 0.01,
                "node {k} length"
            );
            assert!(
                (crate::model::norm_deg(def.nodes[k].rest_angle - back.nodes[k].rest_angle)).abs() < 0.1,
                "node {k} rest"
            );
        }
    }

    // ---- Wave 9: .nodes round-trip properties ----

    /// Field-by-field comparison of a def against its
    /// export → import round trip, node by corresponding node
    /// via `nodes_preorder` (the stream is tree preorder, so
    /// indices generally move). Tolerances follow the spirit of
    /// pivot.rs's round-trips (< 0.5 there): 0.51 here, because
    /// thickness is stored as i32 and rounding alone can cost
    /// 0.5. The only exemptions are the losses documented on
    /// `export_nodes_figure`, and each is asserted explicitly
    /// rather than skipped: the raw root's length reads back 0
    /// and its colour None (the colour returns as the header /
    /// instance colour instead), origin-parented nodes re-parent
    /// to the root, and the head flag and bend direction are
    /// dropped (no grammar fields). ik_end has no grammar field
    /// either, but import derives it for chain leaves (Wave X,
    /// P-4 rider), so it is asserted against that derived rule
    /// on the re-imported figure rather than against the source
    /// def. The gradient end colour
    /// is not a loss: it is compared for every non-root node
    /// (the root's is dropped with its colour), and taper, cap
    /// and opacity are asserted back at their defaults (no
    /// grammar fields for them). `check_geometry` also
    /// compares evaluated rest positions — only valid for defs
    /// in the importer's own image (zero-length root anchor):
    /// for Pivot-style defs the root's lost length shifts every
    /// descendant's evaluated position by design.
    fn check_roundtrip_fields(def: &FigureDef, back: &FigureDef, check_geometry: bool) {
        assert_eq!(back.nodes.len(), def.nodes.len(), "node count");
        // The importer's derived ik_end rule, recomputed on the
        // re-imported figure itself: a node is an end iff no
        // other node parents to it (chain leaf), origin aside.
        let mut back_has_child = vec![false; back.nodes.len()];
        for nd in &back.nodes {
            if let Some(p) = nd.parent {
                back_has_child[p] = true;
            }
        }
        let order = nodes_preorder(def);
        let pa = evaluate(def, &def.rest_pose([100.0, 100.0]));
        let pb = evaluate(back, &back.rest_pose([100.0, 100.0]));
        for (j, &k) in order.iter().enumerate() {
            let (a, b) = (&def.nodes[k], &back.nodes[j + 1]);
            assert!(
                (a.thickness - b.thickness).abs() < 0.51,
                "node {k} thickness {} vs {}",
                a.thickness,
                b.thickness
            );
            assert!(
                (crate::model::norm_deg(a.rest_angle - b.rest_angle)).abs() < 0.51,
                "node {k} rest {} vs {}",
                a.rest_angle,
                b.rest_angle
            );
            assert_eq!(a.shape, b.shape, "node {k} shape");
            assert!(!b.head, "node {k}: head flag has no grammar field");
            assert_eq!(
                b.ik_end,
                !back_has_child[j + 1],
                "node {k}: ik_end is derived on import (chain leaves), not stored"
            );
            assert_eq!(b.bend, 1.0, "node {k}: bend direction has no grammar field");
            assert_eq!(b.style.taper, 1.0, "node {k}: taper has no grammar field");
            assert_eq!(
                b.style.cap,
                crate::model::CapKind::Round,
                "node {k}: cap has no grammar field"
            );
            assert_eq!(b.style.opacity, 1.0, "node {k}: opacity has no grammar field");
            if k == 1 {
                // Documented raw-root losses.
                assert_eq!(b.length, 0.0, "raw root length reads back 0");
                assert_eq!(b.color, None, "raw root colour is dropped");
                assert_eq!(b.style.color2, None, "raw root gradient is dropped");
                assert_eq!(b.parent, Some(0));
            } else {
                assert!(
                    (a.length - b.length).abs() < 0.51,
                    "node {k} length {} vs {}",
                    a.length,
                    b.length
                );
                assert_eq!(a.color, b.color, "node {k} colour");
                assert_eq!(
                    a.style.color2, b.style.color2,
                    "node {k} gradient end colour"
                );
                // Origin-parented nodes re-parent to the root;
                // any other parent maps to its preorder slot.
                let rp = match a.parent {
                    Some(p) if p >= 1 => p,
                    _ => 1,
                };
                let expect = order.iter().position(|&x| x == rp).unwrap() + 1;
                assert_eq!(b.parent, Some(expect), "node {k} parent");
            }
            if check_geometry {
                assert!(
                    (pa[k][0] - pb[j + 1][0]).abs() < 0.51
                        && (pa[k][1] - pb[j + 1][1]).abs() < 0.51,
                    "node {k} pos {:?} vs {:?}",
                    pa[k],
                    pb[j + 1]
                );
            }
        }
    }

    /// Round trip for defs in the importer's own image
    /// (zero-length root anchor): geometry included.
    fn assert_nodes_roundtrip(def: &FigureDef) -> FigureDef {
        let bytes = export_nodes_figure(def, "T");
        let (back, _) = import_nodes_figure(&bytes).unwrap();
        check_roundtrip_fields(def, &back, true);
        back
    }

    /// Round trip for Pivot-style defs: every field except
    /// evaluated geometry (shifted by the root's lost length).
    fn assert_nodes_roundtrip_structure(def: &FigureDef) -> FigureDef {
        let bytes = export_nodes_figure(def, "T");
        let (back, _) = import_nodes_figure(&bytes).unwrap();
        check_roundtrip_fields(def, &back, false);
        back
    }

    #[test]
    fn nodes_roundtrip_wide_star_and_deep_chain() {
        // Importer-image def (node 1 a zero-length anchor, rest
        // 100): five leaves fanned off the root — one a circle,
        // one a hexagon — plus a four-deep chain. Storage order
        // below is already tree preorder.
        let def = FigureDef {
            nodes: vec![
                nd(None, 0.0, 0.0, 5.0, None, Shape::Line),
                nd(Some(0), 0.0, 100.0, 6.0, None, Shape::Line),
                nd(Some(1), 34.0, -12.0, 3.0, None, Shape::Line), // chain a
                nd(Some(2), 28.0, 24.0, 3.0, Some(0x224466), Shape::Line), // chain b
                nd(Some(3), 22.0, -36.0, 2.0, None, Shape::Line), // chain c
                nd(Some(4), 16.0, 48.0, 2.0, None, Shape::Circle), // chain d
                nd(Some(1), 44.0, -150.0, 8.0, Some(0xFF0000), Shape::Line),
                nd(Some(1), 40.0, -75.0, 12.0, None, Shape::Circle),
                nd(Some(1), 52.0, 0.0, 4.0, Some(0x00FF00), Shape::Polygon(6)),
                nd(Some(1), 38.0, 75.0, 6.0, None, Shape::Line),
                nd(Some(1), 46.0, 160.0, 9.0, Some(0x0000FF), Shape::Line),
            ],
        };
        assert_nodes_roundtrip(&def);
    }

    #[test]
    fn nodes_roundtrip_quadruped_preset_structure() {
        // The quadruped preset is Pivot-style: node 1 is a real
        // 95-long spine and four nodes parent the origin, so the
        // documented losses apply — but count, lengths,
        // thicknesses, rest angles, shapes and the re-parenting
        // must all survive. (Its storage order is preorder under
        // the collapsed parent map, so indices line up.)
        let def = FigureDef::quadruped();
        let back = assert_nodes_roundtrip_structure(&def);
        assert_eq!(back.nodes[1].length, 0.0, "the spine's 95 does not survive");
        for k in [4, 6, 12] {
            assert_eq!(
                back.nodes[k].parent,
                Some(1),
                "origin-parented node {k} re-parents to the root"
            );
        }
        assert!(!back.nodes[3].head, "the head flag is dropped");
        // ik_end is not stored, but import derives it for chain
        // leaves (Wave X, P-4 rider — this pin asserted the drop
        // until then): the four feet are leaves, so they come
        // back as end effectors.
        for k in [5, 7, 9, 11] {
            assert!(back.nodes[k].ik_end, "ik_end on node {k} is derived (chain leaf)");
        }
        // The head and the tail are chain leaves too, and the
        // grammar carries nothing that distinguishes them from
        // feet, so they derive ends as well — on the record.
        for k in [3, 12] {
            assert!(back.nodes[k].ik_end, "ik_end on leaf node {k} is derived");
        }
    }

    #[test]
    fn import_derives_ik_ends_for_chain_leaves() {
        // Wave X, P-4 rider: the derivation, asserted as an
        // exact set on a branched figure. Storage order is
        // preorder under the collapsed parent map ([1,2,4,5,3,6]
        // for this def), so in the re-imported figure the
        // internal nodes are 1 (anchor) and 2 (the fork), the
        // leaves are 3, 4, 5, 6 — including depth-1 leaves
        // parented straight to the anchor, whose flag is inert
        // (drag dispatch still requires a grandparent).
        let bytes = export_nodes_figure(&branched_def(), "Branch");
        let (back, _color) = import_nodes_figure(&bytes).unwrap();
        let ends: Vec<bool> = back.nodes.iter().map(|n| n.ik_end).collect();
        assert_eq!(ends, vec![false, false, false, true, true, true, true]);
    }

    #[test]
    fn nodes_roundtrip_simple_preset_with_colour_override_and_circle() {
        // Preset + the brief's edge cases on top: per-node
        // colour overrides, a circle-primitive head, and a
        // non-default thickness on one leg.
        let mut def = FigureDef::simple();
        def.nodes[1].color = Some(0x0A0B0C); // root colour → header
        def.nodes[2].shape = Shape::Circle; // head as a circle
        def.nodes[3].color = Some(0xFF8800);
        def.nodes[4].color = Some(0x123456);
        def.nodes[5].thickness = 9.0;
        let bytes = export_nodes_figure(&def, "T");
        let (back, color) = import_nodes_figure(&bytes).unwrap();
        // Node 1's colour is not a per-node field on the way
        // back: it returns as the figure (instance) colour.
        assert_eq!(color, 0x0A0B0C, "root colour returns via the header");
        assert_eq!(back.nodes[1].color, None);
        check_roundtrip_fields(&def, &back, false);
        assert_eq!(back.nodes[2].shape, Shape::Circle);
        assert_eq!(back.nodes[3].color, Some(0xFF8800));
        assert_eq!(back.nodes[4].color, Some(0x123456));
        assert_eq!(back.nodes[5].thickness, 9.0);
        // The origin-parented legs re-parent to the root.
        assert_eq!(back.nodes[5].parent, Some(1));
        assert_eq!(back.nodes[6].parent, Some(1));
    }

    #[test]
    fn nodes_roundtrip_fractional_thickness_rounds_to_i32_documented() {
        // Documented loss: the format stores thickness as i32,
        // so fractional values round (half away from zero).
        let def = FigureDef {
            nodes: vec![
                nd(None, 0.0, 0.0, 5.0, None, Shape::Line),
                nd(Some(0), 0.0, 100.0, 6.0, None, Shape::Line),
                nd(Some(1), 40.0, 10.0, 4.4, None, Shape::Line),
                nd(Some(1), 40.0, -10.0, 4.6, None, Shape::Line),
                nd(Some(1), 40.0, 90.0, 7.5, None, Shape::Line),
            ],
        };
        let bytes = export_nodes_figure(&def, "T");
        let (back, _) = import_nodes_figure(&bytes).unwrap();
        assert_eq!(back.nodes[2].thickness, 4.0, "4.4 rounds down");
        assert_eq!(back.nodes[3].thickness, 5.0, "4.6 rounds up");
        assert_eq!(back.nodes[4].thickness, 8.0, "7.5 rounds up");
        // Lengths and angles are f32 in the grammar: exact.
        for k in 2..=4 {
            assert!((back.nodes[k].length - 40.0).abs() < 0.01, "node {k} length");
        }
    }

    #[test]
    fn nodes_roundtrip_negative_length_flips_180_documented() {
        // Documented loss: a negative length is written as its
        // magnitude plus the format's 180° local-angle flip, so
        // the value changes but the evaluated geometry is
        // identical.
        let def = FigureDef {
            nodes: vec![
                nd(None, 0.0, 0.0, 5.0, None, Shape::Line),
                nd(Some(0), 0.0, 100.0, 6.0, None, Shape::Line),
                nd(Some(1), 40.0, 20.0, 4.0, None, Shape::Line),
                nd(Some(2), -30.0, 10.0, 4.0, None, Shape::Line),
            ],
        };
        let bytes = export_nodes_figure(&def, "T");
        let (back, _) = import_nodes_figure(&bytes).unwrap();
        assert_eq!(back.nodes.len(), def.nodes.len());
        assert!((back.nodes[3].length - 30.0).abs() < 0.01, "magnitude survives");
        assert!(
            (crate::model::norm_deg(back.nodes[3].rest_angle - (def.nodes[3].rest_angle + 180.0)))
                .abs()
                < 0.05,
            "rest angle gains the 180 flip: {} vs {}",
            back.nodes[3].rest_angle,
            def.nodes[3].rest_angle
        );
        let pa = evaluate(&def, &def.rest_pose([100.0, 100.0]));
        let pb = evaluate(&back, &back.rest_pose([100.0, 100.0]));
        for k in 0..def.nodes.len() {
            assert!(
                (pa[k][0] - pb[k][0]).abs() < 0.05 && (pa[k][1] - pb[k][1]).abs() < 0.05,
                "node {k} pos {:?} vs {:?}",
                pa[k],
                pb[k]
            );
        }
    }

    #[test]
    fn nodes_roundtrip_names_are_regenerated_documented() {
        // Documented loss: the grammar stores no names at all —
        // neither node names nor a figure name. Imported nodes
        // are named from their kind and raw id, and the export's
        // `name` argument is ignored entirely (byte-identical
        // output for any name).
        let mut def = FigureDef {
            nodes: vec![
                nd(None, 0.0, 0.0, 5.0, None, Shape::Line),
                nd(Some(0), 0.0, 100.0, 6.0, None, Shape::Line),
                nd(Some(1), 40.0, 15.0, 4.0, None, Shape::Line),
                nd(Some(2), 24.0, -20.0, 4.0, None, Shape::Circle),
                nd(Some(2), 18.0, 30.0, 4.0, None, Shape::Polygon(3)),
            ],
        };
        def.nodes[1].name = "Alpha".into();
        def.nodes[2].name = "Beta".into();
        def.nodes[3].name = "Gamma".into();
        def.nodes[4].name = "Delta".into();
        let bytes = export_nodes_figure(&def, "T");
        assert_eq!(
            bytes,
            export_nodes_figure(&def, "A Completely Different Name"),
            "the figure name is not stored in the grammar"
        );
        let (back, _) = import_nodes_figure(&bytes).unwrap();
        assert_eq!(back.nodes[0].name, "origin");
        assert_eq!(back.nodes[1].name, "root 0");
        assert_eq!(back.nodes[2].name, "segment 1");
        assert_eq!(back.nodes[3].name, "circle 2");
        assert_eq!(back.nodes[4].name, "triangle 3");
    }

    #[test]
    fn nodes_roundtrip_gradient_end_colour_survives() {
        // The grammar carries a per-node gradient (use_gradient
        // flag + gradient_color, version >= 176) and the model
        // holds it as NodeStyle.color2, so it must round-trip:
        // over a node's own colour, over the figure colour
        // (colour None), and on a shape kind. The raw root's
        // gradient is the one exception — dropped with the
        // root's colour, by convert's design.
        let mut def = FigureDef {
            nodes: vec![
                nd(None, 0.0, 0.0, 5.0, None, Shape::Line),
                nd(Some(0), 0.0, 100.0, 6.0, None, Shape::Line),
                nd(Some(1), 40.0, 10.0, 4.0, Some(0x102030), Shape::Line),
                nd(Some(1), 36.0, -25.0, 4.0, None, Shape::Line),
                nd(Some(1), 30.0, 80.0, 4.0, None, Shape::Circle),
                nd(Some(1), 26.0, 140.0, 4.0, Some(0xA0B0C0), Shape::Line),
            ],
        };
        def.nodes[1].style.color2 = Some(0xFF0000); // root: dropped
        def.nodes[2].style.color2 = Some(0x00FF00); // over own colour
        def.nodes[3].style.color2 = Some(0x0000FF); // over figure colour
        def.nodes[4].style.color2 = Some(0x123123); // on a circle
        // nodes[5] keeps color2 None: no gradient is written.
        let bytes = export_nodes_figure(&def, "T");
        let (back, _) = import_nodes_figure(&bytes).unwrap();
        assert_eq!(back.nodes[1].style.color2, None, "root gradient dropped");
        assert_eq!(back.nodes[2].style.color2, Some(0x00FF00));
        assert_eq!(back.nodes[3].style.color2, Some(0x0000FF));
        assert_eq!(back.nodes[4].style.color2, Some(0x123123));
        assert_eq!(back.nodes[5].style.color2, None);
        // Base colours are unaffected by the gradient channel.
        assert_eq!(back.nodes[2].color, Some(0x102030));
        assert_eq!(back.nodes[5].color, Some(0xA0B0C0));
        check_roundtrip_fields(&def, &back, true);
    }

    #[test]
    fn nodes_roundtrip_taper_cap_opacity_dropped_documented() {
        // Documented losses: taper, cap and opacity have no
        // fields in the .nodes record (see export_nodes_figure),
        // so they read back at their NodeStyle defaults even
        // when set — while the gradient end colour on the same
        // node survives, proving the style was written.
        let mut def = FigureDef {
            nodes: vec![
                nd(None, 0.0, 0.0, 5.0, None, Shape::Line),
                nd(Some(0), 0.0, 100.0, 6.0, None, Shape::Line),
                nd(Some(1), 40.0, 10.0, 4.0, None, Shape::Line),
            ],
        };
        def.nodes[2].style = crate::model::NodeStyle {
            taper: 0.25,
            cap: crate::model::CapKind::Point,
            color2: Some(0x445566),
            opacity: 0.4,
        };
        let bytes = export_nodes_figure(&def, "T");
        let (back, _) = import_nodes_figure(&bytes).unwrap();
        assert_eq!(back.nodes[2].style.color2, Some(0x445566), "gradient survives");
        assert_eq!(back.nodes[2].style.taper, 1.0, "taper has no grammar field");
        assert_eq!(
            back.nodes[2].style.cap,
            crate::model::CapKind::Round,
            "cap has no grammar field"
        );
        assert_eq!(back.nodes[2].style.opacity, 1.0, "opacity has no grammar field");
    }
}
