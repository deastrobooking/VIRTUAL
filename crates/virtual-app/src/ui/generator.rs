//! Recursive-geometry generator UI.
//!
//! Generators are a deck *source*, not an effect, so their controls live in
//! their own window per deck rather than in the deck's effect editor. The
//! window opens when a generator becomes a deck's source and closes when the
//! deck switches to anything else; the deck strip keeps a one-line summary
//! with a button to reopen it.

use virtual_generate::{
    CalculatorExpression, CalculatorValue, Dimension, FRAME_RATES, GeneratorSettings,
    GeneratorStack, GeneratorStats, GeometryGraph, GeometryNode, GeometryNodeKind,
    GeometryParameter, GraphAxis, GraphNodeId, RESOLUTIONS, RecursivePattern,
};
use virtual_media::{DeckId, DeckState, FourDeckMixer};

use super::theme::ThemePalette;
use super::{UiAction, UiState, buttons};

/// Pattern picker grouped by dimensionality, for the compact source loader.
fn pattern_combo(ui: &mut egui::Ui, id: &str, pattern: &mut RecursivePattern) {
    egui::ComboBox::from_id_salt(id)
        .selected_text(pattern.label())
        .width(170.0)
        .show_ui(ui, |ui| {
            for (heading, dimension) in [("2D", Dimension::Planar), ("3D", Dimension::Spatial)] {
                ui.weak(heading);
                for candidate in RecursivePattern::ALL
                    .into_iter()
                    .filter(|candidate| candidate.dimension() == dimension)
                {
                    ui.selectable_value(pattern, candidate, candidate.label())
                        .on_hover_text(candidate.hint());
                }
            }
        });
}

/// Source loader shown beside the video-input controls for the selected
/// deck. Loading opens that deck's generator window.
pub(super) fn draw_generator_source(
    ui: &mut egui::Ui,
    state: &mut UiState,
    deck: DeckId,
    actions: &mut Vec<UiAction>,
) {
    ui.push_id(("generator-source", deck.index()), |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.strong("Generator · recursive geometry");
            pattern_combo(ui, "pattern", &mut state.generator_pattern);
            if buttons::midi_button(
                ui,
                &format!("generator.{}.load", deck.index()),
                &format!("Deck {} · Load generator", deck.label()),
                |ui| {
                    ui.button(format!("Load to Deck {}", deck.label()))
                        .on_hover_text(
                            "Replaces this deck's clip or video input with a live generator \
                             and opens its controls",
                        )
                },
            )
            .clicked()
            {
                actions.push(UiAction::ConnectGenerator {
                    deck,
                    settings: GeneratorSettings {
                        pattern: state.generator_pattern,
                        ..GeneratorSettings::default()
                    },
                });
                state.generator_windows[deck.index()] = true;
            }
        });
    });
}

fn stats_line(
    ui: &mut egui::Ui,
    settings: &GeneratorSettings,
    stats: GeneratorStats,
    palette: ThemePalette,
) {
    ui.horizontal_wrapped(|ui| {
        ui.label(if stats.planar { "2D" } else { "3D" });
        ui.label(format!("{} segments", stats.segments));
        if stats.depth < stats.requested_depth {
            ui.colored_label(
                palette.warning,
                format!("depth {}/{}", stats.depth, stats.requested_depth),
            )
            .on_hover_text("Depth lowered to stay inside the 200,000 segment budget");
        } else {
            ui.label(format!("depth {}", stats.depth));
        }
        let [width, height] = settings.resolution;
        ui.weak(format!(
            "{width}×{height} · {:.1} ms",
            stats.render_micros as f32 / 1000.0
        ))
        .on_hover_text("CPU time for the last generated frame, including geometry rebuilds");
        let budget_ms = 1000.0 / settings.fps.max(1) as f32;
        if stats.render_micros as f32 / 1000.0 > budget_ms {
            ui.colored_label(palette.warning, "Over frame budget")
                .on_hover_text(
                    "Lower depth, line width, resolution or frame rate to reduce CPU load.",
                );
        }
        ui.weak(format!("{} / {} drawn", stats.drawn, stats.segments));
    });
}

/// Deck-strip summary for a generator source: what is running and a
/// toggle for its window. No parameters are edited here.
pub(super) fn draw_generator_summary(
    ui: &mut egui::Ui,
    deck: DeckId,
    settings: &GeneratorStack,
    stats: GeneratorStats,
    palette: ThemePalette,
    window_open: &mut bool,
) {
    ui.horizontal_wrapped(|ui| {
        ui.colored_label(palette.success, "◆ GENERATOR");
        ui.strong(settings.geometry_graph.as_ref().map_or_else(
            || settings.pattern.label().to_owned(),
            |graph| graph.name.clone(),
        ));
        ui.weak(format!("{} layers", settings.layers.len()));
        if buttons::midi_nav_button(
            ui,
            &format!("generator.{}.window", deck.index()),
            &format!("Deck {} · Generator window", deck.label()),
            |ui| {
                ui.selectable_label(*window_open, "Generator controls…")
                    .on_hover_text("Show or hide this deck's generator window")
            },
        )
        .clicked()
        {
            *window_open = !*window_open;
        }
    });
    stats_line(ui, settings, stats, palette);
}

