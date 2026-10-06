use sagex_crypto::error::Error;

pub enum KeyVarient {
    Kem,
    Dsa,
}

pub enum KeyError {
    KemDerivationError(Error),
    DsaDerivationError(Error),

    PublicKeyNotProvided(KeyVarient),
    SerializationError,
    DeserializationError,
}
