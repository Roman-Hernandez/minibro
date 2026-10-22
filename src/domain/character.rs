use crate::domain::mood::Mood;

const BLINK_DURATION_SEC: f32 = 0.18;
const BLINK_INTERVAL_SEC: f32 = 3.5;

#[derive(Debug, Clone, PartialEq)]
pub struct Character {
    pub mood: Mood,
    pub eye_openness: f32, // 1.0 = fully open, 0.0 = fully closed
    is_blinking: bool,
    blink_timer: f32,
    next_blink_in: f32,
    pub anim_tick: u64,
}

impl Default for Character {
    fn default() -> Self {
        Self {
            mood: Mood::Idle,
            eye_openness: 1.0,
            is_blinking: false,
            blink_timer: 0.0,
            next_blink_in: BLINK_INTERVAL_SEC,
            anim_tick: 0,
        }
    }
}

impl Character {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_mood(&mut self, mood: Mood) {
        if self.mood != mood {
            self.mood = mood;
            self.anim_tick = 0;
        }
    }

    pub fn trigger_blink(&mut self) {
        self.is_blinking = true;
        self.blink_timer = 0.0;
    }

    /// Advance character animation by `dt` seconds.
    /// Returns true if visual state changed and requires redraw.
    pub fn tick(&mut self, dt: f32) -> bool {
        self.anim_tick = self.anim_tick.wrapping_add(1);
        let mut changed = false;

        // Continuous moods always animate
        if matches!(self.mood, Mood::Thinking | Mood::Working) {
            changed = true;
        }

        if self.is_blinking {
            self.blink_timer += dt;
            let half = BLINK_DURATION_SEC / 2.0;

            if self.blink_timer >= BLINK_DURATION_SEC {
                self.is_blinking = false;
                self.blink_timer = 0.0;
                self.eye_openness = 1.0;
                self.next_blink_in = BLINK_INTERVAL_SEC;
            } else if self.blink_timer < half {
                // Closing eye: 1.0 -> 0.0
                self.eye_openness = (1.0 - (self.blink_timer / half)).clamp(0.0, 1.0);
            } else {
                // Opening eye: 0.0 -> 1.0
                self.eye_openness = ((self.blink_timer - half) / half).clamp(0.0, 1.0);
            }
            changed = true;
        } else {
            self.next_blink_in -= dt;
            if self.next_blink_in <= 0.0 {
                self.trigger_blink();
                changed = true;
            }
        }

        changed
    }

    /// Returns true if character requires continuous high-frequency updates (e.g. 60 FPS).
    #[must_use]
    pub fn is_actively_animating(&self) -> bool {
        self.is_blinking || matches!(self.mood, Mood::Thinking | Mood::Working)
    }

    pub fn next_update_in(&self) -> f32 {
        if self.is_actively_animating() {
            0.016
        } else {
            self.next_blink_in.max(0.001)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_waits_for_blink_deadline_then_animates() {
        let mut character = Character::new();
        character.tick(2.0);
        assert!((character.next_update_in() - 1.5).abs() < 0.001);
        character.tick(1.5);
        assert!(character.is_actively_animating());
        assert!(character.next_update_in() <= 0.016);
    }

    #[test]
    fn test_initial_state() {
        let c = Character::new();
        assert_eq!(c.mood, Mood::Idle);
        assert!((c.eye_openness - 1.0).abs() < f32::EPSILON);
        assert!(!c.is_actively_animating());
    }

    #[test]
    fn test_blink_lifecycle() {
        let mut c = Character::new();
        c.trigger_blink();
        assert!(c.is_actively_animating());

        // Halfway through closing
        c.tick(BLINK_DURATION_SEC / 4.0);
        assert!(c.eye_openness < 1.0 && c.eye_openness > 0.0);

        // Fully closed at half duration
        c.tick(BLINK_DURATION_SEC / 4.0);
        assert!(c.eye_openness <= 0.05);

        // Finished blink
        c.tick(BLINK_DURATION_SEC / 2.0 + 0.01);
        assert!((c.eye_openness - 1.0).abs() < f32::EPSILON);
        assert!(!c.is_blinking);
    }

    #[test]
    fn test_mood_thinking_animates_continuously() {
        let mut c = Character::new();
        c.set_mood(Mood::Thinking);
        assert!(c.is_actively_animating());
        assert!(c.tick(0.016));
    }
}
