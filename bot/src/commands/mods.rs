use crate::{auth, kube as k, modrinth, reply, values, Context, Error};
use regex::Regex;

pub enum ModResult {
    Modpack(String),
    Mod(String, String),
    Jar,
    CurseForgeModpack(String),
    CurseForgeMod(String),
    /// A recognized URL that can't be applied here; carries a user-facing reason.
    Rejected(String),
    Unrecognized,
}

/// Parse a single mod/modpack URL and apply its effect to `v`.
///
/// CurseForge URLs require a per-user API key; this only records which user's key
/// to use (`cf_api_key_user`). Whether that key actually exists is verified later by
/// [`apply_mod_urls`] so this stays free of network/cluster access for CurseForge paths.
pub async fn apply_mod_url(
    v: &mut values::Values,
    url: &str,
    user_id: &str,
) -> Result<ModResult, Error> {
    let modpack_re = Regex::new(r"^https?://modrinth\.com/modpack/([^/?#]+)")?;
    let mod_re = Regex::new(r"^https?://modrinth\.com/mod/([^/?#]+)")?;
    let cf_modpack_re =
        Regex::new(r"^https?://(?:www\.|legacy\.)?curseforge\.com/minecraft/modpacks/([^/?#]+)")?;
    let cf_mod_re =
        Regex::new(r"^https?://(?:www\.|legacy\.)?curseforge\.com/minecraft/mc-mods/([^/?#]+)")?;

    if let Some(c) = modpack_re.captures(url) {
        let slug = c.get(1).unwrap().as_str().to_string();
        let extra = v.extra_env.get_or_insert_with(serde_yaml::Mapping::new);
        extra.insert("TYPE".into(), "MODRINTH".into());
        extra.insert("MODRINTH_PROJECT".into(), slug.clone().into());
        Ok(ModResult::Modpack(slug))
    } else if let Some(c) = mod_re.captures(url) {
        let slug = c.get(1).unwrap().as_str().to_string();
        let loader = v.server.kind.to_lowercase();
        let mc = v.server.version.clone();
        let (vn, jar) = modrinth::latest_jar(&slug, &loader, &mc).await?;
        if !v.server.mods.contains(&jar) {
            v.server.mods.push(jar);
        }
        Ok(ModResult::Mod(slug, vn))
    } else if let Some(c) = cf_modpack_re.captures(url) {
        let slug = c.get(1).unwrap().as_str().to_string();
        let extra = v.extra_env.get_or_insert_with(serde_yaml::Mapping::new);
        extra.insert("TYPE".into(), "AUTO_CURSEFORGE".into());
        extra.insert("CF_PAGE_URL".into(), url.into());
        v.cf_api_key_user = Some(user_id.to_string());
        Ok(ModResult::CurseForgeModpack(slug))
    } else if let Some(c) = cf_mod_re.captures(url) {
        if v.server.kind.eq_ignore_ascii_case("vanilla") {
            return Ok(ModResult::Rejected(
                "CurseForge mods need a Fabric or Forge server, not Vanilla.".into(),
            ));
        }
        let slug = c.get(1).unwrap().as_str().to_string();
        if !v.server.cf_files.contains(&slug) {
            v.server.cf_files.push(slug.clone());
        }
        v.cf_api_key_user = Some(user_id.to_string());
        Ok(ModResult::CurseForgeMod(slug))
    } else if url.to_lowercase().ends_with(".jar") {
        let url = url.to_string();
        if !v.server.mods.contains(&url) {
            v.server.mods.push(url);
        }
        Ok(ModResult::Jar)
    } else {
        Ok(ModResult::Unrecognized)
    }
}

/// Apply a comma-separated list of mod/modpack URLs to `v`.
///
/// Returns `Ok(Ok(descriptions))` on success, or `Ok(Err(message))` with a
/// user-facing error to display (unrecognized/rejected URL, or a missing
/// CurseForge key). Inner `Err` does not mutate the persisted state by itself —
/// callers must not write `v` when an error is returned.
pub async fn apply_mod_urls(
    v: &mut values::Values,
    client: &kube::Client,
    user_id: &str,
    urls: &str,
) -> Result<Result<Vec<String>, String>, Error> {
    let mut added: Vec<String> = Vec::new();
    let mut needs_cf_key = false;
    for u in urls.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        match apply_mod_url(v, u, user_id).await? {
            ModResult::Modpack(slug) => added.push(format!("modpack `{slug}`")),
            ModResult::Mod(slug, vn) => added.push(format!("`{slug}` ({vn})")),
            ModResult::Jar => added.push("direct jar".into()),
            ModResult::CurseForgeModpack(slug) => {
                needs_cf_key = true;
                added.push(format!("CurseForge modpack `{slug}`"));
            }
            ModResult::CurseForgeMod(slug) => {
                needs_cf_key = true;
                added.push(format!("CurseForge mod `{slug}`"));
            }
            ModResult::Rejected(msg) => return Ok(Err(msg)),
            ModResult::Unrecognized => return Ok(Err(format!("Unrecognized URL: `{u}`"))),
        }
    }
    if needs_cf_key && k::get_curseforge_key(client, user_id).await?.is_none() {
        return Ok(Err(
            "You haven't set a CurseForge API key yet. Run `/curseforge-key` first \
             (get one at <https://legacy.curseforge.com/account/api-tokens>)."
                .into(),
        ));
    }
    Ok(Ok(added))
}

