//! Composer zone: staged file tray + notice pill + message composer.

use gpui::{
    Context, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled,
    Window, div, img, prelude::FluentBuilder, px,
};
use gpui_component::{ActiveTheme, Icon, Sizable, StyledExt as _, input::Input};

use super::{
    ChatApp,
    emoji::{EMOJI_ALIASES, EMOJI_CATEGORIES},
    files::{FileKind, fmt_size},
};
use crate::component::{AttachMenu, EmojiPanel, MessageComposer, NoticePill, ReplyBar};

impl ChatApp {
    /// Staged file tray (WhatsApp-style): cards with live copy state,
    /// batch caption, locked-mode toggle + passphrase, Send/Cancel.
    pub(super) fn render_stage(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        if self.staged.is_empty() {
            return None;
        }
        self.stage_caption
            .update(cx, |s, cx| s.set_placeholder("Add a caption…", window, cx));
        self.stage_pass.update(cx, |s, cx| {
            s.set_placeholder("Passphrase (kept in memory)", window, cx)
        });
        // Tray styling follows the app theme (shadcn tokens: popover,
        // border, muted, primary, danger) — layout mirrors the file mock.
        let theme = cx.theme().clone();
        let staged = self.staged.clone();
        let locked = self.stage_locked;
        let total: u64 = staged.iter().map(|s| s.size).sum();
        let mut cards = div().flex().flex_row().gap_2();
        for s in &staged {
            let sid = s.id;
            let preview_path = s.vault_path.clone().filter(|_| s.kind == FileKind::Image);
            let name = s.name.clone();
            // thumb or type icon
            let visual: gpui::AnyElement = if s.kind == FileKind::Image {
                if let Some(vp) = s.vault_path.clone() {
                    img(vp.as_path()).w_full().h_full().into_any_element()
                } else {
                    div()
                        .size_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_color(theme.muted_foreground)
                        .child(Icon::empty().path("icons/image.svg").large())
                        .into_any_element()
                }
            } else {
                div()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(theme.muted_foreground)
                    .child(Icon::empty().path(s.kind.icon()).large())
                    .into_any_element()
            };
            let mut card = div()
                .w(px(136.))
                .flex_shrink_0()
                .flex()
                .flex_col()
                .gap_1()
                .rounded(px(12.))
                .border_1()
                .border_color(theme.border)
                .bg(theme.background)
                .p(px(6.))
                .child(
                    div()
                        .w_full()
                        .h(px(76.))
                        .rounded(px(8.))
                        .bg(theme.muted)
                        .overflow_hidden()
                        .relative()
                        .child(visual)
                        .when(s.error.is_none() && !s.done(), |t| {
                            t.child(
                                div()
                                    .absolute()
                                    .bottom_0()
                                    .left_0()
                                    .right_0()
                                    .h(px(4.))
                                    .bg(theme.muted_foreground)
                                    .child(
                                        div()
                                            .h_full()
                                            .w(gpui::relative(s.progress.clamp(0.0, 1.0)))
                                            .bg(theme.primary),
                                    ),
                            )
                        })
                        // locked badge floats on the thumb: absolute, so it
                        // never shifts card layout when toggled.
                        .when(locked, |t| {
                            t.child(
                                div()
                                    .absolute()
                                    .bottom(px(6.))
                                    .left(px(6.))
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_1()
                                    .rounded_full()
                                    .bg(theme.primary)
                                    .px_2()
                                    .py(px(1.))
                                    .child(
                                        Icon::empty()
                                            .path("icons/lock.svg")
                                            .size(px(11.))
                                            .text_color(theme.primary_foreground),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(theme.primary_foreground)
                                            .child("Locked"),
                                    ),
                            )
                        }),
                )
                .child(
                    div()
                        .text_xs()
                        .truncate()
                        .text_color(theme.foreground)
                        .child(name),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(format!("{} · {}", s.kind.label(), fmt_size(s.size))),
                );
            if let Some(err) = s.error.clone() {
                card = card.child(div().text_xs().text_color(theme.danger).child(err));
            }
            if locked {
                // (badge itself lives on the thumb overlay below)
            }
            let mut wrap = div().relative().child(card);
            let dot = theme.muted;
            // preview eye / remove ×
            if let Some(vp) = preview_path {
                let vp2 = vp.clone();
                wrap = wrap.child(
                    div()
                        .id(("stage-open", sid))
                        .absolute()
                        .top(px(6.))
                        .left(px(6.))
                        .size(px(22.))
                        .rounded_full()
                        .bg(dot)
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .text_color(theme.foreground)
                        .child(Icon::empty().path("icons/eye.svg").small())
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.preview = Some(super::attach::PreviewSel {
                                path: vp2.clone(),
                                name: this
                                    .staged
                                    .iter()
                                    .find(|s| s.id == sid)
                                    .map(|s| s.name.clone())
                                    .unwrap_or_default(),
                            });
                            cx.notify();
                        })),
                );
            }
            wrap = wrap.child(
                div()
                    .id(("stage-rm", sid))
                    .absolute()
                    .top(px(6.))
                    .right(px(6.))
                    .size(px(24.))
                    .rounded_full()
                    .border_1()
                    .border_color(theme.border)
                    .bg(dot)
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .text_color(theme.foreground)
                    .child(Icon::empty().path("icons/x.svg").small())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.unstage(sid, cx);
                    })),
            );
            cards = cards.child(wrap);
        }
        // NOTE: width comes from the padded wrapper in
        // render_composer_zone (w_full + parent padding insets correctly;
        // w_full + margins overflowed and cropped the right edge).
        let tray = div()
            .id("stage-tray-scroll")
            .w_full()
            .max_h(px(300.))
            .overflow_y_scroll()
            .track_scroll(&self.tray_scroll)
            .flex()
            .flex_col()
            .gap_2()
            .rounded(px(12.))
            .border_1()
            .border_color(if locked { theme.primary } else { theme.border })
            .bg(theme.popover)
            .shadow_lg()
            .p_3()
            .child(
                // header: lock icon + modern switch + count + top-right ×
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .w_full()
                    // lock control: mini ThemeToggle-style segmented switch
                    // (locked | unlocked icons, active side inset)
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(2.))
                            .rounded(px(14.))
                            .border_1()
                            .border_color(theme.border)
                            .bg(theme.muted)
                            .p(px(3.))
                            .child(
                                div()
                                    .id("stage-lock-on")
                                    .size(px(28.))
                                    .flex_shrink_0()
                                    .rounded(px(10.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .cursor_pointer()
                                    .when(locked, |t| t.bg(theme.background))
                                    .text_color(if locked {
                                        theme.foreground
                                    } else {
                                        theme.muted_foreground
                                    })
                                    .child(Icon::empty().path("icons/lock.svg").small())
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.stage_locked = true;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                div()
                                    .id("stage-lock-off")
                                    .size(px(28.))
                                    .flex_shrink_0()
                                    .rounded(px(10.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .cursor_pointer()
                                    .when(!locked, |t| t.bg(theme.background))
                                    .text_color(if locked {
                                        theme.muted_foreground
                                    } else {
                                        theme.foreground
                                    })
                                    .child(Icon::empty().path("icons/lock-open.svg").small())
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.stage_locked = false;
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_row()
                            .items_baseline()
                            .gap_2()
                            .child(
                                div()
                                    .text_sm()
                                    .font_bold()
                                    .text_color(theme.foreground)
                                    .child(format!(
                                        "{} file{} selected",
                                        staged.len(),
                                        if staged.len() == 1 { "" } else { "s" }
                                    )),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(theme.muted_foreground)
                                    .child(format!("({})", fmt_size(total))),
                            ),
                    )
                    .child(
                        div()
                            .id("stage-close")
                            .size(px(32.))
                            .flex_shrink_0()
                            .rounded_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .text_color(theme.muted_foreground)
                            .hover(|s| s.bg(theme.muted))
                            .child(Icon::empty().path("icons/x.svg").large())
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.clear_staged(cx);
                            })),
                    ),
            )
            .child(
                div()
                    .id("stage-cards-scroll")
                    .flex()
                    .flex_row()
                    .gap_2()
                    .min_w(px(0.))
                    .overflow_x_scroll()
                    .child(cards),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .h(px(48.))
                    .child(
                        Icon::empty()
                            .path("icons/image.svg")
                            .large()
                            .text_color(theme.muted_foreground),
                    )
                    .child(
                        // 48px field shell: single-line Inputs ignore height,
                        // so the bordered shell sets the height and a
                        // borderless input lives centered inside it.
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .h_full()
                            .flex()
                            .flex_row()
                            .items_center()
                            .rounded(px(12.))
                            .border_1()
                            .border_color(theme.border)
                            .bg(theme.background)
                            .px_3()
                            .child(
                                Input::new(&self.stage_caption)
                                    .cleanable(true)
                                    .bordered(false)
                                    .appearance(false)
                                    .w_full(),
                            ),
                    )
                    .child(
                        div()
                            .id("stage-send")
                            .size(px(48.))
                            .flex_shrink_0()
                            .rounded_full()
                            .bg(theme.primary)
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .text_color(theme.primary_foreground)
                            .child(Icon::empty().path("icons/send.svg").large())
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.send_staged(cx);
                            })),
                    ),
            );
        Some(tray.into_any_element())
    }

    pub(super) fn render_composer_zone(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let empty = self.composer.read(cx).value().trim().is_empty();
        let staged_tray = self.render_stage(window, cx);
        let staging = staged_tray.is_some();
        let audio_hud = self.render_audio_hud(cx);
        let audio_busy = audio_hud.is_some();
        div()
            .relative()
            .flex()
            .flex_col()
            .w_full()
            // Floating panels sit inside a padded wrapper: parent padding
            // insets them correctly, unlike margins on a w_full child.
            .when_some(staged_tray, |t, tray| {
                t.child(div().w_full().px(px(12.)).pb(px(8.)).child(tray))
            })
            .when_some(audio_hud, |t, hud| {
                t.child(div().w_full().px(px(12.)).pb(px(8.)).child(hud))
            })
            .when(self.show_attach, |t| {
                t.child(AttachMenu::new().on_select(cx.listener(
                    move |this, ix: &usize, _window, cx| {
                        // 0 Document → picker, 1 Photos & videos → picker,
                        // 2 Audio → recorder HUD, 3 Poll → creation dialog.
                        this.show_attach = false;
                        match *ix {
                            0 | 1 => this.pick_files(cx),
                            2 => this.audio_toggle(cx),
                            _ => this.open_poll_sheet(cx),
                        }
                    },
                )))
            })
            .when(self.show_emoji, |t| {
                t.child(
                    EmojiPanel::new(
                        &self.emoji_search,
                        &self.emoji_scroll,
                        EMOJI_CATEGORIES,
                        EMOJI_ALIASES,
                    )
                    .tab(self.emoji_tab)
                    .show_gif(self.show_gif)
                    .on_tab(cx.listener(|this, ix: &usize, _window, cx| {
                        this.emoji_tab = *ix;
                        this.show_gif = false;
                        cx.notify();
                    }))
                    .on_pick(cx.listener(|this, glyph: &String, window, cx| {
                        let cur = this.composer.read(cx).value().to_string();
                        this.composer
                            .update(cx, |s, cx| s.set_value(format!("{cur}{glyph}"), window, cx));
                        cx.notify();
                    }))
                    .on_gif(cx.listener(|this, show: &bool, _window, cx| {
                        this.show_gif = *show;
                        cx.notify();
                    })),
                )
            })
            // reply preview bar (menu Reply actions)
            .when_some(self.reply_to.clone(), |t, (sender, text, color)| {
                t.child(ReplyBar::new(sender, color, text).on_close(cx.listener(
                    |this, _, _, cx| {
                        this.reply_to = None;
                        this.reply_to_id = None;
                        cx.notify();
                    },
                )))
            })
            // transient action feedback pill (menu actions)
            .when_some(self.notice.clone(), |t, msg| {
                t.child(
                    NoticePill::new(msg).on_dismiss(cx.listener(|this, _, _, cx| {
                        this.notice = None;
                        cx.notify();
                    })),
                )
            })
            .child(
                // message input hides while the file tray or audio HUD is open
                div()
                    .flex()
                    .flex_col()
                    .w_full()
                    .when(!staging && !audio_busy, |t| {
                        t.child(
                            MessageComposer::new(&self.composer, empty)
                                .on_attach(cx.listener(|this, _, _, cx| {
                                    this.show_attach = !this.show_attach;
                                    this.show_emoji = false;
                                    cx.notify();
                                }))
                                .on_emoji(cx.listener(|this, _, _, cx| {
                                    this.show_emoji = !this.show_emoji;
                                    this.show_attach = false;
                                    cx.notify();
                                }))
                                .on_send(cx.listener(|this, _, window, cx| {
                                    if this.composer.read(cx).value().trim().is_empty() {
                                        // mic holds are handled by on_mic_down/up;
                                        // a bare click does nothing.
                                        window.prevent_default();
                                        return;
                                    }
                                    // fold an active reply quote into the sent text
                                    // only for local-only chats; server rooms
                                    // thread via reply_to id instead.
                                    let reply_id = this.reply_to_id.clone();
                                    if reply_id.is_none() {
                                        if let Some((sender, text, _)) = this.reply_to.clone() {
                                            let cur = this.composer.read(cx).value().to_string();
                                            this.composer.update(cx, |s, cx| {
                                                s.set_value(
                                                    format!("↩ {sender}: {text} {cur}"),
                                                    window,
                                                    cx,
                                                )
                                            });
                                        }
                                    }
                                    this.reply_to = None;
                                    this.reply_to_id = None;
                                    // live send: optimistic bubble + POST /rooms/:id/messages
                                    let text = this.composer.read(cx).value().trim().to_string();
                                    this.composer
                                        .update(cx, |s, cx| s.set_value("", window, cx));
                                    this.send_remote(text, reply_id, cx);
                                }))
                                .on_mic_down({
                                    let view = cx.entity();
                                    move |_: &mut Window, cx: &mut gpui::App| {
                                        view.update(cx, |this: &mut ChatApp, cx| {
                                            this.audio_hold_start(cx)
                                        });
                                    }
                                })
                                .on_mic_up({
                                    let view = cx.entity();
                                    move |_: &mut Window, cx: &mut gpui::App| {
                                        view.update(cx, |this: &mut ChatApp, cx| {
                                            this.audio_hold_stop(cx)
                                        });
                                    }
                                }),
                        )
                    }),
            )
    }
}
