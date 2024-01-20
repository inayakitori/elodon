use crate::{Data, elo};
use poise::builtins::create_application_commands;
use futures::Stream;
use futures::StreamExt;
use noisy_float::types::{R32, r32};
use poise::serenity_prelude as serenity;
use poise::serenity_prelude::{GuildId, Mentionable, UserId};
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

    ctx.defer().await?;

    let song_id: u32 = song.split(":")
        .next().ok_or(ElodonError::ParseError(song.clone()))?
        .parse().map_err(|_| ElodonError::ParseError(song.clone()))?;

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
        let user = ok_or_say_error!(ctx, play.fetch_one_other::<User>(&mut conn));
        let z_value_txt = match elo::get_z_value(play.score, user.elo(level), chart) {
            Some(z_value) => format!("{:+.2}", z_value),
            None => "?????".to_string()
        };
        response.push_str(&*format!("{:>3}) {} {:>7} by {}",
                                    ranking, z_value_txt, play.score, user.name));
        response.push_str("\n");
    }
    response.push_str("```\n");

    ctx.send(poise::CreateReply::default()
        .embed(serenity::CreateEmbed::default().description(&response))
    ).await?;


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
    level: Option<DisplayLevel>,
) -> Result<(), Error> {

    ctx.defer().await?;

    let mut conn = get_connection().await?;
    let mut filter = GeneralFilter::new()
        .discord_id(Some(discord_user.clone()))
        .display_level(level);

    let user: User = ok_or_say_error!(ctx,
        User::fetch_one(&mut conn, filter)
    );
    filter.set_user_id(Some(user.id));
    let songs: Vec<Song> = ok_or_say_error!(ctx,
        Song::fetch_all(&mut conn, filter)
    );
    let plays: Vec<Play> = ok_or_say_error!(ctx,
        Play::fetch_all(&mut conn, filter)
    );
    let discord_user = discord_user.to_user(ctx).await?;

    let info_filter = filter.discord_id(None).user_id(None);


    let mut ranked_plays: Vec<(R32, &Play, Genre, String)> = vec![];
    for play in &plays {
        if let Some(ranked_play) = get_play_info(&mut conn, user.elo(play.level()), &play).await {
            match level {
                Some(level) if (level != ranked_play.1.level().into()) => continue,
                _ => {}
            }
            ranked_plays.push(ranked_play);
        }
    }
    ranked_plays.sort_by_key(|(z,_,_,_)| *z * r32(-1f32));

    let mut pages_owned: Vec<String> = vec![];
    let page_count = 1 + (plays.len() / 10);
    for (i, plays) in plays.chunks(10).enumerate(){
        let mut response: String = format!("## User <@{}> ({})\n Showing plays{}.\n### Most notable plays\n",
                                         discord_user.id, user.name, info_filter);

        if ranked_plays.is_empty() {
            response.push_str("No plays found. Player has no ELO");
        } else {
            response.push_str("```\n");
            for (z, play, genre, chart_name) in ranked_plays.iter().take(5) {
                let z_value: f32 = z.raw();
                response.push_str(&*format!("{:+.2}. {:>7} on {}\n", z_value, play.score, chart_name));
            }
            response.push_str("```\n");
        }

        response.push_str(&*format!("### Filtered plays ({}/{})\n\n```",
                                    i+1, page_count));
        for play in plays {
            let chart = ok_or_say_error!(ctx, play.fetch_one_other::<Chart>(&mut conn));
            let chart_name = ok_or_say_error!(ctx, chart.full_name(&mut conn));
            response.push_str(&*format!("{:>7} on {}\n", play.score, chart_name));
        }
        response.push_str("```");
        pages_owned.push(response)
    }

    let pages: Vec<&str> = pages_owned.iter().map(|s| &**s).collect();
    paginate::<Data, Error>(ctx, &"", &pages).await;

    Ok(())
}

async fn get_play_info<'a>(conn: &mut SqliteConnection, elo: Option<f32>, play: &'a Play) -> Option<(R32, &'a Play, Genre, String)>{
    let chart = play.fetch_one_other::<Chart>(conn).await.ok()?;
    let song = play.fetch_one_other::<Song>(conn).await.ok()?;
    let chart_name = chart.full_name(conn).await.ok()?;
    let genre = song.genre();
    let z_value = R32::try_new(elo::get_z_value(play.score, elo, chart)?)?;
    Some((z_value, &play, song.genre(), chart_name))
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
