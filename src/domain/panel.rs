#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MockAgentTask {
    pub id: &'static str,
    pub agent: &'static str,
    pub summary: &'static str,
    pub status: &'static str,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PanelState {
    pub is_expanded: bool,
    pub tasks: Vec<MockAgentTask>,
    pub selected_task: Option<usize>,
}

impl Default for PanelState {
    fn default() -> Self {
        Self {
            is_expanded: false,
            tasks: vec![
                MockAgentTask {
                    id: "TSK-101",
                    agent: "Codex",
                    summary: "Refactor database migrations to async sqlx",
                    status: "Running",
                },
                MockAgentTask {
                    id: "TSK-102",
                    agent: "OpenCode",
                    summary: "Generate OpenAPI client bindings for Rust",
                    status: "Awaiting Approval",
                },
                MockAgentTask {
                    id: "TSK-103",
                    agent: "Codex",
                    summary: "Verify unit test coverage on domain logic",
                    status: "Done",
                },
            ],
            selected_task: Some(0),
        }
    }
}

impl PanelState {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn toggle(&mut self) {
        self.is_expanded = !self.is_expanded;
    }

    pub fn expand(&mut self) {
        self.is_expanded = true;
    }

    pub fn collapse(&mut self) {
        self.is_expanded = false;
    }

    pub fn select_task(&mut self, index: usize) {
        if index < self.tasks.len() {
            self.selected_task = Some(index);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_panel_toggle() {
        let mut p = PanelState::new();
        assert!(!p.is_expanded);
        p.toggle();
        assert!(p.is_expanded);
        p.toggle();
        assert!(!p.is_expanded);
    }

    #[test]
    fn test_select_task() {
        let mut p = PanelState::new();
        p.select_task(1);
        assert_eq!(p.selected_task, Some(1));
        // Out of bounds is ignored
        p.select_task(99);
        assert_eq!(p.selected_task, Some(1));
    }
}
