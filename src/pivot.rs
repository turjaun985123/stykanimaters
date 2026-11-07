//! Pivot formats: `.piv` versions 2, 3 and 4 (import and export),
//! the version 5 native container (import), and `.stk` figure
//! files (import and export).
//!
//! The byte grammar is ported from the original app's own codec
//! (`codecs/piv_legacy_v2.py`, `codecs/piv_legacy_v4.py`,
//! `codecs/codec_bridge.py` in StykAnimate-App), which documents the
//! native layout record by record. Nothing here is guessed:
//!
//! * A `.piv` file is raw when its first byte is a version (1..=5),
//!   otherwise it is a zlib stream over the payload.
//! * A `.stk` file is a figure stream, optionally wrapped as
//!   `0x7a` + zlib stream.
//! * A figure stream is `count: u8` then per segment a 24-byte
//!   record (parent id u8, segment id u8, ignored u16, length f32,
//!   absolute angle f64 radians, thickness f32, primitive u8,
//!   static u8, padding u16), then a length-prefixed name (u8 +
//!   cp1252 bytes). Image segments carry extra index bytes and a
//!   bitmap table; those bytes are skipped (sprites are not
//!   imported as geometry).
//! * Pivot angles are absolute, in radians, with direction
//!   (cos a, sin a) in y-down screen space. This model stores
//!   relative angles in degrees where dir = (sin d, cos d), so
//!   `deg = 90 - rad * 180/pi`, relative to the parent segment.
//! * Object colours are COLORREF (0x00BBGGRR).
//!
//! The v5 grammar is ported from `codecs/piv_codec.py`; v5 import
//! covers figures built from line/circle/square segments. Figures
//! with image/text segments are rejected until the side-table
//! grammar is ported. Stick Nodes `.nodes` is a different, much
//! larger format and is not handled here.

