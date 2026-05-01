use url::Url;

use crate::model::TargetKind;
use crate::{
    FetchEngine, ProcessErrorDetail, ProcessErrorKind, ReasonCode, RunFetchSection, TargetDocument,
};

mod file;
mod http;

use self::file::fetch_file_target;
use self::http::fetch_http_target;
#[cfg(test)]
use self::http::{
    build_agent, charset_from_content_type, decode_body, map_http_status_reason, map_ureq_error,
    parse_final_url_or_source, read_limited_bytes, supported_content_type,
};
#[cfg(test)]
use encoding_rs::UTF_8;
#[cfg(test)]
use ureq::tls::RootCerts;

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
    /// Structured detail for the failure.
    pub error_detail: ProcessErrorDetail,
    /// Structured fetch report section.
    pub report: RunFetchSection,
}

pub(crate) type FetchResult<T> = Result<T, Box<FetchFailure>>;

/// Fetches one configured FFHN target.
pub fn fetch_target(target: &TargetDocument) -> FetchResult<FetchSuccess> {
    match target.target.kind() {
        TargetKind::Http => fetch_http_target(target),
        TargetKind::File => fetch_file_target(target),
    }
}

fn config_invalid_failure(engine: FetchEngine, duration_ms: u64) -> Box<FetchFailure> {
    Box::new(FetchFailure {
        reason_code: ReasonCode::ConfigInvalid,
        error_detail: ProcessErrorDetail::new(
            ProcessErrorKind::Contract,
            "target fetch configuration is invalid for the selected target kind",
            None,
        )
        .expect("config-invalid fetch detail"),
        report: RunFetchSection {
            engine,
            final_url: None,
            http_status: None,
            content_type: None,
            bytes_read: None,
            duration_ms,
        },
    })
}

#[cfg(test)]
mod tests;
