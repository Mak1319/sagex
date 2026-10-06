pub mod markdown;

use gpui_kit::{
    component::{ActiveTheme as _, input::{Editor, EditorState}},
    prelude::*,
    *,
};

use self::markdown::{
    chat_markdown_factory, insert_at, prefix_lines, wrap_link, wrap_selection,
};
use crate::{
    components::{canvas::Canvas, shared::{IconBtn, KitIcon, PresenceDot}},
    theme,
};

#[derive(Clone, Copy, PartialEq)]
pub enum ComposerMode {
    Message,
    Markdown,
}

/// Bottom composer. One markdown truth across two editor states:
/// MESSAGE is the editable rendered view (markers concealed, sans text),
/// MARKDOWN is the raw-source editor. SEND stays visual-only.
pub struct Composer {
    mode: ComposerMode,
    message: Entity<EditorState>,
    markdown: Entity<EditorState>,
    code_open: bool,
    code: Entity<EditorState>,
    canvas: WeakEntity<Canvas>,
}

impl Composer {
    pub fn new(
        window: &mut Window,
        cx: &mut Context<Self>,
        canvas: WeakEntity<Canvas>,
    ) -> Self {
        let blue = theme::theme(cx).tint(crate::theme::TintColor::Blue);
        let t = theme::theme(cx);
        let message = cx.new(|cx| {
            let mut state = EditorState::new(window, cx)
                .language("chat-markdown")
                .placeholder("Type technical parameter or command...")
                // Document field, not a code editor: no gutter, no fold
                // markers, no bracket pairing while typing prose.
                .line_number(false)
                .folding(false)
                .auto_close(false);
            state.set_highlighter_factory(
                chat_markdown_factory(blue.bg.into(), blue.fg.into(), t.inset.into()),
                cx,
            );
            state
        });
        let markdown =
            cx.new(|cx| EditorState::new(window, cx).placeholder("Write raw Markdown..."));
        let code = cx.new(|cx| EditorState::new(window, cx).language("plaintext"));
        Self {
            mode: ComposerMode::Message,
            message,
            markdown,
            code_open: false,
            code,
            canvas,
        }
    }

    /// Native file dialog off the UI thread; picked paths land in the
    /// Canvas tray. Cancel is a silent no-op.
    fn pick_files(&self, cx: &mut Context<Self>) {
        let canvas = self.canvas.clone();
        cx.spawn(async move |_, app| {
            let picked = rfd::AsyncFileDialog::new()
                .add_filter("Images", &["png", "jpg", "jpeg", "gif", "webp", "svg"])
                .add_filter("Archives", &["zip", "tar", "gz", "7z"])
                .add_filter("Documents", &["md", "txt", "pdf"])
                .pick_files()
                .await;
            if let Some(files) = picked {
                let paths: Vec<std::path::PathBuf> =
                    files.iter().map(|f| f.path().to_path_buf()).collect();
                let _ = canvas.update(app, |this, cx| this.add_files(paths, cx));
            }
        })
        .detach();
    }

    fn active(&self) -> Entity<EditorState> {
        match self.mode {
            ComposerMode::Message => self.message.clone(),
            ComposerMode::Markdown => self.markdown.clone(),
        }
    }

    fn switch_mode(&mut self, mode: ComposerMode, window: &mut Window, cx: &mut Context<Self>) {
        if self.mode == mode {
            return;
        }
        // Sync the single markdown truth across states (undo-preserving).
        if mode == ComposerMode::Markdown {
            let text = self.message.read(cx).value().to_string();
            self.markdown
                .update(cx, |s, cx| s.replace_all(text, window, cx));
        } else {
            let text = self.markdown.read(cx).value().to_string();
            self.message
                .update(cx, |s, cx| s.replace_all(text, window, cx));
        }
        self.mode = mode;
        cx.notify();
    }

    fn source_edit(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        f: impl FnOnce(&str, std::ops::Range<usize>) -> (String, std::ops::Range<usize>),
    ) {
        let state = self.active();
        let text = state.read(cx).value().to_string();
        let sel = state.read(cx).selected_range();
        let (new_text, caret) = f(&text, sel);
        state.update(cx, |s, cx| {
            s.replace_all(new_text, window, cx);
            s.set_selected_range(caret, cx);
        });
        self.focus_active(window, cx);
    }

    fn focus_active(&self, window: &mut Window, cx: &mut App) {
        let handle = self.active().focus_handle(cx);
        window.focus(&handle, cx);
    }

    fn open_code(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.code_open = true;
        cx.notify();
        let handle = self.code.focus_handle(cx);
        window.focus(&handle, cx);
    }

    fn close_code(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.code_open = false;
        cx.notify();
        self.focus_active(window, cx);
    }

