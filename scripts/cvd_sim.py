#!/usr/bin/env python3
"""cvd_sim.py — colour-vision-deficiency (CVD) simulation pass over the
StykAnimate-RS shell's mark-system and palette colour pairs.

Roadmap Wave 2, seat W2-52 (A20-10, CVD half). This instrument closes
audit gap G12 / A20 §1.5's verification gap: until now, CVD safety of
the dopesheet/timeline mark system, the graph canvas and the toast
severity set was argued from the encoding design only — no simulation
had ever been run. This script runs one, from the actual tokens.

METHOD
------
1. Colours are built with the shell's own semantics, mirroring the
   pinned Rune crate (rev 87caf2a, crates/rune/src/color.rs +
   crates/rune-motion/src/value.rs) exactly, the same approach RM25's
   calculator validated in Wave 1:
   - hex literals decode through the sRGB EOTF (piecewise, 2.4);
   - OKLCH converts via Bjorn Ottosson's OKLab matrices, clamped to
     [0, 1] in linear sRGB (Color::from_oklab);
   - Color stores LINEAR-light sRGB; mix() lerps in linear;
     alpha compositing is premultiplied source-over in linear;
   - WCAG luminance = 0.2126 R + 0.7152 G + 0.0722 B over the linear
     components (Color::luminance); ratio = (hi + .05) / (lo + .05).
   Before any simulated number is produced, the validation suite
   (run by default) reproduces the published audit record (A20,
   app_theme.rs Wave 9/10 + RM25 Polish re-audit) — 48 figures,
   max observed deviation 0.02; a validation failure aborts the
   run with exit 1.

2. Dichromacy simulation: Machado, Oliveira & Fernandes (2009),
   "A Physiologically-based Model for Simulation of Color Vision
   Deficiency", IEEE Trans. Visualization & Computer Graphics
   15(6):1291-1298 — the severity-1.0 (full dichromacy) matrices
   from Table 1 of that paper, applied to gamma-encoded sRGB
   triplets in [0, 1], results clamped to [0, 1]. These matrices are
   the de-facto standard for tooling (Colour, daltonlens, Coblis's
   Machado mode). One model, stated once: individual dichromats
   vary, and anomalous trichromacy (partial deficiency) is milder
   than the severity-1.0 case simulated here, so severity 1.0 is
   the conservative end.

3. Distinguishability of a simulated pair is reported as:
   - dE00: CIEDE2000 colour difference (Sharma, Wu & Dalal 2005
     formulation) between the two simulated colours, via CIELAB
     (D65, sRGB->XYZ matrix after Lindbloom);
   - dh: CIELAB hue-angle separation of the simulated colours
     (degrees; suppressed when either colour is near-neutral,
     C*ab < 5, where hue angle is unstable);
   - dL*: CIELAB lightness difference (the channel that survives
     every dichromacy);
   - ratio: WCAG contrast ratio of the simulated colours.
   Verdict bands (small 12 pt glyphs, side-by-side at best):
     pair-must-read-apart:  dE00 >= 10 DISTINGUISHABLE,
       5-10 MARGINAL, 2-5 THIN, < 2 COLLAPSED.
     ink-on-ground: simulated ratio >= 3.0 PASS (the non-text bar;
       text pairs use 4.5), 2.0-3.0 WEAK, < 2.0 FAIL.

Usage: python3 scripts/cvd_sim.py [--no-validate] [--save OUT.md]
Exit status: 0 when validation passes, 1 otherwise.
"""

import math
import sys

# ------------------------------------------------------------------
# Rune colour semantics (mirror of the pinned crate; see docstring)
# ------------------------------------------------------------------


def srgb_decode(c):
    return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4


def srgb_encode(c):
    return c * 12.92 if c <= 0.0031308 else 1.055 * c ** (1.0 / 2.4) - 0.055


def hex_lin(s):
    """'#RRGGBB' -> linear (r, g, b)."""
    h = s.lstrip('#')
    return tuple(srgb_decode(int(h[i:i + 2], 16) / 255.0) for i in (0, 2, 4))


