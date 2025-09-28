use std::sync::LazyLock;

use amanda_aicore::init_opts::Language;

pub const SUPPORTED_LANGUAGES: LazyLock<Vec<Language>> =
    LazyLock::new(|| vec![autoadapt(), english(), spanish()]);

pub fn autoadapt() -> Language {
    Language {
        code: "autoadapt".to_string(),
        pretty_name: "Adapt LLM to your local language".to_string(),
        system_prompt: "You are a helpful assistant that communicates in the user's local language. You should detect the user's language from their input and reply in that language.".to_string(),
    }
}

pub fn english() -> Language {
    Language {
        code: "en".to_string(),
        pretty_name: "English".to_string(),
        system_prompt: "You are a helpful assistant that communicates in English. you should reply in English INDEPEDENT of the user language".to_string(),
    }
}

pub fn spanish() -> Language {
    Language {
        code: "es".to_string(),
        pretty_name: "Spanish / Español".to_string(),
        system_prompt: "You are a helpful assistant that communicates in Spanish. you should reply in spanish INDEPENDENT of the user language".to_string(),
    }
}
