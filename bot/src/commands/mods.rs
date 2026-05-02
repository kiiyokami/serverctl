use crate::{auth, kube as k, modrinth, reply, values, Context, Error};
use regex::Regex;

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

    let modpack_re = Regex::new(r"^https?://modrinth\.com/modpack/([^/?#]+)")?;
    let mod_re = Regex::new(r"^https?://modrinth\.com/mod/([^/?#]+)")?;

    if let Some(c) = modpack_re.captures(&url) {
        let slug = c.get(1).unwrap().as_str();
        let extra = v.extra_env.get_or_insert_with(serde_yaml::Mapping::new);
        extra.insert("TYPE".into(), "MODRINTH".into());
        extra.insert("MODRINTH_PROJECT".into(), slug.into());
        ctx.send(reply::ok(format!(
            "✅ Configured Modrinth modpack `{slug}` for `{name}`."
        )))
        .await?;
    } else if let Some(c) = mod_re.captures(&url) {
        let slug = c.get(1).unwrap().as_str();
        let loader = v.server.kind.to_lowercase();
        let mc = v.server.version.clone();
        let (vn, jar) = modrinth::latest_jar(slug, &loader, &mc).await?;
        if !v.server.mods.contains(&jar) {
            v.server.mods.push(jar);
        }
        ctx.send(reply::ok(format!("✅ Added `{slug}` ({vn}) to `{name}`.")))
            .await?;
    } else if url.to_lowercase().ends_with(".jar") {
        if !v.server.mods.contains(&url) {
            v.server.mods.push(url);
        }
        ctx.send(reply::ok(format!("✅ Added direct mod URL to `{name}`.")))
            .await?;
    } else {
        ctx.send(reply::err("Unrecognized URL.")).await?;
        return Ok(());
    }

    values::write(&path, &v)?;
    Ok(())
}
