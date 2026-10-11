//! Graphing calculator: a GeoGebra-style algebra list, 2D and 3D views,
//! calculus overlays, a value table and an on-screen keypad. Parsing and
//! sampling live in `virtual_generate::graphing`; this module draws them in
//! the active theme's colours.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use virtual_generate::graphing::{
    self, Plot, PlotKind, Primitives, Sampled3, Scope, View, classify,
};
use virtual_generate::{GeometryGraph, GeometryNodeKind};

use super::theme::ThemePalette;

const DEFAULT_VIEW: View = View {
    x: [-10.0, 10.0],
    y: [-6.0, 6.0],
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewMode {
    Graph2d,
    Graph3d,
}

/// Where the keypad types.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Target {
    Item(usize),
    Input,
}

/// One algebra-list object.
pub struct GraphItem {
    pub source: String,
    pub color: usize,
    pub visible: bool,
    pub derivative: bool,
    /// Roots, extrema and intersections.
    pub points: bool,
    /// Shade and measure the integral over this x range.
    pub area: Option<[f32; 2]>,
    /// Parameter range for parametric (t) and polar (theta) curves.
    pub parameter: [f32; 2],
    /// Slider limits.
    pub range: [f32; 2],
    cache: Option<(u64, Primitives)>,
}

impl GraphItem {
    fn new(source: impl Into<String>, color: usize) -> Self {
        Self {
            source: source.into(),
            color,
            visible: true,
            derivative: false,
            points: false,
            area: None,
            parameter: [0.0, std::f32::consts::TAU],
            range: [-5.0, 5.0],
            cache: None,
        }
    }
}

/// Per-deck calculator state (session only).
pub struct GraphingCalculator {
    pub items: Vec<GraphItem>,
    pub mode: ViewMode,
    pub view: View,
    pub yaw: f32,
    pub pitch: f32,
    pub zoom_3d: f32,
    pub animate: bool,
    pub time: f32,
    pub input: String,
    pub show_table: bool,
    pub table_step: f32,
    pub window_open: bool,
    active: usize,
    target: Target,
    next_color: usize,
}

impl Default for GraphingCalculator {
    fn default() -> Self {
        Self {
            items: vec![GraphItem::new("f(x) = sin(x)", 0)],
            mode: ViewMode::Graph2d,
            view: DEFAULT_VIEW,
            yaw: 0.7,
            pitch: 0.45,
            zoom_3d: 1.0,
            animate: false,
            time: 0.0,
            input: String::new(),
            show_table: false,
            table_step: 1.0,
            window_open: false,
            active: 0,
            target: Target::Input,
            next_color: 1,
        }
    }
}

impl GraphingCalculator {
    fn add(&mut self, source: String) {
        self.items.push(GraphItem::new(source, self.next_color));
        self.next_color += 1;
        self.active = self.items.len() - 1;
    }

    /// Slider names and values defined anywhere in the list.
    fn scope(&self) -> Scope {
        let mut scope = Scope {
            time: self.time,
            ..Scope::default()
        };
        for item in &self.items {
            if let Ok(PlotKind::Slider { name, value }) = classify(&item.source)
                && !scope.names.contains(&name)
            {
                scope.names.push(name);
                scope.values.push(value);
            }
        }
        scope
    }
}

fn series(palette: &ThemePalette, index: usize) -> egui::Color32 {
    let colors = [
        palette.accent,
        palette.secondary,
        palette.success,
        palette.warning,
        palette.danger,
        palette.deck[0],
        palette.deck[1],
        palette.deck[2],
        palette.deck[3],
    ];
    colors[index % colors.len()]
}

fn with_alpha(color: egui::Color32, alpha: u8) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha)
}

fn item_id(salt: egui::Id, index: usize) -> egui::Id {
    salt.with(("calculator-item", index))
}

/// Inserts `token` at the text cursor of the edit `id`, replacing any
/// selection, and leaves the cursor `back` characters before its end.
fn insert(ctx: &egui::Context, id: egui::Id, text: &mut String, token: &str, back: usize) {
    let mut state = egui::text_edit::TextEditState::load(ctx, id).unwrap_or_default();
    let chars = text.chars().count();
    let (start, end) = state.cursor.char_range().map_or((chars, chars), |range| {
        let (a, b) = (
            usize::from(range.primary.index),
            usize::from(range.secondary.index),
        );
        (a.min(b).min(chars), a.max(b).min(chars))
    });
    let byte = |index: usize| {
        text.char_indices()
            .nth(index)
            .map_or(text.len(), |(b, _)| b)
    };
    let (from, to) = (byte(start), byte(end));
    if token == "\u{8}" {
        // Backspace: delete the selection, or the character before the cursor.
        let from = if from == to && start > 0 {
            byte(start - 1)
        } else {
            from
        };
        text.replace_range(from..to, "");
        let cursor = text[..from].chars().count();
        state
            .cursor
            .set_char_range(Some(egui::text::CCursorRange::one(
                egui::text::CCursor::new(cursor),
            )));
    } else {
        text.replace_range(from..to, token);
        let cursor = start + token.chars().count() - back;
        state
            .cursor
            .set_char_range(Some(egui::text::CCursorRange::one(
                egui::text::CCursor::new(cursor),
            )));
    }
    state.store(ctx, id);
    ctx.memory_mut(|memory| memory.request_focus(id));
}

/// Draws the calculator. `graph` enables applying the active object to the
/// focused geometry node. Returns true when the geometry graph changed.
pub(super) fn draw(
    ui: &mut egui::Ui,
    calc: &mut GraphingCalculator,
    graph: Option<&mut GeometryGraph>,
    selected_node: u64,
    palette: ThemePalette,
    salt: egui::Id,
) -> bool {
    if calc.animate {
        calc.time += ui.input(|input| input.stable_dt).min(0.1);
    }
    let mut changed = false;
    ui.horizontal_wrapped(|ui| {
        ui.selectable_value(&mut calc.mode, ViewMode::Graph2d, "2D graph");
        ui.selectable_value(&mut calc.mode, ViewMode::Graph3d, "3D graph");
        ui.separator();
        ui.toggle_value(&mut calc.animate, "▶ Animate time")
            .on_hover_text("Advances `time` (seconds) for every expression that uses it");
        ui.label(
            egui::RichText::new(format!("time = {:.2}", calc.time))
                .monospace()
                .color(palette.muted_text),
        );
        if ui.small_button("Reset time").clicked() {
            calc.time = 0.0;
        }
        ui.separator();
        if ui.button("Reset view").clicked() {
            calc.view = DEFAULT_VIEW;
            calc.yaw = 0.7;
            calc.pitch = 0.45;
            calc.zoom_3d = 1.0;
        }
        changed |= apply_to_graph(ui, calc, graph, selected_node);
    });
    let wide = ui.available_width() >= 820.0;
    if wide {
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(340.0);
                draw_algebra(ui, calc, &palette, salt);
                draw_keypad(ui, calc, &palette, salt);
            });
            ui.vertical(|ui| draw_views(ui, calc, &palette));
        });
    } else {
        draw_algebra(ui, calc, &palette, salt);
        draw_views(ui, calc, &palette);
        draw_keypad(ui, calc, &palette, salt);
    }
    changed
}

