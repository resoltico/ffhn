use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::schema::{
    BATCH_RUN_REPORT_SCHEMA_NAME, BATCH_RUN_REPORT_SCHEMA_VERSION, HTMLCUT_INTEROP_PROFILE,
    NOTIFICATION_PAYLOAD_SCHEMA_NAME, NOTIFICATION_PAYLOAD_SCHEMA_VERSION, RUN_REPORT_SCHEMA_NAME,
    RUN_REPORT_SCHEMA_VERSION, STATUS_REPORT_SCHEMA_NAME, STATUS_REPORT_SCHEMA_VERSION,
};
use super::validate::{require_non_empty, validate_identity, validate_sha256, validate_timestamp};
use super::{
    ChangeKind, CompareBasis, Extensions, FailureClass, FetchEngine, NotificationEvent, OutputKind,
    ReasonCode, RunMode, RunOutcome, SelectionKind, SelectionMatch, StatePhase, TargetStatus,
};
use crate::CoreError;

mod batch;
mod checks;
mod notification;
mod run;
mod status;

pub use batch::{BatchOutcomeCounts, BatchRunEntry, BatchRunReport, BatchRunReportInput};
pub use notification::{
    NotificationPayload, ProcessErrorDetail, ProcessErrorKind, RunNotificationDelivery,
};
pub use run::{
    RunChangeRegion, RunChangeSection, RunCompareSection, RunExtractionSection, RunFetchSection,
    RunPersistSection, RunReport,
};
pub use status::{ArtifactStatus, SnapshotDigestSummary, StatusReport};

#[cfg(test)]
mod tests;
