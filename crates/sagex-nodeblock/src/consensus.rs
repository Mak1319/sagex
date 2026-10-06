use crate::{config::{Config, PeerConfig}, model::{Block, CommitCertificate, LedgerRecord, Vote}, store::Store};
use anyhow::{Context, Result, bail};
use ml_dsa::{EncodedVerifyingKey, Keypair, MlDsa65, Seed, Signature, SigningKey, VerifyingKey};
use ml_dsa::signature::{Signer, Verifier};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::sync::Arc;

#[derive(Clone, Serialize)]
struct HashBody<'a> {
    height: u64,
    previous_hash: &'a str,
    record: &'a LedgerRecord,
}

#[derive(Clone)]
pub struct Consensus {
    config: Arc<Config>,
    store: Arc<Store>,
    client: reqwest::Client,
    signing_key: Arc<SigningKey<MlDsa65>>,
}

impl Consensus {
    pub fn new(config: Arc<Config>, store: Arc<Store>, client: reqwest::Client) -> Result<Self> {
        let seed_bytes = hex::decode(&config.validator.signing_seed_hex)
            .context("decode validator ML-DSA seed")?;
        let seed = Seed::try_from(seed_bytes.as_slice())
            .map_err(|_| anyhow::anyhow!("validator ML-DSA seed must be 32 bytes"))?;
        for peer in &config.peers {
            let public_key = hex::decode(&peer.verifying_key_hex)
                .with_context(|| format!("decode validator {} ML-DSA key", peer.id))?;
            EncodedVerifyingKey::<MlDsa65>::try_from(public_key.as_slice())
                .map_err(|_| anyhow::anyhow!("validator {} has an invalid ML-DSA-65 public key", peer.id))?;
        }
        Ok(Self {
            config,
            store,
            client,
            signing_key: Arc::new(SigningKey::<MlDsa65>::from_seed(&seed)),
        })
    }

    pub fn local_verifying_key_hex(&self) -> String {
        hex::encode(self.signing_key.verifying_key().encode().as_slice())
    }

    pub fn public_key_for_seed(seed_hex: &str) -> Result<String> {
        let seed_bytes = hex::decode(seed_hex).context("decode validator ML-DSA seed")?;
        let seed = Seed::try_from(seed_bytes.as_slice())
            .map_err(|_| anyhow::anyhow!("validator ML-DSA seed must be 32 bytes"))?;
        let key = SigningKey::<MlDsa65>::from_seed(&seed);
        Ok(hex::encode(key.verifying_key().encode().as_slice()))
    }

    pub async fn propose(&self, record: LedgerRecord) -> Result<CommitCertificate> {
        self.sync_from_peers().await?;
        record.validate_for_storage().map_err(anyhow::Error::msg)?;
        if let Some(committed) = self.store.ensure_unique_record(&record)? {
            return Ok(committed.certificate);
        }

        let (tip_height, previous_hash) = self.store.tip()?;
        let block = make_block(tip_height + 1, previous_hash, record)?;
        let mut votes = vec![self.vote(&block)?];

        for peer in &self.config.peers {
            if let Ok(vote) = self.request_vote(peer, &block).await {
                if self.verify_vote(&vote, &block).is_ok()
                    && !votes.iter().any(|existing| existing.validator_id == vote.validator_id)
                {
                    votes.push(vote);
                }
            }
        }

        let quorum = self.validator_count() * 2 / 3 + 1;
        if votes.len() < quorum {
            bail!("commit quorum not reached: received {} of {quorum} votes", votes.len());
        }
        let certificate = CommitCertificate {
            height: block.height,
            block_hash: block.block_hash.clone(),
            votes,
        };
        self.verify_certificate(&certificate, &block)?;
        self.store.append(&block, &certificate)?;

        let commit = CommitRequest { block: block.clone(), certificate: certificate.clone() };
        for peer in &self.config.peers {
            let _ = self.client
                .post(format!("{}/internal/v1/commit", peer.address.trim_end_matches('/')))
                .json(&commit)
                .send()
                .await;
        }
        Ok(certificate)
    }

    pub async fn vote_async(&self, block: &Block) -> Result<Vote> {
        self.sync_from_peers().await?;
        self.vote(block)
    }

    pub async fn commit_async(&self, block: &Block, certificate: &CommitCertificate) -> Result<()> {
        self.sync_from_peers().await?;
        self.commit(block, certificate)
    }

    pub async fn sync_from_peers(&self) -> Result<()> {
        let (height, _) = self.store.tip()?;
        let first = height + 1;
        for peer in &self.config.peers {
            let response = self.client
                .get(format!("{}/internal/v1/blocks?from={first}", peer.address.trim_end_matches('/')))
                .send()
                .await;
            let Ok(response) = response else { continue };
            let Ok(response) = response.error_for_status() else { continue };
            let Ok(records) = response.json::<Vec<crate::model::CommittedRecord>>().await else { continue };
            let mut peer_chain_failed = false;
            for record in records {
                let (current_height, _) = self.store.tip()?;
                if record.block.height <= current_height { continue; }
                if self.validate_candidate(&record.block).is_err()
                    || self.verify_certificate(&record.certificate, &record.block).is_err()
                {
                    peer_chain_failed = true;
                    break;
                }
                if self.store.append(&record.block, &record.certificate).is_err() {
                    peer_chain_failed = true;
                    break;
                }
            }
            if peer_chain_failed { continue; }
        }
        Ok(())
    }

