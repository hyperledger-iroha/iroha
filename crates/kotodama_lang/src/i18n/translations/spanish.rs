use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "No hay funciones para compilar",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "Parámetro desconocido {name}",
    read_file: "Error al leer el archivo {path}: {error}",
    parser_error: "Error del analizador: {error}",
    semantic_error: "Error semántico: {error}",
    lint_unused_state: "El estado `{name}` se declara pero nunca se usa",
    lint_state_shadowed_param: "El parámetro `{name}` en la función `{func}` oculta el estado `{name}`; renombra el parámetro para acceder al estado",
    lint_state_shadowed_binding: "La vinculación `{name}` en la función `{func}` oculta el estado `{name}`; renombra la vinculación para mantener el acceso al estado",
    lint_state_shadowed_map_binding: "La vinculación `{name}` en la función `{func}` oculta el estado `{name}` durante la iteración del mapa",
    lint_unused_parameter: "El parámetro `{name}` en la función `{func}` no se usa nunca",
    lint_unreachable_after_return: "Se detectó una instrucción inaccesible en {context}: el código después de un return nunca se ejecuta",
    lint_ok: "correcto",
    lint_usage: "Uso: koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "Ejecuta las comprobaciones de lint de Kotodama sobre las fuentes proporcionadas.",
    ..english::MESSAGES
};
