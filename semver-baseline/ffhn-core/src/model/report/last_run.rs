use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::*;

/// Persisted last-run snapshot written to `last_run.json`.
///
/// This artifact is not the final emitted `ffhn.run_report` itself. It wraps the live
/// post-notification run-report snapshot that FFHN attempted to publish. The persisted nested
/// report therefore keeps `persist.last_run_write.status = not_attempted`, while the live stdout
/// report may later reflect `written` or `failed` for that final publication step.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LastRunSnapshot {
    schema_name: String,
    schema_version: u32,
    run_report: RunReport,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct RawLastRunSnapshot {
    schema_name: String,
    schema_version: u32,
    run_report: RunReport,
}

impl LastRunSnapshot {
    /// Builds one validated `ffhn.last_run_snapshot` document.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError`] when the nested run report is not a valid live pre-publication
    /// snapshot for `last_run.json`.
    pub fn new(run_report: RunReport) -> Result<Self, CoreError> {
        let snapshot = Self {
            schema_name: LAST_RUN_SNAPSHOT_SCHEMA_NAME.to_owned(),
            schema_version: LAST_RUN_SNAPSHOT_SCHEMA_VERSION,
            run_report,
        };
        snapshot.validate()?;
        Ok(snapshot)
    }

    /// Returns the canonical schema name.
    pub fn schema_name(&self) -> &str {
        &self.schema_name
    }

    /// Returns the schema version.
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Returns the nested live run-report snapshot that FFHN attempted to publish.
    pub const fn run_report(&self) -> &RunReport {
        &self.run_report
    }

    /// Validates this last-run snapshot against FFHN's persisted-artifact contract.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError`] when the schema identity is wrong, the nested report is invalid, or
    /// the nested report is not a live pre-publication snapshot.
    pub fn validate(&self) -> Result<(), CoreError> {
        validate_identity(
            &self.schema_name,
            LAST_RUN_SNAPSHOT_SCHEMA_NAME,
            self.schema_version,
            LAST_RUN_SNAPSHOT_SCHEMA_VERSION,
        )?;
        self.run_report.validate()?;
        if self.run_report.run_mode() != RunMode::Live {
            return Err(CoreError::contract(
                "last_run_snapshot.run_report must be a live run report",
            ));
        }
        if !self
            .run_report
            .persist()
            .last_run_write()
            .is_not_attempted()
        {
            return Err(CoreError::contract(
                "last_run_snapshot.run_report.persist.last_run_write.status must be not_attempted",
            ));
        }
        Ok(())
    }
}

impl Serialize for LastRunSnapshot {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        RawLastRunSnapshot {
            schema_name: self.schema_name.clone(),
            schema_version: self.schema_version,
            run_report: self.run_report.clone(),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for LastRunSnapshot {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawLastRunSnapshot::deserialize(deserializer)?;
        let snapshot = Self {
            schema_name: raw.schema_name,
            schema_version: raw.schema_version,
            run_report: raw.run_report,
        };
        snapshot.validate().map_err(serde::de::Error::custom)?;
        Ok(snapshot)
    }
}
