//! Screens for `Desk`: auth, sidebar + chat/docs/tokens panels.

use gpui::{App, Context, Div, Entity, Render, Window, div, prelude::*, px};
use gpui_component::input::InputState;

use crate::{
    crypto,
    model::{Desk, Panel, Screen},
    ui,
};

fn read(input: &Entity<InputState>, app: &App) -> String {
    input.read(app).value().to_string()
}

impl Desk {
    fn render_auth(&mut self, cx: &mut Context<Self>) -> Div {
        let this = cx.entity();
        let mode = self.register_mode;
        div()
            .flex()
            .items_center()
            .justify_center()
            .size_full()
            .bg(ui::bg_app())
            .child(
                div()
                    .w(px(380.0))
                    .child(
                        ui::card()
                            .child(ui::section_title(if mode {
                                "Create account"
                            } else {
                                "Sign in"
                            }))
                            .child(ui::muted(&format!("server: {}", self.base_url)))
                            .child(ui::muted("username"))
                            .child(ui::field(&self.user_input))
                            .child(ui::muted("password"))
                            .child(ui::field(&self.pass_input))
                            .child(submit_row(this.clone(), mode))
                            .child(ui::muted(&self.status)),
                    ),
            )
    }

    fn render_main(&mut self, cx: &mut Context<Self>) -> Div {
        let panel = match self.panel {
            Panel::Chat => self.render_chat(cx),
            Panel::Docs => self.render_docs(cx),
            Panel::Tokens => self.render_tokens(cx),
        };
        div()
            .flex()
            .flex_row()
            .size_full()
            .bg(ui::bg_app())
            .text_color(ui::text_main())
            .child(self.render_sidebar(cx))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .child(panel)
                    .child(ui::status_bar(&self.status.clone())),
            )
    }

    fn render_sidebar(&mut self, cx: &mut Context<Self>) -> Div {
        let this = cx.entity();
        let groups: Vec<Div> = self
            .groups
            .iter()
            .map(|g| {
                let active = self.active_group.as_deref() == Some(g.object_key.as_str());
                group_button(this.clone(), g.object_key.clone(), g.name.clone(), active)
            })
            .collect();

        let me = self
            .session
            .as_ref()
            .map(|s| s.username.clone())
            .unwrap_or_default();
        div()
            .flex()
            .flex_col()
            .gap_2()
            .w(px(260.0))
            .h_full()
            .bg(ui::bg_panel())
            .border_r_1()
            .border_color(ui::border())
            .p_3()
            .child(ui::section_title("sagex-desk"))
            .child(ui::muted(&format!("signed in as {me}")))
            .child(nav_row(this.clone()))
            .child(ui::muted("groups"))
            .child(div().flex().flex_col().gap_1().id("group-list").overflow_y_scroll().children(groups))
            .child(ui::muted("new group"))
            .child(ui::field(&self.group_name_input))
            .child(ui::field(&self.group_members_input))
            .child({
                let t = this.clone();
                ui::btn("create-group", "Create group").on_click(move |_, _, app| {
                    submit_create_group(t.clone(), app);
                })
            })
            .child({
                let t = this.clone();
                ui::btn_secondary("logout", "Sign out").on_click(move |_, _, app| {
                    t.update(app, |desk, cx| desk.logout(cx));
                })
            })
    }

    fn render_chat(&mut self, cx: &mut Context<Self>) -> Div {
        let this = cx.entity();
        let title = self.active_group_name();
        let bubbles: Vec<Div> = self
            .messages
            .iter()
            .map(|m| {
                let mine = self
                    .session
                    .as_ref()
                    .is_some_and(|s| s.object_key == m.sender_key);
                ui::bubble(mine, &m.sender_key[..m.sender_key.len().min(8)], &crate::crypto::decrypt_incoming(&m.body))
            })
            .collect();
        div()
            .flex()
            .flex_col()
            .flex_1()
            .gap_2()
            .p_4()
            .child(ui::section_title(&format!(
                "# {title}",
            )))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .flex_1()
                    .id("msg-list")
                    .overflow_y_scroll()
                    .children(bubbles),
            )
            .child(ui::field(&self.composer_input))
            .child({
                let t = this.clone();
                ui::btn("send", "Send (raw E2EE placeholder)").on_click(
                    move |_, _, app: &mut gpui::App| {
                        submit_message(t.clone(), app);
                    },
                )
            })
            .child(member_row(this.clone(), &self.add_member_input))
    }

    fn render_docs(&mut self, cx: &mut Context<Self>) -> Div {
        let this = cx.entity();
        div()
            .flex()
            .flex_col()
            .flex_1()
            .gap_2()
            .p_4()
            .child(ui::section_title("Encrypted documents (RustFS)"))
            .child(ui::muted("upload a client-encrypted file; the server only mints the URL"))
            .child(ui::field(&self.file_input))
            .child({
                let t = this.clone();
                ui::btn("upload", "Get URL + upload").on_click(move |_, _, app| {
                    submit_upload(t.clone(), app);
                })
            })
            .child(ui::muted("download by object key"))
            .child(ui::field(&self.dlkey_input))
            .child({
                let t = this.clone();
                ui::btn("dl-url", "Get download URL").on_click(move |_, _, app| {
                    submit_download(t.clone(), app);
                })
            })
            .child(ui::mono_block("dl-url", &self.dl_url_text.clone()))
    }

    fn render_tokens(&mut self, cx: &mut Context<Self>) -> Div {
        let this = cx.entity();
        div()
            .flex()
            .flex_col()
            .flex_1()
            .gap_2()
            .p_4()
            .child(ui::section_title("CA + service tokens"))
            .child({
                let t = this.clone();
                ui::btn("ca-token", "Fetch CA token").on_click(move |_, _, app| {
                    submit_ca_token(t.clone(), app);
                })
            })
            .child(ui::mono_block("ca-token", &self.ca_token_text.clone()))
            .child({
                let t = this.clone();
                ui::btn_secondary("svc-pub", "Fetch service pubkey").on_click(
                    move |_, _, app| {
                        submit_pubkey(t.clone(), app);
                    },
                )
            })
            .child(ui::mono_block("svc-pub", &self.pubkey_text.clone()))
    }
}

