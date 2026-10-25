//! The left-hand browser: media files, generator sources and deck effects,
//! each draggable onto a clip slot (or clicked to use the selected one),
//! like a media server's Files / Sources / Effects tabs.

use std::path::{Path, PathBuf};

use virtual_generate::{Dimension, GeneratorSettings, RecursivePattern};
use virtual_media::ClipAddress;
use virtual_render::DeckEffects;

use super::UiAction;
use super::theme::ThemePalette;

const MEDIA_EXTENSIONS: [&str; 10] = [
    "mov", "mp4", "m4v", "mkv", "avi", "webm", "mxf", "png", "jpg", "jpeg",
];

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum BrowserTab {
    #[default]
    Files,
    Sources,
    Effects,
}

/// Drag payloads the clip grid accepts.
#[derive(Clone, Debug)]
pub(crate) struct BrowserMedia(pub PathBuf);
#[derive(Clone, Copy, Debug)]
pub(crate) struct BrowserGenerator(pub RecursivePattern);
#[derive(Clone, Copy, Debug)]
pub(crate) struct BrowserEffect(pub usize);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EntryKind {
    Folder,
    Video,
    Image,
}

#[derive(Clone, Debug)]
struct Entry {
    path: PathBuf,
    name: String,
    kind: EntryKind,
}

pub(crate) struct BrowserState {
    pub open: bool,
    pub tab: BrowserTab,
    pub folder: PathBuf,
    filter: String,
    /// The folder's listing, read when the folder changes or on refresh.
    listing: Option<(PathBuf, Result<Vec<Entry>, String>)>,
}

impl Default for BrowserState {
    fn default() -> Self {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_default();
        let movies = home.join("Movies");
        Self {
            open: true,
            tab: BrowserTab::Files,
            folder: if movies.is_dir() { movies } else { home },
            filter: String::new(),
            listing: None,
        }
    }
}

impl BrowserState {
    pub fn set_folder(&mut self, folder: PathBuf) {
        self.folder = folder;
        self.listing = None;
        self.filter.clear();
    }
}

