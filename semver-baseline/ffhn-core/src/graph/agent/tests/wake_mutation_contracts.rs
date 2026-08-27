use time::{Duration, OffsetDateTime, format_description::well_known::Rfc3339};

use super::super::*;
use crate::graph::{GraphPaths, SourceDocument, SourceId, TrustedGraphRoot};

#[test]
fn empty_graph_uses_the_exact_idle_wake_interval() {
    let temporary = tempfile::tempdir().expect("temporary graph");
    let graph = TrustedGraphRoot::initialize(
        GraphPaths::new(temporary.path().join("graph")),
        "2026-08-25T00:00:00Z".to_owned(),
    )
    .expect("graph");
    let worker = AgentWorker::try_start(&graph)
        .expect("agent lease")
        .expect("available lease");
    let now = OffsetDateTime::parse("2026-08-25T00:00:00Z", &Rfc3339).expect("now");
    assert_eq!(
        wake::next_wake_at(&worker, &graph, now).expect("next wake"),
        now + Duration::milliseconds(1_000)
    );
}

#[test]
fn disabled_uninitialized_source_has_no_wake_capability() {
    let temporary = tempfile::tempdir().expect("temporary graph");
    let graph = TrustedGraphRoot::initialize(
        GraphPaths::new(temporary.path().join("graph")),
        "2026-08-25T00:00:00Z".to_owned(),
    )
    .expect("graph");
    let source: SourceDocument = toml::from_str(&format!(
        "schema_name = \"ffhn.source\"\nschema_version = 1\nsource_id = \"disabled\"\ndisplay_name = \"Disabled\"\nenabled = false\nescalate_after = 2\n[fetch]\nengine = \"file\"\nfile_path = {:?}\nmax_bytes = 1024\n[conditional]\nenabled = false\n[schedule]\ninterval_ms = 1000\nmin_interval_ms = 1000\n",
        temporary.path().join("source.json").to_string_lossy()
    ))
    .expect("source document");
    graph.create_source_document(&source).expect("source");
    let worker = AgentWorker::try_start(&graph)
        .expect("agent lease")
        .expect("available lease");
    let now = OffsetDateTime::parse("2026-08-25T00:00:00Z", &Rfc3339).expect("now");
    assert!(
        wake::source_wake_candidates(
            &worker,
            &graph,
            SourceId::new("disabled").expect("source id"),
            now,
        )
        .is_empty()
    );
}

#[test]
fn acquisition_due_boundary_includes_the_exact_scheduled_instant() {
    let now = OffsetDateTime::parse("2026-08-25T00:00:00Z", &Rfc3339).expect("now");
    assert!(acquisition_is_due(now, now));
    assert!(acquisition_is_due(now - Duration::nanoseconds(1), now));
    assert!(!acquisition_is_due(now + Duration::nanoseconds(1), now));
}

#[test]
fn record_fallback_preserves_success_and_schedules_failure() {
    let fallback = OffsetDateTime::parse("2026-08-25T00:00:01Z", &Rfc3339).expect("fallback");
    let mut candidates = Vec::new();
    assert_eq!(
        wake::records_or_fallback(
            Ok::<_, crate::CoreError>(vec![7_u8]),
            &mut candidates,
            fallback
        ),
        vec![7]
    );
    assert!(candidates.is_empty());
    assert!(
        wake::records_or_fallback::<u8>(
            Err(crate::CoreError::internal("unreadable records")),
            &mut candidates,
            fallback,
        )
        .is_empty()
    );
    assert_eq!(candidates, [fallback]);
}
