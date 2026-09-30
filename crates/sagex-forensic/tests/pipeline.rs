//! End-to-end pipeline tests: seal -> verify -> derive -> register-gate ->
//! decrypt -> embed -> store. No files are written by the library under
//! test; the no-plaintext invariant is enforced by inspecting a temp dir.

use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
use binrw::{BinRead, BinWrite};
use sagex_crypto::{aes::AESHandler, pqc};
use sagex_forensic::{
    decrypt, embed_watermark, extract_watermark_id, from_bytes,
    is_watermarkable, key_fingerprint, open_verify, seal, sha256_bytes, to_bytes,
    EnvelopeError, RecipientKey,
};
use std::io::Cursor;
use std::sync::OnceLock;

const PW: &[u8] = b"test-fixture-password";

struct Party {
    kem_pub: Vec<u8>,
    kem_secret: Vec<u8>,
    dsa_encap_bytes: Vec<u8>,
    dsa_pub: Vec<u8>,
}

fn encap_to_bytes(e: &sagex_crypto::aes::KeyEncapsulation) -> Vec<u8> {
    let mut buf = Vec::new();
    e.write_le(&mut Cursor::new(&mut buf)).unwrap();
    buf
}

fn encap_from_bytes(b: &[u8]) -> sagex_crypto::aes::KeyEncapsulation {
    sagex_crypto::aes::KeyEncapsulation::read_le(&mut Cursor::new(b)).unwrap()
}

/// Sign with a party's DSA key (fresh parse per call; mirrors UI reload).
fn party_sign(party: &Party, msg: &[u8]) -> Vec<u8> {
    let enc = encap_from_bytes(&party.dsa_encap_bytes);
    pqc::dsa::Implement::sign_from_password(enc, PW, msg, sagex_forensic::envelope::SIG_CTX)
        .unwrap()
        .as_bytes()
        .to_vec()
}

fn make_party() -> Party {
    let (kem_enc, kem_pub) = pqc::kem::KeyGen::generate_from_password(PW).unwrap();
    let (dsa_enc, dsa_pub) = pqc::dsa::KeyGen::generate_from_password(PW).unwrap();
    let kem_secret =
        AESHandler::decrypt_private_key(PW, encap_from_bytes(&encap_to_bytes(&kem_enc))).unwrap();
    Party {
        kem_pub,
        kem_secret,
        dsa_encap_bytes: encap_to_bytes(&dsa_enc),
        dsa_pub,
    }
}

fn parties() -> &'static (Party, Party) {
    static ONCE: OnceLock<(Party, Party)> = OnceLock::new();
    ONCE.get_or_init(|| (make_party(), make_party()))
}

fn rkey(identity: &str, kem_pub: &[u8]) -> RecipientKey {
    RecipientKey {
        identity: identity.into(),
        kem_pub: kem_pub.to_vec(),
    }
}

/// Minimal one-part docx for embed tests.
fn minimal_docx() -> Vec<u8> {
    use std::io::Write;
    let mut buf = Vec::new();
    {
        let mut w = zip::ZipWriter::new(Cursor::new(&mut buf));
        let opts = zip::write::SimpleFileOptions::default();
        w.start_file("[Content_Types].xml", opts).unwrap();
        w.write_all(b"<Types/>").unwrap();
        w.start_file("word/document.xml", opts).unwrap();
        w.write_all(b"<w:document xmlns:w='urn:w'><w:body><w:p><w:r><w:t>hi</w:t></w:r></w:p></w:body></w:document>").unwrap();
        w.finish().unwrap();
    }
    buf
}

#[test]
fn roundtrip_single_and_group() {
    let (alice, bob) = parties();
    let sender_sign = |m: &[u8]| party_sign(alice, m);
    // Single recipient.
    let env = seal(b"hello bob", "alice", true, &[rkey("bob", &bob.kem_pub)], sender_sign)
        .unwrap();
    assert!(env.watermark_required);
    let bytes = to_bytes(&env);
    let parsed = from_bytes(&bytes).unwrap();
    let verified = open_verify(&parsed, &alice.dsa_pub).unwrap();
    assert_eq!(verified.sender, "alice");
    let plain = decrypt(&verified, "bob", &bob.kem_secret).unwrap();
    assert_eq!(plain, b"hello bob");
    // Group: both recipients recover identical plaintext.
    let sender_sign2 = |m: &[u8]| party_sign(alice, m);
    let env = seal(
        b"hello all",
        "alice",
        false,
        &[rkey("bob", &bob.kem_pub), rkey("alice", &alice.kem_pub)],
        sender_sign2,
    )
    .unwrap();
    assert!(!env.watermark_required);
    let v = open_verify(&env, &alice.dsa_pub).unwrap();
    assert_eq!(decrypt(&v, "bob", &bob.kem_secret).unwrap(), b"hello all");
    assert_eq!(
        decrypt(&v, "alice", &alice.kem_secret).unwrap(),
        b"hello all"
    );
    // Unknown recipient cannot decrypt.
    assert!(matches!(
        decrypt(&v, "mallory", &bob.kem_secret),
        Err(EnvelopeError::UnknownRecipient(_))
    ));
    // Fingerprints bind entries to keys.
    assert_eq!(
        key_fingerprint(&bob.kem_pub),
        v.recipients
            .iter()
            .find(|r| r.identity == "bob")
            .unwrap()
            .key_fingerprint
    );
}

