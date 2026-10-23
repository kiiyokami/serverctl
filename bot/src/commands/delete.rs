use crate::commands::{next_click, LivePrompt};
use crate::{auth, helm, kube as k, reply, values, Context, Error};
use poise::serenity_prelude as serenity;
use std::time::Duration;

const CONFIRM_TIMEOUT: Duration = Duration::from_secs(120);

#[poise::command(slash_command, guild_only, default_member_permissions = "MANAGE_GUILD")]
pub async fn delete(
    ctx: Context<'_>,
    #[description = "Server name"]
    #[autocomplete = "super::server_names"]
    name: String,
    #[description = "Also delete the world forever (default: no)"] purge: Option<bool>,
) -> Result<(), Error> {
    ctx.defer().await?;
    let guild = ctx.guild_id().map(|g| g.to_string()).unwrap_or_default();
    let client = ctx.data().kube.clone();
    if !auth::guild_owns(&client, &guild, &name).await? {
        ctx.send(reply::unknown_server(&name)).await?;
        return Ok(());
    }
    let purge = purge.unwrap_or(false);

    let handle = if purge {
        let prompt = reply::err(format!(
            "Delete **`{name}`** and its world forever? This can't be undone."
        ))
        .components(vec![serenity::CreateActionRow::Buttons(vec![
            serenity::CreateButton::new("purge").label("Delete forever").style(serenity::ButtonStyle::Danger),
            serenity::CreateButton::new("keep").label("Cancel").style(serenity::ButtonStyle::Secondary),
        ])]);
        let handle = ctx.send(prompt).await?;
        let msg = handle.message().await?.id;
        let _live = LivePrompt::new(&ctx.data().live_prompts, msg);
        let Some(click) = next_click(ctx, msg, CONFIRM_TIMEOUT).await? else {
            handle
                .edit(ctx, reply::info(format!("Timed out. **`{name}`** wasn't deleted.")).components(vec![]))
                .await?;
            return Ok(());
        };
        if click.data.custom_id != "purge" {
            click
                .create_response(ctx, reply::update(reply::info(format!("Cancelled. **`{name}`** wasn't deleted.")).components(vec![])))
                .await?;
            return Ok(());
        }
        click
            .create_response(ctx, reply::update(reply::pending(format!("Deleting **`{name}`**…")).components(vec![])))
            .await?;
        Some(handle)
    } else {
        None
    };

    let mut failed = Vec::new();
    if let Err(e) = helm::uninstall(&name).await {
        failed.push(format!("Uninstall: {e}"));
    }
    if let Err(e) = k::delete_deployment(&client, &name).await {
        failed.push(format!("Remove deployment: {e}"));
    }
    match std::fs::remove_file(values::path_for(&name)) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => failed.push(format!("Remove config: {e}")),
        _ => {}
    }
    let world = if purge {
        match k::delete_pvc(&client, &format!("{name}-data")).await {
            Ok(()) => "World deleted.".to_string(),
            Err(e) => {
                failed.push(format!("Delete world: {e}"));
                "World not deleted.".to_string()
            }
        }
    } else {
        format!("World kept: a new server named `{name}` will reuse it.")
    };

    let msg = if failed.is_empty() {
        reply::ok(format!("✅ Deleted **`{name}`**. {world}"))
    } else {
        reply::err(format!(
            "⚠️ Deleting **`{name}`** didn't fully work. {world}\n{}",
            failed.join("\n")
        ))
    };
    match handle {
        Some(h) => h.edit(ctx, msg.components(vec![])).await?,
        None => {
            ctx.send(msg).await?;
        }
    }
    Ok(())
}
