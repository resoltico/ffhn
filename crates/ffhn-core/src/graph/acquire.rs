//! Source-owned complete-representation acquisition and validator provenance.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::Read,
    time::Duration,
};

use serde::{Deserialize, Serialize};
use ureq::tls::{RootCerts, TlsConfig};
use url::Url;

use super::{SourceDocument, SourceFetch};

#[path = "acquire/failure.rs"]
mod failure;

pub use failure::{
    SourceAcquireError, SourceFetchFailure, SourceFetchFailureKind, SourceFetchFailureReasonClass,
};

/// Complete source representation bytes and its effective HTTP URL when applicable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceDocumentBytes {
    /// Valid UTF-8 source body.
    pub body: String,
    /// Effective URL supplying the body for HTTP sources.
    pub effective_http_url: Option<Url>,
    /// SHA-256 of accepted file bytes for file sources.
    pub file_content_sha256: Option<String>,
    /// Validators proven to originate from this direct accepted source response.
    pub validators: Option<HttpValidators>,
}

/// HTTP validators proven to originate from one direct accepted source response.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HttpValidators {
    /// Exact source URL that issued the validators.
    pub issued_url: Url,
    /// Entity tag, when supplied by the source response.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub etag: Option<String>,
    /// Last-modified value, when supplied by the source response.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_modified: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HttpValidatorsWire {
    issued_url: Url,
    #[serde(default)]
    etag: Option<String>,
    #[serde(default)]
    last_modified: Option<String>,
}

impl<'de> Deserialize<'de> for HttpValidators {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = HttpValidatorsWire::deserialize(deserializer)?;
        let validators = Self {
            issued_url: wire.issued_url,
            etag: wire.etag,
            last_modified: wire.last_modified,
        };
        validators.validate().map_err(serde::de::Error::custom)?;
        Ok(validators)
    }
}

impl HttpValidators {
    /// Validates direct HTTP provenance and at least one usable validator value.
    pub fn validate(&self) -> Result<(), crate::CoreError> {
        if !matches!(self.issued_url.scheme(), "http" | "https")
            || !self.issued_url.username().is_empty()
            || self.issued_url.password().is_some()
        {
            return Err(crate::CoreError::contract(
                "HTTP validator provenance must be an HTTP(S) URL without userinfo",
            ));
        }
        if self.etag.as_deref().is_none_or(str::is_empty)
            && self.last_modified.as_deref().is_none_or(str::is_empty)
        {
            return Err(crate::CoreError::contract(
                "HTTP validator provenance must retain at least one nonempty validator",
            ));
        }
        if self.etag.as_deref().is_some_and(str::is_empty)
            || self.last_modified.as_deref().is_some_and(str::is_empty)
        {
            return Err(crate::CoreError::contract(
                "HTTP validator values must not be empty",
            ));
        }
        Ok(())
    }
}

/// Source acquisition result before measurement projection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SourceAcquisition {
    /// One complete document is available for every measurement projection.
    Document(SourceDocumentBytes),
    /// A direct conditional source request returned `304 Not Modified`.
    NotModified(HttpValidators),
}

/// Acquires one complete source representation without conditional validators.
pub fn acquire_source(source: &SourceDocument) -> Result<SourceAcquisition, SourceAcquireError> {
    acquire_source_with_validators(source, None)
}

/// Acquires one source representation, using validators only when their issuing URL matches.
pub fn acquire_source_with_validators(
    source: &SourceDocument,
    validators: Option<&HttpValidators>,
) -> Result<SourceAcquisition, SourceAcquireError> {
    acquire_source_with_secret_lookup(source, validators, &|env| std::env::var(env).ok())
}

fn acquire_source_with_secret_lookup(
    source: &SourceDocument,
    validators: Option<&HttpValidators>,
    secret_lookup: &dyn Fn(&str) -> Option<String>,
) -> Result<SourceAcquisition, SourceAcquireError> {
    match source.fetch() {
        SourceFetch::File {
            file_path,
            max_bytes,
        } => acquire_file(file_path, *max_bytes).map(SourceAcquisition::Document),
        SourceFetch::Http {
            source_url,
            user_agent,
            accept,
            max_bytes,
            follow_redirects,
            max_redirects,
            timeouts,
            headers,
            header_secrets,
        } => acquire_http(
            source_url,
            user_agent,
            accept,
            *max_bytes,
            *follow_redirects,
            *max_redirects,
            timeouts,
            headers,
            header_secrets,
            validators,
            secret_lookup,
        ),
    }
}

