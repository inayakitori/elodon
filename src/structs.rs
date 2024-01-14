use std::fmt::{Display, Formatter};
use std::ops::Deref;
use std::str::FromStr;
use poise::ChoiceParameter;
use sqlx::{Connection, Error, FromRow, Row, SqliteConnection};
use sqlx::encode::IsNull::No;
use crate::error::ElodonError;


macro_rules! map_no_rows {
    ($result: ident : Vec[$return_type:literal], $id: expr) => {
        match $result {
            Ok(list) => {
                if list.is_empty(){
                    return Err(ElodonError::NoResults {
                        search: format!("{}s", $return_type),
                        id: $id.to_string(),
                    });
                } else {
                    return Ok(list);
                }
            },
            Err(Error::RowNotFound) => Err(ElodonError::NoResults {
                    search: stringify!($return_type).to_string(),
                    id: $id.to_string(),
                }),
            Err(err) => {Err(ElodonError::DatabaseError(err))}
        }
    };
    ($result: ident : $return_type:literal, $id: expr) => {
        $result.map_err(|err| {
            match err {
                Error::RowNotFound => ElodonError::NoResults {
                    search: stringify!($return_type).to_string(),
                    id: $id.to_string(),
                },
                _ => {ElodonError::DatabaseError(err)}
            }
        })
    };
}

// USER

#[derive(Clone, FromRow)]
pub struct User{
    #[sqlx(rename = "user_id")]
    pub id: i64,
    #[sqlx(rename = "user_name")]
    pub name: String,
    pub elo1: f32,
    pub elo2: f32,
    pub elo3: f32,
    pub elo4: f32
}

impl FromId<i64> for User {
    async fn from_id(conn: &mut SqliteConnection, id: i64) -> Result<Self, ElodonError> {
        let user: Result<User, Error> = sqlx::query_as(
            "SELECT user_id, user_name, elo1, elo2, elo3, elo4 FROM users WHERE user_id=?"
        ).bind(id)
            .fetch_one(conn).await;
        map_no_rows!(user: "user", id)
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
    pub fn get_name<'a>(&self) -> Result<String, ElodonError> {
        let genre: Genre = self.genre.try_into()?;
        if self.name_eng == "" { //if no eng name, use japanese name
            Ok(self.name_jap.clone())
        } else if self.name_eng == self.name_jap { //if names same only show one
            Ok(self.name_eng.clone())
        } else { //if names different show both
            Ok(format!("{} > {} | {}", genre, self.name_eng, self.name_jap))
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

    pub fn genre(&self) -> Result<Genre, ElodonError> {
        return Genre::try_from(self.genre)
    }

}

impl Display for Song {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let name = self.get_name().map_err(|_| std::fmt::Error)?;
        write!(f, "{} (#{})", name,  self.id)
    }
}

impl FromId<u32> for Song {
    async fn from_id(conn: &mut SqliteConnection, id: u32) -> Result<Self, ElodonError> {
        let song: Result<Song, _> = sqlx::query_as(
            "SELECT song_id, song_name_eng, song_name_jap, genre_id FROM songs WHERE song_id = ?"
        ).bind(id)
            .fetch_one(conn).await;
        map_no_rows!(song: "song", id)
    }
}

// LEVEL

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, ChoiceParameter)]
pub enum Level{
    Easy,
    Medium,
    Hard,
    Oni,
    Ura
}

impl Level {
    pub fn id(&self) -> u32{
        match self {
            crate::structs::Level::Easy => 1,
            crate::structs::Level::Medium => 2,
            crate::structs::Level::Hard => 3,
            crate::structs::Level::Oni => 4,
            crate::structs::Level::Ura => 5
        }
    }
}

impl Display for Level {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

impl From<Level> for u32 {
    fn from(value: crate::structs::Level) -> Self {
        value.id()
    }
}

impl TryFrom<u32> for Level {
    type Error = ElodonError;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            1 => {Ok(crate::structs::Level::Easy)},
            2 => {Ok(crate::structs::Level::Medium)},
            3 => {Ok(crate::structs::Level::Hard)},
            4 => {Ok(crate::structs::Level::Oni)},
            5 => {Ok(crate::structs::Level::Ura)},
            i => {Err(ElodonError::WrongLevelId(i))}
        }
    }
}

// GENRE

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, ChoiceParameter)]
pub enum Genre{
    Pop,
    Anime,
    Kids,
    Vocaloid,
    #[name = "Game Music"]
    GameMusic,
    #[name = "Namco Original"]
    NamcoOriginal,
    Variety,
    Classical,
}

impl Genre {
    pub fn id(&self) -> u32{
        match self {
            Genre::Pop => 1,
            Genre::Anime => 2,
            Genre::Kids => 3,
            Genre::Vocaloid => 4,
            Genre::GameMusic => 5,
            Genre::NamcoOriginal => 6,
            Genre::Variety => 7,
            Genre::Classical => 8
        }
    }
}

impl Display for Genre {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

impl From<Genre> for u32 {
    fn from(value: Genre) -> Self {
        value.id()
    }
}

impl TryFrom<u32> for Genre {
    type Error = ElodonError;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Genre::Pop),
            2 => Ok(Genre::Anime),
            3 => Ok(Genre::Kids),
            4 => Ok(Genre::Vocaloid),
            5 => Ok(Genre::GameMusic),
            6 => Ok(Genre::NamcoOriginal),
            7 => Ok(Genre::Variety),
            8 => Ok(Genre::Classical),
            i => {Err(ElodonError::WrongGenreId(i))}
        }
    }
}

// CHART

#[derive(FromRow)]
pub struct Chart {
    #[sqlx(rename = "song_id")]
    pub id: u32,
    #[sqlx(rename = "level_id")]
    pub level: u32,
    pub score_slope: i32,
    pub score_miyabi: i32,
    pub certainty: f32,
}

impl FromId<ChartId> for Chart {
    async fn from_id(conn: &mut SqliteConnection, id: ChartId) -> Result<Self, ElodonError> {
        let chart: Result<Chart, _> = sqlx::query_as(
            "SELECT song_id, level_id, score_slope, score_miyabi, certainty FROM charts WHERE song_id = ? AND level_id = ?"
        ).bind(id.0)
            .bind(id.1.id())
            .fetch_one(conn).await;

        map_no_rows!(chart: "chart", id)
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct ChartId(pub u32, pub Level);

impl ChartId {

    pub async fn plays (&self, conn: &mut SqliteConnection) -> Result<Vec<Play>, ElodonError> {
        let plays = sqlx::query_as(
            "SELECT user_id, song_id, level_id, score FROM top_plays WHERE song_id=? AND level_id=?"
        ).bind(self.0).bind(self.1.id())
            .fetch_all(conn).await;

        map_no_rows!(plays: Vec["play"], self)
    }

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

#[derive(FromRow)]
pub struct Play{
    #[sqlx(rename = "user_id")]
    pub user: i64,
    #[sqlx(rename = "song_id")]
    pub song: u32,
    #[sqlx(rename = "level_id")]
    pub level: u32,
    pub score: u32,
}

impl Play{
    pub async fn get_user(&self, conn: &mut SqliteConnection) -> Result<User, ElodonError> {
        User::from_id(conn, self.user).await
    }
    pub async fn get_song(&self, conn: &mut SqliteConnection) -> Result<Song, ElodonError> {
        Song::from_id(conn, self.song).await
    }
    pub fn get_level(&self) -> Level {
        self.level.try_into().expect("level id was not valid")
    }
}

pub trait FromId<T> : Sized{
    async fn from_id(conn: &mut SqliteConnection, id: T) -> Result<Self, ElodonError>;
}