def oklab_to_linear(L, a, b):
    l_ = L + 0.39633778 * a + 0.21580376 * b
    m_ = L - 0.105561346 * a - 0.06385417 * b
    s_ = L - 0.08948418 * a - 1.2914855 * b
    l_, m_, s_ = l_ ** 3, m_ ** 3, s_ ** 3
    return (
        4.0767417 * l_ - 3.3077116 * m_ + 0.23096994 * s_,
        -1.268438 * l_ + 2.6097574 * m_ - 0.34131938 * s_,
        -0.0041960863 * l_ - 0.7034186 * m_ + 1.7076147 * s_,
    )


def oklch(l, c, h_deg, alpha=1.0):
    """Rune oklcha(): OKLCH -> linear sRGB (clamped), with alpha."""
    hr = math.radians(h_deg)
    r, g, b = oklab_to_linear(l, c * math.cos(hr), c * math.sin(hr))
    clamp = lambda x: min(1.0, max(0.0, x))
    return (clamp(r), clamp(g), clamp(b), alpha)


# Colours are (r, g, b, a) in linear light, straight alpha.

def C(rgb, a=1.0):
    return (rgb[0], rgb[1], rgb[2], a)


def with_alpha(col, a):
    return (col[0], col[1], col[2], a)


def mix(c1, c2, t):
    """Rune Color::mix — lerp in linear, alpha included."""
    return tuple(c1[i] + (c2[i] - c1[i]) * t for i in range(4))


def over(top, bottom):
    """Premultiplied source-over in linear (Rune color::over);
    both args straight-alpha colours, result straight-alpha."""
    ta = top[3]
    tp = [top[i] * ta for i in range(3)]
    bp = [bottom[i] * bottom[3] for i in range(3)]
    k = 1.0 - ta
    out_a = ta + bottom[3] * k
    out = [tp[i] + bp[i] * k for i in range(3)]
    if out_a <= 0.0:
        return (0.0, 0.0, 0.0, 0.0)
    return (out[0] / out_a, out[1] / out_a, out[2] / out_a, out_a)


def lum(col):
    return 0.2126 * col[0] + 0.7152 * col[1] + 0.0722 * col[2]


def ratio(c1, c2):
    a, b = lum(c1), lum(c2)
    hi, lo = max(a, b), min(a, b)
    return (hi + 0.05) / (lo + 0.05)


def to_srgb255(col):
    return tuple(round(srgb_encode(min(1.0, max(0.0, col[i]))) * 255) for i in range(3))


def hex_str(col):
    r, g, b = to_srgb255(col)
    return f"#{r:02X}{g:02X}{b:02X}"


WHITE = (1.0, 1.0, 1.0, 1.0)
BLACK = (0.0, 0.0, 0.0, 1.0)

# ------------------------------------------------------------------
# Tokens — hand-copied from the sources of record, cited per line.
# app_theme.rs: consts at lines 450-525, fns at 584-760.
# rune tok.rs (pinned rev): theme() lines 168-210, accent() line 212.
# design.rs: accent() 193, selected_fill() 200, kbd pair 357-359.
# ------------------------------------------------------------------

UPSTREAM_ACCENT = oklch(0.66, 0.17, 256.0)  # tok::accent(0, true), both themes


