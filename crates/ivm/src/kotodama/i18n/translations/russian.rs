use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "Нет функций для компиляции",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "Неизвестный параметр {name}",
    read_file: "Не удалось прочитать файл {path}: {error}",
    parser_error: "Ошибка парсера: {error}",
    semantic_error: "Семантическая ошибка: {error}",
    lint_unused_state: "Состояние `{name}` объявлено, но никогда не используется",
    lint_state_shadowed_param: "Параметр `{name}` в функции `{func}` скрывает состояние `{name}`; переименуйте параметр, чтобы получить доступ к состоянию",
    lint_state_shadowed_binding: "Связывание `{name}` в функции `{func}` скрывает состояние `{name}`; переименуйте связывание, чтобы сохранить доступ к состоянию",
    lint_state_shadowed_map_binding: "Связывание `{name}` в функции `{func}` скрывает состояние `{name}` при переборе карты",
    lint_unused_parameter: "Параметр `{name}` в функции `{func}` нигде не используется",
    lint_unreachable_after_return: "Обнаружено недостижимое выражение в {context}: код после return никогда не выполняется",
    lint_ok: "ок",
    lint_usage: "Использование: koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "Запускает проверки Kotodama для указанных исходников.",
    ..english::MESSAGES
};
