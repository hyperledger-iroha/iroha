use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "Компиляция өсөн функциялар юҡ",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "Билдәһеҙ параметр {name}",
    read_file: "{path} файлын уҡыу мөмкин булманы: {error}",
    parser_error: "Парсер хатаһы: {error}",
    semantic_error: "Семантик хата: {error}",
    lint_unused_state: "`{name}` хәле иғлан ителгән, ләкин ҡулланылмай",
    lint_state_shadowed_param: "`{func}` функцияһындағы `{name}` параметры `{name}` хәлен күләгәләй; хәлгә инеү өсөн параметр исемен үҙгәртегеҙ",
    lint_state_shadowed_binding: "`{func}` функцияһындағы `{name}` бәйләнеше `{name}` хәлен йәшерә; хәлгә сығыу мөмкинлеге өсөн бәйләнеш исемен алмаштырығыҙ",
    lint_state_shadowed_map_binding: "`{func}` функцияһы карта буйлап йөрөгәндә `{name}` бәйләнеше `{name}` хәлен йәшерә",
    lint_unused_parameter: "`{func}` функцияһындағы `{name}` параметры бөтөнләй ҡулланылмай",
    lint_unreachable_after_return: "{context} эсендә үтәлмәҫ оператор табылды: return-дан һуңғы код эшләмәй",
    lint_ok: "яҡшы",
    lint_usage: "Ҡулланыу: koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "Бирелгән сығанаҡтар өсөн Kotodama lint-тарын эшләтегеҙ.",
    ..english::MESSAGES
};
