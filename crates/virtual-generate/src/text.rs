//! The Text pattern: a 16-segment display font drawn as glowing lines, so
//! text gets the same palette, trails, echoes and tracing as every pattern.

use crate::math::{Rng, Vec3};
use crate::settings::TEXT_MAX_CHARS;
use crate::turtle::Builder;

/// Segment endpoints in a 1 × 2 cell: corners, edge midpoints and centre.
const TL: (f32, f32) = (0.0, 2.0);
const TM: (f32, f32) = (0.5, 2.0);
const TR: (f32, f32) = (1.0, 2.0);
const ML: (f32, f32) = (0.0, 1.0);
const MM: (f32, f32) = (0.5, 1.0);
const MR: (f32, f32) = (1.0, 1.0);
const BL: (f32, f32) = (0.0, 0.0);
const BM: (f32, f32) = (0.5, 0.0);
const BR: (f32, f32) = (1.0, 0.0);

/// The sixteen segments, then a short dot at the baseline (bit 16) and one
/// at mid height (bit 17) for `.` `:` and `!`.
const SEGMENTS: [((f32, f32), (f32, f32)); 18] = [
    (TL, TM),
    (TM, TR),
    (TR, MR),
    (MR, BR),
    (BM, BR),
    (BL, BM),
    (ML, BL),
    (TL, ML),
    (ML, MM),
    (MM, MR),
    (TL, MM),
    (TM, MM),
    (TR, MM),
    (MM, BL),
    (MM, BM),
    (MM, BR),
    ((0.45, 0.0), (0.55, 0.0)),
    ((0.45, 1.3), (0.55, 1.3)),
];

const fn bits(segments: &[u8]) -> u32 {
    let mut out = 0;
    let mut index = 0;
    while index < segments.len() {
        out |= 1 << segments[index];
        index += 1;
    }
    out
}

/// Segments lit for each printable character (unknown ones draw nothing).
fn glyph(character: u8) -> u32 {
    match character.to_ascii_uppercase() {
        b'A' => bits(&[7, 6, 0, 1, 2, 3, 8, 9]),
        b'B' => bits(&[0, 1, 2, 3, 4, 5, 9, 11, 14]),
        b'C' => bits(&[0, 1, 7, 6, 5, 4]),
        b'D' => bits(&[0, 1, 2, 3, 4, 5, 11, 14]),
        b'E' => bits(&[0, 1, 7, 6, 5, 4, 8]),
        b'F' => bits(&[0, 1, 7, 6, 8]),
        b'G' => bits(&[0, 1, 7, 6, 5, 4, 3, 9]),
        b'H' => bits(&[7, 6, 2, 3, 8, 9]),
        b'I' => bits(&[0, 1, 4, 5, 11, 14]),
        b'J' => bits(&[2, 3, 4, 5, 6]),
        b'K' => bits(&[7, 6, 8, 12, 15]),
        b'L' => bits(&[7, 6, 5, 4]),
        b'M' => bits(&[7, 6, 2, 3, 10, 12]),
        b'N' => bits(&[7, 6, 2, 3, 10, 15]),
        b'O' | b'0' => bits(&[0, 1, 2, 3, 4, 5, 6, 7]),
        b'P' => bits(&[0, 1, 7, 6, 2, 8, 9]),
        b'Q' => bits(&[0, 1, 2, 3, 4, 5, 6, 7, 15]),
        b'R' => bits(&[0, 1, 7, 6, 2, 8, 9, 15]),
        b'S' | b'5' => bits(&[0, 1, 7, 8, 9, 3, 4, 5]),
        b'T' => bits(&[0, 1, 11, 14]),
        b'U' => bits(&[7, 6, 5, 4, 3, 2]),
        b'V' => bits(&[7, 6, 13, 12]),
        b'W' => bits(&[7, 6, 2, 3, 13, 15]),
        b'X' => bits(&[10, 12, 13, 15]),
        b'Y' => bits(&[10, 12, 14]),
        b'Z' => bits(&[0, 1, 12, 13, 5, 4]),
        b'1' => bits(&[12, 2, 3]),
        b'2' => bits(&[0, 1, 2, 8, 9, 6, 5, 4]),
        b'3' => bits(&[0, 1, 2, 3, 4, 5, 9]),
        b'4' => bits(&[7, 8, 9, 2, 3]),
        b'6' => bits(&[0, 1, 7, 6, 5, 4, 3, 8, 9]),
        b'7' => bits(&[0, 1, 2, 3]),
        b'8' => bits(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9]),
        b'9' => bits(&[0, 1, 2, 3, 4, 5, 7, 8, 9]),
        b'-' => bits(&[8, 9]),
        b'+' => bits(&[8, 9, 11, 14]),
        b'*' => bits(&[8, 9, 10, 11, 12, 13, 14, 15]),
        b'/' => bits(&[12, 13]),
        b'\\' => bits(&[10, 15]),
        b'=' => bits(&[8, 9, 4, 5]),
        b'_' => bits(&[4, 5]),
        b'(' | b'<' => bits(&[12, 15]),
        b')' | b'>' => bits(&[10, 13]),
        b'\'' => bits(&[11]),
        b'"' => bits(&[11, 2]),
        b'?' => bits(&[0, 1, 2, 9, 14]),
        b'!' => bits(&[11, 16]),
        b'.' | b',' => bits(&[16]),
        b':' => bits(&[16, 17]),
        b'#' => bits(&[2, 3, 8, 9, 11, 14, 4, 5]),
        b'%' => bits(&[7, 1, 12, 13, 3, 5, 8, 9]),
        b'$' => bits(&[0, 1, 7, 8, 9, 3, 4, 5, 11, 14]),
        b'&' => bits(&[0, 10, 11, 8, 6, 5, 4, 15]),
        b'@' => bits(&[0, 1, 2, 9, 11, 7, 6, 5, 4]),
        _ => 0,
    }
}

