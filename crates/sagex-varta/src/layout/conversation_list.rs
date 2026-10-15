//! Scrollable conversation list — manual `gpui-base` expansion only.
//!
//! Layout: outer div takes the rest width/height, split into (i) the item
//! container (`flex_1`, native scrollbar hidden) and (ii) a dedicated 5px
//! scrollbar lane holding the custom `base::Scrollbar`. The bar never
//! overlays rows. Rows come from a fixed-seed generator so variants stay
//! stable across runs without hardcoding each item.

use gpui_kit::base::Scrollbar;
use gpui_kit::*;

use crate::components::conversation_list_item::ConversationListItem;
use crate::theme;

const COUNT: usize = 24;
const SEED: u32 = 0xC0FFEE;

struct Row {
    id: SharedString,
    initial: SharedString,
    avatar_bg: u32,
    avatar_border: u32,
    avatar_fg: u32,
    title: SharedString,
    preview: SharedString,
    time: SharedString,
    unread: usize,
    active: bool,
    online: bool,
    clip: bool,
    pinned: bool,
    critical: bool,
}

const NAMES: [&str; 12] = [
    "Sys Admin", "Marketing", "Crypto Desk", "UX Review", "Ops Alerts",
    "Design Sync", "Backend Guild", "Founders", "Support Inbox", "Data Pipeline",
    "Mobile Team", "Security",
];

const PREVIEWS: [&str; 12] = [
    "You: restarted the relay node",
    "Ava: launch assets are ready",
    "New quote posted for review",
    "Mira: pinned the new mockups",
    "2 incidents need triage",
    "Kai: pushed the migration diff",
    "Standup notes are in the thread",
    "You: shipped the hotfix",
    "Priya: design tokens updated",
    "Build #482 passed on main",
    "Lena: shared the test plan",
    "Weekly digest is out",
];

const TIMES: [&str; 8] = [
    "09:12", "10:04", "11:02", "12:40", "Yesterday", "Tue", "Mon", "Sun",
];

const PALETTES: [(u32, u32, u32); 5] = [
    (0xd1fae5, 0xa7f3d0, 0x047857),
    (0xdbeafe, 0x93c5fd, 0x1d4ed8),
    (0xecfdf5, 0xa7f3d0, 0x047857),
    (0xfaf5ff, 0xe9d5ff, 0x7e22ce),
    (0xfff7ed, 0xfed7aa, 0xc2410c),
];

fn next(state: &mut u32) -> u32 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    *state = x;
    x
}

fn seed_rows() -> Vec<Row> {
    let mut rng = 0x9e37_79b9u32 ^ SEED;
    (0..COUNT)
        .map(|i| {
            let name = NAMES[(next(&mut rng) as usize) % NAMES.len()];
            let critical = next(&mut rng) % 12 == 0;
            let palette = PALETTES[(next(&mut rng) as usize) % PALETTES.len()];
            let roll = next(&mut rng);
            let unread = match roll % 10 {
                0 | 1 => (roll % 12 + 1) as usize,
                2 => 1,
                _ => 0,
            };
            let clip = next(&mut rng) % 3 == 0;
            let mut row = Row {
                id: SharedString::from(format!("conversation-{i:02}")),
                initial: SharedString::from(name.chars().next().unwrap_or('?').to_string()),
                avatar_bg: palette.0,
                avatar_border: palette.1,
                avatar_fg: palette.2,
                title: SharedString::from(name),
                preview: SharedString::from(PREVIEWS[(next(&mut rng) as usize) % PREVIEWS.len()]),
                time: SharedString::from(TIMES[(next(&mut rng) as usize) % TIMES.len()]),
                unread,
                active: false,
                online: next(&mut rng) % 4 == 0,
                clip,
                pinned: next(&mut rng) % 5 == 0,
                critical,
            };
            // First rows showcase each variant so all states are visible
            // without scrolling; the rest stay randomly distributed.
            match i {
                0 => {
                    row.pinned = true;
                    row.unread = 3;
                    row.online = true;
                }
                1 => row.clip = true,
                2 => row.critical = true,
                3 => row.active = true,
                _ => {}
            }
            row
        })
        .collect()
}

pub struct ConversationList {
    scroll: ScrollHandle,
    rows: Vec<Row>,
}

impl ConversationList {
    pub fn new() -> Self {
        Self {
            scroll: ScrollHandle::new(),
            rows: seed_rows(),
        }
    }

    pub fn pinned_count(&self) -> usize {
        self.rows.iter().filter(|row| row.pinned).count()
    }
}

impl Default for ConversationList {
    fn default() -> Self {
        Self::new()
    }
}

impl Render for ConversationList {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_row()
            .flex_1()
            .w_full()
            .overflow_hidden()
            .bg(theme::rgb(theme::colors::SURFACE_LOWEST))
            .child(
                div()
                    .id(ElementId::Name("conversation-list".into()))
                    .flex()
                    .flex_col()
                    .flex_1()
                    .overflow_y_scroll()
                    .scrollbar_width(px(0.0))
                    .track_scroll(&self.scroll)
                    .children(self.rows.iter().map(|row| {
                        ConversationListItem::new(row.id.clone())
                            .initial(row.initial.clone())
                            .avatar(row.avatar_bg, row.avatar_border, row.avatar_fg)
                            .title(row.title.clone())
                            .preview(row.preview.clone())
                            .time(row.time.clone())
                            .unread(row.unread)
                            .active(row.active)
                            .online(row.online)
                            .clip(row.clip)
                            .pinned(row.pinned)
                            .critical(row.critical)
                    })),
            )
            .child(
                div()
                    .w(px(5.0))
                    .h_full()
                    .flex_shrink_0()
                    .relative()
                    .child(
                        Scrollbar::vertical(&self.scroll)
                            .id(ElementId::Name("conversation-scroll".into()))
                            .viewport_from_layout()
                            .styles(|s| {
                                s.track(|t| t.width(px(5.0)))
                                    .thumb(|t| {
                                        t.width(px(3.0))
                                            .radius(px(1.5))
                                            .bg(theme::rgb(theme::colors::GRID_L2))
                                    })
                            }),
                    ),
            )
    }
}
