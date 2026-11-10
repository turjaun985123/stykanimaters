#!/usr/bin/env python3
"""contrast_census.py — full contrast re-census of the StykAnimate-RS
shell (Roadmap Wave 2, seat W2-60). MEASUREMENT ONLY: this script
reads no source files at runtime and changes no values; every colour
below is hand-copied from the source of record cited in its comment,
and the arithmetic is the validated machinery of scripts/cvd_sim.py
(W2-52), imported, not modified. Before any census number is
produced, cvd_sim's validation suite must reproduce the published
audit record (48 figures, +/-0.03) or the run aborts with exit 1.

Programme:
  A. Token text pairs — every text-role ink the design tokens
     produce, on every ground that ink can render on, both themes,
     classified against the WCAG 4.5:1 text bar.
  B. Token glyph / UI-component pairs — marks, borders, tracks,
     knobs, washes — against the 3:1 non-text bar.
  C. The D-1 accent evidence rows, recomputed fresh against the
     current tokens, plus the three standing candidates (#2072D5,
     #1C6FD2, #556FCD) and the pack's boundary value (#2576D9).
     REPORT ONLY — D-1 is the user's open decision; nothing here
     applies or recommends a change.
  D. Data palettes — colours that come from project/panel DATA, not
     theme tokens (subtitle / text / particle / figure / segment /
     onion palettes, the stage dialect), on their designed grounds.
  E. CVD extension (Machado 2009 severity 1.0, via cvd_sim) for the
     Wave-2 mark pairs that interact with the mark system: limit
     arcs, onion past/future discrimination, stage marks, and the
     data palettes' worst-case apart-pairs.

WCAG classification: TEXT = 4.5:1 (normal text; this app's labels
are 11.5-13 pt regular/medium, none qualifies as "large text"),
GLYPH/UI = 3:1 (WCAG 1.4.11 non-text), DATA rows are classified
against the bar for their role and flagged DATA so a data miss is
never read as a token defect.

Usage: python3 scripts/contrast_census.py [--save OUT.md]
Exit status: 0 when cvd_sim validation passes, 1 otherwise.
"""

import importlib.util
import os
import sys

_HERE = os.path.dirname(os.path.abspath(__file__))
_spec = importlib.util.spec_from_file_location(
    "cvd_sim", os.path.join(_HERE, "cvd_sim.py"))
cvd = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(cvd)

# --- machinery aliases (all from cvd_sim, validated there) ---------
hex_lin = cvd.hex_lin
oklch = cvd.oklch
C = cvd.C
with_alpha = cvd.with_alpha
mix = cvd.mix
over = cvd.over
ratio = cvd.ratio
hex_str = cvd.hex_str
WHITE = cvd.WHITE
BLACK = cvd.BLACK


def u32(v):
    """0xRRGGBB (the app's u32 dialect) -> linear colour."""
    return C(hex_lin(f"#{v:06X}"))


# --- upstream tok::theme mechanism fields (rune-src tok.rs:168-210)
# app_theme.rs retains these verbatim; only the fields app_theme
# overrides come from its own constants.
def upstream(dark):
    if dark:
        return dict(
            control_track=oklch(1.0, 0.0, 0.0, 0.05),
            control_off=oklch(1.0, 0.0, 0.0, 0.085),
            indicator=oklch(0.43, 0.006, 265.0),
            knob=oklch(0.985, 0.0, 0.0),
            hover=oklch(1.0, 0.0, 0.0, 0.016),
            press=oklch(1.0, 0.0, 0.0, 0.03),
        )
    return dict(
        control_track=oklch(0.2, 0.01, 265.0, 0.13),
        control_off=oklch(0.2, 0.01, 265.0, 0.24),
        indicator=oklch(1.0, 0.0, 0.0),
        knob=oklch(1.0, 0.0, 0.0),
        hover=oklch(0.2, 0.01, 265.0, 0.07),
        press=oklch(0.2, 0.01, 265.0, 0.12),
    )


