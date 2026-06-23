//! Structured logging utilities for observability hardening.
//!
//! Provides:
//! - [`DedupState`]: shared, thread-safe state for deduplicating identical
//!   ERROR-level log messages within a configurable time window.
//! - [`DedupFormatEvent`]: a [`FormatEvent`] wrapper that suppresses
//!   duplicate ERROR events from reaching the JSON formatter, while
//!   counting them and emitting a summary line to stderr when a new
//!   unique error arrives or the window expires.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tracing::{Event, Level, Subscriber, field::Visit};
use tracing_subscriber::fmt::FormatEvent;
use tracing_subscriber::fmt::FormatFields;
use tracing_subscriber::fmt::FmtContext;
use tracing_subscriber::fmt::format::Writer;
use tracing_subscriber::registry::LookupSpan;

// ─── Message Extractor ───────────────────────────────────────────────

/// Visitor that extracts the `message` field from a tracing Event.
struct MessageVisitor {
    message: Option<String>,
}

impl Visit for MessageVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message = Some(format!("{value:?}"));
        }
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            self.message = Some(value.to_owned());
        }
    }
}

// ─── Error Entry ─────────────────────────────────────────────────────

#[derive(Clone)]
struct ErrorEntry {
    /// Dedup key: `target:message`
    key: String,
    first_seen: Instant,
    count: u64,
}

// ─── DedupState ──────────────────────────────────────────────────────

/// Shared, thread-safe state for deduplicating identical ERROR-level
/// log messages within a configurable time window.
///
/// Designed to be wrapped in `Arc<Mutex<...>>` and shared between
/// the [`DedupFormatEvent`] (which suppresses duplicate output) and
/// any other component that needs to observe duplicate activity.
pub struct DedupState {
    entries: VecDeque<ErrorEntry>,
    window: Duration,
}

impl DedupState {
    /// Create a new dedup state with the given deduplication window.
    ///
    /// `window` controls how long (since first occurrence) identical errors
    /// are considered duplicates. Typical value: 1 second.
    pub fn new(window: Duration) -> Self {
        Self {
            entries: VecDeque::with_capacity(64),
            window,
        }
    }

    /// Register a new error occurrence identified by `key`.
    ///
    /// Returns `true` if this event is a **duplicate** (should be
    /// suppressed from log output), or `false` if it is a first
    /// occurrence or the window has expired.
    ///
    /// Automatically:
    /// - Prunes expired entries from the front of the queue.
    /// - Emits summary lines (to stderr) for entries that accumulated
    ///   repeats, **including** removing the summarized entry from the
    ///   queue so it is never re-summarised on a future prune cycle.
    /// - Caps the queue at 128 entries.
    pub fn register(&mut self, key: &str, now: Instant) -> bool {
        self.prune(now);

        // Check if this event is a duplicate of the last entry
        let is_duplicate = self.entries.back().map_or(false, |last| {
            last.key == key && now.duration_since(last.first_seen) <= self.window
        });

        if is_duplicate {
            if let Some(last) = self.entries.back_mut() {
                last.count += 1;
            }
            return true;
        }

        // Flush the previous entry if it accumulated repeats.
        //
        // IMPORTANT: remove the entry from the VecDeque *before* or
        // *after* emitting the summary so that the next pruning cycle
        // does not re-summarise it (Critical Issue 2 fix).
        if let Some(prev) = self.entries.back() {
            if prev.count > 1 {
                let prev_key = prev.key.clone();
                let prev_count = prev.count;
                // Remove the summarized entry to prevent double-counting
                self.entries.pop_back();
                emit_summary(&prev_key, prev_count, self.window);
            }
        }

        // Push the new entry
        self.entries.push_back(ErrorEntry {
            key: key.to_string(),
            first_seen: now,
            count: 1,
        });

        // Cap the queue
        while self.entries.len() > 128 {
            self.entries.pop_front();
        }

        false
    }

    /// Remove expired entries from the front of the queue, emitting
    /// summaries for any that accumulated repeats.
    fn prune(&mut self, now: Instant) {
        while self.entries.len() > 1 {
            if now.duration_since(self.entries[0].first_seen) > self.window {
                let expired = self.entries.pop_front().unwrap();
                if expired.count > 1 {
                    emit_summary(&expired.key, expired.count, self.window);
                }
            } else {
                break;
            }
        }
    }
}

// ─── DedupFormatEvent ────────────────────────────────────────────────

/// A [`FormatEvent`] wrapper that suppresses duplicate ERROR-level log
/// messages within a configurable time window.
///
/// When an ERROR event is received:
/// 1. Its `message` + `target` is extracted to form a dedup key.
/// 2. If the key matches the last entry and is still within the window,
///    the event is **suppressed** (no output) and the repeat counter is
///    incremented.
/// 3. If the key is new or the window expired, the previous entry's
///    summary is emitted to stderr (if it had repeats) and the new event
///    passes through to the inner formatter.
///
/// # Usage
///
/// ```ignore
/// use std::sync::{Arc, Mutex};
/// use std::time::Duration;
/// use tracing_subscriber::fmt;
///
/// let state = Arc::new(Mutex::new(DedupState::new(Duration::from_secs(1))));
/// let json_fmt = fmt::format().json();
///
/// tracing_subscriber::registry()
///     .with(
///         fmt::layer()
///             .event_format(DedupFormatEvent::new(json_fmt, state))
///             .with_filter(tracing_subscriber::EnvFilter::from_default_env())
///     )
///     .init();
/// ```
pub struct DedupFormatEvent<E> {
    inner: E,
    state: Arc<Mutex<DedupState>>,
}

