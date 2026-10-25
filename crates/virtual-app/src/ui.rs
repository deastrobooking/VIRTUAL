//! egui overlay.
//!
//! The UI never touches the GPU or mutates render state directly. It edits
//! plain values that get read into a per-frame snapshot, which is the same
//! path the parameter/modulation system takes later.

mod audio;
mod browser;
pub(crate) mod buttons;
pub(crate) mod calculator;
mod clips;
mod deck;
mod diagnostics;
pub(crate) mod drop_targets;
mod generator;
mod master_fx;
mod midi;
mod midi_manager;
mod navigation;
mod panel;
mod setup;
pub mod theme;
mod timeline;
mod toolbar;
mod video_input;

use clips::{ClipGridContext, draw_clip_grid};
use deck::{DeckControls, draw_deck};
use master_fx::{draw_custom_effect, draw_master_modulation};
use midi::draw_midi;
use midi_manager::draw_midi_manager;
use setup::draw_setup;
use theme::{CASCADE_STRIP_WIDTH, ResolvedDeckLayout, ThemePalette, ThemeState};
use toolbar::draw_toolbar;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use virtual_core::{
    AUDIO_MAP_SOURCES, AudioAnalysisSettings, AudioBinding, AudioMapMode, AudioMapper, AudioVisual,
    ClockSource, ControlTarget, FIXED_DECK_EFFECT_PARAMETER_COUNT, FrameTime, MappingMode,
    MidiMapper, Quantization, SPECTRUM_BAND_EDGES_HZ, SPECTRUM_BAND_LABELS, SPECTRUM_BANDS,
    SPECTRUM_CURVE_POINTS, TempoClock, audio_map_source_label, audio_map_sources,
    effect_parameter_key,
};
use virtual_io::{
    AudioInputDevice, AudioInputSnapshot, MidiInputDevice, MidiInputStats, MidiOutputDevice,
    MidiOutputStats,
};
use virtual_media::{
    CLIPS_PER_DECK, CameraDevice, ClipAddress, ClipBank, ClipLaunchMode, CrossfadeBus, DeckId,
    DeckState, DeckTransport, EndMode, FourDeckMixer, LaunchQueue, MediaHealth,
};
use virtual_render::{
    BlendModeGroup, DeckEffects, DeckLfos, DeckPackageModulationRoute, DeckPackageSlot,
    DeckTransform, EffectDescriptor, EffectHistoryResource, EffectLfo, EffectParameterControl,
    EffectParameterValue, EffectPreset, EffectTarget, LayerBlendMode, LfoShaping, LfoWaveform,
    MASTER_MODULATION_SOURCES, MOD_ROUTES_PER_DECK, MODULATION_SOURCES, MasterEffectChain,
    MasterEffectKind, MasterEffectSlot, MasterLfo, MasterModulation, ModulationRoute,
    SPECTRUM_SOURCE_OFFSET, SourceMode,
};

