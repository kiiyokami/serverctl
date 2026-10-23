use crate::{auth, config, helm, kube as k, reply, values, Context, Error};
use std::time::Duration;
use tokio::sync::Mutex;

pub const MAX_CONCURRENT: u32 = 2;
const READY_TIMEOUT: Duration = Duration::from_secs(600);

// Serializes count-then-scale so two simultaneous starts can't both pass the limit.
static START_LOCK: Mutex<()> = Mutex::const_new(());

/// Scale `name` up and report until it is ready. Returns `Some(running)`
/// without starting anything when the concurrent limit is already reached.
pub async fn launch(ctx: Context<'_>, client: &kube::Client, name: &str) -> Result<Option<u32>, Error> {
    let path = values::path_for(name);
    {
        let _guard = START_LOCK.lock().await;
        let already_up = k::get_deployment(client, name)
            .await?
            .as_ref()
            .map(k::replicas)
            .unwrap_or(0)
            > 0;
        if !already_up {
            let running = k::running_server_count(client).await?;
            if running >= MAX_CONCURRENT {
                return Ok(Some(running));
            }
            // Re-apply the values file every start so /mods changes take effect.
            helm::upgrade_install(name, &values::chart_dir(), &path).await?;
        }
        k::scale(client, name, 1).await?;
    }
    let _ = k::mark_started(client, name, &ctx.channel_id().to_string()).await;

    let v = values::read(&path).ok();
    let public_port = v.as_ref().map(|v| v.node_port).unwrap_or(0).saturating_sub(5000);
    let ver_str = v.as_ref().map(values::summary).unwrap_or_default();

    let handle = ctx.send(reply::pending(format!(
        "🟡 **`{name}`** is starting ({ver_str})\nThis can take up to 10 minutes for modpacks."
    )))
    .await?;

    if k::wait_until_ready(client, name, READY_TIMEOUT).await? {
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
    Ok(None)
}

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

    if let Some(running) = launch(ctx, &client, &name).await? {
        ctx.send(reply::err(format!(
            "Already {running}/{MAX_CONCURRENT} servers running. Stop one first."
        )))
        .await?;
    }
    Ok(())
}
