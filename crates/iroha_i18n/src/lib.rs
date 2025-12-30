//! Internationalization helpers shared across Iroha components.

use std::{
    borrow::Cow,
    collections::HashMap,
    sync::{Arc, OnceLock},
};

use norito::json::{self, Value};

macro_rules! language_catalog {
    ($(
        $variant:ident => {
            tag: $tag:expr,
            file: $file:expr $(,)?
        }
    ),+ $(,)?) => {
        /// Supported languages across the Iroha toolchain.
        #[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
        pub enum Language {
            $(
                #[doc = concat!("Translation bundle for language tag `", $tag, "`.")]
                $variant,
            )+
        }

        impl Language {
            /// Ordered list of all supported languages.
            pub const ALL: &'static [Language] = &[
                $( Language::$variant, )+
            ];

            /// Canonical language tag used in translation bundles.
            pub const fn tag(self) -> &'static str {
                match self {
                    $( Language::$variant => $tag, )+
                }
            }

            #[cfg(test)]
            const fn file_stem(self) -> &'static str {
                match self {
                    $( Language::$variant => $file, )+
                }
            }

            const fn cli_asset(self) -> &'static str {
                match self {
                    $( Language::$variant => include_str!(concat!("lang/cli/", $file, ".json")), )+
                }
            }

            const fn daemon_asset(self) -> &'static str {
                match self {
                    $( Language::$variant => include_str!(concat!("lang/daemon/", $file, ".json")), )+
                }
            }
        }
    };
}

language_catalog! {
    English => { tag: "en", file: "en" },
    Japanese => { tag: "ja", file: "ja" },
    SimplifiedChinese => { tag: "zh-hans", file: "zh-Hans" },
    TraditionalChinese => { tag: "zh-hant", file: "zh-Hant" },
    Thai => { tag: "th", file: "th" },
    Khmer => { tag: "km", file: "km" },
    Vietnamese => { tag: "vi", file: "vi" },
    Korean => { tag: "ko", file: "ko" },
    Arabic => { tag: "ar", file: "ar" },
    Hebrew => { tag: "he", file: "he" },
    Russian => { tag: "ru", file: "ru" },
    Burmese => { tag: "my", file: "my" },
    Hindi => { tag: "hi", file: "hi" },
    Urdu => { tag: "ur", file: "ur" },
    Sinhala => { tag: "si", file: "si" },
    Tamil => { tag: "ta", file: "ta" },
    French => { tag: "fr", file: "fr" },
    Ukrainian => { tag: "uk", file: "uk" },
    Polish => { tag: "pl", file: "pl" },
    Swedish => { tag: "sv", file: "sv" },
    German => { tag: "de", file: "de" },
    Italian => { tag: "it", file: "it" },
    Serbian => { tag: "sr", file: "sr" },
    Turkish => { tag: "tr", file: "tr" },
    Armenian => { tag: "hy", file: "hy" },
    Amharic => { tag: "am", file: "am" },
    Spanish => { tag: "es", file: "es" },
    Farsi => { tag: "fa", file: "fa" },
    OldAkkadian => { tag: "akk", file: "akk" },
    Quechua => { tag: "qu", file: "qu" },
    Aymara => { tag: "ay", file: "ay" },
    Bengali => { tag: "bn", file: "bn" },
    Portuguese => { tag: "pt", file: "pt" },
    Dutch => { tag: "nl", file: "nl" },
    Danish => { tag: "da", file: "da" },
    Norse => { tag: "non", file: "non" },
    Finnish => { tag: "fi", file: "fi" },
    Estonian => { tag: "et", file: "et" },
    Latvian => { tag: "lv", file: "lv" },
    Hungarian => { tag: "hu", file: "hu" },
    Czech => { tag: "cs", file: "cs" },
    Lao => { tag: "lo", file: "lo" },
    Indonesian => { tag: "id", file: "id" },
    Pijin => { tag: "pis", file: "pis" },
    Divehi => { tag: "dv", file: "dv" },
    Punjabi => { tag: "pa", file: "pa" },
    Sindhi => { tag: "sd", file: "sd" },
    Pashto => { tag: "ps", file: "ps" },
    Balochi => { tag: "bal", file: "bal" },
    Saraiki => { tag: "skr", file: "skr" },
    Brahui => { tag: "brh", file: "brh" },
    Kashmiri => { tag: "ks", file: "ks" },
    Nepali => { tag: "ne", file: "ne" },
    Tibetan => { tag: "bo", file: "bo" },
    Swahili => { tag: "sw", file: "sw" },
    Hausa => { tag: "ha", file: "ha" },
    Yoruba => { tag: "yo", file: "yo" },
    Oromo => { tag: "om", file: "om" },
    Igbo => { tag: "ig", file: "ig" },
    Zulu => { tag: "zu", file: "zu" },
    Shona => { tag: "sn", file: "sn" },
    Somali => { tag: "so", file: "so" },
    Afrikaans => { tag: "af", file: "af" },
    Kazakh => { tag: "kk", file: "kk" },
    Bashkir => { tag: "ba", file: "ba" },
    Tatar => { tag: "tt", file: "tt" },
    Greek => { tag: "el", file: "el" },
    Dzongkha => { tag: "dz", file: "dz" },
    Mongolian => { tag: "mn", file: "mn" },
    Javanese => { tag: "jv", file: "jv" },
    Sundanese => { tag: "su", file: "su" },
    Madurese => { tag: "mad", file: "mad" },
    Minangkabau => { tag: "min", file: "min" },
    Balinese => { tag: "ban", file: "ban" },
    AncientEgyptianHieroglyph => { tag: "egy", file: "egy" },
    Manchurian => { tag: "mnc", file: "mnc" },
}

