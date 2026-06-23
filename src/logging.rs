//! Structured logging utilities for observability hardening.
//!
//! Provides:
//! - [`ErrorDedupLayer`]: a `tracing-subscriber::Layer` that deduplicates
//!   identical ERROR-level log messages within a configurable time window.
//!   Instead of suppressing output, it records repeat counts and emits a
//!   summary line ("Error repeated N times") when a new unique error arrives.
//! - Structured field helpers for enriching tracing events with
//!   `request_id`, `error.kind`, `error.detail`, etc.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tracing::{Event, Level, Subscriber, field::Visit};
use tracing_subscriber::layer::{Context, Layer};
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

// ─── ErrorDedupLayer ─────────────────────────────────────────────────

/// A `tracing_subscriber::Layer` that deduplicates identical ERROR-level
/// log messages within a configurable time window.
///
/// When an ERROR event is received:
/// 1. Its message + target is extracted to form a dedup key.
/// 2. If the same key appears within `window` of the last identical event,
///    the repeat counter is incremented (no additional output).
/// 3. When a *different* ERROR event arrives (or the window expires), a
///    summary is emitted reporting how many times the previous error repeated.
///
/// The summary is emitted via `eprintln!` with a JSON structure to avoid
/// re-entrancy issues with the tracing event system. This ensures dedup
/// output is visible in stderr/logs without risk of infinite recursion.
pub struct ErrorDedupLayer {
    state: Mutex<DedupState>,
    window: Duration,
}

struct DedupState {
    entries: VecDeque<ErrorEntry>,
}

impl ErrorDedupLayer {
    /// Create a new dedup layer with the given deduplication window.
    ///
    /// `window` controls how long (since first occurrence) identical errors
    /// are considered duplicates. Typical value: 1 second.
    pub fn new(window: Duration) -> Self {
        Self {
            state: Mutex::new(DedupState {
                entries: VecDeque::with_capacity(64),
            }),
            window,
        }
    }
}

impl<S> Layer<S> for ErrorDedupLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        // Only process ERROR-level events
        if *event.metadata().level() != Level::ERROR {
            return;
        }

        // Extract the message from the event
        let mut visitor = MessageVisitor { message: None };
        event.record(&mut visitor);

        let msg = match visitor.message {
            Some(m) => m,
            None => return, // no message to dedup on
        };

        let target = event.metadata().target();
        let key = format!("{target}:{msg}");

        let now = Instant::now();
        let window = self.window;

        let mut state = match self.state.lock() {
            Ok(s) => s,
            Err(_) => return, // poisoned — skip
        };

        // Prune entries older than the window
        while state.entries.len() > 1 {
            if now.duration_since(state.entries[0].first_seen) > window {
                let expired = state.entries.pop_front().unwrap();
                if expired.count > 1 {
                    drop(state);
                    emit_summary(&expired.key, expired.count, window);
                    state = match self.state.lock() {
                        Ok(s) => s,
                        Err(_) => return,
                    };
                }
            } else {
                break;
            }
        }

        // Check if this event is a duplicate of the last entry
        let is_duplicate = state.entries.back().map_or(false, |last| {
            last.key == key && now.duration_since(last.first_seen) <= window
        });

        if is_duplicate {
            // Increment counter — same error within window
            if let Some(last) = state.entries.back_mut() {
                last.count += 1;
            }
            return;
        }

        // A new (or expired) error — flush the previous entry if it had repeats
        if let Some(prev) = state.entries.back() {
            if prev.count > 1 {
                let prev_key = prev.key.clone();
                let prev_count = prev.count;
                drop(state);
                emit_summary(&prev_key, prev_count, window);
                state = match self.state.lock() {
                    Ok(s) => s,
                    Err(_) => return,
                };
            }
        }

        // Add new entry
        state.entries.push_back(ErrorEntry {
            key,
            first_seen: now,
            count: 1,
        });

        // Cap the queue
        while state.entries.len() > 128 {
            state.entries.pop_front();
        }
    }
}

