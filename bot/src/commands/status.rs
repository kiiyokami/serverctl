use crate::{config, kube as k, reply, values, Context, Error};

#[poise::command(slash_command)]
pub async fn status(
    ctx: Context<'_>,
    #[description = "Server name"] name: String,
) -> Result<(), Error> {
    let guild = match ctx.guild_id() {
        Some(g) => g.to_string(),
        None => {
            ctx.send(reply::err("Run this in a server.")).await?;
            return Ok(());
        }
    };
    let client = k::client().await?;
    let dep = match k::get_deployment(&client, &name).await? {
        Some(d) => d,
        None => {
            ctx.send(reply::err(format!("`{name}` not found."))).await?;
            return Ok(());
        }
    };
    if k::guild_id(&dep).as_deref() != Some(&guild) {
        ctx.send(reply::err(format!("`{name}` isn't managed by this guild.")))
            .await?;
        return Ok(());
    }
    let r = k::replicas(&dep);
    let avail = k::available_replicas(&dep);
    let public_port = values::read(&values::path_for(&name))
        .ok()
        .map(|v| v.node_port.saturating_sub(5000))
        .unwrap_or(0);

    let (reply_fn, msg): (fn(_) -> _, _) = if r == 0 {
        (reply::info, format!("⚫ `{name}`: stopped"))
    } else if avail >= 1 {
        (
            reply::ok,
            format!(
                "🟢 `{name}`: running — connect at `{}:{public_port}`",
                config::public_domain()
            ),
        )
    } else {
        (reply::pending, format!("🟡 `{name}`: starting up…"))
    };
    ctx.send(reply_fn(msg)).await?;
    Ok(())
}
