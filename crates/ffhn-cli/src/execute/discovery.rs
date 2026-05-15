use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use ffhn_core::{CoreError, TargetId};

use crate::args::RunCommand;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SelectedTargets {
    Explicit(Vec<TargetId>),
    Discovered(Vec<DiscoveredTarget>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DiscoveredTarget {
    pub(crate) requested_id: String,
    pub(crate) validated_id: Option<TargetId>,
    pub(crate) validation_message: Option<String>,
}

pub(super) fn selected_targets(command: &RunCommand) -> Result<SelectedTargets, CoreError> {
    if !command.all {
        return Ok(SelectedTargets::Explicit(command.targets.clone()));
    }

    Ok(SelectedTargets::Discovered(discover_watch_root_targets(
        &command.watch_root,
    )?))
}

pub(crate) fn discover_watch_root_targets(
    watch_root: &Path,
) -> Result<Vec<DiscoveredTarget>, CoreError> {
    let mut targets = Vec::new();
    if !watch_root.exists() {
        return Err(CoreError::io(
            watch_root,
            io::Error::new(io::ErrorKind::NotFound, "watch root does not exist"),
        ));
    }
    if !watch_root.is_dir() {
        return Err(CoreError::io(
            watch_root,
            io::Error::other("watch root is not a directory"),
        ));
    }

    let read_dir = fs::read_dir(watch_root)
        .map_err(|error| CoreError::io(watch_root, error))?
        .map(|entry| entry.map(|entry| entry.path()));
    let mut entries = collect_watch_root_directories(watch_root, read_dir)?;
    entries.sort();

    for target_id in entries.into_iter().filter_map(|path| {
        if !path.join("target.toml").exists() {
            return None;
        }
        path.file_name()
            .and_then(|name| name.to_str())
            .map(str::to_owned)
    }) {
        match TargetId::new(target_id.clone()) {
            Ok(validated_id) => {
                targets.push(DiscoveredTarget {
                    requested_id: target_id,
                    validated_id: Some(validated_id),
                    validation_message: None,
                });
            }
            Err(error) => {
                targets.push(DiscoveredTarget {
                    requested_id: target_id,
                    validated_id: None,
                    validation_message: Some(contract_message(error)),
                });
            }
        }
    }

    Ok(targets)
}

pub(crate) fn collect_watch_root_directories<I>(
    watch_root: &Path,
    entries: I,
) -> Result<Vec<PathBuf>, CoreError>
where
    I: IntoIterator<Item = io::Result<PathBuf>>,
{
    let mut directories = Vec::new();
    for entry in entries {
        let path = entry.map_err(|error| CoreError::io(watch_root, error))?;
        let metadata = fs::symlink_metadata(&path).map_err(|error| CoreError::io(&path, error))?;
        if metadata.is_dir() {
            directories.push(path);
        }
    }
    Ok(directories)
}

pub(super) fn contract_message(error: CoreError) -> String {
    match error {
        CoreError::Contract(message)
        | CoreError::HtmlcutInterop(message)
        | CoreError::Internal(message) => message,
        CoreError::PersistTransaction { summary, .. } => summary,
        other => other.to_string(),
    }
}
