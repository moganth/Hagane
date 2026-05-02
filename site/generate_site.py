#!/usr/bin/env python3
"""
Hagane Docs Site Generator
==========================
Reads .md source files from ../docs/ and writes site/ output:
  site/index.html   — generated page
  site/style.css    — extracted stylesheet
  site/script.js    — extracted scripts

Usage (from workspace root):
    python site/generate_site.py

No external dependencies — uses only Python stdlib.
"""

import re, html, pathlib, textwrap

# ─────────────────────────────────────────────────────────────────────────────
# SITE CONFIG — edit this to add/remove/reorder sections.
# ─────────────────────────────────────────────────────────────────────────────

SITE_CONFIG = [
    # ── Top-level (no group header) ────────────────────────────────────────
    {
        "id": "introduction",
        "label": "Introduction",
        "file": "documentation.md",
        "group": None,
        "children": [],
    },
    # ── Getting Started ───────────────────────────────────────────────────────
    {
        "id": "prerequisites",
        "label": "Prerequisites",
        "file": "PREREQUISITES.md",
        "group": "Getting Started",
        "children": [],
    },
    {
        "id": "quickstart",
        "label": "Quick Start",
        "file": "QUICKSTART.md",
        "group": "Getting Started",
        "children": [],
    },
    # ── Theming ─────────────────────────────────────────────────────────────
    {
        "id": "theming",
        "label": "Theming & Presets",
        "file": "THEMING_PRESETS.md",
        "group": "Theming",
        "children": [],
    },
    # ── Pages ────────────────────────────────────────────────────────────────
    {
        "id": "custom_pages",
        "label": "Custom Pages",
        "file": "CUSTOM_PAGES.md",
        "group": "Pages",
        "children": [],
    },
    # ── Logging ──────────────────────────────────────────────────────────────
    {
        "id": "logging",
        "label": "Logging",
        "file": "LOGGING.md",
        "group": "Logging",
        "children": [],
    },
    # ── Reference ────────────────────────────────────────────────────────────
    {
        "id": "error_codes",
        "label": "Error Codes",
        "file": "ERROR_CODES.md",
        "group": "Reference",
        "children": [],
    },
    {
        "id": "testing",
        "label": "Testing",
        "file": "TESTING.md",
        "group": "Reference",
        "children": [],
    },
    {
        "id": "shipping",
        "label": "Shipping Hagane",
        "file": "hagane.md",
        "group": "Reference",
        "children": [],
    },
]

VERSION = "0.1.5"
DOCS_DIR = pathlib.Path(__file__).parent.parent / "docs"
SITE_DIR = pathlib.Path(__file__).parent

OUT_HTML = SITE_DIR / "index.html"
OUT_CSS  = SITE_DIR / "style.css"
OUT_JS   = SITE_DIR / "script.js"

# ─────────────────────────────────────────────────────────────────────────────
# Minimal Markdown → HTML converter (no external deps)
# ─────────────────────────────────────────────────────────────────────────────

def slugify(text: str) -> str:
    text = re.sub(r"[^\w\s-]", "", text.lower())
    return re.sub(r"[\s_]+", "-", text).strip("-")

def md_inline(text: str) -> str:
    """Convert inline Markdown: bold, italic, code, links."""
    parts = re.split(r"(`+)(.*?)\1", text, flags=re.DOTALL)
    result = []
    i = 0
    while i < len(parts):
        if i + 2 < len(parts) and (i == 0 or i % 3 != 0):
            chunk = parts[i]
            chunk = html.escape(chunk)
            chunk = re.sub(r"\*\*\*(.*?)\*\*\*", r"<strong><em>\1</em></strong>", chunk)
            chunk = re.sub(r"\*\*(.*?)\*\*", r"<strong>\1</strong>", chunk)
            chunk = re.sub(r"\*(.*?)\*", r"<em>\1</em>", chunk)
            chunk = re.sub(
                r"\[([^\]]+)\]\(([^)]+)\)",
                lambda m: f'<a href="{html.escape(m.group(2))}" target="_blank" rel="noopener">{m.group(1)}</a>',
                chunk,
            )
            result.append(chunk)
            i += 1
        else:
            if i + 2 < len(parts) and parts[i + 1].startswith("`"):
                result.append(html.escape(parts[i]))
                result.append(f"<code>{html.escape(parts[i+2])}</code>")
                i += 3
            else:
                chunk = html.escape(parts[i])
                result.append(chunk)
                i += 1
    return "".join(result)


