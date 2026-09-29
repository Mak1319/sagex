//! In-memory ring buffer of recent tracing events, served to auditors.
//!
//! Both ledger nodes and the register gateway install [`LogLayer`] next to
//! their `fmt` layer, so operational history is queryable at runtime:
//! ledger nodes answer the TCP `GetLogs` query from this buffer, and the
//! gateway merges its own buffer with the nodes' behind the restricted
//! `GET /logs` endpoint. Nothing is persisted — restarts clear history,
//! which is fine: the ledger itself (SQLite) remains the durable record.

use std::collections::VecDeque;
use std::fmt;
use std::sync::{Arc, Mutex, OnceLock};

use serde::{Deserialize, Serialize};

/// One captured tracing event.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LogEntry {
    /// RFC3339 UTC timestamp (millis).
    pub ts: String,
    /// `ERROR` | `WARN` | `INFO` | `DEBUG` | `TRACE`.
    pub level: String,
    /// Tracing target (module path).
    pub target: String,
    /// The event's message (truncated to [`MAX_MESSAGE_LEN`]).
    pub message: String,
}

pub const MAX_MESSAGE_LEN: usize = 2000;
const DEFAULT_CAPACITY: usize = 2000;

static GLOBAL: OnceLock<LogBuffer> = OnceLock::new();

/// Bounded, process-wide event buffer. Cheap to clone (shares storage).
#[derive(Debug, Clone)]
pub struct LogBuffer {
    inner: Arc<Mutex<VecDeque<LogEntry>>>,
    capacity: usize,
}

impl LogBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: Arc::new(Mutex::new(VecDeque::new())),
            capacity: capacity.max(16),
        }
    }

    /// Process-global buffer, created on first use (capacity 2000).
    /// Binaries that want a configured size call [`LogBuffer::init_global`]
    /// before serving traffic instead.
    pub fn global() -> &'static LogBuffer {
        GLOBAL.get_or_init(|| LogBuffer::new(DEFAULT_CAPACITY))
    }

    /// Set the global buffer's capacity. First call wins; returns the global.
    pub fn init_global(capacity: usize) -> &'static LogBuffer {
        GLOBAL.get_or_init(|| LogBuffer::new(capacity))
    }

    pub fn push(&self, entry: LogEntry) {
        let mut q = self.inner.lock().unwrap();
        q.push_back(entry);
        while q.len() > self.capacity {
            q.pop_front();
        }
    }

    pub fn push_event(&self, level: &str, target: &str, message: &str) {
        let mut msg = message.to_string();
        if msg.len() > MAX_MESSAGE_LEN {
            msg.truncate(MAX_MESSAGE_LEN);
        }
        self.push(LogEntry {
            ts: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            level: level.to_string(),
            target: target.to_string(),
            message: msg,
        });
    }

    /// Chronological tail of the buffer: at most `limit` entries whose
    /// severity is at least `level` (`None` or unknown level = everything).
    /// Server code clamps `limit`; values are additionally capped here.
    pub fn snapshot(&self, level: Option<&str>, limit: usize) -> Vec<LogEntry> {
        let min_rank = level.map(level_rank).unwrap_or(0);
        let limit = limit.clamp(1, 500);
        let q = self.inner.lock().unwrap();
        q.iter()
            .filter(|e| level_rank(&e.level) >= min_rank)
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .take(limit)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect()
    }

    pub fn len(&self) -> usize {
        self.inner.lock().unwrap().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[cfg(test)]
    fn clear(&self) {
        self.inner.lock().unwrap().clear();
    }
}

fn level_rank(level: &str) -> u8 {
    match level.to_ascii_uppercase().as_str() {
        "ERROR" => 5,
        "WARN" => 4,
        "INFO" => 3,
        "DEBUG" => 2,
        "TRACE" => 1,
        _ => 0,
    }
}

/// A `tracing` layer that mirrors every event into a [`LogBuffer`].
/// Mount next to the `fmt` layer so stdout logging is unchanged.
#[derive(Debug, Clone)]
pub struct LogLayer {
    buffer: LogBuffer,
}

impl LogLayer {
    pub fn new(buffer: LogBuffer) -> Self {
        Self { buffer }
    }

    pub fn global() -> Self {
        Self { buffer: LogBuffer::global().clone() }
    }
}

#[derive(Default)]
struct FieldVisitor {
    message: Option<String>,
    extra: Vec<String>,
}

impl tracing::field::Visit for FieldVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn fmt::Debug) {
        // The `message` field carries `format_args!`; its Debug rendering is
        // the rendered text itself (no extra quoting).
        if field.name() == "message" {
            self.message = Some(format!("{value:?}"));
        } else {
            self.extra.push(format!("{}={value:?}", field.name()));
        }
    }
}

impl<S> tracing_subscriber::Layer<S> for LogLayer
where
    S: tracing::Subscriber,
{
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let meta = event.metadata();
        let mut v = FieldVisitor::default();
        event.record(&mut v);
        let mut message = v.message.unwrap_or_default();
        if !v.extra.is_empty() {
            if !message.is_empty() {
                message.push_str(" | ");
            }
            message.push_str(&v.extra.join(" "));
        }
        self.buffer
            .push_event(meta.level().as_str(), meta.target(), &message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cap_and_chronological_tail() {
        let b = LogBuffer::new(16);
        for i in 0..20 {
            b.push_event("INFO", "t", &format!("m{i}"));
        }
        assert_eq!(b.len(), 16);
        let snap = b.snapshot(None, 100);
        assert_eq!(snap.len(), 16);
        assert_eq!(snap.first().unwrap().message, "m4");
        assert_eq!(snap.last().unwrap().message, "m19");
        let tail = b.snapshot(None, 5);
        assert_eq!(tail.len(), 5);
        assert_eq!(tail.first().unwrap().message, "m15");
    }

    #[test]
    fn level_filter_keeps_severity_floor() {
        let b = LogBuffer::new(32);
        b.push_event("INFO", "t", "info-1");
        b.push_event("WARN", "t", "warn-1");
        b.push_event("ERROR", "t", "err-1");
        b.push_event("DEBUG", "t", "dbg-1");
        let w = b.snapshot(Some("WARN"), 100);
        assert_eq!(w.len(), 2);
        assert!(w.iter().all(|e| e.level == "WARN" || e.level == "ERROR"));
        // Unknown level string = no filtering.
        assert_eq!(b.snapshot(Some("VERBOSE"), 100).len(), 4);
        // Case-insensitive.
        assert_eq!(b.snapshot(Some("warn"), 100).len(), 2);
    }

    #[test]
    fn long_messages_truncated() {
        let b = LogBuffer::new(16);
        b.push_event("INFO", "t", &"x".repeat(5000));
        let snap = b.snapshot(None, 10);
        assert!(snap[0].message.len() <= MAX_MESSAGE_LEN);
    }
}
