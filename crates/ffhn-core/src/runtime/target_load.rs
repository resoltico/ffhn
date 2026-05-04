use std::path::Path;

use serde::Deserialize;

use crate::{CoreError, ProcessErrorDetail, TargetDocument, TargetPaths};

use super::status::validate_target_against_paths;
use super::storage::read_text;

#[derive(Clone, Debug)]
pub(crate) enum TargetLoad {
    Valid(Box<TargetDocument>),
    Invalid(ProcessErrorDetail),
}

/// Loads one target document while preserving process-level read failures.
///
/// Returns [`TargetLoad::Valid`] when the document decoded and validated successfully,
/// [`TargetLoad::Invalid`] when the target exists but violates FFHN's target contract, and
/// `Err(CoreError::Io { .. })` for process-level read failures.
pub(crate) fn load_target_document(paths: &TargetPaths) -> Result<TargetLoad, CoreError> {
    let target = match read_target_document(&paths.target_file()) {
        Ok(target) => target,
        Err(error @ CoreError::Io { .. }) => return Err(error),
        Err(error) => return Ok(TargetLoad::Invalid(target_load_detail(paths, &error))),
    };

    match validate_target_against_paths(paths, target) {
        Ok(target) => Ok(TargetLoad::Valid(Box::new(target))),
        Err(error) => Ok(TargetLoad::Invalid(target_load_detail(paths, &error))),
    }
}

pub(crate) fn read_target_document(path: &Path) -> Result<TargetDocument, CoreError> {
    let text = read_text(path)?;
    let value: toml::Value = toml::from_str(&text)?;
    TargetDocument::deserialize(value).map_err(target_decode_error)
}

fn target_load_detail(paths: &TargetPaths, error: &CoreError) -> ProcessErrorDetail {
    ProcessErrorDetail::from(error).with_fallback_path(paths.target_file().display().to_string())
}

fn target_decode_error(error: toml::de::Error) -> CoreError {
    let message = error.to_string();
    let message = message
        .strip_prefix("contract error: ")
        .unwrap_or(message.as_str())
        .trim_end()
        .to_owned();
    CoreError::contract(message)
}
