use std::default::Default;
use std::fmt::{Display, Formatter};
use std::num::NonZeroU64;
use itertools::Itertools;
use num_enum::{IntoPrimitive, TryFromPrimitive};

use poise::ChoiceParameter;
use poise::serenity_prelude::UserId;
use sqlx::{Connection, Error, FromRow, Row, SqliteConnection};

use paste::paste;
use crate::emoji::{CROWN_IDS, RANK_IDS};

use crate::error::ElodonError;
use crate::filters::*;
use crate::map_no_rows;

pub trait ElodonDisplay{
    fn get_display_text(&self) -> String;

}

pub trait ElodonDisplayList<E>{
    fn get_display_text(&self) -> String;
}

impl<E: ElodonDisplay> ElodonDisplayList<E> for Vec<E>{
    fn get_display_text(&self) -> String {
        return if self.is_empty() {
            "No results".to_string()
        } else {
            self.iter()
                .map(|e| e.get_display_text())
                .join("\n")
                .to_string()
        }
    }
}

// USER

#[derive(Clone, FromRow)]
pub struct User{
    #[sqlx(rename = "user_id")]
    pub id: i64,
    #[sqlx(rename = "discord_id")]
    pub discord: i64,
    #[sqlx(rename = "user_name")]
    pub name: String,
    pub elo1: Option<f32>,
    pub elo2: Option<f32>,
    pub elo3: Option<f32>,
    pub elo4: Option<f32>,
}

impl User {
    pub fn discord_id(&self) -> UserId{
        return UserId::new(self.discord as u64)
    }

    pub fn elo(&self, level: DisplayLevel) -> Option<f32>{
        return match level{
            DisplayLevel::Easy   => self.elo1,
            DisplayLevel::Med => self.elo2,
            DisplayLevel::Hard   => self.elo3,
            DisplayLevel::OniPlus    => self.elo4,
        }
    }

}

impl ElodonDisplay for User{
    fn get_display_text(&self) -> String {
        return format!("`#{:<13}  {:<9}`<@{}>", self.id, self.name, self.discord)
    }
}

impl FetchAll<Play> for User{}

impl From<User> for GeneralFilter{
    fn from(value: User) -> Self {
        GeneralFilter::new()
            .user_id(Some(value.id))
            .discord_id(Some(value.discord_id()))
    }
}

// SONG

#[derive(Clone, Eq, PartialEq, Hash, FromRow)]
pub struct Song{
    #[sqlx(rename = "song_id")]
    pub id: u32,
    #[sqlx(rename = "song_name_jap")]
    pub name_jap: String,
    #[sqlx(rename = "song_name_eng")]
    pub name_eng: String,
    #[sqlx(rename = "genre_id")]
    pub genre: u32,
}

impl Song {
    pub fn get_name<'a>(&self) -> String {
        let genre: Genre = self.genre.try_into().unwrap();
        if self.name_eng == "" { //if no eng name, use japanese name
            self.name_jap.clone()
        } else if self.name_eng == self.name_jap { //if names same only show one
            self.name_eng.clone()
        } else { //if names different show both
            format!("{} | {}", self.name_eng, self.name_jap)
        }
    }

    pub async fn get_matching(conn: &mut SqliteConnection, fragment: &str, genre: Option<Genre>) -> Result<Vec<Song>, ElodonError> {
        let wrapped_fragment = format!("%{fragment}%");

        let songs: Result<Vec<Song>, Error> = match genre {
            None => {
                sqlx::query_as(
                    "SELECT song_id, song_name_eng, song_name_jap, genre_id FROM songs WHERE song_name_eng like ?\
                         UNION \
                         SELECT song_id, song_name_eng, song_name_jap, genre_id FROM songs WHERE song_name_jap like ?"
                ).bind(&wrapped_fragment).bind(&wrapped_fragment)
            }
            Some(genre) => {
                sqlx::query_as(
                    "SELECT song_id, song_name_eng, song_name_jap, genre_id FROM songs WHERE song_name_eng like ? AND genre_id = ?\
                          UNION \
                         SELECT song_id, song_name_eng, song_name_jap, genre_id FROM songs WHERE song_name_jap like ? AND genre_id = ?"
                ).bind(&wrapped_fragment).bind(genre.id()).bind(&wrapped_fragment).bind(genre.id())
            }
        }.fetch_all(conn).await;
        map_no_rows!(songs: Vec["song"], fragment)
    }

    pub fn genre(&self) -> Genre {
        return Genre::try_from(self.genre).unwrap();
    }

}

impl Display for Song {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} (#{})", self.get_name(),  self.id)
    }
}

impl ElodonDisplay for Song{
    fn get_display_text(&self) -> String {
        return format!("`#{:<4}  {} > {}`", self.id, self.genre(), self.get_name())
    }
}

impl FetchAll<Chart> for Song{}
impl FetchAll<Play> for Song{}

impl From<Song> for GeneralFilter{
    fn from(value: Song) -> Self {
        GeneralFilter::new()
            .song_id(Some(value.id))
            .genre(Some(value.genre()))
    }
}


// LEVEL

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, ChoiceParameter, TryFromPrimitive, IntoPrimitive)]
#[num_enum(error_type(name = ElodonError, constructor = ElodonError::WrongLevelId))]
#[repr(u32)]
pub enum Level{
    Easy = 1,
    Med = 2,
    Hard = 3,
    Oni = 4,
    Ura = 5,
}

impl Level {
    pub fn id(&self) -> u32{
        (*self).into()
    }
    pub fn decrease(&self) -> Option<Self> {
        return Level::try_from(self.id()-1).ok();
    }
}

