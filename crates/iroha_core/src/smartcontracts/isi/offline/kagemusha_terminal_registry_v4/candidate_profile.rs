//! Candidate-binding artifact profiles for terminal V4 registry tests.
use iroha_data_model::offline::{
    KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_BOOTSTRAP_FILE_NAME_V4,
    KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V4,
    KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_PARAMS_IPA_FILE_NAME_V4,
    KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_PROVING_KEY_FILE_NAME_V4,
    KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_VERIFYING_KEY_FILE_NAME_V4,
    KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_BOOTSTRAP_FILE_NAME_V4,
    KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V4,
    KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_PARAMS_IPA_FILE_NAME_V4,
    KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_PROVING_KEY_FILE_NAME_V4,
    KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_VERIFYING_KEY_FILE_NAME_V4,
    KAGEMUSHA_STEP_CIRCUIT_MINIMUM_K_V4, KAGEMUSHA_STEP_CIRCUIT_MINIMUM_UNUSABLE_ROWS_V4,
    KAGEMUSHA_STEP_CIRCUIT_PARAMS_VERSION_V4, KAGEMUSHA_STEP_CIRCUIT_RELEASE_ADVICE_COLUMNS_V4,
    KAGEMUSHA_STEP_CIRCUIT_RELEASE_LOOKUP_COLUMNS_V4, KagemushaPastaCycleArtifactKindV4,
    KagemushaPastaCycleArtifactV4, KagemushaPastaCycleParityV1, KagemushaPastaCycleProofProfileV4,
    KagemushaPastaPublicLayoutV4, KagemushaStepCircuitParamsV4,
};
fn candidate_binding_artifact(
    kind: KagemushaPastaCycleArtifactKindV4,
    file_name: &str,
    tag: u8,
) -> KagemushaPastaCycleArtifactV4 {
    KagemushaPastaCycleArtifactV4 {
        kind,
        file_name: file_name.to_owned(),
        size_bytes: 128,
        sha256: [tag; 32],
        payload_size_bytes: 64,
        payload_sha256: [tag.wrapping_add(1); 32],
    }
}
pub(super) fn candidate_binding_profile(
    parity: KagemushaPastaCycleParityV1,
    tag: u8,
) -> KagemushaPastaCycleProofProfileV4 {
    let (circuit_id, file_names) = match parity {
        KagemushaPastaCycleParityV1::StepEq => (
            KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V4,
            [
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_PARAMS_IPA_FILE_NAME_V4,
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_PROVING_KEY_FILE_NAME_V4,
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_VERIFYING_KEY_FILE_NAME_V4,
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_BOOTSTRAP_FILE_NAME_V4,
            ],
        ),
        KagemushaPastaCycleParityV1::StepEp => (
            KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V4,
            [
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_PARAMS_IPA_FILE_NAME_V4,
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_PROVING_KEY_FILE_NAME_V4,
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_VERIFYING_KEY_FILE_NAME_V4,
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_BOOTSTRAP_FILE_NAME_V4,
            ],
        ),
    };
    let k = KAGEMUSHA_STEP_CIRCUIT_MINIMUM_K_V4;
    let layout = KagemushaPastaPublicLayoutV4::for_ipa_round_count(k)
        .expect("candidate-binding public layout");
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
    let kinds = [
        KagemushaPastaCycleArtifactKindV4::ParamsIpa,
        KagemushaPastaCycleArtifactKindV4::ProvingKey,
        KagemushaPastaCycleArtifactKindV4::VerifyingKey,
        KagemushaPastaCycleArtifactKindV4::BootstrapWitness,
    ];
    let artifacts = kinds
        .into_iter()
        .zip(file_names)
        .enumerate()
        .map(|(index, (kind, file_name))| {
            candidate_binding_artifact(
                kind,
                file_name,
                tag + u8::try_from(index).expect("four artifact roles fit u8") * 2,
            )
        })
        .collect();
    KagemushaPastaCycleProofProfileV4 {
        parity,
        circuit_id: circuit_id.to_owned(),
        parameter_generation: "candidate-binding-params".to_owned(),
        ipa_k: k,
        circuit_params,
        compiled_protocol_structure_sha256: [tag.wrapping_add(0x40); 32],
        step_proof_size_bytes: 4_096,
        artifacts,
    }
}
