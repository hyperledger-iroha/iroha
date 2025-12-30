use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "Δεν υπάρχουν συναρτήσεις για μεταγλώττιση",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "Άγνωστη παράμετρος {name}",
    read_file: "Αποτυχία στην ανάγνωση του αρχείου {path}: {error}",
    parser_error: "Σφάλμα αναλυτή: {error}",
    semantic_error: "Σημασιολογικό σφάλμα: {error}",
    lint_unused_state: "Η κατάσταση `{name}` δηλώθηκε αλλά δεν χρησιμοποιείται ποτέ",
    lint_state_shadowed_param: "Η παράμετρος `{name}` στη συνάρτηση `{func}` καλύπτει την κατάσταση `{name}`· μετονομάστε την παράμετρο για πρόσβαση στην κατάσταση",
    lint_state_shadowed_binding: "Η δέσμευση `{name}` στη συνάρτηση `{func}` καλύπτει την κατάσταση `{name}`· μετονομάστε τη δέσμευση για να διατηρήσετε την πρόσβαση",
    lint_state_shadowed_map_binding: "Η δέσμευση `{name}` στη συνάρτηση `{func}` καλύπτει την κατάσταση `{name}` κατά την επανάληψη του map",
    lint_unused_parameter: "Η παράμετρος `{name}` στη συνάρτηση `{func}` δεν χρησιμοποιείται ποτέ",
    lint_unreachable_after_return: "Εντοπίστηκε μη προσπελάσιμη εντολή στο {context}: ο κώδικας μετά το return δεν εκτελείται ποτέ",
    lint_ok: "εντάξει",
    lint_usage: "Χρήση: koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "Εκτελεί τους ελέγχους lint του Kotodama στα δοσμένα αρχεία πηγής.",
    ..english::MESSAGES
};