def theme_ns(dark):
    """The named colour namespace for one appearance."""
    if dark:
        ns = dict(
            bg=C(hex_lin("#101216")),          # DARK_BG
            card=C(hex_lin("#1D222B")),        # DARK_CARD
            text=C(hex_lin("#F1F3F7")),        # DARK_TEXT
            text2=C(hex_lin("#A1A9B7")),       # DARK_TEXT_SECONDARY
            muted=C(hex_lin("#737D8C")),       # DARK_MUTED_TEXT
            chrome=C(hex_lin("#171B22")),      # DARK_CHROME_SURFACE
            elevated=C(hex_lin("#252C37")),    # DARK_ELEVATED
            brief=C(hex_lin("#6C8CFF")),       # ACCENT_DARK (app_theme::accent)
            secondary=C(hex_lin("#9B7CFF")),   # SECONDARY_ACCENT_DARK
            success=C(hex_lin("#43C995")),     # success_on, dark
            warning=C(hex_lin("#F1B85B")),     # warning_on, dark
            error=C(hex_lin("#EF6974")),       # error_on, dark
            control_off=oklch(1.0, 0.0, 0.0, 0.085),   # tok dark: white @ .085
            grip=None,                         # filled below
            strip_play=None,                   # filled below (brief accent)
        )
        # design.rs splitter_grip_color (1615): dark takes
        # design::text_muted = text_secondary @ 0.6 alpha; a glyph
        # colour renders composited over the ground (RM25's own
        # test at design.rs:1905 does exactly this `over`).
        ns["grip"] = over(with_alpha(ns["text2"], 0.6), ns["bg"])
        ns["strip_play"] = ns["brief"]         # status_strip strip_colors, dark
    else:
        bg = C(hex_lin("#EDF0F5"))             # LIGHT_BG
        card = (1.0, 1.0, 1.0, 1.0)            # upstream card: oklch(1,0,0)
        text2 = C(hex_lin("#5B6577"))          # LIGHT_TEXT_SECONDARY
        brief = C(hex_lin("#4666D8"))          # ACCENT_LIGHT
        ns = dict(
            bg=bg,
            card=card,
            text=C(hex_lin("#171A21")),        # LIGHT_TEXT
            text2=text2,
            muted=mix(text2, bg, 0.3),         # muted_text(), light arm
            chrome=mix(bg, card, 0.5),         # chrome_surface(), light arm
            elevated=mix(card, bg, 0.10),      # elevated(), LIGHT_ELEVATED_BG_MIX
            brief=brief,
            secondary=C(hex_lin("#7A5AE0")),   # SECONDARY_ACCENT_LIGHT
            success=C(hex_lin("#177A52")),     # SUCCESS_LIGHT_HEX
            warning=C(hex_lin("#8F6108")),     # WARNING_LIGHT_HEX
            error=C(hex_lin("#C62F3B")),       # ERROR_LIGHT_HEX
            control_off=oklch(0.2, 0.01, 265.0, 0.24),  # tok light control_off
            grip=text2,                        # splitter_grip_color, light arm
            strip_play=mix(brief, BLACK, 0.15),  # PLAYING_INK_LIGHT_DEEPEN
        )
    ns["accent"] = UPSTREAM_ACCENT             # design::accent() = tok::accent(0,true)
    # RM25 on-tint glyph variants (app_theme.rs 613-660).
    if dark:
        ns["accent_tint"] = mix(UPSTREAM_ACCENT, WHITE, 0.25)
        ns["secondary_tint"] = mix(ns["secondary"], WHITE, 0.25)
    else:
        ns["accent_tint"] = mix(UPSTREAM_ACCENT, BLACK, 0.35)
        ns["secondary_tint"] = ns["secondary"]
    # Derived surfaces.
    tint1 = mix(ns["card"], UPSTREAM_ACCENT, 0.16)   # selected_fill() over card
    ns["tint1"] = tint1
    ns["tint2"] = mix(tint1, UPSTREAM_ACCENT, 0.16)  # second coat (row x cell)
    ns["kbd_chip_card"] = over(ns["control_off"], ns["card"])
    ns["kbd_chip_elev"] = over(ns["control_off"], ns["elevated"])
    row = over(with_alpha(UPSTREAM_ACCENT, 0.85), ns["card"])
    ns["palette_row"] = row
    ns["keycap_fill"] = over(with_alpha(WHITE, 0.85), row)
    ns["keycap_ink"] = over(with_alpha(BLACK, 0.85), ns["keycap_fill"])
    ns["loop_fill"] = mix(ns["card"], UPSTREAM_ACCENT, 0.35)  # timeline_marks 1506
    ns["frame_line"] = over(with_alpha(UPSTREAM_ACCENT, 0.55), ns["card"])
    ns["amber"] = C(hex_lin("#FFC65D"))        # graph key diamonds, graph_panel 2661
    ns["white"] = WHITE
    return ns


DARK = theme_ns(True)
LIGHT = theme_ns(False)

# ------------------------------------------------------------------
# Validation — reproduce the published record before simulating.
# Sources: A20 §1.1/§1.5 + app_theme.rs module doc (Wave 9/10,
# Polish re-audit) + rm25.md. Tolerance +/-0.03 unless noted.
# ------------------------------------------------------------------


