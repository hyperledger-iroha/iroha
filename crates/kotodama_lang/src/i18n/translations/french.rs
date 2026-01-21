use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "Aucune fonction à compiler",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "Param inconnu {name}",
    read_file: "Échec de la lecture de {path} : {error}",
    parser_error: "Erreur de l'analyseur : {error}",
    semantic_error: "Erreur sémantique : {error}",
    lint_unused_state: "L'état `{name}` est déclaré mais n'est jamais utilisé",
    lint_state_shadowed_param: "Le paramètre `{name}` de la fonction `{func}` masque l'état `{name}` ; renommez le paramètre pour accéder à l'état",
    lint_state_shadowed_binding: "La liaison `{name}` dans la fonction `{func}` masque l'état `{name}` ; renommez la liaison pour conserver l'accès à l'état",
    lint_state_shadowed_map_binding: "La liaison `{name}` dans la fonction `{func}` masque l'état `{name}` pendant l'itération de la map",
    lint_unused_parameter: "Le paramètre `{name}` de la fonction `{func}` n'est jamais utilisé",
    lint_unreachable_after_return: "Instruction inatteignable détectée dans {context} : le code après un return n'est jamais exécuté",
    lint_ok: "ok",
    lint_usage: "Utilisation : koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "Exécute les lints Kotodama sur les sources fournies.",
    ..english::MESSAGES
};
