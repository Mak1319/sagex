pub mod avatar;
pub mod badge;
pub mod cards;
pub mod channel_row;
pub mod composer;
pub mod icon;
pub mod icon_button;
pub mod icon_kind;
pub mod menus;
pub mod message_card;
pub mod presence;
pub mod rows;
pub mod section_head;

// Public component catalog: every reusable piece, importable from one place.
pub use avatar::{Avatar, avatar_el};
pub use badge::{Badge, UnreadCount, badge_el};
pub use cards::{AttachmentCard, CodeBlock, SchematicCard};
pub use channel_row::ChannelRow;
pub use composer::Composer;
pub use icon::{Icon, icon_el};
pub use icon_button::IconButton;
pub use icon_kind::IconKind;
pub use menus::{ContextMenu, MessageActionBar};
pub use message_card::MessageCard;
pub use presence::PresenceDot;
pub use rows::{AttachmentRow, MetaCell, SearchInput, ThreadRow};
pub use section_head::SectionHead;
