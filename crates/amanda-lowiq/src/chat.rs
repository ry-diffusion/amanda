use amanda_aicore::genai::Client;
use amanda_aicore::genai::chat::{
    ChatMessage, ChatOptions, ChatRequest, ChatStreamEvent, ToolCall, ToolResponse,
};
use amanda_aicore::init_opts::{InitOptions, Persona};
use amanda_aicore::tools::ToolStore;
use amanda_shared::color_eyre::Result as EyreResult;
use amanda_shared::futures::{StreamExt, future::join_all};
use amanda_shared::serde_json::json;
use amanda_shared::tokio::sync::mpsc::UnboundedSender;
use std::error::Error;
use std::sync::Arc;

pub const SYSTEM_INSTRUCTIONS: &str = r#""#;

#[derive(Clone, Debug)]
pub enum TurnEvent {
    /// Signals the model started streaming a response
    ModelResponseStart,
    /// A chunk of assistant text content
    AssistantChunk(String),
    /// Signals the model finished streaming a response
    ModelResponseEnd,
    /// A tool call was captured at the end of a model response
    ToolCallCaptured(ToolCall),
    /// A tool execution started
    ToolExecutionStart {
        call_id: String,
        fn_name: String,
        args: String,
    },
    /// A tool finished executing (result is a JSON/text string)
    ToolExecutionEnd { call_id: String, result: String },
}

/// Application-level chat message used internally by ChatRunner.
/// Keeps a full transcript, including tool calls and tool results.
#[derive(Clone, Debug)]
enum AppChatMessage {
    System(String),
    User(String),
    Assistant(String),
    ToolCall(ToolCall),
    ToolResult(ToolResponse),
}

impl AppChatMessage {
    fn to_user(msg: impl Into<String>) -> Self {
        AppChatMessage::User(msg.into())
    }
    #[allow(dead_code)]
    fn to_system(msg: impl Into<String>) -> Self {
        AppChatMessage::System(msg.into())
    }
    fn to_assistant(msg: impl Into<String>) -> Self {
        AppChatMessage::Assistant(msg.into())
    }
}

/// ChatRunner encapsula o loop de chat com:
/// - 100% streaming do conteúdo do assistente
/// - Suporte a multi-tool-call por turno (executando em paralelo)
/// - Histórico interno (transcript) para permitir múltiplos turnos
pub struct AmandaChat {
    client: Client,
    model: String,
    helper: Arc<ToolStore>,
    transcript: Vec<AppChatMessage>,
    chat_options: ChatOptions,
}

impl AmandaChat {
    /// Cria um novo ChatRunner. Por padrão, ativa a captura de tool calls.
    pub fn new(client: Client, model: impl Into<String>, helper: Arc<ToolStore>) -> Self {
        Self {
            client,
            model: model.into(),
            helper,
            transcript: Vec::new(),
            chat_options: ChatOptions::default().with_capture_tool_calls(true),
        }
    }

    pub fn with_init_options(mut self, init_opts: InitOptions) {
        self.transcript().clear();

        self.transcript.push(AppChatMessage::to_system(format!(
            r#"
            <instructions>
            You are an AI assistant named "{}".
            {}
            </instructions>
            <persona>
            {}
            </persona>
            <extra_instructions>
            {}
            </extra_instructions>
            <language>
            {}
            </language>
            "#,
            SYSTEM_INSTRUCTIONS,
            init_opts.persona.name,
            init_opts.persona.description,
            init_opts.persona.instructions,
            init_opts.language.system_prompt
        )));
    }

    /// Altera as opções de chat.
    #[allow(dead_code)]
    pub fn with_chat_options(mut self, options: ChatOptions) -> Self {
        self.chat_options = options;
        self
    }

    /// Adiciona uma mensagem de sistema ao transcript antes de rodar o chat.
    #[allow(dead_code)]
    pub fn push_system_message(&mut self, content: impl Into<String>) {
        self.transcript.push(AppChatMessage::to_system(content));
    }

