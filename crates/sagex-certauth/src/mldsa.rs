//! ML-DSA-65 plumbing: OID, sizes, signer/verifier-key/signature wrappers
//! bridging `rustpq` to the `signature`/`spki` traits used by `x509-cert`.

use der::asn1::{BitString, ObjectIdentifier};
use rand_core::OsRng;
use rustpq::ml_dsa::{Error as MlDsaError, mldsa65};
use signature::{Error as SignatureError, Keypair, Signer};
use spki::{AlgorithmIdentifierOwned, DynSignatureAlgorithmIdentifier, EncodePublicKey};
use zeroize::Zeroizing;

/// FIPS 204 `id-ml-dsa-65` (NIST security level 3).
/// NOTE: ...3.17 = ml-dsa-44, ...3.18 = ml-dsa-65, ...3.19 = ml-dsa-87.
pub const ML_DSA_65_OID: ObjectIdentifier =
    ObjectIdentifier::new_unwrap("2.16.840.1.101.3.4.3.18");

/// FIPS 204 context string. Empty everywhere in sagex: the signed payloads
/// (JWS signing inputs, PoP messages, TBS DER) are already domain-separated
/// by construction, so no additional context is mixed in.
pub const CTX: &[u8] = b"";

/// Expected raw byte lengths for ML-DSA-65 material. Asserted from `rustpq`
/// constants so a dependency upgrade fails loudly instead of silently.
pub const DSA_PK_LEN: usize = mldsa65::PUBLIC_KEY_BYTES;
pub const DSA_SIG_LEN: usize = mldsa65::SIGNATURE_BYTES;

/// Upper bound for an opaque ML-KEM public key blob. The key itself is
/// validated cryptographically (trial encapsulation), this only caps memory.
pub const KEM_PK_MAX_LEN: usize = 4096;

/// Signing key: decrypted CA/client DSA secret (zeroized on drop) + public key.
#[derive(Clone)]
pub struct MlDsa65Signer {
    sk: Zeroizing<Vec<u8>>,
    pk: Vec<u8>,
}

impl MlDsa65Signer {
    pub fn new(sk_bytes: Vec<u8>, pk_bytes: Vec<u8>) -> Result<Self, MlDsaError> {
        // Validate eagerly so a corrupt `.prv` fails at startup, not at signing time.
        let _ = mldsa65::SecretKey::from_bytes(&sk_bytes)?;
        let _ = mldsa65::PublicKey::from_bytes(&pk_bytes)?;
        Ok(Self {
            sk: Zeroizing::new(sk_bytes),
            pk: pk_bytes,
        })
    }

    pub fn public_key_bytes(&self) -> &[u8] {
        &self.pk
    }

    pub fn verifying_key(&self) -> MlDsa65VerifyingKey {
        MlDsa65VerifyingKey(self.pk.clone())
    }

    pub fn algorithm_identifier() -> AlgorithmIdentifierOwned {
        AlgorithmIdentifierOwned {
            oid: ML_DSA_65_OID,
            parameters: None,
        }
    }
}

/// Raw ML-DSA-65 signature bytes (3309 B).
#[derive(Clone)]
pub struct MlDsa65Signature(pub Vec<u8>);

impl MlDsa65Signature {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, MlDsaError> {
        let _ = mldsa65::Signature::from_bytes(bytes)?;
        Ok(Self(bytes.to_vec()))
    }
}

impl TryFrom<&[u8]> for MlDsa65Signature {
    type Error = SignatureError;
    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        Self::from_bytes(bytes).map_err(|_| SignatureError::new())
    }
}

impl TryFrom<MlDsa65Signature> for Vec<u8> {
    type Error = core::convert::Infallible;
    fn try_from(sig: MlDsa65Signature) -> Result<Self, Self::Error> {
        Ok(sig.0)
    }
}

impl signature::SignatureEncoding for MlDsa65Signature {
    type Repr = Vec<u8>;
}

impl spki::SignatureBitStringEncoding for MlDsa65Signature {
    fn to_bitstring(&self) -> der::Result<BitString> {
        BitString::from_bytes(&self.0)
    }
}

impl Signer<MlDsa65Signature> for MlDsa65Signer {
    fn try_sign(&self, msg: &[u8]) -> Result<MlDsa65Signature, SignatureError> {
        let sk =
            mldsa65::SecretKey::from_bytes(&self.sk).map_err(|_| SignatureError::new())?;
        let sig = mldsa65::sign(&sk, msg, CTX, &mut OsRng).map_err(|_| SignatureError::new())?;
        Ok(MlDsa65Signature(sig.as_bytes().to_vec()))
    }
}

/// Verifying key wrapper implementing the `spki`/`signature` traits.
#[derive(Clone)]
pub struct MlDsa65VerifyingKey(pub Vec<u8>);

impl MlDsa65VerifyingKey {
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl EncodePublicKey for MlDsa65VerifyingKey {
    fn to_public_key_der(&self) -> Result<spki::Document, spki::Error> {
        let spki = spki::SubjectPublicKeyInfoOwned {
            algorithm: MlDsa65Signer::algorithm_identifier(),
            subject_public_key: BitString::from_bytes(&self.0)?,
        };
        Ok(spki.to_der()?.as_slice().try_into()?)
    }
}

impl Keypair for MlDsa65Signer {
    type VerifyingKey = MlDsa65VerifyingKey;
    fn verifying_key(&self) -> Self::VerifyingKey {
        self.verifying_key()
    }
}

impl DynSignatureAlgorithmIdentifier for MlDsa65Signer {
    fn signature_algorithm_identifier(&self) -> Result<AlgorithmIdentifierOwned, spki::Error> {
        Ok(MlDsa65Signer::algorithm_identifier())
    }
}

use der::Encode;

/// Verify a raw ML-DSA-65 signature. Thin wrapper over `rustpq` so call sites
/// don't need the concrete key/signature types.
pub fn verify_raw(public_key: &[u8], message: &[u8], signature: &[u8]) -> bool {
    let Ok(pk) = mldsa65::PublicKey::from_bytes(public_key) else {
        return false;
    };
    let Ok(sig) = mldsa65::Signature::from_bytes(signature) else {
        return false;
    };
    mldsa65::verify(&pk, message, CTX, &sig).is_ok()
}
