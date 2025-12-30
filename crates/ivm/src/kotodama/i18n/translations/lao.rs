use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "ບໍ່ມີຟັງຊັນໃຫ້ຄອມໄພ",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "ພາຣາມິເຕີທີ່ບໍ່ຮູ້ຈັກ {name}",
    read_file: "ບໍ່ສາມາດອ່ານໄຟລ໌ {path}: {error}",
    parser_error: "ເກີດຄວາມຜິດພາດຈາກ parser: {error}",
    semantic_error: "ເກີດຄວາມຜິດພາດດ້ານໃນເນື້ອຫາ: {error}",
    lint_unused_state: "state `{name}` ໄດ້ຖືກປະກາດແຕ່ບໍ່ເຄີຍນຳໃຊ້",
    lint_state_shadowed_param: "ພາຣາມິເຕີ `{name}` ໃນຟັງຊັນ `{func}` ບັງ state `{name}`; ກະລຸນາປ່ຽນຊື່ພາຣາມິເຕີເພື່ອເຂົ້າເຖິງ state",
    lint_state_shadowed_binding: "ການຜູກ `{name}` ໃນຟັງຊັນ `{func}` ບັງ state `{name}`; ກະລຸນາປ່ຽນຊື່ການຜູກເພື່ອຮັກສາການເຂົ້າເຖິງ",
    lint_state_shadowed_map_binding: "ການຜູກ `{name}` ໃນຟັງຊັນ `{func}` ບັງ state `{name}` ໃນຂະນະທີ່ວົງຈອນ map",
    lint_unused_parameter: "ພາຣາມິເຕີ `{name}` ໃນຟັງຊັນ `{func}` ບໍ່ເຄີຍນຳໃຊ້",
    lint_unreachable_after_return: "ພົບຄຳສັ່ງທີ່ບໍ່ສາມາດເຂົ້າເຖິງໄດ້ໃນ {context}: ໂຄດຫຼັງ return ຈະບໍ່ຖືກເຮັດວຽກ",
    lint_ok: "ສຳເລັດ",
    lint_usage: "ວິທີໃຊ້: koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "ເຮັດການກວດ lint ຂອງ Kotodama ກັບແຫຼ່ງທີ່ໃຫ້ມາ.",
    ..english::MESSAGES
};
