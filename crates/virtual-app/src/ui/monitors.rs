//! A/B source preparation cards and the actual composition monitor.
use super::{UiAction, UiState};
use virtual_media::{ClipAddress, ClipBank, CrossfadeBus, DeckId, DeckTransport, FourDeckMixer};

pub(super) fn draw(
    ui: &mut egui::Ui,
    state: &mut UiState,
    mixer: &mut FourDeckMixer,
    bank: &mut ClipBank,
    transports: &mut [DeckTransport; 4],
    actions: &mut Vec<UiAction>,
) {
    ui.columns(3, |columns| {
        for (column, bus, label) in [
            (0, CrossfadeBus::Left, "A · SOURCE"),
            (1, CrossfadeBus::Right, "B · SOURCE"),
        ] {
            let ui = &mut columns[column];
            ui.group(|ui| {
                ui.strong(label);
                let candidates: Vec<_> = DeckId::ALL
                    .into_iter()
                    .filter(|d| mixer.deck(*d).bus == bus)
                    .collect();
                let selected = mixer.selected();
                let deck = candidates
                    .iter()
                    .copied()
                    .find(|d| *d == selected)
                    .or_else(|| candidates.first().copied());
                if let Some(deck) = deck {
                    ui.horizontal(|ui| {
                        for candidate in &candidates {
                            if ui
                                .selectable_label(
                                    selected == *candidate,
                                    format!("Layer {}", candidate.label()),
                                )
                                .clicked()
                            {
                                mixer.select(*candidate);
                            }
                        }
                    });
                    let address = ClipAddress {
                        deck,
                        slot: bank.selected(deck),
                    };
                    let size = egui::vec2(ui.available_width().min(480.0), 115.0);
                    if let Some(texture) = state.thumbnail(address, bank.path(address)) {
                        ui.add(
                            egui::Image::new((texture.id(), texture.size_vec2()))
                                .fit_to_exact_size(size)
                                .maintain_aspect_ratio(true),
                        );
                    } else {
                        ui.allocate_space(size);
                    }
                    ui.small("Source poster · not an isolated live bus monitor");
                    let movie = bank.movie(address);
                    ui.label(movie.map_or("Select a source clip", |m| m.display_name.as_str()));
                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled(movie.is_some(), egui::Button::new("Launch selected"))
                            .clicked()
                        {
                            actions.push(UiAction::Launch(address));
                        }
                        let t = &mut transports[deck.index()];
                        if ui
                            .button(if t.playing {
                                "Pause layer"
                            } else {
                                "Play layer"
                            })
                            .clicked()
                        {
                            t.playing = !t.playing;
                        }
                    });
                    let mut playback = bank.playback(address).unwrap_or_default();
                    let before = playback;
                    ui.add_enabled_ui(!state.show_mode && movie.is_some(), |ui| {
                        ui.horizontal(|ui| {
                            ui.label("In s");
                            ui.add(
                                egui::DragValue::new(&mut playback.in_point)
                                    .speed(0.1)
                                    .range(0.0..=21_600.0),
                            );
                            let mut out = playback.out_point.unwrap_or_else(|| {
                                movie
                                    .and_then(|m| m.duration)
                                    .map_or(10.0, |d| d.as_seconds())
                            });
                            ui.label("Out s");
                            if ui
                                .add(
                                    egui::DragValue::new(&mut out)
                                        .speed(0.1)
                                        .range(0.0..=21_600.0),
                                )
                                .changed()
                            {
                                playback.out_point = Some(out);
                            }
                        });
                    });
                    if playback != before {
                        bank.set_playback(address, playback);
                    }
                } else {
                    ui.weak("Assign a layer to this bus in its deck inspector.");
                }
                if ui
                    .button(if column == 0 { "Cut to A" } else { "Cut to B" })
                    .clicked()
                {
                    state.crossfader = column as f32;
                }
            });
        }
        columns[2].group(|ui| {
            ui.strong("PROGRAM · LIVE COMPOSITION");
            let width = ui.available_width().min(480.0);
            if let Some(texture) = state.program_texture {
                let aspect =
                    state.composition_extent[0] as f32 / state.composition_extent[1].max(1) as f32;
                ui.add(
                    egui::Image::new((texture, egui::vec2(width, width / aspect)))
                        .fit_to_exact_size(egui::vec2(width, width / aspect)),
                );
            }
            ui.small(format!(
                "{} × {} · output {}",
                state.composition_extent[0],
                state.composition_extent[1],
                if state.output_enabled {
                    "enabled"
                } else {
                    "disabled"
                }
            ));
            ui.weak("Composition before output mapping / calibration overlays");
        });
    });
}
