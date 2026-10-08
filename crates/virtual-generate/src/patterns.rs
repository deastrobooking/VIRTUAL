//! The recursive pattern algorithms, their depth mappings and segment-count
//! estimates.
//!
//! Every pattern recursion checks `Builder::full` on entry, so even an
//! underestimated pattern stops doing work once the budget is spent.

use std::f32::consts::{FRAC_PI_2, PI, TAU};

use crate::geometry::GeometryParams;
use crate::math::{Rng, Vec3, position_noise};
use crate::settings::RecursivePattern;
use crate::turtle::{Builder, Turtle};

/// Steps drawn by one Recursive Spiral arm before it ends.
const SPIRAL_STEPS: u32 = 24;

/// Maps the shared 3‥11 levels control onto each pattern's useful range.
pub(crate) fn requested_depth(pattern: RecursivePattern, levels: u32) -> u32 {
    use RecursivePattern::*;
    match pattern {
        FractalTree | Mandala | KochSnowflake | CrystalGrowth | RecursiveWeb => levels,
        RecursiveSpiral | HilbertCurve | SierpinskiTetrahedron => levels.saturating_sub(2),
        SierpinskiTriangle => levels.saturating_sub(1),
        DragonCurve => levels + 5,
        HTree => levels + 2,
        PythagorasTree => levels + 1,
        MengerSponge => levels.saturating_sub(3) / 2,
        GeodesicSphere => levels.saturating_sub(4),
        ChebyshevCurve | PolynomialContours | Supershape | RoseCurve | Lissajous
        | SphericalWeave | MobiusRibbon | Spirograph | TorusKnot => levels,
    }
}

/// The largest depth ≤ `requested` whose estimate fits `budget`. A shared
/// depth maximum is not a safe work budget: Koch grows as 3·4ⁿ, Menger as
/// 12·20ⁿ and Mandala repeats a whole tree per arm.
pub(crate) fn effective_depth(params: &GeometryParams, requested: u32, budget: u64) -> u32 {
    let mut depth = requested;
    while depth > 0 && estimate(params, depth) > budget {
        depth -= 1;
    }
    depth
}

/// Upper-bound segment count, ignoring length cut-offs.
pub(crate) fn estimate(params: &GeometryParams, depth: u32) -> u64 {
    use RecursivePattern::*;
    let pow = |base: u64, exponent: u32| base.saturating_pow(exponent);
    match params.pattern {
        FractalTree => tree_estimate(depth),
        Mandala => u64::from(mandala_arms(params.spread)).saturating_mul(tree_estimate(depth)),
        RecursiveSpiral => (1..=depth).fold(0u64, |count, _| {
            u64::from(SPIRAL_STEPS).saturating_add(count.saturating_mul(3))
        }),
        KochSnowflake => pow(4, depth).saturating_mul(3),
        CrystalGrowth => {
            (1..=depth).fold(0u64, |count, _| count.saturating_mul(3).saturating_add(1)) * 6
        }
        RecursiveWeb => u64::from(depth.max(1)) * u64::from(web_spokes(params.spread)) * 2,
        SierpinskiTriangle => pow(3, depth).saturating_mul(3),
        DragonCurve => pow(2, depth),
        HilbertCurve => pow(4, depth).saturating_mul(2),
        HTree => pow(2, depth).saturating_sub(1),
        PythagorasTree => pow(2, depth.saturating_add(1)).saturating_mul(4),
        SierpinskiTetrahedron => pow(4, depth).saturating_mul(6),
        MengerSponge => pow(20, depth).saturating_mul(12),
        GeodesicSphere => pow(4, depth).saturating_mul(60),
        ChebyshevCurve => 256 * u64::from(depth.max(1)),
        PolynomialContours | Supershape => {
            params.contours.round().clamp(4.0, 64.0) as u64 * 2 * 64 * u64::from(depth.max(1))
        }
        RoseCurve | Lissajous | SphericalWeave | MobiusRibbon | Spirograph | TorusKnot => {
            256 * u64::from(depth.max(1))
        }
    }
}

fn tree_estimate(depth: u32) -> u64 {
    (1..=depth).fold(0u64, |child, level| {
        let children = if level > 2 { 3 } else { 2 };
        child.saturating_mul(children).saturating_add(1)
    })
}

fn mandala_arms(spread: f32) -> u32 {
    8 + (spread * 24.0).floor() as u32
}

fn web_spokes(spread: f32) -> u32 {
    8 + (spread * 32.0).floor() as u32
}

