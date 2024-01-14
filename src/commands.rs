use sqlx::{Connection, SqliteConnection};

use crate::error::ElodonError;

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

    let response = format!("#{}: {}", song.id, song.get_name()?);
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
        let name = match song.get_name() {
            Ok(name) => { name },
            Err(err) => {
                warnings.push(err);
                "???".to_string()
            }
        };
        response.push_str(&*format!("#{:<5}: {}\n", song.id, name));
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
    #[description="song id. find via /song"] song_id: u32,
    #[description="difficulty"] level: Level,
) -> Result<(), Error> {
    let mut response = String::new();
    let mut warnings = ElodonErrorList::new();
    let mut conn = get_connection().await?;

    let song: Song = ok_or_say_error!(ctx, Song::from_id(&mut conn, song_id));
    let chart_id = ChartId(song_id, level);
    let _chart: Chart = ok_or_say_error!(ctx, Chart::from_id(&mut conn, chart_id));
    let mut plays: Vec<Play> = ok_or_say_error!(ctx, chart_id.plays(&mut conn));

    response.push_str(&*format!("### Results for {} ({:?}):\n```\n", song, level));


    plays.sort_by_key(|play| -(play.score as i32));

    for (i, play) in plays.iter().enumerate() {
        let ranking = format!("#{}", i + 1);
        response.push_str(&*format!("{:>3}) {:>7} by ", ranking, play.score));
        match play.get_user(&mut conn).await {
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

async fn get_connection() -> Result<SqliteConnection, ElodonError>{
    Ok(SqliteConnection::connect("./../taiko.db").await?)
}
