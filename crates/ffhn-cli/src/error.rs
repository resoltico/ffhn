use std::io::{self, Write};

/// Canonical fatal error text when FFHN cannot write CLI stdout output.
pub const CLI_OUTPUT_WRITE_ERROR: &str = "could not write CLI output";

/// Writes one human-readable CLI error line to stderr.
pub fn write_cli_error(stderr: &mut (impl Write + ?Sized), message: &str) -> io::Result<()> {
    let normalized = message
        .strip_prefix("error: ")
        .or_else(|| message.strip_prefix("error:"))
        .unwrap_or(message);
    writeln!(stderr, "error: {normalized}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_optional_error_prefixes_before_rendering() {
        for (input, expected) in [
            ("error: one", "error: one\n"),
            ("error:two", "error: two\n"),
            ("three", "error: three\n"),
        ] {
            let mut output = Vec::new();
            write_cli_error(&mut output, input).expect("stderr write");
            assert_eq!(String::from_utf8(output).expect("UTF-8"), expected);
        }
    }
}