struct Ctx<'a> {
    p: &'a GeometryParams,
    rng: Rng,
    turtle: Turtle,
    out: &'a mut Builder,
    max_depth: u32,
}

impl Ctx<'_> {
    fn level_t(&self, depth: u32) -> f32 {
        1.0 - depth as f32 / self.max_depth.max(1) as f32
    }

    fn jitter(&mut self) -> f32 {
        self.rng.jitter(self.p.randomness)
    }

    fn jitter_vec(&mut self, amount: f32, planar: bool) -> Vec3 {
        let amount = amount * self.p.randomness;
        if amount == 0.0 {
            return Vec3::ZERO;
        }
        let z = if planar { 0.0 } else { self.rng.jitter(amount) };
        Vec3::new(self.rng.jitter(amount), self.rng.jitter(amount), z)
    }
}

pub(crate) fn build(p: &GeometryParams, depth: u32, out: &mut Builder) {
    let mut c = Ctx {
        p,
        rng: Rng::new(p.seed),
        turtle: Turtle::new(),
        out,
        max_depth: depth.max(1),
    };
    let scale = p.scale;
    use RecursivePattern::*;
    match p.pattern {
        FractalTree => tree(&mut c, depth, scale * 1.2),
        RecursiveSpiral => spiral(&mut c, depth, scale * 0.35),
        Mandala => {
            let arms = mandala_arms(p.spread);
            for arm in 0..arms {
                if c.out.full() {
                    break;
                }
                c.turtle.reset();
                c.turtle.yaw(arm as f32 / arms as f32 * TAU);
                tree(&mut c, depth, scale * 1.1);
            }
        }
        KochSnowflake => {
            let angle = PI / 3.0 * (1.0 + (p.spread - 0.45)) + p.audio.high * 0.35;
            for _ in 0..3 {
                koch(&mut c, scale * 2.2, depth, angle);
                c.turtle.yaw(-TAU / 3.0);
            }
        }
        CrystalGrowth => {
            for arm in 0..6 {
                c.turtle.reset();
                c.turtle.yaw(arm as f32 / 6.0 * TAU);
                c.turtle.roll((p.twist - 0.35) * PI);
                crystal(&mut c, depth, scale * 1.25);
            }
        }
        RecursiveWeb => web(&mut c, depth.max(1)),
        SierpinskiTriangle => {
            let r = scale * 1.6;
            let vertex = |k: f32| {
                let angle = FRAC_PI_2 + k * TAU / 3.0;
                Vec3::new(r * angle.cos(), r * angle.sin(), 0.0)
            };
            sierpinski(&mut c, [vertex(0.0), vertex(1.0), vertex(2.0)], depth);
        }
        DragonCurve => dragon(&mut c, depth),
        HilbertCurve => hilbert(&mut c, depth.max(1)),
        HTree => h_tree(&mut c, Vec3::ZERO, scale * 1.6, 0.0, depth),
        PythagorasTree => {
            let size = scale * 0.55;
            pythagoras(&mut c, Vec3::new(-size * 0.5, 0.0, 0.0), 0.0, size, depth);
        }
        SierpinskiTetrahedron => {
            let r = scale * 1.4;
            let v = |x: f32, y: f32, z: f32| Vec3::new(x, y, z).normalized() * r;
            let vertices = [
                v(1.0, 1.0, 1.0),
                v(1.0, -1.0, -1.0),
                v(-1.0, 1.0, -1.0),
                v(-1.0, -1.0, 1.0),
            ];
            tetrahedron(&mut c, vertices, depth);
        }
        MengerSponge => menger(&mut c, Vec3::ZERO, scale, depth),
        GeodesicSphere => geodesic(&mut c, depth),
        ChebyshevCurve | PolynomialContours | Supershape => advanced_curves(&mut c, depth),
        RoseCurve | Lissajous | SphericalWeave | MobiusRibbon | Spirograph | TorusKnot => {
            parametric_curve(&mut c, depth)
        }
    }
}

