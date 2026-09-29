//! GPUI front-end for `office-watermark`: full parity with the
//! `office-watermark-demo` CLI (encode / decode / verify / roundtrip).
//!
//! Heavy file work runs on the background executor; the UI thread only
//! renders state. File picking uses the native dialogs.

use std::path::PathBuf;

use gpui::{
    Context, Entity, InteractiveElement, IntoElement, ParentElement, Render,
    ScrollHandle, StatefulInteractiveElement, Styled, Window, div, px,
};
use gpui_component::{
    ActiveTheme, Sizable,
    button::{Button, ButtonVariants},
    input::{Input, InputState},
};

use crate::ops::{self, DecodeResult, EncodeResult, RoundtripResult, VerifyResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Encode,
    Decode,
    Verify,
    Roundtrip,
}

impl Tab {
    fn label(self) -> &'static str {
        match self {
            Tab::Encode => "Encode",
            Tab::Decode => "Decode",
            Tab::Verify => "Verify",
            Tab::Roundtrip => "Roundtrip",
        }
    }
}

pub struct WatermarkApp {
    tab: Tab,
    // encode
    enc_input: Option<PathBuf>,
    enc_output: Option<PathBuf>,
    enc_busy: bool,
    enc_result: Option<EncodeResult>,
    // decode
    dec_file: Option<PathBuf>,
    dec_result: Option<DecodeResult>,
    dec_busy: bool,
    // verify
    ver_file: Option<PathBuf>,
    ver_result: Option<VerifyResult>,
    ver_busy: bool,
    // roundtrip
    rt_format: String,
    rt_busy: bool,
    rt_result: Option<RoundtripResult>,
    // shared
    error: Option<String>,
    pub text_input: Entity<InputState>,
    pub output_input: Entity<InputState>,
    pub rt_text_input: Entity<InputState>,
    list_scroll: ScrollHandle,
}

impl WatermarkApp {
    pub fn new(
        text_input: Entity<InputState>,
        output_input: Entity<InputState>,
        rt_text_input: Entity<InputState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        text_input.update(cx, |s, cx| {
            s.set_placeholder("watermark text, e.g. enc-001", window, cx);
        });
        rt_text_input.update(cx, |s, cx| {
            s.set_value("enc-demo-001".to_string(), window, cx);
            s.set_placeholder("watermark text", window, cx);
        });
        output_input.update(cx, |s, cx| {
            s.set_placeholder("output path (auto-suggested on encode pick)", window, cx);
        });
        Self {
            tab: Tab::Encode,
            enc_input: None,
            enc_output: None,
            enc_busy: false,
            enc_result: None,
            dec_file: None,
            dec_result: None,
            dec_busy: false,
            ver_file: None,
            ver_result: None,
            ver_busy: false,
            rt_format: "docx".to_string(),
            rt_busy: false,
            rt_result: None,
            error: None,
            text_input,
            output_input,
            rt_text_input,
            list_scroll: ScrollHandle::new(),
        }
    }

    fn set_error(&mut self, cx: &mut Context<Self>, msg: String) {
        self.error = Some(msg);
        cx.notify();
    }

    fn clear_error(&mut self) {
        self.error = None;
    }

    // ---------- file picking ----------