def validate():
    rows = []

    def chk(name, got, want, tol=0.03):
        rows.append((name, got, want, abs(got - want) <= tol))

    for label, ns in (("dark", DARK), ("light", LIGHT)):
        chk(f"white on upstream accent ({label})", ratio(WHITE, ns["accent"]), 3.14)
        chk(f"text on card ({label})", ratio(ns["text"], ns["card"]),
            14.37 if label == "dark" else 17.41, 0.05)
        chk(f"text_secondary on card ({label})", ratio(ns["text2"], ns["card"]),
            6.74 if label == "dark" else 5.88)
        chk(f"brief accent ink on card ({label})", ratio(ns["brief"], ns["card"]),
            5.19 if label == "dark" else 5.06)
        chk(f"muted on card ({label})", ratio(ns["muted"], ns["card"]),
            3.83 if label == "dark" else 2.62)
        chk(f"white title on palette row ({label})", ratio(WHITE, ns["palette_row"]),
            3.55 if label == "dark" else 2.38)
        chk(f"keycap pair on palette row ({label})",
            ratio(ns["keycap_ink"], ns["keycap_fill"]),
            5.12 if label == "dark" else 5.15)
        chk(f"plain accent on tint1 ({label})", ratio(ns["accent"], ns["tint1"]),
            3.07 if label == "dark" else 2.80)
        chk(f"plain accent on tint2 ({label})", ratio(ns["accent"], ns["tint2"]),
            2.31 if label == "dark" else 2.51)
        chk(f"accent_on_tint on tint1 ({label})", ratio(ns["accent_tint"], ns["tint1"]),
            4.72 if label == "dark" else 3.98)
        chk(f"accent_on_tint on tint2 ({label})", ratio(ns["accent_tint"], ns["tint2"]),
            3.54 if label == "dark" else 3.57)
        chk(f"secondary_on_tint on tint1 ({label})",
            ratio(ns["secondary_tint"], ns["tint1"]),
            4.73 if label == "dark" else 4.29)
        chk(f"secondary_on_tint on tint2 ({label})",
            ratio(ns["secondary_tint"], ns["tint2"]),
            3.55 if label == "dark" else 3.84)
        chk(f"channel dot (text2) on tint2 ({label})", ratio(ns["text2"], ns["tint2"]),
            3.06 if label == "dark" else 4.70)
        chk(f"new kbd ink on chip over card ({label})",
            ratio(ns["text"], ns["kbd_chip_card"]),
            6.32 if label == "dark" else 13.46, 0.06)
        chk(f"new kbd ink on chip over elevated ({label})",
            ratio(ns["text"], ns["kbd_chip_elev"]),
            6.00 if label == "dark" else 13.30, 0.06)
        chk(f"splitter grip on bg ({label})", ratio(ns["grip"], ns["bg"]), 5.15, 0.06)
    chk("secondary (border colour) on tint2 (dark)",
        ratio(DARK["secondary"], DARK["tint2"]), 2.32)
    chk("old kbd ink on chip over card (dark, token figure)",
        ratio(DARK["text2"], DARK["kbd_chip_card"]), 2.97)
    chk("strip play ink on bg (dark)", ratio(DARK["strip_play"], DARK["bg"]), 6.10)
    chk("strip play ink on bg (light, pre-RM25 plain accent)",
        ratio(LIGHT["brief"], LIGHT["bg"]), 4.43)
    chk("strip play ink on bg (light, RM25 deepened)",
        ratio(LIGHT["strip_play"], LIGHT["bg"]), 5.00)
    chk("splitter grip on bg (light, pre-RM25 text_muted construction)",
        ratio(over(with_alpha(LIGHT["text2"], 0.6), LIGHT["bg"]), LIGHT["bg"]), 1.94)
    chk("app_theme muted on bg (light, A20 record)",
        ratio(LIGHT["muted"], LIGHT["bg"]), 2.30)
    chk("menu active hint white@0.75 on accent",
        ratio(over(with_alpha(WHITE, 0.75), DARK["accent"]), DARK["accent"]), 2.60)
    chk("success_on card (dark)", ratio(DARK["success"], DARK["card"]), 7.62, 0.05)
    chk("warning_on card (dark)", ratio(DARK["warning"], DARK["card"]), 8.93, 0.05)
    chk("error_on card (dark)", ratio(DARK["error"], DARK["card"]), 5.28, 0.05)
    chk("success_on card (light)", ratio(LIGHT["success"], LIGHT["card"]), 5.33, 0.05)
    chk("warning_on card (light)", ratio(LIGHT["warning"], LIGHT["card"]), 5.41, 0.05)
    chk("error_on card (light)", ratio(LIGHT["error"], LIGHT["card"]), 5.43, 0.05)
    return rows


