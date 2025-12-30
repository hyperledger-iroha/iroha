use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "कम्पाइल गर्न कुनै फङ्सन छैन",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "अज्ञात प्यारामिटर {name}",
    read_file: "फाइल {path} पढ्न सकिएन: {error}",
    parser_error: "पार्सर त्रुटि: {error}",
    semantic_error: "अर्थसम्बन्धी त्रुटि: {error}",
    lint_unused_state: "state `{name}` घोषण गरिएको छ तर कहिल्यै प्रयोग गरिएको छैन",
    lint_state_shadowed_param: "फङ्सन `{func}` मा `{name}` प्यारामिटरले state `{name}` लाई लुकाउँछ; state पहुँच गर्न प्यारामिटरको नाम परिवर्तन गर्नुहोस्",
    lint_state_shadowed_binding: "फङ्सन `{func}` मा binding `{name}` ले state `{name}` लाई लुकाउँछ; पहुँच कायम राख्न binding को नाम परिवर्तन गर्नुहोस्",
    lint_state_shadowed_map_binding: "फङ्सन `{func}` मा map दोहोर्‍याउँदा binding `{name}` ले state `{name}` लाई लुकाउँछ",
    lint_unused_parameter: "फङ्सन `{func}` मा प्यारामिटर `{name}` कहिल्यै प्रयोग हुँदैन",
    lint_unreachable_after_return: "{context} मा पुग्न नसकिने विधान फेला पर्यो: return पछि रहेको कोड कहिल्यै चल्दैन",
    lint_ok: "ठिक",
    lint_usage: "प्रयोग: koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "दिइएका स्रोतहरूमा Kotodama lint जाँचहरू चलाउँछ।",
    ..english::MESSAGES
};
