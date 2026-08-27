use std::io::{self, Write};

use super::*;

struct RefusingWriter;

struct StoppingWriter<'a>(&'a std::sync::atomic::AtomicBool);

impl Write for RefusingWriter {
    fn write(&mut self, _buffer: &[u8]) -> io::Result<usize> {
        Err(io::Error::other("refused"))
    }

    fn flush(&mut self) -> io::Result<()> {
        Err(io::Error::other("refused"))
    }
}

impl Write for StoppingWriter<'_> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.0.store(false, std::sync::atomic::Ordering::SeqCst);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn failing_next_wake(
    _worker: &mut graph::AgentWorker,
    _graph: &graph::TrustedGraphRoot,
    _now: String,
) -> Result<String, CoreError> {
    Err(CoreError::internal("wake calculation failed"))
}

#[test]
fn entrypoint_and_render_error_paths_preserve_closed_exit_code_contract() {
    let mut stderr = Vec::new();
    assert_eq!(
        run(["ffhn", "--help"], &mut RefusingWriter, &mut stderr),
        EXIT_CODE_FATAL
    );
    assert_eq!(
        run(["ffhn", "unknown"], &mut Vec::new(), &mut stderr),
        EXIT_CODE_USAGE
    );

    let value = serde_json::json!({"schema_name": "test"});
    assert_eq!(
        render_or_fatal(
            &mut Vec::new(),
            &mut stderr,
            &value,
            OutputFormat::Json,
            false,
        ),
        0
    );
    assert_eq!(
        render_or_fatal(
            &mut Vec::new(),
            &mut stderr,
            &value,
            OutputFormat::Json,
            true,
        ),
        EXIT_CODE_RUN_FAILED
    );
    assert_eq!(
        render_or_fatal(
            &mut RefusingWriter,
            &mut stderr,
            &value,
            OutputFormat::Json,
            false,
        ),
        EXIT_CODE_FATAL
    );
    assert_eq!(
        report_error(&mut stderr, CoreError::contract("source is busy")),
        EXIT_CODE_BUSY
    );
    assert_eq!(
        report_error(&mut stderr, CoreError::contract("agent is already running")),
        EXIT_CODE_BUSY
    );
    assert_eq!(
        report_error(&mut stderr, CoreError::contract("fatal")),
        EXIT_CODE_FATAL
    );
    assert_eq!(write_error(&mut RefusingWriter), EXIT_CODE_FATAL);
    assert_eq!(
        render_measurement_status(
            Err(CoreError::internal("status failed")),
            OutputFormat::Json,
            false,
            &mut Vec::new(),
            &mut stderr,
        ),
        EXIT_CODE_FATAL
    );
}

#[test]
fn graph_initialization_templates_and_clock_helpers_cover_relative_absolute_and_invalid_roots() {
    let temporary = tempfile::tempdir().expect("temporary");
    let empty = temporary.path().join("empty");
    std::fs::create_dir(&empty).expect("empty root");
    let graph = initialize_or_open_graph(&empty).expect("initialize empty");
    assert!(graph.validate_graph_documents().is_ok());
    assert!(initialize_or_open_graph(&empty).is_ok());

    let file = temporary.path().join("file");
    std::fs::write(&file, "not a directory").expect("file");
    assert!(initialize_or_open_graph(&file).is_err());

    let source_id = graph::SourceId::new("source").expect("source");
    let absolute = source_template(&source_id, temporary.path()).expect("absolute template");
    assert_eq!(absolute.source_id(), &source_id);
    let relative = source_template(&source_id, Path::new("relative-graph")).expect("relative");
    assert!(
        matches!(relative.fetch(), graph::SourceFetch::File { file_path, .. } if Path::new(file_path).is_absolute())
    );
    let measurement_id = graph::MeasurementId::new("measurement").expect("measurement");
    let measurement = measurement_template(&measurement_id).expect("measurement template");
    assert_eq!(measurement.measurement_id(), &measurement_id);
    assert_eq!(
        utc(OffsetDateTime::parse("2026-08-25T00:00:00Z", &Rfc3339).expect("time")),
        "2026-08-25T00:00:00Z"
    );
    assert!(!utc_now().is_empty());
}

