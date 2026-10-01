//! Async network: a dedicated Tokio runtime lives on its own threads.
//!
//! The GPUI foreground thread never touches I/O. Call sites use:
//! ```ignore
//! cx.spawn(|view, mut cx| async move {
//!     let res = cx.background_executor().spawn(async move {
//!         net_handle().block_on(client.block(7))
//!     }).await;
//!     view.update(&mut cx, |this, cx| { /* ... */ }).ok();
//! }).detach();
//! ```

use std::sync::OnceLock;
use tokio::runtime::{Handle, Runtime};

static NET_RT: OnceLock<Runtime> = OnceLock::new();

/// Global Tokio runtime (multi-thread, 2 workers) for all network I/O.
pub fn net_runtime() -> &'static Runtime {
    NET_RT.get_or_init(|| Runtime::new().expect("sagex-ledger-ui: failed to start network runtime"))
}

/// Handle to the network runtime — `Send + Sync`, cheap to clone.
pub fn net_handle() -> Handle {
    net_runtime().handle().clone()
}
