use std::fs::File;
#[cfg(feature = "serde")]
use std::{fs::OpenOptions, io::Write};

use sagex_crypto::{
    aes::ITERATIONS,
    pqc::{dsa, kem},
};
use sagex_format::format::{
    KeyDerivatinMechanism::Password, MAGIC_NUMBER, PrivateFileFormatExternal,
    PrivateFileFormatInternal, PublicFileFormatExternal, PublicFileFormatInternal, VERSION,
};

use binrw::BinWrite;

fn main() {
    let (kem_enc, kem_pk) = kem::KeyGen::generate_from_password(b"hi this is awesome")
        .expect("KEM Key encryption failed");

    let (dsa_enc, _dsa_pk) = dsa::KeyGen::generate_from_password(b"hi this is awesome")
        .expect("DSA key encryption failed");

    let pfi = PrivateFileFormatInternal {
        magic_number: MAGIC_NUMBER,
        version: VERSION,
        iteration_count: ITERATIONS as usize,
        key_derivation_mechanism_kem: Password,
        key_derivation_mechanism_dsa: Password,
        key_encapsulation_kem: kem_enc,
        key_encapsulation_dsa: dsa_enc,
        user_name: "mainak manna".into(),
    };

    // bincode::en
    // bincode::

    // let bytes: Vec<u8> = postcard::to_allocvec(&pfi).expect("Can not serialize the files");
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

    // #[cfg(not(feature = "serde"))]
    // {
    //     let _ = &pfi;
    //     eprintln!("serde feature disabled: skipping serialization (run with --features serde)");
    // }

    // --- private side (.prv) ---
    // `ecc` is recomputed on write by the format's try_calc wiring, so the
    // struct carries no `ecc` field to fill — only `internal` + `identity`.
    let prv = PrivateFileFormatExternal {
        internal: pfi,
        identity: b"mainak manna".to_vec(),
    };
    let mut prv_file = File::create(".prv").expect("Can not create .prv file");
    prv.write(&mut prv_file).expect("Can not write .prv file");

    // --- public side (.pub) ---
    let pub_in = PublicFileFormatInternal {
        magic_number: MAGIC_NUMBER,
        version: VERSION,
        key: kem_pk,
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
