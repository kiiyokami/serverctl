use crate::{kube as k, minecraft, reply};
use poise::serenity_prelude as serenity;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Idle shutdown delay for servers without a /ttl.
const DEFAULT_TTL_SECS: u64 = 300;

fn idle_expired(idle: Duration, ttl_secs: u64) -> bool {
    ttl_secs > 0 && idle.as_secs() >= ttl_secs
}

pub async fn run(ctx: serenity::Context, client: kube::Client) {
    let http = &ctx.http;
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
    let mut last: HashMap<String, u32> = HashMap::new();
    let mut last_activity = String::new();
    // In memory: a bot restart restarts every idle timer, which only delays shutdowns.
    let mut idle_since: HashMap<String, Instant> = HashMap::new();

    loop {
        interval.tick().await;
        let deps = match k::list_deployments(&client).await {
            Ok(d) => d,
            Err(_) => continue,
        };
        let mut players = 0;
        for dep in deps {
            let name = match dep.metadata.name.as_deref() {
                Some(n) => n.to_string(),
                None => continue,
            };
            if k::guild_id(&dep).is_none() {
                continue;
            }
            let replicas = k::replicas(&dep);
            if replicas == 0 {
                idle_since.remove(&name);
            } else {
                match minecraft::ping(&name).await {
                    Some(s) if s.online > 0 => {
                        players += s.online;
                        idle_since.remove(&name);
                    }
                    Some(_) => {
                        let since = *idle_since.entry(name.clone()).or_insert_with(Instant::now);
                        let ttl = k::ttl_secs(&dep).unwrap_or(DEFAULT_TTL_SECS);
                        // The stop notice goes out on the next tick, when replicas reads 0.
                        if idle_expired(since.elapsed(), ttl) && k::scale(&client, &name, 0).await.is_ok() {
                            idle_since.remove(&name);
                        }
                    }
                    // Starting up or crashed: leave the timer as it is.
                    None => {}
                }
            }
            let prev = last.get(&name).copied().unwrap_or(replicas);
            if prev > 0 && replicas == 0 && !k::stopped_by_user(&dep) {
                if let Some(ch_str) = k::channel_id(&dep) {
                    if let Ok(ch_id) = ch_str.parse::<u64>() {
                        let channel = serenity::ChannelId::new(ch_id);
                        let embed = serenity::CreateEmbed::default()
                            .description(format!("⚫ **`{name}`** stopped (idle timeout)."))
                            .color(reply::BLUE);
                        let msg = serenity::CreateMessage::new().embed(embed);
                        let _ = channel.send_message(http, msg).await;
                    }
                }
            }
            last.insert(name, replicas);
        }
        let activity = format!("{players} player{} online", if players == 1 { "" } else { "s" });
        if activity != last_activity {
            ctx.set_activity(Some(serenity::ActivityData::custom(&activity)));
            last_activity = activity;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_shutdown_respects_ttl() {
        assert!(!idle_expired(Duration::from_secs(299), 300));
        assert!(idle_expired(Duration::from_secs(300), 300));
        assert!(!idle_expired(Duration::from_secs(86_400), 0));
    }
}
