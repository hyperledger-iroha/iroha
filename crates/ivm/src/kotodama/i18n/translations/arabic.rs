use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "لا توجد وظائف لتجميعها",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "معلمة غير معروفة {name}",
    read_file: "فشل في القراءة {path}: {error}",
    parser_error: "خطأ في المحلل: {error}",
    semantic_error: "خطأ دلالي: {error}",
    lint_unused_state: "الحالة `{name}` مُعلنة ولكن لم يتم استخدامها",
    lint_state_shadowed_param: "المعامل `{name}` في الدالة `{func}` يحجب الحالة `{name}`؛ أعد تسمية المعامل للوصول إلى الحالة",
    lint_state_shadowed_binding: "الربط `{name}` في الدالة `{func}` يحجب الحالة `{name}`؛ أعد تسمية الربط للحفاظ على الوصول إلى الحالة",
    lint_state_shadowed_map_binding: "الربط `{name}` في الدالة `{func}` يحجب الحالة `{name}` أثناء اجتياز الخريطة",
    lint_unused_parameter: "المعامل `{name}` في الدالة `{func}` غير مستخدم إطلاقًا",
    lint_unreachable_after_return: "تم اكتشاف تعليمة غير قابلة للوصول في {context}: الشفرة بعد return لا تُنفَّذ أبدًا",
    lint_ok: "تم",
    lint_usage: "كيفية الاستخدام: koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "يشغّل فحوصات Kotodama على المصادر المقدَّمة.",
    ..english::MESSAGES
};
