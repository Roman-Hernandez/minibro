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
            snapshot.error = None;
        }
        if role == "assistant" && info["parentID"].as_str() == Some(&snapshot.turn_id) {
            snapshot.completed = info["time"]["completed"].is_number();
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
    }
    let statuses = client.json(Method::GET, "/session/status", None).await?;
    result.busy = owned.0.iter().any(|session| {
        statuses[session]["type"]
            .as_str()
            .is_some_and(|s| s != "idle")
    });
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
        if let Err(error) = result
            && sender.send(StreamEvent::Lost(error)).await.is_err()
        {
            return;
        }
        attempt = (attempt + 1).min(5);
        tokio::time::sleep(Duration::from_millis(250 * (1 << attempt))).await;
    }
}

async fn send(output: &mpsc::Sender<Update>, event: Event) {
    let _ = output.send(Update::Event(event)).await;
}

fn model_label(config: &Value, providers: &Value, agents: &Value) -> Result<String, String> {
    let connected = providers["connected"]
        .as_array()
        .ok_or("Invalid provider configuration")?;
    let model = config["model"]
        .as_str()
        .map(String::from)
        .or_else(|| {
            connected.iter().find_map(|p| {
                let p = p.as_str()?;
                Some(format!("{p}/{}", providers["default"][p].as_str()?))
            })
        })
        .ok_or("No configured OpenCode model. Configure OpenCode first.")?;
    let (provider, model_id) = model
        .split_once('/')
        .ok_or("OpenCode model must include its provider")?;
    if !connected.iter().any(|p| p == provider)
        || !providers["all"].as_array().is_some_and(|all| {
            all.iter()
                .any(|p| p["id"] == provider && p["models"].get(model_id).is_some())
        })
    {
        return Err(
            "Configured OpenCode model/provider unavailable. Configure OpenCode first.".into(),
        );
    }
    let agent = config["default_agent"].as_str().unwrap_or("build");
    if !agents.as_array().is_some_and(|all| {
        all.iter()
            .any(|a| a["name"] == agent && a["mode"] != "subagent")
    }) {
        return Err("Configured OpenCode primary agent unavailable".into());
    }
    Ok(format!("{agent} · {model}"))
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
    let client = server.client.clone();
    let result = tokio::select! {
        result = connected(client, &mut settings, &mut commands, &output) => result,
        _ = server.wait() => Err("The owned OpenCode server exited. Reconnect explicitly.".into()),
    };
    if let Some(session) = &settings.session {
        let _ = tokio::time::timeout(
            Duration::from_secs(2),
            server
                .client
                .json(Method::POST, &format!("/session/{session}/abort"), None),
        )
        .await;
    }
    server.shutdown().await;
    if let Err(error) = result {
        send(&output, Event::Disconnected(error)).await;
    }
}

