"""Builds the Bambee Facebook asset set: one HTML per asset in src/, then a PNG of each via headless Chrome.

Run from anywhere: python assets/banners/facebook/build.py [asset-name ...]
With names, only those assets are re-exported. Needs Chrome, and a network
connection for the Google Fonts.
"""
import re, subprocess, pathlib, sys
ROOT = pathlib.Path(__file__).resolve().parents[3]
OUT = ROOT / 'assets/banners/facebook'
SRC = OUT / 'src'
SRC.mkdir(parents=True, exist_ok=True)
CHROME = r'C:/Program Files/Google/Chrome/Application/chrome.exe'

svg = (ROOT / 'assets/icons/bambe_logo.svg').read_text(encoding='utf-8')
svg = re.sub(r'<\?xml[^>]*\?>', '', svg)
svg = re.sub(r'<defs>.*?</defs>', '', svg, flags=re.S)
svg = svg.replace('class="cls-1"', 'fill="currentColor"').replace('id="Layer_1" ', '')
STAG = svg.replace('<svg ', '<svg aria-hidden="true" ', 1)

GREEN, GREEN_DK, MINT, PALE = '#1A8338', '#146B2D', '#CFE8D6', '#E8F3EB'
CTA = 'Shop at bambee.store'


def wordmark(h, color):
    return f'<span class="wm" style="--h:{h}px;color:{color}">{STAG}<span class="wm-t">ambee</span></span>'


# ── Abstract motif ────────────────────────────────────────────
# Every asset draws from the same few pieces: solid circles in the
# greens, one thin ring, and a small dot grid. Positions are given in
# canvas pixels per asset, so each layout places them around its copy.
def circle(cx, cy, r, fill, opacity=1):
    return f'<circle cx="{cx}" cy="{cy}" r="{r}" fill="{fill}" opacity="{opacity}"/>'


def ring(cx, cy, r, stroke, width=3, opacity=.35):
    return f'<circle cx="{cx}" cy="{cy}" r="{r}" fill="none" stroke="{stroke}" stroke-width="{width}" opacity="{opacity}"/>'


def dots(x, y, cols, rows, gap, r, fill, opacity=1):
    return ''.join(f'<circle cx="{x + c * gap}" cy="{y + rw * gap}" r="{r}" fill="{fill}" opacity="{opacity}"/>'
                   for rw in range(rows) for c in range(cols))


def motif(w, h, *parts):
    return f'<svg class="motif" viewBox="0 0 {w} {h}" width="{w}" height="{h}" aria-hidden="true">{"".join(parts)}</svg>'


BASE_CSS = """
:root{--green:#1A8338;--green-dk:#146B2D;--ink:#1A1A18;--mid:#6A665F;--ground:#FCFBF9}
*{box-sizing:border-box;margin:0;padding:0}
html,body{width:var(--w);height:var(--h);overflow:hidden;background:var(--ground)}
body{font-family:'Nunito Sans',sans-serif;color:var(--ink);-webkit-font-smoothing:antialiased}
.canvas{position:relative;width:var(--w);height:var(--h);overflow:hidden}
.motif{position:absolute;inset:0;display:block}
h1{font-family:'Rubik',sans-serif}
.wm{display:inline-flex;align-items:flex-end;line-height:1}
.wm svg{height:var(--h);width:auto;display:block}
.wm-t{font-family:'Rubik',sans-serif;font-weight:400;font-size:calc(var(--h) * 0.53);line-height:1;margin-bottom:-0.153em}
.eyebrow{font-family:'Rubik',sans-serif;font-weight:600;text-transform:uppercase;color:var(--green)}
.pill{display:inline-flex;align-items:center;font-family:'Rubik',sans-serif;font-weight:600;background:var(--green);color:#fff;border-radius:999px}
"""


def page(name, w, h, css, body):
    html = f"""<!doctype html><html><head><meta charset="utf-8">
<link href="https://fonts.googleapis.com/css2?family=Nunito+Sans:wght@400;600;700&family=Rubik:wght@400;500;600;700&display=block" rel="stylesheet">
<style>{BASE_CSS}:root{{--w:{w}px;--h:{h}px}}{css}</style></head>
<body><div class="canvas">{body}</div></body></html>"""
    (SRC / f'{name}.html').write_text(html, encoding='utf-8')
    return name, w, h


assets = []

# 1. Profile picture: the stag alone; Facebook prints the page name beside it.
for variant, bg, fg in (('green', GREEN, '#FFFFFF'), ('white', '#FFFFFF', GREEN)):
    assets.append(page(f'profile-{variant}-720x720', 720, 720, f"""
.canvas{{background:{bg};display:grid;place-items:center}}
.canvas svg{{height:430px;width:auto;color:{fg};transform:translate(6px,-4px)}}
""", STAG))

