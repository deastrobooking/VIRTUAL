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
use super::{UiAction, UiState};

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
            if ui
                .button(format!("Load to Deck {}", deck.label()))
                .on_hover_text(
                    "Replaces this deck's clip or video input with a live generator \
                     and opens its controls",
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
    });
}

/// Deck-strip summary for a generator source: what is running and a
/// toggle for its window. No parameters are edited here.
pub(super) fn draw_generator_summary(
    ui: &mut egui::Ui,
    settings: &GeneratorSettings,
    stats: GeneratorStats,
    palette: ThemePalette,
    window_open: &mut bool,
) {
    ui.horizontal_wrapped(|ui| {
        ui.colored_label(palette.success, "◆ GENERATOR");
        ui.strong(settings.pattern.label());
        if ui
            .selectable_label(*window_open, "Generator controls…")
            .on_hover_text("Show or hide this deck's generator window")
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
            draw_window_body(ui, deck, settings, stats[index], palette, state.show_mode);
        });
        state.generator_windows[index] = open;
    }
}

fn slider(
    ui: &mut egui::Ui,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    label: &str,
) -> egui::Response {
    ui.add(
        egui::Slider::new(value, range)
            .text(label)
            .clamping(egui::SliderClamping::Always),
    )
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

fn draw_window_body(
    ui: &mut egui::Ui,
    deck: DeckId,
    settings: &mut GeneratorSettings,
    stats: GeneratorStats,
    palette: ThemePalette,
    show_mode: bool,
) {
    let accent = palette.deck_color(deck);
    ui.horizontal_wrapped(|ui| {
        ui.heading(settings.pattern.label());
        ui.weak(settings.pattern.hint());
    });
    stats_line(ui, settings, stats, palette);
    ui.separator();

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
                if pattern_tile(ui, pattern, settings.pattern == pattern, palette, accent).clicked()
                {
                    settings.pattern = pattern;
                }
            }
        });
    }
    ui.add_space(6.0);

    ui.columns(3, |columns| {
        section(&mut columns[0], "SHAPE", palette, |ui| {
            let mut levels = settings.depth_levels();
            if ui
                .add(egui::Slider::new(&mut levels, 3..=11).text("depth"))
                .changed()
            {
                settings.depth = (levels - 3) as f32 / 8.0;
            }
            slider(ui, &mut settings.scale, 0.0..=1.0, "scale");
            slider(ui, &mut settings.spread, 0.0..=1.0, "spread");
            slider(ui, &mut settings.twist, 0.0..=1.0, "twist");
            slider(ui, &mut settings.randomness, 0.0..=1.0, "randomness");
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut settings.seed).prefix("seed "))
                    .on_hover_text("The same seed always gives the same shape");
                if ui.button("New seed").clicked() {
                    settings.seed = settings
                        .seed
                        .wrapping_mul(1_664_525)
                        .wrapping_add(1_013_904_223);
                }
            });
            ui.checkbox(&mut settings.flatten, "Flatten to 2D")
                .on_hover_text("Projects 3D patterns onto the flat XY plane");
        });
        section(&mut columns[1], "MOTION & CAMERA", palette, |ui| {
            slider(ui, &mut settings.rotate_speed, -1.0..=1.0, "rotate");
            slider(ui, &mut settings.tilt, -1.0..=1.0, "tilt");
            slider(ui, &mut settings.spin_speed, -1.0..=1.0, "spin");
            ui.add(
                egui::Slider::new(&mut settings.zoom, 0.25..=4.0)
                    .text("zoom")
                    .logarithmic(true),
            );
            slider(ui, &mut settings.perspective, 0.0..=1.0, "perspective");
            slider(ui, &mut settings.reveal, 0.0..=1.0, "reveal");
            slider(ui, &mut settings.grow_speed, 0.0..=1.0, "grow loop")
                .on_hover_text("Above zero, the reveal replays from trunk to tips");
            if ui.button("Stop motion").clicked() {
                settings.rotate_speed = 0.0;
                settings.spin_speed = 0.0;
                settings.grow_speed = 0.0;
            }
        });
        section(&mut columns[2], "COLOR & LIGHT", palette, |ui| {
            slider(ui, &mut settings.hue, 0.0..=1.0, "hue");
            slider(ui, &mut settings.hue_range, 0.0..=1.0, "hue range");
            slider(ui, &mut settings.color_speed, 0.0..=1.0, "hue drift");
            slider(ui, &mut settings.saturation, 0.0..=1.0, "saturation");
            slider(ui, &mut settings.lightness, 0.0..=1.0, "lightness");
            slider(ui, &mut settings.brightness, 0.0..=4.0, "brightness");
            slider(ui, &mut settings.line_width, 0.5..=6.0, "line width");
            slider(ui, &mut settings.depth_fade, 0.0..=1.0, "depth fade");
            slider(ui, &mut settings.trails, 0.0..=0.97, "trails");
            ui.checkbox(&mut settings.transparent, "Transparent background")
                .on_hover_text("Lines layer over lower decks; off draws on black");
        });
    });
    ui.add_space(4.0);

    ui.columns(2, |columns| {
        section(&mut columns[0], "AUDIO", palette, |ui| {
            slider(ui, &mut settings.audio_amount, 0.0..=1.0, "audio amount");
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
                    ui.selectable_value(&mut settings.resolution, extent, label);
                }
            });
            ui.horizontal_wrapped(|ui| {
                for fps in FRAME_RATES {
                    ui.selectable_value(&mut settings.fps, fps, format!("{fps} fps"));
                }
            });
            if ui.button("Reset all generator controls").clicked() {
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
