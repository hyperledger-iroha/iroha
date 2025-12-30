use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "کمپائل کیتے کوئی فنکشن موجود نئیں",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "نامعلوم پیرامیٹر {name}",
    read_file: "فائل {path} پڑھن وچ ناکام: {error}",
    parser_error: "پارسر دی غلطی: {error}",
    semantic_error: "معنوی غلطی: {error}",
    lint_unused_state: "حالت `{name}` دا اعلان تھی ڳیا ہے، پر اوہدے نال کوئی کم نئیں تھیندا",
    lint_state_shadowed_param: "فنکشن `{func}` وچ `{name}` پیرامیٹر حالت `{name}` نوں اوجھل کریندا اے؛ حالت تک رسائی لئی پیرامیٹر دا ناں بدل دواو",
    lint_state_shadowed_binding: "فنکشن `{func}` وچ `{name}` ربط حالت `{name}` نوں اوجھل کریندا اے؛ رسائی برقرار رکھن لئی ربط دا ناں بدل دواو",
    lint_state_shadowed_map_binding: "جدوں `{func}` نقشے تے گشت کریندا اے، `{name}` ربط حالت `{name}` نوں اوجھل کریندا اے",
    lint_unused_parameter: "فنکشن `{func}` وچ `{name}` پیرامیٹر کدے استعمال نئیں تھیندا",
    lint_unreachable_after_return: "{context} وچ نا رسائی والا جملہ ملیا؛ return توں بعد والا کوڈ کدی وی نئیں چلدا",
    lint_ok: "ٹھیک",
    lint_usage: "استعمال: koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "دتی ڳئی سورساں تے Kotodama lint چلاو.",
    ..english::MESSAGES
};
