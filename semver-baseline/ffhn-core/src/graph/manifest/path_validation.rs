//! Closed normal-commit destination-path grammar.

use crate::CoreError;

use super::super::{GraphRouteId, MeasurementId};
use super::ManifestPath;

pub(super) fn validate_final_path(path: &ManifestPath) -> Result<(), CoreError> {
    let parts = path.as_str().split('/').collect::<Vec<_>>();
    let valid = match parts.as_slice() {
        ["source-state.json"] => true,
        ["source-outbox" | "dead-letters", file] => valid_delivery_file_name(file),
        ["measurements", measurement_id, "state.json"] => {
            MeasurementId::new(*measurement_id).is_ok()
        }
        [
            "measurements",
            measurement_id,
            "outbox" | "dead-letters",
            file,
        ] => MeasurementId::new(*measurement_id).is_ok() && valid_delivery_file_name(file),
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(CoreError::contract(
            "commit manifest final path is outside the closed state/outbox/dead-letter layout",
        ))
    }
}

fn valid_delivery_file_name(value: &str) -> bool {
    let Some(stem) = value.strip_suffix(".json") else {
        return false;
    };
    let Some((event_id, route_id)) = stem.split_once("--") else {
        return false;
    };
    !route_id.contains("--")
        && event_id.len() == 64
        && event_id
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        && GraphRouteId::new(route_id).is_ok()
}
