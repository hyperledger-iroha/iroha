use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "Pole funktsioone kompileerimiseks",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "Tundmatu parameeter {name}",
    read_file: "Faili {path} lugemine ebaõnnestus: {error}",
    parser_error: "Parsimisviga: {error}",
    semantic_error: "Semantiline viga: {error}",
    lint_unused_state: "olek `{name}` on deklareeritud, kuid seda ei kasutata",
    lint_state_shadowed_param: "funktsiooni `{func}` parameeter `{name}` varjab olekut `{name}`; nimeta parameeter ümber, et olekule ligi pääseda",
    lint_state_shadowed_binding: "funktsiooni `{func}` sidumine `{name}` varjab olekut `{name}`; nimeta sidumine ümber, et ligipääs säiliks",
    lint_state_shadowed_map_binding: "funktsiooni `{func}` kaardi itereerimisel varjab sidumine `{name}` olekut `{name}`",
    lint_unused_parameter: "funktsiooni `{func}` parameetrit `{name}` ei kasutata",
    lint_unreachable_after_return: "{context} puhul leiti kättesaamatu lause: return'i järgset koodi ei käivitata kunagi",
    lint_ok: "ok",
    lint_usage: "Kasutamine: koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "Käivita Kotodama lint kontrollid antud lähtekoodidel.",
    ..english::MESSAGES
};
