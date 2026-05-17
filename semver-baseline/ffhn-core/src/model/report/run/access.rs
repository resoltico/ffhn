use super::*;

impl RunFetchSection {
    /// Returns the fetch engine used for the run.
    pub const fn engine(&self) -> FetchEngine {
        self.engine
    }

    /// Returns the final URL after redirects when one exists.
    pub fn final_url(&self) -> Option<&str> {
        self.final_url.as_deref()
    }

    /// Returns the HTTP status when one exists.
    pub const fn http_status(&self) -> Option<u16> {
        self.http_status
    }

    /// Returns the response content type when one exists.
    pub fn content_type(&self) -> Option<&str> {
        self.content_type.as_deref()
    }

    /// Returns the number of bytes actually read when one exists.
    pub const fn bytes_read(&self) -> Option<usize> {
        self.bytes_read
    }

    /// Returns the wall-clock duration in milliseconds.
    pub const fn duration_ms(&self) -> u64 {
        self.duration_ms
    }
}

impl RunExtractionSection {
    /// Returns the compare-source digest.
    pub fn compare_source_sha256(&self) -> &str {
        &self.compare_source_sha256
    }

    /// Returns the persisted outer-HTML digest.
    pub fn outer_html_sha256(&self) -> &str {
        &self.outer_html_sha256
    }

    /// Returns the extraction strategy kind.
    pub const fn selection_kind(&self) -> SelectionKind {
        self.selection_kind
    }

    /// Returns the selection mode.
    pub const fn selection_match(&self) -> SelectionMatch {
        self.selection_match
    }

    /// Returns the total candidate count.
    pub const fn candidate_count(&self) -> usize {
        self.candidate_count
    }

    /// Returns the selected one-based candidate index.
    pub const fn selected_candidate_index(&self) -> usize {
        self.selected_candidate_index
    }

    /// Returns any warning codes surfaced through FFHN's extractor seam.
    pub fn warning_codes(&self) -> &[String] {
        &self.warning_codes
    }

    /// Returns the extraction-stage duration in milliseconds.
    pub const fn duration_ms(&self) -> u64 {
        self.duration_ms
    }
}

impl RunCompareSection {
    /// Returns the canonicalizers applied in order.
    pub fn canonicalizers(&self) -> &[String] {
        &self.canonicalizers
    }

    /// Returns the compare-stage duration in milliseconds.
    pub const fn duration_ms(&self) -> u64 {
        self.duration_ms
    }
}

impl RunPersistSection {
    pub(crate) const fn from_writes(
        state_commit_duration_ms: u64,
        state_commit: PersistWriteStatus,
        last_run_write_duration_ms: u64,
        last_run_write: PersistWriteStatus,
    ) -> Self {
        Self {
            state_commit,
            state_commit_duration_ms,
            last_run_write,
            last_run_write_duration_ms,
        }
    }

    /// Returns the primary persistence transaction duration in milliseconds.
    pub const fn state_commit_duration_ms(&self) -> u64 {
        self.state_commit_duration_ms
    }

    /// Returns the `last_run.json` write duration in milliseconds.
    pub const fn last_run_write_duration_ms(&self) -> u64 {
        self.last_run_write_duration_ms
    }

    /// Returns the total persist duration across both durable phases.
    pub const fn total_duration_ms(&self) -> u64 {
        self.state_commit_duration_ms + self.last_run_write_duration_ms
    }

    /// Returns the primary persistence transaction result.
    pub const fn state_commit(&self) -> &PersistWriteStatus {
        &self.state_commit
    }

    /// Returns the write result for `last_run.json`.
    pub const fn last_run_write(&self) -> &PersistWriteStatus {
        &self.last_run_write
    }

    /// Returns whether FFHN wrote `state.json`.
    #[cfg(test)]
    pub const fn committed_state(&self) -> bool {
        self.state_commit.is_written()
    }

    /// Returns whether FFHN wrote `last_run.json`.
    #[cfg(test)]
    pub const fn wrote_last_run(&self) -> bool {
        self.last_run_write.is_written()
    }

    /// Returns whether either persist write failed.
    pub const fn has_failure(&self) -> bool {
        self.state_commit.is_failed() || self.last_run_write.is_failed()
    }

    /// Returns the first persist failure detail when one exists.
    pub const fn error(&self) -> Option<&ProcessErrorDetail> {
        if let Some(error) = self.state_commit.error() {
            return Some(error);
        }
        self.last_run_write.error()
    }
}

impl RunChangeRegion {
    /// Returns the one-based start line in the previous compare value.
    pub const fn previous_start_line(&self) -> usize {
        self.previous_start_line
    }

    /// Returns the number of previous lines in the changed region.
    pub const fn previous_line_count(&self) -> usize {
        self.previous_line_count
    }

