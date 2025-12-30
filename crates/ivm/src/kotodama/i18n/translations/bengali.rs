use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "সংকলনের কোনও ফাংশন নেই",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "অজানা পরম {name}",
    read_file: "পড়তে ব্যর্থ {path}: {error}",
    parser_error: "পার্সার ত্রুটি: {error}",
    semantic_error: "শব্দার্থ ত্রুটি: {error}",
    lint_unused_state: "স্টেট `{name}` ঘোষণা করা হয়েছে কিন্তু কখনও ব্যবহার করা হয়নি",
    lint_state_shadowed_param: "ফাংশন `{func}`-এ প্যারামিটার `{name}` স্টেট `{name}`-কে আড়াল করছে; স্টেটে পৌঁছাতে প্যারামিটারের নাম পরিবর্তন করুন",
    lint_state_shadowed_binding: "ফাংশন `{func}`-এ বাইন্ডিং `{name}` স্টেট `{name}`-কে আড়াল করছে; প্রবেশাধিকার বজায় রাখতে বাইন্ডিংয়ের নাম পরিবর্তন করুন",
    lint_state_shadowed_map_binding: "ফাংশন `{func}`-এ ম্যাপ ইটারেশনের সময় বাইন্ডিং `{name}` স্টেট `{name}`-কে আড়াল করছে",
    lint_unused_parameter: "ফাংশন `{func}`-এ প্যারামিটার `{name}` কখনও ব্যবহার করা হয় না",
    lint_unreachable_after_return: "{context}`-এ অপ্রাপ্য স্টেটমেন্ট পাওয়া গেছে: return-এর পরে থাকা কোড কখনও চালানো হয় না",
    lint_ok: "ঠিক আছে",
    lint_usage: "ব্যবহার: koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "প্রদত্ত সোর্সের উপর Kotodama এর lint পরীক্ষা চালান।",
    ..english::MESSAGES
};
