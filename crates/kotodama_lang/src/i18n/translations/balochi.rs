use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "کمپایل ءَ کار ئی فنکشن نیست",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "نامعلوم پارامیٹر {name}",
    read_file: "فایل {path} پڑھنہ ناکام بوت: {error}",
    parser_error: "پارسر گلت: {error}",
    semantic_error: "معنوی گلت: {error}",
    lint_unused_state: "حالت `{name}` اعلان بوت انت، پر ہچ دم استعمال نہ بوت",
    lint_state_shadowed_param: "فنکشن `{func}` ءِ `{name}` پارامیٹر حالت `{name}` ءَ پوش کراں؛ حالتءَ رسائی کیں پارامیٹرءِ نام بدل کن",
    lint_state_shadowed_binding: "فنکشن `{func}` ءِ `{name}` رابطہ حالت `{name}` ءَ پوش کراں؛ رسائی برقرار کنگ رابطہءِ نام بدل کن",
    lint_state_shadowed_map_binding: "چم `{func}` نقشہ ءِ گردش کنت، `{name}` رابطہ حالت `{name}` ءَ پوش کراں",
    lint_unused_parameter: "فنکشن `{func}` ءِ `{name}` پارامیٹر ہرگیز استعمال نہ بوت",
    lint_unreachable_after_return: "{context} ءِ نا رسائی جملہ پدا بوت؛ return ءِ پچھیں کوڈ کار نہ کن",
    lint_ok: "سئی",
    lint_usage: "کاربرد: koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "دادا سورانءَ Kotodama lint سر کن.",
    ..english::MESSAGES
};
