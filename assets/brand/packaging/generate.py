"""Branded packaging illustrations for Bambee, drawn as isometric SVG scenes.

Each scene is written as a standalone HTML page (so the Rubik wordmark loads
from Google Fonts) and rendered to PNG with headless Chrome by render.sh.
"""
import math
import pathlib
import re

OUT = pathlib.Path(__file__).parent
LOGO = pathlib.Path('D:/bambe/assets/icons/bambe_logo.svg').read_text(encoding='utf-8')
LOGO_PATHS = ''.join(re.sub(r'\sclass="[^"]*"', '', p) for p in re.findall(r'<path[^>]*/>', LOGO))
LOGO_AR = 351.68 / 492.35

C, S = math.cos(math.radians(30)), 0.5
W_, H_ = 1200, 900


def P(o, x, y, z):
    return (o[0] + (x - y) * C, o[1] + (x + y) * S - z)


def pts(*ps):
    return ' '.join(f'{x:.1f},{y:.1f}' for x, y in ps)


def m_top(o, h):
    x, y = P(o, 0, 0, h)
    return f'matrix({C},{S},{-C},{S},{x:.1f},{y:.1f})'


def m_left(o, d, h):   # face y = d, u along +x, v downward
    x, y = P(o, 0, d, h)
    return f'matrix({C},{S},0,1,{x:.1f},{y:.1f})'


def m_right(o, w, d, h):   # face x = w, u along -y, v downward
    x, y = P(o, w, d, h)
    return f'matrix({C},{-S},0,1,{x:.1f},{y:.1f})'


def mark(x, y, h, fill):
    w = h * LOGO_AR
    return f'<use href="#stag" x="{x - w / 2:.1f}" y="{y - h / 2:.1f}" width="{w:.1f}" height="{h:.1f}" fill="{fill}"/>'


def lockup(cx, cy, h, fill, text=True):
    """Stag mark standing in for the B, then "ambee", centred on (cx, cy)."""
    if not text:
        return mark(cx, cy, h, fill)
    mw = h * LOGO_AR
    fs = h * 0.62
    tw = fs * 2.62          # measured width of "ambee" in Rubik 400
    total = mw + fs * 0.08 + tw
    x0 = cx - total / 2
    return (f'<use href="#stag" x="{x0:.1f}" y="{cy - h / 2:.1f}" width="{mw:.1f}" height="{h:.1f}" fill="{fill}"/>'
            f'<text x="{x0 + mw + fs * 0.08:.1f}" y="{cy + h / 2 - fs * 0.02:.1f}" font-family="Rubik" font-weight="400" '
            f'font-size="{fs:.1f}" fill="{fill}">ambee</text>')


