//! Pattern catalogue and the operator-facing generator settings.

/// Every recursive pattern. Ids are persisted in project files; never
/// rename one, only add.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum RecursivePattern {
    #[default]
    FractalTree,
    RecursiveSpiral,
    Mandala,
    KochSnowflake,
    CrystalGrowth,
    RecursiveWeb,
    SierpinskiTriangle,
    DragonCurve,
    HilbertCurve,
    HTree,
    PythagorasTree,
    SierpinskiTetrahedron,
    MengerSponge,
    GeodesicSphere,
    Spirograph,
    TorusKnot,
    ChebyshevCurve,
    PolynomialContours,
    Supershape,
    RoseCurve,
    Lissajous,
    SphericalWeave,
    MobiusRibbon,
}

/// Whether a pattern is drawn in the XY plane or occupies 3D space.
/// Planar output always has `z = 0` unless audio displacement is enabled
/// (Recursive Web) or the operator rotates the camera.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Dimension {
    Planar,
    Spatial,
}

/// How each segment's colour/reveal coordinate `t` is assigned.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ColorMode {
    /// `1 − depth / max_depth`: trunk → tips. Reveal grows level by level.
    Depth,
    /// Fraction along a single continuous path. Reveal draws the curve.
    Path,
    /// Distance from the pattern centre. Reveal grows outward.
    Radial,
}