/// Overlay state, editor selections and session-only background export handles.
pub struct UiState {
    pub navigation: navigation::Navigation,
    pub deck_window: bool,
    pub timeline: timeline::TimelineUi,
    pub clip_clipboard: Option<(DeckId, virtual_media::ClipSlot)>,
    pub scene_clipboard: Option<virtual_io::SceneProject>,
    /// Clip editor target, opened by double-clicking a populated clip cell.
    pub clip_editor: Option<virtual_media::ClipAddress>,
    pub clip_editor_automation: bool,
    pub clip_editor_lane: usize,
    pub clip_editor_keyframe: usize,
    pub clip_editor_draw_mode: bool,
    /// Beat subdivision used by the clip automation editor's draw grid.
    pub clip_editor_snap_beats: f64,
    /// Draw-tool interpolation mode for creating held square steps.
    pub clip_editor_square_steps: bool,
    /// Last normalized pointer sample during a continuous automation stroke.
    pub clip_editor_draw_last: Option<[f32; 2]>,
    /// Selected bank of eight scene rows (A–D, covering 32 scenes).
    pub scene_bank: usize,
    /// Independent clip/stop selections for each of the 32 scene rows.
    pub scenes: [virtual_io::SceneProject; virtual_io::SCENE_COUNT],
    pub master_opacity: f32,
    pub blackout: bool,
    pub master_freeze: bool,
    pub crossfader: f32,
    pub equal_power: bool,
    pub output_enabled: bool,
    /// Session-only safety lock; never restored from project files.
    pub output_locked: bool,
    pub output_fullscreen: bool,
    pub output_display_id: String,
    pub output_test_card: bool,
    pub output_identify: bool,
    pub output_projection: virtual_core::ProjectionMapping,
    /// Selected mask and draw-mode state for the projection calibration canvas.
    pub projection_selected_mask: usize,
    pub projection_mask_draw_mode: bool,
    pub projection_mask_origin: Option<[f32; 2]>,
    pub ndi_enabled: bool,
    pub ndi_name: String,
    /// VIRTUAL Sync networking (leader, follower or off).
    pub sync: virtual_io::SyncProject,
    pub composition_extent: [u32; 2],
    pub custom_composition_extent: [u32; 2],
    pub master_effects: MasterEffectChain,
    pub effect_manifest_path: String,
    pub effect_reload_status: String,
    /// Packages executable by the current master-v1 runtime.
    pub effect_packages: Vec<EffectDescriptor>,
    /// Validated packages executable by the deck-v1 runtime.
    pub deck_effect_packages: Vec<EffectDescriptor>,
    pub deck_packages: [DeckPackageSlot; 4],
    pub deck_effect_reload_status: String,
    pub effect_registry_status: String,
    pub effect_registry_errors: usize,
    pub effect_reload_failed: bool,
    pub deck_effect_reload_failed: bool,
    pub master_modulation: MasterModulation,
    pub effects: [DeckEffects; 4],
    pub transforms: [DeckTransform; 4],
    pub blend_modes: [LayerBlendMode; 4],
    pub solo: [bool; 4],
    pub bypassed: [bool; 4],
    /// Deck indices from bottom to top within each crossfade bus.
    pub layer_order: [u8; 4],
    /// Deck composited over the crossfaded mix, ignoring the crossfader.
    pub pinned_deck: Option<DeckId>,
    /// UI buttons pressed from MIDI/OSC since the last frame.
    pub pending_buttons: std::collections::BTreeSet<u64>,
    /// Clip automation master switch per deck; off leaves every parameter
    /// under manual control.
    pub automation_enabled: [bool; 4],
    /// Parameters the performer has taken over from running clip automation,
    /// per deck. Automation skips them until resumed.
    pub automation_overrides: [Vec<ControlTarget>; 4],
    pub lfos: [DeckLfos; 4],
    /// Last rendered modulation source values per deck, for UI meters.
    pub mod_sources: [[f32; MODULATION_SOURCES]; 4],
    pub master_mod_sources: [f32; MASTER_MODULATION_SOURCES],
    pub bpm: f64,
    pub quantization: Quantization,
    pub project_path: String,
    pub(crate) save_status: crate::project_save::SaveStatus,
    pub camera_device_id: String,
    pub camera_width: u32,
    pub camera_height: u32,
    pub camera_fps: u32,
    pub camera_fps_denominator: u32,
    pub capture_pixel_format: virtual_media::CapturePixelFormat,
    /// Pattern the generator loader puts on a deck.
    pub generator_pattern: virtual_generate::RecursivePattern,
    /// Per-deck generator window visibility.
    pub generator_windows: [bool; 4],
    /// Whether each deck was a generator last frame, to open its window on
    /// the transition into a generator source.
    pub generator_seen: [bool; 4],
    /// Reusable geometry graphs persisted with the show project.
    pub geometry_library: Vec<virtual_generate::GeometryGraph>,
    /// Graphing calculator, one per generator deck (session only).
    pub geometry_calculator: [calculator::GraphingCalculator; 4],
    /// Focused geometry node per generator deck.
    pub geometry_selected_node: [u64; 4],
    pub audio_device_id: String,
    pub audio_analysis: AudioAnalysisSettings,
    /// Zero-based interface channel to analyse, or `None` to mix all.
    pub audio_channel: Option<u16>,
    pub audio_map: AudioMapper,
    /// Spectrum source waiting for a control click in map mode.
    pub audio_learn: Option<AudioLearn>,
    /// Decaying per-band peaks for the spectrum display.
    pub spectrum_peaks: [f32; SPECTRUM_BANDS],
    /// Decaying peak of the fine spectrum curve, in dBFS.
    pub spectrum_curve_peaks: Vec<f32>,
    /// Waveform and spectrum held for inspection while frozen.
    pub audio_display_frozen: Option<AudioVisual>,
    pub midi_device_id: String,
    pub midi_target: ControlTarget,
    /// Where the transport takes its tempo from.
    pub midi_clock_source: ClockSource,
    pub link_peers: u64,
    /// Device trusted for incoming clock; empty follows whichever connected
    /// device clocks first.
    pub midi_clock_input_device: String,
    pub midi_output_device_id: String,
    /// Whether clock is being sent downstream.
    pub midi_clock_send: bool,
    pub osc_bind_address: String,
    pub osc_feedback_address: String,
    pub session_recovery_selected: usize,
    pub take_name_input: String,
    pub random_seed_scope: String,
    pub random_seed_value: u64,
    pub session_replay_seconds: f64,
    pub project_take_selected: usize,
    pub timeline_marker_input: String,
    pub take_export_directory: String,
    pub theme: ThemeState,
    /// Performance lock: keeps launch/mix/transport controls live while
    /// hiding setup and structural editors that can destabilize a show.
    pub show_mode: bool,
    pub midi_map_mode: bool,
    pub midi_manager_open: bool,
    thumbnails: HashMap<ClipAddress, CachedThumbnail>,
    pub browser: browser::BrowserState,
    /// The selected clip's first frame for the preview monitor.
    cue_preview: Option<(ClipAddress, PathBuf, egui::TextureHandle)>,
    thumbnail_failures: HashMap<ClipAddress, (PathBuf, String)>,
    fps: FpsMeter,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            navigation: navigation::Navigation::default(),
            deck_window: false,
            timeline: timeline::TimelineUi::default(),
            clip_clipboard: None,
            scene_clipboard: None,
            clip_editor: None,
            clip_editor_automation: false,
            clip_editor_lane: 0,
            clip_editor_keyframe: 0,
            clip_editor_draw_mode: false,
            clip_editor_snap_beats: 1.0,
            clip_editor_square_steps: true,
            clip_editor_draw_last: None,
            scene_bank: 0,
            scenes: virtual_io::default_scenes(),
            master_opacity: 1.0,
            blackout: false,
            master_freeze: false,
            crossfader: 0.5,
            equal_power: true,
            output_enabled: true,
            output_locked: false,
            output_fullscreen: false,
            output_display_id: String::new(),
            output_test_card: false,
            output_identify: false,
            output_projection: Default::default(),
            projection_selected_mask: 0,
            projection_mask_draw_mode: false,
            projection_mask_origin: None,
            ndi_enabled: false,
            ndi_name: crate::ndi::DEFAULT_SOURCE_NAME.to_owned(),
            sync: virtual_io::SyncProject::default(),
            composition_extent: [1920, 1080],
            custom_composition_extent: [1920, 1080],
            master_effects: MasterEffectChain::default(),
            effect_manifest_path: "effects/master-effects/effect.json".to_owned(),
            effect_reload_status: "Built-in master effect pipeline".to_owned(),
            effect_packages: Vec::new(),
            deck_effect_packages: Vec::new(),
            deck_packages: std::array::from_fn(|_| DeckPackageSlot::default()),
            deck_effect_reload_status: "Deck effect runtime idle".to_owned(),
            effect_registry_status: "Effect registry not scanned".to_owned(),
            effect_registry_errors: 0,
            effect_reload_failed: false,
            deck_effect_reload_failed: false,
            master_modulation: MasterModulation::default(),
            effects: [DeckEffects::default(); 4],
            transforms: [DeckTransform::default(); 4],
            blend_modes: [LayerBlendMode::Normal; 4],
            solo: [false; 4],
            bypassed: [false; 4],
            layer_order: virtual_render::DEFAULT_LAYER_ORDER,
            pinned_deck: None,
            pending_buttons: Default::default(),
            automation_enabled: [true; 4],
            automation_overrides: Default::default(),
            lfos: [DeckLfos::default(); 4],
            mod_sources: [[0.0; MODULATION_SOURCES]; 4],
            master_mod_sources: [0.0; MASTER_MODULATION_SOURCES],
            bpm: 120.0,
            quantization: Quantization::Immediate,
            project_path: "show.virtual".to_owned(),
            save_status: crate::project_save::SaveStatus::default(),
            camera_device_id: virtual_media::default_camera_id().to_owned(),
            camera_width: 1280,
            camera_height: 720,
            camera_fps: 30,
            camera_fps_denominator: 1,
            capture_pixel_format: virtual_media::CapturePixelFormat::Auto,
            generator_pattern: virtual_generate::RecursivePattern::default(),
            generator_windows: [false; 4],
            generator_seen: [false; 4],
            geometry_library: Vec::new(),
            geometry_calculator: Default::default(),
            geometry_selected_node: [0; 4],
            audio_device_id: String::new(),
            audio_analysis: AudioAnalysisSettings::default(),
            audio_channel: None,
            audio_map: AudioMapper::default(),
            audio_learn: None,
            spectrum_peaks: [0.0; SPECTRUM_BANDS],
            spectrum_curve_peaks: vec![-120.0; SPECTRUM_CURVE_POINTS],
            audio_display_frozen: None,
            midi_device_id: String::new(),
            midi_target: ControlTarget::Crossfader,
            midi_clock_source: ClockSource::Internal,
            link_peers: 0,
            midi_clock_input_device: String::new(),
            midi_output_device_id: String::new(),
            midi_clock_send: false,
            osc_bind_address: "0.0.0.0:9000".to_owned(),
            osc_feedback_address: "127.0.0.1:9001".to_owned(),
            session_recovery_selected: 0,
            take_name_input: "Take 1".to_owned(),
            random_seed_scope: "visuals".to_owned(),
            random_seed_value: 1,
            session_replay_seconds: 0.0,
            project_take_selected: 0,
            timeline_marker_input: String::new(),
            take_export_directory: "take-exports".to_owned(),
            theme: ThemeState::default(),
            show_mode: false,
            midi_map_mode: false,
            midi_manager_open: false,
            thumbnails: HashMap::new(),
            browser: browser::BrowserState::default(),
            cue_preview: None,
            thumbnail_failures: HashMap::new(),
            fps: FpsMeter::default(),
        }
    }
}

struct CachedThumbnail {
    path: PathBuf,
    texture: egui::TextureHandle,
    preload: virtual_media::RgbaFrame,
}

impl UiState {
    pub fn install_thumbnail(
        &mut self,
        ctx: &egui::Context,
        address: ClipAddress,
        path: PathBuf,
        thumbnail: virtual_media::Thumbnail,
    ) {
        let [width, height] = thumbnail.extent;
        let image = egui::ColorImage::from_rgba_unmultiplied(
            [width as usize, height as usize],
            &thumbnail.rgba,
        );
        let texture = ctx.load_texture(
            format!("clip-thumbnail-{}-{}", address.deck.label(), address.slot),
            image,
            egui::TextureOptions::LINEAR,
        );
        self.thumbnails.insert(
            address,
            CachedThumbnail {
                path,
                texture,
                preload: thumbnail.preload,
            },
        );
        self.thumbnail_failures.remove(&address);
    }

    pub fn mark_thumbnail_failed(&mut self, address: ClipAddress, path: PathBuf, message: String) {
        self.thumbnails.remove(&address);
        self.thumbnail_failures.insert(address, (path, message));
    }

    pub fn clear_thumbnail(&mut self, address: ClipAddress) {
        self.thumbnails.remove(&address);
        self.thumbnail_failures.remove(&address);
    }

