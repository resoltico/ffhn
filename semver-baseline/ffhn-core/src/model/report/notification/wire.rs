use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::{
    Extensions, NotificationDeliveryOutcome, NotificationPayload, ProcessErrorDetail,
    ProcessErrorKind, RunNotificationDelivery, RunReport,
};
use crate::CoreError;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RawProcessErrorKind {
    Io,
    Json,
    Toml,
    Url,
    TimeFormat,
    TimeParse,
    Contract,
    HtmlcutInterop,
    Internal,
}

impl From<RawProcessErrorKind> for ProcessErrorKind {
    fn from(raw: RawProcessErrorKind) -> Self {
        match raw {
            RawProcessErrorKind::Io => Self::Io,
            RawProcessErrorKind::Json => Self::Json,
            RawProcessErrorKind::Toml => Self::Toml,
            RawProcessErrorKind::Url => Self::Url,
            RawProcessErrorKind::TimeFormat => Self::TimeFormat,
            RawProcessErrorKind::TimeParse => Self::TimeParse,
            RawProcessErrorKind::Contract => Self::Contract,
            RawProcessErrorKind::HtmlcutInterop => Self::HtmlcutInterop,
            RawProcessErrorKind::Internal => Self::Internal,
        }
    }
}

impl From<ProcessErrorKind> for RawProcessErrorKind {
    fn from(kind: ProcessErrorKind) -> Self {
        match kind {
            ProcessErrorKind::Io => Self::Io,
            ProcessErrorKind::Json => Self::Json,
            ProcessErrorKind::Toml => Self::Toml,
            ProcessErrorKind::Url => Self::Url,
            ProcessErrorKind::TimeFormat => Self::TimeFormat,
            ProcessErrorKind::TimeParse => Self::TimeParse,
            ProcessErrorKind::Contract => Self::Contract,
            ProcessErrorKind::HtmlcutInterop => Self::HtmlcutInterop,
            ProcessErrorKind::Internal => Self::Internal,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawProcessErrorDetail {
    kind: RawProcessErrorKind,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
}

impl From<RawProcessErrorDetail> for ProcessErrorDetail {
    fn from(raw: RawProcessErrorDetail) -> Self {
        Self {
            kind: raw.kind.into(),
            message: raw.message,
            path: raw.path,
        }
    }
}

impl From<&ProcessErrorDetail> for RawProcessErrorDetail {
    fn from(detail: &ProcessErrorDetail) -> Self {
        Self {
            kind: detail.kind.into(),
            message: detail.message.clone(),
            path: detail.path.clone(),
        }
    }
}

impl Serialize for ProcessErrorDetail {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        RawProcessErrorDetail::from(self).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ProcessErrorDetail {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawProcessErrorDetail::deserialize(deserializer)?;
        let detail = Self::from(raw);
        detail.validate().map_err(serde::de::Error::custom)?;
        Ok(detail)
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum RawNotificationDeliveryOutcome {
    Delivered {
        exit_code: i32,
    },
    TimedOut {
        error: String,
    },
    Failed {
        #[serde(skip_serializing_if = "Option::is_none")]
        exit_code: Option<i32>,
        error: String,
    },
}

impl From<RawNotificationDeliveryOutcome> for NotificationDeliveryOutcome {
    fn from(raw: RawNotificationDeliveryOutcome) -> Self {
        match raw {
            RawNotificationDeliveryOutcome::Delivered { exit_code } => {
                Self::Delivered { exit_code }
            }
            RawNotificationDeliveryOutcome::TimedOut { error } => Self::TimedOut { error },
            RawNotificationDeliveryOutcome::Failed { exit_code, error } => {
                Self::Failed { exit_code, error }
            }
        }
    }
}

impl From<&NotificationDeliveryOutcome> for RawNotificationDeliveryOutcome {
    fn from(outcome: &NotificationDeliveryOutcome) -> Self {
        match outcome {
            NotificationDeliveryOutcome::Delivered { exit_code } => Self::Delivered {
                exit_code: *exit_code,
            },
            NotificationDeliveryOutcome::TimedOut { error } => Self::TimedOut {
                error: error.clone(),
            },
            NotificationDeliveryOutcome::Failed { exit_code, error } => Self::Failed {
                exit_code: *exit_code,
                error: error.clone(),
            },
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRunNotificationDelivery {
    hook_name: String,
    duration_ms: u64,
    outcome: RawNotificationDeliveryOutcome,
}

impl From<RawRunNotificationDelivery> for RunNotificationDelivery {
    fn from(raw: RawRunNotificationDelivery) -> Self {
        Self {
            hook_name: raw.hook_name,
            duration_ms: raw.duration_ms,
            outcome: raw.outcome.into(),
        }
    }
}

impl From<&RunNotificationDelivery> for RawRunNotificationDelivery {
    fn from(delivery: &RunNotificationDelivery) -> Self {
        Self {
            hook_name: delivery.hook_name.clone(),
            duration_ms: delivery.duration_ms,
            outcome: RawNotificationDeliveryOutcome::from(&delivery.outcome),
        }
    }
}

impl Serialize for RunNotificationDelivery {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        RawRunNotificationDelivery::from(self).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for RunNotificationDelivery {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawRunNotificationDelivery::deserialize(deserializer)?;
        Ok(Self::from(raw))
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawNotificationPayload {
    schema_name: String,
    schema_version: u32,
    hook_name: String,
    delivery_started_at: String,
    run_report: RunReport,
    #[serde(skip_serializing_if = "Option::is_none")]
    extensions: Extensions,
}

impl TryFrom<RawNotificationPayload> for NotificationPayload {
    type Error = CoreError;

    fn try_from(raw: RawNotificationPayload) -> Result<Self, Self::Error> {
        let payload = Self {
            schema_name: raw.schema_name,
            schema_version: raw.schema_version,
            hook_name: raw.hook_name,
            delivery_started_at: raw.delivery_started_at,
            run_report: raw.run_report,
            extensions: raw.extensions,
        };
        payload.validate()?;
        Ok(payload)
    }
}

impl From<&NotificationPayload> for RawNotificationPayload {
    fn from(payload: &NotificationPayload) -> Self {
        Self {
            schema_name: payload.schema_name.clone(),
            schema_version: payload.schema_version,
            hook_name: payload.hook_name.clone(),
            delivery_started_at: payload.delivery_started_at.clone(),
            run_report: payload.run_report.clone(),
            extensions: payload.extensions.clone(),
        }
    }
}

impl Serialize for NotificationPayload {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        RawNotificationPayload::from(self).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for NotificationPayload {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawNotificationPayload::deserialize(deserializer)?;
        Self::try_from(raw).map_err(serde::de::Error::custom)
    }
}
