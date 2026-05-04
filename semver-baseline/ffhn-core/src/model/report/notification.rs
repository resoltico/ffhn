use super::super::validate::validate_timestamp_not_before;
use super::*;

mod wire;

/// Structured detail for one process-level FFHN error.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessErrorDetail {
    /// Stable error kind.
    pub(crate) kind: ProcessErrorKind,
    /// Human-readable error detail.
    pub(crate) message: String,
    /// Path associated with the error when one exists.
    pub(crate) path: Option<String>,
}

impl ProcessErrorDetail {
    /// Builds one validated structured process-level error.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError`] when the message is empty or when a present path is empty.
    pub fn new(
        kind: ProcessErrorKind,
        message: impl Into<String>,
        path: Option<String>,
    ) -> Result<Self, CoreError> {
        let detail = Self {
            kind,
            message: message.into(),
            path,
        };
        detail.validate()?;
        Ok(detail)
    }

    /// Returns the stable error kind.
    pub const fn kind(&self) -> ProcessErrorKind {
        self.kind
    }

    /// Returns the human-readable error detail.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Returns the path associated with the error when one exists.
    pub fn path(&self) -> Option<&str> {
        self.path.as_deref()
    }

    /// Validates one structured process-level error.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError`] when the message is empty or when a present path is empty.
    pub fn validate(&self) -> Result<(), CoreError> {
        require_non_empty("process_error.message", &self.message)?;
        self.path
            .as_deref()
            .map(|path| require_non_empty("process_error.path", path))
            .transpose()?;
        Ok(())
    }

    pub(crate) fn with_fallback_path(mut self, path: impl Into<String>) -> Self {
        if self.path.is_none() {
            self.path = Some(path.into());
        }
        self
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
            CoreError::Contract(message) => Self {
                kind: ProcessErrorKind::Contract,
                message: message.clone(),
                path: None,
            },
            CoreError::HtmlcutInterop(message) => Self {
                kind: ProcessErrorKind::HtmlcutInterop,
                message: message.clone(),
                path: None,
            },
            CoreError::Internal(message) => Self {
                kind: ProcessErrorKind::Internal,
                message: message.clone(),
                path: None,
            },
        }
    }
}

/// Stable process-error kind vocabulary used by FFHN reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
    /// FFHN contract error.
    Contract,
    /// HTMLCut interoperability error.
    HtmlcutInterop,
    /// Internal FFHN invariant failure.
    Internal,
}

/// Stable notification-delivery outcome vocabulary used inside `ffhn.run_report`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum NotificationDeliveryOutcome {
    /// The hook exited successfully.
    Delivered {
        /// Exit status returned by the hook process.
        exit_code: i32,
    },
    /// The hook timed out before it produced a successful exit status.
    TimedOut {
        /// Best-effort timeout detail.
        error: String,
    },
    /// The hook failed before a successful exit status.
    Failed {
        /// Exit status when the process exited normally.
        exit_code: Option<i32>,
        /// Best-effort failure detail.
        error: String,
    },
}

/// Stable notification-delivery status vocabulary for the public Rust report API.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotificationDeliveryStatus {
    /// The hook exited successfully.
    Delivered,
    /// The hook timed out before a successful exit.
    TimedOut,
    /// The hook failed before a successful exit.
    Failed,
}

impl NotificationDeliveryOutcome {
    /// Returns the stable delivery status.
    pub const fn status(&self) -> NotificationDeliveryStatus {
        match self {
            Self::Delivered { .. } => NotificationDeliveryStatus::Delivered,
            Self::TimedOut { .. } => NotificationDeliveryStatus::TimedOut,
            Self::Failed { .. } => NotificationDeliveryStatus::Failed,
        }
    }

    /// Returns the exit code when one exists.
    pub const fn exit_code(&self) -> Option<i32> {
        match self {
            Self::Delivered { exit_code } => Some(*exit_code),
            Self::TimedOut { .. } => None,
            Self::Failed { exit_code, .. } => *exit_code,
        }
    }

    /// Returns the best-effort error detail when delivery failed.
    pub fn error(&self) -> Option<&str> {
        match self {
            Self::Delivered { .. } => None,
            Self::TimedOut { error } | Self::Failed { error, .. } => Some(error),
        }
    }

    /// Returns whether the hook exited successfully.
    #[cfg(test)]
    pub const fn is_delivered(&self) -> bool {
        matches!(self, Self::Delivered { .. })
    }

    /// Returns whether the hook timed out.
    #[cfg(test)]
    pub const fn is_timed_out(&self) -> bool {
        matches!(self, Self::TimedOut { .. })
    }
}