    /// Exchange the cached previews of two slots after a clip move. Entries
    /// stay validated by media path, so a mismatch simply re-requests.
    pub fn swap_thumbnails(&mut self, a: ClipAddress, b: ClipAddress) {
        let first = self.thumbnails.remove(&a);
        let second = self.thumbnails.remove(&b);
        if let Some(entry) = first {
            self.thumbnails.insert(b, entry);
        }
        if let Some(entry) = second {
            self.thumbnails.insert(a, entry);
        }
        let first = self.thumbnail_failures.remove(&a);
        let second = self.thumbnail_failures.remove(&b);
        if let Some(entry) = first {
            self.thumbnail_failures.insert(b, entry);
        }
        if let Some(entry) = second {
            self.thumbnail_failures.insert(a, entry);
        }
    }

    pub fn clear_thumbnails(&mut self) {
        self.thumbnails.clear();
        self.thumbnail_failures.clear();
    }

    fn thumbnail(&self, address: ClipAddress, path: Option<&Path>) -> Option<&egui::TextureHandle> {
        let cached = self.thumbnails.get(&address)?;
        (Some(cached.path.as_path()) == path).then_some(&cached.texture)
    }

    pub fn preloaded_frame(
        &self,
        address: ClipAddress,
        path: Option<&Path>,
    ) -> Option<&virtual_media::RgbaFrame> {
        let cached = self.thumbnails.get(&address)?;
        (Some(cached.path.as_path()) == path).then_some(&cached.preload)
    }

    fn preloaded_count(&self) -> usize {
        self.thumbnails.len()
    }

    fn thumbnail_failure(&self, address: ClipAddress, path: Option<&Path>) -> Option<&str> {
        let (failed_path, message) = self.thumbnail_failures.get(&address)?;
        (Some(failed_path.as_path()) == path).then_some(message.as_str())
    }
}

/// Exponentially smoothed frame rate.
///
/// Instantaneous 1/delta is unreadable and a rolling window costs an
/// allocation; neither is worth it for a number a human reads.
#[derive(Default)]
struct FpsMeter {
    smoothed_delta: f64,
}

impl FpsMeter {
    fn push(&mut self, delta: f64) {
        if delta <= 0.0 {
            return;
        }
        if self.smoothed_delta == 0.0 {
            self.smoothed_delta = delta;
        } else {
            self.smoothed_delta += (delta - self.smoothed_delta) * 0.1;
        }
    }

    fn fps(&self) -> f64 {
        if self.smoothed_delta > 0.0 {
            1.0 / self.smoothed_delta
        } else {
            0.0
        }
    }
}

/// Human-readable name of a control, including loaded package parameters.
pub fn control_target_label(state: &UiState, target: ControlTarget) -> String {
    midi::midi_target_label_for_state(target, state)
}

/// A spectrum source armed for click-to-map.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AudioLearn {
    pub source: u8,
    /// Map mode was off when learning started, so turn it off again after.
    pub restore_map_mode_off: bool,
}

#[derive(Clone, Debug)]
pub enum UiAction {
    Restart(DeckId),
    Seek(DeckId),
    Launch(ClipAddress),
    LaunchAutomation(ClipAddress),
    LaunchScene(usize),
    ClearSlot(ClipAddress),
    PasteClip {
        to: ClipAddress,
        source_deck: DeckId,
        clip: Box<virtual_media::ClipSlot>,
    },
    ExportTimelineVideo,
    ExportTimelineTake,
    MoveClip {
        from: ClipAddress,
        to: ClipAddress,
    },
    BrowseRelink(ClipAddress),
    /// Load a file or folder from the browser into a slot.
    ImportToSlot {
        address: ClipAddress,
        path: PathBuf,
    },
    /// Choose the browser's folder with a system dialog.
    BrowseBrowserFolder,
    Eject(DeckId),
    ConnectGenerator {
        deck: DeckId,
        settings: Box<virtual_generate::GeneratorSettings>,
    },
    SaveProject,
    SaveProjectAs,
    OpenProject,
    BrowseOpenProject,
    RecoverProject,
    RefreshSessionRecoveries,
    RestoreSessionRecovery(usize),
    RestoreSessionRecoveryAt {
        index: usize,
        monotonic_ns: u64,
    },
    StartNamedTake,
    SetRandomSeed,
    RenameProjectTake(usize),
    RemoveProjectTake(usize),
    AddTimelineMarker,
    ExportProjectTake(usize),
    ArchiveProjectTake(usize),
    TapTempo,
    HalfTempo,
    DoubleTempo,
    SetOutputEnabled(bool),
    SetOutputFullscreen(bool),
    SetOutputDisplay(String),
    SetCompositionExtent([u32; 2]),
    WatchEffectManifest,
    ReloadEffectManifest,
    RefreshEffectRegistry,
    RefreshDisplays,
    RefreshCameras,
    RefreshAudioInputs,
    ConnectAudioInput(String),
    DisconnectAudioInput,
    RefreshMidiInputs,
    ConnectMidiInput(String),
    DisconnectMidiInput(String),
    MidiClearDevice(String),
    ConnectOscInput,
    DisconnectOscInput,
    ConnectOscOutput,
    DisconnectOscOutput,
    SetMidiClockSource(ClockSource),
    RefreshMidiOutputs,
    ConnectMidiClockOutput(String),
    DisconnectMidiClockOutput,
    SetMidiClockSend(bool),
    MidiClockContinue,
    MidiLearn(ControlTarget),
    MidiCancelLearn,
    /// Bind the armed spectrum source to this control.
    AudioMapTarget(ControlTarget),
    MidiClearTarget(ControlTarget),
    MidiRemoveBinding(usize),
    ConnectCamera {
        deck: DeckId,
        config: virtual_media::CameraConfig,
    },
    StartCameraRecording(ClipAddress),
    StopCameraRecording(DeckId),
}

#[derive(Clone, Debug)]
pub struct OutputDisplay {
    pub id: String,
    pub label: String,
}

pub struct OutputHealthMetrics<'a> {
    pub status: &'a str,
    pub current_display: &'a str,
    pub surface_extent: [u32; 2],
    pub presented: u64,
    pub skipped: u64,
    pub reconfigurations: u64,
    pub recoveries: u64,
    pub timeouts: u64,
    pub occlusions: u64,
    pub validation_errors: u64,
    pub topology_changes: u64,
}

pub struct PerformanceMetrics<'a> {
    pub tempo: TempoClock,
    pub now_seconds: f64,
    pub scheduler_stats: [virtual_media::SchedulerStats; 4],
    pub frame_pool_stats: [virtual_media::FramePoolStats; 4],
    pub deck_package_stats: virtual_render::DeckPackageFrameStats,
    pub deck_package_timings: virtual_render::DeckPackageTimingStats,
    pub frame_time: &'a FrameTime,
    pub gpu_info: &'a str,
    pub runtime_status: &'a str,
    pub project_dirty: bool,
    pub project_status: &'a str,
    pub folder_status: &'a str,
    pub recovery_available: bool,
    pub session_recoveries: &'a [crate::recovery::RecoveryEntry],
    pub session_recovery_status: &'a str,
    pub project_takes: &'a [virtual_io::TakeMetadataProject],
    pub cameras: &'a [CameraDevice],
    pub camera_status: &'a str,
    pub camera_recordings: [CameraRecordingStatus; 4],
    pub generator_stats: [virtual_generate::GeneratorStats; 4],
    pub audio_inputs: &'a [AudioInputDevice],
    pub audio_status: &'a str,
    pub audio_connected: bool,
    pub audio_required: bool,
    pub audio_snapshot: AudioInputSnapshot,
    pub audio_visual: &'a AudioVisual,
    pub midi: MidiMetrics<'a>,
    pub ndi: crate::ndi::NdiStatus,
    pub sync: crate::sync::SyncStatus,
    pub osc: OscMetrics<'a>,
    pub output_displays: &'a [OutputDisplay],
    pub output_health: OutputHealthMetrics<'a>,
    /// The program texture registered with egui, and its size.
    pub program_monitor: (egui::TextureId, [u32; 2]),
}

