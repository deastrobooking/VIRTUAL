//! Small vector, rotation, colour and PRNG helpers shared by the patterns.

use std::ops::{Add, AddAssign, Mul, Neg, Sub};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub const ZERO: Self = Self::new(0.0, 0.0, 0.0);
    pub const X: Self = Self::new(1.0, 0.0, 0.0);
    pub const Y: Self = Self::new(0.0, 1.0, 0.0);
    pub const Z: Self = Self::new(0.0, 0.0, 1.0);

    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub const fn to_array(self) -> [f32; 3] {
        [self.x, self.y, self.z]
    }

    pub fn dot(self, other: Self) -> f32 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    pub fn cross(self, other: Self) -> Self {
        Self::new(
            self.y * other.z - self.z * other.y,
            self.z * other.x - self.x * other.z,
            self.x * other.y - self.y * other.x,
        )
    }

    pub fn length(self) -> f32 {
        self.dot(self).sqrt()
    }

    pub fn normalized(self) -> Self {
        let length = self.length();
        if length > f32::EPSILON {
            self * (1.0 / length)
        } else {
            self
        }
    }

    pub fn lerp(self, other: Self, t: f32) -> Self {
        self + (other - self) * t
    }

    pub fn min(self, other: Self) -> Self {
        Self::new(
            self.x.min(other.x),
            self.y.min(other.y),
            self.z.min(other.z),
        )
    }

    pub fn max(self, other: Self) -> Self {
        Self::new(
            self.x.max(other.x),
            self.y.max(other.y),
            self.z.max(other.z),
        )
    }

    /// Rodrigues rotation of `self` about the unit `axis` by `angle` radians.
    /// Positive angles are counter-clockwise looking down the axis.
    pub fn rotated(self, axis: Self, angle: f32) -> Self {
        let (sin, cos) = angle.sin_cos();
        self * cos + axis.cross(self) * sin + axis * (axis.dot(self) * (1.0 - cos))
    }
}

impl Add for Vec3 {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self::new(self.x + other.x, self.y + other.y, self.z + other.z)
    }
}

impl AddAssign for Vec3 {
    fn add_assign(&mut self, other: Self) {
        *self = *self + other;
    }
}

impl Sub for Vec3 {
    type Output = Self;
    fn sub(self, other: Self) -> Self {
        Self::new(self.x - other.x, self.y - other.y, self.z - other.z)
    }
}

impl Mul<f32> for Vec3 {
    type Output = Self;
    fn mul(self, scale: f32) -> Self {
        Self::new(self.x * scale, self.y * scale, self.z * scale)
    }
}

impl Neg for Vec3 {
    type Output = Self;
    fn neg(self) -> Self {
        Self::new(-self.x, -self.y, -self.z)
    }
}

/// Row-major 3×3 rotation used by the rasterizer camera.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mat3 {
    rows: [Vec3; 3],
}

impl Mat3 {
    pub const IDENTITY: Self = Self {
        rows: [Vec3::X, Vec3::Y, Vec3::Z],
    };

    pub fn rotation_x(angle: f32) -> Self {
        let (s, c) = angle.sin_cos();
        Self {
            rows: [Vec3::X, Vec3::new(0.0, c, -s), Vec3::new(0.0, s, c)],
        }
    }

    pub fn rotation_y(angle: f32) -> Self {
        let (s, c) = angle.sin_cos();
        Self {
            rows: [Vec3::new(c, 0.0, s), Vec3::Y, Vec3::new(-s, 0.0, c)],
        }
    }

    pub fn rotation_z(angle: f32) -> Self {
        let (s, c) = angle.sin_cos();
        Self {
            rows: [Vec3::new(c, -s, 0.0), Vec3::new(s, c, 0.0), Vec3::Z],
        }
    }

    pub fn transform(&self, v: Vec3) -> Vec3 {
        Vec3::new(
            self.rows[0].dot(v),
            self.rows[1].dot(v),
            self.rows[2].dot(v),
        )
    }

