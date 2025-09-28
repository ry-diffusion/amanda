use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum Provider {
    Google { api_key: String, model: String },
    OpenAI { api_key: String, model: String },
    OpenRouter { api_key: String, model: String },
}