/// One window per generator deck. Call once per frame at top level.
pub(super) fn draw_generator_windows(
    ctx: &egui::Context,
    state: &mut UiState,
    mixer: &mut FourDeckMixer,
    stats: [GeneratorStats; 4],
    palette: ThemePalette,
    midi_map: &super::MidiMapUi,
    actions: &mut Vec<UiAction>,
) {
    for deck in DeckId::ALL {
        let index = deck.index();
        let is_generator = matches!(mixer.deck(deck).state, DeckState::Generator(_));
        // Selecting a generator as the source (from the loader, OSC/MIDI or a
        // project load) opens its window; switching away closes it.
        if is_generator && !state.generator_seen[index] {
            state.generator_windows[index] = true;
        }
        state.generator_seen[index] = is_generator;
        if !is_generator {
            state.generator_windows[index] = false;
            continue;
        }
        if !state.generator_windows[index] {
            continue;
        }
        let DeckState::Generator(settings) = &mut mixer.deck_mut(deck).state else {
            continue;
        };
        let accent = palette.deck_color(deck);
        let mut open = true;
        egui::Window::new(
            egui::RichText::new(format!("◆ GENERATOR · DECK {}", deck.label()))
                .strong()
                .color(accent),
        )
        .id(egui::Id::new(("generator-window", index)))
        .open(&mut open)
        .default_pos([1220.0, 40.0 + index as f32 * 36.0])
        .default_size([720.0, 560.0])
        .min_width(520.0)
        .resizable(true)
        .vscroll(true)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Layers");
                for (layer_index, layer) in settings.layers.iter().enumerate() {
                    if ui
                        .selectable_label(settings.selected == layer_index, &layer.name)
                        .clicked()
                    {
                        settings.selected = layer_index;
                    }
                }
                if ui
                    .add_enabled(
                        settings.layers.len() < GeneratorStack::MAX_LAYERS,
                        egui::Button::new("+ Add layer"),
                    )
                    .clicked()
                {
                    settings.add_layer();
                }
                if ui
                    .add_enabled(settings.layers.len() > 1, egui::Button::new("Remove"))
                    .clicked()
                {
                    settings.remove_selected();
                }
            });
            if let Some(layer) = settings.selected_layer_mut() {
                ui.horizontal(|ui| {
                    ui.label("Placement");
                    ui.add(egui::Slider::new(&mut layer.position[0], -1.0..=1.0).text("X"));
                    ui.add(egui::Slider::new(&mut layer.position[1], -1.0..=1.0).text("Y"));
                    ui.add(egui::Slider::new(&mut layer.scale, 0.1..=3.0).text("Scale"));
                    ui.add(
                        egui::Slider::new(
                            &mut layer.rotation,
                            -std::f32::consts::PI..=std::f32::consts::PI,
                        )
                        .text("Rotation"),
                    );
                    ui.add(egui::Slider::new(&mut layer.opacity, 0.0..=1.0).text("Opacity"));
                });
                ui.horizontal(|ui| {
                    ui.checkbox(&mut layer.enabled, "Enabled");
                    egui::ComboBox::from_id_salt(("generator-layer-blend", deck.index(), layer.id))
                        .selected_text(match layer.blend {
                            virtual_generate::GeneratorBlendMode::Over => "Over",
                            virtual_generate::GeneratorBlendMode::Add => "Add",
                            virtual_generate::GeneratorBlendMode::Screen => "Screen",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut layer.blend,
                                virtual_generate::GeneratorBlendMode::Over,
                                "Over",
                            );
                            ui.selectable_value(
                                &mut layer.blend,
                                virtual_generate::GeneratorBlendMode::Add,
                                "Add",
                            );
                            ui.selectable_value(
                                &mut layer.blend,
                                virtual_generate::GeneratorBlendMode::Screen,
                                "Screen",
                            );
                        });
                });
            }
            ui.separator();
            if settings.selected != 0 {
                ui.weak("Output resolution and frame rate are shared with Layer 1.");
            }
            let controls_output = settings.selected == 0;
            let calculator = &mut state.geometry_calculator[index];
            let geometry_library = &mut state.geometry_library;
            draw_window_body(
                ui,
                deck,
                settings,
                stats[index],
                palette,
                state.show_mode,
                controls_output,
                calculator,
                geometry_library,
                midi_map,
                actions,
            );
        });
        settings.normalize_output();
        state.generator_windows[index] = open;
    }
}