    /// Executa um turno completo para uma mensagem do usuário.
    /// - Streama a resposta do assistente em tempo real
    /// - Captura TODAS as tool calls do turno
    /// - Executa as tools em paralelo e envia os resultados
    /// - Repete até não haver mais tool calls no turno
    pub async fn run_user_turn(
        &mut self,
        user_message: impl Into<String>,
        events_tx: UnboundedSender<TurnEvent>,
    ) -> EyreResult<()> {
        self.transcript.push(AppChatMessage::to_user(user_message));

        loop {
            let chat_req =
                Self::build_chat_request(&self.transcript, self.helper.get_genai_tools());

            let mut chat_stream = self
                .client
                .exec_chat_stream(&self.model, chat_req, Some(&self.chat_options))
                .await?;

            let mut assistant_text = String::new();
            let mut captured_tool_calls: Vec<ToolCall> = Vec::new();

            while let Some(event) = chat_stream.stream.next().await {
                match event? {
                    ChatStreamEvent::Start => {
                        let _ = events_tx.send(TurnEvent::ModelResponseStart);
                    }
                    ChatStreamEvent::Chunk(chunk) => {
                        // 100% stream do texto
                        let _ = events_tx.send(TurnEvent::AssistantChunk(chunk.content.clone()));
                        assistant_text.push_str(&chunk.content);
                    }
                    ChatStreamEvent::ToolCallChunk(_tool_chunk) => {
                        // Stream também dos pedaços de tool call
                        // let tc = &tool_chunk.tool_call;
                        // println!(
                        //     "\n[tool-call chunk] fn={} args={}",
                        //     tc.fn_name, tc.fn_arguments
                        // );
                    }
                    ChatStreamEvent::End(end) => {
                        let _ = events_tx.send(TurnEvent::ModelResponseEnd);
                        if let Some(calls) = end.captured_into_tool_calls() {
                            captured_tool_calls = calls;
                            for c in &captured_tool_calls {
                                let _ = events_tx.send(TurnEvent::ToolCallCaptured(c.clone()));
                            }
                        }
                    }
                    _ => {}
                }
            }

            // Anexa a resposta do assistente no transcript (mesmo que haja tools)
            if !assistant_text.trim().is_empty() {
                self.transcript
                    .push(AppChatMessage::to_assistant(assistant_text.clone()));
            }

            // Se não há tool calls, encerramos o turno
            if captured_tool_calls.is_empty() {
                break;
            }

            // Anexa todas as tool calls ao transcript
            for call in &captured_tool_calls {
                self.transcript.push(AppChatMessage::ToolCall(call.clone()));
            }

            let helper = Arc::clone(&self.helper);
            let exec_tx = events_tx.clone();
            let exec_futs = captured_tool_calls.iter().map(move |call| {
                let name = call.fn_name.clone();
                let args = call.fn_arguments.clone();
                let call_id = call.call_id.clone();
                let fn_name = call.fn_name.clone();
                let args_display = call.fn_arguments.to_string();
                let helper = Arc::clone(&helper);
                let tx = exec_tx.clone();
                async move {
                    let _ = tx.send(TurnEvent::ToolExecutionStart {
                        call_id: call_id.clone(),
                        fn_name: fn_name.clone(),
                        args: args_display.clone(),
                    });
                    let result = helper.execute_tool(&name, args).await;
                    (call_id, result)
                }
            });

            let results = join_all(exec_futs).await;

            // Anexa resultados das tools ao transcript
            for (call_id, res) in results {
                match res {
                    Ok(value) => {
                        let resp_str = value.to_string();
                        let tool_response = ToolResponse::new(call_id.clone(), resp_str.clone());
                        self.transcript
                            .push(AppChatMessage::ToolResult(tool_response));
                        let _ = events_tx.send(TurnEvent::ToolExecutionEnd {
                            call_id,
                            result: resp_str,
                        });
                    }
                    Err(e) => {
                        let err_str =
                            json!({ "error": format!("Tool execution failed: {}", e) }).to_string();
                        let tool_response = ToolResponse::new(call_id.clone(), err_str.clone());
                        self.transcript
                            .push(AppChatMessage::ToolResult(tool_response));
                        let _ = events_tx.send(TurnEvent::ToolExecutionEnd {
                            call_id,
                            result: err_str,
                        });
                    }
                }
            }

            // println!("--- Tool execution completed. Sending tool results back to model. ---");
            // Continua o loop: modelo receberá os resultados e poderá responder ou
            // solicitar novas ferramentas neste mesmo turno.
        }

        Ok(())
    }

    /// Retorna uma cópia do transcript interno.
    #[allow(dead_code)]
    pub fn transcript(&self) -> Vec<String> {
        self.transcript
            .iter()
            .map(|m| match m {
                AppChatMessage::System(s) => format!("system: {}", s),
                AppChatMessage::User(s) => format!("user: {}", s),
                AppChatMessage::Assistant(s) => format!("assistant: {}", s),
                AppChatMessage::ToolCall(tc) => {
                    format!("tool_call: {} {}", tc.fn_name, tc.fn_arguments)
                }
                AppChatMessage::ToolResult(tr) => format!("tool_result: {}", tr.content),
            })
            .collect()
    }

    fn build_chat_request(
        transcript: &[AppChatMessage],
        available_tools: Vec<amanda_aicore::genai::chat::Tool>,
    ) -> ChatRequest {
        // Build the request preserving the full chronological order,
        // including tool calls and tool results.
        let mut req_opt: Option<ChatRequest> = None;

        for m in transcript {
            match m {
                AppChatMessage::System(s) => {
                    let msg = ChatMessage::system(s);
                    req_opt = Some(match req_opt {
                        Some(req) => req.append_message(msg),
                        None => ChatRequest::new(vec![msg]),
                    });
                }
                AppChatMessage::User(s) => {
                    let msg = ChatMessage::user(s);
                    req_opt = Some(match req_opt {
                        Some(req) => req.append_message(msg),
                        None => ChatRequest::new(vec![msg]),
                    });
                }
                AppChatMessage::Assistant(s) => {
                    let msg = ChatMessage::assistant(s);
                    req_opt = Some(match req_opt {
                        Some(req) => req.append_message(msg),
                        None => ChatRequest::new(vec![msg]),
                    });
                }
                AppChatMessage::ToolCall(c) => {
                    // Ensure tool calls appear exactly where they happened in the transcript
                    req_opt = Some(match req_opt {
                        Some(req) => req.append_message(vec![c.clone()]),
                        None => ChatRequest::new(Vec::new()).append_message(vec![c.clone()]),
                    });
                }
                AppChatMessage::ToolResult(r) => {
                    // Ensure tool results follow the corresponding tool calls in order
                    req_opt = Some(match req_opt {
                        Some(req) => req.append_message(r.clone()),
                        None => ChatRequest::new(Vec::new()).append_message(r.clone()),
                    });
                }
            }
        }

        let req = req_opt.unwrap_or_else(|| ChatRequest::new(Vec::new()));
        req.with_tools(available_tools)
    }
}
