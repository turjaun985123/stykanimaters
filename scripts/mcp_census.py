#!/usr/bin/env python3
"""MCP surface census for StykAnimate-RS (Wave 11, E2 — the
reference implementation the gate's check 9 approximates).

Answers, from source alone, three questions about the MCP
(Model Context Protocol) surface:

  1. Every CALLABLE tool: its name, its family, where it is
     defined, where its handler lives, and a one-line purpose
     lifted from its own `description`.
  2. Every JSON-RPC dispatch answer the shared router can
     produce (methods, the notification rule, the error
     codes), plus the transport-level answers of the two
     shells (stdio framing, live-HTTP statuses).
  3. Every place a tool can be invoked FROM: the invocation
     map (stdio binary, live HTTP through the GUI poll loop,
     and the test suites that drive the real dispatch).

The three tool families, in `all_tools()` composition order
(src/mcp.rs `all_tools`):

  - builtin  — the constructors `fn tool_*() -> Tool` in
               src/mcp.rs, spliced into `all_tools()`'s
               opening `vec![...]`; handlers are closures
               inside the constructor.
  - registry — the `Tool { ... }` literals in
               `pub fn tools()` in src/mcp_tools.rs;
               handlers are named `fn tool_*` functions —
               in the same file, or in a DOMAIN file,
               referenced by bare name or by a `crate::…`
               path (the Wave 11 splice pattern: at the
               Wave 13 opening, 119 of 230 registry
               handlers live outside src/mcp_tools.rs.
               The newest domain file is src/media.rs,
               which joined in Wave 12 with the six asset
               handlers; Wave 13's five sprite handlers
               (tool_sprite_*, also in src/media.rs) take
               the registry to 235 — and media.rs's share
               to 11 of the 124 outside handlers — once
               the integrator splices their entries).
  - io       — the `Tool { ... }` literals in
               `pub fn tools()` in src/mcp_io.rs; handlers
               likewise named, same file (all 9 today).

The report prints per-family subtotals — the text
`[family counts]` block and the JSON `totals` object —
so wave close-outs never hand-count the surface.

VERIFICATION (the Wave 9/10 lesson, applied): a counter
that misses a new declaration pattern lies quietly, so
this census never trusts one pattern. Each family is
counted by at least two independent patterns and the
script REFUSES to print a census (exit 2, message on
stderr) when they disagree:

  - builtin: constructor definitions returning `Tool`
    (the `-> Tool` in the signature is what excludes the
    `tool_result` response-wrapper helper) must equal, as
    a set, the constructor calls in `all_tools()`'s vec;
    and each constructor body must hold exactly one
    `name:` literal.
  - registry / io: `Tool {` blocks inside `tools()` must
    equal the `handler:` fields inside `tools()` and the
    `name:` literals inside `tools()` — three patterns,
    one scope. (Scoping is the point: src/mcp_tools.rs
    holds one further `name: "` literal OUTSIDE
    `tools()` — 231 in the whole file against 230
    inside, at the Wave 13 opening — `name:
    "Styk".into()` in a handler body, and a whole-file
    count overcounts by exactly one.)
  - every named handler must resolve to a definition, and
    every handler-shaped `fn tool_*` in the two registry
    files must be referenced by exactly one entry (no
    orphans either way);
  - `all_tools()` must still extend `mcp_tools::tools()`
    and `mcp_io::tools()` (the composition itself);
  - no tool name may appear twice across families (the
    router's `find` would silently shadow the second);
  - the four dispatch methods and the notification rule
    must be found in `McpServer::dispatch` — if the
    router's shape changes, the census says so instead of
    printing a stale map.

WHAT IT CANNOT SEE (named blind spots):

  - Handler-shaped `pub fn tool_*` functions in DOMAIN
    files (paint, draw_ops, fig3d, audio_ops, graph_keys,
    figure_ops, fx_ops, particle_ops, frame_swap, style,
    text_edit, media) are NOT callable until the
    integrator splices a `Tool` entry into
    `mcp_tools::tools()`. They are listed separately as
    UNREGISTERED HANDLERS — the splice checklist — and
    never counted in the totals. The scan set is every
    `src/**/*.rs` file (plus `tests/*.rs` for the
    invocation map), so a domain file joins the census
    the moment it gains handlers — src/media.rs did in
    Wave 12 with its six asset handlers, and Wave 13's
    five sprite handlers in the same file follow
    exactly the same path: they appear on the checklist
    and resolve through the same code path as every
    earlier domain file. Two cautions ride with
    that list: a domain handler's fn name can collide
    with a registered handler's (draw_ops::tool_draw_stroke
    vs the registry's own) without being the same
    function, and two domain files can carry the same
    fn name (tool_list_brushes in draw_ops and paint);
    duplicate symbols in the list are flagged in the
    output.
  - The checklist's signature pattern accepts `pub fn`
    and restricted visibilities (`pub(crate) fn`, …).
    A handler-shaped fn with NO visibility modifier in a
    domain file would be missed by the checklist — it
    also cannot be referenced from src/mcp_tools.rs
    until it gains one, and the definition index used
    for resolution (`all_handler_defs`) is
    visibility-agnostic, so a splice after such a fix
    still resolves. A handler whose SIGNATURE shape
    differs (no `&mut EditorCore` parameter, or a return
    type other than `Result<Value, String>`) is
    deliberately not checklist-shaped: the `Tool`
    struct's handler field is a plain fn pointer of
    exactly that type, so such a fn cannot be a handler
    without a type change the census would see elsewhere.
  - Resolution is first-definition-wins over the scan
    order, and the definition index does not exclude
    `mod tests` regions: a same-named `fn tool_*` inside
    a test block could shadow a production definition in
    the census's answer. (The gate's check 9 is stricter
    — it requires exactly one production definition per
    referenced handler.) No such collision exists today:
    every referenced handler name has exactly one
    definition in the production tree.
  - Tool names are all static literals today; there is no
    dynamically constructed tool name anywhere in the tree
    (no `name: format!`, no `name: <variable>` in any
    registry scope — the extractor treats a non-literal
    `name:` inside a registry scope as a verification
    failure, so the day one appears this census breaks
    loudly instead of undercounting).
  - Argument schemas are not censused (only the
    description is lifted); a tool whose schema drifts
    from its handler is a test-suite concern, not this
    script's.
  - The command palette is NOT an invocation point: its
    only MCP surface is the `toggle-live-mcp` command,
    which switches the live endpoint on/off. The map
    says so, with the line, rather than omitting it.

Read-only, stdlib only, deterministic: the same tree
produces byte-identical output (entries sorted, no
timestamps, repo-relative paths). Run from anywhere:

    python3 scripts/mcp_census.py           # text report
    python3 scripts/mcp_census.py --json    # JSON report
"""

