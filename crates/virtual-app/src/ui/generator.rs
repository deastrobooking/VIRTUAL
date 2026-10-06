//! Recursive-geometry generator: the source loader and the live deck panel.

use virtual_generate::{
    Dimension, FRAME_RATES, GeneratorSettings, GeneratorStats, RESOLUTIONS, RecursivePattern,
};
use virtual_media::DeckId;

use super::theme::ThemePalette;
use super::{UiAction, UiState};

/// Pattern picker grouped by dimensionality.
fn pattern_combo(ui: &mut egui::Ui, id: &str, pattern: &mut RecursivePattern) -> bool {
    let mut changed = false;
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
                    changed |= ui
                        .selectable_value(pattern, candidate, candidate.label())
                        .on_hover_text(candidate.hint())
                        .changed();
                }
            }
        });
    changed
}

/// Loader shown with the video-input controls for the selected deck.
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
                .on_hover_text("Replaces this deck's clip or video input with a live generator")
                .clicked()
            {
                actions.push(UiAction::ConnectGenerator {
                    deck,
                    settings: GeneratorSettings {
                        pattern: state.generator_pattern,
                        ..GeneratorSettings::default()
                    },
                });
            }
        });
    });
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

/// Live controls for a deck whose source is a generator. Edits apply in
/// place; playback forwards them to the deck's worker.
pub(super) fn draw_generator_controls(
    ui: &mut egui::Ui,
    deck: DeckId,
    settings: &mut GeneratorSettings,
    stats: GeneratorStats,
    palette: ThemePalette,
    show_mode: bool,
) {
    ui.push_id(("generator-controls", deck.index()), |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.colored_label(palette.success, "◆ GENERATOR");
            ui.strong(settings.pattern.label());
        });
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

        ui.horizontal_wrapped(|ui| {
            pattern_combo(ui, "pattern", &mut settings.pattern);
            ui.add(egui::DragValue::new(&mut settings.seed).prefix("seed "))
                .on_hover_text("Seeds randomness; the same seed always gives the same shape");
            if ui.button("New seed").clicked() {
                settings.seed = settings
                    .seed
                    .wrapping_mul(1_664_525)
                    .wrapping_add(1_013_904_223);
            }
        });
        ui.weak(settings.pattern.hint());

        egui::CollapsingHeader::new("Shape")
            .default_open(true)
            .show(ui, |ui| {
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
                ui.checkbox(&mut settings.flatten, "Flatten to 2D")
                    .on_hover_text("Projects 3D patterns onto the flat XY plane");
            });

        egui::CollapsingHeader::new("Motion & camera")
            .default_open(true)
            .show(ui, |ui| {
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

        egui::CollapsingHeader::new("Color & light")
            .default_open(false)
            .show(ui, |ui| {
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

        egui::CollapsingHeader::new("Audio")
            .default_open(false)
            .show(ui, |ui| {
                slider(ui, &mut settings.audio_amount, 0.0..=1.0, "audio amount");
                ui.weak(if settings.pattern.audio_shapes_geometry() {
                    "Bass, mid and high bend this pattern's geometry and shift its colour."
                } else {
                    "Bass, mid and high shift this pattern's colour and lightness."
                });
            });

        if !show_mode {
            egui::CollapsingHeader::new("Output")
                .default_open(false)
                .show(ui, |ui| {
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
                    ui.weak("Rendered on this deck's worker thread at its own resolution.");
                    if ui.button("Reset all generator controls").clicked() {
                        *settings = GeneratorSettings {
                            pattern: settings.pattern,
                            seed: settings.seed,
                            ..GeneratorSettings::default()
                        };
                    }
                });
        }
        *settings = settings.clone().sanitized();
    });
}
