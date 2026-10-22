use crate::{
    application::{NotchController, positioning::GestureInput},
    presentation::{
        assistant_runtime::{self, Runtime},
        assistant_view,
        drag_area::drag_area,
        face_canvas::FaceProgram,
        theme::*,
    },
};
use iced::{
    Border, Color, Element, Length, Point, alignment,
    widget::{Space, button, canvas::Canvas, column, container, row, text, tooltip},
};

#[derive(Debug, Clone)]
pub enum Message {
    Tick,
    ToggleExpand,
    CollapsePanel,
    Gesture(GestureInput),
    SelectScreen(String),
    ToggleSize,
    DismissNotice,
    ReviewNotice,
    NoticeExpired,
    Assistant(assistant_runtime::Message),
}

fn face(controller: &NotchController) -> Element<'static, Message> {
    Canvas::new(FaceProgram::new(controller.character.clone()))
        .width(60)
        .height(24)
        .into()
}

pub fn view_notch<'a>(
    controller: &'a NotchController,
    runtime: &'a Runtime,
    origin: Option<Point>,
    size: (f32, f32),
) -> Element<'a, Message> {
    let (width, height) = size;
    if !controller.panel.is_expanded {
        let notch = drag_area(
            container(face(controller))
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .width(NOTCH_COLLAPSED_WIDTH.min(width))
                .height(32)
                .style(surface),
            origin,
        );
        let mut content = column![container(notch).center_x(width)].spacing(8);
        if let Some(notice) = &runtime.assistant.notice {
            content = content.push(
                container(
                    column![
                        text(notice.text.clone()).size(12),
                        row![
                            button("Review")
                                .on_press(Message::ReviewNotice)
                                .padding([3, 8]),
                            Space::new().width(Length::Fill),
                            button("Dismiss")
                                .on_press(Message::DismissNotice)
                                .padding([3, 8])
                        ]
                        .spacing(8),
                    ]
                    .spacing(8),
                )
                .padding(10)
                .width(width)
                .style(surface),
            );
        }
        return content.width(width).height(height).into();
    }

    let header = row![
        drag_area(
            row![
                face(controller),
                text(runtime.assistant.status().to_owned()).size(12)
            ]
            .spacing(8)
            .align_y(alignment::Vertical::Center)
            .width(Length::Fill),
            origin
        ),
        button("⚙")
            .on_press(Message::Assistant(
                assistant_runtime::Message::ToggleSettings
            ))
            .style(if runtime.view == assistant_runtime::View::Settings {
                button::secondary
            } else {
                button::text
            }),
        button(if controller.panel.enlarged {
            "↙"
        } else {
            "↗"
        })
        .on_press(Message::ToggleSize)
        .style(button::text),
        button("⌃")
            .on_press(Message::ToggleExpand)
            .style(button::text),
    ]
    .align_y(alignment::Vertical::Center)
    .height(36)
    .spacing(6);
    let mut monitors = row![].spacing(4);
    if controller.screens.len() > 1 {
        let current = controller.placement.resolve(&controller.screens);
        for (index, screen) in controller.screens.iter().enumerate() {
            let selected = current.is_some_and(|current| current.id == screen.id);
            let button = button(text((index + 1).to_string()).size(11))
                .padding([3, 8])
                .on_press(Message::SelectScreen(screen.id.clone()))
                .style(if selected {
                    button::primary
                } else {
                    button::text
                });
            monitors = monitors.push(
                tooltip(
                    button,
                    text(screen.name.clone()).size(12),
                    tooltip::Position::Top,
                )
                .style(container::rounded_box),
            );
        }
    }
    let footer = row![
        monitors,
        Space::new().width(Length::Fill),
        text(if runtime.connecting {
            "Connecting…"
        } else if runtime.assistant.connected {
            "OpenCode · connected"
        } else {
            "OpenCode · offline"
        })
        .size(10)
        .color(COLOR_TEXT_MUTED)
    ]
    .align_y(alignment::Vertical::Center)
    .height(24);
    container(
        column![
            header,
            assistant_view::view(runtime).map(Message::Assistant),
            footer
        ]
        .spacing(4),
    )
    .padding([6, 12])
    .width(width)
    .height(height)
    .style(surface)
    .into()
}

fn surface(_: &iced::Theme) -> container::Style {
    container::Style {
        background: Some(COLOR_NOTCH_BG.into()),
        border: Border {
            color: COLOR_NOTCH_BORDER,
            width: 1.0,
            radius: 14.0.into(),
        },
        text_color: Some(Color::WHITE),
        ..Default::default()
    }
}