def build_ns(dark):
    """cvd_sim's theme namespace (app_theme tokens + derived
    surfaces), extended with the remaining token constructions the
    census needs. Sources: app_theme.rs consts/fns; design.rs
    (text_muted 216, selected_fill 200); status_strip.rs
    strip_colors; waveform_view.rs bar_colors/bracket/playhead;
    home recovery banner tint alpha 0.18 (app_theme doc)."""
    ns = dict(cvd.theme_ns(dark))
    up = upstream(dark)
    ns.update(up)
    card = ns["card"]
    accent = ns["accent"]
    # Borders/hairlines as rendered (composited over their ground).
    if dark:
        ns["card_border"] = C(hex_lin("#303846"))            # solid
        sep = with_alpha(C(hex_lin("#303846")), 0.55)        # DARK_SEPARATOR_ALPHA
    else:
        ns["card_border"] = over(with_alpha(C(hex_lin("#2A3242")), 0.14), ns["bg"])
        sep = with_alpha(C(hex_lin("#2A3242")), 0.19)        # LIGHT_SEPARATOR_ALPHA
    ns["separator_on_card"] = over(sep, card)
    ns["card_border_on_card"] = over(ns["card_border"], card) if not dark else ns["card_border"]
    # Derived surfaces.
    ns["tint018"] = mix(card, accent, 0.18)                  # recovery banner
    ns["palette_row_elev"] = over(with_alpha(accent, 0.85), ns["elevated"])
    ns["track_on_card"] = over(up["control_track"], card)    # slider track ground
    ns["muted_kit"] = None                                   # per-ground below
    # Waveform (waveform_view.rs): bars/brackets/ticks/playhead.
    brief = ns["brief"]
    ns["wf_outer"] = over(with_alpha(brief, 0.5), card)
    ns["wf_bracket"] = over(with_alpha(brief, 0.75), card)
    ns["wf_outer_muted"] = over(with_alpha(brief, 0.5 * 0.35), card)
    ns["wf_inner_muted"] = over(with_alpha(brief, 0.35), card)
    ns["wf_ticks"] = over(with_alpha(brief, 0.35), card)     # ALL_TICKS_ALPHA
    return ns


DARK = build_ns(True)
LIGHT = build_ns(False)
NS = {"dark": DARK, "light": LIGHT}

BAR_TEXT = 4.5
BAR_UI = 3.0


def verdict(r, bar):
    return "PASS" if r >= bar else "MISS"


def eff_muted(ns, ground):
    """design::text_muted = text_secondary @ 0.6 alpha, composited
    over the ground it renders on (design.rs:216)."""
    return over(with_alpha(ns["text2"], 0.6), ground)


# ------------------------------------------------------------------
# A. Token text pairs
# ------------------------------------------------------------------

