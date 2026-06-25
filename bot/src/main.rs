use anyhow::Result;
use poise::serenity_prelude as serenity;

mod auth;
mod commands;
mod config;
mod helm;
mod kube;
mod minecraft;
mod modrinth;
mod reply;
mod values;
mod watcher;

pub struct Data {}
pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Context<'a> = poise::Context<'a, Data, Error>;

#[poise::command(slash_command)]
async fn ping(ctx: Context<'_>) -> Result<(), Error> {
    ctx.send(reply::ok("pong")).await?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let token = std::env::var("DISCORD_TOKEN")?;
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
            ],
            ..Default::default()
        })
        .setup(|ctx, _ready, framework| {
            let http = std::sync::Arc::clone(&ctx.http);
            tokio::spawn(watcher::run(http));
            Box::pin(async move {
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                Ok(Data {})
            })
        })
        .build();

    let mut client = serenity::ClientBuilder::new(token, intents)
        .framework(framework)
        .await?;
    client.start().await?;
    Ok(())
}
