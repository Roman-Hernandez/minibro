use crate::application::{
    NotchController,
    positioning::{GestureAction, GestureInput, Screen},
};
use crate::infrastructure::{
    animation::{self, AnimationClock},
    position_store::PositionStore,
};
use crate::presentation::panel_view::{self, Message};
use crate::presentation::theme::{
    NOTCH_COLLAPSED_HEIGHT, NOTCH_COLLAPSED_WIDTH, NOTCH_EXPANDED_HEIGHT, NOTCH_EXPANDED_WIDTH,
};
use iced::widget::container;
use iced::window::{self, Id, Level, Position, Settings};
use iced::{Color, Element, Length, Point, Size, Subscription, Task, Theme, theme};

pub struct FallbackApp {
    controller: NotchController,
    window_id: Option<Id>,
    window_origin: Option<Point>,
    position_ready: bool,
    store: PositionStore,
    clock: AnimationClock,
}

impl Default for FallbackApp {
    fn default() -> Self {
        let store = PositionStore::default();
        let mut controller = NotchController::new();
        controller.placement = store.restore();
        controller.screens = platform_screens();
        Self {
            controller,
            window_id: None,
            window_origin: None,
            position_ready: false,
            store,
            clock: AnimationClock::default(),
        }
    }
}

fn platform_screens() -> Vec<Screen> {
    #[cfg(target_os = "macos")]
    {
        crate::infrastructure::screens_macos::screens()
    }
    #[cfg(not(target_os = "macos"))]
    {
        Vec::new()
    }
}

impl FallbackApp {
    pub fn boot() -> (Self, Task<AppMessage>) {
        (
            Self::default(),
            window::latest().map(AppMessage::WindowOpened),
        )
    }

    fn apply_placement(&self, resize: bool) -> Task<AppMessage> {
        let Some(id) = self.window_id else {
            return Task::none();
        };
        let (width, height) = if self.controller.panel.is_expanded {
            (NOTCH_EXPANDED_WIDTH, NOTCH_EXPANDED_HEIGHT)
        } else {
            (NOTCH_COLLAPSED_WIDTH, NOTCH_COLLAPSED_HEIGHT)
        };
        let movement = self
            .controller
            .placement
            .resolve(&self.controller.screens)
            .map_or_else(Task::none, |screen| {
                window::move_to(
                    id,
                    Point::new(
                        screen.x + self.controller.placement.left(screen, width),
                        screen.y,
                    ),
                )
            });
        if resize {
            Task::batch([window::resize(id, Size::new(width, height)), movement])
        } else {
            movement
        }
    }