def md_to_html(md: str, section_id: str = "") -> tuple[str, list[dict]]:
    """
    Convert Markdown to HTML.
    Returns (html_string, headings_list).
    """
    lines = md.splitlines()
    html_parts = []
    headings = []
    in_code = False
    code_lang = ""
    code_lines = []
    in_list_ul = False
    in_list_ol = False
    in_blockquote = False
    in_table = False
    table_rows = []
    para_lines = []

    def flush_para():
        nonlocal para_lines
        if para_lines:
            content = " ".join(para_lines).strip()
            if content:
                html_parts.append(f"<p>{md_inline(content)}</p>")
            para_lines = []

    def flush_list():
        nonlocal in_list_ul, in_list_ol
        if in_list_ul:
            html_parts.append("</ul>")
            in_list_ul = False
        if in_list_ol:
            html_parts.append("</ol>")
            in_list_ol = False

    def flush_blockquote():
        nonlocal in_blockquote
        if in_blockquote:
            html_parts.append("</div>")
            in_blockquote = False

    def flush_table():
        nonlocal in_table, table_rows
        if not in_table or not table_rows:
            return
        html_parts.append('<div class="table-wrap"><table>')
        if len(table_rows) >= 1:
            cells = [c.strip() for c in table_rows[0].strip("|").split("|")]
            html_parts.append("<thead><tr>")
            for c in cells:
                html_parts.append(f"<th>{md_inline(c)}</th>")
            html_parts.append("</tr></thead>")
        if len(table_rows) >= 3:
            html_parts.append("<tbody>")
            for row in table_rows[2:]:
                cells = [c.strip() for c in row.strip("|").split("|")]
                html_parts.append("<tr>")
                for c in cells:
                    html_parts.append(f"<td>{md_inline(c)}</td>")
                html_parts.append("</tr>")
            html_parts.append("</tbody>")
        html_parts.append("</table></div>")
        in_table = False
        table_rows = []

    for raw_line in lines:
        line = raw_line

        # ── Fenced code block ──
        if line.startswith("```"):
            if not in_code:
                flush_para()
                flush_list()
                flush_blockquote()
                flush_table()
                code_lang = line[3:].strip().lower() or "text"
                code_lines = []
                in_code = True
            else:
                lang_attr = f' data-lang="{html.escape(code_lang)}"' if code_lang else ""
                code_content = html.escape("\n".join(code_lines))
                html_parts.append(f"<pre{lang_attr}><code>{code_content}</code></pre>")
                in_code = False
                code_lang = ""
                code_lines = []
            continue

        if in_code:
            code_lines.append(line)
            continue

        # ── Table ──
        if line.startswith("|"):
            flush_para()
            flush_list()
            flush_blockquote()
            in_table = True
            table_rows.append(line)
            continue
        elif in_table:
            flush_table()

        # ── Blank line ──
        if not line.strip():
            flush_para()
            flush_list()
            flush_blockquote()
            continue

        # ── Standalone image ──
        img_m = re.match(r"^!\[([^\]]*)\]\(([^)]+)\)\s*$", line.strip())
        if img_m:
            flush_para()
            flush_list()
            flush_blockquote()
            flush_table()
            alt_text = html.escape(img_m.group(1))
            src_text = html.escape(img_m.group(2))
            if "hagane-cli" in img_m.group(2):
                html_parts.append(
                    f'<div class="cli-screenshot">'
                    f'<div class="cli-bar">'
                    f'<span class="cli-dot cli-dot-r"></span>'
                    f'<span class="cli-dot cli-dot-y"></span>'
                    f'<span class="cli-dot cli-dot-g"></span>'
                    f'<span class="cli-title">{alt_text}</span>'
                    f'</div>'
                    f'<img src="{src_text}" alt="{alt_text}" loading="lazy"/>'
                    f'</div>'
                )
            else:
                html_parts.append(
                    f'<figure>'
                    f'<img class="doc-img" src="{src_text}" alt="{alt_text}" loading="lazy"/>'
                    f'{"<figcaption>" + alt_text + "</figcaption>" if alt_text else ""}'
                    f'</figure>'
                )
            continue

        # ── Blockquote ──
        if line.startswith("> "):
            flush_para()
            flush_list()
            content = line[2:]
            m = re.match(r"\*\*(.*?)\*\*:(.*)", content)
            if m:
                if not in_blockquote:
                    html_parts.append('<div class="callout callout-info">')
                    in_blockquote = True
                html_parts.append(f"<strong>{html.escape(m.group(1))}</strong>{md_inline(m.group(2).strip())}")
            else:
                if not in_blockquote:
                    html_parts.append('<div class="callout callout-info">')
                    in_blockquote = True
                html_parts.append(f"<p>{md_inline(content)}</p>")
            continue
        elif in_blockquote:
            flush_blockquote()

        # ── Headings ──
        m = re.match(r"(#{1,4})\s+(.*)", line)
        if m:
            flush_para()
            flush_list()
            level = len(m.group(1))
            text = m.group(2).strip()
            anchor = slugify(text)
            if section_id:
                anchor = f"{section_id}-{anchor}"
            headings.append({"level": level, "text": text, "anchor": anchor})
            tag = f"h{level}"
            html_parts.append(
                f'<span class="section-anchor" id="{anchor}"></span>'
                f"<{tag}>{md_inline(text)}</{tag}>"
            )
            continue

        # ── Horizontal rule ──
        if re.match(r"^[-*_]{3,}$", line.strip()):
            flush_para()
            flush_list()
            html_parts.append("<hr/>")
            continue

        # ── Unordered list ──
        m = re.match(r"^(\s*)[-*+]\s+(.*)", line)
        if m:
            flush_para()
            flush_blockquote()
            flush_table()
            if not in_list_ul:
                html_parts.append("<ul>")
                in_list_ul = True
            html_parts.append(f"<li>{md_inline(m.group(2))}</li>")
            continue

        # ── Ordered list ──
        m = re.match(r"^(\s*)\d+\.\s+(.*)", line)
        if m:
            flush_para()
            flush_blockquote()
            flush_table()
            if not in_list_ol:
                html_parts.append("<ol>")
                in_list_ol = True
            html_parts.append(f"<li>{md_inline(m.group(2))}</li>")
            continue

        # ── Normal paragraph text ──
        flush_list()
        flush_blockquote()
        flush_table()
        para_lines.append(line.rstrip())

    # flush remaining
    flush_para()
    flush_list()
    flush_blockquote()
    flush_table()

    return "\n".join(html_parts), headings


