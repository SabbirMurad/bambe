/* ═══════════════════════════════════════════════════════════════
   Bambee Web — utils/blurhash.js

   Decodes the blur hash every image document carries (`blur_hash`,
   written by src/utils/blurhash.rs at upload) into a tiny blurred
   picture, so a photo's own colours can stand in for it while the
   real file downloads.

   The algorithm is the reference one (github.com/woltapp/blurhash):
   a hash is a handful of DCT components, base83-encoded. Decoding
   sums those cosines over a small grid of pixels. The grid only has
   to be large enough to look smooth once the browser stretches it —
   32×32 is plenty — which keeps the work to a few thousand
   multiplications per image.
═══════════════════════════════════════════════════════════════ */

const BASE83 = '0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz#$%*+,-.:;=?@[]^_{|}~';

function decode83(str) {
  let value = 0;
  for (let i = 0; i < str.length; i++) {
    const digit = BASE83.indexOf(str[i]);
    if (digit < 0) throw new Error('blurhash: invalid character');
    value = value * 83 + digit;
  }
  return value;
}

function sRGBToLinear(value) {
  const v = value / 255;
  return v <= 0.04045 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4);
}

function linearToSRGB(value) {
  const v = Math.max(0, Math.min(1, value));
  return v <= 0.0031308
    ? Math.trunc(v * 12.92 * 255 + 0.5)
    : Math.trunc((1.055 * Math.pow(v, 1 / 2.4) - 0.055) * 255 + 0.5);
}

function signPow(value, exp) {
  return Math.sign(value) * Math.pow(Math.abs(value), exp);
}

/** True when `hash` has a well-formed header and the length it promises. */
export function isValid(hash) {
  if (typeof hash !== 'string' || hash.length < 6) return false;
  try {
    const sizeFlag = decode83(hash[0]);
    const numX = (sizeFlag % 9) + 1;
    const numY = Math.floor(sizeFlag / 9) + 1;
    return hash.length === 4 + 2 * numX * numY;
  } catch (_) {
    return false;
  }
}

/**
 * Decodes `hash` into `width`×`height` RGBA pixels. `punch` scales the
 * contrast of the non-average components; 1 is the encoder's own.
 * Throws on a malformed hash.
 */
export function decode(hash, width, height, punch = 1) {
  if (!isValid(hash)) throw new Error('blurhash: malformed hash');

  const sizeFlag = decode83(hash[0]);
  const numX = (sizeFlag % 9) + 1;
  const numY = Math.floor(sizeFlag / 9) + 1;
  const maxValue = (decode83(hash[1]) + 1) / 166;

  const colors = new Array(numX * numY);
  for (let i = 0; i < colors.length; i++) {
    if (i === 0) {
      // The first component is the average colour, stored as plain sRGB.
      const value = decode83(hash.substring(2, 6));
      colors[i] = [
        sRGBToLinear(value >> 16),
        sRGBToLinear((value >> 8) & 255),
        sRGBToLinear(value & 255),
      ];
    } else {
      const value = decode83(hash.substring(4 + i * 2, 6 + i * 2));
      const scale = maxValue * punch;
      colors[i] = [
        signPow((Math.floor(value / (19 * 19)) - 9) / 9, 2) * scale,
        signPow(((Math.floor(value / 19) % 19) - 9) / 9, 2) * scale,
        signPow(((value % 19) - 9) / 9, 2) * scale,
      ];
    }
  }

  const pixels = new Uint8ClampedArray(width * height * 4);
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      let r = 0, g = 0, b = 0;
      for (let j = 0; j < numY; j++) {
        const basisY = Math.cos((Math.PI * y * j) / height);
        for (let i = 0; i < numX; i++) {
          const basis = Math.cos((Math.PI * x * i) / width) * basisY;
          const color = colors[i + j * numX];
          r += color[0] * basis;
          g += color[1] * basis;
          b += color[2] * basis;
        }
      }
      const p = 4 * (x + y * width);
      pixels[p] = linearToSRGB(r);
      pixels[p + 1] = linearToSRGB(g);
      pixels[p + 2] = linearToSRGB(b);
      pixels[p + 3] = 255;
    }
  }
  return pixels;
}

/*
  The same hash turns up many times on one page — a product in the
  grid and again in its gallery — so each is decoded once.
*/
const cache = new Map();

/**
 * `hash` as a PNG data URL, ready for a CSS background, or null when
 * the hash is missing or malformed (the caller then just waits for
 * the photo, as it did before).
 */
export function toDataURL(hash, width = 32, height = 32) {
  if (!hash) return null;
  const key = hash + '@' + width + 'x' + height;
  if (cache.has(key)) return cache.get(key);

  let url = null;
  try {
    const canvas = document.createElement('canvas');
    canvas.width = width;
    canvas.height = height;
    const ctx = canvas.getContext('2d');
    const image = ctx.createImageData(width, height);
    image.data.set(decode(hash, width, height));
    ctx.putImageData(image, 0, 0);
    url = canvas.toDataURL();
  } catch (_) {
    url = null;
  }

  cache.set(key, url);
  return url;
}
