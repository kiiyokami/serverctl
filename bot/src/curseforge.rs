use crate::values;
use anyhow::{anyhow, Result};
use serde::Deserialize;

const API: &str = "https://api.curseforge.com/v1";
const MINECRAFT: u32 = 432;
const MODPACKS_CLASS: u32 = 4471;

#[derive(Deserialize)]
struct Resp<T> {
    data: T,
}

#[derive(Deserialize)]
struct Project {
    id: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct File {
    pub file_name: String,
    /// None when the author disallows third-party downloads.
    pub download_url: Option<String>,
    #[serde(default)]
    pub is_server_pack: bool,
}

/// Look up one file of a modpack by its page slug and file ID.
pub async fn modpack_file(key: &str, slug: &str, file_id: u64) -> Result<File> {
    let http = reqwest::Client::new();
    let get = |url: String| {
        http.get(url)
            .header("x-api-key", key)
            .header("User-Agent", "serverctl-bot")
    };
    let projects: Resp<Vec<Project>> = get(format!(
        "{API}/mods/search?gameId={MINECRAFT}&classId={MODPACKS_CLASS}&slug={slug}"
    ))
    .send()
    .await?
    .error_for_status()?
    .json()
    .await?;
    let id = projects
        .data
        .first()
        .ok_or_else(|| anyhow!("no CurseForge modpack named `{slug}`"))?
        .id;
    let file: Resp<File> = get(format!("{API}/mods/{id}/files/{file_id}"))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(file.data)
}

/// Point the server at a specific modpack file. Server packs run as
/// TYPE=CURSEFORGE from a zip the chart's init container downloads with the
/// owner's API key; regular (client) files pin AUTO_CURSEFORGE to that file.
/// Returns a short description for the Discord reply.
pub fn apply_file(
    v: &mut values::Values,
    slug: &str,
    file_id: u64,
    page_url: &str,
    f: &File,
) -> std::result::Result<String, String> {
    if !f.is_server_pack {
        let extra = v.extra_env.get_or_insert_with(serde_yaml::Mapping::new);
        extra.insert("TYPE".into(), "AUTO_CURSEFORGE".into());
        extra.insert("CF_PAGE_URL".into(), page_url.into());
        return Ok(format!("CurseForge modpack `{slug}` ({})", f.file_name));
    }
    let Some(ref download) = f.download_url else {
        return Err(format!(
            "The author of `{slug}` blocks automatic downloads of `{}`. \
             Download it in a browser and place it on the server manually.",
            f.file_name
        ));
    };
    let extra = v.extra_env.get_or_insert_with(serde_yaml::Mapping::new);
    extra.insert("TYPE".into(), "CURSEFORGE".into());
    extra.insert(
        "CF_SERVER_MOD".into(),
        format!("/data/serverpack-{file_id}.zip").into(),
    );
    extra.insert("CF_SERVER_PACK_URL".into(), download.clone().into());
    extra.insert(
        "CF_PAGE_URL".into(),
        format!("https://www.curseforge.com/minecraft/modpacks/{slug}").into(),
    );
    Ok(format!("CurseForge server pack `{slug}` ({})", f.file_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> values::Values {
        serde_yaml::from_str(
            "name: t\nnodePort: 30565\nserver:\n  type: FORGE\n  version: \"1.21.1\"\n  memory: 14G\n",
        )
        .unwrap()
    }

    fn env(v: &values::Values, key: &str) -> Option<String> {
        v.extra_env
            .as_ref()
            .and_then(|e| e.get(key))
            .and_then(|x| x.as_str())
            .map(str::to_string)
    }

    const URL: &str = "https://www.curseforge.com/minecraft/modpacks/all-the-mods-10/files/8945094";

    #[test]
    fn file_parses_from_api_json() {
        let r: Resp<File> = serde_json::from_str(
            r#"{"data":{"id":8945094,"fileName":"ServerFiles-8.2.zip",
                "downloadUrl":"https://edge.forgecdn.net/files/8945/94/ServerFiles-8.2.zip",
                "isServerPack":true,"fileLength":1100000000}}"#,
        )
        .unwrap();
        assert_eq!(r.data.file_name, "ServerFiles-8.2.zip");
        assert!(r.data.is_server_pack);
        assert!(r.data.download_url.is_some());
    }

    #[test]
    fn server_pack_switches_to_curseforge_type() {
        let mut v = base();
        let f = File {
            file_name: "ServerFiles-8.2.zip".into(),
            download_url: Some("https://edge.forgecdn.net/files/8945/94/ServerFiles-8.2.zip".into()),
            is_server_pack: true,
        };
        apply_file(&mut v, "all-the-mods-10", 8945094, URL, &f).unwrap();
        assert_eq!(env(&v, "TYPE").as_deref(), Some("CURSEFORGE"));
        assert_eq!(env(&v, "CF_SERVER_MOD").as_deref(), Some("/data/serverpack-8945094.zip"));
        assert_eq!(
            env(&v, "CF_SERVER_PACK_URL").as_deref(),
            Some("https://edge.forgecdn.net/files/8945/94/ServerFiles-8.2.zip")
        );
        assert_eq!(
            env(&v, "CF_PAGE_URL").as_deref(),
            Some("https://www.curseforge.com/minecraft/modpacks/all-the-mods-10")
        );
    }

    #[test]
    fn client_file_pins_auto_curseforge() {
        let mut v = base();
        let f = File {
            file_name: "All the Mods 10-8.2.zip".into(),
            download_url: Some("https://edge.forgecdn.net/x.zip".into()),
            is_server_pack: false,
        };
        apply_file(&mut v, "all-the-mods-10", 8945094, URL, &f).unwrap();
        assert_eq!(env(&v, "TYPE").as_deref(), Some("AUTO_CURSEFORGE"));
        assert_eq!(env(&v, "CF_PAGE_URL").as_deref(), Some(URL));
        assert!(env(&v, "CF_SERVER_MOD").is_none());
    }

    #[test]
    fn blocked_download_is_rejected() {
        let mut v = base();
        let f = File {
            file_name: "Server.zip".into(),
            download_url: None,
            is_server_pack: true,
        };
        assert!(apply_file(&mut v, "some-pack", 1, URL, &f).is_err());
        assert!(v.extra_env.is_none());
    }
}
