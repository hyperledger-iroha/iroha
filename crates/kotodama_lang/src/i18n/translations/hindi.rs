use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "संकलित करने के लिए कोई कार्य नहीं",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "अज्ञात पैराम {name}",
    read_file: "पढ़ने में विफल {path}: {error}",
    parser_error: "पार्सर त्रुटि: {error}",
    semantic_error: "सैमांटिक त्रुटि: {error}",
    lint_unused_state: "स्टेट `{name}` की घोषणा की गई है लेकिन कभी उपयोग नहीं हुआ",
    lint_state_shadowed_param: "फ़ंक्शन `{func}` का पैरामीटर `{name}` स्टेट `{name}` को छिपा देता है; स्टेट तक पहुँचने के लिए पैरामीटर का नाम बदलें",
    lint_state_shadowed_binding: "फ़ंक्शन `{func}` में बाइंडिंग `{name}` स्टेट `{name}` को छिपा देती है; स्टेट तक पहुँच बनाए रखने के लिए बाइंडिंग का नाम बदलें",
    lint_state_shadowed_map_binding: "मैप को दोहराते समय फ़ंक्शन `{func}` में बाइंडिंग `{name}` स्टेट `{name}` को छिपा देती है",
    lint_unused_parameter: "फ़ंक्शन `{func}` का पैरामीटर `{name}` कभी उपयोग नहीं होता",
    lint_unreachable_after_return: "{context} में अप्राप्य कथन मिला: return के बाद का कोड कभी निष्पादित नहीं होता",
    lint_ok: "ठीक",
    lint_usage: "उपयोग: koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "दिए गए स्रोतों पर Kotodama लिंट चलाता है।",
    ..english::MESSAGES
};
