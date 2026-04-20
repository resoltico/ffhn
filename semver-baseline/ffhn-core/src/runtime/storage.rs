use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use tempfile::NamedTempFile;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::CoreError;
use crate::stable_json::stable_json;

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
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| CoreError::io(parent, error))?;
    }

    let parent = path
        .parent()
        .ok_or_else(|| CoreError::htmlcut("cannot write file without parent directory"))?;
    let mut temp = NamedTempFile::new_in(parent).map_err(|error| CoreError::io(parent, error))?;
    temp.write_all(bytes)
        .map_err(|error| CoreError::io(&path, error))?;
    temp.persist(&path)
        .map_err(|error| CoreError::io(&path, error.error))?;
    Ok(())
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
