#!/usr/bin/env python3
"""Per-language i18n coverage instrument (Roadmap PL-9, seat RM27).

Measures, from the real tables in `src/i18n.rs`, how much of each
language is genuinely translated versus still carrying the English
text as a placeholder (the table convention since Wave 14: untranslated
blocks are inserted with the English value copied verbatim, so a value
byte-identical to the English value IS the placeholder signature —
the same measure AC15/AB16 used for the tier audit).

For every language it reports: table size, values identical to EN,
translated count and percentage, and the tier that percentage falls
in (the bands `lang_tier` in `src/rune_sec/settings_popover.rs`
renders in the language picker). For a language with a base language
(today only es-419 -> es) it additionally reports the EFFECTIVE
coverage under the PL-9 fallback chain (a placeholder cell resolves
to the base table's value before English — IA-RM27-1) and how many
cells the chain changes.

Checks (exit 1 on any failure):
  * all 28 tables present, key sequences identical to EN
    position-for-position, no duplicates, no empty values;
  * no value equal to its own key (the `t()` last-resort leak);
  * `{slot}` multisets match EN per key per language;
  * tier drift: the `lang_tier` match in settings_popover.rs agrees
    with the tiers measured here (es-419 is tiered by its EFFECTIVE
    coverage, since the chain and the picker ship together);
  * baseline regression: no language's translated count falls below
    BASELINE_TRANSLATED (ratchet it up, by hand, when a translation
    tranche lands — never down).

Usage: i18n_coverage.py [--json] [--self-test]
Run from anywhere; paths resolve relative to the repo root.
"""

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
I18N_RS = ROOT / "src" / "i18n.rs"
PICKER_RS = ROOT / "src" / "rune_sec" / "settings_popover.rs"

# (static name, BCP 47 code, Lang variant) in picker order.
LANGS = [
    ("EN", "en", "En"),
    ("BN", "bn", "Bn"),
    ("ES", "es", "Es"),
    ("AR", "ar", "Ar"),
    ("CS", "cs", "Cs"),
    ("DA", "da", "Da"),
    ("DE", "de", "De"),
    ("EL", "el", "El"),
    ("ES419", "es-419", "Es419"),
    ("FI", "fi", "Fi"),
    ("FR", "fr", "Fr"),
    ("HE", "he", "He"),
    ("HR", "hr", "Hr"),
    ("HU", "hu", "Hu"),
    ("ID", "id", "Id"),
    ("IT", "it", "It"),
    ("KK", "kk", "Kk"),
    ("NB", "nb", "Nb"),
    ("NL", "nl", "Nl"),
    ("PL", "pl", "Pl"),
    ("PT_BR", "pt-BR", "PtBr"),
    ("RO", "ro", "Ro"),
    ("RU", "ru", "Ru"),
    ("SL", "sl", "Sl"),
    ("TR", "tr", "Tr"),
    ("UK", "uk", "Uk"),
    ("VI", "vi", "Vi"),
    ("ZH_HANS", "zh-Hans", "ZhHans"),
]

# Base-language fallback chain (PL-9): variant -> base variant.
BASE = {"Es419": "Es"}

# Tier bands, on translated percentage. Mirror of the doc on
# `lang_tier` in settings_popover.rs; the drift check below keeps
# the two in agreement.
TIER_FULL_MIN = 85.0
TIER_PARTIAL_MIN = 40.0


def tier_of(pct: float) -> str:
    if pct >= TIER_FULL_MIN:
        return "Full"
    if pct >= TIER_PARTIAL_MIN:
        return "Partial"
    return "Basic"


# First-run baseline (RM27, 2026-11-10): translated counts on the
# 697-key catalogue, as measured by this instrument's first run.
# Ratchet up by hand when a tranche lands.
# Roadmap Wave 1 ratchet (integrator, 2026-11-10): the Groups 3+5
# tranche (85 + 6 genuine keys x28) landed on the 788-key
# catalogue; baselines below are this instrument's measured
# counts on the settled tree.
BASELINE_TRANSLATED = {
    "en": 954,
    "bn": 905,
    "es": 882,
    "ar": 132,
    "cs": 121,
    "da": 123,
    "de": 853,
    "el": 132,
    "es-419": 129,
    "fi": 128,
    "fr": 851,
    "he": 132,
    "hr": 127,
    "hu": 128,
    "id": 129,
    "it": 872,
    "kk": 132,
    "nb": 124,
    "nl": 120,
    "pl": 128,
    "pt-BR": 887,
    "ro": 123,
    "ru": 905,
    "sl": 127,
    "tr": 130,
    "uk": 132,
    "vi": 129,
    "zh-Hans": 905,
}


