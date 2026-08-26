// First-release declarations must never inherit a pricing-schedule default.
#[test]
fn storage_class_metadata_is_required() {
    let metadata = Metadata::default();
    let provider = ProviderId::new([0x11; 32]);
    let error = super::storage_class_from_declaration_metadata(provider, &metadata)
        .expect_err("missing storage class must fail closed");
    assert!(matches!(
        error,
        InstructionExecutionError::InvalidParameter(
            InvalidParameterError::SmartContract(message)
        ) if message.contains("must explicitly declare")
    ));
}
