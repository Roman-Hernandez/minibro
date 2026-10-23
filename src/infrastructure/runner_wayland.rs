use crate::application::{
    NotchController,
    positioning::{GestureAction, GestureInput, Screen},
};
use crate::infrastructure::{
    animation::{self, AnimationClock},
    position_store::PositionStore,
};
use crate::presentation::{
    assistant_runtime::Runtime,
    panel_view::{self, Message},
    theme::{NOTCH_COLLAPSED_HEIGHT, NOTCH_COLLAPSED_WIDTH},
};
use iced::{
    Color, Element, Length, Subscription, Task, Theme, theme,
    widget::{Space, button, container, row},
    window::Id,
};
use iced_exwlshell::actions::IcedXdgWindowSettings;
use iced_exwlshell::reexport::{
    Anchor, KeyboardInteractivity, Layer, LayerSize, Margin, NewLayerShellSettings, OutputOption,
    PixelSize,
};
use iced_exwlshell::{
    daemon,
    settings::{ExWlSettings, LayerShellSettings},
    shell::{self, ShellEvent},
    to_layer_message,
};
use std::collections::BTreeMap;

pub struct WaylandApp {
    controller: NotchController,
    assistant: Runtime,
    store: PositionStore,
    clock: AnimationClock,
    receiver: shell::ShellReceiver,
    outputs: BTreeMap<u32, Screen>,
    surface: Option<Id>,
    current_screen: Option<String>,
    pending: Option<(Id, String)>,
    pub settings_window: Option<Id>,
}

#[to_layer_message(multi)]
#[derive(Debug, Clone)]
pub enum WaylandMessage {
    Presentation(Message),
    SurfaceReady(Id),
    WindowClosed(Id),
    Shell(Box<ShellEvent>),
    Saved(Result<(), String>),
}

fn screen(info: &shell::ExWlOutputInfo) -> Option<Screen> {
    let (width, height) = info.logical_size.or_else(|| {
        let mode = info.modes.iter().find(|mode| mode.current)?;
        let (mut width, mut height) = mode.dimensions;
        let transform: u32 = info.transform.into();
        if transform % 2 == 1 {
            std::mem::swap(&mut width, &mut height);
        }
        Some((
            width / info.scale_factor.max(1),
            height / info.scale_factor.max(1),
        ))
    })?;
    if width <= 0 || height <= 0 {
        return None;
    }
    let (x, y) = info.logical_position.unwrap_or(info.location);
    let name = info
        .name
        .clone()
        .unwrap_or_else(|| format!("{} {}", info.make, info.model));
    Some(Screen {
        id: name.clone(),
        name,
        x: x as f32,
        y: y as f32,
        width: width as f32,
        height: height as f32,
    })
}

impl WaylandApp {
    fn new(receiver: shell::ShellReceiver) -> Self {
        let store = PositionStore::default();
        let mut controller = NotchController::new();
        controller.placement = store.restore();
        Self {
            controller,
            assistant: Runtime::default(),
            store,
            receiver,
            clock: AnimationClock::default(),
            outputs: BTreeMap::new(),
            surface: None,
            current_screen: None,
            pending: None,
            settings_window: None,
        }
    }

    fn size(&self) -> (f32, f32) {
        let screen = self
            .controller
            .screens
            .iter()
            .find(|s| Some(&s.id) == self.current_screen.as_ref())
            .or_else(|| self.controller.placement.resolve(&self.controller.screens));
        self.assistant.dimensions(&self.controller, screen)
    }