fn read_folder(folder: &Path) -> Result<Vec<Entry>, String> {
    let mut entries: Vec<Entry> = std::fs::read_dir(folder)
        .map_err(|error| error.to_string())?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                return None;
            }
            let kind = if path.is_dir() {
                EntryKind::Folder
            } else {
                let extension = path
                    .extension()
                    .and_then(|extension| extension.to_str())
                    .map(str::to_ascii_lowercase)?;
                if !MEDIA_EXTENSIONS.contains(&extension.as_str()) {
                    return None;
                }
                if matches!(extension.as_str(), "png" | "jpg" | "jpeg") {
                    EntryKind::Image
                } else {
                    EntryKind::Video
                }
            };
            Some(Entry { path, name, kind })
        })
        .collect();
    entries.sort_by(|a, b| {
        (a.kind != EntryKind::Folder)
            .cmp(&(b.kind != EntryKind::Folder))
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(entries)
}

/// One deck effect the Effects tab can switch on and off.
pub(crate) struct QuickEffect {
    pub label: &'static str,
    pub group: &'static str,
    pub amount: fn(&mut DeckEffects) -> &mut f32,
    /// The amount the effect is switched on with.
    pub on: f32,
    /// The amount that means off.
    pub off: f32,
}

pub(crate) const QUICK_EFFECTS: [QuickEffect; 20] = [
    QuickEffect {
        label: "Fractal fold",
        group: "Folds",
        amount: |e| &mut e.fractal,
        on: 0.6,
        off: 0.0,
    },
    QuickEffect {
        label: "Spiral fold",
        group: "Folds",
        amount: |e| &mut e.spiral_fold,
        on: 0.6,
        off: 0.0,
    },
    QuickEffect {
        label: "Kali fold",
        group: "Folds",
        amount: |e| &mut e.kali_fold,
        on: 0.6,
        off: 0.0,
    },
    QuickEffect {
        label: "Koch fold",
        group: "Folds",
        amount: |e| &mut e.koch_fold,
        on: 0.6,
        off: 0.0,
    },
    QuickEffect {
        label: "Julia fold",
        group: "Folds",
        amount: |e| &mut e.julia_fold,
        on: 0.6,
        off: 0.0,
    },
    QuickEffect {
        label: "Polynomial fold",
        group: "Folds",
        amount: |e| &mut e.polynomial_fold,
        on: 0.6,
        off: 0.0,
    },
    QuickEffect {
        label: "Wave distortion",
        group: "Distort",
        amount: |e| &mut e.wave_distort,
        on: 0.5,
        off: 0.0,
    },
    QuickEffect {
        label: "Vortex distortion",
        group: "Distort",
        amount: |e| &mut e.vortex_distort,
        on: 0.5,
        off: 0.0,
    },
    QuickEffect {
        label: "Pixelate",
        group: "Distort",
        amount: |e| &mut e.pixelate,
        on: 0.3,
        off: 0.0,
    },
    QuickEffect {
        label: "Scanline jitter",
        group: "Glitch",
        amount: |e| &mut e.jitter,
        on: 0.5,
        off: 0.0,
    },
    QuickEffect {
        label: "Block jitter",
        group: "Glitch",
        amount: |e| &mut e.block_jitter,
        on: 0.5,
        off: 0.0,
    },
    QuickEffect {
        label: "RGB jitter",
        group: "Glitch",
        amount: |e| &mut e.rgb_jitter,
        on: 0.5,
        off: 0.0,
    },
    QuickEffect {
        label: "Bit reduction",
        group: "Glitch",
        amount: |e| &mut e.bit_reduction,
        on: 0.6,
        off: 0.0,
    },
    QuickEffect {
        label: "Neon glow",
        group: "Light",
        amount: |e| &mut e.neon,
        on: 0.6,
        off: 0.0,
    },
    QuickEffect {
        label: "Bloom",
        group: "Light",
        amount: |e| &mut e.bloom,
        on: 0.6,
        off: 0.0,
    },
    QuickEffect {
        label: "Black light",
        group: "Light",
        amount: |e| &mut e.blacklight,
        on: 0.7,
        off: 0.0,
    },
    QuickEffect {
        label: "Find edges",
        group: "Light",
        amount: |e| &mut e.find_edges,
        on: 0.7,
        off: 0.0,
    },
    QuickEffect {
        label: "Luma key",
        group: "Colour",
        amount: |e| &mut e.luma_key,
        on: 0.2,
        off: 0.0,
    },
    QuickEffect {
        label: "Monochrome",
        group: "Colour",
        amount: |e| &mut e.saturation,
        on: 0.0,
        off: 1.0,
    },
    QuickEffect {
        label: "High contrast",
        group: "Colour",
        amount: |e| &mut e.contrast,
        on: 1.8,
        off: 1.0,
    },
];

/// Turns a quick effect on (or off, when it already is) on one deck.
pub(crate) fn toggle_effect(effects: &mut DeckEffects, index: usize) {
    let effect = &QUICK_EFFECTS[index];
    let amount = (effect.amount)(effects);
    *amount = if (*amount - effect.off).abs() > 1e-3 {
        effect.off
    } else {
        effect.on
    };
}

fn effect_is_on(effects: &mut DeckEffects, index: usize) -> bool {
    let effect = &QUICK_EFFECTS[index];
    (*(effect.amount)(effects) - effect.off).abs() > 1e-3
}

/// A row that can be dragged, with an icon and a name.
fn row(ui: &mut egui::Ui, icon: &str, name: &str, palette: ThemePalette) -> egui::Response {
    let height = 20.0;
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), height),
        egui::Sense::click_and_drag(),
    );
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, 2.0, palette.control_tint(palette.accent, 0.18));
    }
    ui.painter().text(
        rect.left_center() + egui::vec2(4.0, 0.0),
        egui::Align2::LEFT_CENTER,
        icon,
        egui::FontId::proportional(13.0),
        palette.accent,
    );
    let text = rect.with_min_x(rect.min.x + 24.0);
    ui.painter().with_clip_rect(text).text(
        text.left_center(),
        egui::Align2::LEFT_CENTER,
        name,
        egui::FontId::proportional(12.5),
        palette.text,
    );
    response
}