impl RecursivePattern {
    pub const ALL: [Self; 23] = [
        Self::FractalTree,
        Self::RecursiveSpiral,
        Self::Mandala,
        Self::KochSnowflake,
        Self::CrystalGrowth,
        Self::RecursiveWeb,
        Self::SierpinskiTriangle,
        Self::DragonCurve,
        Self::HilbertCurve,
        Self::HTree,
        Self::PythagorasTree,
        Self::SierpinskiTetrahedron,
        Self::MengerSponge,
        Self::GeodesicSphere,
        Self::Spirograph,
        Self::TorusKnot,
        Self::ChebyshevCurve,
        Self::PolynomialContours,
        Self::Supershape,
        Self::RoseCurve,
        Self::Lissajous,
        Self::SphericalWeave,
        Self::MobiusRibbon,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            Self::FractalTree => "fractal_tree",
            Self::RecursiveSpiral => "recursive_spiral",
            Self::Mandala => "mandala",
            Self::KochSnowflake => "koch_snowflake",
            Self::CrystalGrowth => "crystal_growth",
            Self::RecursiveWeb => "recursive_web",
            Self::SierpinskiTriangle => "sierpinski_triangle",
            Self::DragonCurve => "dragon_curve",
            Self::HilbertCurve => "hilbert_curve",
            Self::HTree => "h_tree",
            Self::PythagorasTree => "pythagoras_tree",
            Self::SierpinskiTetrahedron => "sierpinski_tetrahedron",
            Self::MengerSponge => "menger_sponge",
            Self::GeodesicSphere => "geodesic_sphere",
            Self::Spirograph => "spirograph",
            Self::ChebyshevCurve => "chebyshev_curve",
            Self::PolynomialContours => "polynomial_contours",
            Self::Supershape => "supershape",
            Self::TorusKnot => "torus_knot",
            Self::RoseCurve => "rose_curve",
            Self::Lissajous => "lissajous",
            Self::SphericalWeave => "spherical_weave",
            Self::MobiusRibbon => "mobius_ribbon",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|pattern| pattern.id() == id)
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::FractalTree => "Fractal Tree",
            Self::RecursiveSpiral => "Recursive Spiral",
            Self::Mandala => "Mandala",
            Self::KochSnowflake => "Koch Snowflake",
            Self::CrystalGrowth => "Crystal Growth",
            Self::RecursiveWeb => "Recursive Web",
            Self::SierpinskiTriangle => "Sierpinski Triangle",
            Self::DragonCurve => "Dragon Curve",
            Self::HilbertCurve => "Hilbert Curve",
            Self::HTree => "H-Tree",
            Self::PythagorasTree => "Pythagoras Tree",
            Self::SierpinskiTetrahedron => "Sierpinski Tetrahedron",
            Self::MengerSponge => "Menger Sponge",
            Self::GeodesicSphere => "Geodesic Sphere",
            Self::Spirograph => "Spirograph",
            Self::ChebyshevCurve => "Chebyshev Curve",
            Self::PolynomialContours => "Polynomial Contours",
            Self::Supershape => "Supershape",
            Self::TorusKnot => "Torus Knot",
            Self::RoseCurve => "Rose Curve",
            Self::Lissajous => "Lissajous",
            Self::SphericalWeave => "Spherical Weave",
            Self::MobiusRibbon => "Möbius Ribbon",
        }
    }

    /// One line describing what spread and twist do for this pattern.
    pub const fn hint(self) -> &'static str {
        match self {
            Self::FractalTree => "Spread: branch angle · Twist: branch roll",
            Self::RecursiveSpiral => "Spread: turn rate · Twist: roll, lifts the spiral into 3D",
            Self::Mandala => "Spread: arm count and branch angle · Twist: branch roll",
            Self::KochSnowflake => "Spread: bump angle (60° at default)",
            Self::CrystalGrowth => "Spread: side-branch angle · Twist: arm roll",
            Self::RecursiveWeb => "Spread: spoke count · Twist: ring swirl",
            Self::SierpinskiTriangle => "Spread: gap between cells · Twist: cell rotation",
            Self::DragonCurve => "Spread: fold angle (90° at default) · Twist: fold skew",
            Self::HilbertCurve => "Spread: corner rounding · Randomness: wobble",
            Self::HTree => "Spread: child length ratio · Twist: branch rotation",
            Self::PythagorasTree => "Spread: lean angle · Twist: alternates the lean",
            Self::SierpinskiTetrahedron => "Spread: gap between cells · Twist: cell rotation",
            Self::MengerSponge => "Spread: gap between cubes · Twist: cube rotation",
            Self::GeodesicSphere => "Spread: fractal spikes · Twist: spike phase",
            Self::Spirograph => "Depth: curve detail · Spread: lobes · Twist: loop size",
            Self::ChebyshevCurve | Self::PolynomialContours | Self::Supershape => {
                "Use Polynomial & surface controls; depth sets curve detail"
            }
            Self::TorusKnot => "Depth: curve detail · Spread: windings · Twist: tube radius",
            Self::RoseCurve => "Spread: petals · Twist: inner radius",
            Self::Lissajous => "Spread: frequency ratio · Twist: phase",
            Self::SphericalWeave => "Spread: windings · Twist: latitude sweep",
            Self::MobiusRibbon => "Spread: half twists · Twist: ribbon width",
        }
    }

    pub const fn dimension(self) -> Dimension {
        match self {
            Self::RoseCurve
            | Self::Lissajous
            | Self::Spirograph
            | Self::KochSnowflake
            | Self::RecursiveWeb
            | Self::SierpinskiTriangle
            | Self::DragonCurve
            | Self::HilbertCurve
            | Self::HTree
            | Self::PythagorasTree => Dimension::Planar,
            Self::SphericalWeave
            | Self::MobiusRibbon
            | Self::ChebyshevCurve
            | Self::PolynomialContours
            | Self::Supershape
            | Self::TorusKnot
            | Self::FractalTree
            | Self::RecursiveSpiral
            | Self::Mandala
            | Self::CrystalGrowth
            | Self::SierpinskiTetrahedron
            | Self::MengerSponge
            | Self::GeodesicSphere => Dimension::Spatial,
        }
    }

    pub const fn color_mode(self) -> ColorMode {
        match self {
            Self::RoseCurve | Self::Lissajous => ColorMode::Path,
            Self::SphericalWeave
            | Self::MobiusRibbon
            | Self::ChebyshevCurve
            | Self::PolynomialContours
            | Self::Supershape
            | Self::KochSnowflake
            | Self::DragonCurve
            | Self::HilbertCurve
            | Self::Spirograph
            | Self::TorusKnot => ColorMode::Path,
            Self::SierpinskiTriangle
            | Self::SierpinskiTetrahedron
            | Self::MengerSponge
            | Self::GeodesicSphere => ColorMode::Radial,
            Self::FractalTree
            | Self::RecursiveSpiral
            | Self::Mandala
            | Self::CrystalGrowth
            | Self::RecursiveWeb
            | Self::HTree
            | Self::PythagorasTree => ColorMode::Depth,
        }
    }

    /// Whether live audio changes this pattern's geometry (not only colour).
    pub const fn audio_shapes_geometry(self) -> bool {
        matches!(
            self,
            Self::FractalTree
                | Self::RecursiveSpiral
                | Self::Mandala
                | Self::KochSnowflake
                | Self::RecursiveWeb
                | Self::GeodesicSphere
        )
    }
}

