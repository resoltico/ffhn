//! Engine-independent document versus not-modified cycle decision.

use crate::CoreError;

use super::{SourceAcquisition, SourceDocument, SourceState};

/// Decision after source acquisition and before measurement projection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SourceCycleDecision {
    /// The shared source body must be projected by eligible measurements.
    Document(Box<super::SourceDocumentBytes>),
    /// No measurement projection is required for this source cycle.
    NotModified {
        /// Direct HTTP validator evidence returned by a valid `304`, when any.
        validators: Option<super::HttpValidators>,
    },
}

#[cfg(test)]
#[path = "cycle/tests.rs"]
mod tests;

/// Decides whether one acquired source result needs measurement projection.
pub fn decide_source_cycle(
    source: &SourceDocument,
    state: Option<&SourceState>,
    acquisition: SourceAcquisition,
    document_required: bool,
) -> Result<SourceCycleDecision, CoreError> {
    let source_digest = source.source_representation_digest()?;
    match acquisition {
        SourceAcquisition::NotModified(validators) => Ok(SourceCycleDecision::NotModified {
            validators: Some(validators),
        }),
        SourceAcquisition::Document(document) => {
            let unchanged_file = document
                .file_content_sha256
                .as_deref()
                .is_some_and(|digest| {
                    !document_required
                        && state.is_some_and(|state| {
                            state.matches_file_representation(&source_digest, digest)
                        })
                });
            if unchanged_file {
                Ok(SourceCycleDecision::NotModified { validators: None })
            } else {
                Ok(SourceCycleDecision::Document(Box::new(document)))
            }
        }
    }
}
