mod app;
mod components;
mod data;
mod models;
mod theme;
mod views;

use std::path::PathBuf;

use gpui::{
    App, Application, AssetSource, Bounds, Context, KeyDownEvent, SharedString, Window,
    WindowBounds, WindowOptions, div, prelude::*, px, size,
};

use app::AppState;
use theme::palette as p;

/// Asset host for `assets/icons/*.svg`. Only `std::fs` reads.
struct AssetDir {
    base: PathBuf,
}

impl AssetSource for AssetDir {
    fn load(&self, path: &str) -> anyhow::Result<Option<std::borrow::Cow<'static, [u8]>>> {
        std::fs::read(self.base.join(path))
            .map(|data| Some(std::borrow::Cow::Owned(data)))
            .map_err(|err| err.into())
    }

    fn list(&self, path: &str) -> anyhow::Result<Vec<SharedString>> {
        std::fs::read_dir(self.base.join(path))
            .map(|entries| {
                entries
                    .filter_map(|e| {
                        e.ok()
                            .and_then(|e| e.file_name().into_string().ok())
                            .map(SharedString::from)
                    })
                    .collect()
            })
            .map_err(|err| err.into())
    }
}

/// Shell view: owns state, routes keystrokes, composes the 4 columns.
pub struct Root {
    pub state: AppState,
}

impl Root {
    fn on_key(&mut self, ev: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let k = ev.keystroke.key.as_str();
        if k == "enter" {
            self.state.send_draft();
        } else if k == "backspace" {
            if self.state.search_focus {
                self.state.search.pop();
            } else {
                self.state.draft.pop();
            }
        } else if k == "escape" {
            self.state.search_focus = false;
        } else if k == "/" && self.state.draft.is_empty() && !self.state.search_focus {
            self.state.search_focus = true;
        } else if k == "m" && self.state.draft.is_empty() && !self.state.search_focus {
            self.state.show_context = !self.state.show_context;
        } else if k == "a" && self.state.draft.is_empty() && !self.state.search_focus {
            self.state.show_actions = !self.state.show_actions;
        } else if k.chars().count() == 1 {
            if self.state.search_focus {
                self.state.search.push_str(k);
            } else {
                self.state.draft.push_str(k);
            }
        }
        cx.notify();
    }
}

impl Render for Root {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let root = cx.entity();
        div()
            .flex()
            .flex_row()
            .size_full()
            .bg(p::canvas())
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
                this.on_key(ev, window, cx);
            }))
            .child(views::rail::rail(&root, &self.state, cx))
            .child(views::sidebar::sidebar(&root, &self.state, cx))
            .child(views::chat::chat(&root, &self.state, cx))
            .child(views::inspector::inspector(&self.state))
    }
}

fn main() {
    let asset_base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets");
    Application::new()
        .with_assets(AssetDir { base: asset_base })
        .run(|cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(1344.0), px(800.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |_, cx| {
                    cx.new(|_| Root {
                        state: AppState::new(),
                    })
                },
            )
            .unwrap();
        });
}
