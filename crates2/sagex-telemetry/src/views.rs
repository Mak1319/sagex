//! Screens for `Telemetry`: login (key generation + sign in),
//! main (search, record list, detail, verify). Lookup only — no register UI.

use gpui::{App, Context, Div, Entity, Render, Window, div, prelude::*};
use gpui_component::input::InputState;

use crate::{
    model::{Screen, Telemetry},
    ui,
};

fn read(input: &Entity<InputState>, app: &App) -> String {
    input.read(app).value().to_string()
}

fn input_text(
    this: &Entity<Telemetry>,
    app: &App,
    pick: impl Fn(&Telemetry) -> Entity<InputState>,
) -> String {
    let input = this.read(app).let_clone(&pick);
    read(&input, app)
}

trait LetClone {
    fn let_clone(&self, f: impl Fn(&Self) -> Entity<InputState>) -> Entity<InputState>;
}
impl LetClone for Telemetry {
    fn let_clone(&self, f: impl Fn(&Self) -> Entity<InputState>) -> Entity<InputState> {
        f(self)
    }
}

impl Render for Telemetry {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl gpui::IntoElement {
        match self.screen {
            Screen::Login => self.render_login(cx),
            Screen::Main => self.render_main(cx),
        }
    }
}

impl Telemetry {
    fn render_login(&mut self, cx: &mut Context<Self>) -> Div {
        let this = cx.entity();
        div()
            .flex()
            .items_center()
            .justify_center()
            .size_full()
            .bg(ui::bg_app())
            .text_color(ui::text_main())
            .child(
                div().w(gpui::px(440.0)).child(
                    ui::card()
                        .child(ui::section_title("sagex-telemetry"))
                        .child(ui::muted(&format!("gateway: {}", self.gateway_url)))
                        .child(ui::muted("username"))
                        .child(ui::field(&self.user_input))
                        .child(login_row(this.clone()))
                        .child(ui::muted(&self.status))
                        .child(fresh_key_block(&self.fresh_key)),
                ),
            )
    }

    fn render_main(&mut self, cx: &mut Context<Self>) -> Div {
        let this = cx.entity();
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(ui::bg_app())
            .text_color(ui::text_main())
            .child(header_row(this.clone()))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .child(self.render_list(cx))
                    .child(self.render_detail(cx)),
            )
            .child(ui::status_bar(&self.status.clone()))
    }

    fn render_list(&mut self, cx: &mut Context<Self>) -> Div {
        let this = cx.entity();
        // Snapshot of the current filter applied at event time.
        let snapshot: Vec<(String, String, bool)> = self
            .filtered(&self.filter.clone())
            .iter()
            .map(|b| {
                (
                    b.record.watermark_id.clone(),
                    b.record.recipient_id.clone(),
                    self.active_watermark.as_deref() == Some(b.record.watermark_id.as_str()),
                )
            })
            .collect();
        let rows: Vec<Div> = snapshot
            .into_iter()
            .map(|(wm, recip, active)| record_button(this.clone(), wm, recip, active))
            .collect();
        div()
            .flex()
            .flex_col()
            .gap_2()
            .w(gpui::px(320.0))
            .h_full()
            .bg(ui::bg_panel())
            .border_r_1()
            .border_color(ui::border())
            .p_3()
            .child(ui::section_title("records"))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .child(div().flex_1().child(ui::field(&self.search_input)))
                    .child({
                        let t = this.clone();
                        ui::btn_secondary("refresh", "Refresh").on_click(move |_, _, app| {
                            t.update(app, |m, cx| m.refresh_records(cx));
                        })
                    }),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .id("record-list")
                    .overflow_y_scroll()
                    .children(rows),
            )
    }

    fn render_detail(&mut self, cx: &mut Context<Self>) -> Div {
        let this = cx.entity();
        let body: Div = match self.active_block() {
            Some(b) => {
                let r = &b.record;
                let text = format!(
                    "watermark_id: {}\nrecipient_id: {}\ndoc_hash: {}\nsession_id: {}\ntimestamp_ms: {}\nrecipient_dsa_pub: {}\nsignature: {}\nheight: {}\nprev_hash: {}\nblock_hash: {}",
                    r.watermark_id,
                    r.recipient_id,
                    r.doc_hash,
                    r.session_id,
                    r.timestamp_ms,
                    r.recipient_dsa_pub,
                    r.signature,
                    b.height,
                    b.prev_hash,
                    b.block_hash,
                );
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(ui::section_title("record detail"))
                    .child(ui::mono_block("record-detail", &text))
                    .child({
                        let t = this.clone();
                        ui::btn("verify", "Verify (chain + ML-DSA)").on_click(
                            move |_, _, app| {
                                t.update(app, |m, cx| m.start_verify(cx));
                            },
                        )
                    })
                    .child(ui::mono_block("verdict", &self.verdict))
            }
            None => div().child(ui::muted("select a record, or search for one")),
        };
        div().flex().flex_col().flex_1().gap_2().p_4().child(body)
    }
}

fn login_row(this: Entity<Telemetry>) -> Div {
    div()
        .flex()
        .flex_row()
        .gap_2()
        .child({
            let t = this.clone();
            ui::btn("gen-key", "Generate login key").on_click(move |_, _, app| {
                let u = input_text(&t, app, |d| d.user_input.clone());
                t.update(app, |m, cx| m.start_generate_key(u, cx));
            })
        })
        .child({
            let t = this.clone();
            ui::btn_secondary("sign-in", "Sign in").on_click(move |_, _, app| {
                let u = input_text(&t, app, |d| d.user_input.clone());
                t.update(app, |m, cx| m.start_login(u, cx));
            })
        })
}

fn fresh_key_block(fresh: &str) -> Div {
    if fresh.is_empty() {
        return div();
    }
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(ui::muted(
            "new login key (copy to the gateway operator NOW — shown once):",
        ))
        .child(ui::mono_block("fresh-key", fresh))
}

fn header_row(this: Entity<Telemetry>) -> Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap_3()
        .w_full()
        .bg(ui::bg_panel())
        .border_b_1()
        .border_color(ui::border())
        .px_4()
        .py_2()
        .child(ui::section_title("sagex-telemetry"))
        .child({
            let t = this.clone();
            ui::btn_secondary("lookup", "Lookup exact").on_click(move |_, _, app| {
                let q = input_text(&t, app, |d| d.search_input.clone());
                t.update(app, |m, cx| m.lookup_exact(q, cx));
            })
        })
        .child({
            let t = this.clone();
            ui::btn_secondary("filter", "Apply filter").on_click(move |_, _, app| {
                let q = input_text(&t, app, |d| d.search_input.clone());
                t.update(app, |m, cx| {
                    m.filter = q;
                    m.set_status("filter applied", cx);
                });
            })
        })
}

fn record_button(this: Entity<Telemetry>, wm: String, recip: String, active: bool) -> Div {
    let label = format!("{}{recip} / {}", if active { "> " } else { "" }, short(&wm));
    let key: &'static str = Box::leak(format!("rec-{wm}").into_boxed_str());
    let id = wm.clone();
    div().w_full().child(
        ui::btn(key, &label).on_click(move |_, _, app| {
            let id = id.clone();
            this.update(app, |m, cx| {
                m.active_watermark = Some(id);
                m.verdict.clear();
                cx.notify();
            });
        }),
    )
}

fn short(wm: &str) -> String {
    if wm.len() > 18 {
        format!("{}…{}", &wm[..10], &wm[wm.len() - 6..])
    } else {
        wm.to_string()
    }
}