pub(crate) struct BrowserContext<'a> {
    pub palette: ThemePalette,
    pub show_mode: bool,
    pub selected: ClipAddress,
    pub effects: &'a mut [DeckEffects; 4],
}

pub(crate) fn draw_browser(
    ui: &mut egui::Ui,
    browser: &mut BrowserState,
    context: BrowserContext<'_>,
    actions: &mut Vec<UiAction>,
) {
    let palette = context.palette;
    ui.horizontal(|ui| {
        for (tab, label) in [
            (BrowserTab::Files, "🗀 Files"),
            (BrowserTab::Sources, "◆ Sources"),
            (BrowserTab::Effects, "✦ Effects"),
        ] {
            ui.selectable_value(&mut browser.tab, tab, label);
        }
    });
    ui.separator();
    match browser.tab {
        BrowserTab::Files => files_tab(ui, browser, &context, actions),
        BrowserTab::Sources => sources_tab(ui, &context, actions),
        BrowserTab::Effects => effects_tab(ui, context, palette),
    }
}

fn files_tab(
    ui: &mut egui::Ui,
    browser: &mut BrowserState,
    context: &BrowserContext<'_>,
    actions: &mut Vec<UiAction>,
) {
    let palette = context.palette;
    ui.horizontal(|ui| {
        let parent = browser.folder.parent().map(Path::to_path_buf);
        if ui
            .add_enabled(parent.is_some(), egui::Button::new("⬆").small())
            .on_hover_text("Up one folder")
            .clicked()
            && let Some(parent) = parent
        {
            browser.set_folder(parent);
        }
        if ui.small_button("⟳").on_hover_text("Refresh").clicked() {
            browser.listing = None;
        }
        if ui
            .small_button("…")
            .on_hover_text("Choose a folder")
            .clicked()
        {
            actions.push(UiAction::BrowseBrowserFolder);
        }
        let name = browser.folder.file_name().map_or_else(
            || browser.folder.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        );
        ui.add(egui::Label::new(egui::RichText::new(name).strong()).truncate())
            .on_hover_text(browser.folder.display().to_string());
    });
    ui.add(
        egui::TextEdit::singleline(&mut browser.filter)
            .hint_text("Filter")
            .desired_width(f32::INFINITY),
    );
    if browser
        .listing
        .as_ref()
        .is_none_or(|(folder, _)| *folder != browser.folder)
    {
        browser.listing = Some((browser.folder.clone(), read_folder(&browser.folder)));
    }
    let filter = browser.filter.to_lowercase();
    let mut open = None;
    match &browser.listing.as_ref().expect("listed above").1 {
        Err(error) => {
            ui.colored_label(palette.danger, error);
        }
        Ok(entries) => {
            egui::ScrollArea::vertical()
                .id_salt("browser-files")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let mut shown = 0;
                    for entry in entries.iter().filter(|entry| {
                        filter.is_empty() || entry.name.to_lowercase().contains(&filter)
                    }) {
                        shown += 1;
                        let icon = match entry.kind {
                            EntryKind::Folder => "🗀",
                            EntryKind::Video => "🎞",
                            EntryKind::Image => "🖼",
                        };
                        let id = egui::Id::new(("browser-entry", &entry.path));
                        let response = if context.show_mode {
                            row(ui, icon, &entry.name, palette)
                        } else {
                            ui.dnd_drag_source(id, BrowserMedia(entry.path.clone()), |ui| {
                                row(ui, icon, &entry.name, palette)
                            })
                            .response
                        };
                        let response = response.on_hover_text(match entry.kind {
                            EntryKind::Folder => {
                                "Click to open · drag onto a clip slot to fill the deck from it"
                            }
                            _ => "Drag onto a clip slot · double-click to load the selected slot",
                        });
                        if entry.kind == EntryKind::Folder && response.clicked() {
                            open = Some(entry.path.clone());
                        } else if response.double_clicked() && !context.show_mode {
                            actions.push(UiAction::ImportToSlot {
                                address: context.selected,
                                path: entry.path.clone(),
                            });
                        }
                    }
                    if shown == 0 {
                        ui.weak("No folders or media here.");
                    }
                });
        }
    }
    if let Some(folder) = open {
        browser.set_folder(folder);
    }
}