def unescape(raw: str) -> str:
    """Resolve Rust string escapes for comparison purposes."""
    out = []
    i = 0
    while i < len(raw):
        c = raw[i]
        if c != "\\":
            out.append(c)
            i += 1
            continue
        nxt = raw[i + 1]
        if nxt == "u":
            end = raw.index("}", i)
            out.append(chr(int(raw[i + 3:end], 16)))
            i = end + 1
        elif nxt == "n":
            out.append("\n"); i += 2
        elif nxt == "t":
            out.append("\t"); i += 2
        elif nxt == "r":
            out.append("\r"); i += 2
        elif nxt == "0":
            out.append("\0"); i += 2
        else:  # \\, \" and anything else: the char itself
            out.append(nxt); i += 2
    return "".join(out)


def parse_table(text: str, static_name: str):
    """Extract [(key, value)] from one `static NAME: &[(&str,&str)]`
    block. Char scanner: skips // comments, tolerates tuples that
    span lines (the catalogue has one: `status_ready`)."""
    m = re.search(r"static\s+" + static_name + r"\s*:", text)
    if not m:
        return None
    i = text.index("= &[", m.start()) + len("= &[")
    pairs = []
    n = len(text)
    while i < n:
        c = text[i]
        if c == "]" and text[i + 1] == ";":
            break
        if text.startswith("//", i):
            i = text.index("\n", i)
            continue
        if c == '"':
            # One string literal.
            j = i + 1
            raw = []
            while text[j] != '"':
                if text[j] == "\\":
                    raw.append(text[j:j + 2])
                    j += 2
                else:
                    raw.append(text[j])
                    j += 1
            literal = unescape("".join(raw))
            # A pair is ("key", "value"): collect literals pairwise.
            pairs.append(literal)
            i = j + 1
            continue
        i += 1
    if len(pairs) % 2:
        raise ValueError(f"{static_name}: odd literal count {len(pairs)}")
    return [(pairs[k], pairs[k + 1]) for k in range(0, len(pairs), 2)]


def slots(value: str):
    return sorted(re.findall(r"\{([^{}]*)\}", value))


def measure(tables):
    """tables: {variant: [(key, value)]}. Returns (rows, problems).
    Each row: dict with code, variant, total, identical, translated,
    pct, tier, and for base-chained languages also eff_translated,
    eff_pct, eff_tier, chain_gain."""
    en = dict(tables["En"])
    en_keys = [k for k, _ in tables["En"]]
    problems = []
    rows = []
    for _static, code, variant in LANGS:
        tbl = tables[variant]
        keys = [k for k, _ in tbl]
        vals = dict(tbl)
        if keys != en_keys:
            problems.append(f"{code}: key sequence differs from EN")
        if len(set(keys)) != len(keys):
            problems.append(f"{code}: duplicate keys")
        for k, v in tbl:
            if v == "":
                problems.append(f"{code}: empty value for {k}")
            if v == k:
                problems.append(f"{code}: value == key for {k}")
            if slots(v) != slots(en[k]):
                problems.append(f"{code}: slot mismatch for {k}")
        identical = sum(1 for k in en_keys if vals[k] == en[k])
        total = len(en_keys)
        translated = total - identical
        pct = 100.0 * translated / total
        if variant == "En":
            # EN is the reference every other table is measured
            # against: it has no placeholders by definition, so
            # its coverage is 100% and its =EN column reads 0
            # (the raw self-comparison would be meaningless).
            identical = 0
            translated = total
            pct = 100.0
        row = {
            "code": code, "variant": variant, "total": total,
            "identical_to_en": identical, "translated": translated,
            "pct": round(pct, 1), "tier": tier_of(pct),
        }
        if variant in BASE:
            base_vals = dict(tables[BASE[variant]])
            # Resolved under the chain: own value wins when it is
            # not the EN placeholder; else the base value when IT
            # is translated; else English.
            eff = 0
            gain = 0
            for k in en_keys:
                resolved = vals[k]
                if resolved == en[k] and base_vals[k] != en[k]:
                    resolved = base_vals[k]
                    gain += 1
                if resolved != en[k]:
                    eff += 1
            row["base"] = BASE[variant]
            row["chain_gain"] = gain
            row["eff_translated"] = eff
            row["eff_pct"] = round(100.0 * eff / total, 1)
            row["eff_tier"] = tier_of(100.0 * eff / total)
        rows.append(row)
    return rows, problems


