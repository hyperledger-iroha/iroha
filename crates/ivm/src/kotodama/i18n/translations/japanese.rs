use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "コンパイルする関数がありません",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "不明な引数 {name}",
    read_file: "ファイル {path} の読み取りに失敗しました: {error}",
    parser_error: "構文解析エラー: {error}",
    semantic_error: "意味解析エラー: {error}",
    lint_unused_state: "state `{name}` は宣言されていますが使用されていません",
    lint_state_shadowed_param: "関数`{func}`の引数`{name}`が state `{name}` を隠しています。state にアクセスするには引数名を変更してください",
    lint_state_shadowed_binding: "関数`{func}`の束縛`{name}`が state `{name}` を隠しています。state を利用できるように束縛名を変更してください",
    lint_state_shadowed_map_binding: "関数`{func}`のループ内束縛`{name}`が state `{name}` を隠しています (map の反復中)",
    lint_unused_parameter: "関数`{func}`の引数`{name}`は使用されていません",
    lint_unreachable_after_return: "{context} で到達不能な文を検出しました: return の後のコードは実行されません",
    lint_ok: "正常",
    lint_usage: "使用方法: koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "指定したソースに Kotodama の lint を実行します。",
    ..english::MESSAGES
};
