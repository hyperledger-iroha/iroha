use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "Nénten wénten fungsi sane dados dikompilasi",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "Parémetro nénten karahosang {name}",
    read_file: "Gagal maos berkas {path}: {error}",
    parser_error: "Kasalahan parser: {error}",
    semantic_error: "Kasalahan semantik: {error}",
    lint_unused_state: "Kaadan `{name}` sampun kaserat nanging tan dados kaanggén",
    lint_state_shadowed_param: "Parameter `{name}` ring fungsi `{func}` ngamantenin kaadan `{name}`; gantiangang nami parameter mangda kaadan prasida kasengguh",
    lint_state_shadowed_binding: "Binding `{name}` ring fungsi `{func}` ngamantenin kaadan `{name}`; gantiangang nami binding mangda aksés kaadan tetep kabuka",
    lint_state_shadowed_map_binding: "Nalika fungsi `{func}` ngamolihang map, binding `{name}` ngamantenin kaadan `{name}`",
    lint_unused_parameter: "Parameter `{name}` ring fungsi `{func}` tan dados kaanggén wenten pisan",
    lint_unreachable_after_return: "Wangsalan tan kasapuk kapanggih ring {context}; kode sasampun return tan kalaksanayang",
    lint_ok: "becik",
    lint_usage: "Panganggé: koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "Laksanayang Kotodama lint ring sumber sane kapidabdabang.",
    ..english::MESSAGES
};
