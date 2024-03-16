use std::io::ErrorKind;
use std::num::{NonZeroU32, NonZeroU8};
use crate::{Data, elo, emoji};
use poise::builtins::create_application_commands;
use futures::Stream;
use futures::StreamExt;
use itertools::Itertools;
use noisy_float::types::{R32, r32};
use num_traits::real::Real;
use num_traits::Signed;
use poise::serenity_prelude as serenity;
use poise::serenity_prelude::{GuildId, Mentionable, UserId};
use sqlx::{Connection, Executor, Row, SqliteConnection};
use sqlx::sqlite::SqliteQueryResult;

use crate::error::{ElodonError, ElodonErrorList};
use crate::filters::*;
use crate::Error;
use crate::Context;
use crate::elo::get_predicted_score;
use crate::emoji::{COMBO_IDS, CROWN_IDS, JUDGEMENT_IDS, RANK_IDS, ROLLS_IDS};
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
        .filter(move |name| futures::future::ready(name.to_ascii_lowercase().contains(&partial.to_ascii_lowercase())))
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
    level_input: Option<Level>,
    #[description="should good/ok/bad be included or not"]
    detailed_input: Option<bool>,
    #[description="exclude self estimates"]
    exclude_estimates: Option<bool>
) -> Result<(), Error> {
    let mut response = String::new();
    let mut warnings = ElodonErrorList::new();
    let mut conn = get_connection().await?;

    let detailed = detailed_input.unwrap_or(true);
    ctx.defer().await?;

    let author_user = User::fetch_one(&mut conn, UserFilter{
        discord_id: Some(ctx.author().id),
        user_id: None
    }).await.ok();

    let song_id: u32 = extract_song_id(song)?;

    let mut level = level_input.unwrap_or(Level::Ura);

    let (mut filter, song, chart, mut plays) = loop {

        let mut filter = GeneralFilter::new()
            .song_id(Some(song_id))
            .level(Some(level));

        let song = ok_or_say_error!(ctx,
            Song::fetch_one(&mut conn, filter)
        );

        let chart: Chart = ok_or_say_error!(ctx,
            Chart::fetch_one(&mut conn, filter)
        );
        let mut plays: Vec<(Option<u32>, Play)> = ok_or_say_error!(ctx,
            Play::fetch_all(&mut conn, filter)
        ).iter()
                .copied()
                .sorted_by_key(|play| -(play.score as i32))
                .enumerate()
                .map(|(index, play)| (Some(index as u32), play))
                .collect();

        if plays.is_empty() && level != Level::Easy{
            level = level.decrease().unwrap(); //is fine b/c is not easy
        } else {
            break (filter, song, chart, plays);
        }

    };

    let server_players: Vec<u64> = ctx.guild()
        .ok_or(ElodonError::NoGuild)?
        .members.keys()
        .map(|user_id| user_id.get())
        .collect();

    if exclude_estimates.filter(|exclude_estimates| !exclude_estimates).is_some(){
        if let Some(ref caller) = author_user{
            if let Some(sd) = chart.sd_mean {
                // get approximations
                const Z_SCORES: [f32; 4] = [-1., 0., 1., 2.];
                let average_score = get_predicted_score(caller.elo(level.into()), &chart, 0.).unwrap();
                for z in Z_SCORES {
                    let estimated_play: Play = Play {
                        score: (average_score as f32 + sd * z) as u32,
                        user: 0,
                        level: u32::from(level),
                        song: song_id,
                        crown: 0,
                        good_cnt: 0,
                        ok_cnt: 0,
                        bad_cnt: 0,
                        roll_cnt: 0,
                        combo_cnt: 0,
                        rank: 0
                    };
                    plays.push((None, estimated_play));
                }
            }
        }
    }

    plays.sort_by_key(|(_, play)| -(play.score as i32));

    let info_filter = filter.user_id(None).discord_id(None);

    let header = &*format!("### Results for {} ({:?}):\n", song, level);


    for (index, play) in plays {
        let ranking = match index {
            None => {format!("")}
            Some(index) => { format!("#{})", index + 1) }
        };
        let user = match play.fetch_one_other::<User>(&mut conn).await {
            Ok(user) => {
                user
            }
            Err(_) => {//is a generated play
                match author_user {
                    Some(ref author_user) => {
                        User{
                            name: format!("{} (estimated)", author_user.name),
                            ..author_user.clone()
                        }
                    }
                    None => {
                        panic!("expected user to exist")
                    }
                }
            }
        };

        // don't include users not in this server
        if !server_players.contains(&(user.discord as u64)) {continue}

        match detailed{
            true => {
                let z_value_txt = match elo::get_z_value(play.score, user.elo(level.into()), &chart, 1f32) {
                    Some(z_value) => format!("{:+.1}", z_value),
                    None => "????".to_string()
                };

                let ur = elo::get_sd(25., play.good_cnt as f64/ (play.good_cnt + play.ok_cnt + 1) as f64);

                let crown_emoji = format!("<:crown_{}:{}>", play.crown, CROWN_IDS.get(play.crown as usize).expect("invalid crown id"));
                let rank_emoji = if play.rank < 2 {String::new()} else {
                    format!("<:rank_{}:{}>", play.rank, RANK_IDS.get(play.rank as usize).expect("invalid rank id"))
                };
                let good_emoji = format!("<:good:{}>", JUDGEMENT_IDS[3]);
                let ok_emoji = format!("<:ok:{}>", JUDGEMENT_IDS[2]);
                let bad_emoji = format!("<:bad_0:{}><:bad_1:{}>", JUDGEMENT_IDS[0], JUDGEMENT_IDS[1]);
                let combo_emoji = format!("<:combo_0:{}><:combo_1:{}>", COMBO_IDS[0], COMBO_IDS[1]);
                let rolls_emoji = format!("<:rolls_0:{}><:rolls_1:{}><:rolls_2:{}>", ROLLS_IDS[0], ROLLS_IDS[1], ROLLS_IDS[2]);
                response.push_str(&*format!("`{:>4}` `{}` **{}**\n⮱`{:>7}` `{:>4}`{} {} {}\t σ<`{:+>3.1}ms`\n⮱`{:>4}`{} `{:>3}`{}` {:>3}`{} `{:>3}`{}\n",
                                            ranking, z_value_txt, user.name,
                                            play.score,
                                            play.combo_cnt, combo_emoji,
                                            crown_emoji, rank_emoji, ur,
                                            play.good_cnt, good_emoji,
                                            play.ok_cnt, ok_emoji,
                                            play.bad_cnt, bad_emoji,
                                            play.roll_cnt, rolls_emoji
                ));
            }
            false => {
                response.push_str(&*format!("`{:>4} {:>7}` by {}\n",
                    ranking, play.score, user.name
                ))
            }
        }
    }

    let pages_owned: Vec<String> = response.split("\n")
        .chunks(27)
        .into_iter()
        .map(|mut lines|{
            lines.join("\n")
        })
        .filter(|lines| !lines.is_empty())
        .collect();


    let pages: Vec<&str> = pages_owned.iter().map(|s| &**s).collect();

    paginate::<Data, Error>(ctx, &header, &*pages).await?;

    if !warnings.is_empty() {
        return_err!(ElodonError::List(warnings))
    } else {
        Ok(())
    }
}

