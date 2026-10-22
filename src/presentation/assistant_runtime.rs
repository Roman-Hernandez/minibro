//! Shared Iced bridge. Neither runner owns agent-specific state or protocol logic.

use crate::application::assistant::{Assistant, Command, Event};
use crate::infrastructure::opencode::{
    store::{self, Settings},
    worker::{self, Update},
};
use iced::{
    Subscription, Task,
    futures::{SinkExt, StreamExt},
    stream,
    widget::text_editor,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};
use tokio::sync::mpsc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum View {
    #[default]
    Conversation,
    Activity,
    Settings,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quitting_without_an_engine_enters_shutdown() {
        let mut runtime = Runtime::default();
        let _task = runtime.update(Message::Quit);
        assert!(runtime.shutting_down);
    }

    #[test]
    fn cancelled_folder_and_stale_generation_preserve_current_context() {
        let mut runtime = Runtime::default();
        runtime.settings.folder = "/temporary/current".into();
        runtime.assistant.draft = "unsent".into();
        runtime.generation = 2;
        let _task = runtime.update(Message::FolderChosen(None));
        let _task = runtime.update(Message::Engine(
            1,
            Update::Event(Event::Disconnected("old failure".into())),
        ));
        assert_eq!(runtime.settings.folder, PathBuf::from("/temporary/current"));
        assert_eq!(runtime.assistant.draft, "unsent");
        assert_ne!(runtime.assistant.error.as_deref(), Some("old failure"));
    }

    #[test]
    fn an_active_turn_blocks_folder_changes() {
        let mut runtime = Runtime::default();
        runtime.settings.folder = "/temporary/current".into();
        runtime.assistant.snapshot.busy = true;
        let _task = runtime.update(Message::FolderChosen(Some("/temporary/other".into())));
        assert_eq!(runtime.settings.folder, PathBuf::from("/temporary/current"));
    }
}

#[derive(Debug, Clone, Default)]
pub struct AnswerDraft {
    pub choices: Vec<String>,
    pub custom: String,
}

#[derive(Debug, Clone)]
pub enum Message {
    Quit,
    QuitReady,
    Edit(text_editor::Action),
    Send,
    Stop,
    Connect,
    NewConversation,
    ChooseFolder,
    FolderChosen(Option<PathBuf>),
    ChooseExecutable,
    ExecutableChosen(Option<PathBuf>),
    View(View),
    Permission(String, bool),
    Choice(String, usize, String),
    CustomAnswer(String, usize, String),
    Answer(String),
    RejectQuestion(String),
    ToggleActivity(String),
    Ready(u64, mpsc::Sender<Command>),
    Engine(u64, Update),
    Ended(u64),
    Saved(Result<(), String>),
    Scrolled(bool),
}

pub struct Runtime {
    pub shutting_down: bool,
    pub assistant: Assistant,
    pub editor: text_editor::Content,
    pub settings: Settings,
    pub view: View,
    pub answers: BTreeMap<(String, usize), AnswerDraft>,
    pub expanded_activity: BTreeSet<String>,
    pub connecting: bool,
    pub dialog_open: bool,
    pub follow_conversation: bool,
    generation: u64,
    connection: Option<(u64, Settings)>,
    sender: Option<mpsc::Sender<Command>>,
    save_revision: Arc<AtomicU64>,
    save_lock: Arc<Mutex<()>>,
}

impl Default for Runtime {
    fn default() -> Self {
        let loaded = store::default_path().map(|p| store::load(&p)).transpose();
        let (settings, error) = match loaded {
            Ok(Some(Some(settings))) => (Some(settings), None),
            Err(error) => (None, Some(error)),
            _ => (None, None),
        };
        let mut assistant = Assistant::default();
        assistant.error = error;
        Self {
            assistant,
            shutting_down: false,
            editor: text_editor::Content::new(),
            settings: settings.unwrap_or(Settings {
                folder: PathBuf::new(),
                executable: std::env::var("MINIBRO_OPENCODE_BIN").unwrap_or("opencode".into()),
                session: None,
            }),
            view: View::Conversation,
            answers: BTreeMap::new(),
            expanded_activity: BTreeSet::new(),
            connecting: false,
            dialog_open: false,
            follow_conversation: true,
            generation: 0,
            connection: None,
            sender: None,
            save_revision: Arc::default(),
            save_lock: Arc::default(),
        }
    }
}

