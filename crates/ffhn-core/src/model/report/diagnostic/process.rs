//! Complete process-delivery attempt facts and their total failure derivation.

use std::borrow::Cow;

use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Deserializer, Serialize};

use crate::CoreError;

use super::{ExactByteCount, IoErrorClass};

/// The maximum number of raw stderr bytes retained for one process attempt.
const STDERR_RETAINED_BYTES_LIMIT: usize = 2_048;

/// Classification of retained process stderr for text rendering.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StderrEncoding {
    /// Retained bytes were valid UTF-8.
    Utf8,
    /// Retained bytes have a valid UTF-8 prefix and an incomplete terminal UTF-8 sequence.
    ///
    /// This describes only the exact bounded byte artifact. It does not make a claim about the
    /// discarded source suffix.
    Utf8IncompleteAtRetentionBoundary,
    /// Retained bytes needed UTF-8 replacement rendering.
    Utf8Lossy,
}

/// Bounded text retained from one stderr capture.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct StderrCapture {
    #[serde(
        serialize_with = "serialize_retained_bytes",
        deserialize_with = "deserialize_retained_bytes"
    )]
    retained_bytes_base64: Vec<u8>,
    original_len_bytes: ExactByteCount,
    truncated: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StderrCaptureWire {
    #[serde(deserialize_with = "deserialize_retained_bytes")]
    retained_bytes_base64: Vec<u8>,
    original_len_bytes: ExactByteCount,
    truncated: bool,
}

impl<'de> Deserialize<'de> for StderrCapture {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = StderrCaptureWire::deserialize(deserializer)?;
        let capture = Self {
            retained_bytes_base64: wire.retained_bytes_base64,
            original_len_bytes: wire.original_len_bytes,
            truncated: wire.truncated,
        };
        capture.validate().map_err(serde::de::Error::custom)?;
        Ok(capture)
    }
}

impl StderrCapture {
    pub(crate) fn accumulator() -> StderrCaptureAccumulator {
        StderrCaptureAccumulator {
            retained_bytes_base64: Vec::new(),
            original_len_bytes: ExactByteCount::zero(),
        }
    }

    /// Returns retained stderr as human-readable text without presentation decoration.
    ///
    /// A truncated raw prefix that ends inside a UTF-8 code point renders only its complete UTF-8
    /// prefix. The exact raw bytes remain available through the serialized capture evidence.
    pub fn text(&self) -> Cow<'_, str> {
        self.complete_utf8_prefix_at_retention_boundary()
            .map(Cow::Borrowed)
            .unwrap_or_else(|| String::from_utf8_lossy(&self.retained_bytes_base64))
    }
    /// Returns the classification used to render the retained stderr bytes.
    pub fn encoding(&self) -> StderrEncoding {
        match std::str::from_utf8(&self.retained_bytes_base64) {
            Ok(_) => StderrEncoding::Utf8,
            Err(_) if self.complete_utf8_prefix_at_retention_boundary().is_some() => {
                StderrEncoding::Utf8IncompleteAtRetentionBoundary
            }
            Err(_) => StderrEncoding::Utf8Lossy,
        }
    }
    /// Returns every source byte drained before EOF or reader failure.
    pub const fn original_len_bytes(&self) -> &ExactByteCount {
        &self.original_len_bytes
    }
    /// Returns whether retained text omits any drained source bytes.
    pub const fn truncated(&self) -> bool {
        self.truncated
    }

    fn complete_utf8_prefix_at_retention_boundary(&self) -> Option<&str> {
        if !self.truncated {
            return None;
        }
        let error = std::str::from_utf8(&self.retained_bytes_base64).err()?;
        if error.error_len().is_some() {
            return None;
        }
        std::str::from_utf8(&self.retained_bytes_base64[..error.valid_up_to()]).ok()
    }

    pub(super) fn validate(&self) -> Result<(), CoreError> {
        if self.retained_bytes_base64.len() > STDERR_RETAINED_BYTES_LIMIT {
            return Err(CoreError::contract(
                "delivery retained stderr bytes exceed the retained-byte limit",
            ));
        }
        if self
            .original_len_bytes
            .compare_usize(self.retained_bytes_base64.len())
            .is_lt()
        {
            return Err(CoreError::contract(
                "delivery stderr original_len_bytes is smaller than retained-byte evidence",
            ));
        }
        if self.truncated
            != self
                .original_len_bytes
                .compare_usize(self.retained_bytes_base64.len())
                .is_gt()
        {
            return Err(CoreError::contract(
                "delivery stderr truncation must exactly describe retained-byte evidence",
            ));
        }
        Ok(())
    }

    fn shorten_one_byte(&mut self) -> bool {
        if self.retained_bytes_base64.pop().is_none() {
            return false;
        }
        self.truncated = true;
        true
    }
}

