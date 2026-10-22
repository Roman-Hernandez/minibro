use crate::application::NotchController;
use iced::{
    Subscription,
    time::{Duration, Instant, every},
};

pub struct AnimationClock(Instant);

impl Default for AnimationClock {
    fn default() -> Self {
        Self(Instant::now())
    }
}

impl AnimationClock {
    pub fn tick(&mut self, controller: &mut NotchController) {
        let now = Instant::now();
        controller.tick(now.duration_since(self.0).as_secs_f32());
        self.0 = now;
    }

    pub fn reset(&mut self) {
        self.0 = Instant::now();
    }
}

pub fn ticks(controller: &NotchController) -> Subscription<()> {
    every(Duration::from_secs_f32(
        controller.character.next_update_in(),
    ))
    .map(|_| ())
}
