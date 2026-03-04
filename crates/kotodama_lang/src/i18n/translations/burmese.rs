use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "ကွန်ပိုင်လုပ်ရန် ဖန်ရှင် မရှိပါ။",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "မသိသော ပါရာမီတာ {name}",
    read_file: "ဖိုင် {path} ကို ဖတ်ရန် မအောင်မြင်ပါ: {error}",
    parser_error: "Parser အမှား: {error}",
    semantic_error: "အဓိပ္ပါယ်ဆိုင်ရာ အမှား: {error}",
    lint_unused_state: "state `{name}` ကို ကြေညာခဲ့သော်လည်း အသုံးမပြုပါ",
    lint_state_shadowed_param: "ဖန်ရှင် `{func}` တွင် ပါရာမီတာ `{name}` သည် state `{name}` ကို ဝှက်ထားသည်။ state ကို အသုံးပြုနိုင်ရန် ပါရာမီတာအမည်ကို ပြင်ပါ",
    lint_state_shadowed_binding: "ဖန်ရှင် `{func}` တွင် binding `{name}` သည် state `{name}` ကို ဝှက်ထားသည်။ state ကို ဆက်လက်အသုံးပြုနိုင်ရန် binding အမည်ကို ပြင်ပါ",
    lint_state_shadowed_map_binding: "ဖန်ရှင် `{func}` တွင် map ကို လှည့်ကွက်ရာတွင် binding `{name}` သည် state `{name}` ကို ဝှက်ထားသည်",
    lint_unused_parameter: "ဖန်ရှင် `{func}` တွင် ပါရာမီတာ `{name}` ကို အသုံးမပြုပါ",
    lint_unreachable_after_return: "{context} တွင် မရောက်နိုင်သော ကြေညာချက် ရှိသည်ဟုတွေ့တယ်៖ return ပို့ပြီးနောက်ရှိ ကုဒ်ကို မစမ်းကြည့်ပါ",
    lint_ok: "အောင်မြင်",
    lint_usage: "အသုံးပြုပုံ: koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "ပေးထားသော ရင်းမြစ်များအပေါ် Kotodama lint စစ်ဆေးမှုများကို ပြေးဆင်ပါ။",
    ..english::MESSAGES
};