    /// Returns the one-based start line in the current compare value.
    pub const fn current_start_line(&self) -> usize {
        self.current_start_line
    }

    /// Returns the number of current lines in the changed region.
    pub const fn current_line_count(&self) -> usize {
        self.current_line_count
    }

    /// Returns the previous excerpt when one exists.
    pub fn previous_excerpt(&self) -> Option<&str> {
        self.previous_excerpt.as_deref()
    }

    /// Returns the current excerpt when one exists.
    pub fn current_excerpt(&self) -> Option<&str> {
        self.current_excerpt.as_deref()
    }

    /// Returns the previous excerpt digest when one exists.
    pub fn previous_excerpt_sha256(&self) -> Option<&str> {
        self.previous_excerpt_sha256.as_deref()
    }

    /// Returns the current excerpt digest when one exists.
    pub fn current_excerpt_sha256(&self) -> Option<&str> {
        self.current_excerpt_sha256.as_deref()
    }
}

impl RunChangeSection {
    /// Returns the change discriminator.
    pub const fn kind(&self) -> ChangeKind {
        self.kind
    }

    /// Returns the previous compare-value byte length when one exists.
    pub const fn previous_compare_bytes(&self) -> Option<usize> {
        self.previous_compare_bytes
    }

    /// Returns the current compare-value byte length.
    pub const fn current_compare_bytes(&self) -> usize {
        self.current_compare_bytes
    }

    /// Returns the previous compare-value line count when one exists.
    pub const fn previous_compare_line_count(&self) -> Option<usize> {
        self.previous_compare_line_count
    }

    /// Returns the current compare-value line count.
    pub const fn current_compare_line_count(&self) -> usize {
        self.current_compare_line_count
    }

    /// Returns the number of equal leading lines.
    pub const fn common_prefix_lines(&self) -> usize {
        self.common_prefix_lines
    }

    /// Returns the number of equal trailing lines.
    pub const fn common_suffix_lines(&self) -> usize {
        self.common_suffix_lines
    }

    /// Returns the replaced line region when one exists.
    pub fn changed_region(&self) -> Option<&RunChangeRegion> {
        self.changed_region.as_ref()
    }
}

impl RunReport {
    /// Returns the frozen schema name.
    pub fn schema_name(&self) -> &str {
        &self.schema_name
    }

    /// Returns the frozen schema version.
    pub fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Returns the report digest.
    pub fn run_report_digest_sha256(&self) -> &str {
        &self.run_report_digest_sha256
    }

    /// Returns the persisted target id string.
    pub fn target_id(&self) -> &str {
        self.target_id.as_str()
    }

    /// Returns the parsed target display name when FFHN could trust the target document.
    pub fn display_name(&self) -> Option<&str> {
        self.display_name.as_deref()
    }

    /// Returns the run start timestamp.
    pub fn run_started_at(&self) -> &str {
        &self.run_started_at
    }

    /// Returns the run finish timestamp.
    pub fn run_finished_at(&self) -> &str {
        &self.run_finished_at
    }

    /// Returns the run mode.
    pub fn run_mode(&self) -> RunMode {
        self.run_mode
    }

    /// Returns the structured run result.
    pub const fn result(&self) -> &RunResult {
        &self.result
    }

    /// Returns the run outcome.
    pub fn run_outcome(&self) -> RunOutcome {
        self.result.outcome()
    }

    /// Returns the failure class when one exists.
    pub fn failure_class(&self) -> Option<FailureClass> {
        self.result.failure_class()
    }

    /// Returns the run-failure cause when one exists.
    pub fn failure_cause(&self) -> Option<RunFailureCause> {
        self.result.failure_cause()
    }

    /// Returns the structured primary failure detail when one exists.
    pub fn error_detail(&self) -> Option<&ProcessErrorDetail> {
        self.result.error_detail()
    }

    /// Returns the compare basis.
    pub fn compare_basis(&self) -> CompareBasis {
        self.compare_basis
    }

    /// Returns the previous compare digest when one exists.
    pub fn previous_compare_digest_sha256(&self) -> Option<&str> {
        self.previous_compare_digest_sha256.as_deref()
    }

    /// Returns the current compare digest when one exists.
    pub fn current_compare_digest_sha256(&self) -> Option<&str> {
        self.current_compare_digest_sha256.as_deref()
    }

    /// Returns the durable baseline phase before the run.
    pub fn baseline_phase_before_run(&self) -> BaselinePhase {
        self.baseline_phase_before_run
    }

    /// Returns the durable baseline phase after the run.
    pub fn baseline_phase_after_run(&self) -> BaselinePhase {
        self.baseline_phase_after_run
    }

