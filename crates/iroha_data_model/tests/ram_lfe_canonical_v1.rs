//! Canonical V1 RAM-LFE receipt and opening: codec, framing and binding of every field.
//!
//! This is a separate target so it does not depend on the large internal unit suite.

use iroha_crypto::{
    Algorithm, Hash, HashOf, HiddenRamFheInstruction as Op, HiddenRamFheProgram, KeyPair,
    RamLfeAssociatedDataHashV1, RamLfeClassV1, RamLfeEncryptionKeyCommitmentV1, RamLfeError,
    RamLfeEvaluationKeyCommitmentV1, RamLfeExecutionContextIdV1, RamLfeFunctionIdentityV1,
    RamLfeInitialMemoryV1, RamLfeInitializedMemoryCommitmentV1, RamLfeInputCiphertextCommitmentV1,
    RamLfeOpeningKeyCommitmentV1, RamLfeOrderedOutputV1, RamLfeOutputBlindingV1,
    RamLfeOutputCiphertextCommitmentV1, RamLfeOutputCommitmentV1, RamLfePolicyV1,
    RamLfePrfKeyCommitmentV1, RamLfeProfileIdV1, RamLfeProgramKeyV1, RamLfeQueryLimitV1,
    RamLfeRelationIdV1,
};
use iroha_data_model::{
    NetworkId,
    account::AccountId,
    block::BlockHeader,
    ram_lfe::{
        RAM_LFE_V1_MODEL_DOMAINS, RamLfeExecutionContextV1, RamLfeOpeningV1, RamLfeProgramId,
        RamLfeReceiptV1, RamLfeReplayDomainV1, RamLfeReplayV1,
    },
};
use iroha_schema::IntoSchema;
use std::{collections::BTreeSet, fmt};

fn program(multiplier: u64) -> HiddenRamFheProgram {
    let mut builder = HiddenRamFheProgram::builder().unwrap();
    for instruction in [
        Op::LoadInput(0, 1),
        Op::MulPlain(0, 0, multiplier),
        Op::Output(0),
    ] {
        builder.push(instruction).unwrap();
    }
    builder.finish().unwrap()
}

fn program_key(byte: u8) -> RamLfeProgramKeyV1 {
    RamLfeProgramKeyV1::from_bytes([byte; 32]).unwrap()
}

fn limited_function(
    class: RamLfeClassV1,
    key: u8,
    multiplier: u64,
    limit: (u64, u32),
) -> RamLfeFunctionIdentityV1 {
    RamLfeFunctionIdentityV1::commit(
        class,
        &program_key(key),
        &program(multiplier),
        RamLfeQueryLimitV1::new(limit.0, limit.1).unwrap(),
    )
    .unwrap()
}

fn function(class: RamLfeClassV1, key: u8, multiplier: u64) -> RamLfeFunctionIdentityV1 {
    limited_function(class, key, multiplier, (1000, 3))
}

fn policy() -> RamLfePolicyV1 {
    RamLfePolicyV1 {
        function: function(RamLfeClassV1::Affine, 7, 5),
        profile: RamLfeProfileIdV1::from_descriptor(b"profile").unwrap(),
        encryption_key: RamLfeEncryptionKeyCommitmentV1::commit(b"encryption").unwrap(),
        evaluation_key: RamLfeEvaluationKeyCommitmentV1::commit(b"evaluation").unwrap(),
        opening_key: RamLfeOpeningKeyCommitmentV1::commit(b"opening").unwrap(),
        prf_key: RamLfePrfKeyCommitmentV1::commit(b"prf").unwrap(),
        relation: RamLfeRelationIdV1::from_descriptor(b"relation").unwrap(),
    }
}

fn network(seed: &[u8]) -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
        seed,
    )))
}

fn account(seed: u8) -> AccountId {
    AccountId::new(
        KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .unwrap()
            .public_key()
            .clone(),
    )
}

fn program_id(name: &str) -> RamLfeProgramId {
    name.parse().unwrap()
}

fn context() -> RamLfeExecutionContextV1 {
    RamLfeExecutionContextV1 {
        network: network(b"genesis"),
        program_id: program_id("phone_retail"),
        replay: RamLfeReplayV1 {
            domain: RamLfeReplayDomainV1::Execution,
            nonce: [0x5A; 32],
        },
    }
}

