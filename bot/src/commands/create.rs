use crate::commands::settings::{Difficulty, GameMode, Gameplay, OFFLINE_WARNING};
use crate::{commands::{mods as mods_cmd, start}, kube as k, reply, values, Context, Error};
use poise::serenity_prelude as serenity;
use poise::{ChoiceParameter, Modal as _};
use regex::Regex;
use std::time::Duration;

const FORM_TIMEOUT: Duration = Duration::from_secs(900);
// Under the 15-minute token lifetime, so the timeout notice can still be posted.
const PANEL_TIMEOUT: Duration = Duration::from_secs(600);

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
    // Not submit.defer(): that only acknowledges, leaving no message to edit later ("Unknown Message").
    submit
        .create_response(
            ctx.http(),
            serenity::CreateInteractionResponse::Defer(serenity::CreateInteractionResponseMessage::new()),
        )
        .await?;
    // Each click is a new interaction; later edits use the latest token, which lasts 15 minutes.
    let mut token = submit.token.clone();
    if let Err(e) = run(ctx, &submit, &kind, &mut token).await {
        tracing::error!("/create failed: {e}");
        reply::edit(ctx.http(), &token, reply::failure("create", &e)).await?;
    }
    Ok(())
}

async fn run(
    ctx: Context<'_>,
    submit: &serenity::ModalInteraction,
    kind: &ServerType,
    token: &mut String,
) -> Result<(), Error> {
    let http = ctx.http();
    let form = CreateForm::parse(submit.data.clone())?;
    let name = form.name.trim().to_string();
    let mut v = match prepare(ctx, &name, kind, &form).await? {
        Ok(v) => v,
        Err(msg) => {
            reply::edit(http, token, reply::err(msg)).await?;
            return Ok(());
        }
    };

    let mut choice = Gameplay::new_server();
    let panel_msg = reply::edit(http, token, panel(&name, &v, &choice)).await?;
    let start_now = loop {
        let Some(click) = serenity::ComponentInteractionCollector::new(&ctx.serenity_context().shard)
            .message_id(panel_msg.id)
            .author_id(ctx.author().id)
            .timeout(PANEL_TIMEOUT)
            .await
        else {
            reply::edit(http, token, reply::info(format!("Timed out. `{name}` wasn't created."))).await?;
            return Ok(());
        };
        let picked = match &click.data.kind {
            serenity::ComponentInteractionDataKind::StringSelect { values } => {
                values.first().and_then(|i| i.parse::<usize>().ok())
            }
            _ => None,
        };
        let toggle = |b: &mut Option<bool>| *b = Some(!b.unwrap_or(false));
        let next = match click.data.custom_id.as_str() {
            "mode" => {
                choice.mode = picked.and_then(GameMode::from_index).or(choice.mode);
                None
            }
            "difficulty" => {
                choice.difficulty = picked.and_then(Difficulty::from_index).or(choice.difficulty);
                None
            }
            "offline" => {
                toggle(&mut choice.offline);
                None
            }
            "hardcore" => {
                toggle(&mut choice.hardcore);
                None
            }
            "pvp" => {
                toggle(&mut choice.pvp);
                None
            }
            "start" => Some(Some(true)),
            "create" => Some(Some(false)),
            _ => Some(None),
        };
        let body = match next {
            None => panel(&name, &v, &choice),
            Some(None) => reply::info(format!("Cancelled. `{name}` wasn't created.")).components(vec![]),
            Some(Some(_)) => reply::pending(format!("Creating **`{name}`**…")).components(vec![]),
        };
        click
            .create_response(
                http,
                serenity::CreateInteractionResponse::UpdateMessage(
                    body.to_slash_initial_response(serenity::CreateInteractionResponseMessage::new()),
                ),
            )
            .await?;
        *token = click.token.clone();
        match next {
            None => continue,
            Some(None) => return Ok(()),
            Some(Some(start_now)) => break start_now,
        }
    };

    let dest = values::path_for(&name);
    if dest.exists() {
        reply::edit(http, token, reply::err(format!("`{name}` was just created by someone else. Pick another name."))).await?;
        return Ok(());
    }
    choice.apply(&mut v);
    values::write(&dest, &v)?;
    let ver_str = values::summary(&v);
    let tip = format!("Change gameplay settings any time with `/settings {name}`.");

    if !start_now {
        reply::edit(http, token, reply::ok(format!(
            "✅ Created **`{name}`** ({ver_str}). Start it with `/start {name}`.\n{tip}"
        )))
        .await?;
        return Ok(());
    }

    let channel = submit.channel_id;
    let client = k::client().await?;
    if let Some(running) = start::launch(http, token, channel, &client, &name).await? {
        reply::edit(http, token, reply::pending(format!(
            "✅ Created **`{name}`** ({ver_str}), but {running}/{} servers are already running. \
             `/stop` one, then `/start {name}`.\n{tip}",
            start::MAX_CONCURRENT
        )))
        .await?;
    }
    Ok(())
}

