pub use iroha_i18n::Language;
use iroha_i18n::wrap_placeholder;

mod translations;

/// Detect the best-fit language from overrides and environment.
pub fn detect_language() -> Language {
    iroha_i18n::detect_language(None)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateShadowContext {
    Parameter,
    Binding,
    MapBinding,
}

pub enum Message<'a> {
    NoFunctions,
    UnsupportedBinaryOp(&'a str),
    UnknownParam(&'a str),
    ReadFile(&'a str, &'a str),
    ParserError(&'a str),
    SemanticError(&'a str),
    LintUnusedState {
        name: &'a str,
    },
    LintStateShadowed {
        func: &'a str,
        name: &'a str,
        context: StateShadowContext,
    },
    LintUnusedParameter {
        func: &'a str,
        name: &'a str,
    },
    LintUnreachableAfterReturn {
        context: &'a str,
    },
    LintOk,
    LintUsage,
    LintUsageHelp,
}

#[derive(Clone, Copy)]
struct Messages {
    no_functions: &'static str,
    unsupported_binary_op: &'static str,
    unknown_param: &'static str,
    read_file: &'static str,
    parser_error: &'static str,
    semantic_error: &'static str,
    lint_unused_state: &'static str,
    lint_state_shadowed_param: &'static str,
    lint_state_shadowed_binding: &'static str,
    lint_state_shadowed_map_binding: &'static str,
    lint_unused_parameter: &'static str,
    lint_unreachable_after_return: &'static str,
    lint_ok: &'static str,
    lint_usage: &'static str,
    lint_usage_help: &'static str,
}

fn messages_for(lang: Language) -> Messages {
    match lang {
        Language::English => translations::english::MESSAGES,
        Language::Japanese => translations::japanese::MESSAGES,
        Language::SimplifiedChinese => translations::simplified_chinese::MESSAGES,
        Language::TraditionalChinese => translations::traditional_chinese::MESSAGES,
        Language::Thai => translations::thai::MESSAGES,
        Language::Khmer => translations::khmer::MESSAGES,
        Language::Vietnamese => translations::vietnamese::MESSAGES,
        Language::Korean => translations::korean::MESSAGES,
        Language::Arabic => translations::arabic::MESSAGES,
        Language::Hebrew => translations::hebrew::MESSAGES,
        Language::Russian => translations::russian::MESSAGES,
        Language::Burmese => translations::burmese::MESSAGES,
        Language::Hindi => translations::hindi::MESSAGES,
        Language::Urdu => translations::urdu::MESSAGES,
        Language::Sinhala => translations::sinhala::MESSAGES,
        Language::Tamil => translations::tamil::MESSAGES,
        Language::French => translations::french::MESSAGES,
        Language::Ukrainian => translations::ukrainian::MESSAGES,
        Language::Polish => translations::polish::MESSAGES,
        Language::Swedish => translations::swedish::MESSAGES,
        Language::German => translations::german::MESSAGES,
        Language::Greek => translations::greek::MESSAGES,
        Language::Italian => translations::italian::MESSAGES,
        Language::Kazakh => translations::kazakh::MESSAGES,
        Language::Mongolian => translations::mongolian::MESSAGES,
        Language::Javanese => translations::javanese::MESSAGES,
        Language::Madurese => translations::madurese::MESSAGES,
        Language::Balinese => translations::balinese::MESSAGES,
        Language::Minangkabau => translations::minangkabau::MESSAGES,
        Language::AncientEgyptianHieroglyph => translations::ancient_egyptian_hieroglyph::MESSAGES,
        Language::Dzongkha => translations::dzongkha::MESSAGES,
        Language::Serbian => translations::serbian::MESSAGES,
        Language::Turkish => translations::turkish::MESSAGES,
        Language::Armenian => translations::armenian::MESSAGES,
        Language::Amharic => translations::amharic::MESSAGES,
        Language::Hausa => translations::hausa::MESSAGES,
        Language::Tibetan => translations::tibetan::MESSAGES,
        Language::Kashmiri => translations::kashmiri::MESSAGES,
        Language::Nepali => translations::nepali::MESSAGES,
        Language::Afrikaans => translations::afrikaans::MESSAGES,
        Language::Spanish => translations::spanish::MESSAGES,
        Language::Farsi => translations::farsi::MESSAGES,
        Language::OldAkkadian => translations::old_akkadian::MESSAGES,
        Language::Quechua => translations::quechua::MESSAGES,
        Language::Aymara => translations::aymara::MESSAGES,
        Language::Bengali => translations::bengali::MESSAGES,
        Language::Balochi => translations::balochi::MESSAGES,
        Language::Bashkir => translations::bashkir::MESSAGES,
        Language::Brahui => translations::brahui::MESSAGES,
        Language::Portuguese => translations::portuguese::MESSAGES,
        Language::Punjabi => translations::punjabi::MESSAGES,
        Language::Sindhi => translations::sindhi::MESSAGES,
        Language::Pashto => translations::pashto::MESSAGES,
        Language::Saraiki => translations::saraiki::MESSAGES,
        Language::Tatar => translations::tatar::MESSAGES,
        Language::Somali => translations::somali::MESSAGES,
        Language::Sundanese => translations::sundanese::MESSAGES,
        Language::Shona => translations::shona::MESSAGES,
        Language::Swahili => translations::swahili::MESSAGES,
        Language::Oromo => translations::oromo::MESSAGES,
        Language::Igbo => translations::igbo::MESSAGES,
        Language::Yoruba => translations::yoruba::MESSAGES,
        Language::Zulu => translations::zulu::MESSAGES,
        Language::Dutch => translations::dutch::MESSAGES,
        Language::Danish => translations::danish::MESSAGES,
        Language::Norse => translations::norse::MESSAGES,
        Language::Finnish => translations::finnish::MESSAGES,
        Language::Estonian => translations::estonian::MESSAGES,
        Language::Latvian => translations::latvian::MESSAGES,
        Language::Hungarian => translations::hungarian::MESSAGES,
        Language::Czech => translations::czech::MESSAGES,
        Language::Lao => translations::lao::MESSAGES,
        Language::Indonesian => translations::indonesian::MESSAGES,
        Language::Pijin => translations::pijin::MESSAGES,
        Language::Divehi => translations::divehi::MESSAGES,
        Language::Manchurian => translations::manchurian::MESSAGES,
    }
}

fn render_template(
    template: &str,
    replacements: &[(&str, &str)],
    language: Language,
) -> Option<String> {
    let mut result = template.to_string();
    let mut replaced = false;
    for (key, value) in replacements {
        let marker = format!("{{{key}}}");
        if result.contains(&marker) {
            let replacement = wrap_placeholder(language, value);
            result = result.replace(&marker, replacement.as_ref());
            replaced = true;
        }
    }
    if replaced && !result.contains('{') {
        Some(result)
    } else {
        None
    }
}

pub fn translate(lang: Language, msg: Message) -> String {
    let msgs = messages_for(lang);
    match msg {
        Message::NoFunctions => msgs.no_functions.to_string(),
        Message::UnsupportedBinaryOp(op) => {
            render_template(msgs.unsupported_binary_op, &[("op", op)], lang).unwrap_or_else(|| {
                format!(
                    "{} {}",
                    msgs.unsupported_binary_op,
                    wrap_placeholder(lang, op)
                )
            })
        }
        Message::UnknownParam(name) => render_template(msgs.unknown_param, &[("name", name)], lang)
            .unwrap_or_else(|| format!("{} {}", msgs.unknown_param, wrap_placeholder(lang, name))),
        Message::ReadFile(path, err) => {
            render_template(msgs.read_file, &[("path", path), ("error", err)], lang).unwrap_or_else(
                || {
                    format!(
                        "{} {}: {}",
                        msgs.read_file,
                        wrap_placeholder(lang, path),
                        wrap_placeholder(lang, err)
                    )
                },
            )
        }
        Message::ParserError(e) => render_template(msgs.parser_error, &[("error", e)], lang)
            .unwrap_or_else(|| format!("{}: {}", msgs.parser_error, wrap_placeholder(lang, e))),
        Message::SemanticError(e) => render_template(msgs.semantic_error, &[("error", e)], lang)
            .unwrap_or_else(|| format!("{}: {}", msgs.semantic_error, wrap_placeholder(lang, e))),
        Message::LintUnusedState { name } => {
            render_template(msgs.lint_unused_state, &[("name", name)], lang).unwrap_or_else(|| {
                format!(
                    "state {} is declared but never used",
                    wrap_placeholder(lang, name)
                )
            })
        }
        Message::LintStateShadowed {
            func,
            name,
            context,
        } => {
            let template = match context {
                StateShadowContext::Parameter => msgs.lint_state_shadowed_param,
                StateShadowContext::Binding => msgs.lint_state_shadowed_binding,
                StateShadowContext::MapBinding => msgs.lint_state_shadowed_map_binding,
            };
            render_template(template, &[("func", func), ("name", name)], lang).unwrap_or_else(
                || {
                    format!(
                        "state shadowed: {} in {}",
                        wrap_placeholder(lang, name),
                        wrap_placeholder(lang, func),
                    )
                },
            )
        }
        Message::LintUnusedParameter { func, name } => render_template(
            msgs.lint_unused_parameter,
            &[("func", func), ("name", name)],
            lang,
        )
        .unwrap_or_else(|| {
            format!(
                "unused parameter {} in {}",
                wrap_placeholder(lang, name),
                wrap_placeholder(lang, func)
            )
        }),
        Message::LintUnreachableAfterReturn { context } => render_template(
            msgs.lint_unreachable_after_return,
            &[("context", context)],
            lang,
        )
        .unwrap_or_else(|| {
            format!(
                "unreachable statement after return in {}",
                wrap_placeholder(lang, context)
            )
        }),
        Message::LintOk => msgs.lint_ok.to_string(),
        Message::LintUsage => msgs.lint_usage.to_string(),
        Message::LintUsageHelp => msgs.lint_usage_help.to_string(),
    }
}
