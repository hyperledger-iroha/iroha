//! Canonical archive and cross-field substitution checks using real signed device fixtures.

use super::*;
use crate::kagemusha_device_bridge_v1::sender_payload::canonical_command_body_for_tests;

fn candidate() -> KagemushaCoreSenderCandidateArchiveV1 {
    let bytes = canonical_command_body_for_tests(7).expect("signed commit fixture");
    let command = SenderCommandV1::decode_canonical_exact(7, [7; 32], &bytes)
        .expect("canonical commit fixture");
    let SenderCommandBodyV1::Commit {
        selector,
        candidate_digest,
        hardware_authorization,
    } = command.body
    else {
        panic!("commit fixture body")
    };
    KagemushaCoreSenderCandidateArchiveV1 {
        version: VERSION,
        preparation: KagemushaCoreSenderPreparationArchiveV1 {
            version: VERSION,
            operation_id: command.operation_id,
            context: command.context,
            inputs_digest: selector.inputs_digest,
        },
        selector,
        candidate_digest,
        hardware_commit_authorization: hardware_authorization,
    }
}

fn recovery() -> KagemushaCoreSenderRecoveryArchiveV1 {
    let preparation = candidate().preparation;
    KagemushaCoreSenderRecoveryArchiveV1 {
        version: VERSION,
        operation_id: preparation.operation_id,
        terminal_id: [0x81; 32],
        context: preparation.context,
        inputs_digest: preparation.inputs_digest,
    }
}

#[test]
fn coordinator_archives_roundtrip_exact_canonical_bytes() {
    let candidate = candidate();
    let preparation = &candidate.preparation;
    let recovery = recovery();
    assert_eq!(
        KagemushaCoreSenderPreparationArchiveV1::decode_canonical_exact(
            &preparation.encode_canonical().unwrap()
        )
        .unwrap(),
        *preparation
    );
    assert_eq!(
        KagemushaCoreSenderCandidateArchiveV1::decode_canonical_exact(
            &candidate.encode_canonical().unwrap()
        )
        .unwrap(),
        candidate
    );
    assert_eq!(
        KagemushaCoreSenderRecoveryArchiveV1::decode_canonical_exact(
            &recovery.encode_canonical().unwrap()
        )
        .unwrap(),
        recovery
    );
}

#[test]
fn coordinator_archives_reject_other_schemas_trailing_truncation_and_allocation_overflow() {
    let candidate = candidate();
    let preparation = candidate.preparation.encode_canonical().unwrap();
    let candidate = candidate.encode_canonical().unwrap();
    let recovery = recovery().encode_canonical().unwrap();
    type Decoder = fn(&[u8]) -> bool;
    let decoders: [Decoder; 3] = [
        |bytes| KagemushaCoreSenderPreparationArchiveV1::decode_canonical_exact(bytes).is_ok(),
        |bytes| KagemushaCoreSenderCandidateArchiveV1::decode_canonical_exact(bytes).is_ok(),
        |bytes| KagemushaCoreSenderRecoveryArchiveV1::decode_canonical_exact(bytes).is_ok(),
    ];
    let archives = [preparation, candidate, recovery];
    for (index, archive) in archives.iter().enumerate() {
        for (decoder_index, decoder) in decoders.iter().enumerate() {
            assert_eq!(decoder(archive), index == decoder_index);
        }
        let decoder = decoders[index];
        let mut trailing = archive.clone();
        trailing.push(0);
        assert!(!decoder(&trailing));
        assert!(!decoder(&archive[..archive.len() - 1]));
        assert!(!decoder(&[]));
        assert!(!decoder(&vec![
            0;
            KAGEMUSHA_CORE_COORDINATOR_ARCHIVE_MAX_BYTES_V1
                + 1
        ]));
    }
}

#[test]
fn coordinator_preparation_rejects_invalid_identity_context_and_version() {
    let valid = candidate().preparation;
    let mutations: [fn(&mut KagemushaCoreSenderPreparationArchiveV1); 5] = [
        |archive| archive.version = 2,
        |archive| archive.operation_id = [0; 32],
        |archive| archive.inputs_digest = [0; 32],
        |archive| archive.context.release.policy_epoch = 0,
        |archive| archive.context.hardware_epoch.generation = 0,
    ];
    for mutate in mutations {
        let mut archive = valid.clone();
        mutate(&mut archive);
        assert!(archive.validate_shape().is_err());
        assert!(archive.encode_canonical().is_err());
        let raw = norito::encode_canonical(&archive).unwrap();
        assert!(KagemushaCoreSenderPreparationArchiveV1::decode_canonical_exact(&raw).is_err());
    }
}

#[test]
fn coordinator_candidate_rejects_signed_authorization_and_selector_substitution() {
    let valid = candidate();
    let mutations: [fn(&mut KagemushaCoreSenderCandidateArchiveV1); 12] = [
        |archive| archive.version = 2,
        |archive| archive.preparation.version = 2,
        |archive| archive.preparation.operation_id[0] ^= 1,
        |archive| archive.preparation.inputs_digest[0] ^= 1,
        |archive| archive.preparation.context.release.release_id[0] ^= 1,
        |archive| archive.preparation.context.core_authorization_key_reference[0] ^= 1,
        |archive| archive.preparation.context.hardware_epoch.generation += 1,
        |archive| archive.selector.inputs_digest[0] ^= 1,
        |archive| archive.selector.preparation_id[0] ^= 1,
        |archive| archive.candidate_digest[0] ^= 1,
        |archive| archive.hardware_commit_authorization.push(0),
        |archive| archive.hardware_commit_authorization.clear(),
    ];
    for (index, mutate) in mutations.into_iter().enumerate() {
        let mut archive = valid.clone();
        mutate(&mut archive);
        assert!(archive.validate_shape().is_err(), "mutation {index}");
        assert!(archive.encode_canonical().is_err(), "mutation {index}");
        let raw = norito::encode_canonical(&archive).unwrap();
        assert!(
            KagemushaCoreSenderCandidateArchiveV1::decode_canonical_exact(&raw).is_err(),
            "mutation {index}"
        );
    }
}

#[test]
fn coordinator_recovery_rejects_missing_selector_context_and_version() {
    let valid = recovery();
    let mutations: [fn(&mut KagemushaCoreSenderRecoveryArchiveV1); 6] = [
        |archive| archive.version = 2,
        |archive| archive.operation_id = [0; 32],
        |archive| archive.terminal_id = [0; 32],
        |archive| archive.inputs_digest = [0; 32],
        |archive| archive.context.release.hardware_profile_id = [0; 32],
        |archive| archive.context.lane.device_lane_id = [0; 32],
    ];
    for mutate in mutations {
        let mut archive = valid.clone();
        mutate(&mut archive);
        assert!(archive.validate_shape().is_err());
        assert!(archive.encode_canonical().is_err());
        let raw = norito::encode_canonical(&archive).unwrap();
        assert!(KagemushaCoreSenderRecoveryArchiveV1::decode_canonical_exact(&raw).is_err());
    }
}