impl<E> DedupFormatEvent<E> {
    /// Create a new dedup wrapper around the inner [`FormatEvent`].
    pub fn new(inner: E, state: Arc<Mutex<DedupState>>) -> Self {
        Self { inner, state }
    }
}

impl<S, N, E> FormatEvent<S, N> for DedupFormatEvent<E>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
    E: FormatEvent<S, N>,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        writer: Writer<'_>,
        event: &Event<'_>,
    ) -> std::fmt::Result {
        if *event.metadata().level() == Level::ERROR {
            let mut visitor = MessageVisitor { message: None };
            event.record(&mut visitor);

            if let Some(msg) = &visitor.message {
                let target = event.metadata().target();
                let key = format!("{target}:{msg}");

                let mut state = match self.state.lock() {
                    Ok(s) => s,
                    Err(_) => return self.inner.format_event(ctx, writer, event),
                };

                if state.register(&key, Instant::now()) {
                    // Duplicate — suppress output, the DedupState has
                    // already incremented the counter internally.
                    return Ok(());
                }
                // Drop the lock before delegating (avoid holding it
                // across the downstream format call).
                drop(state);
            }
        }

        self.inner.format_event(ctx, writer, event)
    }
}

// ─── Summary Emission ────────────────────────────────────────────────

/// Emit a dedup summary to stderr in JSON format.
///
/// We write directly to stderr rather than calling `tracing::info!`
/// because re-entrant event dispatch from within a FormatEvent call
/// may not propagate correctly to all registered layers.
fn emit_summary(key: &str, count: u64, window: Duration) {
    use std::io::Write;
    let _ = writeln!(
        std::io::stderr(),
        "{{\"level\":\"WARN\",\"target\":\"airouter_dedup\",\"error_message\":{},\"repeat_count\":{},\"window_ms\":{},\"message\":\"Aggregated error repeated {} times\"}}",
        serde_json::to_string(key).unwrap_or_else(|_| format!("\"{key}\"")),
        count,
        window.as_millis(),
        count,
    );
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── DedupState unit tests ────────────────────────────────────────

    #[test]
    fn dedup_single_error_not_suppressed() {
        let mut state = DedupState::new(Duration::from_secs(10));
        let now = Instant::now();
        assert!(
            !state.register("test:single error", now),
            "first occurrence must NOT be suppressed"
        );
    }

    #[test]
    fn dedup_duplicates_are_suppressed() {
        let mut state = DedupState::new(Duration::from_secs(10));
        let now = Instant::now();

        assert!(
            !state.register("test:dup", now),
            "first must NOT be suppressed"
        );
        assert!(
            state.register("test:dup", now),
            "second must BE suppressed"
        );
        assert!(
            state.register("test:dup", now),
            "third must BE suppressed"
        );
    }

    #[test]
    fn dedup_after_window_expiry_renews() {
        let mut state = DedupState::new(Duration::from_millis(50));
        let t0 = Instant::now();

        assert!(!state.register("test:renew", t0), "first ok");
        assert!(state.register("test:renew", t0), "second suppressed");

        // After the window expires the same key is NOT a duplicate
        let t1 = t0 + Duration::from_millis(100);
        assert!(
            !state.register("test:renew", t1),
            "after expiry the same key is no longer a duplicate"
        );
    }

    #[test]
    fn dedup_different_errors_not_deduped() {
        let mut state = DedupState::new(Duration::from_secs(10));
        let now = Instant::now();

        assert!(!state.register("test:a", now), "error a — first ok");
        assert!(
            !state.register("test:b", now),
            "error b — different message, must NOT be suppressed"
        );
    }

    #[test]
    fn dedup_no_suppression_for_single_occurrence() {
        let mut state = DedupState::new(Duration::from_secs(1));
        assert!(!state.register("test:lonely", Instant::now()));
    }

    #[test]
    fn dedup_queue_trim() {
        // Push many different keys past the 128-entry cap.
        let mut state = DedupState::new(Duration::from_secs(60));
        let now = Instant::now();

        for i in 0..200 {
            let key = format!("test:error_{i}");
            assert!(!state.register(&key, now), "first of {i} must not be dup");
        }

        assert!(
            state.entries.len() <= 128,
            "queue should be capped at 128 (len = {})",
            state.entries.len()
        );
    }

    /// Verify that a flushed summary entry is NOT re-summarised on the
    /// next prune cycle (Critical Issue 2 regression guard).
    #[test]
    fn dedup_no_duplicate_summary_on_flush() {
        let mut state = DedupState::new(Duration::from_secs(10));
        let t0 = Instant::now();

        // Register and duplicate error A three times
        assert!(!state.register("test:err_a", t0), "err_a first");
        assert!(state.register("test:err_a", t0), "err_a dup");
        assert!(state.register("test:err_a", t0), "err_a dup");

        // Flushing A's summary: err_a had count=3, so emit_summary is
        // called inside register(). After flush, entry for err_a is
        // REMOVED from the VecDeque and err_b takes its place.
        assert!(!state.register("test:err_b", t0), "err_b first — triggers flush of err_a");

        // Manually prune: err_b is the only entry and it has count=1,
        // so no summary is emitted.  If err_a had been left in the deque
        // by accident it would have been summarised again here.
        state.prune(Instant::now() + Duration::from_secs(20));

        // err_b is still not expired in the sense of register() because
        // prune() only acts when len > 1. So the important part is:
        // err_b is the sole entry left and it has count=1.
        assert_eq!(state.entries.len(), 1, "only err_b should remain");
        assert_eq!(state.entries.back().unwrap().key, "test:err_b");
        assert_eq!(state.entries.back().unwrap().count, 1);
    }
}