#[poise::command(slash_command)]
pub async fn mods(
    ctx: Context<'_>,
    #[description = "Server name"] name: String,
    #[description = "Mod / modpack URL or .jar URL"] url: String,
) -> Result<(), Error> {
    ctx.defer().await?;
    let guild = ctx.guild_id().map(|g| g.to_string()).unwrap_or_default();
    let client = k::client().await?;
    if !auth::guild_owns(&client, &guild, &name).await? {
        ctx.send(reply::err(format!("`{name}` isn't managed by this guild.")))
            .await?;
        return Ok(());
    }

    let path = values::path_for(&name);
    let mut v = values::read(&path)?;
    let user_id = ctx.author().id.to_string();

    match apply_mod_urls(&mut v, &client, &user_id, &url).await? {
        Ok(added) => {
            values::write(&path, &v)?;
            ctx.send(reply::ok(format!("✅ Added {} to `{name}`.", added.join(", "))))
                .await?;
        }
        Err(msg) => {
            ctx.send(reply::err(msg)).await?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_values(kind: &str) -> values::Values {
        serde_yaml::from_str(&format!(
            "name: t\nnodePort: 30565\nserver:\n  type: {kind}\n  version: \"1.21.1\"\n  memory: 4G\n"
        ))
        .unwrap()
    }

    #[tokio::test]
    async fn recognizes_curseforge_modpack() {
        let mut v = base_values("FABRIC");
        let r = apply_mod_url(
            &mut v,
            "https://www.curseforge.com/minecraft/modpacks/all-the-mods-10",
            "123",
        )
        .await
        .unwrap();
        assert!(matches!(r, ModResult::CurseForgeModpack(ref s) if s == "all-the-mods-10"));
        assert_eq!(v.cf_api_key_user.as_deref(), Some("123"));
        let extra = v.extra_env.unwrap();
        assert_eq!(extra.get("TYPE").unwrap().as_str(), Some("AUTO_CURSEFORGE"));
        assert_eq!(
            extra.get("CF_PAGE_URL").unwrap().as_str(),
            Some("https://www.curseforge.com/minecraft/modpacks/all-the-mods-10")
        );
    }

    #[tokio::test]
    async fn recognizes_legacy_curseforge_modpack() {
        let mut v = base_values("FORGE");
        let r = apply_mod_url(
            &mut v,
            "https://legacy.curseforge.com/minecraft/modpacks/all-the-mods-10",
            "1",
        )
        .await
        .unwrap();
        assert!(matches!(r, ModResult::CurseForgeModpack(_)));
    }

    #[tokio::test]
    async fn curseforge_mod_appends_to_cf_files() {
        let mut v = base_values("FABRIC");
        let r = apply_mod_url(
            &mut v,
            "https://www.curseforge.com/minecraft/mc-mods/jei",
            "42",
        )
        .await
        .unwrap();
        assert!(matches!(r, ModResult::CurseForgeMod(ref s) if s == "jei"));
        assert_eq!(v.server.cf_files, vec!["jei".to_string()]);
        assert_eq!(v.cf_api_key_user.as_deref(), Some("42"));
    }

    #[tokio::test]
    async fn curseforge_mod_rejected_on_vanilla() {
        let mut v = base_values("VANILLA");
        let r = apply_mod_url(
            &mut v,
            "https://www.curseforge.com/minecraft/mc-mods/jei",
            "42",
        )
        .await
        .unwrap();
        assert!(matches!(r, ModResult::Rejected(_)));
        assert!(v.server.cf_files.is_empty());
        assert!(v.cf_api_key_user.is_none());
    }

    #[tokio::test]
    async fn unknown_url_is_unrecognized() {
        let mut v = base_values("FABRIC");
        let r = apply_mod_url(&mut v, "https://example.com/whatever", "1")
            .await
            .unwrap();
        assert!(matches!(r, ModResult::Unrecognized));
    }
}