/// The evaluator's commitment to the lanes of one execution: it holds the
/// program key and takes the execution context from the request.
fn memory(lane: u16, execution: &RamLfeExecutionContextV1) -> RamLfeInitializedMemoryCommitmentV1 {
    RamLfeInitializedMemoryCommitmentV1::commit(
        &program_key(7),
        policy().function.id().unwrap(),
        RamLfeAssociatedDataHashV1::commit(b"request").unwrap(),
        execution.id().unwrap(),
        &RamLfeInitialMemoryV1::from_lanes(&[lane; 32]).unwrap(),
    )
    .unwrap()
}

fn receipt_for(policy: &RamLfePolicyV1) -> RamLfeReceiptV1 {
    let execution = context();
    RamLfeReceiptV1 {
        policy: policy.commitment().unwrap(),
        network: execution.network,
        program_id: execution.program_id.clone(),
        beneficiary: account(1),
        associated_data: RamLfeAssociatedDataHashV1::commit(b"request").unwrap(),
        initialized_memory: memory(9, &execution),
        input_ciphertext: RamLfeInputCiphertextCommitmentV1::commit(b"input").unwrap(),
        output_ciphertext: RamLfeOutputCiphertextCommitmentV1::commit(b"output").unwrap(),
        expires_at_ms: 1_777_777_877_000,
        replay: execution.replay,
    }
}

fn receipt() -> RamLfeReceiptV1 {
    receipt_for(&policy())
}

fn output(scalars: &[u16], blinding: u8) -> RamLfeOutputCommitmentV1 {
    RamLfeOutputCommitmentV1::commit(
        &RamLfeOrderedOutputV1::from_scalars(scalars).unwrap(),
        &RamLfeOutputBlindingV1::from_bytes([blinding; 32]).unwrap(),
    )
    .unwrap()
}

fn opening() -> RamLfeOpeningV1 {
    RamLfeOpeningV1::new(&policy(), &receipt(), output(&[1, 2, 256], 3)).unwrap()
}

fn roundtrip<T>(value: &T)
where
    T: norito::NoritoSerialize + for<'de> norito::NoritoDeserialize<'de> + PartialEq + fmt::Debug,
{
    let frame = norito::encode_canonical(value).unwrap();
    assert_eq!(&norito::decode_canonical::<T>(&frame).unwrap(), value);
    assert!(norito::decode_canonical::<T>(&frame[..frame.len() - 1]).is_err());
}

#[test]
fn every_canonical_model_type_roundtrips_through_norito() {
    let receipt = receipt();
    let opening = opening();
    roundtrip(&RamLfeReplayDomainV1::Execution);
    roundtrip(&RamLfeReplayDomainV1::IdentifierClaim);
    roundtrip(&receipt.replay);
    roundtrip(&receipt.execution_context());
    roundtrip(&receipt);
    roundtrip(&receipt.commitment().unwrap());
    roundtrip(&opening);
    roundtrip(&opening.commitment().unwrap());
}

#[test]
fn replay_domain_decoding_rejects_every_unknown_discriminant() {
    // Frame a raw discriminant with the header flags a canonical scope frame carries.
    let canonical = norito::encode_canonical(&RamLfeReplayDomainV1::Execution).unwrap();
    let flags = norito::core::from_bytes_view(&canonical).unwrap().flags();
    let frame = |discriminant: u32| {
        norito::core::frame_bare_with_header_flags::<RamLfeReplayDomainV1>(
            &discriminant.to_le_bytes(),
            flags,
        )
        .unwrap()
    };
    // The hand-built frame is the canonical frame of each declared scope.
    for (discriminant, domain) in [
        RamLfeReplayDomainV1::Execution,
        RamLfeReplayDomainV1::IdentifierClaim,
    ]
    .into_iter()
    .enumerate()
    {
        let discriminant = u32::try_from(discriminant).unwrap();
        assert_eq!(
            frame(discriminant),
            norito::encode_canonical(&domain).unwrap()
        );
        assert_eq!(
            norito::decode_canonical::<RamLfeReplayDomainV1>(&frame(discriminant)).unwrap(),
            domain
        );
    }
    for discriminant in [2_u32, 3, 255, u32::MAX] {
        assert!(
            norito::decode_canonical::<RamLfeReplayDomainV1>(&frame(discriminant)).is_err(),
            "discriminant {discriminant} must not decode"
        );
    }
}