import bisect
import glob
import json
import os
import re
import sys

FAMILY_ORDER = ("builtin", "registry", "io")


# ---------------------------------------------------------------------
# Source handling: a masking lexer + balanced spans.
#
# Rust sources hold braces inside strings, chars and
# (nested) comments, so span location runs on a MASKED
# copy of the file — same length, string/char/comment
# contents replaced by spaces, newlines preserved — while
# literal extraction reads the RAW text inside the spans
# the masked copy located. Line numbers survive masking.
# ---------------------------------------------------------------------

_CHAR_RE = re.compile(r"'(\\.|[^\\'])'")


def mask_source(src: str) -> str:
    out = list(src)
    i, n = 0, len(src)
    while i < n:
        ch = src[i]
        nxt = src[i + 1] if i + 1 < n else ""
        if ch == "/" and nxt == "/":
            j = src.find("\n", i)
            j = n if j < 0 else j
            for k in range(i, j):
                out[k] = " "
            i = j
        elif ch == "/" and nxt == "*":
            depth, j = 0, i
            while j < n:
                if src.startswith("/*", j):
                    depth += 1
                    j += 2
                elif src.startswith("*/", j):
                    depth -= 1
                    j += 2
                    if depth == 0:
                        break
                else:
                    j += 1
            for k in range(i, min(j, n)):
                if out[k] != "\n":
                    out[k] = " "
            i = j
        elif ch == "r" and (nxt == '"' or nxt == "#"):
            # Raw string: r"..." or r#*"..."#*
            j = i + 1
            hashes = 0
            while j < n and src[j] == "#":
                hashes += 1
                j += 1
            if j < n and src[j] == '"':
                closer = '"' + "#" * hashes
                end = src.find(closer, j + 1)
                end = n if end < 0 else end + len(closer)
                for k in range(i, end):
                    if out[k] != "\n":
                        out[k] = " "
                i = end
            else:
                i += 1
        elif ch == '"':
            j = i + 1
            while j < n:
                if src[j] == "\\":
                    j += 2
                    continue
                if src[j] == '"':
                    j += 1
                    break
                j += 1
            for k in range(i, min(j, n)):
                if out[k] != "\n":
                    out[k] = " "
            i = j
        elif ch == "'":
            m = _CHAR_RE.match(src, i)
            if m:
                for k in range(i, m.end()):
                    out[k] = " "
                i = m.end()
            else:
                i += 1  # a lifetime, not a char literal
        else:
            i += 1
    return "".join(out)


