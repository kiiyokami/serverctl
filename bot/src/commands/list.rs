use crate::{config, kube as k, values, Context, Error};

#[poise::command(slash_command)]
pub async fn list(ctx: Context<'_>) -> Result<(), Error> {
    let guild = match ctx.guild_id() {
        Some(g) => g.to_string(),
        None => {
            ctx.say("Run this in a server, not DM.").await?;
            return Ok(());
        }
    };
    let client = k::client().await?;
    let deps = k::list_deployments(&client).await?;
    let mut lines = vec!["**Servers in this guild:**".to_string()];
    let domain = config::public_domain();
    for dep in deps {
        if k::guild_id(&dep).as_deref() != Some(&guild) {
            continue;
        }
        let name = dep.metadata.name.as_deref().unwrap_or("?");
        let r = k::replicas(&dep);
        let avail = k::available_replicas(&dep);
        let line = if r == 0 {
            format!("- `{name}` — ⚫ stopped")
        } else if avail >= 1 {
            let port = values::read(&values::path_for(name))
                .ok()
                .map(|v| v.node_port.saturating_sub(5000))
                .unwrap_or(0);
            format!("- `{name}` — 🟢 running at `{domain}:{port}`")
        } else {
            format!("- `{name}` — 🟡 starting...")
        };
        lines.push(line);
    }
    if lines.len() == 1 {
        lines.push("_(none)_  — try `/create`".into());
    }
    ctx.say(lines.join("\n")).await?;
    Ok(())
}
