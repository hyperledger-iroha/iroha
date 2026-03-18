use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "தொகுக்க செயல்பாடுகள் எதுவும் இல்லை",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "அறியப்படாத அளவுரு {name}",
    read_file: "{path} ஐ படிக்க முடியவில்லை: {error}",
    parser_error: "பகுப்பாய்வி பிழை: {error}",
    semantic_error: "அர்த்த பிழை: {error}",
    lint_unused_state: "`{name}` நிலையைக் கூறியுள்ளீர்கள் ஆனால் அது ஒருபோதும் பயன்படுத்தப்படவில்லை",
    lint_state_shadowed_param: "`{func}` செயல்பாட்டில் உள்ள `{name}` அளவுரு `{name}` நிலையை மறைக்கிறது; நிலையை அணுக அளவுரின் பெயரை மாற்றவும்",
    lint_state_shadowed_binding: "`{func}` செயல்பாட்டில் உள்ள `{name}` பைண்டிங் `{name}` நிலையை மறைக்கிறது; அணுகலைப் பாதுகாக்க பைண்டிங் பெயரை மாற்றவும்",
    lint_state_shadowed_map_binding: "`{func}` செயல்பாட்டில் வரைபடத்தைச் சுழற்றும் போது `{name}` பைண்டிங் `{name}` நிலையை மறைக்கிறது",
    lint_unused_parameter: "`{func}` செயல்பாட்டில் `{name}` அளவுரு ஒருபோதும் பயன்படுத்தப்படவில்லை",
    lint_unreachable_after_return: "{context} இல் அணுக முடியாத கூற்று கண்டறியப்பட்டது: return க்கு பின் உள்ள குறியீடு ஒருபோதும் இயக்கப்படாது",
    lint_ok: "சரி",
    lint_usage: "பயன்பாடு: koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "வழங்கப்பட்ட மூலங்களின் மீது Kotodama lint சோதனைகளை இயக்குகிறது.",
    ..english::MESSAGES
};
