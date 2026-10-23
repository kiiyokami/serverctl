use crate::{auth, reply, values, Context, Error};
use poise::ChoiceParameter;

#[derive(Debug, Clone, Copy, PartialEq, ChoiceParameter)]
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

#[derive(Debug, Clone, Copy, PartialEq, ChoiceParameter)]
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

/// Gameplay settings shared by /create and /settings; `None` keeps the server's current value.
#[derive(Debug, Default, Clone)]
pub struct Gameplay {
    pub mode: Option<GameMode>,
    pub difficulty: Option<Difficulty>,
    pub offline: Option<bool>,
    pub hardcore: Option<bool>,
    pub pvp: Option<bool>,
}

impl Gameplay {
    /// What a new server gets unless the creator changes it.
    pub fn new_server() -> Self {
        Self {
            mode: Some(GameMode::Survival),
            difficulty: Some(Difficulty::Easy),
            offline: Some(false),
            hardcore: Some(false),
            pvp: Some(true),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.mode.is_none()
            && self.difficulty.is_none()
            && self.offline.is_none()
            && self.hardcore.is_none()
            && self.pvp.is_none()
    }

    /// itzg writes these env vars into server.properties on every start.
    pub fn apply(&self, v: &mut values::Values) {
        let bool_str = |b: bool| if b { "true" } else { "false" };
        let pairs = [
            ("MODE", self.mode.map(|m| m.env_value())),
            ("DIFFICULTY", self.difficulty.map(|d| d.env_value())),
            ("HARDCORE", self.hardcore.map(bool_str)),
            ("PVP", self.pvp.map(bool_str)),
        ];
        for (key, value) in pairs {
            if let Some(value) = value {
                v.extra_env
                    .get_or_insert_with(serde_yaml::Mapping::new)
                    .insert(key.into(), value.into());
            }
        }
        if let Some(offline) = self.offline {
            v.server.online_mode = !offline;
        }
    }
}

fn env<'a>(v: &'a values::Values, key: &str) -> Option<&'a str> {
    v.extra_env.as_ref()?.get(key)?.as_str()
}

/// One-line summary of a server's gameplay settings.
pub fn describe(v: &values::Values) -> String {
    let on_off = |b: bool| if b { "on" } else { "off" };
    format!(
        "Game mode: **{}** · Difficulty: **{}**\nOffline mode: **{}** · Hardcore: **{}** · PvP: **{}**",
        env(v, "MODE").unwrap_or("survival"),
        env(v, "DIFFICULTY").unwrap_or("easy"),
        on_off(!v.server.online_mode),
        on_off(env(v, "HARDCORE") == Some("true")),
        on_off(env(v, "PVP") != Some("false")),
    )
}

/// Shown whenever offline mode is on: the server stops checking Mojang accounts.
pub const OFFLINE_WARNING: &str =
    "⚠️ Offline mode lets anyone join under any username, including yours.";

#[allow(clippy::too_many_arguments)]
#[poise::command(slash_command, guild_only)]
pub async fn settings(
    ctx: Context<'_>,
    #[description = "Server name"]
    #[autocomplete = "super::server_names"]
    name: String,
    #[description = "Default game mode for players"] gamemode: Option<GameMode>,
    #[description = "World difficulty"] difficulty: Option<Difficulty>,
    #[description = "Let players join without a Mojang account check"] offline_mode: Option<bool>,
    #[description = "Hardcore: one life, difficulty locked to hard"] hardcore: Option<bool>,
    #[description = "Players can hurt each other"] pvp: Option<bool>,
) -> Result<(), Error> {
    ctx.defer().await?;
    let guild = ctx.guild_id().map(|g| g.to_string()).unwrap_or_default();
    let client = ctx.data().kube.clone();
    if !auth::guild_owns(&client, &guild, &name).await? {
        ctx.send(reply::unknown_server(&name)).await?;
        return Ok(());
    }

    let path = values::path_for(&name);
    let mut v = values::read(&path)?;
    let change = Gameplay { mode: gamemode, difficulty, offline: offline_mode, hardcore, pvp };
    if !change.is_empty() {
        change.apply(&mut v);
        values::write(&path, &v)?;
    }
    let mut body = describe(&v);
    if !v.server.online_mode {
        body = format!("{body}\n{OFFLINE_WARNING}");
    }
    let msg = if change.is_empty() {
        reply::info(format!("**`{name}`**\n{body}"))
    } else {
        reply::ok(format!("✅ Saved for **`{name}`**.\n{body}\nApplies on the next `/start`."))
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
        let g = Gameplay {
            mode: Some(GameMode::Creative),
            difficulty: Some(Difficulty::Peaceful),
            offline: Some(true),
            hardcore: Some(true),
            pvp: Some(false),
        };
        g.apply(&mut v);
        assert_eq!(env(&v, "MODE"), Some("creative"));
        assert_eq!(env(&v, "DIFFICULTY"), Some("peaceful"));
        assert_eq!(env(&v, "HARDCORE"), Some("true"));
        assert_eq!(env(&v, "PVP"), Some("false"));
        assert!(!v.server.online_mode);
    }

    #[test]
    fn omitted_gameplay_choices_leave_server_defaults() {
        let mut v = base();
        Gameplay::default().apply(&mut v);
        assert!(v.extra_env.is_none());
        assert!(v.server.online_mode);
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