fn parameter_slider(
    ui: &mut egui::Ui,
    settings: &mut GeneratorSettings,
    deck: DeckId,
    id: u8,
    map: &super::MidiMapUi,
    actions: &mut Vec<UiAction>,
) {
    let p = &virtual_generate::GENERATOR_PARAMETERS[id as usize];
    let mut value = p.min + settings.parameter(id).unwrap_or_default() * (p.max - p.min);
    let response = super::mappable(
        ui,
        map,
        virtual_core::ControlTarget::GeneratorParameter {
            deck: deck.index() as u8,
            parameter: id,
        },
        actions,
        |ui| {
            if id == 43 || id == 44 {
                let mut enabled = value >= 0.5;
                let response = ui.checkbox(&mut enabled, p.name);
                value = f32::from(enabled);
                response
            } else if id == 45 {
                let mut index = (value * (RecursivePattern::ALL.len() - 1) as f32).round() as usize;
                let response = ui.add(
                    egui::Slider::new(&mut index, 0..=RecursivePattern::ALL.len() - 1)
                        .text("pattern")
                        .custom_formatter(|v, _| {
                            RecursivePattern::ALL[v.round() as usize].label().to_owned()
                        }),
                );
                value = index as f32 / (RecursivePattern::ALL.len() - 1) as f32;
                response
            } else {
                let mut slider = egui::Slider::new(&mut value, p.min..=p.max).text(p.name);
                match id {
                    // Depth renders in whole levels (3–11).
                    0 => slider = slider.step_by(0.125),
                    8 => slider = slider.logarithmic(true),
                    26 => slider = slider.step_by(2.0),
                    22..=24 | 28..=30 | 32 | 39 => slider = slider.step_by(1.0),
                    _ => {}
                }
                ui.add(slider)
            }
        },
    );
    let response = match parameter_hint(id) {
        Some(hint) => response.on_hover_text(hint),
        None => response,
    };
    if response.changed() {
        settings.set_parameter(id, (value - p.min) / (p.max - p.min));
    }
}

fn parameter_hint(id: u8) -> Option<&'static str> {
    Some(match id {
        11 => "Above zero, the reveal replays from trunk to tips",
        14 => "Hue drift speed",
        22..=25 => {
            "Chebyshev Curve: per-axis polynomial degree; mix blends toward a plain Lissajous"
        }
        26..=29 => {
            "Polynomial Contours: even surface order, cross-term bulge, slice count and axis"
        }
        30 | 31 => "Supershape: rotational symmetry and superellipse exponent",
        32..=38 => {
            "Repeats the whole object, each copy rotated, scaled, offset and faded from the last"
        }
        39..=42 => "Draws moving heads along each path; length 1 shows the full wireframe",
        43 => "Projects 3D patterns onto the flat XY plane",
        44 => "Lines layer over lower decks; off draws on black",
        45 => "Selects the pattern; useful for MIDI pattern switching",
        _ => return None,
    })
}

fn pattern_tile(
    ui: &mut egui::Ui,
    pattern: RecursivePattern,
    selected: bool,
    palette: ThemePalette,
    accent: egui::Color32,
) -> egui::Response {
    let text = egui::RichText::new(pattern.label()).strong();
    ui.add(
        egui::Button::new(if selected {
            text.color(ui.visuals().strong_text_color())
        } else {
            text
        })
        .min_size(egui::vec2(150.0, 34.0))
        .selected(selected)
        .fill(if selected {
            palette.control_tint(accent, 0.55)
        } else {
            palette.control
        })
        .stroke(egui::Stroke::new(
            if selected {
                palette.outline_width + 1.5
            } else {
                palette.outline_width
            },
            if selected { accent } else { palette.stroke },
        )),
    )
    .on_hover_text(pattern.hint())
}

fn section(ui: &mut egui::Ui, title: &str, palette: ThemePalette, add: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::group(ui.style())
        .fill(ui.visuals().faint_bg_color)
        .stroke(egui::Stroke::new(palette.outline_width, palette.stroke))
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(8.0)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.strong(title);
            ui.add_space(2.0);
            add(ui);
        });
}

