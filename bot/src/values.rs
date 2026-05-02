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
}

fn default_image() -> String {
    "itzg/minecraft-server:java21".into()
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

pub fn next_free_node_port(used: &std::collections::HashSet<u32>) -> Result<u32> {
    for port in 30565..=30568 {
        if !used.contains(&port) {
            return Ok(port);
        }
    }
    anyhow::bail!("All NodePorts 30565–30568 are in use")
}
