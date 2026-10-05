//! Iconoir SVG icons (vendored in assets/icons), rasterized on demand by
//! [`crate::svg_loader::SvgLoader`] (resvg + tiny-skia). SVG stays the only
//! icon format — no PNGs.
use std::borrow::Cow;

use eframe::egui;

#[derive(Clone, Copy)]
#[allow(dead_code)]
pub enum Icon {
    ChatBubble,
    Network,
    MultiplePages,
    Wrench,
    HalfMoon,
    SunLight,
    Computer,
    Settings,
    Search,
    Pin,
    Filter,
    VideoCamera,
    Group,
    MoreVert,
    MoreHoriz,
    Code,
    Copy,
    Download,
    MediaImage,
    Attachment,
    Reply,
    Forward,
    Quote,
    Bookmark,
    Send,
    ArrowRight,
    Frame,
    TextBox,
    Calendar,
    Clock,
    Database,
    Archive,
    Mail,
    Cube,
    PcCheck,
}

macro_rules! svg {
    ($file:literal) => {
        egui::ImageSource::Bytes {
            uri: Cow::Borrowed(concat!("bytes://iconoir-", $file)),
            bytes: egui::load::Bytes::from(include_bytes!(concat!("../assets/icons/", $file)) as &'static [u8; _]),
        }
    };
}

/// Build the tinted image widget without placing it (for custom rects).
pub fn image(icon: Icon, size: f32, color: egui::Color32) -> egui::Image<'static> {
    let src = match icon {
        Icon::ChatBubble => svg!("chat-bubble.svg"),
        Icon::Network => svg!("network.svg"),
        Icon::MultiplePages => svg!("multiple-pages.svg"),
        Icon::Wrench => svg!("wrench.svg"),
        Icon::HalfMoon => svg!("half-moon.svg"),
        Icon::SunLight => svg!("sun-light.svg"),
        Icon::Computer => svg!("computer.svg"),
        Icon::Settings => svg!("settings.svg"),
        Icon::Search => svg!("search.svg"),
        Icon::Pin => svg!("pin.svg"),
        Icon::Filter => svg!("filter.svg"),
        Icon::VideoCamera => svg!("video-camera.svg"),
        Icon::Group => svg!("group.svg"),
        Icon::MoreVert => svg!("more-vert.svg"),
        Icon::MoreHoriz => svg!("more-horiz.svg"),
        Icon::Code => svg!("code.svg"),
        Icon::Copy => svg!("copy.svg"),
        Icon::Download => svg!("download.svg"),
        Icon::MediaImage => svg!("media-image.svg"),
        Icon::Attachment => svg!("attachment.svg"),
        Icon::Reply => svg!("reply.svg"),
        Icon::Forward => svg!("forward.svg"),
        Icon::Quote => svg!("quote.svg"),
        Icon::Bookmark => svg!("bookmark.svg"),
        Icon::Send => svg!("send.svg"),
        Icon::ArrowRight => svg!("arrow-right.svg"),
        Icon::Frame => svg!("frame.svg"),
        Icon::TextBox => svg!("text-box.svg"),
        Icon::Calendar => svg!("calendar.svg"),
        Icon::Clock => svg!("clock.svg"),
        Icon::Database => svg!("database.svg"),
        Icon::Archive => svg!("archive.svg"),
        Icon::Mail => svg!("mail.svg"),
        Icon::Cube => svg!("cube.svg"),
        Icon::PcCheck => svg!("pc-check.svg"),
    };
    egui::Image::new(src).fit_to_exact_size(egui::vec2(size, size)).tint(color)
}

/// Show a tinted iconoir glyph at `size` px.
///
/// The loader rasterizes strokes in white, so `tint` multiplies to the exact
/// requested color.
pub fn show(ui: &mut egui::Ui, icon: Icon, size: f32, color: egui::Color32) {
    ui.add(image(icon, size, color));
}