def picker_tiers():
    """Parse the `lang_tier` match in settings_popover.rs into
    {variant: tier}. None when the function is absent."""
    text = PICKER_RS.read_text(encoding="utf-8")
    m = re.search(r"pub fn lang_tier\(.*?\n\}", text, re.S)
    if not m:
        return None
    pairs = re.findall(r"Lang::(\w+)\s*=>\s*LangTier::(\w+)", m.group(0))
    return dict(pairs)


def run_checks(rows, problems):
    failures = list(problems)
    tiers = picker_tiers()
    if tiers is None:
        failures.append("settings_popover.rs: lang_tier not found "
                        "(tier machinery missing)")
    else:
        for row in rows:
            want = row.get("eff_tier", row["tier"])
            got = tiers.get(row["variant"])
            if got != want:
                failures.append(
                    f"tier drift: {row['code']} picker says {got}, "
                    f"measured {want} ({row.get('eff_pct', row['pct'])}% "
                    f"translated)")
    for row in rows:
        base = BASELINE_TRANSLATED[row["code"]]
        if row["translated"] < base:
            failures.append(
                f"baseline regression: {row['code']} translated "
                f"{row['translated']} < baseline {base}")
    return failures


def self_test():
    toy_text = '''
static EN: &[(&str, &str)] = &[
    ("a", "Apple"),
    // a comment between tuples
    ("b", "Bee {x}"),
    ("c",
     "Cee"),
];
static ES: &[(&str, &str)] = &[
    ("a", "Manzana"),
    ("b", "Bee {x}"),
    ("c", "Cee"),
];
static ES419: &[(&str, &str)] = &[
    ("a", "Apple"),
    ("b", "Abeja {x}"),
    ("c", "Cee"),
];
'''
    tables = {"En": parse_table(toy_text, "EN"),
              "Es": parse_table(toy_text, "ES"),
              "Es419": parse_table(toy_text, "ES419")}
    assert tables["En"][2] == ("c", "Cee"), "multi-line tuple"
    rows, problems = measure({
        "En": tables["En"], "Es": tables["Es"], "Es419": tables["Es419"],
        **{v: tables["En"] for _s, _c, v in LANGS
           if v not in ("En", "Es", "Es419")},
    })
    assert not problems, problems
    es419 = next(r for r in rows if r["code"] == "es-419")
    # es-419: 'a' is a placeholder that the chain fills from es
    # (gain 1, effective translated 2 of 3); 'c' stays English in
    # every table, so it is never counted translated.
    assert es419["translated"] == 1, es419
    assert es419["chain_gain"] == 1, es419
    assert es419["eff_translated"] == 2, es419
    assert tier_of(90.0) == "Full" and tier_of(50.0) == "Partial" \
        and tier_of(5.0) == "Basic"
    print("self-test PASS (parse, chain math, tiers)")
    return 0


def main(argv):
    if "--self-test" in argv:
        return self_test()
    text = I18N_RS.read_text(encoding="utf-8")
    tables = {}
    for static, _code, variant in LANGS:
        tbl = parse_table(text, static)
        if tbl is None:
            print(f"FAIL: static {static} not found in i18n.rs")
            return 1
        tables[variant] = tbl
    rows, problems = measure(tables)
    failures = run_checks(rows, problems)
    if "--json" in argv:
        print(json.dumps({"rows": rows, "failures": failures}, indent=1))
    else:
        print(f"{'code':<8} {'keys':>4} {'=EN':>4} {'transl':>6} "
              f"{'%':>5}  tier     effective (with base chain)")
        for r in rows:
            eff = ""
            if "eff_translated" in r:
                eff = (f"via {r['base']}: {r['eff_translated']} "
                       f"({r['eff_pct']}%), chain fills "
                       f"{r['chain_gain']} cells -> {r['eff_tier']}")
            print(f"{r['code']:<8} {r['total']:>4} "
                  f"{r['identical_to_en']:>4} {r['translated']:>6} "
                  f"{r['pct']:>5}  {r['tier']:<8} {eff}")
        for f in failures:
            print(f"FAIL: {f}")
        print("RESULT:", "FAIL" if failures else "PASS",
              f"— {len(rows)} languages, {rows[0]['total']} keys each")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
