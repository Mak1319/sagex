pub mod kem {
    use rand_core::OsRng;
    use rustpq::ml_kem_hybrid::x25519_mlkem768::generate;

    use crate::{
        aes::{AESHandler, KeyEncapsulation},
        error::SageXResult,
    };

    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    pub struct KeyGen;

    impl KeyGen {
        pub fn generate_from_password(password: &[u8]) -> SageXResult<(KeyEncapsulation, Vec<u8>)> {
            let (pk, sk) = generate(&mut OsRng);
            let encap = AESHandler::encrypt_private_key(sk.as_bytes().as_ref(), password)?;
            Ok((encap, pk.as_bytes().to_vec()))
        }
    }
}

pub mod dsa {
    use rand_core::OsRng;
    use rustpq::ml_dsa::mldsa65::generate;

    use crate::{
        aes::{AESHandler, KeyEncapsulation},
        error::SageXResult,
    };

    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    pub struct KeyGen;

    impl KeyGen {
        pub fn generate_from_password(password: &[u8]) -> SageXResult<(KeyEncapsulation, Vec<u8>)> {
            let (pk, sk) = generate(&mut OsRng);
            let encap = AESHandler::encrypt_private_key(sk.as_bytes().as_ref(), password)?;
            Ok((encap, pk.as_bytes().to_vec()))
        }
    }
}
