//! Shared sagex identity primitives.
//!
//! - [`username`]: the single username rule (user-chosen names, also used as
//!   X.509 `CN=` values in sagex-certauth).
//! - [`permit`]: server-issued JOSE permits (JWS-shaped, ML-DSA-65 signed).
//!   Minted by sagex-certauth, verified by sagex-certauth (CSR) and
//!   sagex-gateway (`POST /register`) — literally the same code.
//! - [`chatsrv_token`]: verification of chatsrv (BLS) JOSE access tokens by
//!   third parties (certauth permit issuance / CSR triple bind). Pins the
//!   chatsrv key; no syncing, no callbacks.

pub mod chatsrv_token;
pub mod permit;
pub mod rg_token;
pub mod username;

pub use permit::{
    PermitClaims, PermitError, DSA_PUBLIC_KEY_BYTES, PERMIT_ALG, PERMIT_TYP,
    PERMIT_KIND_USER, PERMIT_KIND_SERVER, mint, now_secs, verify,
};
pub use username::valid_username;
