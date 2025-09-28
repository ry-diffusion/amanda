use amanda_aicore::genai;
use amanda_aicore::genai::resolver::{AuthData, AuthResolver};
use amanda_aicore::init_opts::InitOptions;
use amanda_aicore::providers::Provider;
use amanda_aicore::tools::ToolStore;
use amanda_lowiq::chat::{AmandaChat, TurnEvent};
use amanda_lowiq::{languages::SUPPORTED_LANGUAGES, personas::SUPPORTED_PERSONAS};
use amanda_shared::tokio::sync::mpsc::unbounded_channel;
use amanda_shared::{tokio, tracing};
use egui::{Context, RichText, UiKind};
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};

use crate::settings::Settings;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

pub enum ChatRole {
    Assistant,
    User,
}

pub struct Message {
    role: ChatRole,
    content: String,
}

pub struct ToolStatus {
    pub text: String,
}

enum AssistantUpdate {
    Replace { index: usize, content: String },
    SetStatus { index: usize, text: String },
    ClearStatus { index: usize },
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
    msg_status: std::collections::HashMap<usize, ToolStatus>,
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
        amanda_tools_standard::register_tools(&mut tools);
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
            msg_status: std::collections::HashMap::new(),
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

    fn process_incoming_updates(&mut self, ctx: &egui::Context) {
        while let Ok(update) = self.rx.try_recv() {
            match update {
                AssistantUpdate::Replace { index, content } => {
                    if let Some(msg) = self.messages.get_mut(index) {
                        msg.content = content;
                    }
                }
                AssistantUpdate::SetStatus { index, text } => {
                    self.msg_status.insert(index, ToolStatus { text });
                }
                AssistantUpdate::ClearStatus { index } => {
                    self.msg_status.remove(&index);
                }
            }
            ctx.request_repaint();
        }
    }

