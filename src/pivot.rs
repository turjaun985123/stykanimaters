//! Legacy Pivot formats: `.piv` versions 2, 3 and 4, and `.stk`
//! figure files — import and export.
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
//! The modern v5 native container and Stick Nodes `.nodes` are
//! different, much larger formats and are not handled here.

use crate::model::{FigureDef, FigureInstance, Frame, NodeDef, Pose, Project, Shape};
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

/// Import a legacy `.piv` file (versions 2, 3, 4) as a project.
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
        v => Err(format!("unsupported .piv version {v} (this importer covers legacy 2, 3 and 4)")),
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