fn acquire_file(path: &str, max_bytes: usize) -> Result<SourceDocumentBytes, SourceAcquireError> {
    let metadata = std::fs::metadata(path).map_err(classify_file_metadata_error)?;
    if !metadata.is_file() {
        return Err(failure(SourceFetchFailureKind::FileNotRegular));
    }
    let mut file = File::open(path).map_err(classify_file_open_error)?;
    let bytes = read_bounded(
        &mut file,
        max_bytes,
        SourceFetchFailureKind::BodyBytesExceeded,
        classify_file_read_error,
    )?;
    let body =
        String::from_utf8(bytes).map_err(|_| failure(SourceFetchFailureKind::InvalidUtf8))?;
    let file_content_sha256 = crate::stable_json::sha256_hex(body.as_bytes());
    Ok(SourceDocumentBytes {
        body,
        effective_http_url: None,
        file_content_sha256: Some(file_content_sha256),
        validators: None,
    })
}

#[allow(clippy::too_many_arguments)]
fn acquire_http(
    source_url: &Url,
    user_agent: &str,
    accept: &str,
    max_bytes: usize,
    follow_redirects: bool,
    max_redirects: u8,
    timeouts: &super::HttpTimeouts,
    headers: &BTreeMap<String, String>,
    header_secrets: &BTreeMap<String, super::FetchHeaderSecret>,
    validators: Option<&HttpValidators>,
    secret_lookup: &dyn Fn(&str) -> Option<String>,
) -> Result<SourceAcquisition, SourceAcquireError> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .tls_config(
            TlsConfig::builder()
                .root_certs(RootCerts::PlatformVerifier)
                .build(),
        )
        .timeout_global(Some(Duration::from_millis(timeouts.total_ms)))
        .timeout_connect(Some(Duration::from_millis(timeouts.connect_ms)))
        .timeout_recv_response(Some(Duration::from_millis(timeouts.read_idle_ms)))
        .timeout_recv_body(Some(Duration::from_millis(timeouts.read_idle_ms)))
        .max_redirects(0)
        .max_redirects_will_error(false)
        .http_status_as_error(false)
        .build()
        .into();
    let mut current = source_url.clone();
    let mut seen = BTreeSet::from([current.to_string()]);
    let mut redirects = 0;
    let mut extensible_headers_allowed = true;
    loop {
        let mut request = agent
            .get(current.as_str())
            .header("Accept", accept)
            .header("User-Agent", user_agent);
        if extensible_headers_allowed {
            for (name, value) in headers {
                request = request.header(name, value);
            }
            for (name, secret) in header_secrets {
                let value = resolve_fetch_secret(secret, secret_lookup)?;
                request = request.header(name, value);
            }
        }
        if is_direct_response(&current, source_url, redirects)
            && validators.is_some_and(|validators| validators.issued_url == *source_url)
        {
            if let Some(etag) = validators.and_then(|validators| validators.etag.as_deref()) {
                request = request.header("If-None-Match", etag);
            }
            if let Some(last_modified) =
                validators.and_then(|validators| validators.last_modified.as_deref())
            {
                request = request.header("If-Modified-Since", last_modified);
            }
        }
        let mut response = request.call().map_err(classify_http_error)?;
        let status = response.status().as_u16();
        if is_direct_not_modified(status, &current, source_url, redirects) {
            let existing = validators
                .ok_or_else(|| failure_with_status(SourceFetchFailureKind::HttpStatus, status))?;
            return Ok(SourceAcquisition::NotModified(merge_validators(
                existing, &response,
            )));
        }
        if follow_redirects && matches!(status, 301 | 302 | 303 | 307 | 308) {
            let location = response
                .headers()
                .get("location")
                .and_then(|value| value.to_str().ok())
                .ok_or_else(|| failure_with_status(SourceFetchFailureKind::HttpStatus, status))?;
            let next = current
                .join(location)
                .map_err(|_| failure_with_status(SourceFetchFailureKind::HttpStatus, status))?;
            if !validate_redirect(&current, &next, &mut seen, redirects, max_redirects, status)? {
                extensible_headers_allowed = false;
            }
            redirects = redirect_count_after_hop(redirects);
            current = next;
            continue;
        }
        if !(200..300).contains(&status) {
            return Err(failure_with_status(
                SourceFetchFailureKind::HttpStatus,
                status,
            ));
        }
        if !matches!(status, 200 | 203) {
            return Err(failure_with_status(
                SourceFetchFailureKind::HttpSuccessNotRepresentation,
                status,
            ));
        }
        if let Some(length) = response
            .headers()
            .get("content-length")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<usize>().ok())
            && length > max_bytes
        {
            return Err(failure(SourceFetchFailureKind::ContentLengthExceeded));
        }
        let mut reader = response.body_mut().as_reader();
        let bytes = read_bounded(
            &mut reader,
            max_bytes,
            SourceFetchFailureKind::BodyBytesExceeded,
            classify_http_read_error,
        )?;
        let body =
            String::from_utf8(bytes).map_err(|_| failure(SourceFetchFailureKind::InvalidUtf8))?;
        let validators = is_direct_response(&current, source_url, redirects)
            .then(|| validators_from_response(source_url, &response))
            .flatten();
        return Ok(SourceAcquisition::Document(SourceDocumentBytes {
            body,
            effective_http_url: Some(current),
            file_content_sha256: None,
            validators,
        }));
    }
}