#[allow(clippy::too_many_arguments)]
fn draw_window_body(
    ui: &mut egui::Ui,
    deck: DeckId,
    settings: &mut GeneratorSettings,
    stats: GeneratorStats,
    palette: ThemePalette,
    show_mode: bool,
    controls_output: bool,
    calculator: &mut String,
    geometry_library: &mut Vec<GeometryGraph>,
    midi_map: &super::MidiMapUi,
    actions: &mut Vec<UiAction>,
) {
    let accent = palette.deck_color(deck);
    ui.horizontal_wrapped(|ui| {
        if let Some(graph) = &settings.geometry_graph {
            ui.heading(&graph.name);
            ui.weak("Custom node geometry");
        } else {
            ui.heading(settings.pattern.label());
            ui.weak(settings.pattern.hint());
        }
    });
    stats_line(ui, settings, stats, palette);
    draw_geometry_lab(ui, settings, calculator, geometry_library);
    ui.separator();

    ui.weak("Use MIDI Map in the main window, then click a parameter to learn; right-click clears its mapping.");
    {
        // Pattern gallery.
        for (heading, dimension) in [
            ("2D PATTERNS", Dimension::Planar),
            ("3D PATTERNS", Dimension::Spatial),
        ] {
            ui.weak(heading);
            ui.horizontal_wrapped(|ui| {
                for pattern in RecursivePattern::ALL
                    .into_iter()
                    .filter(|pattern| pattern.dimension() == dimension)
                {
                    if buttons::midi_button(
                        ui,
                        &format!("generator.{}.pattern.{}", deck.index(), pattern.id()),
                        &format!("Deck {} · Generator {}", deck.label(), pattern.label()),
                        |ui| {
                            pattern_tile(ui, pattern, settings.pattern == pattern, palette, accent)
                        },
                    )
                    .clicked()
                    {
                        settings.pattern = pattern;
                    }
                }
            });
        }
    }
    ui.add_space(6.0);

    for (heading, ids) in [
        ("SHAPE", vec![0, 1, 2, 3, 4, 43, 45]),
        ("POLYNOMIAL & SURFACE", (22..32).collect()),
        ("RECURSIVE OBJECT ECHOES", (32..39).collect()),
        ("LINE TRACING", (39..43).collect()),
        ("MOTION & CAMERA", (5..12).collect()),
        ("COLOR & LIGHT", (12..21).chain([44]).collect()),
    ] {
        section(ui, heading, palette, |ui| {
            egui::Grid::new(("generator-parameters", deck.index(), heading))
                .num_columns(2)
                .show(ui, |ui| {
                    for (index, id) in ids.into_iter().enumerate() {
                        parameter_slider(ui, settings, deck, id, midi_map, actions);
                        if index % 2 == 1 {
                            ui.end_row();
                        }
                    }
                });
        });
    }
    {
        ui.horizontal_wrapped(|ui| {
            if buttons::midi_button(
                ui,
                &format!("generator.{}.traced_sculpture", deck.index()),
                &format!("Deck {} · Generator Traced sculpture", deck.label()),
                |ui| ui.button("Traced sculpture"),
            )
            .clicked()
            {
                settings.echo_copies = 8.0;
                settings.echo_scale = 0.9;
                settings.trace_heads = 3.0;
                settings.trace_length = 0.35;
                settings.trace_speed = 0.15;
                settings.trace_spread = 1.0;
                settings.trails = 0.65;
            }
            if buttons::midi_button(
                ui,
                &format!("generator.{}.full_wireframe", deck.index()),
                &format!("Deck {} · Generator Full wireframe", deck.label()),
                |ui| ui.button("Full wireframe"),
            )
            .clicked()
            {
                settings.trace_length = 1.0;
                settings.trace_speed = 0.0;
            }
            if buttons::midi_button(
                ui,
                &format!("generator.{}.stop_motion", deck.index()),
                &format!("Deck {} · Generator Stop motion", deck.label()),
                |ui| ui.button("Stop motion"),
            )
            .clicked()
            {
                settings.rotate_speed = 0.0;
                settings.spin_speed = 0.0;
                settings.grow_speed = 0.0;
                settings.trace_speed = 0.0;
            }
            if buttons::midi_button(
                ui,
                &format!("generator.{}.reset_camera", deck.index()),
                &format!("Deck {} · Generator Reset camera", deck.label()),
                |ui| ui.button("Reset camera controls"),
            )
            .clicked()
            {
                settings.tilt = 0.0;
                settings.zoom = 1.0;
                settings.perspective = 0.5;
            }
            if buttons::midi_button(
                ui,
                &format!("generator.{}.new_seed", deck.index()),
                &format!("Deck {} · Generator New seed", deck.label()),
                |ui| ui.button("New seed"),
            )
            .clicked()
            {
                settings.seed = settings
                    .seed
                    .wrapping_mul(1_664_525)
                    .wrapping_add(1_013_904_223);
            }
        });
        ui.horizontal_wrapped(|ui| {
            for (name, rotate, spin, grow) in [
                ("Orbit", 0.15, 0.0, 0.0),
                ("Spin", 0.0, 0.15, 0.0),
                ("Grow", 0.0, 0.0, 0.25),
            ] {
                if buttons::midi_button(
                    ui,
                    &format!("generator.{}.motion.{name}", deck.index()),
                    &format!("Deck {} · Generator {name}", deck.label()),
                    |ui| ui.button(name),
                )
                .clicked()
                {
                    settings.rotate_speed = rotate;
                    settings.spin_speed = spin;
                    settings.grow_speed = grow;
                }
            }
            for (name, hue, range, saturation) in [
                ("Neon", 0.75, 0.45, 0.9),
                ("Fire", 0.0, 0.15, 0.95),
                ("Ice", 0.48, 0.18, 0.7),
                ("Mono", 0.0, 0.0, 0.0),
            ] {
                if buttons::midi_button(
                    ui,
                    &format!("generator.{}.color.{name}", deck.index()),
                    &format!("Deck {} · Generator {name}", deck.label()),
                    |ui| ui.button(name),
                )
                .clicked()
                {
                    settings.hue = hue;
                    settings.hue_range = range;
                    settings.saturation = saturation;
                    settings.color_speed = 0.0;
                }
            }
            ui.add(egui::DragValue::new(&mut settings.seed).prefix("Seed "))
                .on_hover_text("The same seed always gives the same shape");
        });
    }
    ui.columns(2, |columns| {
        section(&mut columns[0], "AUDIO", palette, |ui| {
            parameter_slider(ui, settings, deck, 21, midi_map, actions);
            ui.weak(if settings.pattern.audio_shapes_geometry() {
                "Bass, mid and high bend this pattern's geometry and shift its colour."
            } else {
                "Bass, mid and high shift this pattern's colour and lightness."
            });
        });
        section(&mut columns[1], "OUTPUT", palette, |ui| {
            if !controls_output {
                ui.weak("Output resolution and frame rate are shared with Layer 1.");
                return;
            }
            if show_mode {
                ui.weak("Resolution, frame rate and reset are locked in Show Mode.");
                return;
            }
            ui.horizontal_wrapped(|ui| {
                for (label, extent) in RESOLUTIONS {
                    buttons::midi_select(
                        ui,
                        &format!(
                            "generator.{}.resolution.{}x{}",
                            deck.index(),
                            extent[0],
                            extent[1]
                        ),
                        &format!("Deck {} · Generator {label}", deck.label()),
                        &mut settings.resolution,
                        extent,
                        label,
                    );
                }
            });
            ui.horizontal_wrapped(|ui| {
                for fps in FRAME_RATES {
                    buttons::midi_select(
                        ui,
                        &format!("generator.{}.fps.{fps}", deck.index()),
                        &format!("Deck {} · Generator {fps} fps", deck.label()),
                        &mut settings.fps,
                        fps,
                        format!("{fps} fps"),
                    );
                }
            });
            if buttons::midi_button(
                ui,
                &format!("generator.{}.reset_all", deck.index()),
                &format!("Deck {} · Generator Reset all controls", deck.label()),
                |ui| ui.button("Reset all generator controls"),
            )
            .clicked()
            {
                *settings = GeneratorSettings {
                    pattern: settings.pattern,
                    seed: settings.seed,
                    ..GeneratorSettings::default()
                };
            }
        });
    });
    *settings = settings.clone().sanitized();
}

