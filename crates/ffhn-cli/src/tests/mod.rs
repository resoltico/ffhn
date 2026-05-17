use std::ffi::OsString;
use std::fs;
use std::io::{self, Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::thread;
use tempfile::tempdir;

use super::*;
use crate::error::CLI_OUTPUT_WRITE_ERROR;
use crate::execute::{collect_watch_root_directories, discover_watch_root_targets};
use ffhn_core::{
    BATCH_RUN_REPORT_SCHEMA_NAME, BatchRunReport, CLI_OPERATION_RUN_ID, CLI_OPERATION_STATUS_ID,
    RUN_REPORT_SCHEMA_NAME, RunReport, STATUS_REPORT_SCHEMA_NAME, StatusReport, cli_contract,
    cli_operation, document_write_error, duplicate_target_ids_usage_error,
    positive_batch_concurrency_usage_error, run_target_selection_usage_error,
};

fn run_vec<I, T>(args: I) -> (i32, String, String)
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
{
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let exit_code = run(args, &mut stdout, &mut stderr);
    (
        exit_code,
        String::from_utf8(stdout).expect("stdout utf8"),
        String::from_utf8(stderr).expect("stderr utf8"),
    )
}

fn parse_run_report(stdout: &str) -> RunReport {
    serde_json::from_str(stdout).expect("run report json")
}

fn parse_batch_run_report(stdout: &str) -> BatchRunReport {
    serde_json::from_str(stdout).expect("batch run report json")
}

fn parse_status_report(stdout: &str) -> StatusReport {
    serde_json::from_str(stdout).expect("status report json")
}

fn workspace_package_field(manifest: &str, field: &str) -> Option<String> {
    let mut in_workspace_package = false;

    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_workspace_package = trimmed == "[workspace.package]";
            continue;
        }

        if !in_workspace_package {
            continue;
        }

        if let Some(value) = trimmed.strip_prefix(&format!("{field} = \""))
            && let Some(value) = value.strip_suffix('"')
        {
            return Some(value.to_owned());
        }
    }

    None
}

struct BrokenWriter;

impl Write for BrokenWriter {
    fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
        Err(io::Error::other("broken writer"))
    }

    fn flush(&mut self) -> io::Result<()> {
        Err(io::Error::other("broken writer"))
    }
}

fn serve_once(
    status_line: &'static str,
    content_type: &'static str,
    body: &'static str,
) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind server");
    let address = listener.local_addr().expect("server addr");
    let url = format!("http://{address}");
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        let mut request = [0u8; 2048];
        let _ = stream.read(&mut request);
        let raw = format!(
            "HTTP/1.1 {status_line}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        );
        let _ = stream.write_all(raw.as_bytes());
    });
    (url, handle)
}

fn write_named_http_target(
    watch_root: &Path,
    dir_name: &str,
    declared_target_id: &str,
    source_url: &str,
    enabled: bool,
) {
    let target_dir = watch_root.join(dir_name);
    fs::create_dir_all(&target_dir).expect("create target dir");
    fs::write(
        target_dir.join("target.toml"),
        format!(
            r#"
schema_name = "ffhn.target"
schema_version = 4
target_id = "{declared_target_id}"
display_name = "{dir_name}"
enabled = {enabled}

[target]
kind = "http"
source_url = "{source_url}"

[fetch]
engine = "http"
method = "GET"
timeout_ms = 15000
max_bytes = 2000000
user_agent = "ffhn/example"
follow_redirects = true
accept = "text/html"

[selection]
kind = "css_selector"
selector = "main"
match = "single"

[compare]
basis = "text"
whitespace = "normalize"
rewrite_urls = false
canonicalization = []
"#
        ),
    )
    .expect("write target.toml");
}

fn write_named_file_target(
    watch_root: &Path,
    dir_name: &str,
    declared_target_id: &str,
    source_path: &Path,
    enabled: bool,
) {
    let target_dir = watch_root.join(dir_name);
    fs::create_dir_all(&target_dir).expect("create target dir");
    fs::write(
        target_dir.join("target.toml"),
        format!(
            r#"
schema_name = "ffhn.target"
schema_version = 4
target_id = "{declared_target_id}"
display_name = "{dir_name}"
enabled = {enabled}

[target]
kind = "file"
file_path = {source_path:?}

[fetch]
engine = "file"
max_bytes = 2000000

[selection]
kind = "css_selector"
selector = "main"
match = "single"

[compare]
basis = "text"
whitespace = "normalize"
rewrite_urls = false
canonicalization = []
"#
        ),
    )
    .expect("write target.toml");
}

fn write_target(watch_root: &Path, source_url: &str, enabled: bool) {
    write_named_http_target(watch_root, "demo", "demo", source_url, enabled);
}

mod cli_contract;
mod run;
mod watch_root;
