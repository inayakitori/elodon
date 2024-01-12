use std::fmt::{Display, Formatter};
use std::str::FromStr;
use poise::{async_trait, ChoiceParameter, CommandParameterChoice, create_slash_argument, SlashArgError, SlashArgument};
use poise::serenity_prelude::{ArgumentConvert, CacheHttp, ChannelId, CommandInteraction, CommandOptionType, CreateCommandOption, GuildId, ResolvedValue};
use crate::{Context, Error};

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
            extra_text_at_bottom: "This is an example bot made to showcase features of my custom Discord bot framework",
            ..Default::default()
        },
    )
        .await?;
    Ok(())
}


/// Vote for something
///
/// Enter `~vote pumpkin` to vote for pumpkins
#[poise::command(slash_command)]
pub async fn vote(
    ctx: Context<'_>,
    #[description = "What to vote for"] choice: VoteOption,
) -> Result<(), Error> {
    // Lock the Mutex in a block {} so the Mutex isn't locked across an await point
    let num_votes = {
        let mut hash_map = ctx.data().votes.lock().unwrap();
        let num_votes = hash_map.entry(choice.clone()).or_default();
        *num_votes += 1;
        *num_votes
    };

    let response = format!("Successfully voted for {0}. {0} now has {1} votes!", choice, num_votes);
    ctx.say(response).await?;
    Ok(())
}


#[derive(Clone, Eq, Hash, PartialEq)]
pub enum VoteOption{
    Cat,
    Dog,
    Bird
}

impl FromStr for VoteOption{
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "cat"  => Ok(VoteOption::Cat),
            "dog"  => Ok(VoteOption::Dog),
            "bird" => Ok(VoteOption::Bird),
            _ => Err(())
        }
    }
}

impl ChoiceParameter for VoteOption{
    fn list() -> Vec<CommandParameterChoice> {
        return vec![
            CommandParameterChoice{
                name: "cat".to_string(),
                localizations: Default::default(),
                __non_exhaustive: (),
            },
            CommandParameterChoice{
                name: "dog".to_string(),
                localizations: Default::default(),
                __non_exhaustive: (),
            },
            CommandParameterChoice{
                name: "bird".to_string(),
                localizations: Default::default(),
                __non_exhaustive: (),
            }
        ]
    }

    fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(VoteOption::Cat),
            1 => Some(VoteOption::Dog),
            2 => Some(VoteOption::Bird),
            _ => None
        }
    }

    fn from_name(name: &str) -> Option<Self> {
        name.parse::<VoteOption>().ok()
    }

    fn name(&self) -> &'static str {
        match self {
            VoteOption::Cat  => "Cat",
            VoteOption::Dog  => "Dog",
            VoteOption::Bird => "Bird",
        }
    }

    fn localized_name(&self, locale: &str) -> Option<&'static str> {
        None
    }
}

impl Display for VoteOption{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", match self {
            VoteOption::Cat => "cat",
            VoteOption::Dog => "dog",
            VoteOption::Bird => "bird"
        })
    }
}



/// Retrieve number of votes
///
/// Retrieve the number of votes either in general, or for a specific choice:
/// ```
/// ~getvotes
/// ~getvotes pumpkin
/// ```
#[poise::command(track_edits, aliases("votes"), slash_command)]
pub async fn getvotes(
    ctx: Context<'_>,
    #[description = "Choice to retrieve votes for"] choice: Option<VoteOption>,
) -> Result<(), Error> {
    if let Some(choice) = choice {
        let num_votes = *ctx.data().votes.lock().unwrap().get(&choice).unwrap_or(&0);
        let response = match num_votes {
            0 => format!("Nobody has voted for {} yet", choice),
            _ => format!("{} people have voted for {}", num_votes, choice),
        };
        ctx.say(response).await?;
    } else {
        let mut response = String::new();
        for (choice, num_votes) in ctx.data().votes.lock().unwrap().iter() {
            response += &format!("{}: {} votes", choice, num_votes);
        }

        if response.is_empty() {
            response += "Nobody has voted for anything yet :(";
        }

        ctx.say(response).await?;
    };

    Ok(())
}