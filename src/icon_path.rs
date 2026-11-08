//! SVG path-data (`d` attribute) parser for the icon system.
//!
//! Font Awesome glyphs ship as SVG path strings; the Rune
//! stage draws `Path`s built from move/line/quad/cubic/close
//! calls, so those strings have to be lowered into plain
//! segments first. This module is that lowering: it parses
//! the **full** SVG path grammar into [`Seg`] values in
//! absolute coordinates, ready for a caller to replay into a
//! Rune `Path` (`fill_path`).
//!
//! Grammar coverage (SVG 1.1 §8 / SVG 2 path data):
//!
//! * Commands `M/m L/l H/h V/v C/c S/s Q/q T/t A/a Z/z`,
//!   absolute and relative forms.
//! * Implicit repetition of parameter groups, including the
//!   implicit lineto groups that follow a moveto.
//! * `S`/`T` smooth-curve control-point reflection across
//!   the current point (only when the previous command was
//!   the matching curve family, per spec).
//! * `H`/`V` lowered to plain lines.
//! * Numbers with optional sign, fraction and exponent,
//!   separated by whitespace, commas, a sign, or a second
//!   decimal point (`"10-20"`, `"0.5.5"` both tokenise as
//!   two numbers). Arc flags are single `0`/`1` digits and
//!   may be packed against neighbouring numbers
//!   (`"0 01162.5"` → flags 0, 1, then 162.5).
//! * Arcs (`A/a`) have no Rune primitive, so they are
//!   converted to cubic Béziers here via the endpoint →
//!   centre parametrisation (spec appendix F.6.5): radii
//!   scaled up when too small for the chord, centre chosen
//!   by the large-arc/sweep flags, the sweep split into
//!   steps of at most 90° with the standard
//!   `κ = 4/3 · tan(Δ/4)` control distance. The final
//!   step's end point is snapped to the requested arc end
//!   point, so chained segments join exactly.
//!
//! Failure contract: **this parser never panics.** Icon
//! path data is data, not code — a malformed or truncated
//! string yields the best-effort prefix parsed so far (an
//! incomplete parameter group is dropped whole) and the
//! glyph simply degrades. No `unwrap`, no indexing that
//! can fail, no non-finite coordinates.

use std::f32::consts::{FRAC_PI_2, TAU};

/// One path segment in absolute stage coordinates.
///
/// `QuadTo`/`CubicTo` carry their control point(s) first
/// and the end point last, mirroring the argument order of
/// the Rune `Path` builder calls a caller replays these
/// into.
#[derive(Debug, Clone, PartialEq)]
pub enum Seg {
    MoveTo([f32; 2]),
    LineTo([f32; 2]),
    QuadTo([f32; 2], [f32; 2]),
    CubicTo([f32; 2], [f32; 2], [f32; 2]),
    Close,
}

