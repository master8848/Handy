// PromptClear — theme assembly on top of egui-shadcn. MIT.

use egui_shadcn::theme::shadcn_theme::ShadcnTheme;
use egui_shadcn::theme::shadcn_theme_dark::dark;
use egui_shadcn::theme::shadcn_theme_light::light;
use egui_shadcn::ShadcnThemeExt;
use handy_core::settings::{Accent, CornerRadius, ThemeMode};

/// One accent's per-mode color tokens.
struct AccentColors {
    primary: egui::Color32,
    primary_foreground: egui::Color32,
    ring: egui::Color32,
    accent: egui::Color32,
    accent_foreground: egui::Color32,
}

/// `radius` mapping: the shadcn base themes ship with radius 10.0, so
/// `Default` preserves the library look while the other presets step the
/// scale up or down.
pub fn corner_radius_value(radius: CornerRadius) -> f32 {
    match radius {
        CornerRadius::Sharp => 0.0,
        CornerRadius::Default => 10.0,
        CornerRadius::Rounded => 14.0,
        CornerRadius::VeryRounded => 20.0,
    }
}

fn mix(a: egui::Color32, b: egui::Color32, t: f32) -> egui::Color32 {
    let t = t.clamp(0.0, 1.0);
    egui::Color32::from_rgb(
        (a.r() as f32 * (1.0 - t) + b.r() as f32 * t).round() as u8,
        (a.g() as f32 * (1.0 - t) + b.g() as f32 * t).round() as u8,
        (a.b() as f32 * (1.0 - t) + b.b() as f32 * t).round() as u8,
    )
}

fn accent_colors(accent: Accent, mode: ThemeMode) -> Option<AccentColors> {
    // NeutralGray is the shadcn "gray mode": keep the black/white base
    // untouched so primary/ring/focus stay monochrome.
    let primary = match accent {
        Accent::NeutralGray => return None,
        Accent::HandyPink => egui::Color32::from_rgb(218, 88, 147),
        Accent::Blue => egui::Color32::from_rgb(59, 130, 246),
        Accent::Sky => egui::Color32::from_rgb(14, 165, 233),
        Accent::Cyan => egui::Color32::from_rgb(6, 182, 212),
        Accent::Teal => egui::Color32::from_rgb(20, 184, 166),
        Accent::Green => egui::Color32::from_rgb(34, 197, 94),
        Accent::Lime => egui::Color32::from_rgb(132, 204, 22),
        Accent::Amber => egui::Color32::from_rgb(245, 158, 11),
        Accent::Orange => egui::Color32::from_rgb(249, 115, 22),
        Accent::Red => egui::Color32::from_rgb(239, 68, 68),
        Accent::Violet => egui::Color32::from_rgb(139, 92, 246),
        Accent::Purple => egui::Color32::from_rgb(168, 85, 247),
    };

    // Bright accents (lime/amber) need dark text on top of the accent.
    let luminance =
        0.2126 * primary.r() as f32 + 0.7152 * primary.g() as f32 + 0.0722 * primary.b() as f32;
    let primary_foreground = if luminance > 170.0 {
        egui::Color32::from_rgb(23, 23, 23)
    } else {
        egui::Color32::from_rgb(250, 250, 250)
    };

    let background = match mode {
        ThemeMode::Dark => egui::Color32::from_rgb(10, 10, 10),
        ThemeMode::Light => egui::Color32::from_rgb(255, 255, 255),
    };
    let foreground = match mode {
        ThemeMode::Dark => egui::Color32::from_rgb(250, 250, 250),
        ThemeMode::Light => egui::Color32::from_rgb(10, 10, 10),
    };

    let mix_toward = match mode {
        ThemeMode::Dark => 0.22,
        ThemeMode::Light => 0.14,
    };
    let accent = mix(background, primary, mix_toward);
    let accent_foreground = mix(foreground, primary, 0.55);

    Some(AccentColors {
        primary,
        primary_foreground,
        ring: primary,
        accent,
        accent_foreground,
    })
}

/// Constructs the shadcn theme for a mode/accent/radius combination.
pub fn build_theme(mode: ThemeMode, accent: Accent, radius: CornerRadius) -> ShadcnTheme {
    let mut theme = match mode {
        ThemeMode::Dark => dark(),
        ThemeMode::Light => light(),
    };
    theme.radius = corner_radius_value(radius);
    if let Some(colors) = accent_colors(accent, mode) {
        theme.primary = colors.primary;
        theme.primary_foreground = colors.primary_foreground;
        theme.ring = colors.ring;
        theme.accent = colors.accent;
        theme.accent_foreground = colors.accent_foreground;
    }
    theme
}

/// The swatch color for an accent in the appearance picker. Gray mode shows a
/// neutral monochrome circle, everything else the accent's primary color.
pub fn accent_swatch(accent: Accent, mode: ThemeMode) -> egui::Color32 {
    if matches!(accent, Accent::NeutralGray) {
        return egui::Color32::from_rgb(161, 161, 161);
    }
    accent_colors(accent, mode)
        .map(|colors| colors.primary)
        .unwrap_or_default()
}

/// Applies mode/accent/radius to the whole context (shadcn theme + egui
/// visuals). Safe to call from any UI handler; cheap enough to run on every
/// appearance-settings change.
pub fn apply(ctx: &egui::Context, mode: ThemeMode, accent: Accent, radius: CornerRadius) {
    let theme = build_theme(mode, accent, radius);

    ctx.set_theme(match mode {
        ThemeMode::Dark => egui::Theme::Dark,
        ThemeMode::Light => egui::Theme::Light,
    });
    ctx.set_shadcn_theme(theme.clone());

    ctx.all_styles_mut(|style| {
        let visuals = &mut style.visuals;
        visuals.panel_fill = theme.background;
        visuals.window_fill = theme.background;
        visuals.selection.bg_fill = theme.ring;
        visuals.selection.stroke = egui::Stroke::new(1.0, theme.ring);
        visuals.hyperlink_color = theme.primary;
        // Native widgets still in use (TextEdit under spellcheck, scroll
        // bars, menus) get shadcn-matching colors.
        visuals.widgets.inactive.weak_bg_fill = theme.card;
        visuals.widgets.inactive.bg_fill = theme.input;
        visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, theme.foreground);
        visuals.widgets.hovered.bg_fill = theme.muted;
        visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, theme.foreground);
        visuals.widgets.active.bg_fill = theme.secondary;
        visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0, theme.foreground);
    });
}
