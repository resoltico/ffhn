//! No-follow reads of source- and measurement-owned pending delivery records.

use std::io::Read;

use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::{Dir, OpenOptions};

use crate::CoreError;

use super::{
    DeadLetter, DeliveryRecord, MeasurementId, TrustedStorageDir, storage::open_optional_real_child,
};

impl TrustedStorageDir {
    /// Lists every pending source-owned delivery record in stable storage-filename order.
    pub fn read_source_delivery_records(&self) -> Result<Vec<DeliveryRecord>, CoreError> {
        read_records(
            &self.dir,
            "source-outbox",
            &self.paths.source_outbox_dir(),
            "source outbox",
        )
    }

    /// Lists every terminal source-owned dead letter in stable storage-filename order.
    pub fn read_source_dead_letters(&self) -> Result<Vec<DeadLetter>, CoreError> {
        read_dead_letters(
            &self.dir,
            "dead-letters",
            &self.paths.source_dead_letters_dir(),
            "source dead letters",
        )
    }

    /// Lists every pending record owned by one measurement lineage in stable filename order.
    pub fn read_measurement_delivery_records(
        &self,
        measurement_id: &MeasurementId,
    ) -> Result<Vec<DeliveryRecord>, CoreError> {
        let Some(measurement) = self.open_measurement_storage_dir(measurement_id)? else {
            return Ok(Vec::new());
        };
        read_records(
            &measurement,
            "outbox",
            &self.paths.measurement_outbox_dir(measurement_id),
            "measurement outbox",
        )
    }

    /// Lists every terminal dead letter owned by one measurement lineage.
    pub fn read_measurement_dead_letters(
        &self,
        measurement_id: &MeasurementId,
    ) -> Result<Vec<DeadLetter>, CoreError> {
        let Some(measurement) = self.open_measurement_storage_dir(measurement_id)? else {
            return Ok(Vec::new());
        };
        read_dead_letters(
            &measurement,
            "dead-letters",
            &self.paths.measurement_dead_letters_dir(measurement_id),
            "measurement dead letters",
        )
    }

    fn open_measurement_storage_dir(
        &self,
        measurement_id: &MeasurementId,
    ) -> Result<Option<Dir>, CoreError> {
        let root = match open_optional_real_child(
            &self.dir,
            "measurements",
            &self.paths.storage_dir().join("measurements"),
            "measurement storage root",
        )? {
            Some(dir) => dir,
            None => return Ok(None),
        };
        open_optional_real_child(
            &root,
            measurement_id.as_str(),
            &self.paths.measurement_storage_dir(measurement_id),
            "measurement storage directory",
        )
    }
}

fn read_records(
    parent: &Dir,
    name: &str,
    full_path: &std::path::Path,
    role: &str,
) -> Result<Vec<DeliveryRecord>, CoreError> {
    let Some(directory) = open_optional_real_child(parent, name, full_path, role)? else {
        return Ok(Vec::new());
    };
    let mut entries = directory
        .read_dir(".")
        .map_err(|error| CoreError::io(full_path, error))?
        .map(|entry| {
            let entry = entry.map_err(|error| CoreError::io(full_path, error))?;
            let file_name = utf8_file_name(entry.file_name(), "delivery record")?;
            let file_type = entry
                .file_type()
                .map_err(|error| CoreError::io(full_path.join(&file_name), error))?;
            if file_type.is_symlink() || !file_type.is_file() || !file_name.ends_with(".json") {
                return Err(CoreError::contract(
                    "delivery outbox entries must be non-symlink .json regular files",
                ));
            }
            Ok(file_name)
        })
        .collect::<Result<Vec<_>, CoreError>>()?;
    entries.sort_unstable();
    entries
        .into_iter()
        .map(|file_name| read_record(&directory, &file_name, &full_path.join(&file_name)))
        .collect()
}

fn read_record(
    dir: &Dir,
    name: &str,
    full_path: &std::path::Path,
) -> Result<DeliveryRecord, CoreError> {
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    let mut file = dir
        .open_with(name, &options)
        .map_err(|error| CoreError::io(full_path, error))?;
    let mut text = String::new();
    file.read_to_string(&mut text)
        .map_err(|error| CoreError::io(full_path, error))?;
    let record: DeliveryRecord = serde_json::from_str(&text)?;
    let expected = record.storage_file_name();
    if name == expected {
        Ok(record)
    } else {
        Err(CoreError::contract(
            "delivery record filename does not match its immutable event and route key",
        ))
    }
}

fn read_dead_letters(
    parent: &Dir,
    name: &str,
    full_path: &std::path::Path,
    role: &str,
) -> Result<Vec<DeadLetter>, CoreError> {
    let Some(directory) = open_optional_real_child(parent, name, full_path, role)? else {
        return Ok(Vec::new());
    };
    let mut entries = directory
        .read_dir(".")
        .map_err(|error| CoreError::io(full_path, error))?
        .map(|entry| {
            let entry = entry.map_err(|error| CoreError::io(full_path, error))?;
            let file_name = utf8_file_name(entry.file_name(), "dead-letter")?;
            let file_type = entry
                .file_type()
                .map_err(|error| CoreError::io(full_path.join(&file_name), error))?;
            if file_type.is_symlink() || !file_type.is_file() || !file_name.ends_with(".json") {
                return Err(CoreError::contract(
                    "dead-letter entries must be non-symlink .json regular files",
                ));
            }
            Ok(file_name)
        })
        .collect::<Result<Vec<_>, CoreError>>()?;
    entries.sort_unstable();
    entries
        .into_iter()
        .map(|file_name| read_dead_letter(&directory, &file_name, &full_path.join(&file_name)))
        .collect()
}

fn read_dead_letter(
    dir: &Dir,
    name: &str,
    full_path: &std::path::Path,
) -> Result<DeadLetter, CoreError> {
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    let mut file = dir
        .open_with(name, &options)
        .map_err(|error| CoreError::io(full_path, error))?;
    let mut text = String::new();
    file.read_to_string(&mut text)
        .map_err(|error| CoreError::io(full_path, error))?;
    let letter: DeadLetter = serde_json::from_str(&text)?;
    if name == letter.record().storage_file_name() {
        Ok(letter)
    } else {
        Err(CoreError::contract(
            "dead-letter filename does not match its immutable event and route key",
        ))
    }
}

fn utf8_file_name(name: std::ffi::OsString, role: &str) -> Result<String, CoreError> {
    name.into_string()
        .map_err(|_| CoreError::contract(format!("{role} filename must be valid UTF-8")))
}

#[cfg(test)]
#[path = "outbox_storage/tests.rs"]
mod tests;
