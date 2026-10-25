//! Section bars in the style of a media server's panels: a slim tinted bar
//! with a chevron, an icon and a small upper-case title. Clicking the
//! chevron or the title folds the section, so the surface stays condensed.

use super::theme::ThemePalette;

/// Every section, for layouts that fold several at once.
pub(super) const SECTIONS: [&str; 6] = [
    "setup",
    "clips",
    "audio-spectrum",
    "selected-deck",
    "mix-master",
    "master-effects",
];

fn section_id(id_salt: &str) -> egui::Id {
    egui::Id::new(("section", id_salt))
}

/// Opens or folds a section from outside it.
pub(super) fn set_open(ctx: &egui::Context, id_salt: &str, open: bool) {
    let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(
        ctx,
        section_id(id_salt),
        open,
    );
    state.set_open(open);
    state.store(ctx);
}

/// Draws a foldable section. `header` adds compact status widgets to the
/// right of the title; `body` is only built while the section is open.
#[allow(clippy::too_many_arguments)]
pub(super) fn section<R>(
    ui: &mut egui::Ui,
    id_salt: &str,
    icon: &str,
    title: &str,
    palette: ThemePalette,
    default_open: bool,
    header: impl FnOnce(&mut egui::Ui),
    body: impl FnOnce(&mut egui::Ui) -> R,
) -> Option<R> {
    let id = section_id(id_salt);
    let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(
        ui.ctx(),
        id,
        default_open,
    );
    let header_response = egui::Frame::new()
        .fill(palette.surface_tint(palette.accent, 0.06))
        .stroke(egui::Stroke::new(
            palette.outline_width.min(1.0),
            palette.stroke,
        ))
        .corner_radius(3)
        .inner_margin(egui::Margin::symmetric(6, 3))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                state.show_toggle_button(ui, egui::collapsing_header::paint_default_icon);
                ui.label(egui::RichText::new(icon).color(palette.accent));
                let title = ui
                    .add(
                        egui::Label::new(
                            egui::RichText::new(title)
                                .small()
                                .strong()
                                .color(palette.strong_text),
                        )
                        .sense(egui::Sense::click()),
                    )
                    .on_hover_cursor(egui::CursorIcon::PointingHand);
                if title.clicked() {
                    state.toggle(ui);
                }
                ui.add_space(8.0);
                header(ui);
            });
        });
    let requested = ui
        .ctx()
        .data(|data| data.get_temp::<String>(egui::Id::new("navigate-section")));
    if requested.as_deref() == Some(id_salt) {
        state.set_open(true);
        ui.scroll_to_rect(header_response.response.rect, Some(egui::Align::Min));
        ui.ctx()
            .data_mut(|data| data.remove::<String>(egui::Id::new("navigate-section")));
    }
    state
        .show_body_unindented(ui, |ui| {
            ui.add_space(2.0);
            body(ui)
        })
        .map(|response| response.inner)
}