# 2. Cover photo: 2x Facebook's 820x312 desktop crop. Copy on the left,
#    starting at 300px: the mobile app trims roughly the outer 16% (~262px)
#    of each side, so anything further left is cut off on phones.
assets.append(page('cover-1640x624', 1640, 624, """
.copy{position:absolute;left:300px;top:50%;transform:translateY(-50%);width:700px}
.eyebrow{font-size:19px;letter-spacing:4px}
h1{font-size:96px;line-height:.98;letter-spacing:-2.5px;font-weight:700;margin-top:22px}
p{font-size:24px;line-height:1.45;color:var(--mid);margin-top:24px;max-width:520px}
.pill{font-size:22px;padding:15px 28px;margin-top:30px}
""", motif(1640, 624,
           ring(1300, 330, 250, GREEN),
           circle(1450, 560, 330, GREEN),
           circle(1180, 120, 170, MINT),
           circle(1565, 80, 92, PALE),
           circle(1118, 430, 26, GREEN_DK),
           dots(990, 470, 6, 4, 22, 4, GREEN, .55))
    + f"""<div class="copy"><div class="eyebrow">Skincare · Makeup · Fragrance · Grooming</div>
<h1>Beauty,<br>boldly yours.</h1>
<p>Hand-picked from brands you trust and delivered to your door.</p>
<span class="pill">{CTA}</span></div>"""))

# 3. Launch post: 4:5, the tallest feed shape Facebook shows uncropped.
assets.append(page('post-launch-1080x1350', 1080, 1350, """
.top{position:absolute;left:80px;top:80px}
.copy{position:absolute;left:80px;right:80px;bottom:96px}
.eyebrow{font-size:24px;letter-spacing:5px}
h1{font-size:108px;line-height:.96;letter-spacing:-3px;font-weight:700;margin-top:20px}
p{font-size:30px;line-height:1.45;color:var(--mid);margin-top:24px;max-width:920px}
.pill{font-size:30px;padding:22px 40px;margin-top:36px}
""", motif(1080, 1350,
           ring(700, 420, 360, GREEN),
           circle(820, 430, 300, GREEN),
           circle(380, 300, 150, MINT),
           circle(1010, 110, 110, PALE),
           circle(520, 640, 34, GREEN_DK),
           dots(130, 500, 5, 5, 26, 4.5, GREEN, .55))
    + f"""<div class="top">{wordmark(64, 'var(--ink)')}</div>
<div class="copy"><div class="eyebrow">Now open</div>
<h1>Beauty,<br>boldly yours.</h1>
<p>Skincare, makeup, fragrance and grooming, delivered to your door.</p>
<span class="pill">{CTA}</span></div>"""))

# 4. Category post: what the shop carries, set as type.
cats = ['Cosmetics &amp; Accessories', 'Skincare', 'Body Care &amp; Sprays', 'Wellness &amp; Self-Care',
        'Hair Care &amp; Extensions', 'Nail Care', 'Men&rsquo;s Grooming']
rows = ''.join(f'<li><span class="n">0{i + 1}</span><span>{c}</span></li>' for i, c in enumerate(cats))
assets.append(page('post-categories-1080x1350', 1080, 1350, """
.top{position:absolute;left:80px;top:80px}
h1{position:absolute;left:80px;top:210px;font-size:104px;line-height:.98;letter-spacing:-2.5px;font-weight:700}
h1 em{font-style:normal;color:var(--green)}
ul{position:absolute;left:80px;right:80px;top:470px;list-style:none;border-top:2px solid #E8E5E0}
li{display:flex;align-items:baseline;gap:34px;padding:25px 0;border-bottom:2px solid #E8E5E0;font-family:'Rubik',sans-serif;font-size:42px;font-weight:500}
.n{font-size:24px;font-weight:600;color:var(--green);letter-spacing:2px;width:40px}
.foot{position:absolute;left:80px;right:80px;bottom:72px;display:flex;justify-content:space-between;align-items:center;font-family:'Rubik',sans-serif;font-size:28px;font-weight:600;color:var(--green)}
""", motif(1080, 1350,
           circle(1010, 110, 170, PALE),
           ring(950, 190, 130, GREEN),
           circle(880, 330, 22, GREEN))
    + f"""<div class="top">{wordmark(64, 'var(--ink)')}</div>
<h1>Seven lines.<br><em>One store.</em></h1><ul>{rows}</ul>
<div class="foot"><span>Shop every line</span><span>bambee.store &rarr;</span></div>"""))

