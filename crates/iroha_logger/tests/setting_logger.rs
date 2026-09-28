//! Integration tests for logger setup routines.
//!
//! Ensures that setting the global logger twice fails gracefully.
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
