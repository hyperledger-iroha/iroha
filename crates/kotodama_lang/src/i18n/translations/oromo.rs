use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "Faankishiniin kómpaayiluuf hin jiru",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "Paaramitarri hin beekamne {name}",
    read_file: "Faayilii {path} dubbisuu hin dandeenye: {error}",
    parser_error: "Dogoggora parser: {error}",
    semantic_error: "Dogoggora hiikaa: {error}",
    lint_unused_state: "haalata `{name}` ni ibsame karaa tokko iyyuu hin fayyadamamu",
    lint_state_shadowed_param: "paaramitarri `{name}` faankishinii `{func}` keessatti haalata `{name}` dhoksa; haalata sana bira gahuf maqaa paaramitaraa jijjiiri",
    lint_state_shadowed_binding: "hidhamiinsi `{name}` faankishinii `{func}` keessatti haalata `{name}` dhoksa; hidhamiinsa maqaa jijjiiri akka seeraan seenu dandeessu",
    lint_state_shadowed_map_binding: "yeroo map irra deebi'aa jirutti hidhamiinsi `{name}` faankishinii `{func}` keessatti haalata `{name}` dhoksa",
    lint_unused_parameter: "paaramitarri `{name}` faankishinii `{func}` keessatti yeroo hunda hin fayyadamamu",
    lint_unreachable_after_return: "ibsa gara hin geenye {context} keessatti argame; koodiin return boodaa hin hojjetu",
    lint_ok: "tole",
    lint_usage: "Fayyadama: koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "Bu'uuraalee kennaman irratti sakatta'iinsa lint Kotodama hojjechiisu.",
    ..english::MESSAGES
};
