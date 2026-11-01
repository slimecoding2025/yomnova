#!/usr/bin/env python3
"""Generate the YOMNOVA logo set (pure SVG paths, no fonts, no dependencies).

Usage: python3 scripts/generate-brand.py   (writes into public/brand/)

Concept: three hollow nodes (many systems) converge along strokes into one solid
node (one language) -- a "Y". The wordmark is drawn as monoline geometry so it
renders identically everywhere without a font.
"""
import math
import os

OUT = os.path.join(os.path.dirname(__file__), "..", "public", "brand")
os.makedirs(OUT, exist_ok=True)

DARK = {"ink": "#F2EEE6", "accent": "#19E3FF", "bg": "#07080B"}
LIGHT = {"ink": "#0B0D12", "accent": "#0877A0", "bg": "#FFFFFF"}


def symbol_body(c, stroke=5.0, ring_r=5.0, solid_ends=False):
    """Symbol drawn in a 64x64 box."""
    ax, ay = 13.0, 11.0     # left node
    bx, by = 51.0, 11.0     # right node
    vx, vy = 32.0, 35.0     # vertex
    sx, sy = 32.0, 54.0     # stem node
    dx, dy = vx - ax, vy - ay
    n = math.hypot(dx, dy)
    ux, uy = dx / n, dy / n
    inset = 0 if solid_ends else ring_r + 1.0
    l0 = (ax + ux * inset, ay + uy * inset)
    r0 = (bx - ux * inset, by + uy * inset)
    stem_end = sy - (ring_r + 1.0 if not solid_ends else 0) - 0  # stem ends at the node
    parts = []
    parts.append(
        f'<path d="M{l0[0]:.2f} {l0[1]:.2f} L{vx} {vy}" stroke="{c["ink"]}" stroke-width="{stroke}" stroke-linecap="round" fill="none"/>'
    )
    parts.append(
        f'<path d="M{r0[0]:.2f} {r0[1]:.2f} L{vx} {vy}" stroke="{c["accent"]}" stroke-width="{stroke}" stroke-linecap="round" fill="none"/>'
    )
    parts.append(
        f'<path d="M{vx} {vy} L{sx} {sy - 1}" stroke="{c["ink"]}" stroke-width="{stroke}" stroke-linecap="round" fill="none"/>'
    )
    if solid_ends:
        parts.append(f'<circle cx="{ax}" cy="{ay}" r="{ring_r}" fill="{c["ink"]}"/>')
        parts.append(f'<circle cx="{bx}" cy="{by}" r="{ring_r}" fill="{c["accent"]}"/>')
    else:
        parts.append(f'<circle cx="{ax}" cy="{ay}" r="{ring_r}" fill="none" stroke="{c["ink"]}" stroke-width="3"/>')
        parts.append(f'<circle cx="{bx}" cy="{by}" r="{ring_r}" fill="none" stroke="{c["accent"]}" stroke-width="3"/>')
    parts.append(f'<circle cx="{sx}" cy="{sy}" r="{ring_r + 1}" fill="{c["accent"]}"/>')
    return "\n    ".join(parts)


# Monoline letter geometry, cap height 40. Each: (width, path d)
LETTERS = {
    "Y": (22, "M1 1 L11 21 L21 1 M11 21 L11 39"),
    "O": (24, "M12 1 H12 A11 11 0 0 1 23 12 V28 A11 11 0 0 1 12 39 A11 11 0 0 1 1 28 V12 A11 11 0 0 1 12 1 Z"),
    "M": (26, "M1 39 V1 L13 25 L25 1 V39"),
    "N": (24, "M1 39 V1 L23 39 V1"),
    "V": (24, "M1 1 L12 39 L23 1"),
    "A": (24, "M1 39 L12 1 L23 39 M5.5 27 H18.5"),
}


def wordmark_body(c, stroke=4.6, gap=9.0):
    x = 0.0
    parts = []
    for ch in "YOMNOVA":
        w, d = LETTERS[ch]
        parts.append(
            f'<path transform="translate({x:.1f} 0)" d="{d}" stroke="{c["ink"]}" stroke-width="{stroke}" '
            f'stroke-linecap="round" stroke-linejoin="round" fill="none"/>'
        )
        x += w + gap
    total = x - gap
    # accent: the final letter's crossbar-free apex dot echoes the symbol's solid node
    return "\n    ".join(parts), total


def write(name, svg):
    with open(os.path.join(OUT, name), "w") as f:
        f.write(svg)


