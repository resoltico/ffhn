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

pub use extraction::{ExtractionRecord, SnapshotReference};
pub use report::{
    BatchOutcomeCounts, BatchRunEntry, BatchRunReport, BatchRunReportInput,
    NotificationDeliveryStatus, NotificationPayload, PersistWriteStatus, ProcessErrorDetail,
    ProcessErrorKind, RunChangeRegion, RunChangeSection, RunCompareView, RunExtractionSection,
    RunFetchView, RunNotificationDeliveryView, RunPersistView, RunReport, SnapshotDigestSummary,
    StatusReport,
};
pub(crate) use report::{NotificationDeliveryOutcome, RunNotificationDelivery};
pub(crate) use report::{RunCompareSection, RunFetchSection, RunPersistSection};
pub use schema::{
    BATCH_RUN_REPORT_SCHEMA_NAME, BATCH_RUN_REPORT_SCHEMA_VERSION, EXTRACTION_RECORD_SCHEMA_NAME,
    EXTRACTION_RECORD_SCHEMA_VERSION, HTMLCUT_INTEROP_PROFILE, NOTIFICATION_PAYLOAD_SCHEMA_NAME,
    NOTIFICATION_PAYLOAD_SCHEMA_VERSION, RUN_REPORT_SCHEMA_NAME, RUN_REPORT_SCHEMA_VERSION,
    STATE_SCHEMA_NAME, STATE_SCHEMA_VERSION, STATUS_REPORT_SCHEMA_NAME,
    STATUS_REPORT_SCHEMA_VERSION, TARGET_SCHEMA_NAME, TARGET_SCHEMA_VERSION,
};
pub use state::StateDocument;
pub use target::{CanonicalizerSpec, NotificationHookView, TargetDocument};
#[cfg(any(test, doctest))]
pub(crate) use target::{CompareConfig, FetchConfig, TargetSource};
pub(crate) use target::{
    FileFetchConfig, NetworkFetchConfig, NotificationHook, SelectionConfig, SelectionModeConfig,
};
pub use value::{RelativeArtifactPath, TargetId};
pub use vocab::{
    CanonicalizerKind, ChangeKind, CompareBasis, DelimiterMode, FailureClass, FetchEngine,
    HttpMethod, OutputKind, ReasonCode, RegexFlag, RunMode, RunOutcome, SelectionKind,
    SelectionMatch, SnapshotSlot, StatePhase, TargetKind, TargetStatus, WhitespaceMode,
};
