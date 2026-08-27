//! CLI lifecycle adapter for the graph-lease-owning agent worker.

use std::{
    io::Write,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

use ffhn_core::{CoreError, graph};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::{EXIT_CODE_BUSY, args::AgentOptions, error::write_cli_error, render::render_document};

use super::{open_graph, render_or_fatal, report_error, utc, utc_now, write_error};

/// Executes one finite agent tick.
pub(super) fn tick(options: AgentOptions, stdout: &mut impl Write, stderr: &mut impl Write) -> i32 {
    let graph = match open_graph(&options.graph_root) {
        Ok(graph) => graph,
        Err(error) => return report_error(stderr, error),
    };
    match graph::agent_tick_with_jobs(&graph, utc_now(), options.jobs) {
        Ok(result) => {
            let failed = result.has_handled_failure();
            render_or_fatal(
                stdout,
                stderr,
                &graph::AgentTickReport::from(&result),
                options.output_format,
                failed,
            )
        }
        Err(error) => report_error(stderr, error),
    }
}

/// Renders a status snapshot for every configured source.
pub(super) fn status(
    options: AgentOptions,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> i32 {
    let graph = match open_graph(&options.graph_root) {
        Ok(graph) => graph,
        Err(error) => return report_error(stderr, error),
    };
    let _jobs = options.jobs;
    let mut source_ids = match graph.source_ids() {
        Ok(ids) => ids,
        Err(error) => return report_error(stderr, error),
    };
    source_ids.sort_unstable();
    let mut reports = Vec::new();
    let mut failed = false;
    for source_id in source_ids {
        match graph::status_source(&graph, source_id, None) {
            Ok(result) => {
                failed = aggregate_status_failure(failed, super::status_failed(&result));
                reports.push(graph::GraphSourceStatusReport::from(&result));
            }
            Err(error) => return report_error(stderr, error),
        }
    }
    render_or_fatal(
        stdout,
        stderr,
        &graph::AgentStatusReport::new(reports),
        options.output_format,
        failed,
    )
}

pub(super) const fn aggregate_status_failure(accumulated: bool, next: bool) -> bool {
    accumulated || next
}

/// Runs until a handled termination signal, waking at the earliest permitted source or retry fact.
pub(super) fn run(options: AgentOptions, stdout: &mut impl Write, stderr: &mut impl Write) -> i32 {
    let (graph, mut worker) = match start_worker(&options, stderr) {
        Ok(started) => started,
        Err(exit_code) => return exit_code,
    };
    let running = Arc::new(AtomicBool::new(true));
    let signal = Arc::clone(&running);
    let wake_thread = thread::current();
    if let Err(error) = ctrlc::set_handler(move || {
        signal.store(false, Ordering::SeqCst);
        wake_thread.unpark();
    }) {
        return report_error(
            stderr,
            CoreError::internal(format!("cannot install shutdown handler: {error}")),
        );
    }
    run_worker_loop(options, &graph, &mut worker, &running, stdout, stderr)
}

pub(super) fn start_worker(
    options: &AgentOptions,
    stderr: &mut impl Write,
) -> Result<(graph::TrustedGraphRoot, graph::AgentWorker), i32> {
    let graph = open_graph(&options.graph_root).map_err(|error| report_error(stderr, error))?;
    let worker = graph::AgentWorker::try_start(&graph)
        .map_err(|error| report_error(stderr, error))?
        .ok_or_else(|| {
            let _ = write_cli_error(stderr, "agent is already running");
            EXIT_CODE_BUSY
        })?;
    Ok((graph, worker))
}

pub(super) fn run_worker_loop(
    options: AgentOptions,
    graph: &graph::TrustedGraphRoot,
    worker: &mut graph::AgentWorker,
    running: &AtomicBool,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> i32 {
    run_worker_loop_with_wait(
        options,
        graph,
        worker,
        running,
        stdout,
        stderr,
        thread::park_timeout,
    )
}

pub(super) fn run_worker_loop_with_wait(
    options: AgentOptions,
    graph: &graph::TrustedGraphRoot,
    worker: &mut graph::AgentWorker,
    running: &AtomicBool,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
    mut wait: impl FnMut(Duration),
) -> i32 {
    run_worker_loop_with_hooks(
        options,
        graph,
        worker,
        running,
        stdout,
        stderr,
        AgentLoopHooks {
            wait: &mut wait,
            next_wake: worker_next_wake,
        },
    )
}

fn worker_next_wake(
    worker: &mut graph::AgentWorker,
    graph: &graph::TrustedGraphRoot,
    now: String,
) -> Result<String, CoreError> {
    worker.next_wake_at(graph, now)
}

pub(super) struct AgentLoopHooks<Wait, NextWake> {
    pub(super) wait: Wait,
    pub(super) next_wake: NextWake,
}

pub(super) fn run_worker_loop_with_hooks<Wait, NextWake>(
    options: AgentOptions,
    graph: &graph::TrustedGraphRoot,
    worker: &mut graph::AgentWorker,
    running: &AtomicBool,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
    mut hooks: AgentLoopHooks<Wait, NextWake>,
) -> i32
where
    Wait: FnMut(Duration),
    NextWake: FnMut(
        &mut graph::AgentWorker,
        &graph::TrustedGraphRoot,
        String,
    ) -> Result<String, CoreError>,
{
    while running.load(Ordering::SeqCst) {
        match worker.tick_with_jobs(graph, utc_now(), options.jobs) {
            Ok(result) => {
                if render_document(
                    stdout,
                    &graph::AgentTickReport::from(&result),
                    options.output_format,
                )
                .is_err()
                {
                    return write_error(stderr);
                }
            }
            Err(error) => return report_error(stderr, error),
        }
        if !running.load(Ordering::SeqCst) {
            break;
        }
        let now = OffsetDateTime::now_utc();
        let wake_at = match (hooks.next_wake)(worker, graph, utc(now))
            .and_then(|wake_at| OffsetDateTime::parse(&wake_at, &Rfc3339).map_err(CoreError::from))
        {
            Ok(wake_at) => wake_at,
            Err(error) => return report_error(stderr, error),
        };
        let delay_ms = (wake_at - now).whole_milliseconds().max(1);
        (hooks.wait)(Duration::from_millis(
            u64::try_from(delay_ms).expect("positive wake delay fits u64 milliseconds"),
        ));
    }
    0
}