DEFS = f'''
<symbol id="stag" viewBox="0 0 351.68 492.35">{LOGO_PATHS}</symbol>
<radialGradient id="bg" cx="50%" cy="38%" r="75%">
  <stop offset="0" stop-color="#FFFFFF"/><stop offset=".6" stop-color="#F6F3ED"/><stop offset="1" stop-color="#E9E4DA"/>
</radialGradient>
<filter id="blur30" x="-50%" y="-50%" width="200%" height="200%"><feGaussianBlur stdDeviation="30"/></filter>
<filter id="blur10" x="-50%" y="-50%" width="200%" height="200%"><feGaussianBlur stdDeviation="10"/></filter>
<filter id="blur4" x="-50%" y="-50%" width="200%" height="200%"><feGaussianBlur stdDeviation="4"/></filter>
<filter id="kraft" x="0" y="0" width="100%" height="100%">
  <feTurbulence type="fractalNoise" baseFrequency=".9" numOctaves="3" seed="4" result="n"/>
  <feColorMatrix in="n" type="matrix" values="0 0 0 0 .35  0 0 0 0 .24  0 0 0 0 .12  0 0 0 .22 0"/>
  <feComposite in2="SourceGraphic" operator="in"/>
</filter>
<linearGradient id="gold" x1="0" y1="0" x2="1" y2="1">
  <stop offset="0" stop-color="#F7E3A1"/><stop offset=".35" stop-color="#D9AE4E"/>
  <stop offset=".55" stop-color="#F3D88C"/><stop offset=".8" stop-color="#B88A2E"/><stop offset="1" stop-color="#E9C978"/>
</linearGradient>
<linearGradient id="goldV" x1="0" y1="0" x2="0" y2="1">
  <stop offset="0" stop-color="#F3D88C"/><stop offset=".5" stop-color="#C99A3C"/><stop offset="1" stop-color="#A97C25"/>
</linearGradient>
<linearGradient id="sheenTop" x1="0" y1="0" x2="1" y2="1">
  <stop offset="0" stop-color="#fff" stop-opacity=".22"/><stop offset=".6" stop-color="#fff" stop-opacity="0"/>
</linearGradient>
<linearGradient id="fadeDown" x1="0" y1="0" x2="0" y2="1">
  <stop offset="0" stop-color="#000" stop-opacity="0"/><stop offset="1" stop-color="#000" stop-opacity=".18"/>
</linearGradient>
<linearGradient id="lipShadow" x1="0" y1="0" x2="0" y2="1">
  <stop offset="0" stop-color="#000" stop-opacity=".35"/><stop offset="1" stop-color="#000" stop-opacity="0"/>
</linearGradient>
<linearGradient id="white" x1="0" y1="0" x2="1" y2="0">
  <stop offset="0" stop-color="#E4E4E0"/><stop offset=".3" stop-color="#FFFFFF"/><stop offset=".7" stop-color="#F4F3EF"/><stop offset="1" stop-color="#CFCEC8"/>
</linearGradient>
<linearGradient id="mint" x1="0" y1="0" x2="1" y2="0">
  <stop offset="0" stop-color="#A9CDB3"/><stop offset=".35" stop-color="#E3F1E6"/><stop offset=".7" stop-color="#C5E0CC"/><stop offset="1" stop-color="#8DB89A"/>
</linearGradient>
<linearGradient id="blush" x1="0" y1="0" x2="1" y2="0">
  <stop offset="0" stop-color="#D9A9A3"/><stop offset=".35" stop-color="#F6DCD6"/><stop offset=".7" stop-color="#EDC7C0"/><stop offset="1" stop-color="#C9938C"/>
</linearGradient>
<linearGradient id="goldCyl" x1="0" y1="0" x2="1" y2="0">
  <stop offset="0" stop-color="#A97C25"/><stop offset=".35" stop-color="#F3D88C"/><stop offset=".7" stop-color="#D2A447"/><stop offset="1" stop-color="#8E6519"/>
</linearGradient>
'''


def page(svg_body, name):
    page_one(svg_body, name)
    page_one(re.sub(r'<text x="600.0" y="8(06|46)".*?</text>', '', svg_body), name + '-card', card=True)


def page_one(svg_body, name, card=False):
    view = '150 60 900 675' if card else f'0 0 {W_} {H_}'
    html = f'''<!doctype html><html><head><meta charset="utf-8">
<link href="https://fonts.googleapis.com/css2?family=Rubik:wght@400;500&family=Nunito+Sans:wght@600&display=block" rel="stylesheet">
<style>html,body{{margin:0;padding:0;background:#fff}}svg{{display:block}}</style></head><body>
<svg xmlns="http://www.w3.org/2000/svg" width="{W_}" height="{H_}" viewBox="{view}">
<defs>{DEFS}</defs>
<rect width="{W_}" height="{H_}" fill="url(#bg)"/>
{svg_body}
</svg></body></html>'''
    (OUT / f'{name}.html').write_text(html, encoding='utf-8')


def floor_shadow(o, w, d, spread=40, op=.28):
    a, b, c_, d_ = P(o, -spread * .3, -spread * .3, 0), P(o, w + spread, 0, 0), P(o, w + spread, d + spread, 0), P(o, 0, d + spread, 0)
    return (f'<polygon points="{pts(a, b, c_, d_)}" fill="#3A2E1E" opacity="{op}" filter="url(#blur30)"/>'
            f'<polygon points="{pts(P(o,0,0,0), P(o,w,0,0), P(o,w,d,0), P(o,0,d,0))}" fill="#2A2217" opacity=".35" filter="url(#blur10)"/>')


