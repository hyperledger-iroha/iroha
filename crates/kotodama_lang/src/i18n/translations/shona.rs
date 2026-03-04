use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "Hapana basa rekukompaira",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "Paramita isingazivikanwe {name}",
    read_file: "Takatadza kuverenga faira {path}: {error}",
    parser_error: "Kukanganisa kweparser: {error}",
    semantic_error: "Kukanganisa kwekuudzirwa: {error}",
    lint_unused_state: "mamiriro `{name}` akaziviswa asi haashandiswe",
    lint_state_shadowed_param: "paramita `{name}` mubasa `{func}` inovanza mamiriro `{name}`; chinja zita reparamita kuti uwane mamiriro acho",
    lint_state_shadowed_binding: "kusunga `{name}` mubasa `{func}` kunovanza mamiriro `{name}`; chinja zita rekusunga kuti ugare uchiwana mamiriro acho",
    lint_state_shadowed_map_binding: "kusunga `{name}` mubasa `{func}` kunovanza mamiriro `{name}` panguva yekudzokorora mepu",
    lint_unused_parameter: "paramita `{name}` mubasa `{func}` haishandiswe zvachose",
    lint_unreachable_after_return: "chirevo chisvikike chawanikwa mu {context}: kodhi iri mushure me return haitombomhanya",
    lint_ok: "zvakanaka",
    lint_usage: "Mashandisiro: koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "Mhanya ongororo dze lint dzeKotodama pamafaira akapiwa.",
    ..english::MESSAGES
};
