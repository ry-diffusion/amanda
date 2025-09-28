use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Provider {
    Google { api_key: String, model: String },
    OpenAI { api_key: String, model: String },
    OpenRouter { api_key: String, model: String },
}

impl ToString for Provider {
    fn to_string(&self) -> String {
        match self {
            Provider::Google { .. } => "Google".to_string(),
            Provider::OpenAI { .. } => "OpenAI".to_string(),
            Provider::OpenRouter { .. } => "OpenRouter".to_string(),
        }
    }
}