    pub fn vote(&self, block: &Block) -> Result<Vote> {
        self.validate_candidate(block)?;
        if let Some(locked) = self.store.vote_lock(block.height)? {
            if locked.block_hash != block.block_hash {
                bail!("validator is locked to a different block at this height");
            }
            return Ok(Vote {
                validator_id: self.config.node_id.clone(),
                block_hash: block.block_hash.clone(),
                signature_hex: locked.signature_hex,
            });
        }
        let signature = self.signing_key.sign(block.block_hash.as_bytes());
        let signature_hex = hex::encode(signature.encode().as_slice());
        self.store.record_vote(block.height, &block.block_hash, &signature_hex)?;
        Ok(Vote { validator_id: self.config.node_id.clone(), block_hash: block.block_hash.clone(), signature_hex })
    }

    pub fn commit(&self, block: &Block, certificate: &CommitCertificate) -> Result<()> {
        self.validate_candidate(block)?;
        self.verify_certificate(certificate, block)?;
        self.store.append(block, certificate)
    }

    fn validate_candidate(&self, block: &Block) -> Result<()> {
        block.record.validate_for_storage().map_err(anyhow::Error::msg)?;
        if compute_hash(block.height, &block.previous_hash, &block.record)? != block.block_hash {
            bail!("block hash does not match its contents");
        }
        let (tip_height, tip_hash) = self.store.tip()?;
        if block.height != tip_height + 1 || block.previous_hash != tip_hash {
            bail!("candidate does not extend this validator's current tip");
        }
        Ok(())
    }

    fn verify_vote(&self, vote: &Vote, block: &Block) -> Result<()> {
        if vote.block_hash != block.block_hash {
            bail!("vote is for another block");
        }
        let key = if vote.validator_id == self.config.node_id {
            self.signing_key.verifying_key().encode().as_slice().to_vec()
        } else {
            let peer = self.peer(&vote.validator_id)?;
            hex::decode(&peer.verifying_key_hex).context("decode peer ML-DSA key")?
        };
        verify_signature(&key, &vote.signature_hex, &block.block_hash)
    }

    fn verify_certificate(&self, certificate: &CommitCertificate, block: &Block) -> Result<()> {
        if certificate.height != block.height || certificate.block_hash != block.block_hash {
            bail!("commit certificate does not identify this block");
        }
        let mut seen = std::collections::HashSet::new();
        for vote in &certificate.votes {
            if !seen.insert(vote.validator_id.as_str()) {
                bail!("duplicate validator vote");
            }
            self.verify_vote(vote, block)?;
        }
        let quorum = self.validator_count() * 2 / 3 + 1;
        if certificate.votes.len() < quorum {
            bail!("commit certificate does not contain quorum votes");
        }
        Ok(())
    }

    async fn request_vote(&self, peer: &PeerConfig, block: &Block) -> Result<Vote> {
        let response = self.client
            .post(format!("{}/internal/v1/vote", peer.address.trim_end_matches('/')))
            .json(block)
            .send()
            .await
            .with_context(|| format!("request vote from validator {}", peer.id))?
            .error_for_status()?;
        response.json().await.context("decode validator vote")
    }

    fn peer(&self, id: &str) -> Result<&PeerConfig> {
        self.config.peers.iter().find(|peer| peer.id == id)
            .with_context(|| format!("unknown validator {id}"))
    }

    fn validator_count(&self) -> usize { self.config.peers.len() + 1 }
}

#[derive(Clone, serde::Deserialize, Serialize)]
pub struct CommitRequest {
    pub block: Block,
    pub certificate: CommitCertificate,
}

pub fn make_block(height: u64, previous_hash: String, record: LedgerRecord) -> Result<Block> {
    let block_hash = compute_hash(height, &previous_hash, &record)?;
    Ok(Block { height, previous_hash, block_hash, record })
}

fn compute_hash(height: u64, previous_hash: &str, record: &LedgerRecord) -> Result<String> {
    let body = serde_json::to_vec(&HashBody { height, previous_hash, record })?;
    Ok(hex::encode(Sha256::digest(body)))
}

fn verify_signature(public_key: &[u8], signature: &str, message: &str) -> Result<()> {
    let encoded_key = EncodedVerifyingKey::<MlDsa65>::try_from(public_key)
        .map_err(|_| anyhow::anyhow!("invalid validator ML-DSA public key"))?;
    let key = VerifyingKey::<MlDsa65>::decode(&encoded_key);
    let sig_bytes = hex::decode(signature).context("decode validator signature")?;
    let sig = Signature::<MlDsa65>::try_from(sig_bytes.as_slice())
        .map_err(|_| anyhow::anyhow!("invalid validator ML-DSA signature"))?;
    key.verify(message.as_bytes(), &sig).context("invalid validator signature")
}