/// Closed periodic curves. Reuse the first point at the seam so seeded
/// displacement stays continuous as well as deterministic.
fn parametric_curve(c: &mut Ctx, depth: u32) {
    let steps = 256 * depth.max(1);
    let lobes = 2.0 + (c.p.spread * 10.0).floor();
    let point = |c: &mut Ctx, index: u32| {
        let t = index as f32 / steps as f32 * TAU;
        let v = if c.p.pattern == RecursivePattern::RoseCurve {
            let radius = 0.15 + c.p.twist * 0.5 + 0.8 * (lobes * t).cos();
            Vec3::new(radius * t.cos(), radius * t.sin(), 0.0)
        } else if c.p.pattern == RecursivePattern::Lissajous {
            Vec3::new(
                (lobes * t + c.p.twist * PI).sin(),
                ((lobes + 1.0) * t).sin(),
                0.0,
            )
        } else if c.p.pattern == RecursivePattern::SphericalWeave {
            let latitude = (0.2 + c.p.twist * 1.2) * (lobes * t).sin();
            Vec3::new(
                latitude.cos() * t.cos(),
                latitude.cos() * t.sin(),
                latitude.sin(),
            )
        } else if c.p.pattern == RecursivePattern::MobiusRibbon {
            // A closed thread scans back and forth across a twisted ribbon.
            // Two revolutions close the seam even for an odd half-twist count.
            let angle = t * 2.0;
            let half_twists = 1.0 + 2.0 * (c.p.spread * 4.0).floor();
            let across = (0.1 + c.p.twist * 0.5) * (24.0 * t).sin();
            let roll = half_twists * angle * 0.5;
            let radius = 1.0 + across * roll.cos();
            Vec3::new(
                radius * angle.cos(),
                radius * angle.sin(),
                across * roll.sin(),
            )
        } else if c.p.pattern == RecursivePattern::Spirograph {
            let loop_size = 0.15 + c.p.twist * 0.85;
            Vec3::new(
                t.cos() + loop_size * (lobes * t).cos(),
                t.sin() - loop_size * (lobes * t).sin(),
                0.0,
            )
        } else {
            // Consecutive winding numbers are coprime: one continuous knot.
            let radius = 1.0 + (0.15 + c.p.twist * 0.65) * ((lobes + 1.0) * t).cos();
            Vec3::new(
                radius * (lobes * t).cos(),
                radius * (lobes * t).sin(),
                (0.15 + c.p.twist * 0.65) * ((lobes + 1.0) * t).sin(),
            )
        };
        v * c.p.scale
            + c.jitter_vec(
                c.p.scale * 0.04,
                c.p.pattern.dimension() == crate::settings::Dimension::Planar,
            )
    };
    let first = point(c, 0);
    let mut previous = first;
    for index in 1..=steps {
        let next = if index == steps {
            first
        } else {
            point(c, index)
        };
        if !c.out.line(previous, next, 0.0) {
            break;
        }
        previous = next;
    }
}

fn tree(c: &mut Ctx, depth: u32, length: f32) {
    if depth == 0 || length < 0.01 || c.out.full() {
        return;
    }
    let t = c.level_t(depth);
    c.turtle.forward(length, t, c.out);
    let audio = c.p.audio;
    let angle = (15.0 + c.p.spread * 55.0 + audio.mid * 20.0).to_radians();
    let roll = (c.p.twist * 180.0 + audio.high * 35.0).to_radians();
    let multiplier = 0.62 + audio.bass * 0.08;
    for sign in [1.0, -1.0] {
        c.turtle.push();
        let jitter = c.jitter();
        c.turtle.yaw(sign * angle + jitter);
        c.turtle.roll(sign * roll);
        tree(c, depth - 1, length * multiplier);
        c.turtle.pop();
    }
    if depth > 2 {
        c.turtle.push();
        c.turtle.pitch(0.7 * angle);
        c.turtle.roll(0.5 * roll);
        tree(c, depth - 1, length * 0.85 * multiplier);
        c.turtle.pop();
    }
}

/// A tumbling 3D spiral whose arms sprout three smaller spirals each.
fn spiral(c: &mut Ctx, depth: u32, length: f32) {
    if depth == 0 || length < 0.01 || c.out.full() {
        return;
    }
    let t = c.level_t(depth);
    let audio = c.p.audio;
    let yaw = (6.0 + c.p.spread * 45.0 + audio.mid * 15.0).to_radians();
    let pitch = 0.35 * yaw;
    let roll = (c.p.twist * 120.0).to_radians() / 4.0;
    let mut step_length = length;
    for step in 0..SPIRAL_STEPS {
        if c.out.full() {
            return;
        }
        c.turtle.forward(step_length, t, c.out);
        if depth > 1 && step % 8 == 4 {
            c.turtle.push();
            let jitter = c.jitter();
            c.turtle.yaw(FRAC_PI_2 + jitter);
            spiral(c, depth - 1, step_length * 0.6);
            c.turtle.pop();
        }
        let jitter = c.jitter();
        c.turtle.yaw(yaw + jitter);
        c.turtle.pitch(pitch);
        c.turtle.roll(roll);
        step_length *= 0.94;
    }
}

