use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "Brak funkcji do kompilacji",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "Nieznany parametr {name}",
    read_file: "Nie udało się odczytać pliku {path}: {error}",
    parser_error: "Błąd parsera: {error}",
    semantic_error: "Błąd semantyczny: {error}",
    lint_unused_state: "Stan `{name}` został zadeklarowany, ale nigdy nie jest używany",
    lint_state_shadowed_param: "Parametr `{name}` w funkcji `{func}` ukrywa stan `{name}`; zmień nazwę parametru, aby uzyskać dostęp do stanu",
    lint_state_shadowed_binding: "Powiązanie `{name}` w funkcji `{func}` ukrywa stan `{name}`; zmień nazwę powiązania, aby zachować dostęp",
    lint_state_shadowed_map_binding: "Powiązanie `{name}` w funkcji `{func}` ukrywa stan `{name}` podczas iteracji mapy",
    lint_unused_parameter: "Parametr `{name}` w funkcji `{func}` nie jest nigdzie używany",
    lint_unreachable_after_return: "Wykryto nieosiągalne polecenie w {context}: kod po return nigdy się nie wykonuje",
    lint_ok: "OK",
    lint_usage: "Użycie: koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "Uruchamia lintery Kotodama dla podanych plików źródłowych.",
    ..english::MESSAGES
};