def section_a():
    rows = []
    # (ink key, label, grounds it can render on)
    matrix = [
        ("text", "primary text", ["bg", "card", "chrome", "elevated",
                                  "tint1", "tint2", "tint018", "loop_fill",
                                  "kbd_chip_card", "kbd_chip_elev"]),
        ("text2", "secondary text", ["bg", "card", "chrome", "elevated",
                                     "tint1", "tint2", "tint018", "loop_fill"]),
        ("brief", "brief accent as ink (clips marker / strip / dial)",
         ["bg", "card", "chrome", "elevated", "tint1"]),
        ("secondary", "secondary accent as ink (no text consumer)",
         ["bg", "card", "elevated"]),
        ("success", "success_on (recap text / severity)", ["card", "bg", "elevated", "chrome"]),
        ("warning", "warning_on (recap text / severity)", ["card", "bg", "elevated", "chrome"]),
        ("error", "error_on (recap text / severity)", ["card", "bg", "elevated", "chrome"]),
        ("strip_play", "status-strip playing ink", ["bg", "chrome"]),
    ]
    for theme in ("dark", "light"):
        ns = NS[theme]
        for key, label, grounds in matrix:
            for g in grounds:
                r = ratio(ns[key], ns[g])
                rows.append((theme, f"{label} on {g}", hex_str(ns[key]),
                             hex_str(ns[g]), r, verdict(r, BAR_TEXT)))
        # app_theme::muted_text — glyph duty only; censused against
        # the text bar to keep the record honest (see findings).
        for g in ("card", "bg", "elevated", "chrome"):
            r = ratio(ns["muted"], ns[g])
            rows.append((theme, f"muted_text (GLYPHS ONLY) on {g}",
                         hex_str(ns["muted"]), hex_str(ns[g]), r,
                         verdict(r, BAR_TEXT) + " (glyph role)"))
        # design::text_muted, composited — glyph duty (splitter grip
        # handled in B; graph glyph uses app muted above).
        for g in ("card", "bg"):
            ink = eff_muted(ns, ns[g])
            r = ratio(ink, ns[g])
            rows.append((theme, f"design::text_muted composite on {g}",
                         hex_str(ink), hex_str(ns[g]), r,
                         verdict(r, BAR_TEXT) + " (glyph role)"))
    # White-ink text pairs (fills).
    for theme in ("dark", "light"):
        ns = NS[theme]
        rows.append((theme, "white on upstream accent fill — D-1",
                     "#FFFFFF", hex_str(ns["accent"]),
                     ratio(WHITE, ns["accent"]), "D-1 (see §C)"))
        rows.append((theme, "white on palette row (accent @ .85 over card)",
                     "#FFFFFF", hex_str(ns["palette_row"]),
                     ratio(WHITE, ns["palette_row"]), "D-1 family (see §C)"))
        rows.append((theme, "white on palette row (accent @ .85 over elevated)",
                     "#FFFFFF", hex_str(ns["palette_row_elev"]),
                     ratio(WHITE, ns["palette_row_elev"]), "D-1 family (see §C)"))
        hint = over(with_alpha(WHITE, 0.75), ns["accent"])
        rows.append((theme, "menu active hint (white @ .75) on accent",
                     hex_str(hint), hex_str(ns["accent"]),
                     ratio(hint, ns["accent"]), verdict(ratio(hint, ns["accent"]), BAR_TEXT)))
        rows.append((theme, "keycap ink on keycap fill (palette row)",
                     hex_str(ns["keycap_ink"]), hex_str(ns["keycap_fill"]),
                     ratio(ns["keycap_ink"], ns["keycap_fill"]),
                     verdict(ratio(ns["keycap_ink"], ns["keycap_fill"]), BAR_TEXT)))
    for name, v in (("rest", 0xC93A44), ("hover", 0xB2323E), ("press", 0x9C2934)):
        fill = u32(v)
        rows.append(("both", f"white on danger fill {name} (chrome_view)",
                     "#FFFFFF", f"#{v:06X}", ratio(WHITE, fill),
                     verdict(ratio(WHITE, fill), BAR_TEXT)))
    return rows


# ------------------------------------------------------------------
# B. Token glyph / UI-component pairs (3:1)
# ------------------------------------------------------------------