/// Operator settings for one generator deck. Normalised controls are in
/// `[0, 1]` unless noted; [`GeneratorSettings::sanitized`] enforces ranges.
#[derive(Clone, Debug, PartialEq)]
pub struct GeneratorSettings {
    pub pattern: RecursivePattern,
    pub seed: u32,

    // Geometry.
    /// Maps to 3‥11 recursion levels; each pattern then offsets and clamps
    /// that to stay inside the segment budget.
    pub depth: f32,
    /// Maps to 0.4‥2.9 geometry units.
    pub scale: f32,
    pub spread: f32,
    pub twist: f32,
    /// Seeded angular / positional jitter.
    pub randomness: f32,
    /// Project 3D patterns onto the XY plane (explicit 2D mode).
    pub flatten: bool,

    // Camera and motion.
    /// −1‥1: turntable rotation, up to half a revolution per second.
    pub rotate_speed: f32,
    /// −1‥1: camera tilt, ±90°.
    pub tilt: f32,
    /// −1‥1: rotation about the view axis.
    pub spin_speed: f32,
    /// 0.25‥4.
    pub zoom: f32,
    pub perspective: f32,
    /// Fraction of segments drawn, in reveal order.
    pub reveal: f32,
    /// 0 holds `reveal`; above 0 the reveal loops, up to 0.5 cycles/second.
    pub grow_speed: f32,

    // Look.
    pub hue: f32,
    /// How far hue travels from the trunk / path start to the tips / end.
    pub hue_range: f32,
    /// Hue drift over time.
    pub color_speed: f32,
    pub saturation: f32,
    pub lightness: f32,
    /// 0‥4 additive intensity before tone mapping.
    pub brightness: f32,
    /// 0.5‥6 pixels at the generator's own resolution.
    pub line_width: f32,
    /// Dims segments farther from the camera.
    pub depth_fade: f32,
    /// 0 clears each frame; toward 1 previous frames persist as trails.
    pub trails: f32,
    /// Alpha follows line brightness so the deck layers over others.
    pub transparent: bool,
    /// Scales the live bass / mid / high bands into geometry and colour.
    pub audio_amount: f32,

    // Output.
    pub resolution: [u32; 2],
    pub fps: u32,
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
    pub echo_fade: f32,
    pub trace_heads: f32,
    pub trace_length: f32,
    pub trace_speed: f32,
    pub trace_spread: f32,
}

/// How a generator layer is combined with the layers below it.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum GeneratorBlendMode {
    #[default]
    Over,
    Add,
    Screen,
}

/// One independently editable procedural source on a deck.
#[derive(Clone, Debug, PartialEq)]
pub struct GeneratorLayer {
    /// Stable within this deck, suitable for persisted control assignments.
    pub id: u32,
    pub name: String,
    pub settings: GeneratorSettings,
    /// Normalized frame position, where [0, 0] is centered.
    pub position: [f32; 2],
    pub scale: f32,
    pub rotation: f32,
    pub opacity: f32,
    pub blend: GeneratorBlendMode,
    pub enabled: bool,
}

