use crate::domain::Mood;
use std::{
    collections::BTreeSet,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone)]
pub enum Command {
    Send {
        message_id: String,
        text: String,
    },
    Stop,
    Permission {
        id: String,
        allow: bool,
    },
    Answer {
        id: String,
        answers: Vec<Vec<String>>,
    },
    RejectQuestion(String),
    NewConversation,
    Reconcile,
    Shutdown,
}

#[derive(Debug, Clone)]
pub enum Event {
    Snapshot(Snapshot),
    Sent,
    InputReplied(String),
    Uncertain(String),
    Failed(String),
    Disconnected(String),
    Reconciled,
}

#[derive(Debug, Clone)]
pub struct Notice {
    pub text: String,
    pub expires: Option<Instant>,
}

#[derive(Debug, Clone, Default)]
pub struct Assistant {
    pub snapshot: Snapshot,
    pub draft: String,
    pub connected: bool,
    pub sending: bool,
    pub stopping: bool,
    pub uncertain: bool,
    pub error: Option<String>,
    pub notice: Option<Notice>,
    pub in_flight: BTreeSet<String>,
    pending_turn: Option<String>,
}

impl Assistant {
    pub fn busy(&self) -> bool {
        self.snapshot.busy || self.sending || self.stopping || self.pending_turn.is_some()
    }
    pub fn can_change_context(&self) -> bool {
        !self.busy()
            && !self.uncertain
            && self.snapshot.permissions.is_empty()
            && self.snapshot.questions.is_empty()
            && self.in_flight.is_empty()
    }
    pub fn can_send(&self) -> bool {
        self.connected
            && self.can_change_context()
            && !self.draft.trim().is_empty()
            && self.draft.len() <= 65536
    }

    pub fn send(&mut self) -> Option<Command> {
        if !self.can_send() {
            return None;
        }
        let id = format!(
            "msg_minibro{:x}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        );
        self.pending_turn = Some(id.clone());
        self.sending = true;
        self.error = None;
        Some(Command::Send {
            message_id: id,
            text: self.draft.clone(),
        })
    }

    pub fn stop(&mut self) -> Option<Command> {
        if !self.connected || !self.busy() || self.stopping {
            return None;
        }
        self.stopping = true;
        Some(Command::Stop)
    }

    pub fn permission(&mut self, id: &str, allow: bool) -> Option<Command> {
        if !self.connected
            || self.uncertain
            || !self.snapshot.permissions.iter().any(|p| p.id == id)
            || !self.in_flight.insert(id.to_owned())
        {
            return None;
        }
        Some(Command::Permission {
            id: id.into(),
            allow,
        })
    }

    pub fn answer(&mut self, id: &str, answers: Vec<Vec<String>>) -> Option<Command> {
        let question = self.snapshot.questions.iter().find(|q| q.id == id)?;
        if !self.connected
            || self.uncertain
            || answers.len() != question.items.len()
            || answers.iter().zip(&question.items).any(|(a, q)| {
                a.is_empty()
                    || (!q.multiple && a.len() != 1)
                    || a.iter().any(|text| {
                        text.trim().is_empty()
                            || text.len() > 65536
                            || (!q.custom && !q.options.contains(text))
                    })
            })
            || !self.in_flight.insert(id.into())
        {
            return None;
        }
        Some(Command::Answer {
            id: id.into(),
            answers,
        })
    }

    pub fn reject_question(&mut self, id: &str) -> Option<Command> {
        if !self.connected
            || self.uncertain
            || !self.snapshot.questions.iter().any(|q| q.id == id)
            || !self.in_flight.insert(id.into())
        {
            return None;
        }
        Some(Command::RejectQuestion(id.into()))
    }

    pub fn apply(&mut self, event: Event) {
        match event {
            Event::Snapshot(snapshot) => {
                self.connected = true;
                if self.stopping && !snapshot.busy {
                    self.stopping = false;
                    self.pending_turn = None;
                    self.sending = false;
                }
                if snapshot.completed && self.pending_turn.as_ref() == Some(&snapshot.turn_id) {
                    self.pending_turn = None;
                    self.sending = false;
                    self.notice = Some(Notice {
                        text: "Task finished — review the result".into(),
                        expires: Some(Instant::now() + Duration::from_secs(8)),
                    });
                }
                let new_input = snapshot
                    .permissions
                    .iter()
                    .any(|p| !self.snapshot.permissions.iter().any(|old| old.id == p.id))
                    || snapshot
                        .questions
                        .iter()
                        .any(|q| !self.snapshot.questions.iter().any(|old| old.id == q.id));
                if new_input {
                    self.notice = Some(Notice {
                        text: "Your assistant needs a response".into(),
                        expires: None,
                    });
                }
                if snapshot.permissions.is_empty()
                    && snapshot.questions.is_empty()
                    && self.notice.as_ref().is_some_and(|n| n.expires.is_none())
                {
                    self.notice = None;
                }
                self.in_flight.retain(|id| {
                    snapshot.permissions.iter().any(|p| &p.id == id)
                        || snapshot.questions.iter().any(|q| &q.id == id)
                });
                if let Some(error) = &snapshot.error {
                    self.error = Some(error.clone());
                    self.pending_turn = None;
                    self.sending = false;
                    self.notice = Some(Notice {
                        text: "The task reported an error".into(),
                        expires: Some(Instant::now() + Duration::from_secs(8)),
                    });
                }
                self.snapshot = snapshot;
            }
            Event::Sent => {
                self.sending = false;
                self.draft.clear();
            }
            Event::InputReplied(id) => {
                self.in_flight.remove(&id);
                self.snapshot.permissions.retain(|p| p.id != id);
                self.snapshot.questions.retain(|q| q.id != id);
            }
            Event::Reconciled => {
                self.uncertain = false;
                self.error = None;
                self.in_flight.clear();
                if !self.snapshot.busy {
                    self.pending_turn = None;
                    self.sending = false;
                    self.stopping = false;
                }
            }
            Event::Uncertain(error) => {
                self.uncertain = true;
                self.error = Some(error);
            }
            Event::Failed(error) => {
                self.error = Some(error);
            }
            Event::Disconnected(error) => {
                self.connected = false;
                self.error = Some(error);
                self.notice = Some(Notice {
                    text: "OpenCode disconnected — review connection".into(),
                    expires: Some(Instant::now() + Duration::from_secs(8)),
                });
            }
        }
    }

    pub fn mood(&self) -> Mood {
        if self.error.is_some() {
            Mood::Error
        } else if !self.snapshot.permissions.is_empty() || !self.snapshot.questions.is_empty() {
            Mood::Waiting
        } else if self.snapshot.activity.iter().any(|a| a.status == "running") {
            Mood::Working
        } else if self.busy() {
            Mood::Thinking
        } else {
            Mood::Idle
        }
    }

    pub fn status(&self) -> &str {
        if self.error.is_some() {
            "Needs attention"
        } else if !self.snapshot.permissions.is_empty() || !self.snapshot.questions.is_empty() {
            "Waiting for you"
        } else if self.stopping {
            "Stopping…"
        } else if self.busy() {
            "Working…"
        } else if self.connected {
            "Ready"
        } else {
            "OpenCode · disconnected"
        }
    }
}

pub use crate::domain::assistant::{Permission, Question, QuestionItem, Snapshot};

#[cfg(test)]
mod tests {
    use super::*;

