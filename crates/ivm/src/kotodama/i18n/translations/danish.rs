use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "Ingen funktioner at kompilere",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "Ukendt parameter {name}",
    read_file: "Kunne ikke læse filen {path}: {error}",
    parser_error: "Parserfejl: {error}",
    semantic_error: "Semantisk fejl: {error}",
    lint_unused_state: "State `{name}` er erklæret men bruges aldrig",
    lint_state_shadowed_param: "Parameteren `{name}` i funktionen `{func}` skjuler state `{name}`; omdøb parameteren for at få adgang",
    lint_state_shadowed_binding: "Bindingen `{name}` i funktionen `{func}` skjuler state `{name}`; omdøb bindingen for at bevare adgangen",
    lint_state_shadowed_map_binding: "Bindingen `{name}` i funktionen `{func}` skjuler state `{name}` under iteration over et map",
    lint_unused_parameter: "Parameteren `{name}` i funktionen `{func}` bliver aldrig brugt",
    lint_unreachable_after_return: "Utilgængelig sætning fundet i {context}: kode efter return bliver aldrig kørt",
    lint_ok: "ok",
    lint_usage: "Brug: koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "Kører Kotodamas lint-tjek på de angivne kilder.",
    ..english::MESSAGES
};