/// Translation bundle identifier. Different executables ship distinct key sets.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Bundle {
    /// CLI messages and diagnostics.
    Cli,
    /// Daemon/server messages.
    Daemon,
}

impl Bundle {
    const fn asset(self, language: Language) -> &'static str {
        match self {
            Bundle::Cli => language.cli_asset(),
            Bundle::Daemon => language.daemon_asset(),
        }
    }

    const fn name(self) -> &'static str {
        match self {
            Bundle::Cli => "cli",
            Bundle::Daemon => "daemon",
        }
    }
}

/// Resolve the most appropriate [`Language`] given optional overrides and locale environment.
pub fn detect_language(lang_override: Option<&str>) -> Language {
    detect_language_from_provider(lang_override, |key| std::env::var(key).ok())
}

fn detect_language_from_provider<F>(lang_override: Option<&str>, mut get_var: F) -> Language
where
    F: FnMut(&str) -> Option<String>,
{
    if let Some(lang) = lang_override.and_then(Language::from_locale) {
        return lang;
    }
    for var in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Some(val) = get_var(var)
            && let Some(lang) = Language::from_locale(&val)
        {
            return lang;
        }
    }
    Language::English
}

impl Language {
    /// Resolve a language from a locale string (BCP47 or glibc-style).
    pub fn from_locale(raw: &str) -> Option<Self> {
        let parts = split_locale(raw)?;
        identify_language(&parts)
    }

    /// Parse a canonical language tag.
    pub fn from_tag(tag: &str) -> Option<Self> {
        let lower = tag.trim().to_ascii_lowercase();
        if lower.is_empty() {
            return None;
        }
        identify_language(&split_locale_inner(&lower))
    }
}

fn split_locale(raw: &str) -> Option<Vec<String>> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let lower = trimmed.to_ascii_lowercase();
    let base = lower.split('.').next().unwrap_or(&lower);
    let normalized = base.replace('-', "_");
    let parts: Vec<String> = normalized
        .split('_')
        .filter(|segment| !segment.is_empty())
        .map(ToString::to_string)
        .collect();
    if parts.is_empty() { None } else { Some(parts) }
}

