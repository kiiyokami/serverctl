use poise::serenity_prelude as serenity;

const GREEN: u32 = 0x57F287;
const YELLOW: u32 = 0xFEE75C;
const RED: u32 = 0xED4245;
pub const BLUE: u32 = 0x5865F2;

fn embed(color: u32, body: impl Into<String>) -> poise::CreateReply {
    poise::CreateReply::default()
        .embed(serenity::CreateEmbed::default().description(body).color(color))
}

pub fn ok(body: impl Into<String>) -> poise::CreateReply {
    embed(GREEN, body)
}

pub fn pending(body: impl Into<String>) -> poise::CreateReply {
    embed(YELLOW, body)
}

pub fn err(body: impl Into<String>) -> poise::CreateReply {
    embed(RED, body)
}

pub fn info(body: impl Into<String>) -> poise::CreateReply {
    embed(BLUE, body)
}

/// Same reply for unknown names and other guilds' servers, so neither leaks.
pub fn unknown_server(name: &str) -> poise::CreateReply {
    err(format!("No server named `{name}` here. See `/list`."))
}

/// Unexpected command failure: first line of the error only, since helm stderr can run long.
pub fn failure(command: &str, error: &dyn std::fmt::Display) -> poise::CreateReply {
    let text = error.to_string();
    let line: String = text.lines().next().unwrap_or_default().chars().take(300).collect();
    err(format!("`/{command}` failed: {line}"))
}

/// Replace an interaction's (deferred) response using its token.
pub async fn edit(http: &serenity::Http, token: &str, reply: poise::CreateReply) -> serenity::Result<()> {
    let body = reply.to_slash_initial_response_edit(serenity::EditInteractionResponse::new());
    http.edit_original_interaction_response(token, &body, vec![]).await?;
    Ok(())
}