#[test]
fn every_command_family_reports_an_unopenable_graph_without_panicking() {
    let missing = "/path/that/does/not/exist/ffhn-graph";
    for args in [
        vec![
            "ffhn",
            "measure",
            "--source",
            "shop",
            "--graph-root",
            missing,
        ],
        vec![
            "ffhn",
            "status",
            "--source",
            "shop",
            "--graph-root",
            missing,
        ],
        vec!["ffhn", "reset", "--source", "shop", "--graph-root", missing],
        vec!["ffhn", "validate", "--all", "--graph-root", missing],
        vec!["ffhn", "list", "--sources", "--graph-root", missing],
        vec![
            "ffhn",
            "new",
            "measurement",
            "--source",
            "shop",
            "--measurement",
            "price",
            "--graph-root",
            missing,
        ],
        vec!["ffhn", "agent", "tick", "--graph-root", missing],
        vec!["ffhn", "agent", "status", "--graph-root", missing],
        vec!["ffhn", "agent", "run", "--graph-root", missing],
    ] {
        let mut stderr = Vec::new();
        assert_eq!(run(args, &mut Vec::new(), &mut stderr), EXIT_CODE_FATAL);
        assert!(!stderr.is_empty());
    }
}

#[test]
fn agent_adapters_cover_busy_and_output_failure_routes_before_continuous_handler_installation() {
    let temporary = tempfile::tempdir().expect("temporary");
    let root = temporary.path().join("graph");
    let graph = graph::TrustedGraphRoot::initialize(
        graph::GraphPaths::new(&root),
        "2026-08-25T00:00:00Z".to_owned(),
    )
    .expect("graph");
    let options = crate::args::AgentOptions {
        graph_root: root.clone(),
        jobs: 1,
        output_format: OutputFormat::Json,
    };
    assert_eq!(
        super::agent::tick(options, &mut RefusingWriter, &mut Vec::new()),
        EXIT_CODE_FATAL
    );

    let options = crate::args::AgentOptions {
        graph_root: root.clone(),
        jobs: 1,
        output_format: OutputFormat::Json,
    };
    assert_eq!(
        super::agent::status(options, &mut RefusingWriter, &mut Vec::new()),
        EXIT_CODE_FATAL
    );

    let _worker = graph::AgentWorker::try_start(&graph)
        .expect("agent")
        .expect("lease");
    let options = crate::args::AgentOptions {
        graph_root: root,
        jobs: 1,
        output_format: OutputFormat::Json,
    };
    assert_eq!(
        super::agent::start_worker(&options, &mut Vec::new()).err(),
        Some(EXIT_CODE_BUSY)
    );
}

#[test]
fn agent_tick_and_status_surface_core_and_inventory_failures() {
    assert!(!super::agent::aggregate_status_failure(false, false));
    assert!(super::agent::aggregate_status_failure(false, true));
    assert!(super::agent::aggregate_status_failure(true, false));
    assert!(super::agent::aggregate_status_failure(true, true));
    let temporary = tempfile::tempdir().expect("temporary");
    let root = temporary.path().join("graph");
    graph::TrustedGraphRoot::initialize(
        graph::GraphPaths::new(&root),
        "2026-08-25T00:00:00Z".to_owned(),
    )
    .expect("graph");
    let zero_jobs = crate::args::AgentOptions {
        graph_root: root.clone(),
        jobs: 0,
        output_format: OutputFormat::Json,
    };
    assert_eq!(
        super::agent::tick(zero_jobs, &mut Vec::new(), &mut Vec::new()),
        EXIT_CODE_FATAL
    );

    std::fs::write(root.join("sources/not-a-directory"), "file").expect("source entry");
    assert_eq!(
        run(
            [
                "ffhn",
                "list",
                "--sources",
                "--graph-root",
                root.to_string_lossy().as_ref(),
            ],
            &mut Vec::new(),
            &mut Vec::new(),
        ),
        EXIT_CODE_FATAL
    );
    let status = crate::args::AgentOptions {
        graph_root: root.clone(),
        jobs: 1,
        output_format: OutputFormat::Json,
    };
    assert_eq!(
        super::agent::status(status, &mut Vec::new(), &mut Vec::new()),
        EXIT_CODE_FATAL
    );
    std::fs::remove_file(root.join("sources/not-a-directory")).expect("remove entry");
    std::fs::create_dir(root.join("sources/source")).expect("source");
    std::fs::remove_file(root.join(".ffhn-graph.json")).expect("remove identity");
    let status = crate::args::AgentOptions {
        graph_root: root,
        jobs: 1,
        output_format: OutputFormat::Json,
    };
    assert_eq!(
        super::agent::status(status, &mut Vec::new(), &mut Vec::new()),
        EXIT_CODE_FATAL
    );

    let root_file = temporary.path().join("root-file");
    std::fs::write(&root_file, "file").expect("root file");
    assert_eq!(
        run(
            [
                "ffhn",
                "new",
                "source",
                "--source",
                "other",
                "--graph-root",
                root_file.to_string_lossy().as_ref(),
            ],
            &mut Vec::new(),
            &mut Vec::new(),
        ),
        EXIT_CODE_FATAL
    );
}