def box(o, w, d, h, top, left, right, top_extra='', left_extra='', right_extra='', texture=False):
    tex = lambda ww, hh: f'<rect width="{ww}" height="{hh}" fill="#fff" filter="url(#kraft)"/>' if texture else ''
    top_face = '' if top == 'none' else f'''<g transform="{m_top(o, h)}"><rect width="{w}" height="{d}" fill="{top}"/>{tex(w, d)}
  <rect width="{w}" height="{d}" fill="url(#sheenTop)"/>{top_extra}</g>'''
    return f'''
<g transform="{m_left(o, d, h)}"><rect width="{w}" height="{h}" fill="{left}"/>{tex(w, h)}
  <rect width="{w}" height="{h}" fill="url(#fadeDown)"/>{left_extra}</g>
<g transform="{m_right(o, w, d, h)}"><rect width="{d}" height="{h}" fill="{right}"/>{tex(d, h)}
  <rect width="{d}" height="{h}" fill="url(#fadeDown)"/>{right_extra}</g>
{top_face}
<polyline points="{pts(P(o,0,d,h), P(o,w,d,h), P(o,w,0,h))}" fill="none" stroke="#fff" stroke-opacity=".35" stroke-width="1.5"/>
<line x1="{P(o,w,d,h)[0]:.1f}" y1="{P(o,w,d,h)[1]:.1f}" x2="{P(o,w,d,0)[0]:.1f}" y2="{P(o,w,d,0)[1]:.1f}" stroke="#000" stroke-opacity=".12" stroke-width="1.5"/>
'''


def caption(title, sub):
    return (f'<text x="{W_/2}" y="806" text-anchor="middle" font-family="Rubik" font-weight="500" font-size="34" fill="#1B1B1B">{title}</text>'
            f'<text x="{W_/2}" y="846" text-anchor="middle" font-family="Nunito Sans" font-weight="600" font-size="20" fill="#6B675F">{sub}</text>')


# ── 1. Standard: kraft mailer box ─────────────────────────────────
def standard():
    w, d, h = 360, 270, 120
    o = (600 - (w - d) * C / 2, 330)
    top_extra = (f'<g transform="translate({w/2},{d/2}) rotate(0)">{lockup(0, 0, 92, "#1B1B1B")}</g>'
                 f'<rect x="0" y="{d-16}" width="{w}" height="16" fill="#000" opacity=".06"/>')
    # The mailer's front tuck flap: a slightly darker band along the top of the front face.
    left_extra = (f'<rect width="{w}" height="22" fill="#000" opacity=".08"/>'
                  f'<rect y="22" width="{w}" height="3" fill="url(#lipShadow)" opacity=".7"/>'
                  # The green seal sticker over the flap's join.
                  f'<circle cx="{w/2}" cy="24" r="27" fill="#1A8338"/>'
                  f'<circle cx="{w/2}" cy="24" r="22" fill="none" stroke="#fff" stroke-opacity=".55" stroke-width="1.2" stroke-dasharray="2 3"/>'
                  f'{mark(w/2, 24, 28, "#FFFFFF")}'
                  f'<text x="{w/2}" y="{h-26}" text-anchor="middle" font-family="Nunito Sans" font-weight="600" font-size="15" letter-spacing="3" fill="#5B4426" opacity=".75">BAMBEE.STORE</text>')
    right_extra = (f'<g opacity=".6">{lockup(d/2, h/2 + 6, 40, "#4A3720")}</g>')
    body = (floor_shadow(o, w, d)
            + box(o, w, d, h, '#D8B98C', '#C7A472', '#B08A58', top_extra, left_extra, right_extra, texture=True)
            + caption('Standard Packaging', 'Recyclable kraft mailer, sealed with the Bambee stag'))
    page(body, 'standard')