    pub fn update(&mut self, message: AppMessage) -> Task<AppMessage> {
        match message {
            AppMessage::WindowOpened(id) => {
                self.window_id = id;
                self.apply_placement(true)
                    .chain(id.map_or_else(Task::none, |id| {
                        window::position(id).map(AppMessage::WindowPosition)
                    }))
            }
            AppMessage::WindowPosition(position) => {
                self.window_origin = position;
                self.position_ready = true;
                Task::none()
            }
            AppMessage::WindowMoved(position) => {
                self.window_origin = Some(position);
                self.position_ready = true;
                self.update(AppMessage::RefreshScreens)
            }
            AppMessage::Presentation(Message::Tick) => {
                self.clock.tick(&mut self.controller);
                Task::none()
            }
            AppMessage::Presentation(Message::ToggleExpand) => {
                self.controller.screens = platform_screens();
                self.controller.toggle_panel();
                self.clock.reset();
                self.apply_placement(true)
            }
            AppMessage::Presentation(Message::Gesture(input)) => {
                match self.controller.pointer(input) {
                    GestureAction::Click => {
                        self.update(AppMessage::Presentation(Message::ToggleExpand))
                    }
                    GestureAction::Drag { dx, started, .. } => {
                        if let Some(screen) = self
                            .controller
                            .placement
                            .resolve(&self.controller.screens)
                            .cloned()
                            .filter(|_| self.window_origin.is_some())
                        {
                            let width = if self.controller.panel.is_expanded {
                                NOTCH_EXPANDED_WIDTH
                            } else {
                                NOTCH_COLLAPSED_WIDTH
                            };
                            self.controller.drag_by(&screen, width, dx);
                            self.apply_placement(false)
                        } else if started {
                            // Ordinary Wayland windows cannot be positioned by clients; let the compositor move them.
                            self.window_id.map_or_else(Task::none, window::drag)
                        } else {
                            Task::none()
                        }
                    }
                    GestureAction::Dropped if !self.controller.screens.is_empty() => self
                        .store
                        .save(self.controller.placement.clone())
                        .map(AppMessage::Saved),
                    _ => Task::none(),
                }
            }
            AppMessage::Presentation(Message::SelectScreen(id)) => {
                if !self.controller.screens.iter().any(|screen| screen.id == id) {
                    return Task::none();
                }
                self.controller.placement.screen = id;
                Task::batch([
                    self.apply_placement(false),
                    self.store
                        .save(self.controller.placement.clone())
                        .map(AppMessage::Saved),
                ])
            }
            AppMessage::Presentation(Message::SetMood(mood)) => {
                self.controller.set_mood(mood);
                self.clock.reset();
                Task::none()
            }
            AppMessage::Presentation(Message::CycleMood) => {
                self.controller.cycle_mood();
                self.clock.reset();
                Task::none()
            }
            AppMessage::Presentation(Message::SelectTask(idx)) => {
                self.controller.select_task(idx);
                Task::none()
            }
            AppMessage::RefreshScreens => self.window_id.map_or_else(Task::none, |id| {
                window::run(id, |_| platform_screens()).map(AppMessage::Screens)
            }),
            AppMessage::Screens(screens) => {
                if screens == self.controller.screens {
                    return Task::none();
                }
                self.controller.pointer(GestureInput::Cancelled);
                self.controller.screens = screens;
                self.apply_placement(false)
            }
            AppMessage::Saved(result) => {
                if let Err(error) = result {
                    eprintln!("[minibro] Could not save position: {error}");
                }
                Task::none()
            }
        }
    }

    pub fn view(&self) -> Element<'_, AppMessage> {
        let origin = if self.controller.screens.is_empty() {
            Some(Point::ORIGIN)
        } else {
            self.position_ready
                .then_some(self.window_origin.unwrap_or(Point::ORIGIN))
        };
        container(panel_view::view_notch(&self.controller, origin).map(AppMessage::Presentation))
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .align_y(iced::alignment::Vertical::Top)
            .into()
    }

    pub fn subscription(&self) -> Subscription<AppMessage> {
        Subscription::batch([
            animation::ticks(&self.controller).map(|_| AppMessage::Presentation(Message::Tick)),
            window::events().filter_map(|(_, event)| match event {
                window::Event::Moved(position) => Some(AppMessage::WindowMoved(position)),
                window::Event::Resized(_) | window::Event::Rescaled(_) | window::Event::Focused => {
                    Some(AppMessage::RefreshScreens)
                }
                _ => None,
            }),
        ])
    }
}

#[derive(Debug, Clone)]
pub enum AppMessage {
    WindowOpened(Option<Id>),
    WindowPosition(Option<Point>),
    WindowMoved(Point),
    Presentation(Message),
    RefreshScreens,
    Screens(Vec<Screen>),
    Saved(Result<(), String>),
}

pub fn run() -> iced::Result {
    iced::application(FallbackApp::boot, FallbackApp::update, FallbackApp::view)
        .title("minibro")
        .window(Settings {
            size: Size::new(NOTCH_COLLAPSED_WIDTH, NOTCH_COLLAPSED_HEIGHT),
            decorations: false,
            transparent: true,
            level: Level::AlwaysOnTop,
            position: Position::SpecificWith(|size, screen| {
                Point::new(((screen.width - size.width) / 2.0).max(0.0), 0.0)
            }),
            ..Default::default()
        })
        .style(|_: &FallbackApp, _: &Theme| theme::Style {
            background_color: Color::TRANSPARENT,
            text_color: Color::WHITE,
        })
        .subscription(FallbackApp::subscription)
        .theme(|_: &FallbackApp| Theme::Dark)
        .run()
}
