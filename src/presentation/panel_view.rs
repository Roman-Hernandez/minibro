use crate::application::NotchController;
use crate::application::positioning::GestureInput;
use crate::domain::Mood;
use crate::presentation::drag_area::drag_area;
use crate::presentation::face_canvas::FaceProgram;
use crate::presentation::theme::{
    COLOR_ACCENT_ERROR, COLOR_ACCENT_IDLE, COLOR_ACCENT_SUCCESS, COLOR_ACCENT_THINKING,
    COLOR_ACCENT_WAITING, COLOR_ACCENT_WORKING, COLOR_NOTCH_BG, COLOR_NOTCH_BORDER,
    COLOR_TEXT_MUTED, COLOR_TEXT_PRIMARY, NOTCH_COLLAPSED_HEIGHT, NOTCH_COLLAPSED_WIDTH,
    NOTCH_CORNER_RADIUS, NOTCH_EXPANDED_HEIGHT, NOTCH_EXPANDED_WIDTH,
};
use iced::border::Radius;
use iced::widget::canvas::Canvas;
use iced::widget::{
    Space, button, column, container, mouse_area, pick_list, row, scrollable, text,
};
use iced::{Alignment, Border, Color, Element, Length, Point, Shadow};

#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    Tick,
    ToggleExpand,
    Gesture(GestureInput),
    SelectScreen(String),
    SetMood(Mood),
    CycleMood,
    SelectTask(usize),
}

pub fn view_notch(
    controller: &NotchController,
    origin: Option<Point>,
) -> Element<'static, Message> {
    if controller.panel.is_expanded {
        view_expanded(controller, origin)
    } else {
        view_collapsed(controller, origin)
    }
}

fn view_collapsed(
    controller: &NotchController,
    origin: Option<Point>,
) -> Element<'static, Message> {
    let face = Canvas::new(FaceProgram::new(controller.character.clone()))
        .width(Length::Fixed(60.0))
        .height(Length::Fixed(24.0));

    let content = row![
        Space::new().width(Length::Fill),
        face,
        Space::new().width(Length::Fill),
    ]
    .align_y(Alignment::Center)
    .width(Length::Fill)
    .height(Length::Fill);

    let notch_box = container(content)
        .width(Length::Fixed(NOTCH_COLLAPSED_WIDTH))
        .height(Length::Fixed(NOTCH_COLLAPSED_HEIGHT))
        .style(|_theme| container::Style {
            background: Some(COLOR_NOTCH_BG.into()),
            border: Border {
                color: COLOR_NOTCH_BORDER,
                width: 1.0,
                radius: Radius {
                    top_left: 0.0,
                    top_right: 0.0,
                    bottom_right: NOTCH_CORNER_RADIUS,
                    bottom_left: NOTCH_CORNER_RADIUS,
                },
            },
            shadow: Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.35),
                offset: iced::Vector::new(0.0, 4.0),
                blur_radius: 12.0,
            },
            ..Default::default()
        });

    drag_area(notch_box, origin)
}