    fn pick_single_file(&mut self, title: &'static str, cx: &mut Context<Self>, apply: impl FnOnce(&mut Self, PathBuf, &mut Context<Self>) + 'static) {
        let rx = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(title.into()),
        });
        cx.spawn(async move |view, cx| {
            let out = cx.background_executor().spawn(rx).await;
            match out {
                Ok(Ok(Some(mut paths))) if !paths.is_empty() => {
                    let path = paths.remove(0);
                    let _ = view.update(cx, |v: &mut WatermarkApp, cx| {
                        v.clear_error();
                        apply(v, path, cx);
                        cx.notify();
                    });
                }
                _ => {}
            }
        })
        .detach();
    }

    fn pick_encode_input(&mut self, cx: &mut Context<Self>) {
        self.pick_single_file("Pick Office file to watermark", cx, |v, path, _| {
            v.enc_input = Some(path.clone());
            v.enc_output = Some(ops::default_output(&path));
            v.enc_result = None;
        });
    }

    fn pick_decode_file(&mut self, cx: &mut Context<Self>) {
        self.pick_single_file("Pick watermarked file to inspect", cx, |v, path, _| {
            v.dec_file = Some(path);
            v.dec_result = None;
        });
    }

    fn pick_verify_file(&mut self, cx: &mut Context<Self>) {
        self.pick_single_file("Pick watermarked file to verify", cx, |v, path, _| {
            v.ver_file = Some(path);
            v.ver_result = None;
        });
    }

    // ---------- operations (background) ----------

    fn run_encode(&mut self, cx: &mut Context<Self>) {
        let (Some(input), Some(output)) = (self.enc_input.clone(), self.enc_output.clone()) else {
            self.set_error(cx, "pick an input file first".to_string());
            return;
        };
        let text = self.text_input.read(cx).value().trim().to_string();
        if let Err(e) = ops::validate_text(&text) {
            self.set_error(cx, e);
            return;
        }
        // Output path may have been edited in the text field.
        let edited = self.output_input.read(cx).value().trim().to_string();
        let output = if edited.is_empty() { output } else { PathBuf::from(edited) };
        self.enc_busy = true;
        self.clear_error();
        self.enc_result = None;
        cx.notify();
        cx.spawn(async move |view, cx| {
            let res = cx
                .background_executor()
                .spawn(async move { ops::op_encode(&input, &output, &text) })
                .await;
            let _ = view.update(cx, |v: &mut WatermarkApp, cx| {
                v.enc_busy = false;
                match res {
                    Ok(r) => v.enc_result = Some(r),
                    Err(e) => v.error = Some(e),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn run_decode(&mut self, cx: &mut Context<Self>) {
        let Some(file) = self.dec_file.clone() else {
            self.set_error(cx, "pick a file first".to_string());
            return;
        };
        self.dec_busy = true;
        self.clear_error();
        self.dec_result = None;
        cx.notify();
        cx.spawn(async move |view, cx| {
            let res = cx
                .background_executor()
                .spawn(async move { ops::op_decode(&file) })
                .await;
            let _ = view.update(cx, |v: &mut WatermarkApp, cx| {
                v.dec_busy = false;
                match res {
                    Ok(r) => v.dec_result = Some(r),
                    Err(e) => v.error = Some(e),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn run_verify(&mut self, cx: &mut Context<Self>) {
        let Some(file) = self.ver_file.clone() else {
            self.set_error(cx, "pick a file first".to_string());
            return;
        };
        self.ver_busy = true;
        self.clear_error();
        self.ver_result = None;
        cx.notify();
        cx.spawn(async move |view, cx| {
            let res = cx
                .background_executor()
                .spawn(async move { ops::op_verify(&file) })
                .await;
            let _ = view.update(cx, |v: &mut WatermarkApp, cx| {
                v.ver_busy = false;
                match res {
                    Ok(r) => v.ver_result = Some(r),
                    Err(e) => v.error = Some(e),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn run_roundtrip(&mut self, cx: &mut Context<Self>) {
        let text = self.rt_text_input.read(cx).value().trim().to_string();
        if let Err(e) = ops::validate_text(&text) {
            self.set_error(cx, e);
            return;
        }
        let format = self.rt_format.clone();
        self.rt_busy = true;
        self.clear_error();
        self.rt_result = None;
        cx.notify();
        cx.spawn(async move |view, cx| {
            let res = cx
                .background_executor()
                .spawn(async move { ops::op_roundtrip(&format, &text) })
                .await;
            let _ = view.update(cx, |v: &mut WatermarkApp, cx| {
                v.rt_busy = false;
                match res {
                    Ok(r) => v.rt_result = Some(r),
                    Err(e) => v.error = Some(e),
                }
                cx.notify();
            });
        })
        .detach();
    }

    // ---------- render helpers ----------

    fn tab_button(&self, cx: &mut Context<Self>, tab: Tab) -> impl IntoElement {
        let active = self.tab == tab;
        let id = match tab {
            Tab::Encode => "tab-encode",
            Tab::Decode => "tab-decode",
            Tab::Verify => "tab-verify",
            Tab::Roundtrip => "tab-roundtrip",
        };
        let mut b = Button::new(id)
            .small()
            .label(tab.label().to_string())
            .flex_shrink_0();
        b = if active { b.primary() } else { b.outline() };
        b.on_click(cx.listener(move |this, _, _, cx| {
            this.tab = tab;
            this.clear_error();
            cx.notify();
        }))
    }

    fn path_line(&self, cx: &mut Context<Self>, label: &str, path: &Option<PathBuf>) -> impl IntoElement {
        let theme = cx.theme().clone();
        div()
            .flex()
            .flex_row()
            .gap_2()
            .text_xs()
            .child(
                div()
                    .w(px(64.))
                    .flex_shrink_0()
                    .text_color(theme.muted_foreground)
                    .child(label.to_string()),
            )
            .child(
                div()
                    .flex_1()
                    .truncate()
                    .text_color(theme.foreground)
                    .child(
                        path.as_ref()
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_else(|| "—".to_string()),
                    ),
            )
    }

    fn status_line(&self, cx: &mut Context<Self>, busy: bool, busy_text: &str) -> impl IntoElement {
        let theme = cx.theme().clone();
        div()
            .flex()
            .flex_col()
            .gap_1()
            .px_3()
            .py_1()
            .child(if busy {
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(busy_text.to_string())
            } else {
                div()
            })
            .child(match &self.error {
                Some(e) => div().text_xs().text_color(theme.danger).child(e.clone()),
                None => div(),
            })
    }

    fn hit_rows(&self, cx: &mut Context<Self>, hits: &[office_watermark::WatermarkHit], reference: Option<&str>) -> Vec<gpui::AnyElement> {
        let theme = cx.theme().clone();
        let mut rows: Vec<gpui::AnyElement> = Vec::new();
        for h in hits {
            let mark = match reference {
                Some(r) if h.value != r => ("DIFF", theme.danger),
                Some(_) => ("OK", theme.success),
                None => ("•", theme.muted_foreground),
            };
            rows.push(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .px_3()
                    .py_1()
                    .text_xs()
                    .child(div().w(px(44.)).flex_shrink_0().text_color(mark.1).child(mark.0.to_string()))
                    .child(
                        div()
                            .flex_1()
                            .truncate()
                            .text_color(theme.foreground)
                            .child(format!("{} => {}", h.entry, h.value)),
                    )
                    .into_any_element(),
            );
        }
        rows
    }

    fn render_encode(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let mut rows: Vec<gpui::AnyElement> = Vec::new();
        if let Some(r) = &self.enc_result {
            rows.push(
                div()
                    .px_3()
                    .py_1()
                    .text_sm()
                    .text_color(theme.success)
                    .child(format!(
                        "encoded {}: {} xml parts watermarked",
                        r.format, r.watermarked_parts
                    ))
                    .into_any_element(),
            );
        }
        let busy = self.enc_busy;
        div()
            .flex()
            .flex_col()
            .flex_1()
            .overflow_hidden()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .px_3()
                    .py_2()
                    .child(self.path_line(cx, "input", &self.enc_input))
                    .child(self.path_line(cx, "output", &self.enc_output))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2()
                            .child(div().w(px(300.)).child(Input::new(&self.text_input).cleanable(true)))
                            .child(
                                Button::new("enc-browse")
                                    .small()
                                    .label("Browse input…".to_string())
                                    .on_click(cx.listener(|this, _, _, cx| this.pick_encode_input(cx))),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .flex_1()
                                    .child(Input::new(&self.output_input).cleanable(true)),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child("override output (blank = suggestion)".to_string()),
                            )
                            .child(
                                Button::new("enc-run")
                                    .small()
                                    .label(if busy { "Working…".to_string() } else { "Encode".to_string() })
                                    .on_click(cx.listener(|this, _, _, cx| this.run_encode(cx))),
                            ),
                    ),
            )
            .child(self.status_line(cx, busy, "encoding…"))
            .child(
                div()
                    .id("enc-scroll")
                    .flex_1()
                    .flex()
                    .flex_col()
                    .overflow_y_scroll()
                    .track_scroll(&self.list_scroll)
                    .children(rows),
            )
    }

    fn render_decode(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let mut rows: Vec<gpui::AnyElement> = Vec::new();
        if let Some(r) = &self.dec_result {
            let verdict = match &r.common {
                Some(wm) => format!("watermark: {wm:?}"),
                None => format!("multiple distinct watermarks: {:?}", r.distinct),
            };
            let color = if r.common.is_some() { theme.success } else { theme.warning };
            rows.push(
                div().px_3().py_1().text_sm().text_color(color).child(verdict).into_any_element(),
            );
            rows.extend(self.hit_rows(cx, &r.hits, None));
        }
        let busy = self.dec_busy;
        let file_txt = self
            .dec_file
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "—".to_string());
        div()
            .flex()
            .flex_col()
            .flex_1()
            .overflow_hidden()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .px_3()
                    .py_2()
                    .child(
                        div()
                            .flex_1()
                            .truncate()
                            .text_xs()
                            .text_color(theme.foreground)
                            .child(file_txt),
                    )
                    .child(
                        Button::new("dec-browse")
                            .small()
                            .label("Browse…".to_string())
                            .on_click(cx.listener(|this, _, _, cx| this.pick_decode_file(cx))),
                    )
                    .child(
                        Button::new("dec-run")
                            .small()
                            .label(if busy { "Working…".to_string() } else { "Decode".to_string() })
                            .on_click(cx.listener(|this, _, _, cx| this.run_decode(cx))),
                    ),
            )
            .child(self.status_line(cx, busy, "decoding…"))
            .child(
                div()
                    .id("dec-scroll")
                    .flex_1()
                    .flex()
                    .flex_col()
                    .overflow_y_scroll()
                    .track_scroll(&self.list_scroll)
                    .children(rows),
            )
    }

    fn render_verify(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let mut rows: Vec<gpui::AnyElement> = Vec::new();
        if let Some(r) = &self.ver_result {
            let summary = format!(
                "verify: {}/{} xml parts match (missing wm: {})",
                r.matched, r.total_xml, r.missing
            );
            let color = if r.ok { theme.success } else { theme.danger };
            rows.push(
                div().px_3().py_1().text_sm().text_color(color).child(summary).into_any_element(),
            );
            if r.ok {
                rows.push(
                    div()
                        .px_3()
                        .py_1()
                        .text_sm()
                        .text_color(theme.success)
                        .child(format!("watermark: {:?}", r.reference))
                        .into_any_element(),
                );
            }
            rows.extend(self.hit_rows(cx, &r.hits, Some(&r.reference)));
        }
        let busy = self.ver_busy;
        let file_txt = self
            .ver_file
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "—".to_string());
        div()
            .flex()
            .flex_col()
            .flex_1()
            .overflow_hidden()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .px_3()
                    .py_2()
                    .child(
                        div()
                            .flex_1()
                            .truncate()
                            .text_xs()
                            .text_color(theme.foreground)
                            .child(file_txt),
                    )
                    .child(
                        Button::new("ver-browse")
                            .small()
                            .label("Browse…".to_string())
                            .on_click(cx.listener(|this, _, _, cx| this.pick_verify_file(cx))),
                    )
                    .child(
                        Button::new("ver-run")
                            .small()
                            .label(if busy { "Working…".to_string() } else { "Verify".to_string() })
                            .on_click(cx.listener(|this, _, _, cx| this.run_verify(cx))),
                    ),
            )
            .child(self.status_line(cx, busy, "verifying…"))
            .child(
                div()
                    .id("ver-scroll")
                    .flex_1()
                    .flex()
                    .flex_col()
                    .overflow_y_scroll()
                    .track_scroll(&self.list_scroll)
                    .children(rows),
            )
    }

    fn render_roundtrip(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let mut rows: Vec<gpui::AnyElement> = Vec::new();
        if let Some(r) = &self.rt_result {
            let color = if r.ok { theme.success } else { theme.danger };
            rows.push(
                div()
                    .px_3()
                    .py_1()
                    .text_sm()
                    .text_color(color)
                    .child(format!(
                        "sample format: {}, watermark: {:?} — roundtrip {} ({} parts)",
                        r.format,
                        r.text,
                        if r.ok { "OK" } else { "FAIL" },
                        r.hits.len()
                    ))
                    .into_any_element(),
            );
            rows.extend(self.hit_rows(cx, &r.hits, Some(&r.text)));
        }
        let busy = self.rt_busy;
        div()
            .flex()
            .flex_col()
            .flex_1()
            .overflow_hidden()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .px_3()
                    .py_2()
                    .child(
                        rt_format_button(cx, "rt-docx", "docx", &self.rt_format),
                    )
                    .child(
                        rt_format_button(cx, "rt-ods", "ods", &self.rt_format),
                    )
                    .child(div().w(px(260.)).child(Input::new(&self.rt_text_input).cleanable(true)))
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(format!("sample: {}", self.rt_format)),
                    )
                    .child(
                        Button::new("rt-run")
                            .small()
                            .label(if busy { "Working…".to_string() } else { "Run".to_string() })
                            .on_click(cx.listener(|this, _, _, cx| this.run_roundtrip(cx))),
                    ),
            )
            .child(self.status_line(cx, busy, "running roundtrip…"))
            .child(
                div()
                    .id("rt-scroll")
                    .flex_1()
                    .flex()
                    .flex_col()
                    .overflow_y_scroll()
                    .track_scroll(&self.list_scroll)
                    .children(rows),
            )
    }

    fn render_sidebar(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        div()
            .w(px(224.))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .gap_2()
            .px_3()
            .py_3()
            .border_r_1()
            .border_color(theme.border)
            .bg(theme.sidebar)
            .child(
                div()
                    .text_sm()
                    .text_color(theme.foreground)
                    .child("Office Watermark".to_string()),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child("sagex:wm encode / decode".to_string()),
            )
            .child(self.tab_button(cx, Tab::Encode))
            .child(self.tab_button(cx, Tab::Decode))
            .child(self.tab_button(cx, Tab::Verify))
            .child(self.tab_button(cx, Tab::Roundtrip))
    }
}

/// Roundtrip format toggle button (primary when active).
fn rt_format_button(
    cx: &mut Context<WatermarkApp>,
    id: &'static str,
    val: &'static str,
    current: &str,
) -> gpui::AnyElement {
    let mut b = Button::new(id).small().label(val.to_string());
    b = if current == val { b.primary() } else { b.outline() };
    b.on_click(cx.listener(move |this, _, _, cx| {
        this.rt_format = val.to_string();
        cx.notify();
    }))
    .into_any_element()
}

impl Render for WatermarkApp {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let body = match self.tab {
            Tab::Encode => self.render_encode(cx).into_any_element(),
            Tab::Decode => self.render_decode(cx).into_any_element(),
            Tab::Verify => self.render_verify(cx).into_any_element(),
            Tab::Roundtrip => self.render_roundtrip(cx).into_any_element(),
        };
        div()
            .size_full()
            .flex()
            .flex_row()
            .bg(theme.background)
            .text_color(theme.foreground)
            .child(self.render_sidebar(cx))
            .child(div().flex_1().flex().flex_col().overflow_hidden().child(body))
    }
}
