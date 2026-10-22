use crate::application::NotchController;
use crate::presentation::panel_view::{self, Message};
use crate::presentation::theme::{
    NOTCH_COLLAPSED_HEIGHT, NOTCH_COLLAPSED_WIDTH, NOTCH_EXPANDED_HEIGHT, NOTCH_EXPANDED_WIDTH,
};
use iced::time::{Duration, every};
use iced::widget::container;
use iced::window::{self, Id, Level, Position, Settings};
use iced::{Color, Element, Length, Size, Subscription, Task, Theme, theme};

pub struct FallbackApp {
    controller: NotchController,
    window_id: Option<Id>,
}

impl Default for FallbackApp {
    fn default() -> Self {
        Self {
            controller: NotchController::new(),
            window_id: None,
        }
    }
}

impl FallbackApp {
    pub fn boot() -> (Self, Task<AppMessage>) {
        (
            Self::default(),
            window::latest().map(AppMessage::WindowOpened),
        )
    }

    pub fn update(&mut self, message: AppMessage) -> Task<AppMessage> {
        match message {
            AppMessage::WindowOpened(id) => {
                self.window_id = id;
                Task::none()
            }
            AppMessage::Presentation(Message::Tick) => {
                self.controller.tick(0.016);
                Task::none()
            }
            AppMessage::Presentation(Message::ToggleExpand) => {
                self.controller.toggle_panel();
                let is_expanded = self.controller.panel.is_expanded;
                let target_size = if is_expanded {
                    Size::new(NOTCH_EXPANDED_WIDTH, NOTCH_EXPANDED_HEIGHT)
                } else {
                    Size::new(NOTCH_COLLAPSED_WIDTH, NOTCH_COLLAPSED_HEIGHT)
                };

                if let Some(id) = self.window_id {
                    window::resize(id, target_size)
                } else {
                    Task::none()
                }
            }
            AppMessage::Presentation(Message::SetMood(mood)) => {
                self.controller.set_mood(mood);
                Task::none()
            }
            AppMessage::Presentation(Message::CycleMood) => {
                self.controller.cycle_mood();
                Task::none()
            }
            AppMessage::Presentation(Message::SelectTask(idx)) => {
                self.controller.select_task(idx);
                Task::none()
            }
        }
    }

    pub fn view(&self) -> Element<'_, AppMessage> {
        let content = panel_view::view_notch(&self.controller).map(AppMessage::Presentation);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .align_y(iced::alignment::Vertical::Top)
            .into()
    }

    pub fn subscription(&self) -> Subscription<AppMessage> {
        if self.controller.is_animating() {
            every(Duration::from_millis(16)).map(|_| AppMessage::Presentation(Message::Tick))
        } else {
            every(Duration::from_millis(100)).map(|_| AppMessage::Presentation(Message::Tick))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMessage {
    WindowOpened(Option<Id>),
    Presentation(Message),
}

pub fn run() -> iced::Result {
    iced::application(FallbackApp::boot, FallbackApp::update, FallbackApp::view)
        .title("minibro")
        .window(Settings {
            size: Size::new(NOTCH_COLLAPSED_WIDTH, NOTCH_COLLAPSED_HEIGHT),
            decorations: false,
            transparent: true,
            level: Level::AlwaysOnTop,
            position: Position::Centered,
            ..Default::default()
        })
        .style(|_state: &FallbackApp, _theme: &Theme| theme::Style {
            background_color: Color::TRANSPARENT,
            text_color: Color::WHITE,
        })
        .subscription(FallbackApp::subscription)
        .theme(|_state: &FallbackApp| Theme::Dark)
        .run()
}
