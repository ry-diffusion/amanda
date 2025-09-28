pub struct Persona {
    pub name: String,
    pub description: String,
    pub instructions: String,
}

pub struct Language {
    pub code: String,
    pub pretty_name: String,
    pub system_prompt: String,
}

pub struct InitOptions {
    pub language: Language,
    pub persona: Persona,
    pub extra_instructions: Option<String>,
}
