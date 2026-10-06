//! CPU line rasterizer and the per-deck generator runtime.
//!
//! Segments are projected through an orbiting camera and splatted
//! additively (bilinear, sub-pixel) into a float accumulation buffer, then
//! tone-mapped to straight-alpha RGBA8 — the same frame format the deck
//! decoders produce, so generated decks reuse upload, effects, blending,
//! freeze and recording unchanged.

use std::f32::consts::{FRAC_PI_2, TAU};
use std::time::Instant;

use crate::geometry::{AudioBands, Geometry, GeometryParams, Palette, generate};
use crate::math::{Mat3, Vec3};
use crate::settings::GeneratorSettings;

/// Camera distance in fitted-radius units for the perspective divide.
const CAMERA_DISTANCE: f32 = 2.6;
/// Fraction of half the frame height the fitted bounding sphere fills.
const FIT_FRACTION: f32 = 0.9;
const PALETTE_ENTRIES: usize = 256;
/// Per-line sample cap so a degenerate projection can't stall a frame.
const MAX_STEPS_PER_LINE: f32 = 8192.0;

/// Geometry-affecting inputs, quantised so tiny control/LFO jitter doesn't
/// regenerate on every frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct GeometryKey {
    pattern: crate::RecursivePattern,
    levels: u32,
    controls: [i32; 4],
    seed: u32,
    flatten: bool,
    audio: [i32; 3],
    time: i32,
}

impl GeometryKey {
    fn new(settings: &GeneratorSettings, audio: AudioBands, time: f32) -> Self {
        let q = |value: f32| (value * 256.0).round() as i32;
        let reactive = settings.audio_amount > 0.0 && settings.pattern.audio_shapes_geometry();
        let audio = if reactive {
            let audio = audio.scaled(settings.audio_amount);
            [q(audio.bass), q(audio.mid), q(audio.high)]
        } else {
            [0; 3]
        };
        // Only Recursive Web animates its geometry with time, and only while
        // audio drives the displacement.
        let time = if reactive && settings.pattern == crate::RecursivePattern::RecursiveWeb {
            (time * 30.0) as i32
        } else {
            0
        };
        Self {
            pattern: settings.pattern,
            levels: settings.depth_levels(),
            controls: [
                q(settings.scale),
                q(settings.spread),
                q(settings.twist),
                q(settings.randomness),
            ],
            seed: settings.seed,
            flatten: settings.flatten,
            audio,
            time,
        }
    }
}

/// Snapshot of the most recent frame for diagnostics.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GeneratorStats {
    pub segments: u32,
    pub drawn: u32,
    pub requested_depth: u32,
    pub depth: u32,
    pub truncated: bool,
    pub planar: bool,
    pub generate_micros: u32,
    pub render_micros: u32,
}

pub struct Generator {
    settings: GeneratorSettings,
    geometry: Geometry,
    key: Option<GeometryKey>,
    accumulation: Vec<[f32; 3]>,
    fit_center: Vec3,
    fit_radius: f32,
    yaw: f32,
    spin: f32,
    grow_phase: f32,
    last_time: Option<f64>,
    stats: GeneratorStats,
}

impl Generator {
    pub fn new(settings: GeneratorSettings) -> Self {
        let settings = settings.sanitized();
        let [width, height] = settings.resolution;
        Self {
            settings,
            geometry: Geometry::default(),
            key: None,
            accumulation: vec![[0.0; 3]; (width * height) as usize],
            fit_center: Vec3::ZERO,
            fit_radius: 0.0,
            yaw: 0.0,
            spin: 0.0,
            grow_phase: 0.0,
            last_time: None,
            stats: GeneratorStats::default(),
        }
    }

    pub fn settings(&self) -> &GeneratorSettings {
        &self.settings
    }

    /// Applies new settings. Animation phase (rotation, growth, trails) is
    /// kept so live tweaks never jump; geometry regenerates on next render
    /// only if a geometry input changed.
    pub fn set_settings(&mut self, settings: GeneratorSettings) {
        let settings = settings.sanitized();
        if settings.resolution != self.settings.resolution {
            let [width, height] = settings.resolution;
            self.accumulation = vec![[0.0; 3]; (width * height) as usize];
        }
        if settings.pattern != self.settings.pattern {
            self.fit_radius = 0.0;
        }
        self.settings = settings;
    }

    pub fn extent(&self) -> [u32; 2] {
        self.settings.resolution
    }

    pub fn geometry(&self) -> &Geometry {
        &self.geometry
    }

