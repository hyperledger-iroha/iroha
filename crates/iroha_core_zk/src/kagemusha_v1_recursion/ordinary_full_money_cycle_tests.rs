//! Full genuine active Mint→funded Send→Terminal/Wrapper→Receive proof corridor.
//! Known-public mathematical signatures/data install no FI/clock/release/DATA or Native owner.
//! Descriptor discovery never seeks a cryptographic key/protocol hash fixed point.
use super::ordinary_active_receive_state::prove_ordinary_receive_state_for_testing_v1;
use super::ordinary_active_send_state::prove_ordinary_send_state_for_testing_v1;
use super::ordinary_active_send_terminal::{
    OrdinarySendTerminalForTestingV1, prove_ordinary_send_terminal_for_testing_v1,
};
use super::ordinary_active_state_bootstrap::{
    OrdinaryBootstrapStateForTestingV1,
    generate_ordinary_bootstrap_state_with_held_keys_for_testing_v1, mathematical_protocol,
};
use super::ordinary_mint_genuine_qualification_tests::active_mint_state::prove_funded_ordinary_state_with_held_keys_for_testing_v1;
use super::*;
use crate::kagemusha_v1_recursion::real_handoff_qualification_tests::{
    StateKeys,
    real_payment_corridor::{
        ordinary_zero_bootstrap_hash_keys_for_testing,
        run_ordinary_zero_bootstrap_qualification_worker,
    },
};

#[test]
#[ignore = "whole genuine both-parity Mint/Guard/State/Terminal/Wrapper/Receive family; maintained exclusive proof worker and owned direct-libtest finite CPU/RSS guard required"]
fn ordinary_active_mint_funded_send_receive_freezes_real_key_family_and_all_originals() {
    run_ordinary_zero_bootstrap_qualification_worker(qualify_full_cycle);
}

fn qualify_full_cycle() {
    for apple in [false, true] {
        let (mut hash_eq, mut hash_ep) =
            ordinary_zero_bootstrap_hash_keys_for_testing([41; 32], [42; 32], [43; 32]);
        let mut wrapper_eq = mathematical_protocol(&canonical_kagemusha_eq_parameters_v1(), 83);
        let mut wrapper_ep = mathematical_protocol(&canonical_kagemusha_ep_parameters_v1(), 83);
        let mut frozen = None;
        // Only the value-free four-role dependency geometry is compared across diagnostic passes.
        // These positive mathematical proofs are discarded: their offered Wrapper identities
        // are not claimed to be the final held key family or an installed monetary release.
        for pass in 0..4 {
            let (diagnostic, receiver) =
                prove_pass(apple, hash_eq, hash_ep, &wrapper_eq, &wrapper_ep, None);
            let next_eq = diagnostic.wrapper.generated.eq_protocol.clone();
            let next_ep = diagnostic.wrapper.generated.ep_protocol.clone();
            let same = recursive_state_parent_structure_matches_v1(
                &wrapper_eq,
                &next_eq,
                KagemushaPastaParityV1::Eq,
            )
            .unwrap()
                && recursive_state_parent_structure_matches_v1(
                    &wrapper_ep,
                    &next_ep,
                    KagemushaPastaParityV1::Ep,
                )
                .unwrap();
            let actual_state_keys = Arc::clone(&diagnostic.send.funded.bootstrap.keys);
            let terminal_vk = [
                Arc::clone(&diagnostic.inner.keys.eq_verifying_key),
                Arc::clone(&diagnostic.inner.keys.ep_verifying_key),
            ];
            let wrapper_vk = [
                Arc::clone(&diagnostic.wrapper.keys.eq_verifying_key),
                Arc::clone(&diagnostic.wrapper.keys.ep_verifying_key),
            ];
            drop(receiver);
            let (eq, ep) = recycle_hash_keys(diagnostic);
            hash_eq = eq;
            hash_ep = ep;
            wrapper_eq = next_eq;
            wrapper_ep = next_ep;
            if same {
                frozen = Some((actual_state_keys, terminal_vk, wrapper_vk));
                break;
            }
            assert!(
                pass < 3,
                "full actual State/Terminal/Wrapper value-free geometry did not converge within four passes"
            );
            halo2_proofs::release_allocator_slack();
        }
        let (keys, terminal_vk, wrapper_vk) =
            frozen.expect("descriptor closure precedes final proof family");
        // Actual final Wrapper identities are now known. Replan every whole original/SHA claim
        // and prove the complete chain under the very same frozen State PK/VK, without keygen.
        let (sender, receiver) = prove_pass(
            apple,
            hash_eq,
            hash_ep,
            &wrapper_eq,
            &wrapper_ep,
            Some(Arc::clone(&keys)),
        );
        assert!(Arc::ptr_eq(&keys, &sender.send.funded.bootstrap.keys));
        assert!(Arc::ptr_eq(&keys, &receiver.keys));
        assert_eq!(
            sender.inner.keys.eq_verifying_key.as_ref(),
            terminal_vk[0].as_ref()
        );
        assert_eq!(
            sender.inner.keys.ep_verifying_key.as_ref(),
            terminal_vk[1].as_ref()
        );
        assert_eq!(
            sender.wrapper.keys.eq_verifying_key.as_ref(),
            wrapper_vk[0].as_ref()
        );
        assert_eq!(
            sender.wrapper.keys.ep_verifying_key.as_ref(),
            wrapper_vk[1].as_ref()
        );
        let wrapper_original: super::super::ordinary_cash_terminal_verifier::OrdinaryCashProofPairWireV1 =
            norito::decode_canonical(&sender.wrapper.generated.original).unwrap();
        // Full final exact identities, not just compatible descriptors, must match every State
        // public incoming root and the actually generated Wrapper current/history proofs.
        for (held, actual, parity) in [(
            &wrapper_eq,
            &sender.wrapper.generated.eq_protocol,
            KagemushaPastaParityV1::Eq,
        )] {
            assert!(recursive_state_parent_structure_matches_v1(held, actual, parity).unwrap());
            assert_eq!(
                native_parent_protocol_digest_v1(held, parity).unwrap(),
                native_parent_protocol_digest_v1(actual, parity).unwrap()
            );
        }
        assert!(
            recursive_state_parent_structure_matches_v1(
                &wrapper_ep,
                &sender.wrapper.generated.ep_protocol,
                KagemushaPastaParityV1::Ep
            )
            .unwrap()
        );
        assert_eq!(
            native_parent_protocol_digest_v1(&wrapper_ep, KagemushaPastaParityV1::Ep).unwrap(),
            wrapper_original.ep_protocol_digest
        );
        assert_eq!(
            sender.send.state_relation.commit_wrapper_eq_protocol_digest,
            wrapper_original.eq_protocol_digest
        );
        assert_eq!(
            sender.send.state_relation.commit_wrapper_ep_protocol_digest,
            wrapper_original.ep_protocol_digest
        );
        assert_eq!(sender.send.funded.state.balance, 177);
        assert_eq!(sender.send.state.balance, 160);
        let received = prove_ordinary_receive_state_for_testing_v1(&sender, receiver, apple);
        assert_eq!(received.state.balance, 17);
        assert_eq!(
            sender.send.state.balance + received.state.balance,
            sender.send.funded.state.balance
        );
        assert_eq!(received.state.logical_sequence, 1);
        assert_eq!(received.state.secure_index, 1);
        assert_ne!(received.public_original, received.receiver.public_original);
        assert!(!received.generated.eq_inner_proof.is_empty());
        assert!(!received.generated.ep_inner_proof.is_empty());
        assert!(!received.generated.proof.eq_proof.is_empty());
        assert!(!received.generated.proof.ep_proof.is_empty());
        eprintln!(
            "KAGEMUSHA full active graph mathematical proof PASS: platform={}, both parities, Mint177/Send17/Receive17, exact frozen State/Terminal/Wrapper keys; no external authority/effect qualification",
            if apple { "Apple" } else { "Android" }
        );
        drop(received);
        drop(sender);
        drop(keys);
        halo2_proofs::release_allocator_slack();
    }
}