fn draw_geometry_lab(
    ui: &mut egui::Ui,
    settings: &mut GeneratorSettings,
    calculator: &mut String,
    library: &mut Vec<GeometryGraph>,
) {
    let mut clear_graph = false;
    ui.collapsing("GEOMETRY LAB · node graph & calculator", |ui| {
        if settings.geometry_graph.is_none() {
            if ui.button("Create a custom geometry graph").clicked() {
                settings.geometry_graph = Some(GeometryGraph::default());
            }
            ui.weak("Build a 2D curve or 3D surface, then connect geometry mutations.");
        }
        let Some(graph) = &mut settings.geometry_graph else {
            return;
        };
        let mut changed = false;
        ui.horizontal(|ui| {
            ui.label("Definition");
            changed |= ui.text_edit_singleline(&mut graph.name).changed();
            if ui.button("Use built-in pattern").clicked() {
                clear_graph = true;
            }
            if ui.button("Save definition").clicked()
                && virtual_generate::validate_geometry_graph(graph).is_ok()
            {
                if let Some(existing) = library.iter_mut().find(|saved| saved.name == graph.name) {
                    *existing = graph.clone();
                } else if library.len() < 128 {
                    library.push(graph.clone());
                }
            }
        });
        if !library.is_empty() {
            ui.horizontal(|ui| {
                ui.label("Saved geometry");
                egui::ComboBox::from_id_salt("geometry-graph-library")
                    .selected_text("Load saved…")
                    .show_ui(ui, |ui| {
                        for saved in library.iter() {
                            if ui.selectable_label(false, &saved.name).clicked() {
                                *graph = saved.clone();
                                changed = true;
                            }
                        }
                    });
            });
        }
        ui.collapsing("Exposed parameters", |ui| {
            for parameter in &mut graph.parameters {
                ui.horizontal(|ui| {
                    changed |= ui.text_edit_singleline(&mut parameter.name).changed();
                    changed |= ui
                        .add(
                            egui::DragValue::new(&mut parameter.default)
                                .speed(0.01)
                                .prefix("Default "),
                        )
                        .changed();
                    changed |= ui
                        .add(
                            egui::DragValue::new(&mut parameter.min)
                                .speed(0.01)
                                .prefix("Min "),
                        )
                        .changed();
                    changed |= ui
                        .add(
                            egui::DragValue::new(&mut parameter.max)
                                .speed(0.01)
                                .prefix("Max "),
                        )
                        .changed();
                });
            }
            if graph.parameters.len() < 32 && ui.button("+ Add parameter").clicked() {
                let index = graph.parameters.len() + 1;
                graph.parameters.push(GeometryParameter {
                    name: format!("p{index}"),
                    default: 0.5,
                    min: 0.0,
                    max: 1.0,
                });
                changed = true;
            }
        });
        ui.horizontal_wrapped(|ui| {
            for (label, kind) in [
                (
                    "Curve",
                    GeometryNodeKind::Curve {
                        x: "cos(tau*t)".into(),
                        y: "sin(tau*t)".into(),
                        z: "0".into(),
                        steps: 256,
                        closed: true,
                    },
                ),
                (
                    "3D Surface",
                    GeometryNodeKind::Surface {
                        x: "cos(tau*u)*cos(tau*v)".into(),
                        y: "sin(tau*u)*cos(tau*v)".into(),
                        z: "sin(tau*v)".into(),
                        u_steps: 64,
                        v_steps: 32,
                    },
                ),
                (
                    "Transform",
                    GeometryNodeKind::Transform {
                        translate: ["0".into(), "0".into(), "0".into()],
                        rotate: ["0".into(), "0".into(), "0".into()],
                        scale: ["1".into(), "1".into(), "1".into()],
                    },
                ),
                (
                    "Twist",
                    GeometryNodeKind::Twist {
                        amount: "0.5".into(),
                    },
                ),
                (
                    "Radial Repeat",
                    GeometryNodeKind::RadialRepeat {
                        copies: 6,
                        axis: GraphAxis::Z,
                    },
                ),
                (
                    "Noise",
                    GeometryNodeKind::Noise {
                        amount: "0.05".into(),
                        frequency: "3".into(),
                        seed: 1,
                    },
                ),
                ("Merge", GeometryNodeKind::Merge),
            ] {
                if ui.button(format!("+ {label}")).clicked() {
                    graph_add_node(graph, kind);
                    changed = true;
                }
            }
        });
        ui.separator();
        ui.label("NODE CANVAS · drag nodes; choose exact connections in each node inspector");
        changed |= draw_graph_canvas(ui, graph);
        let ids: Vec<_> = graph.nodes.iter().map(|n| n.id).collect();
        for node in &mut graph.nodes {
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.strong(format!("{} · {}", node.name, graph_node_label(&node.kind)));
                    if ui
                        .button(if graph.output == node.id {
                            "● Output"
                        } else {
                            "Set output"
                        })
                        .clicked()
                    {
                        graph.output = node.id;
                        changed = true;
                    }
                });
                for input_index in 0..node.inputs.len() {
                    let mut selected = node.inputs[input_index].0;
                    egui::ComboBox::from_id_salt(("geometry-input", node.id.0, input_index))
                        .selected_text(format!("Input {}: node {}", input_index + 1, selected))
                        .show_ui(ui, |ui| {
                            for id in &ids {
                                if id != &node.id
                                    && ui
                                        .selectable_value(
                                            &mut selected,
                                            id.0,
                                            format!("Node {}", id.0),
                                        )
                                        .clicked()
                                {}
                            }
                        });
                    if node.inputs[input_index].0 != selected {
                        node.inputs[input_index] = GraphNodeId(selected);
                        changed = true;
                    }
                }
                if matches!(node.kind, GeometryNodeKind::Merge)
                    && node.inputs.len() < 8
                    && ui.button("+ Input").clicked()
                {
                    if let Some(candidate) = ids
                        .iter()
                        .copied()
                        .find(|id| *id != node.id && !node.inputs.contains(id))
                    {
                        node.inputs.push(candidate);
                        changed = true;
                    }
                }
                changed |= draw_node_controls(ui, node);
            });
        }
        ui.separator();
        ui.horizontal(|ui| {
            ui.label("Calculator");
            changed |= ui
                .add(
                    egui::TextEdit::singleline(calculator)
                        .desired_width(320.0)
                        .hint_text("e.g. sin(tau*t) * 0.8"),
                )
                .changed();
            if ui.button("Test").clicked() {}
        });
        let sample = CalculatorValue {
            values: std::collections::HashMap::from([
                ("t".into(), 0.25),
                ("u".into(), 0.25),
                ("v".into(), 0.5),
                ("x".into(), 0.5),
                ("y".into(), -0.25),
                ("z".into(), 0.0),
                ("time".into(), 1.0),
                ("bass".into(), 0.5),
                ("mid".into(), 0.25),
                ("high".into(), 0.75),
            ]),
        };
        match CalculatorExpression::compile(calculator).and_then(|expr| expr.evaluate(&sample)) {
            Ok(value) => {
                ui.label(format!("Result at t=.25, x=.5, y=-.25, time=1: {value:.5}"));
            }
            Err(error) => {
                ui.colored_label(egui::Color32::LIGHT_RED, error.to_string());
            }
        }
        match virtual_generate::validate_geometry_graph(graph) {
            Ok(()) => {
                ui.colored_label(
                    egui::Color32::LIGHT_GREEN,
                    format!("Valid graph · {} nodes", graph.nodes.len()),
                );
            }
            Err(error) => {
                ui.colored_label(egui::Color32::LIGHT_RED, error.to_string());
            }
        }
        if changed {
            graph.revision = graph.revision.wrapping_add(1).max(1);
        }
    });
    if clear_graph {
        settings.geometry_graph = None;
    }
}

