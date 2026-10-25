//! Native epoch roots use the shared signed-genesis verifier.

pub use iroha_data_model::sumeragi_finality::authenticated_genesis;

/// Committed NPoS evidence-delay parameters.
pub mod parameters;

#[cfg(test)]
pub mod tests;
