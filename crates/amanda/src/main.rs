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

            // Load Instrument Sans/Serif and set as default fonts
            let mut fonts = egui::FontDefinitions::default();

            fonts.font_data.insert(
                "InstrumentSans-Regular".to_owned(),
                std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
                    "../assets/Instrument_Sans/static/InstrumentSans-Regular.ttf"
                ))),
            );
            fonts.font_data.insert(
                "InstrumentSerif-Regular".to_owned(),
                std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
                    "../assets/Instrument_Serif/InstrumentSerif-Regular.ttf"
                ))),
            );

            // Use Instrument Sans for proportional (body) text
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, "InstrumentSans-Regular".to_owned());

            // Create a named family for Instrument Serif to use for headings
            fonts.families.insert(
                egui::FontFamily::Name("InstrumentSerif".into()),
                vec!["InstrumentSerif-Regular".to_owned()],
            );

            cc.egui_ctx.set_fonts(fonts);

            // Make headings use the serif family while body stays proportional (sans)
            let mut style = (*cc.egui_ctx.style()).clone();
            if let Some(font_id) = style.text_styles.get_mut(&egui::TextStyle::Heading) {
                font_id.family = egui::FontFamily::Name("InstrumentSerif".into());
            }
            // Set default body text size to 24.0
            style.text_styles.insert(
                egui::TextStyle::Body,
                egui::FontId::new(24.0, egui::FontFamily::Proportional),
            );
            cc.egui_ctx.set_style(style);

            let amanda = AmandaApp::new(settings);
            let boxed_amanda = Box::new(amanda);

            Ok(boxed_amanda)
        }),
    )
    .expect("SO can WE WANDER FOR A SPELL AND LIVE IN PARALEL?");

    Ok(())
}
