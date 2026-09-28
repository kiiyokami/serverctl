use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Values {
    pub name: String,
    #[serde(rename = "nodePort")]
    pub node_port: u32,
    #[serde(rename = "discordGuildId", default, skip_serializing_if = "String::is_empty")]
    pub discord_guild_id: String,
    #[serde(default = "default_image", skip_serializing_if = "String::is_empty")]
    pub image: String,
    pub server: ServerConfig,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resources: Option<serde_yaml::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage: Option<serde_yaml::Value>,
    #[serde(rename = "extraEnv", default, skip_serializing_if = "Option::is_none")]
    pub extra_env: Option<serde_yaml::Mapping>,
    /// Discord user ID whose stored CurseForge key the server uses (secret key name).
    #[serde(rename = "cfApiKeyUser", default, skip_serializing_if = "Option::is_none")]
    pub cf_api_key_user: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    #[serde(rename = "type")]
    pub kind: String,
    pub version: String,
    pub memory: String,
    #[serde(rename = "onlineMode", default = "default_true")]
    pub online_mode: bool,
    #[serde(default)]
    pub mods: Vec<String>,
    
    #[serde(rename = "curseforgeFiles", default, skip_serializing_if = "Vec::is_empty")]
    pub cf_files: Vec<String>,
}

fn default_image() -> String {
    "itzg/minecraft-server:java21".into()
}

/// Pick the itzg image whose bundled JVM matches a Minecraft version.
/// Unknown or unparseable versions fall back to the newest JVM.
pub fn java_image_for(version: &str) -> String {
    let mut parts = version.trim().split('.');
    let (major, minor, patch) = (
        parts.next().and_then(|s| s.parse::<u32>().ok()),
        parts.next().and_then(|s| s.parse::<u32>().ok()),
        parts.next().and_then(|s| s.parse::<u32>().ok()).unwrap_or(0),
    );
    let tag = match (major, minor) {
        (Some(1), Some(m)) if m <= 16 => "java8",
        (Some(1), Some(17)) => "java16",
        (Some(1), Some(m)) if m <= 19 => "java17",
        (Some(1), Some(20)) if patch <= 4 => "java17",
        (Some(y), Some(_)) if y >= 26 => "java25",
        _ => "java21",
    };
    format!("itzg/minecraft-server:{tag}")
}

/// Set the server version and matching Java image. The templates' ZGC
/// JVM_OPTS use -XX:+ZGenerational, which only Java 21 accepts: Java 25
/// removed the flag (generational is the only mode), and pre-21 JVMs
/// either lack it or lack ZGC entirely.
pub fn apply_mc_version(v: &mut Values, version: &str) {
    v.server.version = version.to_string();
    v.image = java_image_for(version);
    let Some(extra) = v.extra_env.as_mut() else { return };
    let is_zgc = extra
        .get("JVM_OPTS")
        .and_then(|o| o.as_str())
        .is_some_and(|o| o.contains("ZGC"));
    if !is_zgc || v.image.ends_with(":java21") {
        return;
    }
    if v.image.ends_with(":java25") {
        extra.insert("JVM_OPTS".into(), "-XX:+UseZGC".into());
    } else {
        extra.remove("JVM_OPTS");
    }
}
fn default_true() -> bool {
    true
}

fn repo_root() -> PathBuf {
    PathBuf::from(std::env::var("SERVERCTL_REPO_ROOT").unwrap_or_else(|_| "/serverctl".into()))
}

pub fn servers_dir() -> PathBuf {
    repo_root().join("k8s/helm/values/servers")
}

pub fn templates_dir() -> PathBuf {
    repo_root().join("k8s/helm/values/templates")
}

pub fn chart_dir() -> PathBuf {
    repo_root().join("k8s/helm/minecraft")
}

pub fn path_for(name: &str) -> PathBuf {
    servers_dir().join(format!("{name}.yaml"))
}


pub fn read(path: &Path) -> Result<Values> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("reading {}", path.display()))?;
    Ok(serde_yaml::from_str(&text)?)
}

pub fn write(path: &Path, v: &Values) -> Result<()> {
    let text = serde_yaml::to_string(v)?;
    std::fs::write(path, text)?;
    Ok(())
}

/// Pin the world seed. Minecraft accepts numeric or text seeds; it only
/// reads the seed when generating a new world, so this matters at create time.
pub fn apply_seed(v: &mut Values, seed: &str) -> std::result::Result<(), String> {
    let seed = seed.trim();
    if seed.is_empty() || seed.chars().count() > 64 {
        return Err("Seed must be 1–64 characters (e.g. `-4172144997902289642`).".into());
    }
    v.extra_env
        .get_or_insert_with(serde_yaml::Mapping::new)
        .insert("SEED".into(), seed.into());
    Ok(())
}

