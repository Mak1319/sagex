use std::collections::HashMap;

use crate::models::{Channel, ChannelId, Message, MsgBody, Participant, Presence};

/// Mock feed mirroring `layout/stitch_pixel_perfect_chat_layout/code.html`.
pub fn channels() -> Vec<Channel> {
    vec![
        Channel {
            id: ChannelId(1),
            name: "Rustic Development",
            tag: "R",
            preview: "You: pushed mask parser changes",
            time: "10:24",
            unread: 0,
            online: false,
            pinned: true,
        },
        Channel {
            id: ChannelId(2),
            name: "Team Nivara",
            tag: "N",
            preview: "Aarav: SIH submission draft",
            time: "09:12",
            unread: 0,
            online: false,
            pinned: true,
        },
        Channel {
            id: ChannelId(3),
            name: "System Design",
            tag: "S",
            preview: "You: shared a diagram",
            time: "Yest",
            unread: 0,
            online: false,
            pinned: true,
        },
        Channel {
            id: ChannelId(4),
            name: "Image Processing",
            tag: "I",
            preview: "Priya: This approach looks good...",
            time: "14:20",
            unread: 3,
            online: true,
            pinned: false,
        },
        Channel {
            id: ChannelId(5),
            name: "Crypto Research",
            tag: "C",
            preview: "ZKP circuit verification logs",
            time: "12:04",
            unread: 1,
            online: false,
            pinned: false,
        },
        Channel {
            id: ChannelId(6),
            name: "Build & DevOps",
            tag: "B",
            preview: "Karan: CI pipeline failed",
            time: "10:51",
            unread: 0,
            online: false,
            pinned: false,
        },
        Channel {
            id: ChannelId(7),
            name: "UI/UX Design",
            tag: "U",
            preview: "Mira: shared a file",
            time: "09:33",
            unread: 0,
            online: false,
            pinned: false,
        },
        Channel {
            id: ChannelId(8),
            name: "Data Collection",
            tag: "D",
            preview: "Target cluster ready for ingestion",
            time: "Yest",
            unread: 0,
            online: false,
            pinned: false,
        },
        Channel {
            id: ChannelId(9),
            name: "General",
            tag: "G",
            preview: "Quarterly demo schedule announced",
            time: "Yest",
            unread: 0,
            online: false,
            pinned: false,
        },
        Channel {
            id: ChannelId(10),
            name: "Archive Format",
            tag: "A",
            preview: "Standardized raw chunk size",
            time: "Sep 30",
            unread: 0,
            online: false,
            pinned: false,
        },
        Channel {
            id: ChannelId(11),
            name: "Hardware",
            tag: "H",
            preview: "FPGA synthesis logs updated",
            time: "Sep 29",
            unread: 0,
            online: false,
            pinned: false,
        },
    ]
}

pub fn messages() -> HashMap<ChannelId, Vec<Message>> {
    let mut m = HashMap::new();
    m.insert(ChannelId(4), vec![
        Message { author: "Priya Sharma", time: "14:12:08 UTC", badge: "CORE-ALGO", mine: false, body: MsgBody::Text("I've been testing the blending approach for the background layer. The results look much cleaner now with the normalized multipliers.") },
        Message { author: "Priya Sharma", time: "14:12:40 UTC", badge: "CORE-ALGO", mine: false, body: MsgBody::Schematic { title: "FIG 2.1 — LAYER BLEND PIPELINE", lines: &["L0 SRC_RGBA (8-bit)  x  L1 BLEND_BUFFER", "CH: RED 0.92 | GRN 0.44 | BLU 0.78 — RES: PASS-01"] } },
        Message { author: "Arjun Mehta", time: "14:15:32 UTC", badge: "OWNER", mine: false, body: MsgBody::Text("Nice! Can you share the updated shader code? Also, does this work with grayscale images or do we require an additional channel expansion step?") },
        Message { author: "You", time: "14:18:19 UTC", badge: "AUTHOR", mine: true, body: MsgBody::Code { file: "blend.glsl", lang: "GLSL ES 3.0", code: "vec3 lum = vec3(dot(c.rgb, vec3(0.299, 0.587, 0.114))); // auto grayscale" } },
        Message { author: "Priya Sharma", time: "14:20:02 UTC", badge: "CORE-ALGO", mine: false, body: MsgBody::Attachment { name: "blend-comparison.png", meta: "PNG · 320 KB · 1024x683" } },
    ]);
    m.insert(
        ChannelId(5),
        vec![Message {
            author: "Zoya",
            time: "12:04:11 UTC",
            badge: "ZK",
            mine: false,
            body: MsgBody::Text(
                "ZKP circuit verification logs attached — 2 constraints failing on blob split.",
            ),
        }],
    );
    m.insert(
        ChannelId(6),
        vec![Message {
            author: "Karan",
            time: "10:51:02 UTC",
            badge: "CI",
            mine: false,
            body: MsgBody::Text("CI pipeline failed on the mask-parser job — investigating."),
        }],
    );
    m
}

pub fn participants() -> Vec<Participant> {
    vec![
        Participant {
            name: "Arjun Mehta",
            status: Presence::Online,
        },
        Participant {
            name: "Priya Sharma",
            status: Presence::Online,
        },
        Participant {
            name: "You",
            status: Presence::Online,
        },
        Participant {
            name: "Mira Patel",
            status: Presence::Away,
        },
        Participant {
            name: "Karan Varma",
            status: Presence::Offline,
        },
    ]
}
