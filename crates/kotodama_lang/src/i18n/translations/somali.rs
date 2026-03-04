use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "Wax shaqo ah looma helin si loo kobaayo",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "Halbeeg aan la aqoon {name}",
    read_file: "Waxaa ku guuldareystay in la akhriyo faylka {path}: {error}",
    parser_error: "Khalad furaha: {error}",
    semantic_error: "Khalad macne: {error}",
    lint_unused_state: "xaaladda `{name}` waa la sheegay balse waligeed lama isticmaalin",
    lint_state_shadowed_param: "halbeegga `{name}` ee ku jira shaqada `{func}` wuxuu qariya xaaladda `{name}`; beddel magaca halbeegga si aad u gasho xaaladda",
    lint_state_shadowed_binding: "isku xidhka `{name}` ee shaqada `{func}` wuxuu qariya xaaladda `{name}`; beddel magaca isku xidhka si helitaanka loo sii hayo",
    lint_state_shadowed_map_binding: "marka shaqada `{func}` ay dul marayso khariidadda, isku xidhka `{name}` wuxuu qariya xaaladda `{name}`",
    lint_unused_parameter: "halbeegga `{name}` ee shaqada `{func}` lama adeegsado marnaba",
    lint_unreachable_after_return: "waxaa laga helay hadal aan la gaadhi karin gudaha {context}: koodhka ka dambeeya return marnaba ma fulayo",
    lint_ok: "wacan",
    lint_usage: "Isticmaal: koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "Ka shaqee jeegagga lint ee Kotodama ee isha la siiyay.",
    ..english::MESSAGES
};
