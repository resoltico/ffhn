use regex::RegexBuilder;

use crate::{CanonicalizerKind, CanonicalizerSpec, CoreError, RegexFlag};

/// Applies one ordered canonicalization pipeline to FFHN comparison input.
pub fn apply_canonicalizers(
    input: &str,
    canonicalizers: &[CanonicalizerSpec],
) -> Result<String, CoreError> {
    let mut output = input.to_owned();

    for canonicalizer in canonicalizers {
        output = match canonicalizer.kind {
            CanonicalizerKind::Trim => output.trim().to_owned(),
            CanonicalizerKind::CollapseWhitespace => collapse_whitespace(&output),
            CanonicalizerKind::NormalizeNewlines => normalize_line_endings(&output),
            CanonicalizerKind::StripRegex => {
                let pattern = canonicalizer.pattern.as_deref().ok_or_else(|| {
                    CoreError::contract("strip_regex canonicalizer is missing pattern")
                })?;
                let regex = build_regex(pattern, &canonicalizer.flags)?;
                regex.replace_all(&output, "").into_owned()
            }
            CanonicalizerKind::Lowercase => output.to_lowercase(),
        };
    }

    Ok(normalize_line_endings(&output))
}

/// Normalizes text line endings to LF.
pub fn normalize_line_endings(input: &str) -> String {
    input.replace("\r\n", "\n").replace('\r', "\n")
}

fn collapse_whitespace(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut last_was_whitespace = false;

    for ch in input.chars() {
        if ch.is_whitespace() {
            if !last_was_whitespace {
                output.push(' ');
            }
            last_was_whitespace = true;
        } else {
            output.push(ch);
            last_was_whitespace = false;
        }
    }

    output
}

fn build_regex(pattern: &str, flags: &[RegexFlag]) -> Result<regex::Regex, CoreError> {
    let mut builder = RegexBuilder::new(pattern);
    builder.unicode(true);

    for flag in flags {
        match flag {
            RegexFlag::CaseInsensitive => builder.case_insensitive(true),
            RegexFlag::MultiLine => builder.multi_line(true),
            RegexFlag::DotMatchesNewLine => builder.dot_matches_new_line(true),
            RegexFlag::SwapGreed => builder.swap_greed(true),
            RegexFlag::IgnoreWhitespace => builder.ignore_whitespace(true),
        };
    }

    builder
        .build()
        .map_err(|error| CoreError::contract(format!("regex compile failed: {error}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strip_regex(pattern: Option<&str>, flags: Vec<RegexFlag>) -> CanonicalizerSpec {
        CanonicalizerSpec {
            kind: CanonicalizerKind::StripRegex,
            pattern: pattern.map(ToOwned::to_owned),
            flags,
        }
    }

    #[test]
    fn canonicalizers_apply_all_supported_transforms_in_order() {
        let output = apply_canonicalizers(
            "  Hello\r\nWORLD 123  ",
            &[
                CanonicalizerSpec {
                    kind: CanonicalizerKind::Trim,
                    pattern: None,
                    flags: Vec::new(),
                },
                CanonicalizerSpec {
                    kind: CanonicalizerKind::NormalizeNewlines,
                    pattern: None,
                    flags: Vec::new(),
                },
                CanonicalizerSpec {
                    kind: CanonicalizerKind::CollapseWhitespace,
                    pattern: None,
                    flags: Vec::new(),
                },
                strip_regex(Some(r"\d+"), Vec::new()),
                CanonicalizerSpec {
                    kind: CanonicalizerKind::Lowercase,
                    pattern: None,
                    flags: Vec::new(),
                },
            ],
        )
        .expect("canonicalization");

        assert_eq!(output, "hello world ");
    }

    #[test]
    fn strip_regex_requires_a_pattern() {
        let error = apply_canonicalizers("hello", &[strip_regex(None, Vec::new())])
            .expect_err("missing pattern should fail");

        assert_eq!(
            error.to_string(),
            "contract error: strip_regex canonicalizer is missing pattern"
        );
    }

    #[test]
    fn strip_regex_respects_configured_regex_flags() {
        let output = apply_canonicalizers(
            "Alpha\nbeta\nGAMMA",
            &[strip_regex(
                Some("(?m)^alpha$|^gamma$"),
                vec![RegexFlag::CaseInsensitive, RegexFlag::MultiLine],
            )],
        )
        .expect("flagged regex");

        assert_eq!(output, "\nbeta\n");
    }

    #[test]
    fn canonicalizers_cover_whitespace_runs_and_remaining_regex_flags() {
        let collapsed = apply_canonicalizers(
            "A \t \n  B",
            &[CanonicalizerSpec {
                kind: CanonicalizerKind::CollapseWhitespace,
                pattern: None,
                flags: Vec::new(),
            }],
        )
        .expect("collapsed whitespace");
        assert_eq!(collapsed, "A B");

        let stripped = apply_canonicalizers(
            "BEGIN\nmiddle\nEND",
            &[strip_regex(
                Some(" begin .* end "),
                vec![
                    RegexFlag::CaseInsensitive,
                    RegexFlag::DotMatchesNewLine,
                    RegexFlag::SwapGreed,
                    RegexFlag::IgnoreWhitespace,
                ],
            )],
        )
        .expect("strip regex with remaining flags");
        assert!(stripped.is_empty());
    }

    #[test]
    fn invalid_regex_patterns_fail_cleanly() {
        let error = apply_canonicalizers("hello", &[strip_regex(Some("["), Vec::new())])
            .expect_err("invalid pattern should fail");

        assert!(
            error
                .to_string()
                .contains("contract error: regex compile failed:")
        );
    }

    #[test]
    fn normalize_line_endings_handles_crlf_and_cr() {
        assert_eq!(normalize_line_endings("a\r\nb\rc"), "a\nb\nc");
    }
}