fn split_locale_inner(lower: &str) -> Vec<String> {
    let base = lower.split('.').next().unwrap_or(lower);
    base.replace('-', "_")
        .split('_')
        .filter(|segment| !segment.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn identify_language(parts: &[String]) -> Option<Language> {
    let lang = parts.first()?.as_str();
    let rest = &parts[1..];
    Some(match lang {
        "en" => Language::English,
        "ja" => Language::Japanese,
        "zh" => identify_chinese(rest),
        "zhhans" | "zh_hans" => Language::SimplifiedChinese,
        "zhhant" | "zh_hant" => Language::TraditionalChinese,
        "th" => Language::Thai,
        "km" => Language::Khmer,
        "vi" => Language::Vietnamese,
        "ko" => Language::Korean,
        "ar" => Language::Arabic,
        "he" | "iw" => Language::Hebrew,
        "ru" => Language::Russian,
        "my" | "bur" => Language::Burmese,
        "hi" => Language::Hindi,
        "ur" => Language::Urdu,
        "si" | "sin" => Language::Sinhala,
        "ta" => Language::Tamil,
        "fr" => Language::French,
        "uk" => Language::Ukrainian,
        "pl" => Language::Polish,
        "sv" => Language::Swedish,
        "de" => Language::German,
        "it" => Language::Italian,
        "sr" => Language::Serbian,
        "tr" => Language::Turkish,
        "hy" => Language::Armenian,
        "am" => Language::Amharic,
        "es" => Language::Spanish,
        "fa" | "ir" | "per" | "prs" => Language::Farsi,
        "akk" => Language::OldAkkadian,
        "qu" => Language::Quechua,
        "ay" => Language::Aymara,
        "bn" => Language::Bengali,
        "pt" | "ptbr" | "pt_br" => Language::Portuguese,
        "nl" => Language::Dutch,
        "da" => Language::Danish,
        "no" | "nn" | "nb" | "non" => Language::Norse,
        "fi" => Language::Finnish,
        "et" => Language::Estonian,
        "lv" => Language::Latvian,
        "hu" => Language::Hungarian,
        "cs" => Language::Czech,
        "lo" => Language::Lao,
        "id" | "in" => Language::Indonesian,
        "pis" => Language::Pijin,
        "dv" => Language::Divehi,
        "pa" => Language::Punjabi,
        "sd" => Language::Sindhi,
        "ps" | "pus" => Language::Pashto,
        "bal" => Language::Balochi,
        "skr" | "sa" | "sar" => Language::Saraiki,
        "brh" => Language::Brahui,
        "ks" => Language::Kashmiri,
        "ne" => Language::Nepali,
        "bo" => Language::Tibetan,
        "sw" => Language::Swahili,
        "ha" => Language::Hausa,
        "yo" => Language::Yoruba,
        "om" | "orm" => Language::Oromo,
        "ig" => Language::Igbo,
        "zu" => Language::Zulu,
        "sn" => Language::Shona,
        "so" => Language::Somali,
        "af" => Language::Afrikaans,
        "kk" => Language::Kazakh,
        "ba" => Language::Bashkir,
        "tt" => Language::Tatar,
        "el" => Language::Greek,
        "dz" => Language::Dzongkha,
        "mn" => Language::Mongolian,
        "jv" => Language::Javanese,
        "su" => Language::Sundanese,
        "mad" => Language::Madurese,
        "min" => Language::Minangkabau,
        "ban" => Language::Balinese,
        "egy" => Language::AncientEgyptianHieroglyph,
        "mnc" => Language::Manchurian,
        _ => return None,
    })
}

fn identify_chinese(rest: &[String]) -> Language {
    if rest
        .iter()
        .any(|p| matches!(p.as_str(), "hant" | "tw" | "hk" | "mo"))
    {
        return Language::TraditionalChinese;
    }
    if rest
        .iter()
        .any(|p| matches!(p.as_str(), "hans" | "cn" | "sg"))
    {
        return Language::SimplifiedChinese;
    }
    // Default to simplified when no region/script marker is present.
    Language::SimplifiedChinese
}

const FIRST_STRONG_ISOLATE: &str = "\u{2068}";
const RIGHT_TO_LEFT_ISOLATE: &str = "\u{2067}";
const POP_DIRECTIONAL_ISOLATE: &str = "\u{2069}";

/// Determine whether the provided language is right-to-left.
pub const fn is_rtl_language(language: Language) -> bool {
    matches!(
        language,
        Language::Arabic | Language::Hebrew | Language::Farsi | Language::Urdu | Language::Divehi
    )
}

/// Wrap dynamic text so that it renders correctly in right-to-left locales.
pub fn wrap_placeholder(language: Language, value: &str) -> Cow<'_, str> {
    if is_rtl_language(language) {
        Cow::Owned(format!(
            "{FIRST_STRONG_ISOLATE}{value}{POP_DIRECTIONAL_ISOLATE}"
        ))
    } else {
        Cow::Borrowed(value)
    }
}

type TranslationMap = HashMap<String, String>;

fn translation_cache(bundle: Bundle) -> &'static HashMap<Language, Arc<TranslationMap>> {
    static CLI: OnceLock<HashMap<Language, Arc<TranslationMap>>> = OnceLock::new();
    static DAEMON: OnceLock<HashMap<Language, Arc<TranslationMap>>> = OnceLock::new();

    match bundle {
        Bundle::Cli => CLI.get_or_init(|| load_bundle(Bundle::Cli)),
        Bundle::Daemon => DAEMON.get_or_init(|| load_bundle(Bundle::Daemon)),
    }
}