#[derive(Clone, Copy, Debug, Default)]
pub struct CameraRecordingStatus {
    pub address: Option<ClipAddress>,
    pub finalizing: bool,
    pub elapsed_seconds: f64,
    pub dropped_frames: u64,
}

pub struct MidiMetrics<'a> {
    pub inputs: &'a [MidiInputDevice],
    pub status: &'a str,
    pub devices: &'a [MidiDeviceStatus],
    pub mapper: &'a mut MidiMapper,
    pub clock: MidiClockMetrics<'a>,
}

/// Beat-clock sync state, in and out, as the operator panel sees it.
///
/// Copyable so the panel can lift it out of the surrounding `MidiMetrics`
/// without holding a borrow that fights the mutable mapper next to it.
#[derive(Clone, Copy)]
pub struct MidiClockMetrics<'a> {
    /// Device currently trusted as clock master, if any.
    pub following: Option<&'a str>,
    pub locked: bool,
    pub follower_bpm: Option<f64>,
    pub follower_running: bool,
    pub pulses: u64,
    pub jitter_micros: u64,
    pub resyncs: u64,
    pub status: &'a str,
    pub outputs: &'a [MidiOutputDevice],
    pub output_connected: bool,
    pub output_running: bool,
    pub output_status: &'a str,
    pub output_stats: MidiOutputStats,
}

impl MidiMetrics<'_> {
    pub fn any_connected(&self) -> bool {
        self.devices.iter().any(|device| device.connected)
    }

    pub fn device_connected(&self, id: &str) -> bool {
        self.devices
            .iter()
            .any(|device| device.connected && device.id == id)
    }
}

/// One MIDI device as the manager window sees it.
#[derive(Clone, Debug, Default)]
pub struct MidiDeviceStatus {
    pub id: String,
    pub label: String,
    /// Present in the latest native discovery snapshot.
    pub available: bool,
    pub connected: bool,
    /// Whether the operator asked for this device; wanted devices reconnect
    /// automatically when the hardware reappears.
    pub wanted: bool,
    pub stats: MidiInputStats,
}

pub struct OscMetrics<'a> {
    pub status: &'a str,
    pub connected: bool,
    pub stats: crate::osc::OscStats,
    pub pending: usize,
    pub schedule_dropped: u64,
    pub output_status: &'a str,
    pub output_connected: bool,
    pub output_stats: crate::osc::OscOutputStats,
}

/// Everything the click-to-arm overlay needs, resolved once per frame.
///
/// Ableton-style mapping: arm the mode, click any highlighted control, then
/// move a knob on any connected device. While the mode is armed the wrapped
/// widgets are disabled so browsing for a control cannot change the mix.
#[derive(Clone)]
pub(super) struct MidiMapUi {
    pub active: bool,
    pub learning: Option<ControlTarget>,
    /// (target, device) pairs for every existing binding.
    pub mapped: Vec<(ControlTarget, String)>,
    /// Spectrum source armed for click-to-map, if any.
    pub audio_learning: Option<u8>,
    /// (target, source) pairs for every audio binding.
    pub audio_mapped: Vec<(ControlTarget, u8)>,
    pub palette: ThemePalette,
}

impl MidiMapUi {
    fn devices_for(&self, target: ControlTarget) -> Vec<&str> {
        self.mapped
            .iter()
            .filter(|(mapped, _)| *mapped == target)
            .map(|(_, device)| device.as_str())
            .chain(
                self.audio_mapped
                    .iter()
                    .filter(|(mapped, _)| *mapped == target)
                    .map(|(_, source)| audio_map_source_label(*source)),
            )
            .collect()
    }
}

/// Wrap a widget so it participates in MIDI map mode.
///
/// Outside map mode this is a transparent pass-through. Inside it, the widget
/// draws disabled and an overlay takes the clicks: primary arms the target
/// for the next incoming message, secondary clears its bindings.
/// Moves `deck` to the top of the bottom-to-top layer order.
pub(crate) fn layer_to_top(order: &mut [u8; 4], deck: u8) {
    if let Some(slot) = order.iter().position(|candidate| *candidate == deck) {
        order[slot..].rotate_left(1);
    }
}

/// Moves `deck` one layer up or down; no-op at either end.
pub(crate) fn layer_step(order: &mut [u8; 4], deck: u8, up: bool) {
    let Some(slot) = order.iter().position(|candidate| *candidate == deck) else {
        return;
    };
    match (up, slot) {
        (true, slot) if slot + 1 < order.len() => order.swap(slot, slot + 1),
        (false, slot) if slot > 0 => order.swap(slot, slot - 1),
        _ => {}
    }
}

/// Bottom-to-top deck stacking, plus one deck pinned over the crossfade.
/// `compact` drops the per-deck arrows for the master toolbar.
pub(super) fn draw_layer_order(
    ui: &mut egui::Ui,
    state: &mut UiState,
    palette: &ThemePalette,
    midi_map: &MidiMapUi,
    actions: &mut Vec<UiAction>,
    compact: bool,
) {
    ui.horizontal_wrapped(|ui| {
        ui.strong("LAYERS").on_hover_text(
            "Stacking order within each crossfade bus, bottom to top. \
             Click a deck to bring it to the top. PIN draws a deck over the \
             crossfaded mix so crossfades and auto blends never fade it out.",
        );
        if !compact {
            ui.weak("bottom");
        }
        for slot in 0..state.layer_order.len() {
            let index = state.layer_order[slot];
            let deck = DeckId::ALL[usize::from(index)];
            let color = palette.deck_color(deck);
            if !compact
                && mappable(
                    ui,
                    midi_map,
                    ControlTarget::DeckLayerDown(index),
                    actions,
                    |ui| {
                        ui.add_enabled(slot > 0, egui::Button::new("◀").small())
                            .on_hover_text(format!("Move deck {} down one layer", deck.label()))
                    },
                )
                .clicked()
            {
                layer_step(&mut state.layer_order, index, false);
            }
            let top = slot + 1 == state.layer_order.len();
            if mappable(
                ui,
                midi_map,
                ControlTarget::DeckLayerTop(index),
                actions,
                |ui| {
                    ui.add(
                        egui::Button::new(egui::RichText::new(deck.label()).strong())
                            .fill(palette.control_tint(color, if top { 0.45 } else { 0.15 }))
                            .stroke(egui::Stroke::new(1.0, color))
                            .min_size(egui::vec2(28.0, 0.0)),
                    )
                    .on_hover_text(format!("Bring deck {} to the top layer", deck.label()))
                },
            )
            .clicked()
            {
                layer_to_top(&mut state.layer_order, index);
            }
            if !compact
                && mappable(
                    ui,
                    midi_map,
                    ControlTarget::DeckLayerUp(index),
                    actions,
                    |ui| {
                        ui.add_enabled(!top, egui::Button::new("▶").small())
                            .on_hover_text(format!("Move deck {} up one layer", deck.label()))
                    },
                )
                .clicked()
            {
                layer_step(&mut state.layer_order, index, true);
            }
            let pinned = state.pinned_deck == Some(deck);
            if mappable(ui, midi_map, ControlTarget::DeckPin(index), actions, |ui| {
                ui.add(
                    egui::Button::new(egui::RichText::new("PIN").small())
                        .selected(pinned)
                        .fill(palette.control_tint(color, if pinned { 0.55 } else { 0.08 })),
                )
                .on_hover_text(format!(
                    "{} deck {} over the crossfaded mix",
                    if pinned { "Unpin" } else { "Pin" },
                    deck.label()
                ))
            })
            .clicked()
            {
                state.pinned_deck = (!pinned).then_some(deck);
            }
            if !compact {
                ui.add_space(4.0);
            }
        }
        if !compact {
            ui.weak("top");
        }
        let changed =
            state.layer_order != virtual_render::DEFAULT_LAYER_ORDER || state.pinned_deck.is_some();
        if mappable(ui, midi_map, ControlTarget::LayerReset, actions, |ui| {
            ui.add_enabled(changed || midi_map.active, egui::Button::new("Reset"))
                .on_hover_text("Restore A–D stacking and unpin")
        })
        .clicked()
        {
            state.layer_order = virtual_render::DEFAULT_LAYER_ORDER;
            state.pinned_deck = None;
        }
    });
}

