use std::sync::OnceLock;

use iroha_i18n::{self, Bundle, Language, Localizer};

static LOCALIZER: OnceLock<Localizer> = OnceLock::new();

/// Detect the language, inspecting overrides and environment variables.
pub fn detect_language(lang_override: Option<&str>) -> Language {
    iroha_i18n::detect_language(lang_override)
}

/// Initialize the global localizer for daemon messages.
pub fn init(language: Language) {
    LOCALIZER.get_or_init(|| Localizer::new(Bundle::Daemon, language));
}

/// Translate the given key using the initialized localizer.
pub fn t(key: &str) -> String {
    LOCALIZER
        .get()
        .map_or_else(|| key.to_string(), |loc| loc.t(key))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_translation() {
        init(Language::English);
        assert_eq!(t("info.welcome"), "Welcome to Hyperledger Iroha!");
    }
}
