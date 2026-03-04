use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "Não há funções para compilar",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "Parâmetro desconhecido {name}",
    read_file: "Falha ao ler o arquivo {path}: {error}",
    parser_error: "Erro do analisador: {error}",
    semantic_error: "Erro semântico: {error}",
    lint_unused_state: "O estado `{name}` foi declarado, mas nunca é usado",
    lint_state_shadowed_param: "O parâmetro `{name}` na função `{func}` oculta o estado `{name}`; renomeie o parâmetro para acessar o estado",
    lint_state_shadowed_binding: "A vinculação `{name}` na função `{func}` oculta o estado `{name}`; renomeie a vinculação para manter o acesso ao estado",
    lint_state_shadowed_map_binding: "A vinculação `{name}` na função `{func}` oculta o estado `{name}` durante a iteração do mapa",
    lint_unused_parameter: "O parâmetro `{name}` na função `{func}` nunca é usado",
    lint_unreachable_after_return: "Instrução inalcançável detectada em {context}: o código após um return nunca é executado",
    lint_ok: "ok",
    lint_usage: "Uso: koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "Executa os lints do Kotodama nas fontes fornecidas.",
    ..english::MESSAGES
};
