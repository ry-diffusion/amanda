use amanda_shared::color_eyre;
use amanda_shared::tokio;
use amanda_shared::tracing;

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    tracing_subscriber::fmt()
        .pretty()
        .with_env_filter(
            tracing_subscriber::EnvFilter::builder()
                .with_default_directive("amanda=info".parse().unwrap())
                .from_env_lossy(),
        )
        .init();

    tracing::info!("Amanda is booting up.");

    Ok(())
}
