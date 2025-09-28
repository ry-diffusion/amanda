use amanda_aicore::genai;
use amanda_aicore::genai::resolver::{AuthData, AuthResolver};
use amanda_aicore::init_opts::InitOptions;
use amanda_aicore::providers::Provider;
use amanda_aicore::tools::ToolStore;
use amanda_lowiq::chat::{AmandaChat, TurnEvent};
use amanda_lowiq::{languages::SUPPORTED_LANGUAGES, personas::SUPPORTED_PERSONAS};
use amanda_shared::tokio::sync::mpsc::unbounded_channel;
use amanda_shared::{tokio, tracing};
use egui::{Context, Label, UiKind};
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};

use crate::settings::Settings;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use tokio::sync::Mutex;

pub enum ChatRole {
    Assistant,
    User,
}

pub struct Message {
    role: ChatRole,
    content: String,
}

enum AssistantUpdate {
    Replace { index: usize, content: String },
}

pub struct AmandaApp {
    on_settings: bool,
    settings: Settings,
    messages: Vec<Message>,
    input: String,
    tx: Sender<AssistantUpdate>,
    rx: Receiver<AssistantUpdate>,
    cache: CommonMarkCache,
    tools: Arc<ToolStore>,
    amanda: Arc<Mutex<Option<AmandaChat>>>,
}

impl AmandaApp {
    fn build_client(settings: Settings, tools: Arc<ToolStore>) -> Option<AmandaChat> {
        let (api_key, model, provider) = match settings.provider.clone() {
            Some(Provider::Google { api_key, model }) => (api_key, model, "Google"),
            Some(Provider::OpenAI { api_key, model }) => (api_key, model, "OpenAI"),
            Some(Provider::OpenRouter { api_key, model }) => (api_key, model, "OpenRouter"),
            None => {
                tracing::error!("No provider configured");
                return None;
            }
        };

        let key = api_key.clone();
        let client = genai::Client::builder()
            .with_auth_resolver(AuthResolver::from_resolver_fn(
                |_iden: genai::ModelIden| -> Result<Option<AuthData>, genai::resolver::Error> {
                    Ok(Some(AuthData::from_single(key)))
                },
            ))
            .build();

        tracing::info!("Loading amanda chat with {model} using {provider}");
        let runner = AmandaChat::new(client, model, tools);
        Some(runner.with_init_options(InitOptions {
            persona: settings.persona.clone(),
            language: settings.language.clone(),
            extra_instructions: None,
        }))
    }

    pub async fn reset_client(&mut self) {
        let new_client = Self::build_client(self.settings.clone(), self.tools.clone());
        let mut amanda_lock = self.amanda.lock().await;
        *amanda_lock = new_client;
        self.messages.clear();
    }

    pub fn new(settings: Settings) -> Self {
        let mut tools = ToolStore::new();
        amanda_tools_hyprland::register_hyprland_tools(&mut tools);
        let finished_ts = Arc::new(tools);
        let amanda = Arc::new(Mutex::new(Self::build_client(
            settings.clone(),
            finished_ts.clone(),
        )));
        let (tx, rx) = mpsc::channel();

        AmandaApp {
            on_settings: false,
            settings,
            input: String::new(),
            cache: CommonMarkCache::default(),
            messages: Vec::from([Message {
                role: ChatRole::Assistant,
                content: "Hello! I'm Amanda, your AI assistant. How can I help you today?"
                    .to_string(),
            }]),
            tx,
            rx,
            tools: finished_ts,
            amanda,
        }
    }
    fn settings(&mut self, ctx: &Context) {
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
                                model: "gemini-2.5-flash".to_string(),
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
    }
}

