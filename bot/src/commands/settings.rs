use crate::{auth, kube as k, reply, values, Context, Error};
use poise::ChoiceParameter;

#[derive(Debug, ChoiceParameter)]
pub enum GameMode {
    Survival,
    Creative,
    Adventure,
    Spectator,
}

impl GameMode {
    fn env_value(&self) -> &'static str {
        match self {
            Self::Survival => "survival",
            Self::Creative => "creative",
            Self::Adventure => "adventure",
            Self::Spectator => "spectator",
        }
    }
}

#[derive(Debug, ChoiceParameter)]
pub enum Difficulty {
    Peaceful,
    Easy,
    Normal,
    Hard,
}

impl Difficulty {
    fn env_value(&self) -> &'static str {
        match self {
            Self::Peaceful => "peaceful",
            Self::Easy => "easy",
            Self::Normal => "normal",
            Self::Hard => "hard",
        }
    }
}

/// itzg writes MODE/DIFFICULTY into server.properties; omitted choices keep its defaults.
fn apply_gameplay(v: &mut values::Values, mode: Option<&GameMode>, difficulty: Option<&Difficulty>) {
    let pairs = [
        ("MODE", mode.map(GameMode::env_value)),
        ("DIFFICULTY", difficulty.map(Difficulty::env_value)),
    ];
    for (key, value) in pairs {
        if let Some(value) = value {
            v.extra_env
                .get_or_insert_with(serde_yaml::Mapping::new)
                .insert(key.into(), value.into());
        }
    }
}

fn env<'a>(v: &'a values::Values, key: &str) -> Option<&'a str> {
    v.extra_env.as_ref()?.get(key)?.as_str()
}

#[poise::command(slash_command, guild_only)]
pub async fn settings(
    ctx: Context<'_>,
    #[description = "Server name"]
    #[autocomplete = "super::server_names"]
    name: String,
    #[description = "Default game mode for players"] gamemode: Option<GameMode>,
    #[description = "World difficulty"] difficulty: Option<Difficulty>,
) -> Result<(), Error> {
    ctx.defer().await?;
    let guild = ctx.guild_id().map(|g| g.to_string()).unwrap_or_default();
    let client = k::client().await?;
    if !auth::guild_owns(&client, &guild, &name).await? {
        ctx.send(reply::unknown_server(&name)).await?;
        return Ok(());
    }

    let path = values::path_for(&name);
    let mut v = values::read(&path)?;
    let changed = gamemode.is_some() || difficulty.is_some();
    if changed {
        apply_gameplay(&mut v, gamemode.as_ref(), difficulty.as_ref());
        values::write(&path, &v)?;
    }
    let current = format!(
        "Game mode: **{}** · Difficulty: **{}**",
        env(&v, "MODE").unwrap_or("survival"),
        env(&v, "DIFFICULTY").unwrap_or("easy"),
    );
    let msg = if changed {
        reply::ok(format!(
            "✅ Saved for **`{name}`**.\n{current}\nApplies on the next `/start`."
        ))
    } else {
        reply::info(format!("**`{name}`**\n{current}"))
    };
    ctx.send(msg).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> values::Values {
        serde_yaml::from_str(
            "name: t\nnodePort: 30565\nserver:\n  type: PAPER\n  version: \"1.21.4\"\n  memory: 4G\n",
        )
        .unwrap()
    }

    #[test]
    fn gameplay_choices_set_mode_and_difficulty() {
        let mut v = base();
        apply_gameplay(&mut v, Some(&GameMode::Creative), Some(&Difficulty::Peaceful));
        assert_eq!(env(&v, "MODE"), Some("creative"));
        assert_eq!(env(&v, "DIFFICULTY"), Some("peaceful"));
    }

    #[test]
    fn omitted_gameplay_choices_leave_server_defaults() {
        let mut v = base();
        apply_gameplay(&mut v, None, None);
        assert!(v.extra_env.is_none());
    }

    #[test]
    fn every_choice_maps_to_an_itzg_value() {
        let modes = [
            (GameMode::Survival, "survival"),
            (GameMode::Creative, "creative"),
            (GameMode::Adventure, "adventure"),
            (GameMode::Spectator, "spectator"),
        ];
        for (m, want) in modes {
            assert_eq!(m.env_value(), want);
        }
        let diffs = [
            (Difficulty::Peaceful, "peaceful"),
            (Difficulty::Easy, "easy"),
            (Difficulty::Normal, "normal"),
            (Difficulty::Hard, "hard"),
        ];
        for (d, want) in diffs {
            assert_eq!(d.env_value(), want);
        }
    }
}
