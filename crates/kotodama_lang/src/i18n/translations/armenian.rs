use super::{super::Messages, english};

pub const MESSAGES: Messages = Messages {
    no_functions: "Կոմպիլյացիայի համար ֆունկցիաներ չկան",
    unsupported_binary_op: "Kotodama compiler hint: {op}",
    unknown_param: "Անհայտ պարամետր {name}",
    read_file: "Չհաջողվեց կարդալ {path} ֆայլը. {error}",
    parser_error: "Վերծանիչի սխալ: {error}",
    semantic_error: "Սեմանտիկ սխալ: {error}",
    lint_unused_state: "`{name}` վիճակը հայտարարված է, բայց երբեք չի օգտագործվում",
    lint_state_shadowed_param: "`{name}` պարամետրը `{func}` ֆունկցիայում փակում է `{name}` վիճակը. փոխեք պարամետրի անունը՝ վիճակին հասանելի լինելու համար",
    lint_state_shadowed_binding: "`{name}` կապը `{func}` ֆունկցիայում փակում է `{name}` վիճակը. փոխեք կապի անունը՝ հասանելիությունը պահպանելու համար",
    lint_state_shadowed_map_binding: "`{func}` ֆունկցիայում քարտեզի կրկնության ժամանակ `{name}` կապը փակում է `{name}` վիճակը",
    lint_unused_parameter: "`{func}` ֆունկցիայում `{name}` պարամետրը երբեք չի օգտագործվում",
    lint_unreachable_after_return: "{context}`-ում հայտնաբերվեց անմատչելի արտահայտություն. return-ից հետո կոդը երբեք չի կատարվում",
    lint_ok: "լավ",
    lint_usage: "Օգտագործում՝ koto_lint <file.ko> [<file2.ko> ...]",
    lint_usage_help: "Գործարկում է Kotodama-ի lint ստուգումները տրամադրված աղբյուրների վրա։",
    ..english::MESSAGES
};
