use crate::{commands::mods as mods_cmd, config, helm, kube as k, reply, values, Context, Error};
use poise::ChoiceParameter;
use regex::Regex;
use std::time::Duration;

const MAX_CONCURRENT: u32 = 2;
const READY_TIMEOUT: Duration = Duration::from_secs(600);

#[derive(Debug, ChoiceParameter)]
pub enum ServerType {
    Vanilla,
    Fabric,
    Forge,
    Paper,
}

impl ServerType {
    fn template_name(&self) -> &'static str {
        match self {
            Self::Vanilla => "vanilla",
            Self::Fabric => "fabric",
            Self::Forge => "forge",
            Self::Paper => "paper",
        }
    }
}

#[derive(Debug, ChoiceParameter)]
pub enum GameMode {
    Survival,
    Creative,
    Adventure,
    Spectator,
}

impl GameMode {
    fn env_value(&self) -> &'static str {
        match self {
            Self::Survival => "survival",
            Self::Creative => "creative",
            Self::Adventure => "adventure",
            Self::Spectator => "spectator",
        }
    }
}

#[derive(Debug, ChoiceParameter)]
pub enum Difficulty {
    Peaceful,
    Easy,
    Normal,
    Hard,
}

impl Difficulty {
    fn env_value(&self) -> &'static str {
        match self {
            Self::Peaceful => "peaceful",
            Self::Easy => "easy",
            Self::Normal => "normal",
            Self::Hard => "hard",
        }
    }
}

/// itzg writes MODE/DIFFICULTY into server.properties; omitted choices keep its defaults.
fn apply_gameplay(v: &mut values::Values, mode: Option<&GameMode>, difficulty: Option<&Difficulty>) {
    let pairs = [
        ("MODE", mode.map(GameMode::env_value)),
        ("DIFFICULTY", difficulty.map(Difficulty::env_value)),
    ];
    for (key, value) in pairs {
        if let Some(value) = value {
            v.extra_env
                .get_or_insert_with(serde_yaml::Mapping::new)
                .insert(key.into(), value.into());
        }
    }
}

