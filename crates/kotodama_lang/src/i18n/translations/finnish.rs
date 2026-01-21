use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "Ei funktioita käännettäväksi",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "Tuntematon parametri {name}",
    read_file: "Tiedoston {path} lukeminen epäonnistui: {error}",
    parser_error: "Jäsenninvirhe: {error}",
    semantic_error: "Semanttinen virhe: {error}",
    lint_unused_state: "Tila `{name}` on määritelty mutta sitä ei koskaan käytetä",
    lint_state_shadowed_param: "Parametri `{name}` funktiossa `{func}` peittää tilan `{name}`; nimeä parametri uudelleen käyttääksesi tilaa",
    lint_state_shadowed_binding: "Sidonta `{name}` funktiossa `{func}` peittää tilan `{name}`; nimeä sidonta uudelleen säilyttääksesi pääsyn",
    lint_state_shadowed_map_binding: "Sidonta `{name}` funktiossa `{func}` peittää tilan `{name}` map-iteroinnin aikana",
    lint_unused_parameter: "Parametria `{name}` funktiossa `{func}` ei käytetä",
    lint_unreachable_after_return: "Havaittiin saavuttamaton lause kontekstissa {context}: returnin jälkeistä koodia ei koskaan suoriteta",
    lint_ok: "ok",
    lint_usage: "Käyttö: koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "Ajaa Kotodaman lint-tarkistukset annetuissa lähdekooditiedostoissa.",
    ..english::MESSAGES
};
