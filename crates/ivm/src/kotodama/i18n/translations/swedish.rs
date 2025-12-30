use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "Inga funktioner att kompilera",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "Okänd parameter {name}",
    read_file: "Misslyckades med att läsa filen {path}: {error}",
    parser_error: "Parserfel: {error}",
    semantic_error: "Semantiskt fel: {error}",
    lint_unused_state: "State `{name}` är deklarerat men används aldrig",
    lint_state_shadowed_param: "Parametern `{name}` i funktionen `{func}` skymmer state `{name}`; byt namn på parametern för att komma åt state",
    lint_state_shadowed_binding: "Bindningen `{name}` i funktionen `{func}` skymmer state `{name}`; byt namn på bindningen för att behålla åtkomsten",
    lint_state_shadowed_map_binding: "Bindningen `{name}` i funktionen `{func}` skymmer state `{name}` under map-iteration",
    lint_unused_parameter: "Parametern `{name}` i funktionen `{func}` används aldrig",
    lint_unreachable_after_return: "Oåtkomligt uttryck upptäcktes i {context}: kod efter return körs aldrig",
    lint_ok: "ok",
    lint_usage: "Användning: koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "Kör Kotodamas lint-kontroller på angivna källor.",
    ..english::MESSAGES
};