fn connection_stream(data: &(u64, Settings)) -> iced::futures::stream::BoxStream<'static, Message> {
    let (generation, settings) = data.clone();
    stream::channel(64, async move |mut output| {
        let (sender, receiver) = mpsc::channel(32);
        let (events, mut updates) = mpsc::channel(64);
        if output.send(Message::Ready(generation, sender)).await.is_err() { return; }
        let run = worker::run(settings, receiver, events);
        tokio::pin!(run);
        loop {
            tokio::select! {
                update = updates.recv() => {
                    if let Some(update) = update && output.send(Message::Engine(generation, update)).await.is_err() { return; }
                },
                () = &mut run => {
                    while let Ok(update) = updates.try_recv() { let _ = output.send(Message::Engine(generation, update)).await; }
                    let _ = output.send(Message::Ended(generation)).await;
                    std::future::pending::<()>().await;
                }
            }
        }
    }).boxed()
}

impl Runtime {
    pub fn dimensions(
        &self,
        controller: &crate::application::NotchController,
        screen: Option<&crate::application::positioning::Screen>,
    ) -> (f32, f32) {
        crate::application::assistant::panel_dimensions(
            controller.panel.is_expanded,
            controller.panel.enlarged,
            self.assistant.notice.is_some(),
            screen.map(|s| (s.width, s.height)),
        )
    }

    pub fn notice_subscription(&self) -> Subscription<()> {
        if let Some(expires) = self.assistant.notice.as_ref().and_then(|n| n.expires) {
            return Subscription::run_with(expires, |expires| {
                let expires = *expires;
                iced::futures::stream::once(async move {
                    tokio::time::sleep_until(expires.into()).await;
                })
            });
        }
        Subscription::none()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let connection = self
            .connection
            .clone()
            .map_or_else(Subscription::none, |data| {
                Subscription::run_with(data, connection_stream)
            });
        Subscription::batch([
            connection,
            Subscription::run(|| {
                iced::futures::stream::once(async {
                    #[cfg(unix)]
                    {
                        match tokio::signal::unix::signal(
                            tokio::signal::unix::SignalKind::terminate(),
                        ) {
                            Ok(mut terminate) => {
                                tokio::select! { _ = tokio::signal::ctrl_c() => {}, _ = terminate.recv() => {} }
                            }
                            Err(_) => {
                                let _ = tokio::signal::ctrl_c().await;
                            }
                        }
                    }
                    #[cfg(not(unix))]
                    {
                        let _ = tokio::signal::ctrl_c().await;
                    }
                    Message::Quit
                })
            }),
        ])
    }

    fn command(&mut self, command: Option<Command>) {
        if let Some(command) = command
            && self
                .sender
                .as_ref()
                .is_none_or(|sender| sender.try_send(command).is_err())
        {
            self.assistant.apply(Event::Uncertain(
                "OpenCode command channel unavailable; reconnect to review state".into(),
            ));
        }
    }

