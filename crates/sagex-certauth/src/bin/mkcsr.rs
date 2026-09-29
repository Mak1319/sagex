//! Test helper: mint a valid CSR body (fresh ephemeral client keys + PoP).
//! Prints `CsrBody` JSON to stdout. Used by the e2e/negative-matrix shell
//! tests; never ships trust — the server re-validates everything.

use base64::{Engine, engine::general_purpose::STANDARD};
use rand_core::{OsRng, RngCore};
use sagex_certauth::{csr::{CsrBody, pop_message}, mldsa::CTX};

fn usage() -> ! {
    eprintln!("usage: mkcsr --identity NAME");
    std::process::exit(2);
}

fn main() {
    let mut args = std::env::args().skip(1);
    let mut identity: Option<String> = None;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--identity" => identity = args.next(),
            _ => usage(),
        }
    }
    let identity = identity.unwrap_or_else(|| usage());

    let mut pw = [0u8; 16];
    OsRng.fill_bytes(&mut pw);
    let (_k_enc, k_pk) =
        sagex_crypto::pqc::kem::KeyGen::generate_from_password(&pw).expect("kem gen");
    let (d_enc, d_pk) =
        sagex_crypto::pqc::dsa::KeyGen::generate_from_password(&pw).expect("dsa gen");
    let msg = pop_message(&identity, &k_pk, &d_pk);
    let sig = sagex_crypto::pqc::dsa::Implement::sign_from_password(d_enc, &pw, &msg, CTX)
        .expect("pop sign");
    let body = CsrBody {
        identity,
        key_kem_b64: STANDARD.encode(&k_pk),
        key_dsa_b64: STANDARD.encode(&d_pk),
        self_sig_b64: STANDARD.encode(sig.as_bytes()),
    };
    println!("{}", serde_json::to_string(&body).expect("json"));
}
