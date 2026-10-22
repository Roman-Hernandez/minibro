use crate::application::NotchController;
use crate::presentation::panel_view::{self, Message};
use crate::presentation::theme::{
    NOTCH_COLLAPSED_HEIGHT, NOTCH_COLLAPSED_WIDTH, NOTCH_EXPANDED_HEIGHT, NOTCH_EXPANDED_WIDTH,
};
use iced::time::{Duration, every};
use iced::widget::container;
use iced::{Color, Element, Length, Subscription, Task, Theme, theme};
use iced_exwlshell::layershell::application;
use iced_exwlshell::reexport::{Anchor, KeyboardInteractivity, Layer, LayerSize, Margin};
use iced_exwlshell::settings::LayerShellSettings;
use iced_exwlshell::to_layer_message;

#[derive(Default)]
pub struct WaylandApp {
    controller: NotchController,
}

#[to_layer_message]
#[derive(Debug, Clone)]
pub enum WaylandMessage {
    Presentation(Message),
}

impl WaylandApp {
    pub fn update(&mut self, message: WaylandMessage) -> Task<WaylandMessage> {
        match message {
            WaylandMessage::Presentation(Message::Tick) => {
                self.controller.tick(0.016);
                Task::none()
            }
            WaylandMessage::Presentation(Message::ToggleExpand) => {
                self.controller.toggle_panel();
                let is_expanded = self.controller.panel.is_expanded;
                let target_size = if is_expanded {
                    LayerSize::px(NOTCH_EXPANDED_WIDTH as u32, NOTCH_EXPANDED_HEIGHT as u32)
                } else {
                    LayerSize::px(NOTCH_COLLAPSED_WIDTH as u32, NOTCH_COLLAPSED_HEIGHT as u32)
                };

                Task::done(WaylandMessage::LayoutChange {
                    anchor: Anchor::Top,
                    size: target_size,
                })
            }
            WaylandMessage::Presentation(Message::SetMood(mood)) => {
                self.controller.set_mood(mood);
                Task::none()
            }
            WaylandMessage::Presentation(Message::CycleMood) => {
                self.controller.cycle_mood();
                Task::none()
            }
            WaylandMessage::Presentation(Message::SelectTask(idx)) => {
                self.controller.select_task(idx);
                Task::none()
            }
            _ => Task::none(),
        }
    }

    pub fn view(&self) -> Element<'_, WaylandMessage> {
        let content = panel_view::view_notch(&self.controller).map(WaylandMessage::Presentation);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .align_y(iced::alignment::Vertical::Top)
            .into()
    }

    pub fn subscription(&self) -> Subscription<WaylandMessage> {
        if self.controller.is_animating() {
            every(Duration::from_millis(16)).map(|_| WaylandMessage::Presentation(Message::Tick))
        } else {
            every(Duration::from_millis(100)).map(|_| WaylandMessage::Presentation(Message::Tick))
        }
    }
}

pub fn run() -> Result<(), iced_exwlshell::Error> {
    application(
        WaylandApp::default,
        "minibro-notch",
        WaylandApp::update,
        WaylandApp::view,
    )
    .layer_settings(LayerShellSettings {
        anchor: Anchor::Top,
        layer: Layer::Top,
        exclusive_zone: 0,
        size: LayerSize::px(NOTCH_COLLAPSED_WIDTH as u32, NOTCH_COLLAPSED_HEIGHT as u32),
        margin: Margin::default(),
        keyboard_interactivity: KeyboardInteractivity::None,
        ..Default::default()
    })
    .style(|_state: &WaylandApp, _theme: &Theme| theme::Style {
        background_color: Color::TRANSPARENT,
        text_color: Color::WHITE,
    })
    .subscription(WaylandApp::subscription)
    .theme(|_state: &WaylandApp| Some(Theme::Dark))
    .run()
}
