//! One keyboard map for the section rail and browser.
use super::{UiState, browser::BrowserTab, panel, theme::ClipView};
use virtual_media::{ClipAddress, ClipBank, FourDeckMixer};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Navigation {
    Files,
    Sources,
    Effects,
    #[default]
    Clips,
    Deck,
    Mixer,
    Master,
    Audio,
    Setup,
    Timeline,
}

impl Navigation {
    const ALL: [(Self, egui::Key, &str); 10] = [
        (Self::Files, egui::Key::F1, "F1  Files"),
        (Self::Sources, egui::Key::F2, "F2  Generators"),
        (Self::Effects, egui::Key::F3, "F3  Deck FX"),
        (Self::Clips, egui::Key::F4, "F4  Clips / Session"),
        (Self::Deck, egui::Key::F5, "F5  Deck UNIT"),
        (Self::Mixer, egui::Key::F6, "F6  Video mixer"),
        (Self::Master, egui::Key::F7, "F7  Master FX"),
        (Self::Audio, egui::Key::F8, "F8  Audio"),
        (Self::Setup, egui::Key::F9, "F9  Setup"),
        (Self::Timeline, egui::Key::F10, "F10 Timeline / Takes"),
    ];

    fn activate(self, ctx: &egui::Context, state: &mut UiState) {
        if self == Self::Setup && state.show_mode {
            return;
        }
        state.navigation = self;
        let section = match self {
            Self::Files | Self::Sources | Self::Effects => {
                state.browser.open = true;
                state.browser.tab = match self {
                    Self::Files => BrowserTab::Files,
                    Self::Sources => BrowserTab::Sources,
                    _ => BrowserTab::Effects,
                };
                if self == Self::Effects {
                    Some("selected-deck")
                } else {
                    None
                }
            }
            Self::Clips => Some("clips"),
            Self::Deck => Some("selected-deck"),
            Self::Mixer => Some("mix-master"),
            Self::Master => Some("master-effects"),
            Self::Audio => Some("audio-spectrum"),
            Self::Setup => Some("setup"),
            Self::Timeline => {
                state.timeline.open = true;
                None
            }
        };
        if let Some(section) = section {
            panel::set_open(ctx, section, true);
            ctx.data_mut(|data| {
                data.insert_temp(egui::Id::new("navigate-section"), section.to_owned())
            });
        }
    }
}

pub(super) fn shortcuts(ctx: &egui::Context, state: &mut UiState) {
    if ctx.text_edit_focused() {
        return;
    }
    for (target, key, _) in Navigation::ALL {
        if ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, key)) {
            target.activate(ctx, state);
        }
    }
}

pub(super) fn draw(
    ui: &mut egui::Ui,
    state: &mut UiState,
    mixer: &FourDeckMixer,
    clips: &mut ClipBank,
) {
    egui::Panel::left("section-rail")
        .exact_size(146.0)
        .show(ui, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("section-rail-scroll")
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(2.0, 3.0);
                    ui.weak("WORKSPACE");
                    for (target, _, label) in Navigation::ALL {
                        if ui
                            .add_sized(
                                [138.0, 23.0],
                                egui::Button::new(egui::RichText::new(label).small())
                                    .selected(state.navigation == target),
                            )
                            .clicked()
                        {
                            target.activate(ui.ctx(), state);
                        }
                    }
                    ui.separator();
                    ui.weak(format!("DECK {}", mixer.selected().label()));
                    if ui.small_button("↗ Expand UNIT").clicked() {
                        state.deck_window = true;
                        Navigation::Deck.activate(ui.ctx(), state);
                    }
                    if ui.small_button("✎ Clip editor").clicked() {
                        state.clip_editor = Some(ClipAddress {
                            deck: mixer.selected(),
                            slot: clips.selected(mixer.selected()),
                        });
                        Navigation::Clips.activate(ui.ctx(), state);
                    }
                    ui.separator();
                    ui.weak("CLIP VIEW");
                    for view in ClipView::ALL {
                        if ui
                            .selectable_value(&mut state.theme.clip_view, view, view.label())
                            .clicked()
                        {
                            Navigation::Clips.activate(ui.ctx(), state);
                        }
                    }
                    ui.separator();
                    ui.weak("1–8  Launch scene");
                    ui.weak("B  Blackout");
                    ui.weak("Space  Freeze");
                    ui.weak("O  Output");
                });
        });
}
