use crate::{kube as k, values};
use anyhow::Result;

/// Returns true if the given guild owns the named server.
/// Returns false if the server doesn't exist OR belongs to a different guild.
pub async fn guild_owns(client: &kube::Client, guild_id: &str, server_name: &str) -> Result<bool> {
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