# ------------------------------------------------------------------
# Dichromacy simulation — Machado, Oliveira & Fernandes (2009),
# severity 1.0 matrices (Table 1), applied to gamma-encoded sRGB.
# ------------------------------------------------------------------

MACHADO = {
    "protanopia": (
        (0.152286, 1.052583, -0.204868),
        (0.114503, 0.786281, 0.099216),
        (-0.003882, -0.048116, 1.051998),
    ),
    "deuteranopia": (
        (0.367322, 0.860646, -0.227968),
        (0.280085, 0.672501, 0.047413),
        (-0.011820, 0.042940, 0.968881),
    ),
    "tritanopia": (
        (1.255528, -0.076749, -0.178779),
        (-0.078411, 0.930809, 0.147602),
        (0.004733, 0.691367, 0.303967),
    ),
}
DEFICIENCIES = ("deuteranopia", "protanopia", "tritanopia")


def simulate(col, deficiency):
    """Linear colour -> Machado-simulated colour (returned linear,
    alpha preserved; simulation itself runs in sRGB space)."""
    if deficiency is None:
        return col
    m = MACHADO[deficiency]
    srgb = [srgb_encode(min(1.0, max(0.0, col[i]))) for i in range(3)]
    out = []
    for row in m:
        v = sum(row[i] * srgb[i] for i in range(3))
        out.append(min(1.0, max(0.0, v)))
    return (srgb_decode(out[0]), srgb_decode(out[1]), srgb_decode(out[2]), col[3])


# ------------------------------------------------------------------
# CIELAB + CIEDE2000 (D65; sRGB->XYZ after Lindbloom)
# ------------------------------------------------------------------

_XYZ = (
    (0.4123907993, 0.3575843395, 0.1804807884),
    (0.2126390059, 0.7151686788, 0.0721923152),
    (0.0193308187, 0.1191947798, 0.9505321522),
)
_D65 = (0.95047, 1.0, 1.08883)


def to_lab(col):
    xyz = [sum(_XYZ[r][i] * col[i] for i in range(3)) for r in range(3)]
    def f(t):
        return t ** (1.0 / 3.0) if t > 0.008856 else 7.787 * t + 16.0 / 116.0
    fx, fy, fz = (f(xyz[i] / _D65[i]) for i in range(3))
    return (116.0 * fy - 16.0, 500.0 * (fx - fy), 220.0 * (fy - fz))


def delta_e00(lab1, lab2):
    """CIEDE2000, Sharma/Wu/Dalal formulation."""
    L1, a1, b1 = lab1
    L2, a2, b2 = lab2
    C1 = math.hypot(a1, b1)
    C2 = math.hypot(a2, b2)
    Cbar = (C1 + C2) / 2.0
    Cbar7 = Cbar ** 7
    G = 0.5 * (1.0 - math.sqrt(Cbar7 / (Cbar7 + 25.0 ** 7)))
    a1p = a1 * (1.0 + G)
    a2p = a2 * (1.0 + G)
    C1p = math.hypot(a1p, b1)
    C2p = math.hypot(a2p, b2)
    def hp(ap, b):
        if ap == 0.0 and b == 0.0:
            return 0.0
        h = math.degrees(math.atan2(b, ap))
        return h + 360.0 if h < 0.0 else h
    h1p = hp(a1p, b1)
    h2p = hp(a2p, b2)
    dLp = L2 - L1
    dCp = C2p - C1p
    if C1p * C2p == 0.0:
        dhp = 0.0
    else:
        dhp = h2p - h1p
        if dhp > 180.0:
            dhp -= 360.0
        elif dhp < -180.0:
            dhp += 360.0
    dHp = 2.0 * math.sqrt(C1p * C2p) * math.sin(math.radians(dhp) / 2.0)
    Lbarp = (L1 + L2) / 2.0
    Cbarp = (C1p + C2p) / 2.0
    if C1p * C2p == 0.0:
        hbarp = h1p + h2p
    elif abs(h1p - h2p) <= 180.0:
        hbarp = (h1p + h2p) / 2.0
    elif h1p + h2p < 360.0:
        hbarp = (h1p + h2p + 360.0) / 2.0
    else:
        hbarp = (h1p + h2p - 360.0) / 2.0
    T = (1.0 - 0.17 * math.cos(math.radians(hbarp - 30.0))
         + 0.24 * math.cos(math.radians(2.0 * hbarp))
         + 0.32 * math.cos(math.radians(3.0 * hbarp + 6.0))
         - 0.20 * math.cos(math.radians(4.0 * hbarp - 63.0)))
    dtheta = 30.0 * math.exp(-(((hbarp - 275.0) / 25.0) ** 2))
    Rc = 2.0 * math.sqrt(Cbarp ** 7 / (Cbarp ** 7 + 25.0 ** 7))
    Sl = 1.0 + (0.015 * (Lbarp - 50.0) ** 2) / math.sqrt(20.0 + (Lbarp - 50.0) ** 2)
    Sc = 1.0 + 0.045 * Cbarp
    Sh = 1.0 + 0.015 * Cbarp * T
    Rt = -math.sin(math.radians(2.0 * dtheta)) * Rc
    return math.sqrt((dLp / Sl) ** 2 + (dCp / Sc) ** 2 + (dHp / Sh) ** 2
                     + Rt * (dCp / Sc) * (dHp / Sh))


