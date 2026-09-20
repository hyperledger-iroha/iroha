//! Private developer wallet custody and native, recoverable account operations.
//!
//! Wallets retain exact network identities and native client configurations outside projects.
//! Signing material is never part of public wallet information or operation receipts.

mod custody;
mod custody_fs;
mod operation_journal;

/// Shared bounded native faucet proof-of-work implementation.
pub mod faucet_pow;
/// Exact paid namespace requests derived from native public policy.
pub mod namespace;
/// Ordinary sponsored account admission and faucet operations.
pub mod onboarding;
/// Native balance reads and exact fee-paying transfer/alias operations.
pub mod operations;

pub use custody::{WalletInfo, WalletNetwork, WalletStore, default_wallet_dir};
