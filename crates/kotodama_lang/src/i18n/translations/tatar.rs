use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "Компиляция өчен функцияләр юк",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "Билгесез параметр {name}",
    read_file: "{path} файлын укып булмады: {error}",
    parser_error: "Парсер хатасы: {error}",
    semantic_error: "Семантик хатасы: {error}",
    lint_unused_state: "`{name}` хәле белдерелгән, ләкин кулланылмый",
    lint_state_shadowed_param: "`{func}` функциясендәге `{name}` параметры `{name}` хәлен каплый; хәлгә керү өчен параметр исемен алыштырыгыз",
    lint_state_shadowed_binding: "`{func}` функциясендәге `{name}` бәйләнеше `{name}` хәлен каплый; хәлгә юлны саклау өчен бәйләнеш исемен алыштырыгыз",
    lint_state_shadowed_map_binding: "`{func}` функциясе карта буенча барганда `{name}` бәйләнеше `{name}` хәлен каплый",
    lint_unused_parameter: "`{func}` функциясендәге `{name}` параметры бөтенләй кулланылмый",
    lint_unreachable_after_return: "{context} эчендә үтәлми торган оператор табылды: return-нан соң код эшләми",
    lint_ok: "яхшы",
    lint_usage: "Куллану: koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "Бирелгән чыганаклар өчен Kotodama lint-ларын эшләтегез.",
    ..english::MESSAGES
};
