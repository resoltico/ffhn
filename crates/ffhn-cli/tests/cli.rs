use std::fs;

use assert_cmd::Command;
use predicates::str::contains;
use tempfile::tempdir;

#[test]
fn status_emits_one_json_document() {
    let temp = tempdir().expect("tempdir");
    let target_dir = temp.path().join("watchlist").join("demo");
    fs::create_dir_all(&target_dir).expect("create target dir");
    fs::write(
        target_dir.join("target.toml"),
        r#"
schema_name = "ffhn.target"
schema_version = 1
target_id = "demo"
display_name = "Demo"
enabled = true

[target]
kind = "http"
source_url = "https://example.com"

[fetch]
engine = "http"
method = "GET"
timeout_ms = 15000
max_bytes = 2000000
user_agent = "ffhn/2.0.0"
follow_redirects = true
accept = "text/html,application/xhtml+xml"

[selection]
kind = "css_selector"
selector = "main"
match = "single"
output = "outer_html"
whitespace = "normalize"
rewrite_urls = false

[compare]
basis = "canonical_text_sha256"
canonicalization = []
"#,
    )
    .expect("write target.toml");

    let mut command = Command::cargo_bin("ffhn").expect("ffhn binary");
    command
        .current_dir(temp.path())
        .args(["status", "--target", "demo"])
        .assert()
        .success()
        .stdout(contains("\"schema_name\":\"ffhn.status_report\""))
        .stdout(contains("\"reason_code\":\"ok\""));
}