#[test]
fn receipt_and_opening_commitments_are_the_documented_framing() {
    let receipt = receipt();
    let frame = norito::encode_canonical(&receipt).unwrap();
    assert_eq!(
        receipt.commitment().unwrap().as_hash(),
        &Hash::new([b"iroha.ram_lfe.v1.receipt".as_slice(), &frame].concat())
    );
    let opening = opening();
    let frame = norito::encode_canonical(&opening).unwrap();
    assert_eq!(
        opening.commitment().unwrap().as_hash(),
        &Hash::new([b"iroha.ram_lfe.v1.opening".as_slice(), &frame].concat())
    );
    // The same bytes under the other role give another digest.
    assert_ne!(
        receipt.commitment().unwrap().as_hash(),
        &Hash::new([b"iroha.ram_lfe.v1.opening".as_slice(), &frame].concat())
    );
}

#[test]
fn execution_context_is_the_unique_execution_fields_and_not_the_receipt() {
    let receipt = receipt();
    let execution = receipt.execution_context();
    assert_eq!(execution, context());
    // Documented framing: the canonical frame of the record, committed as a
    // byte string under the execution-context domain.
    let frame = norito::encode_canonical(&execution).unwrap();
    assert_eq!(
        execution.id().unwrap(),
        RamLfeExecutionContextIdV1::commit(&frame).unwrap()
    );

    // It binds the network, the program, the replay scope and the nonce.
    let mut seen = BTreeSet::from([execution.id().unwrap()]);
    for (field, changed) in [
        (
            "network",
            RamLfeExecutionContextV1 {
                network: network(b"other genesis"),
                ..execution.clone()
            },
        ),
        (
            "program",
            RamLfeExecutionContextV1 {
                program_id: program_id("email_retail"),
                ..execution.clone()
            },
        ),
        (
            "replay domain",
            RamLfeExecutionContextV1 {
                replay: RamLfeReplayV1 {
                    domain: RamLfeReplayDomainV1::IdentifierClaim,
                    ..execution.replay
                },
                ..execution.clone()
            },
        ),
        (
            "replay nonce",
            RamLfeExecutionContextV1 {
                replay: RamLfeReplayV1 {
                    nonce: [0x5B; 32],
                    ..execution.replay
                },
                ..execution.clone()
            },
        ),
    ] {
        assert!(
            seen.insert(changed.id().unwrap()),
            "execution context must bind the {field}"
        );
        // Another execution context gives another commitment to the same lanes.
        assert_ne!(memory(9, &changed), memory(9, &execution), "{field}");
    }

    // It does not depend on anything the evaluator computes afterwards, so the
    // memory commitment it blinds is not circular.
    let later = RamLfeReceiptV1 {
        initialized_memory: memory(10, &execution),
        output_ciphertext: RamLfeOutputCiphertextCommitmentV1::commit(b"other").unwrap(),
        beneficiary: account(2),
        expires_at_ms: receipt.expires_at_ms + 1,
        ..receipt.clone()
    };
    assert_eq!(later.execution_context(), execution);
    assert_ne!(later.commitment().unwrap(), receipt.commitment().unwrap());
}

#[test]
fn initialized_memory_commitment_hides_the_lanes_from_everyone_without_the_program_key() {
    // Everything the receipt publishes about the memory commitment is public:
    // the function identity, the associated-data digest and the execution
    // context. A party that guesses the lanes still cannot recompute it.
    let receipt = receipt();
    let lanes = RamLfeInitialMemoryV1::from_lanes(&[9; 32]).unwrap();
    let recompute = |key: u8| {
        RamLfeInitializedMemoryCommitmentV1::commit(
            &program_key(key),
            policy().function.id().unwrap(),
            receipt.associated_data,
            receipt.execution_context().id().unwrap(),
            &lanes,
        )
        .unwrap()
    };
    assert_eq!(recompute(7), receipt.initialized_memory);
    for key in [1_u8, 6, 8, 0xFF] {
        assert_ne!(recompute(key), receipt.initialized_memory);
    }
}

