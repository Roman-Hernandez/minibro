//! Wire mapping for the verified OpenCode HTTP API.

use reqwest::{Method, Url};
use serde_json::Value;
use std::time::Duration;

#[derive(Clone)]
pub struct Client {
    http: reqwest::Client,
    base: Url,
    password: String,
}

impl Client {
    pub fn new(base: Url, password: String) -> Result<Self, String> {
        Ok(Self {
            http: reqwest::Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .connect_timeout(Duration::from_secs(10))
                .build()
                .map_err(|e| e.to_string())?,
            base,
            password,
        })
    }

    fn request(&self, method: Method, path: &str) -> Result<reqwest::RequestBuilder, String> {
        if !path.starts_with('/') || path.contains("..") || path.contains(['?', '#', '\\']) {
            return Err("Invalid OpenCode API path".into());
        }
        let mut url = self.base.clone();
        url.set_path(path);
        Ok(self
            .http
            .request(method, url)
            .basic_auth("opencode", Some(&self.password)))
    }

    pub async fn json(
        &self,
        method: Method,
        path: &str,
        body: Option<Value>,
    ) -> Result<Value, String> {
        let mut request = self.request(method, path)?.timeout(Duration::from_secs(15));
        if let Some(body) = body {
            request = request.json(&body);
        }
        self.read_json(request).await
    }

    pub async fn history(&self, session: &str) -> Result<Value, String> {
        self.read_json(
            self.request(Method::GET, &format!("/session/{session}/message"))?
                .query(&[("limit", 256)])
                .timeout(Duration::from_secs(15)),
        )
        .await
    }

    async fn read_json(&self, request: reqwest::RequestBuilder) -> Result<Value, String> {
        let response = request
            .send()
            .await
            .map_err(|_| "OpenCode request failed; reconcile before retrying".to_string())?;
        let mut response = response.error_for_status().map_err(|e| {
            format!(
                "OpenCode HTTP {}",
                e.status().map(|s| s.as_u16()).unwrap_or(0)
            )
        })?;
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| "OpenCode response interrupted")?
        {
            if bytes.len() + chunk.len() > 16 * 1024 * 1024 {
                return Err("OpenCode response exceeds 16 MiB".into());
            }
            bytes.extend_from_slice(&chunk);
        }
        if bytes.is_empty() {
            return Ok(Value::Null);
        }
        serde_json::from_slice(&bytes).map_err(|_| "Invalid OpenCode JSON response".into())
    }

    pub async fn events(&self) -> Result<reqwest::Response, String> {
        self.request(Method::GET, "/event")?
            .send()
            .await
            .map_err(|_| "OpenCode event connection failed".to_string())?
            .error_for_status()
            .map_err(|_| "OpenCode event connection rejected".to_string())
    }

    pub async fn verify(&self) -> Result<String, String> {
        let health = self.json(Method::GET, "/global/health", None).await?;
        if health["healthy"] != true {
            return Err("OpenCode is not healthy".into());
        }
        let doc = self.json(Method::GET, "/doc", None).await?;
        for route in [
            "/event",
            "/session",
            "/permission",
            "/question",
            "/session/{sessionID}/prompt_async",
            "/session/{sessionID}/abort",
            "/permission/{requestID}/reply",
            "/question/{requestID}/reply",
        ] {
            if doc["paths"][route].is_null() {
                return Err(format!("Incompatible OpenCode API: {route}"));
            }
        }
        Ok(health["version"].as_str().unwrap_or("unknown").to_owned())
    }
}

pub fn id(value: &Value) -> Result<String, String> {
    let id = value.as_str().ok_or("Missing OpenCode identifier")?;
    if id.is_empty()
        || id.len() > 128
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
    {
        return Err("Invalid OpenCode identifier".into());
    }
    Ok(id.into())
}

pub fn new_message_id() -> Result<String, String> {
    use std::{
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };
    static LAST: AtomicU64 = AtomicU64::new(0);
    let time = (SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "System clock is before epoch")?
        .as_millis() as u64)
        << 12;
    let mut previous = LAST.load(Ordering::SeqCst);
    let encoded = loop {
        let next = (time + 1).max(previous + 1);
        match LAST.compare_exchange_weak(previous, next, Ordering::SeqCst, Ordering::SeqCst) {
            Ok(_) => break next & 0xffffffffffff,
            Err(current) => previous = current,
        }
    };
    let mut random = [0; 14];
    getrandom::fill(&mut random).map_err(|_| "OS randomness unavailable")?;
    let alphabet = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
    let suffix: String = random
        .iter()
        .map(|byte| alphabet[*byte as usize % alphabet.len()] as char)
        .collect();
    // OpenCode's message cursor depends on its six-byte ascending timestamp, not just a msg_ prefix.
    Ok(format!("msg_{encoded:012x}{suffix}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn message_ids_match_the_engine_time_ordered_format() {
        let first = new_message_id().unwrap();
        let second = new_message_id().unwrap();
        assert_eq!(first.len(), 30);
        assert!(first.starts_with("msg_"));
        assert!(first[4..16].bytes().all(|b| b.is_ascii_hexdigit()));
        assert!(first[16..].bytes().all(|b| b.is_ascii_alphanumeric()));
        assert!(first[..16] < second[..16]);
    }
}
