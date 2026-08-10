"""
Composes the Bambe banners from the real logo artwork.

    python assets/brand/build-banner.py

The stag paths are read straight out of assets/icons/bambe_logo.svg rather than
copied, so a change to the logo reruns into matching banners instead of quietly
leaving them out of step.

Three canvases, because they are not the same picture:

    bambe-fb-feed.svg     1200x630   Facebook link/feed post and ad (1.91:1)
    bambe-fb-square.svg   1080x1080  Facebook and Instagram square placement
    bambe-banner.svg      1280x400   wide strip for the repo header

The two Facebook ones carry the offer and a Shop Now call to action; the repo
strip is just the brand. Facebook crops a 1.91:1 image out of anything wider,
so the wide strip would lose its right-hand third in a feed — hence separate
files rather than one image resized.

This writes SVGs only. The PNGs are those files rendered at 2x with the
webfonts actually loaded — a bare SVG has no Rubik or Nunito Sans and falls
back to whatever the viewer happens to have installed:

    chrome --headless=new --hide-scrollbars --default-background-color=00000000 \\
           --force-device-scale-factor=2 --window-size=<W>,<H> \\
           --screenshot=out.png <page inlining the svg + Google Fonts>
"""
import re, pathlib

ROOT = pathlib.Path(__file__).resolve().parents[2]
LOGO = ROOT / "assets" / "icons" / "bambe_logo.svg"
OUTDIR = ROOT / "assets" / "brand"

# ── Offer copy ───────────────────────────────────────────────
# Lifted from the storefront so the ad and the landing page cannot disagree:
# the promo strip (pages/shopping/home.html) and the hero tag. A campaign that
# promises a code the site does not honour is worse than no campaign, so if the
# sale changes, change it in both places.
EYEBROW  = "SUMMER COLLECTION 2026"
HEADLINE = ["Style that", "defines you."]   # "you." is set in the accent
OFFER    = "Up to 50% off  ·  Free shipping on orders $50+"
CTA      = "Shop Now"
CODE_PRE = "Use code"
CODE     = "SUMMER50"

src = LOGO.read_text(encoding="utf-8")
VB_W, VB_H = 351.68, 492.35
paths = re.findall(r'<path[^>]*\sd="([^"]+)"', src)
assert len(paths) == 4, f"expected 4 logo paths, found {len(paths)}"
STAG_BODY = "".join(f'<path d="{d}"/>' for d in paths)


def stag(x, y, height, fill):
    """The mark, scaled to `height` and dropped at (x, y) — its top-left."""
    s = height / VB_H
    return (f'<g transform="translate({x:.2f} {y:.2f}) scale({s:.6f})" '
            f'fill="{fill}">{STAG_BODY}</g>')


# ── Brand tokens, lifted from assets/css/common.css ──────────
SURFACE    = "#FCFBF9"
BORDER     = "#E8E5E0"
BLACK      = "#1A1A18"
DARK       = "#32302C"
SUBTLE     = "#94908A"
PRIMARY    = "#FAAA39"
PRIMARY_DK = "#E8951F"
ON_PRIMARY = "#2A1B06"
PRIMARY_LT = "#FEF1DE"   # flattened rgba(250,170,57,0.13) over --surface

FONT_HEAD = "Rubik, 'Segoe UI', 'Trebuchet MS', sans-serif"
FONT_BODY = "'Nunito Sans', 'Segoe UI', Helvetica, sans-serif"

# The navbar's mark sits in a 32x34 box under `contain`, so a 0.714-aspect
# silhouette fits to height and the leftover width becomes padding the type
# tucks against. `.nav-logo` says `gap: 0`, but the rendered gap is not zero —
# dropping it butts "ambe" straight into the stag's haunch.
GAP_RATIO  = (32 - 34 * VB_W / VB_H) / 2 / 34
# 18px type against a 34px mark, sharing a baseline (`.nav-logo-text`).
WORD_RATIO = 18 / 34


def lockup(x, baseline, mark_h, fill):
    """Mark + wordmark, in the navbar's proportions."""
    mark_w = VB_W / VB_H * mark_h
    return (
        stag(x, baseline - mark_h, mark_h, fill)
        + f'<text x="{x + mark_w + GAP_RATIO * mark_h:.2f}" y="{baseline}" '
          f'font-family="{FONT_HEAD}" font-size="{mark_h * WORD_RATIO:.2f}" '
          f'font-weight="400" fill="{fill}">ambe</text>'
    )


# Advance width of "ambe" in Rubik 400, in ems, read off a render. Needed only
# to centre the lockup on the square, where being a couple of px out is
# invisible — everything else is anchored from a known edge.
AMBE_EM = 2.571


