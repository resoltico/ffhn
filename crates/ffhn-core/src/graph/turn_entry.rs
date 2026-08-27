//! Ordered source-turn manifest recovery, shared by every operation including reset.

use crate::CoreError;

use super::TrustedSourceDir;

/// The durable manifest class that prevents all normal source operations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnresolvableManifest {
    /// A lineage transition cannot be reconciled with source authority.
    Lineage,
    /// A normal source-generation commit cannot pass its lineage or integrity gate.
    Commit,
}

impl UnresolvableManifest {
    /// Returns the stable report spelling for the unresolvable commit-point class.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Lineage => "lineage_manifest",
            Self::Commit => "commit_manifest",
        }
    }
}

/// Ordered turn-entry outcome before the lineage gate or any acquisition/delivery operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceTurnEntry {
    /// Every recoverable manifest was completed or no manifest was present.
    Ready,
    /// A source-level manifest anomaly requires `reset --source`.
    Unresolvable(UnresolvableManifest),
}

impl TrustedSourceDir {
    /// Resolves lineage first, then normal commits, before every source operation.
    ///
    /// A normal caller must not inspect or modify source state after an unresolvable outcome;
    /// only the blind source-reset protocol may discard that scope.
    pub fn recover_turn_entry(&self) -> Result<SourceTurnEntry, CoreError> {
        if self.read_lineage_manifest().is_err() || self.recover_lineage_transition().is_err() {
            return Ok(SourceTurnEntry::Unresolvable(UnresolvableManifest::Lineage));
        }
        if self.open_storage().is_err() {
            return Ok(SourceTurnEntry::Ready);
        }
        match self.recover_normal_commit() {
            Ok(()) => Ok(SourceTurnEntry::Ready),
            Err(_) => Ok(SourceTurnEntry::Unresolvable(UnresolvableManifest::Commit)),
        }
    }
}

#[cfg(test)]
#[path = "turn_entry/tests.rs"]
mod tests;
