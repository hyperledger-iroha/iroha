use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "Nogat fankson fo kompailim",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "Parameta ia yumi no save {name}",
    read_file: "Fail fo ridim fael {path}: {error}",
    parser_error: "Parser i mekem mistek: {error}",
    semantic_error: "Semantik eria: {error}",
    lint_unused_state: "State `{name}` hemi bin olsem bat hemi no yusim nating",
    lint_state_shadowed_param: "Parameter `{name}` long function `{func}` i haedem state `{name}`; senisim nem blong parameter blong kasem state",
    lint_state_shadowed_binding: "Binding `{name}` long function `{func}` i haedem state `{name}`; senisim nem blong binding blong mekem state i stap open",
    lint_state_shadowed_map_binding: "Taem `{func}` i raun ova long map, binding `{name}` i haedem state `{name}`",
    lint_unused_parameter: "Parameter `{name}` long function `{func}` i no yusim nating",
    lint_unreachable_after_return: "Statement insaed long {context} i no save kasem; kode bihaen long return i no ron",
    lint_ok: "oraet",
    lint_usage: "Yus: koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "Ronim Kotodama lint long ol fael we iu givim.",
    ..english::MESSAGES
};
