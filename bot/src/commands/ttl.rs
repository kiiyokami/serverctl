use crate::{auth, kube as k, reply, Context, Error};

#[poise::command(slash_command)]
pub async fn ttl(
    ctx: Context<'_>,
    #[description = "Server name"] name: String,
    #[description = "Minutes before idle server shuts down (0 = never)"] minutes: u32,
) -> Result<(), Error> {
    ctx.defer().await?;
    let guild = ctx.guild_id().map(|g| g.to_string()).unwrap_or_default();
    let client = k::client().await?;
    if !auth::guild_owns(&client, &guild, &name).await? {
        ctx.send(reply::err(format!("`{name}` isn't managed by this guild.")))
            .await?;
        return Ok(());
    }
    let seconds = minutes * 60;
    k::patch_ttl(&client, &name, seconds).await?;
    if minutes == 0 {
        ctx.send(reply::ok(format!("⏱️ `{name}` will never auto-shutdown.")))
            .await?;
    } else {
        ctx.send(reply::ok(format!(
            "⏱️ `{name}` will shut down after {minutes} minute(s) idle."
        )))
        .await?;
    }
    Ok(())
}
