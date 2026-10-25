//! Perform, Edit and Build presets with saved per-project overrides.
use super::{
    UiState, panel,
    theme::{ClipSize, ClipView, Density},
};
use virtual_io::WorkspaceLayoutProject;

pub(super) const NAMES: [&str; 3] = ["Perform", "Edit", "Build"];
fn preset(name: &str) -> WorkspaceLayoutProject {
    match name {
        "Perform" => WorkspaceLayoutProject {
            browser: false,
            ..Default::default()
        },
        "Edit" => WorkspaceLayoutProject {
            sections: [false, true, false, true, false, false],
            timeline: true,
            ..Default::default()
        },
        "Build" => WorkspaceLayoutProject {
            sections: [false, false, true, true, false, true],
            unit_window: true,
            ..Default::default()
        },
        _ => WorkspaceLayoutProject::default(),
    }
}
fn apply(ctx: &egui::Context, state: &mut UiState, layout: WorkspaceLayoutProject) {
    for (id, open) in panel::SECTIONS.into_iter().zip(layout.sections) {
        panel::set_open(ctx, id, open);
    }
    state.browser.open = layout.browser;
    state.theme.density = Density::from_name(&layout.density).unwrap_or_default();
    state.theme.clip_view = ClipView::from_name(&layout.clip_view).unwrap_or_default();
    state.theme.clip_size = ClipSize::from_name(&layout.clip_size).unwrap_or_default();
    state.deck_window = layout.unit_window;
    state.unit_pinned = layout
        .pinned_unit
        .and_then(|index| virtual_media::DeckId::ALL.get(index as usize).copied());
    state.timeline.open = layout.timeline;
    for (id, width) in [
        ("browser-panel", layout.browser_width),
        ("properties-panel", layout.properties_width),
    ] {
        ctx.data_mut(|data| {
            data.insert_persisted(
                egui::Id::new(id),
                egui::containers::panel::PanelState {
                    outer_rect: egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(width, 600.0),
                    ),
                },
            )
        });
    }
}
pub(super) fn restore(ctx: &egui::Context, state: &mut UiState) {
    if !state.workspace_restore_pending {
        return;
    }
    state.workspace_restore_pending = false;
    let name = state.workspaces.active.clone();
    if !name.is_empty() {
        activate(ctx, state, &name);
    }
}
pub(super) fn activate(ctx: &egui::Context, state: &mut UiState, name: &str) {
    let layout = state
        .workspaces
        .layouts
        .get(name)
        .cloned()
        .unwrap_or_else(|| preset(name));
    apply(ctx, state, layout);
    state.workspaces.active = name.to_owned();
    state.navigation = match name {
        "Edit" => super::navigation::Navigation::Timeline,
        "Build" => super::navigation::Navigation::Deck,
        _ => super::navigation::Navigation::Clips,
    };
}
pub(super) fn save(ctx: &egui::Context, state: &mut UiState, name: &str) {
    let sections = panel::SECTIONS.map(|id| panel::is_open(ctx, id));
    let panel_width = |id, fallback: f32, min, max| {
        egui::containers::panel::PanelState::load(ctx, egui::Id::new(id))
            .map_or(fallback, |panel| panel.size().x)
            .clamp(min, max)
    };
    state.workspaces.layouts.insert(
        name.to_owned(),
        WorkspaceLayoutProject {
            sections,
            browser: state.browser.open,
            browser_width: panel_width("browser-panel", 250.0, 180.0, 500.0),
            properties_width: panel_width("properties-panel", 460.0, 340.0, 900.0),
            density: state.theme.density.name().into(),
            clip_view: state.theme.clip_view.name().into(),
            clip_size: state.theme.clip_size.name().into(),
            unit_window: state.deck_window,
            pinned_unit: state.unit_pinned.map(|deck| deck.index() as u8),
            timeline: state.timeline.open,
        },
    );
    state.workspaces.active = name.to_owned();
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn saved_workspaces_restore_selection_pin_density_sections_and_panel_widths() {
        let ctx = egui::Context::default();
        let mut state = UiState {
            deck_window: true,
            unit_pinned: Some(virtual_media::DeckId::C),
            ..Default::default()
        };
        state.theme.density = Density::Roomy;
        state.timeline.open = true;
        panel::set_open(&ctx, "clips", false);
        ctx.data_mut(|data| {
            data.insert_persisted(
                egui::Id::new("browser-panel"),
                egui::containers::panel::PanelState {
                    outer_rect: egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(310.0, 500.0),
                    ),
                },
            )
        });
        save(&ctx, &mut state, "Build");
        let project = virtual_io::ProjectFile {
            workspaces: state.workspaces.clone(),
            ..Default::default()
        };
        project.validate().unwrap();
        let restored: virtual_io::ProjectFile =
            serde_json::from_str(&serde_json::to_string(&project).unwrap()).unwrap();
        let mut state = UiState {
            workspaces: restored.workspaces,
            workspace_restore_pending: true,
            ..Default::default()
        };
        restore(&ctx, &mut state);
        assert_eq!(state.unit_pinned, Some(virtual_media::DeckId::C));
        assert_eq!(state.theme.density, Density::Roomy);
        assert!(state.timeline.open);
        assert!(state.deck_window);
        assert!(!panel::is_open(&ctx, "clips"));
        assert_eq!(
            egui::containers::panel::PanelState::load(&ctx, egui::Id::new("browser-panel"))
                .unwrap()
                .size()
                .x,
            310.0
        );
    }
}
