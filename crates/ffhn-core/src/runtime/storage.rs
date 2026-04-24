use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use tempfile::NamedTempFile;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::CoreError;
use crate::stable_json::stable_json;

#[cfg(test)]
use std::cell::RefCell;
#[cfg(test)]
use std::io;

pub(crate) fn now_utc() -> Result<String, CoreError> {
    Ok(OffsetDateTime::now_utc().format(&Rfc3339)?)
}

pub(crate) fn read_toml<T: DeserializeOwned>(path: &Path) -> Result<T, CoreError> {
    let text = read_text(path)?;
    Ok(toml::from_str(&text)?)
}

#[cfg(test)]
pub(crate) fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T, CoreError> {
    let text = read_text(path)?;
    Ok(serde_json::from_str(&text)?)
}

pub(crate) fn read_text(path: &Path) -> Result<String, CoreError> {
    fs::read_to_string(path).map_err(|error| CoreError::io(path, error))
}

pub(crate) fn write_json(path: PathBuf, value: &impl serde::Serialize) -> Result<(), CoreError> {
    write_text(path, &stable_json(value)?)
}

pub(crate) fn write_text(path: PathBuf, text: &str) -> Result<(), CoreError> {
    let mut line = String::with_capacity(text.len() + 1);
    line.push_str(text);
    line.push('\n');
    write_exact_bytes(path, line.as_bytes())
}

pub(crate) fn write_exact_text(path: PathBuf, text: &str) -> Result<(), CoreError> {
    write_exact_bytes(path, text.as_bytes())
}

fn write_exact_bytes(path: PathBuf, bytes: &[u8]) -> Result<(), CoreError> {
    #[cfg(test)]
    inject_write_error(&path)?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| CoreError::io(parent, error))?;
    }

    let parent = path
        .parent()
        .ok_or_else(|| CoreError::internal("cannot write file without parent directory"))?;
    let mut temp = NamedTempFile::new_in(parent).map_err(|error| CoreError::io(parent, error))?;
    temp.write_all(bytes)
        .map_err(|error| CoreError::io(&path, error))?;
    temp.persist(&path)
        .map_err(|error| CoreError::io(&path, error.error))?;
    Ok(())
}

#[cfg(test)]
thread_local! {
    static WRITE_ERROR_OVERRIDE: RefCell<Option<(String, io::ErrorKind)>> = const { RefCell::new(None) };
}

#[cfg(test)]
fn inject_write_error(path: &Path) -> Result<(), CoreError> {
    WRITE_ERROR_OVERRIDE.with(|override_state| {
        let borrowed = override_state.borrow();
        let Some((file_name, kind)) = borrowed.as_ref() else {
            return Ok(());
        };
        if path.file_name().and_then(|name| name.to_str()) == Some(file_name.as_str()) {
            return Err(CoreError::io(path, io::Error::from(*kind)));
        }
        Ok(())
    })
}

#[cfg(test)]
pub(crate) fn with_write_error_injected<T>(
    file_name: &str,
    kind: io::ErrorKind,
    action: impl FnOnce() -> T,
) -> T {
    WRITE_ERROR_OVERRIDE.with(|override_state| {
        struct ResetWriteOverride<'a> {
            cell: &'a RefCell<Option<(String, io::ErrorKind)>>,
            previous: Option<(String, io::ErrorKind)>,
        }

        impl Drop for ResetWriteOverride<'_> {
            fn drop(&mut self) {
                self.cell.borrow_mut().clone_from(&self.previous);
            }
        }

        let previous = override_state
            .borrow_mut()
            .replace((file_name.to_owned(), kind));
        let guard = ResetWriteOverride {
            cell: override_state,
            previous,
        };
        let result = action();
        drop(guard);
        result
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use tempfile::tempdir;
    use time::{OffsetDateTime, format_description::well_known::Rfc3339};

    #[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
    struct Example {
        value: String,
    }

    #[test]
    fn now_utc_emits_rfc3339_timestamps() {
        let timestamp = now_utc().expect("timestamp");
        OffsetDateTime::parse(&timestamp, &Rfc3339).expect("rfc3339");
    }

    #[test]
    fn write_and_read_text_round_trip_with_trailing_newline() {
        let temp = tempdir().expect("tempdir");
        let path = temp.path().join("nested").join("value.txt");

        write_text(path.clone(), "hello").expect("write text");

        assert_eq!(read_text(&path).expect("read text"), "hello\n");
    }

    #[test]
    fn write_exact_text_round_trips_without_adding_a_trailing_newline() {
        let temp = tempdir().expect("tempdir");
        let path = temp.path().join("nested").join("exact.txt");

        write_exact_text(path.clone(), "hello").expect("write exact text");

        assert_eq!(read_text(&path).expect("read exact text"), "hello");
    }

    #[test]
    fn write_and_read_json_round_trip_with_stable_ordering() {
        let temp = tempdir().expect("tempdir");
        let path = temp.path().join("value.json");
        let example = Example {
            value: "demo".to_owned(),
        };

        write_json(path.clone(), &example).expect("write json");

        assert_eq!(read_json::<Example>(&path).expect("read json"), example);
    }

    #[test]
    fn read_toml_loads_typed_documents() {
        let temp = tempdir().expect("tempdir");
        let path = temp.path().join("value.toml");
        fs::write(&path, "value = \"demo\"\n").expect("write toml");

        assert_eq!(
            read_toml::<Example>(&path).expect("read toml"),
            Example {
                value: "demo".to_owned()
            }
        );
    }

    #[test]
    fn write_text_rejects_paths_without_parent_directory() {
        assert!(write_text(PathBuf::new(), "demo").is_err());
    }
}