fn view_expanded(controller: &NotchController, origin: Option<Point>) -> Element<'static, Message> {
    let face = Canvas::new(FaceProgram::new(controller.character.clone()))
        .width(Length::Fixed(60.0))
        .height(Length::Fixed(24.0));

    let mood_badge = container(
        text(controller.character.mood.label())
            .size(11)
            .color(COLOR_TEXT_PRIMARY),
    )
    .padding([2, 8])
    .style(move |_| container::Style {
        background: Some(Color::from_rgba(1.0, 1.0, 1.0, 0.08).into()),
        border: Border {
            radius: 8.0.into(),
            ..Default::default()
        },
        ..Default::default()
    });

    let header_clickable = drag_area(
        row![
            face,
            mood_badge,
            Space::new().width(Length::Fill),
            text("▲").size(10).color(COLOR_TEXT_MUTED)
        ]
        .align_y(Alignment::Center)
        .spacing(8)
        .padding([4, 8]),
        origin,
    );

    // Mood quick-picker
    let mood_buttons = row(Mood::ALL
        .iter()
        .map(|mood| {
            let is_active = *mood == controller.character.mood;
            let mood_val = *mood;
            let bg_color = if is_active {
                Color::from_rgba(1.0, 1.0, 1.0, 0.16)
            } else {
                Color::from_rgba(1.0, 1.0, 1.0, 0.04)
            };

            let accent = match mood_val {
                Mood::Idle => COLOR_ACCENT_IDLE,
                Mood::Thinking => COLOR_ACCENT_THINKING,
                Mood::Working => COLOR_ACCENT_WORKING,
                Mood::Success => COLOR_ACCENT_SUCCESS,
                Mood::Waiting => COLOR_ACCENT_WAITING,
                Mood::Error => COLOR_ACCENT_ERROR,
            };

            button(text(mood.label()).size(10).color(accent))
                .padding([2, 6])
                .on_press(Message::SetMood(mood_val))
                .style(move |_, _| button::Style {
                    background: Some(bg_color.into()),
                    border: Border {
                        radius: 6.0.into(),
                        width: if is_active { 1.0 } else { 0.0 },
                        color: accent,
                    },
                    text_color: accent,
                    ..Default::default()
                })
                .into()
        })
        .collect::<Vec<_>>())
    .spacing(4)
    .align_y(Alignment::Center);

    // Task list items
    let mut task_items = column![].spacing(6);
    for (idx, task) in controller.panel.tasks.iter().enumerate() {
        let is_selected = controller.panel.selected_task == Some(idx);
        let task_card = mouse_area(
            container(
                column![
                    row![
                        text(task.agent).size(11).color(COLOR_ACCENT_WAITING),
                        Space::new().width(Length::Fill),
                        text(task.status).size(10).color(if task.status == "Done" {
                            COLOR_ACCENT_SUCCESS
                        } else {
                            COLOR_TEXT_MUTED
                        }),
                    ],
                    text(task.summary).size(12).color(COLOR_TEXT_PRIMARY),
                ]
                .spacing(2)
                .padding(8),
            )
            .width(Length::Fill)
            .style(move |_| container::Style {
                background: Some(if is_selected {
                    Color::from_rgba(1.0, 1.0, 1.0, 0.07).into()
                } else {
                    Color::from_rgba(1.0, 1.0, 1.0, 0.02).into()
                }),
                border: Border {
                    radius: 8.0.into(),
                    width: 1.0,
                    color: if is_selected {
                        Color::from_rgba(1.0, 1.0, 1.0, 0.18)
                    } else {
                        Color::from_rgba(1.0, 1.0, 1.0, 0.04)
                    },
                },
                ..Default::default()
            }),
        )
        .on_press(Message::SelectTask(idx));

        task_items = task_items.push(task_card);
    }

    let scroll_tasks = scrollable(task_items).height(Length::Fill);

    let selected_screen = controller.placement.resolve(&controller.screens).cloned();
    let screen_picker: Element<'static, Message> = if controller.screens.len() > 1 {
        row![
            text("Screen").size(11).color(COLOR_TEXT_MUTED),
            pick_list(controller.screens.clone(), selected_screen, |screen| {
                Message::SelectScreen(screen.id)
            })
            .text_size(11),
        ]
        .spacing(8)
        .align_y(Alignment::Center)
        .into()
    } else {
        Space::new().into()
    };

    let content = column![
        header_clickable,
        Space::new().height(Length::Fixed(4.0)),
        mood_buttons,
        screen_picker,
        Space::new().height(Length::Fixed(8.0)),
        scroll_tasks,
    ]
    .spacing(4)
    .padding(12);

    container(content)
        .width(Length::Fixed(NOTCH_EXPANDED_WIDTH))
        .height(Length::Fixed(NOTCH_EXPANDED_HEIGHT))
        .style(|_theme| container::Style {
            background: Some(COLOR_NOTCH_BG.into()),
            border: Border {
                color: COLOR_NOTCH_BORDER,
                width: 1.0,
                radius: Radius {
                    top_left: 0.0,
                    top_right: 0.0,
                    bottom_right: NOTCH_CORNER_RADIUS,
                    bottom_left: NOTCH_CORNER_RADIUS,
                },
            },
            shadow: Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.45),
                offset: iced::Vector::new(0.0, 6.0),
                blur_radius: 16.0,
            },
            ..Default::default()
        })
        .into()
}
