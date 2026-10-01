//! Read-only ledger explorer: records, search, server logs, node status.
//!
//! Data comes **only** from the register gateway over HTTP. Record reads,
//! status and outbox are open endpoints; `GET /logs` additionally needs the
//! CA-issued auditor permit (Bearer), configured on the Logs tab.

use gpui::{
    Context, Entity, InteractiveElement, IntoElement, ParentElement, Render,
    ScrollHandle, StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
};
use gpui_component::{
    ActiveTheme, Sizable,
    button::{Button, ButtonVariants},
    input::{Input, InputState},
};

use sagex_ledger::model::Block;

use crate::backend::{GatewayClient, LogEntryView, NodeLogsView, StatusBody};
use crate::net::request;

const PAGE_SIZE: u64 = 15;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Records,
    Search,
    Logs,
    Status,
}

impl Tab {
    fn label(self) -> &'static str {
        match self {
            Tab::Records => "Records",
            Tab::Search => "Search",
            Tab::Logs => "Logs",
            Tab::Status => "Status",
        }
    }
}

pub struct Explorer {
    client: GatewayClient,
    base_url: String,
    tab: Tab,

    // connection
    connected: Option<bool>,
    conn_error: Option<String>,

    // records (newest-first page cache)
    height: Option<u64>,
    page_top: Option<u64>, // highest index on the current page
    blocks: Vec<Block>,
    selected: Option<u64>,
    records_loading: bool,
    records_error: Option<String>,

    // search
    search_results: Vec<Block>,
    search_loading: bool,
    search_error: Option<String>,

    // logs
    gw_logs: Vec<LogEntryView>,
    node_logs: Vec<NodeLogsView>,
    log_level: String,
    log_source: String,
    logs_loading: bool,
    logs_error: Option<String>,

    // status
    status: Option<StatusBody>,
    outbox: Option<serde_json::Value>,
    status_loading: bool,
    status_error: Option<String>,

    // inputs (created up-front: they need a Window)
    pub url_input: Entity<InputState>,
    pub permit_input: Entity<InputState>,
    pub wm_input: Entity<InputState>,
    pub user_input: Entity<InputState>,

    records_scroll: ScrollHandle,
    logs_scroll: ScrollHandle,
}

impl Explorer {
    pub fn new(
        client: GatewayClient,
        base_url: String,
        url_input: Entity<InputState>,
        permit_input: Entity<InputState>,
        wm_input: Entity<InputState>,
        user_input: Entity<InputState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        url_input.update(cx, |s, cx| {
            s.set_value(base_url.clone(), window, cx);
        });
        Self {
            client,
            base_url,
            tab: Tab::Records,
            connected: None,
            conn_error: None,
            height: None,
            page_top: None,
            blocks: vec![],
            selected: None,
            records_loading: false,
            records_error: None,
            search_results: vec![],
            search_loading: false,
            search_error: None,
            gw_logs: vec![],
            node_logs: vec![],
            log_level: String::new(),
            log_source: String::new(),
            logs_loading: false,
            logs_error: None,
            status: None,
            outbox: None,
            status_loading: false,
            status_error: None,
            url_input,
            permit_input,
            wm_input,
            user_input,
            records_scroll: ScrollHandle::new(),
            logs_scroll: ScrollHandle::new(),
        }
    }

    /// First paint: health check, then load the newest records page.
    pub fn boot(&mut self, cx: &mut Context<Self>) {
        let client = self.client.clone();
        request(cx, async move { client.health().await }, |this, res, cx| {
            match res {
                Ok(()) => {
                    this.connected = Some(true);
                    this.conn_error = None;
                    this.refresh_records(cx);
                }
                Err(e) => {
                    this.connected = Some(false);
                    this.conn_error = Some(e.to_string());
                    cx.notify();
                }
            }
        })
        .detach();
    }

    // ---------- records ----------

