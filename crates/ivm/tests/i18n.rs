use std::sync::Mutex;

use ivm::kotodama::{
    compiler::Compiler,
    i18n::{self, Language, Message},
};

static LANG_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn detect_language_default() {
    use std::env;
    let _g = LANG_LOCK.lock().unwrap();
    let old_lang = env::var("LANG").ok();
    let old_lc_all = env::var("LC_ALL").ok();
    let old_lc_messages = env::var("LC_MESSAGES").ok();
    unsafe {
        env::remove_var("LANG");
        env::remove_var("LC_ALL");
        env::remove_var("LC_MESSAGES");
    }
    assert_eq!(i18n::detect_language(), Language::English);
    unsafe {
        if let Some(v) = old_lang {
            env::set_var("LANG", v);
        } else {
            env::remove_var("LANG");
        }
        if let Some(v) = old_lc_all {
            env::set_var("LC_ALL", v);
        } else {
            env::remove_var("LC_ALL");
        }
        if let Some(v) = old_lc_messages {
            env::set_var("LC_MESSAGES", v);
        } else {
            env::remove_var("LC_MESSAGES");
        }
    }
}

#[test]
fn translate_unknown_param() {
    let msg = i18n::translate(Language::French, Message::UnknownParam("x"));
    assert_eq!(msg, "Param inconnu x");
}

#[test]
fn override_language_in_compiler() {
    // Intentionally reference an unknown parameter to force a compiler error
    // and verify the error is emitted in the overridden language (Japanese).
    let src = "fn f(a){ let c = b + 1; }";
    let c = Compiler::new_with_language(Language::Japanese);
    let err = c.compile_source(src).unwrap_err();
    // Expect the Japanese prefix for semantic errors
    assert!(
        err.contains("意味解析エラー"),
        "unexpected error message: {err}"
    );
}

#[test]
fn translate_parser_error() {
    let msg = i18n::translate(Language::Spanish, Message::ParserError("oops"));
    assert_eq!(msg, "Error del analizador: oops");
}

#[test]
fn detect_language_portuguese() {
    use std::env;
    let _g = LANG_LOCK.lock().unwrap();
    let old_lang = env::var("LANG").ok();
    let old_lc_all = env::var("LC_ALL").ok();
    let old_lc_messages = env::var("LC_MESSAGES").ok();
    unsafe {
        env::remove_var("LC_ALL");
        env::remove_var("LC_MESSAGES");
        env::set_var("LANG", "pt_BR.UTF-8");
    }
    assert_eq!(i18n::detect_language(), Language::Portuguese);
    unsafe {
        if let Some(v) = old_lang {
            env::set_var("LANG", v);
        } else {
            env::remove_var("LANG");
        }
        if let Some(v) = old_lc_all {
            env::set_var("LC_ALL", v);
        } else {
            env::remove_var("LC_ALL");
        }
        if let Some(v) = old_lc_messages {
            env::set_var("LC_MESSAGES", v);
        } else {
            env::remove_var("LC_MESSAGES");
        }
    }
}

#[test]
fn translate_parser_error_portuguese() {
    let msg = i18n::translate(Language::Portuguese, Message::ParserError("oops"));
    assert_eq!(msg, "Erro do analisador: oops");
}

#[test]
fn rtl_placeholders_use_direction_isolates() {
    let arabic = i18n::translate(Language::Arabic, Message::UnknownParam("X"));
    let hebrew = i18n::translate(Language::Hebrew, Message::ParserError("oops"));
    assert!(
        arabic.contains("\u{2068}X\u{2069}"),
        "expected Arabic translation to isolate placeholder: {arabic}"
    );
    assert!(
        hebrew.contains("\u{2068}oops\u{2069}"),
        "expected Hebrew translation to isolate placeholder: {hebrew}"
    );
}
