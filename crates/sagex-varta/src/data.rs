//! Static placeholder content mirroring the prototype.
use eframe::egui;
use crate::theme;

pub struct Node {
    pub letter: &'static str,
    pub bg: egui::Color32,
    pub fg: egui::Color32,
    pub name: &'static str,
    pub time: &'static str,
    pub preview: &'static str,
    pub badge: Option<&'static str>,
    pub active: bool,
}

fn tint(bg: [u8; 3], fg: [u8; 3]) -> (egui::Color32, egui::Color32) {
    (
        egui::Color32::from_rgb(bg[0], bg[1], bg[2]),
        egui::Color32::from_rgb(fg[0], fg[1], fg[2]),
    )
}

pub fn pinned() -> Vec<Node> {
    let (r1, r2) = tint([0xFF, 0xDA, 0xD6], [0x93, 0x00, 0x0A]);
    let (b1, b2) = tint([0xDB, 0xE1, 0xFF], [0x00, 0x3E, 0xA8]);
    let (g1, g2) = tint([0xD1, 0xFA, 0xE5], [0x06, 0x5F, 0x46]);
    vec![
        Node { letter: "R", bg: r1, fg: r2, name: "Rustic Development", time: "10:24", preview: "You: pushed mask parser changes", badge: None, active: false },
        Node { letter: "N", bg: b1, fg: b2, name: "Team Nivara", time: "09:12", preview: "Aarav: SIH submission draft", badge: None, active: false },
        Node { letter: "S", bg: g1, fg: g2, name: "System Design", time: "Yesterday", preview: "You: shared a diagram", badge: None, active: false },
    ]
}

pub fn streams() -> Vec<Node> {
    let mk = |l, bg: [u8; 3], fg: [u8; 3], name, time, prev, badge, active| {
        Node { letter: l, bg: egui::Color32::from_rgb(bg[0], bg[1], bg[2]), fg: egui::Color32::from_rgb(fg[0], fg[1], fg[2]), name, time, preview: prev, badge, active }
    };
    vec![
        mk("I", [0xDB, 0xE1, 0xFF], [0x00, 0x4A, 0xC6], "Image Processing", "14:20", "Priya: This approach looks good...", Some("3"), true),
        mk("C", [0xD1, 0xFA, 0xE5], [0x06, 0x5F, 0x46], "Crypto Research", "12:04", "ZKP circuit verification logs", Some("1"), false),
        mk("B", [0xE8, 0xE0, 0xFF], [0x4B, 0x00, 0xA8], "Build & DevOps", "10:51", "Karan: CI pipeline failed", None, false),
        mk("U", [0xFF, 0xED, 0xD5], [0x92, 0x40, 0x0E], "UI/UX Design", "09:33", "Mira: shared a file", None, false),
        mk("D", [0xC4, 0xE7, 0xFF], [0x00, 0x4C, 0x69], "Data Collection", "Yesterday", "Target cluster ready for ingestion", None, false),
        mk("G", [0xEF, 0xF4, 0xFF], [0x56, 0x5E, 0x74], "General", "Yesterday", "Quarterly demo schedule announced", None, false),
        mk("A", [0xDA, 0xE2, 0xFD], [0x13, 0x1B, 0x2E], "Archive Format", "Sep 30", "Standardized raw chunk size", None, false),
    ]
}

pub struct Message {
    pub letter: &'static str,
    pub bg: [u8; 3],
    pub fg: [u8; 3],
    pub author: &'static str,
    pub time: &'static str,
    pub role: Option<(&'static str, [u8; 3], [u8; 3])>,
    pub loc: &'static str,
    pub body: &'static str,
    pub kind: MsgKind,
    pub own: bool,
}

pub enum MsgKind {
    Schematic,
    Question,
    Code,
    Attachment,
}

pub fn messages() -> Vec<Message> {
    vec![
        Message {
            letter: "P", bg: [0xFF, 0xDA, 0xD6], fg: [0x93, 0x00, 0x0A],
            author: "Priya Sharma", time: "14:12:08 UTC",
            role: Some(("CORE-ALGO", [0x06, 0x5F, 0x46], [0xD1, 0xFA, 0xE5])),
            loc: "LOC: S1-01",
            body: "I've been testing the blending approach for the background layer. The results look much cleaner now with the normalized multipliers.",
            kind: MsgKind::Schematic, own: false,
        },
        Message {
            letter: "A", bg: [0xD1, 0xFA, 0xE5], fg: [0x06, 0x5F, 0x46],
            author: "Arjun Mehta", time: "14:15:32 UTC",
            role: Some(("OWNER", [0x00, 0x3E, 0xA8], [0xDB, 0xE1, 0xFF])),
            loc: "LOC: S1-02",
            body: "Nice! Can you share the updated shader code? Also, does this work with grayscale images or do we require an additional channel expansion step?",
            kind: MsgKind::Question, own: false,
        },
        Message {
            letter: "Y", bg: [0xDB, 0xE1, 0xFF], fg: [0x00, 0x4A, 0xC6],
            author: "You", time: "14:18:19 UTC",
            role: Some(("AUTHOR", [0x92, 0x40, 0x0E], [0xFF, 0xFB, 0xEB])),
            loc: "LOC: S1-03",
            body: "Here is the updated fragment shader stage. Handled the luminance conversion automatically inside the kernel:",
            kind: MsgKind::Code, own: true,
        },
        Message {
            letter: "P", bg: [0xFF, 0xDA, 0xD6], fg: [0x93, 0x00, 0x0A],
            author: "Priya Sharma", time: "14:20:02 UTC",
            role: Some(("CORE-ALGO", [0x06, 0x5F, 0x46], [0xD1, 0xFA, 0xE5])),
            loc: "LOC: S1-04",
            body: "Here are the test images and the comparison results rendered through the WebGL pipeline:",
            kind: MsgKind::Attachment, own: false,
        },
    ]
}

pub fn _operators() -> Vec<(&'static str, bool)> {
    vec![
        ("Arjun Mehta · OWNER", true),
        ("Priya Sharma · ONLINE", true),
        ("You · ONLINE", true),
        ("Mira Patel · AWAY", false),
        ("Karan Varma · OFFLINE", false),
    ]
}

pub fn attachments() -> Vec<(&'static str, &'static str)> {
    vec![
        ("blend-comparison.png", "320 KB"),
        ("shader-notes.md", "14 KB"),
        ("test-images.zip", "4.5 MB"),
    ]
}

pub fn _keep_theme_live() -> egui::Color32 {
    theme::SURFACE
}