# 5. Offer post: a template. Set the number and code to a real coupon first.
assets.append(page('post-offer-1080x1350', 1080, 1350, """
.canvas{background:var(--green)}
.top{position:absolute;left:80px;top:80px}
.copy{position:absolute;left:80px;right:80px;top:200px;color:#fff}
.eyebrow{color:#CFE8D6;font-size:26px;letter-spacing:5px}
.big{font-family:'Rubik',sans-serif;font-weight:700;font-size:300px;line-height:.85;letter-spacing:-10px;margin-top:26px}
.big small{font-size:120px;letter-spacing:-3px;margin-left:12px}
.sub{font-family:'Rubik',sans-serif;font-size:50px;font-weight:500;margin-top:18px}
.code{display:inline-block;margin-top:36px;border:3px dashed rgba(255,255,255,.7);border-radius:18px;padding:16px 30px;font-size:30px}
.code b{font-family:'Rubik',sans-serif;letter-spacing:3px;margin-left:8px}
.pill{position:absolute;left:80px;bottom:96px;font-size:30px;padding:22px 40px;background:#fff;color:var(--green)}
""", motif(1080, 1350,
           ring(880, 1110, 420, '#FFFFFF', 3, .28),
           circle(900, 1190, 330, GREEN_DK),
           circle(740, 1300, 130, MINT, .9),
           circle(1020, 760, 60, '#FFFFFF', .14),
           circle(470, 1010, 22, '#FFFFFF', .9),
           dots(110, 1000, 6, 4, 26, 4.5, '#FFFFFF', .45))
    + f"""<div class="top">{wordmark(64, '#fff')}</div>
<div class="copy"><div class="eyebrow">Welcome offer</div>
<div class="big">20%<small>off</small></div>
<div class="sub">your first order</div>
<div class="code">Use code <b>BAMBEE20</b> at checkout</div></div>
<span class="pill">{CTA}</span>"""))

# 6. Story: 9:16. Facebook lays its own UI over the top and bottom ~250px,
#    so nothing that matters sits there.
assets.append(page('story-launch-1080x1920', 1080, 1920, """
.top{position:absolute;left:0;right:0;top:270px;text-align:center}
.copy{position:absolute;left:90px;right:90px;top:1230px;text-align:center}
.eyebrow{font-size:28px;letter-spacing:6px}
h1{font-size:120px;line-height:.96;letter-spacing:-3px;font-weight:700;margin-top:24px}
p{font-size:34px;line-height:1.45;color:var(--mid);margin-top:28px}
.pill{font-size:34px;padding:26px 46px;margin-top:44px}
""", motif(1080, 1920,
           ring(560, 790, 390, GREEN),
           circle(700, 780, 310, GREEN),
           circle(330, 580, 170, MINT),
           circle(960, 470, 100, PALE),
           circle(360, 1060, 40, GREEN_DK),
           dots(140, 880, 5, 5, 28, 5, GREEN, .55))
    + f"""<div class="top">{wordmark(76, 'var(--ink)')}</div>
<div class="copy"><div class="eyebrow">Now open</div>
<h1>Beauty,<br>boldly yours.</h1>
<p>Skincare, makeup, fragrance and grooming.</p>
<span class="pill">{CTA}</span></div>"""))

# 7. Link preview: what Facebook shows when bambee.store is shared (og:image).
assets.append(page('link-share-1200x630', 1200, 630, """
.top{position:absolute;left:72px;top:64px}
.copy{position:absolute;left:72px;bottom:78px;width:560px}
h1{font-size:78px;line-height:.98;letter-spacing:-2px;font-weight:700}
p{font-size:24px;line-height:1.45;color:var(--mid);margin-top:20px;max-width:520px}
""", motif(1200, 630,
           ring(940, 340, 220, GREEN),
           circle(1070, 560, 280, GREEN),
           circle(840, 130, 130, MINT),
           circle(1150, 70, 72, PALE),
           circle(790, 420, 22, GREEN_DK),
           dots(700, 470, 5, 4, 22, 4, GREEN, .55))
    + f"""<div class="top">{wordmark(52, 'var(--ink)')}</div>
<div class="copy"><h1>Beauty,<br>boldly yours.</h1>
<p>Skincare, makeup, fragrance and grooming, delivered to your door.</p></div>"""))

only = set(sys.argv[1:])
for name, w, h in assets:
    if only and name not in only:
        continue
    out = OUT / f'{name}.png'
    subprocess.run([CHROME, '--headless=new', '--disable-gpu', '--hide-scrollbars', '--force-device-scale-factor=1',
                    f'--window-size={w},{h}', '--virtual-time-budget=8000',
                    f'--screenshot={out}', (SRC / f'{name}.html').as_uri()], check=True, capture_output=True)
    print('wrote', out.name)
