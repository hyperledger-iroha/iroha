use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "Awekho ukusebenza okumele kuhlanganiswe",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "Ipharamitha engaziwa {name}",
    read_file: "Yehlulekile ukufunda ifayela {path}: {error}",
    parser_error: "Iphutha lomhlaziyi: {error}",
    semantic_error: "Iphutha lokuchaza: {error}",
    lint_unused_state: "isimo `{name}` simenyezelwe kodwa asikaze sisetshenziswe",
    lint_state_shadowed_param: "ipharamitha `{name}` kumsebenzi `{func}` ifihla isimo `{name}`; shintsha igama lepharamitha ukuze ufinyelele esimisweni",
    lint_state_shadowed_binding: "ibinding `{name}` kumsebenzi `{func}` ifihla isimo `{name}`; shintsha igama le binding ukuze ugcine ukufinyelela",
    lint_state_shadowed_map_binding: "ibinding `{name}` kumsebenzi `{func}` ifihla isimo `{name}` ngesikhathi uhamba nge mapa",
    lint_unused_parameter: "iphapharamitha `{name}` kumsebenzi `{func}` ayisetshenziswa nhlobo",
    lint_unreachable_after_return: "kutholwe isitatimende esingafinyeleleki ku {context}: ikhodi elandela return ayisoze isebenze",
    lint_ok: "kulungile",
    lint_usage: "Ukusetshenziswa: koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "Sebenzisa ukuhlola kwe-lint ye-Kotodama kuma-source anikeziwe.",
    ..english::MESSAGES
};
