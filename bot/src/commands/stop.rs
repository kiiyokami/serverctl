use crate::{auth, kube as k, reply, Context, Error};

#[poise::command(slash_command, guild_only)]
pub async fn stop(
    ctx: Context<'_>,
    #[description = "Server name"]
    #[autocomplete = "super::server_names"]
    name: String,
) -> Result<(), Error> {
    ctx.defer().await?;
    let guild = ctx.guild_id().map(|g| g.to_string()).unwrap_or_default();
    let client = ctx.data().kube.clone();
    if !auth::guild_owns(&client, &guild, &name).await? {
        ctx.send(reply::unknown_server(&name)).await?;
        return Ok(());
    }
    let running = k::get_deployment(&client, &name)
        .await?
        .is_some_and(|d| k::replicas(&d) > 0);
    if !running {
        ctx.send(reply::info(format!("⚫ **`{name}`** is already stopped.")))
            .await?;
        return Ok(());
    }
    k::mark_stopped_by_user(&client, &name).await?;
    k::scale(&client, &name, 0).await?;
    ctx.send(reply::ok(format!("⚫ **`{name}`** stopped."))).await?;
    Ok(())
}
