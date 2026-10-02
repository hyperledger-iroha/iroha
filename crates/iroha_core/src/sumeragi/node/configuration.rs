//! One signed-genesis native consensus-configuration fingerprint.
//!
//! This commits native protocol, complete initial epoch authority and explicit signed chain
//! parameters. It does not claim to fingerprint local queues, resource budgets or transport.

use super::super::{epoch::genesis_epoch, schedule::ChainParamsRecord};
use iroha_crypto::Hash;
use iroha_data_model::{
    block::SignedBlock,
    isi::SetParameter,
    parameter::{
        Parameter,
        system::{SumeragiParameter, SumeragiParameters},
    },
    transaction::Executable,
};

/// Authenticate signed genesis and fingerprint the exact native consensus configuration.
///
/// # Errors
/// Rejects invalid signed genesis, omitted/duplicated explicit chain parameters or invalid
/// native parameter geometry. No retired adapter configuration or implicit fallback is used.
pub fn consensus_configuration_fingerprint(
    genesis: &SignedBlock,
) -> Result<Hash, crate::execution_attempt::ExecutionAttemptError<String>> {
    let epoch = genesis_epoch(genesis).map_err(|error| {
        crate::execution_attempt::genesis_read_attempt_error(error, |error| error.to_string())
    })?;
    let metadata = iroha_data_model::sumeragi_finality::signed_genesis_consensus_metadata(genesis)
        .map_err(|error| {
            crate::execution_attempt::genesis_read_attempt_error(error, |error| error.to_string())
        })?;
    let mut parameters = ExplicitParameters::new(metadata.block_cadence_ms);
    for transaction in genesis.external_transactions() {
        let Executable::Instructions(instructions) = transaction.instructions() else {
            return Err("native configuration requires explicit signed instructions".into());
        };
        for instruction in instructions {
            if let Some(set) = instruction.as_any().downcast_ref::<SetParameter>() {
                if let Parameter::Sumeragi(parameter) = set.inner() {
                    parameters.insert(*parameter)?;
                }
            }
        }
    }
    let parameters = parameters.finish()?;
    let encoded = norito::encode_canonical(&(
        iroha_data_model::sumeragi::PROTOCOL_VERSION,
        epoch,
        parameters,
    ))
    .map_err(|error| error.to_string())?;
    Ok(Hash::new_from_chunks(&[
        b"iroha:native-config:v1",
        &encoded,
    ]))
}

struct ExplicitParameters {
    value: SumeragiParameters,
    seen: u8,
}
impl ExplicitParameters {
    fn new(cadence: std::num::NonZeroU64) -> Self {
        Self {
            value: SumeragiParameters {
                block_cadence_ms: cadence,
                ..SumeragiParameters::default()
            },
            seen: 0,
        }
    }
    fn insert(&mut self, parameter: SumeragiParameter) -> Result<(), String> {
        let bit = match parameter {
            SumeragiParameter::PayloadRetryIntervalMs(value) => {
                self.value.payload_retry_interval_ms = value;
                1
            }
            SumeragiParameter::ExecBudgetMs(value) => {
                self.value.exec_budget_ms = value;
                2
            }
            SumeragiParameter::ApplyBudgetMs(value) => {
                self.value.apply_budget_ms = value;
                4
            }
            SumeragiParameter::MaxBlockBytes(value) => {
                self.value.max_block_bytes = value;
                8
            }
            SumeragiParameter::EpochLengthBlocks(value) => {
                self.value.epoch_length_blocks = value;
                16
            }
            SumeragiParameter::MaxClockDriftMs(value) => {
                self.value.max_clock_drift_ms = value;
                32
            }
            SumeragiParameter::DemotionWindow(value) => {
                self.value.demotion_window = value;
                64
            }
        };
        if self.seen & bit != 0 {
            return Err("signed genesis repeats a native Sumeragi parameter".into());
        }
        self.seen |= bit;
        Ok(())
    }
    fn finish(self) -> Result<ChainParamsRecord, String> {
        if self.seen != 0x7f {
            return Err("signed genesis omits an explicit native Sumeragi parameter".into());
        }
        let parameters = ChainParamsRecord::from_parameters(&self.value);
        parameters.validate().map_err(|error| error.to_string())?;
        Ok(parameters)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn entries() -> Vec<SumeragiParameter> {
        let p = SumeragiParameters::default();
        vec![
            SumeragiParameter::PayloadRetryIntervalMs(p.payload_retry_interval_ms),
            SumeragiParameter::ExecBudgetMs(p.exec_budget_ms),
            SumeragiParameter::ApplyBudgetMs(p.apply_budget_ms),
            SumeragiParameter::MaxBlockBytes(p.max_block_bytes),
            SumeragiParameter::EpochLengthBlocks(p.epoch_length_blocks),
            SumeragiParameter::MaxClockDriftMs(p.max_clock_drift_ms),
            SumeragiParameter::DemotionWindow(p.demotion_window),
        ]
    }
    #[test]
    fn explicit_parameters_reject_every_missing_and_repeated_signed_field() {
        let values = entries();
        for omitted in 0..values.len() {
            let mut p = ExplicitParameters::new(SumeragiParameters::default().block_cadence_ms);
            for (index, value) in values.iter().enumerate() {
                if index != omitted {
                    p.insert(*value).unwrap();
                }
            }
            assert!(p.finish().is_err());
            let mut p = ExplicitParameters::new(SumeragiParameters::default().block_cadence_ms);
            for value in &values {
                p.insert(*value).unwrap();
            }
            assert!(p.insert(values[omitted]).is_err());
        }
        let mut p = ExplicitParameters::new(SumeragiParameters::default().block_cadence_ms);
        for value in values {
            p.insert(value).unwrap();
        }
        assert_eq!(
            p.finish().unwrap(),
            ChainParamsRecord::from_parameters(&SumeragiParameters::default())
        );
    }
    #[test]
    fn actual_signed_genesis_fingerprint_changes_with_source_and_rejects_tampering() {
        use crate::sumeragi::test_chain::{CertifiedTestChain, TestChainConfig};
        let mut a =
            CertifiedTestChain::start(TestChainConfig::new(crate::state::World::new(), 1_000))
                .unwrap();
        let b = CertifiedTestChain::start(TestChainConfig::new(crate::state::World::new(), 2_000))
            .unwrap();
        let genesis = a.committed(1);
        assert_ne!(
            consensus_configuration_fingerprint(genesis.block()).unwrap(),
            consensus_configuration_fingerprint(b.committed(1).block()).unwrap()
        );
        a.commit(Vec::new());
        assert!(consensus_configuration_fingerprint(a.committed(2).block()).is_err());
        let mut tampered = genesis.block().as_ref().clone();
        let foreign =
            iroha_crypto::KeyPair::from_seed(vec![0xC9; 32], iroha_crypto::Algorithm::Ed25519);
        let signature = iroha_data_model::block::BlockSignature::new(
            0,
            iroha_crypto::SignatureOf::new(foreign.private_key(), &tampered.header()),
        );
        tampered
            .replace_signatures(std::collections::BTreeSet::from([signature]))
            .unwrap();
        assert!(consensus_configuration_fingerprint(&tampered).is_err());
    }
}
