//! Pivot formats: `.piv` versions 2, 3 and 4 (import and export),
//! the version 5 native container (import and export), and `.stk`
//! figure files (import and export).
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
//! The v5 grammar is ported from `codecs/piv_codec.py` (reader)
//! and `codecs/piv_full_writer.py` / `codecs/piv_document.py`
//! (writer). V5 import covers the full sequential grammar — image
//! side tables are walked precisely and text side tables are
//! walked with their contents recovered onto prim-5 segments (see
//! `docs/V5-SIDETABLES-SPEC.md`); v5 export writes the subset of
//! the format this model can express, including text side tables
//! whose glyph outlines are the port's 3x5 bitmap font drawn as
//! tiny square paths. Stick Nodes `.nodes` is a different, much
//! larger format and is not handled here.

use crate::model::{
    CameraKey, FigureDef, FigureInstance, Frame, ImageAsset, NodeDef, Pose, Project, SegmentMedia,
    Shape,
};
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
        min_angle: None,
        max_angle: None,
        style: crate::model::NodeStyle::default(),
        media: None,
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

// ---------- Probe (import diagnostics) ----------

/// A cheap header summary of a `.piv` file: what
/// [`probe_piv`] can report without importing the animation.
#[derive(Clone, Debug, PartialEq)]
pub struct PivProbe {
    /// Container version byte (2, 3, 4 or 5).
    pub version: u8,
    /// Stage size in pixels, as stored in the header.
    pub canvas: (u32, u32),
    /// Playback rate. v2 stores no rate (Pivot's fixed 13 fps);
    /// v3/v4 derive it from the trailer's frame interval exactly
    /// as [`import_piv`] does; v5 stores it directly in its
    /// trailer.
    pub fps: f32,
    /// The figure-slot count stored in the header, in the
    /// importers' own convention: slot 1 is the implicit
    /// built-in man, so a file with one stored figure reports 3.
    pub figure_slots: usize,
    /// The frame/keyframe count stored after the figure table.
    pub frames: usize,
    /// The file carries background entries beyond the implicit
    /// default slot (v2: its has-background byte is set; v3/v4/v5:
    /// the background table holds more than the default slot).
    pub has_backgrounds: bool,
}

/// Inspect a `.piv` file's header and counts without importing
/// it. The probe never builds a [`Project`] and never assembles
/// poses; every version's fields are reachable from the header
/// plus size-only structural walks, so no version needs a
/// full-import fallback:
///
/// * v2 — canvas, slot count and frame count all sit in the
///   header/figure-table preamble, and fps is the fixed 13; the
///   frame records are still walked (skipped by size) so a
///   truncated file errors here just as it would on import.
/// * v3/v4 — the same walk reaches the trailer's frame
///   interval/delay, from which fps derives with the importers'
///   own formula.
/// * v5 — the keyframe count is a plain header field after the
///   figure table (parsed with `parse_figure_v5`, side tables
///   included); the keyframes are then walked by size —
///   honouring the per-object length/bend arrays and the
///   optional camera record — to reach the trailer's stored
///   fps.
///
/// Figure records are parsed with the importers' own
/// `parse_figure`/`parse_figure_v5` because the frame walks
/// need each type's segment count (and whether its objects
/// carry the image/text flags byte); nothing else is retained.
pub fn probe_piv(data: &[u8]) -> Result<PivProbe, String> {
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
        2 | 3 => probe_v23(&mut r, version),
        4 => probe_v4(&mut r),
        5 => probe_v5(&mut r),
        v => Err(format!("unsupported .piv version {v} (this probe covers 2, 3, 4 and 5)")),
    }
}

/// Skip one v2/v3 frame object record (type, scale, colour,
/// optional image/text flags byte, absolute angles, root) using
/// the owning type's segment count. Mirrors the record layout
/// [`import_v23`] reads.
fn skip_object_v23(r: &mut Reader, seg_counts: &[usize], img_flags: &[bool]) -> Result<(), String> {
    let type_id = r.u8()? as usize;
    if type_id == 0 || type_id > seg_counts.len() {
        return Err(format!("unknown figure type {type_id}"));
    }
    r.take(8)?; // scale f32 + colour COLORREF
    if img_flags[type_id - 1] {
        r.u8()?; // image/text flags byte
    }
    r.take(seg_counts[type_id - 1] * 8)?; // absolute angles, f64 each
    r.take(8)?; // root, 2 x f32
    Ok(())
}