    pub fn stats(&self) -> GeneratorStats {
        self.stats
    }

    /// Renders one frame at `time` seconds into tightly packed RGBA8 rows.
    ///
    /// # Panics
    /// If `out` is not exactly `width × height × 4` bytes.
    pub fn render(&mut self, time: f64, audio: AudioBands, out: &mut [u8]) {
        let [width, height] = self.settings.resolution;
        assert_eq!(out.len(), (width * height * 4) as usize, "frame size");
        let started = Instant::now();
        let dt = self
            .last_time
            .map_or(0.0, |last| (time - last).clamp(0.0, 0.25) as f32);
        self.last_time = Some(time);
        let settings = self.settings.clone();
        let seconds = time as f32;

        let key = GeometryKey::new(&settings, audio, seconds);
        if self.key != Some(key) {
            let params = GeometryParams::from_settings(&settings, audio, seconds);
            self.geometry = generate(&params);
            self.key = Some(key);
            self.stats.generate_micros = started.elapsed().as_micros().min(u32::MAX as u128) as u32;
        }

        // Integrate speeds so changing a speed control never jumps the pose.
        self.yaw = (self.yaw + settings.rotate_speed * 0.5 * TAU * dt).rem_euclid(TAU);
        self.spin = (self.spin + settings.spin_speed * 0.5 * TAU * dt).rem_euclid(TAU);
        if settings.grow_speed > 0.0 {
            self.grow_phase = (self.grow_phase + settings.grow_speed * 0.5 * dt).rem_euclid(1.0);
        }

        // Ease the framing toward new geometry instead of popping.
        let target_radius = self.geometry.radius.max(1e-3);
        if self.fit_radius <= 0.0 {
            self.fit_radius = target_radius;
            self.fit_center = self.geometry.center;
        } else {
            let ease = 1.0 - (-dt * 6.0).exp();
            self.fit_radius += (target_radius - self.fit_radius) * ease;
            self.fit_center = self.fit_center.lerp(self.geometry.center, ease);
        }

        if settings.trails > 0.0 {
            let keep = settings.trails;
            for pixel in &mut self.accumulation {
                pixel[0] *= keep;
                pixel[1] *= keep;
                pixel[2] *= keep;
            }
        } else {
            self.accumulation.fill([0.0; 3]);
        }

        let palette = Palette::from_settings(&settings, audio, seconds);
        let lut: Vec<[f32; 3]> = (0..PALETTE_ENTRIES)
            .map(|index| {
                let color = palette.color(index as f32 / (PALETTE_ENTRIES - 1) as f32);
                color.map(|channel| channel * settings.brightness)
            })
            .collect();

        let reveal = settings.reveal
            * if settings.grow_speed > 0.0 {
                smooth(self.grow_phase)
            } else {
                1.0
            };
        let count = self.geometry.segments.len();
        let drawn = ((count as f32 * reveal).ceil() as usize).min(count);

        let camera = Mat3::rotation_y(self.yaw)
            .then(&Mat3::rotation_x(settings.tilt * FRAC_PI_2))
            .then(&Mat3::rotation_z(self.spin));
        // Points are normalised by the fitted radius before projection.
        let pixels_per_unit = settings.zoom * FIT_FRACTION * 0.5 * height as f32;
        let half = [width as f32 * 0.5, height as f32 * 0.5];
        let inverse_radius = 1.0 / self.fit_radius;
        let project = |point: Vec3| {
            let q = camera.transform((point - self.fit_center) * inverse_radius);
            let divide = CAMERA_DISTANCE / (CAMERA_DISTANCE - q.z).max(0.2);
            let s = 1.0 + (divide - 1.0) * settings.perspective;
            let fade = 1.0 - settings.depth_fade * (1.0 - ((q.z + 1.0) * 0.5).clamp(0.0, 1.0));
            (
                half[0] + q.x * s * pixels_per_unit,
                half[1] - q.y * s * pixels_per_unit,
                fade,
            )
        };

        let mut canvas = Canvas {
            pixels: &mut self.accumulation,
            width: width as usize,
            height: height as usize,
        };
        for segment in &self.geometry.segments[..drawn] {
            let (x0, y0, fade0) = project(segment.start);
            let (x1, y1, fade1) = project(segment.end);
            let index = (segment.t.clamp(0.0, 1.0) * (PALETTE_ENTRIES - 1) as f32) as usize;
            let fade = (fade0 + fade1) * 0.5;
            let color = lut[index].map(|channel| channel * fade);
            canvas.line(x0, y0, x1, y1, color, settings.line_width);
        }

        tone_map(&self.accumulation, out, settings.transparent);

        self.stats = GeneratorStats {
            segments: count as u32,
            drawn: drawn as u32,
            requested_depth: self.geometry.requested_depth,
            depth: self.geometry.depth,
            truncated: self.geometry.truncated,
            planar: self.geometry.is_planar(),
            generate_micros: self.stats.generate_micros,
            render_micros: started.elapsed().as_micros().min(u32::MAX as u128) as u32,
        };
    }
}

