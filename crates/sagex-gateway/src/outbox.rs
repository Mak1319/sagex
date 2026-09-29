use std::path::Path;
use std::sync::{Arc, Mutex};

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use sagex_ledger::model::DecryptionRecord;

pub const STATUS_PENDING: &str = "PENDING";
pub const STATUS_INFLIGHT: &str = "INFLIGHT";
pub const STATUS_DONE: &str = "DONE";
pub const STATUS_FAILED: &str = "FAILED";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboxEntry {
    pub watermark: String,
    pub record: DecryptionRecord,
    pub status: String,
    pub attempts: i64,
    pub block_index: Option<u64>,
    pub block_hash: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum OutboxError {
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("{0}")]
    Other(String),
}

/// Durable submit queue. One SQLite file for the gateway.
/// Synchronous rusqlite behind a Mutex (same pattern as the ledger store).
#[derive(Clone)]
pub struct Outbox {
    inner: Arc<Mutex<Connection>>,
    pub path: String,
}

impl Outbox {
    pub fn open(path: &str) -> Result<Self, OutboxError> {
        if let Some(parent) = Path::new(path).parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(|e| OutboxError::Other(e.to_string()))?;
            }
        }
        let conn = Connection::open(path)?;
        let o = Self { inner: Arc::new(Mutex::new(conn)), path: path.into() };
        o.migrate()?;
        Ok(o)
    }

    pub fn open_in_memory() -> Result<Self, OutboxError> {
        let conn = Connection::open_in_memory()?;
        let o = Self { inner: Arc::new(Mutex::new(conn)), path: ":memory:".into() };
        o.migrate()?;
        Ok(o)
    }

    fn migrate(&self) -> Result<(), OutboxError> {
        let c = self.inner.lock().unwrap();
        c.execute_batch(
            "PRAGMA journal_mode=WAL;
             CREATE TABLE IF NOT EXISTS outbox(
               watermark TEXT PRIMARY KEY,
               record_json TEXT NOT NULL,
               status TEXT NOT NULL,
               attempts INTEGER NOT NULL DEFAULT 0,
               block_index INTEGER,
               block_hash TEXT,
               last_error TEXT,
               created_at INTEGER NOT NULL,
               updated_at INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_outbox_status ON outbox(status);
             -- Single-use permit JTIs (CA-auth strategy, same as sagex-certauth).
             -- A JTI is burned at intake, before the first submit attempt: a
             -- failed/rejected submit therefore consumes its permit and the
             -- operator issues a new one. Never retried, never deleted.
             CREATE TABLE IF NOT EXISTS used_permits(
               jti TEXT PRIMARY KEY,
               sub TEXT NOT NULL,
               used_at INTEGER NOT NULL
             );",
        )?;
        Ok(())
    }

    fn now_ms() -> i64 {
        chrono::Utc::now().timestamp_millis()
    }

    /// Insert a new record as PENDING, or return the existing entry
    /// (idempotent retry on watermark — matches ledger dedup).
    pub fn upsert_pending(&self, record: &DecryptionRecord) -> Result<(OutboxEntry, bool), OutboxError> {
        record.validate().map_err(|e| OutboxError::Other(e.to_string()))?;
        if let Some(e) = self.get(&record.watermark)? {
            return Ok((e, false));
        }
        let now = Self::now_ms();
        let json = serde_json::to_string(record).map_err(|e| OutboxError::Other(e.to_string()))?;
        let c = self.inner.lock().unwrap();
        c.execute(
            "INSERT OR IGNORE INTO outbox(watermark,record_json,status,attempts,created_at,updated_at)
             VALUES(?1,?2,?3,0,?4,?4)",
            params![record.watermark, json, STATUS_PENDING, now],
        )?;
        drop(c);
        Ok((self.get(&record.watermark)?.expect("just inserted"), true))
    }

    pub fn get(&self, watermark: &str) -> Result<Option<OutboxEntry>, OutboxError> {
        let c = self.inner.lock().unwrap();
        let mut stmt = c.prepare(
            "SELECT watermark,record_json,status,attempts,block_index,block_hash,last_error
             FROM outbox WHERE watermark=?1",
        )?;
        let mut rows = stmt.query(params![watermark])?;
        if let Some(r) = rows.next()? {
            Ok(Some(row_to_entry(r)?))
        } else {
            Ok(None)
        }
    }

    pub fn set_inflight(&self, watermark: &str) -> Result<(), OutboxError> {
        let c = self.inner.lock().unwrap();
        c.execute(
            "UPDATE outbox SET status=?1, updated_at=?2 WHERE watermark=?3",
            params![STATUS_INFLIGHT, Self::now_ms(), watermark],
        )?;
        Ok(())
    }

    pub fn mark_done(
        &self,
        watermark: &str,
        block_index: u64,
        block_hash: &str,
    ) -> Result<(), OutboxError> {
        let c = self.inner.lock().unwrap();
        c.execute(
            "UPDATE outbox SET status=?1, block_index=?2, block_hash=?3,
             attempts=attempts+1, last_error=NULL, updated_at=?4 WHERE watermark=?5",
            params![
                STATUS_DONE,
                block_index as i64,
                block_hash,
                Self::now_ms(),
                watermark
            ],
        )?;
        Ok(())
    }

    /// Transport failure / timeout: back to PENDING for the worker to retry.
    pub fn mark_retryable(&self, watermark: &str, err: &str) -> Result<(), OutboxError> {
        let c = self.inner.lock().unwrap();
        c.execute(
            "UPDATE outbox SET status=?1, attempts=attempts+1, last_error=?2, updated_at=?3
             WHERE watermark=?4",
            params![STATUS_PENDING, err, Self::now_ms(), watermark],
        )?;
        Ok(())
    }

    /// Ledger actively rejected the record (non-retryable without operator).
    pub fn mark_failed(&self, watermark: &str, err: &str) -> Result<(), OutboxError> {
        let c = self.inner.lock().unwrap();
        c.execute(
            "UPDATE outbox SET status=?1, attempts=attempts+1, last_error=?2, updated_at=?3
             WHERE watermark=?4",
            params![STATUS_FAILED, err, Self::now_ms(), watermark],
        )?;
        Ok(())
    }

    /// Atomically claim up to `limit` PENDING rows for the worker.
    pub fn claim_pending(&self, limit: usize) -> Result<Vec<OutboxEntry>, OutboxError> {
        let watermarks: Vec<String> = {
            let c = self.inner.lock().unwrap();
            let mut stmt = c.prepare(
                "SELECT watermark FROM outbox WHERE status=?1 ORDER BY created_at ASC LIMIT ?2",
            )?;
            let rows = stmt.query_map(params![STATUS_PENDING, limit as i64], |r| r.get(0))?;
            rows.collect::<Result<_, _>>()?
        };
        let now = Self::now_ms();
        {
            let c = self.inner.lock().unwrap();
            for wm in &watermarks {
                c.execute(
                    "UPDATE outbox SET status=?1, updated_at=?2
                     WHERE watermark=?3 AND status=?4",
                    params![STATUS_INFLIGHT, now, wm, STATUS_PENDING],
                )?;
            }
        }
        let mut out = Vec::new();
        for wm in watermarks {
            // Only keep rows this worker actually claimed.
            if let Some(e) = self.get(&wm)? {
                if e.status == STATUS_INFLIGHT {
                    out.push(e);
                }
            }
        }
        Ok(out)
    }

    pub fn pending_count(&self) -> Result<i64, OutboxError> {
        let c = self.inner.lock().unwrap();
        let n: i64 = c.query_row(
            "SELECT COUNT(*) FROM outbox WHERE status IN (?1,?2)",
            params![STATUS_PENDING, STATUS_INFLIGHT],
            |r| r.get(0),
        )?;
        Ok(n)
    }

    /// Has this permit JTI already been consumed?
    pub fn jti_spent(&self, jti: &str) -> Result<bool, OutboxError> {
        let c = self.inner.lock().unwrap();
        let n: i64 = c.query_row(
            "SELECT COUNT(*) FROM used_permits WHERE jti=?1",
            params![jti],
            |r| r.get(0),
        )?;
        Ok(n > 0)
    }

    /// Burn a permit JTI (single-use). Idempotent insert would hide replays,
    /// so callers must check [`Outbox::jti_spent`] first.
    pub fn burn_jti(&self, jti: &str, sub: &str) -> Result<(), OutboxError> {
        let c = self.inner.lock().unwrap();
        c.execute(
            "INSERT INTO used_permits(jti,sub,used_at) VALUES(?1,?2,?3)",
            params![jti, sub, Self::now_ms()],
        )?;
        Ok(())
    }

    pub fn list(&self, limit: usize) -> Result<Vec<OutboxEntry>, OutboxError> {
        let c = self.inner.lock().unwrap();
        let mut stmt = c.prepare(
            "SELECT watermark,record_json,status,attempts,block_index,block_hash,last_error
             FROM outbox ORDER BY created_at DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], row_to_entry)?;
        rows.collect::<Result<_, _>>().map_err(OutboxError::Sqlite)
    }
}

fn row_to_entry(r: &rusqlite::Row) -> Result<OutboxEntry, rusqlite::Error> {
    let json: String = r.get(1)?;
    let record: DecryptionRecord =
        serde_json::from_str(&json).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(
                1,
                rusqlite::types::Type::Text,
                Box::new(e),
            )
        })?;
    Ok(OutboxEntry {
        watermark: r.get(0)?,
        record,
        status: r.get(2)?,
        attempts: r.get(3)?,
        block_index: r.get::<_, Option<i64>>(4)?.map(|v| v as u64),
        block_hash: r.get(5)?,
        last_error: r.get(6)?,
    })
}
