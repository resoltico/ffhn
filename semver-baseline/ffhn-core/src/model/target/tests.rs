use super::*;
use crate::NotificationEvent;
use crate::{TARGET_SCHEMA_NAME, TARGET_SCHEMA_VERSION};
use std::collections::BTreeMap;
use url::Url;

fn valid_target() -> TargetDocument {
    TargetDocument {
        schema_name: TARGET_SCHEMA_NAME.to_owned(),
        schema_version: TARGET_SCHEMA_VERSION,
        target_id: "demo".to_owned(),
        display_name: "Demo".to_owned(),
        enabled: true,
        target: TargetSource {
            kind: TargetKind::Http,
            source_url: Some(Url::parse("https://example.com/page").expect("url")),
            file_path: None,
        },
        fetch: FetchConfig {
            engine: FetchEngine::Http,
            method: HttpMethod::GET,
            timeout_ms: 15_000,
            max_bytes: 2_000_000,
            user_agent: "ffhn/2.0.0".to_owned(),
            follow_redirects: true,
            accept: "text/html".to_owned(),
            headers: BTreeMap::new(),
            extensions: None,
        },
        selection: SelectionConfig {
            kind: SelectionKind::CssSelector,
            r#match: SelectionMatch::Single,
            index: None,
            output: OutputKind::OuterHtml,
            whitespace: WhitespaceMode::Normalize,
            rewrite_urls: false,
            selector: Some("main".to_owned()),
            start: None,
            end: None,
            mode: None,
            include_start: None,
            include_end: None,
            flags: Vec::new(),
        },
        compare: CompareConfig {
            basis: CompareBasis::CanonicalTextSha256,
            canonicalization: Vec::new(),
        },
        storage: Default::default(),
        notifications: Vec::new(),
        extensions: None,
    }
}

fn valid_file_target() -> TargetDocument {
    let mut target = valid_target();
    target.target.kind = TargetKind::File;
    target.target.source_url = None;
    target.target.file_path = Some("/tmp/demo.html".to_owned());
    target.fetch.engine = FetchEngine::File;
    target.fetch.follow_redirects = false;
    target.fetch.user_agent.clear();
    target.fetch.accept.clear();
    target
}

#[test]
fn valid_css_selector_target_document_passes_validation() {
    valid_target().validate().expect("valid target");

    let mut browser_target = valid_target();
    browser_target.fetch.engine = FetchEngine::Browser;
    browser_target
        .validate()
        .expect("valid target with browser engine alias");

    TargetDocument {
        compare: CompareConfig {
            basis: CompareBasis::CanonicalTextSha256,
            canonicalization: vec![CanonicalizerSpec {
                kind: CanonicalizerKind::Trim,
                pattern: None,
                flags: Vec::new(),
            }],
        },
        ..valid_target()
    }
    .validate()
    .expect("valid target with compare pipeline");
}

#[test]
fn target_validation_checks_url_ranges_and_header_values() {
    let mut target = valid_target();
    target.schema_name = "wrong".to_owned();
    assert!(target.validate().is_err());

    let mut target = valid_target();
    target.target.source_url = Some(Url::parse("file:///tmp/demo").expect("file url"));
    assert!(target.validate().is_err());

    let mut target = valid_target();
    target.fetch.timeout_ms = 999;
    assert!(target.validate().is_err());

    let mut target = valid_target();
    target.fetch.max_bytes = 100;
    assert!(target.validate().is_err());

    let mut target = valid_target();
    target.fetch.timeout_ms = 600_001;
    assert!(target.validate().is_err());

    let mut target = valid_target();
    target.fetch.max_bytes = 104_857_601;
    assert!(target.validate().is_err());

    let mut target = valid_target();
    target
        .fetch
        .headers
        .insert("".to_owned(), "value".to_owned());
    assert!(target.validate().is_err());

    let mut target = valid_target();
    target
        .fetch
        .headers
        .insert("x-demo".to_owned(), "".to_owned());
    assert!(target.validate().is_err());

    CompareConfig {
        basis: CompareBasis::CanonicalTextSha256,
        canonicalization: vec![CanonicalizerSpec {
            kind: CanonicalizerKind::Lowercase,
            pattern: None,
            flags: Vec::new(),
        }],
    }
    .validate()
    .expect("compare config");
}

