pub mod attach_menu;
pub mod avatar;
pub mod badge;
pub mod bubble;
pub mod colors;
pub mod composer;
pub mod emoji_panel;
pub mod menu;
pub mod notice;
pub mod reply_bar;
pub mod scroll;
pub mod tail;
pub mod theme_toggle;

use gpui::{App, ClickEvent, Window};
use std::rc::Rc;

/// Click callback shared by interactive components.
pub type ClickHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;
/// Index-payload callback (attach sheet, tabs).
pub type IndexHandler = Rc<dyn Fn(&usize, &mut Window, &mut App)>;
/// Text-payload callback (emoji pick).
pub type TextHandler = Rc<dyn Fn(&String, &mut Window, &mut App)>;
/// Flag-payload callback (GIF switch).
pub type FlagHandler = Rc<dyn Fn(&bool, &mut Window, &mut App)>;
/// Theme-choice callback.
pub type ThemeHandler = Rc<dyn Fn(&theme_toggle::ThemeChoice, &mut Window, &mut App)>;

pub use attach_menu::AttachMenu;
pub use avatar::Avatar;
pub use badge::UnreadBadge;
pub use bubble::{BubbleContent, ChatBubble, LinkCard, ReactionChips, Tick};
pub use colors::{ACCENT_GREEN, OUTGOING_DARK, OUTGOING_LIGHT};
pub use composer::MessageComposer;
pub use emoji_panel::EmojiPanel;
pub use menu::{MenuCard, MenuRow};
pub use notice::NoticePill;
pub use reply_bar::ReplyBar;
pub use scroll::ScrollThumb;
pub use tail::Tail;
pub use theme_toggle::{ThemeChoice, ThemeToggle};
