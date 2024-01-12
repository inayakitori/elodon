use std::fmt::{Display, Formatter};
use std::ops::Deref;
use std::str::FromStr;
use std::sync::OnceLock;
use poise::{async_trait, ChoiceParameter, CommandParameterChoice, create_slash_argument, FrameworkError, PartialContext, SlashArgError, SlashArgument};
use poise::serenity_prelude::{ArgumentConvert, CacheHttp, ChannelId, CommandInteraction, CommandOptionType, CreateCommandOption, GuildId, ResolvedValue};
use lazy_static::lazy_static;
use poise::futures_util::future::err;
use sqlx::{Executor, FromRow, query, SqliteConnection, Connection};
use sqlx::sqlite::{SqliteError, SqliteQueryResult};
use crate::{Context, Error};
use crate::error::{ElodonError, ElodonErrorList};
use crate::structs::{Level, Play, Song, FromId, User};

/// Show this help menu
#[poise::command(track_edits, slash_command)]
pub async fn help(
    ctx: Context<'_>,
    #[description = "Specific command to show help about"]
    #[autocomplete = "poise::builtins::autocomplete_command"]
    command: Option<String>,
) -> Result<(), Error> {
    poise::builtins::help(
        ctx,
        command.as_deref(),
        poise::builtins::HelpConfiguration {
            extra_text_at_bottom: "shows song info, scoreboards and other stuff",
            ..Default::default()
        },
    )
        .await?;
    Ok(())
}


/// See info for song by id
#[poise::command(slash_command)]
pub async fn song(
    ctx: Context<'_>,
    #[description = "Song id"] song_id: u32,
) -> Result<(), Error> {

    let mut conn = get_connection().await.unwrap();
    let song: Result<Song, _> = sqlx::query_as("SELECT song_id, song_name_eng, song_name_jap FROM songs WHERE song_id=?")
        .bind(song_id)
        .fetch_one(&mut conn).await;

    match song {
        Err(err) => {
            Err(Box::new(err))
        }
        Ok(song) => {
            let response = format!("song #{}: {} / {}", song.id, song.name_eng, song.name_jap);
            ctx.say(response).await?;
            Ok(())
        }
    }
}

///search songs to find the id
#[poise::command(slash_command)]
pub async fn find_song(
    ctx: Context<'_>,
    #[description="search keyword"] fragment: String
) -> Result<(), Error> {

    let wrapped_fragment = format!("%{fragment}%");

    let mut conn = get_connection().await?;
    let song: Result<Vec<Song>, _> = sqlx::query_as(
        "SELECT song_id, song_name_eng, song_name_jap FROM songs WHERE song_name_eng like ?\
         UNION \
         SELECT song_id, song_name_eng, song_name_jap FROM songs WHERE song_name_jap like ?"
    ).bind(wrapped_fragment.clone()).bind(wrapped_fragment)
        .fetch_all(&mut conn).await;

    match song {
        Err(err) => {
            Err(Box::new(err))
        }
        Ok(songs) => {
            let mut response: String = String::new();
            response.push_str(&*format!("### Results for {fragment}\n"));
            for song in songs {
                response.push_str(&*format!("song #{}: {} / {}\n", song.id, song.name_eng, song.name_jap));
            }

            ctx.say(response.strip_suffix("\n").unwrap()).await?;
            Ok(())
        }
    }
}

///get scoreboard of chart via song id and difficulty
#[poise::command(slash_command)]
pub async fn scoreboard(
    ctx: Context<'_>,
    #[description="song id. find via /find_song"] song_id: u32,
    #[description="difficulty"] level: Level,
) -> Result<(), Error> {
    let mut warnings : ElodonErrorList = ElodonErrorList::new();

    let mut conn = get_connection().await?;
    let plays: Result<Vec<Play>, _> = sqlx::query_as(
        "SELECT user_id, song_id, level_id, score FROM top_plays WHERE song_id=? AND level_id=?"
    ).bind(song_id).bind::<u32>(level.into())
        .fetch_all(&mut conn).await;

    let song: Song = match Song::from_id(&mut conn, song_id).await {
        Ok(song) => {
            song
        },
        Err(err) => {
            warnings.push(err);
            Song::placeholder()
        }
    };


    match plays {
        Err(err) => {
            warnings.push(ElodonError::DatabaseError(err));
        }
        Ok(mut plays) => {
            plays.sort_by_key(|play| -(play.score as i32));
            let mut response = String::new();

            response.push_str(&*format!("### Results for {} / {} ({:?}):\n```\n", song.name_eng, song.name_jap, level));
            for (i, play) in plays.iter().enumerate() {
                let ranking = format!("#{}", i+1);
                let potential_user: Result<User, ElodonError> = play.get_user(&mut conn).await;
                let user = match potential_user {
                    Ok(user) => {user},
                    Err(err) => {
                        warnings.push(err);
                        User::placeholder()
                    }
                };
                response.push_str(&*format!("{:>3}) {:>7} by {}\n", ranking, play.score, user.name));
            }
            response.strip_suffix("\n").unwrap();
            response.push_str("```");
            ctx.say(response).await?;
        }
    }

    if warnings.is_empty() {
        Ok(())
    } else {
        Err(Box::new(ElodonError::List(warnings)))
    }

}


async fn get_connection() -> Result<SqliteConnection, ElodonError>{
    Ok(SqliteConnection::connect("./res/taiko.db").await?)
}