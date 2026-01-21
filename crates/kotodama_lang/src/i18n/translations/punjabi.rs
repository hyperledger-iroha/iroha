use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "ਕੰਪਾਈਲ ਕਰਨ ਲਈ ਕੋਈ ਫੰਕਸ਼ਨ ਨਹੀਂ",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "ਅਣਜਾਣ ਪੈਰਾਮੀਟਰ {name}",
    read_file: "ਫਾਇਲ {path} ਪੜ੍ਹਣ ਵਿੱਚ ਅਸਫਲ: {error}",
    parser_error: "ਪਾਰਸਰ ਗਲਤੀ: {error}",
    semantic_error: "ਸੈਮੈਂਟਿਕ ਗਲਤੀ: {error}",
    lint_unused_state: "state `{name}` ਦਾ ਐਲਾਨ ਕੀਤਾ ਗਿਆ ਹੈ ਪਰ ਕਦੇ ਵਰਤਿਆ ਨਹੀਂ ਗਿਆ",
    lint_state_shadowed_param: "ਫੰਕਸ਼ਨ `{func}` ਵਿੱਚ ਪੈਰਾਮੀਟਰ `{name}` state `{name}` ਨੂੰ ਲੁਕਾਉਂਦਾ ਹੈ; state ਤੱਕ ਪਹੁੰਚ ਲਈ ਪੈਰਾਮੀਟਰ ਦਾ ਨਾਮ ਬਦਲੋ",
    lint_state_shadowed_binding: "ਫੰਕਸ਼ਨ `{func}` ਵਿੱਚ binding `{name}` state `{name}` ਨੂੰ ਲੁਕਾਉਂਦੀ ਹੈ; ਪਹੁੰਚ ਬਰਕਰਾਰ ਰੱਖਣ ਲਈ binding ਦਾ ਨਾਮ ਬਦਲੋ",
    lint_state_shadowed_map_binding: "ਫੰਕਸ਼ਨ `{func}` ਵਿੱਚ ਮੈਪ ਤੇ ਇਟਰੇਟ ਕਰਦੇ ਸਮੇਂ binding `{name}` state `{name}` ਨੂੰ ਲੁਕਾਉਂਦੀ ਹੈ",
    lint_unused_parameter: "ਫੰਕਸ਼ਨ `{func}` ਵਿੱਚ ਪੈਰਾਮੀਟਰ `{name}` ਕਦੇ ਵਰਤਿਆ ਨਹੀਂ ਜਾਂਦਾ",
    lint_unreachable_after_return: "{context} ਵਿੱਚ ਅਪਹੁੰਚ ਵਿਵਸਥਾ ਮਿਲੀ: return ਤੋਂ ਬਾਅਦ ਵਾਲਾ ਕੋਡ ਕਦੇ ਨਹੀਂ ਚੱਲਦਾ",
    lint_ok: "ਠੀਕ",
    lint_usage: "ਵਰਤੋਂ: koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "ਦਿੱਤੇ ਗਏ ਸਰੋਤਾਂ 'ਤੇ Kotodama lint ਚਲਾਉਂਦਾ ਹੈ।",
    ..english::MESSAGES
};
