use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "Keine Funktionen zu kompilieren",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "Unbekannter Parameter {name}",
    read_file: "Datei {path} konnte nicht gelesen werden: {error}",
    parser_error: "Parser-Fehler: {error}",
    semantic_error: "Semantischer Fehler: {error}",
    lint_unused_state: "State `{name}` ist deklariert, wird aber nie verwendet",
    lint_state_shadowed_param: "Der Parameter `{name}` in Funktion `{func}` überdeckt den State `{name}`; benenne den Parameter um, um auf den State zuzugreifen",
    lint_state_shadowed_binding: "Die Bindung `{name}` in Funktion `{func}` überdeckt den State `{name}`; benenne die Bindung um, damit der State erreichbar bleibt",
    lint_state_shadowed_map_binding: "Die Bindung `{name}` in Funktion `{func}` überdeckt den State `{name}` während der Map-Iteration",
    lint_unused_parameter: "Der Parameter `{name}` in Funktion `{func}` wird niemals verwendet",
    lint_unreachable_after_return: "Nicht erreichbare Anweisung in {context} erkannt: Code nach einem return wird nie ausgeführt",
    lint_ok: "ok",
    lint_usage: "Verwendung: koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "Führt die Kotodama-Lints für die angegebenen Quellen aus.",
    ..english::MESSAGES
};
