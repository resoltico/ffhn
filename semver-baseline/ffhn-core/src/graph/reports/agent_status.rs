//! Public graph-wide agent status document.

use serde::Serialize;

use super::GraphSourceStatusReport;

/// Schema name for one v11 agent status report.
pub const AGENT_STATUS_REPORT_SCHEMA_NAME: &str = "ffhn.agent_status_report";
/// Schema version for one v11 agent status report.
pub const AGENT_STATUS_REPORT_SCHEMA_VERSION: u32 = 1;

/// Public stable status snapshot for every configured source.
#[derive(Clone, Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AgentStatusReport {
    schema_name: String,
    schema_version: u32,
    sources: Vec<GraphSourceStatusReport>,
}

impl AgentStatusReport {
    /// Creates an agent status report from source-id-ordered source reports.
    pub fn new(sources: Vec<GraphSourceStatusReport>) -> Self {
        Self {
            schema_name: AGENT_STATUS_REPORT_SCHEMA_NAME.to_owned(),
            schema_version: AGENT_STATUS_REPORT_SCHEMA_VERSION,
            sources,
        }
    }
}
