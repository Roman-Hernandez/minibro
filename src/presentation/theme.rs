use iced::Color;

pub fn app_theme() -> iced::Theme {
    static THEME: std::sync::LazyLock<iced::Theme> = std::sync::LazyLock::new(|| {
        iced::Theme::custom(
            "minibro",
            iced::theme::Palette {
                background: COLOR_NOTCH_BG,
                text: COLOR_TEXT_PRIMARY,
                primary: Color::from_rgb8(180, 203, 246),
                success: COLOR_ACCENT_SUCCESS,
                danger: COLOR_ACCENT_ERROR,
                warning: COLOR_ACCENT_WAITING,
            },
        )
    });
    THEME.clone()
}

pub const NOTCH_COLLAPSED_WIDTH: f32 = 168.0;
pub const NOTCH_COLLAPSED_HEIGHT: f32 = 32.0;

pub const NOTCH_EXPANDED_WIDTH: f32 = 380.0;
pub const NOTCH_EXPANDED_HEIGHT: f32 = 280.0;

pub const NOTCH_CORNER_RADIUS: f32 = 18.0;

pub const COLOR_NOTCH_BG: Color = Color {
    r: 0.07,
    g: 0.07,
    b: 0.09,
    a: 0.95,
};

pub const COLOR_NOTCH_BORDER: Color = Color {
    r: 1.0,
    g: 1.0,
    b: 1.0,
    a: 0.10,
};

pub const COLOR_FACE: Color = Color {
    r: 0.95,
    g: 0.95,
    b: 0.98,
    a: 0.95,
};

pub const COLOR_TEXT_PRIMARY: Color = Color {
    r: 0.92,
    g: 0.92,
    b: 0.95,
    a: 1.0,
};

pub const COLOR_TEXT_MUTED: Color = Color {
    r: 0.60,
    g: 0.60,
    b: 0.65,
    a: 1.0,
};

pub const COLOR_ACCENT_IDLE: Color = Color {
    r: 0.35,
    g: 0.65,
    b: 0.95,
    a: 1.0,
};

pub const COLOR_ACCENT_THINKING: Color = Color {
    r: 0.65,
    g: 0.40,
    b: 0.95,
    a: 1.0,
};

pub const COLOR_ACCENT_WORKING: Color = Color {
    r: 0.95,
    g: 0.65,
    b: 0.20,
    a: 1.0,
};

pub const COLOR_ACCENT_SUCCESS: Color = Color {
    r: 0.25,
    g: 0.85,
    b: 0.45,
    a: 1.0,
};

pub const COLOR_ACCENT_WAITING: Color = Color {
    r: 0.40,
    g: 0.80,
    b: 0.85,
    a: 1.0,
};

pub const COLOR_ACCENT_ERROR: Color = Color {
    r: 0.95,
    g: 0.30,
    b: 0.35,
    a: 1.0,
};