const fn redirect_count_after_hop(redirects: u8) -> u8 {
    redirects.saturating_add(1)
}

fn resolve_fetch_secret(
    secret: &super::FetchHeaderSecret,
    lookup: impl FnOnce(&str) -> Option<String>,
) -> Result<String, SourceAcquireError> {
    let value = lookup(&secret.env).ok_or(SourceAcquireError::SecretUnavailable)?;
    let value = secret.format.replace("{value}", &value);
    ureq::http::HeaderValue::from_bytes(value.as_bytes())
        .map_err(|_| SourceAcquireError::SecretUnavailable)?;
    Ok(value)
}

fn validators_from_response(
    issued_url: &Url,
    response: &ureq::http::Response<ureq::Body>,
) -> Option<HttpValidators> {
    let etag = response
        .headers()
        .get("etag")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let last_modified = response
        .headers()
        .get("last-modified")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    (etag.is_some() || last_modified.is_some()).then(|| HttpValidators {
        issued_url: issued_url.clone(),
        etag,
        last_modified,
    })
}

fn merge_validators(
    existing: &HttpValidators,
    response: &ureq::http::Response<ureq::Body>,
) -> HttpValidators {
    let returned = validators_from_response(&existing.issued_url, response);
    HttpValidators {
        issued_url: existing.issued_url.clone(),
        etag: returned
            .as_ref()
            .and_then(|validators| validators.etag.clone())
            .or_else(|| existing.etag.clone()),
        last_modified: returned
            .as_ref()
            .and_then(|validators| validators.last_modified.clone())
            .or_else(|| existing.last_modified.clone()),
    }
}