fn extract_song_id(song: String) -> Result<u32, ElodonError> {
    song.split(":")
        .next().ok_or(ElodonError::ParseError(song.clone()))?
        .parse().map_err(|_| ElodonError::ParseError(song.clone()))
}

///get a player via discord id
#[poise::command(track_edits, slash_command)]
pub async fn player(
    ctx: Context<'_>,
    #[description="discord (by default self)"] discord_user_input: Option<UserId>,
    level: Option<DisplayLevel>,
) -> Result<(), Error> {

    ctx.defer().await?;

    let discord_user = discord_user_input.unwrap_or(ctx.author().id);
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
        if let Some(ranked_play) = get_play_info(&mut conn, user.elo(play.level().into()), &play).await {
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
            response.push_str("No plays found. Player has no ELO\n");
        } else {
            for (z, play, genre, chart_name) in ranked_plays.iter().take(5) {
                let z_value: f32 = z.raw();
                response.push_str(&*format!("{:+.2}. {:>7} on {}\n", z_value, play.score, chart_name));
            }
        }

        response.push_str(&*format!("### Filtered plays ({}/{})\n\n```",
                                    i+1, page_count));
        for play in plays {
            match chart_name(&mut conn, play).await{
                Ok(chart_name) => {
                    response.push_str(&*format!("{:>7} on {}\n", play.score, chart_name));
                }
                Err(_) => {
                    response.push_str(&*format!("{:>7} on song_id={}\n", play.score, play.song));}
            }
        }
        response.push_str("```");
        pages_owned.push(response)
    }

    let pages: Vec<&str> = pages_owned.iter().map(|s| &**s).collect();
    paginate::<Data, Error>(ctx, &"", &pages).await;

    Ok(())
}