fn load_bundle(bundle: Bundle) -> HashMap<Language, Arc<TranslationMap>> {
    let mut storage = HashMap::new();
    for &language in Language::ALL {
        let raw = bundle.asset(language);
        let parsed = parse_translations(raw, bundle, language);
        storage.insert(language, Arc::new(parsed));
    }
    storage
}

fn parse_translations(raw: &str, bundle: Bundle, language: Language) -> TranslationMap {
    let value: Value = json::from_str(raw)
        .unwrap_or_else(|err| panic!("invalid JSON for {} ({:?}): {err}", bundle.name(), language));
    let object = value.as_object().unwrap_or_else(|| {
        panic!(
            "translation bundle {} ({:?}) must be a JSON object",
            bundle.name(),
            language
        )
    });
    let mut map = HashMap::with_capacity(object.len());
    for (key, value) in object {
        let text = value.as_str().unwrap_or_else(|| {
            panic!(
                "value for key `{}` in bundle {} ({:?}) is not a string",
                key,
                bundle.name(),
                language
            )
        });
        map.insert(key.clone(), text.to_string());
    }
    map
}

/// Localizer provides read-only access to translation strings.
#[derive(Clone)]
pub struct Localizer {
    language: Language,
    translations: Arc<TranslationMap>,
    default_translations: Arc<TranslationMap>,
}

impl Localizer {
    /// Instantiate a localizer for the given bundle and language.
    pub fn new(bundle: Bundle, language: Language) -> Self {
        let cache = translation_cache(bundle);
        let translations = cache
            .get(&language)
            .cloned()
            .or_else(|| cache.get(&Language::English).cloned());
        let default_translations = cache
            .get(&Language::English)
            .cloned()
            .expect("English translations must be present");
        let translations = translations.unwrap_or_else(|| default_translations.clone());
        Self {
            language,
            translations,
            default_translations,
        }
    }

    /// Resolve the active language.
    pub const fn language(&self) -> Language {
        self.language
    }

    fn lookup(&self, key: &str) -> String {
        self.translations
            .get(key)
            .or_else(|| {
                if self.language == Language::English {
                    None
                } else {
                    self.default_translations.get(key)
                }
            })
            .cloned()
            .unwrap_or_else(|| key.to_string())
    }

    fn wrap_directionality(&self, text: String) -> String {
        if is_rtl_language(self.language) {
            format!("{RIGHT_TO_LEFT_ISOLATE}{text}{POP_DIRECTIONAL_ISOLATE}")
        } else {
            text
        }
    }

    /// Fetch a localized string for the given key.
    ///
    /// When the key is missing in the active language, the English translation is used as the
    /// default. If the key is absent there as well, the key itself is returned.
    pub fn t(&self, key: &str) -> String {
        let text = self.lookup(key);
        self.wrap_directionality(text)
    }