    fn render_messages(&mut self, ui: &mut egui::Ui, _ctx: &egui::Context) {
        let available_height = ui.available_height() - 40.0;
        egui::ScrollArea::vertical()
            .stick_to_bottom(true)
            .auto_shrink([false, false])
            .max_height(available_height.max(100.0))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                for (i, msg) in self.messages.iter().enumerate() {
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
                                ui.vertical(|ui| {
                                    if matches!(msg.role, ChatRole::Assistant) {
                                        if let Some(status) = self.msg_status.get(&i) {
                                            ui.label(
                                                RichText::new(&status.text)
                                                    .small()
                                                    .italics()
                                                    .weak(),
                                            );
                                        }
                                    }
                                    CommonMarkViewer::new().show(ui, &mut self.cache, &msg.content);
                                });
                            });
                    });
                }
            });
    }

    fn handle_input_row(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let text_edit = egui::TextEdit::singleline(&mut self.input)
            .hint_text("Type a message…")
            .desired_width(f32::INFINITY);
        let response = ui.add(text_edit);
        let send_clicked = ui.button("Send").clicked();
        let enter_pressed = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));

        if (send_clicked || enter_pressed) && !self.input.trim().is_empty() {
            let text = self.input.trim().to_owned();
            self.input.clear();
            if text == "/clear" {
                self.messages.clear();
                ctx.request_repaint();
            } else {
                // push user message
                self.messages.push(Message {
                    role: ChatRole::User,
                    content: text.clone(),
                });
                // assistant placeholder and status
                let index = self.prepare_assistant_placeholder();

                let repaint_ctx = ctx.clone();
                let message_tx = self.tx.clone();
                let amanda = self.amanda.clone();
                Self::spawn_turn(index, text, message_tx, amanda, repaint_ctx);

                ctx.request_repaint();
            }
        }
        if enter_pressed {
            response.request_focus();
        }
    }

    fn prepare_assistant_placeholder(&mut self) -> usize {
        self.messages.push(Message {
            role: ChatRole::Assistant,
            content: String::from("Initializing...."),
        });
        let idx = self.messages.len() - 1;
        self.msg_status.insert(
            idx,
            ToolStatus {
                text: "Initializing…".to_string(),
            },
        );
        idx
    }

    fn spawn_turn(
        index: usize,
        text: String,
        message_tx: Sender<AssistantUpdate>,
        amanda: Arc<Mutex<Option<AmandaChat>>>,
        repaint_ctx: egui::Context,
    ) {
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
                let (tx, rx) = unbounded_channel::<TurnEvent>();

                tokio::spawn(Self::run_turn_event_loop(
                    rx,
                    index,
                    message_tx.clone(),
                    repaint_ctx.clone(),
                ));

                let send_result = amanda.run_user_turn(&text, tx).await;
                if let Err(e) = send_result {
                    let _ = message_tx.send(AssistantUpdate::Replace {
                        index,
                        content: format!("Error sending message: {}", e),
                    });
                }
            }
        });
    }

    fn fmt_init(frame: &str) -> String {
        format!("{} Initializing…", frame)
    }

    fn fmt_running(name: &str, elapsed_ms: u128) -> String {
        format!("⏳ Running `{}`… {} ms", name, elapsed_ms)
    }
    fn fmt_ran(name: &str, elapsed_ms: u128) -> String {
        format!("Ran `{}` in {} ms", name, elapsed_ms)
    }
    // Ticker helpers (moved out of inner scope)
    async fn init_spinner_ticker(
        index: usize,
        running: Arc<Mutex<std::collections::HashMap<String, (String, Instant)>>>,
        init_active: Arc<Mutex<bool>>,
        message_tx: Sender<AssistantUpdate>,
        repaint_ctx: egui::Context,
    ) {
        let frames = [".  ", ".. ", "...", " ..", "  .", "   "];
        let mut i = 0usize;
        loop {
            let still_init = *init_active.lock().await;
            let any_running = !running.lock().await.is_empty();
            if !still_init || any_running {
                break;
            }
            let frame = frames[i % frames.len()];
            let _ = message_tx.send(AssistantUpdate::SetStatus {
                index,
                text: Self::fmt_init(frame),
            });
            repaint_ctx.request_repaint();
            i += 1;
            tokio::time::sleep(Duration::from_millis(120)).await;
        }
    }
    async fn running_tools_ticker(
        index: usize,
        running: Arc<Mutex<std::collections::HashMap<String, (String, Instant)>>>,
        message_tx: Sender<AssistantUpdate>,
        repaint_ctx: egui::Context,
    ) {
        loop {
            // pick the most recent running tool
            let status_text = {
                let run = running.lock().await;
                if let Some((_, (name, start))) = run.iter().max_by_key(|(_, (_n, st))| *st) {
                    let elapsed = Instant::now().duration_since(*start).as_millis();
                    Self::fmt_running(name, elapsed)
                } else {
                    String::new()
                }
            };
            if !status_text.is_empty() {
                let _ = message_tx.send(AssistantUpdate::SetStatus {
                    index,
                    text: status_text,
                });
                repaint_ctx.request_repaint();
            }
            tokio::time::sleep(Duration::from_millis(120)).await;
            if running.lock().await.is_empty() {
                break;
            }
        }
    }
    async fn run_turn_event_loop(
        mut rx: tokio::sync::mpsc::UnboundedReceiver<TurnEvent>,
        index: usize,
        message_tx: Sender<AssistantUpdate>,
        repaint_ctx: egui::Context,
    ) {
        // Aggregated assistant content (LLM text)
        let base_content = Arc::new(Mutex::new(String::new()));
        // Currently running tools: call_id -> (fn_name, start_time)
        let running = Arc::new(Mutex::new(std::collections::HashMap::new()));
        // Init spinner active until first chunk or tool start
        let init_active = Arc::new(Mutex::new(true));

        // Init spinner loop
        {
            let running = running.clone();
            let init_active = init_active.clone();
            let message_tx = message_tx.clone();
            let repaint_ctx = repaint_ctx.clone();
            tokio::spawn(Self::init_spinner_ticker(
                index,
                running,
                init_active,
                message_tx,
                repaint_ctx,
            ));
        }

        while let Some(ev) = rx.recv().await {
            match ev {
                TurnEvent::ModelResponseStart => {}
                TurnEvent::AssistantChunk(text) => {
                    {
                        let mut base = base_content.lock().await;
                        base.push_str(&text);
                    }
                    {
                        let mut init = init_active.lock().await;
                        if *init {
                            *init = false;
                            if running.lock().await.is_empty() {
                                let _ = message_tx.send(AssistantUpdate::ClearStatus { index });
                            }
                        }
                    }

                    // Render current content + any running tool overlays
                    let rendered = {
                        let base = base_content.lock().await.clone();
                        let run = running.lock().await;
                        if run.is_empty() {
                            base
                        } else {
                            let mut s = base
                                .lines()
                                .filter(|l| {
                                    let t = l.trim_start();
                                    !(t.starts_with("🧰") || t.starts_with("⏳"))
                                })
                                .collect::<Vec<_>>()
                                .join("\n");
                            s.push_str("\n\n");
                            for (_id, (name, start)) in run.iter() {
                                let elapsed = Instant::now().duration_since(*start);
                                s.push_str(&format!(
                                    "Running `{}`… {} ms\n",
                                    name,
                                    elapsed.as_millis()
                                ));
                            }
                            s
                        }
                    };
                    let _ = message_tx.send(AssistantUpdate::Replace {
                        index,
                        content: rendered,
                    });
                }
                TurnEvent::ModelResponseEnd => {}
                TurnEvent::ToolCallCaptured(_tc) => {}
                TurnEvent::ToolExecutionStart {
                    call_id,
                    fn_name,
                    args: _,
                } => {
                    // Stop init spinner
                    {
                        let mut init = init_active.lock().await;
                        *init = false;
                    }
                    // Insert running tool and note if we need to start a ticker
                    let mut start_ticker = false;
                    {
                        let mut run = running.lock().await;
                        if run.is_empty() {
                            start_ticker = true;
                        }
                        run.insert(call_id.clone(), (fn_name.clone(), Instant::now()));
                    }
                    // Spawn a ticker to animate and update elapsed time while any tool runs
                    if start_ticker {
                        let running_c = running.clone();
                        let message_tx_c = message_tx.clone();
                        let repaint_ctx_c = repaint_ctx.clone();
                        tokio::spawn(Self::running_tools_ticker(
                            index,
                            running_c,
                            message_tx_c,
                            repaint_ctx_c,
                        ));
                    }
                }
                TurnEvent::ToolExecutionEnd { call_id, result: _ } => {
                    if let Some((name, start)) = {
                        let mut run = running.lock().await;
                        run.remove(&call_id)
                    } {
                        let elapsed = Instant::now().duration_since(start).as_millis();
                        let _ = message_tx.send(AssistantUpdate::SetStatus {
                            index,
                            text: Self::fmt_ran(&name, elapsed),
                        });
                        repaint_ctx.request_repaint();
                    }
                }
            }
        }
    }
}

impl eframe::App for AmandaApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.process_incoming_updates(ctx);

        egui::CentralPanel::default().show(ctx, |ui| {
            self.settings(ctx);

            if ui.button("Open Settings").clicked() {
                self.on_settings = true;
            }

            ui.separator();

            // Render messages list
            self.render_messages(ui, ctx);

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                // Input row + send behavior
                self.handle_input_row(ui, ctx);
            });
        });
    }
}