#[test]
fn continuous_agent_loop_covers_tick_render_shutdown_wake_and_handler_failures() {
    use std::{
        cell::Cell,
        sync::atomic::{AtomicBool, Ordering},
    };

    let temporary = tempfile::tempdir().expect("temporary");
    let root = temporary.path().join("graph");
    let graph = graph::TrustedGraphRoot::initialize(
        graph::GraphPaths::new(&root),
        "2026-08-25T00:00:00Z".to_owned(),
    )
    .expect("graph");
    let options = || crate::args::AgentOptions {
        graph_root: root.clone(),
        jobs: 1,
        output_format: OutputFormat::Json,
    };

    {
        let mut worker = graph::AgentWorker::try_start(&graph)
            .expect("agent")
            .expect("lease");
        let stopped = AtomicBool::new(false);
        assert_eq!(
            super::agent::run_worker_loop(
                options(),
                &graph,
                &mut worker,
                &stopped,
                &mut Vec::new(),
                &mut Vec::new(),
            ),
            0
        );
    }
    {
        let mut worker = graph::AgentWorker::try_start(&graph)
            .expect("agent")
            .expect("lease");
        let running = AtomicBool::new(true);
        assert_eq!(
            super::agent::run_worker_loop_with_hooks(
                options(),
                &graph,
                &mut worker,
                &running,
                &mut StoppingWriter(&running),
                &mut Vec::new(),
                super::agent::AgentLoopHooks {
                    wait: |_| {},
                    next_wake: failing_next_wake,
                },
            ),
            0
        );
    }

    {
        let mut worker = graph::AgentWorker::try_start(&graph)
            .expect("agent")
            .expect("lease");
        let running = AtomicBool::new(true);
        assert_eq!(
            super::agent::run_worker_loop_with_wait(
                options(),
                &graph,
                &mut worker,
                &running,
                &mut RefusingWriter,
                &mut Vec::new(),
                |_| running.store(false, Ordering::SeqCst),
            ),
            EXIT_CODE_FATAL
        );
    }
    {
        let mut worker = graph::AgentWorker::try_start(&graph)
            .expect("agent")
            .expect("lease");
        let running = AtomicBool::new(true);
        let mut stderr = Vec::new();
        assert_eq!(
            super::agent::run_worker_loop_with_hooks(
                options(),
                &graph,
                &mut worker,
                &running,
                &mut Vec::new(),
                &mut stderr,
                super::agent::AgentLoopHooks {
                    wait: |_| {},
                    next_wake: failing_next_wake,
                },
            ),
            EXIT_CODE_FATAL
        );
        assert!(
            String::from_utf8(stderr)
                .expect("wake error stderr")
                .contains("wake calculation failed")
        );
    }
    {
        let mut worker = graph::AgentWorker::try_start(&graph)
            .expect("agent")
            .expect("lease");
        let running = AtomicBool::new(true);
        let mut invalid = options();
        invalid.jobs = 0;
        assert_eq!(
            super::agent::run_worker_loop(
                invalid,
                &graph,
                &mut worker,
                &running,
                &mut Vec::new(),
                &mut Vec::new(),
            ),
            EXIT_CODE_FATAL
        );
    }
    {
        let mut worker = graph::AgentWorker::try_start(&graph)
            .expect("agent")
            .expect("lease");
        let running = AtomicBool::new(true);
        assert_eq!(
            super::agent::run_worker_loop_with_wait(
                options(),
                &graph,
                &mut worker,
                &running,
                &mut Vec::new(),
                &mut Vec::new(),
                |_| running.store(false, Ordering::SeqCst),
            ),
            0
        );
    }
    {
        let mut worker = graph::AgentWorker::try_start(&graph)
            .expect("agent")
            .expect("lease");
        let running = AtomicBool::new(true);
        let waits = Cell::new(0_u8);
        assert_eq!(
            super::agent::run_worker_loop_with_wait(
                options(),
                &graph,
                &mut worker,
                &running,
                &mut Vec::new(),
                &mut Vec::new(),
                |_| {
                    let _ = std::fs::remove_dir(root.join("sources"));
                    let next = waits.get() + 1;
                    waits.set(next);
                    if next == 2 {
                        running.store(false, Ordering::SeqCst);
                    }
                },
            ),
            EXIT_CODE_FATAL
        );
        std::fs::create_dir(root.join("sources")).expect("restore sources");
    }

    std::fs::remove_file(root.join("agent.toml")).expect("remove agent config");
    assert_eq!(
        super::agent::start_worker(&options(), &mut Vec::new()).err(),
        Some(EXIT_CODE_FATAL)
    );
    std::fs::write(
        root.join("agent.toml"),
        "schema_name = \"ffhn.agent\"\nschema_version = 1\n",
    )
    .expect("restore agent");
    ctrlc::set_handler(|| {}).expect("first handler");
    assert_eq!(
        super::agent::run(options(), &mut Vec::new(), &mut Vec::new()),
        EXIT_CODE_FATAL
    );
}
