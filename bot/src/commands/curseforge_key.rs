use crate::{kube as k, reply, Context, Error};

#[poise::command(slash_command, rename = "curseforge-key")]
pub async fn curseforge_key(
    ctx: Context<'_>,
    #[description = "Your CurseForge API key (from console.curseforge.com, starts with $2a$10$)"]
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

    let mut msg = "✅ Saved your CurseForge API key. It'll be used for CurseForge packs and mods you add.\n\
         Get or rotate a key at <https://console.curseforge.com/>."
        .to_string();
    if !key.starts_with("$2a$") {
        // Valid CurseForge Eternal API keys are bcrypt strings; a different shape
        // usually means a legacy token, which AUTO_CURSEFORGE rejects.
        msg.push_str(
            "\n\n⚠️ Heads up: valid keys start with `$2a$10$`. Yours doesn't — if downloads \
             fail, you likely grabbed a *legacy* token instead of a console.curseforge.com key.",
        );
    }
    ctx.send(reply::ok(msg).ephemeral(true)).await?;
    Ok(())
}
