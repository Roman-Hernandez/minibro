use crate::domain::{Character, MockAgentTask, Mood, PanelState};

#[derive(Debug, Clone, PartialEq)]
pub struct NotchController {
    pub character: Character,
    pub panel: PanelState,
}

impl Default for NotchController {
    fn default() -> Self {
        Self {
            character: Character::new(),
            panel: PanelState::new(),
        }
    }
}

impl NotchController {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn toggle_panel(&mut self) {
        self.panel.toggle();
        self.character.trigger_blink();
    }

    pub fn set_mood(&mut self, mood: Mood) {
        self.character.set_mood(mood);
    }

    pub fn cycle_mood(&mut self) {
        let all = Mood::ALL;
        let current_idx = all
            .iter()
            .position(|m| *m == self.character.mood)
            .unwrap_or(0);
        let next_idx = (current_idx + 1) % all.len();
        self.set_mood(all[next_idx]);
    }

    pub fn tick(&mut self, dt: f32) -> bool {
        self.character.tick(dt)
    }

    pub fn select_task(&mut self, index: usize) {
        self.panel.select_task(index);
    }

    #[must_use]
    pub fn selected_task(&self) -> Option<&MockAgentTask> {
        self.panel
            .selected_task
            .and_then(|idx| self.panel.tasks.get(idx))
    }

    #[must_use]
    pub fn is_animating(&self) -> bool {
        self.character.is_actively_animating()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_toggle_triggers_blink() {
        let mut ctrl = NotchController::new();
        assert!(!ctrl.panel.is_expanded);
        ctrl.toggle_panel();
        assert!(ctrl.panel.is_expanded);
        assert!(ctrl.character.is_actively_animating());
    }

    #[test]
    fn test_cycle_mood() {
        let mut ctrl = NotchController::new();
        assert_eq!(ctrl.character.mood, Mood::Idle);
        ctrl.cycle_mood();
        assert_eq!(ctrl.character.mood, Mood::Thinking);
        ctrl.cycle_mood();
        assert_eq!(ctrl.character.mood, Mood::Working);
    }
}
