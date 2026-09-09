//! Captured frames for generated privacy and spentness digest carriers.

use iroha_data_model::{
    confidential::spentness::{
        ConfidentialSpentnessCheckpointDigestV1, ConfidentialSpentnessRootV1,
    },
    privacy::*,
};
use norito::{
    NoritoDeserialize, NoritoSerialize,
    json::{self, JsonDeserialize, JsonSerialize, Value},
};
use std::{collections::BTreeMap, fmt::Debug};

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut encoded = String::new();
    for byte in bytes {
        write!(encoded, "{byte:02x}").expect("String formatting");
    }
    encoded
}

fn frame<T>(value: &T) -> String
where
    T: NoritoSerialize + for<'de> NoritoDeserialize<'de> + Debug + PartialEq,
{
    let bytes = norito::to_bytes(value).expect("encode generated privacy frame");
    let decoded: T = norito::decode_from_bytes(&bytes).expect("decode generated privacy frame");
    assert_eq!(&decoded, value);
    let header = norito::core::Header::read(bytes.as_slice()).expect("read frame header");
    assert_eq!(header.schema, norito::schema::identity::frame_hash::<T>());
    let mut wrong_schema = bytes.clone();
    wrong_schema[6] ^= 1;
    assert!(matches!(
        norito::decode_from_bytes::<T>(&wrong_schema),
        Err(norito::Error::SchemaMismatch)
    ));
    hex(&bytes)
}

fn record<T>(values: impl IntoIterator<Item = T>) -> Value
where
    T: NoritoSerialize
        + for<'de> NoritoDeserialize<'de>
        + norito::NoritoSchema
        + JsonSerialize
        + JsonDeserialize
        + Clone
        + Debug
        + PartialEq,
{
    let identity_hash = norito::schema::identity::frame_hash::<T>();
    assert_eq!(T::frame_name(), T::nominal_name());
    let cases = values
        .into_iter()
        .map(|value| {
            let json = json::to_value(&value).expect("generated privacy JSON");
            let decoded: T = json::from_value(json.clone()).expect("decode generated privacy JSON");
            assert_eq!(decoded, value);
            json::object([
                ("json", json),
                ("frame", Value::String(frame(&value))),
                ("vector_frame", Value::String(frame(&vec![value.clone()]))),
                ("option_frame", Value::String(frame(&Some(value.clone())))),
                (
                    "map_frame",
                    Value::String(frame(&BTreeMap::from([(7_u8, value)]))),
                ),
            ])
            .expect("privacy case")
        })
        .collect();
    json::object([
        ("nominal", Value::String(T::nominal_name())),
        ("serialize_hash", Value::String(hex(&identity_hash))),
        ("deserialize_hash", Value::String(hex(&identity_hash))),
        ("cases", Value::Array(cases)),
    ])
    .expect("privacy type")
}

