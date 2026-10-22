use super::{
    process::Server,
    protocol::{self, Client},
    sse::SseDecoder,
    store::Settings,
};
use crate::{
    application::assistant::{Command, Event},
    domain::assistant::*,
};
use reqwest::Method;
use serde_json::{Value, json};
use std::{collections::BTreeSet, time::Duration};
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub enum Update {
    Event(Event),
    Session(Settings),
}

pub struct Owned(BTreeSet<String>);

impl Owned {
    pub fn new(root: String) -> Self {
        Self(BTreeSet::from([root]))
    }
    pub fn contains(&self, id: &str) -> bool {
        self.0.contains(id)
    }
    pub fn accept(&mut self, session: &Value) -> bool {
        if self.0.len() >= 64
            || !session["parentID"]
                .as_str()
                .is_some_and(|p| self.contains(p))
        {
            return false;
        }
        protocol::id(&session["id"]).is_ok_and(|id| self.0.insert(id))
    }
}

pub fn bounded_text(text: &str) -> String {
    if text.len() <= 65536 {
        return text.into();
    }
    let mut end = 65536;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}\n[output truncated]", &text[..end])
}

pub fn append_messages(
    snapshot: &mut Snapshot,
    session: &str,
    messages: &Value,
) -> Result<(), String> {
    let messages = messages.as_array().ok_or("Invalid OpenCode message list")?;
    for message in messages
        .iter()
        .rev()
        .take(256)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
    {
        let info = &message["info"];
        let message_id = protocol::id(&info["id"])?;
        let role = info["role"]
            .as_str()
            .ok_or("Missing message role")?
            .to_owned();
        if role == "user" {
            snapshot.turn_id = message_id.clone();
            snapshot.completed = false;
        }
        if role == "assistant"
            && info["parentID"].as_str() == Some(&snapshot.turn_id)
            && info["time"]["completed"].is_number()
            && info["finish"].as_str() != Some("tool-calls")
        {
            snapshot.completed = true;
        }
        if !info["error"].is_null() {
            snapshot.error = Some(bounded_text(&info["error"].to_string()));
        }
        let mut text = String::new();
        for part in message["parts"].as_array().ok_or("Missing message parts")? {
            if part["type"] == "text" {
                if let Some(content) = part["text"].as_str() {
                    text.push_str(content);
                    text.push('\n');
                }
            } else if part["type"] == "tool" {
                snapshot.activity.push(ToolActivity {
                    id: protocol::id(&part["id"])?,
                    session: session.into(),
                    title: part["tool"].as_str().unwrap_or("Tool").into(),
                    input: bounded_text(&part["state"]["input"].to_string()),
                    output: bounded_text(
                        part["state"]["output"]
                            .as_str()
                            .or(part["state"]["error"].as_str())
                            .unwrap_or(""),
                    ),
                    status: part["state"]["status"].as_str().unwrap_or("pending").into(),
                });
            }
        }
        snapshot.messages.push(ChatMessage {
            id: message_id,
            role,
            text: bounded_text(text.trim_end()),
        });
    }
    Ok(())
}

async fn descendants(client: &Client, owned: &mut Owned) -> Result<(), String> {
    let mut checked = BTreeSet::new();
    while let Some(parent) = owned.0.iter().find(|s| !checked.contains(*s)).cloned() {
        checked.insert(parent.clone());
        let children = client
            .json(Method::GET, &format!("/session/{parent}/children"), None)
            .await?;
        for child in children.as_array().ok_or("Invalid child-session list")? {
            if child["parentID"].as_str() == Some(&parent) {
                owned.accept(child);
            }
        }
    }
    Ok(())
}