    fn refresh_records(&mut self, cx: &mut Context<Self>) {
        self.records_loading = true;
        self.records_error = None;
        cx.notify();
        let client = self.client.clone();
        request(
            cx,
            async move {
                let st = client.status().await?;
                Ok::<_, anyhow::Error>(st)
            },
            |this, res: anyhow::Result<StatusBody>, cx| {
                match res {
                    Ok(st) => {
                        let height = this.height_of(&st);
                        this.height = Some(height);
                        this.connected = Some(true);
                        this.conn_error = None;
                        if height == 0 {
                            this.blocks.clear();
                            this.page_top = None;
                            this.records_loading = false;
                            cx.notify();
                        } else {
                            this.load_page(height - 1, cx);
                        }
                    }
                    Err(e) => {
                        this.records_loading = false;
                        this.records_error = Some(e.to_string());
                        this.connected = Some(false);
                        cx.notify();
                    }
                }
            },
        )
        .detach();
    }

    fn height_of(&self, st: &StatusBody) -> u64 {
        st.ledger
            .iter()
            .filter_map(|n| n.get("height")?.as_u64())
            .max()
            .unwrap_or(0)
    }

    fn load_page(&mut self, top: u64, cx: &mut Context<Self>) {
        self.records_loading = true;
        self.records_error = None;
        self.page_top = Some(top);
        cx.notify();
        let client = self.client.clone();
        let lo = top.saturating_sub(PAGE_SIZE - 1);
        request(
            cx,
            async move {
                let mut out = vec![];
                for idx in (lo..=top).rev() {
                    match client.block(idx).await {
                        Ok(mut b) => out.append(&mut b),
                        Err(e) => return Err::<_, anyhow::Error>(e),
                    }
                }
                Ok::<_, anyhow::Error>(out)
            },
            |this, res: anyhow::Result<Vec<Block>>, cx| {
                this.records_loading = false;
                match res {
                    Ok(blocks) => {
                        this.blocks = blocks;
                        if this
                            .selected
                            .is_some_and(|s| !this.blocks.iter().any(|b| b.header.index == s))
                        {
                            this.selected = None;
                        }
                    }
                    Err(e) => this.records_error = Some(e.to_string()),
                }
                cx.notify();
            },
        )
        .detach();
    }

    fn page_prev(&mut self, cx: &mut Context<Self>) {
        // Older page (lower indices).
        if let Some(top) = self.page_top.filter(|t| *t >= PAGE_SIZE) {
            self.selected = None;
            self.load_page(top - PAGE_SIZE, cx);
        }
    }

    fn page_next(&mut self, cx: &mut Context<Self>) {
        // Newer page (higher indices), clamped to the chain tip.
        if let (Some(top), Some(h)) = (self.page_top, self.height) {
            if h == 0 {
                return;
            }
            let new_top = (top + PAGE_SIZE).min(h - 1);
            if new_top != top {
                self.selected = None;
                self.load_page(new_top, cx);
            } else {
                self.refresh_records(cx);
            }
        }
    }

    /// Self-contained proof: recompute the block hash from its fields.
    fn verify_block(b: &Block) -> bool {
        let h = &b.header;
        Block::compute_hash(
            h.index,
            &h.prev_hash,
            h.timestamp,
            h.view,
            h.seq,
            h.proposer,
            &b.record,
        ) == h.hash
    }

    // ---------- search ----------

    fn search_watermark(&mut self, cx: &mut Context<Self>) {
        let wm = self.wm_input.read(cx).value().trim().to_string();
        if wm.is_empty() {
            return;
        }
        self.search_loading = true;
        self.search_error = None;
        cx.notify();
        let client = self.client.clone();
        request(cx, async move { client.by_watermark(&wm).await }, |this, res, cx| {
            this.search_loading = false;
            match res {
                Ok(blocks) => {
                    this.search_results = blocks;
                    if this.search_results.is_empty() {
                        this.search_error = Some("no record with that watermark".into());
                    }
                }
                Err(e) => this.search_error = Some(e.to_string()),
            }
            cx.notify();
        })
        .detach();
    }

    fn search_user(&mut self, cx: &mut Context<Self>) {
        let user = self.user_input.read(cx).value().trim().to_string();
        if user.is_empty() {
            return;
        }
        self.search_loading = true;
        self.search_error = None;
        cx.notify();
        let client = self.client.clone();
        request(cx, async move { client.by_user(&user).await }, |this, res, cx| {
            this.search_loading = false;
            match res {
                Ok(blocks) => {
                    this.search_results = blocks;
                    if this.search_results.is_empty() {
                        this.search_error = Some("no records for that user".into());
                    }
                }
                Err(e) => this.search_error = Some(e.to_string()),
            }
            cx.notify();
        })
        .detach();
    }

