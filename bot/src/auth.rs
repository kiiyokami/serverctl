use crate::{kube as k, values};
use anyhow::Result;

pub async fn guild_owns(client: &kube::Client, guild_id: &str, server_name: &str) -> Result<bool> {
    // DMs have no guild; "" would match configs written without discordGuildId (server.sh).
    if guild_id.is_empty() {
        return Ok(false);
    }
    if let Some(dep) = k::get_deployment(client, server_name).await? {
        return Ok(k::guild_id(&dep).as_deref() == Some(guild_id));
    }
    let path = values::path_for(server_name);
    if path.exists() {
        let v = values::read(&path)?;
        return Ok(v.discord_guild_id == guild_id);
    }
    Ok(false)
}