#[test]
fn model_domains_extend_the_crypto_domains_without_prefix_collisions() {
    let all: Vec<&[u8]> = iroha_crypto::RAM_LFE_V1_PUBLIC_DOMAINS
        .into_iter()
        .chain(RAM_LFE_V1_MODEL_DOMAINS)
        .collect();
    assert_eq!(all.len(), 15);
    for (index, domain) in all.iter().enumerate() {
        assert!(domain.starts_with(b"iroha.ram_lfe.v1."));
        for (other_index, other) in all.iter().enumerate() {
            assert!(index == other_index || !other.starts_with(domain));
        }
    }
}

fn schema_fields<T: IntoSchema>() -> Vec<String> {
    match T::schema().get::<T>() {
        Some(iroha_schema::Metadata::Struct(meta)) => meta
            .declarations
            .iter()
            .map(|declaration| declaration.name.clone())
            .collect(),
        other => panic!("{} is not a named-field record: {other:?}", T::type_name()),
    }
}

#[test]
fn specification_field_order_is_the_model_record_definitions() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../specs/ram_lfe_execution_proof.md"
    );
    let document = std::fs::read_to_string(path).expect("tracked RAM-LFE specification");
    for domain in RAM_LFE_V1_MODEL_DOMAINS {
        let domain = std::str::from_utf8(domain).unwrap();
        assert!(
            document.contains(&format!("`{domain}`")),
            "specification lacks domain {domain}"
        );
    }
    // The specification lists each record's fields in the order of its definition.
    let records = [
        (
            "iroha.ram_lfe.v1.execution_context",
            "RamLfeExecutionContextV1",
            schema_fields::<RamLfeExecutionContextV1>(),
        ),
        ("", "RamLfeReplayV1", schema_fields::<RamLfeReplayV1>()),
        (
            "iroha.ram_lfe.v1.receipt",
            "RamLfeReceiptV1",
            schema_fields::<RamLfeReceiptV1>(),
        ),
        (
            "iroha.ram_lfe.v1.opening",
            "RamLfeOpeningV1",
            schema_fields::<RamLfeOpeningV1>(),
        ),
    ];
    for (domain, record, fields) in records {
        let marker = format!("`{record}`: ");
        let mut rows = document.lines().filter(|line| line.contains(&marker));
        let row = rows
            .next()
            .unwrap_or_else(|| panic!("specification has no field row for {record}"));
        assert!(
            rows.next().is_none(),
            "specification defines {record} more than once"
        );
        let specified: Vec<&str> = row[row.find(&marker).unwrap() + marker.len()..]
            .split('|')
            .next()
            .unwrap()
            .split('`')
            .skip(1)
            .step_by(2)
            .collect();
        assert_eq!(specified, fields, "field order of {record}");
        assert!(
            domain.is_empty() || row.contains(&format!("`{domain}`")),
            "the row of {record} must name {domain}"
        );
    }
    // The receipt's field count is the one the tests below substitute.
    assert_eq!(schema_fields::<RamLfeReceiptV1>().len(), 10);
    assert_eq!(schema_fields::<RamLfeOpeningV1>().len(), 4);
}

