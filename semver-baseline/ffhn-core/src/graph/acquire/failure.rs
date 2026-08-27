//! Closed acquisition-failure taxonomy and its persisted evidence boundaries.

use serde::{Deserialize, Serialize};

/// Closed source-acquisition failure class.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceFetchFailureKind {
    /// DNS resolution failed.
    DnsError,
    /// A connection could not be established.
    ConnectFailed,
    /// Connection establishment or name resolution exceeded the configured bound.
    ConnectTimeout,
    /// A response body made no progress within the configured idle bound.
    ReadTimeout,
    /// An established connection reset before the representation completed.
    ConnectionReset,
    /// The configured total request timeout expired.
    TotalTimeout,
    /// TLS negotiation or validation failed.
    TlsError,
    /// A framed body ended before its completion signal.
    IncompleteBody,
    /// A redirect chain exceeded its configured bound.
    TooManyRedirects,
    /// A redirect chain revisited one effective URL.
    RedirectLoop,
    /// A redirect would downgrade HTTPS to HTTP.
    RedirectDowngrade,
    /// A declared content length exceeds the configured body bound.
    ContentLengthExceeded,
    /// Streaming body bytes exceeded the configured bound.
    BodyBytesExceeded,
    /// A body was not UTF-8.
    InvalidUtf8,
    /// A non-2xx terminal HTTP response was returned.
    HttpStatus,
    /// A 2xx response was not an accepted complete representation.
    HttpSuccessNotRepresentation,
    /// A source file was absent.
    FileNotFound,
    /// A source file was not readable due to permissions.
    FilePermissionDenied,
    /// A source file path did not name a regular file.
    FileNotRegular,
    /// A source file read failed for another filesystem reason.
    FileReadError,
    /// A native error was outside the explicitly modeled source taxonomy.
    IoUnclassified,
}

impl SourceFetchFailureKind {
    /// Returns the stable event-key and report spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DnsError => "dns_error",
            Self::ConnectFailed => "connect_failed",
            Self::ConnectTimeout => "connect_timeout",
            Self::ReadTimeout => "read_timeout",
            Self::ConnectionReset => "connection_reset",
            Self::TotalTimeout => "total_timeout",
            Self::TlsError => "tls_error",
            Self::IncompleteBody => "incomplete_body",
            Self::TooManyRedirects => "too_many_redirects",
            Self::RedirectLoop => "redirect_loop",
            Self::RedirectDowngrade => "redirect_downgrade",
            Self::ContentLengthExceeded => "content_length_exceeded",
            Self::BodyBytesExceeded => "body_bytes_exceeded",
            Self::InvalidUtf8 => "invalid_utf8",
            Self::HttpStatus => "http_status",
            Self::HttpSuccessNotRepresentation => "http_success_not_representation",
            Self::FileNotFound => "file_not_found",
            Self::FilePermissionDenied => "file_permission_denied",
            Self::FileNotRegular => "file_not_regular",
            Self::FileReadError => "file_read_error",
            Self::IoUnclassified => "io_unclassified",
        }
    }
}

/// Closed broad class retained with every typed source-acquisition failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceFetchFailureReasonClass {
    /// Name resolution, connection, reset, or unclassified HTTP transport failure.
    Network,
    /// Any configured acquisition timeout.
    Timeout,
    /// TLS negotiation or certificate verification failure.
    Tls,
    /// A framing-completion failure.
    Truncation,
    /// Redirect policy failure.
    Redirect,
    /// Configured representation-size bound failure.
    Limit,
    /// UTF-8 decode failure.
    Decode,
    /// Terminal HTTP response status failure.
    HttpStatus,
    /// Local filesystem access failure.
    Filesystem,
}

impl SourceFetchFailureReasonClass {
    /// Returns the stable report spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Network => "network",
            Self::Timeout => "timeout",
            Self::Tls => "tls",
            Self::Truncation => "truncation",
            Self::Redirect => "redirect",
            Self::Limit => "limit",
            Self::Decode => "decode",
            Self::HttpStatus => "http_status",
            Self::Filesystem => "filesystem",
        }
    }
}

/// Closed evidence for one source acquisition failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceFetchFailure {
    /// Stable failure classification.
    pub kind: SourceFetchFailureKind,
    /// HTTP status when the failure was caused by an HTTP response.
    pub status: Option<u16>,
    /// Original platform error retained only for the sole unclassified complement case.
    pub raw_platform_error: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceFetchFailureWire {
    kind: SourceFetchFailureKind,
    reason_class: SourceFetchFailureReasonClass,
    #[serde(default)]
    status: Option<u16>,
    #[serde(default)]
    raw_platform_error: Option<String>,
}

