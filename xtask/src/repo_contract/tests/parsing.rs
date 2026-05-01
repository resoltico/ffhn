use std::ffi::OsStr;

use super::*;
use crate::repo_files::maintained_repo_owned_paths;

#[test]
fn afad_managed_markdown_frontmatter_stays_on_the_protocol_version() {
    let repo_root = repo_root();
    let protocol_afad_version = protocol_afad_version(&repo_root).expect("protocol AFAD version");
    let paths = afad_managed_markdown_paths(&repo_root).expect("markdown paths");

    for path in paths {
        let path_display = path.display().to_string();
        let frontmatter = afad_frontmatter(&path)
            .expect("frontmatter parse")
            .unwrap_or_else(|| panic!("{path_display} is missing AFAD frontmatter"));
        assert_eq!(frontmatter.afad, protocol_afad_version, "{path_display}");
    }
}

#[test]
fn root_special_docs_stay_human_first_without_afad_frontmatter() {
    let repo_root = repo_root();

    for path in ["README.md", "CONTRIBUTING.md", "changelog.md"] {
        let path = repo_root.join(path);
        assert_eq!(
            afad_frontmatter(&path).expect("frontmatter parse"),
            None,
            "{} should remain a special doc without AFAD frontmatter",
            path.display()
        );
    }
}