    pub fn then(&self, next: &Self) -> Self {
        // next × self: apply `self` first.
        let columns = [
            Vec3::new(self.rows[0].x, self.rows[1].x, self.rows[2].x),
            Vec3::new(self.rows[0].y, self.rows[1].y, self.rows[2].y),
            Vec3::new(self.rows[0].z, self.rows[1].z, self.rows[2].z),
        ];
        let row = |r: Vec3| Vec3::new(r.dot(columns[0]), r.dot(columns[1]), r.dot(columns[2]));
        Self {
            rows: [row(next.rows[0]), row(next.rows[1]), row(next.rows[2])],
        }
    }
}

/// HSL in `[0, 1]` to linear-ish RGB in `[0, 1]`. Hue wraps with
/// `rem_euclid`, so negative animated hues stay in range.
pub fn hsl_to_rgb(hue: f32, saturation: f32, lightness: f32) -> [f32; 3] {
    let hue = hue.rem_euclid(1.0);
    let saturation = saturation.clamp(0.0, 1.0);
    let lightness = lightness.clamp(0.0, 1.0);
    if saturation <= f32::EPSILON {
        return [lightness; 3];
    }
    let q = if lightness < 0.5 {
        lightness * (1.0 + saturation)
    } else {
        lightness + saturation - lightness * saturation
    };
    let p = 2.0 * lightness - q;
    let channel = |offset: f32| {
        let t = (hue + offset).rem_euclid(1.0);
        if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        }
    };
    [channel(1.0 / 3.0), channel(0.0), channel(-1.0 / 3.0)]
}

/// SplitMix64: tiny, seedable and identical on every platform, so a pattern
/// seed reproduces the same geometry everywhere.
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed ^ 0x9E37_79B9_7F4A_7C15)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        mix64(self.0)
    }

    /// Uniform in `[0, 1)`.
    pub fn next_f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    /// `(random − 0.5) × amount`.
    pub fn jitter(&mut self, amount: f32) -> f32 {
        if amount == 0.0 {
            return 0.0;
        }
        (self.next_f32() - 0.5) * amount
    }
}

pub fn mix64(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Position-keyed noise in `[-0.5, 0.5)`. Shared vertices produced by
/// neighbouring subdivisions hash identically, so displaced meshes stay
/// crack-free.
pub fn position_noise(point: Vec3, seed: u64) -> f32 {
    let quantize = |v: f32| (v * 4096.0).round() as i64 as u64;
    let hash = mix64(
        seed ^ quantize(point.x).wrapping_mul(0x8DA6_B343)
            ^ quantize(point.y).wrapping_mul(0xD816_3841)
            ^ quantize(point.z).wrapping_mul(0xCB1A_B31F),
    );
    (hash >> 40) as f32 / (1u64 << 24) as f32 - 0.5
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Vec3, b: Vec3) -> bool {
        (a - b).length() < 1e-5
    }

    #[test]
    fn rotation_is_counter_clockwise_about_the_axis() {
        let rotated = Vec3::Y.rotated(Vec3::Z, std::f32::consts::FRAC_PI_2);
        assert!(close(rotated, Vec3::new(-1.0, 0.0, 0.0)));
    }

    #[test]
    fn matrix_composition_applies_left_operand_first() {
        let m = Mat3::rotation_z(std::f32::consts::FRAC_PI_2)
            .then(&Mat3::rotation_x(std::f32::consts::FRAC_PI_2));
        // X → Y (about Z) → Z (about X).
        assert!(close(m.transform(Vec3::X), Vec3::Z));
        assert!(close(Mat3::IDENTITY.transform(Vec3::X), Vec3::X));
    }

    #[test]
    fn hsl_primaries_and_negative_hue_wrap() {
        let red = hsl_to_rgb(0.0, 1.0, 0.5);
        assert!((red[0] - 1.0).abs() < 1e-5 && red[1].abs() < 1e-5 && red[2].abs() < 1e-5);
        assert_eq!(hsl_to_rgb(-0.25, 0.8, 0.4), hsl_to_rgb(0.75, 0.8, 0.4));
        assert_eq!(hsl_to_rgb(0.3, 0.0, 0.6), [0.6; 3]);
    }

    #[test]
    fn seeded_rng_is_repeatable_and_in_range() {
        let mut a = Rng::new(7);
        let mut b = Rng::new(7);
        for _ in 0..1000 {
            let value = a.next_f32();
            assert_eq!(value, b.next_f32());
            assert!((0.0..1.0).contains(&value));
        }
        assert_ne!(Rng::new(7).next_u64(), Rng::new(8).next_u64());
    }
}
