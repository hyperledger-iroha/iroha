//! Regression tests for bounded reader-based KRV4 artifact framing.
use iroha_core::zk::kagemusha_artifact_v4::{
    KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_ARTIFACT_MAGIC_V4,
    write_kagemusha_pasta_cycle_artifact_from_reader_v4, write_kagemusha_pasta_cycle_artifact_v4,
};
use iroha_data_model::offline::{
    KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V4,
    KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V4, KAGEMUSHA_STEP_CIRCUIT_MINIMUM_K_V4,
    KAGEMUSHA_STEP_CIRCUIT_MINIMUM_UNUSABLE_ROWS_V4, KAGEMUSHA_STEP_CIRCUIT_PARAMS_VERSION_V4,
    KAGEMUSHA_STEP_CIRCUIT_RELEASE_ADVICE_COLUMNS_V4,
    KAGEMUSHA_STEP_CIRCUIT_RELEASE_LOOKUP_COLUMNS_V4, KagemushaPastaCycleArtifactKindV4,
    KagemushaPastaCycleParityV1, KagemushaPastaCycleProofProfileV4, KagemushaPastaPublicLayoutV4,
    KagemushaStepCircuitParamsV4,
};
use sha2::{Digest as _, Sha256};
use std::io::{self, Cursor};
fn test_profile() -> KagemushaPastaCycleProofProfileV4 {
    let k = KAGEMUSHA_STEP_CIRCUIT_MINIMUM_K_V4;
    let layout =
        KagemushaPastaPublicLayoutV4::for_ipa_round_count(k).expect("test Kagemusha public layout");
    let circuit_params = KagemushaStepCircuitParamsV4 {
        version: KAGEMUSHA_STEP_CIRCUIT_PARAMS_VERSION_V4,
        k,
        num_advice_per_phase: KAGEMUSHA_STEP_CIRCUIT_RELEASE_ADVICE_COLUMNS_V4.to_vec(),
        num_lookup_advice_per_phase: KAGEMUSHA_STEP_CIRCUIT_RELEASE_LOOKUP_COLUMNS_V4.to_vec(),
        num_fixed: 1,
        lookup_bits: k - 1,
        num_instance_columns: 1,
        public_input_limbs: layout.instance_column_limbs,
        minimum_unusable_rows: KAGEMUSHA_STEP_CIRCUIT_MINIMUM_UNUSABLE_ROWS_V4,
        max_parent_proof_bytes: 4_096,
    };
    KagemushaPastaCycleProofProfileV4 {
        parity: KagemushaPastaCycleParityV1::StepEq,
        circuit_id: KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V4.to_owned(),
        parameter_generation: "test-parameters".to_owned(),
        ipa_k: k,
        circuit_params,
        compiled_protocol_structure_sha256: [0x41; 32],
        step_proof_size_bytes: 4_096,
        artifacts: Vec::new(),
    }
}
#[test]
fn reader_writer_matches_in_memory_frame_and_descriptor() {
    let profile = test_profile();
    let payload = b"reader-framed Kagemusha proving-key fixture";
    let kind = KagemushaPastaCycleArtifactKindV4::ProvingKey;
    let mut expected_frame = Vec::new();
    let expected_descriptor = write_kagemusha_pasta_cycle_artifact_v4(
        &mut expected_frame,
        "test-generation",
        &profile,
        kind,
        payload,
    )
    .expect("write in-memory frame");
    let mut reader = Cursor::new(payload);
    let mut actual_frame = Vec::new();
    let actual_descriptor = write_kagemusha_pasta_cycle_artifact_from_reader_v4(
        &mut actual_frame,
        "test-generation",
        &profile,
        kind,
        &mut reader,
        u64::try_from(payload.len()).expect("small payload"),
        Sha256::digest(payload).into(),
    )
    .expect("write reader frame");
    assert_eq!(actual_frame, expected_frame);
    assert_eq!(actual_descriptor, expected_descriptor);
}
#[test]
fn reader_writer_rejects_early_eof_trailing_bytes_and_digest_mismatch() {
    let profile = test_profile();
    let payload = b"bounded reader fixture";
    let kind = KagemushaPastaCycleArtifactKindV4::ProvingKey;
    let mut early_reader = Cursor::new(payload);
    let early_error = write_kagemusha_pasta_cycle_artifact_from_reader_v4(
        &mut Vec::new(),
        "test-generation",
        &profile,
        kind,
        &mut early_reader,
        u64::try_from(payload.len() + 1).expect("small payload"),
        Sha256::digest(payload).into(),
    )
    .expect_err("short staged payload must fail closed");
    assert!(early_error.contains("ended before its declared length"));
    let declared = &payload[..payload.len() - 1];
    let mut trailing_reader = Cursor::new(payload);
    let trailing_error = write_kagemusha_pasta_cycle_artifact_from_reader_v4(
        &mut Vec::new(),
        "test-generation",
        &profile,
        kind,
        &mut trailing_reader,
        u64::try_from(declared.len()).expect("small payload"),
        Sha256::digest(declared).into(),
    )
    .expect_err("trailing staged payload byte must fail closed");
    assert!(trailing_error.contains("exceeds its declared length"));
    let mut digest_reader = Cursor::new(payload);
    let digest_error = write_kagemusha_pasta_cycle_artifact_from_reader_v4(
        &mut Vec::new(),
        "test-generation",
        &profile,
        kind,
        &mut digest_reader,
        u64::try_from(payload.len()).expect("small payload"),
        [0xA5; 32],
    )
    .expect_err("changed staged payload digest must fail closed");
    assert!(digest_error.contains("digest changed while framing"));
}
struct PartialFailingSink {
    bytes: Vec<u8>,
    remaining: usize,
}
impl io::Write for PartialFailingSink {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.remaining == 0 {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "intentional partial sink failure",
            ));
        }
        let accepted = self.remaining.min(bytes.len());
        self.bytes.extend_from_slice(&bytes[..accepted]);
        self.remaining -= accepted;
        Ok(accepted)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
#[test]
fn reader_writer_enforces_cap_and_propagates_partial_sink_failure() {
    let profile = test_profile();
    let payload = b"sink failure fixture";
    let kind = KagemushaPastaCycleArtifactKindV4::ProvingKey;
    let mut oversized_reader = Cursor::new([]);
    let mut oversized_output = Vec::new();
    let oversized_error = write_kagemusha_pasta_cycle_artifact_from_reader_v4(
        &mut oversized_output,
        "test-generation",
        &profile,
        kind,
        &mut oversized_reader,
        KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V4
            .checked_add(1)
            .expect("artifact cap leaves headroom"),
        [0x5A; 32],
    )
    .expect_err("declared payload above the hard cap must fail closed");
    assert!(oversized_error.contains("profile or payload is invalid"));
    assert!(oversized_output.is_empty());
    let mut sink = PartialFailingSink {
        bytes: Vec::new(),
        remaining: 10,
    };
    let mut reader = Cursor::new(payload);
    let sink_error = write_kagemusha_pasta_cycle_artifact_from_reader_v4(
        &mut sink,
        "test-generation",
        &profile,
        kind,
        &mut reader,
        u64::try_from(payload.len()).expect("small payload"),
        Sha256::digest(payload).into(),
    )
    .expect_err("partial output failure must be propagated");
    assert!(sink_error.contains("failed to write Kagemusha V4 artifact"));
    assert_eq!(sink.bytes.len(), 10);
    assert_eq!(
        &sink.bytes[..KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_ARTIFACT_MAGIC_V4.len()],
        KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_ARTIFACT_MAGIC_V4
    );
}