    fn apply_placement(&self, resize: bool) -> Task<WaylandMessage> {
        let Some(id) = self.surface else {
            return Task::none();
        };
        let Some(screen) = self
            .controller
            .screens
            .iter()
            .find(|screen| Some(&screen.id) == self.current_screen.as_ref())
        else {
            return Task::none();
        };
        let (width, height) = self.size();
        let left = self.controller.placement.left(screen, width).round() as i32;
        let screen_width = screen.width.round() as i32;
        let expanded = self.controller.panel.is_expanded;
        let notice = self.assistant.assistant.notice.is_some();
        let region = Task::done(WaylandMessage::SetInputRegion {
            id,
            callback: iced_exwlshell::actions::RegionActionCB::new(move |region| {
                let result = if expanded {
                    region.add(0, 0, screen_width, height as i32)
                } else {
                    let notch_width = width.min(NOTCH_COLLAPSED_WIDTH);
                    region
                        .add(
                            left + ((width - notch_width) / 2.0) as i32,
                            0,
                            notch_width as i32,
                            32,
                        )
                        .and_then(|_| {
                            if notice {
                                region.add(left, 40, width as i32, (height - 40.0).max(0.0) as i32)
                            } else {
                                Ok(())
                            }
                        })
                };
                if let Err(error) = result {
                    eprintln!("[minibro] Could not set notch input region: {error:?}");
                }
            }),
        });
        if resize {
            Task::batch([
                Task::done(WaylandMessage::LayoutChange {
                    id,
                    anchor: Anchor::Top | Anchor::Left | Anchor::Right,
                    size: LayerSize::fill_width(height as u32),
                }),
                region,
                Task::done(WaylandMessage::KeyboardInteractivityChange {
                    id,
                    keyboard_interactivity: if expanded {
                        KeyboardInteractivity::OnDemand
                    } else {
                        KeyboardInteractivity::None
                    },
                }),
            ])
        } else {
            region
        }
    }

    fn reconcile(&mut self) -> Task<WaylandMessage> {
        let Some(target) = self
            .controller
            .placement
            .resolve(&self.controller.screens)
            .cloned()
        else {
            self.current_screen = None;
            return Task::batch(
                self.surface
                    .take()
                    .into_iter()
                    .chain(self.pending.take().map(|(id, _)| id))
                    .map(|id| Task::done(WaylandMessage::RemoveWindow(id))),
            );
        };
        if self.surface.is_some() && self.current_screen.as_ref() == Some(&target.id) {
            let cancelled = self.pending.take().map(|(id, _)| id);
            return Task::batch([
                self.apply_placement(true),
                cancelled.map_or_else(Task::none, |id| {
                    Task::done(WaylandMessage::RemoveWindow(id))
                }),
            ]);
        }
        if self
            .pending
            .as_ref()
            .is_some_and(|(_, screen)| screen == &target.id)
        {
            return Task::none();
        }
        let Some((&output, _)) = self
            .outputs
            .iter()
            .find(|(_, screen)| screen.id == target.id)
        else {
            return Task::none();
        };
        let stale = self.pending.take().map(|(id, _)| id);
        let id = Id::unique();
        self.pending = Some((id, target.id.clone()));
        let (_, height) = self.size();
        let creation = Task::done(WaylandMessage::NewLayerShell {
            id,
            settings: NewLayerShellSettings {
                size: LayerSize::fill_width(height as u32),
                layer: Layer::Top,
                anchor: Anchor::Top | Anchor::Left | Anchor::Right,
                exclusive_zone: Some(0),
                margin: Some(Margin::default()),
                keyboard_interactivity: KeyboardInteractivity::None,
                output_option: OutputOption::GlobalName(output),
                events_transparent: true,
                namespace: Some("minibro-notch".into()),
                blur_option: Default::default(),
            },
        });
        Task::batch([
            creation,
            stale.map_or_else(Task::none, |id| {
                Task::done(WaylandMessage::RemoveWindow(id))
            }),
        ])
    }

