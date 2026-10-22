use crate::application::{
    NotchController,
    positioning::{GestureAction, GestureInput, Screen},
};
use crate::infrastructure::{
    animation::{self, AnimationClock},
    position_store::PositionStore,
};
use crate::presentation::assistant_runtime::Runtime;
use crate::presentation::panel_view::{self, Message};
use crate::presentation::theme::{NOTCH_COLLAPSED_HEIGHT, NOTCH_COLLAPSED_WIDTH};
use iced::widget::container;
use iced::window::{self, Id, Level, Position, Settings};
use iced::{Color, Element, Length, Point, Size, Subscription, Task, Theme, theme};

pub struct FallbackApp {
    controller: NotchController,
    assistant: Runtime,
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
            assistant: Runtime::default(),
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
    fn size(&self) -> (f32, f32) {
        self.assistant.dimensions(
            &self.controller,
            self.controller.placement.resolve(&self.controller.screens),
        )
    }
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
        let (width, height) = self.size();
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
                            let width = self.size().0;
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
                    self.apply_placement(true),
                    self.store
                        .save(self.controller.placement.clone())
                        .map(AppMessage::Saved),
                ])
            }
            AppMessage::Presentation(Message::Assistant(
                crate::presentation::assistant_runtime::Message::QuitReady,
            )) => iced::exit(),
            AppMessage::Presentation(Message::Assistant(message)) => {
                let previous_size = self.size();
                let task = self
                    .assistant
                    .update(message)
                    .map(|m| AppMessage::Presentation(Message::Assistant(m)));
                let mood = self.assistant.assistant.mood();
                if self.controller.character.mood != mood {
                    self.controller.set_mood(mood);
                    self.clock.reset();
                }
                Task::batch([
                    task,
                    if previous_size != self.size() {
                        self.apply_placement(true)
                    } else {
                        Task::none()
                    },
                ])
            }
            AppMessage::Presentation(Message::ToggleSize) => {
                self.controller.panel.enlarged = !self.controller.panel.enlarged;
                self.apply_placement(true)
            }
            AppMessage::Presentation(Message::DismissNotice | Message::NoticeExpired) => {
                self.assistant.assistant.notice = None;
                self.controller.set_mood(self.assistant.assistant.mood());
                self.clock.reset();
                self.apply_placement(true)
            }
            AppMessage::Presentation(Message::ReviewNotice) => {
                self.controller.panel.expand();
                self.assistant.assistant.notice = None;
                self.assistant.view = crate::presentation::assistant_runtime::View::Conversation;
                self.controller.set_mood(self.assistant.assistant.mood());
                self.clock.reset();
                self.apply_placement(true)
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
                self.apply_placement(true)
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
        container(
            panel_view::view_notch(&self.controller, &self.assistant, origin, self.size())
                .map(AppMessage::Presentation),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .align_y(iced::alignment::Vertical::Top)
        .into()
    }

    pub fn subscription(&self) -> Subscription<AppMessage> {
        Subscription::batch([
            self.assistant
                .subscription()
                .map(|m| AppMessage::Presentation(Message::Assistant(m))),
            self.assistant
                .notice_subscription()
                .map(|_| AppMessage::Presentation(Message::NoticeExpired)),
            animation::ticks(&self.controller).map(|_| AppMessage::Presentation(Message::Tick)),
            window::events().filter_map(|(_, event)| match event {
                window::Event::CloseRequested => Some(AppMessage::Presentation(
                    Message::Assistant(crate::presentation::assistant_runtime::Message::Quit),
                )),
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
            exit_on_close_request: false,
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
        .theme(|_: &FallbackApp| crate::presentation::theme::app_theme())
        .run()
}