#[test]
fn repo_contract_helpers_cover_present_missing_and_invalid_shapes() {
    let empty_repo = tempfile::tempdir().expect("empty tempdir");
    assert!(
        public_markdown_paths(empty_repo.path())
            .expect("empty markdown paths")
            .is_empty()
    );
    assert!(
        public_target_example_paths(empty_repo.path())
            .expect("empty target example paths")
            .is_empty()
    );

    let repo = tempfile::tempdir().expect("tempdir");
    let repo_root = repo.path();
    fs::create_dir_all(repo_root.join("docs/nested")).expect("create docs tree");
    fs::create_dir_all(repo_root.join("fuzz")).expect("create fuzz dir");
    fs::create_dir_all(repo_root.join("examples/file-example")).expect("create examples dir");
    fs::create_dir_all(repo_root.join("watchlist/demo")).expect("create watchlist target");
    fs::create_dir_all(repo_root.join("watchlist/empty")).expect("create empty watchlist dir");
    fs::create_dir_all(repo_root.join("crates/ffhn-cli/src/tests"))
        .expect("create ignored tests dir");
    fs::create_dir_all(repo_root.join("xtask/src")).expect("create xtask src dir");
    fs::create_dir_all(repo_root.join(".codex")).expect("create codex dir");

    fs::write(repo_root.join("AGENTS.md"), "# agents\n").expect("write AGENTS.md");
    fs::write(repo_root.join("README.md"), "# ffhn\n").expect("write README");
    fs::write(repo_root.join("CONTRIBUTING.md"), "# contribute\n").expect("write CONTRIBUTING");
    fs::write(repo_root.join("changelog.md"), "# changelog\n").expect("write changelog");
    fs::write(repo_root.join("examples/note.txt"), "ignore").expect("write ignored example");
    fs::write(repo_root.join("examples/.DS_Store"), "ignore").expect("write ignored metadata");
    fs::write(repo_root.join("examples/._release-notes.html"), "ignore")
        .expect("write ignored appledouble metadata");
    fs::write(
        repo_root.join("examples/file-example/release-notes.html"),
        "<main>demo</main>\n",
    )
    .expect("write maintained html example");
    fs::write(
        repo_root.join("examples/file-example/README.md"),
        "---\nafad: \"4.0\"\n---\n",
    )
    .expect("write example markdown");
    fs::write(repo_root.join("watchlist/file.txt"), "ignore")
        .expect("write ignored watchlist file");
    fs::write(
        repo_root.join("crates/ffhn-cli/src/keep.rs"),
        "pub const KEEP: &str = \"ffhn.run_report\";\n",
    )
    .expect("write kept source");
    fs::write(repo_root.join("crates/ffhn-cli/src/tests.rs"), "ignored\n")
        .expect("write ignored tests.rs");
    fs::write(
        repo_root.join("crates/ffhn-cli/src/tests/helper.rs"),
        "ignored\n",
    )
    .expect("write ignored nested tests source");
    fs::write(repo_root.join("xtask/src/note.txt"), "ignore").expect("write ignored non-rs source");
    fs::write(
        repo_root.join("docs/nested/guide.md"),
        "---\nafad: \"4.0\"\n---\n",
    )
    .expect("write guide");
    fs::write(
        repo_root.join("examples/example.toml"),
        "schema_name = \"ffhn.target\"\nschema_version = 1\ntarget_id = \"example\"\ndisplay_name = \"Example\"\nenabled = true\n\n[target]\nkind = \"http\"\nsource_url = \"https://example.com\"\n\n[fetch]\nengine = \"http\"\nmethod = \"GET\"\ntimeout_ms = 15000\nmax_bytes = 2000000\nuser_agent = \"ffhn/example\"\nfollow_redirects = true\naccept = \"text/html\"\n\n[selection]\nkind = \"css_selector\"\nselector = \"main\"\nmatch = \"single\"\noutput = \"outer_html\"\nwhitespace = \"normalize\"\nrewrite_urls = false\n\n[compare]\nbasis = \"canonical_text_sha256\"\ncanonicalization = []\n",
    )
    .expect("write example target");
    fs::write(
        repo_root.join("watchlist/demo/target.toml"),
        "schema_name = \"ffhn.target\"\nschema_version = 1\ntarget_id = \"demo\"\ndisplay_name = \"Demo\"\nenabled = true\n\n[target]\nkind = \"file\"\nfile_path = \"/tmp/source.html\"\n\n[fetch]\nengine = \"file\"\nmax_bytes = 2000000\n\n[selection]\nkind = \"css_selector\"\nselector = \"main\"\nmatch = \"single\"\noutput = \"outer_html\"\nwhitespace = \"normalize\"\nrewrite_urls = false\n\n[compare]\nbasis = \"canonical_text_sha256\"\ncanonicalization = []\n",
    )
    .expect("write watchlist target");
    fs::create_dir_all(repo_root.join("watchlist/demo/lock")).expect("create watchlist lock dir");
    fs::create_dir_all(repo_root.join("watchlist/demo/snapshots/current"))
        .expect("create watchlist snapshots dir");
    fs::write(repo_root.join("watchlist/demo/state.json"), "{}\n")
        .expect("write ignored watchlist state");
    fs::write(repo_root.join("watchlist/demo/last_run.json"), "{}\n")
        .expect("write ignored watchlist last run");
    fs::write(
        repo_root.join("watchlist/demo/lock/run.lock"),
        "ignored runtime artifact\n",
    )
    .expect("write ignored watchlist lock");
    fs::write(
        repo_root.join("watchlist/demo/snapshots/current/outer.html"),
        "<main>runtime</main>\n",
    )
    .expect("write ignored watchlist snapshot");
    fs::write(
        repo_root.join(".codex/PROTOCOL_AFAD.md"),
        "Protocol: `AGENT_FIRST_DOCUMENTATION`\nVersion: `4.0`\n",
    )
    .expect("write protocol");

    let markdown_paths = public_markdown_paths(repo_root).expect("markdown paths");
    assert_eq!(
        markdown_paths,
        vec![
            repo_root.join("CONTRIBUTING.md"),
            repo_root.join("README.md"),
            repo_root.join("changelog.md"),
            repo_root.join("docs/nested/guide.md"),
            repo_root.join("examples/file-example/README.md"),
        ]
    );
    assert_eq!(
        afad_managed_markdown_paths(repo_root).expect("afad markdown paths"),
        vec![
            repo_root.join("docs/nested/guide.md"),
            repo_root.join("examples/file-example/README.md"),
        ]
    );
    assert!(
        user_facing_source_paths(repo_root)
            .expect("source paths")
            .iter()
            .all(|path| path.extension() == Some(OsStr::new("rs")))
    );
    assert_eq!(
        user_facing_source_paths(repo_root).expect("source paths"),
        vec![repo_root.join("crates/ffhn-cli/src/keep.rs"),]
    );
    assert_eq!(
        protocol_afad_version(repo_root).expect("protocol AFAD version"),
        "4.0"
    );

    let target_paths = public_target_example_paths(repo_root).expect("target example paths");
    assert_eq!(
        target_paths,
        vec![
            repo_root.join("examples/example.toml"),
            repo_root.join("watchlist/demo/target.toml"),
        ]
    );

    let maintained_paths =
        maintained_repo_owned_paths(repo_root).expect("maintained repo-owned paths");
    assert!(maintained_paths.contains(&repo_root.join("AGENTS.md")));
    assert!(maintained_paths.contains(&repo_root.join("examples/file-example/README.md")));
    assert!(maintained_paths.contains(&repo_root.join("examples/file-example/release-notes.html")));
    assert!(maintained_paths.contains(&repo_root.join("watchlist/demo/target.toml")));
    assert!(!maintained_paths.contains(&repo_root.join("examples/.DS_Store")));
    assert!(!maintained_paths.contains(&repo_root.join("examples/._release-notes.html")));
    assert!(!maintained_paths.contains(&repo_root.join("watchlist/demo/state.json")));
    assert!(!maintained_paths.contains(&repo_root.join("watchlist/demo/last_run.json")));
    assert!(!maintained_paths.contains(&repo_root.join("watchlist/demo/lock/run.lock")));
    assert!(
        !maintained_paths.contains(&repo_root.join("watchlist/demo/snapshots/current/outer.html"))
    );

    fs::write(repo_root.join(".codex/PROTOCOL_AFAD.md"), "Version:\n")
        .expect("write invalid protocol");
    assert!(protocol_afad_version(repo_root).is_err());
}

