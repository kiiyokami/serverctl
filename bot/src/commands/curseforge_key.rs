use crate::{kube as k, reply, Context, Error};

#[poise::command(slash_command, rename = "curseforge-key")]
pub async fn curseforge_key(
    ctx: Context<'_>,
    #[description = "Your CurseForge API key (from legacy.curseforge.com/account/api-tokens)"]
    key: String,
) -> Result<(), Error> {
    let key = key.trim().to_string();
    if key.is_empty() {
        ctx.send(reply::err("Key can't be empty.").ephemeral(true))
            .await?;
        return Ok(());
    }

    let user_id = ctx.author().id.to_string();
    let client = k::client().await?;
    k::set_curseforge_key(&client, &user_id, &key).await?;

    ctx.send(
        reply::ok(
            "✅ Saved your CurseForge API key. It'll be used for CurseForge packs and mods you add.\n\
             Get or rotate a key at <https://legacy.curseforge.com/account/api-tokens>.",
        )
        .ephemeral(true),
    )
    .await?;
    Ok(())
}
