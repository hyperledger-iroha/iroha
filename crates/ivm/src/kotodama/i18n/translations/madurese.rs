use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "Tak enngih fungsi reng ateh takokah",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "Paramater tak dikenal {name}",
    read_file: "Tak bisa maosa berkas {path}: {error}",
    parser_error: "Kasalahan parser: {error}",
    semantic_error: "Kasalahan semantik: {error}",
    lint_unused_state: "Kaadaan `{name}` sampun deddhi tapi tak kaanggo sape",
    lint_state_shadowed_param: "Parameter `{name}` dalam fungsi `{func}` ngallempeng kaadaan `{name}`; ganti jenengan parameter ben bisa nyepeng kaadaan",
    lint_state_shadowed_binding: "Ikatan `{name}` dalam fungsi `{func}` ngallempeng kaadaan `{name}`; ganti jenengan ikatan ben aksès kaadaan tetep ana",
    lint_state_shadowed_map_binding: "Nalika fungsi `{func}` ngalantur map, ikatan `{name}` ngallempeng kaadaan `{name}`",
    lint_unused_parameter: "Parameter `{name}` dalam fungsi `{func}` tak kaanggo sape",
    lint_unreachable_after_return: "Instruksi neng {context} tak bisa kasampurna; kode sawisé return tak bakal mlaku",
    lint_ok: "beres",
    lint_usage: "Pangangguy: koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "Lakokno Kotodama lint marang berkas se sampèyan pasraaghi.",
    ..english::MESSAGES
};