fn draw_views(ui: &mut egui::Ui, calc: &mut GraphingCalculator, palette: &ThemePalette) {
    let scope = calc.scope();
    match calc.mode {
        ViewMode::Graph2d => draw_plot_2d(ui, calc, &scope, palette),
        ViewMode::Graph3d => draw_plot_3d(ui, calc, &scope, palette),
    }
    draw_analysis(ui, calc, &scope, palette);
}

/// The algebra list: one row per object, then the input line.
fn draw_algebra(
    ui: &mut egui::Ui,
    calc: &mut GraphingCalculator,
    palette: &ThemePalette,
    salt: egui::Id,
) {
    ui.label(
        egui::RichText::new("ALGEBRA")
            .strong()
            .color(palette.strong_text),
    );
    let mut remove = None;
    let mut slider_updates = Vec::new();
    for (index, item) in calc.items.iter_mut().enumerate() {
        let color = series(palette, item.color);
        let kind = classify(&item.source);
        ui.horizontal(|ui| {
            let (swatch, response) =
                ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::click());
            ui.painter().circle(
                swatch.center(),
                6.0,
                if item.visible {
                    color
                } else {
                    egui::Color32::TRANSPARENT
                },
                egui::Stroke::new(1.5, color),
            );
            if response
                .on_hover_text("Click to show or hide · right-click for the next colour")
                .clicked()
            {
                item.visible = !item.visible;
            }
            if ui.rect_contains_pointer(swatch) && ui.input(|i| i.pointer.secondary_clicked()) {
                item.color += 1;
            }
            let edit = ui.add(
                egui::TextEdit::singleline(&mut item.source)
                    .id(item_id(salt, index))
                    .font(egui::TextStyle::Monospace)
                    .text_color(palette.text)
                    .background_color(palette.extreme)
                    .desired_width(230.0),
            );
            if edit.has_focus() || edit.gained_focus() {
                calc.target = Target::Item(index);
                calc.active = index;
            }
            if edit.changed() {
                item.cache = None;
            }
            if ui.small_button("✕").on_hover_text("Delete").clicked() {
                remove = Some(index);
            }
        });
        match &kind {
            Ok(kind) => {
                ui.horizontal_wrapped(|ui| {
                    ui.add_space(20.0);
                    ui.label(
                        egui::RichText::new(kind.label())
                            .small()
                            .color(palette.muted_text),
                    );
                    match kind {
                        PlotKind::Slider { name, value } => {
                            let mut v = *value;
                            let [lo, hi] = item.range;
                            if ui
                                .add(
                                    egui::Slider::new(&mut v, lo.min(hi)..=hi.max(lo))
                                        .clamping(egui::SliderClamping::Never),
                                )
                                .changed()
                            {
                                slider_updates
                                    .push((index, format!("{name} = {}", trim_number(v))));
                            }
                            ui.add(
                                egui::DragValue::new(&mut item.range[0])
                                    .prefix("min ")
                                    .speed(0.1),
                            );
                            ui.add(
                                egui::DragValue::new(&mut item.range[1])
                                    .prefix("max ")
                                    .speed(0.1),
                            );
                        }
                        PlotKind::Function(_) => {
                            ui.toggle_value(&mut item.derivative, "f′");
                            ui.toggle_value(&mut item.points, "roots/extrema");
                            let mut shade = item.area.is_some();
                            if ui.toggle_value(&mut shade, "∫ area").changed() {
                                item.area = shade.then_some([0.0, 1.0]);
                            }
                            if let Some(area) = &mut item.area {
                                ui.add(egui::DragValue::new(&mut area[0]).prefix("a ").speed(0.05));
                                ui.add(egui::DragValue::new(&mut area[1]).prefix("b ").speed(0.05));
                            }
                        }
                        PlotKind::Parametric(..)
                        | PlotKind::Parametric3(..)
                        | PlotKind::Polar(_) => {
                            let label = if matches!(kind, PlotKind::Polar(_)) {
                                "θ"
                            } else {
                                "t"
                            };
                            ui.label(egui::RichText::new(label).color(palette.muted_text));
                            ui.add(egui::DragValue::new(&mut item.parameter[0]).speed(0.05));
                            ui.label("…");
                            ui.add(egui::DragValue::new(&mut item.parameter[1]).speed(0.05));
                        }
                        _ => {}
                    }
                });
            }
            Err(error) if !item.source.trim().is_empty() => {
                ui.horizontal(|ui| {
                    ui.add_space(20.0);
                    ui.colored_label(palette.danger, egui::RichText::new(error).small());
                });
            }
            Err(_) => {}
        }
    }
    for (index, source) in slider_updates {
        calc.items[index].source = source;
    }
    if let Some(index) = remove {
        calc.items.remove(index);
        calc.active = calc.active.min(calc.items.len().saturating_sub(1));
        calc.target = Target::Input;
    }
    ui.horizontal(|ui| {
        let id = salt.with("calculator-input");
        let response = ui.add(
            egui::TextEdit::singleline(&mut calc.input)
                .id(id)
                .font(egui::TextStyle::Monospace)
                .text_color(palette.text)
                .background_color(palette.extreme)
                .hint_text("Input: y=x^2 · x^2+y^2=9 · (cos(t),sin(t)) · z=…")
                .desired_width(270.0),
        );
        if response.has_focus() || response.gained_focus() {
            calc.target = Target::Input;
        }
        let entered = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if (ui.button("Add").clicked() || entered) && !calc.input.trim().is_empty() {
            let source = std::mem::take(&mut calc.input);
            calc.add(source);
            ui.memory_mut(|memory| memory.request_focus(id));
        }
    });
    ui.collapsing("Examples", |ui| {
        ui.label(
            egui::RichText::new("Click to add")
                .small()
                .color(palette.muted_text),
        );
        ui.horizontal_wrapped(|ui| {
            for example in [
                "y = a sin(b x)",
                "a = 1",
                "b = 2",
                "x^2/9 + y^2/4 = 1",
                "y < x^2 - 2",
                "r = 2 sin(4θ)",
                "(cos(3t), sin(2t))",
                "(-y, x)",
                "dy/dx = y - x",
                "z = sin(x) cos(y)",
                "(cos(t), sin(t), t/5)",
                "y = e^(-0.2x) cos(3x - time)",
                "(10 t cos(0.8), 10 t sin(0.8) - 4.9t^2)",
            ] {
                if ui
                    .small_button(egui::RichText::new(example).monospace())
                    .clicked()
                {
                    calc.add(example.to_owned());
                }
            }
        });
    });
}

