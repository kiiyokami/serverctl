use anyhow::Result;
use poise::serenity_prelude as serenity;

mod auth;
mod commands;
mod config;
mod curseforge;
mod helm;
mod kube;
mod minecraft;
mod modrinth;
mod reply;
mod values;
mod watcher;

pub struct Data {
    pub kube: ::kube::Client,
    pub live_prompts: commands::LivePrompts,
}
pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Context<'a> = poise::Context<'a, Data, Error>;

#[poise::command(slash_command)]
async fn ping(ctx: Context<'_>) -> Result<(), Error> {
    ctx.send(reply::ok("pong")).await?;
    Ok(())
}

async fn on_error(error: poise::FrameworkError<'_, Data, Error>) {
    match error {
        poise::FrameworkError::Command { error, ctx, .. } => {
            let name = &ctx.command().name;
            tracing::error!("/{name} failed: {error}");
            if let Err(e) = ctx.send(reply::failure(name, &error)).await {
                tracing::warn!("couldn't report /{name} failure: {e}");
            }
        }
        other => {
            if let Err(e) = poise::builtins::on_error(other).await {
                tracing::warn!("error handler failed: {e}");
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let token = std::env::var("DISCORD_TOKEN")?;
    let kube = kube::client().await?;
    let intents = serenity::GatewayIntents::non_privileged();

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![
                ping(),
                commands::list::list(),
                commands::status::status(),
                commands::start::start(),
                commands::stop::stop(),
                commands::create::create(),
                commands::delete::delete(),
                commands::mods::mods(),
                commands::curseforge_key::curseforge_key(),
                commands::ttl::ttl(),
                commands::settings::settings(),
            ],
            on_error: |error| Box::pin(on_error(error)),
            event_handler: |ctx, event, _framework, data| {
                Box::pin(commands::expire_stale_click(ctx, event, data))
            },
            ..Default::default()
        })
        .setup(|ctx, _ready, framework| {
            tokio::spawn(watcher::run(ctx.clone(), kube.clone()));
            Box::pin(async move {
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                Ok(Data { kube, live_prompts: Default::default() })
            })
        })
        .build();

    let mut client = serenity::ClientBuilder::new(token, intents)
        .framework(framework)
        .await?;
    client.start().await?;
    Ok(())
}
