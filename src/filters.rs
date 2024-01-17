use poise::serenity_prelude::UserId;
use sqlx::{FromRow, SqliteConnection};
use crate::error::ElodonError;
use crate::Error;
use crate::structs::*;
use std::fmt::{Display, Formatter};

use paste::paste;
use poise::futures_util::stream::iter;
use poise::ChoiceParameter;

#[macro_export]
macro_rules! map_no_rows {
    ($result: ident : Vec[$return_type:literal], $id: expr) => {
        match $result {
            Ok(list) => {
                if list.is_empty(){
                    return Err(ElodonError::NoResults {
                        search: format!("{}", $return_type),
                        id: stringify!($id).to_string(),
                    });
                } else {
                    return Ok(list);
                }
            },
            Err(sqlx::Error::RowNotFound) => Err(ElodonError::NoResults {
                    search: stringify!($return_type).to_string(),
                    id: stringify!($id).to_string(),
                }),
            Err(err) => {Err(ElodonError::DatabaseError(err))}
        }
    };
    ($result: ident : $return_type:literal, $id: expr) => {
        $result.map_err(|err| {
            match err {
                sqlx::Error::RowNotFound => ElodonError::NoResults {
                    search: stringify!($return_type).to_string(),
                    id: $id.to_string(),
                },
                _ => {ElodonError::DatabaseError(err)}
            }
        })
    };
}

macro_rules! create_search_filter {
($filter:ident,
    $($field:ident: $field_type:ty =>
        ($query_format_string:literal, $($query_values:expr),*),
        ($display_format_string:literal, $($display_values:expr),*)
    ),+
) => {
paste!{
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Default)]
pub struct $filter {
    $(
    pub $field: Option<$field_type>,
    )*
}

impl $filter {

    pub fn new() ->$filter {
        Self::default()
    }

    fn query_string(&self) -> Option<String>{
        let mut response = String::new();
        $(
        if let Some($field) = self.$field {
            let processed_query = format!($query_format_string, $($query_values),+);
            response.push_str(&*format!("{} AND ", processed_query))
        }
        )*
        if response == ""{
            None
        } else {
            Some(response.strip_suffix(" AND ").unwrap().to_string())
        }
    }

    $(
    // the chaining query creation
    pub fn [<set_ $field>](&mut self, $field: Option<$field_type>) {
        self.$field = $field;
    }
    // the chaining query creation
    pub fn $field(&self, $field: Option<$field_type>) -> $filter {
        $filter{
        $field: $field,
        ..*self
        }
    }
    )*
}

//the formatted search query
impl Display for $filter {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut response = " where ".to_string();
        $(
        if let Some($field) = self.$field {
            let processed_query = format!($display_format_string, $($display_values),*);
            response.push_str(&*format!("{} and ", processed_query))
        }
        )*
        write!(f, "{}",
            if response == " where "{
                String::new()
            } else {
                response.strip_suffix(" and ").unwrap().to_string()
            }
        )
    }
}
}};}
macro_rules! create_search_filter_with_query_commands {
($row:ident $filter: ident $table_name:literal $columns: literal,
    $($field:ident: $field_type:ty =>
        ($query_format_string:literal, $($query_values:expr),*),
        ($display_format_string:literal, $($display_values:expr),*)
    ),+
) => {
paste!{

create_search_filter!(
    $filter,
        $($field: $field_type =>
            ($query_format_string,$($query_values),*),
            ($display_format_string,$($display_values),*)
        ),*
);

impl $filter{

    //formatting into appending to and an sql query
    fn get_search(&self, columns: &str) -> String {
        match self.query_string(){
            None =>
                format!("SELECT {} FROM {}", columns, $table_name),
            Some(query_string) =>
                format!("SELECT {} FROM {} WHERE {}", columns, $table_name, query_string)
        }
    }
}

impl Filter<$row> for $filter {
    async fn fetch_one(&self, conn: &mut SqliteConnection) -> Result<$row, ElodonError> {
        let final_query = self.get_search($columns);
        let value: Result<$row, sqlx::Error> = sqlx::query_as(&*final_query).fetch_one(conn).await;
        map_no_rows!(value: $table_name, self)
    }
    async fn fetch_all(&self, conn: &mut SqliteConnection) -> Result<Vec<$row>, ElodonError>{
        let final_query = self.get_search($columns);
        let values: Result<Vec<$row>, sqlx::Error> = sqlx::query_as(&*final_query).fetch_all(conn).await;
        map_no_rows!(values: $table_name, self)
    }
}

impl Filterable for $row {
    async fn fetch_one(conn: &mut SqliteConnection, filter: impl Into<GeneralFilter>) -> Result<Self, ElodonError>{
        let general_filter: GeneralFilter = filter.into();
        let specific_filter: $filter = general_filter.into();
        specific_filter.fetch_one(conn).await
    }
    async fn fetch_all(conn: &mut SqliteConnection, filter: impl Into<GeneralFilter>) -> Result<Vec<Self>, ElodonError>{
        let general_filter: GeneralFilter = filter.into();
        let specific_filter: $filter = general_filter.into();
        specific_filter.fetch_all(conn).await
    }
}

impl From<$filter> for GeneralFilter{
    fn from(value: $filter) -> GeneralFilter {
        GeneralFilter{
            $(
            $field: value.$field,
            )*
            ..Default::default()
        }
    }
}

impl From<GeneralFilter> for $filter{
    fn from(value: GeneralFilter) -> $filter {
        $filter{
            $(
            $field: value.$field,
            )*
            ..Default::default()
        }
    }
}
}};}


