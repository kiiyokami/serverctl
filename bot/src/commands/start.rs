use crate::{auth, helm, kube as k, values, Context, Error};

const MAX_CONCURRENT: u32 = 2;

#[poise::command(slash_command)]
pub async fn start(
    ctx: Context<'_>,
    #[description = "Server name"] name: String,
) -> Result<(), Error> {
    ctx.defer().await?;
    let guild = ctx.guild_id().map(|g| g.to_string()).unwrap_or_default();
    let client = k::client().await?;

    if !auth::guild_owns(&client, &guild, &name).await? {
        ctx.say(format!("`{name}` isn't managed by this guild."))
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
            ctx.say(format!(
                "Already {running}/{MAX_CONCURRENT} servers running. Stop one first."
            ))
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
    ctx.say(format!(
        "Starting `{name}`. Use `/status {name}` to check."
    ))
    .await?;
    Ok(())
}
