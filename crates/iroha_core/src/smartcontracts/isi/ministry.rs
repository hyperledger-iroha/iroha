//! Ministry agenda submission handlers.

use iroha_data_model::isi::error::{InstructionExecutionError as Error, InvalidParameterError};
use iroha_telemetry::metrics;

use super::prelude::*;

/// Execution handlers for Ministry ISIs.
pub mod isi {
    use super::*;
    use crate::state::StateTransaction;

    impl Execute for iroha_data_model::isi::ministry::SubmitAgendaProposal {
        #[metrics(+"submit_agenda_proposal")]
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let proposal = self.proposal;
            proposal.validate().map_err(|err| {
                Error::InvalidParameter(InvalidParameterError::SmartContract(
                    err.to_string().into(),
                ))
            })?;

            let proposal_id = proposal.proposal_id.clone();
            if state_transaction
                .world
                .ministry_agenda_proposals
                .get(&proposal_id)
                .is_some()
            {
                return Err(Error::InvariantViolation(
                    format!("Ministry agenda proposal {proposal_id} already exists").into(),
                ));
            }

            let submitted_tx_hash_hex = state_transaction
                .current_tx_hash
                .as_ref()
                .map(|hash| hex::encode(hash.as_ref()))
                .ok_or_else(|| {
                    Error::InvariantViolation(
                        "current signed transaction hash unavailable for ministry submission"
                            .into(),
                    )
                })?;

            state_transaction.world.ministry_agenda_proposals.insert(
                proposal_id,
                iroha_data_model::ministry::AgendaProposalRecordV1 {
                    proposal,
                    authority: authority.clone(),
                    submitted_tx_hash_hex,
                    submitted_height: state_transaction.block_height(),
                },
            );

            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
    use iroha_data_model::{
        block::BlockHeader,
        isi::{error::InstructionExecutionError, ministry::SubmitAgendaProposal},
        ministry::{
            AGENDA_PROPOSAL_VERSION_V1, AgendaEvidenceAttachment, AgendaEvidenceKind,
            AgendaProposalAction, AgendaProposalSubmitter, AgendaProposalSummary,
            AgendaProposalTarget, AgendaProposalV1,
        },
        prelude::AccountId,
        transaction::SignedTransaction,
    };
    use nonzero_ext::nonzero;

    use super::*;
    use crate::{
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, World},
    };

    fn sample_proposal(proposal_id: &str) -> AgendaProposalV1 {
        AgendaProposalV1 {
            version: AGENDA_PROPOSAL_VERSION_V1,
            proposal_id: proposal_id.into(),
            submitted_at_unix_ms: 1_780_000_000_000,
            language: "en".into(),
            action: AgendaProposalAction::AddToDenylist,
            summary: AgendaProposalSummary {
                title: "Add abusive hash family".into(),
                motivation: "Multiple citizen reports reference the same digest.".into(),
                expected_impact: "Blocks known harmful payloads from distribution.".into(),
            },
            tags: vec!["csam".into()],
            targets: vec![AgendaProposalTarget {
                label: "Sample entry".into(),
                hash_family: "blake3-256".into(),
                hash_hex: "0d714bed4b7c63c23a2cf8ee9ce6c3cde1007907c427b4a0754e8ad31c91338d".into(),
                reason: "Flagged by moderators".into(),
            }],
            evidence: vec![AgendaEvidenceAttachment {
                kind: AgendaEvidenceKind::SorafsCid,
                uri: "sorafs://bafybei.../artifact.car".into(),
                digest_blake3_hex: Some(
                    "f1c02fb3bb194a9add242c3dfdf0bb2d94f9f3e1cf11f4a7d79d4012e4d0c2ad".into(),
                ),
                description: Some("Encrypted artifact bundle".into()),
            }],
            submitter: AgendaProposalSubmitter {
                name: "Citizen 42".into(),
                contact: "citizen42@example.org".into(),
                organization: Some("Wonderland Watch".into()),
                pgp_fingerprint: None,
            },
            duplicates: vec!["AC-2025-014".into()],
        }
    }

    fn tx_hash(seed: [u8; 32]) -> HashOf<SignedTransaction> {
        HashOf::from_untyped_unchecked(Hash::prehashed(seed))
    }

    fn checked_account_id() -> AccountId {
        let key_pair =
            KeyPair::try_random().expect("ministry fixture key generation should succeed");
        AccountId::new(key_pair.public_key().clone())
    }

    #[test]
    fn checked_account_id_preserves_default_algorithm() {
        let account_id = checked_account_id();
        assert_eq!(account_id.signatory().algorithm(), Algorithm::default());
    }

    #[test]
    fn submit_agenda_proposal_persists_submission_record() {
        let authority = checked_account_id();
        let proposal = sample_proposal("AC-2026-001");
        let expected_tx_hash = tx_hash([0xAB; 32]);

        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(World::default(), kura, query);

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        tx.current_tx_hash = Some(expected_tx_hash);

        SubmitAgendaProposal {
            proposal: proposal.clone(),
        }
        .execute(&authority, &mut tx)
        .expect("submit agenda proposal");

        let stored = tx
            .world
            .ministry_agenda_proposals
            .get(&proposal.proposal_id)
            .cloned()
            .expect("persisted ministry agenda proposal");
        assert_eq!(stored.proposal, proposal);
        assert_eq!(stored.authority, authority);
        assert_eq!(
            stored.submitted_tx_hash_hex,
            hex::encode(expected_tx_hash.as_ref())
        );
        assert_eq!(stored.submitted_height, tx.block_height());
    }

    #[test]
    fn submit_agenda_proposal_rejects_duplicate_proposal_ids() {
        let authority = checked_account_id();
        let proposal = sample_proposal("AC-2026-001");

        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(World::default(), kura, query);

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        tx.current_tx_hash = Some(tx_hash([0xCD; 32]));

        SubmitAgendaProposal {
            proposal: proposal.clone(),
        }
        .execute(&authority, &mut tx)
        .expect("first ministry agenda proposal");

        let duplicate_error = SubmitAgendaProposal { proposal }
            .execute(&authority, &mut tx)
            .expect_err("duplicate proposal id must fail");
        assert!(matches!(
            duplicate_error,
            InstructionExecutionError::InvariantViolation(message)
                if message.contains("Ministry agenda proposal AC-2026-001 already exists")
        ));
    }
}
