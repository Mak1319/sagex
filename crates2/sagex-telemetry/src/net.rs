//! Background work helper: blocking calls run on the background
//! executor, results come back to the entity on the UI thread.
//! The main thread never blocks on the network.

use gpui::{AsyncApp, Context, WeakEntity};

/// Spawn blocking `work` off-thread; `done` runs back on the UI thread.
/// If the view is gone by then, the result is dropped.
pub fn spawn_bg<T, R, W>(
    cx: &mut Context<T>,
    work: W,
    done: impl FnOnce(WeakEntity<T>, R, &mut AsyncApp) + Send + 'static,
) where
    T: 'static,
    R: Send + 'static,
    W: FnOnce() -> R + Send + 'static,
{
    cx.spawn(async move |this, cx| {
        let out = cx
            .background_executor()
            .spawn(async move { work() })
            .await;
        done(this, out, cx);
    })
    .detach();
}