fn trim_number(value: f32) -> String {
    let text = format!("{value:.3}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// The keypad: a 0–9 dial pad, operators, variables, functions and templates.
fn draw_keypad(
    ui: &mut egui::Ui,
    calc: &mut GraphingCalculator,
    palette: &ThemePalette,
    salt: egui::Id,
) {
    let mut pressed: Option<(&str, usize)> = None;
    let key = |ui: &mut egui::Ui, label: &str, width: f32| {
        ui.add(
            egui::Button::new(egui::RichText::new(label).monospace().color(palette.text))
                .fill(palette.control)
                .min_size(egui::vec2(width, 26.0)),
        )
        .clicked()
    };
    ui.add_space(4.0);
    ui.label(
        egui::RichText::new("KEYPAD")
            .strong()
            .color(palette.strong_text),
    );
    ui.horizontal_top(|ui| {
        // Dial pad, laid out like a keyboard's number pad.
        egui::Grid::new(salt.with("numpad"))
            .spacing([3.0, 3.0])
            .show(ui, |ui| {
                for row in [
                    ["7", "8", "9"],
                    ["4", "5", "6"],
                    ["1", "2", "3"],
                    ["0", ".", "⌫"],
                ] {
                    for label in row {
                        if key(ui, label, 30.0) {
                            pressed = Some(if label == "⌫" {
                                ("\u{8}", 0)
                            } else {
                                (label, 0)
                            });
                        }
                    }
                    ui.end_row();
                }
            });
        ui.add_space(6.0);
        egui::Grid::new(salt.with("operators"))
            .spacing([3.0, 3.0])
            .show(ui, |ui| {
                for row in [
                    [("÷", "/"), ("×", "*")],
                    [("−", "-"), ("+", "+")],
                    [("^", "^"), ("=", "=")],
                    [("(", "("), (")", ")")],
                ] {
                    for (label, token) in row {
                        if key(ui, label, 28.0) {
                            pressed = Some((token, 0));
                        }
                    }
                    ui.end_row();
                }
            });
        ui.add_space(6.0);
        egui::Grid::new(salt.with("variables"))
            .spacing([3.0, 3.0])
            .show(ui, |ui| {
                for row in [
                    [("x", "x"), ("y", "y")],
                    [("z", "z"), ("t", "t")],
                    [("θ", "θ"), ("time", "time")],
                    [(",", ", "), ("<", "<")],
                ] {
                    for (label, token) in row {
                        if key(ui, label, 34.0) {
                            pressed = Some((token, 0));
                        }
                    }
                    ui.end_row();
                }
            });
    });
    ui.add_space(3.0);
    egui::Grid::new(salt.with("functions"))
        .spacing([3.0, 3.0])
        .show(ui, |ui| {
            for row in [
                ["sin", "cos", "tan", "√", "abs", "π"],
                ["asin", "acos", "atan", "∛", "ln", "e"],
                ["sinh", "cosh", "tanh", "exp", "log10", "τ"],
                ["floor", "ceil", "mod", "min", "max", "x²"],
            ] {
                for label in row {
                    if key(ui, label, 48.0) {
                        pressed = Some(match label {
                            "√" => ("sqrt()", 1),
                            "∛" => ("cbrt()", 1),
                            "π" | "e" | "τ" => (label, 0),
                            "x²" => ("^2", 0),
                            "mod" | "min" | "max" => match label {
                                "mod" => ("mod(, )", 3),
                                "min" => ("min(, )", 3),
                                _ => ("max(, )", 3),
                            },
                            name => (
                                match name {
                                    "sin" => "sin()",
                                    "cos" => "cos()",
                                    "tan" => "tan()",
                                    "abs" => "abs()",
                                    "asin" => "asin()",
                                    "acos" => "acos()",
                                    "atan" => "atan()",
                                    "ln" => "ln()",
                                    "sinh" => "sinh()",
                                    "cosh" => "cosh()",
                                    "tanh" => "tanh()",
                                    "exp" => "exp()",
                                    "log10" => "log10()",
                                    "floor" => "floor()",
                                    _ => "ceil()",
                                },
                                1,
                            ),
                        });
                    }
                }
                ui.end_row();
            }
        });
    ui.horizontal_wrapped(|ui| {
        for (label, token, back) in [
            ("y =", "y = ", 0),
            ("z =", "z = ", 0),
            ("r =", "r = ", 0),
            ("(x, y)", "(, )", 3),
            ("(x, y, z)", "(, , )", 5),
            ("dy/dx =", "dy/dx = ", 0),
            ("≤", "<=", 0),
            ("≥", ">=", 0),
        ] {
            if ui
                .add(
                    egui::Button::new(egui::RichText::new(label).monospace().color(palette.text))
                        .fill(palette.control),
                )
                .clicked()
            {
                pressed = Some((token, back));
            }
        }
        if ui.button("Clear").clicked() {
            match calc.target {
                Target::Item(index) if index < calc.items.len() => calc.items[index].source.clear(),
                _ => calc.input.clear(),
            }
        }
        if ui.button("Enter").clicked() && !calc.input.trim().is_empty() {
            let source = std::mem::take(&mut calc.input);
            calc.add(source);
        }
    });
    if let Some((token, back)) = pressed {
        let ctx = ui.ctx().clone();
        match calc.target {
            Target::Item(index) if index < calc.items.len() => {
                insert(
                    &ctx,
                    item_id(salt, index),
                    &mut calc.items[index].source,
                    token,
                    back,
                );
                calc.items[index].cache = None;
            }
            _ => insert(
                &ctx,
                salt.with("calculator-input"),
                &mut calc.input,
                token,
                back,
            ),
        }
    }
}

struct Frame2d {
    rect: egui::Rect,
    view: View,
}

impl Frame2d {
    fn to_screen(&self, [x, y]: [f32; 2]) -> egui::Pos2 {
        egui::pos2(
            self.rect.left()
                + (x - self.view.x[0]) / (self.view.x[1] - self.view.x[0]) * self.rect.width(),
            self.rect.bottom()
                - (y - self.view.y[0]) / (self.view.y[1] - self.view.y[0]) * self.rect.height(),
        )
    }

    fn to_world(&self, pos: egui::Pos2) -> [f32; 2] {
        [
            self.view.x[0]
                + (pos.x - self.rect.left()) / self.rect.width()
                    * (self.view.x[1] - self.view.x[0]),
            self.view.y[0]
                + (self.rect.bottom() - pos.y) / self.rect.height()
                    * (self.view.y[1] - self.view.y[0]),
        ]
    }
}

fn tick_step(span: f32) -> f32 {
    let raw = span / 10.0;
    let magnitude = 10.0_f32.powf(raw.abs().max(1.0e-6).log10().floor());
    [1.0, 2.0, 5.0, 10.0]
        .into_iter()
        .map(|m| m * magnitude)
        .find(|step| raw <= *step)
        .unwrap_or(10.0 * magnitude)
}

fn format_tick(value: f32, step: f32) -> String {
    let decimals = (-step.log10().floor()).max(0.0) as usize;
    format!("{value:.decimals$}")
}

fn cache_key(item: &GraphItem, kind: &PlotKind, scope: &Scope, view: View, width: usize) -> u64 {
    let mut hasher = DefaultHasher::new();
    item.source.hash(&mut hasher);
    for value in view
        .x
        .iter()
        .chain(&view.y)
        .chain(&item.parameter)
        .chain(&scope.values)
    {
        value.to_bits().hash(&mut hasher);
    }
    scope.names.hash(&mut hasher);
    width.hash(&mut hasher);
    if item.source.contains("time") {
        scope.time.to_bits().hash(&mut hasher);
    }
    kind.label().hash(&mut hasher);
    hasher.finish()
}

fn draw_plot_2d(
    ui: &mut egui::Ui,
    calc: &mut GraphingCalculator,
    scope: &Scope,
    palette: &ThemePalette,
) {
    let size = egui::vec2(
        ui.available_width().max(320.0),
        (ui.available_width() * 0.62).clamp(260.0, 620.0),
    );
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 6.0, palette.extreme);
    let frame = Frame2d {
        rect,
        view: calc.view,
    };

    // Grid and axes.
    let (x_step, y_step) = (
        tick_step(calc.view.x[1] - calc.view.x[0]),
        tick_step(calc.view.y[1] - calc.view.y[0]),
    );
    let grid = with_alpha(palette.stroke, 90);
    let axis = palette.muted_text;
    let label_font = egui::FontId::monospace(10.0);
    let mut x = (calc.view.x[0] / x_step).ceil() * x_step;
    while x <= calc.view.x[1] {
        let px = frame.to_screen([x, 0.0]).x;
        let on_axis = x.abs() < x_step * 1e-3;
        painter.line_segment(
            [egui::pos2(px, rect.top()), egui::pos2(px, rect.bottom())],
            egui::Stroke::new(
                if on_axis { 1.4 } else { 0.6 },
                if on_axis { axis } else { grid },
            ),
        );
        if !on_axis {
            let y_label = frame
                .to_screen([0.0, 0.0])
                .y
                .clamp(rect.top() + 2.0, rect.bottom() - 14.0);
            painter.text(
                egui::pos2(px + 2.0, y_label + 2.0),
                egui::Align2::LEFT_TOP,
                format_tick(x, x_step),
                label_font.clone(),
                axis,
            );
        }
        x += x_step;
    }
    let mut y = (calc.view.y[0] / y_step).ceil() * y_step;
    while y <= calc.view.y[1] {
        let py = frame.to_screen([0.0, y]).y;
        let on_axis = y.abs() < y_step * 1e-3;
        painter.line_segment(
            [egui::pos2(rect.left(), py), egui::pos2(rect.right(), py)],
            egui::Stroke::new(
                if on_axis { 1.4 } else { 0.6 },
                if on_axis { axis } else { grid },
            ),
        );
        if !on_axis {
            let x_label = frame
                .to_screen([0.0, 0.0])
                .x
                .clamp(rect.left() + 2.0, rect.right() - 40.0);
            painter.text(
                egui::pos2(x_label + 3.0, py - 1.0),
                egui::Align2::LEFT_BOTTOM,
                format_tick(y, y_step),
                label_font.clone(),
                axis,
            );
        }
        y += y_step;
    }

    // Objects.
    let width = rect.width() as usize;
    let mut functions: Vec<(usize, Plot)> = Vec::new();
    for (index, item) in calc.items.iter_mut().enumerate() {
        let Ok(kind) = classify(&item.source) else {
            continue;
        };
        if !item.visible || kind.is_3d() || matches!(kind, PlotKind::Slider { .. }) {
            continue;
        }
        let Ok(plot) = Plot::new(kind.clone(), scope) else {
            continue;
        };
        let key = cache_key(item, &kind, scope, calc.view, width);
        if item.cache.as_ref().is_none_or(|(cached, _)| *cached != key) {
            item.cache = Some((key, plot.sample(scope, calc.view, width, item.parameter)));
        }
        let color = series(palette, item.color);
        let primitives = &item.cache.as_ref().expect("filled above").1;
        draw_primitives(&painter, &frame, primitives, color, palette);
        if let PlotKind::Function(_) = kind {
            if let Some([a, b]) = item.area {
                shade_area(&painter, &frame, &plot, scope, [a.min(b), a.max(b)], color);
            }
            if item.derivative {
                let samples = width.clamp(64, 1600);
                let mut line = Vec::new();
                for i in 0..=samples {
                    let x = calc.view.x[0]
                        + (calc.view.x[1] - calc.view.x[0]) * i as f32 / samples as f32;
                    let slope = graphing::derivative(|x| plot.evaluate_function(scope, x), x);
                    if slope.is_finite() && slope.abs() < 1e6 {
                        line.push(frame.to_screen([x, slope]));
                    } else if line.len() > 1 {
                        painter.add(egui::Shape::dashed_line(
                            &std::mem::take(&mut line),
                            egui::Stroke::new(1.5, with_alpha(color, 200)),
                            6.0,
                            4.0,
                        ));
                    } else {
                        line.clear();
                    }
                }
                if line.len() > 1 {
                    painter.add(egui::Shape::dashed_line(
                        &line,
                        egui::Stroke::new(1.5, with_alpha(color, 200)),
                        6.0,
                        4.0,
                    ));
                }
            }
            functions.push((index, plot));
        }
    }

    // Roots, extrema and intersections of the functions that ask for them.
    let range = calc.view.x;
    for (index, plot) in &functions {
        if !calc.items[*index].points {
            continue;
        }
        let f = |x| plot.evaluate_function(scope, x);
        let color = series(palette, calc.items[*index].color);
        for x in graphing::roots(f, range, 400) {
            mark(&painter, &frame, [x, 0.0], color, false, palette);
        }
        for point in graphing::extrema(f, range, 400) {
            mark(&painter, &frame, point, color, true, palette);
        }
        for (other, other_plot) in &functions {
            if other <= index {
                continue;
            }
            let g = |x| other_plot.evaluate_function(scope, x);
            for x in graphing::roots(|x| f(x) - g(x), range, 400) {
                mark(&painter, &frame, [x, f(x)], palette.text, true, palette);
            }
        }
    }

    // Hover readout.
    if let Some(pointer) = response.hover_pos() {
        let world = frame.to_world(pointer);
        let mut readout = format!("x = {:.3}   y = {:.3}", world[0], world[1]);
        if let Some((index, plot)) = functions.get(
            functions
                .iter()
                .position(|(index, _)| *index == calc.active)
                .unwrap_or(0),
        ) {
            let value = plot.evaluate_function(scope, world[0]);
            if value.is_finite() {
                let color = series(palette, calc.items[*index].color);
                painter.circle_filled(frame.to_screen([world[0], value]), 3.5, color);
                readout.push_str(&format!("   f({:.3}) = {:.4}", world[0], value));
            }
        }
        painter.text(
            rect.left_bottom() + egui::vec2(8.0, -6.0),
            egui::Align2::LEFT_BOTTOM,
            readout,
            egui::FontId::monospace(11.0),
            palette.text,
        );
    }

    // Pan and zoom about the cursor.
    if response.dragged() {
        let delta = response.drag_delta();
        let dx = delta.x / rect.width() * (calc.view.x[1] - calc.view.x[0]);
        let dy = delta.y / rect.height() * (calc.view.y[1] - calc.view.y[0]);
        calc.view.x = [calc.view.x[0] - dx, calc.view.x[1] - dx];
        calc.view.y = [calc.view.y[0] + dy, calc.view.y[1] + dy];
    }
    if response.hovered() {
        let scroll = ui.input(|input| input.smooth_scroll_delta.y);
        if scroll != 0.0
            && let Some(pointer) = response.hover_pos()
        {
            let [cx, cy] = frame.to_world(pointer);
            let factor = (-scroll * 0.002).exp().clamp(0.5, 2.0);
            let zoom = |[a, b]: [f32; 2], c: f32| [c + (a - c) * factor, c + (b - c) * factor];
            let (nx, ny) = (zoom(calc.view.x, cx), zoom(calc.view.y, cy));
            if (nx[1] - nx[0]).abs() > 1e-4 && (nx[1] - nx[0]).abs() < 1e6 {
                calc.view = View { x: nx, y: ny };
            }
        }
    }
    if response.double_clicked() {
        calc.view = DEFAULT_VIEW;
    }
    ui.label(
        egui::RichText::new("Drag to pan · scroll to zoom at the cursor · double-click to reset")
            .small()
            .color(palette.muted_text),
    );
}

fn mark(
    painter: &egui::Painter,
    frame: &Frame2d,
    point: [f32; 2],
    color: egui::Color32,
    filled: bool,
    palette: &ThemePalette,
) {
    let pos = frame.to_screen(point);
    if !frame.rect.contains(pos) {
        return;
    }
    if filled {
        painter.circle_filled(pos, 4.0, color);
    } else {
        painter.circle(pos, 4.0, palette.extreme, egui::Stroke::new(2.0, color));
    }
    painter.text(
        pos + egui::vec2(6.0, -6.0),
        egui::Align2::LEFT_BOTTOM,
        format!("({:.3}, {:.3})", point[0], point[1]),
        egui::FontId::monospace(10.0),
        palette.text,
    );
}

fn draw_primitives(
    painter: &egui::Painter,
    frame: &Frame2d,
    p: &Primitives,
    color: egui::Color32,
    palette: &ThemePalette,
) {
    for [min, max] in &p.cells {
        painter.rect_filled(
            egui::Rect::from_two_pos(frame.to_screen(*min), frame.to_screen(*max)),
            0.0,
            with_alpha(color, 52),
        );
    }
    // Vector and slope fields carry arrows or ticks plus streamlines; draw
    // those lighter so curves stay dominant.
    let field = !p.arrows.is_empty() || (!p.segments.is_empty() && !p.polylines.is_empty());
    let curve = egui::Stroke::new(2.0, color);
    let light = egui::Stroke::new(1.2, with_alpha(color, 150));
    for line in &p.polylines {
        painter.add(egui::Shape::line(
            line.iter().map(|&q| frame.to_screen(q)).collect(),
            if field { light } else { curve },
        ));
    }
    for [a, b] in &p.segments {
        painter.line_segment(
            [frame.to_screen(*a), frame.to_screen(*b)],
            if field { light } else { curve },
        );
    }
    for [base, tip] in &p.arrows {
        let (a, b) = (frame.to_screen(*base), frame.to_screen(*tip));
        painter.arrow(a, b - a, egui::Stroke::new(1.3, with_alpha(color, 220)));
    }
    for point in &p.points {
        let pos = frame.to_screen(*point);
        painter.circle(pos, 5.0, color, egui::Stroke::new(1.5, palette.extreme));
        painter.text(
            pos + egui::vec2(7.0, -7.0),
            egui::Align2::LEFT_BOTTOM,
            format!("({:.2}, {:.2})", point[0], point[1]),
            egui::FontId::monospace(10.0),
            palette.text,
        );
    }
}

fn shade_area(
    painter: &egui::Painter,
    frame: &Frame2d,
    plot: &Plot,
    scope: &Scope,
    [a, b]: [f32; 2],
    color: egui::Color32,
) {
    let strips = 240;
    let fill = with_alpha(color, 70);
    let width = (b - a) / strips as f32;
    for i in 0..strips {
        let x = a + (i as f32 + 0.5) * width;
        let y = plot.evaluate_function(scope, x);
        if y.is_finite() {
            painter.rect_filled(
                egui::Rect::from_two_pos(
                    frame.to_screen([x - width * 0.5, 0.0]),
                    frame.to_screen([x + width * 0.5, y]),
                ),
                0.0,
                fill,
            );
        }
    }
}

/// Projects normalised scene coordinates onto the 3D view.
struct Camera3d {
    rect: egui::Rect,
    yaw: f32,
    pitch: f32,
    zoom: f32,
    centre: [f32; 3],
    half: [f32; 3],
}

impl Camera3d {
    fn project(&self, [x, y, z]: [f32; 3]) -> Option<(egui::Pos2, f32)> {
        if !(x.is_finite() && y.is_finite() && z.is_finite()) {
            return None;
        }
        let n = [
            (x - self.centre[0]) / self.half[0],
            (y - self.centre[1]) / self.half[1],
            (z - self.centre[2]) / self.half[2],
        ];
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        // Z is up. Yaw spins about Z, pitch tilts the view down onto it.
        let rx = n[0] * cy - n[1] * sy;
        let ry = n[0] * sy + n[1] * cy;
        let depth = ry * cp + n[2] * sp;
        let up = n[2] * cp - ry * sp;
        let perspective = 3.2 / (3.2 + depth);
        let scale = self.rect.width().min(self.rect.height()) * 0.33 * self.zoom * perspective;
        Some((
            self.rect.center() + egui::vec2(rx * scale, -up * scale),
            depth,
        ))
    }
}

fn draw_plot_3d(
    ui: &mut egui::Ui,
    calc: &mut GraphingCalculator,
    scope: &Scope,
    palette: &ThemePalette,
) {
    let size = egui::vec2(
        ui.available_width().max(320.0),
        (ui.available_width() * 0.7).clamp(300.0, 680.0),
    );
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 6.0, palette.extreme);
    let domain = calc.view;

    // Sample 3D objects first so the z range fits what is visible.
    let mut sampled: Vec<(usize, PlotKind, Sampled3)> = Vec::new();
    for (index, item) in calc.items.iter().enumerate() {
        let Ok(kind) = classify(&item.source) else {
            continue;
        };
        if !item.visible || !kind.is_3d() {
            continue;
        }
        if let Ok(plot) = Plot::new(kind.clone(), scope) {
            sampled.push((
                index,
                kind,
                plot.sample_3d(scope, domain, 48, item.parameter),
            ));
        }
    }
    let mut z_range = [f32::INFINITY, f32::NEG_INFINITY];
    for (_, _, s) in &sampled {
        for p in s.grid.iter().chain(&s.curve) {
            if p[2].is_finite() {
                z_range = [z_range[0].min(p[2]), z_range[1].max(p[2])];
            }
        }
    }
    if !z_range[0].is_finite() || z_range[1] - z_range[0] < 1e-3 {
        z_range = [-5.0, 5.0];
    }
    let camera = Camera3d {
        rect,
        yaw: calc.yaw,
        pitch: calc.pitch,
        zoom: calc.zoom_3d,
        centre: [
            (domain.x[0] + domain.x[1]) * 0.5,
            (domain.y[0] + domain.y[1]) * 0.5,
            (z_range[0] + z_range[1]) * 0.5,
        ],
        half: [
            ((domain.x[1] - domain.x[0]) * 0.5).max(1e-3),
            ((domain.y[1] - domain.y[0]) * 0.5).max(1e-3),
            ((z_range[1] - z_range[0]) * 0.5).max(1e-3),
        ],
    };
    let line3 = |a: [f32; 3], b: [f32; 3], stroke: egui::Stroke| {
        if let (Some((pa, _)), Some((pb, _))) = (camera.project(a), camera.project(b)) {
            painter.line_segment([pa, pb], stroke);
        }
    };

    // Bounding box, floor grid and labelled axes.
    let [x0, x1] = domain.x;
    let [y0, y1] = domain.y;
    let [z0, z1] = z_range;
    let box_stroke = egui::Stroke::new(0.8, with_alpha(palette.stroke, 140));
    for (a, b) in [
        ([x0, y0, z0], [x1, y0, z0]),
        ([x1, y0, z0], [x1, y1, z0]),
        ([x1, y1, z0], [x0, y1, z0]),
        ([x0, y1, z0], [x0, y0, z0]),
        ([x0, y0, z1], [x1, y0, z1]),
        ([x1, y0, z1], [x1, y1, z1]),
        ([x1, y1, z1], [x0, y1, z1]),
        ([x0, y1, z1], [x0, y0, z1]),
        ([x0, y0, z0], [x0, y0, z1]),
        ([x1, y0, z0], [x1, y0, z1]),
        ([x1, y1, z0], [x1, y1, z1]),
        ([x0, y1, z0], [x0, y1, z1]),
    ] {
        line3(a, b, box_stroke);
    }
    let floor = egui::Stroke::new(0.5, with_alpha(palette.stroke, 70));
    for i in 0..=10 {
        let f = i as f32 / 10.0;
        line3(
            [x0 + (x1 - x0) * f, y0, z0],
            [x0 + (x1 - x0) * f, y1, z0],
            floor,
        );
        line3(
            [x0, y0 + (y1 - y0) * f, z0],
            [x1, y0 + (y1 - y0) * f, z0],
            floor,
        );
    }
    let cx = 0.0_f32.clamp(x0, x1);
    let cy = 0.0_f32.clamp(y0, y1);
    let cz = 0.0_f32.clamp(z0, z1);
    for (a, b, label) in [
        ([x0, cy, cz], [x1, cy, cz], "x"),
        ([cx, y0, cz], [cx, y1, cz], "y"),
        ([cx, cy, z0], [cx, cy, z1], "z"),
    ] {
        line3(a, b, egui::Stroke::new(1.4, palette.muted_text));
        if let Some((pos, _)) = camera.project(b) {
            painter.text(
                pos + egui::vec2(4.0, -4.0),
                egui::Align2::LEFT_BOTTOM,
                label,
                egui::FontId::monospace(12.0),
                palette.text,
            );
        }
    }

    // 2D curves lie in the z = 0 plane, as in GeoGebra's 3D view.
    for item in &calc.items {
        if let Some((_, primitives)) = &item.cache
            && item.visible
        {
            let color = with_alpha(series(palette, item.color), 160);
            for line in &primitives.polylines {
                for pair in line.windows(2) {
                    line3(
                        [pair[0][0], pair[0][1], cz],
                        [pair[1][0], pair[1][1], cz],
                        egui::Stroke::new(1.2, color),
                    );
                }
            }
        }
    }

    for (index, kind, s) in &sampled {
        let color = series(palette, calc.items[*index].color);
        match kind {
            PlotKind::Surface(_) => {
                let n = s.grid_steps + 1;
                let shade = |z: f32| {
                    // Lower parts darker, higher parts brighter.
                    let t = ((z - z0) / (z1 - z0).max(1e-6)).clamp(0.0, 1.0);
                    let mix =
                        |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * t) as u8;
                    let low = palette.extreme;
                    egui::Color32::from_rgb(
                        mix(low.r() / 2 + color.r() / 2, color.r()),
                        mix(low.g() / 2 + color.g() / 2, color.g()),
                        mix(low.b() / 2 + color.b() / 2, color.b()),
                    )
                };
                for row in 0..n {
                    for column in 0..n {
                        let p = s.grid[row * n + column];
                        if column + 1 < n {
                            let q = s.grid[row * n + column + 1];
                            line3(p, q, egui::Stroke::new(1.0, shade((p[2] + q[2]) * 0.5)));
                        }
                        if row + 1 < n {
                            let q = s.grid[(row + 1) * n + column];
                            line3(p, q, egui::Stroke::new(1.0, shade((p[2] + q[2]) * 0.5)));
                        }
                    }
                }
            }
            PlotKind::Parametric3(..) => {
                let points: Vec<egui::Pos2> = s
                    .curve
                    .iter()
                    .filter_map(|p| camera.project(*p).map(|(pos, _)| pos))
                    .collect();
                painter.add(egui::Shape::line(points, egui::Stroke::new(2.2, color)));
            }
            PlotKind::Point3(..) => {
                if let Some(p) = s.curve.first()
                    && let Some((pos, _)) = camera.project(*p)
                {
                    painter.circle(pos, 5.0, color, egui::Stroke::new(1.5, palette.extreme));
                    painter.text(
                        pos + egui::vec2(7.0, -7.0),
                        egui::Align2::LEFT_BOTTOM,
                        format!("({:.2}, {:.2}, {:.2})", p[0], p[1], p[2]),
                        egui::FontId::monospace(10.0),
                        palette.text,
                    );
                }
            }
            _ => {}
        }
    }

    if response.dragged() {
        let delta = response.drag_delta();
        calc.yaw += delta.x * 0.01;
        calc.pitch = (calc.pitch + delta.y * 0.01).clamp(-1.5, 1.5);
    }
    if response.hovered() {
        let scroll = ui.input(|input| input.smooth_scroll_delta.y);
        if scroll != 0.0 {
            calc.zoom_3d = (calc.zoom_3d * (scroll * 0.002).exp()).clamp(0.2, 8.0);
        }
    }
    if response.double_clicked() {
        calc.yaw = 0.7;
        calc.pitch = 0.45;
        calc.zoom_3d = 1.0;
    }
    ui.label(
        egui::RichText::new(format!(
            "Drag to orbit · scroll to zoom · x,y domain from the 2D view · z {:.2} … {:.2}",
            z0, z1
        ))
        .small()
        .color(palette.muted_text),
    );
}

