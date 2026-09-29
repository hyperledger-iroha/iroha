//! Native epoch roots use the shared signed-genesis verifier.

pub(crate) use iroha_data_model::sumeragi_finality::genesis_epoch;

/// Committed NPoS schedule and evidence-delay parameters.
pub(crate) mod parameters;

#[cfg(test)]
pub(crate) mod tests;