#[test]
fn receipt_commitment_binds_the_full_policy() {
    let base = policy();
    let substitutions: [(&str, RamLfePolicyV1); 11] = [
        (
            "class",
            RamLfePolicyV1 {
                function: function(RamLfeClassV1::Bounded, 7, 5),
                ..base
            },
        ),
        (
            "function tape",
            RamLfePolicyV1 {
                function: function(RamLfeClassV1::Affine, 7, 6),
                ..base
            },
        ),
        (
            "program key and initializer",
            RamLfePolicyV1 {
                function: function(RamLfeClassV1::Affine, 8, 5),
                ..base
            },
        ),
        (
            "query limit total",
            RamLfePolicyV1 {
                function: limited_function(RamLfeClassV1::Affine, 7, 5, (1001, 3)),
                ..base
            },
        ),
        (
            "query limit per beneficiary",
            RamLfePolicyV1 {
                function: limited_function(RamLfeClassV1::Affine, 7, 5, (1000, 4)),
                ..base
            },
        ),
        (
            "profile",
            RamLfePolicyV1 {
                profile: RamLfeProfileIdV1::from_descriptor(b"other").unwrap(),
                ..base
            },
        ),
        (
            "encryption key",
            RamLfePolicyV1 {
                encryption_key: RamLfeEncryptionKeyCommitmentV1::commit(b"other").unwrap(),
                ..base
            },
        ),
        (
            "evaluation key",
            RamLfePolicyV1 {
                evaluation_key: RamLfeEvaluationKeyCommitmentV1::commit(b"other").unwrap(),
                ..base
            },
        ),
        (
            "opening key",
            RamLfePolicyV1 {
                opening_key: RamLfeOpeningKeyCommitmentV1::commit(b"other").unwrap(),
                ..base
            },
        ),
        (
            "PRF key",
            RamLfePolicyV1 {
                prf_key: RamLfePrfKeyCommitmentV1::commit(b"other").unwrap(),
                ..base
            },
        ),
        (
            "relation identity",
            RamLfePolicyV1 {
                relation: RamLfeRelationIdV1::from_descriptor(b"other").unwrap(),
                ..base
            },
        ),
    ];
    let receipt = receipt();
    let genesis = network(b"genesis");
    let program = program_id("phone_retail");
    let mut seen = BTreeSet::from([receipt.commitment().unwrap()]);
    for (field, substituted) in substitutions {
        let other = receipt_for(&substituted);
        assert!(
            seen.insert(other.commitment().unwrap()),
            "receipt commitment must bind the policy's {field}"
        );
        // A receipt for the substituted policy is not a receipt for the
        // authoritative one, and the reverse.
        assert_eq!(
            other.verify_authority(&base, &genesis, &program),
            Err(RamLfeError::InvalidCanonicalValue(
                "receipt is bound to another policy"
            ))
        );
        assert!(
            receipt
                .verify_authority(&substituted, &genesis, &program)
                .is_err()
        );
        other
            .verify_authority(&substituted, &genesis, &program)
            .unwrap();
    }
}

#[test]
fn receipt_commitment_binds_every_request_field() {
    let base = receipt();
    let execution = base.execution_context();
    let substitutions: [(&str, RamLfeReceiptV1); 11] = [
        (
            "network",
            RamLfeReceiptV1 {
                network: network(b"other genesis"),
                ..base.clone()
            },
        ),
        (
            "program",
            RamLfeReceiptV1 {
                program_id: program_id("email_retail"),
                ..base.clone()
            },
        ),
        (
            "beneficiary",
            RamLfeReceiptV1 {
                beneficiary: account(2),
                ..base.clone()
            },
        ),
        (
            "associated data",
            RamLfeReceiptV1 {
                associated_data: RamLfeAssociatedDataHashV1::commit(b"other").unwrap(),
                ..base.clone()
            },
        ),
        (
            "initialized memory",
            RamLfeReceiptV1 {
                initialized_memory: memory(10, &execution),
                ..base.clone()
            },
        ),
        (
            "input ciphertext",
            RamLfeReceiptV1 {
                input_ciphertext: RamLfeInputCiphertextCommitmentV1::commit(b"other").unwrap(),
                ..base.clone()
            },
        ),
        (
            "output ciphertext",
            RamLfeReceiptV1 {
                output_ciphertext: RamLfeOutputCiphertextCommitmentV1::commit(b"other").unwrap(),
                ..base.clone()
            },
        ),
        (
            "swapped ciphertexts",
            RamLfeReceiptV1 {
                input_ciphertext: RamLfeInputCiphertextCommitmentV1::commit(b"output").unwrap(),
                output_ciphertext: RamLfeOutputCiphertextCommitmentV1::commit(b"input").unwrap(),
                ..base.clone()
            },
        ),
        (
            "expiry",
            RamLfeReceiptV1 {
                expires_at_ms: base.expires_at_ms + 1,
                ..base.clone()
            },
        ),
        (
            "replay domain",
            RamLfeReceiptV1 {
                replay: RamLfeReplayV1 {
                    domain: RamLfeReplayDomainV1::IdentifierClaim,
                    ..base.replay
                },
                ..base.clone()
            },
        ),
        (
            "replay nonce",
            RamLfeReceiptV1 {
                replay: RamLfeReplayV1 {
                    nonce: [0x5B; 32],
                    ..base.replay
                },
                ..base.clone()
            },
        ),
    ];
    let mut seen = BTreeSet::from([base.commitment().unwrap()]);
    for (field, substituted) in substitutions {
        assert_ne!(substituted, base);
        assert!(
            seen.insert(substituted.commitment().unwrap()),
            "receipt commitment must bind the {field}"
        );
    }
    assert_eq!(base.commitment().unwrap(), receipt().commitment().unwrap());
}

