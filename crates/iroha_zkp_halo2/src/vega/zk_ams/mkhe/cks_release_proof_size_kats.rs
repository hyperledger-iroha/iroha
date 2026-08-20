// Isolated release-shape CKS proof-size evidence precursor.
//
// This harness emits a candidate digest for an externally contained release
// job. It is not pinned evidence and must not open any resource or wire gate.
// It proves only party 0's per-contribution byte shape against a complete
// synthetic statement; it does not exercise eight-party combination or
// certify the other seven public-key contributions.

use super::*;
use crate::vega::zk_ams::mkhe::{
    active::{ZkAmsMkheActivePartySecretV1, ZkAmsMkheGovernedActiveRosterV1},
    collective::generate_zk_ams_mkhe_collective_party_state_v1,
    manifest::zk_ams_mkhe_resource_certificate_v1,
};

const RELEASE_CKS_PROOF_BYTES_V1: usize = 5_111_863;
const RELEASE_CKS_RECORD_BYTES_V1: usize = 44_958_187;
const RELEASE_CKS_NEGATIVE_CASES_V1: u32 = 6;
const RELEASE_CKS_KAT_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.cks-release-proof-size-candidate";
const COMMON_BINDING_WIRE_BYTES_V1: usize = 114;
const PROOF_ENVELOPE_HEADER_BYTES_V1: usize = 151;

fn rejects_release_record(statement: ZkAmsMkheCksStatementV1<'_>, bytes: &[u8]) -> bool {
    ZkAmsMkheAuthenticatedCksContributionV1::decode_release_wire_exact(statement, 0, bytes).is_err()
}

