//! Shape, framing and digest tests of the native-suffix Unload carriage. Fixture proofs are
//! stand-ins; these tests establish canonical layout and bounds, never proof validity.
use super::*;

use crate::kagemusha::kagemusha_wallet_v1::{
    KagemushaWalletPackageV1, KagemushaWalletRecoveryBodyV1, KagemushaWalletUnloadClaimV1,
};

fn fixture<T>(name: &str) -> T
where
    T: norito::NoritoSerialize,
    for<'de> T: norito::NoritoDeserialize<'de>,
{
    let values: norito::json::Value = norito::json::from_str(include_str!(
        "../../../../fixtures/kagemusha/wallet_v1_vectors.json"
    ))
    .unwrap();
    let row = values["objects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|value| value["type"].as_str() == Some(name))
        .unwrap();
    let bytes = hex::decode(row["canonical_hex"].as_str().unwrap()).unwrap();
    norito::decode_canonical_with_limits(&bytes, norito::canonical_decode_limits(bytes.len()))
        .unwrap()
}

/// A header over the fixture's charged Unload claim: the claim's statement, σ, Ω(pred) as the
/// adjusted tip, account and charge, with three steps in two chunks after the anchor.
fn header() -> KagemushaWalletSuffixHeaderV1 {
    let claim: KagemushaWalletUnloadClaimV1 = fixture("KagemushaWalletUnloadClaimV1");
    let statement = claim.package.statement;
    let adjusted = claim.package.lineage.lineage().expect("Ω(pred)").public;
    let tip_sequence = statement.sequence - 1;
    KagemushaWalletSuffixHeaderV1 {
        version: KAGEMUSHA_WALLET_VERSION_V1,
        method: KAGEMUSHA_WALLET_SUFFIX_METHOD_NATIVE_V1,
        network_id: [7; 32],
        scheme_id: statement.scheme_id,
        wallet_id: adjusted.wallet_id,
        verifier_definition: [8; 32],
        artifact_manifest: [9; 32],
        source_capsule: [10; 32],
        request_id: [11; 32],
        anchor_digest: [12; 32],
        anchor_sequence: tip_sequence - 3,
        step_count: 3,
        chunk_count: 2,
        stream_bytes: 300,
        chunk_digests: vec![[13; 32], [14; 32]],
        tip: statement.predecessor,
        tip_sequence,
        tip_adjusted: adjusted,
        statement,
        sigma: claim.package.step_proof,
        recovery_witness_digest: [15; 32],
        account: claim.account,
        charge: claim.charge,
    }
}

#[test]
fn header_roundtrips_and_binds_its_authority() {
    let header = header();
    header.validate().unwrap();
    let frame = header.to_canonical_bytes().unwrap();
    assert!(frame.len() <= KAGEMUSHA_WALLET_SUFFIX_HEADER_MAX_BYTES_V1);
    let decoded =
        KagemushaWalletSuffixHeaderV1::decode_canonical(&frame, &header.scheme_id).unwrap();
    assert_eq!(decoded, header);
    assert!(KagemushaWalletSuffixHeaderV1::decode_canonical(&frame, &[1; 32]).is_err());
    let authority = kagemusha_wallet_suffix_authority_digest_v1(&frame).unwrap();
    assert_ne!(authority, [0; 32]);
    assert_eq!(
        authority,
        kagemusha_wallet_suffix_authority_digest_v1(&frame).unwrap()
    );
    let mut other = header.clone();
    other.chunk_digests[1] = [16; 32];
    let other = other.to_canonical_bytes().unwrap();
    assert_ne!(
        kagemusha_wallet_suffix_authority_digest_v1(&other).unwrap(),
        authority
    );
    let terms = header.payout_terms().unwrap();
    assert_eq!(terms.amount, terms.account_payout + terms.online_charge);
    assert_ne!(terms.beneficiary_account_digest, [0; 32]);
    assert_eq!(header.finalize_ordinal().unwrap(), 3);
    assert_eq!(header.chunk_digest(1), Some([13; 32]));
    assert_eq!(header.chunk_digest(2), Some([14; 32]));
    assert_eq!(header.chunk_digest(0), None);
    assert_eq!(header.chunk_digest(3), None);
}

#[test]
fn header_rejects_partition_sequence_and_charge_errors() {
    let reject = |edit: &dyn Fn(&mut KagemushaWalletSuffixHeaderV1)| {
        let mut header = header();
        edit(&mut header);
        assert!(header.validate().is_err(), "accepted {header:?}");
    };
    reject(&|h| h.method = 2);
    reject(&|h| h.version = 2);
    reject(&|h| h.request_id = [0; 32]);
    reject(&|h| h.chunk_count = 3);
    reject(&|h| h.chunk_digests.push([1; 32]));
    reject(&|h| h.chunk_digests[0] = [0; 32]);
    reject(&|h| {
        h.step_count = 1;
        h.anchor_sequence += 2;
    });
    // More than eight steps per chunk: refused by the partition rule before any sequence.
    reject(&|h| h.step_count = 17);
    reject(&|h| h.anchor_sequence += 1);
    reject(&|h| h.tip_sequence += 1);
    reject(&|h| h.stream_bytes = 14);
    reject(&|h| h.tip = h.statement.successor);
    reject(&|h| h.tip_adjusted.burned_total += 1);
    reject(&|h| h.charge = KagemushaWalletUnloadChargeV1::None);
    reject(&|h| h.sigma.bytes.clear());
    // One chunk may hold every step.
    let mut header = header();
    header.chunk_count = 1;
    header.chunk_digests.truncate(1);
    header.validate().unwrap();
}

#[test]
fn chunk_stream_frames_complete_steps_only() {
    let frames = vec![vec![1, 2, 3], vec![4; 70], vec![5]];
    let data = KagemushaWalletSuffixChunkV1::stream(&frames).unwrap();
    let chunk = KagemushaWalletSuffixChunkV1 {
        version: KAGEMUSHA_WALLET_VERSION_V1,
        chunk_index: 1,
        stream_offset: 300,
        start: header().tip_adjusted,
        data,
    };
    chunk.validate().unwrap();
    let parsed = chunk.step_frames().unwrap();
    assert_eq!(parsed, frames.iter().map(Vec::as_slice).collect::<Vec<_>>());
    let frame = chunk.to_canonical_bytes().unwrap();
    let decoded =
        KagemushaWalletSuffixChunkV1::decode_canonical(&frame, &chunk.start.scheme_id).unwrap();
    assert_eq!(decoded, chunk);
    let digest =
        kagemusha_wallet_suffix_digest_v1(KagemushaWalletSuffixDigestRoleV1::Chunk, &frame);
    assert_ne!(
        digest,
        kagemusha_wallet_suffix_digest_v1(KagemushaWalletSuffixDigestRoleV1::Anchor, &frame)
    );
    for data in [
        Vec::new(),
        vec![0, 0, 0, 0],
        vec![3, 0, 0, 0, 1, 2],
        vec![1, 0, 0],
        {
            let mut data = 65_537_u32.to_le_bytes().to_vec();
            data.resize(65_541, 0);
            data
        },
    ] {
        let chunk = KagemushaWalletSuffixChunkV1 {
            data,
            ..chunk.clone()
        };
        assert!(chunk.validate().is_err());
    }
    let too_many = vec![vec![1]; KAGEMUSHA_WALLET_SUFFIX_CHUNK_STEPS_MAX_V1 + 1];
    assert!(KagemushaWalletSuffixChunkV1::stream(&too_many).is_err());
    let mut data = Vec::new();
    for _ in 0..=KAGEMUSHA_WALLET_SUFFIX_CHUNK_STEPS_MAX_V1 {
        data.extend_from_slice(&1_u32.to_le_bytes());
        data.push(9);
    }
    assert!(
        KagemushaWalletSuffixChunkV1 { data, ..chunk }
            .validate()
            .is_err()
    );
}

/// A Receive step from the fixture's Receive package and recovery body: its statement, σ,
/// receipt and successor state, with a predecessor one sequence earlier.
fn receive_step() -> KagemushaWalletSuffixStepV1 {
    let package: KagemushaWalletPackageV1 = fixture("KagemushaWalletPackageV1");
    let body: KagemushaWalletRecoveryBodyV1 = fixture("KagemushaWalletRecoveryBodyV1");
    let after = body.successor_state;
    let mut before = after;
    before.core.sequence -= 1;
    let mut statement = package.statement;
    statement.sequence = after.core.sequence;
    let originals = KagemushaWalletSuffixOriginalRoleV1::ALL
        .into_iter()
        .map(|role| KagemushaWalletSuffixOriginalV1 {
            role,
            bytes: vec![1; 64],
        })
        .collect();
    KagemushaWalletSuffixStepV1 {
        version: KAGEMUSHA_WALLET_VERSION_V1,
        kind: KagemushaWalletSuffixStepKindV1::Receive {
            credit: KagemushaWalletSuffixCreditWitnessV1::Inserted {
                low: vec![0; LOW_LEAF_OPENING_BYTES],
                empty: vec![0; EMPTY_SLOT_OPENING_BYTES],
            },
        },
        before,
        after,
        statement,
        sigma: package.step_proof,
        receipt: package.receipt,
        map_openings: vec![
            vec![0; LOW_LEAF_OPENING_BYTES],
            vec![0; EMPTY_SLOT_OPENING_BYTES],
        ],
        originals,
    }
}

#[test]
fn step_shape_requires_exact_originals_and_openings() {
    let step = receive_step();
    step.validate().unwrap();
    let frame = step.to_canonical_bytes().unwrap();
    assert!(frame.len() <= KAGEMUSHA_WALLET_SUFFIX_STEP_MAX_BYTES_V1);
    let decoded =
        KagemushaWalletSuffixStepV1::decode_canonical(&frame, &step.statement.scheme_id).unwrap();
    assert_eq!(decoded, step);
    assert_eq!(
        step.original(KagemushaWalletSuffixOriginalRoleV1::Payment),
        Some(&[1; 64][..])
    );
    let reject = |edit: &dyn Fn(&mut KagemushaWalletSuffixStepV1)| {
        let mut step = receive_step();
        edit(&mut step);
        assert!(step.validate().is_err());
    };
    reject(&|s| s.originals.swap(0, 1));
    reject(&|s| {
        s.originals.pop();
    });
    reject(&|s| s.originals[2].bytes = vec![1; KAGEMUSHA_WALLET_CREDENTIAL_MAX_BYTES_V1 + 1]);
    reject(&|s| s.originals[0].bytes.clear());
    reject(&|s| s.map_openings.reverse());
    reject(&|s| s.map_openings.truncate(1));
    reject(&|s| {
        s.kind = KagemushaWalletSuffixStepKindV1::Receive {
            credit: KagemushaWalletSuffixCreditWitnessV1::Present {
                membership: vec![0; EMPTY_SLOT_OPENING_BYTES],
            },
        }
    });
    reject(&|s| s.before.core.sequence -= 1);
    reject(&|s| s.statement.sequence += 1);
    // A Receive statement is never a prior Unload.
    reject(&|s| {
        s.originals.clear();
        s.kind = KagemushaWalletSuffixStepKindV1::PriorUnload {
            proof_digest: [1; 32],
            charge: None,
        }
    });
}

#[test]
fn digests_are_domain_separated_and_payout_source_binds_both_inputs() {
    let roles = [
        KagemushaWalletSuffixDigestRoleV1::Anchor,
        KagemushaWalletSuffixDigestRoleV1::Chunk,
        KagemushaWalletSuffixDigestRoleV1::Unload,
        KagemushaWalletSuffixDigestRoleV1::PayoutSource,
        KagemushaWalletSuffixDigestRoleV1::VerifierDefinition,
    ];
    let digests: std::collections::BTreeSet<_> = roles
        .iter()
        .map(|role| kagemusha_wallet_suffix_digest_v1(*role, b"same body"))
        .collect();
    assert_eq!(digests.len(), roles.len());
    let source = kagemusha_wallet_suffix_payout_source_v1(&[1; 32], &[3; 32]);
    assert_ne!(
        source,
        kagemusha_wallet_suffix_payout_source_v1(&[2; 32], &[3; 32])
    );
    assert_ne!(
        source,
        kagemusha_wallet_suffix_payout_source_v1(&[1; 32], &[4; 32])
    );
    assert_eq!(
        kagemusha_wallet_suffix_verifier_definition_v1(b"pack"),
        kagemusha_wallet_suffix_digest_v1(
            KagemushaWalletSuffixDigestRoleV1::VerifierDefinition,
            b"pack"
        )
    );
    // Pin the construction so Native, Kotlin and Swift reproduce it exactly.
    let mut expected = Sha256::new();
    expected.update(b"iroha:kagemusha:wallet:suffix-unload:v1:chunk\0");
    expected.update(3_u64.to_le_bytes());
    expected.update(b"abc");
    let expected: [u8; 32] = expected.finalize().into();
    assert_eq!(
        kagemusha_wallet_suffix_digest_v1(KagemushaWalletSuffixDigestRoleV1::Chunk, b"abc"),
        expected
    );
    assert_eq!(
        KAGEMUSHA_WALLET_SUFFIX_AUTHORITY_DOMAIN_V1,
        u64::from_le_bytes(*b"kgwsufa1")
    );
}

#[test]
fn unload_binds_credential_account_certificates_and_successor() {
    let claim: KagemushaWalletUnloadClaimV1 = fixture("KagemushaWalletUnloadClaimV1");
    let header = header();
    let unload = KagemushaWalletSuffixUnloadV1 {
        version: KAGEMUSHA_WALLET_VERSION_V1,
        credential: claim.credential,
        certificates: claim.certificates.clone(),
        after: fixture::<KagemushaWalletRecoveryBodyV1>("KagemushaWalletRecoveryBodyV1")
            .successor_state,
        receipt: claim.package.receipt,
        map_openings: vec![
            vec![0; LOW_LEAF_OPENING_BYTES],
            vec![0; EMPTY_SLOT_OPENING_BYTES],
        ],
    };
    unload.validate().unwrap();
    let frame = unload.to_canonical_bytes().unwrap();
    assert!(frame.len() <= KAGEMUSHA_WALLET_SUFFIX_UNLOAD_MAX_BYTES_V1);
    assert_eq!(
        KagemushaWalletSuffixUnloadV1::decode_canonical(&frame, &header.scheme_id).unwrap(),
        unload
    );
    // The fixture's state is not the Unload successor: binding stops exactly there.
    assert!(matches!(
        unload.require_header(&header),
        Err(KagemushaWalletValidationErrorV1::InvalidField {
            field: "suffix_unload.after"
        })
    ));
    let mut missing_signer = unload.clone();
    missing_signer.certificates =
        KagemushaWalletCertificateSetV1::new(vec![claim.certificates.certificates[0].clone()])
            .unwrap();
    assert!(matches!(
        missing_signer.require_header(&header),
        Err(KagemushaWalletValidationErrorV1::InvalidField {
            field: "suffix.certificates.set"
        })
    ));
    let mut other_account = header.clone();
    other_account.account = header.charge.beneficiary().unwrap().clone();
    assert!(matches!(
        unload.require_header(&other_account),
        Err(KagemushaWalletValidationErrorV1::InvalidField {
            field: "suffix_unload.account"
        })
    ));
}