use crate::model::{CameraKey, FigureDef, FigureInstance, Frame, NodeDef, Pose, Project, Shape};
use std::io::Read as _;
use std::io::Write as _;

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Reader { data, pos: 0 }
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
    fn u16(&mut self) -> Result<u16, String> {
        let b = self.take(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }
    fn u32(&mut self) -> Result<u32, String> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn u64(&mut self) -> Result<u64, String> {
        let b = self.take(8)?;
        Ok(u64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]))
    }
    fn f32(&mut self) -> Result<f32, String> {
        Ok(f32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn f64(&mut self) -> Result<f64, String> {
        Ok(f64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn str8(&mut self) -> Result<String, String> {
        let n = self.u8()? as usize;
        let bytes = self.take(n)?;
        Ok(bytes.iter().map(|&b| b as char).collect())
    }
    fn done(&self) -> bool {
        self.pos == self.data.len()
    }
}

/// One parsed Pivot segment record.
struct Seg {
    parent_id: u8,
    id: u8,
    length: f32,
    angle_rad: f64,
    thickness: f32,
    primitive: u8,
    is_static: bool,
}

fn rad_to_deg(rad: f64) -> f32 {
    90.0 - (rad * 180.0 / std::f64::consts::PI) as f32
}

fn deg_to_rad(deg: f32) -> f64 {
    (90.0 - deg as f64) * std::f64::consts::PI / 180.0
}

fn colorref_to_rgb(raw: u32) -> u32 {
    let raw = if raw >= 0x8000_0000 { 0 } else { raw };
    let (r, g, b) = (raw & 0xFF, (raw >> 8) & 0xFF, (raw >> 16) & 0xFF);
    (r << 16) | (g << 8) | b
}

fn rgb_to_colorref(rgb: u32) -> u32 {
    let (r, g, b) = ((rgb >> 16) & 0xFF, (rgb >> 8) & 0xFF, rgb & 0xFF);
    r | (g << 8) | (b << 16)
}

/// Skip a BMP asset: 'BM', u32 total size, then size-6 more bytes.
fn skip_bmp(r: &mut Reader) -> Result<(), String> {
    let magic = r.take(2)?;
    if magic != b"BM" {
        return Err("expected BMP signature".into());
    }
    let size = r.u32()? as usize;
    if size < 26 {
        return Err("invalid BMP size".into());
    }
    r.take(size - 6)?;
    Ok(())
}

/// Skip a JPEG asset: u64 absolute end offset, then the bytes.
fn skip_jpeg(r: &mut Reader) -> Result<(), String> {
    let start = r.pos;
    let end = r.u64()? as usize;
    if end <= start + 8 || end > r.data.len() {
        return Err("invalid JPEG end pointer".into());
    }
    r.take(end - (start + 8))?;
    Ok(())
}

/// Skip a v2/v3 compact bitmap (figure sprite table entry).
fn skip_compact_bitmap(r: &mut Reader) -> Result<(), String> {
    let mode = r.u8()?;
    let w = r.u16()? as usize;
    let h = r.u16()? as usize;
    r.u32()?; // transparency key
    let indexed = mode == 3;
    r.take(w * h * if indexed { 1 } else { 3 })?;
    if indexed {
        r.take(1024)?;
    }
    Ok(())
}

/// Parse one figure stream. `version4` selects the v4 sprite grammar
/// (2 extra bytes per image segment, per-sprite colour + BMP table).
/// Returns (definition, name, per-segment absolute rest degrees).
fn parse_figure(r: &mut Reader, version4: bool) -> Result<(FigureDef, String, Vec<f32>, bool), String> {
    let count = r.u8()? as usize;
    if count == 0 {
        return Err("empty figure".into());
    }
    let mut segs: Vec<Seg> = Vec::new();
    let mut has_images = false;
    for _ in 0..count {
        let parent_id = r.u8()?;
        let id = r.u8()?;
        r.u16()?; // ignored word
        let length = r.f32()?;
        let angle_rad = r.f64()?;
        let thickness = r.f32()?;
        let primitive = r.u8()?;
        let static_ = r.u8()?;
        r.u16()?; // padding
        if !matches!(primitive, 0 | 1 | 2 | 3 | 4 | 6) {
            return Err(format!("unsupported segment primitive {primitive}"));
        }
        if primitive == 2 {
            has_images = true;
            r.u8()?; // image index
            if version4 {
                r.u8()?; // image flip
            }
        }
        segs.push(Seg {
            parent_id,
            id,
            length,
            angle_rad,
            thickness,
            primitive,
            is_static: static_ != 0,
        });
    }
    if has_images {
        let image_count = r.u8()?;
        for _ in 0..image_count {
            if version4 {
                r.u32()?; // transparent colour
                skip_bmp(r)?;
            } else {
                skip_compact_bitmap(r)?;
            }
        }
    }
    let name = r.str8().unwrap_or_default();

    // Segments stay in record order: Pivot writers emit parents
    // before children (the format's own validators require a valid
    // topology), object angle arrays are indexed by record order,
    // and this model's FK evaluation needs parents first too.
    let order: Vec<usize> = (0..segs.len()).collect();
    {
        let mut seen: Vec<u8> = vec![0];
        for seg in &segs {
            if !seen.contains(&seg.parent_id) {
                return Err("figure segment parent follows its child (unsupported order)".into());
            }
            seen.push(seg.id);
        }
    }
    // Node 0 is the figure origin; node k+1 is order[k]'s segment.
    let mut index_of_id: std::collections::HashMap<u8, usize> = Default::default();
    index_of_id.insert(0, 0);
    for (k, &si) in order.iter().enumerate() {
        index_of_id.insert(segs[si].id, k + 1);
    }
    let abs_of_id = |id: u8, segs: &[Seg]| -> f32 {
        if id == 0 {
            0.0
        } else {
            segs.iter()
                .find(|s| s.id == id)
                .map(|s| rad_to_deg(s.angle_rad))
                .unwrap_or(0.0)
        }
    };
    let mut nodes = vec![NodeDef {
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
    }];
    let mut rest_abs = vec![0.0f32];
    for &si in &order {
        let seg = &segs[si];
        let abs = rad_to_deg(seg.angle_rad);
        let parent_abs = abs_of_id(seg.parent_id, &segs);
        let shape = match seg.primitive {
            1 | 3 | 4 => Shape::Circle,
            6 => Shape::Polygon(4),
            _ => Shape::Line,
        };
        nodes.push(NodeDef {
            name: format!("segment {}", seg.id),
            parent: Some(index_of_id[&seg.parent_id]),
            length: seg.length,
            rest_angle: crate::model::norm_deg(abs - parent_abs),
            head: false,
            bend: 1.0,
            thickness: seg.thickness,
            color: None,
            ik_end: false,
            shape,
        });
        rest_abs.push(abs);
        let _ = seg.is_static;
    }
    Ok((FigureDef { nodes }, name, rest_abs, has_images))
}

/// The Pivot built-in figure (slot 1), from the original codec's
/// independently dumped values: the classic default stickman.
fn builtin_man() -> (FigureDef, Vec<f32>) {
    // (parent_id, length, angle_rad, thickness, primitive)
    let values: [(u8, f32, f64, f32, u8); 12] = [
        (0, 32.0, -1.5707963267948968, 14.0, 0),
        (1, 32.0, -1.5707963267948968, 14.0, 0),
        (2, 33.0, -1.5707963267948968, 14.0, 0),
        (3, 20.0, 1.5707963267948966, 20.0, 1),
        (2, 38.0, 2.356194490192345, 14.0, 0),
        (2, 38.0, 0.7853981633974483, 14.0, 0),
        (5, 40.0, 2.0943951023931957, 14.0, 0),
        (6, 40.0, 1.0471975511965979, 14.0, 0),
        (0, 50.0, 1.9634954084936207, 14.0, 0),
        (0, 50.0, 1.1780972450961724, 14.0, 0),
        (9, 50.0, 1.9634954084936207, 14.0, 0),
        (10, 50.0, 1.1780972450961724, 14.0, 0),
    ];
    let mut bytes = vec![values.len() as u8];
    for (i, (parent, length, angle, thickness, prim)) in values.iter().enumerate() {
        bytes.push(*parent);
        bytes.push((i + 1) as u8);
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes.extend_from_slice(&length.to_le_bytes());
        bytes.extend_from_slice(&angle.to_le_bytes());
        bytes.extend_from_slice(&thickness.to_le_bytes());
        bytes.push(*prim);
        bytes.push(0);
        bytes.extend_from_slice(&0u16.to_le_bytes());
    }
    bytes.push(0); // empty name
    let mut r = Reader::new(&bytes);
    let (def, _name, abs, _) = parse_figure(&mut r, false).expect("builtin figure parses");
    (def, abs)
}

fn inflate(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    flate2::read::ZlibDecoder::new(data)
        .read_to_end(&mut out)
        .map_err(|e| format!("zlib inflate failed: {e}"))?;
    Ok(out)
}

/// Convert one object's absolute angles into a model Pose. Object
/// angles arrive in segment record order, which parse_figure
/// preserved as nodes[1..] order. Parents always precede children
/// in that order, so each parent's absolute angle is the sum of
/// the relative angles chosen so far along its chain.
fn pose_from_abs(def: &FigureDef, rest_abs: &[f32], angles_rad: &[f64], root: [f32; 2]) -> Pose {
    let mut angles = vec![0.0f32]; // origin node
    let mut abs_chosen = vec![0.0f32; def.nodes.len()];
    for k in 1..def.nodes.len() {
        let abs = if (k - 1) < angles_rad.len() {
            rad_to_deg(angles_rad[k - 1])
        } else {
            rest_abs[k]
        };
        let parent = def.nodes[k].parent.unwrap_or(0);
        let rel = crate::model::norm_deg(abs - abs_chosen[parent]);
        angles.push(rel);
        abs_chosen[k] = abs_chosen[parent] + rel;
    }
    Pose { root, angles }
}

/// Import a legacy `.stk` figure file as a definition.
pub fn import_stk(data: &[u8]) -> Result<(FigureDef, String), String> {
    let payload;
    let bytes: &[u8] = if data.len() >= 3 && matches!(data[0], 0x79 | 0x7A) && data[1] == 0x78 {
        payload = inflate(&data[1..])?;
        &payload
    } else {
        data
    };
    // Modern streams carry no name; legacy ones end with str8.
    // parse_figure reads a name if present — try exact-fit first by
    // appending an empty name when the stream ends at the records.
    let mut r = Reader::new(bytes);
    match parse_figure(&mut r, false) {
        Ok((def, name, _, _)) if r.done() => Ok((def, name)),
        _ => {
            let mut extended = bytes.to_vec();
            extended.push(0);
            let mut r2 = Reader::new(&extended);
            let (def, _, _, _) = parse_figure(&mut r2, false)?;
            if r2.pos == extended.len() {
                Ok((def, String::new()))
            } else {
                Err("unparsed trailing bytes in .stk".into())
            }
        }
    }
}

/// Import a `.piv` file (versions 2, 3, 4 and 5) as a project.
pub fn import_piv(data: &[u8]) -> Result<Project, String> {
    if data.is_empty() {
        return Err("empty file".into());
    }
    let payload;
    let bytes: &[u8] = if (1..=5).contains(&data[0]) {
        data
    } else {
        payload = inflate(data)?;
        &payload
    };
    let mut r = Reader::new(bytes);
    let version = r.u8()?;
    match version {
        2 | 3 => import_v23(&mut r, version),
        4 => import_v4(&mut r),
        5 => import_v5(&mut r),
        v => Err(format!("unsupported .piv version {v} (this importer covers 2, 3, 4 and 5)")),
    }
}

struct FigType {
    def: FigureDef,
    rest_abs: Vec<f32>,
    name: String,
    has_images: bool,
}

/// One raw object record from a frame, shared by the importers.
struct RawObj {
    type_idx: usize,
    scale: f32,
    color: u32,
    angles: Vec<f64>,
    root: [f32; 2],
}

fn import_v23(r: &mut Reader, version: u8) -> Result<Project, String> {
    let has_background = if version == 2 { r.u8()? } else { 0 };
    let width = r.u32()? as f32;
    let height = r.u32()? as f32;
    if width <= 0.0 || height <= 0.0 {
        return Err("invalid canvas size".into());
    }
    if version == 2 && has_background == 1 {
        let marker = r.u8()?;
        if marker != 0 {
            return Err("unsupported v2 background marker".into());
        }
        skip_bmp(r)?;
    }
    if version == 3 {
        let slots = r.u8()?;
        if slots == 0 {
            return Err("background table missing default slot".into());
        }
        for _ in 1..slots {
            match r.u8()? {
                0 => skip_bmp(r)?,
                1 => skip_jpeg(r)?,
                k => return Err(format!("unsupported v3 background kind {k}")),
            }
            r.str8()?;
        }
    }
    let figure_slots = r.u8()? as usize;
    if figure_slots < 2 {
        return Err("invalid figure slot count".into());
    }
    let (bdef, babs) = builtin_man();
    let mut types: Vec<FigType> = vec![FigType {
        def: bdef,
        rest_abs: babs,
        name: "Default".into(),
        has_images: false,
    }];
    for _ in 2..figure_slots {
        let (def, name, abs, has_images) = parse_figure(r, false)?;
        types.push(FigType { def, rest_abs: abs, name, has_images });
    }
    let frame_count = r.u32()? as usize;
    // Instances are the object slots of the fullest frame (legacy
    // files carry no stable cross-frame identities).
    let mut raw_frames: Vec<(f32, Vec<RawObj>)> = Vec::new();
    let mut max_objects = 0;
    for _ in 0..frame_count {
        let duration = if version == 3 { r.u16()? as f32 } else { 1.0 };
        if version == 3 {
            r.u8()?; // background id
        }
        let object_count = r.u8()? as usize;
        max_objects = max_objects.max(object_count);
        let mut objects = Vec::new();
        for _ in 0..object_count {
            let type_id = r.u8()? as usize;
            if type_id == 0 || type_id > types.len() {
                return Err(format!("unknown figure type {type_id}"));
            }
            let scale = r.f32()?;
            let raw_color = r.u32()?;
            let fig = &types[type_id - 1];
            if fig.has_images {
                r.u8()?; // image/text flags byte
            }
            let seg_count = fig.def.nodes.len() - 1;
            let mut angles = Vec::new();
            for _ in 0..seg_count {
                let a = r.f64()?;
                if a.abs() > 100.0 {
                    return Err("unsupported angle-reset sentinel".into());
                }
                angles.push(a);
            }
            let root = [r.f32()?, r.f32()?];
            objects.push(RawObj {
                type_idx: type_id - 1,
                scale,
                color: colorref_to_rgb(raw_color),
                angles,
                root,
            });
        }
        raw_frames.push((duration.max(1.0), objects));
    }
    let fps = if version == 3 {
        let interval = r.u32()?;
        if interval == 0 {
            return Err("zero frame interval".into());
        }
        (1000.0 / interval as f32).round().max(1.0)
    } else {
        13.0
    };
    if !r.done() && version == 3 && r.data.len() - r.pos == 1 {
        r.u8()?; // optional loop flag
    }
    if !r.done() {
        return Err("trailing bytes after .piv payload".into());
    }
    Ok(assemble(types, raw_frames_to_instances(raw_frames, max_objects), fps, [width, height]))
}

fn raw_frames_to_instances(
    raw: Vec<(f32, Vec<RawObj>)>,
    max_objects: usize,
) -> (usize, Vec<(f32, Vec<Option<RawObj>>)>) {
    let frames = raw
        .into_iter()
        .map(|(d, objs)| {
            let mut slots: Vec<Option<RawObj>> = (0..max_objects).map(|_| None).collect();
            for (i, o) in objs.into_iter().enumerate() {
                slots[i] = Some(o);
            }
            (d, slots)
        })
        .collect();
    (max_objects, frames)
}

fn assemble(
    types: Vec<FigType>,
    (count, raw_frames): (usize, Vec<(f32, Vec<Option<RawObj>>)>),
    fps: f32,
    stage: [f32; 2],
) -> Project {
    // One instance per object slot; its figure type/scale/colour come
    // from the first frame that uses the slot.
    let mut figures: Vec<FigureInstance> = Vec::new();
    for slot in 0..count {
        let mut chosen: Option<&RawObj> = None;
        for (_, objs) in &raw_frames {
            if let Some(Some(o)) = objs.get(slot) {
                chosen = Some(o);
                break;
            }
        }
        let Some(first) = chosen else {
            continue;
        };
        let ty = &types[first.type_idx];
        let mut def = ty.def.clone();
        if (first.scale - 1.0).abs() > 0.001 {
            for node in def.nodes.iter_mut() {
                node.length *= first.scale;
            }
        }
        figures.push(FigureInstance {
            name: if ty.name.is_empty() {
                format!("Figure {}", slot + 1)
            } else {
                ty.name.clone()
            },
            color: first.color,
            def,
            hidden: false,
            clip: None,
        });
    }
    let slot_to_figure: Vec<usize> = (0..figures.len()).collect();
    let mut frames: Vec<Frame> = Vec::new();
    let mut last_poses: Vec<Pose> = figures
        .iter()
        .map(|f| f.def.rest_pose([stage[0] / 2.0, stage[1] / 2.0]))
        .collect();
    for (duration, objs) in &raw_frames {
        let mut poses = last_poses.clone();
        for (slot, obj) in objs.iter().enumerate() {
            if let Some(o) = obj {
                let fi = slot_to_figure[slot];
                let ty = &types[o.type_idx];
                poses[fi] = pose_from_abs(&figures[fi].def, &ty.rest_abs, &o.angles, o.root);
            }
        }
        last_poses = poses.clone();
        frames.push(Frame { poses, duration: *duration });
    }
    if frames.is_empty() {
        frames.push(Frame {
            poses: last_poses,
            duration: 1.0,
        });
    }
    Project {
        name: "Imported Pivot animation".into(),
        fps,
        stage,
        background: 0xF2F2F2,
        figures,
        frames,
        camera: Vec::new(),
        tween_playback: false,
        easing: Default::default(),
        camera_follow: None,
        particles: Vec::new(),
        strokes: Vec::new(),
        audio: Vec::new(),
        clips: Vec::new(),
    }
}

fn import_v4(r: &mut Reader) -> Result<Project, String> {
    let width = r.u32()? as f32;
    let height = r.u32()? as f32;
    if width <= 0.0 || height <= 0.0 {
        return Err("invalid canvas size".into());
    }
    let bg_slots = r.u8()?;
    if bg_slots == 0 {
        return Err("background table missing default slot".into());
    }
    for _ in 1..bg_slots {
        match r.u8()? {
            0 => skip_bmp(r)?,
            1 => skip_jpeg(r)?,
            k => return Err(format!("unsupported v4 background kind {k}")),
        }
        r.str8()?;
    }
    let figure_slots = r.u16()? as usize;
    if figure_slots < 2 {
        return Err("invalid figure slot count".into());
    }
    let (bdef, babs) = builtin_man();
    let mut types: Vec<FigType> = vec![FigType {
        def: bdef,
        rest_abs: babs,
        name: "Default".into(),
        has_images: false,
    }];
    for _ in 2..figure_slots {
        let (def, name, abs, has_images) = parse_figure(r, true)?;
        types.push(FigType { def, rest_abs: abs, name, has_images });
    }
    let frame_count = r.u32()? as usize;
    let mut raw_frames: Vec<(f32, Vec<RawObj>)> = Vec::new();
    let mut max_objects = 0;
    for _ in 0..frame_count {
        let duration = r.u16()? as f32;
        if duration == 0.0 {
            return Err("zero frame duration".into());
        }
        r.u8()?; // background id
        let object_count = r.u16()? as usize;
        max_objects = max_objects.max(object_count);
        let mut objects = Vec::new();
        for _ in 0..object_count {
            r.u32()?; // unique id
            let type_id = r.u16()? as usize;
            if type_id == 0 || type_id > types.len() {
                return Err(format!("unknown figure type {type_id}"));
            }
            let scale = r.f32()?;
            let raw_color = r.u32()?;
            r.u8()?; // transparency
            let fig = &types[type_id - 1];
            if fig.has_images {
                r.u8()?; // image/text flags byte
            }
            let seg_count = fig.def.nodes.len() - 1;
            let mut angles = Vec::new();
            for _ in 0..seg_count {
                let a = r.f64()?;
                if a.abs() > 100.0 {
                    return Err("unsupported angle-reset sentinel".into());
                }
                angles.push(a);
            }
            let root = [r.f32()?, r.f32()?];
            let len_flag = r.u8()?;
            if len_flag != 0 {
                for _ in 0..seg_count {
                    r.f32()?; // per-frame lengths: not representable, skipped
                }
            }
            let join_flag = r.u8()?;
            if join_flag != 0 {
                r.u16()?;
                r.u8()?;
            }
            objects.push(RawObj {
                type_idx: type_id - 1,
                scale,
                color: colorref_to_rgb(raw_color),
                angles,
                root,
            });
        }
        for _ in 0..object_count {
            r.u16()?; // draw order
        }
        raw_frames.push((duration, objects));
    }
    let delay = r.u32()?;
    if delay == 0 {
        return Err("zero frame delay".into());
    }
    let fps = (1000.0 / delay as f32).round().max(1.0);
    if !r.done() && r.data.len() - r.pos == 1 {
        r.u8()?;
    }
    if !r.done() {
        return Err("trailing bytes after .piv payload".into());
    }
    Ok(assemble(types, raw_frames_to_instances(raw_frames, max_objects), fps, [width, height]))
}

/// One parsed v5 segment record.
struct V5Seg {
    extra_u16: u16,
    parent_id: u16,
    id: u16,
    length: f32,
    angle_rad: f64,
    thickness: f32,
    primitive: u8,
    rgb: [u8; 3],
}

/// Parse one v5 figure stream (grammar from `codecs/piv_codec.py`
/// `_figure`). A flags byte switches off whole field groups; image
/// and text segments are rejected because their side tables are a
/// separate grammar that is not ported yet.
/// Skip a v5 UTF-16 string (u32 code-unit count + LE bytes),
/// returning the code units (text placements need them).
fn skip_utf16_string(r: &mut Reader) -> Result<Vec<u16>, String> {
    let n = r.u32()? as usize;
    let bytes = r.take(2 * n)?;
    Ok(bytes
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect())
}

/// Skip the image and text side tables of a v5 figure. Grammar per
/// the original's piv_side_tables.py (see docs/V5-SIDETABLES-SPEC.md):
/// image table first (iff any segment is primitive 2), then text
/// table (iff any is primitive 5); each starts with a u16 count.
/// Image/text artwork is not modelled — this only walks the bytes.
fn skip_side_tables(r: &mut Reader, segs: &[V5Seg], flags: u8) -> Result<(), String> {
    let has_text = segs.iter().any(|s| s.primitive == 5);
    if segs.iter().any(|s| s.primitive == 2) {
        let count = r.u16()? as usize;
        for seg in segs.iter().filter(|s| s.primitive == 2) {
            if seg.extra_u16 as usize >= count {
                return Err("image segment refers to a missing asset".into());
            }
        }
        for i in 0..count {
            r.take(16)?; // anchor start/end (i32 x4)
            let pointer = r.pos;
            let end = r.u64()? as usize;
            if end == pointer {
                // Unpatched placeholder: empty blob, nothing more.
                continue;
            }
            let allow_missing =
                i + 1 == count && !has_text && flags & 16 != 0 && flags & 64 == 0;
            if allow_missing && end > r.data.len() {
                // The end field may never have been written: the
                // bytes at `pointer` are then the figure's polyfill
                // count (u16, must be 0) and a printable name.
                let at = |o: usize| r.data.get(pointer + o).copied();
                if at(0) == Some(0) && at(1) == Some(0) {
                    let name_len = at(2).unwrap_or(0) as usize;
                    let fits = pointer + 3 + name_len <= r.data.len();
                    let printable = (0..name_len).all(|k| {
                        matches!(at(3 + k), Some(b) if (32..127).contains(&b))
                    });
                    if name_len > 0 && fits && printable {
                        r.pos = pointer; // entry consumed nothing
                        continue;
                    }
                }
            }
            if end < pointer + 8 || end > r.data.len() {
                return Err("image asset end pointer out of range".into());
            }
            r.pos = end;
        }
    }
    if has_text {
        let count = r.u16()? as usize;
        for seg in segs.iter().filter(|s| s.primitive == 5) {
            if seg.extra_u16 as usize >= count {
                return Err("text segment refers to a missing asset".into());
            }
        }
        for _ in 0..count {
            skip_utf16_string(r)?; // font name
            r.u8()?; // font style
            r.u32()?; // alignment
            let content = skip_utf16_string(r)?;
            let glyph_count = r.u8()? as usize;
            // Unique code units in first-appearance order own glyphs.
            let mut uniques: Vec<u16> = Vec::new();
            for u in &content {
                if !uniques.contains(u) {
                    uniques.push(*u);
                }
            }
            if uniques.len() != glyph_count {
                return Err("text glyph count does not match content".into());
            }
            let mut glyph_nodes: Vec<u32> = Vec::new();
            for _ in 0..glyph_count {
                let node_count = r.u32()?;
                glyph_nodes.push(node_count);
                if node_count == 0 {
                    continue;
                }
                // Opcodes are packed two per byte, low nibble first,
                // with no count: simulate the expansion (cubic = 3
                // nodes, everything else 1) until it reaches
                // node_count, tallying coordinate values per opcode
                // (M/L = 2, C = 6, Z = 0, V/H = 1).
                let mut expanded = 0u32;
                let mut values = 0usize;
                let mut high = false;
                let mut byte = 0u8;
                while expanded < node_count {
                    if !high {
                        byte = r.u8()?;
                    }
                    let op = if high { byte >> 4 } else { byte & 0x0F };
                    high = !high;
                    if op > 5 {
                        return Err("invalid glyph opcode".into());
                    }
                    expanded += if op == 2 { 3 } else { 1 };
                    values += match op {
                        0 | 1 => 2,
                        2 => 6,
                        3 => 0,
                        _ => 1,
                    };
                    if expanded > node_count {
                        return Err("glyph opcodes overshoot node count".into());
                    }
                }
                r.take(4 * values)?; // the flat f32 coordinate block
            }
            // Placements: visible glyphs (node_count > 0) store an
            // (x, y) f32 pair per content unit; empty glyphs store
            // nothing.
            for unit in &content {
                let gi = uniques.iter().position(|u| u == unit).unwrap();
                if glyph_nodes[gi] > 0 {
                    r.take(8)?;
                }
            }
        }
    }
    Ok(())
}

fn parse_figure_v5(r: &mut Reader) -> Result<FigType, String> {
    let flags = r.u8()?;
    let count = r.u16()? as usize;
    if count == 0 {
        return Err("empty figure".into());
    }
    let mut segs: Vec<V5Seg> = Vec::new();
    let mut has_media = false;
    for i in 0..count {
        let parent_id = r.u16()?;
        let id = if flags & 32 != 0 {
            (i + 1) as u16 // implicit ids
        } else {
            r.u16()?
        };
        let length = r.f32()?;
        let angle_rad = r.f64()?;
        let thickness = r.f32()?;
        let primitive = if flags & 1 != 0 { 0 } else { r.u8()? };
        if !matches!(primitive, 0 | 1 | 2 | 3 | 4 | 5 | 6) {
            return Err(format!("unsupported segment primitive {primitive}"));
        }
        if flags & 2 == 0 {
            r.u8()?; // aux (static flag etc.; not modelled)
        }
        let rgb = if flags & 4 != 0 {
            [0, 0, 0]
        } else {
            let b = r.take(3)?;
            [b[0], b[1], b[2]]
        };
        if flags & 8 == 0 {
            r.u8()?; // transparency (not modelled)
        }
        if flags & 128 == 0 && r.u8()? != 0 {
            r.take(3)?; // secondary colour rgb
            r.u8()?; // secondary colour alpha
        }
        let mut extra_u16 = 0u16;
        if primitive == 2 || primitive == 5 {
            has_media = true;
            extra_u16 = r.u16()?; // extra: asset index within its table
            r.u8()?;
            r.u8()?;
        }
        segs.push(V5Seg {
            parent_id,
            id,
            length,
            angle_rad,
            thickness,
            primitive,
            rgb,
            extra_u16,
        });
    }
    let _ = has_media;
    // Bends: per-segment arc values. The model has no bend channel,
    // so the values are read and discarded.
    let bend_count = r.u16()? as usize;
    for _ in 0..bend_count {
        r.u16()?; // segment index
        r.f64()?; // bend value (not modelled)
    }
    // Image/text side tables sit between the bends and the
    // polyfills; skip them precisely (assets are not modelled).
    if segs.iter().any(|s| s.primitive == 2 || s.primitive == 5) {
        skip_side_tables(r, &segs, flags)?;
    }
    // Polyfills: filled regions between joints; not modelled, skipped.
    let poly_count = r.u16()? as usize;
    for _ in 0..poly_count {
        let seg_count = r.u16()? as usize;
        r.take(3)?; // colour rgb
        r.u8()?; // alpha
        for _ in 0..seg_count {
            r.u16()?; // joint id
        }
    }
    // Z-order permutation for segments then polyfills; not modelled.
    if flags & 16 == 0 {
        for _ in 0..count {
            r.u16()?;
        }
        for _ in 0..poly_count {
            r.u16()?;
        }
    }
    if flags & 64 != 0 {
        r.u32()?; // global style: packed outline thickness
        r.take(3)?; // global style rgb
        r.u8()?; // global style transparency
    }
    let name = r.str8().unwrap_or_default();

    // Segments stay in record order (same reasoning as the legacy
    // parser): object angle arrays index by record order and the
    // model's FK needs parents before children.
    let mut index_of_id: std::collections::HashMap<u16, usize> = Default::default();
    let mut abs_of_id: std::collections::HashMap<u16, f32> = Default::default();
    index_of_id.insert(0, 0);
    abs_of_id.insert(0, 0.0);
    for (k, seg) in segs.iter().enumerate() {
        if seg.id == 0 {
            return Err("figure segment id 0 is reserved for the origin".into());
        }
        if index_of_id.contains_key(&seg.id) {
            return Err(format!("duplicate segment id {}", seg.id));
        }
        if !index_of_id.contains_key(&seg.parent_id) {
            return Err("figure segment parent follows its child (unsupported order)".into());
        }
        index_of_id.insert(seg.id, k + 1);
        abs_of_id.insert(seg.id, rad_to_deg(seg.angle_rad));
    }
    let mut nodes = vec![NodeDef {
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
    }];
    let mut rest_abs = vec![0.0f32];
    for seg in &segs {
        let abs = abs_of_id[&seg.id];
        let parent_abs = abs_of_id[&seg.parent_id];
        let shape = match seg.primitive {
            1 | 3 | 4 => Shape::Circle,
            6 => Shape::Polygon(4),
            _ => Shape::Line,
        };
        nodes.push(NodeDef {
            name: format!("segment {}", seg.id),
            parent: Some(index_of_id[&seg.parent_id]),
            length: seg.length,
            rest_angle: crate::model::norm_deg(abs - parent_abs),
            head: false,
            bend: 1.0,
            thickness: seg.thickness,
            color: Some(
                ((seg.rgb[0] as u32) << 16) | ((seg.rgb[1] as u32) << 8) | seg.rgb[2] as u32,
            ),
            ik_end: false,
            shape,
        });
        rest_abs.push(abs);
    }
    Ok(FigType {
        def: FigureDef { nodes },
        rest_abs,
        name,
        has_images: segs.iter().any(|s| s.primitive == 2 || s.primitive == 5),
    })
}

/// One raw v5 object record from a keyframe.
struct V5Obj {
    uid: u32,
    type_idx: usize,
    scale: f32,
    color: u32,
    angles: Vec<f64>,
    root: [f32; 2],
}

fn import_v5(r: &mut Reader) -> Result<Project, String> {
    let width = r.u32()? as f32;
    let height = r.u32()? as f32;
    if width <= 0.0 || height <= 0.0 {
        return Err("invalid canvas size".into());
    }
    // Background table. Slot 0 is the implicit default (no entry in
    // the stream); image backgrounds are skipped and gradients keep
    // their first colour.
    let bg_slots = r.u16()? as usize;
    if bg_slots < 1 {
        return Err("background table missing default slot".into());
    }
    let mut bg_colors: Vec<Option<u32>> = vec![None; bg_slots];
    for slot in 1..bg_slots {
        match r.u8()? {
            0 | 1 => {
                let end = r.u64()? as usize;
                if end < r.pos || end > r.data.len() {
                    return Err("invalid background end pointer".into());
                }
                r.take(end - r.pos)?;
            }
            2 => {
                bg_colors[slot] = Some(colorref_to_rgb(r.u32()?));
            }
            3 | 4 => {
                bg_colors[slot] = Some(colorref_to_rgb(r.u32()?)); // colour_a
                r.u32()?; // colour_b
                for _ in 0..4 {
                    r.f32()?; // gradient geometry
                }
            }
            k => return Err(format!("unsupported v5 background kind {k}")),
        }
        r.str8()?; // background name
    }
    // Figure table: slot 1 is the implicit built-in figure.
    let figure_slots = r.u16()? as usize;
    if figure_slots < 2 {
        return Err("invalid figure slot count".into());
    }
    let (bdef, babs) = builtin_man();
    let mut types: Vec<FigType> = vec![FigType {
        def: bdef,
        rest_abs: babs,
        name: "Default".into(),
        has_images: false,
    }];
    for _ in 2..figure_slots {
        types.push(parse_figure_v5(r)?);
    }
    // Keyframes.
    let frame_count = r.u32()? as usize;
    let mut raw_frames: Vec<(f32, u16, Vec<V5Obj>)> = Vec::new();
    let mut camera: Vec<CameraKey> = Vec::new();
    for fi in 0..frame_count {
        let duration = r.u16()?;
        let bg_mix = r.u8()?;
        let bg_id = r.u16()?;
        if bg_mix != 0 {
            r.u16()?; // secondary background id (blend not modelled)
        }
        let object_count = r.u16()? as usize;
        let mut objects = Vec::new();
        for _ in 0..object_count {
            let uid = r.u32()?;
            let type_id = r.u16()? as usize;
            if type_id == 0 || type_id > types.len() {
                return Err(format!("unknown figure type {type_id}"));
            }
            let scale = r.f32()?;
            let colour_raw = r.u32()?;
            r.u8()?; // field28 (transparency; not modelled)
            if types[type_id - 1].has_images {
                r.u8()?; // image/text flags byte
            }
            let seg_count = types[type_id - 1].def.nodes.len() - 1;
            let mut angles = Vec::new();
            for _ in 0..seg_count {
                let a = r.f64()?;
                if a.abs() > 100.0 {
                    return Err("unsupported angle-reset sentinel".into());
                }
                angles.push(a);
            }
            let root = [r.f32()?, r.f32()?];
            let len_flag = r.u16()?;
            if len_flag != 0 {
                for _ in 0..seg_count {
                    r.f32()?; // per-frame lengths: not representable, skipped
                }
            }
            let bend_flag = r.u16()?;
            if bend_flag != 0 {
                for _ in 0..seg_count {
                    r.f64()?; // per-frame bends: not modelled, skipped
                }
            }
            let join_flag = r.u8()?;
            if join_flag != 0 {
                r.u16()?; // target object slot
                r.u16()?; // target handle id
            }
            objects.push(V5Obj {
                uid,
                type_idx: type_id - 1,
                scale,
                color: colorref_to_rgb(colour_raw & 0x00FF_FFFF),
                angles,
                root,
            });
        }
        for _ in 0..object_count {
            r.u16()?; // draw order (instances keep first-seen order)
        }
        let is_default = r.u8()?;
        if is_default == 0 {
            let x = r.f32()?;
            let y = r.f32()?;
            r.f64()?; // camera angle: not modelled
            let zoom = r.f32()?;
            camera.push(CameraKey { frame: fi, x, y, zoom });
        }
        raw_frames.push((duration.max(1) as f32, bg_id, objects));
    }
    let fps = r.u32()? as f32;
    // Any trailer bytes (auto-easing flag etc.) are ignored.

    // Assembly: instances are keyed by the objects' unique ids in
    // first-seen order (v5 files carry stable cross-frame
    // identities, unlike the legacy formats). Type, scale and
    // colour come from each id's first occurrence; a later object
    // reusing the id with a different figure type is skipped for
    // that frame, mirroring the original, which holds the previous
    // pose for incompatible rigs.
    let mut index_of_uid: std::collections::HashMap<u32, usize> = Default::default();
    let mut first_objects: Vec<&V5Obj> = Vec::new();
    for (_, _, objects) in &raw_frames {
        for o in objects {
            if !index_of_uid.contains_key(&o.uid) {
                index_of_uid.insert(o.uid, first_objects.len());
                first_objects.push(o);
            }
        }
    }
    let mut figures: Vec<FigureInstance> = Vec::new();
    for (i, first) in first_objects.iter().enumerate() {
        let ty = &types[first.type_idx];
        let mut def = ty.def.clone();
        if (first.scale - 1.0).abs() > 0.001 {
            for node in def.nodes.iter_mut() {
                node.length *= first.scale;
            }
        }
        figures.push(FigureInstance {
            name: if ty.name.is_empty() {
                format!("Figure {}", i + 1)
            } else {
                ty.name.clone()
            },
            color: first.color,
            def,
            hidden: false,
            clip: None,
        });
    }
    let mut last_poses: Vec<Pose> = figures
        .iter()
        .map(|f| f.def.rest_pose([width / 2.0, height / 2.0]))
        .collect();
    let mut frames: Vec<Frame> = Vec::new();
    for (duration, _, objects) in &raw_frames {
        let mut poses = last_poses.clone();
        for o in objects {
            let ii = index_of_uid[&o.uid];
            if first_objects[ii].type_idx != o.type_idx {
                continue; // same id, different rig: hold previous pose
            }
            let ty = &types[o.type_idx];
            poses[ii] = pose_from_abs(&figures[ii].def, &ty.rest_abs, &o.angles, o.root);
        }
        last_poses = poses.clone();
        frames.push(Frame { poses, duration: *duration });
    }
    if frames.is_empty() {
        frames.push(Frame {
            poses: last_poses,
            duration: 1.0,
        });
    }
    let background = raw_frames
        .first()
        .and_then(|(_, bg_id, _)| bg_colors.get(*bg_id as usize).copied().flatten())
        .unwrap_or(0xF2F2F2);
    Ok(Project {
        name: "Imported Pivot animation".into(),
        fps,
        stage: [width, height],
        background,
        figures,
        frames,
        camera,
        tween_playback: false,
        easing: Default::default(),
        camera_follow: None,
        particles: Vec::new(),
        strokes: Vec::new(),
        audio: Vec::new(),
        clips: Vec::new(),
    })
}

// ---------- Export ----------

fn write_figure(out: &mut Vec<u8>, def: &FigureDef, name: &str, rest_abs: &[f32]) {
    let seg_count = def.nodes.len() - 1;
    out.push(seg_count as u8);
    for (i, node) in def.nodes.iter().enumerate().skip(1) {
        let parent_id = node.parent.unwrap_or(0) as u8;
        out.push(parent_id);
        out.push(i as u8);
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&node.length.to_le_bytes());
        out.extend_from_slice(&deg_to_rad(rest_abs[i]).to_le_bytes());
        out.extend_from_slice(&node.thickness.to_le_bytes());
        out.push(match node.shape {
            Shape::Circle => 1,
            Shape::Polygon(4) => 6,
            _ => 0,
        });
        out.push(0); // static
        out.extend_from_slice(&0u16.to_le_bytes());
    }
    let name_bytes = name.as_bytes();
    out.push(name_bytes.len().min(255) as u8);
    out.extend_from_slice(&name_bytes[..name_bytes.len().min(255)]);
}

fn rest_absolutes(def: &FigureDef) -> Vec<f32> {
    let mut abs = vec![0.0f32; def.nodes.len()];
    for (i, node) in def.nodes.iter().enumerate() {
        abs[i] = match node.parent {
            None => 0.0,
            Some(p) => abs[p] + node.rest_angle,
        };
    }
    abs
}

fn pose_absolutes(def: &FigureDef, pose: &Pose) -> Vec<f32> {
    let mut abs = vec![0.0f32; def.nodes.len()];
    for (i, node) in def.nodes.iter().enumerate() {
        abs[i] = match node.parent {
            None => 0.0,
            Some(p) => abs[p] + pose.angles[i],
        };
    }
    abs
}

/// Export one figure definition as a legacy `.stk` file.
pub fn export_stk(def: &FigureDef, name: &str) -> Vec<u8> {
    let mut out = Vec::new();
    write_figure(&mut out, def, name, &rest_absolutes(def));
    out
}

/// Export the project as a legacy `.piv` version 3 file (raw,
/// uncompressed — Pivot also writes them zlib-wrapped, and both
/// forms load identically).
pub fn export_piv_v3(project: &Project) -> Vec<u8> {
    let mut out = Vec::new();
    out.push(3u8); // version
    out.extend_from_slice(&(project.stage[0] as u32).to_le_bytes());
    out.extend_from_slice(&(project.stage[1] as u32).to_le_bytes());
    out.push(1u8); // background slots: default only
    out.push((project.figures.len() + 2) as u8); // figure slots (builtin + these, per decoder range)
    for fig in &project.figures {
        write_figure(&mut out, &fig.def, &fig.name, &rest_absolutes(&fig.def));
    }
    out.extend_from_slice(&(project.frames.len() as u32).to_le_bytes());
    for frame in &project.frames {
        out.extend_from_slice(&(frame.duration.max(1.0) as u16).to_le_bytes());
        out.push(0u8); // background id
        out.push(project.figures.len() as u8);
        for (fi, fig) in project.figures.iter().enumerate() {
            out.push((fi + 2) as u8); // figure type id
            out.extend_from_slice(&1.0f32.to_le_bytes()); // scale
            out.extend_from_slice(&rgb_to_colorref(fig.color).to_le_bytes());
            let abs = pose_absolutes(&fig.def, &frame.poses[fi]);
            for i in 1..fig.def.nodes.len() {
                out.extend_from_slice(&deg_to_rad(abs[i]).to_le_bytes());
            }
            out.extend_from_slice(&frame.poses[fi].root[0].to_le_bytes());
            out.extend_from_slice(&frame.poses[fi].root[1].to_le_bytes());
        }
    }
    let interval = (1000.0 / project.fps.max(1.0)).round() as u32;
    out.extend_from_slice(&interval.max(1).to_le_bytes());
    out.push(1u8); // loop flag
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_man_parses_with_known_geometry() {
        let (def, abs) = builtin_man();
        assert_eq!(def.nodes.len(), 13); // origin + 12 segments
        assert!((def.nodes[1].length - 32.0).abs() < 0.01);
        assert!((abs[1] - 180.0).abs() < 0.01, "first segment points up: {}", abs[1]);
        assert!(matches!(def.nodes[4].shape, Shape::Circle), "4th segment is the head circle");
    }

    #[test]
    fn stk_roundtrip_preserves_geometry() {
        let def = FigureDef::humanoid();
        let bytes = export_stk(&def, "Hero");
        let (back, name) = import_stk(&bytes).unwrap();
        assert_eq!(name, "Hero");
        assert_eq!(back.nodes.len(), def.nodes.len());
        for (a, b) in def.nodes.iter().zip(back.nodes.iter()) {
            assert!((a.length - b.length).abs() < 0.01, "length {} vs {}", a.length, b.length);
            assert!(
                (crate::model::norm_deg(a.rest_angle - b.rest_angle)).abs() < 0.05,
                "angle {} vs {}",
                a.rest_angle,
                b.rest_angle
            );
        }
    }

    #[test]
    fn piv_v3_roundtrip_preserves_frames_and_poses() {
        let project = Project::new_demo();
        let bytes = export_piv_v3(&project);
        let back = import_piv(&bytes).unwrap();
        assert_eq!(back.frames.len(), project.frames.len());
        assert_eq!(back.figures.len(), 1);
        assert!((back.fps - project.fps).abs() < 1.0);
        for (a, b) in project.frames.iter().zip(back.frames.iter()) {
            let (pa, pb) = (&a.poses[0], &b.poses[0]);
            assert!((pa.root[0] - pb.root[0]).abs() < 0.5);
            assert!((pa.root[1] - pb.root[1]).abs() < 0.5);
            for (x, y) in pa.angles.iter().zip(pb.angles.iter()) {
                assert!(
                    (crate::model::norm_deg(x - y)).abs() < 0.1,
                    "angle {x} vs {y}"
                );
            }
        }
    }

    #[test]
    fn write_fixtures_for_cross_check() {
        let project = Project::new_demo();
        std::fs::write("/tmp/styk-export-v3.piv", export_piv_v3(&project)).unwrap();
        std::fs::write("/tmp/styk-figure.stk", export_stk(&FigureDef::quadruped(), "Quad")).unwrap();
    }

    #[test]
    fn handmade_v3_file_imports_with_known_values() {
        let Ok(bytes) = std::fs::read("/tmp/handmade.piv") else {
            return; // fixture generated by the cross-check script
        };
        let project = import_piv(&bytes).unwrap();
        assert_eq!(project.stage, [800.0, 600.0]);
        assert!((project.fps - 25.0).abs() < 0.1, "fps {}", project.fps);
        assert_eq!(project.frames.len(), 2);
        assert_eq!(project.figures.len(), 1);
        assert_eq!(project.figures[0].name, "Tripod");
        assert_eq!(project.figures[0].color, 0xFF0000);
        assert_eq!(project.figures[0].def.nodes.len(), 4); // origin + 3
        let pose = &project.frames[1].poses[0];
        assert!((pose.root[0] - 121.0).abs() < 0.01 && (pose.root[1] - 222.0).abs() < 0.01);
    }

    #[test]
    fn piv_v5_minimal_file_imports_with_known_values() {
        // A minimal v5 file packed byte-by-byte per piv_codec.py's
        // grammar: one flags=0 figure with two segments, one
        // keyframe holding one object of that figure, fps 24.
        let mut b: Vec<u8> = Vec::new();
        b.push(5); // version
        b.extend_from_slice(&800u32.to_le_bytes()); // canvas width
        b.extend_from_slice(&600u32.to_le_bytes()); // canvas height
        b.extend_from_slice(&1u16.to_le_bytes()); // background slots: default only
        b.extend_from_slice(&3u16.to_le_bytes()); // figure slots: builtin + 1 file figure
        // Figure, flags = 0 (all optional fields present, explicit ids).
        b.push(0); // flags
        b.extend_from_slice(&2u16.to_le_bytes()); // segment count
        // Segment 1: parent 0, id 1, length 50, angle up, thickness 6, line, red.
        b.extend_from_slice(&0u16.to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&50.0f32.to_le_bytes());
        b.extend_from_slice(&(-std::f64::consts::FRAC_PI_2).to_le_bytes());
        b.extend_from_slice(&6.0f32.to_le_bytes());
        b.push(0); // primitive: line
        b.push(0); // aux
        b.extend_from_slice(&[255, 0, 0]); // rgb
        b.push(0); // transparency
        b.push(0); // no secondary colour
        // Segment 2: parent 1, id 2, length 30, angle right, thickness 4, circle, blue.
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&2u16.to_le_bytes());
        b.extend_from_slice(&30.0f32.to_le_bytes());
        b.extend_from_slice(&0.0f64.to_le_bytes());
        b.extend_from_slice(&4.0f32.to_le_bytes());
        b.push(1); // primitive: circle
        b.push(0); // aux
        b.extend_from_slice(&[0, 0, 255]);
        b.push(0); // transparency
        b.push(0); // no secondary colour
        b.extend_from_slice(&0u16.to_le_bytes()); // bend count
        b.extend_from_slice(&0u16.to_le_bytes()); // polyfill count
        b.extend_from_slice(&0u16.to_le_bytes()); // z-order: segment 1
        b.extend_from_slice(&1u16.to_le_bytes()); // z-order: segment 2
        b.push(3); // name length
        b.extend_from_slice(b"Duo");
        // One keyframe with one object of the file figure.
        b.extend_from_slice(&1u32.to_le_bytes()); // frame count
        b.extend_from_slice(&2u16.to_le_bytes()); // duration
        b.push(0); // background mix
        b.extend_from_slice(&0u16.to_le_bytes()); // background id
        b.extend_from_slice(&1u16.to_le_bytes()); // object count
        b.extend_from_slice(&42u32.to_le_bytes()); // unique id
        b.extend_from_slice(&2u16.to_le_bytes()); // figure type id
        b.extend_from_slice(&1.0f32.to_le_bytes()); // scale
        b.extend_from_slice(&0x0000_FF00u32.to_le_bytes()); // colour: COLORREF green
        b.push(0); // field28
        b.extend_from_slice(&(-std::f64::consts::FRAC_PI_2).to_le_bytes()); // angle seg 1
        b.extend_from_slice(&0.0f64.to_le_bytes()); // angle seg 2
        b.extend_from_slice(&111.0f32.to_le_bytes()); // root x
        b.extend_from_slice(&222.0f32.to_le_bytes()); // root y
        b.extend_from_slice(&0u16.to_le_bytes()); // len flag
        b.extend_from_slice(&0u16.to_le_bytes()); // bend flag
        b.push(0); // join flag
        b.extend_from_slice(&0u16.to_le_bytes()); // draw order
        b.push(1); // camera is default
        b.extend_from_slice(&24u32.to_le_bytes()); // fps

        let project = import_piv(&b).unwrap();
        assert_eq!(project.stage, [800.0, 600.0]);
        assert!((project.fps - 24.0).abs() < 0.01, "fps {}", project.fps);
        assert_eq!(project.figures.len(), 1);
        let fig = &project.figures[0];
        assert_eq!(fig.name, "Duo");
        assert_eq!(fig.color, 0x00FF00);
        assert_eq!(fig.def.nodes.len(), 3); // origin + 2 segments
        assert!((fig.def.nodes[1].length - 50.0).abs() < 0.01);
        assert_eq!(fig.def.nodes[1].color, Some(0xFF0000));
        assert!(matches!(fig.def.nodes[2].shape, Shape::Circle));
        assert_eq!(fig.def.nodes[2].color, Some(0x0000FF));
        assert_eq!(project.frames.len(), 1);
        assert!((project.frames[0].duration - 2.0).abs() < 0.01);
        let pose = &project.frames[0].poses[0];
        assert!((pose.root[0] - 111.0).abs() < 0.01 && (pose.root[1] - 222.0).abs() < 0.01);
        assert!((pose.angles[1] - 180.0).abs() < 0.05, "seg 1 rel {}", pose.angles[1]);
        assert!((pose.angles[2] + 90.0).abs() < 0.05, "seg 2 rel {}", pose.angles[2]);
        assert!(project.camera.is_empty());
        assert_eq!(project.background, 0xF2F2F2); // slot 0 has no colour
    }

    #[test]
    fn v5_image_side_table_file_imports() {
        // Hand-built v5 file with an image-segment figure and an
        // image side table; the original app's Python decoder reads
        // the same bytes (figure "Pic", 1 image asset, root 50/60),
        // so a successful import here proves the skip offsets.
        let Ok(bytes) = std::fs::read("/tmp/v5-image.piv") else {
            return; // fixture generated by the cross-check script
        };
        let project = import_piv(&bytes).unwrap();
        assert_eq!(project.figures.len(), 1);
        assert_eq!(project.figures[0].name, "Pic");
        assert_eq!(project.figures[0].def.nodes.len(), 3); // origin + 2
        assert_eq!(project.frames.len(), 1);
        assert!((project.frames[0].duration - 2.0).abs() < 0.01);
        let pose = &project.frames[0].poses[0];
        assert!((pose.root[0] - 50.0).abs() < 0.01 && (pose.root[1] - 60.0).abs() < 0.01);
    }

    #[test]
    fn real_v5_file_from_the_original_encoder_imports() {
        // Generated by the original app's own Python encoder
        // (encode_piv_project over a migrated legacy figure) — the
        // strongest available oracle for the v5 grammar.
        let Ok(bytes) = std::fs::read("/tmp/real-v5.piv") else {
            return; // fixture generated by the cross-check script
        };
        let project = import_piv(&bytes).unwrap();
        assert_eq!(project.stage, [800.0, 600.0]);
        assert!((project.fps - 24.0).abs() < 0.1, "fps {}", project.fps);
        assert_eq!(project.figures.len(), 1);
        assert_eq!(project.figures[0].name, "Tripod");
        assert_eq!(project.figures[0].def.nodes.len(), 4);
        assert_eq!(project.frames.len(), 1);
        assert!((project.frames[0].duration - 3.0).abs() < 0.01);
        let pose = &project.frames[0].poses[0];
        assert!((pose.root[0] - 111.0).abs() < 0.01 && (pose.root[1] - 222.0).abs() < 0.01);
    }

    #[test]
    fn v5_text_side_table_file_imports() {
        // Hand-built v5 file with a text-segment figure; the
        // original's decoder reads the same bytes (figure "Txt",
        // 1 text asset, root 33/44). Exercises the nibble-walk
        // glyph skip and visibility-dependent placements.
        let Ok(bytes) = std::fs::read("/tmp/v5-text.piv") else {
            return; // fixture generated by the cross-check script
        };
        let project = import_piv(&bytes).unwrap();
        assert_eq!(project.figures[0].name, "Txt");
        let pose = &project.frames[0].poses[0];
        assert!((pose.root[0] - 33.0).abs() < 0.01 && (pose.root[1] - 44.0).abs() < 0.01);
    }

    #[test]
    fn original_builtin_stk_hex_parses() {
        // The exact default_pivot2.stk bytes embedded in the original
        // app's codec (piv_legacy_v2.py `_builtin`): a real .stk file.
        let hex = "0a0001000000007042d221337f7cd912400000e04000000000\
                   0102000000001042d221337f7cd912400000e04001000000\
                   0103000000002042d221337f7cd902400000e04000000000\
                   0104000000002042182d4454fb21e93f0000e04000000000\
                   030500000000204266732d3852c100400000e04000000000\
                   040600000000204266732d3852c1f03f0000e04000000000\
                   00070000000048425e3855297a6aff3f0000e04000000000\
                   0008000000004842d221337f7cd9f23f0000e04000000000\
                   07090000000048425e3855297a6aff3f0000e04000000000\
                   080a000000004842d221337f7cd9f23f0000e04000000000";
        let bytes: Vec<u8> = (0..hex.len())
            .step_by(2)
            .filter_map(|i| u8::from_str_radix(&hex[i..i + 2], 16).ok())
            .collect();
        // The hex above is the figure stream without a name; append
        // the empty name the format carries.
        let mut with_name = bytes.clone();
        with_name.push(0);
        let (def, _) = import_stk(&with_name).unwrap();
        assert_eq!(def.nodes.len(), 11); // origin + 10 segments
        assert!((def.nodes[1].length - 60.0).abs() < 0.5, "first segment is 60 long");
    }
}
