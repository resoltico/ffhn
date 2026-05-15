use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::{Extensions, StatusReport, StatusSummary};
use crate::CoreError;

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawStatusReport {
    schema_name: String,
    schema_version: u32,
    target_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    enabled: Option<bool>,
    status: StatusSummary,
    #[serde(skip_serializing_if = "Option::is_none")]
    extensions: Extensions,
}

impl TryFrom<RawStatusReport> for StatusReport {
    type Error = CoreError;

    fn try_from(raw: RawStatusReport) -> Result<Self, Self::Error> {
        let report = Self {
            schema_name: raw.schema_name,
            schema_version: raw.schema_version,
            target_id: raw.target_id.try_into()?,
            display_name: raw.display_name,
            enabled: raw.enabled,
            status: raw.status,
            extensions: raw.extensions,
        };
        report.validate()?;
        Ok(report)
    }
}

impl From<&StatusReport> for RawStatusReport {
    fn from(report: &StatusReport) -> Self {
        Self {
            schema_name: report.schema_name.clone(),
            schema_version: report.schema_version,
            target_id: report.target_id.as_str().to_owned(),
            display_name: report.display_name.clone(),
            enabled: report.enabled,
            status: report.status.clone(),
            extensions: report.extensions.clone(),
        }
    }
}

impl Serialize for StatusReport {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        RawStatusReport::from(self).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for StatusReport {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawStatusReport::deserialize(deserializer)?;
        Self::try_from(raw).map_err(serde::de::Error::custom)
    }
}