fn prove_pass(
    apple: bool,
    hash_eq: KagemushaLoadedEqMintHashArtifactsV1,
    hash_ep: KagemushaLoadedEpMintHashArtifactsV1,
    wrapper_eq: &PlonkProtocol<EqAffine>,
    wrapper_ep: &PlonkProtocol<EpAffine>,
    held: Option<Arc<StateKeys>>,
) -> (
    OrdinarySendTerminalForTestingV1,
    OrdinaryBootstrapStateForTestingV1,
) {
    let funded = prove_funded_ordinary_state_with_held_keys_for_testing_v1(
        apple, [41; 32], [42; 32], [43; 32], hash_eq, hash_ep, wrapper_eq, wrapper_ep, held,
    );
    let source = &funded.mint_source;
    let receiver = generate_ordinary_bootstrap_state_with_held_keys_for_testing_v1(
        apple,
        &ordinary_qualification_wallet_account_v1(63),
        [41; 32],
        [42; 32],
        [43; 32],
        &source.source.hash_eq,
        &source.source.hash_ep,
        wrapper_eq,
        wrapper_ep,
        &source.pair.eq.protocol,
        &source.pair.ep.protocol,
        &source.source.eq_protocol,
        &source.source.ep_protocol,
        Some(Arc::clone(&funded.bootstrap.keys)),
    );
    let send = prove_ordinary_send_state_for_testing_v1(
        funded,
        receiver.credential.clone(),
        wrapper_eq,
        wrapper_ep,
        apple,
    );
    let terminal = prove_ordinary_send_terminal_for_testing_v1(send, apple);
    (terminal, receiver)
}

fn recycle_hash_keys(
    terminal: OrdinarySendTerminalForTestingV1,
) -> (
    KagemushaLoadedEqMintHashArtifactsV1,
    KagemushaLoadedEpMintHashArtifactsV1,
) {
    let source = terminal.send.funded.mint_source.source;
    (source.hash_eq, source.hash_ep)
}
