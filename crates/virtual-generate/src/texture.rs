//! Fill sources drawn per pixel: solid colour, colour bars, ramps, noise,
//! checkerboard and a projection test grid. Like the line patterns they
//! write straight-alpha RGBA8, so decks use them with every effect.

use std::f32::consts::TAU;

use crate::math::mix64;
use crate::settings::RecursivePattern;

/// Everything a fill source reads, already integrated over time.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Fill<'a> {
    pub pattern: RecursivePattern,
    pub extent: [u32; 2],
    /// Palette colours from the start (0) to the end (1), with brightness.
    pub lut: &'a [[f32; 3]],
    /// Scales colour bars, which ignore the palette.
    pub brightness: f32,
    pub depth_fade: f32,
    pub spread: f32,
    pub twist: f32,
    pub scale: f32,
    pub depth: f32,
    pub zoom: f32,
    pub line_width: f32,
    /// Ramp scroll, in ramp periods.
    pub scroll: f32,
    /// Rotation in radians.
    pub angle: f32,
    /// Noise flow along its third axis.
    pub flow: f32,
    pub seed: u32,
    pub transparent: bool,
}

impl Fill<'_> {
    /// Inputs that change the image, for skipping unchanged static frames.
    pub fn key(&self) -> Vec<u32> {
        let mut key = vec![
            self.pattern as u32,
            self.extent[0],
            self.extent[1],
            self.seed,
            u32::from(self.transparent),
        ];
        key.extend(
            [
                self.brightness,
                self.depth_fade,
                self.spread,
                self.twist,
                self.scale,
                self.depth,
                self.zoom,
                self.line_width,
                self.scroll,
                self.angle,
                self.flow,
            ]
            .map(f32::to_bits),
        );
        key.extend(self.lut.iter().flatten().map(|channel| channel.to_bits()));
        key
    }

    /// Colour at ramp position `t`, darkened toward the end by depth fade.
    fn color(&self, t: f32) -> [f32; 3] {
        let t = t.clamp(0.0, 1.0);
        let index = (t * (self.lut.len() - 1) as f32).round() as usize;
        let fade = 1.0 - self.depth_fade * t;
        self.lut[index].map(|channel| channel * fade)
    }
}

/// A triangle wave through 0 at 0 and 1 at 1, period 2: a plain ramp for
/// one repeat and seamless mirrored ramps beyond it.
fn mirror(value: f32) -> f32 {
    1.0 - (1.0 - 2.0 * (value * 0.5).rem_euclid(1.0)).abs()
}