    fn ready() -> Assistant {
        let mut assistant = Assistant::default();
        assistant.apply(Event::Snapshot(Snapshot {
            session: "ses_test".into(),
            ..Snapshot::default()
        }));
        assistant
    }

    #[test]
    fn sending_twice_never_duplicates_a_turn_and_keeps_draft_until_accepted() {
        let mut assistant = ready();
        assistant.draft = "create my project".into();
        assert!(assistant.send().is_some());
        assert!(assistant.send().is_none());
        assert_eq!(assistant.draft, "create my project");
        assistant.apply(Event::Sent);
        assert!(assistant.draft.is_empty());
    }

    #[test]
    fn stop_stays_pending_until_engine_confirms_idle() {
        let mut assistant = ready();
        assistant.snapshot.busy = true;
        assert!(assistant.stop().is_some());
        assert!(assistant.stopping);
        assistant.apply(Event::Snapshot(Snapshot {
            session: "ses_test".into(),
            busy: true,
            ..Snapshot::default()
        }));
        assert!(assistant.stopping);
        assistant.apply(Event::Snapshot(Snapshot {
            session: "ses_test".into(),
            ..Snapshot::default()
        }));
        assert!(!assistant.stopping);
    }

    #[test]
    fn duplicate_and_stale_permission_replies_are_blocked() {
        let mut assistant = ready();
        assistant.snapshot.permissions.push(Permission {
            id: "per_one".into(),
            ..Permission::default()
        });
        assert!(assistant.permission("per_one", true).is_some());
        assert!(assistant.permission("per_one", true).is_none());
        assert!(assistant.permission("per_missing", true).is_none());
        assert!(!assistant.can_change_context());
    }

    #[test]
    fn questions_require_all_answers_and_guard_context() {
        let mut assistant = ready();
        assistant.snapshot.questions.push(Question {
            id: "que_one".into(),
            items: vec![QuestionItem {
                text: "Stack?".into(),
                options: vec!["Rust".into()],
                multiple: false,
                custom: false,
            }],
            ..Question::default()
        });
        assert!(assistant.answer("que_one", vec![vec![]]).is_none());
        assert!(
            assistant
                .answer("que_one", vec![vec!["unknown".into()]])
                .is_none()
        );
        assert!(
            assistant
                .answer("que_one", vec![vec!["Rust".into()]])
                .is_some()
        );
        assert!(
            assistant
                .answer("que_one", vec![vec!["Rust".into()]])
                .is_none()
        );
    }

    #[test]
    fn failed_or_ambiguous_request_never_enables_duplicate_send() {
        let mut assistant = ready();
        assistant.draft = "do something".into();
        assistant.send();
        assistant.apply(Event::Uncertain("Response interrupted".into()));
        assert!(assistant.send().is_none());
        assert!(!assistant.draft.is_empty());
    }
}
