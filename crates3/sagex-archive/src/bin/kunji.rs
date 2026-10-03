use std::path::PathBuf;

use clap::Parser;
use sagex_keys::{
    EncapsulatedKey, FORMAT_SIGNATURE, InternalKey, PasswordDerivationStrategy, PublicKey, VERSION,
};
use zeroize::Zeroizing;

/// This is kunji, the key maker.
///
/// What it does it makes a fresh password locked key pair for one user and
/// writes the private file and the public file, asking the password twice
/// on the terminal so nobody types it wrong.
#[derive(Parser)]
#[command(
    name = "kunji",
    version,
    about = "make sagex user keys",
    long_about = "Make a fresh password-locked key pair for one user.\n\nWrites <user>.prv (private, password locked) and <user>.pub\n(shareable) into the output folder.\n\nExample:\n  kunji -u alice -o ./keys"
)]
struct KunjiArgs {
    /// This is the user name the key belongs to.
    #[arg(short = 'u', long = "user", value_name = "USER")]
    user_name: String,

    /// This is the folder where the key files land.
    #[arg(
        short = 'o',
        long = "out",
        default_value = "keys",
        value_name = "FOLDER"
    )]
    output_folder: PathBuf,
}

fn main() -> Result<(), String> {
    let kunji_arguments = KunjiArgs::parse();
    run_kunji(&kunji_arguments).map_err(|error| error.to_string())
}

fn run_kunji(kunji_arguments: &KunjiArgs) -> Result<(), KunjiFailure> {
    println!("Generating key");
    let password_prompt = format!("Password for {}: ", kunji_arguments.user_name);
    let typed_password: Zeroizing<String> = Zeroizing::new(
        dialoguer::Password::new()
            .with_prompt(&password_prompt)
            .with_confirmation("Confirm password: ", "Passwords do not match")
            .interact()
            .map_err(|error| KunjiFailure::Terminal(error.to_string()))?,
    );

    println!("{:?}", typed_password);
    let internal_key =
        InternalKey::new(typed_password.as_bytes(), kunji_arguments.user_name.clone())
            .map_err(|_| KunjiFailure::KeyGen)?;
    let wrapped_key = EncapsulatedKey {
        format_signature: FORMAT_SIGNATURE,
        version: VERSION,
        key: internal_key,
        key_config: None,
        derived_from: PasswordDerivationStrategy::Password,
    };
    std::fs::create_dir_all(&kunji_arguments.output_folder)
        .map_err(|error| KunjiFailure::Terminal(error.to_string()))?;
    let private_path = kunji_arguments
        .output_folder
        .join(format!("{}.prv", kunji_arguments.user_name));
    let private_bytes = serde_json::to_string_pretty(&wrapped_key)
        .map_err(|error| KunjiFailure::Terminal(error.to_string()))?;
    std::fs::write(&private_path, private_bytes)
        .map_err(|error| KunjiFailure::Terminal(error.to_string()))?;
    let public_key = PublicKey::from(wrapped_key);
    let public_path = kunji_arguments
        .output_folder
        .join(format!("{}.pub", kunji_arguments.user_name));
    let public_bytes = serde_json::to_string_pretty(&public_key)
        .map_err(|error| KunjiFailure::Terminal(error.to_string()))?;
    std::fs::write(&public_path, public_bytes)
        .map_err(|error| KunjiFailure::Terminal(error.to_string()))?;
    println!("wrote private key {}", private_path.display());
    println!("wrote public key  {}", public_path.display());
    println!("keep the .prv secret and share the .pub with friends");
    Ok(())
}

/// This is every way kunji can fail.
#[derive(Debug)]
enum KunjiFailure {
    Terminal(String),
    KeyGen,
}

impl std::fmt::Display for KunjiFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Terminal(message) => write!(formatter, "terminal: {message}"),
            Self::KeyGen => write!(formatter, "key generation failed"),
        }
    }
}

impl std::error::Error for KunjiFailure {}