class Source:
    def __init__(self, root: str, rel: str):
        self.rel = rel
        path = os.path.join(root, rel)
        with open(path, encoding="utf-8") as fh:
            self.raw = fh.read()
        self.masked = mask_source(self.raw)
        self._starts = [0]
        for m in re.finditer("\n", self.raw):
            self._starts.append(m.end())
        # Line (1-based) of the `mod tests` block — the
        # production/test boundary used by several scans.
        # NOT the first #[cfg(test)]: fig3d.rs carries a
        # test-only helper under #[cfg(test)] at line ~852,
        # far above its production handlers, and a
        # first-attribute boundary would blind the census
        # to everything below it.
        m = re.search(r"\bmod\s+tests\b", self.masked)
        self.test_line = self.line_of(m.start()) if m else None

    def line_of(self, offset: int) -> int:
        return bisect.bisect_right(self._starts, offset)

    def span(self, open_brace: int) -> tuple[int, int]:
        """(start, end) offsets of the brace-balanced block
        whose `{` sits at `open_brace` in the masked text."""
        depth = 0
        i = open_brace
        while i < len(self.masked):
            ch = self.masked[i]
            if ch == "{":
                depth += 1
            elif ch == "}":
                depth -= 1
                if depth == 0:
                    return open_brace, i + 1
            i += 1
        raise ValueError(f"unbalanced braces in {self.rel}")


def unescape(lit: str) -> str:
    """Unescape the small Rust string-literal subset the
    registry descriptions use (\\\" \\\\ \\n); any other
    byte passes through untouched."""
    out = []
    i = 0
    while i < len(lit):
        if lit[i] == "\\" and i + 1 < len(lit) and lit[i + 1] in '"\\n':
            out.append({'"': '"', "\\": "\\", "n": " "}[lit[i + 1]])
            i += 2
        else:
            out.append(lit[i])
            i += 1
    return " ".join("".join(out).split())


def first_literal(raw_span: str, field: str) -> str | None:
    m = re.search(field + r':\s*"((?:[^"\\]|\\.)*)"', raw_span)
    return unescape(m.group(1)) if m else None


class CensusError(Exception):
    pass


def fail(msg: str) -> None:
    raise CensusError(msg)


# ---------------------------------------------------------------------
# Family extraction
# ---------------------------------------------------------------------

def fn_span(src: Source, match: re.Match) -> tuple[int, int]:
    brace = src.masked.find("{", match.start())
    if brace < 0:
        fail(f"{src.rel}: no body after {match.group(0)!r}")
    return src.span(brace)


def extract_builtins(mcp: Source) -> tuple[list[dict], list[str]]:
    checks = []
    # Pattern A: constructor definitions returning Tool.
    ctors = {}
    for m in re.finditer(r"fn\s+(tool_\w+)\s*\(\s*\)\s*->\s*Tool\b", mcp.masked):
        start, end = fn_span(mcp, m)
        ctors[m.group(1)] = (start, end, mcp.line_of(m.start()))
    # Pattern B: constructor calls in all_tools()'s vec.
    am = re.search(r"pub\s+fn\s+all_tools\s*\(", mcp.masked)
    if not am:
        fail("src/mcp.rs: all_tools() not found")
    a_start, a_end = fn_span(mcp, am)
    body_masked = mcp.masked[a_start:a_end]
    vec_calls = re.findall(r"\b(tool_\w+)\s*\(\s*\)", body_masked)
    if set(vec_calls) != set(ctors):
        fail(
            "builtin verification failed: all_tools() vec calls "
            f"{sorted(vec_calls)} != constructor definitions {sorted(ctors)}"
        )
    if len(vec_calls) != len(set(vec_calls)):
        fail("builtin verification failed: a constructor is called twice in all_tools()")
    checks.append(
        f"builtin: {len(ctors)} constructors == {len(vec_calls)} all_tools() vec entries (set-equal)"
    )
    # Composition: the two extends must still be there.
    for ext in ("crate::mcp_tools::tools()", "crate::mcp_io::tools()"):
        if ext not in body_masked:
            fail(f"builtin verification failed: all_tools() no longer extends {ext}")
    checks.append("builtin: all_tools() extends mcp_tools::tools() + mcp_io::tools()")

    tools = []
    for fn_name, (start, end, fn_line) in sorted(ctors.items()):
        raw_span = mcp.raw[start:end]
        names = re.findall(r'\bname:\s*"', raw_span)
        if len(names) != 1:
            fail(f"builtin {fn_name}: expected exactly one name: literal, found {len(names)}")
        name = first_literal(raw_span, "name")
        desc = first_literal(raw_span, "description")
        hm = re.search(r"\bhandler\s*:", mcp.masked[start:end])
        if not (name and desc and hm):
            fail(f"builtin {fn_name}: missing name/description/handler field")
        tools.append(
            {
                "name": name,
                "family": "builtin",
                "defined": {"file": "src/mcp.rs", "line": fn_line, "symbol": fn_name},
                "handler": {
                    "file": "src/mcp.rs",
                    "line": mcp.line_of(start + hm.start()),
                    "symbol": "closure in " + fn_name,
                },
                "purpose": desc,
            }
        )
    checks.append("builtin: every constructor holds exactly one name/description/handler")
    return tools, checks


