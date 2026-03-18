use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "אין פונקציות לקומפילציה",
    unsupported_binary_op: "רמז מהמהדר של Kotodama: {op}",
    unknown_param: "פרמטר לא מוכר `{name}`",
    read_file: "קריאת הקובץ {path} נכשלה: {error}",
    parser_error: "שגיאת מנתח: {error}",
    semantic_error: "שגיאה סמנטית: {error}",
    lint_unused_state: "המצב `{name}` הוגדר אך אינו בשימוש",
    lint_state_shadowed_param: "הפרמטר `{name}` בפונקציה `{func}` מסתיר את המצב `{name}`; יש לשנות את שם הפרמטר כדי לגשת למצב",
    lint_state_shadowed_binding: "הקישור `{name}` בפונקציה `{func}` מסתיר את המצב `{name}`; יש לשנות את שם הקישור כדי לשמור על הגישה",
    lint_state_shadowed_map_binding: "הקישור `{name}` בפונקציה `{func}` מסתיר את המצב `{name}` בזמן איטרציה על המפה",
    lint_unused_parameter: "הפרמטר `{name}` בפונקציה `{func}` אינו בשימוש",
    lint_unreachable_after_return: "נמצאה פקודה שאינה נגישה ב-{context}: קוד אחרי return לעולם לא יורץ",
    lint_ok: "תקין",
    lint_usage: "שימוש: koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "הפעילו את בדיקות ה-lint של Kotodama על המקורות שסופקו.",
    ..english::MESSAGES
};
