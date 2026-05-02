use crate::{auth, kube as k, reply, Context, Error};

#[poise::command(slash_command)]
pub async fn stop(
    ctx: Context<'_>,
    #[description = "Server name"] name: String,
) -> Result<(), Error> {
    ctx.defer().await?;
    let guild = ctx.guild_id().map(|g| g.to_string()).unwrap_or_default();
    let client = k::client().await?;
    if !auth::guild_owns(&client, &guild, &name).await? {
        ctx.send(reply::err(format!("`{name}` isn't managed by this guild.")))
            .await?;
        return Ok(());
    }
    k::scale(&client, &name, 0).await?;
    ctx.send(reply::ok(format!("⚫ `{name}` stopped."))).await?;
    Ok(())
}
