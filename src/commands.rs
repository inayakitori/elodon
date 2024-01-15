use crate::Data;
use poise::builtins::create_application_commands;
use futures::Stream;
use futures::StreamExt;
use poise::serenity_prelude::{GuildId, UserId};
use sqlx::{Connection, SqliteConnection};

use crate::error::{ElodonError, ElodonErrorList};
use crate::filters::*;
use crate::Error;
use crate::Context;
use crate::structs::*;
use crate::paginate::paginate;

macro_rules! return_err {
    ($err:expr) => {
        return Err(Box::new($err))
    };
}

macro_rules! ok_or_say_error {
    ($ctx: ident, $search: expr) => {
        match $search.await {
            Ok(value) => {
                value
            },
            Err(err @ ElodonError::NoResults {..}) => {
                $ctx.say(err).await?;
                return Ok(());
            },
            Err(err) => {
                return_err!(err)
            }
        }
    };
}

async fn autocomplete_song<'a>(
    ctx: Context<'a>,
    partial: &'a str,
) -> impl Stream<Item = String> + 'a {
    futures::stream::iter(&ctx.data().songs_autocomplete[..])
        .filter(move |name| futures::future::ready(name.to_ascii_lowercase().contains(partial)))
        .map(|name| name.to_string())
}

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
pub async fn song_info(
    ctx: Context<'_>,
    #[description = "Song id"] song_id: u32,
) -> Result<(), Error> {

    let mut conn = get_connection().await?;
    let song: Song = match sqlx::query_as("SELECT song_id, song_name_eng, song_name_jap, genre_id FROM songs WHERE song_id=?")
        .bind(song_id)
        .fetch_one(&mut conn)
        .await {
            Err(err) => { return_err!(err) }
            Ok(song) => { song }
        };

    let response = format!("#{}: {}", song.id, song.get_name());
    ctx.say(response).await?;
    Ok(())

}

///search songs to find the id
#[poise::command(slash_command)]
pub async fn song(
    ctx: Context<'_>,
    #[description="search keyphrase"] search: String,
    #[description="genre "] genre: Option<Genre>
) -> Result<(), Error> {

    let mut conn = get_connection().await?;
    let songs: Vec<Song> = ok_or_say_error!(
        ctx,
        Song::get_matching(&mut conn, &search, genre)
    );

    let mut warnings = ElodonErrorList::new();
    let mut response: String = String::new();
    match genre {
        Some(genre) => response.push_str(&*format!("### Results for \"{search}\" in {genre}:\n```\n")),
        None => response.push_str(&*format!("### Results for \"{search}\":\n```\n"))
    };

    for song in songs {
        response.push_str(&*format!("#{:<5}: {} > {}\n", song.id, song.genre(), song.get_name()));
    }
    response.push_str("```");
    ctx.say(response).await?;

    if !warnings.is_empty() {
        return_err!(ElodonError::List(warnings))
    } else {
        Ok(())
    }
}

///get scoreboard of chart via song id and difficulty
#[poise::command(slash_command)]
pub async fn scores(
    ctx: Context<'_>,
    #[autocomplete = "autocomplete_song"]
    #[description="song"]
    song: String,
    #[description="difficulty"]
    level: Level,
) -> Result<(), Error> {
    let mut response = String::new();
    let mut warnings = ElodonErrorList::new();
    let mut conn = get_connection().await?;

    let song_id: u32 = song.split(":")
        .next().ok_or(ElodonError::ParseError(songs))?
        .parse().map_err(ElodonError::ParseError(songs))?;

    let mut filter = GeneralFilter::new()
        .song_id(Some(song_id))
        .level(Some(level));

    let song = ok_or_say_error!(ctx,
        Song::fetch_one(&mut conn, filter)
    );
    let chart: Chart = ok_or_say_error!(ctx,
        Chart::fetch_one(&mut conn, filter)
    );
    let mut plays: Vec<Play> = ok_or_say_error!(ctx,
        Play::fetch_all(&mut conn, filter)
    );

    let info_filter = filter.user_id(None).discord_id(None);

    response.push_str(&*format!("### Results for {} ({:?}):\n```\n", song, level));

    plays.sort_by_key(|play| -(play.score as i32));

    for (i, play) in plays.iter().enumerate() {
        let ranking = format!("#{}", i + 1);
        response.push_str(&*format!("{:>3}) {:>7} by ", ranking, play.score));
        match play.fetch_one_other::<User>(&mut conn).await {
            Ok(user) => {
                response.push_str(&*format!("{}\n" ,user.name));
            },
            Err(err) => {
                warnings.push(err);
                response.push_str("[User not found: {}]\n");
            }
        };
    }

    response.push_str("```");
    ctx.say(response).await?;


    if !warnings.is_empty() {
        return_err!(ElodonError::List(warnings))
    } else {
        Ok(())
    }
}

///get a player via discord id
#[poise::command(track_edits, slash_command)]
pub async fn player(
    ctx: Context<'_>,
    #[description="discord"] discord_user: UserId,
    level: Option<Level>,
    genre: Option<Genre>,
) -> Result<(), Error> {

    ctx.defer().await?;

    let mut conn = get_connection().await?;
    let mut filter = GeneralFilter::new()
        .discord_id(Some(discord_user.clone()))
        .level(level)
        .genre(genre);

    let user: User = ok_or_say_error!(ctx,
        User::fetch_one(&mut conn, filter)
    );
    filter.set_user_id(Some(user.id));
    let songs: Vec<Song> = ok_or_say_error!(ctx,
        Song::fetch_all(&mut conn, filter)
    );
    let mut plays: Vec<Play> = ok_or_say_error!(ctx,
        Play::fetch_all(&mut conn, filter)
    );

    let info_filter = filter.discord_id(None).user_id(None);

    let discord_user = discord_user.to_user(ctx).await?;
    let mut pages_owned: Vec<String> = vec![];
    let page_count = 1 + (plays.len() / 10);
    for (i, plays) in plays.chunks(10).enumerate(){
        let mut response = String::new();
        response.push_str(&*format!("### Plays for user @{} with {}, {}/{}:\n\n",
                                    discord_user.name, info_filter, i+1, page_count));
        for play in plays {
            let chart = ok_or_say_error!(ctx, play.fetch_one_other::<Chart>(&mut conn));
            let chart_name = ok_or_say_error!(ctx, chart.full_name(&mut conn));
            response.push_str(&*format!("{:>7} on {}\n", play.score, chart_name));
        }
        pages_owned.push(response)
    }

    let pages: Vec<&str> = pages_owned.iter().map(|s| &**s).collect();
    paginate::<Data, Error>(ctx, &pages).await;

    Ok(())
}

///DEV USE. refreshed slash commands
#[poise::command(prefix_command)]
pub async fn dev_register(
    ctx: Context<'_>
) -> Result<(), Error> {
    poise::builtins::register_application_commands_buttons(ctx).await?;
    Ok(())
}


pub(crate) async fn get_connection() -> Result<SqliteConnection, ElodonError>{
    Ok(SqliteConnection::connect("./../taiko.db").await?)
}
