use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "Ko si iṣẹ́ kankan láti kóńpáílì",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "Paramíta tí a kò mọ̀ {name}",
    read_file: "A kùnà láti kà fáìlì {path}: {error}",
    parser_error: "Aṣiṣe parser: {error}",
    semantic_error: "Aṣiṣe ìtumọ̀: {error}",
    lint_unused_state: "ipò `{name}` ni a kede ṣùgbọ́n a kò lo rárá",
    lint_state_shadowed_param: "paramíta `{name}` nínú iṣẹ́ `{func}` ń bọ̀ ipò `{name}` mọ́lẹ̀; ṣe àtún-orúkọ paramíta náà kí o lè wọlé sí ipò náà",
    lint_state_shadowed_binding: "ìdípọ̀ `{name}` nínú iṣẹ́ `{func}` ń bọ̀ ipò `{name}` mọ́lẹ̀; ṣe àtún-orúkọ ìdípọ̀ náà kí ìwọlé má bàjẹ́",
    lint_state_shadowed_map_binding: "nígbà tí iṣẹ́ `{func}` ń yí map ká, ìdípọ̀ `{name}` ń bọ̀ ipò `{name}` mọ́lẹ̀",
    lint_unused_parameter: "paramíta `{name}` nínú iṣẹ́ `{func}` kò ṣeé lò rárá",
    lint_unreachable_after_return: "a rí ìkílọ̀ pé àṣẹ kan kò lè dé ní {context}: kóòdù lẹ́yìn return kì yóò ṣiṣẹ́ rí",
    lint_ok: "ó dáa",
    lint_usage: "Bí a ṣe ń lò ó: koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "Ṣìṣe àwọn àyẹ̀wò lint Kotodama lórí àwôn orísun tí a fún ni.",
    ..english::MESSAGES
};