    /// Fetch a localized string and replace `{placeholder}` markers with provided values.
    ///
    /// Placeholders are wrapped with directionality isolates for RTL languages.
    pub fn t_with(&self, key: &str, replacements: &[(&str, &str)]) -> String {
        let mut text = self.lookup(key);
        for (placeholder, value) in replacements {
            let marker = format!("{{{placeholder}}}");
            if text.contains(&marker) {
                let wrapped = wrap_placeholder(self.language, value);
                text = text.replace(&marker, wrapped.as_ref());
            }
        }
        self.wrap_directionality(text)
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeSet, fs, path::Path};

    use super::*;

    fn expected_file_stems() -> BTreeSet<String> {
        Language::ALL
            .iter()
            .map(|language| language.file_stem().to_string())
            .collect()
    }

    fn directory_file_stems(relative: &str) -> BTreeSet<String> {
        let base = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
        fs::read_dir(&base)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", base.display()))
            .filter_map(|entry| {
                let entry = entry.expect("directory entry");
                let path = entry.path();
                (path.extension().and_then(|ext| ext.to_str()) == Some("json")).then(|| {
                    path.file_stem()
                        .and_then(|stem| stem.to_str())
                        .unwrap_or_else(|| panic!("non-UTF8 file name in {}", base.display()))
                        .to_string()
                })
            })
            .collect()
    }

    #[test]
    fn cli_languages_match_translation_files() {
        let expected = expected_file_stems();
        let actual = directory_file_stems("src/lang/cli");
        assert_eq!(
            actual, expected,
            "CLI translation files must match Language::ALL entries"
        );
    }

    #[test]
    fn daemon_languages_match_translation_files() {
        let expected = expected_file_stems();
        let actual = directory_file_stems("src/lang/daemon");
        assert_eq!(
            actual, expected,
            "Daemon translation files must match Language::ALL entries"
        );
    }

    #[test]
    fn detects_override_language() {
        let language = detect_language_from_provider(Some("ru"), |_| None);
        assert_eq!(language, Language::Russian);
    }

    #[test]
    fn detects_from_env_with_region() {
        let language = detect_language_from_provider(None, |key| match key {
            "LC_ALL" => Some("pt_BR.UTF-8".to_string()),
            _ => None,
        });
        assert_eq!(language, Language::Portuguese);
    }

    #[test]
    fn defaults_to_english() {
        let language = detect_language_from_provider(None, |_| None);
        assert_eq!(language, Language::English);
    }

    #[test]
    fn loads_cli_translation() {
        let loc = Localizer::new(Bundle::Cli, Language::English);
        assert_eq!(loc.t("info.started"), "CLI started");
    }

    #[test]
    fn loads_daemon_translation() {
        let loc = Localizer::new(Bundle::Daemon, Language::English);
        assert_eq!(loc.t("info.welcome"), "Welcome to Hyperledger Iroha!");
    }

    #[test]
    fn recognizes_portuguese_aliases() {
        for tag in ["pt", "ptbr", "pt_br"] {
            let language = Language::from_tag(tag);
            assert_eq!(
                language,
                Some(Language::Portuguese),
                "expected `{tag}` to map to Portuguese"
            );
        }
    }

    #[test]
    fn wrap_placeholder_handles_rtl_languages() {
        let arabic = wrap_placeholder(Language::Arabic, "IVM");
        assert_eq!(arabic.as_ref(), "\u{2068}IVM\u{2069}");
        let english = wrap_placeholder(Language::English, "IVM");
        assert_eq!(english.as_ref(), "IVM");
    }

    #[test]
    fn rtl_translations_are_wrapped_for_rendering() {
        let urdu = Localizer::new(Bundle::Cli, Language::Urdu).t("info.started");
        assert!(urdu.starts_with(RIGHT_TO_LEFT_ISOLATE));
        assert!(urdu.ends_with(POP_DIRECTIONAL_ISOLATE));

        let english = Localizer::new(Bundle::Cli, Language::English).t("info.started");
        assert!(!english.starts_with(RIGHT_TO_LEFT_ISOLATE));
        assert!(!english.ends_with(POP_DIRECTIONAL_ISOLATE));
    }

