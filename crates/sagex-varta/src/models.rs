#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ChannelId(pub u32);

#[derive(Clone, Debug)]
pub struct Channel {
    pub id: ChannelId,
    pub name: &'static str,
    pub tag: &'static str,
    pub preview: &'static str,
    pub time: &'static str,
    pub unread: u32,
    pub online: bool,
    pub pinned: bool,
}

#[derive(Clone, Debug)]
pub enum MsgBody {
    Text(&'static str),
    Schematic {
        title: &'static str,
        lines: &'static [&'static str],
    },
    Code {
        file: &'static str,
        lang: &'static str,
        code: &'static str,
    },
    Attachment {
        name: &'static str,
        meta: &'static str,
    },
}

#[derive(Clone, Debug)]
pub struct Message {
    pub author: &'static str,
    pub time: &'static str,
    pub badge: &'static str,
    pub mine: bool,
    pub body: MsgBody,
}

#[derive(Clone, Debug)]
pub struct Participant {
    pub name: &'static str,
    pub status: Presence,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Presence {
    Online,
    Away,
    Offline,
}

/// Routes plug in here: new variant + one match arm in `views`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Route {
    Channel(ChannelId),
    Threads,
    Settings,
}