pub(super) fn mappable(
    ui: &mut egui::Ui,
    map: &MidiMapUi,
    target: ControlTarget,
    actions: &mut Vec<UiAction>,
    add: impl FnOnce(&mut egui::Ui) -> egui::Response,
) -> egui::Response {
    if !map.active {
        return add(ui);
    }
    let response = ui.add_enabled_ui(false, add).inner;
    if let Some(source) = map.audio_learning {
        let rect = response.rect.expand(2.0);
        ui.painter().rect(
            rect,
            4.0,
            egui::Color32::TRANSPARENT,
            egui::Stroke::new(1.5, map.palette.success),
            egui::StrokeKind::Outside,
        );
        let hit = ui
            .interact(
                rect,
                response.id.with("audio-map-overlay"),
                egui::Sense::click(),
            )
            .on_hover_text(format!(
                "Click to drive this from the {} band",
                audio_map_source_label(source)
            ));
        if hit.clicked() {
            actions.push(UiAction::AudioMapTarget(target));
        }
        return response;
    }
    let armed = map.learning == Some(target);
    let devices = map.devices_for(target);
    let stroke = if armed {
        egui::Stroke::new(2.0, map.palette.accent)
    } else if devices.is_empty() {
        egui::Stroke::new(1.0, map.palette.stroke)
    } else {
        egui::Stroke::new(1.0, map.palette.secondary)
    };
    let rect = response.rect.expand(2.0);
    // This is painted after the widget: an opaque control tint hides its text
    // and value. Outline the target without painting over its contents.
    ui.painter().rect(
        rect,
        4.0,
        egui::Color32::TRANSPARENT,
        stroke,
        egui::StrokeKind::Outside,
    );
    let hit = ui.interact(
        rect,
        response.id.with("midi-map-overlay"),
        egui::Sense::click(),
    );
    let hit = hit.on_hover_text(if armed {
        "Armed · move a control on any connected device".to_owned()
    } else if devices.is_empty() {
        "Click to arm, then move a control · right-click clears".to_owned()
    } else {
        format!(
            "Mapped to {} · click to remap · right-click clears",
            devices.join(", ")
        )
    });
    if hit.clicked() {
        actions.push(if armed {
            UiAction::MidiCancelLearn
        } else {
            UiAction::MidiLearn(target)
        });
    }
    if hit.secondary_clicked() {
        actions.push(UiAction::MidiClearTarget(target));
    }
    response
}

