//! No-follow configuration reads through already trusted graph and source roots.

use std::io::Read;

use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::{Dir, OpenOptions};
use serde::de::DeserializeOwned;

use crate::CoreError;

use super::{
    AgentDocument, GraphIdentity, GraphPaths, MeasurementDocument, MeasurementId, SourceDocument,
    SourceId, TrustedGraphRoot, TrustedSourceDir,
    storage::{atomic_write_text, open_optional_real_child, open_real_child},
};

impl TrustedGraphRoot {
    /// Reads and validates the graph-wide agent configuration without following its entry.
    pub fn read_agent_document(&self) -> Result<AgentDocument, CoreError> {
        read_toml_regular(
            &self.dir,
            "agent.toml",
            &self.paths.agent_file(),
            "agent configuration",
        )
    }

    /// Requires both immutable graph identity and the current agent configuration envelope.
    pub fn validate_graph_documents(&self) -> Result<(), CoreError> {
        self.read_graph_identity()?
            .ok_or_else(|| CoreError::contract("graph root has no graph identity"))?;
        self.read_agent_document()?;
        Ok(())
    }

    /// Creates a new empty graph root and its immutable graph identity without minting any source
    /// or measurement lineage.
    pub fn initialize(paths: GraphPaths, created_at_utc: String) -> Result<Self, CoreError> {
        std::fs::create_dir_all(paths.root())
            .map_err(|error| CoreError::io(paths.root(), error))?;
        let graph = Self::open(paths)?;
        if graph.read_graph_identity()?.is_some()
            || graph.dir.symlink_metadata("agent.toml").is_ok()
        {
            return Err(CoreError::contract(
                "graph root is already initialized and cannot be initialized again",
            ));
        }
        graph
            .dir
            .create_dir("sources")
            .map_err(|error| CoreError::io(graph.paths.sources_dir(), error))?;
        graph.write_graph_identity(&GraphIdentity::new(created_at_utc)?)?;
        write_toml(
            &graph.dir,
            "agent.toml",
            &AgentDocument::new(),
            &graph.paths.agent_file(),
        )?;
        Ok(graph)
    }

    /// Lists graph source directories after validating every directory-name identity.
    pub fn source_ids(&self) -> Result<Vec<SourceId>, CoreError> {
        let sources = open_real_child(
            &self.dir,
            "sources",
            &self.paths.sources_dir(),
            "graph sources root",
        )?;
        sources
            .read_dir(".")
            .map_err(|error| CoreError::io(self.paths.sources_dir(), error))?
            .map(|entry| {
                let entry =
                    entry.map_err(|error| CoreError::io(self.paths.sources_dir(), error))?;
                let name = utf8_entry_name(entry.file_name(), "source directory")?;
                let file_type = entry
                    .file_type()
                    .map_err(|error| CoreError::io(self.paths.sources_dir().join(&name), error))?;
                if file_type.is_symlink() || !file_type.is_dir() {
                    return Err(CoreError::contract(
                        "source entry must be a non-symlink directory",
                    ));
                }
                SourceId::new(name)
            })
            .collect()
    }

    /// Creates exactly one source configuration directory and TOML document, without state or
    /// identity artifacts. First acquisition owns source lineage initialization.
    pub fn create_source_document(
        &self,
        source: &SourceDocument,
    ) -> Result<TrustedSourceDir, CoreError> {
        source.validate()?;
        let sources = open_real_child(
            &self.dir,
            "sources",
            &self.paths.sources_dir(),
            "graph sources root",
        )?;
        let paths = self.paths.source(source.source_id().clone());
        create_new_real_child(
            &sources,
            paths.source_id().as_str(),
            &paths.source_dir(),
            "source directory",
        )?;
        let source_dir = open_real_child(
            &sources,
            paths.source_id().as_str(),
            &paths.source_dir(),
            "source directory",
        )?;
        write_toml(&source_dir, "source.toml", source, &paths.source_file())?;
        Ok(TrustedSourceDir {
            paths,
            dir: source_dir,
        })
    }
}

impl TrustedSourceDir {
    /// Reads and validates the source configuration without following its filesystem entry.
    pub fn read_source_document(&self) -> Result<SourceDocument, CoreError> {
        let source: SourceDocument = read_toml_regular(
            &self.dir,
            "source.toml",
            &self.paths.source_file(),
            "source configuration",
        )?;
        if source.source_id() != self.paths.source_id() {
            return Err(CoreError::contract(
                "source configuration source_id does not match its source directory",
            ));
        }
        Ok(source)
    }

    /// Lists configuration measurements without making configuration presence a lineage action.
    pub fn measurement_ids(&self) -> Result<Vec<MeasurementId>, CoreError> {
        let Some(measurements) = open_optional_real_child(
            &self.dir,
            "measurements",
            &self.paths.measurements_dir(),
            "measurement configuration root",
        )?
        else {
            return Ok(Vec::new());
        };
        measurements
            .read_dir(".")
            .map_err(|error| CoreError::io(self.paths.measurements_dir(), error))?
            .map(|entry| {
                let entry =
                    entry.map_err(|error| CoreError::io(self.paths.measurements_dir(), error))?;
                let name = utf8_entry_name(entry.file_name(), "measurement directory")?;
                let file_type = entry_file_type(
                    entry.file_type(),
                    &self.paths.measurements_dir().join(&name),
                )?;
                if file_type.is_symlink() || !file_type.is_dir() {
                    return Err(CoreError::contract(
                        "measurement entry must be a non-symlink directory",
                    ));
                }
                MeasurementId::new(name)
            })
            .collect()
    }