def section_b():
    rows = []

    def add(theme, label, ink, ground, note=""):
        r = ratio(ink, ground)
        rows.append((theme, label, hex_str(ink), hex_str(ground), r,
                     verdict(r, BAR_UI) + (f" ({note})" if note else "")))

    for theme in ("dark", "light"):
        ns = NS[theme]
        for g in ("card", "bg", "chrome", "elevated", "tint1", "tint2"):
            add(theme, f"upstream accent glyph on {g}", ns["accent"], ns[g])
        for g in ("tint1", "tint2"):
            add(theme, f"accent_on_tint glyph on {g}", ns["accent_tint"], ns[g])
            add(theme, f"secondary_on_tint glyph on {g}", ns["secondary_tint"], ns[g])
        for g in ("card", "bg", "tint1", "tint2"):
            add(theme, f"channel dot (text2) on {g}", ns["text2"], ns[g])
        add(theme, "graph key diamond (amber #FFC65D) on card", ns["amber"], ns["card"])
        add(theme, "exposure bar (accent @ .55) on card",
            over(with_alpha(ns["accent"], 0.55), ns["card"]), ns["card"])
        add(theme, "exposure held bar (text2 @ .40) on card",
            over(with_alpha(ns["text2"], 0.40), ns["card"]), ns["card"])
        add(theme, "current-frame line (accent @ .55) on card", ns["frame_line"], ns["card"])
        add(theme, "card_border vs bg (card edge)", ns["card_border"], ns["bg"])
        add(theme, "card_border vs card (card edge)", ns["card_border_on_card"], ns["card"])
        add(theme, "separator composite vs card (hairline)", ns["separator_on_card"], ns["card"])
        add(theme, "control_track composite vs card (slider track)", ns["track_on_card"], ns["card"])
        add(theme, "control_off composite vs card (chip/track ground)", ns["kbd_chip_card"], ns["card"])
        add(theme, "indicator vs card", ns["indicator"], ns["card"])
        add(theme, "indicator vs bg", ns["indicator"], ns["bg"])
        add(theme, "toggle knob vs accent track (on)", ns["knob"], ns["accent"])
        add(theme, "toggle knob vs off track (control_off on card)", ns["knob"], ns["kbd_chip_card"])
        add(theme, "slider progress (accent) vs track ground", ns["accent"], ns["track_on_card"])
        add(theme, "splitter grip vs bg", ns["grip"], ns["bg"])
        add(theme, "waveform outer bar (brief @ .50) vs card", ns["wf_outer"], ns["card"])
        add(theme, "waveform inner bar (brief opaque) vs card", ns["brief"], ns["card"])
        add(theme, "waveform bracket (brief @ .75) vs card", ns["wf_bracket"], ns["card"])
        add(theme, "waveform muted outer (brief @ .175) vs card", ns["wf_outer_muted"], ns["card"])
        add(theme, "waveform muted inner (brief @ .35) vs card", ns["wf_inner_muted"], ns["card"])
        add(theme, "waveform ticks (brief @ .35) vs card", ns["wf_ticks"], ns["card"])
        add(theme, "waveform playhead (warning_on) vs card", ns["warning"], ns["card"])
        add(theme, "dirty dot (brief accent) vs chrome", ns["brief"], ns["chrome"])
        add(theme, "swatch selection ring (accent) vs card", ns["accent"], ns["card"])
    return rows


# ------------------------------------------------------------------
# C. D-1 evidence rows — REPORT ONLY
# ------------------------------------------------------------------

CANDIDATES = [
    ("#2072D5", "candidate L 0.56 (headroom)"),
    ("#1C6FD2", "candidate L 0.55 (headroom)"),
    ("#556FCD", "candidate: ACCENT_DARK scaled k=0.611"),
    ("#2576D9", "boundary L~0.571 (exactly 4.50, pack value)"),
]

def section_c():
    rows = []
    for theme in ("dark", "light"):
        ns = NS[theme]
        rows.append((theme, "CURRENT: white on upstream accent #4392F7",
                     "#FFFFFF", hex_str(ns["accent"]), ratio(WHITE, ns["accent"])))
        rows.append((theme, "CURRENT: white on palette row @ .85 over card",
                     "#FFFFFF", hex_str(ns["palette_row"]), ratio(WHITE, ns["palette_row"])))
        rows.append((theme, "CURRENT: white on palette row @ .85 over elevated",
                     "#FFFFFF", hex_str(ns["palette_row_elev"]), ratio(WHITE, ns["palette_row_elev"])))
    for hexv, label in CANDIDATES:
        cand = C(hex_lin(hexv))
        rows.append(("both", f"{label}: white on {hexv} (opaque fill)",
                     "#FFFFFF", hexv, ratio(WHITE, cand)))
        for theme in ("dark", "light"):
            ns = NS[theme]
            row_c = over(with_alpha(cand, 0.85), ns["card"])
            rows.append((theme, f"{label}: white on {hexv} @ .85 over card (palette row)",
                         "#FFFFFF", hex_str(row_c), ratio(WHITE, row_c)))
            rows.append((theme, f"{label}: {hexv} as ink on card",
                         hexv, hex_str(ns["card"]), ratio(cand, ns["card"])))
            rows.append((theme, f"{label}: {hexv} as ink on bg",
                         hexv, hex_str(ns["bg"]), ratio(cand, ns["bg"])))
    # No-consumer hypotheticals, for the complete accent picture.
    for theme in ("dark", "light"):
        ns = NS[theme]
        rows.append((theme, "white on brief accent (NO white-text consumer)",
                     "#FFFFFF", hex_str(ns["brief"]), ratio(WHITE, ns["brief"])))
        rows.append((theme, "white on secondary accent (NO white-text consumer)",
                     "#FFFFFF", hex_str(ns["secondary"]), ratio(WHITE, ns["secondary"])))
    return rows


