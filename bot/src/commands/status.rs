use crate::{config, kube as k, values, Context, Error};

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
    let avail = k::available_replicas(&dep);
    let public_port = values::read(&values::path_for(&name))
        .ok()
        .map(|v| v.node_port.saturating_sub(5000))
        .unwrap_or(0);

    let msg = if r == 0 {
        format!("⚫ `{name}`: stopped")
    } else if avail >= 1 {
        format!(
            "🟢 `{name}`: running — connect at `{}:{public_port}`",
            config::public_domain()
        )
    } else {
        format!("🟡 `{name}`: starting up...")
    };
    ctx.say(msg).await?;
    Ok(())
}
