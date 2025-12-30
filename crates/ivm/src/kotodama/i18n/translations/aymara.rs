use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "Janiw kuna funciones ukanakas compilar",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "Jan uñt’at param {name}",
    read_file: "Janiw liyiñjamäkiti {path}: {error}",
    parser_error: "Parser ukax mä pantjatawa: {error}",
    semantic_error: "Semántica ukan pantjasitapa: {error}",
    lint_unused_state: "`{name}` estado uka uñt’ayatawa ukampisa janiw apnaqatäkiti",
    lint_state_shadowed_param: "Función `{func}` ukana `{name}` parámetro ukaxa `{name}` estado ukar imanta; estado ukar mantañataki parámetro sutipa mayjt’ayaña",
    lint_state_shadowed_binding: "Función `{func}` ukana `{name}` ch’iqchiwi ukaxa `{name}` estado ukar imanta; acceso chikañataki ch’iqchiwi sutipa mayjt’ayaña",
    lint_state_shadowed_map_binding: "Función `{func}` uka mapa thaqki ukaxa `{name}` ch’iqchiwi ukaxa `{name}` estado ukar imanta",
    lint_unused_parameter: "Función `{func}` ukana `{name}` parámetro ukaxa janiw mayampisa apnaqatäkiti",
    lint_unreachable_after_return: "{context} ukana jan purisiri arsu uñjasiwa; return ukat qhiparu qillqatanaka janiw sarnaqkiti",
    lint_ok: "wali",
    lint_usage: "Apnaqaña: koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "Kotodama lint uka sarantayaña wakichat qhanstayat qillqatanakana.",
    ..english::MESSAGES
};