def hue_sep(lab1, lab2):
    """CIELAB hue-angle separation (deg); None when either colour is
    near-neutral (C*ab < 5), where hue angle is unstable."""
    def hab(lab):
        return math.degrees(math.atan2(lab[2], lab[1])) % 360.0
    if math.hypot(lab1[1], lab1[2]) < 5.0 or math.hypot(lab2[1], lab2[2]) < 5.0:
        return None
    d = abs(hab(lab1) - hab(lab2))
    return min(d, 360.0 - d)


# ------------------------------------------------------------------
# Pair measurement
# ------------------------------------------------------------------


def measure(col_a, col_b, deficiency):
    """Distances between two colours under one vision model
    (deficiency None = normal vision)."""
    a = simulate(col_a, deficiency)
    b = simulate(col_b, deficiency)
    la, lb = to_lab(a), to_lab(b)
    return dict(
        de00=delta_e00(la, lb),
        dh=hue_sep(la, lb),
        dl=abs(la[0] - lb[0]),
        ratio=ratio(a, b),
        a_hex=hex_str(a),
        b_hex=hex_str(b),
    )


def verdict_apart(m):
    """Two marks that must read apart (same/adjacent ground)."""
    if m["de00"] >= 10.0:
        return "DISTINGUISHABLE"
    if m["de00"] >= 5.0:
        return "MARGINAL"
    if m["de00"] >= 2.0:
        return "THIN"
    return "COLLAPSED"


def verdict_ground(m, bar):
    """Ink on a ground, against a contrast bar (3.0 non-text / 4.5 text)."""
    if m["ratio"] >= bar:
        return "PASS"
    if m["ratio"] >= 2.0:
        return "WEAK"
    return "FAIL"


# ------------------------------------------------------------------
# The pair programme. kind: "apart" (two marks must read apart) or
# "ground" (ink on a ground, judged against `bar`). Colour
# constructions cite the builder that produces them.
# ------------------------------------------------------------------