    /// Returns a coherent body view instead of independent optional stage sections.
    pub fn body(&self) -> RunBodyView<'_> {
        match (&self.fetch, &self.extraction, &self.compare, &self.change) {
            (Some(fetch), Some(extraction), Some(compare), Some(change)) => {
                RunBodyView::Reportable(ReportableRunBodyView {
                    fetch,
                    extraction,
                    compare,
                    change,
                    previous_compare_digest_sha256: self.previous_compare_digest_sha256(),
                    current_compare_digest_sha256: self
                        .current_compare_digest_sha256()
                        .expect("validated reportable bodies carry the current digest"),
                })
            }
            (Some(fetch), Some(extraction), Some(compare), None) => {
                RunBodyView::FetchExtractionCompare {
                    fetch: RunFetchView(fetch),
                    extraction,
                    compare: RunCompareView(compare),
                }
            }
            (Some(fetch), Some(extraction), None, None) => RunBodyView::FetchAndExtraction {
                fetch: RunFetchView(fetch),
                extraction,
            },
            (Some(fetch), None, None, None) => RunBodyView::Fetch {
                fetch: RunFetchView(fetch),
            },
            (None, None, None, None) => RunBodyView::None,
            _ => RunBodyView::None,
        }
    }

    /// Returns a successful-run view when the run outcome is successful.
    pub fn successful(&self) -> Option<SuccessfulRunReportView<'_>> {
        matches!(
            self.run_outcome(),
            RunOutcome::Initialized | RunOutcome::Changed | RunOutcome::Unchanged
        )
        .then_some(SuccessfulRunReportView { report: self })
    }

    /// Returns a failed-run view when the run outcome is failed.
    pub fn failed(&self) -> Option<FailedRunReportView<'_>> {
        matches!(
            self.run_outcome(),
            RunOutcome::FailedTransient | RunOutcome::FailedPermanent
        )
        .then_some(FailedRunReportView { report: self })
    }

    /// Returns a read-only fetch view when one exists.
    pub fn fetch(&self) -> Option<RunFetchView<'_>> {
        self.fetch.as_ref().map(RunFetchView)
    }

    /// Returns the extraction subsection when one exists.
    pub fn extraction(&self) -> Option<&RunExtractionSection> {
        self.extraction.as_ref()
    }

    /// Returns a read-only compare view when one exists.
    pub fn compare(&self) -> Option<RunCompareView<'_>> {
        self.compare.as_ref().map(RunCompareView)
    }

    /// Returns the change subsection when one exists.
    pub fn change(&self) -> Option<&RunChangeSection> {
        self.change.as_ref()
    }

    /// Returns a read-only persist view.
    pub const fn persist(&self) -> RunPersistView<'_> {
        RunPersistView(&self.persist)
    }

    /// Returns the best-effort notification deliveries.
    pub fn notifications(
        &self,
    ) -> impl ExactSizeIterator<Item = RunNotificationDeliveryView<'_>> + '_ {
        self.notifications.iter().map(RunNotificationDeliveryView)
    }

    /// Returns any reserved extensions.
    pub fn extensions(&self) -> Option<&std::collections::BTreeMap<String, serde_json::Value>> {
        self.extensions.as_ref()
    }
}

impl RunResult {
    /// Returns whether this result represents a failed run.
    pub const fn is_failure(&self) -> bool {
        matches!(
            self,
            Self::FailedTransient { .. } | Self::FailedPermanent { .. }
        )
    }
}

/// Read-only fetch view.
#[derive(Clone, Copy, Debug)]
pub struct RunFetchView<'a>(pub(crate) &'a RunFetchSection);

/// Read-only compare view.
#[derive(Clone, Copy, Debug)]
pub struct RunCompareView<'a>(pub(crate) &'a RunCompareSection);

/// Read-only persist view.
#[derive(Clone, Copy, Debug)]
pub struct RunPersistView<'a>(pub(crate) &'a RunPersistSection);

/// Read-only notification-delivery view.
#[derive(Clone, Copy, Debug)]
pub struct RunNotificationDeliveryView<'a>(pub(crate) &'a RunNotificationDelivery);

impl<'a> ReportableRunBodyView<'a> {
    /// Returns the completed fetch section.
    pub const fn fetch(self) -> RunFetchView<'a> {
        RunFetchView(self.fetch)
    }

    /// Returns the completed extraction section.
    pub fn extraction(self) -> &'a RunExtractionSection {
        self.extraction
    }

    /// Returns the completed compare section.
    pub const fn compare(self) -> RunCompareView<'a> {
        RunCompareView(self.compare)
    }

    /// Returns the completed change section.
    pub fn change(self) -> &'a RunChangeSection {
        self.change
    }

    /// Returns the previous compare digest when one exists.
    pub const fn previous_compare_digest_sha256(self) -> Option<&'a str> {
        self.previous_compare_digest_sha256
    }

    /// Returns the current compare digest.
    pub fn current_compare_digest_sha256(self) -> &'a str {
        self.current_compare_digest_sha256
    }
}

