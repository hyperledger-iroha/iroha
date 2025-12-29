//! Integration tests for logger setup routines.
//!
//! Ensures that setting the global logger twice fails gracefully and that
//! installing the panic hook multiple times is idempotent.

use iroha_logger::{Config, init_global};

#[tokio::test]
async fn setting_logger_twice_fails() {
    let cfg = Config {
        terminal_colors: false,
        ..Config::default()
    };

    let first = init_global(cfg.clone());
    assert!(first.is_ok());

    let second = init_global(cfg);
    assert!(second.is_err());
}

#[test]
fn install_panic_hook_multiple_times_works() {
    iroha_logger::install_panic_hook().unwrap();
    iroha_logger::install_panic_hook().unwrap();
}
