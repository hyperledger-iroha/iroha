//! Re-export Kotodama language support from the dedicated `kotodama_lang` crate.
pub use kotodama_lang::*;

/// Runtime helpers live in the VM crate.
pub mod std {
    pub use crate::kotodama_std::*;
}
