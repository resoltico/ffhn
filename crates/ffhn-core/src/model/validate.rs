use crate::CoreError;

/// Rejects blank contract fields.
pub(crate) fn require_non_empty(field: &str, value: &str) -> Result<(), CoreError> {
    if value.trim().is_empty() {
        return Err(CoreError::contract(format!("{field} must not be empty")));
    }
    Ok(())
}
