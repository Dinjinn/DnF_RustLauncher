use eframe::egui;

pub struct Theme;

impl Theme {
    pub const BG: egui::Color32 = egui::Color32::from_rgb(12, 12, 14);
    pub const BG_ALT: egui::Color32 = egui::Color32::from_rgb(18, 18, 22);
    pub const SURFACE: egui::Color32 = egui::Color32::from_rgb(26, 26, 32);
    pub const SURFACE_ALT: egui::Color32 = egui::Color32::from_rgb(34, 34, 42);
    pub const ACCENT: egui::Color32 = egui::Color32::from_rgb(208, 30, 30);
    pub const ACCENT_SOFT: egui::Color32 = egui::Color32::from_rgb(130, 25, 25);
    pub const SUCCESS: egui::Color32 = egui::Color32::from_rgb(40, 167, 69);
    pub const ERROR: egui::Color32 = egui::Color32::from_rgb(220, 53, 69);
    pub const TEXT: egui::Color32 = egui::Color32::from_rgb(240, 240, 240);
    pub const TEXT_MUTED: egui::Color32 = egui::Color32::from_rgb(150, 150, 160);

    pub fn apply(ctx: &egui::Context) {
        let mut visuals = egui::Visuals::dark();
        visuals.override_text_color = Some(Self::TEXT);
        visuals.panel_fill = Self::BG;
        visuals.window_fill = Self::BG;
        visuals.widgets.noninteractive.bg_fill = Self::BG;
        visuals.widgets.inactive.bg_fill = Self::SURFACE;
        visuals.widgets.hovered.bg_fill = Self::SURFACE_ALT;
        visuals.widgets.active.bg_fill = Self::ACCENT;
        visuals.selection.bg_fill = Self::ACCENT;
        visuals.selection.stroke.color = Self::ACCENT;
        visuals.extreme_bg_color = Self::BG;
        visuals.faint_bg_color = Self::BG_ALT;
        ctx.set_visuals(visuals);
    }
}