fn probe_v23(r: &mut Reader, version: u8) -> Result<PivProbe, String> {
    let has_background = if version == 2 { r.u8()? } else { 0 };
    let width = r.u32()?;
    let height = r.u32()?;
    if width == 0 || height == 0 {
        return Err("invalid canvas size".into());
    }
    if version == 2 && has_background == 1 {
        let marker = r.u8()?;
        if marker != 0 {
            return Err("unsupported v2 background marker".into());
        }
        skip_bmp(r)?;
    }
    let mut bg_slots = 1usize;
    if version == 3 {
        let slots = r.u8()?;
        if slots == 0 {
            return Err("background table missing default slot".into());
        }
        bg_slots = slots as usize;
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
    let (bdef, _babs) = builtin_man();
    let mut seg_counts: Vec<usize> = vec![bdef.nodes.len() - 1];
    let mut img_flags: Vec<bool> = vec![false];
    for _ in 2..figure_slots {
        let (def, _name, _abs, has_images) = parse_figure(r, false)?;
        seg_counts.push(def.nodes.len() - 1);
        img_flags.push(has_images);
    }
    let frame_count = r.u32()? as usize;
    // Walk the frame records by size: v2 needs nothing past the
    // count, but the walk keeps truncation an error, as on
    // import; v3's fps lives in the trailer beyond the frames.
    let fps = if version == 2 {
        for _ in 0..frame_count {
            let object_count = r.u8()? as usize;
            for _ in 0..object_count {
                skip_object_v23(r, &seg_counts, &img_flags)?;
            }
        }
        13.0
    } else {
        for _ in 0..frame_count {
            r.u16()?; // duration
            r.u8()?; // background id
            let object_count = r.u8()? as usize;
            for _ in 0..object_count {
                skip_object_v23(r, &seg_counts, &img_flags)?;
            }
        }
        let interval = r.u32()?;
        if interval == 0 {
            return Err("zero frame interval".into());
        }
        (1000.0 / interval as f32).round().max(1.0)
    };
    if !r.done() && version == 3 && r.data.len() - r.pos == 1 {
        r.u8()?; // optional loop flag
    }
    if !r.done() {
        return Err("trailing bytes after .piv payload".into());
    }
    Ok(PivProbe {
        version,
        canvas: (width, height),
        fps,
        figure_slots,
        frames: frame_count,
        has_backgrounds: if version == 2 { has_background == 1 } else { bg_slots > 1 },
    })
}

fn probe_v4(r: &mut Reader) -> Result<PivProbe, String> {
    let width = r.u32()?;
    let height = r.u32()?;
    if width == 0 || height == 0 {
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
    let (bdef, _babs) = builtin_man();
    let mut seg_counts: Vec<usize> = vec![bdef.nodes.len() - 1];
    let mut img_flags: Vec<bool> = vec![false];
    for _ in 2..figure_slots {
        let (def, _name, _abs, has_images) = parse_figure(r, true)?;
        seg_counts.push(def.nodes.len() - 1);
        img_flags.push(has_images);
    }
    let frame_count = r.u32()? as usize;
    // Walk the frame records by size to reach the trailer's
    // frame delay, from which fps derives as on import.
    for _ in 0..frame_count {
        r.u16()?; // duration
        r.u8()?; // background id
        let object_count = r.u16()? as usize;
        for _ in 0..object_count {
            r.u32()?; // unique id
            let type_id = r.u16()? as usize;
            if type_id == 0 || type_id > seg_counts.len() {
                return Err(format!("unknown figure type {type_id}"));
            }
            r.take(9)?; // scale f32 + colour COLORREF + transparency
            if img_flags[type_id - 1] {
                r.u8()?; // image/text flags byte
            }
            let seg = seg_counts[type_id - 1];
            r.take(seg * 8)?; // absolute angles, f64 each
            r.take(8)?; // root, 2 x f32
            if r.u8()? != 0 {
                r.take(seg * 4)?; // per-frame lengths, f32 each
            }
            if r.u8()? != 0 {
                r.take(3)?; // join record: slot u16 + flag u8
            }
        }
        r.take(object_count * 2)?; // draw order, u16 each
    }
    let delay = r.u32()?;
    if delay == 0 {
        return Err("zero frame delay".into());
    }
    if !r.done() && r.data.len() - r.pos == 1 {
        r.u8()?; // optional loop flag
    }
    if !r.done() {
        return Err("trailing bytes after .piv payload".into());
    }
    Ok(PivProbe {
        version: 4,
        canvas: (width, height),
        fps: (1000.0 / delay as f32).round().max(1.0),
        figure_slots,
        frames: frame_count,
        has_backgrounds: bg_slots > 1,
    })
}

fn probe_v5(r: &mut Reader) -> Result<PivProbe, String> {
    let width = r.u32()?;
    let height = r.u32()?;
    if width == 0 || height == 0 {
        return Err("invalid canvas size".into());
    }
    let bg_slots = r.u16()? as usize;
    if bg_slots < 1 {
        return Err("background table missing default slot".into());
    }
    for _ in 1..bg_slots {
        match r.u8()? {
            0 | 1 => {
                let end = r.u64()? as usize;
                if end < r.pos || end > r.data.len() {
                    return Err("invalid background end pointer".into());
                }
                r.take(end - r.pos)?;
            }
            2 => {
                r.u32()?; // colour
            }
            3 | 4 => {
                r.take(24)?; // two colours + 4 x f32 gradient geometry
            }
            k => return Err(format!("unsupported v5 background kind {k}")),
        }
        r.str8()?; // background name
    }
    let figure_slots = r.u16()? as usize;
    if figure_slots < 2 {
        return Err("invalid figure slot count".into());
    }
    let (bdef, _babs) = builtin_man();
    let mut seg_counts: Vec<usize> = vec![bdef.nodes.len() - 1];
    let mut img_flags: Vec<bool> = vec![false];
    for _ in 2..figure_slots {
        let (t, _bends) = parse_figure_v5(r)?;
        seg_counts.push(t.def.nodes.len() - 1);
        img_flags.push(t.has_images);
    }
    let frame_count = r.u32()? as usize;
    // Walk the keyframes by size to reach the trailer's stored
    // fps (past it sit only ignored trailer bytes, as on import).
    for _ in 0..frame_count {
        r.u16()?; // duration
        let bg_mix = r.u8()?;
        r.u16()?; // background id
        if bg_mix != 0 {
            r.u16()?; // secondary background id
        }
        let object_count = r.u16()? as usize;
        for _ in 0..object_count {
            r.u32()?; // unique id
            let type_id = r.u16()? as usize;
            if type_id == 0 || type_id > seg_counts.len() {
                return Err(format!("unknown figure type {type_id}"));
            }
            r.take(9)?; // scale f32 + colour COLORREF + field28
            if img_flags[type_id - 1] {
                r.u8()?; // image/text flags byte
            }
            let seg = seg_counts[type_id - 1];
            r.take(seg * 8)?; // absolute angles, f64 each
            r.take(8)?; // root, 2 x f32
            if r.u16()? != 0 {
                r.take(seg * 4)?; // per-frame lengths, f32 each
            }
            if r.u16()? != 0 {
                r.take(seg * 8)?; // per-frame bends, f64 each
            }
            if r.u8()? != 0 {
                r.take(4)?; // join record: slot u16 + handle u16
            }
        }
        r.take(object_count * 2)?; // draw order, u16 each
        if r.u8()? == 0 {
            r.take(20)?; // camera record: x/y f32, angle f64, zoom f32
        }
    }
    let fps = r.u32()? as f32;
    Ok(PivProbe {
        version: 5,
        canvas: (width, height),
        fps,
        figure_slots,
        frames: frame_count,
        has_backgrounds: bg_slots > 1,
    })
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
            locked: false,
            opacity: 1.0,
            name: if ty.name.is_empty() {
                format!("Figure {}", slot + 1)
            } else {
                ty.name.clone()
            },
            color: first.color,
            def,
            hidden: false,
            clip: None,
            states: Vec::new(),
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
        frames.push(Frame { poses, duration: *duration, easing: None, bends: Vec::new() });
    }
    if frames.is_empty() {
        frames.push(Frame {
            poses: last_poses,
            duration: 1.0,
            easing: None,
            bends: Vec::new(),
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

/// V5 figure streams are parsed by `parse_figure_v5` below
/// (grammar from `codecs/piv_codec.py` `_figure`): a flags byte
/// switches off whole field groups, and image/text segments carry
/// side tables (see `skip_side_tables`).
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

/// Walk the image and text side tables of a v5 figure. Grammar per
/// the original's piv_side_tables.py (see docs/V5-SIDETABLES-SPEC.md):
/// image table first (iff any segment is primitive 2), then text
/// table (iff any is primitive 5); each starts with a u16 count.
/// Image artwork is not modelled — this only walks its bytes — but
/// text contents ARE recovered: returns the text entries' strings
/// in table order so prim-5 segments can get their text back.
fn skip_side_tables(r: &mut Reader, segs: &[V5Seg], flags: u8) -> Result<Vec<String>, String> {
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
    let mut text_entries: Vec<String> = Vec::new();
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
            text_entries.push(String::from_utf16_lossy(&content));
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
    Ok(text_entries)
}

/// Parse one v5 figure record. Also returns the figure's REST
/// bends (radians, one per segment in record order): the bend a
/// frame uses when its object carries no bend array (wave 7).
fn parse_figure_v5(r: &mut Reader) -> Result<(FigType, Vec<f32>), String> {
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
    // Bends: sparse per-segment arc values (radians), keyed by
    // 0-based segment ordinal (record order, NOT segment id) —
    // the original's decoder validates both the count and the
    // indices, and the last entry for an index wins.
    let bend_count = r.u16()? as usize;
    if bend_count > count {
        return Err(format!("invalid bend count {bend_count}"));
    }
    let mut rest_bends = vec![0.0f32; count];
    for _ in 0..bend_count {
        let index = r.u16()? as usize;
        let value = r.f64()?;
        if index >= count {
            return Err(format!("invalid bend index {index}"));
        }
        rest_bends[index] = value as f32;
    }
    // Image/text side tables sit between the bends and the
    // polyfills; walk them precisely. Image assets stay
    // unmodelled, but text contents are recovered below.
    let text_entries = if segs.iter().any(|s| s.primitive == 2 || s.primitive == 5) {
        skip_side_tables(r, &segs, flags)?
    } else {
        Vec::new()
    };
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
        // Text recovery: a prim-5 segment gets its side-table
        // content back as the node's media. The format stores no
        // text size; Pivot's own rendering spans the text's width
        // across the segment with height = object scale x the
        // placement bounds, so the size that reproduces "text
        // spans the segment" in the port's model (width units
        // 4n - 1 at size/5 per unit, `media.rs`) is
        // size = 5 * length / (4n - 1). Empty content renders
        // nothing; its size is nominal.
        let media = if seg.primitive == 5 {
            text_entries.get(seg.extra_u16 as usize).map(|content| {
                let n = content.chars().count() as f32;
                let size = if n > 0.0 {
                    5.0 * seg.length / (4.0 * n - 1.0)
                } else {
                    seg.length
                };
                SegmentMedia::Text {
                    content: content.clone(),
                    size,
                }
            })
        } else {
            None
        };
        nodes.push(NodeDef {
        min_angle: None,
        max_angle: None,
        style: crate::model::NodeStyle::default(),
        media,
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
    Ok((
        FigType {
            def: FigureDef { nodes },
            rest_abs,
            name,
            has_images: segs.iter().any(|s| s.primitive == 2 || s.primitive == 5),
        },
        rest_bends,
    ))
}

/// One raw v5 object record from a keyframe.
struct V5Obj {
    uid: u32,
    type_idx: usize,
    scale: f32,
    color: u32,
    angles: Vec<f64>,
    root: [f32; 2],
    /// Per-segment bends (radians) when the object's bend_flag
    /// was set; empty means "use the figure record's rest bends".
    bends: Vec<f64>,
}

/// Per-node bend vec (origin = 0) for one v5 object (wave 7):
/// a present per-frame array replaces the figure record's rest
/// bends wholesale; without one the record bends apply — the
/// original's rule everywhere it evaluates a pose
/// (`bend_flag ? bends[i] : segment.bend_rad`). Segment k in the
/// file (0-based, record order) is node k+1 in the model.
fn v5_bends_per_node(rest: &[f32], frame: &[f64], node_count: usize) -> Vec<f32> {
    let mut v = vec![0.0f32; node_count];
    if frame.is_empty() {
        for (k, b) in rest.iter().enumerate() {
            if k + 1 < node_count {
                v[k + 1] = *b;
            }
        }
    } else {
        for (k, b) in frame.iter().enumerate() {
            if k + 1 < node_count {
                v[k + 1] = *b as f32;
            }
        }
    }
    v
}

/// Storage shape for `Frame.bends`: empty outer vec when no
/// figure bends in the frame; straight figures get empty inners.
fn compact_bends(per_figure: &[Vec<f32>]) -> Vec<Vec<f32>> {
    if per_figure.iter().all(|b| b.iter().all(|v| *v == 0.0)) {
        return Vec::new();
    }
    per_figure
        .iter()
        .map(|b| {
            if b.iter().all(|v| *v == 0.0) {
                Vec::new()
            } else {
                b.clone()
            }
        })
        .collect()
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
    let mut type_bends: Vec<Vec<f32>> = vec![Vec::new()];
    for _ in 2..figure_slots {
        let (t, b) = parse_figure_v5(r)?;
        types.push(t);
        type_bends.push(b);
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
            let mut bends: Vec<f64> = Vec::new();
            if bend_flag != 0 {
                for _ in 0..seg_count {
                    bends.push(r.f64()?);
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
                bends,
            });
        }
        for _ in 0..object_count {
            r.u16()?; // draw order (instances keep first-seen order)
        }
        let is_default = r.u8()?;
        if is_default == 0 {
            let x = r.f32()?;
            let y = r.f32()?;
            let cam_angle = r.f64()?; // stored radians; CameraKey.angle is degrees (wave 6)
            let zoom = r.f32()?;
            camera.push(CameraKey {
                frame: fi,
                x,
                y,
                zoom,
                angle: cam_angle.to_degrees() as f32,
            });
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
            locked: false,
            opacity: 1.0,
            name: if ty.name.is_empty() {
                format!("Figure {}", i + 1)
            } else {
                ty.name.clone()
            },
            color: first.color,
            def,
            hidden: false,
            clip: None,
            states: Vec::new(),
        });
    }
    let mut last_poses: Vec<Pose> = figures
        .iter()
        .map(|f| f.def.rest_pose([width / 2.0, height / 2.0]))
        .collect();
    // Bends start at each type's record (rest) bends and hold
    // across frames where the instance is absent, like poses.
    let mut last_bends: Vec<Vec<f32>> = figures
        .iter()
        .enumerate()
        .map(|(ii, f)| {
            v5_bends_per_node(&type_bends[first_objects[ii].type_idx], &[], f.def.nodes.len())
        })
        .collect();
    let mut frames: Vec<Frame> = Vec::new();
    for (duration, _, objects) in &raw_frames {
        let mut poses = last_poses.clone();
        let mut bends_now = last_bends.clone();
        for o in objects {
            let ii = index_of_uid[&o.uid];
            if first_objects[ii].type_idx != o.type_idx {
                continue; // same id, different rig: hold previous pose
            }
            let ty = &types[o.type_idx];
            poses[ii] = pose_from_abs(&figures[ii].def, &ty.rest_abs, &o.angles, o.root);
            bends_now[ii] = v5_bends_per_node(
                &type_bends[o.type_idx],
                &o.bends,
                figures[ii].def.nodes.len(),
            );
        }
        last_poses = poses.clone();
        last_bends = bends_now.clone();
        frames.push(Frame {
            poses,
            duration: *duration,
            easing: None,
            bends: compact_bends(&bends_now),
        });
    }
    if frames.is_empty() {
        frames.push(Frame {
            poses: last_poses,
            duration: 1.0,
            easing: None,
            bends: compact_bends(&last_bends),
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

/// Export the project as a legacy `.piv` version 2 file (raw,
/// uncompressed). Writer grammar: `codecs/piv_legacy_v2.py` —
/// version byte, a has-background byte, canvas, a u8 figure-slot
/// count, the figure streams, then frames with NO per-frame
/// header bytes and NO trailer.
///
/// V2 is the smallest legacy grammar, and these model features
/// have nowhere to go (all confirmed against the decoder):
///
/// * **Timing.** The format stores no frame interval at all; a
///   v2 file always plays at the fixed 13 fps and every frame is
///   a unit hold. The project's fps and per-frame durations are
///   dropped.
/// * **Background.** Written as "no background"; the project's
///   background colour is dropped (no legacy version round-trips
///   it through this port's importers).
/// * **Camera.** No legacy version stores one; camera keys are
///   dropped.
/// * **Per-frame lengths, joins, transparency, bends.** Not in
///   the grammar (lengths exist only in v4/v5).
/// * **Per-node colours, styles and media.** The shared figure
///   stream (`write_figure`) stores geometry and shape only, so
///   image/text segments export as plain segments (the same
///   limitation as v3).
///
/// Object scale is written as 1.0 (this model bakes instance
/// scale into figure defs at import, and poses carry no scale).
/// Figure and object counts are u8 fields, so at most 253
/// figures survive — the same ceiling `export_piv_v3` has.
pub fn export_piv_v2(project: &Project) -> Vec<u8> {
    let mut out = Vec::new();
    out.push(2u8); // version
    out.push(0u8); // has background: none
    out.extend_from_slice(&(project.stage[0] as u32).to_le_bytes());
    out.extend_from_slice(&(project.stage[1] as u32).to_le_bytes());
    out.push((project.figures.len() + 2) as u8); // figure slots (builtin + these, per decoder range)
    for fig in &project.figures {
        write_figure(&mut out, &fig.def, &fig.name, &rest_absolutes(&fig.def));
    }
    out.extend_from_slice(&(project.frames.len() as u32).to_le_bytes());
    for frame in &project.frames {
        out.push(project.figures.len() as u8); // object count
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
    out
}

/// Export the project as a legacy `.piv` version 4 file (raw,
/// uncompressed). Writer grammar: `codecs/piv_legacy_v4.py` —
/// like v3 but with u16 figure-slot and object counts, a u32
/// per-object unique id, a per-object transparency byte, an
/// optional per-frame length array (`len_flag`), a join flag,
/// and a u16 draw-order permutation per frame.
///
/// What survives that v2 drops: per-frame durations (stored
/// verbatim) and per-frame segment lengths — the length array is
/// emitted exactly when the graph Length channels move a segment
/// away from the figure def at that frame (see
/// [`export_frame_lengths`], shared with the v5 writer). The
/// project fps survives millisecond-quantised in the trailer
/// delay, as in v3.
///
/// Still dropped, because the grammar or the shared figure
/// stream has no slot for them: the project background (only
/// the default background slot is written), camera keys, joins
/// (flag always 0), transparency (written 0), bends, and
/// per-node colours/styles/media (image/text segments export as
/// plain segments, as in v3).
pub fn export_piv_v4(project: &Project) -> Vec<u8> {
    let mut out = Vec::new();
    out.push(4u8); // version
    out.extend_from_slice(&(project.stage[0] as u32).to_le_bytes());
    out.extend_from_slice(&(project.stage[1] as u32).to_le_bytes());
    out.push(1u8); // background slots: default only
    out.extend_from_slice(&((project.figures.len() + 2) as u16).to_le_bytes()); // figure slots
    for fig in &project.figures {
        write_figure(&mut out, &fig.def, &fig.name, &rest_absolutes(&fig.def));
    }
    out.extend_from_slice(&(project.frames.len() as u32).to_le_bytes());
    for (frame_idx, frame) in project.frames.iter().enumerate() {
        out.extend_from_slice(&(frame.duration.round().max(1.0) as u16).to_le_bytes());
        out.push(0u8); // background id
        out.extend_from_slice(&(project.figures.len() as u16).to_le_bytes()); // object count
        for (fi, fig) in project.figures.iter().enumerate() {
            out.extend_from_slice(&((fi + 1) as u32).to_le_bytes()); // unique id
            out.extend_from_slice(&((fi + 2) as u16).to_le_bytes()); // figure type id
            out.extend_from_slice(&1.0f32.to_le_bytes()); // scale
            out.extend_from_slice(&rgb_to_colorref(fig.color).to_le_bytes());
            out.push(0u8); // transparency
            let abs = pose_absolutes(&fig.def, &frame.poses[fi]);
            for i in 1..fig.def.nodes.len() {
                out.extend_from_slice(&deg_to_rad(abs[i]).to_le_bytes());
            }
            out.extend_from_slice(&frame.poses[fi].root[0].to_le_bytes());
            out.extend_from_slice(&frame.poses[fi].root[1].to_le_bytes());
            match export_frame_lengths(project, frame_idx, fi) {
                Some(lengths) => {
                    out.push(1u8); // len flag: per-frame lengths follow
                    for len in lengths {
                        out.extend_from_slice(&len.to_le_bytes());
                    }
                }
                None => out.push(0u8),
            }
            out.push(0u8); // join flag: none
        }
        for fi in 0..project.figures.len() {
            out.extend_from_slice(&(fi as u16).to_le_bytes()); // draw order: identity
        }
    }
    let interval = (1000.0 / project.fps.max(1.0)).round() as u32;
    out.extend_from_slice(&interval.max(1).to_le_bytes());
    out.push(1u8); // optional trailing byte (loop), as in v3
    out
}

/// Per-frame segment lengths for one exported object, or `None`
/// when the frame's lengths are exactly the figure def's.
///
/// The only source of per-frame lengths in this model is the graph
/// editor's Length channels: `Pose` carries no lengths at all, and
/// the importers discard the format's own per-frame length arrays
/// as unrepresentable (see `import_v5`). Channels are sampled at
/// the frame's integer index and clamped to >= 1.0, mirroring
/// `graph::apply_channels`. When any sampled length differs from
/// the def, the full segment-length vector is returned (the format
/// stores all-or-nothing per object).
/// The per-frame bend array (f64 per segment) for one object,
/// or `None` when the frame's bends equal the figure-record
/// (frame-0) bends — the frame then stores `bend_flag` clear and
/// inherits the record values on read, exactly like the original
/// writer's omitted-array case. Values come from `Frame.bends`
/// (per node; segment k is node k+1).
fn export_frame_bends(
    project: &Project,
    frame: usize,
    fi: usize,
    record: &[f32],
) -> Option<Vec<f64>> {
    let fig = &project.figures[fi];
    let seg_count = fig.def.nodes.len().saturating_sub(1);
    if seg_count == 0 {
        return None;
    }
    let fb = project.frames.get(frame).and_then(|f| f.bends.get(fi));
    let mut any = false;
    let mut arr = Vec::with_capacity(seg_count);
    for k in 0..seg_count {
        let v = fb.and_then(|b| b.get(k + 1)).copied().unwrap_or(0.0);
        if v != record.get(k + 1).copied().unwrap_or(0.0) {
            any = true;
        }
        arr.push(v as f64);
    }
    if any { Some(arr) } else { None }
}

fn export_frame_lengths(project: &Project, frame: usize, fi: usize) -> Option<Vec<f32>> {
    let fig = &project.figures[fi];
    let seg_count = fig.def.nodes.len().saturating_sub(1);
    if seg_count == 0 {
        return None;
    }
    let mut lengths: Vec<f32> = fig.def.nodes[1..].iter().map(|n| n.length).collect();
    let mut any_channel = false;
    for ch in &project.channels {
        if ch.figure != fi {
            continue;
        }
        let crate::graph::ChannelTarget::Length(n) = ch.target else {
            continue;
        };
        // Node 0 is the origin (no segment) and out-of-range
        // indices are stale editor state; both are ignored, as in
        // apply_channels.
        if n == 0 || n > seg_count {
            continue;
        }
        any_channel = true;
        lengths[n - 1] = ch.value_at(frame as f32).max(1.0);
    }
    if !any_channel {
        return None;
    }
    let differs = lengths
        .iter()
        .zip(fig.def.nodes[1..].iter())
        .any(|(a, b)| (a - b.length).abs() > 1e-4);
    differs.then_some(lengths)
}

/// Width/height of a PNG asset from its IHDR chunk (the asset
/// table stores PNG bytes — see `model::ImageAsset`). `None` for
/// non-PNG or truncated data.
fn png_dimensions(png: &[u8]) -> Option<(u32, u32)> {
    if png.len() < 24 || &png[0..8] != b"\x89PNG\r\n\x1a\n" {
        return None;
    }
    let w = u32::from_be_bytes(png[16..20].try_into().ok()?);
    let h = u32::from_be_bytes(png[20..24].try_into().ok()?);
    Some((w, h))
}

/// The v5 image-table payload for one asset: PNG bytes plus the
/// real pixel size the anchors are computed from (Wave X, P-4r).
/// The table is a PNG container, but assets store their original
/// bytes (`model::ImageAsset` — media.rs decodes by content), so
/// a JPEG asset must not be embedded raw. A PNG blob with a sane
/// IHDR passes through verbatim with its IHDR size; any other
/// decodable blob is transcoded to PNG and reports its decoded
/// size. Bytes that do not decode at all (including a PNG header
/// with a zero dimension) refuse the export with a named error
/// instead of landing in the file behind unit anchors.
fn piv_image_payload(
    asset: usize,
    blob: &[u8],
    name: &str,
) -> Result<(Vec<u8>, u32, u32), String> {
    if let Some((w, h)) = png_dimensions(blob) {
        if w > 0 && h > 0 {
            return Ok((blob.to_vec(), w, h));
        }
    }
    let img = image::load_from_memory(blob).map_err(|e| {
        format!(
            "Pivot v5 export refused: image asset {asset} ('{name}') \
             is not a decodable image (PNG, JPEG or GIF): {e}"
        )
    })?;
    let (w, h) = (img.width(), img.height());
    let mut png = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .map_err(|e| {
            format!(
                "Pivot v5 export refused: image asset {asset} ('{name}') \
                 could not be transcoded to PNG: {e}"
            )
        })?;
    Ok((png, w, h))
}

/// Export the project as a version 5 `.piv` file, zlib-compressed
/// (the form StykAnimate itself writes). Writer grammar:
/// `codecs/piv_full_writer.py` (tables) and `codecs/piv_document.py`
/// (keyframes). The choices are the simplest legal ones the model
/// can express: one solid-colour background slot (kind 2) that
/// every frame references; figure flags 144 (bit 16 = z-order is
/// the identity permutation, so the arrays are omitted; bit 128 =
/// no segment has a secondary colour, which the model does not
/// have); no polyfills; objects keyed by stable unique ids; a
/// camera record only for frames whose key differs from the
/// default (stage centre, zoom 1); and the Auto-Easing trailer
/// byte 0.
///
/// Beyond the wave-3 subset, this writer now emits:
///
/// * **Per-frame lengths** (`len_flag` + f32 array) for objects
///   whose graph Length channels move a segment length away from
///   the figure def at that frame — see [`export_frame_lengths`].
/// * **Bends** (wave 7): the figure-record bend block carries
///   frame 0's per-segment bends — in the format the record bend
///   is the rest value a frame falls back to when it has no bend
///   array, and frame 0 is this port's rest frame. A frame whose
///   bends differ from frame 0's gets `bend_flag` + the full f64
///   array; other frames store the flag clear and inherit the
///   record values, so the round trip is exact under the
///   original's evaluation rule (`bend_flag ? bends[i] :
///   segment.bend_rad`). See [`export_frame_bends`].
/// * **Image side tables** for figures containing image segments
///   (`SegmentMedia::Image`): one table entry per distinct
///   referenced asset, holding the asset's PNG bytes verbatim
///   behind an absolute end offset. Assets are stored as their
///   original bytes, so a non-PNG asset (a JPEG, say) is first
///   transcoded to PNG and its anchors come from the decoded
///   size; an asset that does not decode at all refuses the
///   export with a named error (Wave X, P-4r — this is why the
///   export returns a `Result`). The anchor segment is the
///   image's horizontal centreline in pixel space, which is
///   exactly how `media.rs` renders image segments (the image's
///   width spans the segment, centred on it). Figures with image
///   segments also get the per-object image/text flags byte the
///   grammar requires.
///
/// * **Text side tables** for figures containing text segments
///   (`SegmentMedia::Text`): one table entry per distinct content
///   string. A text entry stores vector glyph outlines and
///   per-character placements; the port has no font outlines —
///   it renders text with a 3x5 raster bitmap font (`media.rs`) —
///   so each glyph is encoded as the bitmap's own outline: one
///   closed square path (M/L/L/L/Z) per lit pixel, in a glyph
///   space where one unit is one bitmap pixel, the glyph cell is
///   3 wide x 5 tall with its top-left at the origin (y down),
///   and character `i` is placed at x = 4i (the renderer's 3 + 1
///   advance). That is real data in the format's own encoding —
///   the original's decoder reads it and derives exactly these
///   bounds — and it matches the port's blocky bitmap look.
///   Characters the bitmap font lacks export as its '?' glyph,
///   exactly as they render. Documented limits: the format has
///   no size field (Pivot derives text geometry from the segment
///   and placements), so segments sharing a content string share
///   one entry even if their model sizes differ; a content with
///   more than 255 distinct UTF-16 code units (the format's u8
///   glyph count) exports as a plain segment instead; glyphs are
///   keyed by UTF-16 code unit, so an astral character gets a
///   '?' glyph per surrogate half where the renderer draws one.
pub fn export_piv_v5(project: &Project) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    out.push(5u8); // version
    out.extend_from_slice(&(project.stage[0] as u32).to_le_bytes());
    out.extend_from_slice(&(project.stage[1] as u32).to_le_bytes());
    // Backgrounds: slot 0 is implicit; slot 1 is a solid colour.
    out.extend_from_slice(&2u16.to_le_bytes()); // background slots
    out.push(2u8); // kind 2: solid colour
    out.extend_from_slice(&rgb_to_colorref(project.background).to_le_bytes());
    out.push(10u8); // name length
    out.extend_from_slice(b"Background");
    // Figure table (slot 1 is the implicit built-in figure).
    out.extend_from_slice(&((project.figures.len() + 2) as u16).to_le_bytes());
    // Figure-record bend blocks carry frame 0's bends (wave 7).
    let record_bends: Vec<Vec<f32>> = project
        .figures
        .iter()
        .enumerate()
        .map(|(i, fig)| {
            let mut v = vec![0.0f32; fig.def.nodes.len()];
            if let Some(b) = project.frames.first().and_then(|f| f.bends.get(i)) {
                for (k, val) in b.iter().enumerate().take(v.len()) {
                    v[k] = *val;
                }
            }
            v
        })
        .collect();
    let mut fig_has_media: Vec<bool> = Vec::with_capacity(project.figures.len());
    for (i, fig) in project.figures.iter().enumerate() {
        fig_has_media.push(write_figure_v5(&mut out, fig, &project.assets, &record_bends[i])?);
    }
    // Keyframes.
    out.extend_from_slice(&(project.frames.len() as u32).to_le_bytes());
    let centre = [project.stage[0] / 2.0, project.stage[1] / 2.0];
    for (fi, frame) in project.frames.iter().enumerate() {
        out.extend_from_slice(&(frame.duration.round().max(1.0) as u16).to_le_bytes());
        out.push(0u8); // background mix
        out.extend_from_slice(&1u16.to_le_bytes()); // background id
        out.extend_from_slice(&(project.figures.len() as u16).to_le_bytes());
        for (i, fig) in project.figures.iter().enumerate() {
            out.extend_from_slice(&((i + 1) as u32).to_le_bytes()); // unique id
            out.extend_from_slice(&((i + 2) as u16).to_le_bytes()); // figure type
            out.extend_from_slice(&1.0f32.to_le_bytes()); // scale
            out.extend_from_slice(&rgb_to_colorref(fig.color).to_le_bytes());
            out.push(0u8); // field28 (transparency)
            if fig_has_media[i] {
                // Image/text flags byte: present iff the figure
                // record carries media segments. 0 = no endpoint
                // reversal / reflection for any segment this frame.
                out.push(0u8);
            }
            let abs = pose_absolutes(&fig.def, &frame.poses[i]);
            for k in 1..fig.def.nodes.len() {
                out.extend_from_slice(&deg_to_rad(abs[k]).to_le_bytes());
            }
            out.extend_from_slice(&frame.poses[i].root[0].to_le_bytes());
            out.extend_from_slice(&frame.poses[i].root[1].to_le_bytes());
            match export_frame_lengths(project, fi, i) {
                Some(lengths) => {
                    out.extend_from_slice(&1u16.to_le_bytes()); // len flag
                    for l in lengths {
                        out.extend_from_slice(&l.to_le_bytes());
                    }
                }
                None => out.extend_from_slice(&0u16.to_le_bytes()), // len flag
            }
            match export_frame_bends(project, fi, i, &record_bends[i]) {
                Some(arr) => {
                    out.extend_from_slice(&1u16.to_le_bytes()); // bend flag
                    for v in arr {
                        out.extend_from_slice(&v.to_le_bytes());
                    }
                }
                None => out.extend_from_slice(&0u16.to_le_bytes()), // bend flag
            }
            out.push(0u8); // join flag
        }
        for i in 0..project.figures.len() {
            out.extend_from_slice(&(i as u16).to_le_bytes()); // draw order: identity
        }
        // Camera: written in full only when a key on this frame
        // differs from the default — the writer's `is_default` rule.
        let key = project.camera.iter().find(|k| k.frame == fi);
        match key {
            Some(key)
                if (key.zoom - 1.0).abs() > 0.0001
                    || (key.x - centre[0]).abs() > 0.01
                    || (key.y - centre[1]).abs() > 0.01
                    || key.angle.abs() > 0.01 =>
            {
                out.push(0u8); // not the default camera
                out.extend_from_slice(&key.x.to_le_bytes());
                out.extend_from_slice(&key.y.to_le_bytes());
                out.extend_from_slice(&(key.angle.to_radians() as f64).to_le_bytes());
                out.extend_from_slice(&key.zoom.to_le_bytes());
            }
            _ => out.push(1u8), // default camera
        }
    }
    out.extend_from_slice(&(project.fps.round().max(1.0) as u32).to_le_bytes());
    out.push(0u8); // trailer: Auto-Easing option byte (0 = enabled)
    let mut encoder =
        flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    encoder
        .write_all(&out)
        .expect("zlib encode into memory cannot fail");
    Ok(encoder.finish().expect("zlib finish into memory cannot fail"))
}

/// Font name written into exported text entries. The format
/// treats the name as an opaque label (rendering derives geometry
/// from the outlines and placements, `piv_side_tables.py`), and
/// this label says honestly what the outlines are: the port's
/// 3x5 bitmap font, drawn as squares.
const TEXT_FONT_NAME: &str = "Bitmap 3x5";

/// The 3x5 bitmap font, mirrored from `media.rs`'s `glyph_rows`
/// (private to that module): five rows top to bottom, 3 bits per
/// row, bit 2 the leftmost pixel. KEEP IN SYNC with `media.rs` —
/// the exporter's glyph outlines must match what the renderer
/// draws. Same fallback rules: ASCII case folds up, control
/// characters are blank, anything else unknown is '?'.
fn bitmap_glyph_rows(ch: char) -> [u8; 5] {
    let up = ch.to_ascii_uppercase();
    match up {
        'A' => [0b010, 0b101, 0b111, 0b101, 0b101],
        'B' => [0b110, 0b101, 0b110, 0b101, 0b110],
        'C' => [0b011, 0b100, 0b100, 0b100, 0b011],
        'D' => [0b110, 0b101, 0b101, 0b101, 0b110],
        'E' => [0b111, 0b100, 0b110, 0b100, 0b111],
        'F' => [0b111, 0b100, 0b110, 0b100, 0b100],
        'G' => [0b011, 0b100, 0b101, 0b101, 0b011],
        'H' => [0b101, 0b101, 0b111, 0b101, 0b101],
        'I' => [0b111, 0b010, 0b010, 0b010, 0b111],
        'J' => [0b001, 0b001, 0b001, 0b101, 0b010],
        'K' => [0b101, 0b101, 0b110, 0b101, 0b101],
        'L' => [0b100, 0b100, 0b100, 0b100, 0b111],
        'M' => [0b101, 0b111, 0b111, 0b101, 0b101],
        'N' => [0b101, 0b111, 0b101, 0b101, 0b101],
        'O' => [0b010, 0b101, 0b101, 0b101, 0b010],
        'P' => [0b110, 0b101, 0b110, 0b100, 0b100],
        'Q' => [0b010, 0b101, 0b101, 0b010, 0b001],
        'R' => [0b110, 0b101, 0b110, 0b101, 0b101],
        'S' => [0b011, 0b100, 0b010, 0b001, 0b110],
        'T' => [0b111, 0b010, 0b010, 0b010, 0b010],
        'U' => [0b101, 0b101, 0b101, 0b101, 0b111],
        'V' => [0b101, 0b101, 0b101, 0b101, 0b010],
        'W' => [0b101, 0b101, 0b101, 0b111, 0b101],
        'X' => [0b101, 0b101, 0b010, 0b101, 0b101],
        'Y' => [0b101, 0b101, 0b010, 0b010, 0b010],
        'Z' => [0b111, 0b001, 0b010, 0b100, 0b111],
        '0' => [0b111, 0b101, 0b101, 0b101, 0b111],
        '1' => [0b010, 0b110, 0b010, 0b010, 0b111],
        '2' => [0b110, 0b001, 0b010, 0b100, 0b111],
        '3' => [0b110, 0b001, 0b010, 0b001, 0b110],
        '4' => [0b101, 0b101, 0b111, 0b001, 0b001],
        '5' => [0b111, 0b100, 0b110, 0b001, 0b110],
        '6' => [0b011, 0b100, 0b110, 0b101, 0b010],
        '7' => [0b111, 0b001, 0b001, 0b010, 0b010],
        '8' => [0b010, 0b101, 0b010, 0b101, 0b010],
        '9' => [0b010, 0b101, 0b011, 0b001, 0b110],
        ' ' => [0, 0, 0, 0, 0],
        '.' => [0, 0, 0, 0, 0b010],
        ',' => [0, 0, 0, 0b010, 0b100],
        '!' => [0b010, 0b010, 0b010, 0, 0b010],
        '?' => [0b110, 0b001, 0b010, 0, 0b010],
        '\'' => [0b010, 0b010, 0, 0, 0],
        '"' => [0b101, 0b101, 0, 0, 0],
        '-' => [0, 0, 0b111, 0, 0],
        ':' => [0, 0b010, 0, 0b010, 0],
        ';' => [0, 0b010, 0, 0b010, 0b100],
        '(' => [0b001, 0b010, 0b010, 0b010, 0b001],
        ')' => [0b100, 0b010, 0b010, 0b010, 0b100],
        '+' => [0, 0b010, 0b111, 0b010, 0],
        '/' => [0b001, 0b001, 0b010, 0b100, 0b100],
        '&' => [0b010, 0b101, 0b010, 0b101, 0b011],
        '%' => [0b101, 0b001, 0b010, 0b100, 0b101],
        '*' => [0, 0b101, 0b010, 0b101, 0],
        '=' => [0, 0b111, 0, 0b111, 0],
        '<' => [0b001, 0b010, 0b100, 0b010, 0b001],
        '>' => [0b100, 0b010, 0b001, 0b010, 0b100],
        '_' => [0, 0, 0, 0, 0b111],
        '#' => [0b101, 0b111, 0b101, 0b111, 0b101],
        '$' => [0b011, 0b110, 0b010, 0b011, 0b110],
        '@' => [0b010, 0b101, 0b111, 0b100, 0b011],
        _ => {
            if ch.is_control() {
                [0, 0, 0, 0, 0]
            } else {
                bitmap_glyph_rows('?')
            }
        }
    }
}

/// Rows for one UTF-16 code unit of stored content. A unit that
/// is not a Unicode scalar value (a surrogate half) has no
/// renderer glyph; it exports as '?' like any unknown character.
fn bitmap_glyph_rows_for_unit(unit: u16) -> [u8; 5] {
    match char::from_u32(unit as u32) {
        Some(c) => bitmap_glyph_rows(c),
        None => bitmap_glyph_rows('?'),
    }
}

/// Distinct UTF-16 code units of `content`, first-appearance
/// order — the order that owns the glyph table.
fn text_unique_units(content: &str) -> Vec<u16> {
    let mut uniques: Vec<u16> = Vec::new();
    for u in content.encode_utf16() {
        if !uniques.contains(&u) {
            uniques.push(u);
        }
    }
    uniques
}

/// Write a string in the side-table UTF-16 form: u32 code-unit
/// count + little-endian bytes.
fn write_utf16_string(out: &mut Vec<u8>, s: &str) {
    let units: Vec<u16> = s.encode_utf16().collect();
    out.extend_from_slice(&(units.len() as u32).to_le_bytes());
    for u in units {
        out.extend_from_slice(&u.to_le_bytes());
    }
}

/// Write one v5 text side-table entry. Grammar per the original's
/// `piv_side_tables.py`: font name (UTF-16), font style u8,
/// alignment u32, content (UTF-16), glyph count u8, then per
/// glyph a u32 node count, the opcode stream packed two 4-bit
/// codes per byte (low nibble first) and ALL f32 coordinates as
/// one flat block; finally one (x, y) f32 placement per content
/// unit whose glyph is visible (node count > 0).
///
/// Glyph space (see `export_piv_v5`'s docs): one unit = one
/// bitmap pixel; pixel (col, row) is the square
/// [col, col+1] x [row, row+1] of the 3x5 cell with its top-left
/// at the origin, y down — exactly `media.rs`'s cell layout —
/// emitted as M/L/L/L/Z, so a glyph's node count is 5 x its lit
/// pixels and a blank glyph (space, control characters) has node
/// count 0. Placements put unit `i` at (4i, 0), the renderer's
/// advance. Callers guarantee at most 255 distinct units.
fn write_text_entry(out: &mut Vec<u8>, content: &str) {
    write_utf16_string(out, TEXT_FONT_NAME);
    out.push(0); // font style: regular
    out.extend_from_slice(&0u32.to_le_bytes()); // alignment
    write_utf16_string(out, content);
    let units: Vec<u16> = content.encode_utf16().collect();
    let uniques = text_unique_units(content);
    debug_assert!(uniques.len() <= 255);
    out.push(uniques.len() as u8); // glyph count
    let mut node_counts: Vec<u32> = Vec::with_capacity(uniques.len());
    for &unit in &uniques {
        let rows = bitmap_glyph_rows_for_unit(unit);
        let mut codes: Vec<u8> = Vec::new();
        let mut values: Vec<f32> = Vec::new();
        for (row, &bits) in rows.iter().enumerate() {
            for col in 0..3usize {
                if (bits >> (2 - col)) & 1 == 1 {
                    let (x, y) = (col as f32, row as f32);
                    codes.extend_from_slice(&[0, 1, 1, 1, 3]); // M L L L Z
                    values.extend_from_slice(&[
                        x,
                        y,
                        x + 1.0,
                        y,
                        x + 1.0,
                        y + 1.0,
                        x,
                        y + 1.0,
                    ]);
                }
            }
        }
        let node_count = codes.len() as u32;
        node_counts.push(node_count);
        out.extend_from_slice(&node_count.to_le_bytes());
        if node_count > 0 {
            // Opcodes packed two per byte, low nibble first; an
            // odd tail pads the high nibble with 0 — an M code
            // the reader's expansion never reaches, because it
            // stops exactly at node_count.
            for pair in codes.chunks(2) {
                let hi = if pair.len() > 1 { pair[1] } else { 0 };
                out.push(pair[0] | (hi << 4));
            }
            for v in values {
                out.extend_from_slice(&v.to_le_bytes());
            }
        }
    }
    // Placements: visible glyphs only, per content unit in order.
    for (i, unit) in units.iter().enumerate() {
        let gi = uniques.iter().position(|u| u == unit).unwrap();
        if node_counts[gi] > 0 {
            out.extend_from_slice(&(4.0 * i as f32).to_le_bytes());
            out.extend_from_slice(&0.0f32.to_le_bytes());
        }
    }
}

/// Write one v5 figure record: flags 144 (implicit z-order, no
/// secondary colours), then segments in node order — a segment's
/// id is its node index, so the parent id is the parent node index
/// (the origin node is 0). The bend block carries `rest_bends`
/// (per node; frame 0's bends, chosen by the caller) as sparse
/// non-zero entries keyed by 0-based segment ordinal. Polyfills
/// are empty (not modelled).
///
/// Image segments (`SegmentMedia::Image` with a valid asset
/// index) are written as primitive 2 with the extra triple
/// (image-table index, 0, 0), and the figure's image side table
/// follows the bend count, per the grammar: a u16 entry count,
/// then per entry four i32 anchors, a u64 **absolute** end offset
/// (into the whole payload), and the entry's PNG payload (the
/// asset's own bytes if PNG, a transcode otherwise — see
/// [`piv_image_payload`], which can refuse the export).
/// Text segments are written as primitive 5 with the extra triple
/// (text-table index, 0, 0), and the figure's text side table
/// follows the image table: a u16 entry count, then one entry per
/// distinct content string (see [`write_text_entry`]). Returns
/// whether the record contains any media segment (the caller
/// then owes each keyframe object an image/text flags byte).
fn write_figure_v5(
    out: &mut Vec<u8>,
    fig: &FigureInstance,
    assets: &[ImageAsset],
    rest_bends: &[f32],
) -> Result<bool, String> {
    let def = &fig.def;
    let name = &fig.name;
    // The figure's image table: distinct referenced assets in
    // first-reference order, plus each segment's index into it.
    // Segments whose asset index dangles fall back to plain
    // segments (there is no blob to write for them).
    let mut table: Vec<usize> = Vec::new();
    let mut seg_table_idx: Vec<Option<usize>> = vec![None; def.nodes.len()];
    for (i, node) in def.nodes.iter().enumerate().skip(1) {
        if let Some(SegmentMedia::Image { asset }) = node.media {
            if asset < assets.len() {
                seg_table_idx[i] = Some(match table.iter().position(|&a| a == asset) {
                    Some(p) => p,
                    None => {
                        table.push(asset);
                        table.len() - 1
                    }
                });
            }
        }
    }
    // The table payloads, prepared before anything for this
    // figure is written: PNG bytes + real pixel size per entry,
    // transcoding non-PNG assets (see `piv_image_payload`). An
    // undecodable asset refuses the whole export here.
    let mut payloads: Vec<(Vec<u8>, u32, u32)> = Vec::with_capacity(table.len());
    for &asset in &table {
        payloads.push(piv_image_payload(asset, &assets[asset].png, &assets[asset].name)?);
    }
    // The figure's text table: distinct content strings in
    // first-reference order (the format stores no size, so
    // segments sharing a string share an entry). A content with
    // more than 255 distinct UTF-16 code units cannot be an
    // entry (u8 glyph count); such a segment falls back to a
    // plain segment, like a dangling image reference.
    let mut text_table: Vec<String> = Vec::new();
    let mut seg_text_idx: Vec<Option<usize>> = vec![None; def.nodes.len()];
    for (i, node) in def.nodes.iter().enumerate().skip(1) {
        if let Some(SegmentMedia::Text { content, .. }) = &node.media {
            if text_unique_units(content).len() <= 255 {
                seg_text_idx[i] = Some(match text_table.iter().position(|c| c == content) {
                    Some(p) => p,
                    None => {
                        text_table.push(content.clone());
                        text_table.len() - 1
                    }
                });
            }
        }
    }
    let rest_abs = rest_absolutes(def);
    out.push(128 | 16); // flags
    out.extend_from_slice(&((def.nodes.len() - 1) as u16).to_le_bytes());
    for (i, node) in def.nodes.iter().enumerate().skip(1) {
        out.extend_from_slice(&(node.parent.unwrap_or(0) as u16).to_le_bytes());
        out.extend_from_slice(&(i as u16).to_le_bytes()); // segment id
        out.extend_from_slice(&node.length.to_le_bytes());
        out.extend_from_slice(&deg_to_rad(rest_abs[i]).to_le_bytes());
        out.extend_from_slice(&node.thickness.to_le_bytes());
        out.push(if seg_table_idx[i].is_some() {
            2 // image primitive; the media replaces the shape
        } else if seg_text_idx[i].is_some() {
            5 // text primitive; the media replaces the shape
        } else {
            match node.shape {
                Shape::Circle => 1,
                Shape::Polygon(4) => 6,
                _ => 0,
            }
        });
        out.push(0); // aux
        let rgb = node.color.unwrap_or(0);
        out.push((rgb >> 16) as u8);
        out.push((rgb >> 8) as u8);
        out.push(rgb as u8);
        out.push(0); // transparency
        // A node carries at most one media, so at most one of the
        // two table indices is set.
        if let Some(ti) = seg_table_idx[i].or(seg_text_idx[i]) {
            out.extend_from_slice(&(ti as u16).to_le_bytes()); // extra: table index
            out.push(0); // extra: endpoint reversal
            out.push(0); // extra: baseline reflection
        }
    }
    // Bend block: non-zero rest bends as (segment ordinal,
    // radians) — the ordinal is the segment's record position,
    // not its id (the original's decoder indexes `segments[i]`).
    let bend_entries: Vec<(u16, f64)> = (1..def.nodes.len())
        .filter_map(|node| {
            let b = rest_bends.get(node).copied().unwrap_or(0.0);
            if b != 0.0 {
                Some(((node - 1) as u16, b as f64))
            } else {
                None
            }
        })
        .collect();
    out.extend_from_slice(&(bend_entries.len() as u16).to_le_bytes());
    for (idx, v) in bend_entries {
        out.extend_from_slice(&idx.to_le_bytes());
        out.extend_from_slice(&v.to_le_bytes());
    }
    if !payloads.is_empty() {
        out.extend_from_slice(&(payloads.len() as u16).to_le_bytes());
        for (blob, w, h) in &payloads {
            // Anchors: the image's horizontal centreline in pixel
            // space (media.rs maps the image's width onto the
            // segment, centred). The size is the payload's real
            // pixel size — for a transcoded asset, its decoded
            // size — never a unit fallback.
            let (ax, ay, bx, by) = (0i32, (h / 2) as i32, *w as i32, (h / 2) as i32);
            // The end offset is absolute (whole payload) and
            // points just past the blob: anchors (16) + this u64
            // field (8) + the blob itself.
            let end = (out.len() + 16 + 8 + blob.len()) as u64;
            for v in [ax, ay, bx, by] {
                out.extend_from_slice(&v.to_le_bytes());
            }
            out.extend_from_slice(&end.to_le_bytes());
            out.extend_from_slice(blob);
        }
    }
    if !text_table.is_empty() {
        out.extend_from_slice(&(text_table.len() as u16).to_le_bytes());
        for content in &text_table {
            write_text_entry(out, content);
        }
    }
    out.extend_from_slice(&0u16.to_le_bytes()); // polyfill count
    // No z-order arrays (flag 16) and no global style (flag 64 clear).
    let name_bytes = name.as_bytes();
    out.push(name_bytes.len().min(255) as u8);
    out.extend_from_slice(&name_bytes[..name_bytes.len().min(255)]);
    Ok(!table.is_empty() || !text_table.is_empty())
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
    fn piv_v5_roundtrip_preserves_frames_and_poses() {
        // Two figures (the second added at a different root) and a
        // non-default camera key, so the v5 writer's object and
        // camera paths are both exercised.
        let mut project = Project::new_demo();
        project.add_figure_with(FigureDef::quadruped(), "Quad", 0x5DB2FF, [300.0, 400.0]);
        project.camera.push(CameraKey {
            frame: 1,
            x: 500.0,
            y: 300.0,
            zoom: 1.5,
            angle: 0.0,
        });
        let bytes = export_piv_v5(&project).unwrap();
        // The export is zlib-wrapped, like the files StykAnimate
        // writes: the first payload byte (version 5) must not be
        // visible in the raw stream.
        assert_ne!(bytes[0], 5, "v5 export should be zlib-compressed");
        let back = import_piv(&bytes).unwrap();
        assert_eq!(back.stage, project.stage);
        assert_eq!(back.background, project.background);
        assert!((back.fps - project.fps).abs() < 1.0, "fps {} vs {}", back.fps, project.fps);
        assert_eq!(back.figures.len(), 2);
        assert_eq!(back.figures[0].name, "Styk");
        assert_eq!(back.figures[1].name, "Quad");
        for (a, b) in project.figures.iter().zip(back.figures.iter()) {
            assert_eq!(a.def.nodes.len(), b.def.nodes.len());
            assert_eq!(a.color, b.color);
        }
        assert_eq!(back.frames.len(), project.frames.len());
        for (a, b) in project.frames.iter().zip(back.frames.iter()) {
            assert!((a.duration - b.duration).abs() < 0.01);
            for (pa, pb) in a.poses.iter().zip(b.poses.iter()) {
                assert!((pa.root[0] - pb.root[0]).abs() < 0.5, "root x {} vs {}", pa.root[0], pb.root[0]);
                assert!((pa.root[1] - pb.root[1]).abs() < 0.5, "root y {} vs {}", pa.root[1], pb.root[1]);
                assert_eq!(pa.angles.len(), pb.angles.len());
                for (x, y) in pa.angles.iter().zip(pb.angles.iter()) {
                    assert!(
                        (crate::model::norm_deg(x - y)).abs() < 0.1,
                        "angle {x} vs {y}"
                    );
                }
            }
        }
        // The camera key survives; frames without a key stay default.
        assert_eq!(back.camera.len(), 1);
        let key = &back.camera[0];
        assert_eq!(key.frame, 1);
        assert!((key.x - 500.0).abs() < 0.01 && (key.y - 300.0).abs() < 0.01);
        assert!((key.zoom - 1.5).abs() < 0.001);
    }



    // ---- Wave 6: deepened v5 export (lengths, image tables) ----

    fn test_png(w: u32, h: u32) -> Vec<u8> {
        let img = image::RgbaImage::from_pixel(w, h, image::Rgba([0, 200, 0, 255]));
        let mut buf = Vec::new();
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
            .unwrap();
        buf
    }

    fn test_jpeg(w: u32, h: u32) -> Vec<u8> {
        let img = image::RgbImage::from_pixel(w, h, image::Rgb([200, 30, 0]));
        let mut buf = Vec::new();
        image::DynamicImage::ImageRgb8(img)
            .write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Jpeg)
            .unwrap();
        buf
    }

    /// Demo project + a 4x2 PNG asset on node 3 (image), a text
    /// label on node 5, and a Length channel driving node 1 from
    /// its def length 46 to 90 across frames 0..4 (flat tangents,
    /// so frame values are the smoothstep blends 46 / 52.875 / 68 /
    /// 83.125 / 90).
    fn media_project() -> Project {
        let mut project = Project::new_demo();
        project.assets.push(ImageAsset {
            name: "dot".into(),
            png: test_png(4, 2),
        });
        project.figures[0].def.nodes[3].media =
            Some(SegmentMedia::Image { asset: 0 });
        project.figures[0].def.nodes[5].media = Some(SegmentMedia::Text {
            content: "HI".into(),
            size: 25.0,
        });
        project.channels.push(crate::graph::Channel {
            figure: 0,
            target: crate::graph::ChannelTarget::Length(1),
            keys: vec![
                crate::graph::Key::flat(0.0, 46.0),
                crate::graph::Key::flat(4.0, 90.0),
            ],
        });
        project
    }

    fn inflate_export(bytes: &[u8]) -> Vec<u8> {
        use std::io::Read as _;
        let mut out = Vec::new();
        flate2::read::ZlibDecoder::new(bytes)
            .read_to_end(&mut out)
            .unwrap();
        out
    }

    struct WalkedObject {
        len_flag: u16,
        lengths: Vec<f32>,
        bend_flag: u16,
        bends: Vec<f64>,
    }

    /// Walk an exported v5 payload with the module's own reader,
    /// collecting the per-object len/bend data. Asserts the walk
    /// lands exactly at the end of the payload (fps + 1 trailer
    /// byte), which proves every variable-length field — angles,
    /// media flags byte, lengths arrays, side tables (skipped by
    /// `parse_figure_v5`) — is sized exactly as written.
    fn walk_v5_export(payload: &[u8]) -> Vec<Vec<WalkedObject>> {
        let mut r = Reader::new(payload);
        assert_eq!(r.u8().unwrap(), 5);
        r.u32().unwrap();
        r.u32().unwrap();
        let bg_slots = r.u16().unwrap();
        for _ in 1..bg_slots {
            assert_eq!(r.u8().unwrap(), 2, "solid background slot");
            r.u32().unwrap();
            r.str8().unwrap();
        }
        let figure_slots = r.u16().unwrap();
        // (segment count, has media) per type id, 1-based slot - 1.
        let (bdef, _) = builtin_man();
        let mut types: Vec<(usize, bool)> = vec![(bdef.nodes.len() - 1, false)];
        for _ in 2..figure_slots {
            let (t, _) = parse_figure_v5(&mut r).unwrap();
            types.push((t.def.nodes.len() - 1, t.has_images));
        }
        let frame_count = r.u32().unwrap() as usize;
        let mut frames = Vec::new();
        for _ in 0..frame_count {
            r.u16().unwrap(); // duration
            r.u8().unwrap(); // background mix
            r.u16().unwrap(); // background id
            let object_count = r.u16().unwrap() as usize;
            let mut objects = Vec::new();
            for _ in 0..object_count {
                r.u32().unwrap(); // unique id
                let tid = r.u16().unwrap() as usize;
                let (seg_count, has_media) = types[tid - 1];
                r.f32().unwrap(); // scale
                r.u32().unwrap(); // colour
                r.u8().unwrap(); // field28
                if has_media {
                    r.u8().unwrap(); // image/text flags byte
                }
                for _ in 0..seg_count {
                    r.f64().unwrap(); // angles
                }
                r.f32().unwrap();
                r.f32().unwrap(); // root
                let len_flag = r.u16().unwrap();
                let lengths = if len_flag != 0 {
                    (0..seg_count).map(|_| r.f32().unwrap()).collect()
                } else {
                    Vec::new()
                };
                let bend_flag = r.u16().unwrap();
                let bends = if bend_flag != 0 {
                    (0..seg_count).map(|_| r.f64().unwrap()).collect()
                } else {
                    Vec::new()
                };
                let join = r.u8().unwrap();
                if join != 0 {
                    r.u16().unwrap();
                    r.u16().unwrap();
                }
                objects.push(WalkedObject {
                    len_flag,
                    lengths,
                    bend_flag,
                    bends,
                });
            }
            for _ in 0..object_count {
                r.u16().unwrap(); // draw order
            }
            let is_default = r.u8().unwrap();
            if is_default == 0 {
                r.f32().unwrap();
                r.f32().unwrap();
                r.f64().unwrap();
                r.f32().unwrap();
            }
            frames.push(objects);
        }
        r.u32().unwrap(); // fps
        r.u8().unwrap(); // trailer
        assert!(r.done(), "walk must consume the payload exactly");
        frames
    }

    /// Walk an exported payload's figure table and return each
    /// stored figure's record (rest) bends, per segment.
    fn walk_v5_record_bends(payload: &[u8]) -> Vec<Vec<f32>> {
        let mut r = Reader::new(payload);
        assert_eq!(r.u8().unwrap(), 5);
        r.u32().unwrap();
        r.u32().unwrap();
        let bg_slots = r.u16().unwrap();
        for _ in 1..bg_slots {
            assert_eq!(r.u8().unwrap(), 2, "solid background slot");
            r.u32().unwrap();
            r.str8().unwrap();
        }
        let figure_slots = r.u16().unwrap();
        let mut out = Vec::new();
        for _ in 2..figure_slots {
            let (_, bends) = parse_figure_v5(&mut r).unwrap();
            out.push(bends);
        }
        out
    }

    #[test]
    fn v5_export_per_frame_lengths_from_graph_channels() {
        let project = media_project();
        let payload = inflate_export(&export_piv_v5(&project).unwrap());
        let frames = walk_v5_export(&payload);
        assert_eq!(frames.len(), 5);
        // Frame 0: the channel value equals the def length (46),
        // so no lengths array is stored.
        assert_eq!(frames[0][0].len_flag, 0, "frame 0 keeps def lengths");
        // Frames 1..4: smoothstep blends of 46 -> 90.
        let want = [52.875f32, 68.0, 83.125, 90.0];
        let def = &project.figures[0].def;
        for (k, w) in want.iter().enumerate() {
            let obj = &frames[k + 1][0];
            assert_eq!(obj.len_flag, 1, "frame {} stores lengths", k + 1);
            assert_eq!(obj.lengths.len(), def.nodes.len() - 1);
            assert!(
                (obj.lengths[0] - w).abs() < 0.001,
                "frame {} node 1 length {} vs {w}",
                k + 1,
                obj.lengths[0]
            );
            // Untouched segments carry their def lengths.
            for (j, node) in def.nodes.iter().enumerate().skip(2) {
                assert!(
                    (obj.lengths[j - 1] - node.length).abs() < 0.001,
                    "frame {} node {j} length",
                    k + 1
                );
            }
        }
        // No bends anywhere (the model holds none).
        assert!(frames.iter().flatten().all(|o| o.bend_flag == 0));
    }

    #[test]
    fn v5_export_image_side_table_and_media_roundtrip() {
        let project = media_project();
        let bytes = export_piv_v5(&project).unwrap();
        let payload = inflate_export(&bytes);
        // The asset's PNG bytes are embedded verbatim.
        let png = &project.assets[0].png;
        assert!(
            payload.windows(png.len()).any(|w| w == png.as_slice()),
            "image blob must appear verbatim in the payload"
        );
        // Round-trip through the port's own importer: structure
        // and poses survive. Image media is not modelled on
        // import (its side table is walked and skipped), but the
        // text label IS recovered from its side table, with the
        // size derived from the segment (see parse_figure_v5).
        let back = import_piv(&bytes).unwrap();
        assert_eq!(back.figures.len(), 1);
        assert_eq!(back.figures[0].name, "Styk");
        assert_eq!(back.figures[0].def.nodes.len(), project.figures[0].def.nodes.len());
        assert_eq!(back.frames.len(), project.frames.len());
        for (a, b) in project.frames.iter().zip(back.frames.iter()) {
            let (pa, pb) = (&a.poses[0], &b.poses[0]);
            assert!((pa.root[0] - pb.root[0]).abs() < 0.5);
            assert!((pa.root[1] - pb.root[1]).abs() < 0.5);
            for (x, y) in pa.angles.iter().zip(pb.angles.iter()) {
                assert!((crate::model::norm_deg(x - y)).abs() < 0.1, "angle {x} vs {y}");
            }
        }
        assert_eq!(
            back.figures[0].def.nodes[3].media, None,
            "image media stays unmodelled on import"
        );
        // "HI" spans 4*2-1 = 7 width units: size = 5 * length / 7.
        let want_size = 5.0 * project.figures[0].def.nodes[5].length / 7.0;
        match &back.figures[0].def.nodes[5].media {
            Some(SegmentMedia::Text { content, size }) => {
                assert_eq!(content, "HI");
                assert!((size - want_size).abs() < 0.01, "size {size} vs {want_size}");
            }
            other => panic!("text label must be recovered, got {other:?}"),
        }
    }

    #[test]
    fn v5_export_transcodes_jpeg_assets_to_png() {
        // Wave X, P-4r: assets store original bytes, so a JPEG
        // asset is a real case (JPEG import, Wave R). The v5
        // image table is a PNG container: the asset must arrive
        // transcoded, with anchors from its real size — never
        // raw behind unit anchors.
        let mut project = media_project();
        project.assets[0].png = test_jpeg(6, 3);
        let jpeg = project.assets[0].png.clone();
        let bytes = export_piv_v5(&project).unwrap();
        let payload = inflate_export(&bytes);
        // The generic walker still lands exactly at payload end:
        // the end offsets are computed over the transcoded bytes.
        walk_v5_export(&payload);
        // The raw JPEG blob appears nowhere in the document.
        assert!(
            !payload.windows(jpeg.len()).any(|w| w == jpeg.as_slice()),
            "raw JPEG must not be embedded in the image table"
        );
        // The table entry: anchors (16 bytes) + u64 end offset
        // + payload. Find the payload's PNG by its magic, then
        // check what precedes it: the anchors must be the real
        // centreline (0, h/2, w, h/2) = (0, 1, 6, 1) for a 6x3
        // image, and the PNG must carry the JPEG's real size.
        let magic: &[u8] = b"\x89PNG\r\n\x1a\n";
        let mut anchor = Vec::new();
        for v in [0i32, 1, 6, 1] {
            anchor.extend_from_slice(&v.to_le_bytes());
        }
        let entry = payload
            .windows(magic.len())
            .positions(|w| w == magic)
            .find(|&pos| {
                pos >= 24
                    && payload[pos - 24..pos - 8] == anchor[..]
                    && png_dimensions(&payload[pos..]) == Some((6, 3))
            });
        assert!(
            entry.is_some(),
            "a PNG entry with real-centreline anchors and the JPEG's 6x3 size must be written"
        );
    }

    #[test]
    fn v5_export_refuses_undecodable_assets() {
        // Wave X, P-4r: bytes that decode as nothing refuse the
        // export with a named error — no file, no unit anchors.
        let mut project = media_project();
        project.assets[0].png = b"not an image at all".to_vec();
        let err = export_piv_v5(&project).unwrap_err();
        assert!(err.contains("refused"), "{err}");
        assert!(err.contains("'dot'"), "the refusal names the asset: {err}");
    }

    #[test]
    fn v5_export_text_side_table_structure() {
        // Byte-level structural assertions on an exported text
        // table, per the documented layout (piv_side_tables.py):
        // entry counts, glyph counts, per-glyph node counts and
        // placement pairs are read straight off the payload.
        let mut project = Project::new_demo();
        project.figures[0].def.nodes[5].media = Some(SegmentMedia::Text {
            content: "A B~".into(),
            size: 25.0,
        });
        let bytes = export_piv_v5(&project).unwrap();
        let payload = inflate_export(&bytes);
        // The generic walker still lands exactly at payload end.
        walk_v5_export(&payload);
        // Walk to the figure record's text table by hand.
        let mut r = Reader::new(&payload);
        assert_eq!(r.u8().unwrap(), 5);
        r.u32().unwrap();
        r.u32().unwrap();
        let bg_slots = r.u16().unwrap();
        for _ in 1..bg_slots {
            r.u8().unwrap(); // kind: solid colour
            r.u32().unwrap(); // colour
            r.str8().unwrap(); // name
        }
        r.u16().unwrap(); // figure slots
        let flags = r.u8().unwrap();
        assert_eq!(flags, 144);
        let count = r.u16().unwrap() as usize;
        let mut prims = Vec::new();
        for _ in 0..count {
            r.u16().unwrap(); // parent
            r.u16().unwrap(); // id
            r.f32().unwrap(); // length
            r.f64().unwrap(); // angle
            r.f32().unwrap(); // thickness
            let prim = r.u8().unwrap();
            prims.push(prim);
            r.u8().unwrap(); // aux
            r.take(3).unwrap(); // rgb
            r.u8().unwrap(); // transparency
            if prim == 2 || prim == 5 {
                r.take(4).unwrap(); // extra triple
            }
        }
        // Node 5 is the 5th segment record; it alone is text.
        assert_eq!(prims[4], 5);
        assert_eq!(prims.iter().filter(|&&p| p == 5).count(), 1);
        let bends = r.u16().unwrap();
        for _ in 0..bends {
            r.u16().unwrap();
            r.f64().unwrap();
        }
        // No image table (no prim-2 segments): the text table
        // follows the bends directly.
        assert_eq!(r.u16().unwrap(), 1, "one text entry");
        let font = skip_utf16_string(&mut r).unwrap();
        assert_eq!(String::from_utf16_lossy(&font), "Bitmap 3x5");
        assert_eq!(r.u8().unwrap(), 0, "font style");
        assert_eq!(r.u32().unwrap(), 0, "alignment");
        let content = skip_utf16_string(&mut r).unwrap();
        assert_eq!(String::from_utf16_lossy(&content), "A B~");
        assert_eq!(r.u8().unwrap(), 4, "glyph count = distinct units");
        // 'A' = 10 lit pixels, ' ' = 0, 'B' = 10, '~' -> '?' = 5;
        // the node count is 5 per lit pixel (M/L/L/L/Z per square).
        let want_nodes = [50u32, 0, 50, 25];
        let mut glyph0_values: Vec<f32> = Vec::new();
        for (gi, &want) in want_nodes.iter().enumerate() {
            let node_count = r.u32().unwrap();
            assert_eq!(node_count, want, "glyph {gi} node count");
            if node_count == 0 {
                continue;
            }
            // Simulate the opcode expansion to size the f32 block
            // (cubic = 3 nodes, everything else 1; M/L = 2 values,
            // C = 6, Z = 0, V/H = 1) — the reader's own rule.
            let mut expanded = 0u32;
            let mut values = 0usize;
            let mut high = false;
            let mut byte = 0u8;
            while expanded < node_count {
                if !high {
                    byte = r.u8().unwrap();
                }
                let op = if high { byte >> 4 } else { byte & 0x0F };
                high = !high;
                assert!(op <= 5, "opcode {op}");
                expanded += if op == 2 { 3 } else { 1 };
                values += match op {
                    0 | 1 => 2,
                    2 => 6,
                    3 => 0,
                    _ => 1,
                };
            }
            assert_eq!(expanded, node_count);
            let mut vals = Vec::new();
            for _ in 0..values {
                vals.push(r.f32().unwrap());
            }
            if gi == 0 {
                glyph0_values = vals;
            }
        }
        // 'A' row 0 is 0b010: its first lit pixel is (col 1,
        // row 0), drawn as the square (1,0) -> (2,1).
        assert_eq!(
            &glyph0_values[..8],
            &[1.0, 0.0, 2.0, 0.0, 2.0, 1.0, 1.0, 1.0]
        );
        // Placements: visible units only — A at x 0, B at x 8,
        // '~' at x 12; the space stores nothing.
        for want_x in [0.0f32, 8.0, 12.0] {
            assert_eq!(r.f32().unwrap(), want_x);
            assert_eq!(r.f32().unwrap(), 0.0);
        }
        assert_eq!(r.u16().unwrap(), 0, "polyfill count follows the table");
        // And the label round-trips through the importer.
        let back = import_piv(&bytes).unwrap();
        match &back.figures[0].def.nodes[5].media {
            Some(SegmentMedia::Text { content, .. }) => assert_eq!(content, "A B~"),
            other => panic!("expected recovered text, got {other:?}"),
        }
    }

    #[test]
    fn v5_text_over_255_unique_units_exports_as_plain_segment() {
        // The glyph count is a u8: a content with more than 255
        // distinct UTF-16 units cannot be a table entry, so the
        // segment falls back to a plain segment (documented) and
        // the file still round-trips structurally.
        let wide: String = (0..300u32)
            .map(|i| char::from_u32(0x100 + i).unwrap())
            .collect();
        let mut project = Project::new_demo();
        project.figures[0].def.nodes[5].media = Some(SegmentMedia::Text {
            content: wide,
            size: 25.0,
        });
        let bytes = export_piv_v5(&project).unwrap();
        let payload = inflate_export(&bytes);
        walk_v5_export(&payload); // exact-EOF walk still holds
        let back = import_piv(&bytes).unwrap();
        assert_eq!(back.figures[0].def.nodes[5].media, None);
    }

    #[test]
    fn v5_text_corpus_fixture_recovers_text() {
        // tests/fixtures/text-v5.piv is produced by the ORIGINAL's
        // Python writer (scripts/verify_text_tables.py): the
        // shared Tripod scene with segment 2 as a text segment
        // carrying "HI" in box-outline glyphs.
        let dir = std::path::Path::new("tests/fixtures");
        let Ok(bytes) = std::fs::read(dir.join("text-v5.piv")) else {
            return; // corpus is generated by scripts/
        };
        let project = import_piv(&bytes).unwrap();
        let fig = project
            .figures
            .iter()
            .find(|f| f.name == "Tripod")
            .expect("Tripod figure");
        // Segment 2 is nodes[2] (node 0 is the origin); its
        // length is 30, so the recovered size is 5*30/(4*2-1).
        assert_eq!(
            fig.def.nodes[2].media,
            Some(SegmentMedia::Text {
                content: "HI".into(),
                size: 150.0 / 7.0,
            })
        );
    }

    #[test]
    fn v5_export_without_channels_or_media_is_unchanged() {
        // Guard: a plain project still writes len_flag 0 / no
        // media byte on every object (the wave-3 subset).
        let project = Project::new_demo();
        let payload = inflate_export(&export_piv_v5(&project).unwrap());
        let frames = walk_v5_export(&payload);
        assert!(frames
            .iter()
            .flatten()
            .all(|o| o.len_flag == 0 && o.bend_flag == 0));
    }

    #[test]
    fn v5_bends_roundtrip_through_import_and_export() {
        // Wave 7 supersedes the wave-6 expectation (this test used
        // to assert bends are dropped on import and never
        // written): `Frame.bends` now carries them end to end.
        // Hand-built v5 file whose figure record carries a bend
        // (segment 0 -> 0.7 rad) and whose frame carries a bend
        // array ([0.7, -0.3]). The original's Python decoder reads
        // the same bytes with those values (cross-check fixture).
        let mut b: Vec<u8> = Vec::new();
        b.push(5); // version
        b.extend_from_slice(&800u32.to_le_bytes());
        b.extend_from_slice(&600u32.to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes()); // background slots: default only
        b.extend_from_slice(&3u16.to_le_bytes()); // figure slots: builtin + 1
        b.push(0); // figure flags: all optional fields present
        b.extend_from_slice(&2u16.to_le_bytes()); // segment count
        // Segment 1: parent 0, id 1, len 50, up, thick 6, line, red.
        b.extend_from_slice(&0u16.to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&50.0f32.to_le_bytes());
        b.extend_from_slice(&(-std::f64::consts::FRAC_PI_2).to_le_bytes());
        b.extend_from_slice(&6.0f32.to_le_bytes());
        b.push(0); // primitive
        b.push(0); // aux
        b.extend_from_slice(&[255, 0, 0]);
        b.push(0); // transparency
        b.push(0); // no secondary colour
        // Segment 2: parent 1, id 2, len 30, right, thick 4, circle, blue.
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
        // Bend block: one record, segment index 0, 0.7 rad.
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&0u16.to_le_bytes());
        b.extend_from_slice(&0.7f64.to_le_bytes());
        b.extend_from_slice(&0u16.to_le_bytes()); // polyfill count
        b.extend_from_slice(&0u16.to_le_bytes()); // z-order seg 1
        b.extend_from_slice(&1u16.to_le_bytes()); // z-order seg 2
        b.push(4);
        b.extend_from_slice(b"Bent");
        // One keyframe with a per-frame bend array.
        b.extend_from_slice(&1u32.to_le_bytes()); // frame count
        b.extend_from_slice(&2u16.to_le_bytes()); // duration
        b.push(0); // background mix
        b.extend_from_slice(&0u16.to_le_bytes()); // background id
        b.extend_from_slice(&1u16.to_le_bytes()); // object count
        b.extend_from_slice(&7u32.to_le_bytes()); // unique id
        b.extend_from_slice(&2u16.to_le_bytes()); // figure type
        b.extend_from_slice(&1.0f32.to_le_bytes()); // scale
        b.extend_from_slice(&0x0000_FF00u32.to_le_bytes()); // COLORREF green
        b.push(0); // field28
        b.extend_from_slice(&(-std::f64::consts::FRAC_PI_2).to_le_bytes());
        b.extend_from_slice(&0.0f64.to_le_bytes());
        b.extend_from_slice(&10.0f32.to_le_bytes()); // root x
        b.extend_from_slice(&20.0f32.to_le_bytes()); // root y
        b.extend_from_slice(&0u16.to_le_bytes()); // len flag
        b.extend_from_slice(&1u16.to_le_bytes()); // bend flag: array follows
        b.extend_from_slice(&0.7f64.to_le_bytes());
        b.extend_from_slice(&(-0.3f64).to_le_bytes());
        b.push(0); // join flag
        b.extend_from_slice(&0u16.to_le_bytes()); // draw order
        b.push(1); // camera default
        b.extend_from_slice(&24u32.to_le_bytes()); // fps
        std::fs::write("/tmp/v5-bent-input.piv", &b).unwrap();

        // Import keeps geometry and pose AND the bends: the
        // frame's bend array replaces the record bend wholesale
        // (the original's rule), stored per node (origin = 0).
        let mut project = import_piv(&b).unwrap();
        assert_eq!(project.figures.len(), 1);
        assert_eq!(project.figures[0].name, "Bent");
        assert_eq!(project.figures[0].def.nodes.len(), 3);
        let pose = &project.frames[0].poses[0];
        assert!((pose.root[0] - 10.0).abs() < 0.01 && (pose.root[1] - 20.0).abs() < 0.01);
        assert!((pose.angles[1] - 180.0).abs() < 0.05);
        assert!((pose.angles[2] + 90.0).abs() < 0.05);
        let (root0, ang1) = (pose.root, pose.angles[1]);
        let bends0 = project.bends_at(0.0, 0);
        assert_eq!(bends0.len(), 3);
        assert!((bends0[1] - 0.7).abs() < 1e-6, "segment 0 bend: {}", bends0[1]);
        assert!((bends0[2] + 0.3).abs() < 1e-6, "segment 1 bend: {}", bends0[2]);

        // A second frame with different bends (vs frame 0's,
        // which become the export's record bends).
        let mut frame2 = project.frames[0].clone();
        frame2.bends = vec![vec![0.0, -0.7, 0.7]];
        project.frames.push(frame2);

        // Export: record block = frame 0's bends; frame 0 stores
        // no array (it equals the record), frame 1 stores its own.
        let bytes = export_piv_v5(&project).unwrap();
        std::fs::write("/tmp/styk-export-v5-bent.piv", &bytes).unwrap();
        let payload = inflate_export(&bytes);
        let record = walk_v5_record_bends(&payload);
        assert_eq!(record.len(), 1);
        assert!((record[0][0] - 0.7).abs() < 1e-6, "record seg 0: {}", record[0][0]);
        assert!((record[0][1] + 0.3).abs() < 1e-6, "record seg 1: {}", record[0][1]);
        let frames = walk_v5_export(&payload);
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0][0].bend_flag, 0, "frame 0 inherits the record bends");
        assert_eq!(frames[1][0].bend_flag, 1, "frame 1 stores its own array");
        assert!((frames[1][0].bends[0] + 0.7).abs() < 1e-6);
        assert!((frames[1][0].bends[1] - 0.7).abs() < 1e-6);

        // Re-import reproduces both frames' bends and the pose.
        let back = import_piv(&bytes).unwrap();
        let b0 = back.bends_at(0.0, 0);
        assert!((b0[1] - 0.7).abs() < 1e-6 && (b0[2] + 0.3).abs() < 1e-6);
        let b1 = back.bends_at(1.0, 0);
        assert!((b1[1] + 0.7).abs() < 1e-6 && (b1[2] - 0.7).abs() < 1e-6, "frame 1 bends: {b1:?}");
        let pose2 = &back.frames[0].poses[0];
        assert!((pose2.root[0] - root0[0]).abs() < 0.01);
        assert!((pose2.angles[1] - ang1).abs() < 0.05);
    }

    #[test]
    fn write_v5_media_fixture_for_cross_check() {
        // Consumed by the original app's Python decoder
        // (decode_piv_bytes): image asset bytes/anchors, per-frame
        // length arrays, and bend absence are asserted there.
        let project = media_project();
        std::fs::write("/tmp/styk-export-v5-media.piv", export_piv_v5(&project).unwrap()).unwrap();
        std::fs::write("/tmp/styk-asset.png", &project.assets[0].png).unwrap();
    }

    #[test]
    fn corpus_fixtures_import_with_manifest_values() {
        let dir = std::path::Path::new("tests/fixtures");
        if !dir.exists() {
            return; // corpus is generated by scripts/gen_fixtures.py
        }
        let manifest: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join("manifest.json")).unwrap())
                .unwrap();
        for (file, expect) in manifest["fixtures"].as_object().unwrap() {
            let data = std::fs::read(dir.join(file)).unwrap();
            if file.ends_with(".stk") {
                let (def, _name) = import_stk(&data).unwrap_or_else(|e| panic!("{file}: {e}"));
                assert_eq!(def.nodes.len(), 4, "{file} nodes"); // origin + 3 segments
                let lens: Vec<f32> = def.nodes[1..].iter().map(|n| n.length).collect();
                assert_eq!(lens, vec![50.0, 30.0, 16.0], "{file} lengths");
                continue;
            }
            if file.ends_with(".nodes") {
                // Stick Nodes figure: the .nodes importer wraps the
                // figure in a fixed 960x720 / 24 fps project, at rest
                // at the stage centre (the container stores neither).
                let project = crate::nodes::import_nodes(&data)
                    .unwrap_or_else(|e| panic!("{file}: {e}"));
                assert_eq!(project.stage, [960.0, 720.0], "{file} canvas");
                assert_eq!(project.figures.len(), 1, "{file} figures");
                let fig = &project.figures[0];
                assert_eq!(
                    fig.name,
                    expect["figures"][0]["name"].as_str().unwrap(),
                    "{file} name"
                );
                let segs = expect["figures"][0]["segments"].as_u64().unwrap() as usize;
                // Model convention: node 0 is the zero-length origin.
                assert_eq!(fig.def.nodes.len(), segs + 1, "{file} nodes");
                assert_eq!(
                    project.frames.len(),
                    expect["frames"].as_u64().unwrap() as usize,
                    "{file} frames"
                );
                let root = project.frames[0].poses[0].root;
                let want = &expect["frame0_roots"][0];
                assert!(
                    (root[0] - want[0].as_f64().unwrap() as f32).abs() < 0.6
                        && (root[1] - want[1].as_f64().unwrap() as f32).abs() < 0.6,
                    "{file} frame-0 root {root:?}"
                );
                continue;
            }
            if file.ends_with(".nodemc") {
                // Stick Nodes animation: the synthetic clip's only
                // frame holds no instances, so it imports with zero
                // figures (the clip itself is not a ClipDef source —
                // it has sprite-free, join-free, empty frames).
                let project = crate::nodes_anim::import_nodes_animation(&data)
                    .unwrap_or_else(|e| panic!("{file}: {e}"));
                assert_eq!(
                    project.figures.len(),
                    expect["figures"].as_array().unwrap().len(),
                    "{file} figures"
                );
                assert_eq!(
                    project.frames.len(),
                    expect["frames"].as_u64().unwrap() as usize,
                    "{file} frames"
                );
                continue;
            }
            let project = import_piv(&data).unwrap_or_else(|e| panic!("{file}: {e}"));
            if file.ends_with(".piv") {
                assert_eq!(project.stage, [800.0, 600.0], "{file} canvas");
                let want_fps = expect["fps"].as_f64().unwrap() as f32;
                assert!((project.fps - want_fps).abs() < 1.5, "{file} fps {}", project.fps);
                assert_eq!(
                    project.frames.len(),
                    expect["frames"].as_u64().unwrap() as usize,
                    "{file} frames"
                );
                let fi = project
                    .figures
                    .iter()
                    .position(|f| f.name == "Tripod")
                    .unwrap_or_else(|| panic!("{file}: Tripod figure missing"));
                // Model convention: node 0 is the zero-length origin,
                // so a 3-segment figure has 4 nodes.
                assert_eq!(project.figures[fi].def.nodes.len(), 4, "{file} nodes");
                let lens: Vec<f32> =
                    project.figures[fi].def.nodes[1..].iter().map(|n| n.length).collect();
                assert_eq!(lens, vec![50.0, 30.0, 16.0], "{file} lengths");
                let root = project.frames[0].poses[fi].root;
                assert!(
                    (root[0] - 111.0).abs() < 0.6 && (root[1] - 222.0).abs() < 0.6,
                    "{file} frame-0 root {root:?}"
                );
            }
        }
    }

    #[test]
    fn probe_corpus_fixtures_match_manifest_values() {
        let dir = std::path::Path::new("tests/fixtures");
        if !dir.exists() {
            return; // corpus is generated by scripts/gen_fixtures.py
        }
        let manifest: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join("manifest.json")).unwrap())
                .unwrap();
        for (file, version) in [
            ("legacy-v2.piv", 2u8),
            ("legacy-v3.piv", 3),
            ("legacy-v4.piv", 4),
            ("modern-v5.piv", 5),
        ] {
            let data = std::fs::read(dir.join(file)).unwrap();
            let probe = probe_piv(&data).unwrap_or_else(|e| panic!("{file}: {e}"));
            assert_eq!(probe.version, version, "{file} version");
            assert_eq!(probe.canvas, (800, 600), "{file} canvas");
            // Header slot count: builtin + the one stored figure.
            assert_eq!(probe.figure_slots, 3, "{file} figure slots");
            let expect = &manifest["fixtures"][file];
            assert_eq!(
                probe.frames,
                expect["frames"].as_u64().unwrap() as usize,
                "{file} frames"
            );
            let want_fps = expect["fps"].as_f64().unwrap() as f32;
            assert!((probe.fps - want_fps).abs() < 1.5, "{file} fps {}", probe.fps);
            assert!(!probe.has_backgrounds, "{file} backgrounds");
        }
    }

    #[test]
    fn probe_agrees_with_demo_project_exports() {
        // Every exporter in this module probes back to the
        // project it was written from (v2's fps is the format
        // constant 13 by definition — the format stores no rate).
        let project = Project::new_demo();
        for (bytes, version) in [
            (export_piv_v2(&project), 2u8),
            (export_piv_v3(&project), 3),
            (export_piv_v4(&project), 4),
            (export_piv_v5(&project).unwrap(), 5),
        ] {
            let probe = probe_piv(&bytes).unwrap_or_else(|e| panic!("v{version}: {e}"));
            assert_eq!(probe.version, version, "v{version} version");
            assert_eq!(
                probe.canvas,
                (project.stage[0] as u32, project.stage[1] as u32),
                "v{version} canvas"
            );
            assert_eq!(probe.figure_slots, project.figures.len() + 2, "v{version} slots");
            assert_eq!(probe.frames, project.frames.len(), "v{version} frames");
            if version == 2 {
                assert_eq!(probe.fps, 13.0, "v2 fps");
            } else {
                assert!(
                    (probe.fps - project.fps).abs() < 1.5,
                    "v{version} fps {}",
                    probe.fps
                );
            }
        }
    }

    #[test]
    fn probe_rejects_garbage_and_truncation() {
        assert!(probe_piv(&[]).is_err());
        assert!(probe_piv(b"not a pivot file at all").is_err());
        // A raw version byte this probe does not cover.
        assert!(probe_piv(&[1, 0, 0, 0, 0]).is_err());
        // A real file cut short inside its frame records.
        let bytes = export_piv_v3(&Project::new_demo());
        assert!(probe_piv(&bytes[..bytes.len() - 3]).is_err());
    }

    #[test]
    fn write_v5_fixture_for_cross_check() {
        let project = Project::new_demo();
        std::fs::write("/tmp/styk-export-v5.piv", export_piv_v5(&project).unwrap()).unwrap();
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

    // ---- Wave 7: legacy v2 / v4 export ----

    /// Shared figure/pose assertions for a legacy round trip.
    fn assert_legacy_roundtrip(project: &Project, back: &Project) {
        assert_eq!(back.stage, project.stage);
        assert_eq!(back.figures.len(), project.figures.len());
        for (a, b) in project.figures.iter().zip(back.figures.iter()) {
            assert_eq!(a.name, b.name);
            assert_eq!(a.color, b.color);
            assert_eq!(a.def.nodes.len(), b.def.nodes.len());
            for (na, nb) in a.def.nodes.iter().zip(b.def.nodes.iter()) {
                assert!(
                    (na.length - nb.length).abs() < 0.01,
                    "length {} vs {}",
                    na.length,
                    nb.length
                );
                assert!(
                    (na.thickness - nb.thickness).abs() < 0.01,
                    "thickness {} vs {}",
                    na.thickness,
                    nb.thickness
                );
                assert!(
                    (crate::model::norm_deg(na.rest_angle - nb.rest_angle)).abs() < 0.1,
                    "rest {} vs {}",
                    na.rest_angle,
                    nb.rest_angle
                );
            }
        }
        assert_eq!(back.frames.len(), project.frames.len());
        for (a, b) in project.frames.iter().zip(back.frames.iter()) {
            for (pa, pb) in a.poses.iter().zip(b.poses.iter()) {
                assert!((pa.root[0] - pb.root[0]).abs() < 0.5, "root x {} vs {}", pa.root[0], pb.root[0]);
                assert!((pa.root[1] - pb.root[1]).abs() < 0.5, "root y {} vs {}", pa.root[1], pb.root[1]);
                assert_eq!(pa.angles.len(), pb.angles.len());
                for (x, y) in pa.angles.iter().zip(pb.angles.iter()) {
                    assert!(
                        (crate::model::norm_deg(x - y)).abs() < 0.1,
                        "angle {x} vs {y}"
                    );
                }
            }
        }
    }

    #[test]
    fn piv_v2_roundtrip_preserves_frames_and_poses() {
        let mut project = Project::new_demo();
        project.add_figure_with(FigureDef::quadruped(), "Quad", 0x5DB2FF, [300.0, 400.0]);
        project.frames[1].duration = 3.0; // v2 cannot store this
        let bytes = export_piv_v2(&project);
        assert_eq!(bytes[0], 2, "raw v2 payload starts with the version byte");
        // Kept on disk for cross-verification with the original
        // app's Python decoder (scripts/verify_fixtures.py style).
        std::fs::write("/tmp/styk-export-v2.piv", &bytes).unwrap();
        let back = import_piv(&bytes).unwrap();
        assert_legacy_roundtrip(&project, &back);
        // The documented v2 losses, asserted so they stay honest:
        // no timing is stored (fixed 13 fps, unit holds).
        assert_eq!(back.fps, 13.0);
        assert!(back.frames.iter().all(|f| f.duration == 1.0));
    }

    #[test]
    fn piv_v4_roundtrip_preserves_frames_poses_durations() {
        let mut project = Project::new_demo();
        project.add_figure_with(FigureDef::quadruped(), "Quad", 0x5DB2FF, [300.0, 400.0]);
        project.frames[1].duration = 3.0;
        let bytes = export_piv_v4(&project);
        assert_eq!(bytes[0], 4, "raw v4 payload starts with the version byte");
        std::fs::write("/tmp/styk-export-v4.piv", &bytes).unwrap();
        let back = import_piv(&bytes).unwrap();
        assert_legacy_roundtrip(&project, &back);
        assert!(
            (back.fps - project.fps).abs() < 1.0,
            "fps {} vs {}",
            back.fps,
            project.fps
        );
        assert_eq!(back.frames[1].duration, 3.0, "v4 keeps per-frame durations");
    }

    #[test]
    fn piv_v4_writes_length_arrays_only_when_channels_move_lengths() {
        // media_project carries a Length channel driving figure
        // 0's node 1 from 46 (frame 0, == def) to 90 (frame 4).
        let project = media_project();
        let bytes = export_piv_v4(&project);
        // Walk the export with the module's own reader, collecting
        // per-object len flags; the walk must land exactly on the
        // end of the payload (delay u32 + 1 trailing byte).
        let mut r = Reader::new(&bytes);
        assert_eq!(r.u8().unwrap(), 4);
        r.u32().unwrap();
        r.u32().unwrap(); // canvas
        assert_eq!(r.u8().unwrap(), 1); // background slots
        let figure_slots = r.u16().unwrap() as usize;
        let mut seg_counts = Vec::new();
        for _ in 2..figure_slots {
            let (def, ..) = parse_figure(&mut r, true).unwrap();
            seg_counts.push(def.nodes.len() - 1);
        }
        let frame_count = r.u32().unwrap() as usize;
        assert_eq!(frame_count, project.frames.len());
        let mut flags: Vec<Vec<u8>> = Vec::new();
        let mut frame2_lengths: Vec<f32> = Vec::new();
        for fi in 0..frame_count {
            r.u16().unwrap(); // duration
            r.u8().unwrap(); // background id
            let object_count = r.u16().unwrap() as usize;
            let mut row = Vec::new();
            for oi in 0..object_count {
                r.u32().unwrap(); // unique id
                let tid = r.u16().unwrap() as usize;
                r.f32().unwrap(); // scale
                r.u32().unwrap(); // colour
                r.u8().unwrap(); // transparency
                let segs = seg_counts[tid - 2];
                for _ in 0..segs {
                    r.f64().unwrap(); // angles
                }
                r.f32().unwrap();
                r.f32().unwrap(); // root
                let len_flag = r.u8().unwrap();
                row.push(len_flag);
                if len_flag != 0 {
                    let lengths: Vec<f32> = (0..segs).map(|_| r.f32().unwrap()).collect();
                    if fi == 2 && oi == 0 {
                        frame2_lengths = lengths;
                    }
                }
                assert_eq!(r.u8().unwrap(), 0, "join flag");
            }
            for _ in 0..object_count {
                r.u16().unwrap(); // draw order
            }
            flags.push(row);
        }
        r.u32().unwrap(); // trailer delay
        r.u8().unwrap(); // trailing loop byte
        assert_eq!(r.pos, bytes.len(), "walk consumes the export exactly");
        // The demo project has a single figure: frame 0 sits on the
        // def length (no array); frames 1..4 carry length arrays.
        assert_eq!(flags[0], vec![0]);
        for row in &flags[1..] {
            assert_eq!(*row, vec![1]);
        }
        assert!(
            (frame2_lengths[0] - 68.0).abs() < 0.5,
            "frame 2 node-1 length {}",
            frame2_lengths[0]
        );
    }
}