impl Display for Level {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, ChoiceParameter)]
pub enum DisplayLevel{
    Easy,
    Med,
    Hard,
    #[name = "Oni+"]
    OniPlus,
}

impl DisplayLevel{
    pub fn min_value(&self) -> u32 {
        match &self {
            DisplayLevel::Easy => 1,
            DisplayLevel::Med => 2,
            DisplayLevel::Hard => 3,
            DisplayLevel::OniPlus => 4,
        }
    }
    pub fn max_value(&self) -> u32 {
        match &self {
            DisplayLevel::Easy => 1,
            DisplayLevel::Med => 2,
            DisplayLevel::Hard => 3,
            DisplayLevel::OniPlus => 5,
        }
    }
}
impl From<Level> for DisplayLevel {
    fn from(value: Level) -> Self {
        match value {
            Level::Easy => DisplayLevel::Easy,
            Level::Med => DisplayLevel::Med,
            Level::Hard => DisplayLevel::Hard,
            Level::Oni => DisplayLevel::OniPlus,
            Level::Ura => DisplayLevel::OniPlus
        }
    }
}

// GENRE

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, ChoiceParameter, TryFromPrimitive, IntoPrimitive)]
#[num_enum(error_type(name = ElodonError, constructor = ElodonError::WrongGenreId))]
#[repr(u32)]
pub enum Genre{
    Pop = 1,
    Anime = 2,
    Kids = 3,
    Vocaloid = 4,
    #[name = "Game Music"]
    GameMusic = 5,
    #[name = "Namco Original"]
    NamcoOriginal = 6,
    Variety = 7,
    Classical = 8,
}

impl Genre {
    pub fn id(&self) -> u32{
        (*self).into()
    }
}

impl Display for Genre {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

// CHART

#[derive(Copy, Clone, Debug, FromRow)]
pub struct Chart {
    #[sqlx(rename = "song_id")]
    pub id: u32,
    #[sqlx(rename = "level_id")]
    pub level: u32,
    pub score_slope: Option<i32>,
    pub score_miyabi: Option<i32>,
    pub sd_mean: Option<f32>,
    pub sd_sd: Option<f32>,
}

impl Chart {
    pub fn id(&self) -> ChartId {
        ChartId(self.id, self.level.try_into().unwrap())
    }
    pub fn level(&self) -> Level {
        Level::try_from(self.level).unwrap()
    }
    pub async fn full_name(&self, conn: &mut SqliteConnection) -> Result<String, ElodonError> {
        let song = self.fetch_one_other::<Song>(conn).await?;
        Ok(format!("{} ({})", song.get_name(), self.level()))
    }
}

impl ElodonDisplay for Chart{
    fn get_display_text(&self) -> String {

        return format!("#{:<4}.{}:\n`Score/ELO={:>4} Miyabi ELO={:>4}\nsd= {} ({})`",
            self.id, self.level,
            self.score_slope.map(|i| format!("{i:>4}")).unwrap_or(" ?? ".to_string()),
            self.score_miyabi.map(|i| format!("{i:>4}")).unwrap_or(" ?? ".to_string()),
            self.sd_mean.map(|i|format!("{i:>7.0}")).unwrap_or("  ???  ".to_string()),
            self.sd_sd.map(|i|format!("{i:>4.0}")).unwrap_or(" ?? ".to_string())
        )
    }
}

impl FetchOne<Song> for Chart{}
impl FetchAll<Play> for Chart{}

impl From<Chart> for GeneralFilter{
    fn from(chart: Chart) -> Self {
        GeneralFilter::new()
            .song_id(Some(chart.id))
            .level(Some(chart.level()))
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct ChartId(pub u32, pub Level);

impl ChartId {
    pub fn song_id(&self) -> u32 {
        return self.0;
    }
    pub fn level(&self) -> Level{
        return self.1;
    }
}

impl Display for ChartId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}({})", self.0, self.1)
    }
}

// PLAY

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, FromRow)]
pub struct Play{
    #[sqlx(rename = "user_id")]
    pub user: i64,
    #[sqlx(rename = "song_id")]
    pub song: u32,
    #[sqlx(rename = "level_id")]
    pub level: u32,
    pub score: u32,
    pub rank: u32,
    pub crown: u32,
    pub good_cnt: u32,
    pub ok_cnt: u32,
    pub bad_cnt: u32,
    pub combo_cnt: u32,
    pub roll_cnt: u32
}

impl Play{
    pub fn level(&self) -> Level {
        Level::try_from(self.level).unwrap()
    }
}

impl ElodonDisplay for Play{
    fn get_display_text(&self) -> String {
        let crown_emoji = format!("<:crown_{}:{}>", self.crown, CROWN_IDS.get(self.crown as usize).expect("invalid crown id"));
        let rank_emoji = if self.rank < 2 {String::new()} else {
            format!("<:rank_{}:{}>", self.rank, RANK_IDS.get(self.rank as usize).expect("invalid rank id"))
        };

        format!("{:>4}.{} {:<13} {:>7} {} {}\n` {:>4} | {:>3} | {:<3} c{:<4} r{:<4}`",
            self.song, self.level, self.score, self.user, crown_emoji, rank_emoji,
            self.good_cnt, self.ok_cnt, self.bad_cnt, self.combo_cnt, self.level
        )

    }
}

impl FetchOne<User> for Play{}
impl FetchOne<Song> for Play{}
impl FetchOne<Chart> for Play{}

impl From<Play> for GeneralFilter{
    fn from(play: Play) -> Self {
        GeneralFilter::new()
            .user_id(Some(play.user))
            .song_id(Some(play.song))
            .level(Some(play.level.try_into().unwrap()))
    }
}