fn smooth(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

/// Reinhard per channel. Transparent output stores straight alpha equal to
/// the brightest channel, so `rgb × a` over black reproduces the opaque look.
fn tone_map(accumulation: &[[f32; 3]], out: &mut [u8], transparent: bool) {
    for (pixel, rgba) in accumulation.iter().zip(out.chunks_exact_mut(4)) {
        let mapped = pixel.map(|value| value / (1.0 + value));
        if transparent {
            let alpha = mapped[0].max(mapped[1]).max(mapped[2]);
            if alpha <= 1.0 / 512.0 {
                rgba.copy_from_slice(&[0, 0, 0, 0]);
                continue;
            }
            let inverse = 1.0 / alpha;
            rgba[0] = to_byte(mapped[0] * inverse);
            rgba[1] = to_byte(mapped[1] * inverse);
            rgba[2] = to_byte(mapped[2] * inverse);
            rgba[3] = to_byte(alpha);
        } else {
            rgba[0] = to_byte(mapped[0]);
            rgba[1] = to_byte(mapped[1]);
            rgba[2] = to_byte(mapped[2]);
            rgba[3] = 255;
        }
    }
}

fn to_byte(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
}

struct Canvas<'a> {
    pixels: &'a mut [[f32; 3]],
    width: usize,
    height: usize,
}

impl Canvas<'_> {
    /// Anti-aliased additive line: samples every pixel step along the line
    /// (and across it for thick lines) and splats each sample bilinearly.
    /// The final endpoint is skipped so joined path segments don't double
    /// up at their joints.
    fn line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, color: [f32; 3], width: f32) {
        let margin = width + 2.0;
        let Some((x0, y0, x1, y1)) = clip(
            x0,
            y0,
            x1,
            y1,
            -margin,
            -margin,
            self.width as f32 + margin,
            self.height as f32 + margin,
        ) else {
            return;
        };
        let (dx, dy) = (x1 - x0, y1 - y0);
        let length = (dx * dx + dy * dy).sqrt();
        let steps = length.ceil().clamp(1.0, MAX_STEPS_PER_LINE);
        let (step_x, step_y) = (dx / steps, dy / steps);
        let across = width.ceil().max(1.0);
        let spacing = width / across;
        let (normal_x, normal_y) = if length > f32::EPSILON {
            (-dy / length, dx / length)
        } else {
            (0.0, 0.0)
        };
        let weight = spacing;
        let color = color.map(|channel| channel * weight);
        let first_offset = -(across - 1.0) * 0.5 * spacing;
        for lane in 0..across as u32 {
            let offset = first_offset + lane as f32 * spacing;
            let (mut x, mut y) = (x0 + normal_x * offset, y0 + normal_y * offset);
            for _ in 0..steps as u32 {
                self.splat(x, y, color);
                x += step_x;
                y += step_y;
            }
        }
    }

    fn splat(&mut self, x: f32, y: f32, color: [f32; 3]) {
        let (x, y) = (x - 0.5, y - 0.5);
        let (fx, fy) = (x.floor(), y.floor());
        let (tx, ty) = (x - fx, y - fy);
        let (ix, iy) = (fx as isize, fy as isize);
        for (dx, dy, weight) in [
            (0, 0, (1.0 - tx) * (1.0 - ty)),
            (1, 0, tx * (1.0 - ty)),
            (0, 1, (1.0 - tx) * ty),
            (1, 1, tx * ty),
        ] {
            let (px, py) = (ix + dx, iy + dy);
            if px < 0 || py < 0 || px >= self.width as isize || py >= self.height as isize {
                continue;
            }
            let pixel = &mut self.pixels[py as usize * self.width + px as usize];
            pixel[0] += color[0] * weight;
            pixel[1] += color[1] * weight;
            pixel[2] += color[2] * weight;
        }
    }
}