#[test]
fn receipt_authority_is_the_committed_policy_network_and_program() {
    let policy = policy();
    let receipt = receipt();
    let genesis = network(b"genesis");
    let program = program_id("phone_retail");
    receipt
        .verify_authority(&policy, &genesis, &program)
        .unwrap();
    assert_eq!(
        receipt.verify_authority(&policy, &network(b"other genesis"), &program),
        Err(RamLfeError::InvalidCanonicalValue(
            "receipt is bound to another network"
        ))
    );

    // Two programs registered with the same policy do not accept each other's
    // receipts: the policy record carries no program identifier, so the
    // authoritative program is compared separately.
    let other_program = program_id("email_retail");
    assert_eq!(
        receipt.verify_authority(&policy, &genesis, &other_program),
        Err(RamLfeError::InvalidCanonicalValue(
            "receipt is bound to another program"
        ))
    );
    let other_receipt = RamLfeReceiptV1 {
        program_id: other_program.clone(),
        ..receipt.clone()
    };
    assert_eq!(other_receipt.policy, receipt.policy);
    other_receipt
        .verify_authority(&policy, &genesis, &other_program)
        .unwrap();
    assert_eq!(
        other_receipt.verify_authority(&policy, &genesis, &program),
        Err(RamLfeError::InvalidCanonicalValue(
            "receipt is bound to another program"
        ))
    );

    // Rotating the encryption keys keeps the logical function and its query
    // limit, and invalidates receipts made under the previous keys.
    let rotated = policy.rotate_encryption_keys(
        RamLfeEncryptionKeyCommitmentV1::commit(b"rotated").unwrap(),
        RamLfeEvaluationKeyCommitmentV1::commit(b"rotated").unwrap(),
        RamLfeOpeningKeyCommitmentV1::commit(b"rotated").unwrap(),
    );
    assert_eq!(
        rotated.function.id().unwrap(),
        policy.function.id().unwrap()
    );
    assert_eq!(rotated.function.query_limit, policy.function.query_limit);
    assert!(
        receipt
            .verify_authority(&rotated, &genesis, &program)
            .is_err()
    );
    let after_rotation = receipt_for(&rotated);
    after_rotation
        .verify_authority(&rotated, &genesis, &program)
        .unwrap();
    // The lanes and their commitment for the same execution are unchanged by
    // the rotation, because neither depends on a rotated key.
    assert_eq!(
        after_rotation.initialized_memory,
        receipt.initialized_memory
    );
}

#[test]
fn only_the_beneficiary_may_request_an_opening() {
    let receipt = receipt();
    assert_eq!(receipt.beneficiary, account(1));
    receipt.verify_opening_requester(&account(1)).unwrap();
    // Any other authenticated authority is refused, whoever it is: another
    // client, the program owner or the opener.
    for other in [2_u8, 3, 0xFF] {
        assert_eq!(
            receipt.verify_opening_requester(&account(other)),
            Err(RamLfeError::UnauthorizedOpeningRequester)
        );
    }
    // The rule follows the receipt: a receipt for another beneficiary is
    // requested by that beneficiary only.
    let other = RamLfeReceiptV1 {
        beneficiary: account(2),
        ..receipt
    };
    other.verify_opening_requester(&account(2)).unwrap();
    assert_eq!(
        other.verify_opening_requester(&account(1)),
        Err(RamLfeError::UnauthorizedOpeningRequester)
    );
}

