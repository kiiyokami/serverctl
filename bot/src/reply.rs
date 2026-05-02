use poise::serenity_prelude as serenity;

const GREEN: u32 = 0x57F287;
const YELLOW: u32 = 0xFEE75C;
const RED: u32 = 0xED4245;
const BLUE: u32 = 0x5865F2;

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