fn read_bounded(
    reader: &mut impl Read,
    max_bytes: usize,
    overflow: SourceFetchFailureKind,
    classify_read_error: fn(std::io::Error) -> SourceAcquireError,
) -> Result<Vec<u8>, SourceAcquireError> {
    let mut bytes = Vec::new();
    reader
        .take((max_bytes + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(classify_read_error)?;
    if bytes.len() > max_bytes {
        Err(failure(overflow))
    } else {
        Ok(bytes)
    }
}

fn classify_file_read_error(error: std::io::Error) -> SourceAcquireError {
    match error.kind() {
        std::io::ErrorKind::PermissionDenied => {
            failure(SourceFetchFailureKind::FilePermissionDenied)
        }
        _ => failure(SourceFetchFailureKind::FileReadError),
    }
}

fn classify_file_metadata_error(error: std::io::Error) -> SourceAcquireError {
    match error.kind() {
        std::io::ErrorKind::NotFound => failure(SourceFetchFailureKind::FileNotFound),
        std::io::ErrorKind::PermissionDenied => {
            failure(SourceFetchFailureKind::FilePermissionDenied)
        }
        _ => failure(SourceFetchFailureKind::FileReadError),
    }
}

fn classify_file_open_error(error: std::io::Error) -> SourceAcquireError {
    match error.kind() {
        std::io::ErrorKind::PermissionDenied => {
            failure(SourceFetchFailureKind::FilePermissionDenied)
        }
        _ => failure(SourceFetchFailureKind::FileReadError),
    }
}

fn classify_http_read_error(error: std::io::Error) -> SourceAcquireError {
    let kind = match error.kind() {
        std::io::ErrorKind::UnexpectedEof => SourceFetchFailureKind::IncompleteBody,
        std::io::ErrorKind::ConnectionReset => SourceFetchFailureKind::ConnectionReset,
        std::io::ErrorKind::TimedOut => SourceFetchFailureKind::ReadTimeout,
        std::io::ErrorKind::ConnectionRefused | std::io::ErrorKind::NotConnected => {
            SourceFetchFailureKind::ConnectFailed
        }
        _ => {
            return match ureq::Error::from(error) {
                ureq::Error::Io(error) => unclassified(error.to_string()),
                classified => classify_http_error(classified),
            };
        }
    };
    failure(kind)
}

fn classify_http_error(error: ureq::Error) -> SourceAcquireError {
    match error {
        ureq::Error::HostNotFound => failure(SourceFetchFailureKind::DnsError),
        ureq::Error::ConnectionFailed => failure(SourceFetchFailureKind::ConnectFailed),
        ureq::Error::Tls(_) | ureq::Error::Rustls(_) | ureq::Error::TlsRequired => {
            failure(SourceFetchFailureKind::TlsError)
        }
        ureq::Error::Protocol(_) | ureq::Error::BodyStalled => {
            failure(SourceFetchFailureKind::IncompleteBody)
        }
        ureq::Error::Timeout(timeout) => match timeout {
            ureq::Timeout::Resolve | ureq::Timeout::Connect => {
                failure(SourceFetchFailureKind::ConnectTimeout)
            }
            ureq::Timeout::RecvResponse | ureq::Timeout::RecvBody => {
                failure(SourceFetchFailureKind::ReadTimeout)
            }
            ureq::Timeout::Global
            | ureq::Timeout::PerCall
            | ureq::Timeout::SendRequest
            | ureq::Timeout::SendBody
            | ureq::Timeout::Await100
            | _ => failure(SourceFetchFailureKind::TotalTimeout),
        },
        ureq::Error::Io(error) => classify_http_read_error(error),
        other => unclassified(other.to_string()),
    }
}

fn same_origin(left: &Url, right: &Url) -> bool {
    left.scheme() == right.scheme()
        && left.host_str() == right.host_str()
        && left.port_or_known_default() == right.port_or_known_default()
}

fn is_direct_response(current: &Url, source: &Url, redirects: u8) -> bool {
    redirects == 0 && current == source
}

fn is_direct_not_modified(status: u16, current: &Url, source: &Url, redirects: u8) -> bool {
    status == 304 && is_direct_response(current, source, redirects)
}

fn validate_redirect(
    current: &Url,
    next: &Url,
    seen: &mut BTreeSet<String>,
    redirects: u8,
    max_redirects: u8,
    status: u16,
) -> Result<bool, SourceAcquireError> {
    if !matches!(next.scheme(), "http" | "https") {
        return Err(failure_with_status(
            SourceFetchFailureKind::HttpStatus,
            status,
        ));
    }
    if current.scheme() == "https" && next.scheme() == "http" {
        return Err(failure(SourceFetchFailureKind::RedirectDowngrade));
    }
    if redirects == max_redirects {
        return Err(failure(SourceFetchFailureKind::TooManyRedirects));
    }
    if !seen.insert(next.to_string()) {
        return Err(failure(SourceFetchFailureKind::RedirectLoop));
    }
    Ok(same_origin(current, next))
}

fn failure(kind: SourceFetchFailureKind) -> SourceAcquireError {
    debug_assert!(!matches!(kind, SourceFetchFailureKind::IoUnclassified));
    SourceAcquireError::Fetch(SourceFetchFailure {
        kind,
        status: None,
        raw_platform_error: None,
    })
}

fn failure_with_status(kind: SourceFetchFailureKind, status: u16) -> SourceAcquireError {
    SourceAcquireError::Fetch(SourceFetchFailure {
        kind,
        status: Some(status),
        raw_platform_error: None,
    })
}

fn unclassified(raw_platform_error: String) -> SourceAcquireError {
    SourceAcquireError::Fetch(SourceFetchFailure {
        kind: SourceFetchFailureKind::IoUnclassified,
        status: None,
        raw_platform_error: Some(raw_platform_error),
    })
}

#[cfg(test)]
#[path = "acquire/tests.rs"]
mod tests;
