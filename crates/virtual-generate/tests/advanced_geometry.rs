use virtual_generate::{
    AudioBands, GENERATOR_PARAMETERS, Generator, GeneratorSettings, GeometryParams,
    RecursivePattern, generate,
};

#[test]
fn echoes_share_budget_and_keep_trace_coordinates() {
    for pattern in RecursivePattern::ALL {
        let settings = GeneratorSettings {
            pattern,
            echo_copies: 24.0,
            depth: 1.0,
            ..Default::default()
        };
        let mut params = GeometryParams::from_settings(&settings, AudioBands::default(), 0.0);
        params.budget = 12_000;
        let geometry = generate(&params);
        assert!(geometry.segments.len() <= params.budget, "{pattern:?}");
        assert!(!geometry.segments.is_empty(), "{pattern:?}");
        for line in &geometry.segments {
            assert!(line.start.length().is_finite() && line.end.length().is_finite());
            assert!(
                line.trace[0] >= 0.0 && line.trace[1] <= 1.00001 && line.trace[0] <= line.trace[1]
            );
            assert!(line.echo < 24);
        }
    }
}

#[test]
fn polynomial_slices_satisfy_the_surface_equation() {
    for order in [2.0, 4.0, 8.0, 12.0] {
        let settings = GeneratorSettings {
            pattern: RecursivePattern::PolynomialContours,
            surface_order: order,
            surface_cross: 1.7,
            ..Default::default()
        };
        let params = GeometryParams::from_settings(&settings, AudioBands::default(), 0.0);
        let geometry = generate(&params);
        for line in &geometry.segments {
            let p = line.start * (1.0 / params.scale);
            let value = p.x.powi(order as i32)
                + p.y.powi(order as i32)
                + p.z.powi(order as i32)
                + 1.7 * (p.x * p.x * p.y * p.y + p.y * p.y * p.z * p.z + p.z * p.z * p.x * p.x);
            assert!((value - 1.0).abs() < 0.0001, "{order}: {value}");
        }
    }
}

#[test]
fn normalized_parameters_are_stable_bounded_and_reject_nonfinite_input() {
    assert_eq!(GENERATOR_PARAMETERS.len(), 46);
    let mut settings = GeneratorSettings::default();
    for (i, p) in GENERATOR_PARAMETERS.iter().enumerate() {
        assert_eq!(p.id as usize, i);
        for value in [0.0, 0.37, 1.0] {
            settings.set_parameter(p.id, value);
            let actual = settings.parameter(p.id).unwrap();
            assert!((0.0..=1.0).contains(&actual));
            if p.id < 43 {
                assert!((actual - value).abs() < 0.00001, "{}", p.name);
            }
        }
        let before = settings.clone();
        settings.set_parameter(p.id, f32::NAN);
        assert_eq!(settings, before);
    }
    assert!(settings.parameter(255).is_none());
}

#[test]
fn traced_echoes_render_animate_and_hold_when_stopped() {
    for pattern in [
        RecursivePattern::ChebyshevCurve,
        RecursivePattern::PolynomialContours,
        RecursivePattern::Supershape,
    ] {
        let mut settings = GeneratorSettings {
            pattern,
            resolution: [160, 96],
            echo_copies: 4.0,
            trace_heads: 3.0,
            trace_length: 0.25,
            trace_speed: 0.2,
            trace_spread: 1.0,
            rotate_speed: 0.0,
            color_speed: 0.0,
            ..Default::default()
        };
        let mut generator = Generator::new(settings.clone());
        let mut first = vec![0; 160 * 96 * 4];
        let mut next = first.clone();
        generator.render(0.0, AudioBands::default(), &mut first);
        generator.render(0.2, AudioBands::default(), &mut next);
        assert!(first.chunks_exact(4).any(|p| p[3] > 0), "{pattern:?}");
        assert_ne!(first, next, "{pattern:?}");
        settings.trace_speed = 0.0;
        generator.set_settings(settings);
        generator.render(0.4, AudioBands::default(), &mut first);
        generator.render(0.6, AudioBands::default(), &mut next);
        assert_eq!(first, next, "{pattern:?}");
    }
}
