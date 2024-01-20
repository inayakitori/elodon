use poise::serenity_prelude::{EmojiId, ReactionType};

pub const UP: ReactionType = ReactionType::Custom {
    animated: false,
    id: EmojiId::new(1197234127494201467),
    name: None,
};
pub const DOWN: ReactionType = ReactionType::Custom {
    animated: false,
    id: EmojiId::new(1197234114999357550),
    name: None,
};
pub const LEFT: ReactionType = ReactionType::Custom {
    animated: false,
    id: EmojiId::new(1197234117406896208),
    name: None,
};
pub const RIGHT: ReactionType = ReactionType::Custom {
    animated: false,
    id: EmojiId::new(1197234122012233898),
    name: None,
};