fn lattice(x: i32, y: i32, z: i32, seed: u32) -> f32 {
    let hash = mix64(
        u64::from(seed)
            ^ (x as u32 as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
            ^ (y as u32 as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
            ^ (z as u32 as u64).wrapping_mul(0x1656_67B1_9E37_79F9),
    );
    (hash >> 40) as f32 / (1u64 << 24) as f32
}

/// Smoothly interpolated value noise in `[0, 1]`.
fn value_noise(x: f32, y: f32, z: f32, seed: u32) -> f32 {
    let (fx, fy, fz) = (x.floor(), y.floor(), z.floor());
    let (ix, iy, iz) = (fx as i32, fy as i32, fz as i32);
    let fade = |t: f32| t * t * (3.0 - 2.0 * t);
    let (u, v, w) = (fade(x - fx), fade(y - fy), fade(z - fz));
    let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let corner = |dx, dy, dz| lattice(ix + dx, iy + dy, iz + dz, seed);
    lerp(
        lerp(
            lerp(corner(0, 0, 0), corner(1, 0, 0), u),
            lerp(corner(0, 1, 0), corner(1, 1, 0), u),
            v,
        ),
        lerp(
            lerp(corner(0, 0, 1), corner(1, 0, 1), u),
            lerp(corner(0, 1, 1), corner(1, 1, 1), u),
            v,
        ),
        w,
    )
}

/// Fractal (fBm) noise in `[0, 1]`.
fn fractal_noise(x: f32, y: f32, z: f32, octaves: u32, seed: u32) -> f32 {
    let (mut total, mut amplitude, mut frequency, mut norm) = (0.0, 1.0, 1.0, 0.0);
    for octave in 0..octaves {
        total += amplitude
            * value_noise(
                x * frequency,
                y * frequency,
                z * frequency,
                seed.wrapping_add(octave),
            );
        norm += amplitude;
        amplitude *= 0.5;
        frequency *= 2.0;
    }
    total / norm
}

/// 75% colour bars over a reverse row, as on broadcast test cards.
const BARS: [[f32; 3]; 7] = [
    [0.75, 0.75, 0.75],
    [0.75, 0.75, 0.0],
    [0.0, 0.75, 0.75],
    [0.0, 0.75, 0.0],
    [0.75, 0.0, 0.75],
    [0.75, 0.0, 0.0],
    [0.0, 0.0, 0.75],
];
const REVERSE: [[f32; 3]; 7] = [
    [0.0, 0.0, 0.75],
    [0.0, 0.0, 0.0],
    [0.75, 0.0, 0.75],
    [0.0, 0.0, 0.0],
    [0.0, 0.75, 0.75],
    [0.0, 0.0, 0.0],
    [0.75, 0.75, 0.75],
];

/// Draws the fill into `out` (tightly packed RGBA8, `extent` sized).
pub(crate) fn render(fill: &Fill, out: &mut [u8]) {
    let [width, height] = fill.extent.map(|v| v as usize);
    debug_assert_eq!(out.len(), width * height * 4);
    let short = width.min(height).max(1) as f32 * 0.5;
    let half = [width as f32 * 0.5, height as f32 * 0.5];
    let (sin, cos) = fill.angle.sin_cos();
    let zoom = fill.zoom.max(0.05);
    // Centred, rotated coordinates where the short half-side is 1.
    let coordinates = |x: f32, y: f32| {
        let (u, v) = ((x - half[0]) / short / zoom, (half[1] - y) / short / zoom);
        (u * cos + v * sin, v * cos - u * sin)
    };
    let extent = (half[0] / short, half[1] / short);
    let repeats = 1.0 + fill.twist * 7.0;

    // Noise is smooth, so it is computed on a coarse grid and interpolated.
    const NOISE_STEP: usize = 4;
    let noise_grid = (fill.pattern == RecursivePattern::Noise).then(|| {
        let columns = width / NOISE_STEP + 2;
        let rows = height / NOISE_STEP + 2;
        let frequency = 1.0 + fill.scale * 6.0;
        let octaves = 1 + (fill.depth * 5.0).round() as u32;
        let mut grid = vec![0.0_f32; columns * rows];
        for row in 0..rows {
            for column in 0..columns {
                let (u, v) = coordinates((column * NOISE_STEP) as f32, (row * NOISE_STEP) as f32);
                let n = fractal_noise(u * frequency, v * frequency, fill.flow, octaves, fill.seed);
                // Stretch the fBm's bunched middle values toward the ends.
                grid[row * columns + column] = ((n - 0.5) * 2.2 + 0.5).clamp(0.0, 1.0);
            }
        }
        (grid, columns)
    });

    let cells = 2.0 + (fill.spread * 30.0).floor();
    for (index, pixel) in out.chunks_exact_mut(4).enumerate() {
        let (px, py) = ((index % width) as f32 + 0.5, (index / width) as f32 + 0.5);
        let rgb = match fill.pattern {
            RecursivePattern::ColorBars => {
                let column = ((px / width as f32) * 7.0).floor().clamp(0.0, 6.0) as usize;
                let row = if py < height as f32 * 0.75 {
                    BARS
                } else {
                    REVERSE
                };
                row[column].map(|c| c * fill.brightness)
            }
            RecursivePattern::LinearRamp => {
                let (u, v) = coordinates(px, py);
                let direction = fill.spread * TAU;
                let (ds, dc) = direction.sin_cos();
                let reach = (dc.abs() * extent.0 + ds.abs() * extent.1).max(1e-3) / zoom;
                let position = ((u * dc + v * ds) / reach + 1.0) * 0.5;
                fill.color(mirror(position * repeats + fill.scroll))
            }
            RecursivePattern::RadialRamp => {
                let (u, v) = coordinates(px, py);
                let reach = extent.0.hypot(extent.1) / zoom;
                fill.color(mirror(u.hypot(v) / reach * repeats - fill.scroll))
            }
            RecursivePattern::Noise => {
                let (grid, columns) = noise_grid.as_ref().expect("noise grid");
                let (gx, gy) = (px / NOISE_STEP as f32, py / NOISE_STEP as f32);
                let (cx, cy) = (gx.floor() as usize, gy.floor() as usize);
                let (fx, fy) = (gx - cx as f32, gy - cy as f32);
                let at = |x: usize, y: usize| grid[y * columns + x];
                let top = at(cx, cy) + (at(cx + 1, cy) - at(cx, cy)) * fx;
                let bottom = at(cx, cy + 1) + (at(cx + 1, cy + 1) - at(cx, cy + 1)) * fx;
                fill.color(top + (bottom - top) * fy)
            }
            RecursivePattern::Checker => {
                let (u, v) = coordinates(px, py);
                let cell = |value: f32| (value * cells * 0.5).floor() as i64;
                fill.color(((cell(u) + cell(v)).rem_euclid(2)) as f32)
            }
            RecursivePattern::Grid => {
                // Lines in pixels from the centre, so the centre cross,
                // border and circle stay sharp at any resolution.
                let spacing = (short * 2.0 / cells).max(2.0);
                let line = fill.line_width.max(1.0);
                let (dx, dy) = (px - half[0], half[1] - py);
                let near = |value: f32| {
                    let offset = value.rem_euclid(spacing);
                    offset.min(spacing - offset) <= line * 0.5
                };
                let border =
                    px < line || py < line || px > width as f32 - line || py > height as f32 - line;
                let axes = dx.abs() <= line || dy.abs() <= line;
                let circle = (dx.hypot(dy) - short * 0.9).abs() <= line * 0.75;
                if border || axes || circle {
                    fill.color(0.0)
                } else if near(dx) || near(dy) {
                    fill.color(0.0).map(|c| c * 0.55)
                } else {
                    [0.0; 3]
                }
            }
            _ => fill.color(0.0),
        };
        let rgb = rgb.map(|c| c.clamp(0.0, 1.0));
        if fill.transparent {
            let alpha = rgb[0].max(rgb[1]).max(rgb[2]);
            if alpha <= 1.0 / 512.0 {
                pixel.copy_from_slice(&[0, 0, 0, 0]);
                continue;
            }
            let inverse = 1.0 / alpha;
            pixel[0] = to_byte(rgb[0] * inverse);
            pixel[1] = to_byte(rgb[1] * inverse);
            pixel[2] = to_byte(rgb[2] * inverse);
            pixel[3] = to_byte(alpha);
        } else {
            pixel[0] = to_byte(rgb[0]);
            pixel[1] = to_byte(rgb[1]);
            pixel[2] = to_byte(rgb[2]);
            pixel[3] = 255;
        }
    }
}

fn to_byte(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fill(pattern: RecursivePattern, lut: &[[f32; 3]]) -> Fill<'_> {
        Fill {
            pattern,
            extent: [64, 32],
            lut,
            brightness: 1.0,
            depth_fade: 0.0,
            spread: 0.0,
            twist: 0.0,
            scale: 0.5,
            depth: 0.5,
            zoom: 1.0,
            line_width: 1.0,
            scroll: 0.0,
            angle: 0.0,
            flow: 0.0,
            seed: 7,
            transparent: false,
        }
    }

    fn draw(fill: &Fill) -> Vec<u8> {
        let mut out = vec![0; 64 * 32 * 4];
        render(fill, &mut out);
        out
    }

    fn pixel(frame: &[u8], x: usize, y: usize) -> [u8; 4] {
        let at = (y * 64 + x) * 4;
        frame[at..at + 4].try_into().unwrap()
    }

    const RED_TO_BLUE: [[f32; 3]; 2] = [[1.0, 0.0, 0.0], [0.0, 0.0, 1.0]];

    #[test]
    fn ramps_run_from_start_to_end_colour() {
        let frame = draw(&fill(RecursivePattern::LinearRamp, &RED_TO_BLUE));
        assert_eq!(pixel(&frame, 0, 16), [255, 0, 0, 255]);
        assert_eq!(pixel(&frame, 63, 16), [0, 0, 255, 255]);
        let mut faded = fill(RecursivePattern::LinearRamp, &RED_TO_BLUE);
        faded.depth_fade = 1.0;
        assert!(pixel(&draw(&faded), 63, 16)[2] < 8, "faded to black");
        let radial = draw(&fill(RecursivePattern::RadialRamp, &RED_TO_BLUE));
        assert_eq!(pixel(&radial, 32, 16), [255, 0, 0, 255]);
        assert!(pixel(&radial, 0, 0)[2] > 240);
    }

    #[test]
    fn noise_is_deterministic_per_seed_and_varied() {
        let ramp: Vec<[f32; 3]> = (0..256).map(|i| [i as f32 / 255.0, 0.0, 0.0]).collect();
        let a = draw(&fill(RecursivePattern::Noise, &ramp));
        assert_eq!(a, draw(&fill(RecursivePattern::Noise, &ramp)));
        let mut other = fill(RecursivePattern::Noise, &ramp);
        other.seed = 8;
        assert_ne!(a, draw(&other));
        let reds: std::collections::HashSet<u8> = a.chunks(4).map(|p| p[0]).collect();
        assert!(reds.len() > 20, "noise has many levels");
    }

    #[test]
    fn bars_checker_and_grid() {
        let white = [[1.0, 1.0, 1.0]];
        let bars = draw(&fill(RecursivePattern::ColorBars, &white));
        assert_eq!(pixel(&bars, 1, 1), [191, 191, 191, 255]);
        assert_eq!(pixel(&bars, 62, 1), [0, 0, 191, 255]);
        let checker = draw(&fill(RecursivePattern::Checker, &RED_TO_BLUE));
        assert_ne!(pixel(&checker, 31, 15), pixel(&checker, 33, 15));
        let grid = draw(&fill(RecursivePattern::Grid, &white));
        assert_eq!(pixel(&grid, 32, 5), [255, 255, 255, 255], "centre axis");
        assert_eq!(pixel(&grid, 0, 10), [255, 255, 255, 255], "border");
        let mut clear = fill(RecursivePattern::Grid, &white);
        clear.transparent = true;
        assert_eq!(pixel(&draw(&clear), 20, 12)[3], 0);
    }
}