fn graph_add_node(graph: &mut GeometryGraph, kind: GeometryNodeKind) {
    if graph.nodes.len() >= virtual_generate::MAX_GEOMETRY_NODES {
        return;
    }
    let id = graph
        .nodes
        .iter()
        .map(|n| n.id.0)
        .max()
        .unwrap_or(0)
        .wrapping_add(1)
        .max(1);
    let output = graph
        .nodes
        .iter()
        .find(|n| n.id == graph.output)
        .map(|n| n.id);
    let input = output.and_then(|out| {
        graph
            .nodes
            .iter()
            .find(|n| n.id == out)
            .and_then(|n| n.inputs.first().copied())
    });
    let input = if matches!(
        kind,
        GeometryNodeKind::Curve { .. } | GeometryNodeKind::Surface { .. }
    ) {
        None
    } else {
        input
    };
    let node = GeometryNode {
        id: GraphNodeId(id),
        name: format!("{} {id}", graph_node_label(&kind)),
        position: [
            40.0 + ((id.saturating_sub(1) % 4) as f32 * 190.0),
            40.0 + (((id.saturating_sub(1) / 4) % 3) as f32 * 100.0),
        ],
        inputs: input.into_iter().collect(),
        kind,
    };
    graph.nodes.push(node);
    if let Some(output) = graph.nodes.iter_mut().find(|n| n.id == graph.output) {
        output.inputs = vec![GraphNodeId(id)];
    }
}

