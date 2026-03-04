use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "کمپایل کرن لۄکھ فنکشن نْہ چھ",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "نامولوم پیرامیٹر {name}",
    read_file: "فایل {path} پڑھن منز ناکام: {error}",
    parser_error: "پارسر غلطی: {error}",
    semantic_error: "معنوی غلطی: {error}",
    lint_unused_state: "`{name}` حالت اعلٕان چھ مگر ہٕندٕ استعمال کنہٕ نہٕ گۄو",
    lint_state_shadowed_param: "فَنکشن `{func}` ہند `{name}` پارامیٹر حالت `{name}` آستاں چھ؛ حالت رسٕی میچان باپت پارامیٹر آکھ بدلٕو",
    lint_state_shadowed_binding: "فَنکشن `{func}` ہند `{name}` ربط حالت `{name}` آستاں چھ؛ رسٕی برقرار کرن باپت ربط آکھ بدلٕو",
    lint_state_shadowed_map_binding: "یَتھ `{func}` نقشو گژھان، ربط `{name}` حالت `{name}` آستاں چھ",
    lint_unused_parameter: "فَنکشن `{func}` ہند `{name}` پارامیٹر کنہٕ وٕنہٕ استعمال گۄو",
    lint_unreachable_after_return: "{context} منز نا رسٕی بیان ویل۪ی؛ ریٹرن پَت کۄڈ نہٕ چلان",
    lint_ok: "ٹھیک",
    lint_usage: "استعمال: koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "دیوان سۄرسن پٲت Kotodama lint چلٲو.",
    ..english::MESSAGES
};
