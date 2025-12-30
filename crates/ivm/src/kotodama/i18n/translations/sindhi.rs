use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "ڪمپائيل ڪرڻ لاءِ ڪو به فنڪشن ناهي",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "اڻڄاتل پيراميٽر {name}",
    read_file: "فائل {path} پڙهڻ ۾ ناڪام: {error}",
    parser_error: "پارسنگ غلطي: {error}",
    semantic_error: "معنوي غلطي: {error}",
    lint_unused_state: "حالت `{name}` جو اعلان ٿيو پر ڪڏهن به استعمال نه ٿيو",
    lint_state_shadowed_param: "فنڪشن `{func}` ۾ پيراميٽر `{name}` حالت `{name}` کي لڪائي ٿو؛ حالت تائين پهچ لاءِ پيراميٽر جو نالو مٽايو",
    lint_state_shadowed_binding: "فنڪشن `{func}` ۾ رابطو `{name}` حالت `{name}` کي لڪائي ٿو؛ پهچ برقرار رکڻ لاءِ رابطو جو نالو مٽايو",
    lint_state_shadowed_map_binding: "فنڪشن `{func}` ۾ نقشي تي ورجاءَ دوران رابطو `{name}` حالت `{name}` کي لڪائي ٿو",
    lint_unused_parameter: "فنڪشن `{func}` ۾ پيراميٽر `{name}` ڪڏهن به استعمال نٿو ٿئي",
    lint_unreachable_after_return: "{context} ۾ نه پهچڻ جو بيان مليو: return کانپوءِ وارو ڪوڊ ڪڏهن به نه هلندو",
    lint_ok: "ٺيڪ",
    lint_usage: "استعمال: koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "Kotodama جي lint چيڪس کي ڏنل سورسز تي هلائي ٿو.",
    ..english::MESSAGES
};