fn koch(c: &mut Ctx, length: f32, depth: u32, angle: f32) {
    if c.out.full() {
        return;
    }
    if depth == 0 {
        c.turtle.forward(length, 0.0, c.out);
        return;
    }
    let child = length / 3.0;
    koch(c, child, depth - 1, angle);
    c.turtle.yaw(angle);
    koch(c, child, depth - 1, angle);
    c.turtle.yaw(-2.0 * angle);
    koch(c, child, depth - 1, angle);
    c.turtle.yaw(angle);
    koch(c, child, depth - 1, angle);
}

fn crystal(c: &mut Ctx, depth: u32, length: f32) {
    if depth == 0 || length < 0.02 || c.out.full() {
        return;
    }
    let t = c.level_t(depth);
    c.turtle.forward(length, t, c.out);
    let angle = (30.0 + c.p.spread * 45.0).to_radians();
    for sign in [1.0, -1.0] {
        c.turtle.push();
        let jitter = c.jitter();
        c.turtle.yaw(sign * angle + jitter);
        c.turtle.pitch(sign * 0.4 * angle);
        crystal(c, depth - 1, length * 0.55);
        c.turtle.pop();
    }
    crystal(c, depth - 1, length * 0.72);
}

/// Concentric polygon rings joined ring-to-ring. Audio adds radial wobble
/// and Z displacement; twist swirls each ring against the previous one.
fn web(c: &mut Ctx, rings: u32) {
    let p = c.p;
    let spokes = web_spokes(p.spread);
    let max_radius = p.scale * 2.0;
    let swirl = (p.twist - 0.35) * 2.0 * TAU / spokes as f32;
    let ring_gap = max_radius / rings as f32;
    let mut previous: Vec<Vec3> = vec![Vec3::ZERO; spokes as usize];
    let mut current = Vec::with_capacity(spokes as usize);
    for ring in 1..=rings {
        current.clear();
        let wobble = (p.time * 2.0 + ring as f32).sin() * p.audio.mid * 0.15;
        for spoke in 0..spokes {
            let angle = spoke as f32 / spokes as f32 * TAU + ring as f32 * swirl;
            let radius = ring as f32 * ring_gap + wobble + c.rng.jitter(p.randomness * ring_gap);
            let z = (angle * 3.0 + p.time).sin() * p.audio.high * 0.2;
            current.push(Vec3::new(radius * angle.cos(), radius * angle.sin(), z));
        }
        let t = ring as f32 / rings as f32;
        let radial_t = (ring as f32 - 0.5) / rings as f32;
        for index in 0..current.len() {
            let next = current[(index + 1) % current.len()];
            if !c.out.line(current[index], next, t)
                || !c.out.line(previous[index], current[index], radial_t)
            {
                return;
            }
        }
        std::mem::swap(&mut previous, &mut current);
    }
}

fn rotate_z(v: Vec3, angle: f32) -> Vec3 {
    v.rotated(Vec3::Z, angle)
}

fn sierpinski(c: &mut Ctx, [a, b, d]: [Vec3; 3], depth: u32) {
    if c.out.full() {
        return;
    }
    if depth == 0 {
        c.out.line(a, b, 0.0);
        c.out.line(b, d, 0.0);
        c.out.line(d, a, 0.0);
        return;
    }
    let edge = (b - a).length();
    let ab = a.lerp(b, 0.5) + c.jitter_vec(edge * 0.15, true);
    let bd = b.lerp(d, 0.5) + c.jitter_vec(edge * 0.15, true);
    let da = d.lerp(a, 0.5) + c.jitter_vec(edge * 0.15, true);
    let shrink = 1.0 - c.p.spread * 0.2;
    let turn = (c.p.twist - 0.35) * PI / 3.0;
    for child in [[a, ab, da], [ab, b, bd], [da, bd, d]] {
        let center = (child[0] + child[1] + child[2]) * (1.0 / 3.0);
        let place = |v: Vec3| center + rotate_z(v - center, turn) * shrink;
        sierpinski(c, child.map(place), depth - 1);
    }
}

/// Heighway dragon: 2ⁿ unit folds. Right/left comes from the bit pattern of
/// the step index, so no recursion stack is needed.
fn dragon(c: &mut Ctx, depth: u32) {
    let steps = 1u64 << depth.min(30);
    let length = c.p.scale * 2.0 / std::f32::consts::SQRT_2.powi(depth as i32);
    let turn = FRAC_PI_2 + (c.p.spread - 0.45) * PI / 3.0;
    let skew = (c.p.twist - 0.35) * 0.5;
    for step in 1..=steps {
        if c.out.full() {
            return;
        }
        c.turtle.forward(length, 0.0, c.out);
        if step == steps {
            break;
        }
        let right = ((step & step.wrapping_neg()) << 1) & step != 0;
        let jitter = c.jitter();
        c.turtle
            .yaw(if right { -turn } else { turn + skew } + jitter);
    }
}

