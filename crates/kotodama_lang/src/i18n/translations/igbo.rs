use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "Enweghị ọrụ iji kọnpịa",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "Parámịtà amaghi {name}",
    read_file: "Agụghị faịlụ {path}: {error}",
    parser_error: "Mperi parser: {error}",
    semantic_error: "Mperi nkọwa: {error}",
    lint_unused_state: "etinyere ọnọdụ `{name}` mana a naghị eji ya eme ihe",
    lint_state_shadowed_param: "parámịtà `{name}` n'ọrụ `{func}` na-eme ka ọnọdụ `{name}` daa n'ọnwụ; gbanwee aha parámịtà iji nweta ọnọdụ ahụ",
    lint_state_shadowed_binding: "njikọ `{name}` n'ọrụ `{func}` na-ekpuchi ọnọdụ `{name}`; gbanwee aha njikọ iji debe ohere",
    lint_state_shadowed_map_binding: "n'oge a na-emegharị map n'ọrụ `{func}`, njikọ `{name}` na-ekpuchi ọnọdụ `{name}`",
    lint_unused_parameter: "parámịtà `{name}` n'ọrụ `{func}` adịghị eji ya eme ihe",
    lint_unreachable_after_return: "achọpụtara okwu a na-agaghị enweta na {context}: koodu dị mgbe return anaghị agba ọsọ",
    lint_ok: "ọ dị mma",
    lint_usage: "Ojiji: koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "Gbaa nyocha lint Kotodama na isi mmalite enyere.",
    ..english::MESSAGES
};
