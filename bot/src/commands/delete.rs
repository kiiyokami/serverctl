use crate::{auth, helm, kube as k, values, Context, Error};

#[poise::command(slash_command)]
pub async fn delete(
    ctx: Context<'_>,
    #[description = "Server name"] name: String,
    #[description = "Also wipe world data?"] purge: Option<bool>,
) -> Result<(), Error> {
    ctx.defer().await?;
    let guild = ctx.guild_id().map(|g| g.to_string()).unwrap_or_default();
    let client = k::client().await?;
    if !auth::guild_owns(&client, &guild, &name).await? {
        ctx.say(format!("`{name}` isn't managed by this guild."))
            .await?;
        return Ok(());
    }

    let _ = helm::uninstall(&name).await;
    let purged = purge.unwrap_or(false);
    if purged {
        let _ = k::delete_pvc(&client, &format!("{name}-data")).await;
        let _ = std::fs::remove_file(values::path_for(&name));
    }
    let msg = if purged {
        format!("Purged `{name}` (release + PVC + config).")
    } else {
        format!("Uninstalled `{name}`. World data preserved.")
    };
    ctx.say(msg).await?;
    Ok(())
}