/// Best-effort notification delivery result inside `ffhn.run_report`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RunNotificationDelivery {
    /// Hook name from `target.toml`.
    pub(crate) hook_name: String,
    /// Delivery duration in milliseconds.
    pub(crate) duration_ms: u64,
    /// Stable delivery result.
    pub(crate) outcome: NotificationDeliveryOutcome,
}

impl RunNotificationDelivery {
    pub(crate) fn delivered(
        hook_name: impl Into<String>,
        duration_ms: u64,
        exit_code: i32,
    ) -> Self {
        Self {
            hook_name: hook_name.into(),
            duration_ms,
            outcome: NotificationDeliveryOutcome::Delivered { exit_code },
        }
    }

    pub(crate) fn timed_out(
        hook_name: impl Into<String>,
        duration_ms: u64,
        error: impl Into<String>,
    ) -> Self {
        Self {
            hook_name: hook_name.into(),
            duration_ms,
            outcome: NotificationDeliveryOutcome::TimedOut {
                error: error.into(),
            },
        }
    }

    pub(crate) fn failed(
        hook_name: impl Into<String>,
        duration_ms: u64,
        exit_code: Option<i32>,
        error: impl Into<String>,
    ) -> Self {
        Self {
            hook_name: hook_name.into(),
            duration_ms,
            outcome: NotificationDeliveryOutcome::Failed {
                exit_code,
                error: error.into(),
            },
        }
    }

    /// Returns the hook name from `target.toml`.
    pub fn hook_name(&self) -> &str {
        &self.hook_name
    }

    /// Returns the delivery duration in milliseconds.
    pub const fn duration_ms(&self) -> u64 {
        self.duration_ms
    }

    #[cfg(test)]
    pub(crate) const fn outcome(&self) -> &NotificationDeliveryOutcome {
        &self.outcome
    }

    /// Returns the stable delivery status.
    pub const fn status(&self) -> NotificationDeliveryStatus {
        self.outcome.status()
    }

    /// Returns whether the hook exited successfully.
    #[cfg(test)]
    pub const fn is_delivered(&self) -> bool {
        self.outcome.is_delivered()
    }

    /// Returns whether the hook timed out.
    #[cfg(test)]
    pub const fn is_timed_out(&self) -> bool {
        self.outcome.is_timed_out()
    }

    /// Returns the exit status when one exists.
    pub const fn exit_code(&self) -> Option<i32> {
        self.outcome.exit_code()
    }

    /// Returns the best-effort failure detail when delivery failed.
    pub fn error(&self) -> Option<&str> {
        self.outcome.error()
    }
}

/// `ffhn.notification_payload` schema written to hook stdin.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NotificationPayload {
    /// Frozen schema identity.
    pub(crate) schema_name: String,
    /// Frozen schema version.
    pub(crate) schema_version: u32,
    /// Hook name receiving the payload.
    pub(crate) hook_name: String,
    /// Timestamp when FFHN started this delivery attempt.
    pub(crate) delivery_started_at: String,
    /// Structured pre-delivery run snapshot.
    pub(crate) run_report: RunReport,
    /// Reserved extensions.
    pub(crate) extensions: Extensions,
}

impl NotificationPayload {
    /// Returns the frozen schema name.
    pub fn schema_name(&self) -> &str {
        &self.schema_name
    }

    /// Returns the frozen schema version.
    pub fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Returns the hook name receiving this payload.
    pub fn hook_name(&self) -> &str {
        &self.hook_name
    }

    /// Returns the notification-delivery start timestamp.
    pub fn delivery_started_at(&self) -> &str {
        &self.delivery_started_at
    }

    /// Returns the pre-delivery run report snapshot.
    pub fn run_report(&self) -> &RunReport {
        &self.run_report
    }

    /// Returns any reserved extensions.
    pub fn extensions(&self) -> Option<&std::collections::BTreeMap<String, serde_json::Value>> {
        self.extensions.as_ref()
    }

    /// Validates one hook-stdin payload.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError`] when the schema identity is wrong, the hook name is empty, the
    /// delivery timestamp is invalid, or the embedded run report violates FFHN's pre-delivery
    /// notification contract.
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
            return Err(CoreError::contract(
                "notification_payload.run_report must be a live run snapshot",
            ));
        }
        if !self.run_report.persist.last_run_write().is_not_attempted() {
            return Err(CoreError::contract(
                "notification_payload.run_report must carry last_run_write.status = not_attempted",
            ));
        }
        if !self.run_report.notifications.is_empty() {
            return Err(CoreError::contract(
                "notification_payload.run_report must not include notification deliveries",
            ));
        }
        validate_timestamp_not_before(
            "notification_payload.run_report.run_finished_at",
            &self.run_report.run_finished_at,
            "notification_payload.delivery_started_at",
            &self.delivery_started_at,
        )?;
        Ok(())
    }
}
