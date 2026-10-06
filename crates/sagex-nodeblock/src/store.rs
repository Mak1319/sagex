use crate::model::{Block, CommitCertificate, CommittedRecord, LedgerRecord};
use anyhow::{Context, Result, bail};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

pub struct Store {
    db: Mutex<Connection>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LockedVote {
    pub height: u64,
    pub block_hash: String,
    pub signature_hex: String,
}

impl Store {
    pub fn open(path: &str) -> Result<Self> {
        if let Some(parent) = std::path::Path::new(path).parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create database directory {}", parent.display()))?;
        }
        let db = Connection::open(path).context("open SQLite ledger")?;
        db.pragma_update(None, "journal_mode", "WAL")?;
        db.pragma_update(None, "synchronous", "FULL")?;
        db.execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE IF NOT EXISTS blocks (
               height INTEGER PRIMARY KEY,
               block_hash TEXT NOT NULL UNIQUE,
               previous_hash TEXT NOT NULL,
               block_json TEXT NOT NULL,
               certificate_json TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS records (
               watermark TEXT PRIMARY KEY,
               block_height INTEGER NOT NULL UNIQUE REFERENCES blocks(height),
               record_json TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS vote_locks (
               height INTEGER PRIMARY KEY,
               block_hash TEXT NOT NULL,
               signature_hex TEXT NOT NULL
             );",
        )?;
        Ok(Self { db: Mutex::new(db) })
    }

    pub fn tip(&self) -> Result<(u64, String)> {
        let db = self.db.lock().expect("database mutex poisoned");
        let result: Option<(i64, String)> = db
            .query_row(
                "SELECT height, block_hash FROM blocks ORDER BY height DESC LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        Ok(result.map_or((0, String::from("genesis")), |(h, hash)| (h as u64, hash)))
    }

    pub fn existing_watermark(&self, watermark: &str) -> Result<Option<CommittedRecord>> {
        let db = self.db.lock().expect("database mutex poisoned");
        let row: Option<(String, String)> = db
            .query_row(
                "SELECT b.block_json, b.certificate_json FROM records r JOIN blocks b ON b.height=r.block_height WHERE r.watermark=?1",
                [watermark],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        row.map(|(block, cert)| {
            Ok(CommittedRecord {
                block: serde_json::from_str(&block)?,
                certificate: serde_json::from_str(&cert)?,
            })
        })
        .transpose()
    }

    pub fn record_vote(&self, height: u64, hash: &str, signature: &str) -> Result<()> {
        let db = self.db.lock().expect("database mutex poisoned");
        let prior: Option<String> = db
            .query_row(
                "SELECT block_hash FROM vote_locks WHERE height=?1",
                [height as i64],
                |row| row.get(0),
            )
            .optional()?;
        match prior {
            Some(existing) if existing == hash => Ok(()),
            Some(_) => bail!("validator already voted for a different block at this height"),
            None => {
                db.execute(
                    "INSERT INTO vote_locks(height, block_hash, signature_hex) VALUES(?1, ?2, ?3)",
                    params![height as i64, hash, signature],
                )?;
                Ok(())
            }
        }
    }

    pub fn vote_lock(&self, height: u64) -> Result<Option<LockedVote>> {
        let db = self.db.lock().expect("database mutex poisoned");
        db.query_row(
            "SELECT block_hash, signature_hex FROM vote_locks WHERE height=?1",
            [height as i64],
            |row| {
                Ok(LockedVote {
                    height,
                    block_hash: row.get(0)?,
                    signature_hex: row.get(1)?,
                })
            },
        )
        .optional()
        .context("read vote lock")
    }

    pub fn append(&self, block: &Block, cert: &CommitCertificate) -> Result<()> {
        let mut db = self.db.lock().expect("database mutex poisoned");
        let tx = db.transaction()?;
        if let Some(existing) = tx
            .query_row(
                "SELECT block_hash FROM blocks WHERE height=?1",
                [block.height as i64],
                |row| row.get::<_, String>(0),
            )
            .optional()?
        {
            if existing == block.block_hash {
                return Ok(());
            }
            bail!("conflicting block already committed at this height");
        }
        let (height, tip_hash): (i64, String) = tx
            .query_row(
                "SELECT height, block_hash FROM blocks ORDER BY height DESC LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?
            .unwrap_or((0, "genesis".to_owned()));
        if block.height != height as u64 + 1 || block.previous_hash != tip_hash {
            bail!("block does not extend the local ledger tip");
        }
        tx.execute(
            "INSERT INTO blocks(height, block_hash, previous_hash, block_json, certificate_json) VALUES(?1, ?2, ?3, ?4, ?5)",
            params![block.height as i64, block.block_hash, block.previous_hash, serde_json::to_string(block)?, serde_json::to_string(cert)?],
        )?;
        tx.execute(
            "INSERT INTO records(watermark, block_height, record_json) VALUES(?1, ?2, ?3)",
            params![block.record.watermark, block.height as i64, serde_json::to_string(&block.record)?],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn lookup(&self, watermark: &str) -> Result<Option<CommittedRecord>> {
        self.existing_watermark(watermark)
    }

    pub fn record_count(&self) -> Result<u64> {
        let db = self.db.lock().expect("database mutex poisoned");
        let count: i64 = db.query_row("SELECT COUNT(*) FROM records", [], |row| row.get(0))?;
        Ok(count as u64)
    }

    pub fn blocks_from(&self, first_height: u64) -> Result<Vec<CommittedRecord>> {
        let db = self.db.lock().expect("database mutex poisoned");
        let mut statement = db.prepare(
            "SELECT block_json, certificate_json FROM blocks WHERE height >= ?1 ORDER BY height",
        )?;
        let rows = statement.query_map([first_height as i64], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        rows.map(|row| {
            let (block, certificate) = row?;
            Ok(CommittedRecord {
                block: serde_json::from_str(&block)?,
                certificate: serde_json::from_str(&certificate)?,
            })
        })
        .collect()
    }

    pub fn ensure_unique_record(&self, record: &LedgerRecord) -> Result<Option<CommittedRecord>> {
        if let Some(existing) = self.existing_watermark(&record.watermark)? {
            if existing.block.record == *record {
                return Ok(Some(existing));
            }
            bail!("watermark is already committed with different record data");
        }
        Ok(None)
    }
}
