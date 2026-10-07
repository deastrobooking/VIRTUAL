//! Renderer-neutral geometry output and the generation entry point.

use crate::math::{Vec3, hsl_to_rgb};
use crate::patterns;
use crate::settings::{ColorMode, Dimension, GeneratorSettings, RecursivePattern};
use crate::turtle::{Builder, MAX_SEGMENTS};

/// Live audio band levels, each nominally `[0, 1]`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AudioBands {
    pub bass: f32,
    pub mid: f32,
    pub high: f32,
}

impl AudioBands {
    pub fn scaled(self, amount: f32) -> Self {
        let band = |value: f32| {
            if value.is_finite() {
                (value * amount).clamp(0.0, 1.0)
            } else {
                0.0
            }
        };
        Self {
            bass: band(self.bass),
            mid: band(self.mid),
            high: band(self.high),
        }
    }
}

/// One independent line segment. `t` in `[0, 1]` drives colour and reveal
/// order; how it is assigned depends on the pattern's [`ColorMode`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Segment {
    pub start: Vec3,
    pub end: Vec3,
    pub t: f32,
    /// Arc-length start/end and stable path phase.
    pub trace: [f32; 3],
    pub echo: u32,
}

/// Engine-level inputs: normalised controls already mapped to pattern units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GeometryParams {
    pub pattern: RecursivePattern,
    /// Requested recursion levels (3‥11 from the depth control).
    pub levels: u32,
    pub scale: f32,
    pub spread: f32,
    pub twist: f32,
    pub randomness: f32,
    pub seed: u64,
    pub flatten: bool,
    /// Already multiplied by the operator's audio amount.
    pub audio: AudioBands,
    pub time: f32,
    pub budget: usize,
    pub degree_x: f32,
    pub degree_y: f32,
    pub degree_z: f32,
    pub polynomial_mix: f32,
    pub surface_order: f32,
    pub surface_cross: f32,
    pub contours: f32,
    pub slice_axis: f32,
    pub symmetry: f32,
    pub exponent: f32,
    pub echo_copies: f32,
    pub echo_scale: f32,
    pub echo_x: f32,
    pub echo_y: f32,
    pub echo_z: f32,
    pub echo_offset: f32,
}