/// Builds bounded stderr evidence directly from the bytes a reader observed.
///
/// The accumulator has no inputs that can fabricate an incoherent capture: each recorded slice
/// advances the exact total and retains only the configured prefix. `finish` consequently cannot
/// fail or panic in a production reader.
pub(crate) struct StderrCaptureAccumulator {
    retained_bytes_base64: Vec<u8>,
    original_len_bytes: ExactByteCount,
}

impl StderrCaptureAccumulator {
    pub(crate) fn record(&mut self, bytes: &[u8]) {
        self.original_len_bytes.add_usize(bytes.len());
        let remaining =
            STDERR_RETAINED_BYTES_LIMIT.saturating_sub(self.retained_bytes_base64.len());
        self.retained_bytes_base64
            .extend_from_slice(&bytes[..remaining.min(bytes.len())]);
    }

    pub(crate) fn finish(self) -> StderrCapture {
        let truncated = self
            .original_len_bytes
            .compare_usize(self.retained_bytes_base64.len())
            .is_gt();
        StderrCapture {
            retained_bytes_base64: self.retained_bytes_base64,
            original_len_bytes: self.original_len_bytes,
            truncated,
        }
    }
}

fn serialize_retained_bytes<S>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(&STANDARD.encode(bytes))
}

fn deserialize_retained_bytes<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let encoded = String::deserialize(deserializer)?;
    STANDARD.decode(encoded).map_err(serde::de::Error::custom)
}

/// Independent result of the process stderr reader.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum StderrOutcome {
    /// The reader did not start because the process boundary was not reached.
    Absent,
    /// The reader drained stderr through EOF.
    Captured {
        /// Retained stderr evidence.
        #[serde(flatten)]
        capture: StderrCapture,
    },
    /// The reader failed after retaining every pre-failure byte it drained.
    ReadFailed {
        /// Closed class of the reader I/O failure.
        io: IoErrorClass,
        /// Retained pre-failure stderr evidence.
        partial: StderrCapture,
    },
    /// The process started but its configured stderr pipe could not be acquired.
    ReaderUnavailable,
    /// The stderr reader thread panicked.
    ReaderPanicked,
}

/// The only stderr anomalies that may accompany an otherwise delivered event.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum StderrCaptureProblem {
    /// The reader failed after retaining its pre-failure bytes.
    ReadFailed {
        /// Closed class of the reader I/O failure.
        io: IoErrorClass,
        /// Retained pre-failure stderr evidence.
        partial: StderrCapture,
    },
    /// The completed delivery had no acquirable stderr pipe.
    ReaderUnavailable,
    /// The stderr reader thread panicked.
    ReaderPanicked,
}

impl StderrOutcome {
    pub(crate) fn captured(capture: StderrCapture) -> Self {
        Self::Captured { capture }
    }
    pub(crate) fn read_failed(io: IoErrorClass, partial: StderrCapture) -> Self {
        Self::ReadFailed { io, partial }
    }
    pub(crate) fn capture_problem(&self) -> Option<StderrCaptureProblem> {
        match self {
            Self::ReadFailed { io, partial } => Some(StderrCaptureProblem::ReadFailed {
                io: *io,
                partial: partial.clone(),
            }),
            Self::ReaderUnavailable => Some(StderrCaptureProblem::ReaderUnavailable),
            Self::ReaderPanicked => Some(StderrCaptureProblem::ReaderPanicked),
            Self::Absent | Self::Captured { .. } => None,
        }
    }
    pub(super) fn validate(&self) -> Result<(), CoreError> {
        match self {
            Self::Captured { capture } => capture.validate(),
            Self::ReadFailed { partial, .. } => partial.validate(),
            Self::Absent | Self::ReaderUnavailable | Self::ReaderPanicked => Ok(()),
        }
    }
    pub(super) fn shorten_one_byte(&mut self) -> bool {
        match self {
            Self::Captured { capture } => capture.shorten_one_byte(),
            Self::ReadFailed { partial, .. } => partial.shorten_one_byte(),
            Self::Absent | Self::ReaderUnavailable | Self::ReaderPanicked => false,
        }
    }
}

/// Terminal process observation independent of writer and reader outcomes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum TerminalOutcome {
    /// Process spawning failed.
    NotStarted {
        /// Closed class of the spawn I/O failure.
        io: IoErrorClass,
    },
    /// Process stdin was unavailable immediately after spawning.
    StdinUnavailable,
    /// A terminal status was observed; absent exit code is a platform fact.
    Exited {
        /// Platform exit code, when supplied.
        exit_code: Option<i32>,
    },
    /// The configured timeout elapsed before a terminal verdict was observed.
    TimedOut {
        /// Configured timeout in milliseconds.
        timeout_ms: u64,
    },
    /// Waiting for terminal status failed.
    WaitFailed {
        /// Closed class of the wait I/O failure.
        io: IoErrorClass,
    },
}

/// Independent process-stdin writer result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum WriterOutcome {
    /// The complete payload was written.
    Completed,
    /// Writing failed with a closed I/O class.
    IoFailed {
        /// Closed class of the writer I/O failure.
        io: IoErrorClass,
    },
    /// The writer thread panicked.
    Panicked,
    /// Writer start was impossible because process stdin was unavailable.
    NotAttempted,
}

