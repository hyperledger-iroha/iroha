use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "Geen funksies om saam te stel nie",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "Onbekende parameter {name}",
    read_file: "Kon nie lêer {path} lees nie: {error}",
    parser_error: "Ontlederfout: {error}",
    semantic_error: "Semantiese fout: {error}",
    lint_unused_state: "toestand `{name}` is gedeclareer maar word nooit gebruik nie",
    lint_state_shadowed_param: "parameter `{name}` in funksie `{func}` verberg toestand `{name}`; hernoem die parameter om toegang tot die toestand te kry",
    lint_state_shadowed_binding: "binding `{name}` in funksie `{func}` verberg toestand `{name}`; hernoem die binding om toegang tot die toestand te behou",
    lint_state_shadowed_map_binding: "binding `{name}` in funksie `{func}` verberg toestand `{name}` tydens kaart-iterasie",
    lint_unused_parameter: "parameter `{name}` in funksie `{func}` word nooit gebruik nie",
    lint_unreachable_after_return: "onbereikbare stelling opgespoor in {context}: kode na ’n return word nooit uitgevoer nie",
    lint_ok: "goed",
    lint_usage: "Gebruik: koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "Voer Kotodama se lint-kontroles op die verskafte bronne uit.",
    ..english::MESSAGES
};
