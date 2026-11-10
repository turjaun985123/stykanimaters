#!/usr/bin/env python3
"""MCP schema <-> catalogue audit — StykAnimate-RS (Roadmap F-10, RM11).

Closes the blind spot `scripts/mcp_census.py` states in its own
header ("Argument schemas are not censused (only the description
is lifted); a tool whose schema drifts from its handler is a
test-suite concern, not this script's") and mechanises the audit
Wave AB's AB1/AB2 seats did BY HAND: all 392 catalogue rows in
`docs/MCP.md` verified cell-by-cell against the tools' argument
schemas, zero drift found. This script makes that a standing,
repeatable check instead of a one-wave hand pass.

For every tool in the five families (the `all_tools()` order of
`scripts/phase_j_checks.py` check 9 / `scripts/mcp_census.py`):

  * the schema's property names are extracted from the source
    form the tool actually uses —
      built-ins / io / subtitles: `schema: json!({ ... })` or
        `schema: empty_args_schema()`;
      registry: `schema: obj_schema(json!({ ... }), &[...])`;
  * the schema's `required` list is extracted the same way;
  * both are compared against the catalogue row's Required and
    Optional columns (backticked names; `—` means none):
      schema required          == catalogue Required
      schema properties-required == catalogue Optional
      schema properties        == catalogue Required ∪ Optional
  * schema-internal consistency: required ⊆ properties;
  * catalogue section membership: each section's rows are the
    family its heading names (the registry family is presented
    as two sections — Editing registry + Shell control — whose
    union must be the registry set, split by the `shell_` name
    prefix);
  * name-set equality overall: no documented-but-nonexistent,
    no existing-but-undocumented (the same contract gate
    check 9 enforces on names, here extended to arguments).

Extraction is source-shape based, like the census: balanced
scans that skip string literals and line comments, property
keys read at depth 1 of the properties object only (nested
fragments — `items`, `properties` of an argument, enum lists —
are not arguments). A schema form this script does not
recognise is a loud failure, never a silent skip: a new form
means the audit must learn it before it can vouch for the row.

Usage:
    python3 scripts/mcp_schema_audit.py            # text report
    python3 scripts/mcp_schema_audit.py --json     # machine-readable

Exit status: 0 when schema and catalogue agree everywhere,
1 on any drift or extraction failure. Stdlib only; reads the
tree, never writes it.
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SRC = ROOT / "src"
DOC = ROOT / "docs" / "MCP.md"


# ---------------------------------------------------------------------------
# Balanced scanning (string- and comment-aware)
# ---------------------------------------------------------------------------

def skip_string(text: str, i: int) -> int:
    """Index just past the string literal opening at text[i]."""
    assert text[i] == '"'
    i += 1
    n = len(text)
    while i < n:
        c = text[i]
        if c == "\\":
            i += 2
            continue
        if c == '"':
            return i + 1
        i += 1
    raise ValueError("unterminated string literal")


def scan_balanced(text: str, start: int, open_ch: str, close_ch: str) -> int:
    """Index just past the close char matching text[start]."""
    assert text[start] == open_ch, (start, text[start:start + 20])
    depth = 0
    i = start
    n = len(text)
    while i < n:
        c = text[i]
        if c == '"':
            i = skip_string(text, i)
            continue
        if c == "/" and i + 1 < n and text[i + 1] == "/":
            j = text.find("\n", i)
            i = n if j < 0 else j + 1
            continue
        if c == open_ch:
            depth += 1
        elif c == close_ch:
            depth -= 1
            if depth == 0:
                return i + 1
        i += 1
    raise ValueError(f"unbalanced {open_ch}{close_ch} from offset {start}")


def strings_in(text: str) -> list[str]:
    """All double-quoted string literals in a fragment, unescaped
    the JSON way (schema fragments use JSON-compatible escapes
    for the simple identifier strings read here)."""
    out = []
    i = 0
    n = len(text)
    while i < n:
        if text[i] == '"':
            j = skip_string(text, i)
            raw = text[i + 1:j - 1]
            out.append(raw.replace('\\"', '"').replace("\\\\", "\\"))
            i = j
        else:
            i += 1
    return out


def top_level_keys(obj: str) -> list[str]:
    """Keys at depth 1 of a `{ ... }` object literal fragment."""
    assert obj.lstrip().startswith("{")
    keys: list[str] = []
    depth = 0
    i = 0
    n = len(obj)
    while i < n:
        c = obj[i]
        if c == '"':
            j = skip_string(obj, i)
            if depth == 1:
                # A depth-1 string followed by ':' is a key.
                k = j
                while k < n and obj[k] in " \t\r\n":
                    k += 1
                if k < n and obj[k] == ":":
                    keys.append(obj[i + 1:j - 1])
            i = j
            continue
        if c == "/" and i + 1 < n and obj[i + 1] == "/":
            e = obj.find("\n", i)
            i = n if e < 0 else e + 1
            continue
        if c in "{[(":
            depth += 1
        elif c in "}])":
            depth -= 1
        i += 1
    return keys


# ---------------------------------------------------------------------------
# Schema extraction
# ---------------------------------------------------------------------------

def parse_schema_value(seg: str, tool: str) -> tuple[list[str], list[str]]:
    """(properties, required) from one `schema: <value>` segment."""
    seg = seg.strip()
    if seg.startswith("empty_args_schema()"):
        return [], []
    if seg.startswith("obj_schema("):
        # obj_schema(json!({ props }), &[required...])
        j = seg.index("json!(")
        brace = seg.index("{", j)
        end = scan_balanced(seg, brace, "{", "}")
        props = top_level_keys(seg[brace:end])
        rest = seg[end:]
        req: list[str] = []
        m = re.search(r"&\[", rest)
        if m:
            b = rest.index("[", m.start())
            bend = scan_balanced(rest, b, "[", "]")
            req = strings_in(rest[b:bend])
        return props, req
    if seg.startswith("json!("):
        brace = seg.index("{", seg.index("("))
        end = scan_balanced(seg, brace, "{", "}")
        obj = seg[brace:end]
        # Depth-1 members of the schema object: find "properties"
        # and "required" as top-level keys and read their values.
        props: list[str] = []
        req: list[str] = []
        depth = 0
        i = 0
        n = len(obj)
        while i < n:
            c = obj[i]
            if c == '"':
                j = skip_string(obj, i)
                if depth == 1:
                    key = obj[i + 1:j - 1]
                    k = j
                    while k < n and obj[k] in " \t\r\n":
                        k += 1
                    if k < n and obj[k] == ":":
                        v = k + 1
                        while v < n and obj[v] in " \t\r\n":
                            v += 1
                        if key == "properties" and v < n and obj[v] == "{":
                            vend = scan_balanced(obj, v, "{", "}")
                            props = top_level_keys(obj[v:vend])
                        elif key == "required" and v < n and obj[v] == "[":
                            vend = scan_balanced(obj, v, "[", "]")
                            req = strings_in(obj[v:vend])
                i = j
                continue
            if c in "{[(":
                depth += 1
            elif c in "}])":
                depth -= 1
            i += 1
        return props, req
    raise ValueError(f"{tool}: unrecognised schema form: {seg[:60]!r}")


def extract_tools(region: str, family: str) -> list[dict]:
    """Every `Tool { ... }` literal in a production region."""
    tools = []
    for m in re.finditer(r"Tool \{", region):
        # The struct declaration is not a literal, and neither is
        # a `fn tool_x() -> Tool {` signature (its brace opens the
        # function body, which merely CONTAINS the literal).
        before = region[max(0, m.start() - 12):m.start()].rstrip()
        if before.endswith("struct") or before.endswith("->"):
            continue
        brace = region.index("{", m.start())
        end = scan_balanced(region, brace, "{", "}")
        lit = region[brace:end]
        nm = re.search(r'\bname:\s*"([^"]+)"', lit)
        if not nm:
            raise ValueError(f"{family}: Tool literal without a literal name "
                             f"near offset {m.start()}")
        name = nm.group(1)
        sm = re.search(r"\bschema:\s*", lit)
        if not sm:
            raise ValueError(f"{family}:{name}: Tool literal without schema")
        props, req = parse_schema_value(lit[sm.end():], name)
        tools.append({"name": name, "family": family,
                      "properties": props, "required": req})
    return tools


def production_region(path: Path, start_marker: str | None) -> str:
    text = path.read_text(encoding="utf-8")
    if start_marker:
        text = text[text.index(start_marker):]
    cut = text.find("#[cfg(test)]")
    if cut >= 0:
        text = text[:cut]
    return text


def extract_surface() -> list[dict]:
    """All five families, in all_tools() order."""
    tools: list[dict] = []
    tools += extract_tools(production_region(SRC / "mcp.rs", None), "builtin")
    tools += extract_tools(
        production_region(SRC / "mcp_tools.rs", "pub fn tools()"), "registry")
    tools += extract_tools(
        production_region(SRC / "mcp_io.rs", "pub fn tools()"), "io")
    tools += extract_tools(
        production_region(SRC / "mcp_subtitle_io.rs", "pub fn tools()"),
        "subtitle_io")
    tools += extract_tools(
        production_region(SRC / "mcp_subtitle_tools.rs", "pub fn tools()"),
        "subtitle")
    return tools


# ---------------------------------------------------------------------------
# Catalogue extraction (docs/MCP.md)
# ---------------------------------------------------------------------------

def split_row(line: str) -> list[str]:
    """Split a markdown table row on unescaped pipes (the
    `set_node` row's Purpose cell carries escaped `\\|`)."""
    placeholder = "\x00PIPE\x00"
    body = line.strip().replace("\\|", placeholder)
    cells = [c.replace(placeholder, "|").strip() for c in body.split("|")]
    # Leading/trailing pipes produce empty edge cells.
    if cells and cells[0] == "":
        cells = cells[1:]
    if cells and cells[-1] == "":
        cells = cells[:-1]
    return cells


def arg_cell(cell: str) -> list[str]:
    if cell in ("—", "-", ""):
        return []
    return re.findall(r"`([^`]+)`", cell)


def extract_catalogue() -> tuple[list[dict], dict[str, list[str]]]:
    text = DOC.read_text(encoding="utf-8")
    cat = text.split("## Catalogue", 1)[1]
    rows: list[dict] = []
    sections: dict[str, list[str]] = {}
    section = ""
    for line in cat.splitlines():
        if line.startswith("### "):
            section = line[4:].strip()
            sections.setdefault(section, [])
            continue
        if line.startswith("| `"):
            cells = split_row(line)
            if len(cells) != 4:
                raise ValueError(f"catalogue row with {len(cells)} cells "
                                 f"(want 4): {line[:80]!r}")
            name = re.fullmatch(r"`([^`]+)`", cells[0]).group(1)
            rows.append({"name": name, "section": section,
                         "required": arg_cell(cells[2]),
                         "optional": arg_cell(cells[3])})
            sections[section].append(name)
    return rows, sections


# ---------------------------------------------------------------------------
# Audit
# ---------------------------------------------------------------------------

SECTION_FAMILY = (
    ("Built-ins", {"builtin"}),
    ("Editing registry", {"registry"}),
    ("Shell control", {"registry"}),
    ("File IO and export", {"io"}),
    ("Subtitles", {"subtitle_io", "subtitle"}),
)


def audit() -> dict:
    problems: list[str] = []
    tools = extract_surface()
    rows, sections = extract_catalogue()

    by_name: dict[str, dict] = {}
    for t in tools:
        if t["name"] in by_name:
            problems.append(f"duplicate tool name in surface: {t['name']}")
        by_name[t["name"]] = t
    doc_by_name: dict[str, dict] = {}
    for r in rows:
        if r["name"] in doc_by_name:
            problems.append(f"duplicate catalogue row: {r['name']}")
        doc_by_name[r["name"]] = r

    for name in sorted(set(by_name) - set(doc_by_name)):
        problems.append(f"tool exists but is not catalogued: {name}")
    for name in sorted(set(doc_by_name) - set(by_name)):
        problems.append(f"catalogued but does not exist: {name}")

    compared = 0
    for name in sorted(set(by_name) & set(doc_by_name)):
        t, r = by_name[name], doc_by_name[name]
        props, req = t["properties"], t["required"]
        if len(props) != len(set(props)):
            problems.append(f"{name}: duplicate property in schema: {props}")
        if not set(req) <= set(props):
            problems.append(
                f"{name}: schema required {sorted(set(req) - set(props))} "
                f"not among its properties {sorted(props)}")
        if set(req) != set(r["required"]):
            problems.append(
                f"{name}: Required drift — schema {sorted(req)} vs "
                f"catalogue {sorted(r['required'])}")
        optional = [p for p in props if p not in set(req)]
        if set(optional) != set(r["optional"]):
            problems.append(
                f"{name}: Optional drift — schema properties-required "
                f"{sorted(optional)} vs catalogue {sorted(r['optional'])}")
        compared += 1

    # Section membership: rows sit under the family they name.
    for section, names in sections.items():
        want = None
        for prefix, fams in SECTION_FAMILY:
            if section.startswith(prefix):
                want = fams
                break
        if want is None:
            problems.append(f"unrecognised catalogue section: {section!r}")
            continue
        for name in names:
            t = by_name.get(name)
            if t and t["family"] not in want:
                problems.append(
                    f"{name}: catalogued under {section!r} but is family "
                    f"{t['family']}")
        if section.startswith("Shell control"):
            for name in names:
                if not name.startswith("shell_"):
                    problems.append(
                        f"{name}: in Shell control section without the "
                        f"shell_ prefix")
        if section.startswith("Editing registry"):
            for name in names:
                if name.startswith("shell_"):
                    problems.append(
                        f"{name}: shell_ tool in the Editing registry "
                        f"section (belongs in Shell control)")

    fam_counts: dict[str, int] = {}
    for t in tools:
        fam_counts[t["family"]] = fam_counts.get(t["family"], 0) + 1
    return {
        "tools": len(tools),
        "catalogue_rows": len(rows),
        "compared": compared,
        "families": fam_counts,
        "sections": {k: len(v) for k, v in sections.items()},
        "problems": problems,
    }


def main() -> int:
    result = audit()
    if "--json" in sys.argv:
        print(json.dumps(result, indent=2, ensure_ascii=False))
    else:
        print("MCP schema <-> catalogue audit — StykAnimate-RS")
        print(f"Tools extracted from source schemas: {result['tools']} "
              f"(families: {result['families']})")
        print(f"Catalogue rows in docs/MCP.md: {result['catalogue_rows']} "
              f"(sections: {result['sections']})")
        print(f"Rows compared (Required + Optional vs schema): "
              f"{result['compared']}")
        if result["problems"]:
            print(f"\nDRIFT / PROBLEMS ({len(result['problems'])}):")
            for p in result["problems"]:
                print(f"  - {p}")
        else:
            print("\nAGREE — every catalogue row's Required/Optional "
                  "matches its tool's schema; sections match families.")
    return 1 if result["problems"] else 0


if __name__ == "__main__":
    sys.exit(main())
