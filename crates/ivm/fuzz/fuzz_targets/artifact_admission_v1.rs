//! Structure-aware fuzzing for production V1 artifact admission and preparation.

#![no_main]

use std::sync::Arc;

use ivm::{ProgramMetadata, VerifiedContractArtifact, prepare_contract, verify_contract_artifact};
use libfuzzer_sys::fuzz_target;

const CANONICAL_ARTIFACT: &[u8] =
    include_bytes!("../../../kotodama_lang/src/samples/zk_vote_ballot.to");
const MAX_MUTATIONS: usize = 64;
const MAX_INSERTED_BYTES: usize = 256;

fuzz_target!(|data: &[u8]| {
    match data.split_first() {
        Some((&b'R', raw)) => {
            exercise_admission(raw);
            return;
        }
        Some((&b'S', _)) => {
            exercise_admission(CANONICAL_ARTIFACT);
            return;
        }
        _ => {}
    }

    let candidate = mutate_canonical_artifact(data);
    exercise_admission(&candidate);
});

fn mutate_canonical_artifact(data: &[u8]) -> Vec<u8> {
    let mut artifact = CANONICAL_ARTIFACT.to_vec();
    let Some((&mode, payload)) = data.split_first() else {
        return artifact;
    };

    match mode % 7 {
        0 => {}
        1 => {
            for mutation in payload.chunks(3).take(MAX_MUTATIONS) {
                if artifact.is_empty() {
                    break;
                }
                let high = usize::from(mutation[0]);
                let low = usize::from(*mutation.get(1).unwrap_or(&0));
                let index = ((high << 8) | low) % artifact.len();
                artifact[index] ^= *mutation.get(2).unwrap_or(&0xff);
            }
        }
        2 => {
            let keep = bounded_index(payload, artifact.len() + 1);
            artifact.truncate(keep);
        }
        3 => artifact.extend(payload.iter().copied().take(MAX_INSERTED_BYTES)),
        4 => {
            if !artifact.is_empty() {
                let start = bounded_index(payload, artifact.len());
                let available = artifact.len() - start;
                let remove = bounded_index(payload.get(2..).unwrap_or_default(), available + 1);
                artifact.drain(start..start + remove);
            }
        }
        5 => {
            let index = bounded_index(payload, artifact.len() + 1);
            artifact.splice(
                index..index,
                payload.iter().copied().skip(2).take(MAX_INSERTED_BYTES),
            );
        }
        6 => {
            for (byte, replacement) in artifact
                .iter_mut()
                .zip(payload.iter().copied())
                .take(MAX_INSERTED_BYTES)
            {
                *byte = replacement;
            }
        }
        _ => unreachable!("modulo seven is exhaustive"),
    }
    artifact
}

fn bounded_index(bytes: &[u8], bound: usize) -> usize {
    debug_assert!(bound > 0);
    let high = usize::from(bytes.first().copied().unwrap_or(0));
    let low = usize::from(bytes.get(1).copied().unwrap_or(0));
    ((high << 8) | low) % bound
}

fn metadata_tuple(metadata: &ProgramMetadata) -> (u8, u8, u8, u8, u64, u8) {
    (
        metadata.version_major,
        metadata.version_minor,
        metadata.mode,
        metadata.vector_length,
        metadata.max_cycles,
        metadata.abi_version,
    )
}

fn assert_verified_equal(first: &VerifiedContractArtifact, second: &VerifiedContractArtifact) {
    assert_eq!(
        metadata_tuple(&first.metadata),
        metadata_tuple(&second.metadata)
    );
    assert_eq!(first.header_len, second.header_len);
    assert_eq!(first.code_offset, second.code_offset);
    assert_eq!(first.code_hash, second.code_hash);
    assert_eq!(first.abi_hash, second.abi_hash);
    assert_eq!(first.contract_interface, second.contract_interface);
    assert_eq!(first.manifest, second.manifest);
}

fn exercise_admission(artifact: &[u8]) {
    let first = verify_contract_artifact(artifact);
    let second = verify_contract_artifact(artifact);

    match (first, second) {
        (Err(first_error), Err(second_error)) => {
            assert_eq!(first_error, second_error);
            match prepare_contract(Arc::<[u8]>::from(artifact)) {
                Err(preparation_error) => assert_eq!(preparation_error, first_error),
                Ok(_) => panic!("native preparation accepted a rejected artifact"),
            }
        }
        (Ok(first_verified), Ok(second_verified)) => {
            assert_verified_equal(&first_verified, &second_verified);
            let prepared = prepare_contract(Arc::<[u8]>::from(artifact))
                .expect("production preparation must accept a verified artifact");
            assert_eq!(prepared.artifact(), artifact);
            assert_eq!(
                metadata_tuple(prepared.metadata()),
                metadata_tuple(&first_verified.metadata)
            );
            assert_eq!(prepared.header_len(), first_verified.header_len);
            assert_eq!(prepared.code_offset(), first_verified.code_offset);
            assert_eq!(prepared.code_hash(), first_verified.code_hash);
            assert_eq!(
                prepared.contract_interface(),
                &first_verified.contract_interface
            );
            assert_eq!(prepared.manifest(), &first_verified.manifest);
        }
        (first, second) => panic!(
            "artifact admission is nondeterministic: first={}, second={}",
            first.is_ok(),
            second.is_ok()
        ),
    }
}