# ─────────────────────────────────────────────────────────────────────────────
# Build site
# ─────────────────────────────────────────────────────────────────────────────

def read_md(filename: str) -> str:
    path = DOCS_DIR / filename
    if not path.exists():
        return f"_File `{filename}` not found._"
    return path.read_text(encoding="utf-8")


def render_sections() -> tuple[str, list[dict]]:
    """Render all sections and collect heading metadata for each."""
    sections_html = []
    nav_data = []

    for entry in SITE_CONFIG:
        sid = entry["id"]
        md_raw = read_md(entry["file"])

        body_html, headings = md_to_html(md_raw, sid)

        children = [
            {"text": h["text"], "anchor": h["anchor"]}
            for h in headings
            if h["level"] == 2
        ]

        nav_data.append({
            "id": sid,
            "label": entry["label"],
            "group": entry.get("group"),
            "children": children,
        })

        is_first = (entry == SITE_CONFIG[0])
        sep = "" if is_first else ' style="border-top:1px solid rgba(200,100,30,0.15);padding-top:60px"'
        sections_html.append(
            f'<section class="doc-section" id="{sid}"{sep}>\n'
            f'<span class="section-anchor" id="anchor-{sid}"></span>\n'
            f"{body_html}\n"
            f"</section>\n"
        )

    return "\n".join(sections_html), nav_data