def extract_registry_file(
    src: Source, family: str, sources: dict[str, Source]
) -> tuple[list[dict], list[str]]:
    checks = []
    tm = re.search(r"pub\s+fn\s+tools\s*\(\s*\)", src.masked)
    if not tm:
        fail(f"{src.rel}: pub fn tools() not found")
    t_start, t_end = fn_span(src, tm)
    masked_body = src.masked[t_start:t_end]
    raw_body = src.raw[t_start:t_end]

    # Pattern A: Tool { blocks. Patterns B/C: handler: and
    # name: counts inside the same scope.
    blocks = []
    for m in re.finditer(r"\bTool\s*\{", masked_body):
        bs, be = src.span(t_start + m.end() - 1)
        blocks.append((bs, be))
    n_handlers = len(re.findall(r"\bhandler\s*:", masked_body))
    n_names = len(re.findall(r'\bname:\s*"', raw_body))
    if not (len(blocks) == n_handlers == n_names):
        fail(
            f"{family} verification failed in {src.rel}: Tool blocks={len(blocks)}, "
            f"handler: fields={n_handlers}, name: literals={n_names} inside tools() disagree"
        )
    checks.append(
        f"{family}: {len(blocks)} Tool blocks == handler: fields == name: literals inside tools()"
    )
    tools = []
    referenced: dict[str, int] = {}
    for bs, be in blocks:
        raw_span = src.raw[bs:be]
        # The entry's own name field is the block's FIRST
        # `name:` (schema JSON deeper in the block may hold
        # a "name" property of its own). If that first field
        # is not a string literal, the tool's name is built
        # dynamically — a blind spot this census refuses to
        # guess at, so it fails loudly instead.
        first_name = re.search(r"\bname:\s*(\S)", raw_span)
        if not first_name or first_name.group(1) != '"':
            fail(f"{family}: a Tool block in {src.rel} has a non-literal name: — dynamic tool names are not censused")
        name = first_literal(raw_span, "name")
        desc = first_literal(raw_span, "description")
        hm = re.search(r"\bhandler\s*:\s*([A-Za-z_][\w:]*)", src.raw[bs:be])
        if not (name and desc and hm):
            fail(f"{family}: a Tool block in {src.rel} is missing name/description/handler")
        token = hm.group(1)
        handler = resolve_handler(token, src, sources)
        referenced.setdefault(handler["symbol"], 0)
        referenced[handler["symbol"]] += 1
        tools.append(
            {
                "name": name,
                "family": family,
                "defined": {
                    "file": src.rel,
                    "line": src.line_of(bs),
                    "symbol": "Tool entry in tools()",
                },
                "handler": handler,
                "purpose": desc,
            }
        )

    # Orphan check: every handler-shaped fn tool_* defined in
    # this file must be referenced exactly once.
    defined = {}
    for m in re.finditer(r"fn\s+(tool_\w+)\s*\(", src.masked):
        if src.test_line and src.line_of(m.start()) >= src.test_line:
            continue
        defined[m.group(1)] = src.line_of(m.start())
    for fn_name in defined:
        if referenced.get(fn_name, 0) != 1:
            fail(
                f"{family} verification failed: handler {fn_name} defined at "
                f"{src.rel}:{defined[fn_name]} is referenced {referenced.get(fn_name, 0)} times (expected 1)"
            )
    for fn_name in referenced:
        if fn_name not in defined and fn_name not in all_handler_defs(sources):
            fail(f"{family} verification failed: handler {fn_name} has no definition")
    checks.append(f"{family}: all {len(defined)} handler definitions referenced exactly once, no orphans")
    return tools, checks


_HANDLER_DEF_CACHE: dict[str, dict] = {}


def all_handler_defs(sources: dict[str, Source]) -> dict[str, dict]:
    if _HANDLER_DEF_CACHE:
        return _HANDLER_DEF_CACHE
    defs: dict[str, dict] = {}
    for rel, src in sources.items():
        for m in re.finditer(r"fn\s+(tool_\w+)\s*\(", src.masked):
            defs.setdefault(
                m.group(1),
                {"file": rel, "line": src.line_of(m.start()), "symbol": m.group(1)},
            )
    _HANDLER_DEF_CACHE.update(defs)
    return defs


