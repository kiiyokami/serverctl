use crate::{commands::status, kube as k, reply, values, Context, Error};
use std::collections::HashSet;

#[poise::command(slash_command, guild_only)]
pub async fn list(ctx: Context<'_>) -> Result<(), Error> {
    ctx.defer().await?;
    let guild = ctx.guild_id().map(|g| g.to_string()).unwrap_or_default();
    let client = ctx.data().kube.clone();
    let deps = k::list_deployments(&client).await?;
    let mut lines = Vec::new();
    let mut seen = HashSet::new();

    for dep in &deps {
        if k::guild_id(dep).as_deref() != Some(&guild) {
            continue;
        }
        let name = dep.metadata.name.as_deref().unwrap_or("?");
        seen.insert(name.to_string());
        let v = values::read(&values::path_for(name)).ok();
        lines.push(status::describe(name, Some(dep), v.as_ref()).await);
    }
    for v in values::all() {
        if v.discord_guild_id == guild && !seen.contains(&v.name) {
            lines.push(status::describe(&v.name, None, Some(&v)).await);
        }
    }

    if lines.is_empty() {
        lines.push("No servers yet. Make one with `/create`.".into());
    }

    ctx.send(reply::info(format!("**Servers**\n{}", lines.join("\n\n"))))
        .await?;
    Ok(())
}