/// Hilbert index → cell coordinate on an `n × n` grid (n a power of two).
fn hilbert_point(n: u32, index: u32) -> (u32, u32) {
    let (mut x, mut y) = (0, 0);
    let mut t = index;
    let mut s = 1;
    while s < n {
        let rx = 1 & (t / 2);
        let ry = 1 & (t ^ rx);
        if ry == 0 {
            if rx == 1 {
                x = s - 1 - x;
                y = s - 1 - y;
            }
            std::mem::swap(&mut x, &mut y);
        }
        x += s * rx;
        y += s * ry;
        t /= 4;
        s *= 2;
    }
    (x, y)
}

fn hilbert(c: &mut Ctx, order: u32) {
    let order = order.min(12);
    let n = 1u32 << order;
    let size = c.p.scale * 2.2;
    let cell = size / n as f32;
    let origin = -size * 0.5 + cell * 0.5;
    let rounding = c.p.spread * 0.4;
    let point = |c: &mut Ctx, index: u32| {
        let (x, y) = hilbert_point(n, index);
        Vec3::new(origin + x as f32 * cell, origin + y as f32 * cell, 0.0)
            + c.jitter_vec(cell * 0.7, true)
    };
    let mut a = point(c, 0);
    let mut b = point(c, 1);
    let mut previous_end: Option<Vec3> = None;
    for index in 1..n * n {
        if c.out.full() {
            return;
        }
        let start = a.lerp(b, rounding);
        let end = b.lerp(a, rounding);
        if let Some(corner) = previous_end
            && rounding > 0.0
        {
            c.out.line(corner, start, 0.0);
        }
        c.out.line(
            if rounding > 0.0 { start } else { a },
            if rounding > 0.0 { end } else { b },
            0.0,
        );
        previous_end = Some(end);
        if index + 1 < n * n {
            a = b;
            b = point(c, index + 1);
        }
    }
}

fn h_tree(c: &mut Ctx, center: Vec3, length: f32, angle: f32, depth: u32) {
    if depth == 0 || c.out.full() {
        return;
    }
    let t = c.level_t(depth);
    let direction = Vec3::new(angle.cos(), angle.sin(), 0.0) * (length * 0.5);
    let (a, b) = (center - direction, center + direction);
    c.out.line(a, b, t);
    let ratio = 0.5 + c.p.spread * 0.3;
    let child_angle = angle + FRAC_PI_2 + (c.p.twist - 0.35) * FRAC_PI_2;
    for end in [a, b] {
        let jitter = c.jitter();
        h_tree(c, end, length * ratio, child_angle + jitter, depth - 1);
    }
}

fn pythagoras(c: &mut Ctx, base: Vec3, angle: f32, size: f32, depth: u32) {
    if c.out.full() || size < 1e-3 {
        return;
    }
    let t = c.level_t(depth);
    let u = Vec3::new(angle.cos(), angle.sin(), 0.0);
    let v = Vec3::new(-angle.sin(), angle.cos(), 0.0);
    let corners = [
        base,
        base + u * size,
        base + u * size + v * size,
        base + v * size,
    ];
    for index in 0..4 {
        c.out.line(corners[index], corners[(index + 1) % 4], t);
    }
    if depth == 0 {
        return;
    }
    let mut lean = PI / 4.0 + (c.p.spread - 0.45) * PI / 3.0;
    if depth % 2 == 1 {
        let alternate = ((c.p.twist - 0.35) / 0.65).clamp(0.0, 1.0);
        lean += (FRAC_PI_2 - 2.0 * lean) * alternate;
    }
    let lean = (lean + c.jitter()).clamp(0.1, FRAC_PI_2 - 0.1);
    let top_left = corners[3];
    let left_size = size * lean.cos();
    let apex = top_left + u.rotated(Vec3::Z, lean) * left_size;
    pythagoras(c, top_left, angle + lean, left_size, depth - 1);
    pythagoras(
        c,
        apex,
        angle + lean - FRAC_PI_2,
        size * lean.sin(),
        depth - 1,
    );
}

