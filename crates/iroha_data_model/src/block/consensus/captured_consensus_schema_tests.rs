//! Immutable compiler-captured identities for this source owner’s existing codecs.

const CASES: &[crate::captured_schema_tests::Case] = &[
    crate::captured_schema_tests::Case::bidirectional::<super::ConsensusGenesisParams>(
        "iroha_data_model::block::consensus::ConsensusGenesisParams",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ConsensusGenesisModeParams>(
        "iroha_data_model::block::consensus::ConsensusGenesisModeParams",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::NposGenesisParams>(
        "iroha_data_model::block::consensus::NposGenesisParams",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SumeragiLaneCommitment>(
        "iroha_data_model::block::consensus::SumeragiLaneCommitment",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SumeragiDataspaceCommitment>(
        "iroha_data_model::block::consensus::SumeragiDataspaceCommitment",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::LaneSettlementReceipt>(
        "iroha_data_model::block::consensus::LaneSettlementReceipt",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::NexusFeeScheduleInputs>(
        "iroha_data_model::block::consensus::NexusFeeScheduleInputs",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::NexusFeeReceipt>(
        "iroha_data_model::block::consensus::NexusFeeReceipt",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::LaneLiquidityProfile>(
        "iroha_data_model::block::consensus::LaneLiquidityProfile",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::LaneVolatilityClass>(
        "iroha_data_model::block::consensus::LaneVolatilityClass",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::LaneSwapMetadata>(
        "iroha_data_model::block::consensus::LaneSwapMetadata",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SumeragiRuntimeUpgradeHook>(
        "iroha_data_model::block::consensus::SumeragiRuntimeUpgradeHook",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SumeragiLaneGovernance>(
        "iroha_data_model::block::consensus::SumeragiLaneGovernance",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SumeragiNposDiagnostics>(
        "iroha_data_model::block::consensus::SumeragiNposDiagnostics",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SumeragiPipelineExecutionStatus>(
        "iroha_data_model::block::consensus::SumeragiPipelineExecutionStatus",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SumeragiDiagnosticsStatus>(
        "iroha_data_model::block::consensus::SumeragiDiagnosticsStatus",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ExecKv>(
        "iroha_data_model::block::consensus::ExecKv",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ExecWitness>(
        "iroha_data_model::block::consensus::ExecWitness",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ExecWitnessMsg>(
        "iroha_data_model::block::consensus::ExecWitnessMsg",
    ),
];

#[test]
fn captured_codec_schema_identities() {
    for case in CASES {
        case.check();
    }
}

crate::captured_schema_tests::native_capture::owner_printer!(CASES);
