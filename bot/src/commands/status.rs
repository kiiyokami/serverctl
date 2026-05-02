use crate::{config, kube as k, minecraft, reply, values, Context, Error};

#[poise::command(slash_command)]
pub async fn status(
    ctx: Context<'_>,
    #[description = "Server name"] name: String,
) -> Result<(), Error> {
    ctx.defer().await?;
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
    let v = values::read(&values::path_for(&name)).ok();
    let node_port = v.as_ref().map(|v| v.node_port).unwrap_or(0);
    let public_port = node_port.saturating_sub(5000);
    let version_str = v.as_ref().map(|v| {
        let kind = &v.server.kind;
        let ver = &v.server.version;
        format!("{kind} {ver}")
    });

    if r == 0 {
        ctx.send(reply::info(format!("⚫ **`{name}`** — stopped"))).await?;
        return Ok(());
    }

    if avail < 1 {
        ctx.send(reply::pending(format!("🟡 **`{name}`** — starting up…"))).await?;
        return Ok(());
    }

    let ping = minecraft::ping(node_port as u16).await;
    let uptime = k::uptime_secs(&dep);

    let mut lines = vec![format!("🟢 **`{name}`** — running")];

    let mut meta = Vec::new();
    if let Some(s) = &ping {
        meta.push(format!("Players: {}/{}", s.online, s.max));
    }
    if let Some(secs) = uptime {
        meta.push(format!("Uptime: {}", minecraft::format_uptime(secs)));
    }
    if let Some(v) = &version_str {
        meta.push(format!("Version: {v}"));
    }
    if !meta.is_empty() {
        lines.push(meta.join(" | "));
    }
    lines.push(format!("Connect: `{}:{public_port}`", config::public_domain()));

    ctx.send(reply::ok(lines.join("\n"))).await?;
    Ok(())
}
