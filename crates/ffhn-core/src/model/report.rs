use std::collections::BTreeSet;

use super::schema::{
    BATCH_RUN_REPORT_SCHEMA_NAME, BATCH_RUN_REPORT_SCHEMA_VERSION, LAST_RUN_SNAPSHOT_SCHEMA_NAME,
    LAST_RUN_SNAPSHOT_SCHEMA_VERSION, NOTIFICATION_PAYLOAD_SCHEMA_NAME,
    NOTIFICATION_PAYLOAD_SCHEMA_VERSION, RUN_REPORT_SCHEMA_NAME, RUN_REPORT_SCHEMA_VERSION,
    STATUS_REPORT_SCHEMA_NAME, STATUS_REPORT_SCHEMA_VERSION,
};
use super::validate::{require_non_empty, validate_identity, validate_sha256, validate_timestamp};
use super::{
    BaselinePhase, ChangeKind, CompareBasis, Extensions, FailureClass, FetchEngine, OutputKind,
    RunFailureCause, RunMode, RunOutcome, SelectionKind, SelectionMatch,
};
use crate::CoreError;

mod batch;
mod checks;
mod last_run;
mod notification;
mod run;
mod status;

pub use batch::{
    BatchOutcomeCounts, BatchRunEntry, BatchRunEntryView, BatchRunReport, BatchRunReportInput,
};
pub use last_run::LastRunSnapshot;
pub(crate) use notification::{NotificationDeliveryOutcome, RunNotificationDelivery};
pub use notification::{
    NotificationDeliveryStatus, NotificationPayload, ProcessErrorDetail, ProcessErrorKind,
};
pub use run::{
    FailedRunReportView, PersistWriteStatus, ReportableRunBodyView, RunBodyView, RunChangeRegion,
    RunChangeSection, RunCompareView, RunExtractionSection, RunFetchView,
    RunNotificationDeliveryView, RunPersistView, RunReport, RunResult, SuccessfulRunReportView,
};
pub(crate) use run::{RunCompareSection, RunFetchSection, RunPersistSection};
pub use status::{SnapshotDigestSummary, StatusReport, StatusSummary};

#[cfg(test)]
mod tests;