def build_sidebar(nav_data: list[dict]) -> str:
    """Build the sidebar HTML with collapsible groups."""
    groups: dict[str | None, list[dict]] = {}
    for item in nav_data:
        g = item["group"]
        groups.setdefault(g, []).append(item)

    parts = []

    for group_name, items in groups.items():
        if group_name is None:
            parts.append('<div class="nav-group nav-top">')
            parts.append("<ul>")
            for item in items:
                parts.append(f'<li><a href="#{item["id"]}" class="nav-link">{html.escape(item["label"])}</a></li>')
            parts.append("</ul>")
            parts.append("</div>")
        else:
            group_id = slugify(group_name)
            parts.append(f'<div class="nav-group" id="group-{group_id}">')
            parts.append(
                f'<button class="nav-group-btn" onclick="toggleGroup(\'{group_id}\')" aria-expanded="true">'
                f'<span class="nav-group-label">{html.escape(group_name)}</span>'
                f'<span class="nav-chevron">&#9662;</span>'
                f"</button>"
            )
            parts.append(f'<div class="nav-group-body" id="body-{group_id}">')
            parts.append("<ul>")
            for item in items:
                parts.append(
                    f'<li><a href="#{item["id"]}" class="nav-link">{html.escape(item["label"])}</a>'
                )
                if item["children"]:
                    parts.append('<ul class="nav-sub">')
                    for child in item["children"]:
                        parts.append(
                            f'<li><a href="#{child["anchor"]}" class="nav-link nav-sub-link">'
                            f'{html.escape(child["text"])}</a></li>'
                        )
                    parts.append("</ul>")
                parts.append("</li>")
            parts.append("</ul>")
            parts.append("</div>")
            parts.append("</div>")

    return "\n".join(parts)


# ─────────────────────────────────────────────────────────────────────────────
# CSS — written to style.css
# ─────────────────────────────────────────────────────────────────────────────