    pub fn update(&mut self, message: WaylandMessage) -> Task<WaylandMessage> {
        match message {
            WaylandMessage::SurfaceReady(id) => {
                if let Some((_, target)) =
                    self.pending.as_ref().filter(|(pending, _)| *pending == id)
                {
                    let target = target.clone();
                    self.pending = None;
                    let previous = self.surface.replace(id);
                    self.current_screen = Some(target);
                    self.clock.reset();
                    Task::batch([
                        self.apply_placement(true),
                        previous.map_or_else(Task::none, |id| {
                            Task::done(WaylandMessage::RemoveWindow(id))
                        }),
                    ])
                } else if self.surface.is_none() {
                    self.surface = Some(id);
                    Task::none()
                } else {
                    Task::none()
                }
            }
            WaylandMessage::Shell(event) => match *event {
                ShellEvent::OutputAdded(info) | ShellEvent::OutputUpdated(info) => {
                    if let Some(screen) = screen(&info) {
                        self.outputs.insert(info.id, screen);
                    }
                    self.controller.screens = self.outputs.values().cloned().collect();
                    self.controller.pointer(GestureInput::Cancelled);
                    self.reconcile()
                }
                ShellEvent::OutputRemoved(info) => {
                    self.outputs.remove(&info.id);
                    self.controller.screens = self.outputs.values().cloned().collect();
                    self.controller.pointer(GestureInput::Cancelled);
                    self.reconcile()
                }
                ShellEvent::WindowOutputChanged {
                    window,
                    output: Some(info),
                } if self.surface == Some(window) => {
                    if let Some(screen) = screen(&info) {
                        self.current_screen = Some(screen.id.clone());
                        self.outputs.insert(info.id, screen);
                        self.controller.screens = self.outputs.values().cloned().collect();
                    }
                    self.reconcile()
                }
                ShellEvent::Closed(id) => {
                    if self.surface == Some(id) {
                        self.surface = None;
                        self.current_screen = None;
                    }
                    if self
                        .pending
                        .as_ref()
                        .is_some_and(|(pending, _)| *pending == id)
                    {
                        self.pending = None;
                    }
                    Task::none()
                }
                _ => Task::none(),
            },
            WaylandMessage::Presentation(Message::Tick) => {
                self.clock.tick(&mut self.controller);
                Task::none()
            }
            WaylandMessage::Presentation(Message::ToggleExpand) => {
                self.controller.toggle_panel();
                self.clock.reset();
                self.apply_placement(true)
            }
            WaylandMessage::Presentation(Message::CollapsePanel) => {
                if self.controller.panel.is_expanded && !self.assistant.dialog_open {
                    self.controller.panel.collapse();
                    self.clock.reset();
                    self.apply_placement(true)
                } else {
                    Task::none()
                }
            }
            WaylandMessage::Presentation(Message::OpenSettings) => {
                if let Some(id) = self.settings_window.take() {
                    Task::done(WaylandMessage::RemoveWindow(id))
                } else {
                    let id = Id::unique();
                    self.settings_window = Some(id);
                    Task::done(WaylandMessage::NewBaseWindow {
                        settings: IcedXdgWindowSettings {
                            size: Some(PixelSize {
                                width: std::num::NonZeroU32::new(480).expect("valid width"),
                                height: std::num::NonZeroU32::new(440).expect("valid height"),
                            }),
                            client_side_decorations: false,
                            app_id: Some("minibro-settings".into()),
                        },
                        id,
                    })
                }
            }
            WaylandMessage::Presentation(Message::CloseSettings) => {
                if let Some(id) = self.settings_window.take() {
                    Task::done(WaylandMessage::RemoveWindow(id))
                } else {
                    Task::none()
                }
            }
            WaylandMessage::WindowClosed(id) => {
                if self.settings_window == Some(id) {
                    self.settings_window = None;
                }
                Task::none()
            }
            WaylandMessage::Presentation(Message::Gesture(input)) => {
                match self.controller.pointer(input) {
                    GestureAction::Click => {
                        self.update(WaylandMessage::Presentation(Message::ToggleExpand))
                    }
                    GestureAction::Drag { dx, .. } => {
                        if let Some(screen) = self
                            .controller
                            .screens
                            .iter()
                            .find(|screen| Some(&screen.id) == self.current_screen.as_ref())
                            .cloned()
                        {
                            let width = self.size().0;
                            self.controller.drag_by(&screen, width, dx);
                            self.apply_placement(false)
                        } else {
                            Task::none()
                        }
                    }
                    GestureAction::Dropped => self
                        .store
                        .save(self.controller.placement.clone())
                        .map(WaylandMessage::Saved),
                    _ => Task::none(),
                }
            }
            WaylandMessage::Presentation(Message::SelectScreen(id)) => {
                if !self.controller.screens.iter().any(|screen| screen.id == id) {
                    return Task::none();
                }
                self.controller.placement.screen = id;
                self.controller.pointer(GestureInput::Cancelled);
                Task::batch([
                    self.reconcile(),
                    self.store
                        .save(self.controller.placement.clone())
                        .map(WaylandMessage::Saved),
                ])
            }
            WaylandMessage::Presentation(Message::Assistant(
                crate::presentation::assistant_runtime::Message::QuitReady,
            )) => iced::exit(),
            WaylandMessage::Presentation(Message::Assistant(message)) => {
                let previous_size = self.size();
                let task = self
                    .assistant
                    .update(message)
                    .map(|m| WaylandMessage::Presentation(Message::Assistant(m)));
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
            WaylandMessage::Presentation(Message::ToggleSize) => {
                self.controller.panel.enlarged = !self.controller.panel.enlarged;
                self.apply_placement(true)
            }
            WaylandMessage::Presentation(Message::DismissNotice | Message::NoticeExpired) => {
                self.assistant.assistant.notice = None;
                self.controller.set_mood(self.assistant.assistant.mood());
                self.clock.reset();
                self.apply_placement(true)
            }
            WaylandMessage::Presentation(Message::ReviewNotice) => {
                self.controller.panel.expand();
                self.assistant.assistant.notice = None;
                self.assistant.view = crate::presentation::assistant_runtime::View::Activity;
                self.controller.set_mood(self.assistant.assistant.mood());
                self.clock.reset();
                self.apply_placement(true)
            }
            WaylandMessage::Saved(result) => {
                if let Err(error) = result {
                    eprintln!("[minibro] Could not save position: {error}");
                }
                Task::none()
            }
            _ => Task::none(),
        }
    }

    pub fn view(&self, id: Id) -> Element<'_, WaylandMessage> {
        if Some(id) == self.settings_window {
            return panel_view::view_settings_window(&self.assistant)
                .map(WaylandMessage::Presentation);
        }
        let target = self
            .pending
            .as_ref()
            .filter(|(pending, _)| *pending == id)
            .map(|(_, screen)| screen)
            .or(self.current_screen.as_ref());
        let Some(screen) = self
            .controller
            .screens
            .iter()
            .find(|screen| Some(&screen.id) == target)
        else {
            return Space::new().into();
        };
        let left = self
            .controller
            .placement
            .left(screen, self.size().0)
            .round();
        let content: Element<'_, WaylandMessage> = if self.controller.panel.is_expanded {
            row![
                button(Space::new().width(left.max(0.0)).height(Length::Fill))
                    .style(button::text)
                    .on_press(WaylandMessage::Presentation(Message::CollapsePanel)),
                panel_view::view_notch(
                    &self.controller,
                    &self.assistant,
                    Some(iced::Point::ORIGIN),
                    self.assistant.dimensions(&self.controller, Some(screen)),
                    self.settings_window.is_some(),
                )
                .map(WaylandMessage::Presentation),
                button(Space::new().width(Length::Fill).height(Length::Fill))
                    .style(button::text)
                    .on_press(WaylandMessage::Presentation(Message::CollapsePanel)),
            ]
            .into()
        } else {
            row![
                Space::new().width(left),
                panel_view::view_notch(
                    &self.controller,
                    &self.assistant,
                    Some(iced::Point::ORIGIN),
                    self.assistant.dimensions(&self.controller, Some(screen)),
                    self.settings_window.is_some(),
                )
                .map(WaylandMessage::Presentation),
            ]
            .into()
        };
        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_y(iced::alignment::Vertical::Top)
            .into()
    }

    pub fn subscription(&self) -> Subscription<WaylandMessage> {
        Subscription::batch([
            self.assistant
                .subscription()
                .map(|m| WaylandMessage::Presentation(Message::Assistant(m))),
            self.assistant
                .notice_subscription()
                .map(|_| WaylandMessage::Presentation(Message::NoticeExpired)),
            if self.surface.is_some() {
                animation::ticks(&self.controller)
                    .map(|_| WaylandMessage::Presentation(Message::Tick))
            } else {
                Subscription::none()
            },
            self.receiver
                .listen()
                .map(|event| WaylandMessage::Shell(Box::new(event))),
            iced::event::listen_with(|event, _status, id| match event {
                iced::Event::Window(iced::window::Event::Closed) => {
                    Some(WaylandMessage::WindowClosed(id))
                }
                iced::Event::Window(iced::window::Event::Unfocused) => {
                    Some(WaylandMessage::Presentation(Message::CollapsePanel))
                }
                _ => None,
            }),
        ])
    }
}

