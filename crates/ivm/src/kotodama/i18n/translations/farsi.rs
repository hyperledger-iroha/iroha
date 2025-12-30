use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "هیچ تابعی برای کامپایل وجود ندارد",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "پارامتر ناشناخته {name}",
    read_file: "خواندن فایل {path} ممکن نشد: {error}",
    parser_error: "خطای تجزیه‌گر: {error}",
    semantic_error: "خطای معنایی: {error}",
    lint_unused_state: "وضعیت `{name}` اعلام شده است اما هرگز استفاده نمی‌شود",
    lint_state_shadowed_param: "پارامتر `{name}` در تابع `{func}` وضعیت `{name}` را پنهان می‌کند؛ برای دسترسی به وضعیت نام پارامتر را تغییر دهید",
    lint_state_shadowed_binding: "پیوند `{name}` در تابع `{func}` وضعیت `{name}` را پنهان می‌کند؛ برای حفظ دسترسی نام پیوند را تغییر دهید",
    lint_state_shadowed_map_binding: "پیوند `{name}` در تابع `{func}` هنگام پیمایش نقشه وضعیت `{name}` را پنهان می‌کند",
    lint_unused_parameter: "پارامتر `{name}` در تابع `{func}` هرگز استفاده نمی‌شود",
    lint_unreachable_after_return: "دستور غیرقابل دسترس در {context} شناسایی شد: کد پس از return هرگز اجرا نمی‌شود",
    lint_ok: "موفق",
    lint_usage: "نحوه استفاده: koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "بر روی منابع ارائه‌شده lint های Kotodama را اجرا می‌کند.",
    ..english::MESSAGES
};