trait Filter<R> {
    async fn fetch_one(&self, conn: &mut SqliteConnection) -> Result<R, ElodonError>;
    async fn fetch_all(&self, conn: &mut SqliteConnection) -> Result<Vec<R>, ElodonError>;
}

pub trait Filterable: Clone + Sized{
    async fn fetch_one(conn: &mut SqliteConnection, filter: impl Into<GeneralFilter>) -> Result<Self, ElodonError>;
    async fn fetch_all(conn: &mut SqliteConnection, filter: impl Into<GeneralFilter>) -> Result<Vec<Self>, ElodonError>;
}


pub trait FetchOne<R: Filterable>: Into<GeneralFilter> {
    async fn fetch_one_other<B: From<R>>(self, conn: &mut SqliteConnection) -> Result<B, ElodonError>{
        R::fetch_one(conn, self).await.map(|r| B::from(r))
    }
}

pub trait FetchAll<R: Filterable>: Into<GeneralFilter> {
    async fn fetch_all_other<B: From<R>>(self, conn: &mut SqliteConnection) -> Result<Vec<B>, ElodonError>{
        Ok(R::fetch_all(conn, self).await?.iter().cloned().map(|r| B::from(r)).collect())
    }
}

//used for printing the filter. sort of a dud class

create_search_filter! (
    GeneralFilter,
    user_id: i64 =>
        ("user_id = {}", user_id),
        ("[user ids hidden]",),
    discord_id: UserId  =>
        ("discord_id = {}", discord_id.get()),
        ("discord <@{}>", discord_id.get()),
    song_id: u32 =>
        ("song_id = {}", song_id),
        ("song_id is {}", song_id),
    level: Level =>
        ("level_id = {}", level.id()),
        ("level_id is {}", level.id()),
    display_level: DisplayLevel =>
        ("level_id BETWEEN {} AND {}", display_level.min_value(), display_level.max_value()),
        ("level is {}", display_level.name()),
    genre: Genre =>
        ("genre_id = {}", genre.id()),
        ("genre is {}", genre.name())
);

create_search_filter_with_query_commands!(
    User UserFilter "users" "user_id, discord_id, user_name, elo1, elo2, elo3, elo4",
    user_id: i64 =>
        ("user_id = {}", user_id),
        ("[user ids hidden]",),
    discord_id: UserId  =>
        ("discord_id = {}", discord_id.get()),
        ("discord <@{}>", discord_id.get())
);

create_search_filter_with_query_commands!(
    Song SongFilter "songs" "song_id, song_name_eng, song_name_jap, genre_id",
    song_id: u32 =>
        ("song_id = {}", song_id),
        ("song_id is {}", song_id),
    genre: Genre =>
        ("genre_id = {}", genre.id()),
        ("genre is {}", genre.name())
);

create_search_filter_with_query_commands!(
    Chart ChartFilter "charts" "song_id, level_id, score_slope, score_miyabi, certainty",
    song_id: u32 =>
        ("song_id = {}", song_id),
        ("song_id is {}", song_id),
    level: Level =>
        ("level_id = {}", level.id()),
        ("level_id is {}", level.id()),
    display_level: DisplayLevel =>
        ("level_id BETWEEN {} AND {}", display_level.min_value(), display_level.max_value()),
        ("level is {}", display_level.name())
);

create_search_filter_with_query_commands!(
    Play PlayFilter "top_plays" "user_id, song_id, level_id, score",
    user_id: i64 =>
        ("user_id = {}", user_id),
        ("[user ids hidden]",),
    song_id: u32 =>
        ("song_id = {}", song_id),
        ("song_id is {}", song_id),
    level: Level =>
        ("level_id = {}", level.id()),
        ("level_id is {}", level.id()),
    display_level: DisplayLevel =>
        ("level_id BETWEEN {} AND {}", display_level.min_value(), display_level.max_value()),
        ("level is {}", display_level.name())
);