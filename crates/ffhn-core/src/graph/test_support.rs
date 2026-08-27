//! Cross-platform paths and process adapters used only by observation-graph tests.

#[derive(Clone, Debug)]
pub(super) struct ProcessFixture {
    pub(super) program: String,
    pub(super) args: Vec<String>,
}

pub(super) fn absolute_file_path(name: &str) -> String {
    std::env::temp_dir()
        .join("ffhn-graph-tests")
        .join(name)
        .to_string_lossy()
        .into_owned()
}

pub(super) fn successful_process() -> ProcessFixture {
    process_fixture(true)
}

pub(super) fn failing_process() -> ProcessFixture {
    process_fixture(false)
}

pub(super) fn process_adapter_toml(success: bool, timeout_ms: u64) -> String {
    let fixture = process_fixture(success);
    let args = fixture
        .args
        .iter()
        .map(|argument| format!("{argument:?}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "kind = \"process_stdin\"\nprogram = {:?}\nargs = [{args}]\ntimeout_ms = {timeout_ms}\n",
        fixture.program,
    )
}

#[cfg(unix)]
fn process_fixture(success: bool) -> ProcessFixture {
    if success {
        ProcessFixture {
            program: "/bin/sh".to_owned(),
            args: vec!["-c".to_owned(), "cat >/dev/null".to_owned()],
        }
    } else {
        ProcessFixture {
            program: "/usr/bin/false".to_owned(),
            args: Vec::new(),
        }
    }
}

#[cfg(windows)]
fn process_fixture(success: bool) -> ProcessFixture {
    ProcessFixture {
        program: std::env::var("COMSPEC")
            .unwrap_or_else(|_| r"C:\Windows\System32\cmd.exe".to_owned()),
        args: vec![
            "/C".to_owned(),
            if success {
                "more >NUL".to_owned()
            } else {
                "exit 1".to_owned()
            },
        ],
    }
}