CSS = """\
/* AUTO-GENERATED — do not edit by hand. Run: python site/generate_site.py */

/* ─── Reset & Base ─── */
*, *::before, *::after { box-sizing: border-box; margin: 0; padding: 0; }
:root {
  --brand:        #C8641E;
  --brand-dark:   #9A4610;
  --brand-glow:   #E07530;
  --brand-dim:    #7A3A0C;
  --bg:           #0C0C0C;
  --sidebar-bg:   #101010;
  --card-bg:      #171717;
  --border:       #242424;
  --border-light: #2E2E2E;
  --text:         #DEDAD6;
  --text-muted:   #7A7068;
  --code-bg:      #111111;
  --code-border:  #1E1E1E;
  --green:        #4EC94E;
  --amber:        #C8960C;
  --red:          #D94040;
  --blue:         #5094E8;
  --sidebar-w:    280px;
  --header-h:     58px;
  --content-max:  860px;
}

html { scroll-behavior: smooth; font-size: 16px; }
body {
  background: var(--bg);
  color: var(--text);
  font-family: 'Segoe UI', system-ui, -apple-system, sans-serif;
  line-height: 1.7;
}
a { color: var(--brand-glow); text-decoration: none; }
a:hover { color: #fff; text-decoration: underline; }
hr { border: none; border-top: 1px solid var(--border); margin: 32px 0; }

/* ─── Header ─── */
.site-header {
  position: fixed; top: 0; left: 0; right: 0; z-index: 100;
  height: var(--header-h);
  background: #0A0A0A;
  border-bottom: 1px solid var(--border);
  display: flex; align-items: center; padding: 0 24px;
  gap: 16px;
}
.logo { display: flex; align-items: center; gap: 12px; text-decoration: none; }
.logo-ascii {
  font-family: 'Courier New', monospace;
  font-size: 13px; font-weight: bold;
  color: var(--brand); letter-spacing: 2px; line-height: 1;
  text-shadow: 0 0 12px rgba(200,100,30,0.5);
}
.logo-tag {
  font-size: 11px; color: var(--text-muted);
  border-left: 1px solid var(--border-light);
  padding-left: 12px; letter-spacing: 0.5px;
}
.header-spacer { flex: 1; }
.header-version {
  font-family: 'Courier New', monospace; font-size: 12px;
  color: var(--text-muted); background: var(--code-bg);
  border: 1px solid var(--border); border-radius: 4px; padding: 3px 9px;
}
.mobile-menu-btn {
  display: none; background: none; border: 1px solid var(--border);
  color: var(--text-muted); border-radius: 4px;
  padding: 6px 10px; cursor: pointer; font-size: 16px;
}

/* ─── Layout ─── */
.layout {
  display: flex;
  margin-top: var(--header-h);
  min-height: calc(100vh - var(--header-h));
}

/* ─── Sidebar ─── */
.sidebar {
  width: var(--sidebar-w); min-width: var(--sidebar-w);
  background: var(--sidebar-bg); border-right: 1px solid var(--border);
  position: sticky; top: var(--header-h);
  height: calc(100vh - var(--header-h));
  overflow-y: auto; overflow-x: hidden;
  padding: 16px 0 40px;
  scrollbar-width: thin; scrollbar-color: var(--border-light) transparent;
}
.sidebar::-webkit-scrollbar { width: 4px; }
.sidebar::-webkit-scrollbar-thumb { background: var(--border-light); border-radius: 2px; }

/* Top-level (ungrouped) nav items */
.nav-top ul { list-style: none; padding: 0 0 8px; }
.nav-top ul li a {
  display: block; padding: 7px 20px 7px 20px;
  font-size: 13.5px; font-weight: 600;
  color: #9A9288; border-left: 2px solid transparent;
  text-decoration: none; transition: color .15s, border-color .15s, background .15s;
}
.nav-top ul li a:hover { color: var(--text); background: rgba(200,100,30,0.06); border-left-color: var(--border-light); }
.nav-top ul li a.active { color: var(--brand-glow); border-left-color: var(--brand); background: rgba(200,100,30,0.1); }

/* Collapsible group */
.nav-group { border-top: 1px solid var(--border); }
.nav-group-btn {
  width: 100%; background: none; border: none; cursor: pointer;
  display: flex; align-items: center; justify-content: space-between;
  padding: 10px 18px 10px 20px;
  color: var(--text-muted); text-align: left;
}
.nav-group-btn:hover { background: rgba(255,255,255,0.03); }
.nav-group-label {
  font-size: 10px; font-weight: 700; letter-spacing: 1.2px;
  text-transform: uppercase; color: var(--text-muted);
}
.nav-chevron {
  font-size: 11px; color: var(--text-muted);
  transition: transform .2s; display: inline-block;
}
.nav-group.collapsed .nav-chevron { transform: rotate(-90deg); }
.nav-group-body { overflow: hidden; transition: max-height .25s ease; max-height: 600px; }
.nav-group.collapsed .nav-group-body { max-height: 0; }
.nav-group-body ul { list-style: none; padding: 2px 0 8px; }
.nav-group-body ul li a.nav-link {
  display: block; padding: 6px 20px 6px 28px;
  font-size: 13.5px; color: #9A9288;
  border-left: 2px solid transparent;
  transition: color .15s, border-color .15s, background .15s;
  text-decoration: none; white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
}
.nav-group-body ul li a.nav-link:hover {
  color: var(--text); background: rgba(200,100,30,0.06); border-left-color: var(--border-light);
}
.nav-group-body ul li a.nav-link.active {
  color: var(--brand-glow); border-left-color: var(--brand); background: rgba(200,100,30,0.1); font-weight: 600;
}
/* Sub-links (H2 headings) */
.nav-sub { list-style: none !important; padding: 0 !important; }
.nav-sub li a.nav-sub-link { padding-left: 40px !important; font-size: 12px !important; color: #787068 !important; }
.nav-sub li a.nav-sub-link.active { color: var(--brand-glow) !important; }

/* ─── Content ─── */
.content {
  flex: 1; padding: 52px 56px 100px;
  min-width: 0; max-width: calc(var(--content-max) + 112px);
}
.doc-section { margin-bottom: 72px; }

/* ─── Typography ─── */
h1 { font-size: 2.2rem; font-weight: 800; color: #fff; line-height: 1.2; margin-bottom: 12px; }
h1 span { color: var(--brand-glow); }
h2 {
  font-size: 1.45rem; font-weight: 700; color: #EEE; margin: 48px 0 16px;
  padding-bottom: 10px; padding-left: 13px;
  border-bottom: 1px solid rgba(200,100,30,0.22);
  border-left: 3px solid var(--brand);
}
h3 { font-size: 1.1rem; font-weight: 600; color: #C89060; margin: 32px 0 12px; }
h4 { font-size: 0.95rem; font-weight: 600; color: var(--text-muted); margin: 24px 0 8px; text-transform: uppercase; letter-spacing: 0.5px; }
p { margin-bottom: 14px; color: var(--text); }
strong { color: #EEE; }
ul, ol { padding-left: 22px; margin-bottom: 14px; }
li { margin-bottom: 5px; }
blockquote { border-left: 3px solid var(--blue); padding: 12px 16px; border-radius: 0 6px 6px 0; margin: 20px 0; background: rgba(80,148,232,0.07); font-size: .92rem; }

/* Inline code */
code {
  font-family: 'Cascadia Code', 'Fira Code', 'Courier New', monospace;
  font-size: 0.85em; background: var(--code-bg);
  border: 1px solid var(--code-border); color: #E8B87A;
  border-radius: 4px; padding: 1px 6px;
}
/* Code blocks */
pre {
  background: var(--code-bg); border: 1px solid var(--code-border);
  border-radius: 8px; padding: 18px 20px; overflow-x: auto;
  margin: 16px 0 22px; position: relative;
}
pre code { background: none; border: none; padding: 0; font-size: 0.88rem; color: #C8C0B8; line-height: 1.65; }
pre[data-lang]::before {
  content: attr(data-lang); position: absolute; top: 0; right: 0;
  font-size: 10px; font-family: 'Courier New', monospace; color: var(--text-muted);
  background: var(--card-bg); border-bottom-left-radius: 6px;
  border-left: 1px solid var(--code-border); border-bottom: 1px solid var(--code-border);
  padding: 3px 10px; letter-spacing: 0.5px;
}

/* Callouts */
.callout {
  border-left: 3px solid; padding: 12px 16px; border-radius: 0 6px 6px 0;
  margin: 20px 0; font-size: 0.92rem;
}
.callout-info  { border-color: var(--blue);  background: rgba(80,148,232,0.07); }
.callout-tip   { border-color: var(--green); background: rgba(78,201,78,0.06); }
.callout-warn  { border-color: var(--amber); background: rgba(200,150,12,0.07); }
.callout-error { border-color: var(--red);   background: rgba(217,64,64,0.07); }
.callout strong { display: block; margin-bottom: 4px; font-size: 0.8rem; text-transform: uppercase; letter-spacing: 0.5px; }
.callout-info strong  { color: var(--blue); }
.callout-tip strong   { color: var(--green); }
.callout-warn strong  { color: var(--amber); }
.callout-error strong { color: var(--red); }

/* Tables */
.table-wrap { overflow-x: auto; margin: 16px 0 22px; border-radius: 8px; border: 1px solid var(--border); }
table { width: 100%; border-collapse: collapse; font-size: 0.88rem; }
thead { background: var(--card-bg); }
thead th { padding: 10px 14px; text-align: left; font-weight: 600; color: #CCC; border-bottom: 1px solid var(--border); white-space: nowrap; }
tbody tr { border-bottom: 1px solid var(--code-border); transition: background .1s; }
tbody tr:last-child { border-bottom: none; }
tbody tr:hover { background: rgba(255,255,255,0.025); }
td { padding: 9px 14px; vertical-align: top; color: var(--text); }
td code { font-size: 0.82rem; }

/* ─── Images ─── */
.doc-img {
  max-width: 100%; border-radius: 8px; border: 1px solid var(--border);
  margin: 16px 0; display: block;
  box-shadow: 0 4px 24px rgba(0,0,0,0.5);
}
figure { margin: 16px 0; }
figcaption { font-size: 12px; color: var(--text-muted); margin-top: 6px; text-align: center; font-style: italic; }

/* ─── CLI terminal screenshots ─── */
.cli-screenshot {
  border: 1px solid var(--border-light); border-radius: 8px;
  overflow: hidden; margin: 20px 0;
  box-shadow: 0 4px 24px rgba(0,0,0,0.5);
}
.cli-bar {
  background: #161616; padding: 8px 14px;
  border-bottom: 1px solid var(--border);
  display: flex; align-items: center; gap: 6px;
  font-family: 'Courier New', monospace; font-size: 11px; color: var(--text-muted);
}
.cli-dot { width: 10px; height: 10px; border-radius: 50%; flex-shrink: 0; }
.cli-dot-r { background: #FF5F56; }
.cli-dot-y { background: #FFBD2E; }
.cli-dot-g { background: #27C93F; }
.cli-title { flex: 1; text-align: center; }
.cli-screenshot img { width: 100%; display: block; background: #000; }

/* Section anchor offset for fixed header */
.section-anchor { display: block; height: var(--header-h); margin-top: calc(-1 * var(--header-h)); visibility: hidden; }

/* Footer */
.site-footer {
  text-align: center; padding: 40px 24px;
  color: var(--text-muted); font-size: 12px;
  border-top: 1px solid var(--border);
}

/* Responsive */
@media (max-width: 900px) {
  .sidebar { display: none; position: fixed; top: var(--header-h); left: 0; bottom: 0; z-index: 90; width: var(--sidebar-w); }
  .sidebar.open { display: block; }
  .content { padding: 32px 24px 80px; }
  .mobile-menu-btn { display: block; }
  h1 { font-size: 1.75rem; }
  h2 { font-size: 1.25rem; }
}
@media (max-width: 600px) {
  .content { padding: 24px 16px 60px; }
}
"""