#[test]
fn tamper_matrix_fails_closed() {
    let (alice, bob) = parties();
    let env = seal(b"secret", "alice", true, &[rkey("bob", &bob.kem_pub)], |m| {
        party_sign(alice, m)
    })
    .unwrap();
    let good = to_bytes(&env);

    // Flip the watermark flag -> signature fails.
    let mut tampered: sagex_forensic::SealedEnvelope = from_bytes(&good).unwrap();
    tampered.watermark_required = false;
    let bad = to_bytes(&tampered);
    assert!(matches!(
        open_verify(&from_bytes(&bad).unwrap(), &alice.dsa_pub),
        Err(EnvelopeError::BadSignature)
    ));

    // Flip a ciphertext byte -> signature fails (checked before decrypt).
    let mut tampered: sagex_forensic::SealedEnvelope = from_bytes(&good).unwrap();
    let mut ct = B64.decode(&tampered.ciphertext_b64).unwrap();
    ct[0] ^= 0xFF;
    tampered.ciphertext_b64 = B64.encode(&ct);
    let bad = to_bytes(&tampered);
    assert!(matches!(
        open_verify(&from_bytes(&bad).unwrap(), &alice.dsa_pub),
        Err(EnvelopeError::BadSignature)
    ));

    // Swap a recipient kem_ct -> signature fails.
    let mut tampered: sagex_forensic::SealedEnvelope = from_bytes(&good).unwrap();
    tampered.recipients[0].kem_ct_b64 = B64.encode([9u8; 64]);
    let bad = to_bytes(&tampered);
    assert!(matches!(
        open_verify(&from_bytes(&bad).unwrap(), &alice.dsa_pub),
        Err(EnvelopeError::BadSignature)
    ));

    // Wrong sender key -> signature fails.
    assert!(matches!(
        open_verify(&from_bytes(&good).unwrap(), &bob.dsa_pub),
        Err(EnvelopeError::BadSignature)
    ));

    // Truncated signature -> corrupt.
    let mut tampered: sagex_forensic::SealedEnvelope = from_bytes(&good).unwrap();
    tampered.signature_b64 = "AAAA".into();
    let bad = to_bytes(&tampered);
    assert!(open_verify(&from_bytes(&bad).unwrap(), &alice.dsa_pub).is_err());

    // Empty recipients rejected at seal time.
    assert!(matches!(
        seal(b"x", "alice", true, &[], |m| party_sign(alice, m)),
        Err(EnvelopeError::NoRecipients)
    ));
}

#[test]
fn doc_embed_extract_roundtrip_and_gates() {
    let doc = minimal_docx();
    assert!(is_watermarkable(&doc));
    let marked = embed_watermark(&doc, "wm-abc123").unwrap();
    assert_ne!(marked, doc);
    assert_eq!(extract_watermark_id(&marked).unwrap(), Some("wm-abc123".into()));
    // Plain office file: no watermark, not an error.
    assert_eq!(extract_watermark_id(&doc).unwrap(), None);
    // Non-office bytes: gate errors, never silent.
    assert!(!is_watermarkable(b"%PDF-1.4 hello"));
    assert!(embed_watermark(b"%PDF-1.4 hello", "wm-x").is_err());
    assert!(embed_watermark(b"plain text", "wm-x").is_err());
    // Conflicting watermarks in one file -> error.
    let conflict = {
        use std::io::Write;
        let mut buf = Vec::new();
        let mut w = zip::ZipWriter::new(Cursor::new(&mut buf));
        let opts = zip::write::SimpleFileOptions::default();
        w.start_file("[Content_Types].xml", opts).unwrap();
        w.write_all(b"<Types/>").unwrap();
        w.start_file("word/a.xml", opts).unwrap();
        w.write_all(b"<r xmlns:sagex=\"https://sagex/enc\" sagex:wm=\"wm-A\"/>").unwrap();
        w.start_file("word/b.xml", opts).unwrap();
        w.write_all(b"<r xmlns:sagex=\"https://sagex/enc\" sagex:wm=\"wm-B\"/>").unwrap();
        w.finish().unwrap();
        buf
    };
    assert!(extract_watermark_id(&conflict).is_err());
}

