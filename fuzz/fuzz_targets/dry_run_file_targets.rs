#![no_main]

use arbitrary::{Arbitrary, Unstructured};
use ffhn_core::{TargetPaths, run_once_dry_run};
use libfuzzer_sys::fuzz_target;
use tempfile::tempdir;

#[derive(Debug)]
enum Strategy {
    Css,
    Delimiter,
}

#[derive(Debug)]
struct DryRunInput {
    html: String,
    target_id: String,
    strategy: Strategy,
    selector: String,
    delimiter_start: String,
    delimiter_end: String,
}

impl<'a> Arbitrary<'a> for Strategy {
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        Ok(if u.arbitrary::<bool>()? {
            Self::Css
        } else {
            Self::Delimiter
        })
    }
}

impl<'a> Arbitrary<'a> for DryRunInput {
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        Ok(Self {
            html: u.arbitrary::<String>()?,
            target_id: u.arbitrary::<String>()?,
            strategy: u.arbitrary()?,
            selector: u.arbitrary::<String>()?,
            delimiter_start: u.arbitrary::<String>()?,
            delimiter_end: u.arbitrary::<String>()?,
        })
    }
}

fuzz_target!(|input: DryRunInput| {
    let temp = tempdir().expect("tempdir");
    let target_id = safe_target_id(&input.target_id);
    let target_paths = TargetPaths::new(temp.path(), target_id.clone());
    let source_path = temp.path().join("source.html");
    std::fs::write(&source_path, normalize_html(&input.html)).expect("write source");
    std::fs::create_dir_all(target_paths.target_dir()).expect("target dir");
    std::fs::write(
        target_paths.target_file(),
        target_toml(
            &target_id,
            &source_path.to_string_lossy(),
            &input.strategy,
            &input.selector,
            &input.delimiter_start,
            &input.delimiter_end,
        ),
    )
    .expect("target toml");

    let _ = run_once_dry_run(&target_paths);
});

fn safe_target_id(raw: &str) -> String {
    let mut cleaned = raw
        .chars()
        .filter_map(|ch| {
            let lowered = ch.to_ascii_lowercase();
            if lowered.is_ascii_lowercase() || lowered.is_ascii_digit() {
                Some(lowered)
            } else if matches!(lowered, '-' | '_') {
                Some('-')
            } else {
                None
            }
        })
        .collect::<String>();
    if cleaned.is_empty() {
        cleaned.push_str("demo");
    }
    if !cleaned
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit())
    {
        cleaned.insert(0, 'd');
    }
    while cleaned.ends_with(['-', '_']) {
        cleaned.pop();
    }
    cleaned.truncate(32);
    if cleaned.is_empty() {
        "demo".to_owned()
    } else {
        cleaned
    }
}

fn normalize_html(raw: &str) -> String {
    if raw.trim().is_empty() {
        "<html><body><main>Hello</main></body></html>".to_owned()
    } else {
        raw.replace('\0', "")
    }
}

fn target_toml(
    target_id: &str,
    source_path: &str,
    strategy: &Strategy,
    selector: &str,
    delimiter_start: &str,
    delimiter_end: &str,
) -> String {
    let strategy_section = match strategy {
        Strategy::Css => format!(
            "[selection]\nkind = \"css_selector\"\nselector = \"{}\"\nmatch = \"single\"\noutput = \"outer_html\"\nwhitespace = \"normalize\"\nrewrite_urls = false\n",
            safe_selector(selector)
        ),
        Strategy::Delimiter => format!(
            "[selection]\nkind = \"delimiter_pair\"\nstart = \"{}\"\nend = \"{}\"\nmode = \"literal\"\ninclude_start = false\ninclude_end = true\nmatch = \"single\"\noutput = \"outer_html\"\nwhitespace = \"normalize\"\nrewrite_urls = false\n",
            safe_delimiter(delimiter_start, "BEGIN"),
            safe_delimiter(delimiter_end, "END")
        ),
    };

    format!(
        "schema_name = \"ffhn.target\"\nschema_version = 1\ntarget_id = \"{target_id}\"\ndisplay_name = \"Fuzz\"\nenabled = true\n\n[target]\nkind = \"file\"\nfile_path = {source_path:?}\n\n[fetch]\nengine = \"file\"\nfollow_redirects = false\nmax_bytes = 2000000\n\n{strategy_section}\n[compare]\nbasis = \"canonical_text_sha256\"\ncanonicalization = []\n"
    )
}

fn safe_selector(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        "main".to_owned()
    } else {
        trimmed.replace('"', "")
    }
}

fn safe_delimiter(raw: &str, fallback: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        fallback.to_owned()
    } else {
        trimmed.replace('"', "")
    }
}
