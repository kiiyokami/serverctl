use crate::{values, Context};

pub mod create;
pub mod curseforge_key;
pub mod delete;
pub mod list;
pub mod mods;
pub mod settings;
pub mod start;
pub mod status;
pub mod stop;
pub mod ttl;

/// Autocomplete for server-name options: this guild's servers starting with what's typed.
pub async fn server_names(ctx: Context<'_>, partial: &str) -> Vec<String> {
    let Some(guild) = ctx.guild_id().map(|g| g.to_string()) else {
        return Vec::new();
    };
    let mut names: Vec<String> = values::all()
        .into_iter()
        .filter(|v| v.discord_guild_id == guild && v.name.starts_with(partial))
        .map(|v| v.name)
        .collect();
    names.sort();
    names.truncate(25);
    names
}