impl GeneratorLayer {
    /// Centered at full size with no rotation: pixels map one to one.
    pub fn is_untransformed(&self) -> bool {
        self.position == [0.0, 0.0] && self.scale == 1.0 && self.rotation == 0.0
    }

    pub fn new(id: u32, settings: GeneratorSettings) -> Self {
        Self {
            id,
            name: format!("Generator {id}"),
            settings,
            position: [0.0; 2],
            scale: 1.0,
            rotation: 0.0,
            opacity: 1.0,
            blend: GeneratorBlendMode::Over,
            enabled: true,
        }
    }
}

/// A bounded collection of independently controlled sources on one deck.
#[derive(Clone, Debug, PartialEq)]
pub struct GeneratorStack {
    pub layers: Vec<GeneratorLayer>,
    pub selected: usize,
    pub next_id: u32,
}

impl GeneratorStack {
    pub const MAX_LAYERS: usize = 4;
    pub fn new(settings: GeneratorSettings) -> Self {
        Self {
            layers: vec![GeneratorLayer::new(1, settings)],
            selected: 0,
            next_id: 2,
        }
    }
    pub fn selected_layer(&self) -> Option<&GeneratorLayer> {
        self.layers.get(self.selected)
    }
    pub fn selected_layer_mut(&mut self) -> Option<&mut GeneratorLayer> {
        self.layers.get_mut(self.selected)
    }
    pub fn add_layer(&mut self) -> bool {
        if self.layers.len() >= Self::MAX_LAYERS {
            return false;
        }
        let mut settings = self
            .layers
            .get(self.selected)
            .map(|l| l.settings.clone())
            .unwrap_or_default();
        settings.seed = settings.seed.wrapping_add(self.next_id);
        settings.transparent = true;
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1).max(1);
        self.layers.push(GeneratorLayer::new(id, settings));
        self.selected = self.layers.len() - 1;
        true
    }
    pub fn remove_selected(&mut self) -> bool {
        if self.layers.len() <= 1 {
            return false;
        }
        self.layers.remove(self.selected);
        self.selected = self.selected.min(self.layers.len() - 1);
        true
    }
    /// Deck output has one shared pixel size and frame clock across layers.
    pub fn normalize_output(&mut self) {
        if let Some(first) = self.layers.first() {
            let resolution = first.settings.resolution;
            let fps = first.settings.fps;
            for layer in &mut self.layers {
                layer.settings.resolution = resolution;
                layer.settings.fps = fps;
            }
        }
    }
}

impl From<GeneratorSettings> for GeneratorStack {
    fn from(settings: GeneratorSettings) -> Self {
        Self::new(settings)
    }
}

impl std::ops::Deref for GeneratorStack {
    type Target = GeneratorSettings;
    fn deref(&self) -> &Self::Target {
        &self.layers[self.selected.min(self.layers.len() - 1)].settings
    }
}
impl std::ops::DerefMut for GeneratorStack {
    fn deref_mut(&mut self) -> &mut Self::Target {
        let i = self.selected.min(self.layers.len() - 1);
        &mut self.layers[i].settings
    }
}

