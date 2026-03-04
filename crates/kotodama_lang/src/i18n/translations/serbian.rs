use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "Нема функција за компилацију",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "Непознати параметар {name}",
    read_file: "Није успело читање датотеке {path}: {error}",
    parser_error: "Грешка парсера: {error}",
    semantic_error: "Семантичка грешка: {error}",
    lint_unused_state: "стање `{name}` је декларисано, али се никада не користи",
    lint_state_shadowed_param: "параметар `{name}` у функцији `{func}` прикрива стање `{name}`; преименујте параметар да бисте приступили стању",
    lint_state_shadowed_binding: "веза `{name}` у функцији `{func}` прикрива стање `{name}`; преименујте везу да бисте задржали приступ",
    lint_state_shadowed_map_binding: "веза `{name}` у функцији `{func}` прикрива стање `{name}` током итерирања мапе",
    lint_unused_parameter: "параметар `{name}` у функцији `{func}` се никада не користи",
    lint_unreachable_after_return: "откривена је недостижна наредба у {context}: код после return наредбе се никада не извршава",
    lint_ok: "у реду",
    lint_usage: "Употреба: koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "Покреће Kotodama lint провере над наведеним изворним датотекама.",
    ..english::MESSAGES
};