pub fn run() -> Result<(), iced_exwlshell::Error> {
    let (sender, receiver) = shell::channel();
    daemon(
        move || WaylandApp::new(receiver.clone()),
        "minibro-notch",
        WaylandApp::update,
        WaylandApp::view,
    )
    .wl_settings(ExWlSettings {
        shell_broadcast: sender,
        layer_settings: LayerShellSettings {
            anchor: Anchor::Top | Anchor::Left | Anchor::Right,
            layer: Layer::Top,
            exclusive_zone: 0,
            size: LayerSize::fill_width(NOTCH_COLLAPSED_HEIGHT as u32),
            events_transparent: true,
            keyboard_interactivity: KeyboardInteractivity::None,
            ..Default::default()
        },
        ..Default::default()
    })
    .on_new_shell(|info| Some(WaylandMessage::SurfaceReady(info.window)))
    .style(|_: &WaylandApp, _: &Theme| theme::Style {
        background_color: Color::TRANSPARENT,
        text_color: Color::WHITE,
    })
    .subscription(WaylandApp::subscription)
    .theme(|_: &WaylandApp, _| Some(crate::presentation::theme::app_theme()))
    .run()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> WaylandApp {
        let (_, receiver) = shell::channel();
        let screens: Vec<_> = ["one", "two"]
            .into_iter()
            .map(|id| Screen {
                id: id.into(),
                name: id.into(),
                x: 0.0,
                y: 0.0,
                width: 1600.0,
                height: 900.0,
            })
            .collect();
        let mut controller = NotchController::new();
        controller.screens = screens.clone();
        controller.placement.screen = "one".into();
        WaylandApp {
            controller,
            assistant: Runtime::default(),
            store: PositionStore::default(),
            clock: AnimationClock::default(),
            receiver,
            outputs: screens
                .into_iter()
                .enumerate()
                .map(|(id, screen)| (id as u32, screen))
                .collect(),
            surface: Some(Id::unique()),
            current_screen: Some("one".into()),
            pending: None,
            settings_window: None,
        }
    }

    #[test]
    fn transfer_preserves_avatar_and_panel_until_replacement_is_ready() {
        let mut app = app();
        app.controller.set_mood(crate::domain::Mood::Working);
        app.controller.toggle_panel();
        app.controller.tick(0.05);
        let character = app.controller.character.clone();
        let panel = app.controller.panel.clone();
        let previous = app.surface;
        app.controller.placement.screen = "two".into();
        let _ = app.reconcile();
        let pending = app.pending.as_ref().unwrap().0;
        assert_eq!(app.surface, previous);
        let _ = app.update(WaylandMessage::SurfaceReady(pending));
        assert_eq!(app.surface, Some(pending));
        assert_eq!(app.current_screen.as_deref(), Some("two"));
        assert_eq!(app.controller.character, character);
        assert_eq!(app.controller.panel, panel);
    }

    #[test]
    fn returning_to_current_screen_cancels_pending_transfer() {
        let mut app = app();
        let previous = app.surface;
        app.controller.placement.screen = "two".into();
        let _ = app.reconcile();
        let stale = app.pending.as_ref().unwrap().0;
        app.controller.placement.screen = "one".into();
        let _ = app.reconcile();
        assert!(app.pending.is_none());
        let _ = app.update(WaylandMessage::SurfaceReady(stale));
        assert_eq!(app.surface, previous);
    }

    #[test]
    fn disconnecting_all_screens_preserves_preference_and_can_recover() {
        let mut app = app();
        app.controller.placement.screen = "two".into();
        app.outputs.clear();
        app.controller.screens.clear();
        let _ = app.reconcile();
        assert!(app.surface.is_none());
        assert!(app.pending.is_none());
        assert_eq!(app.controller.placement.screen, "two");
        let connected = Screen {
            id: "one".into(),
            name: "one".into(),
            x: 0.0,
            y: 0.0,
            width: 1600.0,
            height: 900.0,
        };
        app.outputs.insert(1, connected.clone());
        app.controller.screens.push(connected);
        let _ = app.reconcile();
        assert_eq!(
            app.pending.as_ref().map(|(_, target)| target.as_str()),
            Some("one")
        );
        assert_eq!(app.controller.placement.screen, "two");
    }

    #[test]
    fn collapse_panel_collapses_when_expanded() {
        let mut app = app();
        app.controller.toggle_panel();
        assert!(app.controller.panel.is_expanded);
        let _ = app.update(WaylandMessage::Presentation(Message::CollapsePanel));
        assert!(!app.controller.panel.is_expanded);
    }

    #[test]
    fn collapse_panel_does_not_collapse_if_dialog_open() {
        let mut app = app();
        app.controller.toggle_panel();
        app.assistant.dialog_open = true;
        assert!(app.controller.panel.is_expanded);
        let _ = app.update(WaylandMessage::Presentation(Message::CollapsePanel));
        assert!(app.controller.panel.is_expanded);
    }

    #[test]
    fn open_and_close_settings_window() {
        let mut app = app();
        assert!(app.settings_window.is_none());
        let _ = app.update(WaylandMessage::Presentation(Message::OpenSettings));
        assert!(app.settings_window.is_some());
        let _ = app.update(WaylandMessage::Presentation(Message::CloseSettings));
        assert!(app.settings_window.is_none());
    }
}
