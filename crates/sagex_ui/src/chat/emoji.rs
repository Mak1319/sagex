//! Emoji picker tables (picker content, not chrome).

/// Popular emoji keywords so the panel search box matches text queries.
pub const EMOJI_ALIASES: &[(&str, &str)] = &[
    ("😀", "smile happy grin"),
    ("😁", "smile happy grin teeth"),
    ("😂", "laugh lol joy tears"),
    ("🤣", "laugh rofl lol"),
    ("😊", "smile blush happy"),
    ("😍", "love heart eyes"),
    ("😘", "kiss love"),
    ("🥰", "love hearts smile"),
    ("😎", "cool sunglasses"),
    ("🤔", "thinking hmm"),
    ("😴", "sleep tired"),
    ("🥳", "party celebrate"),
    ("😢", "cry sad tears"),
    ("😭", "cry sob tears sad"),
    ("😡", "angry rage red"),
    ("👍", "thumbsup like yes ok"),
    ("👎", "thumbsdown no dislike"),
    ("👏", "clap applause"),
    ("🙏", "pray thanks please"),
    ("💪", "strong muscle flex"),
    ("👋", "wave hello hi bye"),
    ("🎉", "party celebrate tada"),
    ("❤️", "love heart red"),
    ("🔥", "fire hot lit"),
    ("🐶", "dog puppy"),
    ("🐱", "cat kitten"),
    ("🍕", "pizza food"),
    ("🍔", "burger food"),
    ("☕", "coffee tea drink"),
    ("⚽", "football soccer sport"),
    ("🎮", "gaming videogame"),
    ("✅", "check yes done ok"),
    ("🎤", "mic microphone karaoke"),
    ("📷", "camera photo"),
    ("⭐", "star favorite"),
];
pub const EMOJI_CATEGORIES: &[(&str, &str, &[&str])] = &[
    (
        "Smileys & People",
        "😀",
        &[
            "😀", "😁", "😂", "🤣", "😊", "😍", "😘", "🥰", "😎", "🤔", "😐", "🙄", "😴", "🤯",
            "🥳", "😢", "😭", "😡", "👍", "👎", "👏", "🙏", "💪", "👋", "🎉", "❤️", "🔥", "✨",
        ],
    ),
    (
        "Animals & Nature",
        "🐻",
        &[
            "🐶", "🐱", "🐭", "🐹", "🐰", "🦊", "🐻", "🐼", "🐨", "🐯", "🦁", "🐮", "🐷", "🐸",
            "🐵", "🌸", "🌹", "🍀", "🌈", "☀️", "🌙", "⭐",
        ],
    ),
    (
        "Food & Drink",
        "🍔",
        &[
            "🍎", "🍐", "🍊", "🍋", "🍌", "🍉", "🍇", "🍓", "🍒", "🍑", "🍍", "🥭", "🍕", "🍔",
            "🍟", "🌭", "🍿", "🍩", "🍪", "🎂", "🍰", "☕",
        ],
    ),
    (
        "Activity",
        "⚽",
        &[
            "⚽", "🏀", "🏈", "⚾", "🎾", "🏐", "🎱", "🏓", "🏸", "🥅", "🎮", "🎲", "🎯", "🎨",
            "🎭", "🎪", "🎤", "🎧", "🎸", "🥁", "🏆", "🚴", "🏊", "🎿",
        ],
    ),
    (
        "Symbols",
        "💡",
        &[
            "❤️", "🧡", "💛", "💚", "💙", "💜", "🖤", "💔", "💯", "💢", "💥", "💫", "💦", "💤",
            "✅", "❌", "⚠️", "🚫", "💡", "🔔", "🎵", "📌", "🔒", "🔑",
        ],
    ),
];