# ------------------------------------------------------------------
# D. Data palettes
# ------------------------------------------------------------------

SUBTITLE_PALETTE = [0xFFFFFF, 0xFFD94D, 0xFF9F43, 0xFF5D5D,
                    0x7BE495, 0x57C7E3, 0xA184FF, 0x000000]
TEXT_PALETTE = [0xFF5D5D, 0xFF9F43, 0xFFC65D, 0x7BE495,
                0x57C7E3, 0xA184FF, 0xFFFFFF, 0x000000]  # == PARTICLE_PALETTE
FIGURE_PALETTE = [0xE8E8EC, 0xFF5D5D, 0x5DB2FF, 0x7BE495, 0xFFC65D,
                  0xC792FF, 0x232830, 0xF16F78, 0xFF9F43, 0xB7E26B,
                  0x2ED573, 0x67C9B3, 0x2686E8, 0x8C7BFF, 0xFF6BCE,
                  0x8A5A2B]
SEGMENT_PALETTE = [0xFF5D5D, 0x5DB2FF, 0x7BE495, 0xFFC65D, 0xC792FF, 0x232830]
ONION_TINT_PALETTE = [0xFF5D5D, 0xFFC65D, 0x7BE495, 0x5DB2FF,
                      0xC792FF, 0xFF6BCE, 0xE8E8EC, 0x232830]
STAGE_BG_DEFAULT = u32(0x232830)   # Project::background default (data)
SELECTION_ACCENT = u32(0x6D8CFF)   # stage dialect (stage_view.rs:2548)
STORED_GUIDE = u32(0x2FB9D8)       # stage_view.rs:5723
LIMIT_ALLOWED = u32(0x7BE495)
LIMIT_FORBIDDEN = u32(0xFF5D5D)


