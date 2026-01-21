use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "ไม่มีฟังก์ชันให้คอมไพล์",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "ไม่รู้จักพารามิเตอร์ {name}",
    read_file: "ไม่สามารถอ่านไฟล์ {path}: {error}",
    parser_error: "เกิดข้อผิดพลาดจากตัวแยกโค้ด: {error}",
    semantic_error: "เกิดข้อผิดพลาดด้านความหมาย: {error}",
    lint_unused_state: "state `{name}` ถูกประกาศแต่ไม่เคยถูกใช้งาน",
    lint_state_shadowed_param: "พารามิเตอร์ `{name}` ในฟังก์ชัน `{func}` บดบัง state `{name}`; เปลี่ยนชื่อพารามิเตอร์เพื่อเข้าถึง state",
    lint_state_shadowed_binding: "ตัวผูก `{name}` ในฟังก์ชัน `{func}` บดบัง state `{name}`; เปลี่ยนชื่อตัวผูกเพื่อรักษาการเข้าถึง state",
    lint_state_shadowed_map_binding: "ตัวผูก `{name}` ในฟังก์ชัน `{func}` บดบัง state `{name}` ระหว่างการวนซ้ำ map",
    lint_unused_parameter: "พารามิเตอร์ `{name}` ในฟังก์ชัน `{func}` ไม่เคยถูกใช้งาน",
    lint_unreachable_after_return: "ตรวจพบคำสั่งที่เข้าถึงไม่ได้ใน {context}: โค้ดหลัง return จะไม่ถูกรัน",
    lint_ok: "สำเร็จ",
    lint_usage: "วิธีใช้: koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "รันการตรวจ lint ของ Kotodama กับซอร์สที่ระบุ.",
    ..english::MESSAGES
};