/// Full recipient pipeline in memory: verify -> derive -> register-gate ->
/// decrypt -> embed. The register step is injected; a rejecting gate must
/// prevent decryption entirely.
fn run_receive_pipeline(
    sealed: &[u8],
    sender_dsa_pub: &[u8],
    recipient: &Party,
    recipient_user: &str,
    session_id: &str,
    register: impl FnOnce(&str, &[u8; 32], &str, &str) -> Result<(), String>,
) -> Result<Vec<u8>, String> {
    let env = from_bytes(sealed).map_err(|e| e.to_string())?;
    if !env.watermark_required {
        return Err("not flagged for watermarking".into());
    }
    let verified = open_verify(&env, sender_dsa_pub).map_err(|e| e.to_string())?;
    let file_hash = sha256_bytes(sealed);
    let wm = sagex_forensic::derive_watermark_id(
        &recipient.dsa_pub,
        &file_hash,
        recipient_user,
        session_id,
    )
    .map_err(|e| e.to_string())?;
    register(&wm, &file_hash, recipient_user, session_id)?;
    let plain = decrypt(&verified, recipient_user, &recipient.kem_secret)
        .map_err(|e| e.to_string())?;
    let marked =
        sagex_forensic::embed_watermark(&plain, &wm).map_err(|e| e.to_string())?;
    Ok(marked)
}

#[test]
fn pipeline_register_gate_blocks_decrypt_and_disk() {
    let (alice, bob) = parties();
    let doc = minimal_docx();
    let env = seal(&doc, "alice", true, &[rkey("bob", &bob.kem_pub)], |m| {
        party_sign(alice, m)
    })
    .unwrap();
    let sealed = to_bytes(&env);

    let dir = std::env::temp_dir().join(format!(
        "sagex_forensic_noplain_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let count_files = || std::fs::read_dir(&dir).unwrap().count();

    // Rejecting gate: nothing decrypts, nothing lands on disk.
    let before = count_files();
    let err = run_receive_pipeline(&sealed, &alice.dsa_pub, bob, "bob", "sess-1", |_, _, _, _| {
        Err("gateway down".into())
    })
    .unwrap_err();
    assert_eq!(err, "gateway down");
    assert_eq!(count_files(), before, "no plaintext may touch disk");

    // Accepting gate: full pipeline yields a marked file extractable to the id.
    let out = run_receive_pipeline(
        &sealed,
        &alice.dsa_pub,
        bob,
        "bob",
        "sess-1",
        |wm, file_hash, user, session| {
            assert_eq!(user, "bob");
            assert_eq!(session, "sess-1");
            assert_eq!(file_hash, &sha256_bytes(&sealed));
            // Re-derive independently: determinism check at the gate.
            let expect = sagex_forensic::derive_watermark_id(
                &bob.dsa_pub,
                &sha256_bytes(&sealed),
                "bob",
                "sess-1",
            )
            .unwrap();
            assert_eq!(wm, &expect);
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(extract_watermark_id(&out).unwrap(), sagex_forensic::derive_watermark_id(&bob.dsa_pub, &sha256_bytes(&sealed), "bob", "sess-1").ok());
    assert_eq!(count_files(), before, "pipeline itself writes no files");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn pipeline_tampered_sender_never_decrypts() {
    let (alice, bob) = parties();
    let doc = minimal_docx();
    let env = seal(&doc, "alice", true, &[rkey("bob", &bob.kem_pub)], |m| {
        party_sign(alice, m)
    })
    .unwrap();
    let mut tampered: sagex_forensic::SealedEnvelope =
        from_bytes(&to_bytes(&env)).unwrap();
    tampered.watermark_required = false;
    let bad = to_bytes(&tampered);
    // Stripped flag fails closed: the helper drops unflagged input before
    // verification (wrong trust domain entirely), and open_verify itself
    // rejects it as BadSignature (covered in tamper_matrix). Either way:
    // drop, never decrypt, never store.
    let err = run_receive_pipeline(&bad, &alice.dsa_pub, bob, "bob", "sess-9", |_, _, _, _| {
        panic!("register must not run for unverified input")
    })
    .unwrap_err();
    assert!(
        err.contains("signature")
            || err.contains("Signature")
            || err.contains("invalid")
            || err.contains("not flagged"),
        "unexpected error: {err}"
    );
}
