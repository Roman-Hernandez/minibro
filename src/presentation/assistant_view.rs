use super::assistant_runtime::{Message, Runtime, View};
use super::theme::{COLOR_ACCENT_WAITING, COLOR_TEXT_MUTED};
use iced::{
    Element, Length, alignment, keyboard,
    widget::{Space, button, column, container, row, scrollable, text, text_editor, text_input},
};

pub fn view(runtime: &Runtime) -> Element<'_, Message> {
    let mut tabs = row![].spacing(4);
    for (label, view) in [("Activity", View::Activity), ("Chat", View::Conversation)] {
        let selected = view == runtime.view;
        tabs = tabs.push(
            button(text(label).size(12))
                .padding([4, 8])
                .style(move |_, status| button::Style {
                    background: (selected || matches!(status, button::Status::Hovered))
                        .then_some(iced::Color::from_rgba(1.0, 1.0, 1.0, 0.07).into()),
                    text_color: if selected {
                        super::theme::COLOR_TEXT_PRIMARY
                    } else {
                        COLOR_TEXT_MUTED
                    },
                    border: iced::Border {
                        radius: 6.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                })
                .on_press(Message::View(view)),
        );
    }
    let mut body = column![tabs].spacing(8).height(Length::Fill);
    if let Some(error) = &runtime.assistant.error {
        body = body.push(
            text(error_summary(error))
                .size(12)
                .color(COLOR_ACCENT_WAITING),
        );
    }
    let mut content = column![].spacing(14).padding([8, 0]);
    if runtime.view == View::Activity
        && let Some(error) = &runtime.assistant.error
    {
        content = content.push(text(error.clone()).size(11));
    }
    if !runtime.assistant.connected {
        content = content.push(text("What shall we do today?").size(22));
        content = content.push(
            text("Choose a working folder and connect OpenCode to get started.")
                .size(12)
                .color(COLOR_TEXT_MUTED),
        );
        content = content.push(button("Open Settings…").on_press(Message::OpenSettings));
    }
    for permission in &runtime.assistant.snapshot.permissions {
        let pending = runtime.assistant.in_flight.contains(&permission.id)
            || !runtime.assistant.connected
            || runtime.assistant.uncertain;
        content = content.push(
            container(
                column![
                    text(format!("Permission · {}", permission.operation))
                        .size(14)
                        .color(COLOR_ACCENT_WAITING),
                    text(format!(
                        "Folder: {}\nSession: {}",
                        runtime.settings.folder.display(),
                        permission.session
                    ))
                    .size(11)
                    .color(COLOR_TEXT_MUTED),
                    text(permission.patterns.join("\n"))
                        .font(iced::Font::MONOSPACE)
                        .size(12),
                    text(
                        if permission.details == "null" || permission.details == "{}" {
                            "Additional action details unavailable".into()
                        } else {
                            permission.details.clone()
                        }
                    )
                    .size(12),
                    row![
                        button("Reject").style(button::danger).on_press_maybe(
                            (!pending).then(|| Message::Permission(permission.id.clone(), false))
                        ),
                        button("Allow once").on_press_maybe(
                            (!pending).then(|| Message::Permission(permission.id.clone(), true))
                        )
                    ]
                    .spacing(8),
                ]
                .spacing(8),
            )
            .padding(10)
            .width(Length::Fill)
            .style(container::rounded_box),
        );
    }
    for question in &runtime.assistant.snapshot.questions {
        let mut items = column![text("Your assistant has a question").size(14)].spacing(8);
        for (index, item) in question.items.iter().enumerate() {
            items = items.push(text(item.text.clone()).size(13));
            let answer = runtime.answers.get(&(question.id.clone(), index));
            for option in &item.options {
                let selected = answer.is_some_and(|a| a.choices.contains(option));
                items = items.push(
                    button(
                        text(format!("{} {}", if selected { "●" } else { "○" }, option)).size(12),
                    )
                    .style(if selected {
                        button::secondary
                    } else {
                        button::text
                    })
                    .on_press(Message::Choice(
                        question.id.clone(),
                        index,
                        option.clone(),
                    )),
                );
            }
            if item.custom {
                let id = question.id.clone();
                items = items.push(
                    text_input(
                        "Your answer…",
                        answer.map(|a| a.custom.as_str()).unwrap_or(""),
                    )
                    .on_input(move |text| Message::CustomAnswer(id.clone(), index, text))
                    .size(12),
                );
            }
        }
        let available = !runtime.assistant.in_flight.contains(&question.id)
            && runtime.assistant.connected
            && !runtime.assistant.uncertain;
        items = items.push(
            row![
                button("Reject").style(button::danger).on_press_maybe(
                    available.then(|| Message::RejectQuestion(question.id.clone()))
                ),
                button("Reply")
                    .on_press_maybe(available.then(|| Message::Answer(question.id.clone())))
            ]
            .spacing(8),
        );
        content = content.push(
            container(items)
                .padding(10)
                .width(Length::Fill)
                .style(container::rounded_box),
        );
    }
    if runtime.view == View::Conversation {
        for message in &runtime.assistant.snapshot.messages {
            if !message.text.is_empty() {
                let is_user = message.role == "user";
                let sender = text(if is_user { "You" } else { "minibro" })
                    .size(11)
                    .color(COLOR_TEXT_MUTED);
                let md: Element<'_, Message> = if let Some((_, items)) =
                    runtime.markdown_cache.get(&message.id)
                    && !items.is_empty()
                {
                    iced::widget::markdown::view(
                        items.as_slice(),
                        iced::widget::markdown::Settings::with_text_size(
                            13,
                            iced::widget::markdown::Style::from(&iced::Theme::Dark),
                        ),
                    )
                    .map(|_| Message::Noop)
                } else {
                    text(&message.text).size(13).into()
                };
                content = content.push(column![sender, md].spacing(4));
            }
        }
    } else {
        if runtime.assistant.snapshot.activity.is_empty() {
            content = content.push(
                text("Actions will appear here.")
                    .size(12)
                    .color(COLOR_TEXT_MUTED),
            );
        }
        for activity in &runtime.assistant.snapshot.activity {
            let mut item = column![
                button(text(format!("{} · {}", activity.title, activity.status)).size(12))
                    .style(button::text)
                    .on_press(Message::ToggleActivity(activity.id.clone()))
            ]
            .spacing(6);
            if runtime.expanded_activity.contains(&activity.id) {
                item = item.push(
                    text(format!(
                        "Session {}\n{}\n{}",
                        activity.session, activity.input, activity.output
                    ))
                    .font(iced::Font::MONOSPACE)
                    .size(11),
                );
            }
            content = content.push(item);
        }
    }
    content = content.push(
        text("Showing the recent conversation; full history stays in OpenCode.")
            .size(10)
            .color(COLOR_TEXT_MUTED),
    );
    body = body.push(
        scrollable(content)
            .id("assistant-thread")
            .on_scroll(|viewport| Message::Scrolled(viewport.relative_offset().y >= 0.95))
            .height(Length::Fill),
    );
    let editor = text_editor(&runtime.editor)
        .placeholder("Ask minibro to do something…")
        .on_action(Message::Edit)
        .height(64)
        .size(13)
        .key_binding(|press| {
            if matches!(press.key, keyboard::Key::Named(keyboard::key::Named::Enter))
                && !press.modifiers.shift()
                && matches!(press.status, text_editor::Status::Focused { .. })
            {
                Some(text_editor::Binding::Custom(Message::Send))
            } else {
                text_editor::Binding::from_key_press(press)
            }
        });
    let send = if runtime.assistant.busy() {
        button(if runtime.assistant.stopping {
            "Stopping…"
        } else {
            "Stop"
        })
        .on_press_maybe(
            (runtime.assistant.connected && !runtime.assistant.stopping).then_some(Message::Stop),
        )
    } else {
        button("Send ↑").on_press_maybe(runtime.assistant.can_send().then_some(Message::Send))
    };
    body = body.push(editor).push(
        row![
            text(runtime.assistant.snapshot.model.clone())
                .size(10)
                .color(COLOR_TEXT_MUTED),
            Space::new().width(Length::Fill),
            send.padding([4, 12])
        ]
        .align_y(alignment::Vertical::Center),
    );
    body.into()
}

fn error_summary(error: &str) -> String {
    let summary: String = error.chars().take(240).collect();
    if summary.len() < error.len() {
        format!("{summary}…")
    } else {
        summary
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn long_errors_cannot_push_all_controls_outside_the_panel() {
        let summary = super::error_summary(&"🦀".repeat(1000));
        assert!(summary.chars().count() <= 241);
        assert!(summary.ends_with('…'));
    }
}
