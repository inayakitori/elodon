use std::fmt::Display;
use poise::ChoiceParameter;
use sqlx::{Error, FromRow, Row, SqliteConnection};
use sqlx::encode::IsNull::No;
use crate::error::ElodonError;

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
        user.map_err(|err| ElodonError::WrongId {
            sql_error: err,
            search: "user_id".to_string(),
            id,
        })
    }

    fn placeholder() -> Self {
        User{
            id: -1,
            name: "User DNE".to_string(),
            elo1: f32::NAN,
            elo2: f32::NAN,
            elo3: f32::NAN,
            elo4: f32::NAN,
        }
    }
}

#[derive(Clone, Eq, PartialEq, Hash, FromRow)]
pub struct Song{
    #[sqlx(rename = "song_id")]
    pub id: u32,
    #[sqlx(rename = "song_name_jap")]
    pub name_jap: String,
    #[sqlx(rename = "song_name_eng")]
    pub name_eng: String,
}

impl FromId<u32> for Song {
    async fn from_id(conn: &mut SqliteConnection, id: u32) -> Result<Self, ElodonError> {
        let song: Result<Song, _> = sqlx::query_as(
            "SELECT song_id, song_name_eng, song_name_jap FROM songs WHERE song_id = ?"
        ).bind(id)
            .fetch_one(conn).await;
        song.map_err(|err| ElodonError::WrongId {
            sql_error: err,
            search: "song_id".to_string(),
            id: id as i64,
        })
    }

    fn placeholder() -> Self {
        Song{
            id: 0,
            name_jap: "Song DNE".to_string(),
            name_eng: "Song DNE".to_string(),
        }
    }
}

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

#[derive(Copy, Clone, Debug, ChoiceParameter)]
pub enum Level{
    Easy,
    Medium,
    Hard,
    Oni,
    Ura
}

pub trait FromId<T> : Sized{
    async fn from_id(conn: &mut SqliteConnection, id: T) -> Result<Self, ElodonError>;
    // this allows for filling in values so that "warnings" can exist
    fn placeholder() -> Self;
}


impl From<Level> for u32 {
    fn from(value: Level) -> Self {
        match value{
            Level::Easy => {1}
            Level::Medium => {2}
            Level::Hard => {3}
            Level::Oni => {4}
            Level::Ura => {5}
        }
    }
}

impl TryFrom<u32> for Level {
    type Error = ();

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            1 => {Ok(Level::Easy)},
            2 => {Ok(Level::Medium)},
            3 => {Ok(Level::Hard)},
            4 => {Ok(Level::Oni)},
            5 => {Ok(Level::Ura)},
            _ => {Err(())}
        }
    }
}
