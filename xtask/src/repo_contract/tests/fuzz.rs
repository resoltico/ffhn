use std::ffi::OsStr;

use super::*;
use ffhn_core::{BatchRunReport, NotificationPayload, RunReport, StateDocument, StatusReport};

fn seed_paths(dir: &Path) -> Vec<PathBuf> {
    let mut seeds = fs::read_dir(dir)
        .expect("read seed dir")
        .map(|entry| entry.expect("seed entry").path())
        .filter(|path| path.extension() == Some(OsStr::new("seed")))
        .collect::<Vec<_>>();
    seeds.sort();
    seeds
}

fn is_intentionally_invalid_seed(path: &Path) -> bool {
    path.file_stem()
        .and_then(OsStr::to_str)
        .is_some_and(|stem| stem.contains("invalid"))
}

#[cfg(unix)]
#[test]
fn maintained_report_and_target_seed_files_stay_valid() {
    let repo_root = repo_root();

    for path in seed_paths(&repo_root.join("fuzz/corpus/target_toml_documents")) {
        let document = fs::read_to_string(&path).expect("read target seed");
        let parsed = toml::from_str::<TargetDocument>(&document);
        if is_intentionally_invalid_seed(&path) {
            assert!(
                match parsed.as_ref() {
                    Ok(target) => target.validate().is_err(),
                    Err(_) => true,
                },
                "{}",
                path.display()
            );
        } else {
            assert!(
                parsed
                    .as_ref()
                    .is_ok_and(|target| target.validate().is_ok()),
                "{}",
                path.display()
            );
        }
    }

    for path in seed_paths(&repo_root.join("fuzz/corpus/state_and_report_json_documents")) {
        let document = fs::read_to_string(&path).expect("read json seed");
        let valid = serde_json::from_str::<StateDocument>(&document)
            .is_ok_and(|state| state.validate().is_ok())
            || serde_json::from_str::<RunReport>(&document)
                .is_ok_and(|report| report.validate().is_ok())
            || serde_json::from_str::<NotificationPayload>(&document)
                .is_ok_and(|payload| payload.validate().is_ok())
            || serde_json::from_str::<BatchRunReport>(&document)
                .is_ok_and(|report| report.validate().is_ok())
            || serde_json::from_str::<StatusReport>(&document)
                .is_ok_and(|report| report.validate().is_ok());
        if is_intentionally_invalid_seed(&path) {
            assert!(!valid, "{}", path.display());
        } else {
            assert!(valid, "{}", path.display());
        }
    }
}