#[test]
fn opening_takes_its_keys_from_the_policy_and_binds_every_field() {
    let policy = policy();
    let receipt = receipt();
    let base = opening();
    assert_eq!(base.receipt, receipt.commitment().unwrap());
    assert_eq!(base.opening_key, policy.opening_key);
    assert_eq!(base.prf_key, policy.prf_key);
    base.verify_authority(&policy, &receipt).unwrap();

    let substitutions: [(&str, RamLfeOpeningV1); 7] = [
        (
            "receipt",
            RamLfeOpeningV1 {
                receipt: RamLfeReceiptV1 {
                    expires_at_ms: receipt.expires_at_ms + 1,
                    ..receipt.clone()
                }
                .commitment()
                .unwrap(),
                ..base
            },
        ),
        (
            "output scalar",
            RamLfeOpeningV1 {
                output: output(&[1, 2, 255], 3),
                ..base
            },
        ),
        (
            "output order",
            RamLfeOpeningV1 {
                output: output(&[2, 1, 256], 3),
                ..base
            },
        ),
        (
            "output count",
            RamLfeOpeningV1 {
                output: output(&[1, 2], 3),
                ..base
            },
        ),
        (
            "output blinding",
            RamLfeOpeningV1 {
                output: output(&[1, 2, 256], 4),
                ..base
            },
        ),
        (
            "opening key",
            RamLfeOpeningV1 {
                opening_key: RamLfeOpeningKeyCommitmentV1::commit(b"other").unwrap(),
                ..base
            },
        ),
        (
            "PRF key",
            RamLfeOpeningV1 {
                prf_key: RamLfePrfKeyCommitmentV1::commit(b"other").unwrap(),
                ..base
            },
        ),
    ];
    let mut seen = BTreeSet::from([base.commitment().unwrap()]);
    for (field, substituted) in substitutions {
        assert!(
            seen.insert(substituted.commitment().unwrap()),
            "opening commitment must bind the {field}"
        );
        // The output is the opener's claim; every other field is authoritative.
        if !field.starts_with("output") {
            assert_eq!(
                substituted.verify_authority(&policy, &receipt),
                Err(RamLfeError::InvalidCanonicalValue(
                    "opening is bound to another receipt or key"
                )),
                "{field}"
            );
        }
    }
}

#[test]
fn opening_rejects_another_policys_receipt_and_another_receipts_opening() {
    let policy = policy();
    let other_policy = RamLfePolicyV1 {
        prf_key: RamLfePrfKeyCommitmentV1::commit(b"other").unwrap(),
        ..policy
    };
    let receipt = receipt();
    assert_eq!(
        RamLfeOpeningV1::new(&other_policy, &receipt, output(&[1], 3)),
        Err(RamLfeError::InvalidCanonicalValue(
            "receipt is bound to another policy"
        ))
    );
    // An opening made for one receipt does not open another receipt.
    let other_receipt = RamLfeReceiptV1 {
        beneficiary: account(2),
        ..receipt.clone()
    };
    let opening = opening();
    opening.verify_authority(&policy, &receipt).unwrap();
    assert!(opening.verify_authority(&policy, &other_receipt).is_err());
    assert!(opening.verify_authority(&other_policy, &receipt).is_err());
}

#[test]
fn canonical_model_vectors_are_pinned() {
    assert_eq!(
        [
            hex::encode(context().id().unwrap().as_hash().as_ref()),
            hex::encode(receipt().commitment().unwrap().as_hash().as_ref()),
            hex::encode(opening().commitment().unwrap().as_hash().as_ref()),
        ],
        [
            "0de460773668037e8b4241ec4becd05ae486884589afc126f533593166eb94f9",
            "5e0e07f417d9ec7055433f7aa7ffb9d817068cebc2e7f4c60b5bd038cc43de87",
            "d7d59b3f20cd4efd6c09d2ca1f6441e505ec5d4103fe6885b2e32582f2ea70bb"
        ]
    );
}
