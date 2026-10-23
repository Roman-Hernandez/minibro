use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Settings {
    pub folder: PathBuf,
    pub executable: String,
    pub session: Option<String>,
}

pub fn default_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute());
    #[cfg(target_os = "macos")]
    let base = home.map(|p| p.join("Library/Application Support"));
    #[cfg(not(target_os = "macos"))]
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| home.map(|p| p.join(".local/state")));
    base.map(|p| p.join("minibro/assistant.json"))
}

fn valid(settings: &Settings) -> bool {
    settings.folder.is_absolute()
        && settings.folder.as_os_str().len() <= 4096
        && !settings.executable.is_empty()
        && settings.executable.len() <= 4096
        && settings
            .session
            .as_ref()
            .is_none_or(|s| super::protocol::id(&serde_json::Value::String(s.clone())).is_ok())
}

pub fn load(path: &Path) -> Result<Option<Settings>, String> {
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    let mut bytes = Vec::new();
    file.take(16385)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 16384 {
        return Err("Assistant state exceeds 16 KiB".into());
    }
    let value: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|_| "Invalid assistant state")?;
    let settings: Settings = serde_json::from_value(value["settings"].clone())
        .map_err(|_| "Invalid assistant settings")?;
    if value["version"] != 1 || !valid(&settings) {
        return Err("Invalid or unsupported assistant state".into());
    }
    Ok(Some(settings))
}

pub fn save(path: &Path, settings: &Settings) -> Result<(), String> {
    if !valid(settings) {
        return Err("Invalid assistant settings".into());
    }
    let parent = path.parent().ok_or("Missing state directory")?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let mut file = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    serde_json::to_writer(
        &mut file,
        &serde_json::json!({"version":1,"settings":settings}),
    )
    .map_err(|e| e.to_string())?;
    file.write_all(b"\n").map_err(|e| e.to_string())?;
    file.as_file().sync_all().map_err(|e| e.to_string())?;
    file.persist(path).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn settings_are_bounded_atomic_and_do_not_include_transcripts_or_credentials() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("assistant.json");
        assert!(load(&path).unwrap().is_none());
        let settings = Settings {
            folder: dir.path().into(),
            executable: "opencode".into(),
            session: Some("ses_test".into()),
        };
        save(&path, &settings).unwrap();
        assert_eq!(load(&path).unwrap(), Some(settings));
        std::fs::write(&path, vec![b' '; 16385]).unwrap();
        assert!(load(&path).is_err());
        std::fs::write(&path, "{}").unwrap();
        assert!(load(&path).is_err());
    }
}