pub fn draw(
    ctx: &egui::Context,
    state: &mut UiState,
    mixer: &mut FourDeckMixer,
    clips: &mut ClipBank,
    launches: &LaunchQueue,
    transports: &mut [DeckTransport; 4],
    mut metrics: PerformanceMetrics<'_>,
) -> Vec<UiAction> {
    state.theme.ensure_applied(ctx);
    navigation::shortcuts(ctx, state);
    let palette = state.theme.palette();
    state.fps.push(metrics.frame_time.delta);
    let mut actions = Vec::new();
    drop_targets::begin_frame();
    buttons::begin_frame(
        std::mem::take(&mut state.pending_buttons),
        MidiMapUi {
            active: state.midi_map_mode,
            learning: metrics.midi.mapper.learning(),
            mapped: metrics
                .midi
                .mapper
                .bindings
                .iter()
                .map(|binding| (binding.target, binding.device.clone()))
                .collect(),
            audio_learning: state.audio_learn.map(|learn| learn.source),
            audio_mapped: state
                .audio_map
                .bindings
                .iter()
                .map(|binding| (binding.target, binding.source))
                .collect(),
            palette,
        },
    );

    let mut floating_browser = false;
    egui::Window::new("VIRTUAL")
        .default_pos([16.0, 16.0])
        .default_size([1440.0, 880.0])
        .min_size([560.0, 420.0])
        .resizable(true)
        .scroll([false, false])
        .show(ctx, |ui| {
            let midi_map = buttons::current_map().expect("buttons::begin_frame runs first");
            draw_toolbar(ui, state, clips, &metrics, palette, &midi_map, &mut actions);
            navigation::draw(ui, state, mixer, clips);
            floating_browser = state.browser.open && ui.available_width() < 780.0;
            if state.browser.open && !floating_browser {
                let selected = ClipAddress {
                    deck: mixer.selected(),
                    slot: clips.selected(mixer.selected()),
                };
                egui::Panel::left("browser-panel")
                    .resizable(true)
                    .default_size(250.0)
                    .min_size(180.0)
                    .show(ui, |ui| {
                        browser::draw_browser(
                            ui,
                            &mut state.browser,
                            browser::BrowserContext {
                                palette,
                                show_mode: state.show_mode,
                                selected,
                                effects: &mut state.effects,
                            },
                            &mut actions,
                        );
                    });
            }
            let docked = ui.available_width() >= 1000.0;
            if docked {
                egui::Panel::right("properties-panel")
                    .resizable(true)
                    .default_size(460.0)
                    .min_size(340.0)
                    .show(ui, |ui| {
                        draw_monitors(
                            ui,
                            state,
                            mixer,
                            clips,
                            metrics.program_monitor,
                            palette,
                            &mut actions,
                        );
                        ui.add_space(4.0);
                        egui::ScrollArea::vertical()
                            .id_salt("properties-scroll")
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                if !state.deck_window {
                                    draw_deck_section(
                                        ui,
                                        state,
                                        mixer,
                                        transports,
                                        metrics.generator_stats,
                                        palette,
                                        &midi_map,
                                        &mut actions,
                                    );
                                }
                            });
                    });
            }
            egui::ScrollArea::both()
                .id_salt("performance-editor")
                .auto_shrink([false, false])
                .max_height(ui.available_height())
                .show(ui, |ui| {
                    ui.separator();
                    // Everything an operator sets up before the show - output,
                    // project, devices, diagnostics - lives in one collapsible
                    // region with its own scrollbar, so on a small screen it can
                    // be scrolled through or folded away entirely while the
                    // performance surface below keeps the space.
                    draw_setup(ui, state, mixer, &midi_map, &mut actions, &mut metrics);
                    let beat_position = metrics.tempo.beat_at(metrics.now_seconds);
                    let playing = DeckId::ALL
                        .iter()
                        .filter(|deck| clips.active(**deck).is_some())
                        .count();
                    panel::section(
                        ui,
                        "clips",
                        "▦",
                        "CLIPS",
                        palette,
                        true,
                        |ui| {
                            ui.weak(format!("{playing} of 4 decks playing"));
                        },
                        |ui| {
                            draw_clip_grid(
                                ui,
                                state,
                                mixer,
                                clips,
                                ClipGridContext {
                                    launches,
                                    midi_map: &midi_map,
                                    cameras: metrics.cameras,
                                    camera_status: metrics.camera_status,
                                    camera_recordings: metrics.camera_recordings,
                                    transports,
                                    beat_position,
                                },
                                &mut actions,
                            );
                        },
                    );

                    ui.separator();
                    audio::draw_audio_panel(
                        ui,
                        state,
                        audio::AudioPanelContext {
                            inputs: metrics.audio_inputs,
                            status: metrics.audio_status,
                            connected: metrics.audio_connected,
                            snapshot: metrics.audio_snapshot,
                            visual: metrics.audio_visual,
                            palette,
                        },
                        &mut actions,
                    );

                    ui.separator();
                    if !docked {
                        draw_monitors(
                            ui,
                            state,
                            mixer,
                            clips,
                            metrics.program_monitor,
                            palette,
                            &mut actions,
                        );
                        if !state.deck_window {
                            draw_deck_section(
                                ui,
                                state,
                                mixer,
                                transports,
                                metrics.generator_stats,
                                palette,
                                &midi_map,
                                &mut actions,
                            );
                        }
                    }

                    ui.separator();
                    let mix_status = format!(
                        "master {:.0}%{}{}",
                        state.master_opacity * 100.0,
                        if state.blackout { " · BLACKOUT" } else { "" },
                        if state.master_freeze {
                            " · FROZEN"
                        } else {
                            ""
                        },
                    );
                    panel::section(
                        ui,
                        "mix-master",
                        "⇄",
                        "MIX & MASTER",
                        palette,
                        true,
                        |ui| {
                            ui.weak(mix_status);
                        },
                        |ui| {
                            ui.horizontal(|ui| {
                                ui.label("A");
                                mappable(
                                    ui,
                                    &midi_map,
                                    ControlTarget::Crossfader,
                                    &mut actions,
                                    |ui| {
                                        ui.add(
                                            egui::Slider::new(&mut state.crossfader, 0.0..=1.0)
                                                .text("crossfader")
                                                .clamping(egui::SliderClamping::Always),
                                        )
                                    },
                                );
                                ui.label("B");
                                buttons::midi_toggle(
                                    ui,
                                    "mixer.equal_power",
                                    "Mixer · Equal power",
                                    &mut state.equal_power,
                                    |ui, value| ui.checkbox(value, "equal power"),
                                );
                                if buttons::midi_button(
                                    ui,
                                    "mixer.center",
                                    "Mixer · Center crossfader",
                                    |ui| ui.button("Center"),
                                )
                                .clicked()
                                {
                                    state.crossfader = 0.5;
                                }
                            });
                            draw_layer_order(ui, state, &palette, &midi_map, &mut actions, false);
                            ui.horizontal(|ui| {
                                mappable(
                                    ui,
                                    &midi_map,
                                    ControlTarget::MasterOpacity,
                                    &mut actions,
                                    |ui| {
                                        ui.add(
                                            egui::Slider::new(&mut state.master_opacity, 0.0..=1.0)
                                                .text("master")
                                                .clamping(egui::SliderClamping::Always),
                                        )
                                    },
                                );
                                let blackout = mappable(
                                    ui,
                                    &midi_map,
                                    ControlTarget::MasterBlackout,
                                    &mut actions,
                                    |ui| ui.selectable_label(state.blackout, "BLACKOUT"),
                                );
                                if blackout.clicked() {
                                    state.blackout = !state.blackout;
                                }
                                mappable(
                                    ui,
                                    &midi_map,
                                    ControlTarget::MasterFreeze,
                                    &mut actions,
                                    |ui| ui.checkbox(&mut state.master_freeze, "master freeze"),
                                );
                            });
                        },
                    );
                    if !state.show_mode {
                        panel::section(
                            ui,
                            "master-effects",
                            "✦",
                            "MASTER EFFECTS",
                            palette,
                            false,
                            |_| {},
                            |ui| {
                                let effect_packages = &state.effect_packages;
                                let master_effects = &mut state.master_effects;
                                let slot_count = master_effects.slots.len();
                                let mut reorder = None;
                                for index in 0..slot_count {
                                    ui.group(|ui| {
                                        let slot = &mut master_effects.slots[index];
                                        ui.horizontal(|ui| {
                                            ui.monospace(format!("{}", index + 1));
                                            egui::ComboBox::from_id_salt(format!(
                                                "master-fx-kind-{index}"
                                            ))
                                            .selected_text(slot.kind.label())
                                            .show_ui(
                                                ui,
                                                |ui| {
                                                    for kind in MasterEffectKind::ALL {
                                                        ui.selectable_value(
                                                            &mut slot.kind,
                                                            kind,
                                                            kind.label(),
                                                        );
                                                    }
                                                },
                                            );
                                            buttons::midi_toggle(
                                                ui,
                                                &format!("master.slot.{index}.bypass"),
                                                &format!("Master slot {} · Bypass", index + 1),
                                                &mut slot.bypassed,
                                                |ui, value| ui.checkbox(value, "Bypass"),
                                            );
                                            ui.add(
                                                egui::Slider::new(&mut slot.mix, 0.0..=1.0)
                                                    .text("wet"),
                                            );
                                            if buttons::midi_button(
                                                ui,
                                                &format!("master.slot.{index}.up"),
                                                &format!("Master slot {} · Move up", index + 1),
                                                |ui| {
                                                    ui.add_enabled(
                                                        index > 0,
                                                        egui::Button::new("↑"),
                                                    )
                                                },
                                            )
                                            .clicked()
                                            {
                                                reorder = Some((index, index - 1));
                                            }
                                            if buttons::midi_button(
                                                ui,
                                                &format!("master.slot.{index}.down"),
                                                &format!("Master slot {} · Move down", index + 1),
                                                |ui| {
                                                    ui.add_enabled(
                                                        index + 1 < slot_count,
                                                        egui::Button::new("↓"),
                                                    )
                                                },
                                            )
                                            .clicked()
                                            {
                                                reorder = Some((index, index + 1));
                                            }
                                        });
                                        if slot.kind == MasterEffectKind::Blur {
                                            ui.add(
                                                egui::Slider::new(&mut slot.amount, 0.0..=32.0)
                                                    .text("radius px"),
                                            );
                                        } else if slot.kind == MasterEffectKind::Feedback {
                                            ui.add(
                                                egui::Slider::new(&mut slot.feedback, 0.0..=0.99)
                                                    .text("persistence"),
                                            );
                                        } else if slot.kind == MasterEffectKind::Custom {
                                            draw_custom_effect(
                                                ui,
                                                index,
                                                slot,
                                                effect_packages,
                                                palette,
                                                &mut actions,
                                            );
                                        }
                                    });
                                }
                                if let Some((from, to)) = reorder {
                                    master_effects.slots.swap(from, to);
                                }
                                if buttons::midi_button(
                                    ui,
                                    "master.reset_effects",
                                    "Master · Reset effects",
                                    |ui| ui.button("Reset master effects"),
                                )
                                .clicked()
                                {
                                    *master_effects = MasterEffectChain::default();
                                }
                                master_effects.sanitize();
                                ui.weak(
                        "Blur uses fixed ping-pong textures allocated with the composition target.",
                    );
                                draw_master_modulation(
                                    ui,
                                    &mut state.master_modulation,
                                    master_effects,
                                    effect_packages,
                                    palette,
                                    state.master_mod_sources,
                                );
                                ui.separator();
                                ui.label("Effect package");
                                ui.text_edit_singleline(&mut state.effect_manifest_path);
                                ui.horizontal(|ui| {
                                    if buttons::midi_button(
                                        ui,
                                        "master.refresh_registry",
                                        "Master · Refresh effect registry",
                                        |ui| ui.button("Refresh registry"),
                                    )
                                    .clicked()
                                    {
                                        actions.push(UiAction::RefreshEffectRegistry);
                                    }
                                    if buttons::midi_button(
                                        ui,
                                        "master.watch_manifest",
                                        "Master · Watch effect package",
                                        |ui| ui.button("Watch"),
                                    )
                                    .clicked()
                                    {
                                        actions.push(UiAction::WatchEffectManifest);
                                    }
                                    if buttons::midi_button(
                                        ui,
                                        "master.reload_manifest",
                                        "Master · Reload effect package",
                                        |ui| ui.button("Reload now"),
                                    )
                                    .clicked()
                                    {
                                        actions.push(UiAction::ReloadEffectManifest);
                                    }
                                });
                                ui.weak(&state.effect_registry_status);
                                if state.effect_reload_status.contains("rejected") {
                                    ui.colored_label(palette.warning, &state.effect_reload_status);
                                } else {
                                    ui.weak(&state.effect_reload_status);
                                }
                            },
                        );
                    } else {
                        panel::section(
                            ui,
                            "master-effects",
                            "✦",
                            "LIVE MASTER EFFECTS",
                            palette,
                            true,
                            |_| {},
                            |ui| {
                                for (index, slot) in
                                    state.master_effects.slots.iter_mut().enumerate()
                                {
                                    let label = if slot.kind == MasterEffectKind::Custom {
                                        state
                                            .effect_packages
                                            .iter()
                                            .find(|package| package.id == slot.package_id)
                                            .map_or("Missing custom package", |package| {
                                                package.name.as_str()
                                            })
                                    } else {
                                        slot.kind.label()
                                    };
                                    ui.horizontal(|ui| {
                                        ui.monospace(format!("{}", index + 1));
                                        ui.strong(label);
                                        buttons::midi_toggle(
                                            ui,
                                            &format!("master.slot.{index}.bypass"),
                                            &format!("Master slot {} · Bypass", index + 1),
                                            &mut slot.bypassed,
                                            |ui, value| ui.checkbox(value, "Bypass"),
                                        );
                                        ui.add(
                                            egui::Slider::new(&mut slot.mix, 0.0..=1.0).text("wet"),
                                        );
                                    });
                                }
                                state.master_effects.sanitize();
                                ui.weak(
                            "Effect choice, order and advanced parameters are locked in Show Mode.",
                        );
                            },
                        );
                    }
                });
        });
    if floating_browser {
        let mut open = state.browser.open;
        egui::Window::new("Browser · Files / Generators / FX")
            .id(egui::Id::new("floating-browser"))
            .open(&mut open)
            .default_size([290.0, 440.0])
            .show(ctx, |ui| {
                let selected = ClipAddress {
                    deck: mixer.selected(),
                    slot: clips.selected(mixer.selected()),
                };
                browser::draw_browser(
                    ui,
                    &mut state.browser,
                    browser::BrowserContext {
                        palette,
                        show_mode: state.show_mode,
                        selected,
                        effects: &mut state.effects,
                    },
                    &mut actions,
                );
            });
        state.browser.open = open;
    }
    if state.deck_window {
        let mut open = true;
        egui::Window::new("Deck UNIT")
            .id(egui::Id::new("deck-unit-window"))
            .open(&mut open)
            .default_width(440.0)
            .vscroll(true)
            .show(ctx, |ui| {
                let map = buttons::current_map().expect("button frame active");
                draw_deck_section(
                    ui,
                    state,
                    mixer,
                    transports,
                    metrics.generator_stats,
                    palette,
                    &map,
                    &mut actions,
                );
            });
        state.deck_window = open;
    }
    timeline::draw(ctx, state, &metrics, &mut actions);
    if !state.show_mode {
        state.theme.editor_ui(ctx);
    }
    draw_midi_manager(ctx, state, &mut metrics.midi, &palette, &mut actions);
    let generator_map = buttons::current_map().expect("buttons::begin_frame runs first");
    generator::draw_generator_windows(
        ctx,
        state,
        mixer,
        metrics.generator_stats,
        palette,
        &generator_map,
        &mut actions,
    );
    actions.extend(buttons::end_frame());
    actions
}