/// The text as zero-padded ASCII; other characters become `?`.
pub(crate) fn encode(text: &str) -> [u8; TEXT_MAX_CHARS] {
    let mut out = [0; TEXT_MAX_CHARS];
    for (slot, character) in out.iter_mut().zip(text.chars()) {
        *slot = if character.is_ascii() && character != '\0' {
            character as u8
        } else {
            b'?'
        };
    }
    out
}

/// Lays out the text centred on the origin, one line per `|` or newline.
/// `spread` sets letter spacing, `twist` the slant and `randomness` jitter.
pub(crate) fn build(
    text: &[u8; TEXT_MAX_CHARS],
    scale: f32,
    spread: f32,
    twist: f32,
    randomness: f32,
    seed: u64,
    out: &mut Builder,
) {
    let length = text.iter().position(|&c| c == 0).unwrap_or(TEXT_MAX_CHARS);
    let lines: Vec<&[u8]> = text[..length].split(|&c| c == b'|' || c == b'\n').collect();
    let advance = 1.25 + spread * 1.0;
    let line_height = 2.8;
    let slant = (twist - 0.5) * 0.8;
    let widest = lines
        .iter()
        .map(|line| line.len())
        .max()
        .unwrap_or(0)
        .max(1);
    // Fit the widest line to about four units before the camera fit.
    let unit = scale * 4.0 / (widest as f32 * advance).max(4.0);
    let total = lines.iter().map(|line| line.len()).sum::<usize>().max(1);
    let mut rng = Rng::new(seed);
    let mut index = 0;
    for (row, line) in lines.iter().enumerate() {
        let left = -(line.len() as f32 * advance - (advance - 1.0)) * 0.5;
        let baseline = ((lines.len() as f32 - 1.0) * 0.5 - row as f32) * line_height - 1.0;
        for (column, &character) in line.iter().enumerate() {
            let jitter = (rng.jitter(randomness * 0.3), rng.jitter(randomness * 0.3));
            let origin = (
                left + column as f32 * advance + jitter.0,
                baseline + jitter.1,
            );
            let t = index as f32 / total as f32;
            let lit = glyph(character);
            for (bit, (a, b)) in SEGMENTS.iter().enumerate() {
                if lit & (1 << bit) == 0 {
                    continue;
                }
                let point = |(x, y): (f32, f32)| {
                    let x = origin.0 + x + (y - 1.0) * slant;
                    Vec3::new(x * unit, (origin.1 + y) * unit, 0.0)
                };
                if !out.line(point(*a), point(*b), t) {
                    return;
                }
            }
            index += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letters_have_expected_segment_counts() {
        assert_eq!(glyph(b'A').count_ones(), 8);
        assert_eq!(glyph(b'a'), glyph(b'A'));
        assert_eq!(glyph(b'8').count_ones(), 10);
        assert_eq!(glyph(b' '), 0);
        let mut builder = Builder::new(10_000, 100);
        build(&encode("HI|8"), 1.0, 0.5, 0.5, 0.0, 1, &mut builder);
        assert_eq!(builder.segments.len(), 6 + 6 + 10);
        assert_eq!(encode("é")[0], b'?');
    }
}