/// Build the server's config from the form; `Err` is a message for the user. Nothing is written.
async fn prepare(
    ctx: Context<'_>,
    name: &str,
    kind: &ServerType,
    form: &CreateForm,
) -> Result<Result<values::Values, String>, Error> {
    if let Err(msg) = validate_name(name) {
        return Ok(Err(msg));
    }
    if values::path_for(name).exists() {
        return Ok(Err(format!("`{name}` already exists. Pick another name.")));
    }

    let client = k::client().await?;
    let mut used_ports = k::used_node_ports(&client).await?;
    used_ports.extend(values::local_node_ports());

    let template = values::templates_dir().join(format!("{}.yaml", kind.template_name()));
    let mut v: values::Values = serde_yaml::from_str(&std::fs::read_to_string(&template)?)?;
    v.name = name.to_string();
    v.node_port = values::next_free_node_port(&used_ports)?;
    v.discord_guild_id = ctx.guild_id().map(|g| g.to_string()).unwrap_or_default();

    if let Some(ver) = form.version.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        if !Regex::new(r"^\d+\.\d+(\.\d+)?$")?.is_match(ver) {
            return Ok(Err(format!("`{ver}` isn't a valid Minecraft version (e.g. `1.21.1`).")));
        }
        values::apply_mc_version(&mut v, ver);
    }

    if let Some(s) = &form.seed {
        if let Err(msg) = values::apply_seed(&mut v, s) {
            return Ok(Err(msg));
        }
    }

    if let Some(urls) = &form.mods {
        let user_id = ctx.author().id.to_string();
        if let Err(msg) = mods_cmd::apply_mod_urls(&mut v, &client, &user_id, urls).await? {
            return Ok(Err(msg));
        }
    }
    Ok(Ok(v))
}

/// The settings step after the form: dropdowns, toggles and the create buttons.
fn panel(name: &str, v: &values::Values, choice: &Gameplay) -> poise::CreateReply {
    let select = |id: &str, label: &str, names: Vec<String>, current: &str| {
        let options = names
            .into_iter()
            .enumerate()
            .map(|(i, n)| {
                serenity::CreateSelectMenuOption::new(format!("{label}: {n}"), i.to_string())
                    .default_selection(n == current)
            })
            .collect();
        serenity::CreateActionRow::SelectMenu(serenity::CreateSelectMenu::new(
            id,
            serenity::CreateSelectMenuKind::String { options },
        ))
    };
    let names = |list: Vec<poise::CommandParameterChoice>| list.into_iter().map(|c| c.name).collect();
    let toggle = |id: &str, label: &str, on: Option<bool>| {
        let on = on.unwrap_or(false);
        serenity::CreateButton::new(id)
            .label(format!("{label}: {}", if on { "on" } else { "off" }))
            .style(if on { serenity::ButtonStyle::Success } else { serenity::ButtonStyle::Secondary })
    };
    let rows = vec![
        select("mode", "Game mode", names(GameMode::list()), choice.mode.map_or("", |m| m.name())),
        select("difficulty", "Difficulty", names(Difficulty::list()), choice.difficulty.map_or("", |d| d.name())),
        serenity::CreateActionRow::Buttons(vec![
            toggle("offline", "Offline mode", choice.offline),
            toggle("hardcore", "Hardcore", choice.hardcore),
            toggle("pvp", "PvP", choice.pvp),
        ]),
        serenity::CreateActionRow::Buttons(vec![
            serenity::CreateButton::new("start").label("Create & start").style(serenity::ButtonStyle::Primary),
            serenity::CreateButton::new("create").label("Create only").style(serenity::ButtonStyle::Secondary),
            serenity::CreateButton::new("cancel").label("Cancel").style(serenity::ButtonStyle::Danger),
        ]),
    ];
    let mut body = format!(
        "**New server `{name}`** ({})\nPick the gameplay settings, then create it.",
        values::summary(v)
    );
    if choice.offline == Some(true) {
        body = format!("{body}\n{OFFLINE_WARNING}");
    }
    reply::info(body).components(rows)
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
