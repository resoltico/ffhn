use std::io::{self, Write};

use serde::Serialize;

/// Serializes one schema document to stdout as a single JSON document.
pub fn render_json_document(stdout: &mut impl Write, value: &impl Serialize) -> io::Result<()> {
    serde_json::to_writer(&mut *stdout, value).map_err(io::Error::other)?;
    writeln!(stdout)
}
