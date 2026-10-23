use crate::{auth, kube as k, reply, Context, Error};

#[poise::command(slash_command, guild_only)]
pub async fn ttl(
    ctx: Context<'_>,
    #[description = "Server name"]
    #[autocomplete = "super::server_names"]
    name: String,
    #[description = "Minutes before idle server shuts down (0 = never)"] minutes: u32,
) -> Result<(), Error> {
    ctx.defer().await?;
    let guild = ctx.guild_id().map(|g| g.to_string()).unwrap_or_default();
    let client = ctx.data().kube.clone();
    if !auth::guild_owns(&client, &guild, &name).await? {
        ctx.send(reply::unknown_server(&name)).await?;
        return Ok(());
    }
    // The timeout lives on the Deployment, which doesn't exist until the first start.
    if k::get_deployment(&client, &name).await?.is_none() {
        ctx.send(reply::err(format!(
            "`{name}` hasn't been started yet. Run `/start {name}` once, then set its idle timeout."
        )))
        .await?;
        return Ok(());
    }
    let seconds = minutes.saturating_mul(60);
    k::patch_ttl(&client, &name, seconds).await?;
    let msg = match minutes {
        0 => format!("⏱️ `{name}` will never shut down on its own."),
        1 => format!("⏱️ `{name}` will shut down after 1 minute with no players."),
        n => format!("⏱️ `{name}` will shut down after {n} minutes with no players."),
    };
    ctx.send(reply::ok(msg)).await?;
    Ok(())
}
