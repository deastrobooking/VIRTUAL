//! Averages the program image into vertical colour strips for LED fixtures.

/// Mean sRGB colour of `zones` equal-width vertical strips, left to right,
/// from RGBA8 rows `stride` bytes apart. Samples every fourth pixel and row,
/// which is ample for a colour average and keeps 1080p under a millisecond.
pub fn average_zones(data: &[u8], extent: [u32; 2], stride: u32, zones: usize) -> Vec<[u8; 3]> {
    let [width, height] = [extent[0] as usize, extent[1] as usize];
    let zones = zones.clamp(1, crate::protocol::MAX_ZONES).min(width.max(1));
    let mut sums = vec![[0u64; 3]; zones];
    let mut counts = vec![0u64; zones];
    for y in (0..height).step_by(4) {
        let row = y * stride as usize;
        for x in (0..width).step_by(4) {
            let i = row + x * 4;
            let Some(pixel) = data.get(i..i + 3) else {
                continue;
            };
            let zone = x * zones / width;
            for c in 0..3 {
                sums[zone][c] += u64::from(pixel[c]);
            }
            counts[zone] += 1;
        }
    }
    sums.iter()
        .zip(&counts)
        .map(|(sum, &count)| sum.map(|s| s.checked_div(count).unwrap_or(0) as u8))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_average_left_to_right() {
        // 8×4 image: left half red, right half blue, rows padded to 64 bytes.
        let (w, h, stride) = (8u32, 4u32, 64u32);
        let mut data = vec![0u8; (stride * h) as usize];
        for y in 0..h {
            for x in 0..w {
                let i = (y * stride + x * 4) as usize;
                data[i..i + 4].copy_from_slice(if x < 4 {
                    &[255, 0, 0, 255]
                } else {
                    &[0, 0, 255, 255]
                });
            }
        }
        assert_eq!(
            average_zones(&data, [w, h], stride, 2),
            [[255, 0, 0], [0, 0, 255]]
        );
        assert_eq!(average_zones(&data, [w, h], stride, 1), [[127, 0, 127]]);
    }
}