def eyebrow(x, y, h, size, ls):
    """The starred pill. Width is generous rather than measured — the type is
    centred inside it, so a loose fit reads as padding, not as a mistake."""
    w = len(EYEBROW) * (size * 0.62 + ls) + h * 1.9
    star_s = size * 1.15 / 12
    sx, sy = x + h * 0.62, y + (h - size * 1.15) / 2
    return (
        f'<rect x="{x}" y="{y}" width="{w:.1f}" height="{h}" rx="{h/2}" fill="{PRIMARY_LT}"/>'
        f'<g transform="translate({sx:.1f} {sy:.1f}) scale({star_s:.4f})" fill="{PRIMARY_DK}">'
        f'<path d="M6 1l1.2 3.6H11L8.2 6.5l1 3.5L6 8.2 2.8 10l1-3.5L1 4.6h3.8L6 1Z"/></g>'
        f'<text x="{sx + size * 1.15 + h * 0.42:.1f}" y="{y + h/2 + size * 0.36:.1f}" '
        f'font-family="{FONT_BODY}" font-size="{size}" font-weight="700" '
        f'letter-spacing="{ls}" fill="{PRIMARY_DK}">{EYEBROW}</text>'
    ), w


def cta(x, y, h, size):
    """Shop Now pill with the hero's arrow glyph."""
    w = len(CTA) * size * 0.60 + h * 1.85
    arrow_s = size * 1.0 / 16
    ax = x + w - h * 0.78 - size
    return (
        f'<rect x="{x}" y="{y}" width="{w:.1f}" height="{h}" rx="{h/2}" fill="url(#cta)"/>'
        f'<text x="{x + h * 0.72:.1f}" y="{y + h/2 + size * 0.35:.1f}" font-family="{FONT_HEAD}" '
        f'font-size="{size}" font-weight="600" fill="{ON_PRIMARY}">{CTA}</text>'
        f'<g transform="translate({ax:.1f} {y + h/2 - size/2:.1f}) scale({arrow_s:.4f})" '
        f'fill="none" stroke="{ON_PRIMARY}" stroke-width="1.8" stroke-linecap="round" '
        f'stroke-linejoin="round"><path d="M3 8h10M9 4l4 4-4 4"/></g>'
    ), w


def code_chip(x, y, h, size):
    """`Use code SUMMER50` on a dashed chip — reads as something to tear off."""
    label = f"{CODE_PRE} "
    w = (len(label) * size * 0.52) + (len(CODE) * size * 0.66) + h * 1.1
    ty = y + h/2 + size * 0.35
    return (
        f'<rect x="{x + 0.75}" y="{y + 0.75}" width="{w:.1f}" height="{h - 1.5}" rx="{h/2}" '
        f'fill="none" stroke="{PRIMARY}" stroke-width="1.5" stroke-dasharray="5 4"/>'
        f'<text x="{x + h * 0.55:.1f}" y="{ty:.1f}" font-family="{FONT_BODY}" font-size="{size}" '
        f'font-weight="400" fill="{DARK}">{label}'
        f'<tspan font-family="{FONT_HEAD}" font-weight="700" fill="{BLACK}" '
        f'letter-spacing="0.6">{CODE}</tspan></text>'
    ), w


def rings(cx, cy, radii):
    circles = "".join(f'<circle cx="{cx:.1f}" cy="{cy:.1f}" r="{r}"/>' for r in radii)
    return f'<g fill="none" stroke="#FFFFFF" stroke-opacity="0.16">{circles}</g>'


def document(w, h, r, body):
    return f'''<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}" role="img" aria-label="Bambe — style that defines you">
  <title>Bambe</title>
  <defs>
    <linearGradient id="panel" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0" stop-color="{PRIMARY}"/><stop offset="1" stop-color="{PRIMARY_DK}"/>
    </linearGradient>
    <linearGradient id="cta" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0" stop-color="{PRIMARY}"/><stop offset="1" stop-color="{PRIMARY_DK}"/>
    </linearGradient>
    <!-- Rounds the whole canvas, panel corners included, without per-corner
         path maths on the panel itself. -->
    <clipPath id="frame"><rect x="0" y="0" width="{w}" height="{h}" rx="{r}"/></clipPath>
  </defs>
  <g clip-path="url(#frame)">
    <rect x="0" y="0" width="{w}" height="{h}" fill="{SURFACE}"/>
    {body}
    <rect x="0.5" y="0.5" width="{w-1}" height="{h-1}" rx="{r}" fill="none" stroke="{BORDER}"/>
  </g>
</svg>
'''


def headline(x, baseline, size, leading, anchor="start"):
    """Two lines, with `you.` in the accent as the hero sets it."""
    l1, l2 = HEADLINE
    head, tail = l2.rsplit(" ", 1)
    common = (f'font-family="{FONT_HEAD}" font-size="{size}" font-weight="700" '
              f'fill="{BLACK}" text-anchor="{anchor}"')
    return (
        f'<text x="{x}" y="{baseline}" {common}>{l1}</text>'
        f'<text x="{x}" y="{baseline + leading}" {common}>{head} '
        f'<tspan font-style="italic" fill="{PRIMARY_DK}">{tail}</tspan></text>'
    )


