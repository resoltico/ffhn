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
    /// Returns the frozen HTMLCut interop profile.
    pub fn interop_profile(&self) -> &str {
        &self.interop_profile
    }

    /// Returns the exact HTMLCut plan digest.
    pub fn htmlcut_plan_digest_sha256(&self) -> &str {
        &self.htmlcut_plan_digest_sha256
    }

    /// Returns the exact HTMLCut result digest.
    pub fn htmlcut_result_digest_sha256(&self) -> &str {
        &self.htmlcut_result_digest_sha256
    }

    /// Returns the comparison-input digest.
    pub fn comparison_input_sha256(&self) -> &str {
        &self.comparison_input_sha256
    }

    /// Returns the persisted outer-HTML digest.
    pub fn outer_html_sha256(&self) -> &str {
        &self.outer_html_sha256
    }

    /// Returns the echoed extraction strategy kind.
    pub const fn strategy_kind(&self) -> SelectionKind {
        self.strategy_kind
    }

    /// Returns the echoed selection mode.
    pub const fn selection_mode(&self) -> SelectionMatch {
        self.selection_mode
    }

    /// Returns the echoed output kind.
    pub const fn output_kind(&self) -> OutputKind {
        self.output_kind
    }

    /// Returns the total candidate count.
    pub const fn candidate_count(&self) -> usize {
        self.candidate_count
    }

    /// Returns the selected one-based candidate index.
    pub const fn selected_candidate_index(&self) -> usize {
        self.selected_candidate_index
    }

    /// Returns any warning codes emitted by HTMLCut.
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
        duration_ms: u64,
        state_write: PersistWriteStatus,
        last_run_write: PersistWriteStatus,
    ) -> Self {
        Self {
            duration_ms,
            state_write,
            last_run_write,
        }
    }

    /// Returns the persist-stage duration in milliseconds.
    pub const fn duration_ms(&self) -> u64 {
        self.duration_ms
    }

    /// Returns the write result for `state.json`.
    pub const fn state_write(&self) -> &PersistWriteStatus {
        &self.state_write
    }

    /// Returns the write result for `last_run.json`.
    pub const fn last_run_write(&self) -> &PersistWriteStatus {
        &self.last_run_write
    }

    /// Returns whether FFHN wrote `state.json`.
    #[cfg(test)]
    pub const fn wrote_state(&self) -> bool {
        self.state_write.is_written()
    }

    /// Returns whether FFHN wrote `last_run.json`.
    #[cfg(test)]
    pub const fn wrote_last_run(&self) -> bool {
        self.last_run_write.is_written()
    }

    /// Returns whether either persist write failed.
    pub const fn has_failure(&self) -> bool {
        self.state_write.is_failed() || self.last_run_write.is_failed()
    }

    /// Returns the first persist failure detail when one exists.
    pub const fn error(&self) -> Option<&ProcessErrorDetail> {
        if let Some(error) = self.state_write.error() {
            return Some(error);
        }
        self.last_run_write.error()
    }
}

impl RunChangeRegion {
    /// Returns the one-based start line in the previous canonical text.
    pub const fn previous_start_line(&self) -> usize {
        self.previous_start_line
    }

    /// Returns the number of previous lines in the changed region.
    pub const fn previous_line_count(&self) -> usize {
        self.previous_line_count
    }

    /// Returns the one-based start line in the current canonical text.
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

    /// Returns the previous canonical-text byte length when one exists.
    pub const fn previous_text_bytes(&self) -> Option<usize> {
        self.previous_text_bytes
    }

    /// Returns the current canonical-text byte length.
    pub const fn current_text_bytes(&self) -> usize {
        self.current_text_bytes
    }

    /// Returns the previous canonical-text line count when one exists.
    pub const fn previous_line_count(&self) -> Option<usize> {
        self.previous_line_count
    }

