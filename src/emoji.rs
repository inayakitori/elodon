use std::sync::OnceLock;
use std::string::ToString;
use lazy_static::lazy::Lazy;
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

pub const COMBO_IDS: [u64;2] = [
    1213193226119413810,
    1213193227629232229
];

pub const ROLLS_IDS: [u64;3] = [
    1213203737674252318,
    1213203739742306304,
    1213203741906444318
];

pub const JUDGEMENT_IDS: [u64;4] = [
    1213185463419142206,
    1213185465201725520,
    1213185469207023646,
    1213185467009335427
];

pub const CROWN_IDS: [u64;4] = [
    1213187539750486046,
    1213187542263009351,
    1213187545383575593,
    1213187548902457434,
];


pub const RANK_IDS: [u64;9] = [
    1213187552673144903,
    1213187552673144903,
    1213187554837536788,
    1213187556884353075,
    1213187558922784768,
    1213187560894111835,
    1213187562559111248,
    1213187564442226688,
    1213187566220742656,
];