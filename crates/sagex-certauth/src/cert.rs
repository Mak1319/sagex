//! X.509 v3 leaf issuance signed with the CA's ML-DSA-65 key.
//!
//! Design notes:
//! - The subject key is the client's ML-DSA-65 key (signature certificate).
//!   The client's ML-KEM768 key is cryptographically bound by the PoP message
//!   and stored in the Mongo enrollment record; it is deliberately NOT placed
//!   in a custom X.509 extension (no OID squatting — the certificate stays
//!   strictly RFC 5280-shaped and parseable by stock tooling).
//! - Stock `openssl`/browsers parse these certificates but cannot *validate*
//!   ML-DSA signatures; [`verify_pem`] (our own verifier) is the authority.

use der::{
    Encode, EncodePem, asn1::OctetString, pem::LineEnding,
};
use rand_core::{OsRng, RngCore};
use sha2::{Digest, Sha256};
use std::{fmt, str::FromStr, time::Duration};
use x509_cert::{
    Certificate,
    builder::{Builder, CertificateBuilder, profile::BuilderProfile},
    ext::{
        Extension,
        pkix::{AuthorityKeyIdentifier, BasicConstraints, KeyUsage, KeyUsages, SubjectKeyIdentifier},
    },
    name::Name,
    serial_number::SerialNumber,
    time::Validity,
};
use spki::SubjectPublicKeyInfoOwned as SubjectPublicKeyInfo;

use crate::{
    mldsa::{MlDsa65Signature, MlDsa65Signer, verify_raw},
};

#[derive(Debug)]
pub enum CertError {
    BadName(String),
    Builder(x509_cert::builder::Error),
    Der(der::Error),
    Spki(spki::Error),
    BadPem(String),
    BadSignature,
    WrongSubject { expected: String },
    NotCurrentlyValid,
}

impl fmt::Display for CertError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadName(e) => write!(f, "bad distinguished name: {e}"),
            Self::Builder(e) => write!(f, "certificate build error: {e:?}"),
            Self::Der(e) => write!(f, "DER error: {e}"),
            Self::Spki(e) => write!(f, "SPKI error: {e}"),
            Self::BadPem(e) => write!(f, "PEM error: {e}"),
            Self::BadSignature => write!(f, "certificate signature invalid"),
            Self::WrongSubject { expected } => {
                write!(f, "certificate subject mismatch (expected CN={expected})")
            }
            Self::NotCurrentlyValid => write!(f, "certificate outside validity window"),
        }
    }
}

impl std::error::Error for CertError {}

/// SHA-256 fingerprint helper (SKI/AKI values).
pub fn fingerprint(public_key: &[u8]) -> [u8; 32] {
    Sha256::digest(public_key).into()
}

/// Minimal private-CA leaf profile: fixed issuer (the CA), per-request
/// subject, standard extensions (SKI/AKI/basicConstraints/keyUsage).
struct SagexLeafProfile {
    subject: Name,
    issuer: Name,
    ski: Vec<u8>,
    aki: Vec<u8>,
}

impl BuilderProfile for SagexLeafProfile {
    fn get_issuer(&self, _subject: &Name) -> Name {
        self.issuer.clone()
    }
    fn get_subject(&self) -> Name {
        self.subject.clone()
    }
    fn build_extensions(
        &self,
        _spk: spki::SubjectPublicKeyInfoRef<'_>,
        _issuer_spk: spki::SubjectPublicKeyInfoRef<'_>,
        _tbs: &x509_cert::TbsCertificate,
    ) -> x509_cert::builder::Result<Vec<Extension>> {
        use x509_cert::ext::ToExtension;
        Ok(vec![
            SubjectKeyIdentifier(OctetString::new(self.ski.clone())?)
                .to_extension(&self.subject, &[])?,
            AuthorityKeyIdentifier {
                key_identifier: Some(OctetString::new(self.aki.clone())?),
                authority_cert_issuer: None,
                authority_cert_serial_number: None,
            }
            .to_extension(&self.subject, &[])?,
            BasicConstraints {
                ca: false,
                path_len_constraint: None,
            }
            .to_extension(&self.subject, &[])?,
            KeyUsage(KeyUsages::DigitalSignature.into())
                .to_extension(&self.subject, &[])?,
        ])
    }
}

fn subject_spki(client_dsa_public: &[u8]) -> Result<SubjectPublicKeyInfo, CertError> {
    use der::asn1::BitString;
    Ok(SubjectPublicKeyInfo {
        algorithm: MlDsa65Signer::algorithm_identifier(),
        subject_public_key: BitString::from_bytes(client_dsa_public).map_err(CertError::Der)?,
    })
}

/// Issued certificate: PEM text plus the random serial used as Mongo key.
pub struct Issued {
    pub serial: u64,
    pub pem: String,
}