fn tetrahedron(c: &mut Ctx, v: [Vec3; 4], depth: u32) {
    if c.out.full() {
        return;
    }
    if depth == 0 {
        for i in 0..4 {
            for j in i + 1..4 {
                c.out.line(v[i], v[j], 0.0);
            }
        }
        return;
    }
    let edge = (v[1] - v[0]).length();
    let shrink = 1.0 - c.p.spread * 0.2;
    let turn = (c.p.twist - 0.35) * PI / 3.0;
    for corner in 0..4 {
        let child = std::array::from_fn::<_, 4, _>(|m| v[corner].lerp(v[m], 0.5));
        let center = (child[0] + child[1] + child[2] + child[3]) * 0.25;
        let axis = center.normalized();
        let offset = c.jitter_vec(edge * 0.15, false);
        let place = |p: Vec3| center + offset + (p - center).rotated(axis, turn) * shrink;
        tetrahedron(c, child.map(place), depth - 1);
    }
}

fn menger(c: &mut Ctx, center: Vec3, half: f32, depth: u32) {
    if c.out.full() {
        return;
    }
    if depth == 0 {
        let half = half * (1.0 - c.p.spread * 0.3);
        let turn = (c.p.twist - 0.35) * FRAC_PI_2;
        let offset = c.jitter_vec(half * 0.6, false);
        let corner = |i: u32| {
            let local = Vec3::new(
                if i & 1 == 0 { -half } else { half },
                if i & 2 == 0 { -half } else { half },
                if i & 4 == 0 { -half } else { half },
            );
            center + offset + local.rotated(Vec3::Y, turn)
        };
        for i in 0..8u32 {
            for bit in [1u32, 2, 4] {
                if i & bit == 0 {
                    c.out.line(corner(i), corner(i | bit), 0.0);
                }
            }
        }
        return;
    }
    let child = half / 3.0;
    for x in -1i32..=1 {
        for y in -1i32..=1 {
            for z in -1i32..=1 {
                if (x == 0) as u8 + (y == 0) as u8 + (z == 0) as u8 >= 2 {
                    continue;
                }
                let offset = Vec3::new(x as f32, y as f32, z as f32) * (child * 2.0);
                menger(c, center + offset, child, depth - 1);
            }
        }
    }
}

const ICOSAHEDRON_FACES: [[usize; 3]; 20] = [
    [0, 11, 5],
    [0, 5, 1],
    [0, 1, 7],
    [0, 7, 10],
    [0, 10, 11],
    [1, 5, 9],
    [5, 11, 4],
    [11, 10, 2],
    [10, 7, 6],
    [7, 1, 8],
    [3, 9, 4],
    [3, 4, 2],
    [3, 2, 6],
    [3, 6, 8],
    [3, 8, 9],
    [4, 9, 5],
    [2, 4, 11],
    [6, 2, 10],
    [8, 6, 7],
    [9, 8, 1],
];

/// Recursively subdivided icosahedron. New edge midpoints are pushed in or
/// out by a per-level amount; displacement depends only on the edge's two
/// endpoints, so neighbouring faces agree and the surface never cracks.
fn geodesic(c: &mut Ctx, depth: u32) {
    let golden = (1.0 + 5.0f32.sqrt()) * 0.5;
    let radius = c.p.scale * 1.3;
    let v = |x: f32, y: f32, z: f32| Vec3::new(x, y, z).normalized() * radius;
    let vertices = [
        v(-1.0, golden, 0.0),
        v(1.0, golden, 0.0),
        v(-1.0, -golden, 0.0),
        v(1.0, -golden, 0.0),
        v(0.0, -1.0, golden),
        v(0.0, 1.0, golden),
        v(0.0, -1.0, -golden),
        v(0.0, 1.0, -golden),
        v(golden, 0.0, -1.0),
        v(golden, 0.0, 1.0),
        v(-golden, 0.0, -1.0),
        v(-golden, 0.0, 1.0),
    ];
    for face in ICOSAHEDRON_FACES {
        if c.out.full() {
            return;
        }
        geodesic_face(c, face.map(|i| vertices[i]), depth, 0);
    }
}

fn geodesic_face(c: &mut Ctx, [a, b, d]: [Vec3; 3], depth: u32, level: u32) {
    if c.out.full() {
        return;
    }
    if depth == 0 {
        c.out.line(a, b, 0.0);
        c.out.line(b, d, 0.0);
        c.out.line(d, a, 0.0);
        return;
    }
    let p = c.p;
    let spike = (p.spread - 0.45) * 1.2 + p.audio.bass * 0.6;
    let falloff = 0.6f32.powi(level as i32);
    let phase = (level as f32 * PI * (0.5 + p.twist)).cos();
    let midpoint = |x: Vec3, y: Vec3| {
        let direction = (x + y).normalized();
        let noise = position_noise(direction, p.seed) * 2.0;
        let shape = phase + (noise - phase) * p.randomness;
        let length = (x.length() + y.length()) * 0.5 * (1.0 + spike * falloff * shape);
        direction * length.max(0.05)
    };
    let (ab, bd, da) = (midpoint(a, b), midpoint(b, d), midpoint(d, a));
    for child in [[a, ab, da], [ab, b, bd], [da, bd, d], [ab, bd, da]] {
        geodesic_face(c, child, depth - 1, level + 1);
    }
}