fn draw_graph_canvas(ui: &mut egui::Ui, graph: &mut GeometryGraph) -> bool {
    let content_width = ui.available_width().max(760.0);
    let content_height = graph
        .nodes
        .iter()
        .map(|node| node.position[1] + 90.0)
        .fold(280.0, f32::max);
    let mut changed = false;
    egui::ScrollArea::both().max_height(300.0).show(ui, |ui| {
        let (rect, _) = ui.allocate_exact_size(
            egui::vec2(content_width, content_height),
            egui::Sense::hover(),
        );
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 5.0, ui.visuals().extreme_bg_color);
        let positions: std::collections::HashMap<_, _> = graph
            .nodes
            .iter()
            .map(|node| (node.id, node.position))
            .collect();
        for node in &graph.nodes {
            let target = rect.min + egui::vec2(node.position[0], node.position[1] + 27.0);
            for input in &node.inputs {
                if let Some(position) = positions.get(input) {
                    let start = rect.min + egui::vec2(position[0] + 170.0, position[1] + 27.0);
                    painter.line_segment(
                        [start, target],
                        egui::Stroke::new(2.0, ui.visuals().selection.stroke.color),
                    );
                }
            }
        }
        for node in &mut graph.nodes {
            let max_x = (content_width - 180.0).max(0.0);
            let max_y = (content_height - 70.0).max(0.0);
            node.position[0] = node.position[0].clamp(0.0, max_x);
            node.position[1] = node.position[1].clamp(0.0, max_y);
            let node_rect = egui::Rect::from_min_size(
                rect.min + egui::vec2(node.position[0], node.position[1]),
                egui::vec2(170.0, 54.0),
            );
            let response = ui.interact(
                node_rect,
                ui.make_persistent_id(("geometry-node-canvas", node.id.0)),
                egui::Sense::drag(),
            );
            if response.dragged() {
                let delta = response.drag_delta();
                node.position[0] = (node.position[0] + delta.x).clamp(0.0, max_x);
                node.position[1] = (node.position[1] + delta.y).clamp(0.0, max_y);
                changed = true;
            }
            let color = if graph.output == node.id {
                egui::Color32::from_rgb(40, 100, 75)
            } else {
                ui.visuals().window_fill
            };
            painter.rect_filled(node_rect, 5.0, color);
            painter.rect_stroke(
                node_rect,
                5.0,
                ui.visuals().widgets.noninteractive.bg_stroke,
                egui::StrokeKind::Inside,
            );
            painter.text(
                node_rect.center(),
                egui::Align2::CENTER_CENTER,
                format!("{}\n{}", node.name, graph_node_label(&node.kind)),
                egui::FontId::proportional(13.0),
                ui.visuals().text_color(),
            );
            if response.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
            }
        }
    });
    changed
}

