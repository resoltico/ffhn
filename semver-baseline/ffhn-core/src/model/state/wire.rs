use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::{Extensions, LastRunRecord, StateDocument, StoredBaseline};
use crate::CoreError;

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawStateDocument {
    schema_name: String,
    schema_version: u32,
    target_id: String,
    baseline: StoredBaseline,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_run: Option<LastRunRecord>,
    #[serde(skip_serializing_if = "Option::is_none")]
    extensions: Extensions,
}

impl TryFrom<RawStateDocument> for StateDocument {
    type Error = CoreError;

    fn try_from(raw: RawStateDocument) -> Result<Self, Self::Error> {
        let document = Self {
            schema_name: raw.schema_name,
            schema_version: raw.schema_version,
            target_id: raw.target_id.try_into()?,
            baseline: raw.baseline,
            last_run: raw.last_run,
            extensions: raw.extensions,
        };
        document.validate()?;
        Ok(document)
    }
}

impl From<&StateDocument> for RawStateDocument {
    fn from(document: &StateDocument) -> Self {
        Self {
            schema_name: document.schema_name.clone(),
            schema_version: document.schema_version,
            target_id: document.target_id.as_str().to_owned(),
            baseline: document.baseline.clone(),
            last_run: document.last_run.clone(),
            extensions: document.extensions.clone(),
        }
    }
}

impl Serialize for StateDocument {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        RawStateDocument::from(self).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for StateDocument {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawStateDocument::deserialize(deserializer)?;
        Self::try_from(raw).map_err(serde::de::Error::custom)
    }
}