/// Liang–Barsky clip of a segment to an axis-aligned rectangle.
#[allow(clippy::too_many_arguments)]
fn clip(
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    min_x: f32,
    min_y: f32,
    max_x: f32,
    max_y: f32,
) -> Option<(f32, f32, f32, f32)> {
    if !(x0.is_finite() && y0.is_finite() && x1.is_finite() && y1.is_finite()) {
        return None;
    }
    let (dx, dy) = (x1 - x0, y1 - y0);
    let (mut enter, mut exit) = (0.0f32, 1.0f32);
    for (p, q) in [
        (-dx, x0 - min_x),
        (dx, max_x - x0),
        (-dy, y0 - min_y),
        (dy, max_y - y0),
    ] {
        if p == 0.0 {
            if q < 0.0 {
                return None;
            }
        } else {
            let r = q / p;
            if p < 0.0 {
                enter = enter.max(r);
            } else {
                exit = exit.min(r);
            }
            if enter > exit {
                return None;
            }
        }
    }
    Some((
        x0 + enter * dx,
        y0 + enter * dy,
        x0 + exit * dx,
        y0 + exit * dy,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RecursivePattern;

    fn small(pattern: RecursivePattern) -> GeneratorSettings {
        GeneratorSettings {
            pattern,
            resolution: [160, 90],
            ..GeneratorSettings::default()
        }
    }

    fn frame(generator: &mut Generator, time: f64) -> Vec<u8> {
        let [w, h] = generator.extent();
        let mut out = vec![0; (w * h * 4) as usize];
        generator.render(time, AudioBands::default(), &mut out);
        out
    }

    #[test]
    fn every_pattern_draws_visible_pixels() {
        for pattern in RecursivePattern::ALL {
            let mut generator = Generator::new(small(pattern));
            let out = frame(&mut generator, 0.0);
            let lit = out.chunks_exact(4).filter(|p| p[3] > 0).count();
            assert!(lit > 50, "{} lit only {lit} pixels", pattern.label());
            assert!(generator.stats().segments > 0);
        }
    }

    #[test]
    fn opaque_mode_writes_full_alpha_and_transparent_background_is_clear() {
        let mut generator = Generator::new(GeneratorSettings {
            transparent: false,
            ..small(RecursivePattern::KochSnowflake)
        });
        assert!(
            frame(&mut generator, 0.0)
                .chunks_exact(4)
                .all(|p| p[3] == 255)
        );
        let mut generator = Generator::new(small(RecursivePattern::KochSnowflake));
        let out = frame(&mut generator, 0.0);
        assert_eq!(&out[..4], &[0, 0, 0, 0], "corner stays transparent");
    }

    #[test]
    fn reveal_zero_draws_nothing_and_rendering_is_deterministic() {
        let mut generator = Generator::new(GeneratorSettings {
            reveal: 0.0,
            ..small(RecursivePattern::FractalTree)
        });
        assert!(frame(&mut generator, 0.0).iter().all(|&b| b == 0));
        let mut a = Generator::new(small(RecursivePattern::MengerSponge));
        let mut b = Generator::new(small(RecursivePattern::MengerSponge));
        assert_eq!(frame(&mut a, 0.5), frame(&mut b, 0.5));
    }

    #[test]
    fn geometry_regenerates_only_when_geometry_inputs_change() {
        let mut generator = Generator::new(small(RecursivePattern::CrystalGrowth));
        frame(&mut generator, 0.0);
        let before = generator.geometry().clone();
        let mut look = generator.settings().clone();
        look.hue = 0.1;
        look.zoom = 2.0;
        generator.set_settings(look.clone());
        frame(&mut generator, 0.1);
        assert_eq!(generator.geometry(), &before);
        look.spread = 0.9;
        generator.set_settings(look);
        frame(&mut generator, 0.2);
        assert_ne!(generator.geometry(), &before);
    }

    #[test]
    fn resolution_change_resizes_output() {
        let mut generator = Generator::new(small(RecursivePattern::HTree));
        let mut settings = generator.settings().clone();
        settings.resolution = [64, 64];
        generator.set_settings(settings);
        assert_eq!(frame(&mut generator, 0.0).len(), 64 * 64 * 4);
    }

    #[test]
    fn clip_rejects_outside_and_trims_crossing_segments() {
        assert!(clip(-10.0, -10.0, -5.0, -1.0, 0.0, 0.0, 100.0, 100.0).is_none());
        let (x0, _, x1, _) = clip(-50.0, 10.0, 150.0, 10.0, 0.0, 0.0, 100.0, 100.0).unwrap();
        assert_eq!((x0, x1), (0.0, 100.0));
        assert!(clip(f32::NAN, 0.0, 1.0, 1.0, 0.0, 0.0, 10.0, 10.0).is_none());
    }
}
