use crate::{config, kube as k, minecraft, reply, values, Context, Error};

#[poise::command(slash_command)]
pub async fn list(ctx: Context<'_>) -> Result<(), Error> {
    ctx.defer().await?;
    let guild = match ctx.guild_id() {
        Some(g) => g.to_string(),
        None => {
            ctx.send(reply::err("Run this in a server, not DM.")).await?;
            return Ok(());
        }
    };
    let client = k::client().await?;
    let deps = k::list_deployments(&client).await?;
    let domain = config::public_domain();
    let mut lines = Vec::new();

    for dep in deps {
        if k::guild_id(&dep).as_deref() != Some(&guild) {
            continue;
        }
        let name = dep.metadata.name.as_deref().unwrap_or("?");
        let r = k::replicas(&dep);
        let avail = k::available_replicas(&dep);
        let v = values::read(&values::path_for(name)).ok();
        let node_port = v.as_ref().map(|v| v.node_port).unwrap_or(0);
        let public_port = node_port.saturating_sub(5000);

        if r == 0 {
            lines.push(format!("⚫ **`{name}`** — stopped"));
        } else if avail >= 1 {
            let ping = minecraft::ping(node_port as u16).await;
            let uptime = k::uptime_secs(&dep);
            let version_str = v.as_ref().map(|v| format!("{} {}", v.server.kind, v.server.version));

            let mut meta = Vec::new();
            if let Some(s) = &ping {
                meta.push(format!("Players: {}/{}", s.online, s.max));
            }
            if let Some(secs) = uptime {
                meta.push(format!("Uptime: {}", minecraft::format_uptime(secs)));
            }
            if let Some(ver) = &version_str {
                meta.push(format!("Version: {ver}"));
            }

            let detail = if meta.is_empty() {
                String::new()
            } else {
                format!("\n  {} | Connect: `{domain}:{public_port}`", meta.join(" | "))
            };
            lines.push(format!("🟢 **`{name}`** — running{detail}"));
        } else {
            lines.push(format!("🟡 **`{name}`** — starting…"));
        }
    }

    if lines.is_empty() {
        lines.push("_(none)_ — try `/create`".into());
    }

    ctx.send(reply::info(
        format!("**Servers in this guild:**\n{}", lines.join("\n"))
    ))
    .await?;
    Ok(())
}
