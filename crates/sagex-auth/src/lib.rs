//! Shared sagex identity primitives.
//!
//! - [`username`]: the single username rule (user-chosen names, also used as
//!   X.509 `CN=` values in sagex-certauth).
//! - [`permit`]: server-issued JOSE permits (JWS-shaped, ML-DSA-65 signed).
//!   Minted by sagex-certauth, verified by sagex-certauth (CSR) and
//!   sagex-gateway (`POST /register`) — literally the same code.

pub mod permit;
pub mod username;

pub use permit::{
    PermitClaims, PermitError, DSA_PUBLIC_KEY_BYTES, PERMIT_ALG, PERMIT_TYP, mint, now_secs,
    verify,
};
pub use username::valid_username;
