use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "没有可编译的函数",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "未知参数 {name}",
    read_file: "无法读取 {path}: {error}",
    parser_error: "解析错误：{error}",
    semantic_error: "语义错误：{error}",
    lint_unused_state: "状态`{name}`已声明但从未使用",
    lint_state_shadowed_param: "函数`{func}`中的参数`{name}`遮蔽了状态`{name}`；请重命名该参数以访问该状态",
    lint_state_shadowed_binding: "函数`{func}`中的绑定`{name}`遮蔽了状态`{name}`；请重命名该绑定以保持对状态的访问",
    lint_state_shadowed_map_binding: "函数`{func}`在遍历映射时的绑定`{name}`遮蔽了状态`{name}`",
    lint_unused_parameter: "函数`{func}`中的参数`{name}`从未使用",
    lint_unreachable_after_return: "在{context}中检测到不可达语句：return 之后的代码永远不会执行",
    lint_ok: "正常",
    lint_usage: "用法：koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "对提供的源文件运行 Kotodama 的 lint。",
    ..english::MESSAGES
};
