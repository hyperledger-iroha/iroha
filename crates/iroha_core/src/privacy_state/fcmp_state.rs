fn fcmp_output_to_native_v1(
    output: PrivacyFcmpOutputTupleV1,
) -> Result<crate::privacy_engines::fcmp_plus_plus::FcmpOutputTupleV1, &'static str> {
    crate::privacy_engines::fcmp_plus_plus::FcmpOutputTupleV1::new(
        output.output_key,
        output.linking_tag_generator,
        output.amount_commitment,
    )
    .map_err(|_| "FCMP++ output tuple is not a canonical prime-order Edwards tuple")
}

fn fcmp_output_from_native_v1(
    output: crate::privacy_engines::fcmp_plus_plus::FcmpOutputTupleV1,
) -> PrivacyFcmpOutputTupleV1 {
    let (output_key, linking_tag_generator, amount_commitment) = output.components();
    PrivacyFcmpOutputTupleV1 {
        output_key,
        linking_tag_generator,
        amount_commitment,
    }
}

fn fcmp_root_to_native_v1(
    root: PrivacyFcmpTreeRootV1,
) -> Result<crate::privacy_engines::fcmp_plus_plus::FcmpTreeRootV1, &'static str> {
    crate::privacy_engines::fcmp_plus_plus::FcmpTreeRootV1::new(root.layers, root.point)
        .map_err(|_| "FCMP++ root is not canonical for its layer-selected curve")
}

fn fcmp_root_from_native_v1(
    root: crate::privacy_engines::fcmp_plus_plus::FcmpTreeRootV1,
) -> PrivacyFcmpTreeRootV1 {
    PrivacyFcmpTreeRootV1 {
        layers: root.layers(),
        point: root.point(),
    }
}

/// Validator-owned alternating Selene/Helios frontier for one FCMP++ pool.
///
/// The complete typed root, active `(O, I, C)` branch, and every mixed-radix
/// level are durable. Restore validates the native frontier and independently
/// rebuilds it from the complete position-bound output registry.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize, Encode, Decode)]
#[norito(deny_unknown_fields)]
pub struct PrivacyFcmpAccumulatorStateV1 {
    namespace: PrivacyNamespaceV1,
    bootstrap_digest: PrivacyProofManagedPoolBootstrapDigestV1,
    epoch: u64,
    root: PrivacyFcmpTreeRootV1,
    tree_size: u64,
    active_outputs: Vec<PrivacyFcmpOutputTupleV1>,
    levels: Vec<Vec<[u8; 32]>>,
}