    /// Reads a measurement configuration through its validated source-relative directory.
    pub fn read_measurement_document(
        &self,
        measurement_id: &MeasurementId,
    ) -> Result<MeasurementDocument, CoreError> {
        let measurements = open_real_child(
            &self.dir,
            "measurements",
            &self.paths.measurements_dir(),
            "measurement configuration root",
        )?;
        let measurement_directory = self.paths.measurements_dir().join(measurement_id.as_str());
        let measurement = open_real_child(
            &measurements,
            measurement_id.as_str(),
            &measurement_directory,
            "measurement configuration directory",
        )?;
        let document: MeasurementDocument = read_toml_regular(
            &measurement,
            "measurement.toml",
            &self.paths.measurement_file(measurement_id),
            "measurement configuration",
        )?;
        if document.measurement_id() != measurement_id {
            return Err(CoreError::contract(
                "measurement configuration measurement_id does not match its directory",
            ));
        }
        Ok(document)
    }

    /// Creates exactly one measurement configuration document without changing source identity
    /// or creating measurement state. First projection is the sole lineage-minting point.
    pub fn create_measurement_document(
        &self,
        measurement: &MeasurementDocument,
    ) -> Result<(), CoreError> {
        measurement.validate()?;
        let measurement_id = measurement.measurement_id();
        let measurements = match open_optional_real_child(
            &self.dir,
            "measurements",
            &self.paths.measurements_dir(),
            "measurement configuration root",
        )? {
            Some(dir) => dir,
            None => {
                self.dir
                    .create_dir("measurements")
                    .map_err(|error| CoreError::io(self.paths.measurements_dir(), error))?;
                open_real_child(
                    &self.dir,
                    "measurements",
                    &self.paths.measurements_dir(),
                    "measurement configuration root",
                )?
            }
        };
        let measurement_directory = self.paths.measurements_dir().join(measurement_id.as_str());
        create_new_real_child(
            &measurements,
            measurement_id.as_str(),
            &measurement_directory,
            "measurement configuration directory",
        )?;
        let directory = open_real_child(
            &measurements,
            measurement_id.as_str(),
            &measurement_directory,
            "measurement configuration directory",
        )?;
        write_toml(
            &directory,
            "measurement.toml",
            measurement,
            &self.paths.measurement_file(measurement_id),
        )
    }
}

fn create_new_real_child(
    parent: &Dir,
    name: &str,
    full_path: &std::path::Path,
    role: &str,
) -> Result<(), CoreError> {
    require_absent(parent.symlink_metadata(name), full_path, role)?;
    parent
        .create_dir(name)
        .map_err(|error| CoreError::io(full_path, error))
}

fn require_absent<T>(
    result: std::io::Result<T>,
    full_path: &std::path::Path,
    role: &str,
) -> Result<(), CoreError> {
    match result {
        Ok(_) => Err(CoreError::contract(format!("{role} already exists"))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(CoreError::io(full_path, error)),
    }
}

fn entry_file_type<T>(
    result: std::io::Result<T>,
    full_path: &std::path::Path,
) -> Result<T, CoreError> {
    result.map_err(|error| CoreError::io(full_path, error))
}

fn utf8_entry_name(name: std::ffi::OsString, role: &str) -> Result<String, CoreError> {
    name.into_string()
        .map_err(|_| CoreError::contract(format!("{role} name must be valid UTF-8")))
}

fn write_toml<T: serde::Serialize>(
    dir: &Dir,
    name: &str,
    value: &T,
    full_path: &std::path::Path,
) -> Result<(), CoreError> {
    let text = toml::to_string(value)
        .map_err(|error| CoreError::internal(format!("TOML serialization failed: {error}")))?;
    atomic_write_text(dir, name, &text, full_path)
}

fn read_toml_regular<T: DeserializeOwned>(
    dir: &Dir,
    name: &str,
    full_path: &std::path::Path,
    role: &str,
) -> Result<T, CoreError> {
    let metadata = dir
        .symlink_metadata(name)
        .map_err(|error| CoreError::io(full_path, error))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(CoreError::contract(format!(
            "{role} must be a non-symlink regular file"
        )));
    }
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    let mut file = dir
        .open_with(name, &options)
        .map_err(|error| CoreError::io(full_path, error))?;
    let mut text = String::new();
    file.read_to_string(&mut text)
        .map_err(|error| CoreError::io(full_path, error))?;
    toml::from_str(&text).map_err(CoreError::from)
}

#[cfg(test)]
#[path = "config_io/tests.rs"]
mod tests;
