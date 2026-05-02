use crate::{commands::mods as mods_cmd, config, helm, kube as k, reply, values, Context, Error};
use poise::ChoiceParameter;
use std::time::Duration;

const MAX_CONCURRENT: u32 = 2;
const READY_TIMEOUT: Duration = Duration::from_secs(600);

#[derive(Debug, ChoiceParameter)]
pub enum ServerType {
    Vanilla,
    Fabric,
    Forge,
}

impl ServerType {
    fn template_name(&self) -> &'static str {
        match self {
            Self::Vanilla => "vanilla",
            Self::Fabric => "fabric",
            Self::Forge => "forge",
        }
    }
}

#[poise::command(slash_command)]
pub async fn create(
    ctx: Context<'_>,
    #[description = "Server name (lowercase, no spaces)"] name: String,
    #[description = "Server type"] kind: ServerType,
    #[description = "Modrinth mod/modpack URL or .jar URL"] mods_url: Option<String>,
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

    let template = values::templates_dir().join(format!("{}.yaml", kind.template_name()));
    let mut v: values::Values = serde_yaml::from_str(&std::fs::read_to_string(&template)?)?;
    v.name = name.clone();
    v.node_port = values::next_free_node_port()?;
    v.discord_guild_id = guild;

    if let Some(ref url) = mods_url {
        match mods_cmd::apply_mod_url(&mut v, url).await? {
            mods_cmd::ModResult::Unrecognized => {
                ctx.send(reply::err("Unrecognized mods URL.")).await?;
                return Ok(());
            }
            _ => {}
        }
    }

    values::write(&dest, &v)?;
    let public_port = v.node_port - 5000;

    let client = k::client().await?;
    let running = k::running_server_count(&client).await?;
    if running >= MAX_CONCURRENT {
        ctx.send(reply::pending(format!(
            "Created `{name}` ({}) but {running}/{MAX_CONCURRENT} servers are already running. Stop one, then `/start {name}`.",
            kind.name()
        )))
        .await?;
        return Ok(());
    }

    let ver_str = {
        let mods = v.server.mods.len();
        let base = format!("{} {}", v.server.kind, v.server.version);
        if mods > 0 { format!("{base} • {mods} mods") } else { base }
    };

    ctx.send(reply::pending(format!(
        "🟡 **`{name}`** is starting ({ver_str})\nThis can take up to 10 minutes for modpacks."
    )))
    .await?;

    let chart = values::chart_dir();
    helm::upgrade_install(&name, &chart, &dest).await?;
    k::scale(&client, &name, 1).await?;

    if k::wait_until_ready(&client, &name, READY_TIMEOUT).await? {
        ctx.send(reply::ok(format!(
            "✅ **`{name}`** is ready! ({ver_str})\nConnect: `{}:{public_port}`",
            config::public_domain()
        )))
        .await?;
    } else {
        ctx.send(reply::pending(format!(
            "⏳ **`{name}`** is still starting after 10 min. Use `/status {name}` to check."
        )))
        .await?;
    }
    Ok(())
}
