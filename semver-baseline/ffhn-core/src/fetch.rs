use std::fs;
use std::io::Read;
use std::path::Path;
use std::time::{Duration, Instant};

use encoding_rs::{Encoding, UTF_8};
use ureq::ResponseExt;
use ureq::tls::{RootCerts, TlsConfig};
use url::Url;

use crate::canonical::normalize_line_endings;
use crate::model::TargetKind;
use crate::{FetchConfig, FetchEngine, ReasonCode, RunFetchSection, TargetDocument};

/// Successful fetch payload returned to FFHN's extraction stage.
#[derive(Clone, Debug)]
pub struct FetchSuccess {
    /// Final URL after redirects when known.
    pub final_url: Url,
    /// Decoded HTML string passed into HTMLCut.
    pub html: String,
    /// Structured fetch report section.
    pub report: RunFetchSection,
}

/// Structured fetch failure returned before extraction starts.
#[derive(Clone, Debug)]
pub struct FetchFailure {
    /// FFHN reason code for the failure.
    pub reason_code: ReasonCode,
    /// Structured fetch report section.
    pub report: RunFetchSection,
}

/// Fetches one configured FFHN target.
pub fn fetch_target(target: &TargetDocument) -> Result<FetchSuccess, FetchFailure> {
    match target.target.kind {
        TargetKind::Http => fetch_http_target(target),
        TargetKind::File => fetch_file_target(target),
    }
}

fn fetch_http_target(target: &TargetDocument) -> Result<FetchSuccess, FetchFailure> {
    let started = Instant::now();
    let fetch = &target.fetch;
    let agent = build_agent(fetch);
    let source_url = validated_http_source_url(target)?;

    let mut request = agent.get(source_url.as_str());
    request = request.header("Accept", &fetch.accept);
    request = request.header("User-Agent", &fetch.user_agent);

    for (name, value) in &fetch.headers {
        request = request.header(name, value);
    }

    let mut response = match request.call() {
        Ok(response) => response,
        Err(error) => {
            let duration_ms = started.elapsed().as_millis() as u64;
            let reason_code = map_ureq_error(&error);
            return Err(FetchFailure {
                reason_code,
                report: RunFetchSection {
                    engine: fetch.engine,
                    final_url: None,
                    http_status: None,
                    content_type: None,
                    bytes_read: None,
                    duration_ms,
                },
            });
        }
    };

    let duration_ms = started.elapsed().as_millis() as u64;
    let final_url = parse_final_url_or_source(&response.get_uri().to_string(), source_url);
    let http_status = response.status().as_u16();
    let content_type = header_value(&response, "content-type");

    if !http_status_is_success(http_status) {
        return Err(FetchFailure {
            reason_code: map_http_status_reason(http_status),
            report: RunFetchSection {
                engine: fetch.engine,
                final_url: Some(final_url.to_string()),
                http_status: Some(http_status),
                content_type,
                bytes_read: None,
                duration_ms,
            },
        });
    }

    if !supported_content_type(content_type.as_deref()) {
        return Err(FetchFailure {
            reason_code: ReasonCode::FetchUnsupportedContentType,
            report: RunFetchSection {
                engine: fetch.engine,
                final_url: Some(final_url.to_string()),
                http_status: Some(http_status),
                content_type,
                bytes_read: None,
                duration_ms,
            },
        });
    }

    if content_length_exceeds_limit(&response, fetch.max_bytes) {
        return Err(FetchFailure {
            reason_code: ReasonCode::FetchTooLarge,
            report: RunFetchSection {
                engine: fetch.engine,
                final_url: Some(final_url.to_string()),
                http_status: Some(http_status),
                content_type,
                bytes_read: None,
                duration_ms,
            },
        });
    }

    let bytes = match read_limited_bytes(response.body_mut().as_reader(), fetch.max_bytes) {
        Ok(bytes) => bytes,
        Err(reason_code) => {
            return Err(FetchFailure {
                reason_code,
                report: RunFetchSection {
                    engine: fetch.engine,
                    final_url: Some(final_url.to_string()),
                    http_status: Some(http_status),
                    content_type,
                    bytes_read: None,
                    duration_ms,
                },
            });
        }
    };

    let html = match decode_body(&bytes, content_type.as_deref()) {
        Ok(html) => html,
        Err(reason_code) => {
            return Err(FetchFailure {
                reason_code,
                report: RunFetchSection {
                    engine: fetch.engine,
                    final_url: Some(final_url.to_string()),
                    http_status: Some(http_status),
                    content_type,
                    bytes_read: Some(bytes.len()),
                    duration_ms,
                },
            });
        }
    };

    Ok(FetchSuccess {
        final_url: final_url.clone(),
        html: normalize_line_endings(&html),
        report: RunFetchSection {
            engine: fetch.engine,
            final_url: Some(final_url.to_string()),
            http_status: Some(http_status),
            content_type,
            bytes_read: Some(bytes.len()),
            duration_ms,
        },
    })
}