#[test]
fn selection_validation_enforces_match_index_rules() {
    let mut selection = valid_target().selection;
    selection.r#match = SelectionMatch::Nth;
    selection.index = Some(2);
    selection.validate().expect("nth with index");

    selection.index = None;
    assert!(selection.validate().is_err());

    selection.index = Some(0);
    assert!(selection.validate().is_err());

    let mut selection = valid_target().selection;
    selection.index = Some(2);
    assert!(selection.validate().is_err());
}

#[test]
fn css_selection_forbids_delimiter_specific_fields_and_flags() {
    let mut selection = valid_target().selection;
    selection.start = Some("BEGIN".to_owned());
    assert!(selection.validate().is_err());

    let mut selection = valid_target().selection;
    selection.flags = vec![RegexFlag::CaseInsensitive];
    assert!(selection.validate().is_err());
}

#[test]
fn delimiter_selection_requires_its_full_contract() {
    let mut selection = valid_target().selection;
    selection.kind = SelectionKind::DelimiterPair;
    selection.r#match = SelectionMatch::Nth;
    selection.index = Some(1);
    selection.selector = None;
    selection.start = Some("BEGIN".to_owned());
    selection.end = Some("END".to_owned());
    selection.mode = Some(DelimiterMode::Regex);
    selection.include_start = Some(false);
    selection.include_end = Some(true);
    selection.flags = vec![RegexFlag::CaseInsensitive];
    selection.validate().expect("valid delimiter selection");

    let mut missing_mode = selection.clone();
    missing_mode.mode = None;
    assert!(missing_mode.validate().is_err());

    let mut literal_with_flags = selection.clone();
    literal_with_flags.mode = Some(DelimiterMode::Literal);
    assert!(literal_with_flags.validate().is_err());

    let mut missing_include = selection.clone();
    missing_include.include_end = None;
    assert!(missing_include.validate().is_err());

    let mut missing_include_start = selection.clone();
    missing_include_start.include_start = None;
    assert!(missing_include_start.validate().is_err());

    let mut literal_without_flags = selection.clone();
    literal_without_flags.mode = Some(DelimiterMode::Literal);
    literal_without_flags.flags = Vec::new();
    literal_without_flags
        .validate()
        .expect("literal delimiter without regex flags");

    let mut with_selector = selection;
    with_selector.selector = Some("main".to_owned());
    assert!(with_selector.validate().is_err());
}

#[test]
fn canonicalizer_validation_checks_pattern_usage() {
    CanonicalizerSpec {
        kind: CanonicalizerKind::Trim,
        pattern: None,
        flags: Vec::new(),
    }
    .validate()
    .expect("trim");

    assert!(
        CanonicalizerSpec {
            kind: CanonicalizerKind::Trim,
            pattern: Some("x".to_owned()),
            flags: Vec::new(),
        }
        .validate()
        .is_err()
    );

    assert!(
        CanonicalizerSpec {
            kind: CanonicalizerKind::Trim,
            pattern: None,
            flags: vec![RegexFlag::CaseInsensitive],
        }
        .validate()
        .is_err()
    );

    CanonicalizerSpec {
        kind: CanonicalizerKind::StripRegex,
        pattern: Some(r"\d+".to_owned()),
        flags: vec![RegexFlag::CaseInsensitive],
    }
    .validate()
    .expect("strip regex");

    assert!(
        CanonicalizerSpec {
            kind: CanonicalizerKind::StripRegex,
            pattern: None,
            flags: Vec::new(),
        }
        .validate()
        .is_err()
    );

    assert!(
        CanonicalizerSpec {
            kind: CanonicalizerKind::StripRegex,
            pattern: Some("[".to_owned()),
            flags: Vec::new(),
        }
        .validate()
        .is_err()
    );
}

