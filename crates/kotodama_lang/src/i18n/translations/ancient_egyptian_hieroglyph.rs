use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "𓈖𓄿𓊵𓅱 𓆑𓅱𓈖𓐍𓏏𓅱𓈖 𓉔𓄿𓂋 𓎡𓅱𓊪𓃀𓇋𓃭",
    unsupported_binary_op: "𓎼𓂋𓏏𓅱𓄿 𓃀𓇋𓄿𓈖𓏭 𓉔𓈖𓂧𓄿𓏏𓄿: {op}",
    unknown_param: "𓄿𓈙 𓅱𓎛𓈖𓏏𓄿 𓉔𓄿𓂋 𓂝𓅱𓈖 `{name}`",
    read_file: "𓄿𓅱𓈖 𓂝𓅱𓏏𓈖 `{path}` 𓏏𓅱𓎼𓄿𓃀 𓎛𓇌𓂧𓄿: `{error}`",
    parser_error: "𓊪𓄿𓂋𓋴𓇋𓂋 𓅓𓎛𓄿𓂧: {error}",
    semantic_error: "𓉔𓄿𓂧 𓋴𓅊𓈖𓂧𓄿𓂋𓂧: {error}",
    lint_unused_state: "`{name}` 𓋴𓍿𓄿𓏏 𓅓𓂝𓂻𓄿 𓅓𓅱𓈖 𓊪𓅓𓂝",
    lint_state_shadowed_param: "𓋴𓅓𓋴 `{func}` 𓄿𓂋𓈙 `{name}` 𓋴𓍿𓄿𓏏 𓅓𓂝𓂻𓄿; 𓋴𓈖𓆑𓂋 𓈖𓂝𓈖 𓄿𓂋𓈙 𓉔𓆑𓂋 𓋴𓍿𓄿𓏏",
    lint_state_shadowed_binding: "𓋴𓅓𓋴 `{func}` 𓉔𓆑𓂋 𓈖𓍿𓂋𓈙 `{name}` 𓋴𓍿𓄿𓏏 𓅓𓂝𓂻𓄿; 𓋴𓈖𓆑𓂋 𓈖𓂝𓈖 𓍿𓂋𓈙 𓉔𓆑𓂋 𓋴𓍿𓄿𓏏",
    lint_state_shadowed_map_binding: "𓎛𓈖𓏏𓇋 `{func}` 𓋴𓂋𓊪 𓎡𓂋𓈖𓂋 𓍿𓂋𓈙 `{name}` 𓋴𓍿𓄿𓏏 𓅓𓂝𓂻𓄿",
    lint_unused_parameter: "𓋴𓅓𓋴 `{func}` 𓄿𓂋𓈙 `{name}` 𓅓𓅱𓈖 𓊪𓅓𓂝",
    lint_unreachable_after_return: "{context} 𓅓 𓋴𓂻𓈖𓅱 𓂋𓏲𓏏𓈖 𓅱𓂝 𓂋𓈖 𓂋𓏤; 𓎛𓂧𓋴 𓎝𓂋𓈖 𓎼𓃭𓂧 𓅱𓈗 𓂝",
    lint_ok: "𓈖𓆑𓂋",
    lint_usage: "𓎛𓄿𓂧 𓋴𓂝𓄿𓃀: 𓎟𓏏𓃀𓇋𓆑𓏏 <𓅱𓅱𓋴𓏏.𓎡𓅱> [<𓅱𓅱𓋴𓎡𓅱2> 𓂝𓂝𓎛]",
    lint_usage_help: "𓋴𓆑𓂋𓂋 𓍿𓂋𓈙𓄿 𓂝𓈘𓄿 𓅓 𓐍𓂝𓈖𓌻𓈖",
    ..english::MESSAGES
};