impl Render for Desk {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl gpui::IntoElement {
        match self.screen {
            Screen::Auth => self.render_auth(cx),
            Screen::Main => self.render_main(cx),
        }
    }
}

// ---------- submit helpers (thin: read inputs, delegate to Desk) ----------

/// Read an input entity's text via a field selector.
fn input_text(
    this: &Entity<Desk>,
    app: &App,
    f: impl Fn(&Desk) -> Entity<InputState>,
) -> String {
    let input = f(&this.read(app));
    read(&input, app)
}

fn submit_row(this: Entity<Desk>, register_mode: bool) -> Div {
    let t_go = this.clone();
    let t_toggle = this.clone();
    div()
        .flex()
        .flex_row()
        .gap_2()
        .child(
            ui::btn("auth-go", if register_mode { "Create" } else { "Sign in" }).on_click(
                move |_, _, app: &mut gpui::App| {
                    let user = input_text(&t_go, app, |d| d.user_input.clone());
                    let pass = input_text(&t_go, app, |d| d.pass_input.clone());
                    t_go.update(app, |desk, cx| {
                        desk.start_login(user, pass, register_mode, cx)
                    });
                },
            ),
        )
        .child(
            ui::btn_secondary("auth-toggle", if register_mode {
                "have an account?"
            } else {
                "new here?"
            })
            .on_click(move |_, _, app| {
                t_toggle.update(app, |desk, cx| {
                    desk.register_mode = !desk.register_mode;
                    cx.notify();
                });
            }),
        )
}

