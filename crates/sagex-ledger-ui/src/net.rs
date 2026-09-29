//! Bridge GPUI views to the dedicated Tokio network runtime.
//!
//! No GPUI thread ever performs I/O: `block_on` executes on a background
//! pool thread under the network runtime handle.

use gpui::{Context, Task};
use std::future::Future;

use crate::runtime::net_handle;

/// Run an async network future on the dedicated Tokio runtime, then deliver
/// the result to the view on the GPUI foreground thread.
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
