use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "Indak ado fungsi nan dikompilasi",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "Parameter indak dikenal {name}",
    read_file: "Gagal mambaco berkas {path}: {error}",
    parser_error: "Kesalahan parser: {error}",
    semantic_error: "Kesalahan semantik: {error}",
    lint_unused_state: "Kaadaan `{name}` lah diisiahkan tapi indak dipakai",
    lint_state_shadowed_param: "Parameter `{name}` dalam fungsi `{func}` manutub kaadaan `{name}`; ganti namo parameter supaya kaadaan bisa dicapai",
    lint_state_shadowed_binding: "Binding `{name}` dalam fungsi `{func}` manutub kaadaan `{name}`; ganti namo binding untuak manyaga akses kaadaan",
    lint_state_shadowed_map_binding: "Saat fungsi `{func}` malewangkan map, binding `{name}` manutub kaadaan `{name}`",
    lint_unused_parameter: "Parameter `{name}` dalam fungsi `{func}` indak dipakai samo sakali",
    lint_unreachable_after_return: "Pernyataan nan ndak bisa dicapai kapanggih dalam {context}; kode salapeh return indak bakal jalan",
    lint_ok: "baik",
    lint_usage: "Panggunaan: koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "Jalanan Kotodama lint ka sumber nan disadiokan.",
    ..english::MESSAGES
};