# ═════════════════════════════════════════════════════════════
# 1200x630 — Facebook feed / link post / ad
# ═════════════════════════════════════════════════════════════
def feed():
    W, H, PAD = 1200, 630, 80
    PANEL_X = 748
    PANEL_MID = (PANEL_X + W) / 2
    STAG_H = 430

    eb, _ = eyebrow(PAD, 191, 34, 12, 2.4)
    c, cw = cta(PAD, 483, 58, 17)
    chip, _ = code_chip(PAD + cw + 18, 483, 58, 16)

    body = (
        f'<rect x="{PANEL_X}" y="0" width="{W - PANEL_X}" height="{H}" fill="url(#panel)"/>'
        + rings(PANEL_MID, H / 2, (250, 190, 130))
        + stag(PANEL_MID - VB_W / VB_H * STAG_H / 2, (H - STAG_H) / 2, STAG_H, "#FFFFFF")
        + lockup(PAD, 157, 62, BLACK)
        + eb
        + headline(PAD, 305, 72, 82)
        + f'<text x="{PAD}" y="443" font-family="{FONT_BODY}" font-size="18" '
          f'font-weight="400" fill="{DARK}">{OFFER}</text>'
        + c + chip
    )
    # Square corners, deliberately. Facebook re-encodes uploads to JPEG, which
    # has no alpha channel, so rounded corners cut out of a transparent canvas
    # composite to black on the way through. Rounding is the platform's job.
    return "bambe-fb-feed.svg", W, H, document(W, H, 0, body)


# ═════════════════════════════════════════════════════════════
# 1080x1080 — Facebook / Instagram square
# ═════════════════════════════════════════════════════════════
def square():
    """Centred, unlike the other two.

    Left-aligned copy leaves a 1:1 canvas visibly lopsided — the right half of
    the upper block reads as a gap rather than as breathing room. A centre axis
    is also what survives Facebook's square-to-4:5 crops, which trim the sides.
    """
    W, H = 1080, 1080
    CX = W / 2
    PANEL_Y = 620
    STAG_H = 360

    MARK_H = 68
    lockup_w = VB_W / VB_H * MARK_H + GAP_RATIO * MARK_H + MARK_H * WORD_RATIO * AMBE_EM

    eb, ebw = eyebrow(0, 0, 38, 13, 2.6)
    eb, _ = eyebrow(CX - ebw / 2, 190, 38, 13, 2.6)

    _, cw = cta(0, 0, 64, 19)
    _, chw = code_chip(0, 0, 64, 17)
    row_x = CX - (cw + 20 + chw) / 2
    c, _ = cta(row_x, 476, 64, 19)
    chip, _ = code_chip(row_x + cw + 20, 476, 64, 17)

    body = (
        f'<rect x="0" y="{PANEL_Y}" width="{W}" height="{H - PANEL_Y}" fill="url(#panel)"/>'
        + rings(CX, PANEL_Y + (H - PANEL_Y) / 2, (300, 230, 160))
        + stag(CX - VB_W / VB_H * STAG_H / 2, PANEL_Y + (H - PANEL_Y - STAG_H) / 2,
               STAG_H, "#FFFFFF")
        + lockup(CX - lockup_w / 2, 156, MARK_H, BLACK)
        + eb
        + headline(CX, 306, 76, 86, anchor="middle")
        + f'<text x="{CX}" y="440" font-family="{FONT_BODY}" font-size="21" '
          f'font-weight="400" fill="{DARK}" text-anchor="middle">{OFFER}</text>'
        + c + chip
    )
    return "bambe-fb-square.svg", W, H, document(W, H, 0, body)  # see feed()


# ═════════════════════════════════════════════════════════════
# 1280x400 — repo header. Brand only, no offer.
# ═════════════════════════════════════════════════════════════
def strip():
    W, H, PAD = 1280, 400, 96
    PANEL_X = 780
    PANEL_MID = (PANEL_X + W) / 2
    STAG_H = 320

    body = (
        f'<rect x="{PANEL_X}" y="0" width="{W - PANEL_X}" height="{H}" fill="url(#panel)"/>'
        + rings(PANEL_MID, H / 2, (188, 142, 96))
        + stag(PANEL_MID - VB_W / VB_H * STAG_H / 2, (H - STAG_H) / 2, STAG_H, "#FFFFFF")
        + lockup(PAD, 220, 148, BLACK)
        + f'<text x="{PAD}" y="292" font-family="{FONT_BODY}" font-size="28" '
          f'font-weight="300" fill="{DARK}">Style that defines '
          f'<tspan font-style="italic" font-weight="600" fill="{PRIMARY_DK}">you.</tspan></text>'
    )
    return "bambe-banner.svg", W, H, document(W, H, 16, body)


if __name__ == "__main__":
    OUTDIR.mkdir(parents=True, exist_ok=True)
    for build in (feed, square, strip):
        name, w, h, svg = build()
        (OUTDIR / name).write_text(svg, encoding="utf-8")
        print(f"{name:<24} {w}x{h}  {len(svg):>6} bytes")
