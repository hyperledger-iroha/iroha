// Exact current typed identity capture, produced only by the native test owner.

#[derive(Debug, PartialEq, Eq, norito::derive::JsonSerialize, norito::derive::JsonDeserialize)]
struct CapturedGatewayComplianceIdentity {
    nominal: String,
    frame: String,
    serialize_hash: String,
    deserialize_hash: Option<String>,
}
fn captured_gateway_compliance_serialize<T>() -> CapturedGatewayComplianceIdentity
where
    T: norito::NoritoSchema + norito::NoritoSerialize,
{
    CapturedGatewayComplianceIdentity {
        nominal: T::nominal_name(),
        frame: T::frame_name(),
        serialize_hash: hex::encode(norito::schema::identity::frame_hash::<T>()),
        deserialize_hash: None,
    }
}
fn captured_gateway_compliance_both<T>() -> CapturedGatewayComplianceIdentity
where
    T: norito::NoritoSchema + norito::NoritoSerialize + for<'de> norito::NoritoDeserialize<'de>,
{
    let mut row = captured_gateway_compliance_serialize::<T>();
    row.deserialize_hash = Some(hex::encode(norito::schema::identity::frame_hash::<T>()));
    row
}
fn captured_gateway_compliance_identities() -> Vec<CapturedGatewayComplianceIdentity> {
    let rows = vec![
        captured_gateway_compliance_both::<GatewayComplianceTrustedSignerV1>(),
        captured_gateway_compliance_both::<GatewayComplianceTrustPolicyV1>(),
        captured_gateway_compliance_both::<GatewayComplianceSubjectKindV1>(),
        captured_gateway_compliance_both::<GatewayComplianceBaselineRuleV1>(),
        captured_gateway_compliance_both::<GatewayComplianceAppealOverrideV1>(),
        captured_gateway_compliance_both::<GatewayComplianceLegalSafetyHoldV1>(),
        captured_gateway_compliance_both::<GatewayComplianceToggleV1>(),
        captured_gateway_compliance_both::<GatewayComplianceSourceAnchorV1>(),
        captured_gateway_compliance_both::<GatewayComplianceCatalogPayloadV1>(),
        captured_gateway_compliance_both::<GatewayComplianceCatalogApprovalV1>(),
        captured_gateway_compliance_both::<GatewayComplianceCatalogV1>(),
        captured_gateway_compliance_both::<GatewayComplianceAcknowledgementPayloadV1>(),
        captured_gateway_compliance_both::<GatewayComplianceAcknowledgementV1>(),
        captured_gateway_compliance_both::<GatewayComplianceRollbackPayloadV1>(),
        captured_gateway_compliance_both::<GatewayComplianceRollbackV1>(),
        captured_gateway_compliance_both::<GatewayComplianceFeedDocumentV1>(),
        captured_gateway_compliance_serialize::<GatewayComplianceFeedTransportPolicyDigestV1>(),
        captured_gateway_compliance_serialize::<GatewayComplianceFeedTransportHostDigestV1>(),
    ];
    assert_eq!(rows.len(), 18);
    let mut names = std::collections::BTreeSet::new();
    for row in &rows {
        assert!(names.insert(row.nominal.clone()), "duplicate typed owner");
        assert!(
            row.nominal
                .starts_with("sorafs_manifest::gateway_compliance::")
        );
        assert_eq!(row.nominal, row.frame);
    }
    rows
}
#[test]
fn current_gateway_compliance_identities_match_native_capture() {
    use std::io::Read as _;
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/gateway_compliance_identities.json");
    // Runtime loading lets the compiled native printer supply the reviewed fixture without
    // recompiling or substituting calculated expected hashes. Missing capture is a failure.
    let file = std::fs::File::open(path)
        .expect("run the native identity printer and retain its actual output");
    const MAX: usize = 32 * 1024;
    assert!(file.metadata().unwrap().len() <= MAX as u64);
    let mut bytes = Vec::new();
    file.take(MAX as u64 + 1).read_to_end(&mut bytes).unwrap();
    assert!(bytes.len() <= MAX);
    let expected: Vec<CapturedGatewayComplianceIdentity> =
        norito::json::from_slice(&bytes).expect("captured identity document");
    assert_eq!(expected, captured_gateway_compliance_identities());
}
#[test]
#[ignore = "explicit maintenance capture of current typed compliance protocol owners"]
fn print_gateway_compliance_identity_capture_v1() {
    use std::io::Write as _;
    let body = norito::json::to_json(&captured_gateway_compliance_identities())
        .expect("native identity output");
    assert!(body.len() <= 32 * 1024);
    let mut output = std::io::stdout().lock();
    writeln!(output, "GATEWAY_COMPLIANCE_CODEC_CAPTURE_V1\t{body}").expect("write exact capture");
}
