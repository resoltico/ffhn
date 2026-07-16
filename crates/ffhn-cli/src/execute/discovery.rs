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
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn discovery_handles_explicit_missing_invalid_and_directory_entries() {
        let temporary = tempdir().expect("temporary directory");
        let root = temporary.path();
        let explicit = RunCommand {
            watch_root: root.to_path_buf(),
            targets: vec![TargetId::new("demo").expect("target")],
            all: false,
            jobs: 1,
            dry_run: false,
            output_format: crate::args::OutputFormat::Json,
        };
        assert!(matches!(
            selected_targets(&explicit),
            Ok(SelectedTargets::Explicit(_))
        ));
        assert!(discover_watch_root_targets(&root.join("missing")).is_err());
        let file = root.join("not-a-directory");
        fs::write(&file, "file").expect("file");
        assert!(discover_watch_root_targets(&file).is_err());

        fs::create_dir(root.join("valid")).expect("valid dir");
        fs::write(root.join("valid/target.toml"), "target").expect("target");
        fs::create_dir(root.join("invalid!")).expect("invalid dir");
        fs::write(root.join("invalid!/target.toml"), "target").expect("target");
        fs::create_dir(root.join("ignored")).expect("ignored dir");
        let discovered = discover_watch_root_targets(root).expect("discovery");
        assert_eq!(discovered.len(), 2);
        assert_eq!(discovered[0].requested_id, "invalid!");
        assert!(discovered[0].validated_id.is_none());
        assert!(discovered[0].validation_message.is_some());
        assert_eq!(
            discovered[1].validated_id.as_ref().map(TargetId::as_str),
            Some("valid")
        );

        let all = RunCommand {
            all: true,
            ..explicit
        };
        assert!(matches!(
            selected_targets(&all),
            Ok(SelectedTargets::Discovered(_))
        ));
        let entries = vec![
            Ok(root.join("valid")),
            Ok(root.join("not-a-directory")),
            Err(io::Error::other("entry failure")),
        ];
        assert!(collect_watch_root_directories(root, entries).is_err());
        assert!(contract_message(CoreError::contract("message")).contains("message"));
    }
}
