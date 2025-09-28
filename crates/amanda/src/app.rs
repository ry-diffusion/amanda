use amanda_aicore::providers::Provider;
use amanda_lowiq::{languages::SUPPORTED_LANGUAGES, personas::SUPPORTED_PERSONAS};
use amanda_shared::tracing;
use egui::UiKind;

use crate::settings::Settings;

pub struct AmandaApp {
    on_settings: bool,
    settings: Settings,
}

impl AmandaApp {
    pub fn new(settings: Settings) -> Self {
        AmandaApp {
            on_settings: false,
            settings,
        }
    }
}

impl eframe::App for AmandaApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::Window::new("Amanda Settings")
                .open(&mut self.on_settings)
                .show(ctx, |ui| {
                    // select provider from dropdown
                    ui.label("Settings");
                    ui.separator();
                    egui::ComboBox::from_label("Persona")
                        .selected_text(format!("{:?}", self.settings.persona.name))
                        .show_ui(ui, |c| {
                            SUPPORTED_PERSONAS.iter().for_each(|persona| {
                                c.selectable_value(
                                    &mut self.settings.persona,
                                    persona.clone(),
                                    format!("{:?}", persona.name),
                                );
                            });
                        });

                    egui::ComboBox::from_label("Language")
                        .selected_text(format!("{:?}", self.settings.language.pretty_name))
                        .show_ui(ui, |c| {
                            SUPPORTED_LANGUAGES.iter().for_each(|lang| {
                                c.selectable_value(
                                    &mut self.settings.language,
                                    lang.clone(),
                                    format!("{:?}", lang.pretty_name),
                                );
                            });
                        });

                    egui::ComboBox::from_label("Provider")
                        .selected_text(match &self.settings.provider {
                            Some(p) => p.to_string(),
                            None => "None".to_string(),
                        })
                        .show_ui(ui, |c| {
                            c.selectable_value(&mut self.settings.provider, None, "None");
                            c.selectable_value(
                                &mut self.settings.provider,
                                Some(Provider::Google {
                                    api_key: "".to_string(),
                                    model: "gpt-3.5-turbo".to_string(),
                                }),
                                "Google",
                            );

                            c.selectable_value(
                                &mut self.settings.provider,
                                Some(Provider::OpenAI {
                                    api_key: "".to_string(),
                                    model: "gpt-3.5-turbo".to_string(),
                                }),
                                "OpenAI",
                            );

                            c.selectable_value(
                                &mut self.settings.provider,
                                Some(Provider::OpenRouter {
                                    api_key: "".to_string(),
                                    model: "open-llama-3b-v2".to_string(),
                                }),
                                "OpenRouter",
                            );
                        });

                    if let Some(provider) = &mut self.settings.provider {
                        tracing::debug!("Current provider: {provider:#?}");
                        match provider {
                            Provider::Google { api_key, model } => {
                                ui.label("Google API Key");
                                // ui.text_edit_singleline(api_key);
                                //
                                //
                                egui::TextEdit::singleline(api_key).password(true).show(ui);
                                ui.label("Model");
                                ui.text_edit_singleline(model);
                            }
                            Provider::OpenAI { api_key, model } => {
                                ui.label("OpenAI API Key");
                                ui.text_edit_singleline(api_key);
                                ui.label("Model");
                                ui.text_edit_singleline(model);
                            }
                            Provider::OpenRouter { api_key, model } => {
                                ui.label("OpenRouter API Key");
                                ui.text_edit_singleline(api_key);
                                ui.label("Model");
                                ui.text_edit_singleline(model);
                            }
                        }
                    }

                    if ui.button("Save").clicked() {
                        if let Err(e) = self.settings.save() {
                            eprintln!("Failed to save settings: {:?}", e);
                        }

                        ui.close_kind(UiKind::Window)
                    }
                });

            if ui.button("Open Settings").clicked() {
                self.on_settings = true;
            }
        });
    }
}