fn chebyshev(x: f32, degree: u32) -> f32 {
    let (mut a, mut b) = (1.0, x);
    if degree == 0 {
        return a;
    }
    for _ in 1..degree {
        (a, b) = (b, 2.0 * x * b - a);
    }
    b
}

fn advanced_curves(c: &mut Ctx, depth: u32) {
    let p = *c.p;
    let steps = if p.pattern == RecursivePattern::ChebyshevCurve {
        256
    } else {
        64
    } * depth.max(1);
    let count = p.contours.round().clamp(4.0, 64.0) as u32;
    let degree = |v: f32| v.round().clamp(1.0, 12.0) as u32;
    let orient = |v: Vec3| match p.slice_axis.round() as u32 {
        0 => Vec3::new(v.z, v.x, v.y),
        1 => Vec3::new(v.x, v.z, v.y),
        _ => v,
    };
    let cheb_point = |t: f32| {
        let component = |phase: f32, d: f32| {
            let x = (t + phase).cos();
            chebyshev(x, degree(d)) * (1.0 - p.polynomial_mix) + x * p.polynomial_mix
        };
        Vec3::new(
            component(0.0, p.degree_x),
            component(0.4 + p.twist, p.degree_y),
            component(1.1 + p.spread, p.degree_z),
        )
    };
    // Positive even powers and nonnegative cross terms keep each radial
    // slice bounded and monotone, making bisection deterministic.
    let order = ((p.surface_order / 2.0).round().clamp(1.0, 6.0) as i32) * 2;
    let contour_point = |t: f32, z: f32| {
        let (sin, cos) = t.sin_cos();
        let field = |r: f32| {
            let (x, y) = (r * cos, r * sin);
            x.powi(order)
                + y.powi(order)
                + z.powi(order)
                + p.surface_cross * (x * x * y * y + y * y * z * z + z * z * x * x)
        };
        let (mut lo, mut hi) = (0.0, 2.0);
        for _ in 0..24 {
            let mid = (lo + hi) * 0.5;
            if field(mid) > 1.0 {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        orient(Vec3::new(cos * (lo + hi) * 0.5, sin * (lo + hi) * 0.5, z))
    };
    let super_radius = |angle: f32| {
        let a = p.symmetry.round() * angle * 0.25;
        (a.cos().abs().powf(p.exponent) + a.sin().abs().powf(p.exponent))
            .max(0.001)
            .powf(-1.0 / p.exponent)
    };
    let super_point = |lon: f32, lat: f32| {
        let r = super_radius(lon);
        let v = super_radius(lat);
        orient(Vec3::new(
            r * lon.cos() * v * lat.cos(),
            r * lon.sin() * v * lat.cos(),
            v * lat.sin(),
        ))
    };
    let paths = if p.pattern == RecursivePattern::ChebyshevCurve {
        1
    } else if p.pattern == RecursivePattern::Supershape {
        count * 2
    } else {
        count
    };
    for path in 0..paths {
        if c.out.full() {
            break;
        }
        let point = |i: u32| {
            let t = i as f32 / steps as f32;
            let v = match p.pattern {
                RecursivePattern::ChebyshevCurve => cheb_point(t * TAU),
                RecursivePattern::PolynomialContours => {
                    contour_point(t * TAU, -0.98 + 1.96 * path as f32 / (count - 1) as f32)
                }
                _ if path < count => super_point(
                    t * TAU,
                    -FRAC_PI_2 + PI * (path as f32 + 0.5) / count as f32,
                ),
                _ => super_point(
                    (path - count) as f32 / count as f32 * TAU,
                    -FRAC_PI_2 + t * PI,
                ),
            };
            v * p.scale
        };
        let first = point(0);
        let mut previous = first;
        for i in 1..=steps {
            let closed = p.pattern != RecursivePattern::Supershape || path < count;
            let next = if i == steps && closed {
                first
            } else {
                point(i)
            };
            if !c.out.line(previous, next, 0.0) {
                return;
            }
            previous = next;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{AudioBands, generate};
    use crate::settings::GeneratorSettings;

    fn params(pattern: RecursivePattern, levels: u32) -> GeometryParams {
        let mut p = GeometryParams::from_settings(
            &GeneratorSettings {
                pattern,
                ..GeneratorSettings::default()
            },
            AudioBands::default(),
            0.0,
        );
        p.levels = levels;
        p
    }

    #[test]
    fn new_curves_are_closed_seeded_and_respond_to_controls() {
        for pattern in [
            RecursivePattern::Spirograph,
            RecursivePattern::TorusKnot,
            RecursivePattern::RoseCurve,
            RecursivePattern::Lissajous,
            RecursivePattern::SphericalWeave,
            RecursivePattern::MobiusRibbon,
        ] {
            let mut p = params(pattern, 7);
            p.randomness = 0.3;
            let geometry = generate(&p);
            assert_eq!(geometry, generate(&p));
            assert_eq!(
                geometry.segments.first().unwrap().start,
                geometry.segments.last().unwrap().end
            );
            p.seed += 1;
            assert_ne!(geometry, generate(&p));
            p.spread = 1.0;
            assert_ne!(geometry, generate(&p));
        }
    }

    #[test]
    fn hilbert_point_covers_the_grid_with_adjacent_steps() {
        let n = 8;
        let mut seen = vec![false; (n * n) as usize];
        let mut last = hilbert_point(n, 0);
        seen[(last.1 * n + last.0) as usize] = true;
        for index in 1..n * n {
            let point = hilbert_point(n, index);
            assert_eq!(point.0.abs_diff(last.0) + point.1.abs_diff(last.1), 1);
            assert!(!seen[(point.1 * n + point.0) as usize]);
            seen[(point.1 * n + point.0) as usize] = true;
            last = point;
        }
    }

    #[test]
    fn hilbert_without_rounding_has_unit_steps() {
        let mut p = params(RecursivePattern::HilbertCurve, 5);
        p.spread = 0.0;
        let geometry = generate(&p);
        assert_eq!(geometry.segments.len(), 63);
    }

    #[test]
    fn estimates_are_upper_bounds() {
        for pattern in RecursivePattern::ALL {
            for levels in [3, 7] {
                let p = params(pattern, levels);
                let geometry = generate(&p);
                assert!(
                    geometry.segments.len() as u64 <= estimate(&p, geometry.depth),
                    "{} at {levels}: {} > {}",
                    pattern.label(),
                    geometry.segments.len(),
                    estimate(&p, geometry.depth)
                );
            }
        }
    }

    #[test]
    fn exact_counts_for_fixed_shape_patterns() {
        let count = |pattern, levels| generate(&params(pattern, levels)).segments.len();
        assert_eq!(count(RecursivePattern::SierpinskiTriangle, 5), 3 * 81);
        assert_eq!(count(RecursivePattern::DragonCurve, 3), 256);
        assert_eq!(count(RecursivePattern::HTree, 3), 31);
        assert_eq!(count(RecursivePattern::SierpinskiTetrahedron, 5), 6 * 64);
        assert_eq!(count(RecursivePattern::MengerSponge, 5), 12 * 20);
        assert_eq!(count(RecursivePattern::GeodesicSphere, 6), 60 * 16);
        assert_eq!(count(RecursivePattern::CrystalGrowth, 3), 6 * 13);
    }

    #[test]
    fn mandala_arm_count_follows_spread() {
        assert_eq!(mandala_arms(0.0), 8);
        assert_eq!(mandala_arms(1.0), 32);
        let mut p = params(RecursivePattern::Mandala, 11);
        p.spread = 1.0;
        let geometry = generate(&p);
        assert!(geometry.depth < 11);
        assert!(!geometry.truncated);
    }

    #[test]
    fn geodesic_subdivision_stays_closed_without_spikes() {
        let mut p = params(RecursivePattern::GeodesicSphere, 6);
        p.spread = 0.45; // zero spike
        let geometry = generate(&p);
        let radius = p.scale * 1.3;
        for segment in &geometry.segments {
            assert!((segment.start.length() - radius).abs() < 1e-3);
        }
    }

    #[test]
    fn audio_changes_tree_geometry_only_when_present() {
        let quiet = params(RecursivePattern::FractalTree, 6);
        let mut loud = quiet;
        loud.audio = AudioBands {
            bass: 1.0,
            mid: 1.0,
            high: 1.0,
        };
        assert_ne!(generate(&quiet), generate(&loud));
        assert_eq!(generate(&quiet), generate(&quiet));
    }
}
