use super::*;
use crate::model::TargetKind;
use crate::{
    FetchEngine, SelectionKind, SelectionMatch, TARGET_SCHEMA_NAME, TARGET_SCHEMA_VERSION, TargetId,
};
use serde_json::json;
use std::collections::BTreeMap;
use std::num::NonZeroUsize;
use url::Url;

fn valid_target() -> TargetDocument {
    TargetDocument {
        schema_name: TARGET_SCHEMA_NAME.to_owned(),
        schema_version: TARGET_SCHEMA_VERSION,
        target_id: TargetId::new("demo").expect("target id"),
        display_name: "Demo".to_owned(),
        enabled: true,
        target: TargetSource::Http {
            source_url: Url::parse("https://example.com/page").expect("url"),
        },
        fetch: FetchConfig::Http(NetworkFetchConfig {
            method: HttpMethod::GET,
            timeout_ms: 15_000,
            max_bytes: 2_000_000,
            user_agent: "ffhn/example".to_owned(),
            follow_redirects: true,
            accept: "text/html".to_owned(),
            headers: BTreeMap::new(),
            extensions: None,
        }),
        selection: SelectionConfig::CssSelector {
            selection_mode: SelectionModeConfig::Single,
            selector: "main".to_owned(),
        },
        compare: CompareConfig {
            basis: CompareBasis::Text,
            whitespace: Some(WhitespaceMode::Normalize),
            rewrite_urls: false,
            canonicalization: Vec::new(),
        },
        storage: Default::default(),
        notification_endpoints: Vec::new(),
        notification_routes: Vec::new(),
        extensions: None,
    }
}

fn valid_file_target() -> TargetDocument {
    let mut target = valid_target();
    let file_path = std::env::temp_dir()
        .join("demo.html")
        .to_string_lossy()
        .into_owned();
    target.target = TargetSource::File { file_path };
    target.fetch = FetchConfig::File(FileFetchConfig {
        max_bytes: 2_000_000,
        extensions: None,
    });
    target
}

fn notification_route(
    name: &str,
    on: Vec<RunOutcome>,
    endpoint: impl Into<String>,
) -> NotificationRoute {
    NotificationRoute {
        name: name.to_owned(),
        on,
        endpoint: endpoint.into(),
    }
}

fn notification_endpoint(
    name: &str,
    program: impl Into<String>,
    args: Vec<&str>,
    timeout_ms: u64,
) -> NotificationEndpoint {
    NotificationEndpoint {
        name: name.to_owned(),
        adapter: NotificationAdapter::ProcessStdin {
            program: program.into(),
            args: args.into_iter().map(str::to_owned).collect(),
            timeout_ms,
        },
    }
}

