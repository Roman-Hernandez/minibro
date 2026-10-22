//! Versioned placement storage, isolated from gesture and platform policy.

use crate::application::positioning::Placement;
use iced::Task;
use std::{
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

pub fn load(path: &Path) -> io::Result<Placement> {
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Placement::default()),
        Err(error) => return Err(error),
    };
    let mut bytes = Vec::new();
    file.take(4097).read_to_end(&mut bytes)?;
    if bytes.len() > 4096 {
        return Err(invalid_state());
    }
    let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|_| invalid_state())?;
    let screen = value["screen"].as_str().ok_or_else(invalid_state)?;
    let horizontal = value["horizontal"].as_f64().ok_or_else(invalid_state)?;
    if value["version"].as_u64() != Some(1)
        || screen.len() > 256
        || !(0.0..=1.0).contains(&horizontal)
    {
        return Err(invalid_state());
    }
    Ok(Placement {
        screen: screen.into(),
        horizontal: horizontal as f32,
    })
}

fn invalid_state() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "invalid or unsupported notch position",
    )
}

fn write(path: &Path, placement: &Placement) -> io::Result<()> {
    if placement.screen.len() > 256 || !(0.0..=1.0).contains(&placement.horizontal) {
        return Err(invalid_state());
    }
    let directory = path.parent().ok_or_else(invalid_state)?;
    std::fs::create_dir_all(directory)?;
    let mut file = tempfile::NamedTempFile::new_in(directory)?;
    serde_json::to_writer(
        &mut file,
        &serde_json::json!({
            "version": 1, "screen": placement.screen, "horizontal": placement.horizontal,
        }),
    )?;
    file.write_all(b"\n")?;
    file.as_file().sync_all()?;
    file.persist(path).map_err(|error| error.error)?;
    Ok(())
}

pub struct PositionStore {
    path: Option<PathBuf>,
    next_revision: AtomicU64,
    written_revision: Arc<Mutex<u64>>,
}

impl Default for PositionStore {
    fn default() -> Self {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute());
        #[cfg(target_os = "macos")]
        let directory = home.map(|p| p.join("Library/Application Support"));
        #[cfg(target_os = "linux")]
        let directory = std::env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| home.map(|p| p.join(".local/state")));
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        let directory = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| home.map(|p| p.join(".local/share")));
        Self {
            path: directory.map(|p| p.join("minibro/position.json")),
            next_revision: AtomicU64::new(0),
            written_revision: Arc::default(),
        }
    }
}

impl PositionStore {
    /// Call before starting the UI event loop.
    pub fn restore(&self) -> Placement {
        self.path.as_ref().map_or_else(Placement::default, |path| {
            load(path).unwrap_or_else(|error| {
                eprintln!("[minibro] Could not restore position: {error}");
                Placement::default()
            })
        })
    }

    pub fn save(&self, placement: Placement) -> Task<Result<(), String>> {
        let Some(path) = self.path.clone() else {
            return Task::done(Err("application state directory unavailable".into()));
        };
        let revision = self.next_revision.fetch_add(1, Ordering::Relaxed) + 1;
        let written_revision = self.written_revision.clone();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    let mut written = written_revision
                        .lock()
                        .map_err(|_| io::Error::other("position writer unavailable"))?;
                    if revision > *written {
                        write(&path, &placement)?;
                        *written = revision;
                    }
                    Ok::<(), io::Error>(())
                })
                .await
                .map_err(|error| error.to_string())?
                .map_err(|error| error.to_string())
            },
            std::convert::identity,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_position_round_trips_and_replaces_previous_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state/position.json");
        let expected = Placement {
            screen: "Display \"one\"".into(),
            horizontal: 0.25,
        };
        write(&path, &expected).unwrap();
        assert_eq!(load(&path).unwrap(), expected);
        write(&path, &Placement::default()).unwrap();
        assert_eq!(load(&path).unwrap(), Placement::default());
        assert_eq!(
            std::fs::read_dir(path.parent().unwrap()).unwrap().count(),
            1
        );
    }

    #[test]
    fn corrupt_or_unsupported_state_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("position.json");
        for bytes in [
            "not json",
            r#"{"version":2,"screen":"one","horizontal":0.5}"#,
            r#"{"version":1,"screen":"one","horizontal":2}"#,
            r#"{"version":1,"screen":"one","horizontal":"NaN"}"#,
        ] {
            std::fs::write(&path, bytes).unwrap();
            assert!(load(&path).is_err(), "accepted: {bytes}");
        }
        std::fs::write(&path, vec![b' '; 4097]).unwrap();
        assert!(load(&path).is_err());
    }

    #[test]
    fn missing_state_uses_centered_default() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            load(&dir.path().join("missing")).unwrap(),
            Placement::default()
        );
    }
}
