use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "Nessuna funzione da compilare",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "Parametro sconosciuto {name}",
    read_file: "Impossibile leggere il file {path}: {error}",
    parser_error: "Errore del parser: {error}",
    semantic_error: "Errore semantico: {error}",
    lint_unused_state: "Lo stato `{name}` è dichiarato ma non viene mai usato",
    lint_state_shadowed_param: "Il parametro `{name}` nella funzione `{func}` oscura lo stato `{name}`; rinomina il parametro per accedere allo stato",
    lint_state_shadowed_binding: "Il binding `{name}` nella funzione `{func}` oscura lo stato `{name}`; rinomina il binding per mantenere l'accesso allo stato",
    lint_state_shadowed_map_binding: "Il binding `{name}` nella funzione `{func}` oscura lo stato `{name}` durante l'iterazione della mappa",
    lint_unused_parameter: "Il parametro `{name}` nella funzione `{func}` non viene mai usato",
    lint_unreachable_after_return: "Istruzione irraggiungibile rilevata in {context}: il codice dopo un return non viene mai eseguito",
    lint_ok: "ok",
    lint_usage: "Uso: koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "Esegue i lint di Kotodama sulle sorgenti fornite.",
    ..english::MESSAGES
};