impl Default for GeneratorSettings {
    fn default() -> Self {
        Self {
            pattern: RecursivePattern::FractalTree,
            seed: 1,
            depth: 0.60,
            scale: 0.50,
            spread: 0.45,
            twist: 0.35,
            randomness: 0.0,
            flatten: false,
            rotate_speed: 0.15,
            tilt: 0.0,
            spin_speed: 0.0,
            zoom: 1.0,
            perspective: 0.5,
            reveal: 1.0,
            grow_speed: 0.0,
            hue: 0.55,
            hue_range: 0.45,
            color_speed: 0.5,
            saturation: 0.65,
            lightness: 0.45,
            brightness: 1.6,
            line_width: 1.25,
            depth_fade: 0.35,
            trails: 0.0,
            transparent: true,
            audio_amount: 0.0,
            resolution: [1280, 720],
            fps: 60,
            degree_x: 3.0,
            degree_y: 4.0,
            degree_z: 5.0,
            polynomial_mix: 0.25,
            surface_order: 4.0,
            surface_cross: 0.0,
            contours: 24.0,
            slice_axis: 2.0,
            symmetry: 6.0,
            exponent: 1.0,
            echo_copies: 1.0,
            echo_scale: 0.88,
            echo_x: 0.08,
            echo_y: 0.12,
            echo_z: 0.04,
            echo_offset: 0.0,
            echo_fade: 0.85,
            trace_heads: 1.0,
            trace_length: 1.0,
            trace_speed: 0.0,
            trace_spread: 0.0,
        }
    }
}

pub const RESOLUTIONS: [(&str, [u32; 2]); 6] = [
    ("540p", [960, 540]),
    ("720p", [1280, 720]),
    ("1080p", [1920, 1080]),
    ("Square 1080", [1080, 1080]),
    ("Portrait 720", [720, 1280]),
    ("Portrait 1080", [1080, 1920]),
];

pub const FRAME_RATES: [u32; 4] = [24, 30, 50, 60];

impl GeneratorSettings {
    pub fn sanitized(mut self) -> Self {
        let unit = |value: f32, fallback: f32| finite_or(value, fallback).clamp(0.0, 1.0);
        let signed = |value: f32| finite_or(value, 0.0).clamp(-1.0, 1.0);
        let defaults = Self::default();
        self.degree_x = finite_or(self.degree_x, defaults.degree_x).clamp(1.0, 12.0);
        self.degree_y = finite_or(self.degree_y, defaults.degree_y).clamp(1.0, 12.0);
        self.degree_z = finite_or(self.degree_z, defaults.degree_z).clamp(1.0, 12.0);
        self.polynomial_mix =
            finite_or(self.polynomial_mix, defaults.polynomial_mix).clamp(0.0, 1.0);
        self.surface_order = finite_or(self.surface_order, defaults.surface_order).clamp(2.0, 12.0);
        self.surface_cross = finite_or(self.surface_cross, defaults.surface_cross).clamp(0.0, 4.0);
        self.contours = finite_or(self.contours, defaults.contours).clamp(4.0, 64.0);
        self.slice_axis = finite_or(self.slice_axis, defaults.slice_axis).clamp(0.0, 2.0);
        self.symmetry = finite_or(self.symmetry, defaults.symmetry).clamp(2.0, 12.0);
        self.exponent = finite_or(self.exponent, defaults.exponent).clamp(0.25, 4.0);
        self.echo_copies = finite_or(self.echo_copies, defaults.echo_copies).clamp(1.0, 24.0);
        self.echo_scale = finite_or(self.echo_scale, defaults.echo_scale).clamp(0.5, 1.0);
        self.echo_x = finite_or(self.echo_x, defaults.echo_x).clamp(-1.0, 1.0);
        self.echo_y = finite_or(self.echo_y, defaults.echo_y).clamp(-1.0, 1.0);
        self.echo_z = finite_or(self.echo_z, defaults.echo_z).clamp(-1.0, 1.0);
        self.echo_offset = finite_or(self.echo_offset, defaults.echo_offset).clamp(-0.25, 0.25);
        self.echo_fade = finite_or(self.echo_fade, defaults.echo_fade).clamp(0.1, 1.0);
        self.trace_heads = finite_or(self.trace_heads, defaults.trace_heads).clamp(1.0, 8.0);
        self.trace_length = finite_or(self.trace_length, defaults.trace_length).clamp(0.0, 1.0);
        self.trace_speed = finite_or(self.trace_speed, defaults.trace_speed).clamp(-1.0, 1.0);
        self.trace_spread = finite_or(self.trace_spread, defaults.trace_spread).clamp(0.0, 1.0);

        self.depth = unit(self.depth, defaults.depth);
        self.scale = unit(self.scale, defaults.scale);
        self.spread = unit(self.spread, defaults.spread);
        self.twist = unit(self.twist, defaults.twist);
        self.randomness = unit(self.randomness, 0.0);
        self.rotate_speed = signed(self.rotate_speed);
        self.tilt = signed(self.tilt);
        self.spin_speed = signed(self.spin_speed);
        self.zoom = finite_or(self.zoom, 1.0).clamp(0.25, 4.0);
        self.perspective = unit(self.perspective, defaults.perspective);
        self.reveal = unit(self.reveal, 1.0);
        self.grow_speed = unit(self.grow_speed, 0.0);
        self.hue = finite_or(self.hue, defaults.hue).rem_euclid(1.0);
        self.hue_range = unit(self.hue_range, defaults.hue_range);
        self.color_speed = unit(self.color_speed, defaults.color_speed);
        self.saturation = unit(self.saturation, defaults.saturation);
        self.lightness = unit(self.lightness, defaults.lightness);
        self.brightness = finite_or(self.brightness, defaults.brightness).clamp(0.0, 4.0);
        self.line_width = finite_or(self.line_width, defaults.line_width).clamp(0.5, 6.0);
        self.depth_fade = unit(self.depth_fade, defaults.depth_fade);
        self.trails = finite_or(self.trails, 0.0).clamp(0.0, 0.97);
        self.audio_amount = unit(self.audio_amount, 0.0);
        self.resolution = [
            self.resolution[0].clamp(64, 3840),
            self.resolution[1].clamp(64, 2160),
        ];
        self.fps = self.fps.clamp(1, 120);
        self
    }

