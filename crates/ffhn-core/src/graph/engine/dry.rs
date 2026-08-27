//! Non-mutating manifest readiness check for a dry source cycle.

use super::super::{SourceTurnEntry, TrustedSourceDir, UnresolvableManifest};

pub(super) fn turn_entry(source_dir: &TrustedSourceDir) -> SourceTurnEntry {
    if !matches!(source_dir.read_lineage_manifest(), Ok(None)) {
        return SourceTurnEntry::Unresolvable(UnresolvableManifest::Lineage);
    }
    match source_dir.open_storage() {
        Ok(storage) if !matches!(storage.read_commit_manifest(), Ok(None)) => {
            SourceTurnEntry::Unresolvable(UnresolvableManifest::Commit)
        }
        Ok(_) | Err(_) => SourceTurnEntry::Ready,
    }
}