/// Byte cursor over one path string.
///
/// Path data is ASCII by construction (command letters,
/// digits, signs, dots, separators); any other byte fails
/// the number/flag reads below and stops the parse, which
/// is the graceful path — the scanner itself never indexes
/// unchecked.
struct Scanner<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Scanner<'a> {
    fn new(d: &'a str) -> Self {
        Scanner { bytes: d.as_bytes(), pos: 0 }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    /// Whitespace and commas — the two separator kinds the
    /// grammar allows between parameters.
    fn skip_separators(&mut self) {
        while let Some(b) = self.peek() {
            if matches!(b, b' ' | b'\t' | b'\n' | b'\r' | b',') {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    /// Read one number at the cursor (after separators):
    /// optional sign, integer and/or fraction digits,
    /// optional exponent. A sign or a decimal point doubles
    /// as the separator from the previous number, so no
    /// explicit delimiter is required between numbers. On
    /// failure (no digits, or a non-finite value) returns
    /// `None` and leaves the cursor where the token began.
    fn read_number(&mut self) -> Option<f32> {
        self.skip_separators();
        let start = self.pos;
        let mut i = self.pos;
        let at = |i: usize| self.bytes.get(i).copied();
        if matches!(at(i), Some(b'+') | Some(b'-')) {
            i += 1;
        }
        let mut digits = false;
        while matches!(at(i), Some(b) if b.is_ascii_digit()) {
            i += 1;
            digits = true;
        }
        if at(i) == Some(b'.') {
            i += 1;
            while matches!(at(i), Some(b) if b.is_ascii_digit()) {
                i += 1;
                digits = true;
            }
        }
        if !digits {
            return None;
        }
        // Exponent: consumed only if at least one digit
        // follows the optional sign, so a trailing command
        // letter is never eaten by mistake.
        if matches!(at(i), Some(b'e') | Some(b'E')) {
            let mut j = i + 1;
            if matches!(at(j), Some(b'+') | Some(b'-')) {
                j += 1;
            }
            let exp_start = j;
            while matches!(at(j), Some(b) if b.is_ascii_digit()) {
                j += 1;
            }
            if j > exp_start {
                i = j;
            }
        }
        let text = std::str::from_utf8(self.bytes.get(start..i)?).ok()?;
        let value: f32 = text.parse().ok()?;
        if !value.is_finite() {
            return None;
        }
        self.pos = i;
        Some(value)
    }

    /// Read one arc flag: exactly one `0` or `1` digit
    /// (flags are fixed-width in the grammar so they can be
    /// packed directly against the following number).
    fn read_flag(&mut self) -> Option<bool> {
        self.skip_separators();
        match self.peek() {
            Some(b'0') => {
                self.pos += 1;
                Some(false)
            }
            Some(b'1') => {
                self.pos += 1;
                Some(true)
            }
            _ => None,
        }
    }
}

fn is_path_command(b: u8) -> bool {
    matches!(
        b,
        b'M' | b'm'
            | b'L'
            | b'l'
            | b'H'
            | b'h'
            | b'V'
            | b'v'
            | b'C'
            | b'c'
            | b'S'
            | b's'
            | b'Q'
            | b'q'
            | b'T'
            | b't'
            | b'A'
            | b'a'
            | b'Z'
            | b'z'
    )
}

/// Read one `(x, y)` parameter group, resolved to absolute
/// coordinates against `cur` for relative commands.
fn read_point(sc: &mut Scanner, cur: [f32; 2], rel: bool) -> Option<[f32; 2]> {
    let x = sc.read_number()?;
    let y = sc.read_number()?;
    Some(if rel {
        [cur[0] + x, cur[1] + y]
    } else {
        [x, y]
    })
}

/// The `S`/`T` reflection: mirror `prev` (the previous
/// curve's relevant control point) across the current
/// point; without a matching previous curve the current
/// point itself is the control point (spec behaviour).
fn reflect(prev: Option<[f32; 2]>, cur: [f32; 2]) -> [f32; 2] {
    match prev {
        Some(p) => [2.0 * cur[0] - p[0], 2.0 * cur[1] - p[1]],
        None => cur,
    }
}

/// Append the cubic approximation of one elliptical arc
/// (`from` → `to`, radii `rx`/`ry`, x-axis rotation
/// `rot_deg`, large-arc/sweep flags) to `segs`.
///
/// Endpoint → centre conversion per SVG spec F.6.5. A
/// zero radius degenerates to a straight line and a
/// zero-length arc to nothing, both per spec.
fn append_arc(
    segs: &mut Vec<Seg>,
    from: [f32; 2],
    rx: f32,
    ry: f32,
    rot_deg: f32,
    large: bool,
    sweep: bool,
    to: [f32; 2],
) {
    if from == to {
        return;
    }
    let mut rx = rx.abs();
    let mut ry = ry.abs();
    if rx == 0.0 || ry == 0.0 {
        segs.push(Seg::LineTo(to));
        return;
    }
    let phi = rot_deg.to_radians();
    let (sinp, cosp) = (phi.sin(), phi.cos());

    // F.6.5.1: centre candidate in prime (rotated) coords.
    let dx = (from[0] - to[0]) / 2.0;
    let dy = (from[1] - to[1]) / 2.0;
    let x1p = cosp * dx + sinp * dy;
    let y1p = -sinp * dx + cosp * dy;

    // F.6.5.2: radii too small for the chord scale up.
    let lambda = x1p * x1p / (rx * rx) + y1p * y1p / (ry * ry);
    if lambda > 1.0 {
        let s = lambda.sqrt();
        rx *= s;
        ry *= s;
    }

    // F.6.5.3: centre in prime coords; the flag pair picks
    // which of the two candidate centres is meant.
    let num = rx * rx * ry * ry - rx * rx * y1p * y1p - ry * ry * x1p * x1p;
    let den = rx * rx * y1p * y1p + ry * ry * x1p * x1p;
    let mut coef = (num / den).max(0.0).sqrt();
    if large == sweep {
        coef = -coef;
    }
    let cxp = coef * rx * y1p / ry;
    let cyp = -coef * ry * x1p / rx;

    // F.6.5.4: centre back in the original coordinates.
    let cx = cosp * cxp - sinp * cyp + (from[0] + to[0]) / 2.0;
    let cy = sinp * cxp + cosp * cyp + (from[1] + to[1]) / 2.0;

    // F.6.5.5/6: start angle and signed sweep angle,
    // normalised so the sweep flag's direction wins.
    let ux = (x1p - cxp) / rx;
    let uy = (y1p - cyp) / ry;
    let vx = (-x1p - cxp) / rx;
    let vy = (-y1p - cyp) / ry;
    let theta1 = uy.atan2(ux);
    let mut dtheta = (ux * vy - uy * vx).atan2(ux * vx + uy * vy);
    if !sweep && dtheta > 0.0 {
        dtheta -= TAU;
    }
    if sweep && dtheta < 0.0 {
        dtheta += TAU;
    }

    // Split into ≤90° steps; each step is one cubic with
    // the standard κ control distance for its angle.
    let steps = ((dtheta.abs() / FRAC_PI_2).ceil() as i32).max(1);
    let step = dtheta / steps as f32;
    let kappa = (4.0 / 3.0) * (step / 4.0).tan();
    let point = |t: f32| -> [f32; 2] {
        let (s, c) = (t.sin(), t.cos());
        [
            cx + cosp * rx * c - sinp * ry * s,
            cy + sinp * rx * c + cosp * ry * s,
        ]
    };
    let deriv = |t: f32| -> [f32; 2] {
        let (s, c) = (t.sin(), t.cos());
        [
            -cosp * rx * s - sinp * ry * c,
            -sinp * rx * s + cosp * ry * c,
        ]
    };
    for k in 0..steps {
        let t0 = theta1 + step * k as f32;
        let t1 = t0 + step;
        let p0 = point(t0);
        let p3 = point(t1);
        let d0 = deriv(t0);
        let d1 = deriv(t1);
        let c1 = [p0[0] + kappa * d0[0], p0[1] + kappa * d0[1]];
        let c2 = [p3[0] - kappa * d1[0], p3[1] - kappa * d1[1]];
        // The last step ends exactly on the requested arc
        // end point (not the ellipse evaluation of it), so
        // the following segment starts where the path
        // string says it does.
        let end = if k == steps - 1 { to } else { p3 };
        segs.push(Seg::CubicTo(c1, c2, end));
    }
}

/// Parse an SVG path-data string into absolute segments.
///
/// Movetos open subpaths, `Z` becomes [`Seg::Close`], arcs
/// become cubics (see [`append_arc`]); everything else maps
/// one-to-one. Malformed or truncated input stops the
/// parse and returns the segments gathered so far — this
/// function never panics.
pub fn parse_path(d: &str) -> Vec<Seg> {
    let mut sc = Scanner::new(d);
    let mut segs: Vec<Seg> = Vec::new();
    let mut cmd: Option<u8> = None;
    let mut cur = [0.0f32; 2];
    let mut start = [0.0f32; 2];
    // Reflection sources: the previous cubic's second
    // control point, the previous quadratic's control
    // point. Each is `Some` only directly after its own
    // curve family ran (the spec's condition for `S`/`T`).
    let mut prev_cubic: Option<[f32; 2]> = None;
    let mut prev_quad: Option<[f32; 2]> = None;

    loop {
        sc.skip_separators();
        let Some(byte) = sc.peek() else { break };
        if byte.is_ascii_alphabetic() {
            if !is_path_command(byte) {
                break;
            }
            sc.pos += 1;
            if byte == b'Z' || byte == b'z' {
                segs.push(Seg::Close);
                cur = start;
                prev_cubic = None;
                prev_quad = None;
                cmd = None;
                continue;
            }
            cmd = Some(byte);
        }
        let Some(c) = cmd else { break };
        let rel = c.is_ascii_lowercase();
        match c.to_ascii_uppercase() {
            b'M' => {
                let Some(p) = read_point(&mut sc, cur, rel) else { break };
                segs.push(Seg::MoveTo(p));
                cur = p;
                start = p;
                prev_cubic = None;
                prev_quad = None;
                // Further coordinate pairs for this moveto
                // are implicit linetos of the same case.
                cmd = Some(if rel { b'l' } else { b'L' });
            }
            b'L' => {
                let Some(p) = read_point(&mut sc, cur, rel) else { break };
                segs.push(Seg::LineTo(p));
                cur = p;
                prev_cubic = None;
                prev_quad = None;
            }
            b'H' => {
                let Some(x) = sc.read_number() else { break };
                let p = [if rel { cur[0] + x } else { x }, cur[1]];
                segs.push(Seg::LineTo(p));
                cur = p;
                prev_cubic = None;
                prev_quad = None;
            }
            b'V' => {
                let Some(y) = sc.read_number() else { break };
                let p = [cur[0], if rel { cur[1] + y } else { y }];
                segs.push(Seg::LineTo(p));
                cur = p;
                prev_cubic = None;
                prev_quad = None;
            }
            b'C' => {
                let Some(c1) = read_point(&mut sc, cur, rel) else { break };
                let Some(c2) = read_point(&mut sc, cur, rel) else { break };
                let Some(e) = read_point(&mut sc, cur, rel) else { break };
                segs.push(Seg::CubicTo(c1, c2, e));
                prev_cubic = Some(c2);
                prev_quad = None;
                cur = e;
            }
            b'S' => {
                let c1 = reflect(prev_cubic, cur);
                let Some(c2) = read_point(&mut sc, cur, rel) else { break };
                let Some(e) = read_point(&mut sc, cur, rel) else { break };
                segs.push(Seg::CubicTo(c1, c2, e));
                prev_cubic = Some(c2);
                prev_quad = None;
                cur = e;
            }
            b'Q' => {
                let Some(q) = read_point(&mut sc, cur, rel) else { break };
                let Some(e) = read_point(&mut sc, cur, rel) else { break };
                segs.push(Seg::QuadTo(q, e));
                prev_quad = Some(q);
                prev_cubic = None;
                cur = e;
            }
            b'T' => {
                let q = reflect(prev_quad, cur);
                let Some(e) = read_point(&mut sc, cur, rel) else { break };
                segs.push(Seg::QuadTo(q, e));
                prev_quad = Some(q);
                prev_cubic = None;
                cur = e;
            }
            b'A' => {
                let Some(rx) = sc.read_number() else { break };
                let Some(ry) = sc.read_number() else { break };
                let Some(rot) = sc.read_number() else { break };
                let Some(large) = sc.read_flag() else { break };
                let Some(sweep) = sc.read_flag() else { break };
                let Some(e) = read_point(&mut sc, cur, rel) else { break };
                append_arc(&mut segs, cur, rx, ry, rot, large, sweep, e);
                cur = e;
                prev_cubic = None;
                prev_quad = None;
            }
            // Unreachable: `cmd` only ever holds a validated
            // command letter. Break rather than panic anyway.
            _ => break,
        }
    }
    segs
}

/// Bounding box `[min_x, min_y, max_x, max_y]` of a parsed
/// path, or `None` when it has no points at all (empty, or
/// only [`Seg::Close`]).
///
/// This is the **control hull**: curve control points are
/// included, so the box can slightly overestimate the true
/// curve bounds (a Bézier never leaves its control hull,
/// so it can never underestimate). For icon layout — the
/// consumer here — the hull is the safe bound to fit by.
pub fn path_bbox(segs: &[Seg]) -> Option<[f32; 4]> {
    let mut bb: Option<[f32; 4]> = None;
    let mut eat = |p: [f32; 2]| match &mut bb {
        Some(b) => {
            b[0] = b[0].min(p[0]);
            b[1] = b[1].min(p[1]);
            b[2] = b[2].max(p[0]);
            b[3] = b[3].max(p[1]);
        }
        None => bb = Some([p[0], p[1], p[0], p[1]]),
    };
    for seg in segs {
        match seg {
            Seg::MoveTo(p) | Seg::LineTo(p) => eat(*p),
            Seg::QuadTo(c, e) => {
                eat(*c);
                eat(*e);
            }
            Seg::CubicTo(c1, c2, e) => {
                eat(*c1);
                eat(*c2);
                eat(*e);
            }
            Seg::Close => {}
        }
    }
    bb
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pt(x: f32, y: f32) -> [f32; 2] {
        [x, y]
    }

    fn assert_pt_close(actual: [f32; 2], expected: [f32; 2], eps: f32) {
        assert!(
            (actual[0] - expected[0]).abs() < eps && (actual[1] - expected[1]).abs() < eps,
            "expected {expected:?} (+/-{eps}), got {actual:?}"
        );
    }

    /// Cubic Bézier evaluation at `t` (test oracle for the
    /// arc-conversion tests).
    fn cubic_at(p0: [f32; 2], c1: [f32; 2], c2: [f32; 2], p1: [f32; 2], t: f32) -> [f32; 2] {
        let u = 1.0 - t;
        let w = [u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t];
        [
            w[0] * p0[0] + w[1] * c1[0] + w[2] * c2[0] + w[3] * p1[0],
            w[0] * p0[1] + w[1] * c1[1] + w[2] * c2[1] + w[3] * p1[1],
        ]
    }

    #[test]
    fn line_only_square() {
        let segs = parse_path("M0 0L100 0L100 100L0 100Z");
        assert_eq!(
            segs,
            vec![
                Seg::MoveTo(pt(0.0, 0.0)),
                Seg::LineTo(pt(100.0, 0.0)),
                Seg::LineTo(pt(100.0, 100.0)),
                Seg::LineTo(pt(0.0, 100.0)),
                Seg::Close,
            ]
        );
        assert_eq!(path_bbox(&segs), Some([0.0, 0.0, 100.0, 100.0]));
    }

    #[test]
    fn bbox_of_empty_and_close_only_is_none() {
        assert_eq!(path_bbox(&[]), None);
        assert_eq!(path_bbox(&parse_path("Z")), None);
    }

    #[test]
    fn hv_are_lowered_to_lines() {
        let segs = parse_path("M10 10H90V90H10Z");
        assert_eq!(
            segs,
            vec![
                Seg::MoveTo(pt(10.0, 10.0)),
                Seg::LineTo(pt(90.0, 10.0)),
                Seg::LineTo(pt(90.0, 90.0)),
                Seg::LineTo(pt(10.0, 90.0)),
                Seg::Close,
            ]
        );
        // Relative H/V move along one axis from the current point.
        let segs = parse_path("m10 10h80v80h-80z");
        assert_eq!(
            segs,
            vec![
                Seg::MoveTo(pt(10.0, 10.0)),
                Seg::LineTo(pt(90.0, 10.0)),
                Seg::LineTo(pt(90.0, 90.0)),
                Seg::LineTo(pt(10.0, 90.0)),
                Seg::Close,
            ]
        );
    }

    #[test]
    fn relative_commands_and_implicit_group_repeat() {
        // One `l` letter, four coordinate pairs.
        let segs = parse_path("m10 10l50 0 0 50-50 0 0-50z");
        assert_eq!(
            segs,
            vec![
                Seg::MoveTo(pt(10.0, 10.0)),
                Seg::LineTo(pt(60.0, 10.0)),
                Seg::LineTo(pt(60.0, 60.0)),
                Seg::LineTo(pt(10.0, 60.0)),
                Seg::LineTo(pt(10.0, 10.0)),
                Seg::Close,
            ]
        );
    }

    #[test]
    fn implicit_lineto_after_moveto() {
        let segs = parse_path("M0 0 50 50 100 0");
        assert_eq!(
            segs,
            vec![
                Seg::MoveTo(pt(0.0, 0.0)),
                Seg::LineTo(pt(50.0, 50.0)),
                Seg::LineTo(pt(100.0, 0.0)),
            ]
        );
        // Relative moveto: implicit pairs are relative linetos.
        let segs = parse_path("m10 10 5 5");
        assert_eq!(
            segs,
            vec![Seg::MoveTo(pt(10.0, 10.0)), Seg::LineTo(pt(15.0, 15.0))]
        );
    }

    #[test]
    fn sign_and_dot_separated_numbers() {
        // A sign is its own separator: "10-20" is two numbers.
        let segs = parse_path("M10-20L30-40");
        assert_eq!(
            segs,
            vec![Seg::MoveTo(pt(10.0, -20.0)), Seg::LineTo(pt(30.0, -40.0))]
        );
        // As is a second decimal point: "0.5.5" is 0.5, 0.5.
        let segs = parse_path("M.5.5L1.5.25");
        assert_eq!(
            segs,
            vec![Seg::MoveTo(pt(0.5, 0.5)), Seg::LineTo(pt(1.5, 0.25))]
        );
    }

    #[test]
    fn exponents_and_fractions() {
        let segs = parse_path("M1e2 .5L.25e1 5");
        assert_eq!(
            segs,
            vec![Seg::MoveTo(pt(100.0, 0.5)), Seg::LineTo(pt(2.5, 5.0))]
        );
    }

    #[test]
    fn s_reflects_the_previous_cubic_control_point() {
        let segs = parse_path("M0 0C10 0 20 10 30 10S50 20 60 10");
        assert_eq!(
            segs,
            vec![
                Seg::MoveTo(pt(0.0, 0.0)),
                Seg::CubicTo(pt(10.0, 0.0), pt(20.0, 10.0), pt(30.0, 10.0)),
                // Reflected: (20,10) mirrored across (30,10) is (40,10).
                Seg::CubicTo(pt(40.0, 10.0), pt(50.0, 20.0), pt(60.0, 10.0)),
            ]
        );
    }

    #[test]
    fn s_without_a_previous_curve_uses_the_current_point() {
        let segs = parse_path("M0 0L30 10S50 20 60 10");
        assert_eq!(
            segs,
            vec![
                Seg::MoveTo(pt(0.0, 0.0)),
                Seg::LineTo(pt(30.0, 10.0)),
                Seg::CubicTo(pt(30.0, 10.0), pt(50.0, 20.0), pt(60.0, 10.0)),
            ]
        );
    }

    #[test]
    fn t_reflects_the_previous_quad_control_point() {
        let segs = parse_path("M0 0Q10 10 20 0T40 0");
        assert_eq!(
            segs,
            vec![
                Seg::MoveTo(pt(0.0, 0.0)),
                Seg::QuadTo(pt(10.0, 10.0), pt(20.0, 0.0)),
                // Reflected: (10,10) mirrored across (20,0) is (30,-10).
                Seg::QuadTo(pt(30.0, -10.0), pt(40.0, 0.0)),
            ]
        );
    }

    #[test]
    fn arc_quarter_circle_endpoint_exact_midpoint_on_circle() {
        // Quarter of a radius-100 circle centred at the origin.
        let segs = parse_path("M100 0A100 100 0 0 1 0 100");
        assert_eq!(segs.len(), 2);
        assert_eq!(segs[0], Seg::MoveTo(pt(100.0, 0.0)));
        let Seg::CubicTo(c1, c2, end) = segs[1] else {
            panic!("a <=90-degree arc must be one cubic, got {:?}", segs[1]);
        };
        // Endpoint is the requested arc end, exactly (snapped).
        assert_eq!(end, pt(0.0, 100.0));
        // With kappa = 4/3*tan(pi/8) the cubic's midpoint lies
        // on the circle: (100/sqrt(2), 100/sqrt(2)).
        let mid = cubic_at(pt(100.0, 0.0), c1, c2, end, 0.5);
        assert_pt_close(mid, pt(70.7107, 70.7107), 0.5);
        // Control hull of the quarter is the square corner.
        let bb = path_bbox(&segs).expect("arc has points");
        assert_pt_close([bb[0], bb[1]], pt(0.0, 0.0), 0.01);
        assert_pt_close([bb[2], bb[3]], pt(100.0, 100.0), 0.01);
    }

    #[test]
    fn arc_semicircle_sweep_picks_the_side() {
        // sweep=1: halfway point of the two cubics is (50,-50).
        let segs = parse_path("M0 0A50 50 0 0 1 100 0");
        let Seg::CubicTo(_, _, junction) = segs[1] else {
            panic!("semicircle is two cubics, got {:?}", segs[1]);
        };
        assert_pt_close(junction, pt(50.0, -50.0), 0.05);
        // sweep=0: the same chord bulges the other way.
        let segs = parse_path("M0 0A50 50 0 0 0 100 0");
        let Seg::CubicTo(_, _, junction) = segs[1] else {
            panic!("semicircle is two cubics, got {:?}", segs[1]);
        };
        assert_pt_close(junction, pt(50.0, 50.0), 0.05);
    }

    #[test]
    fn arc_relative_form_and_zero_radius_line() {
        let segs = parse_path("M100 0a100 100 0 0 1-100 100");
        let Some(Seg::CubicTo(_, _, end)) = segs.last() else {
            panic!("relative arc must end in a cubic, got {segs:?}");
        };
        assert_eq!(*end, pt(0.0, 100.0));
        // rx = 0 degenerates to a straight line (spec).
        let segs = parse_path("M0 0A0 50 0 0 1 100 0");
        assert_eq!(
            segs,
            vec![Seg::MoveTo(pt(0.0, 0.0)), Seg::LineTo(pt(100.0, 0.0))]
        );
        // Start == end: no segments at all (spec).
        let segs = parse_path("M5 5A50 50 0 0 1 5 5L9 9");
        assert_eq!(
            segs,
            vec![Seg::MoveTo(pt(5.0, 5.0)), Seg::LineTo(pt(9.0, 9.0))]
        );
    }

    #[test]
    fn packed_arc_flags() {
        // "0 01162.5": rotation 0, flags 0 and 1 packed
        // against the endpoint x (162.5).
        let segs = parse_path("M0 0A30 50 0 01162.5 121.5");
        let Some(Seg::CubicTo(_, _, end)) = segs.last() else {
            panic!("arc must end in a cubic, got {segs:?}");
        };
        assert_eq!(*end, pt(162.5, 121.5));
    }

    #[test]
    fn garbage_never_panics_and_keeps_the_good_prefix() {
        assert_eq!(parse_path(""), vec![]);
        assert_eq!(parse_path("M"), vec![]);
        assert_eq!(parse_path("X10 10"), vec![]);
        assert_eq!(parse_path("garbage"), vec![]);
        assert_eq!(parse_path("A5 5"), vec![]);
        // Non-finite numbers are rejected, not propagated.
        assert_eq!(parse_path("M1e999 0"), vec![]);
        // A moveto whose second coordinate never arrives
        // contributes nothing.
        assert_eq!(parse_path("M10"), vec![]);
        // Truncated arc: the complete moveto survives.
        assert_eq!(
            parse_path("M10 10A5 5 0 1"),
            vec![Seg::MoveTo(pt(10.0, 10.0))]
        );
        // Truncated line group likewise.
        assert_eq!(parse_path("M10 10L"), vec![Seg::MoveTo(pt(10.0, 10.0))]);
        // After Z a bare number has no command to feed: stop.
        assert_eq!(
            parse_path("M0 0Z5 5"),
            vec![Seg::MoveTo(pt(0.0, 0.0)), Seg::Close]
        );
    }

    /// Font Awesome `play` (view box 448×512), copied
    /// verbatim from `src/rune_sec/icon_data.rs` so this
    /// lib test stays independent of the shell crate.
    const PLAY_D: &str = "M91.2 36.9c-12.4-6.8-27.4-6.5-39.6 .7S32 57.9 32 72l0 368c0 14.1 7.5 27.2 19.6 34.4s27.2 7.5 39.6 .7l336-184c12.8-7 20.8-20.5 20.8-35.1s-8-28.1-20.8-35.1l-336-184z";

    #[test]
    fn font_awesome_play_parses_and_fits_its_view_box() {
        let segs = parse_path(PLAY_D);
        assert_eq!(segs.first(), Some(&Seg::MoveTo(pt(91.2, 36.9))));
        assert_eq!(segs.last(), Some(&Seg::Close));
        let bb = path_bbox(&segs).expect("play has points");
        // Inside the 448x512 view box (0.1 slack for f32 dust).
        assert!(
            bb[0] >= -0.1 && bb[1] >= -0.1 && bb[2] <= 448.1 && bb[3] <= 512.1,
            "play hull {bb:?} escapes its view box"
        );
        // Pinned hull, cross-checked against a reference parse.
        assert_pt_close([bb[0], bb[1]], pt(32.0, 30.1), 0.5);
        assert_pt_close([bb[2], bb[3]], pt(448.0, 481.9), 0.5);
    }

    /// Font Awesome `gear` (view box 512×512), copied
    /// verbatim from `src/rune_sec/icon_data.rs`.
    const GEAR_D: &str = "M195.1 9.5C198.1-5.3 211.2-16 226.4-16l59.8 0c15.2 0 28.3 10.7 31.3 25.5L332 79.5c14.1 6 27.3 13.7 39.3 22.8l67.8-22.5c14.4-4.8 30.2 1.2 37.8 14.4l29.9 51.8c7.6 13.2 4.9 29.8-6.5 39.9L447 233.3c.9 7.4 1.3 15 1.3 22.7s-.5 15.3-1.3 22.7l53.4 47.5c11.4 10.1 14 26.8 6.5 39.9l-29.9 51.8c-7.6 13.1-23.4 19.2-37.8 14.4l-67.8-22.5c-12.1 9.1-25.3 16.7-39.3 22.8l-14.4 69.9c-3.1 14.9-16.2 25.5-31.3 25.5l-59.8 0c-15.2 0-28.3-10.7-31.3-25.5l-14.4-69.9c-14.1-6-27.2-13.7-39.3-22.8L73.5 432.3c-14.4 4.8-30.2-1.2-37.8-14.4L5.8 366.1c-7.6-13.2-4.9-29.8 6.5-39.9l53.4-47.5c-.9-7.4-1.3-15-1.3-22.7s.5-15.3 1.3-22.7L12.3 185.8c-11.4-10.1-14-26.8-6.5-39.9L35.7 94.1c7.6-13.2 23.4-19.2 37.8-14.4l67.8 22.5c12.1-9.1 25.3-16.7 39.3-22.8L195.1 9.5zM256.3 336a80 80 0 1 0 -.6-160 80 80 0 1 0 .6 160z";

    #[test]
    fn font_awesome_gear_parses_fully() {
        let segs = parse_path(GEAR_D);
        assert_eq!(segs.first(), Some(&Seg::MoveTo(pt(195.1, 9.5))));
        assert_eq!(segs.last(), Some(&Seg::Close));
        // A dense glyph: 43 command letters (many with
        // repeated parameter groups), and the centre hole
        // is two full circles drawn as arcs (4 cubics each).
        assert!(segs.len() >= 30, "gear parsed only {} segments", segs.len());
        // NOTE: unlike `play`, this glyph's data overshoots
        // its nominal view box on every side (tooth tips
        // reach y = -16, the hull reaches 528) — that is the
        // shipped Font Awesome data, not a parser artefact,
        // so pin the true hull instead of the box.
        let bb = path_bbox(&segs).expect("gear has points");
        assert_pt_close([bb[0], bb[1]], pt(-1.8, -16.0), 1.0);
        assert_pt_close([bb[2], bb[3]], pt(514.4, 528.0), 1.0);
    }
}
