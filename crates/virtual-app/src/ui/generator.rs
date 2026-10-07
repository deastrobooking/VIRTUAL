//! Recursive-geometry generator UI.
//!
//! Generators are a deck *source*, not an effect, so their controls live in
//! their own window per deck rather than in the deck's effect editor. The
//! window opens when a generator becomes a deck's source and closes when the
//! deck switches to anything else; the deck strip keeps a one-line summary
//! with a button to reopen it.

use virtual_generate::{
    Dimension, FRAME_RATES, GeneratorSettings, GeneratorStats, RESOLUTIONS, RecursivePattern,
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
    settings: &GeneratorSettings,
    stats: GeneratorStats,
    palette: ThemePalette,
    window_open: &mut bool,
) {
    ui.horizontal_wrapped(|ui| {
        ui.colored_label(palette.success, "◆ GENERATOR");
        ui.strong(settings.pattern.label());
        if buttons::midi_button(
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
            draw_window_body(
                ui,
                deck,
                settings,
                stats[index],
                palette,
                state.show_mode,
                midi_map,
                actions,
            );
        });
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
    midi_map: &super::MidiMapUi,
    actions: &mut Vec<UiAction>,
) {
    let accent = palette.deck_color(deck);
    ui.horizontal_wrapped(|ui| {
        ui.heading(settings.pattern.label());
        ui.weak(settings.pattern.hint());
    });
    stats_line(ui, settings, stats, palette);
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
