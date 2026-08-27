use time::{OffsetDateTime, UtcOffset, format_description::well_known::Rfc3339};

use crate::CoreError;

/// Returns whether a string is one lowercase hexadecimal SHA-256 digest.
pub(crate) fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

/// Requires the stable timestamp spelling used by every durable graph document.
pub(crate) fn require_canonical_utc_rfc3339(field: &str, value: &str) -> Result<(), CoreError> {
    let timestamp = OffsetDateTime::parse(value, &Rfc3339)
        .map_err(|_| CoreError::contract(format!("{field} must be RFC 3339")))?;
    let canonical = timestamp
        .format(&Rfc3339)
        .map_err(|_| CoreError::internal("could not format timestamp"))?;
    if timestamp.offset() != UtcOffset::UTC || value != canonical {
        return Err(CoreError::contract(format!(
            "{field} must be canonical UTC RFC 3339"
        )));
    }
    Ok(())
}