def svg_open(w, h, title, extra=""):
    return (
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}" width="{w}" height="{h}" role="img" aria-label="{title}"{extra}>\n'
        f"  <title>{title}</title>\n"
    )


def symbol_svg(c):
    return svg_open(64, 64, "YOMNOVA symbol") + f"  <g>\n    {symbol_body(c)}\n  </g>\n</svg>\n"


def wordmark_svg(c):
    body, w = wordmark_body(c)
    return svg_open(round(w + 4), 42, "YOMNOVA") + f'  <g transform="translate(2 1)">\n    {body}\n  </g>\n</svg>\n'


def primary_svg(c):
    body, w = wordmark_body(c, stroke=4.8, gap=8.5)
    scale_w = 0.62  # wordmark height 40*0.62 ~ 25
    sym_h = 56
    gap = 22
    wm_w = w * scale_w
    total_w = sym_h + gap + wm_w + 4
    h = 64
    return (
        svg_open(round(total_w), h, "YOMNOVA")
        + f'  <g transform="translate(0 4) scale({sym_h / 64:.4f})">\n    {symbol_body(c)}\n  </g>\n'
        + f'  <g transform="translate({sym_h + gap} {(h - 40 * scale_w) / 2:.2f}) scale({scale_w})">\n    {body}\n  </g>\n</svg>\n'
    )


def favicon_svg():
    c = DARK
    return (
        svg_open(64, 64, "YOMNOVA")
        + f'  <rect width="64" height="64" rx="14" fill="{c["bg"]}"/>\n'
        + f'  <g transform="translate(4 4) scale(0.875)">\n    {symbol_body(c, stroke=7.0, ring_r=6.5, solid_ends=True)}\n  </g>\n</svg>\n'
    )


def og_svg():
    c = DARK
    body, w = wordmark_body(c, stroke=5.6, gap=11)
    ws = 0.0 + 1.9
    return f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1200 630" width="1200" height="630" role="img" aria-label="YOMNOVA - ONE LANGUAGE. EVERY SYSTEM.">
  <title>YOMNOVA - ONE LANGUAGE. EVERY SYSTEM.</title>
  <defs>
    <pattern id="g" width="48" height="48" patternUnits="userSpaceOnUse">
      <path d="M48 0H0V48" fill="none" stroke="#FFFFFF" stroke-opacity="0.045" stroke-width="1"/>
    </pattern>
    <radialGradient id="glow" cx="26%" cy="46%" r="46%">
      <stop offset="0" stop-color="#19E3FF" stop-opacity="0.22"/>
      <stop offset="1" stop-color="#19E3FF" stop-opacity="0"/>
    </radialGradient>
  </defs>
  <rect width="1200" height="630" fill="{c["bg"]}"/>
  <rect width="1200" height="630" fill="url(#g)"/>
  <rect width="1200" height="630" fill="url(#glow)"/>
  <g transform="translate(120 155) scale(5)">
    {symbol_body(c)}
  </g>
  <g transform="translate(520 236) scale({ws})">
    {body}
  </g>
  <text x="522" y="378" fill="{c["accent"]}" font-family="ui-monospace, 'SF Mono', Menlo, Consolas, 'DejaVu Sans Mono', monospace" font-size="27" letter-spacing="4">ONE LANGUAGE. EVERY SYSTEM.</text>
  <text x="522" y="430" fill="#F2EEE6" fill-opacity="0.62" font-family="ui-monospace, 'SF Mono', Menlo, Consolas, 'DejaVu Sans Mono', monospace" font-size="20" letter-spacing="1">An experimental hybrid programming language</text>
  <line x1="522" y1="478" x2="1080" y2="478" stroke="#FFFFFF" stroke-opacity="0.14"/>
  <text x="522" y="520" fill="#F2EEE6" fill-opacity="0.45" font-family="ui-monospace, 'SF Mono', Menlo, Consolas, 'DejaVu Sans Mono', monospace" font-size="18" letter-spacing="2">github.com/slimecoding2025/yomnova</text>
</svg>
'''


write("symbol.svg", symbol_svg(DARK))
write("symbol-light.svg", symbol_svg(LIGHT))
write("wordmark.svg", wordmark_svg(DARK))
write("wordmark-light.svg", wordmark_svg(LIGHT))
write("logo.svg", primary_svg(DARK))
write("logo-light.svg", primary_svg(LIGHT))
write("favicon.svg", favicon_svg())
write("og-image.svg", og_svg())
print("wrote", sorted(os.listdir(OUT)))