#[test]
fn file_targets_storage_and_notifications_validate_their_specific_contracts() {
    let mut file_target = valid_file_target();
    file_target.validate().expect("valid file target");

    file_target.target.source_url = Some(Url::parse("https://example.com").expect("url"));
    assert!(file_target.validate().is_err());

    let mut file_target = valid_file_target();
    file_target.target.file_path = None;
    assert!(file_target.validate().is_err());

    let mut target = valid_target();
    target.fetch.engine = FetchEngine::File;
    assert!(target.validate().is_err());

    let mut file_target = valid_file_target();
    file_target.fetch.engine = FetchEngine::Http;
    assert!(file_target.validate().is_err());

    let mut file_target = valid_file_target();
    file_target.fetch.follow_redirects = true;
    assert!(file_target.validate().is_err());

    let mut file_target = valid_file_target();
    file_target.fetch.timeout_ms = 20_000;
    assert!(file_target.validate().is_err());

    let mut file_target = valid_file_target();
    file_target.fetch.user_agent = "bogus-agent".to_owned();
    assert!(file_target.validate().is_err());

    let mut file_target = valid_file_target();
    file_target.fetch.accept = "text/html".to_owned();
    assert!(file_target.validate().is_err());

    let mut file_target = valid_file_target();
    file_target
        .fetch
        .headers
        .insert("x-demo".to_owned(), "value".to_owned());
    assert!(file_target.validate().is_err());

    let mut target = valid_target();
    target.target.source_url = None;
    assert!(target.validate().is_err());

    let mut target = valid_target();
    target.target.file_path = Some("/tmp/demo.html".to_owned());
    assert!(target.validate().is_err());

    let mut target = valid_target();
    target.storage.history_limit = 0;
    assert!(target.validate().is_err());

    let mut target = valid_target();
    target.notifications = vec![NotificationHook {
        name: "notify".to_owned(),
        on: vec![NotificationEvent::Changed],
        shell: "sh".to_owned(),
        command: "echo changed".to_owned(),
        timeout_ms: 500,
    }];
    assert!(target.validate().is_err());

    let mut target = valid_target();
    target.notifications = vec![
        NotificationHook {
            name: "notify".to_owned(),
            on: vec![NotificationEvent::Changed],
            shell: "/bin/sh".to_owned(),
            command: "echo changed".to_owned(),
            timeout_ms: 500,
        },
        NotificationHook {
            name: "notify".to_owned(),
            on: vec![NotificationEvent::FailedPermanent],
            shell: "/bin/sh".to_owned(),
            command: "echo failed".to_owned(),
            timeout_ms: 500,
        },
    ];
    assert!(target.validate().is_err());

    let mut target = valid_target();
    target.notifications = vec![NotificationHook {
        name: "notify".to_owned(),
        on: Vec::new(),
        shell: "/bin/sh".to_owned(),
        command: "echo changed".to_owned(),
        timeout_ms: 500,
    }];
    assert!(target.validate().is_err());

    let mut target = valid_target();
    target.notifications = vec![NotificationHook {
        name: "notify".to_owned(),
        on: vec![NotificationEvent::Changed],
        shell: "/bin/sh".to_owned(),
        command: "echo changed".to_owned(),
        timeout_ms: 99,
    }];
    assert!(target.validate().is_err());

    let mut target = valid_target();
    target.notifications = vec![NotificationHook {
        name: "notify".to_owned(),
        on: vec![NotificationEvent::Changed],
        shell: "/bin/sh".to_owned(),
        command: "echo changed".to_owned(),
        timeout_ms: 60_001,
    }];
    assert!(target.validate().is_err());

    NotificationHook {
        name: "notify".to_owned(),
        on: vec![NotificationEvent::Changed],
        shell: "/bin/sh".to_owned(),
        command: "echo changed".to_owned(),
        timeout_ms: 500,
    }
    .validate()
    .expect("valid notification hook");
}

#[test]
fn serde_defaults_fill_fetch_and_notification_fields() {
    let parsed: TargetDocument = toml::from_str(
        r#"
schema_name = "ffhn.target"
schema_version = 1
target_id = "demo"
display_name = "Demo"
enabled = true

[target]
kind = "file"
file_path = "/tmp/demo.html"

[fetch]
engine = "file"

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

[[notifications]]
name = "notify"
on = ["changed"]
command = "echo changed"
"#,
    )
    .expect("parse target");

    assert_eq!(parsed.fetch.method, HttpMethod::GET);
    assert_eq!(parsed.fetch.timeout_ms, 15_000);
    assert_eq!(parsed.fetch.max_bytes, 2_000_000);
    assert!(parsed.fetch.follow_redirects);
    assert_eq!(parsed.storage.history_limit, 10);
    assert_eq!(parsed.notifications[0].shell, "/bin/sh");
    assert_eq!(parsed.notifications[0].timeout_ms, 5_000);
}
