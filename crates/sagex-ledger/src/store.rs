use std::path::Path;
use std::sync::{Arc, Mutex};

use rusqlite::{Connection, params};

use crate::model::{Block, BlockHeader, DecryptionRecord};

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("duplicate watermark: {0}")]
    Duplicate(String),
    #[error("chain broken at index {0}: {1}")]
    Broken(u64, String),
    #[error("{0}")]
    Other(String),
}

/// SQLite-backed tamper-evident store. One file per node.
/// Synchronous rusqlite behind a Mutex; async callers use spawn_blocking.
#[derive(Clone)]
pub struct Store {
    inner: Arc<Mutex<Connection>>,
    pub path: String,
}

impl Store {
    pub fn open(path: &str) -> Result<Self, StoreError> {
        if let Some(parent) = Path::new(path).parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| StoreError::Other(e.to_string()))?;
            }
        }
        let conn = Connection::open(path)?;
        let s = Self {
            inner: Arc::new(Mutex::new(conn)),
            path: path.to_string(),
        };
        s.migrate()?;
        Ok(s)
    }

    pub fn open_in_memory() -> Result<Self, StoreError> {
        let conn = Connection::open_in_memory()?;
        let s = Self {
            inner: Arc::new(Mutex::new(conn)),
            path: ":memory:".into(),
        };
        s.migrate()?;
        Ok(s)
    }

    fn migrate(&self) -> Result<(), StoreError> {
        let c = self.inner.lock().unwrap();
        c.execute_batch(
            "PRAGMA journal_mode=WAL;
             CREATE TABLE IF NOT EXISTS blocks(
               idx INTEGER PRIMARY KEY,
               prev_hash TEXT NOT NULL,
               hash TEXT NOT NULL UNIQUE,
               timestamp INTEGER NOT NULL,
               view INTEGER NOT NULL,
               seq INTEGER NOT NULL UNIQUE,
               proposer INTEGER NOT NULL,
               record_json TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS records(
               watermark TEXT PRIMARY KEY,
               session_id TEXT NOT NULL,
               user_id TEXT NOT NULL,
               file_hash TEXT NOT NULL,
               payload_hash TEXT NOT NULL,
               block_idx INTEGER NOT NULL REFERENCES blocks(idx)
             );
             CREATE INDEX IF NOT EXISTS idx_records_user ON records(user_id);
             CREATE INDEX IF NOT EXISTS idx_records_session ON records(session_id);",
        )?;
        Ok(())
    }

    pub fn tip(&self) -> Result<Option<BlockHeader>, StoreError> {
        let c = self.inner.lock().unwrap();
        let mut stmt = c.prepare(
            "SELECT idx,prev_hash,hash,timestamp,view,seq,proposer FROM blocks ORDER BY idx DESC LIMIT 1",
        )?;
        let mut rows = stmt.query([])?;
        if let Some(r) = rows.next()? {
            Ok(Some(BlockHeader {
                index: r.get(0)?,
                prev_hash: r.get(1)?,
                hash: r.get(2)?,
                timestamp: r.get(3)?,
                view: r.get::<_, i64>(4)? as u64,
                seq: r.get::<_, i64>(5)? as u64,
                proposer: r.get::<_, i64>(6)? as u64,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn height(&self) -> Result<u64, StoreError> {
        Ok(self.tip()?.map(|t| t.index + 1).unwrap_or(0))
    }

    pub fn has_watermark(&self, wm: &str) -> Result<bool, StoreError> {
        let c = self.inner.lock().unwrap();
        let n: i64 = c.query_row(
            "SELECT COUNT(*) FROM records WHERE watermark=?1",
            params![wm],
            |r| r.get(0),
        )?;
        Ok(n > 0)
    }

    /// Insert a committed block atomically. Verifies hash linkage.
    pub fn insert_block(&self, block: &Block) -> Result<(), StoreError> {
        block.record.validate().map_err(|e| StoreError::Other(e.to_string()))?;
        let c = self.inner.lock().unwrap();
        // chain check
        let expected_prev: String = if block.header.index == 0 {
            "GENESIS".into()
        } else {
            let prev: String = c
                .query_row(
                    "SELECT hash FROM blocks WHERE idx=?1",
                    params![block.header.index as i64 - 1],
                    |r| r.get(0),
                )
                .map_err(|_| {
                    StoreError::Broken(block.header.index, "missing predecessor".into())
                })?;
            prev
        };
        if block.header.prev_hash != expected_prev {
            return Err(StoreError::Broken(
                block.header.index,
                format!(
                    "prev mismatch: got {} want {}",
                    block.header.prev_hash, expected_prev
                ),
            ));
        }
        if !block.verify_link(&expected_prev) {
            return Err(StoreError::Broken(block.header.index, "hash mismatch".into()));
        }
        if c.query_row(
            "SELECT COUNT(*) FROM records WHERE watermark=?1",
            params![block.record.watermark],
            |r| r.get::<_, i64>(0),
        )? > 0
        {
            return Err(StoreError::Duplicate(block.record.watermark.clone()));
        }
        let record_json = serde_json::to_string(&block.record)
            .map_err(|e| StoreError::Other(e.to_string()))?;
        c.execute(
            "INSERT INTO blocks(idx,prev_hash,hash,timestamp,view,seq,proposer,record_json)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
            params![
                block.header.index as i64,
                block.header.prev_hash,
                block.header.hash,
                block.header.timestamp,
                block.header.view as i64,
                block.header.seq as i64,
                block.header.proposer as i64,
                record_json,
            ],
        )?;
        c.execute(
            "INSERT INTO records(watermark,session_id,user_id,file_hash,payload_hash,block_idx)
             VALUES(?1,?2,?3,?4,?5,?6)",
            params![
                block.record.watermark,
                block.record.session_id,
                block.record.user_id,
                block.record.file_hash,
                block.record.payload_hash,
                block.header.index as i64,
            ],
        )?;
        Ok(())
    }

    fn row_to_block(
        idx: i64,
        prev_hash: String,
        hash: String,
        timestamp: i64,
        view: i64,
        seq: i64,
        proposer: i64,
        record_json: String,
    ) -> Result<Block, StoreError> {
        let record: DecryptionRecord = serde_json::from_str(&record_json)
            .map_err(|e| StoreError::Other(e.to_string()))?;
        Ok(Block {
            header: BlockHeader {
                index: idx as u64,
                prev_hash,
                hash,
                timestamp,
                view: view as u64,
                seq: seq as u64,
                proposer: proposer as u64,
            },
            record,
        })
    }

    pub fn get_by_index(&self, index: u64) -> Result<Option<Block>, StoreError> {
        let c = self.inner.lock().unwrap();
        let mut stmt = c.prepare(
            "SELECT idx,prev_hash,hash,timestamp,view,seq,proposer,record_json FROM blocks WHERE idx=?1",
        )?;
        let mut rows = stmt.query(params![index as i64])?;
        if let Some(r) = rows.next()? {
            Ok(Some(Self::row_to_block(
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
                r.get(6)?,
                r.get(7)?,
            )?))
        } else {
            Ok(None)
        }
    }

    pub fn get_by_watermark(&self, wm: &str) -> Result<Option<Block>, StoreError> {
        let c = self.inner.lock().unwrap();
        let idx: Option<i64> = c
            .query_row(
                "SELECT block_idx FROM records WHERE watermark=?1",
                params![wm],
                |r| r.get(0),
            )
            .ok();
        let Some(idx) = idx else { return Ok(None) };
        drop(c);
        self.get_by_index(idx as u64)
    }

    pub fn get_by_user(&self, user_id: &str) -> Result<Vec<Block>, StoreError> {
        let idxs: Vec<i64> = {
            let c = self.inner.lock().unwrap();
            let mut stmt =
                c.prepare("SELECT block_idx FROM records WHERE user_id=?1 ORDER BY block_idx")?;
            let rows = stmt.query_map(params![user_id], |r| r.get(0))?;
            rows.collect::<Result<_, _>>()?
        };
        let mut out = Vec::new();
        for i in idxs {
            if let Some(b) = self.get_by_index(i as u64)? {
                out.push(b);
            }
        }
        Ok(out)
    }

    /// Full hash-chain verification (forensic / startup check).
    pub fn verify_chain(&self) -> Result<u64, StoreError> {
        let c = self.inner.lock().unwrap();
        let mut stmt = c.prepare(
            "SELECT idx,prev_hash,hash,timestamp,view,seq,proposer,record_json FROM blocks ORDER BY idx ASC",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, i64>(4)?,
                r.get::<_, i64>(5)?,
                r.get::<_, i64>(6)?,
                r.get::<_, String>(7)?,
            ))
        })?;
        let mut prev = "GENESIS".to_string();
        let mut count = 0u64;
        for row in rows {
            let (idx, prev_hash, hash, ts, view, seq, proposer, record_json) =
                row.map_err(StoreError::Sqlite)?;
            let record: DecryptionRecord = serde_json::from_str(&record_json)
                .map_err(|e| StoreError::Other(e.to_string()))?;
            let b = Block {
                header: BlockHeader {
                    index: idx as u64,
                    prev_hash: prev_hash.clone(),
                    hash: hash.clone(),
                    timestamp: ts,
                    view: view as u64,
                    seq: seq as u64,
                    proposer: proposer as u64,
                },
                record,
            };
            if !b.verify_link(&prev) {
                return Err(StoreError::Broken(idx as u64, "recomputed hash differs".into()));
            }
            prev = hash;
            count += 1;
        }
        Ok(count)
    }
}