#[test]
#[ignore = "release-shape CKS proof; run only in the isolated release resource harness"]
fn release_cks_proof_size_kat_emits_candidate_digest() {
    let mut random = KatRandom::new(b"iroha.zk-ams.v1.mkhe.cks-release-proof-size-kat");
    let mut secrets = (0..ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)
        .map(|_| ZkAmsMkheActivePartySecretV1::generate(&mut random).expect("party secret"))
        .collect::<Vec<_>>();
    secrets.sort_by_key(|secret| secret.party().expect("party identity"));
    let secret_refs: [&ZkAmsMkheActivePartySecretV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1] = secrets
        .iter()
        .collect::<Vec<_>>()
        .try_into()
        .expect("exact governed roster");
    let roster = ZkAmsMkheGovernedActiveRosterV1::new(101, secret_refs, &mut random)
        .expect("release governed roster");
    let transcript_digest = keccak256(b"iroha.zk-ams.v1.mkhe.cks-release-proof-size-transcript");
    let (state, share) = generate_zk_ams_mkhe_collective_party_state_v1(
        &roster,
        transcript_digest,
        0,
        &secrets[0],
        &mut random,
    )
    .expect("release party state and public share");
    let wire_roster = roster.to_wire_roster().expect("canonical wire roster");
    let profile = release_profile_v1();
    let coefficient_count = profile
        .ring_degree
        .checked_mul(profile.moduli.len())
        .expect("release coefficient count");
    let source = ZkAmsMkheCksSourceCiphertextV1::new(
        &wire_roster,
        transcript_digest,
        0,
        0,
        0,
        ZkAmsMkheRnsPolynomialWireV1::new(vec![0; coefficient_count])
            .expect("canonical zero source constant"),
        Vec::new(),
    )
    .expect("canonical zero-extended source");
    let target_a = share.public_a().clone();
    let party_public_b: [ZkAmsMkheRnsPolynomialWireV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1] =
        core::array::from_fn(|_| share.party_public_b().clone());
    let party_public_b_refs: [&ZkAmsMkheRnsPolynomialWireV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1] =
        core::array::from_fn(|index| &party_public_b[index]);
    let statement = ZkAmsMkheCksStatementV1::new(
        &wire_roster,
        &source,
        &target_a,
        share.public_a(),
        &party_public_b_refs,
    )
    .expect("release CKS statement");
    let contribution =
        prove_zk_ams_mkhe_cks_contribution_v1(statement, 0, &state, &secrets[0], &mut random)
            .expect("release CKS contribution");
    verify_zk_ams_mkhe_cks_contribution_v1(statement, &contribution)
        .expect("native release CKS verification");

    let proof = contribution
        .canonical_proof_bytes()
        .expect("canonical release proof");
    assert_eq!(proof.len(), RELEASE_CKS_PROOF_BYTES_V1);
    assert_eq!(
        ZkAmsMkheCksProofV1::decode_release_exact(&proof)
            .expect("strict release proof decode")
            .encode()
            .expect("canonical proof re-encode"),
        proof
    );
    let record = contribution
        .to_release_wire(statement)
        .expect("release CKS wire")
        .encode()
        .expect("canonical release CKS record");
    assert_eq!(record.len(), RELEASE_CKS_RECORD_BYTES_V1);
    let decoded =
        ZkAmsMkheAuthenticatedCksContributionV1::decode_release_wire_exact(statement, 0, &record)
            .expect("strict release record decode and verification");
    verify_zk_ams_mkhe_cks_contribution_v1(statement, &decoded)
        .expect("decoded release CKS verification");
    assert_eq!(
        decoded
            .to_release_wire(statement)
            .expect("decoded release wire")
            .encode()
            .expect("decoded release re-encode"),
        record
    );

    let mut rejected = Vec::with_capacity(RELEASE_CKS_NEGATIVE_CASES_V1 as usize);
    rejected.push(rejects_release_record(
        statement,
        &record[..record.len() - 1],
    ));
    {
        let mut mutation = record.clone();
        mutation.push(0);
        rejected.push(rejects_release_record(statement, &mutation));
    }
    {
        let mut mutation = record.clone();
        mutation[0] ^= 1;
        rejected.push(rejects_release_record(statement, &mutation));
    }
    let proof_offset = record
        .len()
        .checked_sub(proof.len())
        .expect("proof suffix offset");
    let envelope = proof_offset
        .checked_sub(PROOF_ENVELOPE_HEADER_BYTES_V1)
        .expect("proof envelope offset");
    assert_eq!(&record[envelope..envelope + 4], b"ZAPE");
    assert_eq!(&record[proof_offset..proof_offset + 4], b"ZACP");
    {
        let mut mutation = record.clone();
        let kind = &mut mutation[envelope + COMMON_BINDING_WIRE_BYTES_V1];
        assert_eq!(*kind, ZkAmsMkheProofKindV1::CksContribution as u8);
        *kind = ZkAmsMkheProofKindV1::RkgContribution as u8;
        rejected.push(rejects_release_record(statement, &mutation));
    }
    {
        let mut mutation = record.clone();
        let seed = &mut mutation[proof_offset + 11];
        *seed = if *seed == 1 { 2 } else { 1 };
        rejected.push(rejects_release_record(statement, &mutation));
    }
    let mut alternate_target_residues = target_a.residues().to_vec();
    alternate_target_residues[0] = if alternate_target_residues[0] == 1 {
        2
    } else {
        1
    };
    let alternate_target = ZkAmsMkheRnsPolynomialWireV1::new(alternate_target_residues)
        .expect("distinct canonical target");
    let alternate_statement = ZkAmsMkheCksStatementV1::new(
        &wire_roster,
        &source,
        &alternate_target,
        share.public_a(),
        &party_public_b_refs,
    )
    .expect("alternate bound statement");
    rejected.push(rejects_release_record(alternate_statement, &record));
    assert_eq!(rejected.len(), RELEASE_CKS_NEGATIVE_CASES_V1 as usize);
    assert!(rejected.iter().all(|rejected| *rejected));

    let evidence = zk_ams_mkhe_cks_resource_evidence_v1().expect("CKS accounting");
    assert_eq!(evidence.proof_payload_bytes, proof.len() as u64);
    assert_eq!(
        evidence.total_contribution_record_bytes,
        record.len() as u64
    );
    assert!(proof.len() <= ZK_AMS_MKHE_MAX_PROOF_BYTES_V1);
    assert!(record.len() <= profile.max_round_bytes);
    let resource = zk_ams_mkhe_resource_certificate_v1().expect("resource certificate");
    assert!(!resource.contribution_proof_sizes_certified);

    let proof_blake3 = norito::streaming::blake3_hash(&proof);
    let record_blake3 = norito::streaming::blake3_hash(&record);
    let mut kat = Keccak256::new();
    kat.update(RELEASE_CKS_KAT_DOMAIN_V1);
    kat.update(&profile.digest().expect("release profile digest"));
    kat.update(&wire_roster.roster_digest());
    kat.update(&transcript_digest);
    kat.update(&evidence.evidence_digest);
    kat.update(&(proof.len() as u64).to_be_bytes());
    kat.update(&proof);
    kat.update(&(record.len() as u64).to_be_bytes());
    kat.update(&record);
    kat.update(&RELEASE_CKS_NEGATIVE_CASES_V1.to_be_bytes());
    for result in rejected {
        kat.update(&[result.into()]);
    }
    let candidate = kat.finalize();
    assert_ne!(candidate, [0; 32]);
    eprintln!(
        concat!(
            "ZK-AMS CKS release proof-size candidate={} proof_blake3={} ",
            "record_blake3={} proof_bytes={} record_bytes={} negatives={}",
        ),
        hex::encode(candidate),
        hex::encode(proof_blake3),
        hex::encode(record_blake3),
        proof.len(),
        record.len(),
        RELEASE_CKS_NEGATIVE_CASES_V1,
    );
}

#[test]
fn release_cks_proof_size_harness_stays_inert_and_capped() {
    let source = include_str!("cks_release_proof_size_kats.rs");
    let harness = source
        .split_once("#[test]\nfn release_cks_proof_size_harness_stays_inert_and_capped()")
        .map(|(harness, _)| harness)
        .expect("bounded release harness");
    let parent = include_str!("cks.rs");
    let resource = include_str!("resource.rs");
    let manifest = include_str!("manifest.rs");
    assert!(source.lines().count() <= 500 && source.len() <= 24 * 1024);
    assert!(harness.contains("#[ignore = \"release-shape CKS proof;"));
    assert!(harness.contains("prove_zk_ams_mkhe_cks_contribution_v1("));
    assert!(harness.contains("decode_release_wire_exact("));
    assert!(harness.contains("release_cks_proof_size_kat_emits_candidate_digest"));
    assert!(parent.contains("mod release_proof_size_kats"));
    assert!(resource.contains("contribution_proof_sizes_certified: false"));
    assert!(manifest.contains("release_kat_digest: [0; 32]"));
    assert!(manifest.contains("wire_gate: false"));
    assert!(!source.contains(concat!("const RELEASE_CKS_PINNED_", "KAT_DIGEST_V1")));
}