# ── 2. Premium: rigid green gift box, gold ribbon ─────────────────
def bow(cx, cy):
    return f'''
<g transform="translate({cx:.1f},{cy:.1f})">
  <ellipse cx="0" cy="10" rx="70" ry="22" fill="#000" opacity=".22" filter="url(#blur10)"/>
  <path d="M0 0 C-40 -60 -105 -40 -82 -6 C-70 12 -30 10 0 0Z" fill="url(#gold)"/>
  <path d="M0 0 C-30 -40 -78 -32 -66 -10 C-58 2 -28 4 0 0Z" fill="#B88A2E" opacity=".45"/>
  <path d="M0 0 C40 -60 105 -40 82 -6 C70 12 30 10 0 0Z" fill="url(#gold)"/>
  <path d="M0 0 C30 -40 78 -32 66 -10 C58 2 28 4 0 0Z" fill="#B88A2E" opacity=".45"/>
  <path d="M-4 4 C-20 30 -40 50 -58 66 L-40 72 C-26 54 -12 34 4 6Z" fill="url(#goldV)"/>
  <path d="M4 4 C22 28 46 44 66 54 L54 68 C32 54 14 36 -2 8Z" fill="url(#goldV)"/>
  <ellipse cx="0" cy="0" rx="17" ry="13" fill="url(#gold)"/>
  <ellipse cx="-4" cy="-4" rx="7" ry="4" fill="#fff" opacity=".45"/>
</g>'''


def premium():
    w, d, h = 320, 320, 150
    o = (600, 300)
    rb = 30            # ribbon width
    ru, rv = w * .78, d * .78
    lid = 40           # lid depth on the front faces
    top_extra = (f'<rect x="{ru - rb/2}" y="0" width="{rb}" height="{d}" fill="url(#gold)"/>'
                 f'<rect x="0" y="{rv - rb/2}" width="{w}" height="{rb}" fill="url(#gold)"/>'
                 f'<g>{lockup(w*.40, d*.38, 58, "url(#gold)")}</g>')
    left_extra = (f'<rect y="{lid}" width="{w}" height="10" fill="url(#lipShadow)"/>'
                  f'<rect y="{lid-1}" width="{w}" height="1.5" fill="#000" opacity=".25"/>'
                  f'<rect x="{ru - rb/2}" width="{rb}" height="{h}" fill="url(#goldV)"/>'
                  f'<rect x="{ru - rb/2}" y="{lid}" width="{rb}" height="8" fill="#000" opacity=".18"/>'
                  f'<text x="{ru/2 - 4}" y="{lid/2 + 6}" text-anchor="middle" font-family="Nunito Sans" font-weight="600" font-size="13" letter-spacing="5" fill="#E9C978">BAMBEE</text>')
    right_extra = (f'<rect y="{lid}" width="{d}" height="10" fill="url(#lipShadow)"/>'
                   f'<rect y="{lid-1}" width="{d}" height="1.5" fill="#000" opacity=".25"/>'
                   f'<rect x="{d - rv - rb/2}" width="{rb}" height="{h}" fill="url(#goldV)"/>'
                   f'<rect x="{d - rv - rb/2}" y="{lid}" width="{rb}" height="8" fill="#000" opacity=".18"/>')
    bx, by = P(o, ru, rv, h)
    # A cream gift tag on a gold thread, hanging off the bow.
    tag = f'''
<g transform="translate({bx + 40:.1f},{by + 70:.1f}) rotate(-14)">
  <path d="M-30 -8 Q-50 -40 -42 -66" stroke="#C99A3C" stroke-width="2" fill="none"/>
  <rect x="-12" y="-12" width="96" height="54" rx="6" fill="#000" opacity=".18" filter="url(#blur4)" transform="translate(4,6)"/>
  <path d="M-12 6 L2 -10 H84 V40 H2 L-12 24Z" fill="#FBF7EE"/>
  <circle cx="2" cy="15" r="4" fill="#E6DFD0"/>
  <text x="44" y="22" text-anchor="middle" font-family="Rubik" font-size="17" fill="#1A8338">For you</text>
</g>'''
    body = (floor_shadow(o, w, d)
            + box(o, w, d, h, '#1E8E3E', '#16742F', '#0F5B24', top_extra, left_extra, right_extra)
            + bow(bx, by) + tag
            + caption('Premium Gift Packaging', 'Rigid green gift box, gold-foil stag and satin ribbon'))
    page(body, 'premium')


