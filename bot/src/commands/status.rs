use crate::{kube as k, Context, Error};

#[poise::command(slash_command)]
pub async fn status(
    ctx: Context<'_>,
    #[description = "Server name"] name: String,
) -> Result<(), Error> {
    let guild = match ctx.guild_id() {
        Some(g) => g.to_string(),
        None => {
            ctx.say("Run this in a server.").await?;
            return Ok(());
        }
    };
    let client = k::client().await?;
    let dep = match k::get_deployment(&client, &name).await? {
        Some(d) => d,
        None => {
            ctx.say(format!("`{name}` not found.")).await?;
            return Ok(());
        }
    };
    if k::guild_id(&dep).as_deref() != Some(&guild) {
        ctx.say(format!("`{name}` isn't managed by this guild."))
            .await?;
        return Ok(());
    }
    let r = k::replicas(&dep);
    let state = if r > 0 { "running" } else { "stopped" };
    ctx.say(format!("`{name}`: **{state}** (replicas={r})"))
        .await?;
    Ok(())
}