def resolve_handler(token: str, home: Source, sources: dict[str, Source]) -> dict:
    """Resolve a `handler:` token (a bare fn name today; a
    `crate::module::fn` path after Squad B splices) to its
    definition site. Bare names resolve in the entry's own
    file first, matching Rust's own scoping."""
    fn_name = token.split("::")[-1]
    if "::" in token:
        parts = token.split("::")
        if parts[0] == "crate" and len(parts) >= 3:
            rel = "src/" + "/".join(parts[1:-1]) + ".rs"
            src = sources.get(rel)
            if src:
                m = re.search(r"fn\s+" + fn_name + r"\s*\(", src.masked)
                if m:
                    return {"file": rel, "line": src.line_of(m.start()), "symbol": fn_name}
        fail(f"handler path {token} does not resolve to a definition")
    m = re.search(r"fn\s+" + fn_name + r"\s*\(", home.masked)
    if m:
        return {"file": home.rel, "line": home.line_of(m.start()), "symbol": fn_name}
    defs = all_handler_defs(sources)
    if fn_name in defs:
        return defs[fn_name]
    fail(f"handler {token} has no definition anywhere in src/")


# ---------------------------------------------------------------------
# Dispatch + transports + invocation map
# ---------------------------------------------------------------------

def extract_dispatch(mcp: Source) -> tuple[dict, list[str]]:
    checks = []
    dm = re.search(r"fn\s+dispatch\s*\(", mcp.masked)
    if not dm:
        fail("src/mcp.rs: McpServer::dispatch not found")
    d_start, d_end = fn_span(mcp, dm)
    masked = mcp.masked[d_start:d_end]
    raw = mcp.raw[d_start:d_end]
    # Method arms are string literals, so they are searched
    # in the RAW span (masking blanks literal contents);
    # the dispatch body holds no comment quoting a method
    # arm, and the arm pattern demands the `=>` after it.
    methods = []
    for m in re.finditer(r'"(initialize|ping|tools/list|tools/call)"\s*=>', raw):
        methods.append({"method": m.group(1), "line": mcp.line_of(d_start + m.start())})
    found = {m["method"] for m in methods}
    if found != {"initialize", "ping", "tools/list", "tools/call"}:
        fail(f"dispatch verification failed: method arms found are {sorted(found)}")
    checks.append("dispatch: all four method arms present in McpServer::dispatch")

    notif = re.search(r"let\s+Some\(id\)\s*=\s*msg\.get\(\"id\"\)", mcp.raw[d_start:d_end])
    if not notif:
        fail("dispatch verification failed: the notification rule (no id -> no reply) not found")
    checks.append("dispatch: notification rule present (message without id is never answered)")

    def codes_in(span_start: int, span_end: int) -> list[dict]:
        out = []
        for m in re.finditer(r"-\d{5}\b", mcp.masked[span_start:span_end]):
            out.append(
                {"code": int(m.group(0)), "line": mcp.line_of(span_start + m.start())}
            )
        return out

    dispatch_codes = codes_in(d_start, d_end)
    tm = re.search(r"fn\s+tools_call_result\s*\(", mcp.masked)
    t_start, t_end = fn_span(mcp, tm)
    call_codes = codes_in(t_start, t_end)
    rm = re.search(r"fn\s+tool_result\s*\(", mcp.masked)
    r_start, r_end = fn_span(mcp, rm)
    is_error_line = None
    ie = re.search(r"isError", mcp.raw[r_start:r_end])
    if ie:
        is_error_line = mcp.line_of(r_start + ie.start())

    init = re.search(r"fn\s+initialize_result\s*\(", mcp.masked)
    i_start, i_end = fn_span(mcp, init)
    # The result is a json! literal, so the key is quoted —
    # unlike the struct-field form first_literal expects.
    pm = re.search(r'"protocolVersion":\s*"([^"]+)"', mcp.raw[i_start:i_end])
    proto = pm.group(1) if pm else None

    return (
        {
            "methods": methods,
            "notification_rule_line": mcp.line_of(d_start + notif.start()),
            "dispatch_error_codes": dispatch_codes,
            "tools_call_error_codes": call_codes,
            "handler_error_shape": {
                "line": is_error_line,
                "shape": "handler Err -> result with isError: true (tool_result)",
            },
            "protocol_version": proto,
        },
        checks,
    )


