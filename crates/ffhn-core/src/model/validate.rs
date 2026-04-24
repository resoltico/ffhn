use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use url::Url;

use crate::CoreError;

use super::RegexFlag;

const SHA256_HEX_LEN: usize = 64;

pub(crate) fn validate_identity(
    schema_name: &str,
    expected_schema_name: &str,
    schema_version: u32,
    expected_schema_version: u32,
) -> Result<(), CoreError> {
    if schema_name != expected_schema_name {
        return Err(CoreError::contract(format!(
            "schema_name must be {expected_schema_name:?}"
        )));
    }
    if schema_version != expected_schema_version {
        return Err(CoreError::contract(format!(
            "schema_version must be {expected_schema_version}"
        )));
    }
    Ok(())
}

pub(crate) fn validate_absolute_url(url: &Url) -> Result<(), CoreError> {
    match (url.scheme(), url.has_host()) {
        ("http" | "https", true) => Ok(()),
        _ => Err(CoreError::contract(
            "target.source_url must be an absolute http or https URL",
        )),
    }
}

pub(crate) fn validate_absolute_file_path(path: &str) -> Result<(), CoreError> {
    require_non_empty("target.file_path", path)?;
    let candidate = std::path::Path::new(path);
    if !candidate.is_absolute() {
        return Err(CoreError::contract(
            "target.file_path must be an absolute filesystem path",
        ));
    }
    Ok(())
}

pub(crate) fn validate_sha256(value: &str) -> Result<(), CoreError> {
    if value.len() != SHA256_HEX_LEN
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(CoreError::contract(
            "expected one 64-character lowercase hex SHA-256 digest",
        ));
    }
    Ok(())
}

pub(crate) fn validate_timestamp(value: &str) -> Result<(), CoreError> {
    OffsetDateTime::parse(value, &Rfc3339)?;
    Ok(())
}

pub(crate) fn require_non_empty(field: &str, value: &str) -> Result<(), CoreError> {
    if value.trim().is_empty() {
        return Err(CoreError::contract(format!("{field} must not be empty")));
    }
    Ok(())
}

#[cfg(test)]
pub(crate) fn require_non_empty_option(field: &str, value: Option<&str>) -> Result<(), CoreError> {
    require_non_empty(
        field,
        value.ok_or_else(|| CoreError::contract(format!("{field} is required")))?,
    )
}

pub(crate) fn forbid_option<T>(field: &str, value: Option<T>) -> Result<(), CoreError> {
    if value.is_some() {
        return Err(CoreError::contract(format!(
            "{field} is not valid for this selection kind"
        )));
    }
    Ok(())
}

pub(crate) fn apply_regex_flag(flag: &RegexFlag, builder: &mut regex::RegexBuilder) {
    match flag {
        RegexFlag::CaseInsensitive => {
            builder.case_insensitive(true);
        }
        RegexFlag::MultiLine => {
            builder.multi_line(true);
        }
        RegexFlag::DotMatchesNewLine => {
            builder.dot_matches_new_line(true);
        }
        RegexFlag::SwapGreed => {
            builder.swap_greed(true);
        }
        RegexFlag::IgnoreWhitespace => {
            builder.ignore_whitespace(true);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_identity_requires_exact_name_and_version() {
        validate_identity("ffhn.target", "ffhn.target", 1, 1).expect("matching identity");
        assert!(validate_identity("ffhn.state", "ffhn.target", 1, 1).is_err());
        assert!(validate_identity("ffhn.target", "ffhn.target", 2, 1).is_err());
    }

    #[test]
    fn absolute_url_validation_requires_http_or_https_with_host() {
        validate_absolute_url(&Url::parse("https://example.com").expect("url")).expect("https ok");
        assert!(validate_absolute_url(&Url::parse("file:///tmp/x").expect("file url")).is_err());
        assert!(
            validate_absolute_url(&Url::parse("mailto:test@example.com").expect("mailto")).is_err()
        );
        validate_absolute_file_path("/tmp/demo.html").expect("absolute file path");
        assert!(validate_absolute_file_path("demo.html").is_err());
    }

    #[test]
    fn sha256_validation_rejects_invalid_shapes() {
        validate_sha256(&"a".repeat(64)).expect("lowercase sha");
        assert!(validate_sha256(&"A".repeat(64)).is_err());
    }

    #[test]
    fn timestamp_and_option_helpers_cover_required_edge_cases() {
        validate_timestamp("2026-04-05T10:15:30Z").expect("timestamp");
        assert!(validate_timestamp("bad").is_err());
        require_non_empty("field", "value").expect("non-empty");
        assert!(require_non_empty("field", "   ").is_err());
        require_non_empty_option("field", Some("value")).expect("some");
        assert!(require_non_empty_option("field", Some("   ")).is_err());
        assert!(require_non_empty_option("field", None).is_err());
        forbid_option::<u8>("field", None).expect("none");
        assert!(forbid_option("field", Some(1u8)).is_err());
    }

    #[test]
    fn regex_flags_apply_to_the_regex_builder() {
        let mut builder = regex::RegexBuilder::new("^a.b$");
        builder.unicode(true);
        for flag in [
            RegexFlag::CaseInsensitive,
            RegexFlag::MultiLine,
            RegexFlag::DotMatchesNewLine,
            RegexFlag::SwapGreed,
            RegexFlag::IgnoreWhitespace,
        ] {
            apply_regex_flag(&flag, &mut builder);
        }

        let regex = builder.build().expect("regex");
        assert!(regex.is_match("A\nB"));
    }
}
