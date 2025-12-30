use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "མཉེན་ཆས་ཀྱི་ལས་འཆར་མེད་པས་མཉེན་ཆས་ཚང་མེད།",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "ངོ་མ་མེད་པའི་ཚད་བསྡུས {name}",
    read_file: "ཡིག་ཆ་ {path} ཀློག་མི་ཚུབ།: {error}",
    parser_error: "དབྱེ་སུབ་ནོར་འཁྲུལ: {error}",
    semantic_error: "དོན་ཚན་ནོར་འཁྲུལ: {error}",
    lint_unused_state: "གནས་སྟངས་ `{name}` གསལ་བཀོད་བྱས་ཡོད་ཀྱང་སྤྱོད་པ་མེད།",
    lint_state_shadowed_param: "ལས་འཆར་ `{func}` ནང་གི་ ཚད་བསྡུས་ `{name}` གནས་སྟངས་ `{name}` སྐུད་འཁྱོར་བྱེད་པས་ འདི་ལས་འགྲོ་བརྗེ་བསྒྱུར་བྱས་ནས་གནས་སྟངས་ལ་འཛུལ།",
    lint_state_shadowed_binding: "ལས་འཆར་ `{func}` ནང་གི་མཐུན་སྒྲིག `{name}` གནས་སྟངས་ `{name}` སྐུད་འཁྱོར་བྱས། འཛུལ་ལམ་ལུས་མི་བཅུག་པར་མཐུན་སྒྲིག་གི་མིང་བསྒྱུར།",
    lint_state_shadowed_map_binding: "ལས་འཆར་ `{func}` ནང་ས་ཁྲ་ལ་བསྐྱར་འཁོར་བྱེད་སྐབས་མཐུན་སྒྲིག `{name}` གནས་སྟངས་ `{name}` སྐུད་འཁྱོར་བྱས།",
    lint_unused_parameter: "ལས་འཆར་ `{func}` ནང་གི་ཚད་བསྡུས་ `{name}` སྤྱོད་པ་མེད།",
    lint_unreachable_after_return: "{context} ནང་འཛུལ་མི་རུང་བའི་གསལ་བརྗོད་མཐོང་སྟེ་ return རྗེས་ཀྱི་ཨང་རྟགས་སྤྱོད་མེད།",
    lint_ok: "བདེ་ལེགས།",
    lint_usage: "བཀོལ་སྤྱོད་: koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "Kotodama གི་ལིནཊི་བརྟག་ཞིབ་དེབ་གྲངས་འདིར་སྤྱོད།",
    ..english::MESSAGES
};
