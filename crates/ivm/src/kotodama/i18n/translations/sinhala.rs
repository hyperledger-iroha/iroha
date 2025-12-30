use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "සම්පාදනයට ක්‍රියාකාරකම් කිසිවක් නොමැත",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "හඳුනා නොගත් පරාමිතිය {name}",
    read_file: "{path} කියවීමට නොහැකි විය: {error}",
    parser_error: "විග්‍රහ දෝෂය: {error}",
    semantic_error: "අර්ථ දෝෂය: {error}",
    lint_unused_state: "`{name}` තත්වය ප්‍රකාශිත නමුත් කිසිවිටෙක භාවිතා නොවේ",
    lint_state_shadowed_param: "`{func}` ක්‍රියාවලියේ `{name}` පරාමිතිය `{name}` තත්වය සගවයි; තත්වය වෙත ප්‍රවේශ වීමට පරාමිතියේ නම වෙනස් කරන්න",
    lint_state_shadowed_binding: "`{func}` ක්‍රියාවලියේ `{name}` බැඳීම `{name}` තත්වය සගවයි; ප්‍රවේශය පවත්වා ගැනීමට බැඳීමේ නම වෙනස් කරන්න",
    lint_state_shadowed_map_binding: "`{func}` ක්‍රියාවලියේ map එක මඟින් චක්‍රය කරන විට `{name}` බැඳීම `{name}` තත්වය සගවයි",
    lint_unused_parameter: "`{func}` ක්‍රියාවලියේ `{name}` පරාමිතිය කිසිදා භාවිතා නොකරයි",
    lint_unreachable_after_return: "{context} තුළ නොහැකි ප්‍රකාශයක් අනාවරණය විය: return පසු ඇති කේතය කිසිදා ක්‍රියාත්මක නොවේ",
    lint_ok: "හරි",
    lint_usage: "භාවිතය: koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "සපයා ඇති මූලාශ්‍ර මත Kotodama lint පරීක්ෂණ ක්‍රියාත්මක කරයි.",
    ..english::MESSAGES
};
