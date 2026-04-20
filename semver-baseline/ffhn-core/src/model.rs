use std::collections::BTreeMap;

use serde_json::Value;

mod extraction;
mod report;
mod schema;
mod state;
mod target;
mod validate;
mod vocab;

type Extensions = Option<BTreeMap<String, Value>>;

pub use extraction::{ExtractionRecord, SnapshotReference};
pub use report::{
    ArtifactStatus, BatchOutcomeCounts, BatchRunEntry, BatchRunReport, RunChangeRegion,
    RunChangeSection, RunCompareSection, RunExtractionSection, RunFetchSection,
    RunNotificationDelivery, RunPersistSection, RunReport, SnapshotDigestSummary, StatusReport,
};
pub use schema::{
    BATCH_RUN_REPORT_SCHEMA_NAME, BATCH_RUN_REPORT_SCHEMA_VERSION,
    EXTRACTION_RECORD_SCHEMA_NAME, EXTRACTION_RECORD_SCHEMA_VERSION, HTMLCUT_INTEROP_PROFILE,
    RUN_REPORT_SCHEMA_NAME, RUN_REPORT_SCHEMA_VERSION, STATE_SCHEMA_NAME, STATE_SCHEMA_VERSION,
    STATUS_REPORT_SCHEMA_NAME, STATUS_REPORT_SCHEMA_VERSION, TARGET_SCHEMA_NAME,
    TARGET_SCHEMA_VERSION,
};
pub use state::StateDocument;
pub use target::{
    CanonicalizerSpec, CompareConfig, FetchConfig, NotificationHook, SelectionConfig,
    StorageConfig, TargetDocument, TargetSource,
};
pub use vocab::{
    CanonicalizerKind, ChangeKind, CompareBasis, DelimiterMode, FailureClass, FetchEngine,
    HttpMethod, NotificationEvent, OutputKind, ReasonCode, RegexFlag, RunMode, RunOutcome,
    SelectionKind, SelectionMatch, SnapshotSlot, StatePhase, TargetKind, TargetStatus,
    WhitespaceMode,
};
