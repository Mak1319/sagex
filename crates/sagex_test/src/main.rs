use sagex_crypto::{
    aes::ITERATIONS,
    pqc::{dsa, kem},
};
use sagex_format::format::{
    KeyDerivatinMechanism::Password, MAGIC_NUMBER, PrivateExternal, PrivateInternal,
    PublicFileFormatExternal, PublicInternal, VERSION,
};
use std::fs::File;
#[cfg(feature = "serde")]
use std::{fs::OpenOptions, io::Write};

use binrw::BinWrite;

fn main() {
    let (kem_enc, kem_pk) = kem::KeyGen::generate_from_password(b"hi this is awesome")
        .expect("KEM Key encryption failed");

    let (dsa_enc, dsa_pk) = dsa::KeyGen::generate_from_password(b"hi this is awesome")
        .expect("DSA key encryption failed");

    let pfi = PrivateInternal {
        magic_number: MAGIC_NUMBER,
        version: VERSION,
        iteration_count: ITERATIONS as usize,
        key_derivation_mechanism_kem: Password,
        key_derivation_mechanism_dsa: Password,
        key_encapsulation_kem: kem_enc,
        key_encapsulation_dsa: dsa_enc,
        user_name: "mainak manna".into(),
    };

    #[cfg(feature = "serde")]
    {
        let bytes: Vec<u8> = postcard::to_allocvec(&pfi).expect("Can not serialize the files");
        // fs::write("~/.pdk.pk", bytes).expect("Can not write to storage")
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(".pdk.pk")
            .expect("Can not open file");
        file.write_all(&bytes).expect("Can not write to file");
    }

    let prv = PrivateExternal {
        internal: pfi,
        identity: b"mainak manna".to_vec(),
    };
    let mut prv_file = File::create(".prv").expect("Can not create .prv file");
    prv.write(&mut prv_file).expect("Can not write .prv file");

    // --- public side (.pub) ---
    let pub_in = PublicInternal {
        magic_number: MAGIC_NUMBER,
        version: VERSION,
        key_kem: kem_pk,
        key_dsa: dsa_pk,
        user_name: "mainak manna".into(),
    };
    let publ = PublicFileFormatExternal {
        internal: pub_in,
        ecc: vec![],
        identity: b"mainak manna".to_vec(),
    };
    let mut pub_file = File::create(".pub").expect("Can not create .pub file");
    publ.write(&mut pub_file).expect("Can not write .pub file");

    let cwd = std::env::current_dir().expect("Can not get current directory");
    println!("wrote .prv + .pub in {}", cwd.display());
}
