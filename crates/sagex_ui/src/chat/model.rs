#![allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum ChatKind {
    #[default]
    Dm,
    Group,
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum ChatFilter {
    #[default]
    All,
    Unread,
    Favourites,
    Groups,
    Archived,
}

#[derive(Clone, Default)]
pub struct LinkPreview {
    pub domain: String,
    pub title: String,
    pub url: String,
}

#[derive(Clone, Default)]
pub enum MessageKind {
    #[default]
    Text,
    Image {
        caption: String,
    },
    Voice {
        duration: String,
        bars: Vec<u32>,
    },
    Document {
        name: String,
        size: String,
    },
    Contact {
        name: String,
        phone: String,
    },
    Poll {
        question: String,
        options: Vec<String>,
        votes: Vec<u32>,
        my_vote: Option<usize>,
    },
    Event {
        title: String,
        when: String,
    },
    Sticker {
        glyph: String,
    },
}

/// Deterministic waveform bars for voice bubbles.
pub fn voice_bars(seed: usize) -> Vec<u32> {
    (0..28)
        .map(|i| 4 + ((seed * 7 + i * 13) % 18) as u32)
        .collect()
}

/// Read-receipt state for outgoing messages. Flip one value per message
/// to change its ticks.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum MessageStatus {
    /// Single grey tick: sent.
    #[default]
    Sent,
    /// Double grey tick: delivered.
    Delivered,
    /// Double blue tick: read.
    Read,
}

/// Watermark-registration state for a received sealed message (fail-closed
/// receive pipeline). `None` on `Message.seal` = ordinary message.
#[derive(Clone, Default, PartialEq, Eq)]
pub enum SealState {
    /// Placeholder inserted while verify → register → decrypt runs.
    #[default]
    Pending,
    /// Gate passed (201 committed / 202 queued / duplicate) and content
    /// decrypted + marked.
    Done {
        watermark: String,
        /// "committed" | "queued" (+ "duplicate" detail kept in text).
        status: String,
        block_index: Option<u64>,
    },
    /// Gate failed or device locked: plaintext never shown. `retry` re-runs
    /// permit → register for the same sealed bytes (idempotent).
    Blocked { reason: String },
}

#[derive(Clone)]
pub struct Message {
    pub id: usize,
    pub sender: String,
    pub text: String,
    pub time: String,
    pub mine: bool,
    /// Server message id (hex) once synced; used for WS dedup.
    pub server_id: Option<String>,
    /// Local file attachment (offline vault). Drives the File bubble.
    pub attachment: Option<super::files::Attachment>,
    /// Date divider label shown above this message ("Yesterday", "Today").
    pub date: String,
    /// Rich link card; auto-detected on send, seeded for demo threads.
    pub link: Option<LinkPreview>,
    /// Bubble layout variant (text, image, voice, ...).
    pub kind: MessageKind,
    /// Emoji reactions toggled from the message menu.
    pub reactions: Vec<String>,
    /// Server message id this message replies to (None = top-level).
    pub reply_to: Option<String>,
    /// Deleted messages stay as a "message deleted" record (WhatsApp style).
    pub deleted: bool,
    /// Watermark-registration state (None = ordinary message).
    pub seal: Option<SealState>,
    /// Read-receipt ticks (only shown on outgoing bubbles).
    pub ticks: MessageStatus,
}

/// First `http(s)://` token in `text` becomes a preview card.
pub fn extract_link(text: &str) -> Option<LinkPreview> {
    let start = text.find("http://").or_else(|| text.find("https://"))?;
    let rest = &text[start..];
    let end = rest
        .find(char::is_whitespace)
        .map(|i| start + i)
        .unwrap_or(text.len());
    let url = text[start..end].to_string();
    let host = url
        .trim_start_matches("https://")
        .trim_start_matches("http://");
    let host = host.split('/').next().unwrap_or(host);
    Some(LinkPreview {
        domain: host.to_string(),
        title: host.to_string(),
        url,
    })
}

/// Sender name color for group bubbles (WhatsApp-style palette).
pub fn sender_color(name: &str) -> u32 {
    const PALETTE: &[u32] = &[0x00a884, 0x0199d5, 0xd3396c, 0x7f5af0, 0xe6890b];
    let h: usize = name.bytes().fold(0, |a, b| a.wrapping_add(b as usize));
    PALETTE[h % PALETTE.len()]
}

/// Up to 2 initials from a display name ("Suman Rana" -> "SR").
pub fn sender_initials(name: &str) -> String {
    name.split_whitespace()
        .filter_map(|w| w.chars().next())
        .take(2)
        .collect()
}

#[derive(Clone)]
pub struct Chat {
    pub id: usize,
    pub name: String,
    pub subtitle: String,
    pub kind: ChatKind,
    pub initials: String,
    pub color: u32,
    pub unread: usize,
    pub fav: bool,
    pub archived: bool,
    pub chat_pinned: bool,
    pub last_time: String,
    pub messages: Vec<Message>,
    /// Server room id (hex). `None` for local-only preview chats.
    pub server_id: Option<String>,
}

impl Chat {
    pub fn preview(&self) -> String {
        self.messages
            .last()
            .map(|m| {
                if m.deleted {
                    "🚫 This message was deleted".to_string()
                } else if let Some(SealState::Blocked { .. }) = m.seal {
                    "🚫 Blocked: watermark not registered".to_string()
                } else if let Some(SealState::Pending) = m.seal {
                    "⏳ Verifying watermark…".to_string()
                } else if m.mine {
                    format!("You: {}", m.text)
                } else if self.kind == ChatKind::Group {
                    format!("{}: {}", m.sender, m.text)
                } else {
                    m.text.clone()
                }
            })
            .unwrap_or_default()
    }
}