async fn connected(
    client: Client,
    settings: &mut Settings,
    commands: &mut mpsc::Receiver<Command>,
    output: &mpsc::Sender<Update>,
) -> Result<(), String> {
    let providers = client.json(Method::GET, "/provider", None).await?;
    let config = client.json(Method::GET, "/config", None).await?;
    let agents = client.json(Method::GET, "/agent", None).await?;
    let model = model_label(&config, &providers, &agents)?;
    drop(providers);
    drop(config);
    drop(agents);
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
    let mut reconcile = false;
    let mut stream_online = true;
    let mut pending_turn: Option<String> = None;
    let mut uncertain = false;
    let mut engine_error = None;
    let mut stopping = false;
    let mut refresh = tokio::time::Instant::now();
    loop {
        tokio::select! {
            command = commands.recv() => {
                let Some(command) = command else { return Ok(()); };
                if matches!(command, Command::Shutdown) { return Ok(()); }
                if uncertain && !matches!(command, Command::Reconcile | Command::Stop) {
                    send(output, Event::Uncertain("Review current state before retrying an action".into())).await;
                    continue;
                }
                let mut replied = None;
                let mut sent = false;
                let result = match command {
                    Command::Send { message_id, text } if stream_online && pending_turn.is_none() && !current.busy && current.permissions.is_empty() && current.questions.is_empty() => {
                        sent = true;
                        engine_error = None;
                        let wire_id = protocol::new_message_id()?;
                        pending_turn = Some(wire_id.clone());
                        send(output, Event::TurnId { request: message_id, turn: wire_id.clone() }).await;
                        client.json(Method::POST, &format!("/session/{root}/prompt_async"), Some(json!({"messageID":wire_id,"parts":[{"type":"text","text":text}]}))).await
                    },
                    Command::Stop => { stopping = true; client.json(Method::POST, &format!("/session/{root}/abort"), None).await },
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
                    Command::Reconcile => { reconcile = true; Ok(Value::Null) },
                    _ => Err("Action no longer available; review current state".into()),
                };
                match result {
                    Ok(_) => {
                        if sent { send(output, Event::Sent).await; }
                        if let Some(id) = replied { send(output, Event::InputReplied(id)).await; }
                    },
                    Err(error) => { uncertain = true; send(output, Event::Uncertain(error)).await; },
                }
                dirty = true;
                refresh = tokio::time::Instant::now();
            },
            event = stream_rx.recv() => match event {
                Some(StreamEvent::Connected) => { stream_online = true; descendants(&client, &mut owned).await?; dirty = true; refresh = tokio::time::Instant::now(); },
                Some(StreamEvent::Changed(event)) => {
                    if event["type"] == "session.created" { owned.accept(&event["properties"]["info"]); }
                    let properties = &event["properties"];
                    let session = properties["sessionID"].as_str().or(properties["info"]["sessionID"].as_str()).or(properties["part"]["sessionID"].as_str()).or(properties["info"]["id"].as_str());
                    if session.is_some_and(|s| owned.contains(s)) || event["type"] == "session.created" {
                        if event["type"] == "session.error" { engine_error = Some(bounded_text(&properties["error"].to_string())); }
                        if !dirty { refresh = tokio::time::Instant::now() + Duration::from_millis(150); }
                        dirty = true;
                    }
                },
                Some(StreamEvent::Lost(error)) => { stream_online = false; send(output, Event::Disconnected(error)).await; },
                None => return Err("OpenCode event reader stopped".into()),
            },
            _ = tokio::time::sleep_until(refresh), if dirty && stream_online => {
                current = snapshot(&client, &root, &model, &owned).await?;
                if engine_error.is_some() { current.error = engine_error.clone(); }
                if current.error.is_some() || (current.completed && pending_turn.as_ref() == Some(&current.turn_id)) { pending_turn = None; }
                if stopping && !current.busy { pending_turn = None; stopping = false; }
                send(output, Event::Snapshot(current.clone())).await;
                if reconcile { uncertain = false; if !current.busy { pending_turn = None; } send(output, Event::Reconciled).await; reconcile = false; }
                dirty = false;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn reconnect_and_ambiguous_reply_reconcile_without_post_replay() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        use tokio::{
            io::{AsyncReadExt, AsyncWriteExt},
            net::TcpListener,
        };
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let posts = Arc::new(AtomicUsize::new(0));
        let event_connections = Arc::new(AtomicUsize::new(0));
        let observed = posts.clone();
        let events = event_connections.clone();
        let fake = tokio::spawn(async move {
            let mut tasks = tokio::task::JoinSet::new();
            loop {
                tokio::select! {
                    accepted = listener.accept() => {
                        let (mut socket, _) = accepted.unwrap();
                        let observed = observed.clone();
                        let events = events.clone();
                        tasks.spawn(async move {
                            let mut request = Vec::new();
                            loop {
                                let mut buffer = [0; 4096];
                                let count = socket.read(&mut buffer).await.unwrap();
                                if count == 0 { return; }
                                request.extend_from_slice(&buffer[..count]);
                                if request.windows(4).any(|w| w == b"\r\n\r\n") { break; }
                            }
                            let header = String::from_utf8_lossy(&request);
                            assert!(header.to_ascii_lowercase().contains("authorization: basic "));
                            let first = header.lines().next().unwrap();
                            let path = first.split_whitespace().nth(1).unwrap().split('?').next().unwrap();
                            if path == "/event" {
                                let index = events.fetch_add(1, Ordering::SeqCst);
                                socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\ndata: {\"type\":\"server.connected\"}\n\n").await.unwrap();
                                if index == 0 { return; }
                                socket.write_all(b"data: {\"type\":\"message.part.delta\",\"properties\":{\"sessionID\":\"ses_root\",\"delta\":\"hello\"}}\n\n").await.unwrap();
                                std::future::pending::<()>().await;
                                return;
                            }
                            if path == "/permission/per_child/reply" {
                                observed.fetch_add(1, Ordering::SeqCst);
                                return; // The reply may have executed; deliberately lose its HTTP response.
                            }
                            let value = match path {
                                "/provider" => json!({"connected":["test"],"default":{"test":"model"},"all":[{"id":"test","models":{"model":{}}}]}),
                                "/agent" => json!([{"name":"build","mode":"primary"}]),
                                "/config" => json!({"model":"test/model"}),
                                "/session" => json!({"id":"ses_root"}),
                                "/session/ses_root/children" => json!([{"id":"ses_child","parentID":"ses_root"}]),
                                "/session/ses_child/children" | "/question" | "/session/ses_child/message" => json!([]),
                                "/session/status" => json!({}),
                                "/permission" => json!([
                                    {"id":"per_child","sessionID":"ses_child","permission":"bash","patterns":["pwd"],"metadata":{"command":"pwd"}},
                                    {"id":"per_external","sessionID":"ses_external","permission":"bash","patterns":["external"],"metadata":{}}
                                ]),
                                "/session/ses_root/message" => json!([{"info":{"id":"msg_test","role":"assistant"},"parts":[{"type":"text","text":"hello"}]}]),
                                _ => json!(null),
                            };
                            let bytes = serde_json::to_vec(&value).unwrap();
                            let header = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", bytes.len());
                            let _ = socket.write_all(header.as_bytes()).await;
                            let _ = socket.write_all(&bytes).await;
                        });
                    },
                    _ = tasks.join_next(), if !tasks.is_empty() => {}
                }
            }
        });
        struct Fake(tokio::task::JoinHandle<()>);
        impl Drop for Fake {
            fn drop(&mut self) {
                self.0.abort();
            }
        }
        let _fake = Fake(fake);
        let client = Client::new(
            reqwest::Url::parse(&format!("http://{address}")).unwrap(),
            "test-only".into(),
        )
        .unwrap();
        let (commands, mut receiver) = mpsc::channel(8);
        let (output, mut updates) = mpsc::channel(32);
        let mut settings = Settings {
            folder: "/temporary".into(),
            executable: "unused".into(),
            session: None,
        };
        let run = connected(client, &mut settings, &mut receiver, &output);
        let exercise = async {
            let mut snapshots = 0;
            while snapshots < 2 {
                if let Some(Update::Event(Event::Snapshot(snapshot))) = updates.recv().await {
                    assert_eq!(snapshot.messages.len(), 1);
                    assert_eq!(snapshot.messages[0].text, "hello");
                    assert_eq!(snapshot.permissions.len(), 1);
                    assert_eq!(snapshot.permissions[0].id, "per_child");
                    snapshots += 1;
                }
            }
            commands
                .send(Command::Permission {
                    id: "per_child".into(),
                    allow: true,
                })
                .await
                .unwrap();
            loop {
                if matches!(
                    updates.recv().await,
                    Some(Update::Event(Event::Uncertain(_)))
                ) {
                    break;
                }
            }
            commands.send(Command::Reconcile).await.unwrap();
            loop {
                if matches!(updates.recv().await, Some(Update::Event(Event::Reconciled))) {
                    break;
                }
            }
            assert_eq!(posts.load(Ordering::SeqCst), 1);
            assert!(event_connections.load(Ordering::SeqCst) >= 2);
            commands.send(Command::Shutdown).await.unwrap();
        };
        tokio::time::timeout(Duration::from_secs(8), async {
            let (result, ()) = tokio::join!(run, exercise);
            result.unwrap();
        })
        .await
        .unwrap();
    }

    #[tokio::test]
    #[ignore = "real free reference model; requires isolated XDG directories, network, and MINIBRO_TEST_OPENCODE"]
    async fn real_engine_creates_only_the_approved_temporary_project() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(
            directory.path().join("opencode.json"),
            r#"{"model":"opencode/big-pickle","permission":{"bash":"ask"}}"#,
        )
        .unwrap();
        let settings = Settings {
            folder: directory.path().into(),
            executable: std::env::var("MINIBRO_TEST_OPENCODE").unwrap(),
            session: None,
        };
        let (commands, receiver) = mpsc::channel(16);
        let (events, mut updates) = mpsc::channel(64);
        let engine = run(settings, receiver, events);
        let exercise = async {
            let mut phase = 0;
            let mut pending = BTreeSet::new();
            let mut allowed = false;
            let mut rejected = false;
            let mut answered = false;
            let mut turn_id = String::new();
            while let Some(update) = updates.recv().await {
                if let Update::Event(event) = update {
                    match event {
                        Event::Disconnected(error) => {
                            panic!("Reference engine disconnected: {error}")
                        }
                        Event::Uncertain(error) => panic!("Reference action failed: {error}"),
                        Event::TurnId { turn, .. } => turn_id = turn,
                        Event::Snapshot(snapshot) => {
                            assert!(
                                snapshot.error.is_none(),
                                "Reference task error: {:?}",
                                snapshot.error
                            );
                            if phase == 0 {
                                commands.send(Command::Send { message_id: "msg_minibro_probe_one".into(), text: "Use bash exactly once to run `mkdir minibro-probe` in the current working directory. Do not run anything else or use other tools. Then say DONE.".into() }).await.unwrap();
                                phase = 1;
                            }
                            for permission in snapshot.permissions {
                                if !pending.insert(permission.id.clone()) {
                                    continue;
                                }
                                let details: Value =
                                    serde_json::from_str(&permission.details).unwrap();
                                let command = details["command"].as_str().unwrap_or("");
                                if phase == 1
                                    && matches!(
                                        command,
                                        "mkdir minibro-probe" | "mkdir -p minibro-probe"
                                    )
                                {
                                    allowed = true;
                                    commands
                                        .send(Command::Permission {
                                            id: permission.id,
                                            allow: true,
                                        })
                                        .await
                                        .unwrap();
                                } else {
                                    rejected = true;
                                    commands
                                        .send(Command::Permission {
                                            id: permission.id,
                                            allow: false,
                                        })
                                        .await
                                        .unwrap();
                                }
                            }
                            for question in snapshot.questions {
                                if pending.insert(question.id.clone()) {
                                    let answers = question
                                        .items
                                        .iter()
                                        .map(|item| {
                                            vec![
                                                item.options
                                                    .first()
                                                    .cloned()
                                                    .unwrap_or("VS Code".into()),
                                            ]
                                        })
                                        .collect();
                                    commands
                                        .send(Command::Answer {
                                            id: question.id,
                                            answers,
                                        })
                                        .await
                                        .unwrap();
                                    answered = true;
                                }
                            }
                            if phase == 1 && snapshot.turn_id == turn_id && snapshot.completed {
                                assert!(allowed && directory.path().join("minibro-probe").is_dir());
                                commands.send(Command::Send { message_id: "msg_minibro_probe_two".into(), text: "Use bash exactly once to run `mkdir rejected-probe`. Do not retry if rejected, and do not use other tools. Then finish.".into() }).await.unwrap();
                                phase = 2;
                            } else if phase == 2
                                && snapshot.turn_id == turn_id
                                && snapshot.completed
                            {
                                assert!(
                                    rejected && !directory.path().join("rejected-probe").exists()
                                );
                                commands.send(Command::Send { message_id: "msg_minibro_probe_three".into(), text: "Use the question tool to ask me to choose an editor, with options VS Code and Vim. Wait for my answer, then acknowledge it. Do not run commands or any other tool.".into() }).await.unwrap();
                                phase = 3;
                            } else if phase == 3
                                && snapshot.turn_id == turn_id
                                && snapshot.completed
                            {
                                assert!(answered);
                                commands.send(Command::Shutdown).await.unwrap();
                                return;
                            }
                        }
                        _ => {}
                    }
                }
            }
            panic!("Engine closed before acceptance finished");
        };
        tokio::time::timeout(Duration::from_secs(180), async {
            tokio::join!(engine, exercise);
        })
        .await
        .unwrap();
    }

    #[test]
    fn owned_descendants_exclude_unrelated_roots() {
        let mut owned = Owned::new("ses_root".into());
        assert!(!owned.accept(&json!({"id":"ses_other"})));
        assert!(!owned.accept(&json!({"id":"ses_child","parentID":"ses_other"})));
        assert!(owned.accept(&json!({"id":"ses_child","parentID":"ses_root"})));
        assert!(owned.contains("ses_child"));
    }

    #[test]
    fn model_setup_rejects_unavailable_providers_and_models() {
        let providers = json!({"connected":["test"],"default":{"test":"one"},"all":[{"id":"test","models":{"one":{}}}]});
        assert!(
            model_label(
                &json!({}),
                &providers,
                &json!([{"name":"build","mode":"primary"}])
            )
            .unwrap()
            .contains("test/one")
        );
        assert!(model_label(&json!({"model":"other/one"}), &providers, &json!([])).is_err());
        assert!(model_label(&json!({"model":"test/missing"}), &providers, &json!([])).is_err());
    }

    #[test]
    fn completed_tool_call_can_be_terminal_when_engine_reports_idle() {
        let mut snapshot = Snapshot::default();
        append_messages(&mut snapshot, "ses_root", &json!([
            {"info":{"id":"msg_one","role":"user"},"parts":[]},
            {"info":{"id":"msg_two","role":"assistant","parentID":"msg_one","finish":"tool-calls","time":{"completed":1}},"parts":[{"id":"prt_one","type":"tool","tool":"bash","state":{"status":"error","error":"Permission rejected"}}]}
        ])).unwrap();
        assert!(
            snapshot.completed,
            "An idle engine may end at a rejected tool call without a final text message"
        );
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
