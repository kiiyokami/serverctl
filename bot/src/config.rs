pub fn public_domain() -> String {
    std::env::var("PUBLIC_DOMAIN").unwrap_or_else(|_| "your-domain".into())
}
