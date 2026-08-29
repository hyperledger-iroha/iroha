//! Shared helper utilities for procedural macro crates.
//!
//! The diagnostic [`Emitter`] is always available. Enable the `repr` feature
//! only for macros that parse `#[repr(...)]` attributes.
mod emitter;
#[cfg(feature = "repr")]
pub mod repr;

pub use emitter::Emitter;

#[cfg(test)]
mod tests {
    use super::*;
    use manyhow::{Error as ManyhowError, error_message};
    #[test]
    fn emitter_finish_ok_when_no_errors() {
        let emitter = Emitter::new();
        assert!(emitter.finish().is_ok());
    }
    #[test]
    fn emitter_emit_and_finish_err() {
        let mut emitter = Emitter::new();
        emitter.emit(ManyhowError::from(error_message!("err")));
        assert!(emitter.finish().is_err());
    }
    #[test]
    fn emitter_handle_variants() {
        let mut emitter = Emitter::new();
        let val = emitter.handle::<manyhow::Error, _>(Ok(10));
        assert_eq!(val, Some(10));
        let val2: Option<i32> =
            emitter.handle::<ManyhowError, _>(Err(error_message!("oops").into()));
        assert!(val2.is_none());
        assert!(emitter.finish().is_err());
    }
    #[test]
    fn emitter_finish_token_stream() {
        let mut emitter = Emitter::new();
        emitter.emit(ManyhowError::from(error_message!("err")));
        let tokens = emitter.finish_token_stream();
        assert!(!tokens.is_empty());
    }
}
