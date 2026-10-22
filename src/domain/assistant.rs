//! Conversation state independent of the execution backend and UI.

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChatMessage {
    pub id: String,
    pub role: String,
    pub text: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ToolActivity {
    pub id: String,
    pub session: String,
    pub title: String,
    pub input: String,
    pub output: String,
    pub status: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Permission {
    pub id: String,
    pub session: String,
    pub operation: String,
    pub patterns: Vec<String>,
    pub details: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Question {
    pub id: String,
    pub session: String,
    pub items: Vec<QuestionItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestionItem {
    pub text: String,
    pub options: Vec<String>,
    pub multiple: bool,
    pub custom: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Snapshot {
    pub session: String,
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub activity: Vec<ToolActivity>,
    pub permissions: Vec<Permission>,
    pub questions: Vec<Question>,
    pub busy: bool,
    pub turn_id: String,
    pub completed: bool,
    pub error: Option<String>,
}
