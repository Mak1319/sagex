//! Minimal SigV4 query-auth presigner for an S3-compatible store
//! (RustFS). No AWS SDK: hmac + sha2 only. HTTP endpoints only for the
//! bucket-ensure call; presigned URLs echo the configured scheme.

use chrono::Utc;
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;

#[derive(Debug, Clone)]
pub struct S3 {
    pub scheme: String,
    pub host: String,
    pub port: Option<u16>,
    pub bucket: String,
    pub region: String,
    pub access_key: String,
    pub secret_key: String,
}

#[derive(Debug)]
pub enum S3Error {
    BadEndpoint(String),
    Io(String),
}

impl std::fmt::Display for S3Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BadEndpoint(e) => write!(f, "bad endpoint: {e}"),
            Self::Io(e) => write!(f, "s3 io: {e}"),
        }
    }
}

impl std::error::Error for S3Error {}

impl S3 {
    pub fn from_config(endpoint: &str, bucket: &str, region: &str, access: &str, secret: &str) -> Result<Self, S3Error> {
        let (scheme, rest) = endpoint
            .split_once("://")
            .ok_or_else(|| S3Error::BadEndpoint("need scheme://host[:port]".to_string()))?;
        if scheme != "http" && scheme != "https" {
            return Err(S3Error::BadEndpoint("scheme must be http(s)".to_string()));
        }
        let (host, port) = match rest.split_once(':') {
            Some((h, p)) => (
                h.to_string(),
                Some(p.parse::<u16>().map_err(|_| S3Error::BadEndpoint("bad port".to_string()))?),
            ),
            None => (rest.to_string(), None),
        };
        if host.is_empty() {
            return Err(S3Error::BadEndpoint("empty host".to_string()));
        }
        Ok(Self {
            scheme: scheme.to_string(),
            host,
            port,
            bucket: bucket.to_string(),
            region: region.to_string(),
            access_key: access.to_string(),
            secret_key: secret.to_string(),
        })
    }

    fn host_header(&self) -> String {
        match self.port {
            Some(p) if (self.scheme == "http" && p != 80) || (self.scheme == "https" && p != 443) => {
                format!("{}:{p}", self.host)
            }
            _ => self.host.clone(),
        }
    }

    fn base_url(&self) -> String {
        match self.port {
            Some(p) => format!("{}://{}:{p}", self.scheme, self.host),
            None => format!("{}://{}", self.scheme, self.host),
        }
    }
}

fn hex(data: &[u8]) -> String {
    let mut s = String::with_capacity(data.len() * 2);
    for b in data {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("hmac key");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

/// SigV4 percent-encoding. `slash_ok` keeps '/' (object-key paths).
fn encode(s: &str, slash_ok: bool) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(b as char)
            }
            b'/' if slash_ok => out.push('/'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn sha256_hex(data: &[u8]) -> String {
    use sha2::Digest;
    hex(&sha2::Sha256::digest(data))
}

pub struct Presigned {
    pub url: String,
    pub expires_in: u64,
}

pub fn presign(
    s3: &S3,
    method: &str,
    key: &str,
    expires_secs: u64,
) -> Presigned {
    let now = Utc::now();
    let date = now.format("%Y%m%d").to_string();
    let amz_date = now.format("%Y%m%dT%H%M%SZ").to_string();
    let credential_scope = format!("{}/{}/s3/aws4_request", date, s3.region);
    let credential = format!("{}/{credential_scope}", s3.access_key);

    let canonical_uri = format!("/{}/{}", s3.bucket, encode(key, true));
    let query = format!(
        "X-Amz-Algorithm=AWS4-HMAC-SHA256&X-Amz-Credential={}&X-Amz-Date={}&X-Amz-Expires={}&X-Amz-SignedHeaders=host",
        encode(&credential, false),
        amz_date,
        expires_secs,
    );
    let canonical_headers = format!("host:{}\n", s3.host_header());
    let canonical_request = format!(
        "{method}\n{canonical_uri}\n{query}\n{canonical_headers}\nhost\nUNSIGNED-PAYLOAD"
    );
    let to_sign = format!(
        "AWS4-HMAC-SHA256\n{amz_date}\n{credential_scope}\n{}",
        sha256_hex(canonical_request.as_bytes())
    );
    let k_date = hmac_sha256(format!("AWS4{}", s3.secret_key).as_bytes(), date.as_bytes());
    let k_region = hmac_sha256(&k_date, s3.region.as_bytes());
    let k_service = hmac_sha256(&k_region, b"s3");
    let k_signing = hmac_sha256(&k_service, b"aws4_request");
    let signature = hex(&hmac_sha256(&k_signing, to_sign.as_bytes()));

    Presigned {
        url: format!("{}{canonical_uri}?{query}&X-Amz-Signature={signature}", s3.base_url()),
        expires_in: expires_secs,
    }
}

/// Best-effort bucket creation over plain HTTP (dev endpoints).
/// Returns Ok on 200/204/409, Err otherwise — callers must warn, not fail.
pub async fn ensure_bucket(s3: &S3) -> Result<(), S3Error> {
    if s3.scheme != "http" {
        return Err(S3Error::BadEndpoint(
            "https bucket-ensure not implemented; create the bucket via console".to_string(),
        ));
    }
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpStream,
        time::{Duration, timeout},
    };
    let addr = match s3.port {
        Some(p) => format!("{}:{p}", s3.host),
        None => format!("{}:80", s3.host),
    };
    let mut stream = timeout(Duration::from_secs(5), TcpStream::connect(&addr))
        .await
        .map_err(|e| S3Error::Io(e.to_string()))?
        .map_err(|e| S3Error::Io(e.to_string()))?;
    let req = format!(
        "PUT /{} HTTP/1.1\r\nHost: {}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        s3.bucket,
        s3.host_header()
    );
    timeout(Duration::from_secs(5), stream.write_all(req.as_bytes()))
        .await
        .map_err(|e| S3Error::Io(e.to_string()))?
        .map_err(|e| S3Error::Io(e.to_string()))?;
    let mut buf = vec![0u8; 4096];
    let n = timeout(Duration::from_secs(5), stream.read(&mut buf))
        .await
        .map_err(|e| S3Error::Io(e.to_string()))?
        .map_err(|e| S3Error::Io(e.to_string()))?;
    let head = String::from_utf8_lossy(&buf[..n]);
    let status = head.lines().next().unwrap_or("").to_string();
    if status.contains(" 200") || status.contains(" 204") || status.contains(" 409") {
        Ok(())
    } else {
        Err(S3Error::Io(format!("unexpected response: {status}")))
    }
}
