use std::io::{self, Write};

/// Writes one human-readable CLI error line to stderr.
pub fn write_cli_error(stderr: &mut impl Write, message: &str) -> io::Result<()> {
    writeln!(stderr, "{message}")
}
