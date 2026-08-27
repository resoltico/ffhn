//! Validation of one hash-guarded normal-commit filesystem operation.

use crate::CoreError;

use super::{CommitOperation, require_sha256};

impl CommitOperation {
    pub(super) fn validate(&self) -> Result<(), CoreError> {
        match self {
            Self::Install {
                final_path,
                staged_path,
                expected_prior_sha256,
                result_sha256,
            } => {
                if final_path == staged_path {
                    return Err(CoreError::contract(
                        "commit install final_path and staged_path must differ",
                    ));
                }
                if final_path.is_staged() || !staged_path.is_staged() {
                    return Err(CoreError::contract(
                        "commit install paths must keep final files outside and staged files inside the reserved staged directory",
                    ));
                }
                super::path_validation::validate_final_path(final_path)?;
                if let Some(hash) = expected_prior_sha256 {
                    require_sha256("commit install expected_prior_sha256", hash)?;
                }
                require_sha256("commit install result_sha256", result_sha256)
            }
            Self::Remove {
                final_path,
                expected_prior_sha256,
            } => {
                super::path_validation::validate_final_path(final_path)?;
                require_sha256("commit remove expected_prior_sha256", expected_prior_sha256)
            }
        }
    }
}