/// Calculus summary and table of values for the active function.
fn draw_analysis(
    ui: &mut egui::Ui,
    calc: &mut GraphingCalculator,
    scope: &Scope,
    palette: &ThemePalette,
) {
    let Some(item) = calc.items.get(calc.active) else {
        return;
    };
    let Ok(kind @ PlotKind::Function(_)) = classify(&item.source) else {
        return;
    };
    let Ok(plot) = Plot::new(kind, scope) else {
        return;
    };
    let f = |x| plot.evaluate_function(scope, x);
    ui.horizontal_wrapped(|ui| {
        let color = series(palette, item.color);
        ui.label(egui::RichText::new("ANALYSIS").strong().color(color));
        let roots = graphing::roots(f, calc.view.x, 400);
        ui.label(egui::RichText::new(format!("{} root(s)", roots.len())).color(palette.text));
        let extrema = graphing::extrema(f, calc.view.x, 400);
        ui.label(egui::RichText::new(format!("{} extremum/a", extrema.len())).color(palette.text));
        if let Some([a, b]) = item.area {
            let value = graphing::integral(f, [a, b], 512);
            ui.label(
                egui::RichText::new(format!("∫[{a:.2}, {b:.2}] f dx = {value:.5}"))
                    .monospace()
                    .color(palette.text),
            );
        }
        ui.toggle_value(&mut calc.show_table, "Table of values");
    });
    if calc.show_table {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("step").color(palette.muted_text));
            ui.add(
                egui::DragValue::new(&mut calc.table_step)
                    .range(0.001..=1000.0)
                    .speed(0.05),
            );
        });
        egui::Grid::new("calculator-table")
            .striped(true)
            .min_col_width(80.0)
            .show(ui, |ui| {
                for heading in ["x", "f(x)", "f′(x)"] {
                    ui.label(
                        egui::RichText::new(heading)
                            .strong()
                            .color(palette.strong_text),
                    );
                }
                ui.end_row();
                let start = (calc.view.x[0] / calc.table_step).ceil() * calc.table_step;
                let mut x = start;
                let mut rows = 0;
                while x <= calc.view.x[1] && rows < 41 {
                    for value in [x, f(x), graphing::derivative(f, x)] {
                        ui.label(
                            egui::RichText::new(if value.is_finite() {
                                format!("{value:.4}")
                            } else {
                                "undefined".into()
                            })
                            .monospace()
                            .color(palette.text),
                        );
                    }
                    ui.end_row();
                    x += calc.table_step;
                    rows += 1;
                }
            });
    }
}