#[test]
fn typed_target_and_fetch_accessors_and_raw_contracts_cover_all_variants() {
    let mut http_target = valid_target();
    if let FetchConfig::Http(fetch) = &mut http_target.fetch {
        fetch.headers.insert("x-demo".to_owned(), "demo".to_owned());
        fetch.extensions = Some(BTreeMap::from([(
            "demo".to_owned(),
            json!({"kind": "fetch"}),
        )]));
    }
    http_target.compare = CompareConfig {
        basis: CompareBasis::Text,
        whitespace: Some(WhitespaceMode::Normalize),
        rewrite_urls: false,
        canonicalization: vec![CanonicalizerSpec {
            kind: CanonicalizerKind::StripRegex,
            pattern: Some("demo".to_owned()),
            flags: vec![RegexFlag::CaseInsensitive, RegexFlag::MultiLine],
        }],
    };
    http_target.storage = StorageConfig { history_limit: 12 };
    #[cfg(unix)]
    let notification_program = "/bin/sh".to_owned();
    #[cfg(windows)]
    let notification_program = "C:/Windows/System32/cmd.exe".to_owned();
    http_target.notification_endpoints = vec![notification_endpoint(
        "notify-endpoint",
        notification_program.clone(),
        vec!["-c", "echo changed"],
        5_000,
    )];
    http_target.notification_routes = vec![notification_route(
        "notify",
        vec![RunOutcome::Changed],
        "notify-endpoint",
    )];
    http_target.extensions = Some(BTreeMap::from([(
        "demo".to_owned(),
        json!({"kind": "ext"}),
    )]));

    assert_eq!(http_target.schema_name(), TARGET_SCHEMA_NAME);
    assert_eq!(http_target.schema_version(), TARGET_SCHEMA_VERSION);
    assert_eq!(http_target.target_id(), "demo");
    assert_eq!(http_target.display_name(), "Demo");
    assert!(http_target.enabled());
    assert_eq!(http_target.target_kind(), TargetKind::Http);
    let source = http_target.source();
    assert_eq!(source.kind(), TargetKind::Http);
    match source {
        TargetSourceView::Http(source) => {
            assert_eq!(source.source_url().as_str(), "https://example.com/page");
        }
        TargetSourceView::File(_) => panic!("expected http source"),
    }
    assert_eq!(
        http_target.source_url().map(Url::as_str),
        Some("https://example.com/page")
    );
    assert_eq!(http_target.file_path(), None);
    let fetch = http_target.fetch_config();
    assert_eq!(fetch.engine(), FetchEngine::Http);
    assert_eq!(fetch.max_bytes(), 2_000_000);
    match fetch {
        FetchConfigView::Http(fetch) => {
            assert_eq!(fetch.method(), HttpMethod::GET);
            assert_eq!(fetch.timeout_ms(), 15_000);
            assert_eq!(fetch.max_bytes(), 2_000_000);
            assert_eq!(fetch.user_agent(), "ffhn/example");
            assert!(fetch.follow_redirects());
            assert_eq!(fetch.accept(), "text/html");
            assert_eq!(
                fetch.headers().get("x-demo").map(String::as_str),
                Some("demo")
            );
            assert_eq!(
                fetch.extensions().expect("fetch extensions").get("demo"),
                Some(&json!({"kind": "fetch"}))
            );
        }
        FetchConfigView::File(_) => panic!("expected http fetch"),
    }
    assert_eq!(http_target.fetch_engine(), FetchEngine::Http);
    assert_eq!(http_target.fetch_max_bytes(), 2_000_000);
    assert_eq!(http_target.fetch_http_method(), Some(HttpMethod::GET));
    assert_eq!(http_target.fetch_timeout_ms(), Some(15_000));
    assert_eq!(http_target.fetch_user_agent(), Some("ffhn/example"));
    assert_eq!(http_target.fetch_follow_redirects(), Some(true));
    assert_eq!(http_target.fetch_accept(), Some("text/html"));
    assert_eq!(
        http_target
            .fetch_headers()
            .expect("http headers")
            .get("x-demo")
            .map(String::as_str),
        Some("demo")
    );
    assert_eq!(
        http_target
            .fetch_extensions()
            .expect("fetch extensions")
            .get("demo"),
        Some(&json!({"kind": "fetch"}))
    );
    assert_eq!(http_target.compare_basis(), CompareBasis::Text);
    assert_eq!(http_target.compare_canonicalization().len(), 1);
    let canonicalizer = &http_target.compare_canonicalization()[0];
    assert_eq!(canonicalizer.kind(), CanonicalizerKind::StripRegex);
    assert_eq!(canonicalizer.pattern(), Some("demo"));
    assert_eq!(
        canonicalizer.flags(),
        [RegexFlag::CaseInsensitive, RegexFlag::MultiLine]
    );
    assert_eq!(http_target.storage_history_limit(), 12);
    let selection = http_target.selection();
    assert_eq!(selection.kind(), SelectionKind::CssSelector);
    match selection {
        SelectionConfigView::CssSelector(selection) => {
            assert_eq!(selection.selection_mode(), SelectionModeView::Single);
            assert_eq!(selection.selector(), "main");
        }
        SelectionConfigView::DelimiterPair(_) => panic!("expected css selector"),
    }
    assert_eq!(http_target.selection_kind(), SelectionKind::CssSelector);
    assert_eq!(http_target.selection_match(), SelectionMatch::Single);
    assert_eq!(http_target.selection_index(), None);
    assert_eq!(http_target.selection_selector(), Some("main"));
    assert_eq!(http_target.selection_start(), None);
    assert_eq!(http_target.selection_end(), None);
    assert_eq!(http_target.selection_delimiter_mode(), None);
    assert_eq!(http_target.selection_include_start(), None);
    assert_eq!(http_target.selection_include_end(), None);
    assert!(http_target.selection_regex_flags().is_empty());
    let notifications = http_target.notifications().collect::<Vec<_>>();
    assert_eq!(notifications.len(), 1);
    let notification = notifications[0];
    assert_eq!(notification.name(), "notify");
    assert_eq!(notification.on(), [RunOutcome::Changed]);
    assert_eq!(notification.endpoint(), "notify-endpoint");
    assert_eq!(notification.transport_kind(), "process_stdin");
    assert_eq!(notification.program(), notification_program);
    assert_eq!(
        notification.args(),
        ["-c".to_owned(), "echo changed".to_owned()]
    );
    assert_eq!(notification.timeout_ms(), 5_000);
    assert_eq!(
        http_target.extensions().expect("extensions").get("demo"),
        Some(&json!({"kind": "ext"}))
    );
    let http_round_trip: TargetDocument =
        toml::from_str(&toml::to_string(&http_target).expect("http target toml"))
            .expect("http target round-trip");
    assert_eq!(http_round_trip.fetch().engine(), FetchEngine::Http);
    let compare = http_target.compare_config();
    assert_eq!(compare.basis(), CompareBasis::Text);
    assert_eq!(compare.whitespace(), Some(WhitespaceMode::Normalize));
    assert!(!compare.rewrite_urls());
    assert_eq!(compare.canonicalization().len(), 1);

    let file_target = valid_file_target();
    let expected_file_path = std::env::temp_dir()
        .join("demo.html")
        .to_string_lossy()
        .into_owned();
    assert_eq!(file_target.source().kind(), TargetKind::File);
    match file_target.source() {
        TargetSourceView::File(source) => {
            assert_eq!(source.file_path(), expected_file_path.as_str());
        }
        TargetSourceView::Http(_) => panic!("expected file source"),
    }
    assert_eq!(file_target.target_kind(), TargetKind::File);
    assert!(file_target.source_url().is_none());
    assert_eq!(file_target.file_path(), Some(expected_file_path.as_str()));
    match file_target.fetch_config() {
        FetchConfigView::File(fetch) => {
            assert_eq!(fetch.max_bytes(), 2_000_000);
            assert!(fetch.extensions().is_none());
        }
        FetchConfigView::Http(_) => panic!("expected file fetch"),
    }
    assert_eq!(file_target.fetch_engine(), FetchEngine::File);
    assert_eq!(file_target.fetch_max_bytes(), 2_000_000);
    assert_eq!(file_target.fetch_http_method(), None);
    assert_eq!(file_target.fetch_timeout_ms(), None);
    assert_eq!(file_target.fetch_user_agent(), None);
    assert_eq!(file_target.fetch_follow_redirects(), None);
    assert_eq!(file_target.fetch_accept(), None);
    assert_eq!(file_target.fetch_headers(), None);
    assert!(file_target.fetch_extensions().is_none());
    let file_fetch = file_target.fetch_config();
    assert_eq!(file_fetch.engine(), FetchEngine::File);
    assert_eq!(file_fetch.max_bytes(), 2_000_000);
    let file_round_trip: TargetDocument =
        toml::from_str(&toml::to_string(&file_target).expect("file target toml"))
            .expect("file target round-trip");
    assert_eq!(file_round_trip.fetch_engine(), FetchEngine::File);

    let mut delimiter_target = valid_target();
    delimiter_target.selection = SelectionConfig::DelimiterPair {
        selection_mode: SelectionModeConfig::Nth {
            index: NonZeroUsize::new(2).expect("nonzero index"),
        },
        start: "<main>".to_owned(),
        end: "</main>".to_owned(),
        mode: DelimiterMode::Regex,
        include_start: true,
        include_end: false,
        flags: vec![RegexFlag::DotMatchesNewLine],
    };
    delimiter_target.compare = CompareConfig {
        basis: CompareBasis::InnerHtml,
        whitespace: None,
        rewrite_urls: true,
        canonicalization: Vec::new(),
    };
    assert_eq!(
        delimiter_target.selection_kind(),
        SelectionKind::DelimiterPair
    );
    assert_eq!(
        delimiter_target.selection().kind(),
        SelectionKind::DelimiterPair
    );
    match delimiter_target.selection() {
        SelectionConfigView::DelimiterPair(selection) => {
            assert_eq!(
                selection.selection_mode(),
                SelectionModeView::Nth { index: 2 }
            );
            assert_eq!(
                selection.selection_mode().selection_match(),
                SelectionMatch::Nth
            );
            assert_eq!(selection.selection_mode().index(), Some(2));
            assert_eq!(selection.start(), "<main>");
            assert_eq!(selection.end(), "</main>");
            assert_eq!(selection.delimiter_mode(), DelimiterMode::Regex);
            assert!(selection.include_start());
            assert!(!selection.include_end());
            assert_eq!(selection.regex_flags(), [RegexFlag::DotMatchesNewLine]);
        }
        SelectionConfigView::CssSelector(_) => panic!("expected delimiter selection"),
    }
    assert_eq!(delimiter_target.selection_match(), SelectionMatch::Nth);
    assert_eq!(delimiter_target.selection_index(), Some(2));
    assert_eq!(delimiter_target.compare_basis(), CompareBasis::InnerHtml);
    assert_eq!(delimiter_target.compare_whitespace(), None);
    assert!(delimiter_target.compare_rewrite_urls());
    assert_eq!(delimiter_target.selection_selector(), None);
    assert_eq!(delimiter_target.selection_start(), Some("<main>"));
    assert_eq!(delimiter_target.selection_end(), Some("</main>"));
    assert_eq!(
        delimiter_target.selection_delimiter_mode(),
        Some(DelimiterMode::Regex)
    );
    assert_eq!(delimiter_target.selection_include_start(), Some(true));
    assert_eq!(delimiter_target.selection_include_end(), Some(false));
    assert_eq!(
        delimiter_target.selection_regex_flags(),
        [RegexFlag::DotMatchesNewLine]
    );

    let mut first_match_target = valid_target();
    first_match_target.selection = SelectionConfig::CssSelector {
        selection_mode: SelectionModeConfig::First,
        selector: "article".to_owned(),
    };
    first_match_target.compare = CompareConfig {
        basis: CompareBasis::Text,
        whitespace: Some(WhitespaceMode::Preserve),
        rewrite_urls: false,
        canonicalization: Vec::new(),
    };
    match first_match_target.selection() {
        SelectionConfigView::CssSelector(selection) => {
            assert_eq!(selection.selection_mode(), SelectionModeView::First);
            assert_eq!(
                selection.selection_mode().selection_match(),
                SelectionMatch::First
            );
            assert_eq!(selection.selection_mode().index(), None);
        }
        SelectionConfigView::DelimiterPair(_) => panic!("expected css selector"),
    }
    assert_eq!(
        SelectionModeView::Single.selection_match(),
        SelectionMatch::Single
    );
    assert_eq!(SelectionModeView::Single.index(), None);

    assert!(
        toml::from_str::<TargetDocument>(
            r#"
schema_name = "ffhn.target"
schema_version = 4
target_id = "demo"
display_name = "Demo"
enabled = true

[target]
kind = "http"
source_url = "https://example.com/page"
file_path = "/tmp/demo.html"

[fetch]
engine = "http"
user_agent = "ffhn/example"
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
"#,
        )
        .is_err()
    );

    assert!(
        toml::from_str::<TargetDocument>(
            r#"
schema_name = "ffhn.target"
schema_version = 4
target_id = "demo"
display_name = "Demo"
enabled = true

[target]
kind = "file"
source_url = "https://example.com/page"
file_path = "/tmp/demo.html"

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
"#,
        )
        .is_err()
    );

    assert!(
        toml::from_str::<TargetDocument>(
            r#"
schema_name = "ffhn.target"
schema_version = 4
target_id = "demo"
display_name = "Demo"
enabled = true

[target]
kind = "file"

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
"#,
        )
        .is_err()
    );

    for invalid_fetch in [
        r#"
[fetch]
engine = "file"
method = "GET"
max_bytes = 2000000
"#,
        r#"
[fetch]
engine = "file"
user_agent = "ffhn/example"
max_bytes = 2000000
"#,
        r#"
[fetch]
engine = "file"
accept = "text/html"
max_bytes = 2000000
"#,
        r#"
[fetch]
engine = "file"
max_bytes = 2000000

[fetch.headers]
x-demo = "demo"
"#,
    ] {
        let document = format!(
            r#"
schema_name = "ffhn.target"
schema_version = 4
target_id = "demo"
display_name = "Demo"
enabled = true

[target]
kind = "file"
file_path = "/tmp/demo.html"

{invalid_fetch}
[selection]
kind = "css_selector"
selector = "main"
match = "single"

[compare]
basis = "text"
whitespace = "normalize"
rewrite_urls = false
canonicalization = []
"#,
        );
        assert!(toml::from_str::<TargetDocument>(&document).is_err());
    }
}

