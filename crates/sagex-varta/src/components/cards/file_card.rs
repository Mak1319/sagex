//! File card shell — manual `gpui-base` expansion only.
//!
//! One robust shell for every file tile (PDF, image, video, audio, sheet,
//! slides, doc). The kind drives chip label, tint and glyph; everything
//! variable is data or a slot — no fixed widths anywhere, text truncates
//! with an ellipsis on narrow cards.
//!
//! Slots (all optional, all override the matching default):
//! - `leading` — replaces the [`KindBox`] (e.g. a real thumbnail).
//! - `actions` — replaces the whole right button cluster.
//! - `footer` — extra row under the header ([`ScrubberBar`], [`WaveformBar`]
//!   or anything else).
//!
//! Typed builders cover the common case without touching slots:
//! filename + kind chip, `•`-joined meta segments with one optional accent
//! tail, a labeled primary button and a tinted square icon button.

use std::rc::Rc;

use gpui_kit::base::Button as BaseButton;
use gpui_kit::*;

use crate::components::cards::kind::FileKind;
use crate::components::cards::kind_box::KindBox;
use crate::theme;

type CardHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

/// One `•`-separated meta segment (e.g. `14.8 MB`, `CRC Verified`).
pub struct MetaSeg {
    text: SharedString,
    color: u32,
    bold: bool,
}

impl MetaSeg {
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self {
            text: text.into(),
            color: theme::colors::MUTED,
            bold: false,
        }
    }

    pub fn color(mut self, color: u32) -> Self {
        self.color = color;
        self
    }

    pub fn bold(mut self) -> Self {
        self.bold = true;
        self
    }
}

#[derive(IntoElement)]
pub struct FileCard {
    id: Option<SharedString>,
    kind: FileKind,
    filename: SharedString,
    chip: Option<SharedString>,
    meta: Vec<MetaSeg>,
    leading: Option<AnyElement>,
    primary_label: Option<SharedString>,
    primary_icon: Option<SharedString>,
    on_primary: Option<CardHandler>,
    icon_action: Option<SharedString>,
    icon_tint: Option<(u32, u32, u32)>,
    on_icon: Option<CardHandler>,
    actions: Option<AnyElement>,
    footer: Option<AnyElement>,
}

impl FileCard {
    pub fn new(kind: FileKind, filename: impl Into<SharedString>) -> Self {
        Self {
            id: None,
            kind,
            filename: filename.into(),
            chip: None,
            meta: Vec::new(),
            leading: None,
            primary_label: None,
            primary_icon: None,
            on_primary: None,
            icon_action: None,
            icon_tint: None,
            on_icon: None,
            actions: None,
            footer: None,
        }
    }

    pub fn chip(mut self, chip: impl Into<SharedString>) -> Self {
        self.chip = Some(chip.into());
        self
    }

    /// Unique id stem for the card buttons (`{id}-primary`, `{id}-icon`).
    /// Defaults to the filename when unset.
    pub fn id(mut self, id: impl Into<SharedString>) -> Self {
        self.id = Some(id.into());
        self
    }

    pub fn meta(mut self, meta: Vec<MetaSeg>) -> Self {
        self.meta = meta;
        self
    }

    pub fn leading(mut self, leading: impl IntoElement) -> Self {
        self.leading = Some(leading.into_any_element());
        self
    }

    pub fn primary(
        mut self,
        label: impl Into<SharedString>,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.primary_label = Some(label.into());
        self.on_primary = Some(Rc::new(handler));
        self
    }

    pub fn primary_icon(mut self, icon: impl Into<SharedString>) -> Self {
        self.primary_icon = Some(icon.into());
        self
    }

    pub fn icon_action(
        mut self,
        icon: impl Into<SharedString>,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.icon_action = Some(icon.into());
        self.on_icon = Some(Rc::new(handler));
        self
    }

    pub fn icon_tint(mut self, bg: u32, border: u32, fg: u32) -> Self {
        self.icon_tint = Some((bg, border, fg));
        self
    }

    pub fn actions(mut self, actions: impl IntoElement) -> Self {
        self.actions = Some(actions.into_any_element());
        self
    }

    pub fn footer(mut self, footer: impl IntoElement) -> Self {
        self.footer = Some(footer.into_any_element());
        self
    }
}