    fn persist(&self) -> Task<Message> {
        let Some(path) = store::default_path() else {
            return Task::done(Message::Saved(Err(
                "Application state directory unavailable".into(),
            )));
        };
        let settings = self.settings.clone();
        let revision = self.save_revision.fetch_add(1, Ordering::SeqCst) + 1;
        let latest = self.save_revision.clone();
        let lock = self.save_lock.clone();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    let _guard = lock
                        .lock()
                        .map_err(|_| "Assistant state writer unavailable")?;
                    if latest.load(Ordering::SeqCst) == revision {
                        store::save(&path, &settings)?;
                    }
                    Ok(())
                })
                .await
                .map_err(|e| e.to_string())?
            },
            Message::Saved,
        )
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        let folder_response = matches!(&message, Message::FolderChosen(_));
        let mut effect = Task::none();
        match message {
            Message::Quit if !self.shutting_down => {
                eprintln!("[minibro] Shutting down the owned assistant connection...");
                self.shutting_down = true;
                if let Some(sender) = self.sender.clone() {
                    let _ = sender.try_send(Command::Shutdown);
                    return Task::perform(
                        async move {
                            let _ = tokio::time::timeout(
                                std::time::Duration::from_secs(4),
                                sender.closed(),
                            )
                            .await;
                        },
                        |_| Message::QuitReady,
                    );
                }
                return Task::done(Message::QuitReady);
            }
            Message::Edit(action) => {
                self.editor.perform(action);
                self.assistant.draft = self.editor.text();
            }
            Message::Send => {
                let command = self.assistant.send();
                self.command(command);
            }
            Message::Stop => {
                let command = self.assistant.stop();
                self.command(command);
            }
            Message::Connect => {
                if self.connecting || self.dialog_open {
                    return Task::none();
                }
                if self
                    .sender
                    .as_ref()
                    .is_some_and(|sender| !sender.is_closed())
                {
                    self.command(Some(Command::Reconcile));
                } else if self.settings.folder.is_absolute() && self.settings.folder.is_dir() {
                    self.generation += 1;
                    self.connection = Some((self.generation, self.settings.clone()));
                    self.connecting = true;
                    self.assistant.connected = false;
                    self.assistant.error = None;
                } else {
                    self.assistant.error = Some("Choose an existing working folder first".into());
                    self.view = View::Settings;
                }
            }
            Message::NewConversation
                if !self.connecting
                    && (self.assistant.can_change_context()
                        || self.sender.as_ref().is_none_or(|s| s.is_closed())) =>
            {
                if let Some(sender) = self.sender.take() {
                    let _ = sender.try_send(Command::Shutdown);
                }
                self.connection = None;
                self.settings.session = None;
                self.assistant = Assistant::default();
                self.assistant.draft = self.editor.text();
                return self.update(Message::Connect);
            }
            Message::ChooseFolder | Message::ChooseExecutable
                if self.assistant.can_change_context() && !self.connecting && !self.dialog_open =>
            {
                self.dialog_open = true;
                let folder = matches!(message, Message::ChooseFolder);
                return Task::perform(
                    async move {
                        let dialog = rfd::AsyncFileDialog::new();
                        let file = if folder {
                            dialog.pick_folder().await
                        } else {
                            dialog.pick_file().await
                        };
                        file.map(|f| f.path().to_owned())
                    },
                    move |path| {
                        if folder {
                            Message::FolderChosen(path)
                        } else {
                            Message::ExecutableChosen(path)
                        }
                    },
                );
            }
            Message::FolderChosen(path) | Message::ExecutableChosen(path) => {
                self.dialog_open = false;
                if self.assistant.can_change_context()
                    && let Some(path) = path
                {
                    if folder_response {
                        self.settings.folder = path;
                        self.settings.session = None;
                    } else {
                        self.settings.executable = path.to_string_lossy().into_owned();
                    }
                    if let Some(sender) = self.sender.take() {
                        let _ = sender.try_send(Command::Shutdown);
                    }
                    self.connection = None;
                    self.assistant = Assistant::default();
                    self.assistant.draft = self.editor.text();
                    return self.persist();
                }
            }
            Message::View(view) => self.view = view,
            Message::Scrolled(follow) => self.follow_conversation = follow,
            Message::Permission(id, allow) => {
                let command = self.assistant.permission(&id, allow);
                self.command(command);
            }
            Message::RejectQuestion(id) => {
                let command = self.assistant.reject_question(&id);
                self.command(command);
            }
            Message::Choice(id, index, choice) => {
                if let Some(item) = self
                    .assistant
                    .snapshot
                    .questions
                    .iter()
                    .find(|q| q.id == id)
                    .and_then(|q| q.items.get(index))
                {
                    let answer = self.answers.entry((id, index)).or_default();
                    if !item.multiple {
                        answer.choices.clear();
                    }
                    if answer.choices.contains(&choice) {
                        answer.choices.retain(|c| c != &choice);
                    } else {
                        answer.choices.push(choice);
                    }
                    answer.custom.clear();
                }
            }
            Message::CustomAnswer(id, index, text) => {
                if let Some(item) = self
                    .assistant
                    .snapshot
                    .questions
                    .iter()
                    .find(|q| q.id == id)
                    .and_then(|q| q.items.get(index))
                    .filter(|q| q.custom)
                {
                    let answer = self.answers.entry((id, index)).or_default();
                    if !item.multiple {
                        answer.choices.clear();
                    }
                    answer.custom = text;
                }
            }
            Message::Answer(id) => {
                if let Some(question) = self
                    .assistant
                    .snapshot
                    .questions
                    .iter()
                    .find(|q| q.id == id)
                {
                    let answers = (0..question.items.len())
                        .map(|index| {
                            let answer = self
                                .answers
                                .get(&(id.clone(), index))
                                .cloned()
                                .unwrap_or_default();
                            let mut choices = answer.choices;
                            if !answer.custom.trim().is_empty() {
                                choices.push(answer.custom);
                            }
                            choices
                        })
                        .collect();
                    let command = self.assistant.answer(&id, answers);
                    self.command(command);
                }
            }
            Message::ToggleActivity(id) => {
                if !self.expanded_activity.remove(&id) {
                    self.expanded_activity.insert(id);
                }
            }
            Message::Ready(generation, sender) if generation == self.generation => {
                self.sender = Some(sender);
            }
            Message::Engine(generation, update) if generation == self.generation => match update {
                Update::Event(event) => {
                    let sent = matches!(event, Event::Sent);
                    let snapshot = matches!(event, Event::Snapshot(_));
                    let input_before = self
                        .assistant
                        .snapshot
                        .permissions
                        .iter()
                        .map(|p| &p.id)
                        .chain(self.assistant.snapshot.questions.iter().map(|q| &q.id))
                        .cloned()
                        .collect::<BTreeSet<_>>();
                    if matches!(event, Event::Snapshot(_) | Event::Disconnected(_)) {
                        self.connecting = false;
                    }
                    self.assistant.apply(event);
                    let new_input = self
                        .assistant
                        .snapshot
                        .permissions
                        .iter()
                        .map(|p| &p.id)
                        .chain(self.assistant.snapshot.questions.iter().map(|q| &q.id))
                        .any(|id| !input_before.contains(id));
                    let has_input = !self.assistant.snapshot.permissions.is_empty()
                        || !self.assistant.snapshot.questions.is_empty();
                    if snapshot
                        && self.view == View::Conversation
                        && (new_input || self.follow_conversation && !has_input)
                    {
                        effect = iced::widget::operation::snap_to(
                            iced::widget::Id::new("assistant-thread"),
                            if new_input {
                                iced::widget::scrollable::RelativeOffset::START
                            } else {
                                iced::widget::scrollable::RelativeOffset::END
                            },
                        );
                    }
                    if sent && self.assistant.draft.is_empty() {
                        self.editor = text_editor::Content::new();
                    }
                    self.answers.retain(|(id, _), _| {
                        self.assistant
                            .snapshot
                            .questions
                            .iter()
                            .any(|q| &q.id == id)
                    });
                }
                Update::Session(settings) => {
                    self.settings = settings;
                    return self.persist();
                }
            },
            Message::Ended(generation) if generation == self.generation => {
                self.connecting = false;
                self.sender = None;
                self.assistant.connected = false;
            }
            Message::Saved(Err(error)) => {
                self.assistant.error = Some(format!("Could not save assistant state: {error}"));
            }
            _ => {}
        }
        effect
    }
}