    fn insert_code(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let code = self.code.read(cx).value().to_string();
        let fence = format!("```\n{code}\n```");
        let state = self.active();
        let text = state.read(cx).value().to_string();
        let sel = state.read(cx).selected_range();
        let (new_text, caret) = insert_at(&text, sel, &fence);
        state.update(cx, |s, cx| {
            s.replace_all(new_text, window, cx);
            s.set_selected_range(caret, cx);
        });
        self.code.update(cx, |s, cx| {
            s.replace_all(String::new(), window, cx)
        });
        self.code_open = false;
        cx.notify();
        self.focus_active(window, cx);
    }
}

impl Render for Composer {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme::theme(cx);
        let message_tab = self.mode == ComposerMode::Message;

        let field: AnyElement = match self.mode {
            // Editable rendered view: markers concealed, sans text.
            ComposerMode::Message => Editor::new(&self.message)
                .appearance(false)
                .bordered(false)
                .font_family(cx.theme().font_family.clone())
                .text_size(px(13.0))
                .into_any_element(),
            ComposerMode::Markdown => Editor::new(&self.markdown)
                .appearance(false)
                .bordered(false)
                .into_any_element(),
        };

        // Code-insert overlay (built here so Cancel/Insert get listeners).
        let popup: Option<AnyElement> = self.code_open.then(|| {
            let t = theme::theme(cx);
            div()
                .absolute()
                .left(px(8.0))
                .right(px(8.0))
                .bottom(px(40.0))
                .flex()
                .flex_col()
                .bg(t.lowest)
                .border_1()
                .border_color(t.line_strong)
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
                                .text_color(t.on_variant)
                                .child("INSERT CODE"),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(4.0))
                                .child(
                                    div()
                                        .id("code-cancel")
                                        .px(px(8.0))
                                        .py(px(2.0))
                                        .border_1()
                                        .border_color(t.line)
                                        .rounded(px(t.radius_sm))
                                        .text_size(px(9.5))
                                        .text_color(t.muted)
                                        .cursor_pointer()
                                        .child("Cancel")
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.close_code(window, cx);
                                        })),
                                )
                                .child(
                                    div()
                                        .id("code-insert")
                                        .px(px(8.0))
                                        .py(px(2.0))
                                        .bg(t.primary)
                                        .border_1()
                                        .border_color(t.primary)
                                        .rounded(px(t.radius_sm))
                                        .text_size(px(9.5))
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(t.on_primary)
                                        .cursor_pointer()
                                        .child("Insert")
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.insert_code(window, cx);
                                        })),
                                ),
                        ),
                )
                // No wrapper padding, no scroll container of our own: the
                // editor brings its own padding and scrolls only when its
                // content requires it.
                .child(
                    Editor::new(&self.code)
                        .appearance(false)
                        .bordered(false),
                )
                .into_any_element()
        });

        div()
            .relative()
            .flex_none()
            .flex()
            .flex_col()
            .w_full()
            .bg(t.lowest)
            .border_t_1()
            .border_color(t.line)
            .p(px(8.0))
            // Mode tabs + syntax badge.
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .pb(px(4.0))
                    .mb(px(4.0))
                    .border_b_1()
                    .border_color(t.line)
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(4.0))
                            .p(px(2.0))
                            .bg(t.inset)
                            .border_1()
                            .border_color(t.line)
                            .rounded(px(t.radius_sm))
                            .child(
                                div()
                                    .id("composer-tab-message")
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(4.0))
                                    .px(px(8.0))
                                    .py(px(2.0))
                                    .rounded(px(t.radius_sm))
                                    .text_size(px(9.5))
                                    .cursor_pointer()
                                    .when(message_tab, |this| {
                                        this.bg(t.lowest)
                                            .border_1()
                                            .border_color(t.line)
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(t.primary)
                                    })
                                    .when(!message_tab, |this| this.text_color(t.muted))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.switch_mode(ComposerMode::Message, window, cx);
                                    }))
                                    .when(message_tab, |this| {
                                        this.child(PresenceDot::new(t.primary))
                                    })
                                    .child("MESSAGE"),
                            )
                            .child(
                                div()
                                    .id("composer-tab-markdown")
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(4.0))
                                    .px(px(8.0))
                                    .py(px(2.0))
                                    .rounded(px(t.radius_sm))
                                    .text_size(px(9.5))
                                    .cursor_pointer()
                                    .when(!message_tab, |this| {
                                        this.bg(t.lowest)
                                            .border_1()
                                            .border_color(t.line)
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(t.primary)
                                    })
                                    .when(message_tab, |this| this.text_color(t.muted))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.switch_mode(ComposerMode::Markdown, window, cx);
                                    }))
                                    .child(
                                        KitIcon::new("icons/code.svg", 12.0).color(t.muted),
                                    )
                                    .child("MARKDOWN"),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(4.0))
                            .px(px(6.0))
                            .py(px(1.0))
                            .bg(t.inset)
                            .border_1()
                            .border_color(t.line)
                            .rounded(px(t.radius_sm))
                            .text_size(px(9.5))
                            .text_color(t.muted)
                            .child(PresenceDot::online(cx))
                            .child(
                                div()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(t.on_variant)
                                    .child("SYNTAX:"),
                            )
                            .child("GLSL / MD ACTIVE"),
                    ),
            )
            // Field + toolbar. No padding wrapper and no scroll container
            // around the editor: it sizes itself and scrolls only when its
            // own content requires it.
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_col()
                    .w_full()
                    .bg(t.inset)
                    .border_1()
                    .border_color(t.line)
                    .rounded(px(t.radius_sm))
                    .child(field)
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_between()
                            .px(px(6.0))
                            .py(px(4.0))
                            .bg(t.lowest)
                            .border_t_1()
                            .border_color(t.line)
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(2.0))
                                    .text_color(t.muted)
                                    .child(
                                        IconBtn::new("icons/attachment.svg", 14.0, 20.0)
                                            .on_action(
                                                "attach",
                                                cx.listener(|this, _, _, cx| {
                                                    this.pick_files(cx);
                                                }),
                                            ),
                                    )
                                    .child(IconBtn::new("icons/media-image.svg", 14.0, 20.0))
                                    .child(
                                        IconBtn::new("icons/code.svg", 14.0, 20.0).on_action(
                                            "code",
                                            cx.listener(|this, _, window, cx| {
                                                this.open_code(window, cx);
                                            }),
                                        ),
                                    )
                                    .child(
                                        div()
                                            .flex_none()
                                            .w(px(1.0))
                                            .h(px(12.0))
                                            .bg(t.line)
                                            .mx(px(2.0)),
                                    )
                                    .child(
                                        div()
                                            .id("composer-bold")
                                            .flex_none()
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .w(px(20.0))
                                            .h(px(20.0))
                                            .rounded(px(t.radius_sm))
                                            .text_size(px(11.0))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(t.muted)
                                            .cursor_pointer()
                                            .child("B")
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.source_edit(window, cx, |text, sel| {
                                                    wrap_selection(text, sel, "**", "**")
                                                });
                                            })),
                                    )
                                    .child(
                                        div()
                                            .id("composer-italic")
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
                                            .child("I")
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.source_edit(window, cx, |text, sel| {
                                                    wrap_selection(text, sel, "*", "*")
                                                });
                                            })),
                                    )
                                    .child(
                                        IconBtn::new("icons/list.svg", 14.0, 20.0).on_action(
                                            "bullets",
                                            cx.listener(|this, _, window, cx| {
                                                this.source_edit(window, cx, |text, sel| {
                                                    prefix_lines(text, sel, "- ")
                                                });
                                            }),
                                        ),
                                    )
                                    .child(
                                        IconBtn::new("icons/list-ordered.svg", 14.0, 20.0)
                                            .on_action(
                                                "numbered",
                                                cx.listener(|this, _, window, cx| {
                                                    this.source_edit(window, cx, |text, sel| {
                                                        prefix_lines(text, sel, "1. ")
                                                    });
                                                }),
                                            ),
                                    )
                                    .child(
                                        IconBtn::new("icons/link.svg", 14.0, 20.0).on_action(
                                            "link",
                                            cx.listener(|this, _, window, cx| {
                                                this.source_edit(window, cx, |text, sel| {
                                                    wrap_link(text, sel)
                                                });
                                            }),
                                        ),
                                    )
                                    .child(IconBtn::new("icons/at-sign.svg", 14.0, 20.0)),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(8.0))
                                    .child(
                                        div()
                                            .text_size(px(9.0))
                                            .text_color(t.muted)
                                            .child("⌘ ↵ transmit"),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .bg(t.primary)
                                            .border_1()
                                            .border_color(t.primary)
                                            .rounded(px(t.radius_sm))
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_row()
                                                    .items_center()
                                                    .gap(px(4.0))
                                                    .px(px(8.0))
                                                    .py(px(2.0))
                                                    .text_size(px(11.0))
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(t.on_primary)
                                                    .child("SEND")
                                                    .child(
                                                        KitIcon::new("icons/send.svg", 13.0)
                                                            .color(t.on_primary),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .flex_none()
                                                    .w(px(1.0))
                                                    .h(px(14.0))
                                                    .bg(t.on_primary_dim),
                                            )
                                            .child(
                                                div()
                                                    .px(px(4.0))
                                                    .py(px(2.0))
                                                    .text_color(t.on_primary)
                                                    .child(
                                                        KitIcon::new(
                                                            "icons/nav-arrow-down.svg",
                                                            13.0,
                                                        )
                                                        .color(t.on_primary),
                                                    ),
                                            ),
                                    ),
                            ),
                    )
                    .when_some(popup, |this, popup| this.child(popup)),
            )
    }
}