fn graph_node_label(kind: &GeometryNodeKind) -> &'static str {
    match kind {
        GeometryNodeKind::Curve { .. } => "Curve",
        GeometryNodeKind::Surface { .. } => "Surface",
        GeometryNodeKind::Transform { .. } => "Transform",
        GeometryNodeKind::Twist { .. } => "Twist",
        GeometryNodeKind::RadialRepeat { .. } => "Repeat",
        GeometryNodeKind::Noise { .. } => "Noise",
        GeometryNodeKind::Merge => "Merge",
        GeometryNodeKind::Output => "Output",
    }
}

fn draw_node_controls(ui: &mut egui::Ui, node: &mut GeometryNode) -> bool {
    let mut changed = false;
    match &mut node.kind {
        GeometryNodeKind::Curve {
            x,
            y,
            z,
            steps,
            closed,
        } => {
            ui.horizontal(|ui| {
                ui.label("x(t)");
                changed |= ui.text_edit_singleline(x).changed();
                ui.label("y(t)");
                changed |= ui.text_edit_singleline(y).changed();
                ui.label("z(t)");
                changed |= ui.text_edit_singleline(z).changed();
            });
            changed |= ui
                .add(egui::Slider::new(steps, 2..=2048).text("Samples"))
                .changed();
            changed |= ui.checkbox(closed, "Closed curve").changed();
        }
        GeometryNodeKind::Surface {
            x,
            y,
            z,
            u_steps,
            v_steps,
        } => {
            ui.horizontal(|ui| {
                ui.label("x(u,v)");
                changed |= ui.text_edit_singleline(x).changed();
                ui.label("y(u,v)");
                changed |= ui.text_edit_singleline(y).changed();
                ui.label("z(u,v)");
                changed |= ui.text_edit_singleline(z).changed();
            });
            changed |= ui
                .add(egui::Slider::new(u_steps, 2..=256).text("U grid"))
                .changed();
            changed |= ui
                .add(egui::Slider::new(v_steps, 2..=256).text("V grid"))
                .changed();
        }
        GeometryNodeKind::Transform {
            translate,
            rotate,
            scale,
        } => {
            for (label, values) in [
                ("Translate", translate),
                ("Rotate · radians", rotate),
                ("Scale", scale),
            ] {
                ui.horizontal(|ui| {
                    ui.label(label);
                    for expr in values {
                        changed |= ui
                            .add(egui::TextEdit::singleline(expr).desired_width(75.0))
                            .changed();
                    }
                });
            }
        }
        GeometryNodeKind::Twist { amount } => {
            ui.horizontal(|ui| {
                ui.label("Twist amount(x,y,z,t,time)");
                changed |= ui.text_edit_singleline(amount).changed();
            });
        }
        GeometryNodeKind::RadialRepeat { copies, axis } => {
            ui.horizontal(|ui| {
                changed |= ui
                    .add(egui::Slider::new(copies, 1..=32).text("Copies"))
                    .changed();
                egui::ComboBox::from_id_salt(("repeat-axis", node.id.0))
                    .selected_text(format!("{axis:?}"))
                    .show_ui(ui, |ui| {
                        changed |= ui.selectable_value(axis, GraphAxis::X, "X").changed();
                        changed |= ui.selectable_value(axis, GraphAxis::Y, "Y").changed();
                        changed |= ui.selectable_value(axis, GraphAxis::Z, "Z").changed();
                    });
            });
        }
        GeometryNodeKind::Noise {
            amount,
            frequency,
            seed,
        } => {
            ui.horizontal(|ui| {
                ui.label("Amount");
                changed |= ui.text_edit_singleline(amount).changed();
                ui.label("Frequency");
                changed |= ui.text_edit_singleline(frequency).changed();
                changed |= ui.add(egui::DragValue::new(seed).prefix("Seed ")).changed();
            });
        }
        GeometryNodeKind::Merge | GeometryNodeKind::Output => {}
    }
    changed
}
