use super::super::Messages;

pub const MESSAGES: Messages = Messages {
    no_functions: "no functions to compile",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "unknown param {name}",
    read_file: "failed to read {path}: {error}",
    parser_error: "parser error: {error}",
    semantic_error: "semantic error: {error}",
    lint_unused_state: "state `{name}` is declared but never used",
    lint_state_shadowed_param: "parameter `{name}` in function `{func}` shadows state `{name}`; rename the parameter to access the state",
    lint_state_shadowed_binding: "binding `{name}` in function `{func}` shadows state `{name}`; rename the binding to keep the state accessible",
    lint_state_shadowed_map_binding: "binding `{name}` in function `{func}` shadows state `{name}` while iterating a map",
    lint_unused_parameter: "parameter `{name}` in function `{func}` is never used",
    lint_unreachable_after_return: "unreachable statement detected in {context}: code after a return never executes",
    lint_ok: "ok",
    lint_usage: "Usage: koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "Run Kotodama lints against the provided sources.",
};
