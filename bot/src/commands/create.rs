use crate::{commands::{mods as mods_cmd, start}, kube as k, reply, values, Context, Error};
use poise::serenity_prelude as serenity;
use poise::{ChoiceParameter, Modal as _};
use regex::Regex;
use std::time::Duration;

const FORM_TIMEOUT: Duration = Duration::from_secs(900);

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

#[derive(Debug, poise::Modal)]
#[name = "New Minecraft server"]
struct CreateForm {
    #[name = "Name"]
    #[placeholder = "e.g. survival-smp (lowercase, digits, hyphens)"]
    #[min_length = 1]
    #[max_length = 30]
    name: String,
    #[name = "Minecraft version"]
    #[placeholder = "e.g. 1.21.1. Leave blank for the default."]
    #[max_length = 10]
    version: Option<String>,
    #[name = "World seed"]
    #[placeholder = "Leave blank for a random world"]
    #[max_length = 64]
    seed: Option<String>,
    #[name = "Mods, modpack or plugins"]
    #[placeholder = "Modrinth/CurseForge links or .jar URLs, comma-separated"]
    #[paragraph]
    mods: Option<String>,
}

/// Names become k8s Service names (DNS-1035), so they must start with a letter.
fn validate_name(name: &str) -> Result<(), String> {
    let valid = name.len() <= 30
        && name.starts_with(|c: char| c.is_ascii_lowercase())
        && !name.ends_with('-')
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if valid {
        Ok(())
    } else {
        Err(format!(
            "`{name}` isn't a valid name. Use up to 30 lowercase letters, digits or hyphens, \
             starting with a letter (e.g. `survival-smp`)."
        ))
    }
}

#[poise::command(slash_command, guild_only)]
pub async fn create(
    ctx: Context<'_>,
    #[description = "Server type"] kind: ServerType,
    #[description = "Start it right after creating (default: yes)"] start_now: Option<bool>,
) -> Result<(), Error> {
    let poise::Context::Application(app) = ctx else {
        return Ok(());
    };
    let form_id = app.interaction.id.to_string();
    app.interaction
        .create_response(ctx.http(), CreateForm::create(None, form_id.clone()))
        .await?;
    app.has_sent_initial_response
        .store(true, std::sync::atomic::Ordering::SeqCst);

    let Some(submit) = serenity::ModalInteractionCollector::new(&ctx.serenity_context().shard)
        .custom_ids(vec![form_id])
        .author_id(ctx.author().id)
        .timeout(FORM_TIMEOUT)
        .await
    else {
        return Ok(());
    };
    // The modal used up the slash command's response, so reply through the submission instead.
    submit.defer(ctx.http()).await?;
    if let Err(e) = finish(ctx, &submit, &kind, start_now).await {
        tracing::error!("/create failed: {e}");
        reply::edit(ctx.http(), &submit.token, reply::failure("create", &e)).await?;
    }
    Ok(())
}

async fn finish(
    ctx: Context<'_>,
    submit: &serenity::ModalInteraction,
    kind: &ServerType,
    start_now: Option<bool>,
) -> Result<(), Error> {
    let http = ctx.http();
    let token = &submit.token;
    let fail = |msg: String| reply::edit(http, token, reply::err(msg));

    let form = CreateForm::parse(submit.data.clone())?;
    let name = form.name.trim().to_string();
    if let Err(msg) = validate_name(&name) {
        return Ok(fail(msg).await?);
    }
    let dest = values::path_for(&name);
    if dest.exists() {
        return Ok(fail(format!("`{name}` already exists. Pick another name.")).await?);
    }

    let client = k::client().await?;
    let mut used_ports = k::used_node_ports(&client).await?;
    used_ports.extend(values::local_node_ports());

    let template = values::templates_dir().join(format!("{}.yaml", kind.template_name()));
    let mut v: values::Values = serde_yaml::from_str(&std::fs::read_to_string(&template)?)?;
    v.name = name.clone();
    v.node_port = values::next_free_node_port(&used_ports)?;
    v.discord_guild_id = ctx.guild_id().map(|g| g.to_string()).unwrap_or_default();

    if let Some(ver) = form.version.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        if !Regex::new(r"^\d+\.\d+(\.\d+)?$")?.is_match(ver) {
            return Ok(fail(format!("`{ver}` isn't a valid Minecraft version (e.g. `1.21.1`).")).await?);
        }
        values::apply_mc_version(&mut v, ver);
    }

    if let Some(s) = &form.seed {
        if let Err(msg) = values::apply_seed(&mut v, s) {
            return Ok(fail(msg).await?);
        }
    }

    if let Some(urls) = &form.mods {
        let user_id = ctx.author().id.to_string();
        if let Err(msg) = mods_cmd::apply_mod_urls(&mut v, &client, &user_id, urls).await? {
            return Ok(fail(msg).await?);
        }
    }

    values::write(&dest, &v)?;
    let ver_str = values::summary(&v);
    let tip = format!("Change game mode or difficulty with `/settings {name}`.");

    if start_now == Some(false) {
        reply::edit(http, token, reply::ok(format!(
            "✅ Created **`{name}`** ({ver_str}). Start it with `/start {name}`.\n{tip}"
        )))
        .await?;
        return Ok(());
    }

    if let Some(running) = start::launch(http, token, submit.channel_id, &client, &name).await? {
        reply::edit(http, token, reply::pending(format!(
            "✅ Created **`{name}`** ({ver_str}), but {running}/{} servers are already running. \
             `/stop` one, then `/start {name}`.\n{tip}",
            start::MAX_CONCURRENT
        )))
        .await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_must_be_dns_safe() {
        for ok in ["smp", "survival-smp", "a1", "x"] {
            assert!(validate_name(ok).is_ok(), "{ok}");
        }
        for bad in ["", "1smp", "-smp", "smp-", "Smp", "my smp", "smp_2", &"a".repeat(31)] {
            assert!(validate_name(bad).is_err(), "{bad}");
        }
    }
}