fn parse_target_with_selection(selection: &str) -> Result<TargetDocument, toml::de::Error> {
    toml::from_str(&format!(
        r#"
schema_name = "ffhn.target"
schema_version = 4
target_id = "demo"
display_name = "Demo"
enabled = true

[target]
kind = "http"
source_url = "https://example.com/page"

[fetch]
engine = "http"
user_agent = "ffhn/example"
accept = "text/html"

[selection]
{selection}

[compare]
basis = "text"
whitespace = "normalize"
rewrite_urls = false
canonicalization = []
"#
    ))
}

#[test]
fn valid_css_selector_target_document_passes_validation() {
    valid_target().validate().expect("valid target");

    TargetDocument {
        compare: CompareConfig {
            basis: CompareBasis::Text,
            whitespace: Some(WhitespaceMode::Normalize),
            rewrite_urls: false,
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
    target.target = TargetSource::Http {
        source_url: Url::parse("file:///tmp/demo").expect("file url"),
    };
    assert!(target.validate().is_err());

    let mut target = valid_target();
    if let FetchConfig::Http(fetch) = &mut target.fetch {
        fetch.timeout_ms = 999;
    }
    assert!(target.validate().is_err());

    let mut target = valid_target();
    if let FetchConfig::Http(fetch) = &mut target.fetch {
        fetch.max_bytes = 100;
    }
    assert!(target.validate().is_err());

    let mut target = valid_target();
    if let FetchConfig::Http(fetch) = &mut target.fetch {
        fetch.timeout_ms = 600_001;
    }
    assert!(target.validate().is_err());

    let mut target = valid_target();
    if let FetchConfig::Http(fetch) = &mut target.fetch {
        fetch.max_bytes = 104_857_601;
    }
    assert!(target.validate().is_err());

    let mut target = valid_target();
    if let FetchConfig::Http(fetch) = &mut target.fetch {
        fetch.headers.insert("".to_owned(), "value".to_owned());
    }
    assert!(target.validate().is_err());

    let mut target = valid_target();
    if let FetchConfig::Http(fetch) = &mut target.fetch {
        fetch.headers.insert("x-demo".to_owned(), "".to_owned());
    }
    assert!(target.validate().is_err());

    CompareConfig {
        basis: CompareBasis::Text,
        whitespace: Some(WhitespaceMode::Normalize),
        rewrite_urls: false,
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
    let selection = SelectionConfig::CssSelector {
        selection_mode: SelectionModeConfig::Nth {
            index: NonZeroUsize::new(2).expect("non-zero index"),
        },
        selector: "main".to_owned(),
    };
    selection.validate().expect("nth with index");

    assert!(
        parse_target_with_selection(
            r#"
kind = "css_selector"
selector = "main"
match = "nth"
"#,
        )
        .is_err()
    );

    assert!(
        parse_target_with_selection(
            r#"
kind = "css_selector"
selector = "main"
match = "nth"
index = 0
"#,
        )
        .is_err()
    );

    assert!(
        parse_target_with_selection(
            r#"
kind = "css_selector"
selector = "main"
match = "first"
index = 2
"#,
        )
        .is_err()
    );

    assert!(
        parse_target_with_selection(
            r#"
kind = "css_selector"
match = "single"
"#,
        )
        .is_err()
    );

    assert!(
        parse_target_with_selection(
            r#"
kind = "css_selector"
selector = "main"
match = "single"
index = 2
"#,
        )
        .is_err()
    );
}

#[test]
fn compare_config_validation_enforces_basis_specific_whitespace_rules() {
    CompareConfig {
        basis: CompareBasis::Text,
        whitespace: Some(WhitespaceMode::Normalize),
        rewrite_urls: false,
        canonicalization: Vec::new(),
    }
    .validate()
    .expect("text compare with whitespace");

    assert!(
        CompareConfig {
            basis: CompareBasis::Text,
            whitespace: None,
            rewrite_urls: false,
            canonicalization: Vec::new(),
        }
        .validate()
        .is_err()
    );

    assert!(
        CompareConfig {
            basis: CompareBasis::InnerHtml,
            whitespace: Some(WhitespaceMode::Normalize),
            rewrite_urls: false,
            canonicalization: Vec::new(),
        }
        .validate()
        .is_err()
    );

    assert!(
        CompareConfig {
            basis: CompareBasis::OuterHtml,
            whitespace: Some(WhitespaceMode::Normalize),
            rewrite_urls: false,
            canonicalization: Vec::new(),
        }
        .validate()
        .is_err()
    );
}

#[test]
fn css_selection_rejects_delimiter_fields_and_invalid_selectors() {
    assert!(
        parse_target_with_selection(
            r#"
kind = "css_selector"
selector = "main"
start = "BEGIN"
match = "single"
"#,
        )
        .is_err()
    );

    assert!(
        parse_target_with_selection(
            r#"
kind = "css_selector"
selector = "main"
end = "END"
match = "single"
"#,
        )
        .is_err()
    );

    assert!(
        parse_target_with_selection(
            r#"
kind = "css_selector"
selector = "main"
mode = "regex"
match = "single"
"#,
        )
        .is_err()
    );

    assert!(
        parse_target_with_selection(
            r#"
kind = "css_selector"
selector = "main"
include_start = true
match = "single"
"#,
        )
        .is_err()
    );

    assert!(
        parse_target_with_selection(
            r#"
kind = "css_selector"
selector = "main"
include_end = true
match = "single"
"#,
        )
        .is_err()
    );

    assert!(
        parse_target_with_selection(
            r#"
kind = "css_selector"
selector = "main"
flags = ["case_insensitive"]
match = "single"
"#,
        )
        .is_err()
    );

    let mut selection = valid_target().selection;
    if let SelectionConfig::CssSelector { selector, .. } = &mut selection {
        *selector = "main[".to_owned();
    }
    assert!(selection.validate().is_err());
}

#[test]
fn delimiter_selection_requires_its_full_contract() {
    let selection = SelectionConfig::DelimiterPair {
        selection_mode: SelectionModeConfig::Nth {
            index: NonZeroUsize::new(1).expect("non-zero index"),
        },
        start: "BEGIN".to_owned(),
        end: "END".to_owned(),
        mode: DelimiterMode::Regex,
        include_start: false,
        include_end: true,
        flags: vec![RegexFlag::CaseInsensitive],
    };
    selection.validate().expect("valid delimiter selection");

    assert!(
        parse_target_with_selection(
            r#"
kind = "delimiter_pair"
start = "BEGIN"
end = "END"
include_start = false
include_end = true
match = "nth"
index = 1
"#,
        )
        .is_err()
    );

    assert!(
        parse_target_with_selection(
            r#"
kind = "delimiter_pair"
start = "BEGIN"
end = "END"
mode = "regex"
include_start = false
match = "nth"
index = 1
"#,
        )
        .is_err()
    );

    assert!(
        parse_target_with_selection(
            r#"
kind = "delimiter_pair"
start = "BEGIN"
end = "END"
mode = "regex"
include_end = true
match = "nth"
index = 1
"#,
        )
        .is_err()
    );

    assert!(
        parse_target_with_selection(
            r#"
kind = "delimiter_pair"
selector = "main"
start = "BEGIN"
end = "END"
mode = "regex"
include_start = false
include_end = true
match = "nth"
index = 1
"#,
        )
        .is_err()
    );

    let mut literal_with_flags = selection.clone();
    if let SelectionConfig::DelimiterPair { mode, .. } = &mut literal_with_flags {
        *mode = DelimiterMode::Literal;
    }
    assert!(literal_with_flags.validate().is_err());

    let mut literal_without_flags = selection.clone();
    if let SelectionConfig::DelimiterPair { mode, flags, .. } = &mut literal_without_flags {
        *mode = DelimiterMode::Literal;
        *flags = Vec::new();
    }
    literal_without_flags
        .validate()
        .expect("literal delimiter without regex flags");
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

    file_target.target = TargetSource::Http {
        source_url: Url::parse("https://example.com").expect("url"),
    };
    assert!(file_target.validate().is_err());

    let mut file_target = valid_file_target();
    file_target.target = TargetSource::File {
        file_path: String::new(),
    };
    assert!(file_target.validate().is_err());

    let mut target = valid_target();
    target.fetch = FetchConfig::File(FileFetchConfig {
        max_bytes: 2_000_000,
        extensions: None,
    });
    assert!(target.validate().is_err());

    let mut file_target = valid_file_target();
    file_target.fetch = FetchConfig::Http(NetworkFetchConfig {
        method: HttpMethod::GET,
        timeout_ms: 15_000,
        max_bytes: 2_000_000,
        user_agent: "ffhn/example".to_owned(),
        follow_redirects: true,
        accept: "text/html".to_owned(),
        headers: BTreeMap::new(),
        extensions: None,
    });
    assert!(file_target.validate().is_err());

    let mut file_target = valid_file_target();
    file_target.fetch = FetchConfig::File(FileFetchConfig {
        max_bytes: 100,
        extensions: None,
    });
    assert!(file_target.validate().is_err());

    assert!(
        toml::from_str::<TargetDocument>(
            r#"
schema_name = "ffhn.target"
schema_version = 4
target_id = "demo"
display_name = "Demo"
enabled = true

[target]
kind = "http"

[fetch]
engine = "http"
user_agent = "ffhn/example"
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
"#,
        )
        .is_err()
    );

    let mut target = valid_target();
    target.target = TargetSource::File {
        file_path: "/tmp/demo.html".to_owned(),
    };
    assert!(target.validate().is_err());

    let mut target = valid_target();
    target.storage.history_limit = 0;
    assert!(target.validate().is_err());

    let mut target = valid_target();
    target.notification_endpoints = vec![notification_endpoint(
        "notify-endpoint",
        "sh",
        vec!["-c", "echo changed"],
        500,
    )];
    target.notification_routes = vec![notification_route(
        "notify",
        vec![RunOutcome::Changed],
        "notify-endpoint",
    )];
    assert!(target.validate().is_err());

    let mut target = valid_target();
    target.notification_endpoints = vec![notification_endpoint(
        "notify-endpoint",
        "/bin/sh",
        vec!["-c", "echo changed"],
        500,
    )];
    target.notification_routes = vec![
        notification_route("notify", vec![RunOutcome::Changed], "notify-endpoint"),
        notification_route(
            "notify",
            vec![RunOutcome::FailedPermanent],
            "notify-endpoint",
        ),
    ];
    assert!(target.validate().is_err());

    let mut target = valid_target();
    target.notification_routes = vec![notification_route("notify", Vec::new(), "notify-endpoint")];
    assert!(target.validate().is_err());

    let mut target = valid_target();
    target.notification_endpoints = vec![
        notification_endpoint("notify-endpoint", "sh", vec!["-c", "echo changed"], 500),
        notification_endpoint("notify-endpoint", "sh", vec!["-c", "echo changed"], 500),
    ];
    assert!(target.validate().is_err());

    let mut target = valid_target();
    target.notification_routes = vec![notification_route(
        "notify",
        vec![RunOutcome::Changed],
        "missing-endpoint",
    )];
    assert!(target.validate().is_err());

    let mut target = valid_target();
    target.notification_endpoints = vec![notification_endpoint(
        "notify-endpoint",
        "/bin/sh",
        vec!["-c", "echo changed"],
        500,
    )];
    target.notification_routes = vec![notification_route(
        "notify",
        vec![RunOutcome::Changed, RunOutcome::Changed],
        "notify-endpoint",
    )];
    assert!(target.validate().is_err());

    let mut target = valid_target();
    target.notification_endpoints = vec![notification_endpoint(
        "notify-endpoint",
        "/bin/sh",
        vec![""],
        500,
    )];
    target.notification_routes = vec![notification_route(
        "notify",
        vec![RunOutcome::Changed],
        "notify-endpoint",
    )];
    assert!(target.validate().is_err());

    let mut target = valid_target();
    target.notification_endpoints = vec![notification_endpoint(
        "notify-endpoint",
        "/bin/sh",
        vec!["-c", "echo changed"],
        99,
    )];
    target.notification_routes = vec![notification_route(
        "notify",
        vec![RunOutcome::Changed],
        "notify-endpoint",
    )];
    assert!(target.validate().is_err());

    let mut target = valid_target();
    target.notification_endpoints = vec![notification_endpoint(
        "notify-endpoint",
        "/bin/sh",
        vec!["-c", "echo changed"],
        60_001,
    )];
    target.notification_routes = vec![notification_route(
        "notify",
        vec![RunOutcome::Changed],
        "notify-endpoint",
    )];
    assert!(target.validate().is_err());

    #[cfg(unix)]
    let valid_program = "/bin/sh";
    #[cfg(windows)]
    let valid_program = "C:/Windows/System32/cmd.exe";
    notification_endpoint(
        "notify-endpoint",
        valid_program,
        vec!["-c", "echo changed"],
        500,
    )
    .validate()
    .expect("valid notification endpoint");
    notification_route("notify", vec![RunOutcome::Changed], "notify-endpoint")
        .validate()
        .expect("valid notification route");
}

#[test]
fn serde_defaults_fill_http_fetch_storage_and_notification_fields() {
    #[cfg(unix)]
    let notification_program = "/bin/sh";
    #[cfg(windows)]
    let notification_program = "C:/Windows/System32/cmd.exe";

    let parsed: TargetDocument = toml::from_str(&format!(
        r#"
schema_name = "ffhn.target"
schema_version = 4
target_id = "demo"
display_name = "Demo"
enabled = true

[target]
kind = "http"
source_url = "https://example.com/demo"

[fetch]
engine = "http"
user_agent = "ffhn/example"
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

[[notification_endpoints]]
name = "notify-endpoint"
kind = "process_stdin"
program = {notification_program:?}

[[notification_routes]]
name = "notify"
on = ["changed"]
endpoint = "notify-endpoint"
"#
    ))
    .expect("parse target");

    match parsed.fetch() {
        FetchConfig::Http(fetch) => {
            assert_eq!(fetch.method, HttpMethod::GET);
            assert_eq!(fetch.timeout_ms, 15_000);
            assert_eq!(fetch.max_bytes, 2_000_000);
            assert!(fetch.follow_redirects);
        }
        other => panic!("expected http fetch config, got {other:?}"),
    }
    assert_eq!(parsed.storage.history_limit, 10);
    match &parsed.notification_endpoints[0].adapter {
        NotificationAdapter::ProcessStdin {
            program,
            args,
            timeout_ms,
        } => {
            assert_eq!(program, notification_program);
            assert!(args.is_empty());
            assert_eq!(*timeout_ms, 5_000);
        }
    }
}

#[test]
fn selection_config_serialization_round_trips_all_typed_shapes() {
    let cases = vec![
        valid_target().selection.clone(),
        SelectionConfig::CssSelector {
            selection_mode: SelectionModeConfig::First,
            selector: "article".to_owned(),
        },
        SelectionConfig::DelimiterPair {
            selection_mode: SelectionModeConfig::Nth {
                index: NonZeroUsize::new(2).expect("non-zero index"),
            },
            start: "BEGIN".to_owned(),
            end: "END".to_owned(),
            mode: DelimiterMode::Literal,
            include_start: false,
            include_end: true,
            flags: Vec::new(),
        },
    ];

    for selection in cases {
        let encoded = toml::to_string(&selection).expect("serialize selection");
        let decoded: SelectionConfig = toml::from_str(&encoded).expect("deserialize selection");
        assert_eq!(decoded, selection);
    }
}

#[test]
fn file_targets_use_the_typed_file_fetch_shape_when_deserialized() {
    let test_file_path = std::env::temp_dir().join("demo.html");
    let parsed = toml::from_str::<TargetDocument>(&format!(
        r#"
schema_name = "ffhn.target"
schema_version = 4
target_id = "demo"
display_name = "Demo"
enabled = true

[target]
kind = "file"
file_path = {test_file_path:?}

[fetch]
engine = "file"

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
    ))
    .expect("parse file target");

    match parsed.fetch() {
        FetchConfig::File(fetch) => {
            assert_eq!(fetch.max_bytes, 2_000_000);
        }
        other => panic!("expected file fetch config, got {other:?}"),
    }

    assert!(
        toml::from_str::<TargetDocument>(
            r#"
schema_name = "ffhn.target"
schema_version = 4
target_id = "demo"
display_name = "Demo"
enabled = true

[target]
kind = "file"
file_path = "/tmp/demo.html"

[fetch]
engine = "file"
follow_redirects = false

[selection]
kind = "css_selector"
selector = "main"
match = "single"

[compare]
basis = "text"
whitespace = "normalize"
rewrite_urls = false
canonicalization = []
"#,
        )
        .is_err()
    );
}
