#[test]
fn every_governed_envelope_binding_fails_closed_when_tampered() {
    let (envelope, activation, network_id) = valid_envelope();
    let mutations: [(&str, fn(&mut PrivacyProofEnvelopeV1)); 9] = [
        ("protocol", |value| {
            value.protocol_id = PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1
        }),
        ("parameter-id", |value| value.parameter_id.0[0] ^= 1),
        ("parameter-digest", |value| value.parameter_digest.0[0] ^= 1),
        ("verifier-digest", |value| value.verifier_digest.0[0] ^= 1),
        ("schema-digest", |value| {
            value.statement_schema_digest.0[0] ^= 1
        }),
        ("engine-manifest", |value| {
            value.engine_manifest_digest.0[0] ^= 1
        }),
        ("statement-digest", |value| value.statement_digest.0[0] ^= 1),
        ("proof-system", |value| {
            value.proof_system_id = PrivacyProofSystemIdV1::StarkFriSha256Goldilocks
        }),
        ("engine", |value| {
            value.engine_id = PrivacyEngineIdV1::NativeGoldilocksStarkFri
        }),
    ];
    for (label, mutate) in mutations {
        let mut candidate = envelope.clone();
        mutate(&mut candidate);
        assert_rejected(&candidate, &activation, &network_id, label);
    }
}

#[test]
fn every_statement_context_artifact_binding_fails_closed_when_tampered() {
    let (envelope, activation, network_id) = valid_envelope();
    let mutations: [(&str, fn(&mut PrivacyStatementContextV1)); 5] = [
        ("statement-parameter-id", |context| {
            context.parameter_id.0[0] ^= 1
        }),
        ("statement-parameter-digest", |context| {
            context.parameter_digest.0[0] ^= 1
        }),
        ("statement-verifier-digest", |context| {
            context.verifier_digest.0[0] ^= 1
        }),
        ("statement-schema-digest", |context| {
            context.statement_schema_digest.0[0] ^= 1
        }),
        ("statement-engine-manifest", |context| {
            context.engine_manifest_digest.0[0] ^= 1
        }),
    ];
    for (label, mutate) in mutations {
        let mut candidate = envelope.clone();
        mutate(&mut verange_statement_mut(&mut candidate).context);
        refresh_statement_digest(&mut candidate);
        assert_rejected(&candidate, &activation, &network_id, label);
    }
}