fn fetch_file_target(target: &TargetDocument) -> Result<FetchSuccess, FetchFailure> {
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

fn config_invalid_failure(engine: FetchEngine, duration_ms: u64) -> FetchFailure {
    FetchFailure {
        reason_code: ReasonCode::ConfigInvalid,
        report: RunFetchSection {
            engine,
            final_url: None,
            http_status: None,
            content_type: None,
            bytes_read: None,
            duration_ms,
        },
    }
}

fn validated_http_source_url(target: &TargetDocument) -> Result<&Url, FetchFailure> {
    let source_url = target
        .target
        .source_url
        .as_ref()
        .ok_or_else(|| config_invalid_failure(target.fetch.engine, 0))?;
    if !matches!(source_url.scheme(), "http" | "https") {
        return Err(config_invalid_failure(target.fetch.engine, 0));
    }
    Ok(source_url)
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

fn build_agent(fetch: &FetchConfig) -> ureq::Agent {
    let tls_config = TlsConfig::builder()
        .root_certs(RootCerts::PlatformVerifier)
        .build();

    ureq::Agent::config_builder()
        .tls_config(tls_config)
        .timeout_global(Some(Duration::from_millis(fetch.timeout_ms)))
        .max_redirects(if fetch.follow_redirects { 10 } else { 0 })
        .max_redirects_will_error(false)
        .save_redirect_history(true)
        .http_status_as_error(false)
        .build()
        .into()
}

fn map_ureq_error(error: &ureq::Error) -> ReasonCode {
    match error {
        ureq::Error::Timeout(_) => ReasonCode::FetchTimeout,
        ureq::Error::BodyExceedsLimit(_) => ReasonCode::FetchTooLarge,
        ureq::Error::StatusCode(status) => map_http_status_reason(*status),
        ureq::Error::ConnectionFailed
        | ureq::Error::TooManyRedirects
        | ureq::Error::ConnectProxyFailed(_)
        | ureq::Error::RequireHttpsOnly(_)
        | ureq::Error::HostNotFound
        | ureq::Error::Io(_)
        | ureq::Error::Tls(_)
        | ureq::Error::Protocol(_)
        | ureq::Error::RedirectFailed
        | ureq::Error::BadUri(_)
        | ureq::Error::LargeResponseHeader(_, _) => ReasonCode::FetchNetworkError,
        _ => ReasonCode::FetchNetworkError,
    }
}

fn map_http_status_reason(status: u16) -> ReasonCode {
    if status >= 500 {
        ReasonCode::FetchHttpServerError
    } else {
        ReasonCode::FetchHttpClientError
    }
}

fn parse_final_url_or_source(candidate: &str, fallback: &Url) -> Url {
    Url::parse(candidate).unwrap_or_else(|_| fallback.clone())
}

fn http_status_is_success(status: u16) -> bool {
    (200..300).contains(&status)
}

fn supported_content_type(value: Option<&str>) -> bool {
    value.is_none_or(|value| {
        let media_type = value
            .split(';')
            .next()
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();
        media_type == "text/html" || media_type == "application/xhtml+xml"
    })
}

fn content_length_exceeds_limit(
    response: &ureq::http::Response<ureq::Body>,
    max_bytes: usize,
) -> bool {
    header_value(response, "content-length")
        .and_then(|value| value.parse::<usize>().ok())
        .is_some_and(|value| value > max_bytes)
}

fn header_value(response: &ureq::http::Response<ureq::Body>, name: &str) -> Option<String> {
    response
        .headers()
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned)
}

fn read_limited_bytes(mut reader: impl Read, max_bytes: usize) -> Result<Vec<u8>, ReasonCode> {
    let mut bytes = Vec::new();
    let mut buffer = [0u8; 8192];

    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|_| ReasonCode::FetchNetworkError)?;
        if read == 0 {
            break;
        }

        bytes.extend_from_slice(&buffer[..read]);
        if bytes.len() > max_bytes {
            return Err(ReasonCode::FetchTooLarge);
        }
    }

    Ok(bytes)
}

fn decode_body(bytes: &[u8], content_type: Option<&str>) -> Result<String, ReasonCode> {
    let encoding = charset_from_content_type(content_type).ok_or(ReasonCode::FetchDecodeError)?;
    let (text, _, had_errors) = encoding.decode(bytes);
    if had_errors {
        return Err(ReasonCode::FetchDecodeError);
    }
    Ok(text.into_owned())
}

fn charset_from_content_type(content_type: Option<&str>) -> Option<&'static Encoding> {
    let Some(content_type) = content_type else {
        return Some(UTF_8);
    };

    for parameter in content_type.split(';').skip(1) {
        let mut parts = parameter.trim().splitn(2, '=');
        let name = parts.next()?.trim();
        let value = parts.next()?.trim().trim_matches('"');
        if name.eq_ignore_ascii_case("charset") {
            return Encoding::for_label(value.as_bytes());
        }
    }

    Some(UTF_8)
}

#[cfg(test)]
mod tests;
