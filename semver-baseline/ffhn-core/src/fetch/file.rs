use std::fs::{self, File};
use std::io::Read;
use std::path::Path;
use std::time::Instant;

use url::Url;

use crate::canonical::normalize_line_endings;
use crate::time::elapsed_ms;
use crate::{
    FetchEngine, FileFetchConfig, ProcessErrorDetail, ProcessErrorKind, RunFailureCause,
    RunFetchSection, TargetDocument,
};

use super::{FetchFailure, FetchResult, FetchSuccess, config_invalid_failure};

pub(super) fn fetch_file_target(target: &TargetDocument) -> FetchResult<FetchSuccess> {
    let started = Instant::now();
    let (fetch, path) = validated_file_input(target)?;

    let bytes = match read_limited_file_bytes(path, fetch.max_bytes) {
        Ok(bytes) => bytes,
        Err(_) => {
            return Err(Box::new(FetchFailure {
                failure_cause: RunFailureCause::FetchSourceError,
                error_detail: ProcessErrorDetail::new(
                    ProcessErrorKind::Io,
                    "could not read configured file source",
                    Some(path.display().to_string()),
                )
                .expect("file fetch source detail"),
                report: RunFetchSection {
                    engine: FetchEngine::File,
                    final_url: None,
                    http_status: None,
                    content_type: None,
                    bytes_read: None,
                    duration_ms: elapsed_ms(&started),
                },
            }));
        }
    };

    if bytes.len() > fetch.max_bytes {
        return Err(Box::new(FetchFailure {
            failure_cause: RunFailureCause::FetchTooLarge,
            error_detail: ProcessErrorDetail::new(
                ProcessErrorKind::Contract,
                format!(
                    "file source exceeded fetch.max_bytes ({} > {})",
                    bytes.len(),
                    fetch.max_bytes
                ),
                Some(path.display().to_string()),
            )
            .expect("file fetch size detail"),
            report: RunFetchSection {
                engine: FetchEngine::File,
                final_url: None,
                http_status: None,
                content_type: None,
                bytes_read: Some(bytes.len()),
                duration_ms: elapsed_ms(&started),
            },
        }));
    }

    let html = match std::str::from_utf8(&bytes) {
        Ok(text) => normalize_line_endings(text),
        Err(_) => {
            return Err(Box::new(FetchFailure {
                failure_cause: RunFailureCause::FetchDecodeError,
                error_detail: ProcessErrorDetail::new(
                    ProcessErrorKind::Contract,
                    "configured file source is not valid UTF-8 HTML text",
                    Some(path.display().to_string()),
                )
                .expect("file fetch decode detail"),
                report: RunFetchSection {
                    engine: FetchEngine::File,
                    final_url: None,
                    http_status: None,
                    content_type: None,
                    bytes_read: Some(bytes.len()),
                    duration_ms: elapsed_ms(&started),
                },
            }));
        }
    };

    let canonical_path = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let final_url = validated_file_url(&canonical_path, elapsed_ms(&started))?;

    Ok(FetchSuccess {
        final_url: final_url.clone(),
        html,
        report: RunFetchSection {
            engine: FetchEngine::File,
            final_url: Some(final_url.to_string()),
            http_status: None,
            content_type: None,
            bytes_read: Some(bytes.len()),
            duration_ms: elapsed_ms(&started),
        },
    })
}

fn read_limited_file_bytes(path: &Path, max_bytes: usize) -> std::io::Result<Vec<u8>> {
    let mut file = File::open(path)?;
    let limit = max_bytes.saturating_add(1);
    let mut bytes = Vec::new();
    let mut buffer = [0u8; 8192];

    loop {
        let remaining = limit.saturating_sub(bytes.len());
        if remaining == 0 {
            break;
        }
        let read_len = buffer.len().min(remaining);
        let read = file.read(&mut buffer[..read_len])?;
        if read == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..read]);
    }

    Ok(bytes)
}

fn validated_file_input(target: &TargetDocument) -> FetchResult<(&FileFetchConfig, &Path)> {
    let fetch = target
        .fetch
        .file()
        .ok_or_else(|| config_invalid_failure(target.fetch.engine(), 0))?;
    let path = target
        .target
        .file_path()
        .map(Path::new)
        .ok_or_else(|| config_invalid_failure(target.fetch.engine(), 0))?;
    if !path.is_absolute() {
        return Err(config_invalid_failure(target.fetch.engine(), 0));
    }
    Ok((fetch, path))
}

fn validated_file_url(path: &Path, duration_ms: u64) -> FetchResult<Url> {
    Url::from_file_path(path).map_err(|_| config_invalid_failure(FetchEngine::File, duration_ms))
}