/// Writes the active object into the focused geometry node, substituting
/// slider values and mapping its range onto the node's 0–1 parameters.
fn apply_to_graph(
    ui: &mut egui::Ui,
    calc: &GraphingCalculator,
    graph: Option<&mut GeometryGraph>,
    selected_node: u64,
) -> bool {
    let Some(graph) = graph else {
        ui.add_enabled(false, egui::Button::new("Apply to geometry node"))
            .on_disabled_hover_text("Create a custom geometry graph in Geometry Lab first");
        return false;
    };
    let Some(item) = calc.items.get(calc.active) else {
        return false;
    };
    let Ok(kind) = classify(&item.source) else {
        return false;
    };
    let scope = calc.scope();
    let substitute = |source: &str| {
        let mut out = source.to_owned();
        for (name, value) in scope.names.iter().zip(&scope.values) {
            out = replace_variable(&out, name, &format!("({value})"));
        }
        out
    };
    let [x0, x1] = calc.view.x;
    let [y0, y1] = calc.view.y;
    let [p0, p1] = item.parameter;
    let along =
        |start: f32, end: f32, var: &str| format!("({start:.6}+({:.6})*{var})", end - start);
    // (curve x, y, z) for curve nodes, or (x, y, z) in u, v for surfaces.
    let target = match &kind {
        PlotKind::Function(f) => Some((
            false,
            along(x0, x1, "t"),
            replace_variable(&substitute(f), "x", &along(x0, x1, "t")),
            "0".to_owned(),
        )),
        PlotKind::Parametric(a, b) => {
            let t = along(p0, p1, "t");
            Some((
                false,
                replace_variable(&substitute(a), "t", &t),
                replace_variable(&substitute(b), "t", &t),
                "0".to_owned(),
            ))
        }
        PlotKind::Polar(r) => {
            let theta = along(p0, p1, "t");
            let r = replace_variable(
                &replace_variable(&substitute(r), "theta", &theta),
                "t",
                &theta,
            );
            Some((
                false,
                format!("({r})*cos({theta})"),
                format!("({r})*sin({theta})"),
                "0".to_owned(),
            ))
        }
        PlotKind::Parametric3(a, b, c) => {
            let t = along(p0, p1, "t");
            Some((
                false,
                replace_variable(&substitute(a), "t", &t),
                replace_variable(&substitute(b), "t", &t),
                replace_variable(&substitute(c), "t", &t),
            ))
        }
        PlotKind::Surface(z) => {
            let (x, y) = (along(x0, x1, "u"), along(y0, y1, "v"));
            Some((
                true,
                x.clone(),
                y.clone(),
                replace_variable(&replace_variable(&substitute(z), "x", &x), "y", &y),
            ))
        }
        _ => None,
    };
    let Some((surface, nx, ny, nz)) = target else {
        ui.add_enabled(false, egui::Button::new("Apply to geometry node"))
            .on_disabled_hover_text(
                "Functions, parametric, polar and 3D curves, and surfaces can be applied",
            );
        return false;
    };
    let label = if surface {
        "Apply to surface node"
    } else {
        "Apply to curve node"
    };
    if !ui
        .button(label)
        .on_hover_text("Maps the visible range (or the t/θ range) onto the node's 0–1 parameters")
        .clicked()
    {
        return false;
    }
    let matches_kind = |kind: &GeometryNodeKind| {
        if surface {
            matches!(kind, GeometryNodeKind::Surface { .. })
        } else {
            matches!(kind, GeometryNodeKind::Curve { .. })
        }
    };
    let index = graph
        .nodes
        .iter()
        .position(|node| node.id.0 == selected_node && matches_kind(&node.kind))
        .or_else(|| graph.nodes.iter().position(|node| matches_kind(&node.kind)));
    match index
        .and_then(|index| graph.nodes.get_mut(index))
        .map(|node| &mut node.kind)
    {
        Some(GeometryNodeKind::Curve { x, y, z, .. })
        | Some(GeometryNodeKind::Surface { x, y, z, .. }) => {
            (*x, *y, *z) = (nx, ny, nz);
            true
        }
        _ => false,
    }
}

