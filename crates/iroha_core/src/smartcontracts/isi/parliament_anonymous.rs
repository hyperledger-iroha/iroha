//! Credential registration of the anonymous Parliament family, the
//! `iroha.governance.parliament.anonymous.v1` addition (design §3.2, §12.3 slice 2).
//!
//! [`RegisterParliamentCredentialV1`] executes only as the sole instruction of a direct signed
//! input under the network's candidate-testing selection. The handler takes the input's
//! one-use original, which locates the proof the credential record names, and lets the
//! attempt's reducer accept the authority's binding seat with its verified credential. The
//! reducer rebuilds the election's registration ledger from the credentials World keeps, so
//! seat and commitment freshness is checked before the proof is verified. The attempt and the
//! new credential are written together. A refusal the node cannot decide defers the input
//! instead of rejecting it: an unavailable verifier, or stored credentials that are not the
//! election's, which is World's corruption and never the registrant's fault.

use iroha_data_model::{
    account::AccountId, governance::anonymous_ballot::RegisterParliamentCredentialV1,
};
use ivm::error::ExecutionDeferral;
use mv::storage::StorageReadOnly;

use crate::{
    governance::parliament_anonymous::AnonymousParliamentErrorV1,
    smartcontracts::{Execute, isi::Error},
    state::{StateReadOnly, StateTransaction, parliament_anonymous_tables::election_credentials},
};

fn refused(reason: impl core::fmt::Display) -> Error {
    Error::InvariantViolation(reason.to_string().into())
}

/// The rejection of a refused registration, or a deferral recorded in `state` for a refusal the
/// node cannot decide.
pub(crate) fn registration_refusal(
    error: &AnonymousParliamentErrorV1,
    state: &mut StateTransaction<'_, '_>,
) -> Error {
    let deferral = if error.is_local() {
        Some(ExecutionDeferral::VerifierArtifactsUnavailable)
    } else if matches!(error, AnonymousParliamentErrorV1::CredentialStore) {
        Some(ExecutionDeferral::LocalInvariantViolation)
    } else {
        None
    };
    match deferral {
        Some(reason) => {
            let _ = state.defer_execution(reason);
            refused("local credential registration did not complete")
        }
        None => refused(format!("credential registration refused: {error}")),
    }
}

impl Execute for RegisterParliamentCredentialV1 {
    fn execute(
        self,
        authority: &AccountId,
        state: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let source = match state.consume_direct_parliament_registration() {
            Ok(source) => source,
            Err(error) => {
                return Err(match error.into_deferral() {
                    Ok(reason) => {
                        let _ = state.defer_execution(reason);
                        refused("local credential registration did not complete")
                    }
                    Err(error) => refused(error),
                });
            }
        };
        let now_ms = u64::try_from(state._curr_block.creation_time().as_millis())
            .map_err(|_| refused("the block time exceeds the Parliament time domain"))?;
        let network_id = *state.network_id();
        let mut attempt = state
            .world
            .parliament_anonymous_attempts
            .get(&self.governance_attempt_id)
            .cloned()
            .ok_or_else(|| refused("unknown anonymous Parliament attempt"))?;
        // An election the attempt does not hold has no credentials; the reducer refuses it.
        let records = attempt
            .accepted_assignments(self.election_attempt_id)
            .map(|accepted| {
                election_credentials(&state.world.parliament_anonymous_registrations, accepted)
            })
            .unwrap_or_default();
        match attempt.register_credential(&network_id, authority, &self, &records, source, now_ms) {
            Ok((assignment, record)) => {
                state
                    .world
                    .parliament_anonymous_attempts
                    .insert(self.governance_attempt_id, attempt);
                state
                    .world
                    .parliament_anonymous_registrations
                    .insert(assignment, record);
                Ok(())
            }
            Err(error) => Err(registration_refusal(&error, state)),
        }
    }
}
