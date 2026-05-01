use crate::{values, Context, Error};
use poise::ChoiceParameter;

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
) -> Result<(), Error> {
    ctx.defer().await?;
    let guild = match ctx.guild_id() {
        Some(g) => g.to_string(),
        None => {
            ctx.say("Run this in a server.").await?;
            return Ok(());
        }
    };
    if name.is_empty()
        || name.contains(char::is_whitespace)
        || name.chars().any(|c| !c.is_ascii_lowercase() && !c.is_ascii_digit() && c != '-')
    {
        ctx.say("Name must be lowercase ASCII letters, digits, or hyphens.")
            .await?;
        return Ok(());
    }
    let dest = values::path_for(&name);
    if dest.exists() {
        ctx.say(format!("`{name}` already exists.")).await?;
        return Ok(());
    }

    let template = values::templates_dir().join(format!("{}.yaml", kind.template_name()));
    let mut v: values::Values = serde_yaml::from_str(&std::fs::read_to_string(&template)?)?;
    v.name = name.clone();
    v.node_port = values::next_free_node_port()?;
    v.discord_guild_id = guild;
    values::write(&dest, &v)?;

    let public_port = v.node_port - 5000;
    ctx.say(format!(
        "Created `{name}` ({}). Public port: **{public_port}**.\nUse `/start {name}` to launch.",
        kind.name()
    ))
    .await?;
    Ok(())
}
