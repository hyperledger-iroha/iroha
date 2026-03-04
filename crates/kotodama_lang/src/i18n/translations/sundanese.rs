use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "Teu aya fungsi pikeun dikompilasi",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "Paraméter teu dipikawanoh {name}",
    read_file: "Gagal maca berkas {path}: {error}",
    parser_error: "Kasalahan parser: {error}",
    semantic_error: "Kasalahan semantik: {error}",
    lint_unused_state: "state `{name}` geus diumumkeun tapi teu kungsi dipaké",
    lint_state_shadowed_param: "paraméter `{name}` dina fungsi `{func}` nyumputkeun state `{name}`; ganti ngaran paraméter pikeun ngaksés state",
    lint_state_shadowed_binding: "binding `{name}` dina fungsi `{func}` nyumputkeun state `{name}`; ganti ngaran binding supaya aksés kana state tetep aya",
    lint_state_shadowed_map_binding: "binding `{name}` dina fungsi `{func}` nyumputkeun state `{name}` nalika iterasi map",
    lint_unused_parameter: "paraméter `{name}` dina fungsi `{func}` teu kungsi dipaké",
    lint_unreachable_after_return: "pernyataan teu kahontal kapanggih dina {context}: kode sanggeus return moal kungsi dijalankeun",
    lint_ok: "hasil",
    lint_usage: "Pamakéan: koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "Jalankeun lint Kotodama kana sumber anu disadiakeun.",
    ..english::MESSAGES
};
