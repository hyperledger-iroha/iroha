use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "د کمپایل لپاره هېڅ فنکشن شتون نه لري",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "نامعلوم پارامیټر {name}",
    read_file: "دوتنه {path} نه شي لوستل کېدای: {error}",
    parser_error: "پارسر تېروتنه: {error}",
    semantic_error: "منځپانګه يي تېروتنه: {error}",
    lint_unused_state: "حالت `{name}` اعلان شوی خو هېڅکله نه کارېږي",
    lint_state_shadowed_param: "پارامیټر `{name}` په فنکشن `{func}` کې حالت `{name}` پټوي؛ د حالت لاسرسي لپاره د پارامیټر نوم بدل کړئ",
    lint_state_shadowed_binding: "تړاو `{name}` په فنکشن `{func}` کې حالت `{name}` پټوي؛ د لاسرسي ساتلو لپاره د تړاو نوم بدل کړئ",
    lint_state_shadowed_map_binding: "کله چې فنکشن `{func}` پر نقشه ګرځي، تړاو `{name}` حالت `{name}` پټوي",
    lint_unused_parameter: "پارامیټر `{name}` په فنکشن `{func}` کې هېڅکله نه کارېږي",
    lint_unreachable_after_return: "په {context} کې نه رسیېدونکی عبارت وموندل شو: د return وروسته کوډ هېڅکله نه اجرا کېږي",
    lint_ok: "سمه ده",
    lint_usage: "کارونه: koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "د ورکړل شويو سرچینو لپاره د Kotodama lint تفتیشونه چلوئ.",
    ..english::MESSAGES
};
