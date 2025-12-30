use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "کمپائلءِ فنکشن نست",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "ناشنا پارامیټر {name}",
    read_file: "فایل {path} مئے نہ اتگ: {error}",
    parser_error: "پارسر خطا: {error}",
    semantic_error: "معنوی خطا: {error}",
    lint_unused_state: "حالت `{name}` اعلان بوت انت، پر ہرگیز کار نئیں بوت",
    lint_state_shadowed_param: "فنکشن `{func}` ءِ `{name}` پارامیٹر حالت `{name}` ءَ پوش کراں؛ حالتءَ دسترسی کیں پارامیٹرءِ نام بدل کن",
    lint_state_shadowed_binding: "فنکشن `{func}` ءِ `{name}` رابطہ حالت `{name}` ءَ پوش کراں؛ دسترسی برقرار کنگ رابطہءِ نام بدل کن",
    lint_state_shadowed_map_binding: "جا `{func}` نقشہ ءِ گردش کن، `{name}` رابطہ حالت `{name}` ءَ پوش کراں",
    lint_unused_parameter: "فنکشن `{func}` ءِ `{name}` پارامیٹر ہرگیز کار نئیں بوت",
    lint_unreachable_after_return: "{context} ءِ نا رسائی دستو پابند بوت؛ return ءِ پچھوں کوڈ نہ چلے",
    lint_ok: "ٹھیک",
    lint_usage: "کاربرد: koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "دیدا سورانءَ Kotodama lint چل کن.",
    ..english::MESSAGES
};
