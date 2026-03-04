use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "Немає функцій для компіляції",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "Невідомий параметр {name}",
    read_file: "Не вдалося прочитати файл {path}: {error}",
    parser_error: "Помилка парсера: {error}",
    semantic_error: "Семантична помилка: {error}",
    lint_unused_state: "Стан `{name}` оголошений, але жодного разу не використовується",
    lint_state_shadowed_param: "Параметр `{name}` у функції `{func}` приховує стан `{name}`; перейменуйте параметр, щоб отримати доступ до стану",
    lint_state_shadowed_binding: "Зв'язування `{name}` у функції `{func}` приховує стан `{name}`; перейменуйте зв'язування, щоб зберегти доступ",
    lint_state_shadowed_map_binding: "Зв'язування `{name}` у функції `{func}` приховує стан `{name}` під час ітерації карти",
    lint_unused_parameter: "Параметр `{name}` у функції `{func}` ніде не використовується",
    lint_unreachable_after_return: "Виявлено недосяжний оператор у {context}: код після return ніколи не виконується",
    lint_ok: "ок",
    lint_usage: "Використання: koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "Запускає lint-перевірки Kotodama для наданих вихідних файлів.",
    ..english::MESSAGES
};
