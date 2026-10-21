/*
  Blurhash encoder — https://blurha.sh

  A blurhash is a ~30 character string holding the first few cosine terms of
  an image, enough to draw the blurred placeholder that sits in a card while
  the real photo loads. `ImageStruct.blur_hash` has always been part of the
  image metadata, but the upload endpoint made the *client* produce it, which
  is backwards: the server already decodes every upload to convert it to
  webp, and a client that gets the string wrong (or omits it) poisons the
  field for every reader.

  Written out rather than pulled in as a dependency — it is one DCT and a
  base83 alphabet, and the algorithm is frozen by the format.
*/

use std::f32::consts::PI;

const BASE83: &[u8] =
    b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz#$%*+,-.:;=?@[]^_{|}~";

/*
  Encodes `rgb` (3 bytes per pixel, row major) as a blurhash with
  `components_x` x `components_y` terms. 4x3 is the usual choice for a photo.

  Returns None if the buffer is too short for the dimensions given, rather
  than reading past it.
*/
pub fn encode(
    components_x: usize,
    components_y: usize,
    width: usize,
    height: usize,
    rgb: &[u8],
) -> Option<String> {
    if width == 0 || height == 0 || rgb.len() < width * height * 3 {
        return None;
    }

    let components_x = components_x.clamp(1, 9);
    let components_y = components_y.clamp(1, 9);

    /*
      The linear conversion is the expensive part and depends only on the
      pixel, not on the component, so it is done once up front instead of
      once per term — 12 passes over the image otherwise.
    */
    let linear: Vec<[f32; 3]> = (0..width * height)
        .map(|i| {
            [
                srgb_to_linear(rgb[i * 3]),
                srgb_to_linear(rgb[i * 3 + 1]),
                srgb_to_linear(rgb[i * 3 + 2]),
            ]
        })
        .collect();

    let scale = 1.0 / (width * height) as f32;
    let mut factors: Vec<[f32; 3]> = Vec::with_capacity(components_x * components_y);

    for j in 0..components_y {
        for i in 0..components_x {
            let normalisation = if i == 0 && j == 0 { 1.0 } else { 2.0 };
            let mut factor = [0.0f32; 3];

            for y in 0..height {
                let basis_y = (PI * j as f32 * y as f32 / height as f32).cos();

                for x in 0..width {
                    let basis = normalisation
                        * (PI * i as f32 * x as f32 / width as f32).cos()
                        * basis_y;

                    let pixel = &linear[y * width + x];
                    factor[0] += basis * pixel[0];
                    factor[1] += basis * pixel[1];
                    factor[2] += basis * pixel[2];
                }
            }

            factors.push([factor[0] * scale, factor[1] * scale, factor[2] * scale]);
        }
    }

    let (dc, ac) = factors.split_first()?;

    let mut hash = String::new();
    hash.push_str(&encode83(((components_x - 1) + (components_y - 1) * 9) as u32, 1));

    // The AC terms are stored relative to the largest of them, so the one
    // scale factor is written first and every term quantised against it.
    let maximum_value = if ac.is_empty() {
        hash.push_str(&encode83(0, 1));
        1.0
    } else {
        let actual_max = ac
            .iter()
            .flat_map(|f| f.iter())
            .fold(0.0f32, |max, value| max.max(value.abs()));

        let quantised = (actual_max * 166.0 - 0.5).floor().clamp(0.0, 82.0) as u32;
        hash.push_str(&encode83(quantised, 1));
        (quantised + 1) as f32 / 166.0
    };

    hash.push_str(&encode83(encode_dc(dc), 4));
    for factor in ac {
        hash.push_str(&encode83(encode_ac(factor, maximum_value), 2));
    }

    Some(hash)
}

fn encode83(value: u32, length: usize) -> String {
    (0..length)
        .map(|i| {
            let divisor = 83u32.pow((length - i - 1) as u32);
            BASE83[((value / divisor) % 83) as usize] as char
        })
        .collect()
}

/* The average colour, stored as a plain 24-bit sRGB triple. */
fn encode_dc(value: &[f32; 3]) -> u32 {
    (linear_to_srgb(value[0]) << 16) + (linear_to_srgb(value[1]) << 8) + linear_to_srgb(value[2])
}

/*
  Each AC term is squashed to 19 levels per channel and packed base-19. The
  square root spreads resolution towards zero, where most terms sit.
*/
fn encode_ac(value: &[f32; 3], maximum_value: f32) -> u32 {
    let quantise = |v: f32| -> u32 {
        let normalised = v / maximum_value;
        let signed = normalised.signum() * normalised.abs().sqrt();
        (signed * 9.0 + 9.5).floor().clamp(0.0, 18.0) as u32
    };

    quantise(value[0]) * 19 * 19 + quantise(value[1]) * 19 + quantise(value[2])
}

fn srgb_to_linear(value: u8) -> f32 {
    let v = value as f32 / 255.0;

    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(value: f32) -> u32 {
    let v = value.clamp(0.0, 1.0);

    if v <= 0.0031308 {
        (v * 12.92 * 255.0 + 0.5) as u32
    } else {
        ((1.055 * v.powf(1.0 / 2.4) - 0.055) * 255.0 + 0.5) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_a_flat_image() {
        /*
          Solid white: `L` is the 4x3 size flag and `TSUA` the DC term, 255
          in all three channels.

          The AC terms are not zero, which looks wrong for an image with no
          detail. It isn't: the basis samples at `x / width` with no
          half-pixel offset, so it is not orthogonal over the grid and a flat
          field leaks into the first terms. The decoder sums the same basis,
          so the pair still reconstructs flat. Kept faithful to the reference
          encoder rather than corrected, because a hash that "improves" on it
          decodes wrong everywhere else.
        */
        let pixels = vec![255u8; 8 * 8 * 3];

        assert_eq!(
            encode(4, 3, 8, 8, &pixels).unwrap(),
            "LfTSUA~qfQ~q~qt7fQt7fQfQfQfQ"
        );
    }

    #[test]
    fn averages_towards_mid_grey() {
        // Black beside white, one component, so the hash is only the
        // average — and the average of the two in *linear* light is 188,
        // not the 128 you would get from averaging the sRGB bytes.
        let pixels = [0u8, 0, 0, 255, 255, 255];

        assert_eq!(encode(1, 1, 2, 1, &pixels).unwrap(), "00Lqe9");
    }

    #[test]
    fn rejects_a_short_buffer() {
        assert!(encode(4, 3, 8, 8, &[0u8; 10]).is_none());
    }

    #[test]
    fn length_follows_the_component_count() {
        // 1 size flag + 1 scale + 4 DC + 2 per AC term.
        let pixels = vec![128u8; 16 * 16 * 3];

        for (x, y) in [(1, 1), (4, 3), (9, 9)] {
            let hash = encode(x, y, 16, 16, &pixels).unwrap();
            assert_eq!(hash.len(), 6 + 2 * (x * y - 1));
        }
    }
}
