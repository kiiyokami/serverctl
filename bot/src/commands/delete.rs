use crate::{auth, helm, kube as k, reply, values, Context, Error};

#[poise::command(slash_command, guild_only, default_member_permissions = "MANAGE_GUILD")]
pub async fn delete(
    ctx: Context<'_>,
    #[description = "Server name"]
    #[autocomplete = "super::server_names"]
    name: String,
    #[description = "Also wipe world data PVC and local config (default: false)"]
    purge: Option<bool>,
) -> Result<(), Error> {
    ctx.defer().await?;
    let guild = ctx.guild_id().map(|g| g.to_string()).unwrap_or_default();
    let client = ctx.data().kube.clone();
    if !auth::guild_owns(&client, &guild, &name).await? {
        ctx.send(reply::unknown_server(&name)).await?;
        return Ok(());
    }

    let mut report = Vec::new();

    match helm::uninstall(&name).await {
        Ok(()) => report.push("✅ Helm release uninstalled".to_string()),
        Err(e) => report.push(format!("⚠️ Helm uninstall: {e}")),
    }

    match k::delete_deployment(&client, &name).await {
        Ok(()) => report.push("✅ Deployment deleted".to_string()),
        Err(e) => report.push(format!("⚠️ Deployment delete: {e}")),
    }

    let path = values::path_for(&name);
    match std::fs::remove_file(&path) {
        Ok(()) => report.push("✅ Local config removed".to_string()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => report.push(format!("⚠️ Config remove: {e}")),
    }

    if purge.unwrap_or(false) {
        match k::delete_pvc(&client, &format!("{name}-data")).await {
            Ok(()) => report.push(format!("✅ PVC `{name}-data` deleted (world data wiped)")),
            Err(e) => report.push(format!("⚠️ PVC delete: {e}")),
        }
    } else {
        report.push(
            format!("ℹ️ World data kept. A new server named `{name}` will reuse it."),
        );
    }

    ctx.send(reply::info(format!(
        "**`{name}`**\n{}",
        report.join("\n")
    )))
    .await?;
    Ok(())
}
