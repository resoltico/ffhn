use std::fs;

use tempfile::tempdir;

use super::*;
use crate::graph::GraphPaths;

const SOURCE: &str = r#"
schema_name = "ffhn.source"
schema_version = 1
source_id = "shop"
display_name = "Shop"
enabled = true
escalate_after = 2
[fetch]
engine = "file"
file_path = "/tmp/shop.json"
max_bytes = 1024
[conditional]
enabled = false
[schedule]
interval_ms = 1000
min_interval_ms = 1000
"#;

const MEASUREMENT: &str = r#"
schema_name = "ffhn.measurement"
schema_version = 1
measurement_id = "price"
display_name = "Price"
enabled = true
escalate_after = 2
declared_type = "integer"
conditions = []
[projection]
kind = "json_pointer"
pointer = "/price"
"#;

fn source_toml() -> String {
    SOURCE.replace(
        "/tmp/shop.json",
        &crate::graph::test_support::absolute_file_path("shop.json").replace('\\', "\\\\"),
    )
}

#[test]
fn configuration_reads_are_no_follow_and_must_match_identity_directories() {
    let temporary = tempdir().expect("temporary graph");
    let root = temporary.path().join("graph");
    let source_directory = root.join("sources/shop");
    fs::create_dir_all(source_directory.join("measurements/price")).expect("directories");
    fs::write(source_directory.join("source.toml"), source_toml()).expect("source config");
    fs::write(
        source_directory.join("measurements/price/measurement.toml"),
        MEASUREMENT,
    )
    .expect("measurement config");
    let graph = TrustedGraphRoot::open(GraphPaths::new(root)).expect("graph root");
    assert_eq!(
        graph
            .source_ids()
            .expect("source ids")
            .iter()
            .map(SourceId::as_str)
            .collect::<Vec<_>>(),
        ["shop"]
    );
    let source = graph
        .open_source(SourceId::new("shop").expect("source id"))
        .expect("source");
    assert_eq!(
        source
            .read_source_document()
            .expect("source")
            .display_name(),
        "Shop"
    );
    let id = MeasurementId::new("price").expect("measurement id");
    assert_eq!(
        source.measurement_ids().expect("measurement ids"),
        std::slice::from_ref(&id)
    );
    assert_eq!(
        source
            .read_measurement_document(&id)
            .expect("measurement")
            .display_name(),
        "Price"
    );
}

#[test]
fn graph_and_configuration_creation_never_preallocate_source_or_measurement_lineage() {
    let temporary = tempdir().expect("temporary graph");
    let paths = GraphPaths::new(temporary.path().join("graph"));
    let graph = TrustedGraphRoot::initialize(paths, "2026-08-25T00:00:00Z".to_owned())
        .expect("graph initialization");
    assert!(
        graph
            .read_graph_identity()
            .expect("identity read")
            .is_some()
    );
    let source: SourceDocument = toml::from_str(&source_toml()).expect("source document");
    let source = graph
        .create_source_document(&source)
        .expect("source creation");
    assert!(source.read_identity().expect("source identity").is_none());
    assert!(source.open_storage().is_err());
    let measurement: MeasurementDocument = toml::from_str(MEASUREMENT).expect("measurement");
    source
        .create_measurement_document(&measurement)
        .expect("measurement creation");
    assert_eq!(source.measurement_ids().expect("measurement ids").len(), 1);
    assert!(source.read_identity().expect("source identity").is_none());

    let second: MeasurementDocument =
        toml::from_str(&MEASUREMENT.replace("price", "stock")).expect("second measurement");
    source
        .create_measurement_document(&second)
        .expect("second measurement creation");
    assert_eq!(source.measurement_ids().expect("measurement ids").len(), 2);
    assert!(source.create_measurement_document(&measurement).is_err());
    let source_document: SourceDocument = toml::from_str(&source_toml()).expect("source");
    assert!(graph.create_source_document(&source_document).is_err());
    assert!(
        TrustedGraphRoot::initialize(graph.paths().clone(), "2026-08-25T00:00:01Z".to_owned(),)
            .is_err()
    );
    let agent_only = temporary.path().join("agent-only");
    fs::create_dir(&agent_only).expect("agent-only root");
    fs::write(
        agent_only.join("agent.toml"),
        "schema_name = \"ffhn.agent\"\n",
    )
    .expect("agent");
    assert!(
        TrustedGraphRoot::initialize(
            GraphPaths::new(agent_only),
            "2026-08-25T00:00:00Z".to_owned(),
        )
        .is_err()
    );
}