impl GeometryParams {
    pub fn from_settings(settings: &GeneratorSettings, audio: AudioBands, time: f32) -> Self {
        Self {
            pattern: settings.pattern,
            levels: settings.depth_levels(),
            scale: settings.scale_units(),
            spread: settings.spread,
            twist: settings.twist,
            randomness: settings.randomness,
            seed: u64::from(settings.seed),
            flatten: settings.flatten,
            audio: audio.scaled(settings.audio_amount),
            time,
            budget: MAX_SEGMENTS,
            degree_x: settings.degree_x,
            degree_y: settings.degree_y,
            degree_z: settings.degree_z,
            polynomial_mix: settings.polynomial_mix,
            surface_order: settings.surface_order,
            surface_cross: settings.surface_cross,
            contours: settings.contours,
            slice_axis: settings.slice_axis,
            symmetry: settings.symmetry,
            exponent: settings.exponent,
            echo_copies: settings.echo_copies,
            echo_scale: settings.echo_scale,
            echo_x: settings.echo_x,
            echo_y: settings.echo_y,
            echo_z: settings.echo_z,
            echo_offset: settings.echo_offset,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Geometry {
    pub segments: Vec<Segment>,
    pub pattern: RecursivePattern,
    /// `Planar` when every vertex has `z = 0`.
    pub dimension: Option<Dimension>,
    /// Depth requested by the pattern's own mapping of the levels control.
    pub requested_depth: u32,
    /// Depth actually generated after the segment-budget clamp.
    pub depth: u32,
    /// The builder hit its hard ceiling (the clamp underestimated).
    pub truncated: bool,
    /// Bounding-box centre and the radius of the bounding sphere about it.
    pub center: Vec3,
    pub radius: f32,
}

impl Geometry {
    pub fn is_planar(&self) -> bool {
        self.segments
            .iter()
            .all(|segment| segment.start.z == 0.0 && segment.end.z == 0.0)
    }

    /// Packed GPU-style buffers: six floats of position and six of colour
    /// per segment; both endpoints share the segment colour.
    pub fn to_buffers(&self, palette: &Palette) -> GeometryBuffers {
        let mut buffers = GeometryBuffers {
            positions: Vec::with_capacity(self.segments.len() * 6),
            colors: Vec::with_capacity(self.segments.len() * 6),
        };
        for segment in &self.segments {
            buffers.positions.extend(segment.start.to_array());
            buffers.positions.extend(segment.end.to_array());
            let color = palette.color(segment.t);
            buffers.colors.extend(color);
            buffers.colors.extend(color);
        }
        buffers
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct GeometryBuffers {
    pub positions: Vec<f32>,
    pub colors: Vec<f32>,
}

/// Per-segment colour as a function of `t`:
/// `hue = base + t × range + time × speed × 0.04 + high × 0.15`,
/// saturation lifted by mid, lightness by bass.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    pub hue: f32,
    pub hue_range: f32,
    pub saturation: f32,
    pub lightness: f32,
    pub time: f32,
    pub color_speed: f32,
    pub audio: AudioBands,
}

impl Palette {
    pub fn from_settings(settings: &GeneratorSettings, audio: AudioBands, time: f32) -> Self {
        Self {
            hue: settings.hue,
            hue_range: settings.hue_range,
            saturation: settings.saturation,
            lightness: settings.lightness,
            time,
            color_speed: settings.color_speed,
            audio: audio.scaled(settings.audio_amount),
        }
    }

    pub fn color(&self, t: f32) -> [f32; 3] {
        let hue = self.hue
            + t * self.hue_range
            + self.time * self.color_speed * 0.04
            + self.audio.high * 0.15;
        hsl_to_rgb(
            hue.rem_euclid(1.0),
            (self.saturation + self.audio.mid * 0.35).clamp(0.0, 1.0),
            (self.lightness + self.audio.bass * 0.35).clamp(0.0, 1.0),
        )
    }
}

/// Generates a pattern. Never allocates more than `params.budget` segments.
pub fn generate(params: &GeometryParams) -> Geometry {
    let copies = params.echo_copies.round().clamp(1.0, 24.0) as usize;
    let budget = params.budget.min(MAX_SEGMENTS) / copies;
    let requested_depth = patterns::requested_depth(params.pattern, params.levels);
    let depth = patterns::effective_depth(params, requested_depth, budget as u64);
    let mut out = Builder::new(budget, patterns::estimate(params, depth));
    patterns::build(params, depth, &mut out);
    let truncated = out.truncated;
    let mut segments = out.segments;

    // Preserve continuous paths before color/reveal ordering changes them.
    let mut first = 0;
    let mut path_id = 0;
    while first < segments.len() {
        let mut end = first + 1;
        while end < segments.len() && segments[end - 1].end == segments[end].start {
            end += 1;
        }
        let length: f32 = segments[first..end]
            .iter()
            .map(|s| (s.end - s.start).length())
            .sum();
        let mut distance = 0.0;
        for segment in &mut segments[first..end] {
            let next = distance + (segment.end - segment.start).length();
            segment.trace = [
                distance / length.max(1e-9),
                next / length.max(1e-9),
                (path_id as f32 * 0.618_034).fract(),
            ];
            distance = next;
        }
        first = end;
        path_id += 1;
    }
    let base_count = segments.len();
    let rotation = crate::Mat3::rotation_x(params.echo_x * std::f32::consts::PI)
        .then(&crate::Mat3::rotation_y(
            params.echo_y * std::f32::consts::PI,
        ))
        .then(&crate::Mat3::rotation_z(
            params.echo_z * std::f32::consts::PI,
        ));
    let offset = Vec3::new(0.0, 0.0, params.echo_offset * params.scale);
    for copy in 1..copies {
        for index in 0..base_count {
            let mut segment = segments[(copy - 1) * base_count + index];
            segment.start = rotation.transform(segment.start) * params.echo_scale + offset;
            segment.end = rotation.transform(segment.end) * params.echo_scale + offset;
            segment.echo = copy as u32;
            segments.push(segment);
        }
    }
    if params.flatten {
        for segment in &mut segments {
            segment.start.z = 0.0;
            segment.end.z = 0.0;
        }
    }
    let (center, radius) = bounds(&segments);
    match params.pattern.color_mode() {
        ColorMode::Depth => {}
        ColorMode::Path => {
            let count = segments.len().max(1) as f32;
            for (index, segment) in segments.iter_mut().enumerate() {
                segment.t = index as f32 / count;
            }
        }
        ColorMode::Radial => {
            let radius = radius.max(f32::EPSILON);
            for segment in &mut segments {
                let middle = segment.start.lerp(segment.end, 0.5);
                segment.t = ((middle - center).length() / radius).clamp(0.0, 1.0);
            }
        }
    }
    if params.pattern.color_mode() != ColorMode::Path {
        // Reveal order: trunk → tips, or centre → edge. Stable, so ties keep
        // their deterministic generation order.
        segments.sort_by(|a, b| a.t.total_cmp(&b.t));
    }
    let mut geometry = Geometry {
        segments,
        pattern: params.pattern,
        dimension: None,
        requested_depth,
        depth,
        truncated,
        center,
        radius,
    };
    geometry.dimension = Some(if geometry.is_planar() {
        Dimension::Planar
    } else {
        Dimension::Spatial
    });
    geometry
}

fn bounds(segments: &[Segment]) -> (Vec3, f32) {
    let Some(first) = segments.first() else {
        return (Vec3::ZERO, 1.0);
    };
    let (mut low, mut high) = (first.start, first.start);
    for segment in segments {
        low = low.min(segment.start).min(segment.end);
        high = high.max(segment.start).max(segment.end);
    }
    let center = low.lerp(high, 0.5);
    let radius = segments
        .iter()
        .map(|segment| {
            (segment.start - center)
                .length()
                .max((segment.end - center).length())
        })
        .fold(0.0_f32, f32::max);
    (center, radius.max(1e-3))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params(pattern: RecursivePattern) -> GeometryParams {
        GeometryParams::from_settings(
            &GeneratorSettings {
                pattern,
                ..GeneratorSettings::default()
            },
            AudioBands::default(),
            0.0,
        )
    }

    #[test]
    fn echoes_repeat_the_object_within_the_budget() {
        let base = generate(&params(RecursivePattern::Supershape));
        assert!(base.segments.iter().all(|segment| segment.echo == 0));
        let mut p = params(RecursivePattern::Supershape);
        p.echo_copies = 4.0;
        let echoed = generate(&p);
        assert!(echoed.segments.len() <= MAX_SEGMENTS);
        for copy in 0..4 {
            assert!(echoed.segments.iter().any(|segment| segment.echo == copy));
        }
        p.echo_copies = 24.0;
        p.budget = 1_000;
        assert!(generate(&p).segments.len() <= 1_000);
    }

    #[test]
    fn continuous_paths_carry_monotone_trace_distances() {
        let geometry = generate(&params(RecursivePattern::ChebyshevCurve));
        let mut traces: Vec<_> = geometry.segments.iter().map(|s| s.trace).collect();
        traces.sort_by(|a, b| a[0].total_cmp(&b[0]));
        assert_eq!(traces.first().unwrap()[0], 0.0);
        assert!((traces.last().unwrap()[1] - 1.0).abs() < 1e-4);
        for pair in traces.windows(2) {
            assert!((pair[0][1] - pair[1][0]).abs() < 1e-4);
        }
    }

    #[test]
    fn advanced_patterns_respond_to_their_controls() {
        for (pattern, tweak) in [
            (
                RecursivePattern::ChebyshevCurve,
                (|p: &mut GeometryParams| p.degree_x = 7.0) as fn(&mut GeometryParams),
            ),
            (RecursivePattern::PolynomialContours, |p| {
                p.surface_cross = 2.0
            }),
            (RecursivePattern::PolynomialContours, |p| p.contours = 8.0),
            (RecursivePattern::Supershape, |p| p.symmetry = 3.0),
            (RecursivePattern::Supershape, |p| p.exponent = 0.5),
        ] {
            let mut p = params(pattern);
            let before = generate(&p);
            tweak(&mut p);
            assert_ne!(before, generate(&p), "{pattern:?}");
        }
    }

    #[test]
    fn every_pattern_generates_finite_bounded_geometry() {
        for pattern in RecursivePattern::ALL {
            for depth in [0.0, 0.6, 1.0] {
                let mut p = params(pattern);
                p.levels = GeneratorSettings {
                    depth,
                    ..GeneratorSettings::default()
                }
                .depth_levels();
                let geometry = generate(&p);
                assert!(
                    !geometry.segments.is_empty(),
                    "{} at depth {depth} is empty",
                    pattern.label()
                );
                assert!(geometry.segments.len() <= MAX_SEGMENTS);
                assert!(!geometry.truncated, "{} truncated", pattern.label());
                for segment in &geometry.segments {
                    for v in [segment.start, segment.end] {
                        assert!(v.x.is_finite() && v.y.is_finite() && v.z.is_finite());
                    }
                    assert!((0.0..=1.0).contains(&segment.t));
                }
                assert!(geometry.radius.is_finite() && geometry.radius > 0.0);
            }
        }
    }

    #[test]
    fn planar_patterns_have_zero_z_and_flatten_forces_it() {
        for pattern in RecursivePattern::ALL {
            let geometry = generate(&params(pattern));
            if pattern.dimension() == Dimension::Planar {
                assert!(geometry.is_planar(), "{} left the plane", pattern.label());
                assert_eq!(geometry.dimension, Some(Dimension::Planar));
            }
            let mut flat = params(pattern);
            flat.flatten = true;
            assert!(generate(&flat).is_planar());
        }
        let tree = generate(&params(RecursivePattern::FractalTree));
        assert_eq!(tree.dimension, Some(Dimension::Spatial));
    }

    #[test]
    fn koch_count_is_three_times_four_to_the_depth_and_clamped() {
        let mut p = params(RecursivePattern::KochSnowflake);
        p.levels = 4;
        let geometry = generate(&p);
        assert_eq!(geometry.depth, 4);
        assert_eq!(geometry.segments.len(), 3 * 4usize.pow(4));
        p.levels = 11;
        let geometry = generate(&p);
        assert_eq!(geometry.requested_depth, 11);
        assert!(geometry.depth < 11, "depth 11 would be 12.5M segments");
        assert_eq!(geometry.segments.len(), 3 * 4usize.pow(geometry.depth));
    }

    #[test]
    fn koch_with_default_spread_closes_its_outline() {
        let mut p = params(RecursivePattern::KochSnowflake);
        p.levels = 3;
        let geometry = generate(&p);
        let first = geometry.segments.first().unwrap().start;
        let last = geometry.segments.last().unwrap().end;
        // Default spread 0.45 is exactly the classic 60° construction.
        assert!((first - last).length() < 1e-3);
    }

    #[test]
    fn hilbert_visits_every_cell_once_with_unit_steps() {
        let mut p = params(RecursivePattern::HilbertCurve);
        p.levels = 5; // order 3
        p.spread = 0.0; // no corner rounding
        let geometry = generate(&p);
        assert_eq!(geometry.depth, 3);
        assert_eq!(geometry.segments.len(), 64 - 1);
        let step = (geometry.segments[0].end - geometry.segments[0].start).length();
        for segment in &geometry.segments {
            assert!(((segment.end - segment.start).length() - step).abs() < 1e-4);
        }
    }

    #[test]
    fn seeded_randomness_is_repeatable() {
        for pattern in RecursivePattern::ALL {
            let mut p = params(pattern);
            p.randomness = 0.8;
            p.seed = 42;
            assert_eq!(generate(&p), generate(&p), "{}", pattern.label());
        }
        let mut a = params(RecursivePattern::FractalTree);
        a.randomness = 0.8;
        let mut b = a;
        b.seed = 43;
        assert_ne!(generate(&a), generate(&b));
    }

    #[test]
    fn heavy_settings_respect_a_small_budget() {
        for pattern in RecursivePattern::ALL {
            let mut p = params(pattern);
            p.levels = 11;
            p.spread = 1.0;
            p.budget = 5_000;
            let geometry = generate(&p);
            assert!(geometry.segments.len() <= 5_000, "{}", pattern.label());
        }
    }

    #[test]
    fn buffers_have_six_floats_per_segment_and_matching_endpoint_colours() {
        let settings = GeneratorSettings::default();
        let geometry = generate(&params(RecursivePattern::CrystalGrowth));
        let buffers = geometry.to_buffers(&Palette::from_settings(
            &settings,
            AudioBands::default(),
            0.0,
        ));
        assert_eq!(buffers.positions.len(), geometry.segments.len() * 6);
        assert_eq!(buffers.colors.len(), buffers.positions.len());
        for color in buffers.colors.chunks_exact(6) {
            assert_eq!(color[..3], color[3..]);
        }
    }
}