//suggest new maps for player
#[poise::command(track_edits, slash_command)]
pub async fn suggest(
    ctx: Context<'_>,
    #[description="the desired score in ks e.g miyabi = 1000"] score_k: u32,
    #[description="the desired z value"] z_input: Option<f32>,
    level_input: Option<DisplayLevel>,
    #[description="discord (by default self)"] discord_user_input: Option<UserId>,
    dev_info_input: Option<bool>
) -> Result<(), Error> {
    let score = score_k * 1000;
    let discord_user = discord_user_input.unwrap_or(ctx.author().id);
    let level = level_input.unwrap_or(DisplayLevel::OniPlus);
    let desired_z = z_input.unwrap_or(0.);
    let dev_info = dev_info_input.unwrap_or(false);

    let mut conn = get_connection().await?;
    let mut filter = GeneralFilter::new()
        .discord_id(Some(discord_user.clone()))
        .display_level(Some(level));
    let user: User = ok_or_say_error!(ctx,
        User::fetch_one(&mut conn, filter)
    );
    filter.set_user_id(Some(user.id));

    let charts: Vec<Chart> = ok_or_say_error!(ctx,
        Chart::fetch_all(&mut conn, filter)
    );

    let matching_charts: Vec<(R32, R32, &Chart)> = charts.iter().filter_map(|chart| {

        if chart.score_slope? < 0 {return None}

        let (z_lower, z_upper) = lower_to_higher(
            R32::try_new(elo::get_z_value(score, user.elo(level), chart, 1f32)?)?,
            R32::try_new(elo::get_z_value(score, user.elo(level), chart,-1f32)?)?
        );

        Some((z_lower, z_upper, chart))
    }).sorted_by_key(|(z_lower, z_upper, chart)| {
        (*z_lower - desired_z).abs() + (*z_upper - desired_z).abs()
    }).take(8).collect();

    dbg!(matching_charts.clone());

    let mut matching_songs: Vec<(R32, R32, Song, Level)> = Vec::with_capacity(matching_charts.len());

    for (z_lower, z_upper, chart) in matching_charts {

        let song = ok_or_say_error!(ctx,
            Song::fetch_one(&mut conn, *chart)
        );

        matching_songs.push(
            (z_lower, z_upper, song, chart.level())
        );
    }

    let mut response_text = format!("### Songs that <@{}> has a 70% chance of getting a score of {} with a z value of as least {}:\n",
                                    user.discord_id(), score,  desired_z);

    let results_text = matching_songs.iter().map(|(z_lower, z_upper, song, level)| {
        match dev_info {
            true => {
                format!("`{:+.2} to {:+.2}` {} ({})", z_lower, z_upper, song.get_name(), level)
            }
            false => {
                format!("{} ({})", song.get_name(), level)
            }
        }
    }).join("\n");

    if results_text.is_empty() {
         response_text.push_str("No results found");
    } else {
        response_text.push_str(&results_text);
    }

    ctx.send(poise::CreateReply::default()
        .embed(serenity::CreateEmbed::default().description(&response_text))
    ).await?;

    Ok(())
}

fn lower_to_higher(a: R32, b: R32) -> (R32, R32) {
    return (
        a.min(b),
        a.max(b)
        )
}

async fn chart_name(conn: &mut SqliteConnection, play: &Play) -> Result<String, ElodonError>{
    let chart = play.fetch_one_other::<Chart>(conn).await?;
    let chart_name = chart.full_name(conn).await?;
    return Ok(chart_name);
}


#[poise::command(slash_command)]
pub async fn register(
    ctx: Context<'_>,
    donder_id: i64
) -> Result<(), Error> {
    let mut conn = get_connection().await?;
     match sqlx::query("INSERT INTO users (user_id, discord_id, user_name) VALUES (?,?,?);")
         .bind(donder_id)
         .bind(ctx.author().id.get() as i64)
         .bind("temp_name")
         .execute(&mut conn).await {
         Ok(_) => {
             ctx.say("Adding user. data will be retrieved on the next scrape").await?;
         }
         Err(err) => {
             return_err!(
                 ElodonError::DatabaseError(err)
             )
         }
     }
    Ok(())
}

