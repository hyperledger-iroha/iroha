use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "Tidak ada fungsi untuk dikompilasi",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "Parameter tidak dikenal {name}",
    read_file: "Gagal membaca berkas {path}: {error}",
    parser_error: "Kesalahan parser: {error}",
    semantic_error: "Kesalahan semantik: {error}",
    lint_unused_state: "state `{name}` dideklarasikan tetapi tidak pernah digunakan",
    lint_state_shadowed_param: "parameter `{name}` pada fungsi `{func}` menyembunyikan state `{name}`; ganti nama parameter untuk mengakses state",
    lint_state_shadowed_binding: "binding `{name}` pada fungsi `{func}` menyembunyikan state `{name}`; ganti nama binding untuk mempertahankan akses ke state",
    lint_state_shadowed_map_binding: "binding `{name}` pada fungsi `{func}` menyembunyikan state `{name}` saat iterasi map",
    lint_unused_parameter: "parameter `{name}` pada fungsi `{func}` tidak pernah digunakan",
    lint_unreachable_after_return: "pernyataan tidak dapat dijangkau terdeteksi di {context}: kode setelah return tidak akan pernah dijalankan",
    lint_ok: "berhasil",
    lint_usage: "Penggunaan: koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "Jalankan lint Kotodama pada sumber yang diberikan.",
    ..english::MESSAGES
};
