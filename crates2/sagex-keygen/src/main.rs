//! sagex-keygen: generate a SAGEX private/public key pair.
//!
//! Flow: obtain a wrapping password (TPM-sealed when a TPM is reachable,
//! interactive prompt otherwise), generate ML-KEM-768 + ML-DSA-65 seeds,
//! wrap them per `sagex-format`, and write `<user>.prv` / `<user>.pub`
//! as postcard binaries.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use zeroize::Zeroizing;

use sagex_crypto::pqc::{dsa, kem};
use sagex_format::format::{
    KeyDerivationMechanism, MAGIC_NUMBER, PrivateExternal, PrivateInternal, PublicFileFormatExternal,
    PublicInternal, VERSION, ecc_chunks_for,
};

#[derive(Parser, Debug)]
#[command(name = "sagex-keygen", about = "Generate a SAGEX key pair")]
struct Args {
    /// User name stored in the key files (plain string)
    #[arg(long)]
    username: Option<String>,

    /// Output directory for <username>.prv / <username>.pub
    #[arg(long, default_value = ".")]
    out_dir: PathBuf,

    /// Skip the TPM and always prompt for a password
    #[arg(long)]
    no_tpm: bool,

    /// TPM device node used for sealing
    #[arg(long, default_value = "/dev/tpmrm0")]
    tpm_device: String,

    /// Overwrite existing .prv/.pub/.seal files
    #[arg(long)]
    force: bool,

    /// Read the password from this env var instead of TPM/prompt
    /// (for scripts; the var must already be exported)
    #[arg(long)]
    password_env: Option<String>,
}

type DynError = Box<dyn std::error::Error>;

/// What `TPM2_Create` hands back, in the form that goes on disk next to
/// the `.prv` file. `private` is encrypted to this chip's SRK — the file
/// is inert off the machine that made it. The SRK itself is re-derived
/// on demand (`create_primary` with the same template), so no TPM
/// persistent handles are consumed.
#[derive(serde::Serialize, serde::Deserialize)]
struct SealedBlob {
    private: Vec<u8>,
    public: Vec<u8>,
}

fn prompt_username(arg: Option<String>) -> Result<String, DynError> {
    if let Some(u) = arg {
        if u.trim().is_empty() {
            return Err("username must not be empty".into());
        }
        return Ok(u);
    }
    let u: String = dialoguer::Input::new()
        .with_prompt("Username")
        .interact_text()?;
    if u.trim().is_empty() {
        return Err("username must not be empty".into());
    }
    Ok(u)
}

fn prompt_password() -> Result<Zeroizing<Vec<u8>>, DynError> {
    let pw = dialoguer::Password::new()
        .with_prompt("Password")
        .with_confirmation("Repeat password", "passwords do not match")
        .interact()?;
    Ok(Zeroizing::new(pw.into_bytes()))
}

/// Seal a fresh random password to the TPM. Returns the password bytes
/// plus the sealed blob (goes to `<user>.seal`). Pure-Rust wire protocol,
/// no C TSS. The storage parent is re-derived on demand, so nothing is
/// stored in TPM NV — only the blob file.
fn tpm_sealed_password(device: &str) -> Result<(Zeroizing<Vec<u8>>, SealedBlob), DynError> {
    use purecrypto_tpm::{
        Auth, Tpm,
        transport::DeviceTransport,
        types::{Alg, Buffer, Handle, Public, SensitiveCreate, constants::rh},
    };

    let mut tpm = Tpm::new(DeviceTransport::open(device)?);

    // Restricted ECC storage parent under the owner hierarchy (empty auth).
    // create_primary is deterministic: same template + same chip = same SRK,
    // so unseal later just recreates it — no persistent handles needed.
    let parent_tmpl = Public::ecc_storage_parent(Alg::SHA256);
    let parent = tpm.create_primary(
        Handle(rh::OWNER),
        &SensitiveCreate::default(),
        &parent_tmpl,
        &mut Auth::Password(b""),
    )?;

    // Fresh 32-byte password (GetRandom may return fewer; loop).
    let mut pw = Vec::new();
    while pw.len() < 32 {
        pw.extend_from_slice(&tpm.get_random(32)?);
    }
    pw.truncate(32);

    // Seal under the parent with an empty object auth.
    let mut sealed = SensitiveCreate::default();
    sealed.user_auth = Buffer::empty();
    sealed.data = Buffer::from(&pw[..]);
    let blob = tpm.create(
        parent.handle,
        parent.name.as_slice(),
        &sealed,
        &Public::sealed_data(Alg::SHA256),
        &mut Auth::Password(b""),
    )?;

    // Round-trip check: load + unseal must recover the password now,
    // otherwise the .seal file would be useless. Then flush everything.
    let obj = tpm.load(
        parent.handle,
        parent.name.as_slice(),
        &blob.private,
        &blob.public,
        &mut Auth::Password(b""),
    )?;
    let back = tpm.unseal(obj.handle, obj.name.as_slice(), &mut Auth::Password(b""))?;
    if back != pw {
        return Err("TPM seal round-trip mismatch".into());
    }
    tpm.flush_context(obj.handle)?;
    tpm.flush_context(parent.handle)?;

    let sealed_blob = SealedBlob {
        private: blob.private.into_vec(),
        public: blob.public.into_vec(),
    };
    Ok((Zeroizing::new(pw), sealed_blob))
}

