use std::collections::BTreeMap;

use serde_json::Value;

mod extraction;
mod report;
mod schema;
mod state;
mod target;
mod validate;
mod value;
mod vocab;

type Extensions = Option<BTreeMap<String, Value>>;

pub use extraction::{ExtractionRecord, SelectionEvidence, SelectionRange, SnapshotReference};
pub use report::{
    BatchOutcomeCounts, BatchRunEntry, BatchRunEntryView, BatchRunReport, BatchRunReportInput,
    FailedRunReportView, LastRunSnapshot, NotificationDeliveryStatus, NotificationPayload,
    PersistWriteStatus, ProcessErrorDetail, ProcessErrorKind, ReportableRunBodyView, RunBodyView,
    RunChangeRegion, RunChangeSection, RunCompareView, RunExtractionSection, RunFetchView,
    RunNotificationDeliveryView, RunPersistView, RunReport, RunResult, SnapshotDigestSummary,
    StatusReport, StatusSummary, SuccessfulRunReportView,
};
pub(crate) use report::{NotificationDeliveryOutcome, RunNotificationDelivery};
pub(crate) use report::{RunCompareSection, RunFetchSection, RunPersistSection};
pub use schema::{
    BATCH_RUN_REPORT_SCHEMA_NAME, BATCH_RUN_REPORT_SCHEMA_VERSION, EXTRACTION_RECORD_SCHEMA_NAME,
    EXTRACTION_RECORD_SCHEMA_VERSION, LAST_RUN_SNAPSHOT_SCHEMA_NAME,
    LAST_RUN_SNAPSHOT_SCHEMA_VERSION, NOTIFICATION_PAYLOAD_SCHEMA_NAME,
    NOTIFICATION_PAYLOAD_SCHEMA_VERSION, RUN_REPORT_SCHEMA_NAME, RUN_REPORT_SCHEMA_VERSION,
    STATE_SCHEMA_NAME, STATE_SCHEMA_VERSION, STATUS_REPORT_SCHEMA_NAME,
    STATUS_REPORT_SCHEMA_VERSION, TARGET_SCHEMA_NAME, TARGET_SCHEMA_VERSION,
};
pub use state::{LastRunRecord, StateDocument, StoredBaseline};
pub(crate) use target::CompareConfig;
#[cfg(all(test, unix))]
pub(crate) use target::NotificationAdapter;
#[cfg(all(test, unix))]
pub(crate) use target::NotificationEndpoint;
pub use target::{CanonicalizerSpec, NotificationRouteView, TargetDocument};
pub use target::{
    CompareConfigView, CssSelectorSelectionView, DelimiterPairSelectionView, FetchConfigView,
    FileFetchConfigView, FileTargetSourceView, HttpFetchConfigView, HttpTargetSourceView,
    SelectionConfigView, SelectionModeView, TargetSourceView,
};
#[cfg(any(test, doctest))]
pub(crate) use target::{FetchConfig, TargetSource};
pub(crate) use target::{
    FileFetchConfig, NetworkFetchConfig, NotificationRoute, SelectionConfig, SelectionModeConfig,
};
pub use value::{RelativeArtifactPath, TargetId};
pub use vocab::{
    BaselinePhase, CanonicalizerKind, ChangeKind, CompareBasis, DelimiterMode, FailureClass,
    FetchEngine, HttpMethod, RegexFlag, RunFailureCause, RunMode, RunOutcome, SelectionKind,
    SelectionMatch, SnapshotSlot, TargetKind, WhitespaceMode,
};
