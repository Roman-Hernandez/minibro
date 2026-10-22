//! Platform-independent pointer gestures and logical screen placement.

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GestureInput {
    Pressed { x: f32, y: f32 },
    Moved { x: f32, y: f32 },
    Released,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GestureAction {
    None,
    Click,
    Drag { dx: f32, dy: f32, started: bool },
    Dropped,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Gesture {
    origin: Option<(f32, f32)>,
    dragging: bool,
}

impl Gesture {
    pub fn update(&mut self, input: GestureInput) -> GestureAction {
        match input {
            GestureInput::Pressed { x, y } if x.is_finite() && y.is_finite() => {
                self.origin = Some((x, y));
                self.dragging = false;
                GestureAction::None
            }
            GestureInput::Moved { x, y } if x.is_finite() && y.is_finite() => {
                let Some((start_x, start_y)) = self.origin else {
                    return GestureAction::None;
                };
                let (dx, dy) = (x - start_x, y - start_y);
                let started = !self.dragging && dx * dx + dy * dy >= 36.0;
                self.dragging |= started;
                if self.dragging {
                    GestureAction::Drag { dx, dy, started }
                } else {
                    GestureAction::None
                }
            }
            GestureInput::Released => {
                let action = match (self.origin.is_some(), self.dragging) {
                    (true, true) => GestureAction::Dropped,
                    (true, false) => GestureAction::Click,
                    _ => GestureAction::None,
                };
                *self = Self::default();
                action
            }
            _ => {
                *self = Self::default();
                GestureAction::None
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Screen {
    pub id: String,
    pub name: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl std::fmt::Display for Screen {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.name.fmt(f)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Placement {
    pub screen: String,
    pub horizontal: f32,
}

impl Default for Placement {
    fn default() -> Self {
        Self {
            screen: String::new(),
            horizontal: 0.5,
        }
    }
}

impl Placement {
    pub fn left(&self, screen: &Screen, width: f32) -> f32 {
        (self.horizontal * screen.width - width / 2.0).clamp(0.0, (screen.width - width).max(0.0))
    }

    pub fn move_by(&mut self, screen: &Screen, width: f32, dx: f32) {
        if !dx.is_finite() || screen.width <= 0.0 {
            return;
        }
        let left = (self.left(screen, width) + dx).clamp(0.0, (screen.width - width).max(0.0));
        self.horizontal = ((left + width / 2.0) / screen.width).clamp(0.0, 1.0);
        self.screen.clone_from(&screen.id);
    }

    pub fn resolve<'a>(&self, screens: &'a [Screen]) -> Option<&'a Screen> {
        screens
            .iter()
            .find(|screen| screen.id == self.screen)
            .or_else(|| screens.first())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen(id: &str) -> Screen {
        Screen {
            id: id.into(),
            name: id.into(),
            x: -1920.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        }
    }

    #[test]
    fn click_waits_for_release_and_tolerates_jitter() {
        let mut gesture = Gesture::default();
        assert_eq!(
            gesture.update(GestureInput::Pressed { x: 20.0, y: 10.0 }),
            GestureAction::None
        );
        assert_eq!(
            gesture.update(GestureInput::Moved { x: 23.0, y: 14.0 }),
            GestureAction::None
        );
        assert_eq!(gesture.update(GestureInput::Released), GestureAction::Click);
        assert_eq!(gesture.update(GestureInput::Released), GestureAction::None);
    }

    #[test]
    fn crossing_threshold_never_clicks_even_after_returning() {
        let mut gesture = Gesture::default();
        gesture.update(GestureInput::Pressed { x: 20.0, y: 10.0 });
        assert_eq!(
            gesture.update(GestureInput::Moved { x: 26.0, y: 10.0 }),
            GestureAction::Drag {
                dx: 6.0,
                dy: 0.0,
                started: true
            }
        );
        assert_eq!(
            gesture.update(GestureInput::Moved { x: 20.0, y: 10.0 }),
            GestureAction::Drag {
                dx: 0.0,
                dy: 0.0,
                started: false
            }
        );
        assert_eq!(
            gesture.update(GestureInput::Released),
            GestureAction::Dropped
        );
    }

    #[test]
    fn cancellation_and_invalid_coordinates_do_not_click() {
        for input in [
            GestureInput::Cancelled,
            GestureInput::Moved {
                x: f32::NAN,
                y: 1.0,
            },
        ] {
            let mut gesture = Gesture::default();
            gesture.update(GestureInput::Pressed { x: 1.0, y: 1.0 });
            gesture.update(input);
            assert_eq!(gesture.update(GestureInput::Released), GestureAction::None);
        }
    }

    #[test]
    fn resizing_keeps_chosen_center_and_clamps_edges() {
        let monitor = screen("one");
        let mut placement = Placement {
            screen: "one".into(),
            horizontal: 0.25,
        };
        assert_eq!(placement.left(&monitor, 168.0), 396.0);
        assert_eq!(placement.left(&monitor, 380.0), 290.0);
        placement.horizontal = 1.0;
        assert_eq!(placement.left(&monitor, 380.0), 1540.0);
        assert_eq!(placement.left(&monitor, 2000.0), 0.0);
    }

    #[test]
    fn movement_is_clamped_and_ignores_non_finite_deltas() {
        let monitor = screen("one");
        let mut placement = Placement::default();
        placement.move_by(&monitor, 168.0, 10000.0);
        assert_eq!(placement.left(&monitor, 168.0), 1752.0);
        placement.move_by(&monitor, 168.0, f32::INFINITY);
        assert_eq!(placement.left(&monitor, 168.0), 1752.0);
        placement.move_by(&monitor, 168.0, -10000.0);
        assert_eq!(placement.left(&monitor, 168.0), 0.0);
    }

    #[test]
    fn missing_screen_recovers_without_overwriting_preference() {
        let placement = Placement {
            screen: "missing".into(),
            horizontal: 0.3,
        };
        let available = [screen("connected")];
        assert_eq!(
            placement.resolve(&available).map(|s| s.id.as_str()),
            Some("connected")
        );
        assert_eq!(placement.screen, "missing");
        assert!(placement.resolve(&[]).is_none());
    }
}
