use std::fs::File;
use std::io::Read;
use std::time::Duration;

use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use ureq::{
    ResponseExt,
    tls::{RootCerts, TlsConfig},
};

use crate::model::{fetch_detail, io_detail, plain_detail};
use crate::{
    DiagnosticDetail, DiagnosticKind, DiagnosticOperation, FetchConfig, FetchFailureDetails,
    IoErrorClass, TargetDocument,
};

use super::FetchedSource;

pub(in crate::runtime) fn fetch_source(
    target: &TargetDocument,
) -> Result<FetchedSource, DiagnosticDetail> {
    match (target.source(), target.fetch()) {
        (crate::TargetSource::File { file_path }, FetchConfig::File { max_bytes }) => {
            read_file_source(file_path, *max_bytes).map(|body| FetchedSource {
                body,
                effective_http_url: None,
            })
        }
        (
            crate::TargetSource::Http { source_url },
            FetchConfig::Http {
                timeout_ms,
                max_bytes,
                user_agent,
                follow_redirects,
                accept,
                headers,
                ..
            },
        ) => fetch_http_response(
            source_url,
            *timeout_ms,
            *max_bytes,
            user_agent,
            *follow_redirects,
            accept,
            headers,
        ),
        _ => Err(plain_detail(
            DiagnosticKind::Contract,
            DiagnosticOperation::TargetValidation,
            "target source and fetch.engine must agree",
            None,
        )),
    }
}

pub(in crate::runtime) fn read_file_source(
    file_path: &str,
    max_bytes: usize,
) -> Result<String, DiagnosticDetail> {
    let mut file = File::open(file_path).map_err(|error| {
        io_detail(
            IoErrorClass::from_error(&error),
            DiagnosticOperation::FileRead,
            "the file could not be opened",
            Some(file_path.to_owned()),
        )
    })?;
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take((max_bytes + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| {
            io_detail(
                IoErrorClass::from_error(&error),
                DiagnosticOperation::FileRead,
                "the file could not be read",
                Some(file_path.to_owned()),
            )
        })?;
    if bytes.len() > max_bytes {
        return Err(fetch_detail(
            DiagnosticOperation::FileRead,
            "the file source exceeded its configured byte limit",
            Some(file_path.to_owned()),
            FetchFailureDetails::BodyBytesExceeded {
                configured_max_bytes: max_bytes,
                observed_bytes: bytes.len(),
            },
        ));
    }
    String::from_utf8(bytes).map_err(|_| {
        fetch_detail(
            DiagnosticOperation::FileRead,
            "file contents are not valid UTF-8",
            Some(file_path.to_owned()),
            FetchFailureDetails::InvalidUtf8,
        )
    })
}

#[cfg(test)]
pub(in crate::runtime) fn fetch_http_source(
    url: &url::Url,
    timeout_ms: u64,
    max_bytes: usize,
    user_agent: &str,
    follow_redirects: bool,
    accept: &str,
    headers: &std::collections::BTreeMap<String, String>,
) -> Result<String, DiagnosticDetail> {
    fetch_http_response(
        url,
        timeout_ms,
        max_bytes,
        user_agent,
        follow_redirects,
        accept,
        headers,
    )
    .map(|source| source.body)
}

pub(in crate::runtime) fn fetch_http_response(
    url: &url::Url,
    timeout_ms: u64,
    max_bytes: usize,
    user_agent: &str,
    follow_redirects: bool,
    accept: &str,
    headers: &std::collections::BTreeMap<String, String>,
) -> Result<FetchedSource, DiagnosticDetail> {
    let tls_config = TlsConfig::builder()
        .root_certs(RootCerts::PlatformVerifier)
        .build();
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .tls_config(tls_config)
        .timeout_global(Some(Duration::from_millis(timeout_ms)))
        .max_redirects(if follow_redirects { 10 } else { 0 })
        .max_redirects_will_error(false)
        .http_status_as_error(false)
        .build()
        .into();
    let mut request = agent
        .get(url.as_str())
        .header("Accept", accept)
        .header("User-Agent", user_agent);
    for (name, value) in headers {
        request = request.header(name, value);
    }
    let mut response = request.call().map_err(|error| {
        let error = error.into_io();
        io_detail(
            IoErrorClass::from_error(&error),
            DiagnosticOperation::HttpFetch,
            "the HTTP request could not be completed",
            Some(url.to_string()),
        )
    })?;
    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        return Err(fetch_detail(
            DiagnosticOperation::HttpFetch,
            "the HTTP response returned a non-success status",
            Some(response.get_uri().to_string()),
            FetchFailureDetails::HttpStatus { status },
        ));
    }
    if let Some(length) = response
        .headers()
        .get("content-length")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<usize>().ok())
        && length > max_bytes
    {
        return Err(fetch_detail(
            DiagnosticOperation::HttpFetch,
            "the HTTP response Content-Length exceeded its configured byte limit",
            Some(response.get_uri().to_string()),
            FetchFailureDetails::HttpContentLengthExceeded {
                configured_max_bytes: max_bytes,
                content_length: length,
            },
        ));
    }
    let mut bytes = Vec::new();
    response
        .body_mut()
        .as_reader()
        .take((max_bytes + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| {
            io_detail(
                IoErrorClass::from_error(&error),
                DiagnosticOperation::HttpFetch,
                "the HTTP response body could not be read",
                Some(response.get_uri().to_string()),
            )
        })?;
    if bytes.len() > max_bytes {
        return Err(fetch_detail(
            DiagnosticOperation::HttpFetch,
            "the HTTP response body exceeded its configured byte limit",
            Some(response.get_uri().to_string()),
            FetchFailureDetails::BodyBytesExceeded {
                configured_max_bytes: max_bytes,
                observed_bytes: bytes.len(),
            },
        ));
    }
    let effective_http_url = parse_effective_http_url(response.get_uri().to_string().as_str())?;
    String::from_utf8(bytes)
        .map(|body| FetchedSource {
            body,
            effective_http_url: Some(effective_http_url),
        })
        .map_err(|_| {
            fetch_detail(
                DiagnosticOperation::HttpFetch,
                "the HTTP response body is not valid UTF-8",
                Some(response.get_uri().to_string()),
                FetchFailureDetails::InvalidUtf8,
            )
        })
}

pub(in crate::runtime) fn parse_effective_http_url(
    value: &str,
) -> Result<url::Url, DiagnosticDetail> {
    url::Url::parse(value).map_err(|_| {
        plain_detail(
            DiagnosticKind::Contract,
            DiagnosticOperation::HttpFetch,
            "the HTTP client returned an invalid effective URL",
            Some(value.to_owned()),
        )
    })
}

pub(in crate::runtime) fn now_utc() -> Result<String, crate::CoreError> {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(crate::CoreError::from)
}
