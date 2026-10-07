//! Append-only generator parameter IDs. MIDI values use the normalized 0–1 domain.
use crate::GeneratorSettings;
pub struct GeneratorParameter {
    pub id: u8,
    pub name: &'static str,
    pub min: f32,
    pub max: f32,
}
pub const GENERATOR_PARAMETERS: &[GeneratorParameter] = &[
    GeneratorParameter {
        id: 0,
        name: "depth",
        min: 0.0,
        max: 1.0,
    },
    GeneratorParameter {
        id: 1,
        name: "scale",
        min: 0.0,
        max: 1.0,
    },
    GeneratorParameter {
        id: 2,
        name: "spread",
        min: 0.0,
        max: 1.0,
    },
    GeneratorParameter {
        id: 3,
        name: "twist",
        min: 0.0,
        max: 1.0,
    },
    GeneratorParameter {
        id: 4,
        name: "randomness",
        min: 0.0,
        max: 1.0,
    },
    GeneratorParameter {
        id: 5,
        name: "rotate speed",
        min: -1.0,
        max: 1.0,
    },
    GeneratorParameter {
        id: 6,
        name: "tilt",
        min: -1.0,
        max: 1.0,
    },
    GeneratorParameter {
        id: 7,
        name: "spin speed",
        min: -1.0,
        max: 1.0,
    },
    GeneratorParameter {
        id: 8,
        name: "zoom",
        min: 0.25,
        max: 4.0,
    },
    GeneratorParameter {
        id: 9,
        name: "perspective",
        min: 0.0,
        max: 1.0,
    },
    GeneratorParameter {
        id: 10,
        name: "reveal",
        min: 0.0,
        max: 1.0,
    },
    GeneratorParameter {
        id: 11,
        name: "grow speed",
        min: 0.0,
        max: 1.0,
    },
    GeneratorParameter {
        id: 12,
        name: "hue",
        min: 0.0,
        max: 1.0,
    },
    GeneratorParameter {
        id: 13,
        name: "hue range",
        min: 0.0,
        max: 1.0,
    },
    GeneratorParameter {
        id: 14,
        name: "color speed",
        min: 0.0,
        max: 1.0,
    },
    GeneratorParameter {
        id: 15,
        name: "saturation",
        min: 0.0,
        max: 1.0,
    },
    GeneratorParameter {
        id: 16,
        name: "lightness",
        min: 0.0,
        max: 1.0,
    },
    GeneratorParameter {
        id: 17,
        name: "brightness",
        min: 0.0,
        max: 4.0,
    },
    GeneratorParameter {
        id: 18,
        name: "line width",
        min: 0.5,
        max: 6.0,
    },
    GeneratorParameter {
        id: 19,
        name: "depth fade",
        min: 0.0,
        max: 1.0,
    },
    GeneratorParameter {
        id: 20,
        name: "trails",
        min: 0.0,
        max: 0.97,
    },
    GeneratorParameter {
        id: 21,
        name: "audio amount",
        min: 0.0,
        max: 1.0,
    },
    GeneratorParameter {
        id: 22,
        name: "degree x",
        min: 1.0,
        max: 12.0,
    },
    GeneratorParameter {
        id: 23,
        name: "degree y",
        min: 1.0,
        max: 12.0,
    },
    GeneratorParameter {
        id: 24,
        name: "degree z",
        min: 1.0,
        max: 12.0,
    },
    GeneratorParameter {
        id: 25,
        name: "polynomial mix",
        min: 0.0,
        max: 1.0,
    },
    GeneratorParameter {
        id: 26,
        name: "surface order",
        min: 2.0,
        max: 12.0,
    },
    GeneratorParameter {
        id: 27,
        name: "surface cross",
        min: 0.0,
        max: 4.0,
    },
    GeneratorParameter {
        id: 28,
        name: "contours",
        min: 4.0,
        max: 64.0,
    },
    GeneratorParameter {
        id: 29,
        name: "slice axis",
        min: 0.0,
        max: 2.0,
    },
    GeneratorParameter {
        id: 30,
        name: "symmetry",
        min: 2.0,
        max: 12.0,
    },
    GeneratorParameter {
        id: 31,
        name: "exponent",
        min: 0.25,
        max: 4.0,
    },
    GeneratorParameter {
        id: 32,
        name: "echo copies",
        min: 1.0,
        max: 24.0,
    },
    GeneratorParameter {
        id: 33,
        name: "echo scale",
        min: 0.5,
        max: 1.0,
    },
    GeneratorParameter {
        id: 34,
        name: "echo x",
        min: -1.0,
        max: 1.0,
    },
    GeneratorParameter {
        id: 35,
        name: "echo y",
        min: -1.0,
        max: 1.0,
    },
    GeneratorParameter {
        id: 36,
        name: "echo z",
        min: -1.0,
        max: 1.0,
    },
    GeneratorParameter {
        id: 37,
        name: "echo offset",
        min: -0.25,
        max: 0.25,
    },
    GeneratorParameter {
        id: 38,
        name: "echo fade",
        min: 0.1,
        max: 1.0,
    },
    GeneratorParameter {
        id: 39,
        name: "trace heads",
        min: 1.0,
        max: 8.0,
    },
    GeneratorParameter {
        id: 40,
        name: "trace length",
        min: 0.0,
        max: 1.0,
    },
    GeneratorParameter {
        id: 41,
        name: "trace speed",
        min: -1.0,
        max: 1.0,
    },
    GeneratorParameter {
        id: 42,
        name: "trace spread",
        min: 0.0,
        max: 1.0,
    },
    GeneratorParameter {
        id: 43,
        name: "flatten",
        min: 0.0,
        max: 1.0,
    },
    GeneratorParameter {
        id: 44,
        name: "transparent",
        min: 0.0,
        max: 1.0,
    },
    GeneratorParameter {
        id: 45,
        name: "pattern",
        min: 0.0,
        max: 1.0,
    },
];
impl GeneratorSettings {
    pub fn parameter(&self, id: u8) -> Option<f32> {
        let p = GENERATOR_PARAMETERS.get(id as usize)?;
        let value = match id {
            0 => self.depth,
            1 => self.scale,
            2 => self.spread,
            3 => self.twist,
            4 => self.randomness,
            5 => self.rotate_speed,
            6 => self.tilt,
            7 => self.spin_speed,
            8 => self.zoom,
            9 => self.perspective,
            10 => self.reveal,
            11 => self.grow_speed,
            12 => self.hue,
            13 => self.hue_range,
            14 => self.color_speed,
            15 => self.saturation,
            16 => self.lightness,
            17 => self.brightness,
            18 => self.line_width,
            19 => self.depth_fade,
            20 => self.trails,
            21 => self.audio_amount,
            22 => self.degree_x,
            23 => self.degree_y,
            24 => self.degree_z,
            25 => self.polynomial_mix,
            26 => self.surface_order,
            27 => self.surface_cross,
            28 => self.contours,
            29 => self.slice_axis,
            30 => self.symmetry,
            31 => self.exponent,
            32 => self.echo_copies,
            33 => self.echo_scale,
            34 => self.echo_x,
            35 => self.echo_y,
            36 => self.echo_z,
            37 => self.echo_offset,
            38 => self.echo_fade,
            39 => self.trace_heads,
            40 => self.trace_length,
            41 => self.trace_speed,
            42 => self.trace_spread,
            43 => f32::from(self.flatten),
            44 => f32::from(self.transparent),
            45 => {
                crate::RecursivePattern::ALL
                    .iter()
                    .position(|p| *p == self.pattern)
                    .unwrap_or(0) as f32
                    / (crate::RecursivePattern::ALL.len() - 1) as f32
            }
            _ => return None,
        };
        Some(((value - p.min) / (p.max - p.min)).clamp(0.0, 1.0))
    }
    pub fn set_parameter(&mut self, id: u8, normalized: f32) {
        if !normalized.is_finite() {
            return;
        }
        let Some(p) = GENERATOR_PARAMETERS.get(id as usize) else {
            return;
        };
        let v = p.min + normalized.clamp(0.0, 1.0) * (p.max - p.min);
        match id {
            0 => self.depth = v,
            1 => self.scale = v,
            2 => self.spread = v,
            3 => self.twist = v,
            4 => self.randomness = v,
            5 => self.rotate_speed = v,
            6 => self.tilt = v,
            7 => self.spin_speed = v,
            8 => self.zoom = v,
            9 => self.perspective = v,
            10 => self.reveal = v,
            11 => self.grow_speed = v,
            12 => self.hue = v,
            13 => self.hue_range = v,
            14 => self.color_speed = v,
            15 => self.saturation = v,
            16 => self.lightness = v,
            17 => self.brightness = v,
            18 => self.line_width = v,
            19 => self.depth_fade = v,
            20 => self.trails = v,
            21 => self.audio_amount = v,
            22 => self.degree_x = v,
            23 => self.degree_y = v,
            24 => self.degree_z = v,
            25 => self.polynomial_mix = v,
            26 => self.surface_order = v,
            27 => self.surface_cross = v,
            28 => self.contours = v,
            29 => self.slice_axis = v,
            30 => self.symmetry = v,
            31 => self.exponent = v,
            32 => self.echo_copies = v,
            33 => self.echo_scale = v,
            34 => self.echo_x = v,
            35 => self.echo_y = v,
            36 => self.echo_z = v,
            37 => self.echo_offset = v,
            38 => self.echo_fade = v,
            39 => self.trace_heads = v,
            40 => self.trace_length = v,
            41 => self.trace_speed = v,
            42 => self.trace_spread = v,
            43 => self.flatten = v >= 0.5,
            44 => self.transparent = v >= 0.5,
            45 => {
                self.pattern = crate::RecursivePattern::ALL
                    [(v * (crate::RecursivePattern::ALL.len() - 1) as f32).round() as usize]
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RecursivePattern;

    #[test]
    fn ids_are_dense_and_append_only() {
        for (index, parameter) in GENERATOR_PARAMETERS.iter().enumerate() {
            assert_eq!(usize::from(parameter.id), index, "{}", parameter.name);
            assert!(parameter.max > parameter.min, "{}", parameter.name);
        }
        // Saved MIDI/OSC mappings reference these ids; never reorder them.
        assert_eq!(GENERATOR_PARAMETERS[0].name, "depth");
        assert_eq!(GENERATOR_PARAMETERS[45].name, "pattern");
    }

    #[test]
    fn every_parameter_round_trips_through_the_normalized_domain() {
        let defaults = GeneratorSettings::default();
        for parameter in GENERATOR_PARAMETERS {
            let mut settings = defaults.clone();
            for normalized in [0.0, 1.0] {
                settings.set_parameter(parameter.id, normalized);
                assert_eq!(
                    settings.parameter(parameter.id),
                    Some(normalized),
                    "{}",
                    parameter.name
                );
            }
            // Out-of-range and non-finite values never escape the domain.
            settings.set_parameter(parameter.id, 2.0);
            assert_eq!(settings.parameter(parameter.id), Some(1.0));
            settings.set_parameter(parameter.id, f32::NAN);
            assert_eq!(settings.parameter(parameter.id), Some(1.0));
            // Mid-range values survive project sanitizing (hue wraps at 1).
            settings.set_parameter(parameter.id, 0.5);
            assert_eq!(settings.clone().sanitized(), settings, "{}", parameter.name);
        }
        assert_eq!(defaults.parameter(GENERATOR_PARAMETERS.len() as u8), None);
    }

    #[test]
    fn pattern_parameter_reaches_every_pattern() {
        let mut settings = GeneratorSettings::default();
        let last = (RecursivePattern::ALL.len() - 1) as f32;
        for (index, pattern) in RecursivePattern::ALL.into_iter().enumerate() {
            settings.set_parameter(45, index as f32 / last);
            assert_eq!(settings.pattern, pattern);
        }
    }
}