impl<'de> Deserialize<'de> for SourceFetchFailure {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = SourceFetchFailureWire::deserialize(deserializer)?;
        let failure = Self {
            kind: wire.kind,
            status: wire.status,
            raw_platform_error: wire.raw_platform_error,
        };
        failure.validate().map_err(serde::de::Error::custom)?;
        if wire.reason_class != failure.reason_class() {
            return Err(serde::de::Error::custom(
                "source failure reason_class does not match its kind",
            ));
        }
        Ok(failure)
    }
}

impl Serialize for SourceFetchFailure {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        #[derive(Serialize)]
        #[serde(deny_unknown_fields)]
        struct Wire<'a> {
            kind: SourceFetchFailureKind,
            reason_class: SourceFetchFailureReasonClass,
            #[serde(skip_serializing_if = "Option::is_none")]
            status: Option<u16>,
            #[serde(skip_serializing_if = "Option::is_none")]
            raw_platform_error: Option<&'a str>,
        }
        self.validate().map_err(serde::ser::Error::custom)?;
        Wire {
            kind: self.kind,
            reason_class: self.reason_class(),
            status: self.status,
            raw_platform_error: self.raw_platform_error.as_deref(),
        }
        .serialize(serializer)
    }
}

impl SourceFetchFailure {
    /// Returns the contract-defined broad class for this failure kind.
    pub const fn reason_class(&self) -> SourceFetchFailureReasonClass {
        match self.kind {
            SourceFetchFailureKind::DnsError
            | SourceFetchFailureKind::ConnectFailed
            | SourceFetchFailureKind::ConnectionReset
            | SourceFetchFailureKind::IoUnclassified => SourceFetchFailureReasonClass::Network,
            SourceFetchFailureKind::ConnectTimeout
            | SourceFetchFailureKind::ReadTimeout
            | SourceFetchFailureKind::TotalTimeout => SourceFetchFailureReasonClass::Timeout,
            SourceFetchFailureKind::TlsError => SourceFetchFailureReasonClass::Tls,
            SourceFetchFailureKind::IncompleteBody => SourceFetchFailureReasonClass::Truncation,
            SourceFetchFailureKind::TooManyRedirects
            | SourceFetchFailureKind::RedirectLoop
            | SourceFetchFailureKind::RedirectDowngrade => SourceFetchFailureReasonClass::Redirect,
            SourceFetchFailureKind::ContentLengthExceeded
            | SourceFetchFailureKind::BodyBytesExceeded => SourceFetchFailureReasonClass::Limit,
            SourceFetchFailureKind::InvalidUtf8 => SourceFetchFailureReasonClass::Decode,
            SourceFetchFailureKind::HttpStatus
            | SourceFetchFailureKind::HttpSuccessNotRepresentation => {
                SourceFetchFailureReasonClass::HttpStatus
            }
            SourceFetchFailureKind::FileNotFound
            | SourceFetchFailureKind::FilePermissionDenied
            | SourceFetchFailureKind::FileNotRegular
            | SourceFetchFailureKind::FileReadError => SourceFetchFailureReasonClass::Filesystem,
        }
    }

    /// Validates the typed status and raw-platform complement boundaries.
    pub fn validate(&self) -> Result<(), crate::CoreError> {
        let status_kind = matches!(
            self.kind,
            SourceFetchFailureKind::HttpStatus
                | SourceFetchFailureKind::HttpSuccessNotRepresentation
        );
        if status_kind != self.status.is_some() {
            return Err(crate::CoreError::contract(
                "source HTTP failure status presence does not match its kind",
            ));
        }
        if self.status.is_some_and(|status| status == 0) {
            return Err(crate::CoreError::contract(
                "source HTTP failure status must be a valid nonzero status",
            ));
        }
        match (self.kind, self.status) {
            (SourceFetchFailureKind::HttpStatus, Some(status)) if !(200..300).contains(&status) => {
            }
            (SourceFetchFailureKind::HttpSuccessNotRepresentation, Some(status))
                if (200..300).contains(&status) && !matches!(status, 200 | 203) => {}
            (
                SourceFetchFailureKind::HttpStatus
                | SourceFetchFailureKind::HttpSuccessNotRepresentation,
                Some(_),
            ) => {
                return Err(crate::CoreError::contract(
                    "source HTTP failure kind does not match its status complement",
                ));
            }
            _ => {}
        }
        if matches!(self.kind, SourceFetchFailureKind::IoUnclassified)
            != self.raw_platform_error.is_some()
        {
            return Err(crate::CoreError::contract(
                "only io_unclassified source failures retain a raw platform error",
            ));
        }
        if self
            .raw_platform_error
            .as_deref()
            .is_some_and(str::is_empty)
        {
            return Err(crate::CoreError::contract(
                "raw platform error must not be empty",
            ));
        }
        Ok(())
    }
}

/// Source acquisition failure that is either typed transport evidence or missing secret integration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SourceAcquireError {
    /// Source bytes could not be acquired as a complete representation.
    Fetch(SourceFetchFailure),
    /// A configured environment-backed header was unavailable at acquisition time.
    SecretUnavailable,
}
