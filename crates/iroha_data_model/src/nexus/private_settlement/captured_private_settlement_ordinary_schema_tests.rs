//! Exact compiler-captured identities for unchanged ordinary codec declarations.
//!
//! These checks preserve nominal/root identity and both codec directions.
//! Full payload and feature qualification remains with the owning runtime suites.

fn check<T>(nominal: &str, serialize_hash: &str, deserialize_hash: &str)
where
    T: norito::NoritoSchema + norito::NoritoSerialize + for<'de> norito::NoritoDeserialize<'de>,
{
    let parse = |value: &str| -> [u8; 16] {
        hex::decode(value)
            .expect("captured hexadecimal schema hash")
            .try_into()
            .expect("captured schema hash has sixteen bytes")
    };
    let serialize_hash = parse(serialize_hash);
    let deserialize_hash = parse(deserialize_hash);
    assert_eq!(T::nominal_name(), nominal);
    assert_eq!(T::frame_name(), nominal);
    assert_eq!(norito::schema::identity::frame_hash::<T>(), serialize_hash);
    assert_eq!(
        norito::schema::identity::frame_hash::<T>(),
        deserialize_hash
    );
}

// These noncapturing cases keep the literal capture order in static storage.
const CASES: &[fn()] = &[
    || {
        check::<super::PrivateSettlementRouteV1>(
            "iroha_data_model::nexus::private_settlement::PrivateSettlementRouteV1",
            "b71972a320201e71eef75110f733c4d8",
            "b71972a320201e71eef75110f733c4d8",
        )
    },
    || {
        check::<super::PrivateSettlementAssetBindingMaterialV1>(
            "iroha_data_model::nexus::private_settlement::PrivateSettlementAssetBindingMaterialV1",
            "2d17f147b7847c053d2417f81a894e94",
            "2d17f147b7847c053d2417f81a894e94",
        )
    },
    || {
        check::<super::PrivateSettlementLegCommitmentV1>(
            "iroha_data_model::nexus::private_settlement::PrivateSettlementLegCommitmentV1",
            "af7d660707346aeb820ff1912c8a03e8",
            "af7d660707346aeb820ff1912c8a03e8",
        )
    },
    || {
        check::<super::PrivateSettlementBundleIdMaterialV1>(
            "iroha_data_model::nexus::private_settlement::PrivateSettlementBundleIdMaterialV1",
            "0c264fa43ca48458e43781b267a52d09",
            "0c264fa43ca48458e43781b267a52d09",
        )
    },
    || {
        check::<super::PrivateSettlementBundleLegMaterialV1>(
            "iroha_data_model::nexus::private_settlement::PrivateSettlementBundleLegMaterialV1",
            "fa3c5eca8ed78a7d56e8b950f65a5481",
            "fa3c5eca8ed78a7d56e8b950f65a5481",
        )
    },
    || {
        check::<super::PrivateSettlementProofBindingMaterialV1>(
            "iroha_data_model::nexus::private_settlement::PrivateSettlementProofBindingMaterialV1",
            "104b8491ba998056a2b97fd44efb74ed",
            "104b8491ba998056a2b97fd44efb74ed",
        )
    },
    || {
        check::<super::AtomicPrivateSettlementV1>(
            "iroha_data_model::nexus::private_settlement::AtomicPrivateSettlementV1",
            "706f3ebb37cf440e4436f966cc09e06c",
            "706f3ebb37cf440e4436f966cc09e06c",
        )
    },
    || {
        check::<super::PrivateSettlementProofProfileV1>(
            "iroha_data_model::nexus::private_settlement::PrivateSettlementProofProfileV1",
            "02fe48844fcb00e4be6ff00496d8d25f",
            "02fe48844fcb00e4be6ff00496d8d25f",
        )
    },
    || {
        check::<super::PrivateSettlementDeltaV1>(
            "iroha_data_model::nexus::private_settlement::PrivateSettlementDeltaV1",
            "7c60160f72d8d99396407f7d1e3530da",
            "7c60160f72d8d99396407f7d1e3530da",
        )
    },
    || {
        check::<super::PrivateSettlementCapsulePaddingV1>(
            "iroha_data_model::nexus::private_settlement::PrivateSettlementCapsulePaddingV1",
            "0e304b176bb4e9d33ed796aa9fbbf71d",
            "0e304b176bb4e9d33ed796aa9fbbf71d",
        )
    },
    || {
        check::<super::PrivateSettlementAuditOutputRoleV1>(
            "iroha_data_model::nexus::private_settlement::PrivateSettlementAuditOutputRoleV1",
            "23dd003d8c5e71695ebf8b057a9dca0a",
            "23dd003d8c5e71695ebf8b057a9dca0a",
        )
    },
    || {
        check::<super::PrivateSettlementAuditPayerInputV1>(
            "iroha_data_model::nexus::private_settlement::PrivateSettlementAuditPayerInputV1",
            "a7d730f232de6b33e9321d0a522dd8cc",
            "a7d730f232de6b33e9321d0a522dd8cc",
        )
    },
    || {
        check::<super::PrivateSettlementAuditPayerAuthorizationBodyV1>(
            "iroha_data_model::nexus::private_settlement::PrivateSettlementAuditPayerAuthorizationBodyV1",
            "0ecb6b972456053419b8020229e90246",
            "0ecb6b972456053419b8020229e90246",
        )
    },
    || {
        check::<super::PrivateSettlementAuditPayerSignatureV1>(
            "iroha_data_model::nexus::private_settlement::PrivateSettlementAuditPayerSignatureV1",
            "958714376efd9e319169eb70b0f69c28",
            "958714376efd9e319169eb70b0f69c28",
        )
    },
    || {
        check::<super::PrivateSettlementAuditPayerAuthorizationV1>(
            "iroha_data_model::nexus::private_settlement::PrivateSettlementAuditPayerAuthorizationV1",
            "e75e25c44e1ec06b07cf772060c408a0",
            "e75e25c44e1ec06b07cf772060c408a0",
        )
    },
    || {
        check::<super::PrivateSettlementAuditViewKeyAuthorizationBodyV1>(
            "iroha_data_model::nexus::private_settlement::PrivateSettlementAuditViewKeyAuthorizationBodyV1",
            "c0e132c310218967a2680a2e0e27317c",
            "c0e132c310218967a2680a2e0e27317c",
        )
    },
    || {
        check::<super::PrivateSettlementAuditViewKeySignatureV1>(
            "iroha_data_model::nexus::private_settlement::PrivateSettlementAuditViewKeySignatureV1",
            "9519ce1295def4f2ca936a08d685d199",
            "9519ce1295def4f2ca936a08d685d199",
        )
    },
    || {
        check::<super::PrivateSettlementAuditViewKeyAuthorizationV1>(
            "iroha_data_model::nexus::private_settlement::PrivateSettlementAuditViewKeyAuthorizationV1",
            "e413865fbde1c691300638e84728dd2d",
            "e413865fbde1c691300638e84728dd2d",
        )
    },
    || {
        check::<super::PrivateSettlementAuditEncryptionOpeningV1>(
            "iroha_data_model::nexus::private_settlement::PrivateSettlementAuditEncryptionOpeningV1",
            "1d4554100fd766ebb0c8c34d51bee115",
            "1d4554100fd766ebb0c8c34d51bee115",
        )
    },
    || {
        check::<super::PrivateSettlementAuditNoteOpeningV1>(
            "iroha_data_model::nexus::private_settlement::PrivateSettlementAuditNoteOpeningV1",
            "440cb8a0c31b404b07d62de27e8f38ff",
            "440cb8a0c31b404b07d62de27e8f38ff",
        )
    },
    || {
        check::<super::PrivateSettlementAuditOutputV1>(
            "iroha_data_model::nexus::private_settlement::PrivateSettlementAuditOutputV1",
            "e827d2fac54454aa944d40eb79c16b93",
            "e827d2fac54454aa944d40eb79c16b93",
        )
    },
    || {
        check::<super::PrivateSettlementAuditPlaintextV1>(
            "iroha_data_model::nexus::private_settlement::PrivateSettlementAuditPlaintextV1",
            "69e86050b2ce87807183090695111079",
            "69e86050b2ce87807183090695111079",
        )
    },
    || {
        check::<super::PrivateSettlementAuditAadV1>(
            "iroha_data_model::nexus::private_settlement::PrivateSettlementAuditAadV1",
            "f5c619b7ffdb96b93e9b142309c89ae0",
            "f5c619b7ffdb96b93e9b142309c89ae0",
        )
    },
    || {
        check::<super::PrivateSettlementHybridPublicKeyV1>(
            "iroha_data_model::nexus::private_settlement::PrivateSettlementHybridPublicKeyV1",
            "3322a905681696efed4421f4b2f39488",
            "3322a905681696efed4421f4b2f39488",
        )
    },
    || {
        check::<super::PrivateSettlementAuditorV1>(
            "iroha_data_model::nexus::private_settlement::PrivateSettlementAuditorV1",
            "0e2d35fad3b02a854f1ff7827ba58fc9",
            "0e2d35fad3b02a854f1ff7827ba58fc9",
        )
    },
    || {
        check::<super::PrivateSettlementAuditPolicyBodyV1>(
            "iroha_data_model::nexus::private_settlement::PrivateSettlementAuditPolicyBodyV1",
            "b11a17aa64d8eee5d93eb45a01bd8172",
            "b11a17aa64d8eee5d93eb45a01bd8172",
        )
    },
    || {
        check::<super::PrivateSettlementAuditPolicyV1>(
            "iroha_data_model::nexus::private_settlement::PrivateSettlementAuditPolicyV1",
            "a7454a459f735bb4ee82454383c1267a",
            "a7454a459f735bb4ee82454383c1267a",
        )
    },
    || {
        check::<super::PrivateSettlementPoolGovernanceLifecycleV1>(
            "iroha_data_model::nexus::private_settlement::PrivateSettlementPoolGovernanceLifecycleV1",
            "b58f0f703228b1d9b2864b26730ad2fc",
            "b58f0f703228b1d9b2864b26730ad2fc",
        )
    },
    || {
        check::<super::PrivateSettlementPoolGovernanceBodyV1>(
            "iroha_data_model::nexus::private_settlement::PrivateSettlementPoolGovernanceBodyV1",
            "eec02bb0875f223aeb9505c345e47a54",
            "eec02bb0875f223aeb9505c345e47a54",
        )
    },
    || {
        check::<super::PrivateSettlementPoolGovernanceV1>(
            "iroha_data_model::nexus::private_settlement::PrivateSettlementPoolGovernanceV1",
            "8829e8a5acfad0a4c6796e3d3ae08249",
            "8829e8a5acfad0a4c6796e3d3ae08249",
        )
    },
    || {
        check::<super::PrivateSettlementWrappedDekV1>(
            "iroha_data_model::nexus::private_settlement::PrivateSettlementWrappedDekV1",
            "473a20ab505e9645dbf7616c563e45ef",
            "473a20ab505e9645dbf7616c563e45ef",
        )
    },
    || {
        check::<super::PrivateSettlementAuditCapsuleV1>(
            "iroha_data_model::nexus::private_settlement::PrivateSettlementAuditCapsuleV1",
            "c7bda96fc57fd55fc3c9f89beda9a1a6",
            "c7bda96fc57fd55fc3c9f89beda9a1a6",
        )
    },
    || {
        check::<super::PrivateSettlementSidecarAvailabilityBodyV1>(
            "iroha_data_model::nexus::private_settlement::PrivateSettlementSidecarAvailabilityBodyV1",
            "ab6f6e1aa39875a5745798a7f79b0edf",
            "ab6f6e1aa39875a5745798a7f79b0edf",
        )
    },
    || {
        check::<super::PrivateSettlementAvailabilityShareV1>(
            "iroha_data_model::nexus::private_settlement::PrivateSettlementAvailabilityShareV1",
            "87f7f9e17206bd21336cc248c200ff30",
            "87f7f9e17206bd21336cc248c200ff30",
        )
    },
    || {
        check::<super::PrivateSettlementSidecarAvailabilityV1>(
            "iroha_data_model::nexus::private_settlement::PrivateSettlementSidecarAvailabilityV1",
            "e5bc824c38414510de26491840c5affb",
            "e5bc824c38414510de26491840c5affb",
        )
    },
    || {
        check::<super::PrivateSettlementAuditorViewDigestMaterialV1>(
            "iroha_data_model::nexus::private_settlement::PrivateSettlementAuditorViewDigestMaterialV1",
            "d172a246c01a8da6d27031bd3d524f18",
            "d172a246c01a8da6d27031bd3d524f18",
        )
    },
    || {
        check::<super::PrivateSettlementAuditorViewAttestationBodyV1>(
            "iroha_data_model::nexus::private_settlement::PrivateSettlementAuditorViewAttestationBodyV1",
            "c023f4c23be3e2b24bfbd0d5fd892df1",
            "c023f4c23be3e2b24bfbd0d5fd892df1",
        )
    },
    || {
        check::<super::PrivateSettlementAuditorViewAttestationV1>(
            "iroha_data_model::nexus::private_settlement::PrivateSettlementAuditorViewAttestationV1",
            "ca043a733aff02280ac64c86b28e7f34",
            "ca043a733aff02280ac64c86b28e7f34",
        )
    },
    || {
        check::<super::PrivateSettlementAuditApprovalAcknowledgementDigestMaterialV1>(
            "iroha_data_model::nexus::private_settlement::PrivateSettlementAuditApprovalAcknowledgementDigestMaterialV1",
            "5a3f9ca720ba2db0a0eeef90054d4c20",
            "5a3f9ca720ba2db0a0eeef90054d4c20",
        )
    },
    || {
        check::<super::PrivateSettlementAuditApprovalAcknowledgementAttestationBodyV1>(
            "iroha_data_model::nexus::private_settlement::PrivateSettlementAuditApprovalAcknowledgementAttestationBodyV1",
            "da4ed49b6196e2d3059c05ac9f817e8c",
            "da4ed49b6196e2d3059c05ac9f817e8c",
        )
    },
    || {
        check::<super::PrivateSettlementAuditApprovalAcknowledgementAttestationV1>(
            "iroha_data_model::nexus::private_settlement::PrivateSettlementAuditApprovalAcknowledgementAttestationV1",
            "e206c4a31449b63c533e755e8ac449d0",
            "e206c4a31449b63c533e755e8ac449d0",
        )
    },
    || {
        check::<super::PrivateSettlementLegPayloadV1>(
            "iroha_data_model::nexus::private_settlement::PrivateSettlementLegPayloadV1",
            "b849c2376f52055a28eab578d38cf1be",
            "b849c2376f52055a28eab578d38cf1be",
        )
    },
    || {
        check::<super::PrivateSettlementProvisionalLegMaterialV1>(
            "iroha_data_model::nexus::private_settlement::PrivateSettlementProvisionalLegMaterialV1",
            "c230e67c1c77aab3777bede51cf1d40c",
            "c230e67c1c77aab3777bede51cf1d40c",
        )
    },
    || {
        check::<super::PrivateSettlementAuditApprovalBodyV1>(
            "iroha_data_model::nexus::private_settlement::PrivateSettlementAuditApprovalBodyV1",
            "3f9b25c10547fc167dde4e36ef04a767",
            "3f9b25c10547fc167dde4e36ef04a767",
        )
    },
    || {
        check::<super::PrivateSettlementAuditApprovalV1>(
            "iroha_data_model::nexus::private_settlement::PrivateSettlementAuditApprovalV1",
            "c5175fd055675342e8125ae53168e8e6",
            "c5175fd055675342e8125ae53168e8e6",
        )
    },
    || {
        check::<super::PrivateSettlementPhaseV1>(
            "iroha_data_model::nexus::private_settlement::PrivateSettlementPhaseV1",
            "eb026cb58d72f677d611d80fbf89df25",
            "eb026cb58d72f677d611d80fbf89df25",
        )
    },
    || {
        check::<super::PrivateSettlementCommitteeRosterV1>(
            "iroha_data_model::nexus::private_settlement::PrivateSettlementCommitteeRosterV1",
            "c1c6519cd264dfbfa4ba0cdaf7fe34e6",
            "c1c6519cd264dfbfa4ba0cdaf7fe34e6",
        )
    },
    || {
        check::<super::PrivateSettlementCommitteeAuthorityV1>(
            "iroha_data_model::nexus::private_settlement::PrivateSettlementCommitteeAuthorityV1",
            "c5cac2d2da7fde54b45c684f2da4517c",
            "c5cac2d2da7fde54b45c684f2da4517c",
        )
    },
    || {
        check::<super::PrivateSettlementAuthorityCatalogV1>(
            "iroha_data_model::nexus::private_settlement::PrivateSettlementAuthorityCatalogV1",
            "1212a06f3807d2ba43a85870d07b3e16",
            "1212a06f3807d2ba43a85870d07b3e16",
        )
    },
    || {
        check::<super::PrivateSettlementPhaseBodyV1>(
            "iroha_data_model::nexus::private_settlement::PrivateSettlementPhaseBodyV1",
            "f91062cec727f3382f2eab5c505b6415",
            "f91062cec727f3382f2eab5c505b6415",
        )
    },
    || {
        check::<super::PrivateSettlementPhaseVoteV1>(
            "iroha_data_model::nexus::private_settlement::PrivateSettlementPhaseVoteV1",
            "d324f61a074ca4e8ee7a6c80a7535027",
            "d324f61a074ca4e8ee7a6c80a7535027",
        )
    },
    || {
        check::<super::PrivateSettlementPhaseCertificateV1>(
            "iroha_data_model::nexus::private_settlement::PrivateSettlementPhaseCertificateV1",
            "f5b84dee535ad10ecb38abcebbbb5726",
            "f5b84dee535ad10ecb38abcebbbb5726",
        )
    },
    || {
        check::<super::PrivateSettlementPrepareBarrierV1>(
            "iroha_data_model::nexus::private_settlement::PrivateSettlementPrepareBarrierV1",
            "10d2795fb29b82364607e5cf171b0806",
            "10d2795fb29b82364607e5cf171b0806",
        )
    },
    || {
        check::<super::PrivateSettlementLegReceiptV1>(
            "iroha_data_model::nexus::private_settlement::PrivateSettlementLegReceiptV1",
            "fff4f3e083d04d412721ef96a3e8d6b2",
            "fff4f3e083d04d412721ef96a3e8d6b2",
        )
    },
    || {
        check::<super::PrivateSettlementCommitBundleV1>(
            "iroha_data_model::nexus::private_settlement::PrivateSettlementCommitBundleV1",
            "ad171db6cb66a8086a69d91e6062cf86",
            "ad171db6cb66a8086a69d91e6062cf86",
        )
    },
    || {
        check::<super::PrivateSettlementReceiptV1>(
            "iroha_data_model::nexus::private_settlement::PrivateSettlementReceiptV1",
            "cb8000dfa2ef0ffe44b40be7ae19cde7",
            "cb8000dfa2ef0ffe44b40be7ae19cde7",
        )
    },
    || {
        check::<super::PrivateSettlementAbortReasonV1>(
            "iroha_data_model::nexus::private_settlement::PrivateSettlementAbortReasonV1",
            "326258a4da26a00eb62057ef56290617",
            "326258a4da26a00eb62057ef56290617",
        )
    },
    || {
        check::<super::PrivateSettlementAbortReceiptV1>(
            "iroha_data_model::nexus::private_settlement::PrivateSettlementAbortReceiptV1",
            "d6de3e3575ce68eaa92a76ade1346395",
            "d6de3e3575ce68eaa92a76ade1346395",
        )
    },
];

#[test]
fn captured_ordinary_codec_schema_identities() {
    for check in CASES {
        check();
    }
}