/// Emit a dedup summary to stderr in JSON format.
///
/// We write directly to stderr rather than calling `tracing::info!`
/// because re-entrant event dispatch from within a Layer's `on_event`
/// may not propagate to all registered layers, causing the summary
/// to be silently dropped.
fn emit_summary(key: &str, count: u64, window: Duration) {
    // Simple JSON line to stderr — the log collector can merge this with
    // stdout-based tracing output if desired.
    use std::io::Write;
    let _ = writeln!(
        std::io::stderr(),
        "{{\"level\":\"WARN\",\"target\":\"airouter_dedup\",\"error_message\":{},\"repeat_count\":{},\"window_ms\":{},\"message\":\"Aggregated error repeated {} times\"}}",
        serde_json::to_string(key).unwrap_or_else(|_| format!("\"{}\"", key)),
        count,
        window.as_millis(),
        count,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tracing_subscriber::prelude::*;

    /// A simple subscriber that collects events for testing.
    struct RecordingLayer {
        events: Arc<std::sync::Mutex<Vec<String>>>,
    }

    impl RecordingLayer {
        fn new(events: Arc<std::sync::Mutex<Vec<String>>>) -> Self {
            Self { events }
        }
    }

    impl<S: Subscriber + for<'a> LookupSpan<'a>> Layer<S> for RecordingLayer {
        fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
            let mut visitor = MessageVisitor { message: None };
            event.record(&mut visitor);
            let msg = visitor
                .message
                .unwrap_or_else(|| "<no message>".to_string());
            let level = event.metadata().level();
            self.events
                .lock()
                .unwrap()
                .push(format!("{level} {msg}"));
        }
    }

    fn setup_test_subscriber(
        dedup_window: Duration,
    ) -> (tracing::dispatcher::Dispatch, Arc<std::sync::Mutex<Vec<String>>>) {
        let events = Arc::new(std::sync::Mutex::new(Vec::new()));
        let events_clone = events.clone();

        let subscriber = tracing_subscriber::registry()
            .with(ErrorDedupLayer::new(dedup_window))
            .with(RecordingLayer::new(events_clone));

        let dispatch = tracing::dispatcher::Dispatch::new(subscriber);
        (dispatch, events)
    }

    #[test]
    fn dedup_single_error_passthrough() {
        let (dispatch, events) = setup_test_subscriber(Duration::from_secs(1));
        tracing::dispatcher::with_default(&dispatch, || {
            tracing::error!("test error");
        });

        let events = events.lock().unwrap();
        assert!(events.iter().any(|e| e.contains("ERROR") && e.contains("test error")));
    }

    #[test]
    fn dedup_duplicates_within_window() {
        let (dispatch, events) = setup_test_subscriber(Duration::from_secs(10));
        tracing::dispatcher::with_default(&dispatch, || {
            tracing::error!("dup error");
            tracing::error!("dup error");
            tracing::error!("dup error");
        });

        let events = events.lock().unwrap();
        // Should have at least the original ERROR events (3)
        // plus potentially the dedup summary
        let error_count = events.iter().filter(|e| e.starts_with("ERROR")).count();
        assert_eq!(error_count, 3, "all ERROR messages should pass through");
    }

    #[test]
    fn dedup_layer_info_events_not_affected() {
        let (dispatch, events) = setup_test_subscriber(Duration::from_secs(1));
        tracing::dispatcher::with_default(&dispatch, || {
            tracing::info!("info event");
            tracing::info!("info event");
        });

        let events = events.lock().unwrap();
        let info_count = events.iter().filter(|e| e.starts_with("INFO")).count();
        assert_eq!(info_count, 2, "INFO messages should not be affected");
    }

    #[test]
    fn dedup_no_summary_for_single_occurrence() {
        let (dispatch, events) = setup_test_subscriber(Duration::from_secs(1));
        tracing::dispatcher::with_default(&dispatch, || {
            tracing::error!("single error");
        });

        let events = events.lock().unwrap();
        let error_count = events.iter().filter(|e| e.starts_with("ERROR")).count();
        assert_eq!(error_count, 1);
    }
}