async fn snapshot(
    client: &Client,
    root: &str,
    model: &str,
    owned: &Owned,
) -> Result<Snapshot, String> {
    let statuses = client.json(Method::GET, "/session/status", None).await?;
    let mut result = Snapshot {
        session: root.into(),
        model: model.into(),
        ..Snapshot::default()
    };
    for session in owned
        .0
        .iter()
        .filter(|s| s.as_str() != root)
        .chain(std::iter::once(&root.to_owned()))
    {
        let history = client.history(session).await?;
        append_messages(&mut result, session, &history)?;
        if statuses[session]["type"]
            .as_str()
            .is_some_and(|s| s != "idle")
        {
            result.busy = true;
        }
    }
    if result.busy {
        result.completed = false;
    }
    let permissions = client.json(Method::GET, "/permission", None).await?;
    for permission in permissions.as_array().ok_or("Invalid permission list")? {
        let session = protocol::id(&permission["sessionID"])?;
        if !owned.contains(&session) {
            continue;
        }
        result.permissions.push(Permission {
            id: protocol::id(&permission["id"])?,
            session,
            operation: permission["permission"]
                .as_str()
                .unwrap_or("Requested operation")
                .into(),
            patterns: permission["patterns"]
                .as_array()
                .map(|p| {
                    p.iter()
                        .filter_map(|p| p.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default(),
            details: bounded_text(&permission["metadata"].to_string()),
        });
    }
    let questions = client.json(Method::GET, "/question", None).await?;
    for question in questions.as_array().ok_or("Invalid question list")? {
        let session = protocol::id(&question["sessionID"])?;
        if !owned.contains(&session) {
            continue;
        }
        let items = question["questions"]
            .as_array()
            .ok_or("Missing agent questions")?
            .iter()
            .map(|q| QuestionItem {
                text: q["question"].as_str().unwrap_or("Question").into(),
                options: q["options"]
                    .as_array()
                    .map(|options| {
                        options
                            .iter()
                            .filter_map(|o| o["label"].as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default(),
                multiple: q["multiple"].as_bool().unwrap_or(false),
                custom: q["custom"].as_bool().unwrap_or(true),
            })
            .collect();
        result.questions.push(Question {
            id: protocol::id(&question["id"])?,
            session,
            items,
        });
    }
    if result.messages.len() > 256 {
        result.messages.drain(..result.messages.len() - 256);
    }
    if result.activity.len() > 256 {
        result.activity.drain(..result.activity.len() - 256);
    }
    Ok(result)
}

enum StreamEvent {
    Connected,
    Changed(Value),
    Lost(String),
}

async fn read_events(client: Client, sender: mpsc::Sender<StreamEvent>) {
    let mut attempt = 0;
    loop {
        let result: Result<(), String> = async {
            let mut stream = client.events().await?;
            sender
                .send(StreamEvent::Connected)
                .await
                .map_err(|_| "Reader closed")?;
            attempt = 0;
            let mut decoder = SseDecoder::default();
            loop {
                let bytes = stream
                    .chunk()
                    .await
                    .map_err(|_| "OpenCode event stream interrupted")?
                    .ok_or("OpenCode event stream ended")?;
                for frame in decoder.push(&bytes)? {
                    let value: Value =
                        serde_json::from_str(&frame).map_err(|_| "Malformed OpenCode event")?;
                    if !matches!(
                        value["type"].as_str(),
                        Some("server.heartbeat" | "server.connected")
                    ) {
                        sender
                            .send(StreamEvent::Changed(value))
                            .await
                            .map_err(|_| "Reader closed")?;
                    }
                }
            }
        }
        .await;
        if sender.is_closed() {
            return;
        }
        if let Err(error) = result {
            if sender.send(StreamEvent::Lost(error)).await.is_err() {
                return;
            }
        }
        attempt = (attempt + 1).min(5);
        tokio::time::sleep(Duration::from_millis(250 * (1 << attempt))).await;
    }
}

async fn send(output: &mpsc::Sender<Update>, event: Event) {
    let _ = output.send(Update::Event(event)).await;
}

pub async fn run(
    mut settings: Settings,
    mut commands: mpsc::Receiver<Command>,
    output: mpsc::Sender<Update>,
) {
    let mut server = match Server::start(&settings.executable, &settings.folder).await {
        Ok(server) => server,
        Err(error) => {
            send(&output, Event::Disconnected(error)).await;
            return;
        }
    };
    let result = connected(&mut server, &mut settings, &mut commands, &output).await;
    if let Some(session) = &settings.session {
        let _ = server
            .client
            .json(Method::POST, &format!("/session/{session}/abort"), None)
            .await;
    }
    server.shutdown().await;
    if let Err(error) = result {
        send(&output, Event::Disconnected(error)).await;
    }
}

async fn connected(
    server: &mut Server,
    settings: &mut Settings,
    commands: &mut mpsc::Receiver<Command>,
    output: &mpsc::Sender<Update>,
) -> Result<(), String> {
    let client = server.client.clone();
    let providers = client.json(Method::GET, "/provider", None).await?;
    let config = client.json(Method::GET, "/config", None).await?;
    let model = config["model"]
        .as_str()
        .map(String::from)
        .or_else(|| {
            providers["connected"].as_array()?.iter().find_map(|p| {
                let p = p.as_str()?;
                Some(format!("{p}/{}", providers["default"][p].as_str()?))
            })
        })
        .ok_or("No configured OpenCode model. Configure a provider/model in OpenCode first.")?;
    if providers["connected"]
        .as_array()
        .is_none_or(|p| p.is_empty())
    {
        return Err("No connected OpenCode provider. Configure OpenCode first.".into());
    }
    let (stream_tx, mut stream_rx) = mpsc::channel(64);
    let reader = tokio::spawn(read_events(client.clone(), stream_tx));
    // Abort the reader when the owning server task is cancelled, including a subscription change.
    struct Reader(tokio::task::JoinHandle<()>);
    impl Drop for Reader {
        fn drop(&mut self) {
            self.0.abort();
        }
    }
    let _reader = Reader(reader);
    match stream_rx.recv().await {
        Some(StreamEvent::Connected) => {}
        _ => return Err("OpenCode events unavailable".into()),
    }
    let root = if let Some(root) = &settings.session {
        let session = client
            .json(Method::GET, &format!("/session/{root}"), None)
            .await
            .map_err(
                |_| "Previous conversation unavailable. Start a new conversation explicitly.",
            )?;
        if session["directory"].as_str().map(std::path::Path::new)
            != Some(settings.folder.as_path())
            || !session["parentID"].is_null()
        {
            return Err("Saved conversation does not belong to the chosen folder/root".into());
        }
        root.clone()
    } else {
        protocol::id(
            &client
                .json(Method::POST, "/session", Some(json!({"title":"minibro"})))
                .await?["id"],
        )?
    };
    settings.session = Some(root.clone());
    output
        .send(Update::Session(settings.clone()))
        .await
        .map_err(|_| "Assistant closed")?;
    let mut owned = Owned::new(root.clone());
    descendants(&client, &mut owned).await?;
    let mut current = snapshot(&client, &root, &model, &owned).await?;
    send(output, Event::Snapshot(current.clone())).await;
    let mut dirty = false;
    let mut refresh = tokio::time::Instant::now();
    loop {
        tokio::select! {
            _ = server.wait() => return Err("The owned OpenCode server exited. Reconnect explicitly.".into()),
            command = commands.recv() => {
                let Some(command) = command else { return Ok(()); };
                if matches!(command, Command::Shutdown) { return Ok(()); }
                let mut replied = None;
                let mut sent = false;
                let result = match command {
                    Command::Send { message_id, text } if !current.busy && current.permissions.is_empty() && current.questions.is_empty() => {
                        sent = true;
                        client.json(Method::POST, &format!("/session/{root}/prompt_async"), Some(json!({"messageID":message_id,"parts":[{"type":"text","text":text}]}))).await
                    },
                    Command::Stop => client.json(Method::POST, &format!("/session/{root}/abort"), None).await,
                    Command::Permission { id, allow } if current.permissions.iter().any(|p| p.id == id && owned.contains(&p.session)) => {
                        replied = Some(id.clone());
                        client.json(Method::POST, &format!("/permission/{id}/reply"), Some(json!({"reply":if allow {"once"} else {"reject"}}))).await
                    },
                    Command::Answer { id, answers } if current.questions.iter().any(|q| q.id == id && owned.contains(&q.session)) => {
                        replied = Some(id.clone());
                        client.json(Method::POST, &format!("/question/{id}/reply"), Some(json!({"answers":answers}))).await
                    },
                    Command::RejectQuestion(id) if current.questions.iter().any(|q| q.id == id && owned.contains(&q.session)) => {
                        replied = Some(id.clone());
                        client.json(Method::POST, &format!("/question/{id}/reject"), None).await
                    },
                    Command::Reconcile => Ok(Value::Null),
                    _ => Err("Action no longer available; review current state".into()),
                };
                match result {
                    Ok(_) => { if sent { send(output, Event::Sent).await; } if let Some(id) = replied { send(output, Event::InputReplied(id)).await; } },
                    Err(error) => send(output, Event::Uncertain(error)).await,
                }
                dirty = true;
                refresh = tokio::time::Instant::now();
            },
            event = stream_rx.recv() => match event {
                Some(StreamEvent::Connected) => { descendants(&client, &mut owned).await?; dirty = true; refresh = tokio::time::Instant::now(); },
                Some(StreamEvent::Changed(event)) => {
                    if event["type"] == "session.created" { owned.accept(&event["properties"]["info"]); }
                    let properties = &event["properties"];
                    let session = properties["sessionID"].as_str().or(properties["info"]["sessionID"].as_str()).or(properties["part"]["sessionID"].as_str()).or(properties["info"]["id"].as_str());
                    if session.is_some_and(|s| owned.contains(s)) || event["type"] == "session.created" {
                        if !dirty { refresh = tokio::time::Instant::now() + Duration::from_millis(150); }
                        dirty = true;
                    }
                },
                Some(StreamEvent::Lost(error)) => send(output, Event::Disconnected(error)).await,
                None => return Err("OpenCode event reader stopped".into()),
            },
            _ = tokio::time::sleep_until(refresh), if dirty => {
                current = snapshot(&client, &root, &model, &owned).await?;
                send(output, Event::Snapshot(current.clone())).await;
                dirty = false;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn owned_descendants_exclude_unrelated_roots() {
        let mut owned = Owned::new("ses_root".into());
        assert!(!owned.accept(&json!({"id":"ses_other"})));
        assert!(!owned.accept(&json!({"id":"ses_child","parentID":"ses_other"})));
        assert!(owned.accept(&json!({"id":"ses_child","parentID":"ses_root"})));
        assert!(owned.contains("ses_child"));
    }

    #[test]
    fn snapshot_is_authoritative_and_completion_matches_the_last_user_message() {
        let messages = json!([
            {"info":{"id":"msg_one","role":"user"},"parts":[{"type":"text","text":"Hello"}]},
            {"info":{"id":"msg_two","role":"assistant","parentID":"msg_one","time":{"completed":1}},"parts":[{"type":"text","text":"Hi"},{"id":"prt_tool","type":"tool","callID":"call_one","tool":"bash","state":{"status":"completed","input":{"command":"pwd"},"output":"/temporary"}}]}
        ]);
        let mut snapshot = Snapshot::default();
        append_messages(&mut snapshot, "ses_root", &messages).unwrap();
        assert_eq!(snapshot.messages.len(), 2);
        assert_eq!(snapshot.turn_id, "msg_one");
        assert!(snapshot.completed);
        assert_eq!(snapshot.activity[0].status, "completed");
        let mut second = Snapshot::default();
        append_messages(&mut second, "ses_root", &messages).unwrap();
        assert_eq!(snapshot, second);
        assert!(bounded_text(&"a".repeat(70000)).ends_with("[output truncated]"));
    }
}
