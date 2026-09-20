use std::{env, net::SocketAddr};

use levus::api;
use tracing_subscriber::{EnvFilter, fmt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("levus=info,tower_http=info"));
    fmt().with_env_filter(filter).json().init();

    let bind = env::var("LEVUS_BIND")
        .unwrap_or_else(|_| "127.0.0.1:8080".to_owned())
        .parse::<SocketAddr>()?;
    let database_url = env::var("DATABASE_URL")
        .ok()
        .filter(|value| !value.is_empty());
    let demo_enabled = env::var("LEVUS_ENABLE_DEMO")
        .map(|value| value.eq_ignore_ascii_case("true") || value == "1")
        .unwrap_or(false);

    api::run(bind, database_url, demo_enabled).await
}