///DEV USE. refreshed slash commands

#[poise::command(slash_command, subcommands("kill", "sql", "register_commands"), owners_only)]
pub async fn dev(
    ctx: Context<'_>
) -> Result<(), Error> { Ok(()) }


#[poise::command(slash_command, owners_only)]
pub async fn kill(
    ctx: Context<'_>
) -> Result<(), Error> {
    return_err!(
    ElodonError::Shutdown(std::io::Error::new(ErrorKind::Interrupted, "Manual shutdown"))
    )
}

#[poise::command(prefix_command, owners_only)]
pub async fn register_commands(
    ctx: Context<'_>
) -> Result<(), Error> {
    poise::builtins::register_application_commands_buttons(ctx).await?;
    Ok(())
}
#[poise::command(slash_command, subcommands("execute", "fetch"), owners_only)]
pub async fn sql(
    ctx: Context<'_>
) -> Result<(), Error> { Ok(()) }
#[poise::command(slash_command, owners_only)]
pub async fn execute(
    ctx: Context<'_>,
    execute: String,
) -> Result<(), Error> {
    let mut conn = get_connection().await?;
    match conn.execute(&*execute).await {
        Ok(response) => {
            ctx.say(format!("> {execute}\n Modified {} rows", response.rows_affected())).await?;
        }
        Err(db_err) => {
            return_err!(
                ElodonError::DatabaseError(db_err)
            )
        }
    }
    Ok(())
}


#[poise::command(track_edits, slash_command, owners_only)]
pub async fn fetch(
    ctx: Context<'_>,
    #[description= "What to search for"]
    table: FilterType,
    #[description= "Donder id"]
    donder: Option<i64>,
    #[description= "User's Discord"]
    discord: Option<UserId>,
    #[description= "Song"]
    #[autocomplete = "autocomplete_song"]
    song: Option<String>,
    #[description= "Easy to Ura"]
    chart_level: Option<Level>,
    #[description= "Easy to Oni+"]
    display_level: Option<DisplayLevel>,
    #[description= "Genre"]
    genre: Option<Genre>
) -> Result<(), Error> {

    let song_id: Option<u32> = match song {
        None => None,
        Some(song) => Some(extract_song_id(song)?)
    };

    let mut conn = get_connection().await?;

    let filter = GeneralFilter{
        user_id: donder,
        discord_id: discord,
        song_id,
        level: chart_level,
        display_level,
        genre,
    };

    let response: String = match table{
        FilterType::User => {
            User::fetch_all(&mut conn, filter).await?.get_display_text()
        }
        FilterType::Song => {
            Song::fetch_all(&mut conn, filter).await?.get_display_text()
        }
        FilterType::Chart => {
            Chart::fetch_all(&mut conn, filter).await?.get_display_text()
        }
        FilterType::Play => {
            Play::fetch_all(&mut conn, filter).await?.get_display_text()
        }
    };

    let pages_owned: Vec<String> = response.split("\n")
        .chunks(18)
        .into_iter()
        .map(|mut lines|{
            lines.join("\n")
        })
        .filter(|lines| !lines.is_empty())
        .collect();


    let pages: Vec<&str> = pages_owned.iter().map(|s| &**s).collect();

    paginate::<Data, Error>(ctx, &"mrrp", &*pages).await?;

    Ok(())
}



async fn get_play_info<'a>(conn: &mut SqliteConnection, elo: Option<f32>, play: &'a Play) -> Option<(R32, &'a Play, Genre, String)>{
    let chart = play.fetch_one_other::<Chart>(conn).await.ok()?;
    let song = play.fetch_one_other::<Song>(conn).await.ok()?;
    let chart_name = chart.full_name(conn).await.ok()?;
    let z_value = R32::try_new(elo::get_z_value(play.score, elo, &chart, 1f32)?)?;
    Some((z_value, &play, song.genre(), chart_name))
}


pub(crate) async fn get_connection() -> Result<SqliteConnection, ElodonError>{
    Ok(SqliteConnection::connect("./../taiko.db").await?)
}
