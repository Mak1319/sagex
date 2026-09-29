//! sagex-chatsrv library root (the binary in `main.rs` is a thin wrapper).
//! Integration tests build [`state::AppState`] directly with an
//! in-memory [`storage::FakeStorage`] and exercise the HTTP API.

pub mod auth;
pub mod common;
pub mod config;
pub mod engage;
pub mod error;
pub mod jose_mldsa;
pub mod media;
pub mod models;
pub mod rooms_more;
pub mod routes;
pub mod state;
pub mod storage;
pub mod ws;