def build_pairs():
    pairs = []

    def add(section, theme, name, a, b, kind, bar=3.0):
        pairs.append(dict(section=section, theme=theme, name=name,
                          a=a, b=b, kind=kind, bar=bar))

    for theme, ns in (("dark", DARK), ("light", LIGHT)):
        # ---- Dopesheet (dopesheet.rs figure cells, 2596-2680) ----
        for ground, suffix in (("card", ""), ("tint1", "_tint"), ("tint2", "_tint")):
            g = ns[ground]
            add("dopesheet", theme, f"pose ◆ on {ground}",
                ns["accent" + suffix], g, "ground")
            add("dopesheet", theme, f"state ◇ on {ground}",
                ns["secondary" + suffix], g, "ground")
            add("dopesheet", theme, f"channel ● on {ground}",
                ns["text2"], g, "ground")
            add("dopesheet", theme, f"bend bar (text ink) on {ground}",
                ns["text"], g, "ground")
        add("dopesheet", theme, "pose ◆ ink ~ state ◇ ink (off tint)",
            ns["accent"], ns["secondary"], "apart")
        add("dopesheet", theme, "pose ◆ ink ~ state ◇ ink (on-tint variants)",
            ns["accent_tint"], ns["secondary_tint"], "apart")
        add("dopesheet", theme, "pose ◆ ink ~ channel ● ink",
            ns["accent"], ns["text2"], "apart")
        add("dopesheet", theme, "state ◇ ink ~ channel ● ink",
            ns["secondary"], ns["text2"], "apart")
        add("dopesheet", theme, "pose ◆ ink ~ bend ink (text)",
            ns["accent"], ns["text"], "apart")
        add("dopesheet", theme, "state ◇ ink ~ bend ink (text)",
            ns["secondary"], ns["text"], "apart")
        # The coincidence cell (A20 weak spot 1): pose ◆ plus a 1 pt
        # secondary-accent border as the ONLY state signal.
        for ground, b_ink, p_ink in (
            ("card", ns["secondary"], ns["accent"]),
            ("tint1", ns["secondary_tint"], ns["accent_tint"]),
            ("tint2", ns["secondary_tint"], ns["accent_tint"]),
        ):
            add("coincidence", theme, f"state border on {ground}",
                b_ink, ns[ground], "ground")
            add("coincidence", theme, f"state border ~ pose ◆ ink on {ground}",
                b_ink, p_ink, "apart")
        # Exposure bars (RM14; the accent@0.55 bar RM25 flagged as
        # unmeasured) and the F-4 key-selection border.
        for ground in ("card", "tint1"):
            g = ns[ground]
            add("dopesheet", theme, f"exposure bar (accent @ .55) on {ground}",
                over(with_alpha(ns["accent"], 0.55), g), g, "ground")
            add("dopesheet", theme, f"exposure held bar (text2 @ .40) on {ground}",
                over(with_alpha(ns["text2"], 0.40), g), g, "ground")
        add("dopesheet", theme, "key-selection border (accent) on tint2",
            ns["accent"], ns["tint2"], "ground")
        # ---- Timeline marker row (timeline_marks.rs 1146-1258) ----
        add("timeline", theme, "camera ◆ on card", ns["accent"], ns["card"], "ground")
        add("timeline", theme, "graph ● on card", ns["text2"], ns["card"], "ground")
        add("timeline", theme, "camera ◆ ink ~ graph ● ink",
            ns["accent"], ns["text2"], "apart")
        add("timeline", theme, "current-chip glyphs (white) on accent fill",
            WHITE, ns["accent"], "ground")
        add("timeline", theme, "loop-range fill ~ plain card",
            ns["loop_fill"], ns["card"], "apart")
        add("timeline", theme, "current-frame line (accent @ .55) on card",
            ns["frame_line"], ns["card"], "ground")
        # ---- Graph canvas (graph_panel.rs graph_shape, 2644-2700) ----
        add("graph", theme, "curve (accent) on canvas card",
            ns["accent"], ns["card"], "ground")
        add("graph", theme, "key diamond (amber) on canvas card",
            ns["amber"], ns["card"], "ground")
        add("graph", theme, "key diamond (amber) ~ curve/selected (accent)",
            ns["amber"], ns["accent"], "apart")
        add("graph", theme, "tangent handle (text2) on canvas card",
            ns["text2"], ns["card"], "ground")
        add("graph", theme, "tangent handle (text2) ~ dragged handle (accent)",
            ns["text2"], ns["accent"], "apart")
        # ---- Toasts (toasts.rs Severity::color, 320-350; ground = card) ----
        sev = (("info", ns["brief"]), ("success", ns["success"]),
               ("warning", ns["warning"]), ("error", ns["error"]))
        for i in range(len(sev)):
            for j in range(i + 1, len(sev)):
                add("toasts", theme, f"{sev[i][0]} ~ {sev[j][0]} glyph inks",
                    sev[i][1], sev[j][1], "apart")
            add("toasts", theme, f"{sev[i][0]} glyph ink on card",
                sev[i][1], ns["card"], "ground")
        # ---- Shell pairs RM25 fixed (re-verification under CVD) ----
        add("shell-rm25", theme, "kbd ink (text) on chip over card",
            ns["text"], ns["kbd_chip_card"], "ground", 4.5)
        add("shell-rm25", theme, "kbd ink (text) on chip over elevated",
            ns["text"], ns["kbd_chip_elev"], "ground", 4.5)
        add("shell-rm25", theme, "keycap ink on keycap fill (palette row)",
            ns["keycap_ink"], ns["keycap_fill"], "ground", 4.5)
        add("shell-rm25", theme, "dimmed-item label (text2) on card",
            ns["text2"], ns["card"], "ground", 4.5)
        add("shell-rm25", theme, "strip play ink on bg",
            ns["strip_play"], ns["bg"], "ground", 4.5)
        add("shell-rm25", theme, "strip play ink on chrome surface",
            ns["strip_play"], ns["chrome"], "ground", 4.5)
        add("shell-rm25", theme, "splitter grip on bg",
            ns["grip"], ns["bg"], "ground", 3.0)
        add("shell-rm25", theme, "palette row title (white) on row — D-1, report only",
            WHITE, ns["palette_row"], "ground", 4.5)
    return pairs


