use std::io::Read;
use std::time::{Duration, Instant};

use encoding_rs::{Encoding, UTF_8};
use ureq::ResponseExt;
use ureq::tls::{RootCerts, TlsConfig};
use url::Url;

use crate::canonical::normalize_line_endings;
use crate::time::elapsed_ms;
use crate::{
    NetworkFetchConfig, ProcessErrorDetail, ProcessErrorKind, ReasonCode, RunFetchSection,
    TargetDocument,
};

use super::{FetchFailure, FetchResult, FetchSuccess, config_invalid_failure};

pub(super) fn fetch_http_target(target: &TargetDocument) -> FetchResult<FetchSuccess> {
    let started = Instant::now();
    let fetch = validated_network_fetch(target)?;
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
            let reason_code = map_ureq_error(&error);
            return Err(Box::new(FetchFailure {
                reason_code,
                error_detail: ProcessErrorDetail::new(
                    ProcessErrorKind::Io,
                    error.to_string(),
                    Some(source_url.to_string()),
                )
                .expect("http request failure detail"),
                report: RunFetchSection {
                    engine: target.fetch.engine(),
                    final_url: None,
                    http_status: None,
                    content_type: None,
                    bytes_read: None,
                    duration_ms: elapsed_ms(&started),
                },
            }));
        }
    };

    let final_url = parse_final_url_or_source(&response.get_uri().to_string(), source_url);
    let http_status = response.status().as_u16();
    let content_type = header_value(&response, "content-type");

    if !http_status_is_success(http_status) {
        return Err(Box::new(FetchFailure {
            reason_code: map_http_status_reason(http_status),
            error_detail: ProcessErrorDetail::new(
                ProcessErrorKind::Contract,
                format!("HTTP request returned status {http_status}"),
                Some(final_url.to_string()),
            )
            .expect("http status failure detail"),
            report: RunFetchSection {
                engine: target.fetch.engine(),
                final_url: Some(final_url.to_string()),
                http_status: Some(http_status),
                content_type,
                bytes_read: None,
                duration_ms: elapsed_ms(&started),
            },
        }));
    }

    if !supported_content_type(content_type.as_deref()) {
        return Err(Box::new(FetchFailure {
            reason_code: ReasonCode::FetchUnsupportedContentType,
            error_detail: ProcessErrorDetail::new(
                ProcessErrorKind::Contract,
                format!(
                    "HTTP response content type is not supported: {}",
                    content_type.as_deref().unwrap_or("<missing>")
                ),
                Some(final_url.to_string()),
            )
            .expect("http content-type detail"),
            report: RunFetchSection {
                engine: target.fetch.engine(),
                final_url: Some(final_url.to_string()),
                http_status: Some(http_status),
                content_type,
                bytes_read: None,
                duration_ms: elapsed_ms(&started),
            },
        }));
    }

    if content_length_exceeds_limit(&response, fetch.max_bytes) {
        return Err(Box::new(FetchFailure {
            reason_code: ReasonCode::FetchTooLarge,
            error_detail: ProcessErrorDetail::new(
                ProcessErrorKind::Contract,
                format!(
                    "HTTP response exceeded fetch.max_bytes ({})",
                    fetch.max_bytes
                ),
                Some(final_url.to_string()),
            )
            .expect("http size detail"),
            report: RunFetchSection {
                engine: target.fetch.engine(),
                final_url: Some(final_url.to_string()),
                http_status: Some(http_status),
                content_type,
                bytes_read: None,
                duration_ms: elapsed_ms(&started),
            },
        }));
    }

    let bytes = match read_limited_bytes(response.body_mut().as_reader(), fetch.max_bytes) {
        Ok(bytes) => bytes,
        Err(reason_code) => {
            return Err(Box::new(FetchFailure {
                reason_code,
                error_detail: ProcessErrorDetail::new(
                    ProcessErrorKind::Io,
                    "could not read the full HTTP response body",
                    Some(final_url.to_string()),
                )
                .expect("http body-read detail"),
                report: RunFetchSection {
                    engine: target.fetch.engine(),
                    final_url: Some(final_url.to_string()),
                    http_status: Some(http_status),
                    content_type,
                    bytes_read: None,
                    duration_ms: elapsed_ms(&started),
                },
            }));
        }
    };

    let html = match decode_body(&bytes, content_type.as_deref()) {
        Ok(html) => html,
        Err(reason_code) => {
            return Err(Box::new(FetchFailure {
                reason_code,
                error_detail: ProcessErrorDetail::new(
                    ProcessErrorKind::Contract,
                    "HTTP response body could not be decoded into supported HTML text",
                    Some(final_url.to_string()),
                )
                .expect("http decode detail"),
                report: RunFetchSection {
                    engine: target.fetch.engine(),
                    final_url: Some(final_url.to_string()),
                    http_status: Some(http_status),
                    content_type,
                    bytes_read: Some(bytes.len()),
                    duration_ms: elapsed_ms(&started),
                },
            }));
        }
    };

    Ok(FetchSuccess {
        final_url: final_url.clone(),
        html: normalize_line_endings(&html),
        report: RunFetchSection {
            engine: target.fetch.engine(),
            final_url: Some(final_url.to_string()),
            http_status: Some(http_status),
            content_type,
            bytes_read: Some(bytes.len()),
            duration_ms: elapsed_ms(&started),
        },
    })
}

