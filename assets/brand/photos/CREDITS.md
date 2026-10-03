# Home page photography

All four are Unsplash Licence (free, commercial use permitted, attribution
appreciated but not required). None is Unsplash+ — that one is paid, and
several of the better-styled beauty shots on Unsplash are Unsplash+, so
check the source host before swapping any of these: a photo served from
`plus.unsplash.com` is licensed, `images.unsplash.com` is free.

| File | Unsplash ID | Used for | Edits |
|---|---|---|---|
| `hero-wide.jpg` | `TQSPgNqeCo8` | Hero, full-bleed | Backdrop extended rightward from 3:2 to 2.2:1 (see note below) |
| `portrait.jpg` | `n-A9Mc7F7Fg` | Collage feature panel | Cropped 2:3 → 3:4, trimmed from the top |
| `clip.jpg` | `ZOPrzxoS1X4` | Collage clip tile | Cropped 3:2 → 16:10 |
| `still.jpg` | `tlOxSSBgwq8` | Collage small still | Cropped 3:2 → 4:3, trimmed off the right |

## Why these, and what to watch for when replacing them

The shop sells seven lines, not just hair, so the imagery has to read as
beauty in general. The hero was picked because one frame carries four of
the categories at once — cream on the cheek (skincare), a manicure (nail
care), bare shoulder and a spa setting (body care, wellness) — and the
model's hair is pulled back, so nothing says "hair shop".

**The constraint that rules most beauty stock out: visible competitor
branding.** Most well-styled cosmetics photography on Unsplash shows real
products — Dior, Guerlain, Bioderma, Chanel, The Ordinary, Anne Semonin,
Kjaer Weis were all in the first two pages of results. Putting those on
Bambe's own home page implies Bambe stocks them and reproduces someone
else's trade dress. Everything above is either unbranded, a blank-label
render, or cropped so no maker's name is legible.

## The hero's extended backdrop

The layout wants roughly 2.2:1 full-bleed with the subject on the left and
clear space on the right for the headline. The source is 3:2, so the plain
backdrop was continued to the right: each row is filled with the mean of
that row's last 100 columns, smoothed vertically so sensor noise does not
become horizontal streaks, then the photograph is cross-faded over it
across 900px so there is no join to see. The script that did it is not
checked in; if the hero is ever recropped, redo it the same way rather
than pasting a hard edge.

Placeholder colour `#E6E1E0` in `.hero-photo` is this file's average, so a
slow load is not a flash of the wrong colour. Change it if the hero changes.

## The hero flat-lay

`brush.jpg` is the file as supplied: a cosmetics flat-lay with the
products ringed around a blush-pink ground. The pink fought the green
and gold palette. `brush-custom.jpg` is Sabbir's recolour of it onto a
warm off-white, and is the one the site uses.

What is actually served is `brush-custom.webp` — 102KB against the
JPEG's 2.3MB, at PSNR 45.4dB, which is indistinguishable at any size
the hero is drawn:

    ffmpeg -y -i brush-custom.jpg       -c:v libwebp -quality 86 -compression_level 6 -preset picture       -update 1 -frames:v 1 brush-custom.webp

Quality 86 rather than lower: most of this picture is a smooth gradient,
which is exactly where WebP bands. Measured along a line through the
open middle, 86 holds the largest step between neighbouring pixels to 3
— no worse than the JPEG it came from. Below 80 the ground starts
flattening into visible steps.

`brush-custom-og.jpg` exists only for `og:image`: a 1200px JPEG at 29KB,
derived from the WebP. Social scrapers are still uneven on WebP, so the
share image stays a JPEG.

    ffmpeg -y -i brush-custom.webp -vf "scale=1200:-2"       -update 1 -frames:v 1 -q:v 4 brush-custom-og.jpg

The full-size `brush-custom.jpg` this was all cut from is not in the
repo — it was a 2.3MB working file. Everything the site needs is
derived from `brush-custom.webp`, which is.

An earlier attempt recoloured the pink automatically, with
`colorkey=0xEED8DF:0.06:0.10` compositing over the cream token. It is
recorded here only as a warning: a global colour grade cannot do this
job, because the products *are* pink and peach — the blush, the pearls,
the gloss — so `colorbalance` and `selectivecolor` both turned the
foundation bottle yellow-green before the ground was anywhere near
neutral. Keying worked, but left the drop shadows pink. The hand
recolour is better.