const LFO_WAVEFORMS: [LfoWaveform; 7] = [
    LfoWaveform::Sine,
    LfoWaveform::Triangle,
    LfoWaveform::Saw,
    LfoWaveform::SawDown,
    LfoWaveform::Square,
    LfoWaveform::SampleHold,
    LfoWaveform::SmoothRandom,
];

fn waveform_label(waveform: LfoWaveform) -> &'static str {
    match waveform {
        LfoWaveform::Sine => "Sine",
        LfoWaveform::Triangle => "Triangle",
        LfoWaveform::Saw => "Saw up",
        LfoWaveform::SawDown => "Saw down",
        LfoWaveform::Square => "Square",
        LfoWaveform::SampleHold => "Sample & hold",
        LfoWaveform::SmoothRandom => "Smooth random",
    }
}

fn deck_label(deck: u8) -> char {
    char::from(b'A'.saturating_add(deck.min(3)))
}

fn effect_parameter_label(effect: u8) -> &'static str {
    [
        "Hue",
        "Contrast",
        "Saturation",
        "Black level",
        "White level",
        "Gamma",
        "Pixelate",
        "Luma key",
        "Neon",
        "Fractal",
        "Jitter",
        "Find edges",
        "Bit reduction",
        "Black light",
        "Bloom",
        "Bloom threshold",
        "Bloom radius",
        "Bloom chroma",
        "Spiral fold",
        "Kali fold",
        "Koch fold",
        "Wave distortion",
        "Vortex distortion",
        "Block jitter",
        "RGB jitter",
        "Julia fold",
        "Polynomial fold",
    ]
    .get(usize::from(effect))
    .copied()
    .unwrap_or("Unknown")
}

fn mapping_mode_label(mode: MappingMode) -> &'static str {
    match mode {
        MappingMode::Continuous => "Absolute",
        MappingMode::Momentary => "Momentary",
        MappingMode::Toggle => "Toggle",
        MappingMode::RelativeBinaryOffset => "Relative offset",
        MappingMode::RelativeTwosComplement => "Relative 2's comp",
    }
}

/// Every control target the MIDI manager lists, for mapping network ids.
pub(crate) fn known_control_targets() -> Vec<ControlTarget> {
    midi::midi_targets()
}

/// The selected deck's performance controls and the other deck editors.
#[allow(clippy::too_many_arguments)]
fn draw_deck_section(
    ui: &mut egui::Ui,
    state: &mut UiState,
    mixer: &mut FourDeckMixer,
    transports: &mut [DeckTransport; 4],
    generator_stats: [virtual_generate::GeneratorStats; 4],
    palette: ThemePalette,
    midi_map: &MidiMapUi,
    actions: &mut Vec<UiAction>,
) {
    let layout = state.theme.deck_layout.resolve(ui.available_width());
    let selected_deck = mixer.selected();
    let transforms_ref = &mut state.transforms;
    let blend_modes_ref = &mut state.blend_modes;
    let solo_ref = &mut state.solo;
    let bypassed_ref = &mut state.bypassed;
    let effects_ref = &mut state.effects;
    let lfos_ref = &mut state.lfos;
    let mod_sources = state.mod_sources;
    let deck_packages_ref = &mut state.deck_packages;
    let deck_effect_packages = &state.deck_effect_packages;
    let generator_windows_ref = &mut state.generator_windows;
    let actions_ref = actions;
    let mut deck_strip = |ui: &mut egui::Ui, deck_id: DeckId| {
        draw_deck(
            ui,
            mixer,
            deck_id,
            DeckControls {
                palette,
                midi_map,
                show_mode: state.show_mode,
                transport: &mut transports[deck_id.index()],
                transform: &mut transforms_ref[deck_id.index()],
                blend_mode: &mut blend_modes_ref[deck_id.index()],
                solo: &mut solo_ref[deck_id.index()],
                bypassed: &mut bypassed_ref[deck_id.index()],
                effects: &mut effects_ref[deck_id.index()],
                lfos: &mut lfos_ref[deck_id.index()],
                mod_sources: mod_sources[deck_id.index()],
                package: &mut deck_packages_ref[deck_id.index()],
                packages: deck_effect_packages,
                generator: generator_stats[deck_id.index()],
                generator_window: &mut generator_windows_ref[deck_id.index()],
            },
            actions_ref,
        );
    };
    let master_freeze = state.master_freeze;
    let show_mode = state.show_mode;
    panel::section(
        ui,
        "selected-deck",
        "◧",
        &format!("DECK {} · PERFORMANCE & FX", selected_deck.label()),
        palette,
        true,
        |_| {},
        |ui| {
            if master_freeze {
                ui.colored_label(
                        palette.warning,
                        "PROGRAM FROZEN · Deck FX and source changes are staged until master freeze is released.",
                    );
            }
            deck_strip(ui, selected_deck);

            if !show_mode {
                egui::CollapsingHeader::new("Other deck editors")
                    .default_open(false)
                    .show(ui, |ui| match layout {
                        ResolvedDeckLayout::Cascade => {
                            egui::ScrollArea::horizontal()
                                .id_salt("other-deck-cascade")
                                .show(ui, |ui| {
                                    ui.horizontal_top(|ui| {
                                        for deck_id in DeckId::ALL
                                            .into_iter()
                                            .filter(|id| *id != selected_deck)
                                        {
                                            ui.allocate_ui_with_layout(
                                                egui::vec2(CASCADE_STRIP_WIDTH, 10.0),
                                                egui::Layout::top_down(egui::Align::Min),
                                                |ui| {
                                                    ui.set_width(CASCADE_STRIP_WIDTH);
                                                    deck_strip(ui, deck_id);
                                                },
                                            );
                                        }
                                    });
                                });
                        }
                        ResolvedDeckLayout::Grid => {
                            egui::Grid::new("other-decks")
                                .num_columns(2)
                                .spacing([12.0, 12.0])
                                .show(ui, |ui| {
                                    for (index, deck_id) in DeckId::ALL
                                        .into_iter()
                                        .filter(|id| *id != selected_deck)
                                        .enumerate()
                                    {
                                        deck_strip(ui, deck_id);
                                        if index % 2 == 1 {
                                            ui.end_row();
                                        }
                                    }
                                });
                        }
                        ResolvedDeckLayout::Stack => {
                            for deck_id in DeckId::ALL.into_iter().filter(|id| *id != selected_deck)
                            {
                                deck_strip(ui, deck_id);
                            }
                        }
                    });
            }
        },
    );
}

