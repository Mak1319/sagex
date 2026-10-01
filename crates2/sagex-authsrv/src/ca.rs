use der::{Decode, Encode, asn1::BitString};
use ml_dsa::{KeyInit, MlDsa65, SigningKey};
use spki::ObjectIdentifier;
use std::str::FromStr;
use x509_cert::{
    AlgorithmIdentifier, SubjectPublicKeyInfo,
    builder::{Builder, CertificateBuilder, profile::BuilderProfile},
    ext::Extension,
    name::Name,
    serial_number::SerialNumber,
    spki::SubjectPublicKeyInfoRef,
    time::Validity,
};

/// NIST PQC OID id-ml-dsa-65.
pub fn mldsa65_oid() -> ObjectIdentifier {
    ObjectIdentifier::new_unwrap("2.16.840.1.101.3.4.3.18")
}

/// Private extension OIDs carrying the user's raw PQC keys.
pub fn kem_key_oid() -> ObjectIdentifier {
    ObjectIdentifier::new_unwrap("1.3.6.1.4.1.61095.1.1")
}
pub fn dsa_key_oid() -> ObjectIdentifier {
    ObjectIdentifier::new_unwrap("1.3.6.1.4.1.61095.1.2")
}

fn mldsa_alg() -> AlgorithmIdentifier {
    AlgorithmIdentifier {
        oid: mldsa65_oid(),
        parameters: None,
    }
}

/// CA signer: holds the ML-DSA-65 signing key (rebuilt from seed on load).
pub struct CaSigner {
    sk: SigningKey<MlDsa65>,
    pub vk_bytes: Vec<u8>,
}

impl CaSigner {
    pub fn from_seed(seed: &[u8]) -> anyhow::Result<Self> {
        if seed.len() != 32 {
            anyhow::bail!("CA DSA seed must be 32 bytes");
        }
        let seed_arr =
            ml_dsa::Seed::try_from(seed).map_err(|_| anyhow::anyhow!("bad CA seed"))?;
        let sk = SigningKey::<MlDsa65>::new(&seed_arr);
        use ml_dsa::KeyExport;
        use signature::Keypair as _;
        let vk_bytes = sk.verifying_key().to_bytes().to_vec();
        Ok(Self { sk, vk_bytes })
    }

    /// Load the CA seed from a keygen `.prv` file (postcard PrivateExternal)
    /// + password: uses `key_encapsulation_dsa` (same pattern as chatsrv).
    pub fn load(path: &str, password: &str) -> anyhow::Result<Self> {
        let bytes = std::fs::read(path)?;
        let prv = sagex_format::format::PrivateExternal::from_bytes(&bytes)
            .map_err(|e| anyhow::anyhow!("parse CA .prv: {e}"))?;
        let seed = sagex_crypto::aes::AESHandler::decrypt_private_key(
            password.as_bytes(),
            &prv.internal.key_encapsulation_dsa,
        )?;
        Self::from_seed(&seed)
    }
}

struct LeafProfile {
    subject: Name,
    issuer: Name,
}

impl BuilderProfile for LeafProfile {
    fn get_issuer(&self, _subject: &Name) -> Name {
        self.issuer.clone()
    }
    fn get_subject(&self) -> Name {
        self.subject.clone()
    }
    fn build_extensions(
        &self,
        _spk: SubjectPublicKeyInfoRef<'_>,
        _issuer_spk: SubjectPublicKeyInfoRef<'_>,
        _tbs: &x509_cert::certificate::TbsCertificate,
    ) -> x509_cert::builder::Result<Vec<Extension>> {
        Ok(vec![])
    }
}

