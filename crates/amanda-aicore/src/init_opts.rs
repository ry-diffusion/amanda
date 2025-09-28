#[derive(Clone, Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct Persona {
    pub name: String,
    pub description: String,
    pub instructions: String,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct Language {
    pub code: String,
    pub pretty_name: String,
    pub system_prompt: String,
}
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct InitOptions {
    pub language: Language,
    pub persona: Persona,
    pub extra_instructions: Option<String>,
}
