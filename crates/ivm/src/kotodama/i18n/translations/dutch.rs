use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "Geen functies om te compileren",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "Onbekende param {name}",
    read_file: "Kan niet lezen {path}: {error}",
    parser_error: "parserfout: {error}",
    semantic_error: "semantische fout: {error}",
    lint_unused_state: "State `{name}` is gedeclareerd maar wordt nooit gebruikt",
    lint_state_shadowed_param: "De parameter `{name}` in functie `{func}` verbergt state `{name}`; hernoem de parameter om toegang te krijgen tot de state",
    lint_state_shadowed_binding: "De binding `{name}` in functie `{func}` verbergt state `{name}`; hernoem de binding om toegang te behouden",
    lint_state_shadowed_map_binding: "De binding `{name}` in functie `{func}` verbergt state `{name}` tijdens het itereren over de map",
    lint_unused_parameter: "De parameter `{name}` in functie `{func}` wordt nergens gebruikt",
    lint_unreachable_after_return: "Onbereikbare instructie gedetecteerd in {context}: code na return wordt nooit uitgevoerd",
    lint_ok: "ok",
    lint_usage: "Gebruik: koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "Voert de Kotodama-lints uit op de opgegeven bronnen.",
    ..english::MESSAGES
};
