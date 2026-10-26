//! Public holder construction and link coverage against the ordinary library build.
//!
//! Run with default features and with `--no-default-features`, without enabling
//! `privacy-release-evidence`. These shape tests do not claim that their placeholder
//! DER or signatures are authentic; the complete native prover retains that duty.

use iroha_core_privacy::{
    privacy_engines::zk_x509::holder::{
        ZkX509AttributeOpeningV1, ZkX509EngineErrorV1, ZkX509HolderAttemptErrorV1,
        ZkX509HolderAttemptV1, ZkX509PendingCredentialProofV1, ZkX509WitnessCodecErrorV1,
        ZkX509WitnessV1, ownership_challenge_v1, ownership_signing_message_v1,
        verify_zk_x509_credential_proof_v1,
    },
    privacy_state::PrivacyZkX509AuthoritativeStateV1,
};
use iroha_data_model::privacy::{
    IrohaZkX509StarkP256StatementV1, PrivacyConsensusLimitsV1, PrivacyZkX509HolderBudgetV1,
};
use zeroize::Zeroizing;

fn shape_witness(
    index: u16,
    openings: Vec<ZkX509AttributeOpeningV1>,
) -> Result<ZkX509WitnessV1, ZkX509WitnessCodecErrorV1> {
    ZkX509WitnessV1::from_parts_v1(
        vec![vec![0x30, 0], vec![0x30, 0]],
        vec![0x30, 0],
        index,
        [[0x42; 32]; 12],
        [0x24; 64],
        openings,
    )
}

#[test]
fn ordinary_library_exports_the_complete_prover_and_local_verifier() {
    type Prepare = fn(
        IrohaZkX509StarkP256StatementV1,
        ZkX509WitnessV1,
        &PrivacyZkX509AuthoritativeStateV1,
        u64,
        &PrivacyConsensusLimitsV1,
        [u8; 32],
        PrivacyZkX509HolderBudgetV1,
        u64,
    ) -> Result<ZkX509HolderAttemptV1, ZkX509HolderAttemptErrorV1>;
    type Prover = fn(
        ZkX509HolderAttemptV1,
        &PrivacyZkX509AuthoritativeStateV1,
        u64,
        &PrivacyConsensusLimitsV1,
        [u8; 32],
        u64,
        &mut rand::rngs::OsRng,
    ) -> Result<ZkX509PendingCredentialProofV1, ZkX509HolderAttemptErrorV1>;
    type Release =
        fn(
            ZkX509PendingCredentialProofV1,
            &PrivacyZkX509AuthoritativeStateV1,
            u64,
            &PrivacyConsensusLimitsV1,
            [u8; 32],
            u64,
        )
            -> Result<(IrohaZkX509StarkP256StatementV1, Vec<u8>), ZkX509HolderAttemptErrorV1>;
    type Verifier = fn(
        &IrohaZkX509StarkP256StatementV1,
        &PrivacyZkX509AuthoritativeStateV1,
        [u8; 32],
        &[u8],
    ) -> Result<(), ZkX509EngineErrorV1>;
    let prepare: Prepare = ZkX509HolderAttemptV1::prepare;
    let prover: Prover = ZkX509HolderAttemptV1::prove_with_rng::<rand::rngs::OsRng>;
    let release: Release = ZkX509PendingCredentialProofV1::release;
    let verifier: Verifier = verify_zk_x509_credential_proof_v1;
    std::hint::black_box((
        prepare,
        prover,
        release,
        verifier,
        ownership_challenge_v1,
        ownership_signing_message_v1,
    ));
}

#[test]
fn holder_constructs_and_encodes_bounded_inputs_without_a_private_key() {
    let opening = ZkX509AttributeOpeningV1::new_v1(3, [0x17; 32]);
    assert_eq!(
        format!("{opening:?}"),
        "ZkX509AttributeOpeningV1 { index: 3, salt: \"[REDACTED]\" }"
    );
    let witness = shape_witness(4_095, vec![opening]).expect("bounded private witness shape");
    assert_eq!(witness.certificate_chain_len_v1(), 2);
    assert_eq!(format!("{witness:?}"), "ZkX509WitnessV1 { [REDACTED] }");
    let encoded = Zeroizing::new(witness.encode_v1().expect("canonical Norito frame"));
    let header = norito::core::Header::read(encoded.as_slice()).expect("Norito header");
    assert_eq!(header.flags, 0);
    assert_eq!(header.compression, norito::core::Compression::None);
    assert_eq!(
        header.schema,
        norito::schema::identity::frame_hash::<ZkX509WitnessV1>()
    );
}

#[test]
fn holder_rejects_out_of_capacity_ca_indices() {
    assert_eq!(
        shape_witness(4_096, vec![]),
        Err(ZkX509WitnessCodecErrorV1::InvalidCaPathIndex)
    );
}

#[test]
fn holder_rejects_out_of_range_duplicate_and_unsorted_openings() {
    for indices in [vec![4], vec![1, 1], vec![3, 0]] {
        let openings = indices
            .into_iter()
            .map(|index| ZkX509AttributeOpeningV1::new_v1(index, [0x17; 32]))
            .collect();
        assert_eq!(
            shape_witness(0, openings),
            Err(ZkX509WitnessCodecErrorV1::InvalidAttributeOpenings)
        );
    }
}

#[test]
fn holder_rejects_invalid_chain_and_crl_lengths() {
    for (chain, crl, error) in [
        (
            vec![],
            vec![1],
            ZkX509WitnessCodecErrorV1::InvalidChainDepth,
        ),
        (
            vec![vec![], vec![1]],
            vec![1],
            ZkX509WitnessCodecErrorV1::InvalidCertificateLength,
        ),
        (
            vec![vec![1; 4_097], vec![1]],
            vec![1],
            ZkX509WitnessCodecErrorV1::InvalidCertificateLength,
        ),
        (
            vec![vec![1], vec![1]],
            vec![],
            ZkX509WitnessCodecErrorV1::InvalidCrlLength,
        ),
        (
            vec![vec![1], vec![1]],
            vec![1; 4_097],
            ZkX509WitnessCodecErrorV1::InvalidCrlLength,
        ),
    ] {
        assert_eq!(
            ZkX509WitnessV1::from_parts_v1(chain, crl, 0, [[0; 32]; 12], [0; 64], vec![]),
            Err(error)
        );
    }
}
