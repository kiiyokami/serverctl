use crate::{reply, values, Context, Data, Error};
use poise::serenity_prelude as serenity;
use std::collections::HashSet;
use std::sync::Mutex;
use std::time::Duration;

pub mod create;
pub mod curseforge_key;
pub mod delete;
pub mod list;
pub mod mods;
pub mod settings;
pub mod start;
pub mod status;
pub mod stop;
pub mod ttl;

/// Autocomplete for server-name options: this guild's servers starting with what's typed.
pub async fn server_names(ctx: Context<'_>, partial: &str) -> Vec<String> {
    let Some(guild) = ctx.guild_id().map(|g| g.to_string()) else {
        return Vec::new();
    };
    let mut names: Vec<String> = values::all()
        .into_iter()
        .filter(|v| v.discord_guild_id == guild && v.name.starts_with(partial))
        .map(|v| v.name)
        .collect();
    names.sort();
    names.truncate(25);
    names
}

/// Messages whose buttons a running command is still waiting on.
pub type LivePrompts = Mutex<HashSet<serenity::MessageId>>;

/// Keeps a message's buttons live until dropped.
pub struct LivePrompt<'a>(&'a LivePrompts, serenity::MessageId);

impl<'a> LivePrompt<'a> {
    pub fn new(prompts: &'a LivePrompts, msg: serenity::MessageId) -> Self {
        prompts.lock().unwrap().insert(msg);
        Self(prompts, msg)
    }
}

impl Drop for LivePrompt<'_> {
    fn drop(&mut self) {
        self.0.lock().unwrap().remove(&self.1);
    }
}

/// Next click on `msg` by the command's author; anyone else gets a private "not yours".
/// `None` once `timeout` passes with no click from the author.
pub async fn next_click(
    ctx: Context<'_>,
    msg: serenity::MessageId,
    timeout: Duration,
) -> Result<Option<serenity::ComponentInteraction>, Error> {
    let author = ctx.author().id;
    loop {
        let Some(click) = serenity::ComponentInteractionCollector::new(&ctx.serenity_context().shard)
            .message_id(msg)
            .timeout(timeout)
            .await
        else {
            return Ok(None);
        };
        if click.user.id == author {
            return Ok(Some(click));
        }
        click
            .create_response(ctx.http(), reply::private(format!("Only <@{author}> can use these buttons.")))
            .await?;
    }
}

/// Answers clicks on buttons nothing is waiting for, e.g. a panel left open across a bot restart.
pub async fn expire_stale_click(
    ctx: &serenity::Context,
    event: &serenity::FullEvent,
    data: &Data,
) -> Result<(), Error> {
    let serenity::FullEvent::InteractionCreate { interaction: serenity::Interaction::Component(click) } = event
    else {
        return Ok(());
    };
    if data.live_prompts.lock().unwrap().contains(&click.message.id) {
        return Ok(());
    }
    click
        .create_response(ctx, reply::private("These buttons expired. Run the command again."))
        .await?;
    Ok(())
}