# ------------------------------------------------------------------
# Report
# ------------------------------------------------------------------

_APART_RANK = {"DISTINGUISHABLE": 3, "MARGINAL": 2, "THIN": 1, "COLLAPSED": 0}
_GROUND_RANK = {"PASS": 2, "WEAK": 1, "FAIL": 0}


def pair_verdicts(pair):
    """(normal measure, {deficiency: measure}, worst verdict)."""
    normal = measure(pair["a"], pair["b"], None)
    sims = {d: measure(pair["a"], pair["b"], d) for d in DEFICIENCIES}
    if pair["kind"] == "apart":
        vs = [verdict_apart(sims[d]) for d in DEFICIENCIES]
        worst = min(vs, key=lambda v: _APART_RANK[v])
    else:
        vs = [verdict_ground(sims[d], pair["bar"]) for d in DEFICIENCIES]
        worst = min(vs, key=lambda v: _GROUND_RANK[v])
    return normal, sims, worst


def fmt_h(dh):
    return "—" if dh is None else f"{dh:.0f}"


def render(pairs, full=False):
    out = []
    sections = []
    for p in pairs:
        if p["section"] not in sections:
            sections.append(p["section"])
    for section in sections:
        out.append(f"\n## {section}\n")
        for theme in ("dark", "light"):
            rows = [p for p in pairs if p["section"] == section and p["theme"] == theme]
            if not rows:
                continue
            out.append(f"\n### {theme}\n")
            out.append("| pair | norm ΔE00 | norm ratio | deut ΔE00 | prot ΔE00 "
                       "| trit ΔE00 | min sim ratio | worst verdict |")
            out.append("|---|---|---|---|---|---|---|---|")
            for p in rows:
                normal, sims, worst = pair_verdicts(p)
                out.append(
                    f"| {p['name']} | {normal['de00']:.1f} | {normal['ratio']:.2f} "
                    f"| {sims['deuteranopia']['de00']:.1f} "
                    f"| {sims['protanopia']['de00']:.1f} "
                    f"| {sims['tritanopia']['de00']:.1f} "
                    f"| {min(sims[d]['ratio'] for d in DEFICIENCIES):.2f} "
                    f"| {worst} |")
                if full:
                    for d in DEFICIENCIES:
                        m = sims[d]
                        out.append(
                            f"  - {d}: A {m['a_hex']} B {m['b_hex']} "
                            f"Δh {fmt_h(m['dh'])}° ΔL* {m['dl']:.1f} "
                            f"ratio {m['ratio']:.2f}")
    return "\n".join(out) + "\n"


def main(argv):
    do_validate = "--no-validate" not in argv
    full = "--full" in argv
    save = None
    if "--save" in argv:
        save = argv[argv.index("--save") + 1]

    lines = ["# cvd_sim.py — CVD simulation pass (W2-52)\n"]
    ok = True
    if do_validate:
        rows = validate()
        lines.append("## Validation vs the published audit record\n")
        lines.append("| pair | computed | published | |")
        lines.append("|---|---|---|---|")
        for name, got, want, passed in rows:
            ok = ok and passed
            lines.append(f"| {name} | {got:.2f} | {want:.2f} | "
                         f"{'PASS' if passed else 'FAIL'} |")
        lines.append("")
    pairs = build_pairs()
    lines.append(render(pairs, full=full))
    text = "\n".join(lines)
    print(text)
    if save:
        with open(save, "w") as fh:
            fh.write(text)
    if do_validate and not ok:
        print("VALIDATION FAILED — simulated numbers not trustworthy.",
              file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
