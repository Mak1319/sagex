use std::{fmt, io, net::AddrParseError, num::ParseIntError, path::PathBuf};

#[derive(Debug)]
pub enum AppError {
    BadPort(ParseIntError),
    BadHost(AddrParseError),
    MissingEnv(&'static str),
    BadMongoUri(mongodb::error::Error),
    Mongo(mongodb::error::Error),
    ReadDir(PathBuf, io::Error),
    // Used once trusted-keys loading is wired into `listen`.
    #[allow(dead_code)]
    ReadKey(PathBuf, io::Error),
    // Used once trusted-keys loading is wired into `listen`.
    #[allow(dead_code)]
    BadKey(PathBuf, String),
    KeyGen(sagex_archive::KeyGenError),
    CopyKey(PathBuf, io::Error),
    Bind(io::Error),
    Serve(io::Error),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadPort(e) => write!(f, "invalid PORT: {e}"),
            Self::BadHost(e) => write!(f, "invalid HOST: {e}"),
            Self::MissingEnv(key) => write!(f, "missing environment variable: {key}"),
            Self::BadMongoUri(e) => write!(f, "invalid MONGODB_URI: {e}"),
            Self::Mongo(e) => write!(f, "mongodb error: {e}"),
            Self::ReadDir(dir, e) => write!(f, "cannot read keys dir {}: {e}", dir.display()),
            Self::ReadKey(path, e) => write!(f, "cannot read key {}: {e}", path.display()),
            Self::BadKey(path, e) => write!(f, "invalid key {}: {e}", path.display()),
            Self::KeyGen(e) => write!(f, "key generation failed: {e}"),
            Self::CopyKey(path, e) => write!(f, "cannot copy key to {}: {e}", path.display()),
            Self::Bind(e) => write!(f, "failed to bind listener: {e}"),
            Self::Serve(e) => write!(f, "server error: {e}"),
        }
    }
}

impl std::error::Error for AppError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::BadPort(e) => Some(e),
            Self::BadHost(e) => Some(e),
            Self::MissingEnv(_) => None,
            Self::BadMongoUri(e) => Some(e),
            Self::Mongo(e) => Some(e),
            Self::ReadDir(_, e) => Some(e),
            Self::ReadKey(_, e) => Some(e),
            Self::BadKey(_, _) => None,
            Self::KeyGen(e) => Some(e),
            Self::CopyKey(_, e) => Some(e),
            Self::Bind(e) => Some(e),
            Self::Serve(e) => Some(e),
        }
    }
}
