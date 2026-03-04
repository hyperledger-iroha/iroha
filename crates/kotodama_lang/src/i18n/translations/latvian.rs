use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "Nav funkciju apkopošanai",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "Nezināms param {name}",
    read_file: "Neizdevās lasīt {path}: {error}",
    parser_error: "parsētāja kļūda: {error}",
    semantic_error: "semantiskā kļūda: {error}",
    lint_unused_state: "stāvoklis `{name}` ir deklarēts, bet nekad netiek izmantots",
    lint_state_shadowed_param: "parametrs `{name}` funkcijā `{func}` nosedz stāvokli `{name}`; pārdēvējiet parametru, lai piekļūtu stāvoklim",
    lint_state_shadowed_binding: "piesaiste `{name}` funkcijā `{func}` nosedz stāvokli `{name}`; pārdēvējiet piesaisti, lai saglabātu piekļuvi",
    lint_state_shadowed_map_binding: "funkcijas `{func}` kartes iterācijas laikā piesaiste `{name}` nosedz stāvokli `{name}`",
    lint_unused_parameter: "parametrs `{name}` funkcijā `{func}` netiek izmantots",
    lint_unreachable_after_return: "konstatēts neaizsniedzams teikums {context}: kods pēc return nekad netiek izpildīts",
    lint_ok: "labi",
    lint_usage: "Lietošana: koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "Palaidiet Kotodama lint pārbaudes uz norādītajiem avotiem.",
    ..english::MESSAGES
};
