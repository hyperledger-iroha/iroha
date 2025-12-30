use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "Žádné funkce ke kompilaci",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "Neznámý param {name}",
    read_file: "Nepodařilo se číst {path}: {error}",
    parser_error: "Chyba analyzátoru: {error}",
    semantic_error: "sémantická chyba: {error}",
    lint_unused_state: "Stav `{name}` je deklarován, ale nikdy se nepoužívá",
    lint_state_shadowed_param: "Parametr `{name}` ve funkci `{func}` skrývá stav `{name}`; přejmenujte parametr, abyste se ke stavu dostali",
    lint_state_shadowed_binding: "Vazba `{name}` ve funkci `{func}` skrývá stav `{name}`; přejmenujte vazbu, abyste zachovali přístup",
    lint_state_shadowed_map_binding: "Vazba `{name}` ve funkci `{func}` skrývá stav `{name}` během iterace mapy",
    lint_unused_parameter: "Parametr `{name}` ve funkci `{func}` se nikde nepoužívá",
    lint_unreachable_after_return: "V kontextu {context} byla nalezena nedosažitelná instrukce: kód za return se nikdy nespustí",
    lint_ok: "ok",
    lint_usage: "Použití: koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "Spustí linty Kotodama nad zadanými zdrojovými soubory.",
    ..english::MESSAGES
};