// poise maps each slash-command option to a parameter.
#[allow(clippy::too_many_arguments)]
#[poise::command(slash_command)]
pub async fn create(
    ctx: Context<'_>,
    #[description = "Server name (lowercase, no spaces)"] name: String,
    #[description = "Server type"] kind: ServerType,
    #[description = "Minecraft version (e.g. 1.20.1). Defaults to the type's pinned version."]
    mc_version: Option<String>,
    #[description = "Modrinth/CurseForge mods or modpack, Modrinth plugins (Paper), or .jar URLs"]
    mods_url: Option<String>,
    #[description = "World seed (number or text). Random if omitted."] seed: Option<String>,
    #[description = "Default game mode for players. Survival if omitted."] gamemode: Option<GameMode>,
    #[description = "World difficulty. Easy if omitted."] difficulty: Option<Difficulty>,
    #[description = "Start the server right away (default: yes)"] start_now: Option<bool>,
) -> Result<(), Error> {
    ctx.defer().await?;
    let guild = match ctx.guild_id() {
        Some(g) => g.to_string(),
        None => {
            ctx.send(reply::err("Run this in a server.")).await?;
            return Ok(());
        }
    };
    if name.is_empty()
        || name
            .chars()
            .any(|c| !c.is_ascii_lowercase() && !c.is_ascii_digit() && c != '-')
    {
        ctx.send(reply::err(
            "Name must be lowercase ASCII letters, digits, or hyphens.",
        ))
        .await?;
        return Ok(());
    }
    let dest = values::path_for(&name);
    if dest.exists() {
        ctx.send(reply::err(format!("`{name}` already exists.")))
            .await?;
        return Ok(());
    }

    let client = k::client().await?;
    let used_ports = k::used_node_ports(&client).await?;

    let template = values::templates_dir().join(format!("{}.yaml", kind.template_name()));
    let mut v: values::Values = serde_yaml::from_str(&std::fs::read_to_string(&template)?)?;
    v.name = name.clone();
    v.node_port = values::next_free_node_port(&used_ports)?;
    v.discord_guild_id = guild;

    if let Some(ref ver) = mc_version {
        let ver = ver.trim();
        if !Regex::new(r"^\d+\.\d+(\.\d+)?$")?.is_match(ver) {
            ctx.send(reply::err(format!(
                "`{ver}` isn't a valid Minecraft version (e.g. `1.20.1`)."
            )))
            .await?;
            return Ok(());
        }
        values::apply_mc_version(&mut v, ver);
    }

    if let Some(ref s) = seed {
        if let Err(msg) = values::apply_seed(&mut v, s) {
            ctx.send(reply::err(msg)).await?;
            return Ok(());
        }
    }

    apply_gameplay(&mut v, gamemode.as_ref(), difficulty.as_ref());

    if let Some(ref urls) = mods_url {
        let user_id = ctx.author().id.to_string();
        if let Err(msg) = mods_cmd::apply_mod_urls(&mut v, &client, &user_id, urls).await? {
            ctx.send(reply::err(msg)).await?;
            return Ok(());
        }
    }

    values::write(&dest, &v)?;
    let public_port = v.node_port - 5000;
    let ver_str = values::summary(&v);

    if start_now == Some(false) {
        ctx.send(reply::ok(format!(
            "✅ Created **`{name}`** ({ver_str}). Start it with `/start {name}`."
        )))
        .await?;
        return Ok(());
    }

    let running = k::running_server_count(&client).await?;
    if running >= MAX_CONCURRENT {
        ctx.send(reply::pending(format!(
            "Created `{name}` ({}) but {running}/{MAX_CONCURRENT} servers are already running. Stop one, then `/start {name}`.",
            kind.name()
        )))
        .await?;
        return Ok(());
    }

    let handle = ctx.send(reply::pending(format!(
        "🟡 **`{name}`** is starting ({ver_str})\nThis can take up to 10 minutes for modpacks."
    )))
    .await?;

    let chart = values::chart_dir();
    helm::upgrade_install(&name, &chart, &dest).await?;
    k::scale(&client, &name, 1).await?;
    let _ = k::patch_channel_id(&client, &name, &ctx.channel_id().to_string()).await;

    if k::wait_until_ready(&client, &name, READY_TIMEOUT).await? {
        handle.edit(ctx, reply::ok(format!(
            "✅ **`{name}`** is ready! ({ver_str})\nConnect: `{}:{public_port}`",
            config::public_domain()
        )))
        .await?;
    } else {
        handle.edit(ctx, reply::pending(format!(
            "⏳ **`{name}`** is still starting after 10 min. Use `/status {name}` to check."
        )))
        .await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> values::Values {
        serde_yaml::from_str(
            "name: t\nnodePort: 30565\nserver:\n  type: PAPER\n  version: \"1.21.4\"\n  memory: 4G\n",
        )
        .unwrap()
    }

    fn env(v: &values::Values, key: &str) -> Option<String> {
        v.extra_env
            .as_ref()
            .and_then(|e| e.get(key))
            .and_then(|x| x.as_str())
            .map(str::to_string)
    }

    #[test]
    fn gameplay_choices_set_mode_and_difficulty() {
        let mut v = base();
        apply_gameplay(&mut v, Some(&GameMode::Creative), Some(&Difficulty::Peaceful));
        assert_eq!(env(&v, "MODE").as_deref(), Some("creative"));
        assert_eq!(env(&v, "DIFFICULTY").as_deref(), Some("peaceful"));
    }

    #[test]
    fn omitted_gameplay_choices_leave_server_defaults() {
        let mut v = base();
        apply_gameplay(&mut v, None, None);
        assert!(v.extra_env.is_none());
    }

    #[test]
    fn every_choice_maps_to_an_itzg_value() {
        let modes = [
            (GameMode::Survival, "survival"),
            (GameMode::Creative, "creative"),
            (GameMode::Adventure, "adventure"),
            (GameMode::Spectator, "spectator"),
        ];
        for (m, want) in modes {
            assert_eq!(m.env_value(), want);
        }
        let diffs = [
            (Difficulty::Peaceful, "peaceful"),
            (Difficulty::Easy, "easy"),
            (Difficulty::Normal, "normal"),
            (Difficulty::Hard, "hard"),
        ];
        for (d, want) in diffs {
            assert_eq!(d.env_value(), want);
        }
    }
}
