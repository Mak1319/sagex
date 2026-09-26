use rand_core::OsRng;
use zeroize::Zeroize;

pub enum Purpose {
    Encapsulation,
    Signature,
}

#[derive(Zeroize)]
#[zeroize(drop)]
pub struct KeyPair {
    pub public: Vec<u8>,
    pub private: Vec<u8>,
}

/// Generates new key for Post Quantum Cryptography with Hybrid system
pub fn generate(alg: Purpose) -> crate::error::Result<KeyPair> {
    match alg {
        Purpose::Encapsulation => {
            let (pk, sk) = rustpq::ml_kem_hybrid::x25519_mlkem768::generate(&mut OsRng);
            Ok(KeyPair {
                public: pk.as_bytes().to_vec(),
                private: sk.as_bytes().to_vec(),
            })
        }
        Purpose::Signature => {
            let (pk, sk) = rustpq::ml_dsa::mldsa65::generate(&mut OsRng);
            Ok(KeyPair {
                public: pk.as_bytes().to_vec(),
                private: sk.as_bytes().to_vec(),
            })
        }
    }
}
