use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "Ora ana fungsi kanggo dikompilasi",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "Paramèter ora dikenal {name}",
    read_file: "Gagal maca berkas {path}: {error}",
    parser_error: "Kesalahan parser: {error}",
    semantic_error: "Kesalahan semantik: {error}",
    lint_unused_state: "state `{name}` wis diumumaké nanging ora tau digunakaké",
    lint_state_shadowed_param: "paramèter `{name}` ing fungsi `{func}` ndhelikaké state `{name}`; ganti jeneng paramèter supaya bisa ngakses state",
    lint_state_shadowed_binding: "ikat `{name}` ing fungsi `{func}` ndhelikaké state `{name}`; ganti jeneng ikatan supaya akses tetep ana",
    lint_state_shadowed_map_binding: "ikat `{name}` ing fungsi `{func}` ndhelikaké state `{name}` nalika iterasi map",
    lint_unused_parameter: "paramèter `{name}` ing fungsi `{func}` ora tau dienggo",
    lint_unreachable_after_return: "pernyataan ora bisa digayuh ditemokaké ing {context}: kode sawisé return ora bakal mlaku",
    lint_ok: "berhasil",
    lint_usage: "Panggunaan: koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "Mbukak lint Kotodama marang sumber sing diwènèhaké.",
    ..english::MESSAGES
};
