use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "ކޮމްޕައިލް ކުރުމަށް ފަންކްޝަނެއް ނެތް",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "ނޭނގޭ ޕެރަމީޓަރ {name}",
    read_file: "{path} ކިޔާކުރުމަށް ނުކުރެވުނެވެ: {error}",
    parser_error: "ޕާސަރ އެރޯރ: {error}",
    semantic_error: "ސެމެންޓިކް އެރޯރ: {error}",
    lint_unused_state: "`{name}` ސްޓޭޓް އަދިކަމަށް ބޭނުންކޮށް ބަލައިފި، އެއްވެސް ނުބަލާ",
    lint_state_shadowed_param: "`{func}` މާނައަޅުގެ `{name}` ޕެރަމީޓަރު `{name}` ސްޓޭޓް ފަރުވާނެ؛ ސްޓޭޓް ބަލާންވީ ފަސް ޕެރަމީޓަރުގެ ނަން ބަލާލުން",
    lint_state_shadowed_binding: "`{func}` މާނައަޅުގެ `{name}` ބައިންޑިން `{name}` ސްޓޭޓް ފަރުވާނެ؛ ސްޓޭޓް ބަލާންވީ ފަސް ބައިންޑިންގެ ނަން ބަލާލުން",
    lint_state_shadowed_map_binding: "`{func}` މާނައަޅު މޭޕް ހޯދެއްޖެހޭގޮތަކަށް `{name}` ބައިންޑިން `{name}` ސްޓޭޓް ފަރުވާނެ",
    lint_unused_parameter: "`{func}` މާނައަޅުގެ `{name}` ޕެރަމީޓަރު ބައެއްވެސް ނުބަލާ",
    lint_unreachable_after_return: "{context} ގައި ދެކެވެސް ނުފެނުނު ސްޓޭޓްމަންޓެއް ލިބި؛ return ކުރުމުގެ ފަހަރު ކޯޑު ނުހަރައްދޭ",
    lint_ok: "ރަނގަޅު",
    lint_usage: "ޔޫސް: koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "ދެވެނި ފައިލުން ތަކެތި Kotodama lint ރަންކަމުގެ.",
    ..english::MESSAGES
};
