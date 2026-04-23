use std::fs;
use std::path::Path;
use std::time::Instant;

use url::Url;

use crate::canonical::normalize_line_endings;
use crate::{FetchEngine, ReasonCode, RunFetchSection, TargetDocument};

use super::{FetchFailure, FetchSuccess, config_invalid_failure};

pub(super) fn fetch_file_target(target: &TargetDocument) -> Result<FetchSuccess, FetchFailure> {
    let started = Instant::now();
    let fetch = &target.fetch;
    let path = validated_file_path(target)?;

    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(_) => {
            return Err(FetchFailure {
                reason_code: ReasonCode::FetchSourceError,
                report: RunFetchSection {
                    engine: FetchEngine::File,
                    final_url: None,
                    http_status: None,
                    content_type: None,
                    bytes_read: None,
                    duration_ms: started.elapsed().as_millis() as u64,
                },
            });
        }
    };

    if bytes.len() > fetch.max_bytes {
        return Err(FetchFailure {
            reason_code: ReasonCode::FetchTooLarge,
            report: RunFetchSection {
                engine: FetchEngine::File,
                final_url: None,
                http_status: None,
                content_type: None,
                bytes_read: Some(bytes.len()),
                duration_ms: started.elapsed().as_millis() as u64,
            },
        });
    }

    let html = match std::str::from_utf8(&bytes) {
        Ok(text) => normalize_line_endings(text),
        Err(_) => {
            return Err(FetchFailure {
                reason_code: ReasonCode::FetchDecodeError,
                report: RunFetchSection {
                    engine: FetchEngine::File,
                    final_url: None,
                    http_status: None,
                    content_type: None,
                    bytes_read: Some(bytes.len()),
                    duration_ms: started.elapsed().as_millis() as u64,
                },
            });
        }
    };

    let canonical_path = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let final_url = validated_file_url(&canonical_path, started.elapsed().as_millis() as u64)?;

    Ok(FetchSuccess {
        final_url: final_url.clone(),
        html,
        report: RunFetchSection {
            engine: FetchEngine::File,
            final_url: Some(final_url.to_string()),
            http_status: None,
            content_type: None,
            bytes_read: Some(bytes.len()),
            duration_ms: started.elapsed().as_millis() as u64,
        },
    })
}

fn validated_file_path(target: &TargetDocument) -> Result<&Path, FetchFailure> {
    let path = target
        .target
        .file_path
        .as_deref()
        .map(Path::new)
        .ok_or_else(|| config_invalid_failure(target.fetch.engine, 0))?;
    if !path.is_absolute() {
        return Err(config_invalid_failure(target.fetch.engine, 0));
    }
    Ok(path)
}

fn validated_file_url(path: &Path, duration_ms: u64) -> Result<Url, FetchFailure> {
    Url::from_file_path(path).map_err(|_| config_invalid_failure(FetchEngine::File, duration_ms))
}