def scan_hits(
    sources: dict[str, Source], pattern: str, scope: str
) -> list[dict]:
    hits = []
    rx = re.compile(pattern)
    for rel, src in sorted(sources.items()):
        in_src = rel.startswith("src/")
        if scope == "src" and not in_src:
            continue
        if scope == "tests" and in_src:
            continue
        for m in rx.finditer(src.masked):
            line = src.line_of(m.start())
            kind = "production"
            if not in_src or (src.test_line and line >= src.test_line):
                kind = "test"
            hits.append({"file": rel, "line": line, "kind": kind})
    return hits


def extract_transports_and_map(
    sources: dict[str, Source],
) -> tuple[dict, dict, list[str]]:
    checks = []
    stdio = sources["src/bin/mcp_server.rs"]
    live = sources["src/live_mcp.rs"]
    main = sources["src/main.rs"]

    def need(src: Source, pattern: str, what: str) -> re.Match:
        m = re.search(pattern, src.masked)
        if not m:
            fail(f"invocation map anchor missing: {what} not found in {src.rel}")
        return m

    # --- stdio shell ---
    handle_hit = need(stdio, r"server\.handle\(&msg\)", "stdio server.handle call")
    codes = {}
    for code in ("-32700", "-32600", "-32603"):
        m = re.search(re.escape(code), stdio.masked)
        codes[code] = stdio.line_of(m.start()) if m else None
    cap = re.search(r"MAX_LINE_BYTES:\s*usize\s*=\s*([\d\s\*]+);", stdio.raw)
    stdio_info = {
        "file": "src/bin/mcp_server.rs",
        "dispatch_call": {"line": stdio.line_of(handle_hit.start()), "call": "server.handle(&msg)"},
        "framing_error_codes": codes,
        "max_line_bytes_expr": cap.group(1).strip() if cap else None,
    }

    # --- live HTTP shell ---
    drain_hit = need(live, r"router\.handle_on\(core", "live drain handle_on call")
    port = re.search(r"pub\s+const\s+PORT:\s*u16\s*=\s*(\d+)", live.raw)
    statuses = []
    hc = re.search(r"fn\s+handle_connection\s*\(", live.masked)
    h_start, h_end = fn_span(live, hc)
    for m in re.finditer(
        r"write_(?:response|json)\(&mut stream,\s*(\d+)", live.masked[h_start:h_end]
    ):
        statuses.append(
            {"status": int(m.group(1)), "line": live.line_of(h_start + m.start())}
        )
    live_codes = {}
    for code in ("-32700", "-32600"):
        m = re.search(re.escape(code), live.masked)
        live_codes[code] = live.line_of(m.start()) if m else None
    live_info = {
        "file": "src/live_mcp.rs",
        "port": int(port.group(1)) if port else None,
        "dispatch_call": {"line": live.line_of(drain_hit.start()), "call": "router.handle_on(core, &req.msg) via drain()"},
        "http_statuses_in_handle_connection": statuses,
        "framing_error_codes": live_codes,
    }

    # --- GUI glue ---
    poll_hit = need(main, r"live_mcp::drain\(", "GUI poll drain call")
    transports = {"stdio": stdio_info, "live_http": live_info}
    invocations = [
        {
            "point": "stdio binary main loop",
            "file": "src/bin/mcp_server.rs",
            "line": stdio.line_of(handle_hit.start()),
            "kind": "production",
            "via": "McpServer::handle (server's own core)",
        },
        {
            "point": "live HTTP drain (listener queue -> UI thread)",
            "file": "src/live_mcp.rs",
            "line": live.line_of(drain_hit.start()),
            "kind": "production",
            "via": "McpServer::handle_on (live core)",
        },
        {
            "point": "GUI poll loop drains the live queue (<= 8 per tick)",
            "file": "src/main.rs",
            "line": main.line_of(poll_hit.start()),
            "kind": "production",
            "via": "live_mcp::drain(&mut s.core, router, 8)",
        },
    ]
    for pattern, label in (
        (r"\.handle\(", "McpServer::handle"),
        (r"\.handle_on\(", "McpServer::handle_on"),
        (r"\.handler\)\(", "direct handler call"),
    ):
        for hit in scan_hits(sources, pattern, "all"):
            if hit["kind"] != "test":
                continue
            invocations.append(
                {
                    "point": f"test suite drives {label}",
                    "file": hit["file"],
                    "line": hit["line"],
                    "kind": "test",
                    "via": label,
                }
            )
    invocations.sort(key=lambda h: (h["file"], h["line"], h["point"]))
    checks.append("map: stdio handle, live handle_on and GUI poll drain anchors all found")

    consumers = []
    for hit in scan_hits(sources, r"all_tools\(\)", "src"):
        consumers.append(hit)
    consumers.sort(key=lambda h: (h["file"], h["line"]))
    return transports, {"invocations": invocations, "registry_consumers": consumers}, checks


