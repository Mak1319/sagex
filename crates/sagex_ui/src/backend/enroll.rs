//! Device enrollment orchestration: password -> keys -> CSR -> certificate.
//!
//! Contract for screens (including ones owned by other workstreams):
//! - After any successful sign-in, call [`EnrollState::on_signed_in`]; it
//!   only inspects the vault and sets [`EnrollStatus`] — it never blocks.
//! - Drive [`EnrollState::run_enrollment`] via `backend::request` to perform
//!   the async permit -> CSR -> store flow; render [`EnrollStatus`].
//! - Chat entry is NOT gated on enrollment (keys become mandatory only when
//!   E2EE messaging lands); status display is the integration surface.
//!
//! One active device identity at a time: enrolling as another user
//! overwrites the vault keys/cert (documented, v1 scope).

use zeroize::Zeroizing;

use super::{
    ca_client::{CaClient, CertIssued},
    identity,
    vault::{Vault, VaultError},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnrollStatus {
    /// Nothing known yet (fresh boot, no sign-in).
    Idle,
    /// Signed in, but no password is available to unlock/generate keys
    /// (e.g. OTP-only login). Chat works; enrollment waits for a
    /// password login.
    Locked,
    /// Vault has no keys for this user yet.
    NeedsKeys,
    /// Keys exist, still needs a pasted operator permit to submit the CSR.
    NeedsPermit,
    /// Enrollment running.
    Submitting,
    /// Device certificate stored.
    Done { identity: String, serial: u64 },
    /// Last run failed; message is display-safe.
    Failed { message: String },
}

pub struct EnrollState {
    status: EnrollStatus,
    vault: Vault,
    ca: CaClient,
    pending_password: Option<Zeroizing<Vec<u8>>>,
    pending_permit: Option<String>,
    active_user: Option<String>,
}

impl EnrollState {
    pub fn new(vault: Vault, ca: CaClient) -> Self {
        Self {
            status: EnrollStatus::Idle,
            vault,
            ca,
            pending_password: None,
            pending_permit: None,
            active_user: None,
        }
    }

    pub fn status(&self) -> EnrollStatus {
        self.status.clone()
    }

    pub fn vault_dir(&self) -> std::path::PathBuf {
        self.vault.dir().to_path_buf()
    }

    /// Stash the login/signup password for vault unlock + enrollment.
    /// Called from the submit handlers (the only place the password
    /// exists). Kept in memory only, zeroized on consume/replace/clear.
    pub fn set_password(&mut self, password: String) {
        self.pending_password = Some(Zeroizing::new(password.into_bytes()));
    }

    /// Store a pasted operator permit for one enrollment run.
    pub fn set_permit(&mut self, permit: String) {
        let trimmed = permit.trim().to_string();
        self.pending_permit = Some(trimmed);
        if self.status == EnrollStatus::NeedsPermit {
            self.status = EnrollStatus::NeedsKeys;
        }
    }

    pub fn clear_secrets(&mut self) {
        self.pending_password = None;
        self.pending_permit = None;
    }

    /// Post-sign-in reconciliation (sync, non-blocking): reflect what the
    /// vault already holds for `username`.
    pub fn on_signed_in(&mut self, username: &str) {
        self.active_user = Some(username.to_string());
        self.status = match self.vault.load_cert() {
            Ok(Some((meta, _))) if meta.identity == username => EnrollStatus::Done {
                identity: meta.identity,
                serial: meta.serial,
            },
            Ok(_) => {
                if self.vault.has_identity() {
                    // Keys exist but may belong to another user; the run
                    // regenerates when the stored identity mismatches.
                    EnrollStatus::NeedsPermit
                } else {
                    EnrollStatus::NeedsKeys
                }
            }
            Err(_) => EnrollStatus::Failed {
                message: "Vault unreadable.".into(),
            },
        };
    }

    /// Full run: keys -> CSR -> submit -> store. Consumes the stashed
    /// password and permit (single-use, zeroized/cleared after use).
    pub async fn run_enrollment(&mut self, username: &str) -> Result<CertIssued, String> {
        self.status = EnrollStatus::Submitting;
        let result = self.run_inner(username).await;
        self.clear_secrets();
        match &result {
            Ok(issued) => {
                self.status = EnrollStatus::Done {
                    identity: issued.identity.clone(),
                    serial: issued.serial,
                };
            }
            Err(message) => {
                self.status = EnrollStatus::Failed {
                    message: message.clone(),
                };
            }
        }
        result
    }

    async fn run_inner(&mut self, username: &str) -> Result<CertIssued, String> {
        if !sagex_auth::valid_username(username) {
            return Err("Invalid username for enrollment.".into());
        }
        let password: Vec<u8> = match self.pending_password.take() {
            Some(pw) => pw.to_vec(),
            None => {
                self.status = EnrollStatus::Locked;
                return Err("Vault locked: sign in with your password first.".into());
            }
        };
        // Keys: reuse only if the stored pair belongs to this user.
        let keys_current = self
            .vault
            .load_pub()
            .map(|publ| publ.internal.user_name == username)
            .unwrap_or(false);
        if !keys_current {
            let (prv, publ) =
                identity::generate_identity(&password, username).map_err(|e| e.to_string())?;
            self.vault
                .store_identity(&prv, &publ)
                .map_err(|e| e.to_string())?;
        }
        let permit = match self.pending_permit.take() {
            Some(p) if !p.is_empty() => p,
            _ => {
                self.status = EnrollStatus::NeedsPermit;
                return Err("A permit from your operator is required.".into());
            }
        };
        // Reload for an owned encapsulation (mirrors the server flow).
        let prv = self.vault.load_prv().map_err(|e| map_vault_err(e))?;
        let publ = self.vault.load_pub().map_err(|e| map_vault_err(e))?;
        if publ.internal.user_name != username {
            return Err("Stored keys belong to another user.".into());
        }
        let encap = prv.internal.key_encapsulation_dsa;
        let csr = identity::build_csr_body(
            &password,
            username,
            &publ.internal.key_kem,
            &publ.internal.key_dsa,
            encap,
        )
        .map_err(|e| e.to_string())?;
        let issued = self
            .ca
            .submit_csr(&permit, &csr)
            .await
            .map_err(|e| e.to_string())?;
        if issued.identity != username {
            return Err("CA returned a certificate for another identity.".into());
        }
        self.vault
            .store_cert(&issued.identity, issued.serial, &issued.certificate_pem)
            .map_err(|e| map_vault_err(e))?;
        Ok(issued)
    }

    /// One-way upgrade of a legacy plaintext `session.json`: if the legacy
    /// file exists and a password is stashed, seal the session into the
    /// vault and delete the plaintext. Does NOT consume the password (the
    /// enrollment run consumes it later). Returns true when migrated.
    pub fn migrate_legacy_session(
        &mut self,
        legacy_persisted: bool,
        drop_legacy: impl FnOnce(),
        sess: &super::session::StoredSession,
    ) -> bool {
        if !legacy_persisted {
            return false;
        }
        let Some(pw) = &self.pending_password else {
            return false;
        };
        if self.vault.store_session(pw, sess).is_err() {
            return false;
        }
        drop_legacy();
        true
    }

    /// Lock on sign-out: drop in-memory secrets but keep vault files, so
    /// the next password login unlocks without re-enrollment (no fresh
    /// permit burned).
    pub fn lock(&mut self) {
        self.clear_secrets();
        self.active_user = None;
        self.status = EnrollStatus::Idle;
    }

    /// Forget device state (explicit device removal), including vault files.
    pub fn reset(&mut self) {
        self.clear_secrets();
        self.vault.clear();
        self.active_user = None;
        self.status = EnrollStatus::Idle;
    }

    pub fn vault(&self) -> &Vault {
        &self.vault
    }

    pub fn ca(&self) -> &CaClient {
        &self.ca
    }
}

fn map_vault_err(e: VaultError) -> String {
    // Never leak key material or file contents into UI strings.
    match e {
        VaultError::Crypto(_) => "Wrong vault password or corrupted keys.".to_string(),
        VaultError::Io(_) => "Vault file missing or unreadable.".to_string(),
        other => format!("Vault error: {other}"),
    }
}
