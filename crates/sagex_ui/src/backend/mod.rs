//! Live `sagex-chatsrv` backend: async REST + WS on a dedicated Tokio
//! runtime, session persistence, root-`.env` config.
//!
//! The client layer intentionally exposes the full server surface (some
//! methods are only used by future screens).
#![allow(dead_code)]

pub mod ca_client;
pub mod client;
pub mod config;
pub mod enroll;
pub mod identity;
pub mod runtime;
pub mod session;
pub mod types;
pub mod vault;
pub mod ws;

#[allow(unused_imports)]
pub use ca_client::{CaClient, CertIssued, CertVerify};
pub use client::{ApiClient, ApiResult, BackendError};
pub use config::BackendConfig;
// Public enrollment/vault surface for screens (owned by other workstreams);
// unused until those screens land, so allow the import lint here.
#[allow(unused_imports)]
pub use enroll::{EnrollState, EnrollStatus};
#[allow(unused_imports)]
pub use identity::CsrBody;
pub use runtime::net_handle;
pub use session::{SessionStore, StoredSession, now_unix};
pub use types::*;
#[allow(unused_imports)]
pub use vault::{Vault, default_vault_dir};
pub use ws::{WsCmd, WsEvent, start_ws, ws_url_for};

use gpui::{Context, Task};
use std::future::Future;

/// Run an async network future on the dedicated Tokio runtime, then deliver
/// the result to the view on the GPUI foreground thread.
///
/// No GPUI thread ever performs I/O: `block_on` executes on a background
/// pool thread under the network runtime handle.
pub fn request<V, Fut, T>(
    cx: &mut Context<V>,
    fut: Fut,
    f: impl FnOnce(&mut V, T, &mut Context<V>) + 'static,
) -> Task<()>
where
    V: 'static,
    Fut: Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    cx.spawn(async move |weak, cx| {
        let executor = cx.background_executor().clone();
        let out = executor
            .spawn(async move { net_handle().block_on(fut) })
            .await;
        let _ = weak.update(cx, |v, cx| f(v, out, cx));
    })
}
