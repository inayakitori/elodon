use futures::{Stream, StreamExt};
use std::fmt::Write as _;

use poise::serenity_prelude as serenity;
use poise::serenity_prelude::{ButtonStyle, EmojiId, ReactionType};

use crate::{Context, emoji, Error};


pub async fn paginate<U, E>(
    ctx: Context<'_>,
    header: &str,
    pages: &[&str],
) -> Result<(), serenity::Error> {
    // Define some unique identifiers for the navigation buttons
    let ctx_id = ctx.id();
    let prev_button_id = format!("{}prev", ctx_id);
    let next_button_id = format!("{}next", ctx_id);

    // Send the embed with the first page as content
    let reply = {

        let mut reply = poise::CreateReply::default()
            .content(header);

        if pages.len() > 1 {
            let components = serenity::CreateActionRow::Buttons(
                vec![
                    serenity::CreateButton::new(&prev_button_id)
                        .style(ButtonStyle::Primary)
                        .emoji(emoji::LEFT),
                    serenity::CreateButton::new(&next_button_id)
                        .style(ButtonStyle::Primary)
                        .emoji(emoji::RIGHT),
                ]
            );
            reply = reply.components(vec![components]);
        }

        if pages.len() > 0 {
            reply = reply.embed(serenity::CreateEmbed::default().description(pages[0]));
        } else {
            reply.content.as_mut().unwrap().push_str("\n No results :c");
        }

        reply
    };

    let _reply_handle = ctx.send(reply).await?;

    // Loop through incoming interactions with the navigation buttons
    let mut current_page = 0;
    while let Some(press) = serenity::collector::ComponentInteractionCollector::new(ctx)
        // We defined our button IDs to start with `ctx_id`. If they don't, some other command's
        // button was pressed
        .filter(move |press| press.data.custom_id.starts_with(&ctx_id.to_string()))
        // Timeout when no navigation button has been pressed for 24 hours
        .timeout(std::time::Duration::from_secs(3600 * 24))
        .await
    {
        if *ctx.author() == press.user {
            // Depending on which button was pressed, go to next or previous page
            let id = &press.data.custom_id;
            match id {
                _ if id == &next_button_id => {
                    current_page = (current_page + 1) % pages.len();
                },
                _ if id == &prev_button_id  => {
                    current_page = current_page.checked_sub(1).unwrap_or(pages.len() - 1);
                },
                _ => {}
            }
        }

        // Update the message with the new page contents
        press
            .create_response(
                ctx.serenity_context(),
                serenity::CreateInteractionResponse::UpdateMessage(
                    serenity::CreateInteractionResponseMessage::new()
                        .content(header)
                        .embed(serenity::CreateEmbed::new().description(pages[current_page])),
                ),
            )
            .await?;
    }

    Ok(())
}