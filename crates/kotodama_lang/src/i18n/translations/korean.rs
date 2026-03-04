use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "컴파일할 함수가 없습니다",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "알 수 없는 매개변수 {name}",
    read_file: "파일 {path}을(를) 읽을 수 없습니다: {error}",
    parser_error: "파서 오류: {error}",
    semantic_error: "시맨틱 오류: {error}",
    lint_unused_state: "state `{name}`이 선언되었지만 사용되지 않습니다",
    lint_state_shadowed_param: "함수 `{func}`의 매개변수 `{name}`이 상태 `{name}`을 가립니다. 상태에 접근하려면 매개변수 이름을 변경하세요",
    lint_state_shadowed_binding: "함수 `{func}`의 바인딩 `{name}`이 상태 `{name}`을 가립니다. 상태 접근을 유지하려면 바인딩 이름을 변경하세요",
    lint_state_shadowed_map_binding: "함수 `{func}`에서 맵을 순회할 때 바인딩 `{name}`이 상태 `{name}`을 가립니다",
    lint_unused_parameter: "함수 `{func}`의 매개변수 `{name}`은 사용되지 않습니다",
    lint_unreachable_after_return: "{context}에서 도달할 수 없는 문장을 발견했습니다: return 이후의 코드는 실행되지 않습니다",
    lint_ok: "정상",
    lint_usage: "사용법: koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "제공된 소스에 Kotodama lint를 실행합니다.",
    ..english::MESSAGES
};