# ---------------------------------------------------------------------
# Unregistered handlers (informational — never in the totals)
# ---------------------------------------------------------------------

# `pub fn`, or a restricted visibility — `pub(crate) fn`,
# `pub(super) fn`, … — so a domain handler declared with
# one is not silently missing from the splice checklist.
# (No such declaration exists today; this keeps it that
# way loudly instead of blind.)
_HANDLER_SIG = re.compile(
    r"pub(?:\s*\([^)]*\))?\s+fn\s+(tool_\w+)\s*\(", re.M
)


def extract_unregistered(
    sources: dict[str, Source], registered_defs: set[tuple[str, str]]
) -> list[dict]:
    out = []
    skip = {"src/mcp.rs", "src/mcp_tools.rs", "src/mcp_io.rs"}
    for rel, src in sorted(sources.items()):
        if rel in skip or not rel.startswith("src/"):
            continue
        for m in _HANDLER_SIG.finditer(src.masked):
            line = src.line_of(m.start())
            if src.test_line and line >= src.test_line:
                continue
            # Balanced argument list + return type check: the
            # registry handler shape only.
            depth = 0
            i = m.end() - 1
            while i < len(src.masked):
                if src.masked[i] == "(":
                    depth += 1
                elif src.masked[i] == ")":
                    depth -= 1
                    if depth == 0:
                        break
                i += 1
            args = src.masked[m.end():i]
            tail = src.masked[i:i + 60]
            if "EditorCore" not in args or "&mut" not in args:
                continue
            if not re.search(r"->\s*Result<Value,\s*String>", tail):
                continue
            fn_name = m.group(1)
            if (rel, fn_name) in registered_defs:
                continue
            out.append({"file": rel, "line": line, "symbol": fn_name})
    out.sort(key=lambda h: (h["file"], h["line"]))
    return out


# ---------------------------------------------------------------------
# Assembly + output
# ---------------------------------------------------------------------

def repo_root() -> str:
    guess = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    if os.path.isfile(os.path.join(guess, "src", "mcp.rs")):
        return guess
    return os.getcwd()


def build(root: str) -> dict:
    rels = ["src/mcp.rs", "src/mcp_tools.rs", "src/mcp_io.rs",
            "src/live_mcp.rs", "src/bin/mcp_server.rs", "src/main.rs"]
    extra = sorted(
        os.path.relpath(p, root)
        for p in glob.glob(os.path.join(root, "src", "**", "*.rs"), recursive=True)
    )
    test_rels = sorted(
        os.path.relpath(p, root)
        for p in glob.glob(os.path.join(root, "tests", "*.rs"))
    )
    sources: dict[str, Source] = {}
    for rel in dict.fromkeys(rels + extra + test_rels):
        sources[rel] = Source(root, rel)

    checks: list[str] = []
    builtins, c = extract_builtins(sources["src/mcp.rs"])
    checks += c
    registry, c = extract_registry_file(sources["src/mcp_tools.rs"], "registry", sources)
    checks += c
    io_tools, c = extract_registry_file(sources["src/mcp_io.rs"], "io", sources)
    checks += c

    tools = builtins + registry + io_tools
    seen: dict[str, str] = {}
    for t in tools:
        if t["name"] in seen:
            fail(f"duplicate tool name {t['name']!r} in families {seen[t['name']]} and {t['family']}")
        seen[t["name"]] = t["family"]
    checks.append(f"global: {len(tools)} tool names, no duplicates across families")
    n_src = sum(1 for r in sources if r.startswith("src/"))
    checks.append(
        f"sources: {len(sources)} files in the scan set "
        f"({n_src} under src/, {len(sources) - n_src} under tests/) — "
        "handler definitions resolve across all of them"
    )

    dispatch, c = extract_dispatch(sources["src/mcp.rs"])
    checks += c
    transports, inv_map, c = extract_transports_and_map(sources)
    checks += c

    registered_defs = {
        (t["handler"]["file"], t["handler"]["symbol"])
        for t in registry + io_tools
        if not t["handler"]["symbol"].startswith("closure")
    }
    unregistered = extract_unregistered(sources, registered_defs)
    sym_seen: dict[str, int] = {}
    for h in unregistered:
        sym_seen[h["symbol"]] = sym_seen.get(h["symbol"], 0) + 1
    unregistered_dupes = sorted(s for s, n in sym_seen.items() if n > 1)

    by_family = {
        fam: sorted((t for t in tools if t["family"] == fam), key=lambda t: t["name"])
        for fam in FAMILY_ORDER
    }
    return {
        "totals": {
            "builtin": len(by_family["builtin"]),
            "registry": len(by_family["registry"]),
            "io": len(by_family["io"]),
            "total": len(tools),
        },
        "tools_by_family": by_family,
        "dispatch": dispatch,
        "transports": transports,
        "invocation_map": inv_map,
        "unregistered_handlers": unregistered,
        "unregistered_duplicate_symbols": unregistered_dupes,
        "verification": checks,
    }


