use ratatui::style::Color;

#[derive(Debug, Clone, Copy)]
pub struct ThemePalette {
    pub name: &'static str,
    pub accent_primary: Color,
    pub accent_success: Color,
    pub accent_danger: Color,
    pub accent_subtle: Color,
    pub bg_app: Color,
    pub bg_panel: Color,
    pub bg_panel_alt: Color,
    pub text_primary: Color,
    pub text_muted: Color,
    pub border_panel: Color,
    pub border_focus: Color,
    pub border_popup: Color,
    pub bg_popup: Color,
    pub bg_overlay: Color,
    pub bg_selected: Color,
    pub bg_visual_range: Color,
}

pub const THEMES: [ThemePalette; 3] = [
    ThemePalette {
        name: "github_dark",
        accent_primary: Color::Rgb(65, 105, 225),
        accent_success: Color::Rgb(74, 222, 128),
        accent_danger: Color::Rgb(234, 92, 124),
        accent_subtle: Color::Rgb(145, 171, 255),
        bg_app: Color::Rgb(0, 0, 0),
        bg_panel: Color::Rgb(0, 0, 0),
        bg_panel_alt: Color::Rgb(0, 0, 0),
        text_primary: Color::Rgb(226, 235, 255),
        text_muted: Color::Rgb(124, 138, 175),
        border_panel: Color::Rgb(35, 50, 88),
        border_focus: Color::Rgb(105, 138, 255),
        border_popup: Color::Rgb(128, 160, 255),
        bg_popup: Color::Rgb(0, 0, 0),
        bg_overlay: Color::Rgb(0, 0, 0),
        bg_selected: Color::Rgb(12, 24, 54),
        bg_visual_range: Color::Rgb(7, 15, 36),
    },
    ThemePalette {
        name: "midnight",
        accent_primary: Color::Rgb(91, 157, 255),
        accent_success: Color::Rgb(68, 201, 127),
        accent_danger: Color::Rgb(238, 111, 129),
        accent_subtle: Color::Rgb(166, 189, 255),
        bg_app: Color::Rgb(9, 13, 20),
        bg_panel: Color::Rgb(15, 20, 31),
        bg_panel_alt: Color::Rgb(20, 27, 40),
        text_primary: Color::Rgb(226, 234, 250),
        text_muted: Color::Rgb(141, 157, 190),
        border_panel: Color::Rgb(39, 57, 98),
        border_focus: Color::Rgb(115, 156, 255),
        border_popup: Color::Rgb(151, 186, 255),
        bg_popup: Color::Rgb(18, 24, 37),
        bg_overlay: Color::Rgb(6, 10, 15),
        bg_selected: Color::Rgb(28, 42, 71),
        bg_visual_range: Color::Rgb(19, 31, 55),
    },
    ThemePalette {
        name: "graphite",
        accent_primary: Color::Rgb(120, 170, 255),
        accent_success: Color::Rgb(103, 205, 147),
        accent_danger: Color::Rgb(241, 124, 149),
        accent_subtle: Color::Rgb(186, 204, 255),
        bg_app: Color::Rgb(16, 18, 22),
        bg_panel: Color::Rgb(24, 27, 34),
        bg_panel_alt: Color::Rgb(31, 35, 45),
        text_primary: Color::Rgb(231, 236, 245),
        text_muted: Color::Rgb(151, 160, 179),
        border_panel: Color::Rgb(54, 62, 81),
        border_focus: Color::Rgb(132, 177, 255),
        border_popup: Color::Rgb(165, 195, 255),
        bg_popup: Color::Rgb(29, 33, 42),
        bg_overlay: Color::Rgb(12, 14, 18),
        bg_selected: Color::Rgb(44, 51, 66),
        bg_visual_range: Color::Rgb(36, 42, 56),
    },
];

pub fn resolve_theme(name: Option<&str>) -> &'static ThemePalette {
    if let Some(name) = name
        && let Some(theme) = THEMES
            .iter()
            .find(|theme| theme.name.eq_ignore_ascii_case(name))
    {
        return theme;
    }
    default_theme()
}

pub fn default_theme() -> &'static ThemePalette {
    &THEMES[0]
}

#[cfg(test)]
mod tests {
    use super::{default_theme, resolve_theme};

    #[test]
    fn resolves_known_theme_case_insensitive() {
        let theme = resolve_theme(Some("MiDnIgHt"));
        assert_eq!(theme.name, "midnight");
    }

    #[test]
    fn falls_back_to_default_for_unknown_theme() {
        let theme = resolve_theme(Some("unknown"));
        assert_eq!(theme.name, default_theme().name);
    }
}
