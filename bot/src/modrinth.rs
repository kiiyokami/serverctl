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

pub async fn latest_jar(slug: &str, loader: &str, mc_version: &str) -> Result<(String, String)> {
    let url = format!(
        "https://api.modrinth.com/v2/project/{slug}/version?loaders=[%22{loader}%22]&game_versions=[%22{mc_version}%22]"
    );
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
        .ok_or_else(|| anyhow!("No '{slug}' version for {loader} on MC {mc_version}"))?;
    let jar = v
        .files
        .iter()
        .find(|f| f.primary.unwrap_or(false))
        .or_else(|| v.files.first())
        .map(|f| f.url.clone())
        .ok_or_else(|| anyhow!("no files in version"))?;
    Ok((v.name, jar))
}
