use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "Компиляция жасайтын функция жоқ",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "Белгісіз параметр {name}",
    read_file: "{path} файлын оқу сәтсіз аяқталды: {error}",
    parser_error: "Парсер қатесі: {error}",
    semantic_error: "Семантикалық қате: {error}",
    lint_unused_state: "`{name}` күйі жарияланған, бірақ ешқашан қолданылмайды",
    lint_state_shadowed_param: "`{func}` функциясындағы `{name}` параметрі `{name}` күйін көлегейлейді; күйге қол жеткізу үшін параметрдің атын өзгертіңіз",
    lint_state_shadowed_binding: "`{func}` функциясындағы `{name}` байланысы `{name}` күйін көлегейлейді; қол жетімділікті сақтау үшін байланыстың атын өзгертіңіз",
    lint_state_shadowed_map_binding: "`{func}` функциясында карта бойынша өту кезінде `{name}` байланысы `{name}` күйін көлегейлейді",
    lint_unused_parameter: "`{func}` функциясындағы `{name}` параметрі қолданылмайды",
    lint_unreachable_after_return: "{context} ішінде жетуге болмайтын оператор анықталды: return-нен кейінгі код ешқашан орындалмайды",
    lint_ok: "ок",
    lint_usage: "Қолданылуы: koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "Берілген бастапқы файлдарда Kotodama lint тексерістерін іске қосады.",
    ..english::MESSAGES
};