fn write_new(path: PathBuf, bytes: &[u8], force: bool) -> Result<(), DynError> {
    let mut opts = OpenOptions::new();
    opts.write(true);
    if force {
        opts.create(true).truncate(true);
    } else {
        opts.create_new(true);
    }
    let mut f = opts
        .open(&path)
        .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    f.write_all(bytes)?;
    Ok(())
}

fn run(args: Args) -> Result<(), DynError> {
    let username = prompt_username(args.username)?;

    // 1. Password: explicit env var wins (scripts), else TPM-sealed
    // when reachable, else interactive prompt.
    let (password, kdm, seal_blob) = if let Some(var) = args.password_env.as_deref() {
        let pw = std::env::var(var)
            .map_err(|_| format!("env var {var} is not set or not unicode"))?;
        if pw.is_empty() {
            return Err(format!("env var {var} is empty").into());
        }
        (
            Zeroizing::new(pw.into_bytes()),
            KeyDerivationMechanism::Password,
            None,
        )
    } else if !args.no_tpm {
        match tpm_sealed_password(&args.tpm_device) {
            Ok((pw, blob)) => {
                println!("TPM: password sealed (blob kept in {}.seal)", username);
                (pw, KeyDerivationMechanism::TPM, Some(blob))
            }
            Err(e) => {
                eprintln!("TPM unavailable ({e}); falling back to password prompt");
                (prompt_password()?, KeyDerivationMechanism::Password, None)
            }
        }
    } else {
        (prompt_password()?, KeyDerivationMechanism::Password, None)
    };

    // 2. Generate + wrap both seeds.
    let (enc_kem, ek_pub) = kem::KeyGen::generate_from_password(&password)?;
    let (enc_dsa, vk_pub) = dsa::KeyGen::generate_from_password(&password)?;

    // 3. Private side (.prv).
    let internal = PrivateInternal {
        magic_number: MAGIC_NUMBER,
        version: VERSION,
        key_encapsulation_kem: enc_kem,
        key_encapsulation_dsa: enc_dsa,
        user_name: username.clone(),
        key_derivation_mechanism_kem: kdm,
        key_derivation_mechanism_dsa: kdm,
        iteration_count: sagex_crypto::aes::ITERATIONS as usize,
    };
    let ecc = ecc_chunks_for(&internal);
    let identity = username.as_bytes().to_vec();
    let prv = PrivateExternal {
        internal,
        ecc,
        identity: identity.clone(),
    };

    // 4. Public side (.pub).
    let publ = PublicFileFormatExternal {
        internal: PublicInternal {
            magic_number: MAGIC_NUMBER,
            version: VERSION,
            key_kem: ek_pub,
            key_dsa: vk_pub,
            user_name: username.clone(),
        },
        ecc: vec![],
        identity,
    };

    // 5. Write postcard binaries (+ sealed blob when TPM was used).
    std::fs::create_dir_all(&args.out_dir)?;
    write_new(
        args.out_dir.join(format!("{username}.prv")),
        &prv.to_bytes()?,
        args.force,
    )?;
    write_new(
        args.out_dir.join(format!("{username}.pub")),
        &publ.to_bytes()?,
        args.force,
    )?;
    if let Some(blob) = seal_blob {
        write_new(
            args.out_dir.join(format!("{username}.seal")),
            &postcard::to_allocvec(&blob)?,
            args.force,
        )?;
        println!("wrote {username}.prv + {username}.pub + {username}.seal in {}", args.out_dir.display());
    } else {
        println!("wrote {username}.prv + {username}.pub in {}", args.out_dir.display());
    }
    Ok(())
}

fn main() -> ExitCode {
    match run(Args::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("sagex-keygen: error: {e}");
            ExitCode::FAILURE
        }
    }
}