    /// Returns the current canonical-text line count.
    pub const fn current_line_count(&self) -> usize {
        self.current_line_count
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

    /// Returns the run mode.
    pub fn run_mode(&self) -> RunMode {
        self.run_mode
    }

    /// Returns the run outcome.
    pub fn run_outcome(&self) -> RunOutcome {
        self.run_outcome
    }

    /// Returns the reason code.
    pub fn reason_code(&self) -> ReasonCode {
        self.reason_code
    }

    /// Returns the persisted target id string.
    pub fn target_id(&self) -> &str {
        self.target_id.as_str()
    }

    /// Returns the run start timestamp.
    pub fn run_started_at(&self) -> &str {
        &self.run_started_at
    }

    /// Returns the run finish timestamp.
    pub fn run_finished_at(&self) -> &str {
        &self.run_finished_at
    }

    /// Returns the failure class when one exists.
    pub fn failure_class(&self) -> Option<FailureClass> {
        self.failure_class
    }

    /// Returns the structured primary failure detail when one exists.
    pub fn error_detail(&self) -> Option<&ProcessErrorDetail> {
        self.error_detail.as_ref()
    }

    /// Returns the target status after the run.
    pub fn target_status_after_run(&self) -> TargetStatus {
        self.target_status_after_run
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

    /// Returns the state phase before the run.
    pub fn state_phase_before_run(&self) -> StatePhase {
        self.state_phase_before_run
    }

    /// Returns the state phase after the run.
    pub fn state_phase_after_run(&self) -> StatePhase {
        self.state_phase_after_run
    }

    /// Returns the fetch subsection when one exists.
    pub fn fetch(&self) -> Option<RunFetchView<'_>> {
        self.fetch.as_ref().map(RunFetchView)
    }

    /// Returns the extraction subsection when one exists.
    pub fn extraction(&self) -> Option<&RunExtractionSection> {
        self.extraction.as_ref()
    }

    /// Returns the compare subsection when one exists.
    pub fn compare(&self) -> Option<RunCompareView<'_>> {
        self.compare.as_ref().map(RunCompareView)
    }

    /// Returns the change subsection when one exists.
    pub fn change(&self) -> Option<&RunChangeSection> {
        self.change.as_ref()
    }

    /// Returns the persist subsection.
    pub fn persist(&self) -> RunPersistView<'_> {
        RunPersistView(&self.persist)
    }

    /// Returns the attempted notification deliveries.
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

/// Public read-only projection of one fetch subsection.
#[derive(Clone, Copy, Debug)]
pub struct RunFetchView<'a>(pub(super) &'a RunFetchSection);

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

/// Public read-only projection of one compare subsection.
#[derive(Clone, Copy, Debug)]
pub struct RunCompareView<'a>(pub(super) &'a RunCompareSection);

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

/// Public read-only projection of one persist subsection.
#[derive(Clone, Copy, Debug)]
pub struct RunPersistView<'a>(pub(super) &'a RunPersistSection);

impl<'a> RunPersistView<'a> {
    /// Returns the persist-stage duration in milliseconds.
    pub const fn duration_ms(self) -> u64 {
        self.0.duration_ms()
    }

    /// Returns the write result for `state.json`.
    pub const fn state_write(self) -> &'a PersistWriteStatus {
        self.0.state_write()
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
}

/// Public read-only projection of one notification delivery.
#[derive(Clone, Copy, Debug)]
pub struct RunNotificationDeliveryView<'a>(pub(super) &'a RunNotificationDelivery);

impl<'a> RunNotificationDeliveryView<'a> {
    /// Returns the hook name from `target.toml`.
    pub fn hook_name(self) -> &'a str {
        self.0.hook_name()
    }

    /// Returns the delivery duration in milliseconds.
    pub const fn duration_ms(self) -> u64 {
        self.0.duration_ms()
    }

    /// Returns the exit code when one exists.
    pub const fn exit_code(self) -> Option<i32> {
        self.0.exit_code()
    }

    /// Returns the best-effort error detail when delivery failed.
    pub fn error(self) -> Option<&'a str> {
        self.0.error()
    }

    /// Returns the stable delivery status.
    pub const fn status(self) -> NotificationDeliveryStatus {
        self.0.status()
    }
}