def section_d():
    rows = []

    def add(palette, name, v, ground, gname, bar, role):
        ink = u32(v)
        r = ratio(ink, ground)
        rows.append((name, f"#{v:06X}", gname, hex_str(ground), r,
                     verdict(r, bar), role))

    for v in SUBTITLE_PALETTE:
        add("subtitle", "subtitle palette", v, BLACK, "black (default box_color)",
            BAR_TEXT, "DATA text")
        add("subtitle", "subtitle palette", v, WHITE, "white (light video frame)",
            BAR_TEXT, "DATA text")
    for v in TEXT_PALETTE:
        add("text/particle", "text+particle palette", v, STAGE_BG_DEFAULT,
            "default stage bg #232830", BAR_UI, "DATA mark")
        add("text/particle", "text+particle palette", v, WHITE,
            "white stage bg (project data)", BAR_UI, "DATA mark")
    for v in FIGURE_PALETTE:
        add("figure", "figure palette", v, STAGE_BG_DEFAULT,
            "default stage bg #232830", BAR_UI, "DATA mark")
        add("figure", "figure palette", v, WHITE,
            "white stage bg (project data)", BAR_UI, "DATA mark")
    for v in SEGMENT_PALETTE:
        add("segment", "segment palette (subset of figure families)", v,
            STAGE_BG_DEFAULT, "default stage bg #232830", BAR_UI, "DATA mark")
    for v in ONION_TINT_PALETTE:
        add("onion", "onion tint palette (full strength)", v, STAGE_BG_DEFAULT,
            "default stage bg #232830", BAR_UI, "DATA mark")
        for a, aname in ((0x4D / 255.0, "ghost near @ .302"), (0x26 / 255.0, "ghost far @ .149")):
            ghost = over(with_alpha(u32(v), a), STAGE_BG_DEFAULT)
            r = ratio(ghost, STAGE_BG_DEFAULT)
            rows.append(("onion", f"#{v:06X} {aname}", "default stage bg #232830",
                         hex_str(STAGE_BG_DEFAULT), r, verdict(r, BAR_UI),
                         "DATA mark (faint by design)"))
    # Stage dialect marks on the default stage ground.
    for label, ink in (("selection/gizmo accent #6D8CFF", SELECTION_ACCENT),
                       ("stored guide #2FB9D8", STORED_GUIDE),
                       ("limit arc allowed #7BE495", LIMIT_ALLOWED),
                       ("limit arc forbidden #FF5D5D", LIMIT_FORBIDDEN),
                       ("joint marker selected #FFC65D", u32(0xFFC65D)),
                       ("joint marker unselected #FF5D5D", u32(0xFF5D5D))):
        for ground, gname in ((STAGE_BG_DEFAULT, "default stage bg #232830"),
                              (WHITE, "white stage bg (project data)")):
            r = ratio(ink, ground)
            rows.append(("stage", label, gname, hex_str(ground), r,
                         verdict(r, BAR_UI), "DATA-ground mark"))
    # Frustum/grid guides: white @ 0.22 over the stage (stage_view.rs:3037).
    g = over(with_alpha(WHITE, 0.22), STAGE_BG_DEFAULT)
    rows.append(("stage", "frustum guide (white @ .22)", "default stage bg #232830",
                 hex_str(STAGE_BG_DEFAULT), ratio(g, STAGE_BG_DEFAULT),
                 verdict(ratio(g, STAGE_BG_DEFAULT), BAR_UI), "DATA-ground mark"))
    # Stage HUD: drag plate = theme card @ .88 over the stage; ink
    # @ .95 on the plate. Coordinate readout: ink @ .80, NO plate,
    # directly over the stage ground (stage_view.rs:1700, 1760, 1785).
    for theme, ink_v in (("dark", 0xF2F4F8), ("light", 0x14161A)):
        ns = NS[theme]
        ink = u32(ink_v)
        plate = over(with_alpha(ns["card"], 0.88), STAGE_BG_DEFAULT)
        on_plate = over(with_alpha(ink, 0.95), plate)
        rows.append(("stage", f"HUD ink ({theme} #{ink_v:06X} @ .95) on drag plate",
                     hex_str(plate), hex_str(plate), ratio(on_plate, plate),
                     verdict(ratio(on_plate, plate), BAR_TEXT), "DATA-ground text"))
        bare = over(with_alpha(ink, 0.80), STAGE_BG_DEFAULT)
        rows.append(("stage", f"coord readout ink ({theme} #{ink_v:06X} @ .80), no plate",
                     "default stage bg #232830", hex_str(STAGE_BG_DEFAULT),
                     ratio(bare, STAGE_BG_DEFAULT),
                     verdict(ratio(bare, STAGE_BG_DEFAULT), BAR_TEXT), "DATA-ground text"))
    # Title-card default pair (SubtitleStyle::default via title_cards.rs).
    rows.append(("title card", "#FFFFFF (default text_color)", "black (default plate)",
                 "#000000", ratio(WHITE, BLACK), verdict(ratio(WHITE, BLACK), BAR_TEXT),
                 "DATA text"))
    return rows


# ------------------------------------------------------------------
# E. CVD extension for Wave-2 mark pairs
# ------------------------------------------------------------------

