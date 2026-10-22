#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mood {
    #[default]
    Idle,
    Thinking,
    Working,
    Success,
    Waiting,
    Error,
}

impl Mood {
    pub const ALL: [Mood; 6] = [
        Mood::Idle,
        Mood::Thinking,
        Mood::Working,
        Mood::Success,
        Mood::Waiting,
        Mood::Error,
    ];

    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Idle => "Idle",
            Self::Thinking => "Thinking",
            Self::Working => "Working",
            Self::Success => "Success",
            Self::Waiting => "Waiting",
            Self::Error => "Error",
        }
    }
}