fn generated_privacy_carriers() -> Vec<Value> {
    let mut rows = Vec::new();
    // These are byte-carrier fixtures. Ristretto point validity and the nonzero
    // admission rules are enforced by their protocol owners, not these wrappers.
    macro_rules! bytes32 {
        ($ty:ty) => {
            rows.push(record([
                <$ty>::new([0; 32]),
                <$ty>::new([0xff; 32]),
                <$ty>::new([0x42; 32]),
                <$ty>::new(core::array::from_fn(|i| {
                    u8::try_from(i).expect("byte index")
                })),
            ]));
        };
    }
    macro_rules! digest384 {
        ($ty:ty) => {
            rows.push(record(
                [
                    [0; 6],
                    [1; 6],
                    [fastpq_isi::poseidon::FIELD_MODULUS - 1; 6],
                    [0, 1, 2, 3, 4, 5],
                ]
                .map(|words| {
                    <$ty>::from_digest(
                        GoldilocksDigest384V1::new(words).expect("canonical field lanes"),
                    )
                }),
            ));
        };
    }
    digest384!(ConfidentialSpentnessCheckpointDigestV1);
    digest384!(ConfidentialSpentnessRootV1);
    bytes32!(PrivacyActionDigestV1);
    bytes32!(PrivacyAttributeDigestV1);
    bytes32!(PrivacyAuditBundleDigestV1);
    bytes32!(PrivacyAuthorizationKeyDigestV1);
    bytes32!(PrivacyBootleLanternIssuerPolicyDigestV1);
    bytes32!(PrivacyCertificateKeyDigestV1);
    bytes32!(PrivacyChallengeV1);
    bytes32!(PrivacyCommitmentV1);
    bytes32!(PrivacyEncryptionKeyV1);
    bytes32!(PrivacyEngineManifestDigestV1);
    bytes32!(PrivacyExact12CapabilityManifestDigestV1);
    bytes32!(PrivacyExact12DeploymentQualificationDigestV1);
    bytes32!(PrivacyExact12ReleaseManifestDigestV1);
    bytes32!(PrivacyFcmpKeyImageV1);
    bytes32!(PrivacyFcmpOutputIdV1);
    bytes32!(PrivacyIssuerIdV1);
    bytes32!(PrivacyNativeConsensusBindingDigestV1);
    bytes32!(PrivacyNoteEncryptionKeyDigestV1);
    bytes32!(PrivacyNullifierV1);
    bytes32!(PrivacyOrchardPoolBootstrapDigestV1);
    bytes32!(PrivacyParameterDigestV1);
    bytes32!(PrivacyParameterIdV1);
    bytes32!(PrivacyPgcAccountBootstrapDigestV1);
    bytes32!(PrivacyPgcBootstrapProofDigestV1);
    bytes32!(PrivacyPolicyDigestV1);
    bytes32!(PrivacyPolicyIdV1);
    bytes32!(PrivacyPoolIdV1);
    bytes32!(PrivacyProgramIdV1);
    bytes32!(PrivacyProofManagedPoolBootstrapDigestV1);
    bytes32!(PrivacyRecipientIdV1);
    bytes32!(PrivacyReleaseArtifactDigestV1);
    bytes32!(PrivacyRootPublicationDigestV1);
    bytes32!(PrivacyRootV1);
    bytes32!(PrivacySecurityClaimDigestV1);
    bytes32!(PrivacySecurityReductionDigestV1);
    bytes32!(PrivacySessionTranscriptDigestV1);
    bytes32!(PrivacyStatementDigestV1);
    bytes32!(PrivacyStatementSchemaDigestV1);
    bytes32!(PrivacyTransactionIntentDigestV1);
    bytes32!(PrivacyVegaDeviceAuthenticationDigestV1);
    bytes32!(PrivacyVegaIssuerRecordDigestV1);
    bytes32!(PrivacyVerifierDigestV1);
    bytes32!(PrivacyX509CrlDerDigestV1);
    bytes32!(PrivacyX509CrlIssuerSpkiDigestV1);
    bytes32!(PrivacyX509TrustStoreDigestV1);
    digest384!(PrivacyZkAceIdentityCommitmentV1);
    bytes32!(PrivacyZkAcePolicyRecordDigestV1);
    digest384!(PrivacyZkAceReplayNullifierV1);
    bytes32!(PrivacyZkAmsCredentialNonceV1);
    bytes32!(PrivacyZkAmsIssuerPolicyRecordDigestV1);
    bytes32!(PrivacyZkAmsKeyImageV1);
    bytes32!(PrivacyZkAmsPhcHashV1);
    bytes32!(PrivacyZkAmsRegistryBootstrapDigestV1);
    bytes32!(PrivacyZkAmsRegistryIdV1);
    bytes32!(PrivacyZkAmsRegistryRecordDigestV1);
    bytes32!(PrivacyZkAmsSeedPublicKeyV1);
    bytes32!(PrivacyZkAmsSubjectCommitmentV1);
    bytes32!(PrivacyZkX509CertificatePolicyRecordDigestV1);
    bytes32!(PrivacyZkX509CrlRecordDigestV1);
    bytes32!(PrivacyZkX509TrustAnchorRecordDigestV1);
    rows
}

#[test]
fn generated_privacy_carriers_preserve_captured_frames() {
    let rows = generated_privacy_carriers();
    assert_eq!(rows.len(), 62);
    assert_eq!(
        rows.iter()
            .map(|row| row.get("cases").unwrap().as_array().unwrap().len())
            .sum::<usize>(),
        248
    );
    let captured: Value = json::from_str(include_str!(
        "fixtures/privacy_generated_identity_frames.json"
    ))
    .expect("immutable generated privacy capture");
    super::fixture_json::assert_json_matches(
        &captured,
        &Value::Array(rows),
        "generated privacy identities",
    );
}
