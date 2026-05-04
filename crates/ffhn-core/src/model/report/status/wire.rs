use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::{
    Extensions, ProcessErrorDetail, ReasonCode, SnapshotDigestSummary, StatePhase, StatusReport,
    TargetStatus,
};
use crate::CoreError;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct RawSnapshotDigestSummary {
    canonical_text_sha256: String,
    outer_html_sha256: String,
    captured_at: String,
}

impl From<RawSnapshotDigestSummary> for SnapshotDigestSummary {
    fn from(raw: RawSnapshotDigestSummary) -> Self {
        Self {
            canonical_text_sha256: raw.canonical_text_sha256,
            outer_html_sha256: raw.outer_html_sha256,
            captured_at: raw.captured_at,
        }
    }
}

impl From<&SnapshotDigestSummary> for RawSnapshotDigestSummary {
    fn from(snapshot: &SnapshotDigestSummary) -> Self {
        Self {
            canonical_text_sha256: snapshot.canonical_text_sha256.clone(),
            outer_html_sha256: snapshot.outer_html_sha256.clone(),
            captured_at: snapshot.captured_at.clone(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawStatusReport {
    schema_name: String,
    schema_version: u32,
    target_id: String,
    target_status: TargetStatus,
    reason_code: ReasonCode,
    #[serde(skip_serializing_if = "Option::is_none")]
    error_detail: Option<ProcessErrorDetail>,
    state_phase: Option<StatePhase>,
    current_snapshot: Option<RawSnapshotDigestSummary>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    snapshot_history: Vec<RawSnapshotDigestSummary>,
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
            target_status: raw.target_status,
            reason_code: raw.reason_code,
            error_detail: raw.error_detail,
            state_phase: raw.state_phase,
            current_snapshot: raw.current_snapshot.map(SnapshotDigestSummary::from),
            snapshot_history: raw
                .snapshot_history
                .into_iter()
                .map(SnapshotDigestSummary::from)
                .collect(),
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
            target_status: report.target_status,
            reason_code: report.reason_code,
            error_detail: report.error_detail.clone(),
            state_phase: report.state_phase,
            current_snapshot: report
                .current_snapshot
                .as_ref()
                .map(RawSnapshotDigestSummary::from),
            snapshot_history: report
                .snapshot_history
                .iter()
                .map(RawSnapshotDigestSummary::from)
                .collect(),
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
