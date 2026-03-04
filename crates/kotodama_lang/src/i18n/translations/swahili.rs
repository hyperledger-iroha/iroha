use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "Hakuna kazi za kukusanya",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "Kigezo kisichojulikana {name}",
    read_file: "Imeshindikana kusoma faili {path}: {error}",
    parser_error: "Hitilafu ya parser: {error}",
    semantic_error: "Hitilafu ya maana: {error}",
    lint_unused_state: "hali `{name}` imetangazwa lakini haitumiki kamwe",
    lint_state_shadowed_param: "kigezo `{name}` kwenye kazi `{func}` kinaficha hali `{name}`; badilisha jina la kigezo ili kufikia hali",
    lint_state_shadowed_binding: "binding `{name}` kwenye kazi `{func}` inaficha hali `{name}`; badilisha jina la binding ili kudumisha ufikiaji",
    lint_state_shadowed_map_binding: "binding `{name}` kwenye kazi `{func}` inaficha hali `{name}` wakati wa kuzunguka ramani",
    lint_unused_parameter: "kigezo `{name}` kwenye kazi `{func}` hakitumiki kamwe",
    lint_unreachable_after_return: "tamko lisiloweza kufikiwa limegunduliwa katika {context}: msimbo baada ya return hautatekelezwa kamwe",
    lint_ok: "sawa",
    lint_usage: "Matumizi: koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "Endesha ukaguzi wa lint wa Kotodama kwenye chanzo kilichotolewa.",
    ..english::MESSAGES
};