# ── 3. Premium, one box: open box with every item nested inside ──
def bottle(x, base, w, hgt, fill, cap_h, cap_w, cap_fill, kind='pump'):
    """A round bottle standing with its base at screen y = base."""
    top = base - hgt
    s = (f'<ellipse cx="{x}" cy="{base}" rx="{w/2}" ry="{w*.18}" fill="#000" opacity=".25" filter="url(#blur4)"/>'
         f'<rect x="{x - w/2}" y="{top}" width="{w}" height="{hgt}" rx="{w*.22}" fill="url(#{fill})"/>'
         f'<rect x="{x - w*.32}" y="{top + hgt*.3}" width="{w*.64}" height="{hgt*.38}" rx="3" fill="#fff" opacity=".55"/>'
         f'{mark(x, top + hgt*.49, hgt*.2, "#1A8338")}')
    if kind == 'pump':
        s += (f'<rect x="{x - cap_w/2}" y="{top - cap_h}" width="{cap_w}" height="{cap_h}" rx="3" fill="url(#goldCyl)"/>'
              f'<rect x="{x - 4}" y="{top - cap_h - 16}" width="8" height="16" fill="url(#goldCyl)"/>'
              f'<rect x="{x - 4}" y="{top - cap_h - 20}" width="30" height="7" rx="3" fill="url(#goldCyl)"/>')
    else:   # dropper
        s += (f'<rect x="{x - cap_w/2}" y="{top - cap_h}" width="{cap_w}" height="{cap_h}" rx="3" fill="url(#goldCyl)"/>'
              f'<rect x="{x - cap_w*.32}" y="{top - cap_h - 30}" width="{cap_w*.64}" height="32" rx="{cap_w*.3}" fill="url(#white)"/>')
    return s


def jar(x, base, w, hgt):
    top = base - hgt
    return (f'<ellipse cx="{x}" cy="{base}" rx="{w/2+4}" ry="{w*.2}" fill="#000" opacity=".25" filter="url(#blur4)"/>'
            f'<rect x="{x - w/2}" y="{top}" width="{w}" height="{hgt}" rx="8" fill="url(#white)"/>'
            f'<rect x="{x - w/2 - 3}" y="{top - 26}" width="{w + 6}" height="30" rx="6" fill="url(#goldCyl)"/>'
            f'<ellipse cx="{x}" cy="{top - 26}" rx="{w/2 + 3}" ry="{w*.16}" fill="#E9C978"/>'
            f'{mark(x, top - 26, 18, "#8E6519")}')


def lipstick(x, base, angle=0):
    return (f'<g transform="translate({x},{base}) rotate({angle})">'
            f'<rect x="-13" y="-120" width="26" height="70" rx="3" fill="url(#goldCyl)"/>'
            f'<rect x="-13" y="-52" width="26" height="54" rx="3" fill="url(#blush)"/>'
            f'<path d="M-9 -120 V-150 L9 -162 V-120Z" fill="#C2525E"/>'
            f'<path d="M-9 -150 L9 -162 V-155 L-9 -144Z" fill="#fff" opacity=".25"/></g>')


def tube(x, base, angle=0):
    return (f'<g transform="translate({x},{base}) rotate({angle})">'
            f'<path d="M-30 -170 H30 L22 -24 H-22Z" fill="url(#mint)"/>'
            f'<rect x="-32" y="-178" width="64" height="12" rx="2" fill="#B9D8C1"/>'
            f'<rect x="-14" y="-26" width="28" height="26" rx="4" fill="url(#white)"/>'
            f'{mark(0, -110, 34, "#1A8338")}</g>')


