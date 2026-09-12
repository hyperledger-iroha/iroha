//! Immutable compiler-captured identities for this source owner’s existing codecs.

const CASES: &[crate::captured_schema_tests::Case] = &[
    crate::captured_schema_tests::Case::bidirectional::<super::CanonicalError>(
        "iroha_data_model::errors::CanonicalError",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::CanonicalErrorKind>(
        "iroha_data_model::errors::CanonicalErrorKind",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::DaDeadlineExceeded>(
        "iroha_data_model::errors::DaDeadlineExceeded",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::OracleStale>(
        "iroha_data_model::errors::OracleStale",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::CircuitBreakerActive>(
        "iroha_data_model::errors::CircuitBreakerActive",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::BufferDepletedXorOnly>(
        "iroha_data_model::errors::BufferDepletedXorOnly",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::RwsetUnbounded>(
        "iroha_data_model::errors::RwsetUnbounded",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::AmxTimeout>(
        "iroha_data_model::errors::AmxTimeout",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::AmxLockConflict>(
        "iroha_data_model::errors::AmxLockConflict",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::PvoMissingOrExpired>(
        "iroha_data_model::errors::PvoMissingOrExpired",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::HeavyInstructionDisallowed>(
        "iroha_data_model::errors::HeavyInstructionDisallowed",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SettlementRouterUnavailable>(
        "iroha_data_model::errors::SettlementRouterUnavailable",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::AmxStage>(
        "iroha_data_model::errors::AmxStage",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::CircuitBreakerKind>(
        "iroha_data_model::errors::CircuitBreakerKind",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SettlementRouterOutage>(
        "iroha_data_model::errors::SettlementRouterOutage",
    ),
];

#[test]
fn captured_codec_schema_identities() {
    for case in CASES {
        case.check();
    }
}
