#![warn(clippy::str_to_string)]
#![feature(async_closure)]

use crate::filters::SongFilter;
use crate::filters::Filterable;
use std::{
    env::var,
    sync::Arc,
    time::Duration,
};
use std::sync::OnceLock;

use poise::serenity_prelude as serenity;
use poise::serenity_prelude::FullEvent;
use sqlx::SqliteConnection;
use crate::commands::get_connection;

use crate::structs::Song;

mod commands;
mod structs;
mod error;
mod filters;
mod paginate;
mod elo;

static SONG_NAMES: OnceLock<Vec<Song>> = OnceLock::new();

// Types used by all command functions
type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, Error>;

// Custom user data passed to all command functions
pub struct Data {
    songs_autocomplete: Vec<String>
}

async fn on_error(error: poise::FrameworkError<'_, Data, Error>) {
    // This is our custom error handler
    // They are many errors that can occur, so we only handle the ones we want to customize
    // and forward the rest to the default handler
    match error {
        poise::FrameworkError::Setup { error, .. } => panic!("Failed to start bot: {:?}", error),
        poise::FrameworkError::Command { error, ctx, .. } => {
            println!("Error in command `{}`: {:?}", ctx.command().name, error,);
            ctx.say(format!("Error in command:\n{error}")).await;
        },
        error => {
            if let Err(e) = poise::builtins::on_error(error).await {
                println!("Error while handling error: {}", e)
            }
        }
    }

}

//noinspection RsUnresolvedReference
#[tokio::main]
async fn main() {
    env_logger::init();

    // FrameworkOptions contains all of poise's configuration option in one struct
    // Every option can be omitted to use its default value
    let options = poise::FrameworkOptions {
        commands: vec![
            commands::help(),
            commands::song(),
            commands::song_info(),
            commands::scores(),
            commands::player(),
            // commands::dev_register(),
        ],
        prefix_options: poise::PrefixFrameworkOptions {
            prefix: Some("~".to_string()),
            edit_tracker: Some(Arc::new(poise::EditTracker::for_timespan(
                Duration::from_secs(3600),
            ))),
            additional_prefixes: vec![],
            ..Default::default()
        },
        // The global error handler for all error cases that may occur
        on_error: |error| Box::pin(on_error(error)),
        // This code is run before every command
        pre_command: |ctx| {
            Box::pin(async move {
                //println!("Executing command {}...", ctx.command().qualified_name);
            })
        },
        // This code is run after a command if it was successful (returned Ok)
        post_command: |ctx| {
            Box::pin(async move {
                //println!("Executed command {}!", ctx.command().qualified_name);
            })
        },
        // Every command invocation must pass this check to continue execution
        command_check: Some(|ctx| {
            Box::pin(async move {
                if ctx.author().id == 123456789 {
                    return Ok(false);
                }
                Ok(true)
            })
        }),
        // Enforce command checks even for owners (enforced by default)
        // Set to true to bypass checks, which is useful for testing
        skip_checks_for_owners: false,
        event_handler: |ctx, event, _framework, _data| {
            Box::pin(async move {
                // println!(
                //     "Got an event in event handler: {:?}",
                //     event
                // );

                match event {
                    FullEvent::Message {new_message: msg} => {
                        if msg.content.to_ascii_lowercase().contains("elodon") && !msg.author.bot{
                            let _ = msg.react(ctx, '💜').await;
                        }
                    }
                    _ => {}
                }

                Ok(())
            })
        },
        ..Default::default()
    };

    let framework = poise::Framework::builder()
        .setup(move |ctx, _ready, framework| {
            Box::pin(async move {
                println!("Logged in as {}", _ready.user.name);
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                println!("slash commands registered");
                let song_names: Vec<String> = match get_connection().await {
                    Ok(mut conn) => {
                        match Song::fetch_all(&mut conn, SongFilter::new()).await {
                            Ok(songs) => {
                                let song_names: Vec<String> = songs.iter().map(|song: &Song| {
                                    format!("{}: {} > {}", song.id, song.genre(), song.get_name())
                                }).collect();
                                song_names
                            },
                            Err(_) => { vec![] }
                        }
                    }
                    Err(_) => { vec![] }
                };
                Ok(Data {
                    songs_autocomplete: song_names
                })
            })
        })
        .options(options)
        .build();

    let token = var("DISCORD_TOKEN")
        .expect("Missing `DISCORD_TOKEN` env var, see README for more information.");
    let intents =
        serenity::GatewayIntents::non_privileged() | serenity::GatewayIntents::MESSAGE_CONTENT;

    let client = serenity::ClientBuilder::new(token, intents)
        .framework(framework)
        .await;

    client.unwrap().start().await.unwrap()
}