def render_text(data: dict) -> str:
    lines = []
    t = data["totals"]
    lines.append("MCP surface census — StykAnimate-RS")
    lines.append(
        f"Totals: {t['total']} callable tools "
        f"(builtin {t['builtin']} + registry {t['registry']} + io {t['io']})"
    )
    # One greppable line per family, so a close-out can
    # lift a subtotal without parsing the combined line.
    lines.append("[family counts]")
    for fam in FAMILY_ORDER:
        lines.append(f"  {fam}: {t[fam]}")
    lines.append(f"  total: {t['total']}")
    lines.append("")
    for fam in FAMILY_ORDER:
        lines.append(f"[{fam}] ({t[fam]})")
        for tool in data["tools_by_family"][fam]:
            d, h = tool["defined"], tool["handler"]
            lines.append(
                f"  {tool['name']}  def {d['file']}:{d['line']} ({d['symbol']})"
                f"  handler {h['file']}:{h['line']} ({h['symbol']})"
            )
            lines.append(f"      {tool['purpose']}")
        lines.append("")
    d = data["dispatch"]
    lines.append("[dispatch] McpServer::dispatch (src/mcp.rs)")
    for m in d["methods"]:
        lines.append(f"  method {m['method']}  src/mcp.rs:{m['line']}")
    lines.append(
        f"  notification (no id) -> no reply  src/mcp.rs:{d['notification_rule_line']}"
    )
    for c in d["dispatch_error_codes"]:
        lines.append(f"  dispatch error {c['code']}  src/mcp.rs:{c['line']}")
    for c in d["tools_call_error_codes"]:
        lines.append(f"  tools/call error {c['code']}  src/mcp.rs:{c['line']}")
    if d["handler_error_shape"]["line"]:
        lines.append(
            "  handler Err -> isError result  "
            f"src/mcp.rs:{d['handler_error_shape']['line']}"
        )
    lines.append(f"  protocolVersion {d['protocol_version']}")
    lines.append("")
    lines.append("[transports]")
    for name, tr in data["transports"].items():
        lines.append(f"  {name}: {tr['file']}")
        dc = tr["dispatch_call"]
        lines.append(f"    dispatch: {dc['call']}  line {dc['line']}")
        for code, line in sorted(tr["framing_error_codes"].items()):
            if line:
                lines.append(f"    framing error {code}  line {line}")
        if "max_line_bytes_expr" in tr and tr["max_line_bytes_expr"]:
            lines.append(f"    max line bytes: {tr['max_line_bytes_expr']}")
        if "port" in tr and tr["port"]:
            lines.append(f"    port: {tr['port']}")
        for s in tr.get("http_statuses_in_handle_connection", []):
            lines.append(f"    HTTP {s['status']}  line {s['line']}")
    lines.append("")
    lines.append("[invocation map]")
    for inv in data["invocation_map"]["invocations"]:
        lines.append(
            f"  {inv['point']}  {inv['file']}:{inv['line']}  [{inv['kind']}] via {inv['via']}"
        )
    lines.append("  registry consumers (all_tools()):")
    for hit in data["invocation_map"]["registry_consumers"]:
        lines.append(f"    {hit['file']}:{hit['line']}  [{hit['kind']}]")
    lines.append("")
    lines.append(
        f"[unregistered handlers] ({len(data['unregistered_handlers'])}) — "
        "handler-shaped, NOT callable until spliced into mcp_tools::tools()"
    )
    for h in data["unregistered_handlers"]:
        lines.append(f"  {h['symbol']}  {h['file']}:{h['line']}")
    for sym in data["unregistered_duplicate_symbols"]:
        lines.append(f"  WARNING duplicate unregistered symbol: {sym} (defined in more than one domain file)")
    lines.append("")
    lines.append("[verification]")
    for check in data["verification"]:
        lines.append(f"  PASS {check}")
    return "\n".join(lines) + "\n"


def main(argv: list[str]) -> int:
    root = repo_root()
    try:
        data = build(root)
    except (CensusError, ValueError, OSError) as exc:
        print(f"mcp_census: REFUSING to print a census: {exc}", file=sys.stderr)
        return 2
    if "--json" in argv:
        print(json.dumps(data, indent=2, sort_keys=False))
    else:
        sys.stdout.write(render_text(data))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
