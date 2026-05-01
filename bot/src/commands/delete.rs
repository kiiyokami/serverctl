use crate::{auth, helm, kube as k, values, Context, Error};

#[poise::command(slash_command)]
pub async fn delete(
    ctx: Context<'_>,
    #[description = "Server name"] name: String,
    #[description = "Also wipe world data PVC and local config (default: false)"]
    purge: Option<bool>,
) -> Result<(), Error> {
    ctx.defer().await?;
    let guild = ctx.guild_id().map(|g| g.to_string()).unwrap_or_default();
    let client = k::client().await?;
    if !auth::guild_owns(&client, &guild, &name).await? {
        ctx.say(format!("`{name}` isn't managed by this guild."))
            .await?;
        return Ok(());
    }

    let mut report = Vec::new();

    match helm::uninstall(&name).await {
        Ok(()) => report.push(format!("✅ Helm release uninstalled")),
        Err(e) => report.push(format!("⚠️ Helm uninstall: {e}")),
    }

    if purge.unwrap_or(false) {
        match k::delete_pvc(&client, &format!("{name}-data")).await {
            Ok(()) => report.push(format!("✅ PVC `{name}-data` deleted (world data wiped)")),
            Err(e) => report.push(format!("⚠️ PVC delete: {e}")),
        }
        let path = values::path_for(&name);
        match std::fs::remove_file(&path) {
            Ok(()) => report.push(format!("✅ Local config removed")),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => report.push(format!("⚠️ Config remove: {e}")),
        }
    } else {
        report.push(format!(
            "ℹ️ World data preserved. Re-run with `purge:true` to also wipe the PVC."
        ));
    }

    ctx.say(format!("**`{name}`**\n{}", report.join("\n"))).await?;
    Ok(())
}