# ─────────────────────────────────────────────────────────────────────────────
# JS — written to script.js
# ─────────────────────────────────────────────────────────────────────────────

JS = """\
/* AUTO-GENERATED — do not edit by hand. Run: python site/generate_site.py */

function toggleSidebar() {
  document.getElementById('sidebar').classList.toggle('open');
}

document.getElementById('main-content').addEventListener('click', function() {
  document.getElementById('sidebar').classList.remove('open');
});

function toggleGroup(groupId) {
  var el = document.getElementById('group-' + groupId);
  if (el) el.classList.toggle('collapsed');
}

// Active link highlighting on scroll
var allAnchors = document.querySelectorAll('.section-anchor[id]');
var allNavLinks = document.querySelectorAll('.sidebar a.nav-link');

function updateActiveLink() {
  var scrollY = window.scrollY + 80;
  var current = '';
  allAnchors.forEach(function(a) { if (a.offsetTop <= scrollY) current = a.id; });
  allNavLinks.forEach(function(link) {
    var href = link.getAttribute('href');
    if (!href) return;
    var target = href.slice(1);
    link.classList.toggle('active', target === current);
  });
}

window.addEventListener('scroll', updateActiveLink, { passive: true });
updateActiveLink();
"""

# ─────────────────────────────────────────────────────────────────────────────
# HTML Shell — references style.css and script.js
# ─────────────────────────────────────────────────────────────────────────────

