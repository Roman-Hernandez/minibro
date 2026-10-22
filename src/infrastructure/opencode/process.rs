use super::protocol::Client;
use reqwest::Url;
use std::{path::Path, process::Stdio, time::Duration};
use tokio::{
    io::AsyncReadExt,
    process::{Child, Command},
    sync::oneshot,
    task::JoinHandle,
};

pub fn ready_url(line: &str, port: u16) -> Result<Url, String> {
    let raw = line
        .strip_prefix("opencode server listening on ")
        .ok_or("Invalid OpenCode ready line")?;
    let url = Url::parse(raw.trim()).map_err(|_| "Invalid OpenCode ready URL")?;
    if url.scheme() != "http"
        || url.host_str() != Some("127.0.0.1")
        || url.port() != Some(port)
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("OpenCode ready URL is not the requested loopback endpoint".into());
    }
    Ok(url)
}

pub struct Server {
    pub client: Client,
    pub version: String,
    child: Child,
    readers: Vec<JoinHandle<()>>,
}

impl Server {
    pub async fn start(executable: &str, directory: &Path) -> Result<Self, String> {
        if !directory.is_absolute() || !directory.is_dir() {
            return Err("Choose an existing absolute working folder".into());
        }
        let port = std::net::TcpListener::bind(("127.0.0.1", 0))
            .map_err(|_| "Cannot select a local port")?
            .local_addr()
            .map_err(|_| "Cannot read local port")?
            .port();
        // OpenCode 1.19.4 treats port zero as 4096; startup reports a bind conflict if this port races.
        let mut random = [0; 32];
        getrandom::fill(&mut random).map_err(|_| "OS randomness unavailable")?;
        let password: String = random.iter().map(|b| format!("{b:02x}")).collect();
        let mut child = Command::new(executable)
            .args(["serve", "--hostname=127.0.0.1", &format!("--port={port}")])
            .current_dir(directory)
            .env("OPENCODE_SERVER_PASSWORD", &password)
            .env("OPENCODE_SERVER_USERNAME", "opencode")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| {
                format!("Cannot start OpenCode: {e}. Choose its executable in Settings.")
            })?;
        let mut stdout = child.stdout.take().ok_or("OpenCode stdout unavailable")?;
        let mut stderr = child.stderr.take().ok_or("OpenCode stderr unavailable")?;
        let (ready_tx, ready_rx) = oneshot::channel();
        let reader = tokio::spawn(async move {
            let mut sender = Some(ready_tx);
            let mut line = Vec::new();
            let mut total = 0;
            let mut chunk = [0; 4096];
            while let Ok(n) = stdout.read(&mut chunk).await {
                if n == 0 {
                    break;
                }
                for &byte in &chunk[..n] {
                    if sender.is_none() {
                        continue;
                    }
                    total += 1;
                    if total > 65536 {
                        if let Some(sender) = sender.take() {
                            let _ =
                                sender.send(Err("OpenCode startup output exceeds 64 KiB".into()));
                        }
                    } else if byte == b'\n' {
                        let text = String::from_utf8_lossy(&line);
                        if text.starts_with("opencode server listening on ") {
                            if let Some(sender) = sender.take() {
                                let _ = sender.send(ready_url(&text, port));
                            }
                        }
                        line.clear();
                    } else {
                        line.push(byte);
                    }
                }
            }
        });
        let error_reader = tokio::spawn(async move {
            let mut chunk = [0; 4096];
            while let Ok(n) = stderr.read(&mut chunk).await {
                if n == 0 {
                    break;
                }
            }
        });
        let client = Client::new(
            Url::parse(&format!("http://127.0.0.1:{port}")).map_err(|e| e.to_string())?,
            password,
        )?;
        let mut server = Self {
            client,
            version: String::new(),
            child,
            readers: vec![reader, error_reader],
        };
        let startup = tokio::time::timeout(Duration::from_secs(20), async {
            ready_rx
                .await
                .map_err(|_| "OpenCode exited before startup".to_string())??;
            server.client.verify().await
        })
        .await
        .map_err(|_| "OpenCode startup timed out after 20 seconds".to_string())
        .and_then(|r| r);
        match startup {
            Ok(version) => {
                server.version = version;
                Ok(server)
            }
            Err(error) => {
                server.shutdown().await;
                Err(error)
            }
        }
    }

    pub async fn exited(&mut self) -> bool {
        !matches!(self.child.try_wait(), Ok(None))
    }

    pub async fn wait(&mut self) {
        let _ = self.child.wait().await;
    }

    pub async fn shutdown(&mut self) {
        let _ = self.child.start_kill();
        let _ = tokio::time::timeout(Duration::from_secs(3), self.child.wait()).await;
        for reader in &self.readers {
            reader.abort();
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
        for reader in &self.readers {
            reader.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ready_url_rejects_remote_or_credentials() {
        assert!(ready_url("opencode server listening on http://127.0.0.1:1234", 1234).is_ok());
        for url in [
            "http://example.com:1234",
            "http://user@127.0.0.1:1234",
            "http://127.0.0.1:9999",
            "https://127.0.0.1:1234/path",
        ] {
            assert!(ready_url(&format!("opencode server listening on {url}"), 1234).is_err());
        }
    }

    #[tokio::test]
    async fn startup_failure_is_bounded() {
        let dir = tempfile::tempdir().unwrap();
        assert!(
            Server::start("/minibro/missing-executable", dir.path())
                .await
                .is_err()
        );
    }

    #[tokio::test]
    #[ignore = "requires an explicitly selected OpenCode executable and isolated XDG directories"]
    async fn reference_server_contract_and_shutdown() {
        use reqwest::Method;
        let executable = std::env::var("MINIBRO_TEST_OPENCODE").unwrap();
        let dir = tempfile::tempdir().unwrap();
        let mut server = Server::start(&executable, dir.path()).await.unwrap();
        assert!(!server.version.is_empty());
        let session = server
            .client
            .json(
                Method::POST,
                "/session",
                Some(serde_json::json!({"title":"minibro contract test"})),
            )
            .await
            .unwrap();
        let session = super::super::protocol::id(&session["id"]).unwrap();
        for path in [
            "/permission".to_string(),
            "/question".into(),
            format!("/session/{session}/message"),
            format!("/session/{session}/children"),
        ] {
            assert!(
                server
                    .client
                    .json(Method::GET, &path, None)
                    .await
                    .unwrap()
                    .is_array()
            );
        }
        let mut stream = server.client.events().await.unwrap();
        let chunk = tokio::time::timeout(Duration::from_secs(5), stream.chunk())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(String::from_utf8_lossy(&chunk).contains("server.connected"));
        server
            .client
            .json(Method::POST, &format!("/session/{session}/abort"), None)
            .await
            .unwrap();
        server.shutdown().await;
        assert!(server.exited().await);
    }
}