    // ---------- logs ----------

    fn fetch_logs(&mut self, cx: &mut Context<Self>) {
        self.logs_loading = true;
        self.logs_error = None;
        cx.notify();
        let client = self.client.clone();
        let level = if self.log_level.trim().is_empty() {
            None
        } else {
            Some(self.log_level.trim().to_uppercase())
        };
        let source = if self.log_source.trim().is_empty() {
            None
        } else {
            Some(self.log_source.trim().to_string())
        };
        request(
            cx,
            async move {
                client
                    .logs(level.as_deref(), 200, source.as_deref())
                    .await
            },
            |this, res, cx| {
                this.logs_loading = false;
                match res {
                    Ok(body) => {
                        this.gw_logs = body.gateway.unwrap_or_default();
                        this.node_logs = body.nodes.unwrap_or_default();
                    }
                    Err(e) => this.logs_error = Some(e.to_string()),
                }
                cx.notify();
            },
        )
        .detach();
    }

    fn set_log_level(&mut self, level: &str, cx: &mut Context<Self>) {
        self.log_level = if level == "ALL" { String::new() } else { level.to_string() };
        self.fetch_logs(cx);
    }

    // ---------- status ----------

    fn refresh_status(&mut self, cx: &mut Context<Self>) {
        self.status_loading = true;
        self.status_error = None;
        cx.notify();
        let client = self.client.clone();
        request(
            cx,
            async move {
                let st = client.status().await?;
                let ob = client.outbox().await.unwrap_or(serde_json::Value::Null);
                Ok::<_, anyhow::Error>((st, ob))
            },
            |this, res, cx| {
                this.status_loading = false;
                match res {
                    Ok((st, ob)) => {
                        this.height = Some(this.height_of(&st));
                        this.status = Some(st);
                        this.outbox = Some(ob);
                        this.connected = Some(true);
                    }
                    Err(e) => this.status_error = Some(e.to_string()),
                }
                cx.notify();
            },
        )
        .detach();
    }

    // ---------- connection ----------

    fn apply_url(&mut self, cx: &mut Context<Self>) {
        let url = self.url_input.read(cx).value().trim().to_string();
        if url.is_empty() {
            return;
        }
        let permit = self.permit_input.read(cx).value().trim().to_string();
        self.base_url = url.clone();
        self.client = GatewayClient::new(&url);
        if !permit.is_empty() {
            self.client.set_permit(&permit);
        }
        self.connected = None;
        self.conn_error = None;
        self.boot(cx);
    }

    fn save_permit(&mut self, cx: &mut Context<Self>) {
        let permit = self.permit_input.read(cx).value().trim().to_string();
        self.client.set_permit(&permit);
        cx.notify();
    }

    // ---------- render helpers ----------

    fn tab_button(&self, cx: &mut Context<Self>, tab: Tab) -> impl IntoElement {
        let active = self.tab == tab;
        let id = match tab {
            Tab::Records => "tab-records",
            Tab::Search => "tab-search",
            Tab::Logs => "tab-logs",
            Tab::Status => "tab-status",
        };
        let mut b = Button::new(id)
            .small()
            .label(tab.label().to_string())
            .flex_shrink_0();
        b = if active { b.primary() } else { b.outline() };
        b.on_click(cx.listener(move |this, _, _, cx| {
            this.tab = tab;
            cx.notify();
        }))
    }

    fn conn_dot(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let (label, color) = match self.connected {
            Some(true) => ("● connected", theme.success),
            Some(false) => ("● unreachable", theme.danger),
            None => ("● connecting…", theme.muted_foreground),
        };
        div().text_xs().text_color(color).child(label.to_string())
    }

    fn error_banner(&self, cx: &mut Context<Self>, err: &Option<String>) -> impl IntoElement {
        let theme = cx.theme().clone();
        match err {
            Some(e) => div()
                .text_xs()
                .text_color(theme.danger)
                .px_3()
                .py_1()
                .child(e.clone()),
            None => div(),
        }
    }

