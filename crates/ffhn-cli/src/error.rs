use std::io::{self, Write};

/// Canonical fatal error text when FFHN cannot write CLI stdout output.
pub const CLI_OUTPUT_WRITE_ERROR: &str = "could not write CLI output";

/// Writes one human-readable CLI error line to stderr.
pub fn write_cli_error(stderr: &mut (impl Write + ?Sized), message: &str) -> io::Result<()> {
    writeln!(stderr, "{message}")
}
