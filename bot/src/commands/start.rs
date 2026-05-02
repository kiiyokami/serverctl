use crate::{auth, config, helm, kube as k, reply, values, Context, Error};
use std::time::Duration;

const MAX_CONCURRENT: u32 = 2;
const READY_TIMEOUT: Duration = Duration::from_secs(600);

#[poise::command(slash_command)]
pub async fn start(
    ctx: Context<'_>,
    #[description = "Server name"] name: String,
) -> Result<(), Error> {
    ctx.defer().await?;
    let guild = ctx.guild_id().map(|g| g.to_string()).unwrap_or_default();
    let client = k::client().await?;

    if !auth::guild_owns(&client, &guild, &name).await? {
        ctx.send(reply::err(format!("`{name}` isn't managed by this guild.")))
            .await?;
        return Ok(());
    }

    let already_up = k::get_deployment(&client, &name)
        .await?
        .as_ref()
        .map(k::replicas)
        .unwrap_or(0)
        > 0;

    if !already_up {
        let running: u32 = k::list_deployments(&client)
            .await?
            .iter()
            .map(k::replicas)
            .filter(|r| *r > 0)
            .count() as u32;
        if running >= MAX_CONCURRENT {
            ctx.send(reply::err(format!(
                "Already {running}/{MAX_CONCURRENT} servers running. Stop one first."
            )))
            .await?;
            return Ok(());
        }
    }

    if !helm::release_exists(&name).await? {
        let chart = values::chart_dir();
        let vfile = values::path_for(&name);
        helm::upgrade_install(&name, &chart, &vfile).await?;
    }
    k::scale(&client, &name, 1).await?;

    ctx.send(reply::pending(format!(
        "Starting `{name}`… this can take up to 10 minutes for modpacks."
    )))
    .await?;

    let public_port = values::read(&values::path_for(&name))
        .ok()
        .map(|v| v.node_port.saturating_sub(5000))
        .unwrap_or(0);

    if k::wait_until_ready(&client, &name, READY_TIMEOUT).await? {
        ctx.send(reply::ok(format!(
            "✅ `{name}` is ready! Connect at `{}:{public_port}`",
            config::public_domain()
        )))
        .await?;
    } else {
        ctx.send(reply::pending(format!(
            "⏳ `{name}` is still starting after 10 min. Use `/status {name}` to check."
        )))
        .await?;
    }
    Ok(())
}
