use crate::{CoreError, TargetDocument, TargetPaths};

use super::status::validate_target_against_paths;
use super::storage::read_toml;

/// Loads one target document while preserving process-level read failures.
///
/// Returns `Ok(Some(target))` when the document decoded and validated
/// successfully, `Ok(None)` when the target exists but violates FFHN's target
/// contract, and `Err(CoreError::Io { .. })` for process-level read failures.
pub(crate) fn load_target_document(
    paths: &TargetPaths,
) -> Result<Option<TargetDocument>, CoreError> {
    let target = match read_toml::<TargetDocument>(&paths.target_file()) {
        Ok(target) => target,
        Err(error @ CoreError::Io { .. }) => return Err(error),
        Err(_) => return Ok(None),
    };

    match validate_target_against_paths(paths, target) {
        Ok(target) => Ok(Some(target)),
        Err(_) => Ok(None),
    }
}
