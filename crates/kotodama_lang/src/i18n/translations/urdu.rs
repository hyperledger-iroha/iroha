use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "ترتیب کے لیے کوئی فنکشن موجود نہیں",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "نامعلوم پیرامیٹر {name}",
    read_file: "فائل {path} پڑھنے میں ناکامی: {error}",
    parser_error: "پارسر کی خرابی: {error}",
    semantic_error: "معنوی خرابی: {error}",
    lint_unused_state: "اسٹیٹ `{name}` کا اعلان کیا گیا لیکن کبھی استعمال نہیں ہوا",
    lint_state_shadowed_param: "فنکشن `{func}` میں پیرامیٹر `{name}` اسٹیٹ `{name}` کو چھپا دیتا ہے؛ اسٹیٹ تک رسائی کے لیے پیرامیٹر کا نام تبدیل کریں",
    lint_state_shadowed_binding: "فنکشن `{func}` میں ربط `{name}` اسٹیٹ `{name}` کو چھپا دیتا ہے؛ رسائی برقرار رکھنے کے لیے ربط کا نام تبدیل کریں",
    lint_state_shadowed_map_binding: "فنکشن `{func}` میں نقشہ پر تکرار کرتے وقت ربط `{name}` اسٹیٹ `{name}` کو چھپا دیتا ہے",
    lint_unused_parameter: "فنکشن `{func}` میں پیرامیٹر `{name}` کبھی استعمال نہیں ہوتا",
    lint_unreachable_after_return: "{context} میں ناقابلِ رسائی بیان ملا: return کے بعد کا کوڈ کبھی نہیں چلتا",
    lint_ok: "ٹھیک",
    lint_usage: "استعمال: koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "مہیا کردہ سورس پر Kotodama کے lint چلائیں۔",
    ..english::MESSAGES
};