    #[test]
    fn t_with_replaces_placeholders_and_handles_directionality() {
        let english = Localizer::new(Bundle::Cli, Language::English)
            .t_with("info.client_git_sha", &[("sha", "abc123")]);
        assert_eq!(english, "Client git SHA: abc123");

        let urdu = Localizer::new(Bundle::Cli, Language::Urdu)
            .t_with("info.client_git_sha", &[("sha", "abc123")]);
        assert!(urdu.starts_with(RIGHT_TO_LEFT_ISOLATE));
        assert!(urdu.ends_with(POP_DIRECTIONAL_ISOLATE));
        assert!(
            urdu.contains("\u{2068}abc123\u{2069}"),
            "placeholder must be wrapped for RTL languages: {urdu:?}"
        );
    }

    #[test]
    fn localizer_falls_back_to_english_translation() {
        use std::sync::Arc;

        let cache = translation_cache(Bundle::Cli);
        let english = cache
            .get(&Language::English)
            .cloned()
            .expect("English translations must exist");
        let mut stripped = (*english).clone();
        stripped.remove("info.started");
        let localizer = Localizer {
            language: Language::Arabic,
            translations: Arc::new(stripped),
            default_translations: english.clone(),
        };

        assert_eq!(
            localizer.t("info.started"),
            format!("{RIGHT_TO_LEFT_ISOLATE}CLI started{POP_DIRECTIONAL_ISOLATE}")
        );
        assert_eq!(
            localizer.t("unknown.key"),
            format!("{RIGHT_TO_LEFT_ISOLATE}unknown.key{POP_DIRECTIONAL_ISOLATE}")
        );
    }

    #[test]
    fn rtl_language_detection_matches_catalog() {
        let rtl_languages = [
            Language::Arabic,
            Language::Hebrew,
            Language::Farsi,
            Language::Urdu,
            Language::Divehi,
        ];
        for &language in &rtl_languages {
            assert!(
                is_rtl_language(language),
                "expected {language:?} to be treated as RTL"
            );
        }

        let ltr_languages = [Language::English, Language::Japanese, Language::French];
        for &language in &ltr_languages {
            assert!(
                !is_rtl_language(language),
                "expected {language:?} to be treated as LTR"
            );
        }
    }

    #[test]
    fn cli_translations_are_complete() {
        assert_bundle_translations(Bundle::Cli);
    }

    #[test]
    fn daemon_translations_are_complete() {
        assert_bundle_translations(Bundle::Daemon);
    }

    fn assert_bundle_translations(bundle: Bundle) {
        use std::collections::{BTreeMap, BTreeSet};

        let english =
            parse_translations(bundle.asset(Language::English), bundle, Language::English);
        let english_keys: BTreeSet<_> = english.keys().cloned().collect();
        let english_placeholders: BTreeMap<_, _> = english
            .iter()
            .map(|(key, value)| (key.clone(), extract_placeholders(value)))
            .collect();

        for &language in Language::ALL {
            let translations = parse_translations(bundle.asset(language), bundle, language);
            let keys: BTreeSet<_> = translations.keys().cloned().collect();
            assert_eq!(
                english_keys,
                keys,
                "bundle {} missing or extra keys for language {:?}",
                bundle.name(),
                language
            );

            for key in &english_keys {
                let placeholders = extract_placeholders(
                    translations
                        .get(key)
                        .unwrap_or_else(|| panic!("key `{key}` missing for {language:?}")),
                );
                let expected = english_placeholders
                    .get(key)
                    .unwrap_or_else(|| panic!("key `{key}` missing from English bundle"));
                assert_eq!(
                    expected,
                    &placeholders,
                    "placeholder mismatch for key `{key}` in bundle {} for language {:?}",
                    bundle.name(),
                    language
                );
            }
        }
    }

    fn extract_placeholders(text: &str) -> std::collections::BTreeSet<String> {
        use std::collections::BTreeSet;

        let mut placeholders = BTreeSet::new();
        let mut remainder = text;
        while let Some(start) = remainder.find('{') {
            let after_start = &remainder[start + 1..];
            if let Some(end) = after_start.find('}') {
                let candidate = &after_start[..end];
                if !candidate.is_empty()
                    && candidate
                        .chars()
                        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '.')
                {
                    placeholders.insert(candidate.to_string());
                }
                remainder = &after_start[end + 1..];
            } else {
                break;
            }
        }
        placeholders
    }
}