#[test]
fn graph_document_preflight_and_configuration_identity_mismatches_fail_closed() {
    let temporary = tempdir().expect("temporary");
    let root = temporary.path().join("graph");
    fs::create_dir_all(root.join("sources/shop/measurements/price")).expect("directories");
    fs::write(
        root.join("agent.toml"),
        toml::to_string(&AgentDocument::new()).expect("agent"),
    )
    .expect("agent");
    let graph = TrustedGraphRoot::open(GraphPaths::new(root.clone())).expect("graph");
    assert!(graph.validate_graph_documents().is_err());
    graph
        .write_graph_identity(
            &GraphIdentity::new("2026-08-25T00:00:00Z".to_owned()).expect("identity"),
        )
        .expect("identity");
    graph.validate_graph_documents().expect("documents");

    fs::write(
        root.join("sources/shop/source.toml"),
        source_toml().replace("source_id = \"shop\"", "source_id = \"other\""),
    )
    .expect("source");
    let source = graph
        .open_source(SourceId::new("shop").expect("source id"))
        .expect("source");
    assert!(source.read_source_document().is_err());
    fs::write(root.join("sources/shop/source.toml"), source_toml()).expect("source");
    fs::write(
        root.join("sources/shop/measurements/price/measurement.toml"),
        MEASUREMENT.replace("measurement_id = \"price\"", "measurement_id = \"other\""),
    )
    .expect("measurement");
    assert!(
        source
            .read_measurement_document(&MeasurementId::new("price").expect("id"))
            .is_err()
    );

    fs::remove_file(root.join("agent.toml")).expect("remove agent");
    assert!(graph.validate_graph_documents().is_err());
}

#[test]
fn source_and_measurement_inventories_reject_non_directory_entries_and_regular_file_contracts() {
    let temporary = tempdir().expect("temporary");
    let root = temporary.path().join("graph");
    fs::create_dir_all(root.join("sources/shop")).expect("source");
    fs::write(root.join("sources/not-a-directory"), "file").expect("file entry");
    let graph = TrustedGraphRoot::open(GraphPaths::new(root.clone())).expect("graph");
    assert!(graph.source_ids().is_err());
    fs::remove_file(root.join("sources/not-a-directory")).expect("remove entry");
    let source = graph
        .open_source(SourceId::new("shop").expect("source"))
        .expect("source");
    assert!(
        source
            .measurement_ids()
            .expect("absent measurements")
            .is_empty()
    );
    fs::write(source.paths().source_file(), "not TOML").expect("bad source");
    assert!(source.read_source_document().is_err());
    fs::remove_file(source.paths().source_file()).expect("remove source");
    fs::create_dir(source.paths().source_file()).expect("source directory");
    assert!(
        source
            .read_source_document()
            .expect_err("directory is not a source document")
            .to_string()
            .contains("source configuration must be a non-symlink regular file")
    );
    fs::remove_dir(source.paths().source_file()).expect("remove directory");
    fs::create_dir(source.paths().measurements_dir()).expect("measurements");
    fs::write(
        source.paths().measurements_dir().join("not-a-directory"),
        "file",
    )
    .expect("measurement entry");
    assert!(source.measurement_ids().is_err());
    assert!(require_absent::<()>(Ok(()), std::path::Path::new("entry"), "entry").is_err());
    assert!(
        require_absent::<()>(
            Err(std::io::Error::from(std::io::ErrorKind::NotFound)),
            std::path::Path::new("entry"),
            "entry",
        )
        .is_ok()
    );
    assert!(
        require_absent::<()>(
            Err(std::io::Error::other("failed")),
            std::path::Path::new("entry"),
            "entry",
        )
        .is_err()
    );
    assert!(
        entry_file_type::<()>(
            Err(std::io::Error::other("failed")),
            std::path::Path::new("entry"),
        )
        .is_err()
    );
}

#[cfg(unix)]
#[test]
fn configuration_inventories_reject_non_utf8_and_symlink_directory_entries() {
    use std::os::unix::ffi::OsStringExt;

    let temporary = tempdir().expect("temporary");
    let root = temporary.path().join("graph");
    fs::create_dir_all(root.join("sources/shop/measurements")).expect("directories");
    let graph = TrustedGraphRoot::open(GraphPaths::new(root.clone())).expect("graph");
    let invalid = std::ffi::OsString::from_vec(vec![0xff]);
    assert!(utf8_entry_name(invalid.clone(), "source directory").is_err());
    std::os::unix::fs::symlink(root.join("sources/shop"), root.join("sources/link"))
        .expect("source symlink");
    assert!(graph.source_ids().is_err());
    fs::remove_file(root.join("sources/link")).expect("remove symlink");

    let source = graph
        .open_source(SourceId::new("shop").expect("source"))
        .expect("source");
    assert!(utf8_entry_name(invalid, "measurement directory").is_err());
    std::os::unix::fs::symlink(
        source.paths().source_dir(),
        source.paths().measurements_dir().join("link"),
    )
    .expect("measurement symlink");
    assert!(source.measurement_ids().is_err());
    fs::write(
        source.paths().source_dir().join("source-target.toml"),
        source_toml(),
    )
    .expect("source target");
    std::os::unix::fs::symlink(
        source.paths().source_dir().join("source-target.toml"),
        source.paths().source_file(),
    )
    .expect("source config symlink");
    assert!(source.read_source_document().is_err());
}
