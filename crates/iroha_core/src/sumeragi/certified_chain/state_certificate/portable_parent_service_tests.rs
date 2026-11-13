//! Parent participation uses the sealed cut then resumes original native producer custody.

use super::*;

#[test]
fn portable_resume_parent_service_preserves_exact_quorum_refusal_and_native_production() {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(|| {
            let fixture = crate::snapshot::tests::transferred_checkpoint_fixture();
            let budget = fixture.restored.ivm_execution_budget();
            let mut proposal = fixture
                .source
                .committed(3)
                .block()
                .as_ref()
                .clone()
                .into_resultless_proposal()
                .unwrap();
            admit_parent_service_fixture(&mut proposal, &budget).unwrap();
            {
                let view = fixture.restored.view();
                let mut prepared = ParentServicePreparation::default();
                prepared.prepare(&view, 2).unwrap();
                assert!(prepared.native_current().is_err());
                prepared.prepare_proposal_qc(&view, &proposal).unwrap();
                let service = prepared.authenticate(&view, &proposal).unwrap().unwrap();
                assert_eq!(service.signer_keys().count(), 3);
                let source = prepared.original_pointers();
                let signers = prepared.signer_pointers();
                let refusal = norito::core::with_decode_limits_scope(
                    norito::core::DecodeLimits::new(
                        usize::MAX,
                        1,
                        usize::MAX,
                        usize::MAX,
                        usize::MAX,
                    ),
                    || prepared.authenticate(&view, &proposal),
                )
                .err()
                .expect("the original decoder refusal survives");
                assert!(matches!(refusal, ParentServiceError::Deferred(_)));
                assert_eq!(prepared.original_pointers(), source);
                assert_eq!(prepared.signer_pointers(), signers);
                assert_eq!(
                    prepared
                        .authenticate(&view, &proposal)
                        .unwrap()
                        .unwrap()
                        .signer_keys()
                        .count(),
                    3
                );
                let mut foreign = proposal.clone();
                let mut effects = foreign.npos_consensus_effects().unwrap().clone();
                let bytes = effects.parent_service_commit_qc.as_ref().unwrap();
                let mut qc: Qc = norito::decode_canonical(bytes).unwrap();
                qc.result.0[0] ^= 1;
                effects.parent_service_commit_qc = Some(
                    iroha_data_model::consensus::NposParentServiceCommitQc::from_untrusted(
                        norito::encode_canonical(&qc).unwrap(),
                    ),
                );
                foreign.set_npos_consensus_effects(Some(effects));
                admit_parent_service_fixture(&mut foreign, &budget).unwrap();
                let mut changed = ParentServicePreparation::default();
                changed.prepare(&view, 2).unwrap();
                assert!(matches!(
                    changed.prepare_proposal_qc(&view, &foreign),
                    Err(ParentServiceError::Invalid(_))
                ));
            }
            let (events, _) = tokio::sync::broadcast::channel(64);
            let _prepared = crate::sumeragi::node::prepare(crate::sumeragi::node::PrepareInputs {
                state: fixture.restored.clone(),
                events,
                genesis: None,
                genesis_account: fixture.source.genesis_account().clone(),
                consensus_mode: iroha_data_model::parameter::system::ConsensusMode::Permissioned,
            })
            .unwrap();
            let mut proposal = fixture.source.proposal(None, Vec::new());
            admit_parent_service_fixture(&mut proposal, &budget).unwrap();
            let view = fixture.restored.view();
            let parent = view.latest_block().unwrap().unwrap();
            let mut prepared = ParentServicePreparation::default();
            prepared.prepare(&view, 3).unwrap();
            assert_eq!(
                prepared.native_current().unwrap().block_hash(),
                parent.hash()
            );
            prepared.prepare_producer_copy(&view, &parent).unwrap();
            assert_eq!(
                &*prepared.proposal_original(&view, &parent).unwrap(),
                parent.commit_certificate().unwrap().commit_qc()
            );
            prepared.prepare_proposal_qc(&view, &proposal).unwrap();
            assert_eq!(
                prepared
                    .authenticate(&view, &proposal)
                    .unwrap()
                    .unwrap()
                    .signer_keys()
                    .count(),
                3
            );
        })
        .unwrap()
        .join()
        .unwrap_or_else(|failure| std::panic::resume_unwind(failure));
}
