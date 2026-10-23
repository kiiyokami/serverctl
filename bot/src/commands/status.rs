use crate::{auth, config, kube as k, minecraft, reply, values, Context, Error};
use k8s_openapi::api::apps::v1::Deployment;

pub enum State {
    Stopped,
    Starting,
    Running,
}

/// A server with no Deployment was created but never started.
pub fn state(dep: Option<&Deployment>) -> State {
    match dep {
        Some(d) if k::replicas(d) > 0 && k::available_replicas(d) >= 1 => State::Running,
        Some(d) if k::replicas(d) > 0 => State::Starting,
        _ => State::Stopped,
    }
}

/// One server's status line(s), shared by /status and /list.
pub async fn describe(name: &str, dep: Option<&Deployment>, v: Option<&values::Values>) -> String {
    match state(dep) {
        State::Stopped => format!("⚫ **`{name}`** — stopped"),
        State::Starting => format!("🟡 **`{name}`** — starting…"),
        State::Running => {
            let mut meta = Vec::new();
            if let Some(s) = minecraft::ping(name).await {
                meta.push(format!("{}/{} players", s.online, s.max));
            }
            if let Some(secs) = dep.and_then(k::uptime_secs) {
                meta.push(format!("up {}", minecraft::format_uptime(secs)));
            }
            if let Some(v) = v {
                meta.push(values::summary(v));
            }
            let mut out = format!("🟢 **`{name}`** — running");
            if !meta.is_empty() {
                out.push_str(&format!("\n{}", meta.join(" · ")));
            }
            if let Some(v) = v {
                out.push_str(&format!(
                    "\nConnect: `{}:{}`",
                    config::public_domain(),
                    v.node_port.saturating_sub(5000)
                ));
            }
            out
        }
    }
}

#[poise::command(slash_command, guild_only)]
pub async fn status(
    ctx: Context<'_>,
    #[description = "Server name"]
    #[autocomplete = "super::server_names"]
    name: String,
) -> Result<(), Error> {
    ctx.defer().await?;
    let guild = ctx.guild_id().map(|g| g.to_string()).unwrap_or_default();
    let client = k::client().await?;
    if !auth::guild_owns(&client, &guild, &name).await? {
        ctx.send(reply::unknown_server(&name)).await?;
        return Ok(());
    }

    let dep = k::get_deployment(&client, &name).await?;
    let v = values::read(&values::path_for(&name)).ok();
    let body = describe(&name, dep.as_ref(), v.as_ref()).await;
    let msg = match state(dep.as_ref()) {
        State::Running => reply::ok(body),
        State::Starting => reply::pending(body),
        State::Stopped => reply::info(body),
    };
    ctx.send(msg).await?;
    Ok(())
}
