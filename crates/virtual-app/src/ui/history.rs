//! Bounded edit transactions shared by clip and timeline editors.
const LIMIT: usize = 64;

pub struct EditHistory<T> {
    undo: Vec<T>,
    redo: Vec<T>,
    gesture: Option<T>,
}
impl<T> Default for EditHistory<T> {
    fn default() -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            gesture: None,
        }
    }
}
impl<T: Clone + PartialEq> EditHistory<T> {
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    fn push(&mut self, before: T) {
        if self.undo.len() == LIMIT {
            self.undo.remove(0);
        }
        self.undo.push(before);
        self.redo.clear();
    }
    /// A held pointer groups all changes until release. A click or keyboard
    /// edit commits immediately. No-change gestures create no history entry.
    pub fn observe(&mut self, before: T, after: &T, interacting: bool) {
        if before != *after && self.gesture.is_none() {
            self.gesture = Some(before);
        }
        if !interacting {
            self.commit(after);
        }
    }
    pub fn commit(&mut self, current: &T) {
        if let Some(before) = self.gesture.take()
            && before != *current
        {
            self.push(before);
        }
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty() || self.gesture.is_some()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    pub fn undo(&mut self, current: &mut T) {
        self.commit(current);
        if let Some(previous) = self.undo.pop() {
            self.redo.push(std::mem::replace(current, previous));
        }
    }
    pub fn redo(&mut self, current: &mut T) {
        self.commit(current);
        if let Some(next) = self.redo.pop() {
            self.undo.push(std::mem::replace(current, next));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_long_drag_is_one_transaction_and_redo_restores_its_endpoint() {
        let mut history = EditHistory::default();
        let mut value = 0;
        for next in 1..=100 {
            let before = value;
            value = next;
            history.observe(before, &value, true);
        }
        history.observe(value, &value, false);
        history.undo(&mut value);
        assert_eq!(value, 0);
        assert!(!history.can_undo());
        history.redo(&mut value);
        assert_eq!(value, 100);
    }
    #[test]
    fn returning_to_the_start_creates_no_undo_and_new_edits_discard_redo() {
        let mut history = EditHistory::default();
        history.observe(0, &1, true);
        history.observe(1, &0, false);
        assert!(!history.can_undo());
        let mut value = 1;
        history.observe(0, &value, false);
        history.undo(&mut value);
        value = 2;
        history.observe(0, &value, false);
        assert!(!history.can_redo());
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum HistoryAction {
    Undo,
    Redo,
}
pub fn shortcut(ui: &egui::Ui) -> Option<HistoryAction> {
    if ui.ctx().text_edit_focused() || ui.ctx().top_layer_id() != Some(ui.layer_id()) {
        return None;
    }
    ui.input_mut(|input| {
        if input.consume_key(
            egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
            egui::Key::Z,
        ) {
            Some(HistoryAction::Redo)
        } else if input.consume_key(egui::Modifiers::COMMAND, egui::Key::Z) {
            Some(HistoryAction::Undo)
        } else {
            None
        }
    })
}