fn chip(label: SharedString, bg: u32, border: u32, fg: u32) -> impl IntoElement {
    div()
        .flex_shrink_0()
        .px_1()
        .flex()
        .flex_row()
        .items_center()
        .bg(theme::rgb(bg))
        .border_1()
        .border_color(theme::rgb(border))
        .rounded(px(2.0))
        .text_size(px(8.0))
        .font_weight(FontWeight::BOLD)
        .text_color(theme::rgb(fg))
        .child(label)
}

impl RenderOnce for FileCard {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let (tint_bg, tint_border, tint_fg) = self.kind.tint();
        let chip_label: SharedString = self.chip.unwrap_or_else(|| self.kind.chip().into());
        let stem: SharedString = self
            .id
            .unwrap_or_else(|| self.filename.clone());

        let title = div()
            .flex_1()
            .min_w_0()
            .overflow_hidden()
            .text_ellipsis()
            .whitespace_nowrap()
            .text_size(px(11.0))
            .font_weight(FontWeight::BOLD)
            .text_color(theme::rgb(theme::colors::INK))
            .child(self.filename);

        let mut meta_row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap_1()
            .w_full()
            .overflow_hidden();
        for (ix, seg) in self.meta.iter().enumerate() {
            if ix > 0 {
                meta_row = meta_row.child(
                    div()
                        .text_size(px(9.0))
                        .text_color(theme::rgb(theme::colors::MUTED))
                        .child("•"),
                );
            }
            let mut text = div()
                .text_size(px(9.0))
                .whitespace_nowrap()
                .text_color(theme::rgb(seg.color));
            if seg.bold {
                text = text.font_weight(FontWeight::BOLD);
            }
            meta_row = meta_row.child(text.child(seg.text.clone()));
        }

        let meta_col = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .gap_1()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_1()
                    .w_full()
                    .min_w_0()
                    .child(title)
                    .child(chip(chip_label, tint_bg, tint_border, tint_fg)),
            )
            .child(meta_row);

        let leading: AnyElement = self
            .leading
            .unwrap_or_else(|| KindBox::new(self.kind).into_any_element());

        let actions: AnyElement = match self.actions {
            Some(custom) => custom,
            None => {
                let mut cluster = div().flex().flex_row().items_center().gap_1().flex_shrink_0();
                if let Some(label) = self.primary_label {
                    let mut button = BaseButton::new(ElementId::Name(
                        format!("{stem}-primary").into(),
                    ))
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_1()
                        .py_1()
                        .px_2()
                        .rounded(px(2.0))
                        .bg(theme::rgb(theme::colors::INSET))
                        .border_1()
                        .border_color(theme::rgb(theme::colors::GRID_L1))
                        .text_size(px(10.0))
                        .text_color(theme::rgb(theme::colors::INK));
                    if let Some(handler) = self.on_primary {
                        button =
                            button.on_click(move |event, window, cx| handler(event, window, cx));
                    }
                    if let Some(icon) = self.primary_icon {
                        button = button.child(
                            svg()
                                .path(icon)
                                .w(px(12.0))
                                .h(px(12.0))
                                .text_color(theme::rgb(theme::colors::INK)),
                        );
                    }
                    cluster = cluster.child(button.child(label));
                }
                if let Some(icon) = self.icon_action {
                    let (bg, border, fg) = self.icon_tint.unwrap_or((tint_bg, tint_border, tint_fg));
                    let mut button = BaseButton::new(ElementId::Name(
                        format!("{stem}-icon").into(),
                    ))
                        .w(px(28.0))
                        .h(px(28.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(2.0))
                        .bg(theme::rgb(bg))
                        .border_1()
                        .border_color(theme::rgb(border))
                        .text_color(theme::rgb(fg));
                    if let Some(handler) = self.on_icon {
                        button =
                            button.on_click(move |event, window, cx| handler(event, window, cx));
                    }
                    cluster = cluster.child(button.child(
                        svg()
                            .path(icon)
                            .w(px(14.0))
                            .h(px(14.0))
                            .text_color(theme::rgb(fg)),
                    ));
                }
                cluster.into_any_element()
            }
        };

        let mut card = div()
            .w_full()
            .flex()
            .flex_col()
            .gap_2()
            .p_2()
            .bg(theme::rgb(theme::colors::SURFACE_LOWEST))
            .border_1()
            .border_color(theme::rgb(theme::colors::GRID_L1))
            .rounded(px(4.0))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .w_full()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2()
                            .flex_1()
                            .min_w_0()
                            .child(leading)
                            .child(meta_col),
                    )
                    .child(actions),
            );
        if let Some(footer) = self.footer {
            card = card.child(footer);
        }
        card
    }
}
