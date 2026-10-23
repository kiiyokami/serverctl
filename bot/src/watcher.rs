use crate::{kube as k, minecraft, values};
use poise::serenity_prelude as serenity;
use std::collections::HashMap;

pub async fn run(ctx: serenity::Context) {
    let http = &ctx.http;
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
    let mut last: HashMap<String, u32> = HashMap::new();
    let mut last_activity = String::new();

    loop {
        interval.tick().await;
        let client = match k::client().await {
            Ok(c) => c,
            Err(_) => continue,
        };
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
            if replicas > 0 {
                if let Ok(v) = values::read(&values::path_for(&name)) {
                    if let Some(s) = minecraft::ping(v.node_port as u16).await {
                        players += s.online;
                    }
                }
            }
            let prev = last.get(&name).copied().unwrap_or(replicas);
            if prev > 0 && replicas == 0 && !k::stopped_by_user(&dep) {
                if let Some(ch_str) = k::channel_id(&dep) {
                    if let Ok(ch_id) = ch_str.parse::<u64>() {
                        let channel = serenity::ChannelId::new(ch_id);
                        let embed = serenity::CreateEmbed::default()
                            .description(format!("⚫ **`{name}`** stopped (idle timeout)."))
                            .color(0x5865F2u32);
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