    /// Recursion levels before pattern-specific offsets: 3 at 0, 7 at the
    /// 0.60 default, 11 at 1.
    pub fn depth_levels(&self) -> u32 {
        (3.0 + self.depth.clamp(0.0, 1.0) * 8.0).floor() as u32
    }

    /// Geometry scale in pattern units: 0.4 at 0, 2.9 at 1.
    pub fn scale_units(&self) -> f32 {
        0.4 + self.scale.clamp(0.0, 1.0) * 2.5
    }
}

fn finite_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() { value } else { fallback }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_round_trip_and_are_unique() {
        for pattern in RecursivePattern::ALL {
            assert_eq!(RecursivePattern::from_id(pattern.id()), Some(pattern));
        }
        let mut ids: Vec<_> = RecursivePattern::ALL.iter().map(|p| p.id()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), RecursivePattern::ALL.len());
        assert_eq!(RecursivePattern::from_id("nope"), None);
    }

    #[test]
    fn depth_and_scale_mappings() {
        let mut settings = GeneratorSettings {
            depth: 0.0,
            scale: 0.0,
            ..GeneratorSettings::default()
        };
        assert_eq!(settings.depth_levels(), 3);
        assert!((settings.scale_units() - 0.4).abs() < 1e-6);
        settings.depth = 0.6;
        assert_eq!(settings.depth_levels(), 7);
        settings.depth = 1.0;
        settings.scale = 1.0;
        assert_eq!(settings.depth_levels(), 11);
        assert!((settings.scale_units() - 2.9).abs() < 1e-6);
    }

    #[test]
    fn sanitize_clamps_and_replaces_non_finite_values() {
        let settings = GeneratorSettings {
            depth: f32::NAN,
            zoom: 100.0,
            trails: 1.0,
            hue: -0.25,
            resolution: [0, 99_999],
            fps: 0,
            ..GeneratorSettings::default()
        }
        .sanitized();
        assert_eq!(settings.depth, GeneratorSettings::default().depth);
        assert_eq!(settings.zoom, 4.0);
        assert!(settings.trails < 1.0);
        assert!((settings.hue - 0.75).abs() < 1e-6);
        assert_eq!(settings.resolution, [64, 2160]);
        assert_eq!(settings.fps, 1);
    }
}