HTML_SHELL = """\
<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8"/>
<meta name="viewport" content="width=device-width, initial-scale=1.0"/>
<title>Hagane — Installer Engine Docs</title>
<!-- AUTO-GENERATED — do not edit by hand. Run: python site/generate_site.py -->
<link rel="stylesheet" href="style.css"/>
</head>
<body>

<header class="site-header">
  <button class="mobile-menu-btn" onclick="toggleSidebar()" aria-label="Menu">&#9776;</button>
  <a class="logo" href="#introduction">
    <span class="logo-ascii">HAGANE</span>
    <span class="logo-tag">Installer Engine</span>
  </a>
  <div class="header-spacer"></div>
  <span class="header-version">v{version}</span>
</header>

<div class="layout">
<aside class="sidebar" id="sidebar">
  <nav>
{sidebar}
  </nav>
</aside>

<main class="content" id="main-content">
{sections}
  <footer class="site-footer">
    <p>Hagane Installer Engine &mdash; v{version} &mdash; Built with Rust &amp; WebView2</p>
    <p style="margin-top:6px;font-size:11px;color:#4A4540">
      Generated from <code>.md</code> source files &mdash;
      edit the <code>docs/</code> files, then run <code>python site/generate_site.py</code> to rebuild.
    </p>
  </footer>
</main>
</div>

<script src="script.js"></script>
</body>
</html>
"""


def main():
    print("Hagane Docs Site Generator")
    print(f"  Reading .md files from : {DOCS_DIR}")
    print(f"  Writing output to      : {SITE_DIR}")

    sections_html, nav_data = render_sections()
    sidebar_html = build_sidebar(nav_data)

    html_output = HTML_SHELL.format(
        version=VERSION,
        sidebar=textwrap.indent(sidebar_html, "    "),
        sections=sections_html,
    )

    OUT_CSS.write_text(CSS, encoding="utf-8")
    print(f"  Written: {OUT_CSS.name}")

    OUT_JS.write_text(JS, encoding="utf-8")
    print(f"  Written: {OUT_JS.name}")

    OUT_HTML.write_text(html_output, encoding="utf-8")
    print(f"  Written: {OUT_HTML.name}")

    print(f"  Sections: {len(SITE_CONFIG)}")

    total_headings = sum(len(item["children"]) for item in nav_data)
    print(f"  Sidebar child links: {total_headings}")
    print("Done.")


if __name__ == "__main__":
    main()