pub fn next_free_node_port(used: &std::collections::HashSet<u32>) -> Result<u32> {
    for port in 30565..=30568 {
        if !used.contains(&port) {
            return Ok(port);
        }
    }
    anyhow::bail!("All NodePorts 30565–30568 are in use")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_up_to_1_16_get_java8() {
        assert_eq!(java_image_for("1.7.10"), "itzg/minecraft-server:java8");
        assert_eq!(java_image_for("1.12.2"), "itzg/minecraft-server:java8");
        assert_eq!(java_image_for("1.16.5"), "itzg/minecraft-server:java8");
    }

    #[test]
    fn versions_1_17_get_java16() {
        assert_eq!(java_image_for("1.17"), "itzg/minecraft-server:java16");
        assert_eq!(java_image_for("1.17.1"), "itzg/minecraft-server:java16");
    }

    #[test]
    fn versions_1_18_to_1_20_4_get_java17() {
        assert_eq!(java_image_for("1.18"), "itzg/minecraft-server:java17");
        assert_eq!(java_image_for("1.19.4"), "itzg/minecraft-server:java17");
        assert_eq!(java_image_for("1.20"), "itzg/minecraft-server:java17");
        assert_eq!(java_image_for("1.20.4"), "itzg/minecraft-server:java17");
    }

    #[test]
    fn versions_1_20_5_and_up_get_java21() {
        assert_eq!(java_image_for("1.20.5"), "itzg/minecraft-server:java21");
        assert_eq!(java_image_for("1.20.6"), "itzg/minecraft-server:java21");
        assert_eq!(java_image_for("1.21"), "itzg/minecraft-server:java21");
        assert_eq!(java_image_for("1.21.1"), "itzg/minecraft-server:java21");
    }

    fn values_with_jvm_opts(version: &str) -> Values {
        serde_yaml::from_str(&format!(
            "name: t\nnodePort: 30565\nserver:\n  type: FORGE\n  version: \"{version}\"\n  memory: 4G\nextraEnv:\n  JVM_OPTS: \"-XX:+UseZGC -XX:+ZGenerational\"\n  OTHER: \"keep\"\n"
        ))
        .unwrap()
    }

    #[test]
    fn apply_mc_version_strips_zgc_opts_on_older_java() {
        let mut v = values_with_jvm_opts("1.21.1");
        apply_mc_version(&mut v, "1.12.2");
        assert_eq!(v.server.version, "1.12.2");
        assert_eq!(v.image, "itzg/minecraft-server:java8");
        let extra = v.extra_env.as_ref().unwrap();
        assert!(!extra.contains_key("JVM_OPTS"));
        assert!(extra.contains_key("OTHER"));
    }

    #[test]
    fn apply_mc_version_keeps_zgc_opts_on_java21() {
        let mut v = values_with_jvm_opts("1.21.1");
        apply_mc_version(&mut v, "1.21.1");
        assert_eq!(v.image, "itzg/minecraft-server:java21");
        assert!(v.extra_env.as_ref().unwrap().contains_key("JVM_OPTS"));
    }

    #[test]
    fn year_based_versions_get_java25() {
        assert_eq!(java_image_for("26.1"), "itzg/minecraft-server:java25");
        assert_eq!(java_image_for("26.1.2"), "itzg/minecraft-server:java25");
        assert_eq!(java_image_for("27.1"), "itzg/minecraft-server:java25");
    }

    #[test]
    fn apply_mc_version_drops_zgenerational_on_java25() {
        let mut v = values_with_jvm_opts("1.21.1");
        apply_mc_version(&mut v, "26.1");
        assert_eq!(v.image, "itzg/minecraft-server:java25");
        let extra = v.extra_env.as_ref().unwrap();
        assert_eq!(extra.get("JVM_OPTS").unwrap().as_str(), Some("-XX:+UseZGC"));
        assert!(extra.contains_key("OTHER"));
    }

    fn values_without_extra_env() -> Values {
        serde_yaml::from_str(
            "name: t\nnodePort: 30565\nserver:\n  type: PAPER\n  version: \"1.21.4\"\n  memory: 4G\n",
        )
        .unwrap()
    }

    #[test]
    fn apply_seed_sets_seed_env() {
        let mut v = values_with_jvm_opts("1.21.1");
        apply_seed(&mut v, " -4172144997902289642 ").unwrap();
        let extra = v.extra_env.as_ref().unwrap();
        assert_eq!(extra.get("SEED").unwrap().as_str(), Some("-4172144997902289642"));
        assert!(extra.contains_key("JVM_OPTS"));
    }

    #[test]
    fn apply_seed_creates_extra_env_when_missing() {
        let mut v = values_without_extra_env();
        apply_seed(&mut v, "glacier").unwrap();
        assert_eq!(
            v.extra_env.unwrap().get("SEED").unwrap().as_str(),
            Some("glacier")
        );
    }

    #[test]
    fn numeric_seed_survives_yaml_round_trip_as_string() {
        let mut v = values_without_extra_env();
        apply_seed(&mut v, "12345").unwrap();
        let back: Values = serde_yaml::from_str(&serde_yaml::to_string(&v).unwrap()).unwrap();
        assert_eq!(back.extra_env.unwrap().get("SEED").unwrap().as_str(), Some("12345"));
    }

    #[test]
    fn apply_seed_rejects_blank_and_oversized() {
        let mut v = values_without_extra_env();
        assert!(apply_seed(&mut v, "   ").is_err());
        assert!(apply_seed(&mut v, &"x".repeat(65)).is_err());
        assert!(v.extra_env.is_none());
    }

    #[test]
    fn unparseable_versions_fall_back_to_java21() {
        assert_eq!(java_image_for(""), "itzg/minecraft-server:java21");
        assert_eq!(java_image_for("latest"), "itzg/minecraft-server:java21");
        assert_eq!(java_image_for("1"), "itzg/minecraft-server:java21");
    }
}
