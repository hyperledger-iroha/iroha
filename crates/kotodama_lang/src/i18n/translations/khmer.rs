use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "មិនមានមុខងារឱ្យចងក្រងទេ",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "ប៉ារ៉ាម៉ែត្រដែលមិនស្គាល់ {name}",
    read_file: "មិនអាចអានឯកសារ {path}: {error}",
    parser_error: "កំហុសក្នុងការបំបែកកូដ: {error}",
    semantic_error: "កំហុសអត្ថន័យ: {error}",
    lint_unused_state: "ស្ថានភាព `{name}` ត្រូវបានប្រកាសប៉ុន្តែមិនដែលត្រូវបានប្រើឡើយ",
    lint_state_shadowed_param: "ប៉ារ៉ាម៉ែត្រ `{name}` ក្នុងអនុគមន៍ `{func}` បាំងស្ថានភាព `{name}`; សូមប្តូរឈ្មោះប៉ារ៉ាម៉ែត្រដើម្បីចូលដំណើរការ",
    lint_state_shadowed_binding: "ការចង `{name}` ក្នុងអនុគមន៍ `{func}` បាំងស្ថានភាព `{name}`; សូមប្តូរឈ្មោះការចងដើម្បីរក្សាការចូលដំណើរការ",
    lint_state_shadowed_map_binding: "ការចង `{name}` ក្នុងអនុគមន៍ `{func}` បាំងស្ថានភាព `{name}` អំឡុងពេលវង្វេងតាមផែនទី",
    lint_unused_parameter: "ប៉ារ៉ាម៉ែត្រ `{name}` ក្នុងអនុគមន៍ `{func}` មិនដែលត្រូវបានប្រើ",
    lint_unreachable_after_return: "រកឃើញសេចក្តីថ្លែងការណ៍ដែលមិនអាចទៅដល់នៅក្នុង {context}: កូដបន្ទាប់ពី return មិនត្រូវបានដំណើរការឡើយ",
    lint_ok: "រួចរាល់",
    lint_usage: "របៀបប្រើ: koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "ដំណើរការការត្រួតពិនិត្យ lint របស់ Kotodama លើប្រភពដែលបានផ្តល់ជូន។",
    ..english::MESSAGES
};
