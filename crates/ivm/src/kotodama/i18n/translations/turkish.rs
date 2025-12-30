use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "Derleme için işlev bulunmuyor",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "Bilinmeyen parametre {name}",
    read_file: "{path} dosyası okunamadı: {error}",
    parser_error: "Ayrıştırıcı hatası: {error}",
    semantic_error: "Anlamsal hata: {error}",
    lint_unused_state: "`{name}` durumu tanımlandı ancak hiç kullanılmıyor",
    lint_state_shadowed_param: "`{func}` fonksiyonundaki `{name}` parametresi `{name}` durumunu gizliyor; duruma erişmek için parametreyi yeniden adlandırın",
    lint_state_shadowed_binding: "`{func}` fonksiyonundaki `{name}` bağlaması `{name}` durumunu gizliyor; erişimi korumak için bağlamayı yeniden adlandırın",
    lint_state_shadowed_map_binding: "`{func}` fonksiyonunda harita yinelemesi sırasında `{name}` bağlaması `{name}` durumunu gizliyor",
    lint_unused_parameter: "`{func}` fonksiyonundaki `{name}` parametresi hiç kullanılmıyor",
    lint_unreachable_after_return: "{context} içinde erişilemeyen ifade bulundu: return sonrasındaki kod hiçbir zaman çalışmaz",
    lint_ok: "tamam",
    lint_usage: "Kullanım: koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "Belirtilen kaynaklar üzerinde Kotodama lint kontrollerini çalıştırır.",
    ..english::MESSAGES
};