/// Issue a leaf certificate binding `user_name` + PQC keys, signed by the CA.
/// Returns DER bytes.
pub fn issue_cert_der(
    ca: &CaSigner,
    issuer_cn: &str,
    user_name: &str,
    key_kem: &[u8],
    key_dsa: &[u8],
    ttl_days: u64,
) -> anyhow::Result<Vec<u8>> {
    use rand::TryRng;
    let mut serial_bytes = [0u8; 20];
    rand::rngs::SysRng
        .try_fill_bytes(&mut serial_bytes)
        .map_err(|e| anyhow::anyhow!("rng: {e}"))?;
    serial_bytes[0] &= 0x7f;
    if serial_bytes.iter().all(|b| *b == 0) {
        serial_bytes[19] = 1;
    }
    let serial = SerialNumber::new(&serial_bytes)?;
    let validity = Validity::from_now(std::time::Duration::from_secs(ttl_days * 86400))?;

    let subject = Name::from_str(&format!("CN={}", escape_cn(user_name)))?;
    let issuer = Name::from_str(&format!("CN={}", escape_cn(issuer_cn)))?;

    let user_spki = SubjectPublicKeyInfo {
        algorithm: mldsa_alg(),
        subject_public_key: BitString::new(0, key_dsa.to_vec())?,
    };

    let mut builder = CertificateBuilder::new(
        LeafProfile { subject, issuer },
        serial,
        validity,
        user_spki,
    )
    .map_err(|e| anyhow::anyhow!("cert builder: {e}"))?;
    for (oid, bytes) in [(kem_key_oid(), key_kem), (dsa_key_oid(), key_dsa)] {
        builder
            .add_extension(Extension {
                extn_id: oid,
                critical: false,
                extn_value: der::asn1::OctetString::new(bytes.to_vec())?,
            })
            .map_err(|e| anyhow::anyhow!("ext: {e}"))?;
    }

    let cert = builder
        .build::<_, ml_dsa::Signature<MlDsa65>>(&ca.sk)
        .map_err(|e| anyhow::anyhow!("cert build: {e}"))?;
    Ok(cert.to_der()?)
}

fn escape_cn(s: &str) -> String {
    s.replace(',', "\\,").replace('+', "\\+")
}

/// Verify a presenter leaf certificate against the CA verifying key.
/// Checks ML-DSA signature over TBS + validity window. Returns subject CN.
pub fn verify_presenter_cert(der_bytes: &[u8], ca_vk: &[u8]) -> anyhow::Result<String> {
    use x509_cert::certificate::Certificate;
    let cert = Certificate::from_der(der_bytes)?;
    if cert.signature_algorithm().oid != mldsa65_oid() {
        anyhow::bail!("unexpected signature algorithm");
    }
    let tbs_der = cert.tbs_certificate().to_der()?;
    let sig_bytes = cert
        .signature()
        .as_bytes()
        .unwrap_or(&[])
        .to_vec();
    sagex_crypto::pqc::dsa::verify_signature_bytes(ca_vk, &tbs_der, &sig_bytes)
        .map_err(|e| anyhow::anyhow!("cert signature invalid: {e}"))?;

    let now = std::time::SystemTime::now();
    let nb: std::time::SystemTime = cert.tbs_certificate().validity().not_before.to_system_time();
    let na: std::time::SystemTime = cert.tbs_certificate().validity().not_after.to_system_time();
    if now < nb || now > na {
        anyhow::bail!("certificate not currently valid");
    }
    use x509_cert::ext::pkix::name::DirectoryString;
    let cn = cert
        .tbs_certificate()
        .subject()
        .common_name()
        .map_err(|e| anyhow::anyhow!("cn parse: {e}"))?
        .ok_or_else(|| anyhow::anyhow!("certificate has no CN"))?;
    Ok(match cn {
        DirectoryString::PrintableString(s) => s.to_string(),
        DirectoryString::TeletexString(s) => s.to_string(),
        DirectoryString::Utf8String(s) => s,
        DirectoryString::BmpString(s) => s.to_string(),
    })
}

pub fn der_to_pem(der_bytes: &[u8]) -> String {
    pem_rfc7468::encode_string("CERTIFICATE", der::pem::LineEnding::LF, der_bytes)
        .unwrap_or_default()
}

pub fn pem_to_der(pem_text: &str) -> anyhow::Result<Vec<u8>> {
    let (label, der) = pem_rfc7468::decode_vec(pem_text.as_bytes())?;
    if label != "CERTIFICATE" {
        anyhow::bail!("not a CERTIFICATE pem");
    }
    Ok(der)
}