/// Issue a leaf certificate for `identity`/`client_dsa_public`.
/// Returns the PEM and serial; the caller persists the enrollment record.
pub fn issue(
    ca_identity: &str,
    ca_dsa_public: &[u8],
    signer: &MlDsa65Signer,
    identity: &str,
    client_dsa_public: &[u8],
    validity_days: u64,
) -> Result<Issued, CertError> {
    let mut serial_bytes = [0u8; 8];
    OsRng.fill_bytes(&mut serial_bytes);
    let serial = u64::from_be_bytes(serial_bytes).max(1);

    let profile = SagexLeafProfile {
        subject: Name::from_str(&format!("CN={identity}"))
            .map_err(|e| CertError::BadName(format!("{e:?}")))?,
        issuer: Name::from_str(&format!("CN={ca_identity}"))
            .map_err(|e| CertError::BadName(format!("{e:?}")))?,
        ski: fingerprint(client_dsa_public).to_vec(),
        aki: fingerprint(ca_dsa_public).to_vec(),
    };
    let validity = Validity::from_now(Duration::from_secs(
        validity_days.saturating_mul(86_400).max(3_600),
    ))
    .map_err(CertError::Der)?;
    let builder = CertificateBuilder::new(
        profile,
        SerialNumber::from(serial),
        validity,
        subject_spki(client_dsa_public)?,
    )
    .map_err(CertError::Builder)?;
    let cert: Certificate = builder
        .build::<_, MlDsa65Signature>(signer)
        .map_err(CertError::Builder)?;
    let pem = cert
        .to_pem(LineEnding::LF)
        .map_err(|e| CertError::BadPem(format!("{e:?}")))?;
    Ok(Issued { serial, pem })
}

/// Verify a PEM certificate against the CA: structure, issuer identity,
/// expected subject, validity window, and the ML-DSA-65 TBS signature.
pub fn verify_pem(
    pem: &str,
    ca_identity: &str,
    ca_dsa_public: &[u8],
    expected_subject: &str,
) -> Result<u64, CertError> {
    use der::DecodePem;
    let cert = Certificate::from_pem(pem.as_bytes())        .map_err(|e| CertError::BadPem(format!("{e:?}")))?;
    let tbs = cert.tbs_certificate();

    let want_issuer = Name::from_str(&format!("CN={ca_identity}"))
        .map_err(|e| CertError::BadName(format!("{e:?}")))?;
    let want_subject = Name::from_str(&format!("CN={expected_subject}"))
        .map_err(|e| CertError::BadName(format!("{e:?}")))?;
    if tbs.issuer() != &want_issuer {
        return Err(CertError::WrongSubject {
            expected: expected_subject.to_string(),
        });
    }
    if tbs.subject() != &want_subject {
        return Err(CertError::WrongSubject {
            expected: expected_subject.to_string(),
        });
    }
    // Validity window: not_before <= now <= not_after.
    let now = crate::permit::now_secs();
    let nb = system_secs(&tbs.validity().not_before)?;
    let na = system_secs(&tbs.validity().not_after)?;
    if now < nb || now > na {
        return Err(CertError::NotCurrentlyValid);
    }
    // Signature: recompute over the TBS DER with the CA public key.
    if tbs.signature().oid != crate::mldsa::ML_DSA_65_OID {
        return Err(CertError::BadSignature);
    }
    let tbs_der = tbs.to_der().map_err(CertError::Der)?;
    let sig_bytes = cert
        .signature()
        .as_bytes()
        .ok_or(CertError::BadSignature)?;
    if !verify_raw(ca_dsa_public, &tbs_der, sig_bytes) {
        return Err(CertError::BadSignature);
    }
    let serial_bytes = tbs.serial_number().as_bytes();
    let mut padded = [0u8; 8];
    let start = 8usize.saturating_sub(serial_bytes.len());
    padded[start..].copy_from_slice(&serial_bytes[serial_bytes.len().saturating_sub(8)..]);
    Ok(u64::from_be_bytes(padded))
}

fn system_secs(t: &x509_cert::time::Time) -> Result<u64, CertError> {
    use std::time::UNIX_EPOCH;
    t.to_system_time()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .map_err(|_| CertError::NotCurrentlyValid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ca::CaMaterial;

    fn test_ca(identity: &str) -> (crate::ca::CaMaterial, MlDsa65Signer) {
        let dir = std::env::temp_dir().join(format!(
            "sagex-cert-test-{}-{}-{}",
            identity,
            std::process::id(),
            crate::permit::now_secs()
        ));
        let (ca, _) =
            CaMaterial::load_or_generate(&dir, b"test-password", identity).unwrap();
        let signer = ca.signer(b"test-password").unwrap();
        (ca, signer)
    }

    #[test]
    fn issue_and_verify_roundtrip() {
        let (ca, signer) = test_ca("roundtrip-ca");
        let (_enc, client_pk) =
            sagex_crypto::pqc::dsa::KeyGen::generate_from_password(b"client-pw").unwrap();
        let issued = issue(&ca.identity, &ca.dsa_public, &signer, "alice", &client_pk, 30).unwrap();
        assert!(issued.pem.contains("BEGIN CERTIFICATE"));
        let serial = verify_pem(&issued.pem, &ca.identity, &ca.dsa_public, "alice").unwrap();
        assert_eq!(serial, issued.serial);
    }

    #[test]
    fn verify_rejects_wrong_subject_and_foreign_ca() {
        let (ca, signer) = test_ca("reject-ca");
        let (_enc, client_pk) =
            sagex_crypto::pqc::dsa::KeyGen::generate_from_password(b"client-pw").unwrap();
        let issued = issue(&ca.identity, &ca.dsa_public, &signer, "alice", &client_pk, 30).unwrap();
        assert!(verify_pem(&issued.pem, &ca.identity, &ca.dsa_public, "bob").is_err());
        let (other, _) = test_ca("other-ca");
        assert!(verify_pem(&issued.pem, &other.identity, &other.dsa_public, "alice").is_err());
    }
}
