use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "ለመኮምፓይል ምንም ፋንክሽን የለም",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "ያልታወቀ ፓራሜትር {name}",
    read_file: "ፋይል {path} ማንበብ አልተቻለም: {error}",
    parser_error: "የፓርሰር ስህተት: {error}",
    semantic_error: "የትርጉም ስህተት: {error}",
    lint_unused_state: "ሁኔታ `{name}` ተገልጿል፣ ግን በግልጽ ሁኔታ አልተጠቀምበትም",
    lint_state_shadowed_param: "በ`{func}` ፋንክሽን ውስጥ ያለው ፓራሜትር `{name}` የ`{name}` ሁኔታን ይሸፍናል፤ ሁኔታውን ለመድረስ የፓራሜቱን ስም ይቀይሩ",
    lint_state_shadowed_binding: "በ`{func}` ፋንክሽን ውስጥ ያለው መቆለፊያ `{name}` የ`{name}` ሁኔታን ይሸፍናል፤ መዳረሻው እንዲቀጥል የመቆለፊያውን ስም ይቀይሩ",
    lint_state_shadowed_map_binding: "በ`{func}` ፋንክሽን ውስጥ ሜፕን ሲዘዋወር መቆለፊያ `{name}` የ`{name}` ሁኔታን ይሸፍናል",
    lint_unused_parameter: "ፓራሜትር `{name}` በ`{func}` ፋንክሽን ውስጥ ምንም ጊዜ አይጠቀምም",
    lint_unreachable_after_return: "በ{context} ውስጥ የማይደረስ መግለጫ ተገኘ፤ return በኋላ ያለው ኮድ በፍላጎት አይሰራም",
    lint_ok: "እሺ",
    lint_usage: "መጠቀም: koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "የKotodama lint ምርመራዎችን በተሰጡት ምንጮች ላይ ይከናውኑ.",
    ..english::MESSAGES
};