def section_e():
    out = []

    def apart(label, a, b):
        ms = {d: cvd.measure(a, b, d) for d in cvd.DEFICIENCIES}
        worst = min((cvd.verdict_apart(ms[d]) for d in cvd.DEFICIENCIES),
                    key=lambda v: {"COLLAPSED": 0, "THIN": 1,
                                   "MARGINAL": 2, "DISTINGUISHABLE": 3}[v])
        out.append((label,
                    {d: ms[d]["de00"] for d in cvd.DEFICIENCIES}, worst))

    def ground(label, ink, g):
        ms = {d: cvd.measure(ink, g, d) for d in cvd.DEFICIENCIES}
        worst = min((cvd.verdict_ground(ms[d], BAR_UI) for d in cvd.DEFICIENCIES),
                    key=lambda v: {"FAIL": 0, "WEAK": 1, "PASS": 2}[v])
        out.append((label,
                    {d: ms[d]["ratio"] for d in cvd.DEFICIENCIES}, worst))

    apart("limit allowed #7BE495 ~ limit forbidden #FF5D5D",
          LIMIT_ALLOWED, LIMIT_FORBIDDEN)
    ground("limit allowed on default stage bg", LIMIT_ALLOWED, STAGE_BG_DEFAULT)
    ground("limit forbidden on default stage bg", LIMIT_FORBIDDEN, STAGE_BG_DEFAULT)
    apart("figure green #7BE495 ~ future-onion green #7BE495 (identical)",
          u32(0x7BE495), u32(0x7BE495))
    apart("selection accent #6D8CFF ~ stored guide #2FB9D8",
          SELECTION_ACCENT, STORED_GUIDE)
    apart("selection accent #6D8CFF ~ limit allowed #7BE495",
          SELECTION_ACCENT, LIMIT_ALLOWED)
    apart("selection accent #6D8CFF ~ limit forbidden #FF5D5D",
          SELECTION_ACCENT, LIMIT_FORBIDDEN)
    apart("stored guide #2FB9D8 ~ limit allowed #7BE495",
          STORED_GUIDE, LIMIT_ALLOWED)
    # Worst apart-pairs inside each data palette (chromatic entries).
    for name, pal in (("subtitle", SUBTITLE_PALETTE), ("figure", FIGURE_PALETTE),
                      ("onion", ONION_TINT_PALETTE)):
        cols = [v for v in pal if v not in (0xFFFFFF, 0x000000, 0x232830, 0xE8E8EC)]
        for d in cvd.DEFICIENCIES:
            worst = None
            for i in range(len(cols)):
                for j in range(i + 1, len(cols)):
                    m = cvd.measure(u32(cols[i]), u32(cols[j]), d)
                    if worst is None or m["de00"] < worst[0]:
                        worst = (m["de00"], cols[i], cols[j])
            out.append((f"{name} palette worst apart pair under {d}",
                        {d: worst[0]}, f"#{worst[1]:06X} ~ #{worst[2]:06X} "
                        f"({cvd.verdict_apart({'de00': worst[0]})})"))
    return out


# ------------------------------------------------------------------

def table(rows, header):
    out = [header, "|" + "---|" * (len(rows[0]))]
    for r in rows:
        cells = [f"{c:.2f}" if isinstance(c, float) else str(c) for c in r]
        out.append("| " + " | ".join(cells) + " |")
    return "\n".join(out)


def main(argv):
    rows = cvd.validate()
    bad = [r for r in rows if not r[3]]
    if bad:
        print("CVD VALIDATION FAILED — census numbers not trustworthy.",
              file=sys.stderr)
        for r in bad:
            print("  ", r, file=sys.stderr)
        return 1
    lines = ["# contrast_census.py — full contrast re-census (W2-60)\n",
             f"Validation: cvd_sim published-record suite "
             f"{len(rows) - len(bad)}/{len(rows)} PASS — census proceeds.\n"]
    lines.append("\n## A. Token text pairs (bar 4.5:1)\n")
    lines.append(table(section_a(),
                       "| theme | pair | ink | ground | ratio | verdict |\n"))
    lines.append("\n## B. Token glyph / UI pairs (bar 3:1)\n")
    lines.append(table(section_b(),
                       "| theme | pair | ink | ground | ratio | verdict |\n"))
    lines.append("\n## C. D-1 evidence rows (report only)\n")
    lines.append(table(section_c(),
                       "| theme | pair | ink | ground | ratio |\n"))
    lines.append("\n## D. Data palettes\n")
    lines.append(table(section_d(),
                       "| palette | colour | ground | ground hex | ratio | verdict | role |\n"))
    lines.append("\n## E. CVD extension (Machado severity 1.0)\n")
    erows = []
    for label, vals, worst in section_e():
        cells = " / ".join(f"{k[:4]} {v:.2f}" for k, v in vals.items())
        erows.append((label, cells, worst))
    lines.append(table(erows, "| pair | simulated values | worst |\n"))
    text = "\n".join(lines) + "\n"
    print(text)
    if "--save" in argv:
        with open(argv[argv.index("--save") + 1], "w") as fh:
            fh.write(text)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
