use anyhow::{anyhow, Result};
use serde::Deserialize;

#[derive(Deserialize)]
struct Version {
    name: String,
    files: Vec<File>,
}

#[derive(Deserialize)]
struct File {
    url: String,
    primary: Option<bool>,
}

fn version_query_url(slug: &str, loaders: &[&str], mc_version: &str) -> String {
    let loaders = loaders
        .iter()
        .map(|l| format!("%22{l}%22"))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "https://api.modrinth.com/v2/project/{slug}/version?loaders=[{loaders}]&game_versions=[%22{mc_version}%22]"
    )
}

/// Newest version of `slug` built for any of `loaders` on `mc_version`.
pub async fn latest_jar(slug: &str, loaders: &[&str], mc_version: &str) -> Result<(String, String)> {
    let url = version_query_url(slug, loaders, mc_version);
    let versions: Vec<Version> = reqwest::Client::new()
        .get(url)
        .header("User-Agent", "serverctl-bot")
        .send()
        .await?
        .json()
        .await?;
    let v = versions
        .into_iter()
        .next()
        .ok_or_else(|| {
            anyhow!("No '{slug}' version for {} on MC {mc_version}", loaders.join("/"))
        })?;
    let jar = v
        .files
        .iter()
        .find(|f| f.primary.unwrap_or(false))
        .or_else(|| v.files.first())
        .map(|f| f.url.clone())
        .ok_or_else(|| anyhow!("no files in version"))?;
    Ok((v.name, jar))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_query_lists_every_loader() {
        assert_eq!(
            version_query_url("luckperms", &["paper", "spigot", "bukkit"], "26.2"),
            "https://api.modrinth.com/v2/project/luckperms/version\
             ?loaders=[%22paper%22,%22spigot%22,%22bukkit%22]&game_versions=[%2226.2%22]"
        );
    }
}