impl<'a> FailedRunReportView<'a> {
    /// Returns the stable run-failure cause.
    pub fn failure_cause(self) -> RunFailureCause {
        self.report
            .failure_cause()
            .expect("failed-run views always carry a failure cause")
    }

    /// Returns the structured primary failure detail.
    pub fn error_detail(self) -> &'a ProcessErrorDetail {
        self.report
            .error_detail()
            .expect("failed-run views always carry an error detail")
    }

    /// Returns the coherent run body that completed before the failure.
    pub fn body(self) -> RunBodyView<'a> {
        self.report.body()
    }
}

impl<'a> SuccessfulRunReportView<'a> {
    /// Returns the coherent reportable body for a successful run.
    pub fn body(self) -> ReportableRunBodyView<'a> {
        match self.report.body() {
            RunBodyView::Reportable(body) => body,
            _ => unreachable!("successful runs always carry a full reportable body"),
        }
    }
}

impl<'a> RunFetchView<'a> {
    /// Returns the fetch engine used for the run.
    pub const fn engine(self) -> FetchEngine {
        self.0.engine()
    }

    /// Returns the final URL after redirects when one exists.
    pub fn final_url(self) -> Option<&'a str> {
        self.0.final_url()
    }

    /// Returns the HTTP status when one exists.
    pub const fn http_status(self) -> Option<u16> {
        self.0.http_status()
    }

    /// Returns the response content type when one exists.
    pub fn content_type(self) -> Option<&'a str> {
        self.0.content_type()
    }

    /// Returns the number of bytes actually read when one exists.
    pub const fn bytes_read(self) -> Option<usize> {
        self.0.bytes_read()
    }

    /// Returns the wall-clock duration in milliseconds.
    pub const fn duration_ms(self) -> u64 {
        self.0.duration_ms()
    }
}

impl<'a> RunCompareView<'a> {
    /// Returns the canonicalizers applied in order.
    pub fn canonicalizers(self) -> &'a [String] {
        self.0.canonicalizers()
    }

    /// Returns the compare-stage duration in milliseconds.
    pub const fn duration_ms(self) -> u64 {
        self.0.duration_ms()
    }
}

impl<'a> RunPersistView<'a> {
    /// Returns the primary persistence transaction duration in milliseconds.
    pub const fn state_commit_duration_ms(self) -> u64 {
        self.0.state_commit_duration_ms()
    }

    /// Returns the `last_run.json` write duration in milliseconds.
    pub const fn last_run_write_duration_ms(self) -> u64 {
        self.0.last_run_write_duration_ms()
    }

    /// Returns the total persist duration across both durable phases.
    pub const fn total_duration_ms(self) -> u64 {
        self.0.total_duration_ms()
    }

    /// Returns the primary persistence transaction result.
    pub const fn state_commit(self) -> &'a PersistWriteStatus {
        self.0.state_commit()
    }

    /// Returns the write result for `last_run.json`.
    pub const fn last_run_write(self) -> &'a PersistWriteStatus {
        self.0.last_run_write()
    }

    /// Returns whether either persist write failed.
    pub const fn has_failure(self) -> bool {
        self.0.has_failure()
    }

    /// Returns the first persist failure detail when one exists.
    pub const fn error(self) -> Option<&'a ProcessErrorDetail> {
        self.0.error()
    }

    /// Returns whether FFHN wrote `state.json`.
    #[cfg(test)]
    pub const fn committed_state(self) -> bool {
        self.0.committed_state()
    }

    /// Returns whether FFHN wrote `last_run.json`.
    #[cfg(test)]
    pub const fn wrote_last_run(self) -> bool {
        self.0.wrote_last_run()
    }
}

impl<'a> RunNotificationDeliveryView<'a> {
    /// Returns the route name from `target.toml`.
    pub fn route_name(self) -> &'a str {
        self.0.route_name()
    }

    /// Returns the delivery duration in milliseconds.
    pub const fn duration_ms(self) -> u64 {
        self.0.duration_ms()
    }

    /// Returns the stable delivery status.
    pub const fn status(self) -> NotificationDeliveryStatus {
        self.0.status()
    }

    /// Returns the exit status when one exists.
    pub const fn exit_code(self) -> Option<i32> {
        self.0.exit_code()
    }

    /// Returns the best-effort failure detail when delivery failed.
    pub fn error(self) -> Option<&'a str> {
        self.0.error()
    }
}
