//! Saved operator workspaces. These are interface state, not render state.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WorkspacesProject {
    pub active: String,
    pub layouts: BTreeMap<String, WorkspaceLayoutProject>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WorkspaceLayoutProject {
    pub sections: [bool; 6],
    pub browser: bool,
    pub browser_width: f32,
    pub properties_width: f32,
    pub density: String,
    pub clip_view: String,
    pub clip_size: String,
    pub unit_window: bool,
    pub pinned_unit: Option<u8>,
    pub timeline: bool,
}
impl Default for WorkspaceLayoutProject {
    fn default() -> Self {
        Self {
            sections: [false, true, false, true, true, false],
            browser: true,
            browser_width: 250.0,
            properties_width: 460.0,
            density: "compact".into(),
            clip_view: "session".into(),
            clip_size: "small".into(),
            unit_window: false,
            pinned_unit: None,
            timeline: false,
        }
    }
}
impl WorkspacesProject {
    pub fn valid(&self) -> bool {
        self.active.len() <= 32
            && !self.active.chars().any(char::is_control)
            && self.layouts.len() <= 16
            && self.layouts.iter().all(|(name, layout)| {
                !name.is_empty()
                    && name.len() <= 32
                    && !name.chars().any(char::is_control)
                    && layout.browser_width.is_finite()
                    && (180.0..=500.0).contains(&layout.browser_width)
                    && layout.properties_width.is_finite()
                    && (340.0..=900.0).contains(&layout.properties_width)
                    && layout.density.len() <= 32
                    && layout.clip_view.len() <= 32
                    && layout.clip_size.len() <= 32
                    && layout.pinned_unit.is_none_or(|deck| deck < 4)
            })
    }
}