    fn short_hash(h: &str) -> String {
        if h.len() > 16 { format!("{}…", &h[..16]) } else { h.to_string() }
    }

    fn kv_row(&self, cx: &mut Context<Self>, k: &str, v: &str) -> impl IntoElement {
        let theme = cx.theme().clone();
        div()
            .flex()
            .flex_row()
            .gap_2()
            .text_xs()
            .child(
                div()
                    .w(px(112.))
                    .flex_shrink_0()
                    .text_color(theme.muted_foreground)
                    .child(k.to_string()),
            )
            .child(
                div()
                    .flex_1()
                    .text_color(theme.foreground)
                    .child(v.to_string()),
            )
    }

    fn block_row(
        &self,
        cx: &mut Context<Self>,
        b: &Block,
        selected: bool,
    ) -> impl IntoElement {
        let theme = cx.theme().clone();
        let idx = b.header.index;
        let ok = Self::verify_block(b);
        div()
            .id(("block-row", idx))
            .flex()
            .flex_row()
            .items_center()
            .gap_3()
            .px_3()
            .py_2()
            .rounded_md()
            .cursor_pointer()
            .when(selected, |t| t.bg(theme.muted))
            .hover(|s| s.bg(theme.muted))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.selected = Some(idx);
                cx.notify();
            }))
            .child(
                div()
                    .w(px(56.))
                    .flex_shrink_0()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(format!("#{idx}")),
            )
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .child(
                        div()
                            .text_sm()
                            .truncate()
                            .text_color(theme.foreground)
                            .child(b.record.watermark.clone()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .truncate()
                            .text_color(theme.muted_foreground)
                            .child(format!(
                                "{} · {} · {}",
                                b.record.user_id,
                                b.record.session_id,
                                Self::short_hash(&b.header.hash)
                            )),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .flex_shrink_0()
                    .text_color(if ok { theme.success } else { theme.danger })
                    .child(if ok { "✓ hash" } else { "✗ hash" }.to_string()),
            )
    }

    fn render_detail(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let Some(idx) = self.selected else {
            return div()
                .px_3()
                .py_2()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child("Select a record to inspect its proof.".to_string());
        };
        let Some(b) = self
            .blocks
            .iter()
            .chain(self.search_results.iter())
            .find(|b| b.header.index == idx)
            .cloned()
        else {
            return div();
        };
        let ok = Self::verify_block(&b);
        let h = &b.header;
        let r = &b.record;
        div()
            .flex()
            .flex_col()
            .gap_1()
            .px_3()
            .py_2()
            .child(
                div()
                    .text_sm()
                    .text_color(if ok { theme.success } else { theme.danger })
                    .child(
                        if ok {
                            "✓ hash recomputed from fields — matches header".to_string()
                        } else {
                            "✗ HASH MISMATCH — treat as suspect".to_string()
                        },
                    ),
            )
            .child(self.kv_row(cx, "index", &h.index.to_string()))
            .child(self.kv_row(cx, "hash", &h.hash))
            .child(self.kv_row(cx, "prev_hash", &h.prev_hash))
            .child(self.kv_row(
                cx,
                "view / seq / proposer",
                &format!("{} / {} / {}", h.view, h.seq, h.proposer),
            ))
            .child(self.kv_row(cx, "watermark", &r.watermark))
            .child(self.kv_row(cx, "session", &r.session_id))
            .child(self.kv_row(cx, "timestamp", &r.timestamp.to_string()))
            .child(self.kv_row(cx, "user", &r.user_id))
            .child(self.kv_row(cx, "file_hash", &r.file_hash))
            .child(self.kv_row(cx, "payload_hash", &r.payload_hash))
            .child(self.kv_row(cx, "auth_server", &r.auth_server))
            .child(self.kv_row(cx, "signature", &r.signature))
    }

    fn render_records(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let height_txt = self
            .height
            .map(|h| format!("{h} blocks"))
            .unwrap_or_else(|| "—".to_string());
        let page_txt = match (self.page_top, self.height) {
            (Some(top), Some(h)) if h > 0 => {
                format!("showing #{}–#{} ", top.saturating_sub(PAGE_SIZE - 1), top)
            }
            _ => String::new(),
        };
        let selected = self.selected;
        let mut rows: Vec<gpui::AnyElement> = Vec::new();
        for b in &self.blocks {
            let sel = selected == Some(b.header.index);
            rows.push(self.block_row(cx, b, sel).into_any_element());
        }
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
                            .text_sm()
                            .text_color(theme.foreground)
                            .child(format!("Chain  ·  {height_txt}")),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(page_txt),
                    )
                    .child(
                        Button::new("rec-refresh")
                            .small()
                            .label("Refresh".to_string())
                            .on_click(cx.listener(|this, _, _, cx| this.refresh_records(cx))),
                    )
                    .child(
                        Button::new("rec-newer")
                            .small()
                            .label("▲ newer".to_string())
                            .on_click(cx.listener(|this, _, _, cx| this.page_next(cx))),
                    )
                    .child(
                        Button::new("rec-older")
                            .small()
                            .label("▼ older".to_string())
                            .on_click(cx.listener(|this, _, _, cx| this.page_prev(cx))),
                    )
                    .child(if self.records_loading {
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child("loading…".to_string())
                    } else {
                        div()
                    }),
            )
            .child(self.error_banner(cx, &self.records_error.clone()))
            .child(
                div()
                    .id("records-scroll")
                    .flex_1()
                    .flex()
                    .flex_col()
                    .overflow_y_scroll()
                    .track_scroll(&self.records_scroll)
                    .px_1()
                    .py_1()
                    .children(rows),
            )
            .child(
                div()
                    .id("records-detail-scroll")
                    .h(px(220.))
                    .flex_shrink_0()
                    .border_t_1()
                    .border_color(theme.border)
                    .overflow_y_scroll()
                    .child(self.render_detail(cx)),
            )
    }

    fn render_search(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let selected = self.selected;
        let mut rows: Vec<gpui::AnyElement> = Vec::new();
        for b in &self.search_results {
            let sel = selected == Some(b.header.index);
            rows.push(self.block_row(cx, b, sel).into_any_element());
        }
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
                    .child(div().w(px(256.)).child(Input::new(&self.wm_input).cleanable(true)))
                    .child(
                        Button::new("search-wm")
                            .small()
                            .label("By watermark".to_string())
                            .on_click(cx.listener(|this, _, _, cx| this.search_watermark(cx))),
                    )
                    .child(div().w(px(192.)).child(Input::new(&self.user_input).cleanable(true)))
                    .child(
                        Button::new("search-user")
                            .small()
                            .label("By user".to_string())
                            .on_click(cx.listener(|this, _, _, cx| this.search_user(cx))),
                    )
                    .child(if self.search_loading {
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child("searching…".to_string())
                    } else {
                        div()
                    }),
            )
            .child(self.error_banner(cx, &self.search_error.clone()))
            .child(
                div()
                    .id("search-scroll")
                    .flex_1()
                    .flex()
                    .flex_col()
                    .overflow_y_scroll()
                    .track_scroll(&self.records_scroll)
                    .px_1()
                    .py_1()
                    .children(rows),
            )
            .child(
                div()
                    .id("search-detail-scroll")
                    .h(px(220.))
                    .flex_shrink_0()
                    .border_t_1()
                    .border_color(theme.border)
                    .overflow_y_scroll()
                    .child(self.render_detail(cx)),
            )
    }

    fn log_entry_row(
        &self,
        cx: &mut Context<Self>,
        ts: String,
        level: String,
        target: String,
        msg: String,
    ) -> impl IntoElement {
        let theme = cx.theme().clone();
        let color = match level.as_str() {
            "ERROR" => theme.danger,
            "WARN" => theme.warning,
            _ => theme.muted_foreground,
        };
        div()
            .flex()
            .flex_row()
            .gap_2()
            .px_3()
            .py_1()
            .text_xs()
            .child(
                div()
                    .flex_shrink_0()
                    .w(px(160.))
                    .text_color(theme.muted_foreground)
                    .child(ts.to_string()),
            )
            .child(div().flex_shrink_0().w(px(56.)).text_color(color).child(level.to_string()))
            .child(
                div()
                    .flex_shrink_0()
                    .w(px(224.))
                    .truncate()
                    .text_color(theme.muted_foreground)
                    .child(target.to_string()),
            )
            .child(div().flex_1().text_color(theme.foreground).child(msg.to_string()))
    }

    fn render_logs(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let levels = ["ALL", "ERROR", "WARN", "INFO", "DEBUG"];
        let mut level_btns: Vec<gpui::AnyElement> = Vec::new();
        for l in levels {
            let active = if l == "ALL" {
                self.log_level.is_empty()
            } else {
                self.log_level == l
            };
            let mut b = Button::new(l)
                .small()
                .label(l.to_string())
                .flex_shrink_0();
            b = if active { b.primary() } else { b.outline() };
            let lvl = l.to_string();
            level_btns.push(
                b.on_click(cx.listener(move |this, _, _, cx| this.set_log_level(&lvl, cx)))
                    .into_any_element(),
            );
        }
        let gw_logs = self.gw_logs.clone();
        let node_logs = self.node_logs.clone();
        let mut rows: Vec<gpui::AnyElement> = Vec::new();
        for e in &gw_logs {
            rows.push(
                self.log_entry_row(
                    cx,
                    e.ts.clone(),
                    e.level.clone(),
                    format!("gateway:{}", e.target),
                    e.message.clone(),
                )
                .into_any_element(),
            );
        }
        for n in &node_logs {
            if let Some(entries) = &n.entries {
                for e in entries {
                    rows.push(
                        self.log_entry_row(
                            cx,
                            e.ts.clone(),
                            e.level.clone(),
                            format!("{}:{}", n.node_addr, e.target),
                            e.message.clone(),
                        )
                        .into_any_element(),
                    );
                }
            } else {
                rows.push(
                    self.log_entry_row(
                        cx,
                        String::new(),
                        "ERROR".to_string(),
                        n.node_addr.clone(),
                        n.error.clone().unwrap_or_else(|| "no entries".to_string()),
                    )
                    .into_any_element(),
                );
            }
        }
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
                    .children(level_btns)
                    .child(div().w(px(192.)).child(Input::new(&self.permit_input).cleanable(true)))
                    .child(
                        Button::new("permit-save")
                            .small()
                            .label("Use permit".to_string())
                            .on_click(cx.listener(|this, _, _, cx| this.save_permit(cx))),
                    )
                    .child(
                        Button::new("logs-fetch")
                            .small()
                            .label("Fetch".to_string())
                            .on_click(cx.listener(|this, _, _, cx| this.fetch_logs(cx))),
                    )
                    .child(if self.logs_loading {
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child("fetching…".to_string())
                    } else {
                        div()
                    }),
            )
            .child(
                div()
                    .px_3()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(
                        "Auditor permit required (mint: sagex-certauth issue-permit --identity ledger-auditor). Paste it above; never committed to the repo.".to_string(),
                    ),
            )
            .child(self.error_banner(cx, &self.logs_error.clone()))
            .child(
                div()
                    .id("logs-scroll")
                    .flex_1()
                    .flex()
                    .flex_col()
                    .overflow_y_scroll()
                    .track_scroll(&self.logs_scroll)
                    .py_1()
                    .children(rows),
            )
    }

    fn render_status(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let mut cards: Vec<gpui::AnyElement> = Vec::new();
        if let Some(st) = &self.status {
            for n in &st.ledger {
                let addr = n
                    .get("node_addr")
                    .and_then(|v| v.as_str())
                    .unwrap_or("?")
                    .to_string();
                let txt = format!(
                    "node {} · height {} · view {} · seq {}\ntip {}",
                    n.get("node_id").map(|v| v.to_string()).unwrap_or("?".into()),
                    n.get("height").map(|v| v.to_string()).unwrap_or("?".into()),
                    n.get("view").map(|v| v.to_string()).unwrap_or("?".into()),
                    n.get("seq").map(|v| v.to_string()).unwrap_or("?".into()),
                    n.get("tip_hash")
                        .and_then(|v| v.as_str())
                        .map(Self::short_hash)
                        .unwrap_or_else(|| "?".into()),
                );
                cards.push(
                    div()
                        .flex_1()
                        .rounded_md()
                        .border_1()
                        .border_color(theme.border)
                        .px_3()
                        .py_2()
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(addr),
                        )
                        .child(div().text_sm().text_color(theme.foreground).child(txt))
                        .into_any_element(),
                );
            }
        }
        let pending = self
            .status
            .as_ref()
            .map(|s| s.outbox_pending.to_string())
            .unwrap_or_else(|| "—".to_string());
        let mut outbox_rows: Vec<gpui::AnyElement> = Vec::new();
        if let Some(arr) = self.outbox.as_ref().and_then(|v| v.as_array()) {
            for e in arr.iter().take(20) {
                let wm = e
                    .get("watermark")
                    .and_then(|v| v.as_str())
                    .unwrap_or("?")
                    .to_string();
                let stxt = e
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("?")
                    .to_string();
                outbox_rows.push(
                    div()
                        .flex()
                        .flex_row()
                        .gap_2()
                        .px_3()
                        .py_1()
                        .text_xs()
                        .child(div().flex_1().text_color(theme.foreground).child(wm))
                        .child(div().text_color(theme.muted_foreground).child(stxt))
                        .into_any_element(),
                );
            }
        }
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
                        div().text_sm().text_color(theme.foreground).child(format!(
                            "Cluster  ·  outbox pending: {pending}"
                        )),
                    )
                    .child(
                        Button::new("status-refresh")
                            .small()
                            .label("Refresh".to_string())
                            .on_click(cx.listener(|this, _, _, cx| this.refresh_status(cx))),
                    )
                    .child(if self.status_loading {
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child("loading…".to_string())
                    } else {
                        div()
                    }),
            )
            .child(self.error_banner(cx, &self.status_error.clone()))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .gap_2()
                    .px_3()
                    .py_1()
                    .children(cards),
            )
            .child(
                div()
                    .px_3()
                    .py_1()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child("Gateway outbox (latest 20):".to_string()),
            )
            .child(
                div()
                    .id("status-scroll")
                    .flex_1()
                    .flex()
                    .flex_col()
                    .overflow_y_scroll()
                    .track_scroll(&self.records_scroll)
                    .children(outbox_rows),
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
                    .child("Ledger Explorer".to_string()),
            )
            .child(self.conn_dot(cx))
            .child(self.tab_button(cx, Tab::Records))
            .child(self.tab_button(cx, Tab::Search))
            .child(self.tab_button(cx, Tab::Logs))
            .child(self.tab_button(cx, Tab::Status))
            .child(
                div()
                    .mt_4()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child("Gateway URL".to_string()),
                    )
                    .child(Input::new(&self.url_input).small()),
            )
            .child(
                Button::new("apply-url")
                    .small()
                    .label("Connect".to_string())
                    .on_click(cx.listener(|this, _, _, cx| this.apply_url(cx))),
            )
            .child(self.error_banner(cx, &self.conn_error.clone()))
    }
}

impl Render for Explorer {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let body = match self.tab {
            Tab::Records => self.render_records(cx).into_any_element(),
            Tab::Search => self.render_search(cx).into_any_element(),
            Tab::Logs => self.render_logs(cx).into_any_element(),
            Tab::Status => self.render_status(cx).into_any_element(),
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

/// Periodically refresh the visible tab (records height, logs, status).
/// Call once from setup with the view's context.
pub fn tick(cx: &mut Context<Explorer>) {
    cx.spawn(async move |weak, cx| {
        loop {
            cx.background_executor()
                .timer(std::time::Duration::from_secs(10))
                .await;
            let alive = weak.update(cx, |this, cx| {
                match this.tab {
                    Tab::Records => this.refresh_records(cx),
                    Tab::Logs => {
                        if this.client.has_permit() {
                            this.fetch_logs(cx);
                        }
                    }
                    Tab::Status => this.refresh_status(cx),
                    Tab::Search => {}
                }
            });
            if alive.is_err() {
                break;
            }
        }
    })
    .detach();
}
