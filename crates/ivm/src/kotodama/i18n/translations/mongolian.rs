use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "Компиляцлах функц алга",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "Тодорхойгүй параметр {name}",
    read_file: "{path} файлыг уншиж чадсангүй: {error}",
    parser_error: "Парсерын алдаа: {error}",
    semantic_error: "Семантик алдаа: {error}",
    lint_unused_state: "`{name}` төлөв зарлагдсан боловч хэзээ ч ашиглагдахгүй байна",
    lint_state_shadowed_param: "`{func}` функц доторх `{name}` параметр нь `{name}` төлөвийг халхалж байна; төлөвт хандахын тулд параметрийн нэрийг өөрчилнө үү",
    lint_state_shadowed_binding: "`{func}` функц доторх `{name}` холбоос нь `{name}` төлөвийг халхалж байна; хандалтыг хадгалахын тулд холбоосын нэрийг өөрчилнө үү",
    lint_state_shadowed_map_binding: "`{func}` функц map-ийг давтах үед `{name}` холбоос нь `{name}` төлөвийг халхалж байна",
    lint_unused_parameter: "`{func}` функц дотор `{name}` параметр огт ашиглагддаггүй",
    lint_unreachable_after_return: "{context} хэсэгт хүрэх боломжгүй мэдэгдлийг илрүүллээ: return-ийн дараах код хэзээ ч ажиллахгүй",
    lint_ok: "зөв",
    lint_usage: "Ашиглах арга: koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "Өгөгдсөн эх сурвалж дээр Kotodama-ийн lint шалгалтуудыг ажиллуулна.",
    ..english::MESSAGES
};
