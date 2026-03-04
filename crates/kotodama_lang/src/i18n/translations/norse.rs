use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "Ingen funksjoner å kompilere",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "Ukjent parameter {name}",
    read_file: "Klarte ikke å lese filen {path}: {error}",
    parser_error: "Parserfeil: {error}",
    semantic_error: "Semantisk feil: {error}",
    lint_unused_state: "Tilstand `{name}` er erklært, men blir aldri brukt",
    lint_state_shadowed_param: "Parameteren `{name}` i funksjonen `{func}` skjuler tilstanden `{name}`; gi parameteren nytt navn for å få tilgang til tilstanden",
    lint_state_shadowed_binding: "Bindingen `{name}` i funksjonen `{func}` skjuler tilstanden `{name}`; gi bindingen nytt navn for å holde tilgangen åpen",
    lint_state_shadowed_map_binding: "Når funksjonen `{func}` går gjennom et kart, skjuler bindingen `{name}` tilstanden `{name}`",
    lint_unused_parameter: "Parameteren `{name}` i funksjonen `{func}` blir aldri brukt",
    lint_unreachable_after_return: "Fant en utilgjengelig setning i {context}: kode etter en return blir ikke kjørt",
    lint_ok: "bra",
    lint_usage: "Bruk: koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "Kjør Kotodama-lintene på de oppgitte kildene.",
    ..english::MESSAGES
};
