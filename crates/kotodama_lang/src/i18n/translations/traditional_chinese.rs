use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "沒有可編譯的函數",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "未知參數 {name}",
    read_file: "無法讀取 {path}: {error}",
    parser_error: "解析錯誤：{error}",
    semantic_error: "語義錯誤：{error}",
    lint_unused_state: "狀態`{name}`已宣告但從未使用",
    lint_state_shadowed_param: "函式`{func}`中的參數`{name}`遮蔽了狀態`{name}`；請重新命名該參數以存取該狀態",
    lint_state_shadowed_binding: "函式`{func}`中的綁定`{name}`遮蔽了狀態`{name}`；請重新命名該綁定以維持對狀態的存取",
    lint_state_shadowed_map_binding: "函式`{func}`在巡訪映射時的綁定`{name}`遮蔽了狀態`{name}`",
    lint_unused_parameter: "函式`{func}`中的參數`{name}`從未使用",
    lint_unreachable_after_return: "在 {context} 偵測到無法到達的敘述：return 之後的程式碼不會執行",
    lint_ok: "正常",
    lint_usage: "使用方式：koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "對提供的原始碼執行 Kotodama 的 lint。",
    ..english::MESSAGES
};