/// A monitor's title row: a coloured dot, the name and a detail.
fn monitor_title(
    ui: &mut egui::Ui,
    dot: egui::Color32,
    title: &str,
    detail: &str,
    palette: ThemePalette,
) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("●").color(dot));
        ui.label(
            egui::RichText::new(title)
                .small()
                .strong()
                .color(palette.strong_text),
        );
        ui.add(
            egui::Label::new(
                egui::RichText::new(detail)
                    .small()
                    .color(palette.muted_text),
            )
            .truncate(),
        );
    });
}

/// Paints `texture` letterboxed into the available width, on black.
fn monitor_image(
    ui: &mut egui::Ui,
    texture: Option<(egui::TextureId, [u32; 2])>,
    max_height: f32,
    palette: ThemePalette,
) {
    let available = ui.available_width();
    let box_size = egui::vec2(available, (available * 9.0 / 16.0).min(max_height));
    let (rect, _) = ui.allocate_exact_size(box_size, egui::Sense::hover());
    ui.painter().rect_filled(rect, 2.0, egui::Color32::BLACK);
    if let Some((texture, [width, height])) = texture {
        let aspect = width.max(1) as f32 / height.max(1) as f32;
        let size = if rect.width() / rect.height() > aspect {
            egui::vec2(rect.height() * aspect, rect.height())
        } else {
            egui::vec2(rect.width(), rect.width() / aspect)
        };
        ui.painter().image(
            texture,
            egui::Rect::from_center_size(rect.center(), size),
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
    } else {
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "no clip",
            egui::FontId::proportional(12.0),
            palette.muted_text,
        );
    }
    ui.painter().rect_stroke(
        rect,
        2.0,
        egui::Stroke::new(palette.outline_width.min(1.0), palette.stroke),
        egui::StrokeKind::Inside,
    );
}

/// The selected clip's first frame, downscaled for the preview monitor.
fn preview_image(frame: &virtual_media::RgbaFrame) -> egui::ColorImage {
    let [width, height] = frame.extent.map(|v| v.max(1) as usize);
    let scale = (480.0 / width as f32).min(1.0);
    let (out_width, out_height) = (
        ((width as f32 * scale) as usize).max(1),
        ((height as f32 * scale) as usize).max(1),
    );
    let data = frame.data.as_slice();
    let mut pixels = Vec::with_capacity(out_width * out_height * 4);
    for y in 0..out_height {
        let source_y = (y * height / out_height).min(height - 1);
        for x in 0..out_width {
            let source_x = (x * width / out_width).min(width - 1);
            let at = (source_y * width + source_x) * 4;
            pixels.extend_from_slice(data.get(at..at + 4).unwrap_or(&[0, 0, 0, 255]));
        }
    }
    egui::ColorImage::from_rgba_unmultiplied([out_width, out_height], &pixels)
}

/// Preview (the selected clip, ready to launch) beside Program (the live
/// output), as on a media server.
#[allow(clippy::too_many_arguments)]
fn draw_monitors(
    ui: &mut egui::Ui,
    state: &mut UiState,
    mixer: &FourDeckMixer,
    clips: &ClipBank,
    program: (egui::TextureId, [u32; 2]),
    palette: ThemePalette,
    actions: &mut Vec<UiAction>,
) {
    let deck = mixer.selected();
    let address = ClipAddress {
        deck,
        slot: clips.selected(deck),
    };
    let path = clips.path(address).map(Path::to_path_buf);
    let fresh = matches!(
        (&state.cue_preview, &path),
        (Some((cued, cued_path, _)), Some(path)) if *cued == address && cued_path == path
    );
    if !fresh {
        state.cue_preview = path.as_ref().and_then(|path| {
            let frame = state.preloaded_frame(address, Some(path))?;
            let texture = ui.ctx().load_texture(
                "cue-preview",
                preview_image(frame),
                egui::TextureOptions::LINEAR,
            );
            Some((address, path.clone(), texture))
        });
    }
    let name = clips
        .movie(address)
        .map_or_else(String::new, |movie| movie.display_name.clone());
    let preview = |ui: &mut egui::Ui, state: &UiState, actions: &mut Vec<UiAction>, height: f32| {
        monitor_title(
            ui,
            palette.warning,
            "PREVIEW",
            &format!("{}{} {name}", deck.label(), address.slot + 1),
            palette,
        );
        let texture = state
            .cue_preview
            .as_ref()
            .map(|(_, _, texture)| {
                let [width, height] = texture.size();
                (texture.id(), [width as u32, height as u32])
            })
            .or_else(|| {
                state.thumbnail(address, path.as_deref()).map(|texture| {
                    let [width, height] = texture.size();
                    (texture.id(), [width as u32, height as u32])
                })
            });
        monitor_image(ui, texture, height, palette);
        if ui
            .add_enabled(
                clips.movie(address).is_some(),
                egui::Button::new(egui::RichText::new("▶ Launch").small()),
            )
            .on_hover_text("Launch the previewed clip")
            .clicked()
        {
            actions.push(UiAction::Launch(address));
        }
    };
    let (texture, extent) = program;
    let program_detail = format!(
        "{}×{}{}{}",
        extent[0],
        extent[1],
        if state.blackout { " · BLACKOUT" } else { "" },
        if state.master_freeze {
            " · FROZEN"
        } else {
            ""
        }
    );
    if ui.available_width() >= 420.0 {
        ui.columns(2, |columns| {
            preview(&mut columns[0], state, actions, 200.0);
            monitor_title(
                &mut columns[1],
                palette.danger,
                "PROGRAM",
                &program_detail,
                palette,
            );
            monitor_image(&mut columns[1], Some((texture, extent)), 200.0, palette);
        });
    } else {
        monitor_title(ui, palette.danger, "PROGRAM", &program_detail, palette);
        monitor_image(ui, Some((texture, extent)), 260.0, palette);
        preview(ui, state, actions, 160.0);
    }
}

#[cfg(test)]
mod layer_tests {
    use super::{layer_step, layer_to_top};

    #[test]
    fn layer_helpers_reorder_without_dropping_decks() {
        let mut order = [0, 1, 2, 3];
        layer_to_top(&mut order, 1);
        assert_eq!(order, [0, 2, 3, 1]);
        layer_to_top(&mut order, 1);
        assert_eq!(order, [0, 2, 3, 1]);
        layer_step(&mut order, 0, false);
        assert_eq!(order, [0, 2, 3, 1], "bottom layer cannot move down");
        layer_step(&mut order, 1, true);
        assert_eq!(order, [0, 2, 3, 1], "top layer cannot move up");
        layer_step(&mut order, 1, false);
        assert_eq!(order, [0, 2, 1, 3]);
        layer_step(&mut order, 0, true);
        assert_eq!(order, [2, 0, 1, 3]);
        layer_to_top(&mut order, 9);
        assert_eq!(order, [2, 0, 1, 3], "unknown decks are ignored");
    }
}
