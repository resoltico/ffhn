//! Closed selected-measurement validation for an explicit one-source command.

use std::collections::BTreeSet;

use crate::CoreError;

use super::super::MeasurementId;

/// Confirms that an explicit selection is unique and contained in the source configuration.
pub(super) fn selected_measurements(
    configured: Vec<MeasurementId>,
    selection: Option<Vec<MeasurementId>>,
) -> Result<Vec<MeasurementId>, CoreError> {
    let Some(selection) = selection else {
        return Ok(configured);
    };
    let configured = configured.into_iter().collect::<BTreeSet<_>>();
    let mut selected = BTreeSet::new();
    for measurement_id in selection {
        if !configured.contains(&measurement_id) {
            return Err(CoreError::contract(
                "selected measurement is not configured for this source",
            ));
        }
        if !selected.insert(measurement_id) {
            return Err(CoreError::contract(
                "selected measurement identifiers must be unique",
            ));
        }
    }
    Ok(selected.into_iter().collect())
}
