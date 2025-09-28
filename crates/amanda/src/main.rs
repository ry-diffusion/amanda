use amanda_shared::color_eyre;
use amanda_shared::color_eyre::Section;
use amanda_shared::color_eyre::eyre::Context;
use amanda_shared::tokio;
use amanda_shared::tracing;

mod app;
mod settings;

use crate::app::AmandaApp;
use crate::settings::Settings;

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

    let settings = Settings::load_or_create()?;

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([320.0, 240.0]),
        ..Default::default()
    };

    eframe::run_native(
        "HyprAmanda",
        options,
        Box::new(|cc| {
            // This gives us image support:
            egui_extras::install_image_loaders(&cc.egui_ctx);

            let amanda = AmandaApp::new(settings);
            let boxed_amanda = Box::new(amanda);

            Ok(boxed_amanda)
        }),
    )
    .expect("SO can WE WANDER FOR A SPELL AND LIVE IN PARALEL?");

    Ok(())
}
