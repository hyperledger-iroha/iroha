use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "Mana ima ruwaykunapas huñunapaq",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "Mana riqsisqa param {name}",
    read_file: "mana ñawinchayta atirqanchu {path}: {error}",
    parser_error: "Parser pantay: {error}",
    semantic_error: "Semántico pantay: {error}",
    lint_unused_state: "estado `{name}` rurasqa kani, ichaqa manam hinaspa llamk'aykuykuchu",
    lint_state_shadowed_param: "parametru `{name}` ruway `{func}` nisqapi estado `{name}` tapuqkuchkan; estado ta haykuykuykama parametru sutita hukmanta churay",
    lint_state_shadowed_binding: "huñuy `{name}` ruway `{func}` nisqapi estado `{name}` pakachkan; haykuyta qhispichiy nirqaykama huñu sutita hukmanta churay",
    lint_state_shadowed_map_binding: "map nisqata kutichkaptin ruway `{func}` nisqapi huñuy `{name}` estado `{name}` pakachkan",
    lint_unused_parameter: "parametru `{name}` ruway `{func}` nisqapi manam miderankuchu",
    lint_unreachable_after_return: "{context} nisqapi manam chaynaman chayarimayta atinchu: return ñawpanpi kachkan kodigoqa manam llamk'achkanchu",
    lint_ok: "allin",
    lint_usage: "Imayna llamk'achiy: koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "Chay apachisqa source nisqakunapi Kotodama lint qhawariykuna purichiy",
    ..english::MESSAGES
};
