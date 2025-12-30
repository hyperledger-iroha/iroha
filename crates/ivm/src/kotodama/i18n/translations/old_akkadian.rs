use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "𒅆𒅁𒊒 𒀭𒈾 𒈾𒀊𒆷𒅆 𒌑𒇻 𒄿𒋫𒆷𒀝",
    unsupported_binary_op: "𒀭𒊑 𒋗𒈪 𒄷𒋾 {op}",
    unknown_param: "𒉺𒊏𒈬 𒆷 𒄿𒁺 {name}",
    read_file: "𒋻𒁍 {path} 𒌑𒇻 𒄿𒋗𒌅 {error}",
    parser_error: "𒋗𒁺 𒉺𒊏𒍑 {error}",
    semantic_error: "𒋗𒁺 𒈾𒁍𒌈 {error}",
    lint_unused_state: "𒋳𒉺 `{name}` 𒁺𒁍 𒆷 𒋳𒅕",
    lint_state_shadowed_param: "𒋻 `{func}` 𒅁𒉺 `{name}` 𒋳𒆷 𒋳𒉺 `{name}`; 𒅁𒉺 `{name}` 𒋾𒈪 𒀭𒈾 𒋳𒉺",
    lint_state_shadowed_binding: "𒋻 `{func}` 𒋻𒉺 `{name}` 𒋳𒆷 𒋳𒉺 `{name}`; 𒋻𒉺 `{name}` 𒋾𒈪 𒀭𒈾 𒋳𒉺",
    lint_state_shadowed_map_binding: "𒋻 `{func}` 𒋻𒉺 `{name}` 𒋳𒆷 𒋳𒉺 `{name}` 𒉈𒍑 𒂍; 𒋻𒉺 `{name}` 𒋾𒈪 𒀭𒈾 𒋳𒉺",
    lint_unused_parameter: "𒋻 `{func}` 𒅁𒉺 `{name}` 𒆷 𒅁𒋾",
    lint_unreachable_after_return: "𒉈𒍑 {context} 𒆷 𒋳𒌋; 𒄿𒊑 𒆷 𒂊𒉺 𒅁𒋾",
    lint_ok: "𒋗𒇻𒈬",
    lint_usage: "𒄿𒊑𒄩 `koto_lint` 𒅁𒋾 <file.ko> [<file2.ko> ...]",
    lint_usage_help: "𒉈𒍑 Kotodama 𒋼𒇻𒅕 𒀭𒈾 𒄿𒊑",
    ..english::MESSAGES
};
