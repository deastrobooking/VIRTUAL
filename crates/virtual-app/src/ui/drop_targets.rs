//! Where clip cells and deck strips were drawn last frame, so a file dropped
//! from the desktop lands on the slot under the pointer instead of the
//! selected one.

use std::cell::RefCell;

use virtual_media::{ClipAddress, DeckId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DropTarget {
    Clip(ClipAddress),
    /// The deck's own selected slot.
    Deck(DeckId),
}

#[derive(Clone, Copy)]
struct Entry {
    rect: egui::Rect,
    layer: egui::LayerId,
    target: DropTarget,
}

thread_local! {
    static TARGETS: RefCell<Vec<Entry>> = const { RefCell::new(Vec::new()) };
}

pub(super) fn begin_frame() {
    TARGETS.with(|targets| targets.borrow_mut().clear());
}

/// Records a drop zone, clipped to what is visible inside scroll areas.
pub(super) fn register(ui: &egui::Ui, rect: egui::Rect, target: DropTarget) {
    let rect = rect.intersect(ui.clip_rect());
    if rect.is_positive() {
        TARGETS.with(|targets| {
            targets.borrow_mut().push(Entry {
                rect,
                layer: ui.layer_id(),
                target,
            });
        });
    }
}

/// The drop zone under `pos`, ignoring zones covered by another window.
/// Clip cells win over the deck strip or grid area around them.
pub(crate) fn at(ctx: &egui::Context, pos: egui::Pos2) -> Option<DropTarget> {
    let top = ctx.layer_id_at(pos);
    TARGETS.with(|targets| {
        let targets = targets.borrow();
        let visible =
            |entry: &&Entry| entry.rect.contains(pos) && top.is_none_or(|top| top == entry.layer);
        targets
            .iter()
            .filter(visible)
            .find(|entry| matches!(entry.target, DropTarget::Clip(_)))
            .or_else(|| targets.iter().find(visible))
            .map(|entry| entry.target)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clip_cells_win_over_the_deck_strip_around_them() {
        let ctx = egui::Context::default();
        let clip = ClipAddress {
            deck: DeckId::C,
            slot: 1,
        };
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            begin_frame();
            register(
                ui,
                egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(400.0, 200.0)),
                DropTarget::Deck(DeckId::C),
            );
            register(
                ui,
                egui::Rect::from_min_size(egui::pos2(100.0, 50.0), egui::vec2(50.0, 20.0)),
                DropTarget::Clip(clip),
            );
        });
        assert_eq!(
            at(&ctx, egui::pos2(120.0, 60.0)),
            Some(DropTarget::Clip(clip))
        );
        assert_eq!(
            at(&ctx, egui::pos2(10.0, 10.0)),
            Some(DropTarget::Deck(DeckId::C))
        );
        assert_eq!(at(&ctx, egui::pos2(900.0, 900.0)), None);
    }
}