fn validated_http_source_url(target: &TargetDocument) -> FetchResult<&Url> {
    let source_url = target
        .target
        .source_url()
        .ok_or_else(|| config_invalid_failure(target.fetch.engine(), 0))?;
    if !matches!(source_url.scheme(), "http" | "https") {
        return Err(config_invalid_failure(target.fetch.engine(), 0));
    }
    Ok(source_url)
}

fn validated_network_fetch(target: &TargetDocument) -> FetchResult<&NetworkFetchConfig> {
    target
        .fetch
        .http()
        .ok_or_else(|| config_invalid_failure(target.fetch.engine(), 0))
}

pub(super) fn build_agent(fetch: &NetworkFetchConfig) -> ureq::Agent {
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

pub(super) fn map_ureq_error(error: &ureq::Error) -> ReasonCode {
    match error {
        ureq::Error::Timeout(_) => ReasonCode::FetchTimeout,
        ureq::Error::BodyExceedsLimit(_) => ReasonCode::FetchTooLarge,
        ureq::Error::Decompress(_, _) => ReasonCode::FetchDecodeError,
        ureq::Error::StatusCode(status) => map_http_status_reason(*status),
        ureq::Error::Http(_)
        | ureq::Error::InvalidProxyUrl
        | ureq::Error::ConnectionFailed
        | ureq::Error::TooManyRedirects
        | ureq::Error::ConnectProxyFailed(_)
        | ureq::Error::RequireHttpsOnly(_)
        | ureq::Error::TlsRequired
        | ureq::Error::HostNotFound
        | ureq::Error::Io(_)
        | ureq::Error::Tls(_)
        | ureq::Error::Pem(_)
        | ureq::Error::Rustls(_)
        | ureq::Error::Protocol(_)
        | ureq::Error::RedirectFailed
        | ureq::Error::BadUri(_)
        | ureq::Error::LargeResponseHeader(_, _)
        | ureq::Error::BodyStalled => ReasonCode::FetchNetworkError,
        // Treat bespoke or future non-exhaustive ureq variants as transient network failures until
        // FFHN classifies them explicitly. Revisit this arm whenever ureq is upgraded.
        _ => ReasonCode::FetchNetworkError,
    }
}

pub(super) fn map_http_status_reason(status: u16) -> ReasonCode {
    if status >= 500 {
        ReasonCode::FetchHttpServerError
    } else {
        ReasonCode::FetchHttpClientError
    }
}

pub(super) fn parse_final_url_or_source(candidate: &str, fallback: &Url) -> Url {
    Url::parse(candidate).unwrap_or_else(|_| fallback.clone())
}

fn http_status_is_success(status: u16) -> bool {
    (200..300).contains(&status)
}

pub(super) fn supported_content_type(value: Option<&str>) -> bool {
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

pub(super) fn read_limited_bytes(
    mut reader: impl Read,
    max_bytes: usize,
) -> Result<Vec<u8>, ReasonCode> {
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

pub(super) fn decode_body(bytes: &[u8], content_type: Option<&str>) -> Result<String, ReasonCode> {
    let encoding = charset_from_content_type(content_type).ok_or(ReasonCode::FetchDecodeError)?;
    let (text, _, had_errors) = encoding.decode(bytes);
    if had_errors {
        return Err(ReasonCode::FetchDecodeError);
    }
    Ok(text.into_owned())
}

pub(super) fn charset_from_content_type(content_type: Option<&str>) -> Option<&'static Encoding> {
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