/// Complete independent process-delivery facts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct DeliveryProcessAttempt {
    terminal: TerminalOutcome,
    writer: WriterOutcome,
    stderr: StderrOutcome,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DeliveryProcessAttemptWire {
    terminal: TerminalOutcome,
    writer: WriterOutcome,
    stderr: StderrOutcome,
}

impl<'de> Deserialize<'de> for DeliveryProcessAttempt {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = DeliveryProcessAttemptWire::deserialize(deserializer)?;
        let attempt = Self {
            terminal: wire.terminal,
            writer: wire.writer,
            stderr: wire.stderr,
        };
        attempt.validate().map_err(serde::de::Error::custom)?;
        Ok(attempt)
    }
}

/// Deterministic primary category for one failing process-delivery attempt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryFailurePrimary {
    /// Process spawning failed.
    SpawnFailed,
    /// Process stdin was unavailable.
    StdinUnavailable,
    /// Timeout elapsed before a terminal verdict was observed.
    TimedOut,
    /// Waiting for a terminal verdict failed.
    WaitFailed,
    /// The FFHN writer thread panicked.
    WriterPanicked,
    /// The FFHN writer had an I/O failure.
    WriterIoFailed,
    /// The process exited unsuccessfully or without a platform exit code.
    UnsuccessfulExit,
}

impl DeliveryFailurePrimary {
    /// Returns the stable report-contract spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SpawnFailed => "spawn_failed",
            Self::StdinUnavailable => "stdin_unavailable",
            Self::TimedOut => "timed_out",
            Self::WaitFailed => "wait_failed",
            Self::WriterPanicked => "writer_panicked",
            Self::WriterIoFailed => "writer_io_failed",
            Self::UnsuccessfulExit => "unsuccessful_exit",
        }
    }
}

impl DeliveryProcessAttempt {
    pub(crate) fn new(
        terminal: TerminalOutcome,
        writer: WriterOutcome,
        stderr: StderrOutcome,
    ) -> Self {
        Self {
            terminal,
            writer,
            stderr,
        }
    }
    /// Returns the terminal observation.
    pub const fn terminal(&self) -> &TerminalOutcome {
        &self.terminal
    }
    /// Returns the writer observation.
    pub const fn writer(&self) -> &WriterOutcome {
        &self.writer
    }
    /// Returns the stderr-reader observation.
    pub const fn stderr(&self) -> &StderrOutcome {
        &self.stderr
    }
    /// Returns the total deterministic primary failure, or `None` for success.
    pub fn primary(&self) -> Option<DeliveryFailurePrimary> {
        match &self.terminal {
            TerminalOutcome::NotStarted { .. } => Some(DeliveryFailurePrimary::SpawnFailed),
            TerminalOutcome::StdinUnavailable => Some(DeliveryFailurePrimary::StdinUnavailable),
            TerminalOutcome::TimedOut { .. } => Some(DeliveryFailurePrimary::TimedOut),
            TerminalOutcome::WaitFailed { .. } => Some(DeliveryFailurePrimary::WaitFailed),
            TerminalOutcome::Exited { exit_code } => match self.writer {
                WriterOutcome::Panicked => Some(DeliveryFailurePrimary::WriterPanicked),
                WriterOutcome::IoFailed { .. } => Some(DeliveryFailurePrimary::WriterIoFailed),
                WriterOutcome::Completed if *exit_code == Some(0) => None,
                WriterOutcome::Completed | WriterOutcome::NotAttempted => {
                    Some(DeliveryFailurePrimary::UnsuccessfulExit)
                }
            },
        }
    }
    /// Returns whether the terminal verdict and writer result establish success.
    pub fn is_success(&self) -> bool {
        self.primary().is_none()
    }
    pub(super) fn shorten_stderr_one_byte(&mut self) -> bool {
        self.stderr.shorten_one_byte()
    }
    pub(crate) fn validate(&self) -> Result<(), CoreError> {
        if matches!(self.terminal, TerminalOutcome::TimedOut { timeout_ms: 0 }) {
            return Err(CoreError::contract(
                "delivery timeout evidence must be positive",
            ));
        }
        let never_started = matches!(
            self.terminal,
            TerminalOutcome::NotStarted { .. } | TerminalOutcome::StdinUnavailable
        );
        if never_started {
            if !matches!(self.writer, WriterOutcome::NotAttempted)
                || !matches!(self.stderr, StderrOutcome::Absent)
            {
                return Err(CoreError::contract(
                    "delivery process facts claim work after a process boundary that was never reached",
                ));
            }
        } else if matches!(self.writer, WriterOutcome::NotAttempted)
            || matches!(self.stderr, StderrOutcome::Absent)
        {
            return Err(CoreError::contract(
                "delivery process facts omit writer or stderr evidence after process start",
            ));
        }
        self.stderr.validate()
    }
}

#[cfg(test)]
#[path = "process/tests.rs"]
mod tests;