/// Replaces whole-word occurrences of `variable` in `expression`.
pub(super) fn replace_variable(expression: &str, variable: &str, replacement: &str) -> String {
    let bytes = expression.as_bytes();
    let word = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    let mut result = String::with_capacity(expression.len());
    let (mut start, mut index) = (0, 0);
    while index < bytes.len() {
        if expression[index..].starts_with(variable)
            && (index == 0 || !word(bytes[index - 1]))
            && bytes.get(index + variable.len()).is_none_or(|&b| !word(b))
        {
            result.push_str(&expression[start..index]);
            result.push_str(replacement);
            index += variable.len();
            start = index;
        } else {
            index += 1;
        }
    }
    result.push_str(&expression[start..]);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_whole_words_only() {
        assert_eq!(
            replace_variable("x + max(x, xx) + x2", "x", "(t)"),
            "(t) + max((t), xx) + x2"
        );
        assert_eq!(replace_variable("theta*t", "t", "u"), "theta*u");
    }

    #[test]
    fn sliders_feed_the_scope_once() {
        let mut calc = GraphingCalculator::default();
        calc.add("a = 2".into());
        calc.add("a = 9".into());
        calc.add("y = a x".into());
        let scope = calc.scope();
        assert_eq!(scope.names, ["a"]);
        assert_eq!(scope.values, [2.0]);
    }

    #[test]
    fn keypad_inserts_at_the_cursor_and_backspaces() {
        let ctx = egui::Context::default();
        let id = egui::Id::new("edit");
        let mut text = String::from("sin()");
        let mut state = egui::text_edit::TextEditState::default();
        state
            .cursor
            .set_char_range(Some(egui::text::CCursorRange::one(
                egui::text::CCursor::new(4),
            )));
        state.store(&ctx, id);
        insert(&ctx, id, &mut text, "2x", 0);
        assert_eq!(text, "sin(2x)");
        insert(&ctx, id, &mut text, "\u{8}", 0);
        assert_eq!(text, "sin(2)");
        insert(&ctx, id, &mut text, "cos()", 1);
        assert_eq!(text, "sin(2cos())");
        insert(&ctx, id, &mut text, "θ", 0);
        assert_eq!(text, "sin(2cos(θ))");
    }
}
