use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "ཕན་ཚུན་སྤྱོད་མི་བྱེད་པའི་ལས་སྦྱོར་མེད།",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "མ་ཤེས་པའི་ཚད་གཞི {name}",
    read_file: "{path} ཡིག་ཆ་ལྷག་མ་ཚུབ།: {error}",
    parser_error: "ཞིབ་འགྲེལ་མི་འགྲུབ་པའི་ནོར་འཁྲུལ: {error}",
    semantic_error: "དོན་ཚན་ནོར་འཁྲུལ: {error}",
    lint_unused_state: "state `{name}` གསལ་བསྒྲགས་བྱས་ཡོད་ཀྱང་སྤྱོད་པ་མེད།",
    lint_state_shadowed_param: "ལས་འགན `{func}` ནང་ `{name}` ཚད་གཞིས་ state `{name}` མུན་ནག་བཟོས། state ལྟ་སྤྱོད་བྱ་ནི་ལུ་ཚད་གཞིའི་མིང་སླར་གསར་བཟོ།",
    lint_state_shadowed_binding: "ལས་འགན `{func}` ནང་ `{name}` མཐུད་མཚམས་ཀྱིས་ state `{name}` མུན་ནག་བཟོས། state ལུ་ཐུགས་རྟགས་བཟོ་ནི་ལུ་མཐུད་མཚམས་མིང་སླར་གསར་བཟོ།",
    lint_state_shadowed_map_binding: "ལས་འགན `{func}` གིས་མེ་ལོང་ལུ་ལོག་རྒྱུད་སྐབས་ `{name}` མཐུད་མཚམས་ཀྱིས་ state `{name}` མུན་ནག་བཟོས།",
    lint_unused_parameter: "ལས་འགན `{func}` ནང་ `{name}` ཚད་གཞི་དེ་སྤྱོད་མེད།",
    lint_unreachable_after_return: "{context} ནང་ལྷག་མེད་པར་ཐོབ་མི་བརྡ་དོན་མངོན་ཡོད། return གཏང་ཞིང་གི་ཤུལ་མམ་གྱི་ཀོཌྲ་མེད།",
    lint_ok: "བཟང་",
    lint_usage: "སྤྱོད་ཐབས: koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "བྱིན་ཡོད་པའི་ཁྱབ་བསྒྲགས་ལུ Kotodama lint འཁོར་སྐོར་འབད།",
    ..english::MESSAGES
};
