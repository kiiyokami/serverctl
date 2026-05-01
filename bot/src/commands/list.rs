use crate::{kube as k, Context, Error};

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
    for dep in deps {
        if k::guild_id(&dep).as_deref() != Some(&guild) {
            continue;
        }
        let name = dep.metadata.name.as_deref().unwrap_or("?");
        let r = k::replicas(&dep);
        let state = if r > 0 { "🟢 running" } else { "⚫ stopped" };
        lines.push(format!("- `{name}` — {state}"));
    }
    if lines.len() == 1 {
        lines.push("_(none)_".into());
    }
    ctx.say(lines.join("\n")).await?;
    Ok(())
}
