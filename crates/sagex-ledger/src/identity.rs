use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
use rand_core::OsRng;
use rustpq::ml_dsa::{mldsa65, sign};

/// Node identity: raw ML-DSA-65 keys, base64-encoded for TOML storage.
/// Offline-friendly: no KMS, keys live in the node config file.
#[derive(Clone)]
pub struct Identity {
    pub node_id: u64,
    pub secret: mldsa65::SecretKey,
    pub public: mldsa65::PublicKey,
    pub public_b64: String,
}

impl Identity {
    pub fn generate(node_id: u64) -> Self {
        let (pk, sk) = mldsa65::generate(&mut OsRng);
        let public_b64 = B64.encode(pk.as_bytes());
        Self {
            node_id,
            secret: sk,
            public: pk,
            public_b64,
        }
    }

    pub fn from_secret_b64(_node_id: u64, secret_b64: &str) -> anyhow::Result<Self> {
        let raw = B64.decode(secret_b64.trim())?;
        let secret =
            mldsa65::SecretKey::from_bytes(&raw).map_err(|_| anyhow::anyhow!("bad secret key"))?;
        // Derive public bytes from secret? rustpq doesn't expose that directly,
        // so require pubkey alongside: caller should use from_keypair_b64.
        // Fallback: generate check via sign/verify is not possible without pk,
        // so error out with guidance.
        let _ = &secret;
        anyhow::bail!("use from_keypair_b64 (need both secret_key and pubkey in config)")
    }

    pub fn from_keypair_b64(
        node_id: u64,
        secret_b64: &str,
        pub_b64: &str,
    ) -> anyhow::Result<Self> {
        let sraw = B64.decode(secret_b64.trim())?;
        let praw = B64.decode(pub_b64.trim())?;
        let secret =
            mldsa65::SecretKey::from_bytes(&sraw).map_err(|_| anyhow::anyhow!("bad secret key"))?;
        let public =
            mldsa65::PublicKey::from_bytes(&praw).map_err(|_| anyhow::anyhow!("bad public key"))?;
        Ok(Self {
            node_id,
            secret,
            public,
            public_b64: pub_b64.trim().to_string(),
        })
    }

    pub fn secret_b64(&self) -> String {
        B64.encode(self.secret.as_bytes())
    }

    pub fn sign(&self, msg: &[u8]) -> String {
        let sig = sign::mldsa65::sign(&self.secret, msg, b"sagex-ledger-pbft", &mut OsRng)
            .expect("ml-dsa sign");
        B64.encode(sig.as_bytes())
    }

    pub fn verify_with_pubkey(pub_b64: &str, msg: &[u8], sig_b64: &str) -> anyhow::Result<()> {
        let praw = B64.decode(pub_b64.trim())?;
        let sraw = B64.decode(sig_b64.trim())?;
        let pk = mldsa65::PublicKey::from_bytes(&praw)
            .map_err(|_| anyhow::anyhow!("bad peer pubkey"))?;
        let sig = mldsa65::Signature::from_bytes(&sraw)
            .map_err(|_| anyhow::anyhow!("bad signature bytes"))?;
        mldsa65::verify(&pk, msg, b"sagex-ledger-pbft", &sig)
            .map_err(|_| anyhow::anyhow!("node signature verification failed"))
    }

    /// Bytes bound by PBFT votes: view|seq|digest.
    pub fn vote_bytes(view: u64, seq: u64, digest: &str) -> Vec<u8> {
        format!("{view}:{seq}:{digest}").into_bytes()
    }

    /// Bytes bound by PrePrepare: full block hash.
    pub fn preprepare_bytes(view: u64, seq: u64, block_hash: &str) -> Vec<u8> {
        format!("pp:{view}:{seq}:{block_hash}").into_bytes()
    }
}
