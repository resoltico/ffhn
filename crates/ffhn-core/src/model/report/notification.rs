use super::*;

/// Structured detail for one process-level FFHN error.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProcessErrorDetail {
    /// Stable error kind.
    pub kind: ProcessErrorKind,
    /// Human-readable error detail.
    pub message: String,
    /// Path associated with the error when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

impl ProcessErrorDetail {
    /// Validates one structured process-level error.
    pub fn validate(&self) -> Result<(), CoreError> {
        require_non_empty("process_error.message", &self.message)?;
        self.path
            .as_deref()
            .map(|path| require_non_empty("process_error.path", path))
            .transpose()?;
        Ok(())
    }
}

impl From<&CoreError> for ProcessErrorDetail {
    fn from(error: &CoreError) -> Self {
        match error {
            CoreError::Io { path, source } => Self {
                kind: ProcessErrorKind::Io,
                message: source.to_string(),
                path: Some(path.to_string_lossy().into_owned()),
            },
            CoreError::Json(source) => Self {
                kind: ProcessErrorKind::Json,
                message: source.to_string(),
                path: None,
            },
            CoreError::Toml(source) => Self {
                kind: ProcessErrorKind::Toml,
                message: source.to_string(),
                path: None,
            },
            CoreError::Url(source) => Self {
                kind: ProcessErrorKind::Url,
                message: source.to_string(),
                path: None,
            },
            CoreError::TimeFormat(source) => Self {
                kind: ProcessErrorKind::TimeFormat,
                message: source.to_string(),
                path: None,
            },
            CoreError::TimeParse(source) => Self {
                kind: ProcessErrorKind::TimeParse,
                message: source.to_string(),
                path: None,
            },
            CoreError::Htmlcut(message) => Self {
                kind: ProcessErrorKind::Htmlcut,
                message: message.clone(),
                path: None,
            },
        }
    }
}

/// Stable process-error kind vocabulary used by FFHN reports.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProcessErrorKind {
    /// Filesystem or lock I/O error.
    Io,
    /// JSON serialization or decoding error.
    Json,
    /// TOML decoding error.
    Toml,
    /// URL parsing error.
    Url,
    /// Timestamp formatting error.
    TimeFormat,
    /// Timestamp parsing error.
    TimeParse,
    /// HTMLCut interop or contract error.
    Htmlcut,
}

/// Best-effort notification delivery result inside `ffhn.run_report`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RunNotificationDelivery {
    /// Hook name from `target.toml`.
    pub hook_name: String,
    /// Notification event that triggered the hook.
    pub event: NotificationEvent,
    /// Whether the hook process exited successfully.
    pub delivered: bool,
    /// Whether the hook timed out.
    pub timed_out: bool,
    /// Exit status when available.
    pub exit_code: Option<i32>,
    /// Delivery duration in milliseconds.
    pub duration_ms: u64,
    /// Best-effort error detail for failures.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// `ffhn.notification_payload` schema written to hook stdin.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct NotificationPayload {
    /// Frozen schema identity.
    pub schema_name: String,
    /// Frozen schema version.
    pub schema_version: u32,
    /// Hook name receiving the payload.
    pub hook_name: String,
    /// Event that caused FFHN to invoke the hook.
    pub event: NotificationEvent,
    /// Timestamp when FFHN started this delivery attempt.
    pub delivery_started_at: String,
    /// Structured pre-delivery run snapshot.
    pub run_report: RunReport,
    /// Reserved extensions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extensions: Extensions,
}

impl NotificationPayload {
    /// Validates one hook-stdin payload.
    pub fn validate(&self) -> Result<(), CoreError> {
        validate_identity(
            &self.schema_name,
            NOTIFICATION_PAYLOAD_SCHEMA_NAME,
            self.schema_version,
            NOTIFICATION_PAYLOAD_SCHEMA_VERSION,
        )?;
        require_non_empty("notification_payload.hook_name", &self.hook_name)?;
        validate_timestamp(&self.delivery_started_at)?;
        self.run_report.validate()?;
        if self.run_report.run_mode != RunMode::Live {
            return Err(CoreError::htmlcut(
                "notification_payload.run_report must be a live run snapshot",
            ));
        }
        if self.run_report.persist.wrote_last_run {
            return Err(CoreError::htmlcut(
                "notification_payload.run_report must precede the final last_run.json write",
            ));
        }
        if !self.run_report.notifications.is_empty() {
            return Err(CoreError::htmlcut(
                "notification_payload.run_report must not include notification deliveries",
            ));
        }
        Ok(())
    }
}