def tissue(o, w, d, h):
    """Cream tissue paper filling the box, folded over just below the rim."""
    z = h - 4
    a, b, c_, e = P(o, 8, 8, z), P(o, w - 8, 8, z), P(o, w - 8, d - 8, z), P(o, 8, d - 8, z)
    mid = P(o, w * .55, d * .45, z + 6)
    folds = ''.join(
        f'<path d="M{x1:.1f},{y1:.1f} Q{mid[0]:.1f},{mid[1]:.1f} {x2:.1f},{y2:.1f}" stroke="#E2D6C0" stroke-width="2" fill="none" opacity=".5"/>'
        for (x1, y1), (x2, y2) in [(a, c_), (b, e), (P(o, w*.5, 8, z), P(o, w*.4, d-8, z))])
    return (f'<polygon points="{pts(a, b, c_, e)}" fill="#F6F0E4"/>'
            f'<polygon points="{pts(a, b, P(o, w-8, 40, z), P(o, 8, 40, z))}" fill="#E9DFCC" opacity=".7"/>'
            + folds)


def premium_one_box():
    w, d, h = 380, 280, 130
    o = (660, 400)
    t = 10   # wall thickness, shown as the rim
    # Lid lying behind to the upper right, drawn first so the box sits in front of it.
    lo = P(o, -330, -150, 0)
    lw, ld, lh = w + 8, d + 8, 36
    lid_top = lockup(lw * .3, ld * .42, 62, 'url(#gold)')
    lid = floor_shadow(lo, lw, ld, 30, .22) + box(lo, lw, ld, lh, '#1E8E3E', '#16742F', '#0F5B24', lid_top)
    # Inside: the two far walls' inner faces.
    inner = (f'<polygon points="{pts(P(o,t,t,h), P(o,w-t,t,h), P(o,w-t,t,8), P(o,t,t,8))}" fill="#0E5A23"/>'
             f'<polygon points="{pts(P(o,t,t,h), P(o,t,d-t,h), P(o,t,d-t,8), P(o,t,t,8))}" fill="#126B2B"/>')
    # Items standing in the box; base y values sit inside the footprint.
    fx, fy = P(o, w * .5, d * .5, 0)
    items = (tissue(o, w, d, h)
             + bottle(fx - 120, fy - 18, 62, 150, 'white', 26, 30, 'goldCyl', 'pump')
             + tube(fx - 30, fy - 30, -8)
             + bottle(fx + 60, fy - 40, 54, 110, 'mint', 22, 30, 'goldCyl', 'dropper')
             + jar(fx + 150, fy - 6, 96, 52)
             + lipstick(fx + 95, fy - 10, 0))
    # Front walls drawn over the items' lower halves, then the rim.
    front = box(o, w, d, h, 'none', '#16742F', '#0F5B24',
                left_extra=(f'<g>{lockup(w/2, h/2 + 4, 60, "url(#gold)")}</g>'),
                right_extra='')
    rim = (f'<polygon points="{pts(P(o,0,0,h), P(o,w,0,h), P(o,w,d,h), P(o,0,d,h))}" fill="none" stroke="#2BA14F" stroke-width="{t*1.1}" stroke-linejoin="miter"/>'
           f'<polygon points="{pts(P(o,0,0,h), P(o,w,0,h), P(o,w,d,h), P(o,0,d,h))}" fill="none" stroke="#fff" stroke-opacity=".25" stroke-width="1"/>')
    # Gold seal sticker on the tissue at the front.
    sx, sy = P(o, w * .78, d * .8, h - 4)
    seal = (f'<g transform="translate({sx:.1f},{sy:.1f})"><ellipse rx="30" ry="22" fill="url(#gold)"/>'
            f'<ellipse rx="24" ry="17" fill="none" stroke="#8E6519" stroke-opacity=".5" stroke-dasharray="2 3"/>'
            f'{mark(0, 0, 24, "#0F5B24")}</g>')
    body = (lid + floor_shadow(o, w, d) + inner + items + front.replace('<rect width="380" height="280" fill="none"/>', '') + rim + seal
            + caption('Premium Packaging, One Box', 'Every item nested together in tissue, in one gift box'))
    page(body, 'premium-one-box')


standard()
premium()
premium_one_box()
print('written')
