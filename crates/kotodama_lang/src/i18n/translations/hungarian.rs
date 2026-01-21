use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "Nincs függvény a fordításhoz",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "Ismeretlen paraméter {name}",
    read_file: "A(z) {path} fájl olvasása sikertelen: {error}",
    parser_error: "Elemző hiba: {error}",
    semantic_error: "Szemantikai hiba: {error}",
    lint_unused_state: "A(z) `{name}` állapot deklarálva van, de soha nem kerül használatra",
    lint_state_shadowed_param: "A(z) `{func}` függvény `{name}` paramétere elfedi a(z) `{name}` állapotot; nevezze át a paramétert az állapot eléréséhez",
    lint_state_shadowed_binding: "A(z) `{func}` függvény `{name}` kötése elfedi a(z) `{name}` állapotot; nevezze át a kötést az állapothoz való hozzáférés megtartásához",
    lint_state_shadowed_map_binding: "A(z) `{func}` függvény map bejárása közben a(z) `{name}` kötés elfedi a(z) `{name}` állapotot",
    lint_unused_parameter: "A(z) `{func}` függvény `{name}` paramétere nincs használva",
    lint_unreachable_after_return: "Nem elérhető utasítást találtunk itt: {context}; a return utáni kód nem fog lefutni",
    lint_ok: "rendben",
    lint_usage: "Használat: koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "Futtassa a Kotodama lint ellenőrzéseket a megadott forrásokon.",
    ..english::MESSAGES
};