#[test]
fn parse_afad_frontmatter_handles_missing_and_malformed_blocks() {
    assert_eq!(
        parse_afad_frontmatter("# ffhn").expect("no frontmatter"),
        None
    );
    assert_eq!(
        parse_afad_frontmatter("---\nroute:\n  keywords: []\n---\n")
            .expect("metadata-only frontmatter"),
        None
    );

    assert!(parse_afad_frontmatter("---\nafad: \"4.0\"\nversion: \"2.0.0\"\n").is_err());
    assert!(parse_afad_frontmatter("---\nafad: \"4.0\"\nversion: \"2.0.0\"\n---\n").is_err());
    assert_eq!(
        parse_afad_frontmatter("---\nafad: \"4.0\"\n---\n").expect("frontmatter"),
        Some(AfadFrontmatter {
            afad: "4.0".to_owned(),
        })
    );
    assert!(parse_afad_frontmatter("---\nversion: \"2.0.0\"\n---\n").is_err());

    assert_eq!(
        parse_protocol_afad_version("Protocol: `AGENT_FIRST_DOCUMENTATION`\nVersion: `4.0`\n")
            .expect("protocol version"),
        "4.0"
    );
    assert_eq!(
        parse_protocol_afad_version("PROTOCOL: AGENT_FIRST_DOCUMENTATION\nVERSION: 3.5\n")
            .expect("legacy protocol version"),
        "3.5"
    );
    assert_eq!(
        parse_protocol_afad_version("**Version:** 4.0\n**Updated:** 2026-04-24\n")
            .expect("markdown protocol version"),
        "4.0"
    );
    assert!(parse_protocol_afad_version("VERSION:\n").is_err());
    assert!(parse_protocol_afad_version("Protocol: `AGENT_FIRST_DOCUMENTATION`\n").is_err());

    assert_eq!(
        marked_section(
            "before\n<!-- contract:demo:start -->\ncontent\n<!-- contract:demo:end -->\nafter\n",
            "demo"
        )
        .expect("marked section"),
        "content"
    );
    assert!(marked_section("before\n<!-- contract:demo:start -->\ncontent\n", "demo").is_err());

    assert_eq!(
        extract_cli_operation_ids(
            "`ffhn run --target demo`\n```bash\ncargo run -p ffhn-cli -- status --target demo\n```"
        ),
        BTreeSet::from(["run".to_owned(), "status".to_owned()])
    );
    assert_eq!(
        extract_cli_operation_ids(
            "`/usr/local/bin/ffhn status --target demo`\n`cargo run -p ffhn-cli -- run --target demo`\n`ffhn --help`"
        ),
        BTreeSet::from(["run".to_owned(), "status".to_owned()])
    );
    assert!(extract_cli_operation_ids("`-- status`").is_empty());
    assert!(extract_cli_operation_ids("`ffhn --help`").is_empty());
    assert!(extract_cli_operation_ids("`ffhn !`").is_empty());
    assert!(code_segments("before `unterminated").is_empty());
    assert_eq!(
        string_literals(
            "const A: &str = \"ffhn.run_report\";\nlet raw = r#\"ffhn status --target demo\"#;"
        ),
        vec![
            "ffhn.run_report".to_owned(),
            "ffhn status --target demo".to_owned(),
        ]
    );
    assert_eq!(
        string_literals("let raw = r\"ffhn.state\";"),
        vec!["ffhn.state".to_owned()]
    );
    assert_eq!(
        string_literals(
            "let raw = r##\"ffhn.extraction_record\"##;\nlet escaped = \"say \\\"hi\\\"\";\nlet marker = r#not_a_string;\n"
        ),
        vec![
            "ffhn.extraction_record".to_owned(),
            "say \\\"hi\\\"".to_owned(),
        ]
    );
    assert!(string_literals("r").is_empty());
    assert!(string_literals("let broken = r#\"unterminated").is_empty());
    assert!(string_literals("let broken = r\"unterminated").is_empty());
    assert!(string_literals("let broken = r#\"x\"").is_empty());
    assert!(string_literals("let broken = \"unterminated").is_empty());
    assert_eq!(
        production_source_text(
            "fn main() {}\n#[cfg(test)]\nmod tests { const X: &str = \"ffhn.unknown\"; }\n"
        ),
        "fn main() {}"
    );
    assert_eq!(production_source_text("fn main() {}\n"), "fn main() {}\n");
    assert!(looks_like_contract_document_id("ffhn.unknown_report"));
    assert!(!looks_like_contract_document_id("ffhn.exe"));
    assert!(!looks_like_contract_document_id("not_ffhn.state"));
    assert_eq!(
        extract_document_ids("`ffhn.run_report!` plus `ffhn.status_report`"),
        BTreeSet::from([
            "ffhn.run_report".to_owned(),
            "ffhn.status_report".to_owned(),
        ])
    );
    assert_eq!(
        extract_document_ids("ffhn.unknown_report ffhn.exe ffhn.run.report ffhn.state"),
        BTreeSet::from(["ffhn.state".to_owned(), "ffhn.unknown_report".to_owned(),])
    );
    assert_eq!(
        extract_document_ids("ffhn.target ffhn.extraction_record ffhn.batch_run_report"),
        BTreeSet::from([
            "ffhn.batch_run_report".to_owned(),
            "ffhn.extraction_record".to_owned(),
            "ffhn.target".to_owned(),
        ])
    );
    assert!(extract_document_ids("ffhn.").is_empty());
}
