use super::positioning::{Gesture, GestureAction, GestureInput, Placement, Screen};
use crate::domain::{Character, Mood, PanelState};

#[derive(Debug, Clone, PartialEq)]
pub struct NotchController {
    pub character: Character,
    pub panel: PanelState,
    gesture: Gesture,
    pub placement: Placement,
    pub screens: Vec<Screen>,
    drag_origin: Option<Placement>,
}

impl Default for NotchController {
    fn default() -> Self {
        Self {
            character: Character::new(),
            panel: PanelState::new(),
            gesture: Gesture::default(),
            placement: Placement::default(),
            screens: Vec::new(),
            drag_origin: None,
        }
    }
}

impl NotchController {
    pub fn pointer(&mut self, input: GestureInput) -> GestureAction {
        if matches!(input, GestureInput::Pressed { .. }) {
            self.drag_origin = Some(self.placement.clone());
        }
        let action = self.gesture.update(input);
        if matches!(input, GestureInput::Released | GestureInput::Cancelled) {
            self.drag_origin = None;
        }
        action
    }

    pub fn drag_by(&mut self, screen: &Screen, width: f32, dx: f32) {
        if let Some(origin) = &self.drag_origin {
            self.placement = origin.clone();
            self.placement.move_by(screen, width, dx);
        }
    }
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

    #[must_use]
    pub fn is_animating(&self) -> bool {
        self.character.is_actively_animating()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drag_uses_initial_placement_without_accumulating_absolute_motion() {
        let mut controller = NotchController::new();
        let character = controller.character.clone();
        let panel = controller.panel.clone();
        let screen = Screen {
            id: "display".into(),
            name: "display".into(),
            x: 0.0,
            y: 0.0,
            width: 1600.0,
            height: 900.0,
        };
        controller.pointer(GestureInput::Pressed { x: 800.0, y: 40.0 });
        for (pointer_x, expected_left) in [(850.0, 766.0), (900.0, 816.0)] {
            let GestureAction::Drag { dx, .. } = controller.pointer(GestureInput::Moved {
                x: pointer_x,
                y: 40.0,
            }) else {
                panic!("expected drag");
            };
            controller.drag_by(&screen, 168.0, dx);
            assert_eq!(controller.placement.left(&screen, 168.0), expected_left);
        }
        assert_eq!(controller.character, character);
        assert_eq!(controller.panel, panel);
    }

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
