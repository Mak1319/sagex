pub mod chat_header;
pub mod message;
pub mod stream;

use std::path::{Path, PathBuf};

use gpui_kit::{prelude::*, *};

use crate::{
    components::{
        canvas::{chat_header::ChatHeader, stream::Stream},
        composer::Composer,
        shared::KitIcon,
    },
    theme,
};

/// A file attached to the composer (memory-only for now).
#[derive(Clone)]
pub struct AttachedFile {
    pub name: String,
    pub meta: String,
    pub icon: &'static str,
}

fn file_icon(name: &str) -> &'static str {
    let ext = Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "bmp" => "icons/media-image.svg",
        "zip" | "tar" | "gz" | "7z" | "rar" => "icons/archive.svg",
        "md" | "markdown" | "txt" | "pdf" => "icons/message-text.svg",
        _ => "icons/folder.svg",
    }
}

fn fmt_size(bytes: u64) -> String {
    if bytes >= 1_048_576 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{} KB", bytes / 1024)
    } else {
        format!("{bytes} B")
    }
}

fn attach_from_paths(paths: &[PathBuf]) -> Vec<AttachedFile> {
    paths
        .iter()
        .filter_map(|p| {
            let name = p.file_name()?.to_str()?.to_string();
            let size = std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
            let ext = Path::new(&name)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_uppercase();
            let meta = if ext.is_empty() {
                fmt_size(size)
            } else {
                format!("{ext} • {}", fmt_size(size))
            };
            Some(AttachedFile {
                icon: file_icon(&name),
                name,
                meta,
            })
        })
        .collect()
}

pub struct Canvas {
    composer: Entity<Composer>,
    dragging: bool,
    tray_open: bool,
    pending: Vec<PathBuf>,
    files: Vec<AttachedFile>,
}

impl Canvas {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let canvas = cx.entity().downgrade();
        Self {
            composer: cx.new(|cx| Composer::new(window, cx, canvas)),
            dragging: false,
            tray_open: false,
            pending: Vec::new(),
            files: Vec::new(),
        }
    }

    pub fn add_files(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        self.files.extend(attach_from_paths(&paths));
        self.tray_open = true;
        cx.notify();
    }

    fn tray_visible(&self) -> bool {
        self.dragging || self.tray_open || !self.files.is_empty()
    }
}

impl Render for Canvas {
    fn render(&mut self, _w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme::theme(cx);
        div()
            .id("canvas-drop")
            .relative()
            .flex()
            .flex_col()
            .flex_1()
            .h_full()
            .min_h_0()
            .bg(t.canvas)
            .on_drop(cx.listener(|this: &mut Self, ev: &FileDropEvent, _, cx| {
                match ev {
                    // Paths arrive with Entered; Submit carries none.
                    FileDropEvent::Entered { paths, .. } => {
                        this.pending = paths.0.to_vec();
                        this.dragging = true;
                        cx.notify();
                    }
                    FileDropEvent::Submit { .. } => {
                        let pending = std::mem::take(&mut this.pending);
                        this.files.extend(attach_from_paths(&pending));
                        this.dragging = false;
                        this.tray_open = true;
                        cx.notify();
                    }
                    FileDropEvent::Exited | FileDropEvent::Ended => {
                        this.dragging = false;
                        cx.notify();
                    }
                    FileDropEvent::Pending { .. } => {}
                }
            }))
            // Header (h_auto) + stream (leftover only) + composer (h_auto):
            // the middle flex_1 can never starve the composer.
            .child(ChatHeader)
            .child(Stream)
            .child(self.composer.clone())
            .when(self.tray_visible(), |this| this.child(self.tray(cx)))
    }
}

impl Canvas {
    fn tray(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme::theme(cx);
        let hint = self.dragging && self.files.is_empty();
        div()
            .absolute()
            .right(px(12.0))
            .bottom(px(12.0))
            .w(px(280.0))
            .flex()
            .flex_col()
            .bg(t.lowest)
            .border_1()
            .border_color(if hint { t.primary } else { t.line_strong })
            .rounded(px(t.radius))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .px(px(8.0))
                    .py(px(4.0))
                    .border_b_1()
                    .border_color(t.line)
                    .child(
                        div()
                            .text_size(px(9.5))
                            .font_weight(FontWeight::BOLD)
                            .text_color(if hint { t.primary } else { t.on_variant })
                            .child(if hint {
                                SharedString::from(format!(
                                    "DROP {} FILE{} TO ATTACH",
                                    self.pending.len(),
                                    if self.pending.len() == 1 { "" } else { "S" }
                                ))
                            } else {
                                SharedString::from(format!("ATTACHED ({})", self.files.len()))
                            }),
                    )
                    .child(
                        div()
                            .id("tray-close")
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .w(px(20.0))
                            .h(px(20.0))
                            .rounded(px(t.radius_sm))
                            .text_size(px(11.0))
                            .text_color(t.muted)
                            .cursor_pointer()
                            .child("×")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.tray_open = false;
                                this.dragging = false;
                                cx.notify();
                            })),
                    ),
            )
            .children(self.files.iter().enumerate().map(|(index, file)| {
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .px(px(8.0))
                    .py(px(6.0))
                    .child(
                        div()
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .w(px(28.0))
                            .h(px(28.0))
                            .bg(t.inset)
                            .border_1()
                            .border_color(t.line)
                            .rounded(px(t.radius_sm))
                            .child(KitIcon::new(file.icon, 16.0).color(t.primary)),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_1()
                            .flex_col()
                            .min_w_0()
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(t.on_surface)
                                    .child(
                                        div()
                                            .truncate()
                                            .child(SharedString::from(file.name.clone())),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(9.0))
                                    .text_color(t.muted)
                                    .child(SharedString::from(file.meta.clone())),
                            ),
                    )
                    .child(
                        div()
                            .id(SharedString::from(format!("tray-remove-{index}")))
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .w(px(20.0))
                            .h(px(20.0))
                            .rounded(px(t.radius_sm))
                            .text_size(px(11.0))
                            .text_color(t.muted)
                            .cursor_pointer()
                            .child("×")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if index < this.files.len() {
                                    this.files.remove(index);
                                }
                                cx.notify();
                            })),
                    )
            }))
    }
}
