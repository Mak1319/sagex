use clap::{Args, Parser, Subcommand};
use promptuity::{
    Promptuity, Term,
    prompts::{Input, Password},
    themes::FancyTheme,
};
use sagex_keys::{
    DerivationStrategy, KEY_FORMAT_MAGIC_BYTES, KeyExternal, KeyInternal, KeyType,
    PublicKeyInternal,
};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "kunji", version, about = "sagex key manager")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    Gen(GenArgs),
    Get(GetArgs),
}

#[derive(Args, Debug)]
struct GenArgs {
    #[arg(long)]
    email: Option<String>,
    #[arg(long)]
    name: Option<String>,
    #[arg(long)]
    address: Option<String>,
    #[arg(long, value_name = "DIR")]
    out_dir: Option<PathBuf>,
}

#[derive(Args, Debug)]
struct GetArgs {
    #[arg(short = 'i', long = "input")]
    input: PathBuf,
}

fn main() {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Gen(args) => run_gen(args),
        Cmd::Get(args) => run_get(args),
    }
}

fn prompt_result<T>(res: Result<T, promptuity::Error>) -> T {
    match res {
        Ok(v) => v,
        Err(e) => {
            eprintln!("prompt aborted: {e}");
            std::process::exit(1);
        }
    }
}

fn ask_text<W: std::io::Write>(
    p: &mut Promptuity<'_, W>,
    message: &str,
    initial: Option<String>,
    extra_check: Option<fn(&str) -> Result<(), String>>,
) -> String {
    if let Some(v) = initial {
        let ok = v.trim().is_empty() || extra_check.map(|f| f(&v).is_err()).unwrap_or(false);
        if !ok {
            return v;
        }
    }
    let mut input = Input::new(message);
    input.with_validator(move |value: &String| {
        if value.trim().is_empty() {
            return Err("value must not be empty".to_string());
        }
        if let Some(f) = extra_check {
            f(value)?;
        }
        Ok(())
    });
    prompt_result(p.prompt(&mut input))
}

fn ask_password<W: std::io::Write>(p: &mut Promptuity<'_, W>, message: &str) -> String {
    let mut input = Password::new(message);
    input.with_validator(|value: &String| {
        if value.len() < 8 {
            Err("password must be at least 8 characters".to_string())
        } else {
            Ok(())
        }
    });
    prompt_result(p.prompt(&mut input))
}

fn strategy_name(s: &DerivationStrategy) -> &'static str {
    match s {
        DerivationStrategy::Softwere => "software",
        DerivationStrategy::Hardwere => "hardware",
        DerivationStrategy::Password => "password",
    }
}

fn run_gen(args: GenArgs) {
    let mut term = Term::default();
    let mut theme = FancyTheme::default();
    let mut p = Promptuity::new(&mut term, &mut theme);
    prompt_result(p.with_intro("kunji key generation").begin());

    let email = ask_text(
        &mut p,
        "Email address",
        args.email,
        Some(|v: &str| {
            if v.contains('@') {
                Ok(())
            } else {
                Err("email must contain @".to_string())
            }
        }),
    );
    let name = ask_text(&mut p, "Full name", args.name, None);
    let address = ask_text(&mut p, "Address", args.address, None);

    let password = ask_password(&mut p, "Password");
    let confirm = ask_password(&mut p, "Confirm password");
    if password != confirm {
        eprintln!("passwords do not match");
        std::process::exit(1);
    }
    prompt_result(p.with_outro(format!("Generating key for {email}")).finish());

    let file_stem: String = email
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let out_dir = args.out_dir.unwrap_or_else(|| PathBuf::from("."));

    let internal = KeyInternal::new(
        password.as_bytes(),
        email.clone(),
        format!("{name}|{address}"),
    )
    .unwrap_or_else(|_| {
        eprintln!("key generation failed");
        std::process::exit(1);
    });

    // Serialize the private file first (borrows), then move the live
    // key out and convert it to public. A postcard round-trip would lose
    // the public bytes (KeyEncapsulation::public_key is #[serde(skip)]),
    // so the live object must be consumed directly.
    let private_ext = KeyExternal {
        format_magic: KEY_FORMAT_MAGIC_BYTES,
        internals: KeyType::Private(internal),
        key_derivation_strategy: DerivationStrategy::Password,
    };
    let prv_bytes = postcard::to_allocvec(&private_ext).unwrap_or_else(|_| {
        eprintln!("cannot encode private key");
        std::process::exit(1);
    });

    let KeyExternal {
        internals: KeyType::Private(inner),
        ..
    } = private_ext
    else {
        eprintln!("internal error");
        std::process::exit(1);
    };
    let public_ext = {
        let public = PublicKeyInternal::try_from(inner).unwrap_or_else(|_| {
            eprintln!("public key derivation failed");
            std::process::exit(1);
        });
        KeyExternal {
            format_magic: KEY_FORMAT_MAGIC_BYTES,
            internals: KeyType::Public(public),
            key_derivation_strategy: DerivationStrategy::Password,
        }
    };
    let pub_bytes = postcard::to_allocvec(&public_ext).unwrap_or_else(|_| {
        eprintln!("cannot encode public key");
        std::process::exit(1);
    });

    let prv_path = out_dir.join(format!("{file_stem}.prv"));
    let pub_path = out_dir.join(format!("{file_stem}.pub"));
    if let Err(e) = std::fs::write(&prv_path, prv_bytes) {
        eprintln!("cannot write {}: {e}", prv_path.display());
        std::process::exit(1);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Err(e) = std::fs::set_permissions(&prv_path, std::fs::Permissions::from_mode(0o600))
        {
            eprintln!("cannot set permissions on {}: {e}", prv_path.display());
            std::process::exit(1);
        }
    }
    if let Err(e) = std::fs::write(&pub_path, pub_bytes) {
        eprintln!("cannot write {}: {e}", pub_path.display());
        std::process::exit(1);
    }

    println!("wrote {}", prv_path.display());
    println!("wrote {}", pub_path.display());
}

fn run_get(args: GetArgs) {
    let bytes = std::fs::read(&args.input).unwrap_or_else(|e| {
        eprintln!("cannot read {}: {e}", args.input.display());
        std::process::exit(1);
    });
    let ext: KeyExternal = postcard::from_bytes(&bytes).unwrap_or_else(|_| {
        eprintln!("not a kunji key file");
        std::process::exit(1);
    });
    if ext.format_magic != KEY_FORMAT_MAGIC_BYTES {
        eprintln!("bad magic bytes");
        std::process::exit(1);
    }
    println!("strategy: {}", strategy_name(&ext.key_derivation_strategy));
    match ext.internals {
        KeyType::Private(_) => {
            println!("type: private (identity hidden without unlock)");
        }
        KeyType::Public(k) => {
            println!("type: public");
            println!("user: {}", k.user_name);
            println!("other: {}", k.user_others);
            println!("dsa public bytes: {}", k.dsa_key.len());
            println!("kem public bytes: {}", k.kem_key.len());
        }
    }
}
