use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "Babu aikin da za a haɗa",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "Sigogi da ba a sani ba {name}",
    read_file: "An kasa karanta fayil {path}: {error}",
    parser_error: "Kuskuren mai fassara: {error}",
    semantic_error: "Kuskuren ma'ana: {error}",
    lint_unused_state: "halin `{name}` an ayyana shi amma ba a taba amfani da shi ba",
    lint_state_shadowed_param: "sigar `{name}` a cikin aikin `{func}` na rufe halin `{name}`; canza sunan sigar don samun damar halin",
    lint_state_shadowed_binding: "haɗin `{name}` a cikin aikin `{func}` na rufe halin `{name}`; canza sunan haɗin don kiyaye damar shiga",
    lint_state_shadowed_map_binding: "haɗin `{name}` a cikin aikin `{func}` na rufe halin `{name}` yayin jujjuyawar taswira",
    lint_unused_parameter: "sigar `{name}` a cikin aikin `{func}` ba ta taba amfani ba",
    lint_unreachable_after_return: "an gano jimla da ba za a iya kai wa gare ta ba a {context}: lambar bayan return ba za ta taba gudana ba",
    lint_ok: "lafiya",
    lint_usage: "Amfani: koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "Gudanar da binciken lint na Kotodama a kan tushen da aka bayar.",
    ..english::MESSAGES
};