fn sources_tab(ui: &mut egui::Ui, context: &BrowserContext<'_>, actions: &mut Vec<UiAction>) {
    let palette = context.palette;
    let deck = context.selected.deck;
    ui.weak(format!(
        "Click to load on deck {} · drag onto any deck's clip slot",
        deck.label()
    ));
    egui::ScrollArea::vertical()
        .id_salt("browser-sources")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for (heading, filter) in [
                ("SOURCES", (true, None)),
                ("2D PATTERNS", (false, Some(Dimension::Planar))),
                ("3D PATTERNS", (false, Some(Dimension::Spatial))),
            ] {
                ui.label(
                    egui::RichText::new(heading)
                        .small()
                        .strong()
                        .color(palette.muted_text),
                );
                for pattern in RecursivePattern::ALL.into_iter().filter(|pattern| {
                    pattern.is_source() == filter.0
                        && filter
                            .1
                            .is_none_or(|dimension| pattern.dimension() == dimension)
                }) {
                    let icon = if pattern.is_source() { "▣" } else { "◆" };
                    let response = ui
                        .dnd_drag_source(
                            egui::Id::new(("browser-pattern", pattern.id())),
                            BrowserGenerator(pattern),
                            |ui| row(ui, icon, pattern.label(), palette),
                        )
                        .response
                        .on_hover_text(pattern.hint());
                    if response.clicked() {
                        actions.push(UiAction::ConnectGenerator {
                            deck,
                            settings: Box::new(GeneratorSettings {
                                pattern,
                                ..GeneratorSettings::default()
                            }),
                        });
                    }
                }
                ui.add_space(4.0);
            }
        });
}

fn effects_tab(ui: &mut egui::Ui, context: BrowserContext<'_>, palette: ThemePalette) {
    let deck = context.selected.deck;
    ui.weak(format!(
        "Click to switch on deck {} · drag onto a deck's clip slot",
        deck.label()
    ));
    let effects = context.effects;
    egui::ScrollArea::vertical()
        .id_salt("browser-effects")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let mut group = "";
            for (index, effect) in QUICK_EFFECTS.iter().enumerate() {
                if effect.group != group {
                    group = effect.group;
                    ui.label(
                        egui::RichText::new(group.to_uppercase())
                            .small()
                            .strong()
                            .color(palette.muted_text),
                    );
                }
                let on = effect_is_on(&mut effects[deck.index()], index);
                let response = ui
                    .dnd_drag_source(
                        egui::Id::new(("browser-effect", index)),
                        BrowserEffect(index),
                        |ui| row(ui, if on { "●" } else { "○" }, effect.label, palette),
                    )
                    .response;
                if response.clicked() {
                    toggle_effect(&mut effects[deck.index()], index);
                }
            }
            ui.add_space(4.0);
            ui.weak("Fine-tune amounts in the deck's FX controls.");
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folders_list_first_and_only_media_files_show() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("virtual-browser-{stamp}"));
        std::fs::create_dir_all(root.join("Loops")).unwrap();
        for name in ["b.MOV", "a.png", "notes.txt", ".hidden.mp4"] {
            std::fs::write(root.join(name), b"").unwrap();
        }
        let entries = read_folder(&root).unwrap();
        let names: Vec<_> = entries.iter().map(|entry| entry.name.as_str()).collect();
        assert_eq!(names, ["Loops", "a.png", "b.MOV"]);
        assert_eq!(entries[0].kind, EntryKind::Folder);
        assert_eq!(entries[1].kind, EntryKind::Image);
        assert_eq!(entries[2].kind, EntryKind::Video);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn quick_effects_toggle_between_off_and_on() {
        let mut effects = DeckEffects::default();
        for (index, effect) in QUICK_EFFECTS.iter().enumerate() {
            assert!(!effect_is_on(&mut effects, index), "{}", effect.label);
            toggle_effect(&mut effects, index);
            assert!(effect_is_on(&mut effects, index), "{}", effect.label);
            toggle_effect(&mut effects, index);
            assert!(!effect_is_on(&mut effects, index));
        }
    }
}