fn nav_row(this: Entity<Desk>) -> Div {
    let t_docs = this.clone();
    let t_tokens = this.clone();
    div().flex().flex_row().gap_2().child(
        ui::btn_secondary("nav-chat", "Chat").on_click(move |_, _, app| {
            this.update(app, |desk, cx| desk.switch_panel(Panel::Chat, cx));
        }),
    ).child({
        ui::btn_secondary("nav-docs", "Docs").on_click(move |_, _, app| {
            t_docs.update(app, |desk, cx| desk.switch_panel(Panel::Docs, cx));
        })
    }).child({
        ui::btn_secondary("nav-tokens", "Tokens").on_click(move |_, _, app| {
            t_tokens.update(app, |desk, cx| desk.switch_panel(Panel::Tokens, cx));
        })
    })
}

fn submit_create_group(this: Entity<Desk>, app: &mut gpui::App) {
    let name = input_text(&this, app, |d| d.group_name_input.clone());
    let members_raw = input_text(&this, app, |d| d.group_members_input.clone());
    if name.trim().is_empty() {
        return;
    }
    let members: Vec<String> = members_raw
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    this.update(app, |desk, cx| {
        desk.start_create_group(name, members, cx)
    });
}

fn group_button(this: Entity<Desk>, id: String, name: String, active: bool) -> Div {
    let label = format!("{}{name}", if active { "> " } else { "" });
    let key: &'static str = Box::leak(format!("grp-{id}").into_boxed_str());
    div().w_full().child(
        ui::btn(key, &label).on_click(move |_, _, app| {
            select_group(this.clone(), id.clone(), app);
        }),
    )
}

fn select_group(this: Entity<Desk>, id: String, app: &mut gpui::App) {
    this.update(app, |desk, cx| {
        desk.active_group = Some(id);
        desk.panel = Panel::Chat;
        desk.persist();
        desk.refresh_messages(cx);
    });
}

fn member_row(this: Entity<Desk>, add_input: &Entity<InputState>) -> Div {
    div()
        .flex()
        .flex_row()
        .gap_2()
        .child(div().flex_1().child(ui::field(add_input)))
        .child({
            let t = this.clone();
            ui::btn_secondary("add-member", "Add").on_click(move |_, _, app| {
                submit_add_member(t.clone(), app);
            })
        })
        .child({
            let t = this.clone();
            ui::btn_secondary("leave-group", "Leave").on_click(move |_, _, app| {
                submit_leave(t.clone(), app);
            })
        })
}

fn submit_add_member(this: Entity<Desk>, app: &mut gpui::App) {
    let username = input_text(&this, app, |d| d.add_member_input.clone());
    if username.trim().is_empty() {
        return;
    }
    this.update(app, |desk, cx| desk.start_add_member(username, cx));
}

fn submit_leave(this: Entity<Desk>, app: &mut gpui::App) {
    this.update(app, |desk, cx| desk.start_leave(cx));
}

fn submit_message(this: Entity<Desk>, app: &mut gpui::App) {
    let raw = input_text(&this, app, |d| d.composer_input.clone());
    if raw.trim().is_empty() {
        return;
    }
    let body = crypto::encrypt_outgoing(&raw);
    this.update(app, |desk, cx| desk.start_send(body, cx));
}

fn submit_upload(this: Entity<Desk>, app: &mut gpui::App) {
    let path = input_text(&this, app, |d| d.file_input.clone());
    if path.trim().is_empty() {
        return;
    }
    this.update(app, |desk, cx| desk.start_upload(path, cx));
}

fn submit_download(this: Entity<Desk>, app: &mut gpui::App) {
    let key = input_text(&this, app, |d| d.dlkey_input.clone());
    if key.trim().is_empty() {
        return;
    }
    this.update(app, |desk, cx| desk.start_download(key, cx));
}

fn submit_ca_token(this: Entity<Desk>, app: &mut gpui::App) {
    this.update(app, |desk, cx| desk.start_ca_token(cx));
}

fn submit_pubkey(this: Entity<Desk>, app: &mut gpui::App) {
    this.update(app, |desk, cx| desk.start_pubkey(cx));
}

