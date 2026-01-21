use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "Không có hàm nào để biên dịch",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "Tham số không xác định {name}",
    read_file: "Không thể đọc tệp {path}: {error}",
    parser_error: "Lỗi bộ phân tích cú pháp: {error}",
    semantic_error: "Lỗi ngữ nghĩa: {error}",
    lint_unused_state: "trạng thái `{name}` đã được khai báo nhưng không bao giờ được sử dụng",
    lint_state_shadowed_param: "tham số `{name}` trong hàm `{func}` che khuất trạng thái `{name}`; hãy đổi tên tham số để truy cập trạng thái",
    lint_state_shadowed_binding: "ràng buộc `{name}` trong hàm `{func}` che khuất trạng thái `{name}`; hãy đổi tên ràng buộc để giữ quyền truy cập",
    lint_state_shadowed_map_binding: "ràng buộc `{name}` trong hàm `{func}` che khuất trạng thái `{name}` khi lặp qua map",
    lint_unused_parameter: "tham số `{name}` trong hàm `{func}` không bao giờ được sử dụng",
    lint_unreachable_after_return: "phát hiện câu lệnh không thể tới trong {context}: mã sau return sẽ không bao giờ chạy",
    lint_ok: "ok",
    lint_usage: "Cách dùng: koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "Chạy các kiểm tra lint Kotodama trên mã nguồn đã cung cấp.",
    ..english::MESSAGES
};
