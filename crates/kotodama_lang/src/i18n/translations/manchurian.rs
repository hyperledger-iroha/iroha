use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "Kompilacijangga fungsi akū",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "Beye hūwa parametar akū {name}",
    read_file: "Beye bithe {path} sabuha bolodahūn akū: {error}",
    parser_error: "Parser sain akū: {error}",
    semantic_error: "Semantika sain akū: {error}",
    lint_unused_state: "`{name}` state ilanaha, bicike ufarakū",
    lint_state_shadowed_param: "`{func}` function-i `{name}` parameter `{name}` state be hūlha; state de tucire de parameter mingge be muterengge",
    lint_state_shadowed_binding: "`{func}` function-i `{name}` binding `{name}` state be hūlha; state-i tucibume sain binding mingge be muterengge",
    lint_state_shadowed_map_binding: "`{func}` function map be jergi sindara seme `{name}` binding `{name}` state be hūlha",
    lint_unused_parameter: "`{func}` function-i `{name}` parameter afara seme ufarakū",
    lint_unreachable_after_return: "{context} de dorgi akū gisun narhūn tucibufi; return-i ice code gemu bihire",
    lint_ok: "saikan",
    lint_usage: "Kihebe: koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "Gisurehe faile be emu biya Kotodama lint de bahafi tucimbi.",
    ..english::MESSAGES
};