impl eframe::App for AmandaApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        while let Ok(update) = self.rx.try_recv() {
            match update {
                AssistantUpdate::Replace { index, content } => {
                    if let Some(msg) = self.messages.get_mut(index) {
                        msg.content = content;
                    }
                }
            }
        }
        egui::CentralPanel::default().show(ctx, |ui| {
            self.settings(ctx);

            if ui.button("Open Settings").clicked() {
                self.on_settings = true;
            }

            ui.separator();

            let available_height = ui.available_height() - 40.0;
            egui::ScrollArea::vertical()
                .stick_to_bottom(true)
                .auto_shrink([false, false])
                .max_height(available_height.max(100.0))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    for msg in &self.messages {
                        let is_user = matches!(msg.role, ChatRole::User);
                        let layout = if is_user {
                            egui::Layout::right_to_left(egui::Align::TOP)
                        } else {
                            egui::Layout::left_to_right(egui::Align::TOP)
                        };
                        ui.with_layout(layout, |ui| {
                            let max_width = ui.available_width() * 0.8;
                            ui.set_max_width(max_width);
                            let fill = if is_user {
                                ui.style().visuals.widgets.inactive.bg_fill
                            } else {
                                ui.style().visuals.extreme_bg_color
                            };
                            egui::Frame::NONE
                                .fill(fill)
                                .corner_radius(8.0)
                                .inner_margin(8.0)
                                .outer_margin(6.0)
                                .show(ui, |ui| {
                                    // ui.add(Label::new(&msg.content).wrap());
                                    CommonMarkViewer::new().show(ui, &mut self.cache, &msg.content);
                                });
                        });
                    }
                });

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let text_edit = egui::TextEdit::singleline(&mut self.input)
                    .hint_text("Type a message…")
                    .desired_width(f32::INFINITY);
                let response = ui.add(text_edit);
                let send_clicked = ui.button("Send").clicked();
                let enter_pressed =
                    response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if (send_clicked || enter_pressed) && !self.input.trim().is_empty() {
                    let text = self.input.trim().to_owned();
                    self.input.clear();
                    if text == "/clear" {
                        self.messages.clear();
                        ctx.request_repaint();
                    } else {
                        self.messages.push(Message {
                            role: ChatRole::User,
                            content: text.clone(),
                        });
                        // Assistant streaming placeholder
                        let index = {
                            self.messages.push(Message {
                                role: ChatRole::Assistant,
                                content: String::from("Initializing...."),
                            });
                            self.messages.len() - 1
                        };
                        let message_tx = self.tx.clone();
                        let amanda = self.amanda.clone();
                        tokio::spawn(async move {
                            let mut may_amanda = amanda.lock().await;
                            if may_amanda.is_none() {
                                let _ = message_tx.send(AssistantUpdate::Replace {
                                    index,
                                    content: "Error: API Key is not set.".to_string(),
                                });
                                return;
                            }

                            if let Some(amanda) = may_amanda.as_mut() {
                                let (tx, mut rx) = unbounded_channel::<TurnEvent>();

                                tokio::spawn({
                                    let message_tx = message_tx.clone();
                                    async move {
                                        let mut content = String::new();
                                        let mut tool_map = std::collections::HashMap::new();
                                        while let Some(ev) = rx.recv().await {
                                            match ev {
                                                TurnEvent::ModelResponseStart => {}
                                                TurnEvent::AssistantChunk(text) => {
                                                    content.push_str(&text);

                                                    let _ =
                                                        message_tx.send(AssistantUpdate::Replace {
                                                            index,
                                                            content: content.clone(),
                                                        });
                                                }
                                                TurnEvent::ModelResponseEnd => {}
                                                TurnEvent::ToolCallCaptured(tc) => {
                                                    let tool = format!(
                                                        "🔨 {}({})\n",
                                                        tc.fn_name, tc.fn_arguments
                                                    );
                                                    content += &tool;
                                                    tool_map.insert(tc.call_id, tc.fn_name);
                                                    let _ =
                                                        message_tx.send(AssistantUpdate::Replace {
                                                            index,
                                                            content: content.clone(),
                                                        });
                                                }
                                                TurnEvent::ToolExecutionStart {
                                                    call_id: _,
                                                    fn_name,
                                                    args,
                                                } => {
                                                    let tool = format!(
                                                        "⏳ Executing tool: {}({})\n",
                                                        fn_name, args
                                                    );
                                                    content += &tool;
                                                    let _ =
                                                        message_tx.send(AssistantUpdate::Replace {
                                                            index,
                                                            content: content.clone(),
                                                        });
                                                }
                                                TurnEvent::ToolExecutionEnd {
                                                    call_id,
                                                    result: _,
                                                } => {
                                                    let fn_name = tool_map
                                                        .remove(&call_id)
                                                        .unwrap_or("unknown".to_string());
                                                    let tool =
                                                        format!("✅ Tool {} executed.\n", fn_name);
                                                    content += &tool;
                                                    let _ =
                                                        message_tx.send(AssistantUpdate::Replace {
                                                            index,
                                                            content: content.clone(),
                                                        });
                                                }
                                            }
                                        }
                                    }
                                });

                                let send_result = amanda.run_user_turn(&text, tx).await;
                                if let Err(e) = send_result {
                                    let _ = message_tx.send(AssistantUpdate::Replace {
                                        index,
                                        content: format!("Error sending message: {}", e),
                                    });
                                }
                            }
                        });
                        ctx.request_repaint();
                    }
                }
                if enter_pressed {
                    response.request_focus();
                }
            });
        });
    }
}
