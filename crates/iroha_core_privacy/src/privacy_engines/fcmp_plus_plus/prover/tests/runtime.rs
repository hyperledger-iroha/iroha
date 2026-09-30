use super::*;

#[derive(Default)]
struct ZeroRng {
    calls: usize,
}
impl RngCore for ZeroRng {
    fn next_u32(&mut self) -> u32 {
        0
    }
    fn next_u64(&mut self) -> u64 {
        0
    }
    fn fill_bytes(&mut self, destination: &mut [u8]) {
        destination.fill(0);
    }
    fn try_fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), rand_core_06::Error> {
        self.calls += 1;
        destination.fill(0);
        Ok(())
    }
}
impl CryptoRng for ZeroRng {}
#[derive(Default)]
struct ZeroThenOneRng {
    calls: usize,
}
impl RngCore for ZeroThenOneRng {
    fn next_u32(&mut self) -> u32 {
        0
    }
    fn next_u64(&mut self) -> u64 {
        0
    }
    fn fill_bytes(&mut self, destination: &mut [u8]) {
        self.try_fill_bytes(destination)
            .expect("infallible fixture");
    }
    fn try_fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), rand_core_06::Error> {
        self.calls += 1;
        destination.fill(0);
        if self.calls == 2 {
            destination[0] = 1;
        }
        Ok(())
    }
}
impl CryptoRng for ZeroThenOneRng {}
struct PeriodicRng {
    period: usize,
    cursor: usize,
}
impl RngCore for PeriodicRng {
    fn next_u32(&mut self) -> u32 {
        panic!("FCMP++ public prover must reject the periodic prefix")
    }
    fn next_u64(&mut self) -> u64 {
        panic!("FCMP++ public prover must reject the periodic prefix")
    }
    fn fill_bytes(&mut self, _destination: &mut [u8]) {
        panic!("FCMP++ public prover must use fallible entropy")
    }
    fn try_fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), rand_core_06::Error> {
        for byte in destination {
            *byte = ((self.cursor % self.period) as u8)
                .wrapping_mul(73)
                .wrapping_add(19);
            self.cursor += 1;
        }
        Ok(())
    }
}
impl CryptoRng for PeriodicRng {}
#[test]
fn prover_witness_debug_is_redacted_and_explicit_zeroize_covers_the_full_path() {
    let (mut input, _new_output, _root) = one_layer_fixture();
    let output_debug = format!("{:?}", input.output);
    let witness_debug = format!("{input:?}");
    assert!(!witness_debug.contains(&output_debug));
    for secret_field in [
        "spend_x",
        "output_y",
        "rerandomization",
        "leaves",
        "additional_branches",
    ] {
        assert!(
            !witness_debug.contains(secret_field),
            "witness debug exposed {secret_field}"
        );
    }
    input.additional_branches = vec![
        AdditionalBranch::ToHelios(vec![HelioseleneField::ONE]),
        AdditionalBranch::ToSelene(vec![Field25519::ONE]),
    ];
    input.zeroize();
    assert_eq!(input.output.encode(), [0; FCMP_OUTPUT_TUPLE_BYTES_V1]);
    assert_eq!(input.spend_x, Scalar::ZERO);
    assert_eq!(input.output_y, Scalar::ZERO);
    assert_eq!(input.rerandomization.output, Scalar::ZERO);
    assert_eq!(input.rerandomization.linking, Scalar::ZERO);
    assert_eq!(input.rerandomization.rerandomization_blind, Scalar::ZERO);
    assert_eq!(input.rerandomization.commitment, Scalar::ZERO);
    assert!(input.leaves.is_empty());
    assert!(input.additional_branches.is_empty());
}
#[test]
fn constant_work_scan_primitives_visit_every_element_and_pair() {
    let values = [11_u8, 22, 33, 44, 55];
    for (target, expected) in [(11, true), (33, true), (55, true), (99, false)] {
        let comparisons = std::cell::Cell::new(0_usize);
        let found = ct_slice_contains_by(&values, &target, |left, right| {
            comparisons.set(comparisons.get() + 1);
            Choice::from(u8::from(left == right))
        });
        assert_eq!(bool::from(found), expected);
        assert_eq!(comparisons.get(), values.len());
    }
    let duplicate_cases = [
        ([7_u8, 7, 2, 3, 4], true),
        ([0_u8, 7, 7, 3, 4], true),
        ([0_u8, 1, 2, 7, 7], true),
        ([0_u8, 1, 2, 3, 4], false),
    ];
    let expected_pairs = values.len() * (values.len() - 1) / 2;
    for (values, expected) in duplicate_cases {
        let comparisons = std::cell::Cell::new(0_usize);
        let duplicate = ct_has_duplicate_by(&values, |left, right| {
            comparisons.set(comparisons.get() + 1);
            Choice::from(u8::from(left == right))
        });
        assert_eq!(bool::from(duplicate), expected);
        assert_eq!(comparisons.get(), expected_pairs);
    }
    for mismatch in [Some(0_usize), Some(2), Some(4), None] {
        let mut candidates = [9_u8; 5];
        if let Some(index) = mismatch {
            candidates[index] = 8;
        }
        let comparisons = std::cell::Cell::new(0_usize);
        let all_match = ct_all_match_by(&candidates, &9, |left, right| {
            comparisons.set(comparisons.get() + 1);
            Choice::from(u8::from(left == right))
        });
        assert_eq!(bool::from(all_match), mismatch.is_none());
        assert_eq!(comparisons.get(), candidates.len());
    }
    let left = [5_u8; 5];
    for mismatch in [Some(0_usize), Some(2), Some(4), None] {
        let mut right = left;
        if let Some(index) = mismatch {
            right[index] = 6;
        }
        let comparisons = std::cell::Cell::new(0_usize);
        let equal = ct_equal_slices_by(&left, &right, |left, right| {
            comparisons.set(comparisons.get() + 1);
            Choice::from(u8::from(left == right))
        });
        assert_eq!(bool::from(equal), mismatch.is_none());
        assert_eq!(comparisons.get(), left.len());
    }
}
#[test]
fn typed_membership_and_duplicate_scans_cover_every_position() {
    let digests = [[1_u8; 32], [2_u8; 32], [3_u8; 32], [4_u8; 32], [5_u8; 32]];
    for (target, expected) in [
        (digests[0], true),
        (digests[2], true),
        (digests[4], true),
        ([9_u8; 32], false),
    ] {
        assert_eq!(ct_digest_slice_contains(&digests, &target), expected);
    }
    for (duplicate_pair, expected) in [
        (Some((0_usize, 1_usize)), true),
        (Some((1, 2)), true),
        (Some((3, 4)), true),
        (None, false),
    ] {
        let mut candidates = digests;
        if let Some((source, destination)) = duplicate_pair {
            candidates[destination] = candidates[source];
        }
        assert_eq!(ct_has_duplicate_digests(&candidates), expected);
    }
    let helios_hash =
        prover_secret_hash_helios_v1(&[HelioseleneField::ONE]).expect("private Helios target hash");
    let field_target = prover_secret_helios_x_v1(&helios_hash).expect("owned Field25519 target");
    for target_index in [0, FCMP_LAYER_ONE_LEN_V1 / 2, FCMP_LAYER_ONE_LEN_V1 - 1] {
        let mut padded = vec![Field25519::ONE; FCMP_LAYER_ONE_LEN_V1];
        padded[target_index] = *field_target.as_ref();
        assert!(ct_field25519_slice_contains(&padded, &field_target));
    }
    let absent_field = field_target.as_ref().add_ref(&Field25519::ONE);
    assert!(!ct_field25519_slice_contains(
        &vec![absent_field; FCMP_LAYER_ONE_LEN_V1],
        &field_target,
    ));
    let selene_hash =
        prover_secret_hash_selene_v1(&[Field25519::ONE]).expect("private Selene target hash");
    let helioselene_target =
        prover_secret_selene_x_v1(&selene_hash).expect("owned Helioselene target");
    for target_index in [0, FCMP_LAYER_TWO_LEN_V1 / 2, FCMP_LAYER_TWO_LEN_V1 - 1] {
        let mut padded = vec![HelioseleneField::ONE; FCMP_LAYER_TWO_LEN_V1];
        padded[target_index] = *helioselene_target.as_ref();
        assert!(ct_helioselene_slice_contains(&padded, &helioselene_target));
    }
    let absent_helioselene = helioselene_target.as_ref().add_ref(&HelioseleneField::ONE);
    assert!(!ct_helioselene_slice_contains(
        &vec![absent_helioselene; FCMP_LAYER_TWO_LEN_V1],
        &helioselene_target,
    ));
}
#[test]
fn hidden_leaf_membership_and_duplicates_cover_first_middle_last_and_absent() {
    let xs = [101_u64, 103, 107, 109, 113];
    let ys = [127_u64, 131, 137, 139, 149];
    let leaves: [FcmpOutputTupleV1; 5] = core::array::from_fn(|index| {
        spendable_output(
            Scalar::from(xs[index]),
            Scalar::from(ys[index]),
            Scalar::from(151_u64 + u64::try_from(index).expect("index")),
            Scalar::from(163_u64 + u64::try_from(index).expect("index")),
        )
    });
    for target_index in [0_usize, 2, 4] {
        FcmpProverInputV1::new(
            leaves[target_index],
            Scalar::from(xs[target_index]).to_bytes(),
            Scalar::from(ys[target_index]).to_bytes(),
            rerandomization(173, 179, 181, 191),
            leaves.to_vec(),
            Vec::new(),
        )
        .expect("hidden output at any position is accepted");
    }
    let absent_x = Scalar::from(193_u64);
    let absent_y = Scalar::from(197_u64);
    let absent = spendable_output(
        absent_x,
        absent_y,
        Scalar::from(199_u64),
        Scalar::from(211_u64),
    );
    assert!(matches!(
        FcmpProverInputV1::new(
            absent,
            absent_x.to_bytes(),
            absent_y.to_bytes(),
            rerandomization(223, 227, 229, 233),
            leaves.to_vec(),
            Vec::new(),
        ),
        Err(FcmpNativeErrorV1::ArithmeticInvariant)
    ));
    for duplicate_pair in [(0_usize, 1_usize), (1, 2), (3, 4)] {
        let mut candidates = leaves;
        candidates[duplicate_pair.1] = candidates[duplicate_pair.0];
        assert!(matches!(
            FcmpProverInputV1::new(
                leaves[0],
                Scalar::from(xs[0]).to_bytes(),
                Scalar::from(ys[0]).to_bytes(),
                rerandomization(239, 241, 251, 257),
                candidates.to_vec(),
                Vec::new(),
            ),
            Err(FcmpNativeErrorV1::DuplicateOutput)
        ));
    }
}
#[test]
fn shared_root_scan_covers_first_middle_last_and_absent_mismatches() {
    let root_coordinates = [Field25519::ONE; 5];
    let shared_root = RootValues::C1(root_coordinates.to_vec());
    for mismatch in [Some(0_usize), Some(2), Some(4), None] {
        let mut paths = Vec::with_capacity(5);
        for path_index in 0..5 {
            let mut coordinates = root_coordinates;
            if mismatch == Some(path_index) {
                coordinates[2] += Field25519::ONE;
            }
            paths.push(PathValues {
                c1_non_root: Vec::new(),
                c2_non_root: Vec::new(),
                root: RootValues::C1(coordinates.to_vec()),
            });
        }
        assert_eq!(
            all_paths_share_root(&paths, &shared_root),
            mismatch.is_none()
        );
    }
    for mismatch in [Some(0_usize), Some(2), Some(4), None] {
        let mut coordinates = root_coordinates;
        if let Some(index) = mismatch {
            coordinates[index] += Field25519::ONE;
        }
        let candidate = RootValues::C1(coordinates.to_vec());
        assert_eq!(
            bool::from(root_values_ct_eq(&candidate, &shared_root)),
            mismatch.is_none()
        );
    }
    let c2_coordinates = [HelioseleneField::ONE; 5];
    let c2_shared_root = RootValues::C2(c2_coordinates.to_vec());
    for mismatch in [Some(0_usize), Some(2), Some(4), None] {
        let mut coordinates = c2_coordinates;
        if let Some(index) = mismatch {
            coordinates[index] += HelioseleneField::ONE;
        }
        let candidate = RootValues::C2(coordinates.to_vec());
        assert_eq!(
            bool::from(root_values_ct_eq(&candidate, &c2_shared_root)),
            mismatch.is_none()
        );
    }
}
#[test]
fn private_push_guard_forbids_vector_growth() {
    let mut values = Vec::with_capacity(3);
    let allocation_capacity = values.capacity();
    for _ in 0..allocation_capacity {
        require_preallocated_push(values.len(), values.capacity()).expect("preallocated slot");
        values.push(Field25519::ONE);
        assert_eq!(values.capacity(), allocation_capacity);
    }
    assert_eq!(
        require_preallocated_push(values.len(), values.capacity()),
        Err(FcmpNativeErrorV1::ArithmeticInvariant)
    );
}
#[test]
fn maximum_compiled_shape_has_canonical_paths_and_exact_resource_bound() {
    let (inputs, outputs, root) = maximum_bound_fixture();
    assert_eq!(inputs.len(), FCMP_MAX_INPUTS_NATIVE_V1);
    assert_eq!(outputs.len(), FCMP_MAX_OUTPUTS_NATIVE_V1);
    assert_eq!(root.layers(), FCMP_MAX_TREE_LAYERS_V1);
    let paths = inputs
        .iter()
        .map(|input| parse_path(input, root))
        .collect::<Result<Vec<_>, _>>()
        .expect("maximum-depth paths resolve");
    let shared_root = &paths.first().expect("at least one path").root;
    assert!(all_paths_share_root(&paths, shared_root));
    assert_eq!(
        ipa_rows(inputs.len(), usize::from(root.layers())).expect("maximum IPA rows"),
        (2_048, 1_024)
    );
    assert_eq!(
        fcmp_plus_plus_wire_size_v1(inputs.len(), root.layers(), outputs.len())
            .expect("maximum wire size"),
        FCMP_MAX_PROOF_WIRE_BYTES_V1
    );
}
#[test]
fn parse_path_private_owners_cover_success_error_and_unwind() {
    let (mut inputs, _outputs, root) = maximum_bound_fixture();
    let expected_point_drops = inputs[0].additional_branches.len() + 1;
    let expected_pair_drops = inputs[0].leaves.len() * 3;
    let expected_difference_drops = inputs[0]
        .additional_branches
        .iter()
        .map(|branch| match branch {
            AdditionalBranch::ToHelios(_) => FCMP_LAYER_TWO_LEN_V1,
            AdditionalBranch::ToSelene(_) => FCMP_LAYER_ONE_LEN_V1,
        })
        .sum::<usize>();
    let expected_coordinate_drops = inputs[0]
        .additional_branches
        .iter()
        .map(|branch| match branch {
            AdditionalBranch::ToHelios(values) => values.len(),
            AdditionalBranch::ToSelene(values) => values.len(),
        })
        .sum::<usize>();
    let first_branch_coordinate_drops = match &inputs[0].additional_branches[0] {
        AdditionalBranch::ToHelios(values) => values.len(),
        AdditionalBranch::ToSelene(values) => values.len(),
    };
    let expected_leaf_scalar_drops = expected_pair_drops * 2;
    let expected_scalar_drops = expected_leaf_scalar_drops + expected_coordinate_drops;
    reset_prover_secret_copy_owner_drops();
    reset_prover_secret_point_owner_drops();
    reset_prover_secret_scalar_owner_drops();
    let path = parse_path(&inputs[0], root).expect("owned maximum-depth path");
    assert_eq!(prover_secret_copy_owner_drops(), expected_difference_drops);
    assert_eq!(prover_secret_point_owner_drops(), expected_point_drops);
    assert_eq!(prover_secret_scalar_owner_drops(), expected_scalar_drops);
    drop(path);
    assert_eq!(prover_secret_point_owner_drops(), expected_point_drops);
    assert_eq!(prover_secret_scalar_owner_drops(), expected_scalar_drops);
    assert!(matches!(
        &inputs[0].additional_branches[0],
        AdditionalBranch::ToHelios(_)
    ));
    replace_first_secret_coordinate_v1(&mut inputs[0].additional_branches[0])
        .expect("replace first private path coordinate");
    reset_prover_secret_copy_owner_drops();
    reset_prover_secret_point_owner_drops();
    reset_prover_secret_scalar_owner_drops();
    assert!(matches!(
        parse_path(&inputs[0], root),
        Err(FcmpNativeErrorV1::ArithmeticInvariant)
    ));
    assert_eq!(prover_secret_copy_owner_drops(), FCMP_LAYER_TWO_LEN_V1);
    assert_eq!(prover_secret_point_owner_drops(), 1);
    assert_eq!(
        prover_secret_scalar_owner_drops(),
        expected_leaf_scalar_drops + first_branch_coordinate_drops
    );
    reset_prover_secret_copy_owner_drops();
    reset_prover_secret_point_owner_drops();
    reset_prover_secret_scalar_owner_drops();
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let path = parse_path(&inputs[1], root).expect("second owned maximum-depth path");
        assert_eq!(prover_secret_copy_owner_drops(), expected_difference_drops);
        assert_eq!(prover_secret_point_owner_drops(), expected_point_drops);
        assert_eq!(prover_secret_scalar_owner_drops(), expected_scalar_drops);
        let _ = core::hint::black_box(&path);
        panic!("exercise parsed private path unwind");
    }));
    assert!(unwind.is_err());
    assert_eq!(prover_secret_copy_owner_drops(), expected_difference_drops);
    assert_eq!(prover_secret_point_owner_drops(), expected_point_drops);
    assert_eq!(prover_secret_scalar_owner_drops(), expected_scalar_drops);
}
#[test]
fn secret_root_comparison_owns_encoding_on_match_mismatch_and_error() {
    let selene_values = [Field25519::ONE];
    let expected_selene = hash_selene(&selene_values).expect("public Selene root");
    let other_selene = hash_selene(&[Field25519::ONE.add_ref(&Field25519::ONE)])
        .expect("other public Selene root");
    reset_prover_secret_point_owner_drops();
    let actual_selene = prover_secret_hash_selene_v1(&selene_values).expect("owned Selene root");
    assert!(ct_secret_selene_point_eq_v1(&actual_selene, &expected_selene).unwrap());
    assert!(!ct_secret_selene_point_eq_v1(&actual_selene, &other_selene).unwrap());
    drop(actual_selene);
    assert_eq!(prover_secret_point_owner_drops(), 1);
    let helios_values = [HelioseleneField::ONE];
    let expected_helios = hash_helios(&helios_values).expect("public Helios root");
    let other_helios = hash_helios(&[HelioseleneField::ONE.add_ref(&HelioseleneField::ONE)])
        .expect("other public Helios root");
    reset_prover_secret_point_owner_drops();
    let actual_helios = prover_secret_hash_helios_v1(&helios_values).expect("owned Helios root");
    assert!(ct_secret_helios_point_eq_v1(&actual_helios, &expected_helios).unwrap());
    assert!(!ct_secret_helios_point_eq_v1(&actual_helios, &other_helios).unwrap());
    drop(actual_helios);
    assert_eq!(prover_secret_point_owner_drops(), 1);
    let mut identity = SelenePoint::identity();
    let identity = ProverSecretPointV1::take(&mut identity);
    assert_eq!(
        ct_secret_selene_point_eq_v1(&identity, &expected_selene),
        Err(FcmpNativeErrorV1::ArithmeticInvariant)
    );
    drop(identity);
    assert_eq!(prover_secret_point_owner_drops(), 2);

    reset_prover_secret_point_owner_drops();
    let unwind_point = prover_secret_hash_helios_v1(&helios_values).expect("owned Helios root");
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        assert!(ct_secret_helios_point_eq_v1(&unwind_point, &expected_helios).unwrap());
        assert_eq!(prover_secret_point_owner_drops(), 0);
        panic!("exercise borrowed root-comparison unwind");
    }));
    assert!(unwind.is_err());
    assert_eq!(prover_secret_point_owner_drops(), 0);
    drop(unwind_point);
    assert_eq!(prover_secret_point_owner_drops(), 1);
}
#[test]
fn parse_path_source_keeps_private_values_in_owned_borrowed_order() {
    let source = include_str!("../../prover.rs");
    let borrowed_push = source_part!(
        source;
        "fn push_borrowed_secret_scalar_v1<F: ProofScalar + Zeroize>(" =>
        "fn prover_secret_decode_edwards_point_v1("
    );
    source_has!(
        borrowed_push;
        "value: &F",
        "let allocation_capacity = values.capacity()",
        "let allocation_ptr = values.as_ptr()",
        "require_preallocated_push(values.len(), allocation_capacity)?",
        "let value = ProverSecretScalarV1::copy_from_borrowed(value)",
        "push_owned_secret_scalar_v1(values, value)?"
    );
    source_order!(
        borrowed_push;
        "let allocation_capacity = values.capacity()",
        "let allocation_ptr = values.as_ptr()",
        "require_preallocated_push(values.len(), allocation_capacity)?",
        "let value = ProverSecretScalarV1::copy_from_borrowed(value)",
        "push_owned_secret_scalar_v1(values, value)?"
    );
    source_counts!(
        borrowed_push;
        "debug_assert_eq!(values.capacity(), allocation_capacity)" => 2,
        "debug_assert_eq!(values.as_ptr(), allocation_ptr)" => 2
    );
    source_lacks!(
        borrowed_push;
        "mut value: F",
        "ProverSecretScalarV1::take(",
        "values.push(*value)",
        "value.expose_copy()",
        ".copied()",
        ".cloned()",
        "callback",
        "getter",
        "FnOnce",
        "FnMut",
        "Deref",
        "Clone"
    );
    let parse = source_part!(source; "fn parse_path(" => "fn random_proof_scalar<F: ProofScalar>");
    source_has!(parse; "prover_secret_leaf_coordinates_v1(", "secret_edwards_to_wei25519_v1,", "Some(prover_secret_hash_selene_v1(&leaves)?)", "prover_secret_selene_x_v1(", "ct_helioselene_slice_contains(&padded, &prior_x)", "let next_c2 = prover_secret_hash_helios_v1(&padded)?", "current_c2 = Some(next_c2)", "prover_secret_helios_x_v1(", "ct_field25519_slice_contains(&padded, &prior_x)", "let next_c1 = prover_secret_hash_selene_v1(&padded)?", "current_c1 = Some(next_c1)", "ct_secret_selene_point_eq_v1(actual, &expected)?", "ct_secret_helios_point_eq_v1(actual, &expected)?");
    source_counts!(parse; "push_borrowed_secret_scalar_v1(&mut padded, coordinate)?" => 2);
    source_lacks!(parse; ".components()", "let (x, y) = edwards_to_wei25519", "hash_selene(&leaves)", "Some(hash_helios(&padded)?)", "Some(hash_selene(&padded)?)", ".and_then(SelenePoint::x)", ".and_then(HeliosPoint::x)", ".x()", "current_c1.take()", "current_c2.take()", "ct_selene_point_eq(actual.expose_ref(), &expected)", "ct_helios_point_eq(actual.expose_ref(), &expected)", "push_secret_scalar_v1(&mut padded", "*coordinate", ".copied()", ".cloned()", "callback", "getter", "Deref", "Clone");
    source_order!(source_part!(parse; "AdditionalBranch::ToHelios(branch) => {" => "AdditionalBranch::ToSelene(branch) => {"); "prover_secret_selene_x_v1(", "push_borrowed_secret_scalar_v1(&mut padded, coordinate)?", "ct_helioselene_slice_contains(&padded, &prior_x)", "let next_c2 = prover_secret_hash_helios_v1(&padded)?", "current_c2 = Some(next_c2)");
    source_order!(source_part!(parse; "AdditionalBranch::ToSelene(branch) => {" => "let matches_root = match root.curve()"); "prover_secret_helios_x_v1(", "push_borrowed_secret_scalar_v1(&mut padded, coordinate)?", "ct_field25519_slice_contains(&padded, &prior_x)", "let next_c1 = prover_secret_hash_selene_v1(&padded)?", "current_c1 = Some(next_c1)");
    let containment =
        source_part!(source; "fn ct_field25519_slice_contains(" => "enum AdditionalBranch");
    source_counts!(containment; "target: &SecretCycleScalarV1<" => 2, "ProverSecretCopyValueV1::new(value.sub_ref(target))" => 2, "difference.expose_ref().ct_is_zero()" => 2);
    source_lacks!(containment; "target: Field25519", "target: HelioseleneField", "*value - *target");
    let root_comparison = source_part!(source; "fn ct_secret_selene_point_eq_v1(" => "fn ct_field25519_slice_contains(");
    source_counts!(root_comparison; ".secret_encoding_owner_v1()" => 2, "Zeroizing::new(public_right.encode())" => 2, "left.as_ref().as_slice().ct_eq(public_right.as_slice())" => 2);
    source_lacks!(root_comparison; "left.encode()", "left.expose_copy()", "left.expose_public_copy_v1()");
    let tuple_source = include_str!("../../mod.rs");
    source_has!(source_part!(tuple_source; "pub(crate) const fn component_refs_v1(" => "/// Encode the tuple without framing."); "&self.output_key", "&self.linking_tag_generator", "&self.amount_commitment");
    let production_cfg = "#[cfg(any(test, feature = \"privacy-release-evidence\"))]";
    let immediately_cfg_gated = |source: &str, helper: &str| {
        let helper = source.find(helper).expect("production helper");
        source[..helper]
            .lines()
            .rev()
            .find(|line| !line.trim().is_empty())
            .is_some_and(|line| line.trim() == production_cfg)
    };
    for production_helper in [
        "fn zeroizing_exact_secret_buffer_v1<T: Zeroize>(",
        "fn ct_secret_selene_point_eq_v1(",
        "fn ct_secret_helios_point_eq_v1(",
        "fn prover_secret_hash_selene_v1(",
        "fn prover_secret_hash_helios_v1(",
        "fn prover_secret_selene_x_v1(",
        "fn prover_secret_helios_x_v1(",
        "fn push_borrowed_secret_scalar_v1<F: ProofScalar + Zeroize>(",
    ] {
        assert!(source.contains(production_helper));
        assert!(!immediately_cfg_gated(source, production_helper));
    }
    let field_source = include_str!("../../field.rs");
    for production_helper in [
        "pub(super) struct SecretEncodedScalarV1",
        "pub(super) struct SecretCycleScalarV1",
        "pub(super) fn encode_secret_field25519_scalar_v1",
        "pub(super) fn encode_secret_helioselene_scalar_v1",
        "pub(super) fn secret_x_ref_v1(&self)",
        "pub(super) fn secret_encode_v1(mut self)",
        "pub(super) fn secret_encode_ref_v1(&self)",
    ] {
        assert!(field_source.contains(production_helper));
        assert!(!immediately_cfg_gated(field_source, production_helper));
    }
    assert!(!field_source.contains("pub(super) fn secret_x_v1(mut self)"));
    let secret_point_adapters = source_part!(
        source;
        "impl ProverSecretPointV1<SelenePoint> {" =>
        "impl<P: ProofPoint> Drop for ProverSecretPointV1<P>"
    );
    source_counts!(secret_point_adapters; "fn secret_x_owner_v1(&self) -> Option<SecretCycleScalarV1<" => 2);
    source_counts!(secret_point_adapters; "fn secret_encoding_owner_v1(&self) -> Option<SecretEncodedScalarV1>" => 2);
    source_counts!(secret_point_adapters; ".secret_x_ref_v1()" => 2, ".secret_encode_ref_v1()" => 2);
    source_lacks!(secret_point_adapters; production_cfg, "secret_x_copy_v1", "secret_encoding_copy_v1", "self.0.secret_x_v1()", "self.0.secret_encode_v1()");
}
#[test]
fn malicious_zero_rng_exhausts_a_fixed_bound_instead_of_hanging() {
    let mut rng = ZeroRng::default();
    reset_prover_secret_scalar_owner_drops();
    assert!(matches!(
        random_proof_scalar::<Field25519>(&mut rng),
        Err(FcmpNativeErrorV1::ProverRandomnessExhausted)
    ));
    assert_eq!(rng.calls, MAX_PROVER_SCALAR_ATTEMPTS_V1);
    assert_eq!(MAX_PROVER_SCALAR_ATTEMPTS_V1, 128);
    assert_eq!(
        prover_secret_scalar_owner_drops(),
        MAX_PROVER_SCALAR_ATTEMPTS_V1
    );
}
#[test]
fn borrowed_path_coordinate_handoff_preflights_and_keeps_allocation_stable() {
    reset_prover_secret_scalar_owner_drops();
    let source = Field25519::ONE;
    let mut values =
        zeroizing_exact_secret_buffer_v1::<Field25519>(1).expect("one borrowed Field25519 slot");
    let allocation_capacity = values.capacity();
    let allocation_ptr = values.as_ptr();
    push_borrowed_secret_scalar_v1(&mut values, &source)
        .expect("preallocated borrowed Field25519 handoff");
    assert_eq!(source, Field25519::ONE);
    assert_eq!(values.as_slice(), &[Field25519::ONE]);
    assert_eq!(values.capacity(), allocation_capacity);
    assert_eq!(values.as_ptr(), allocation_ptr);
    assert_eq!(prover_secret_scalar_owner_drops(), 1);

    reset_prover_secret_scalar_owner_drops();
    let source = HelioseleneField::ONE;
    let mut no_capacity: Zeroizing<Vec<HelioseleneField>> = Zeroizing::new(Vec::new());
    let allocation_capacity = no_capacity.capacity();
    let allocation_ptr = no_capacity.as_ptr();
    assert_eq!(
        push_borrowed_secret_scalar_v1(&mut no_capacity, &source),
        Err(FcmpNativeErrorV1::ArithmeticInvariant)
    );
    assert_eq!(source, HelioseleneField::ONE);
    assert!(no_capacity.is_empty());
    assert_eq!(no_capacity.capacity(), allocation_capacity);
    assert_eq!(no_capacity.as_ptr(), allocation_ptr);
    assert_eq!(prover_secret_scalar_owner_drops(), 0);

    reset_prover_secret_scalar_owner_drops();
    let unwind = std::panic::catch_unwind(|| {
        let source = Field25519::ONE;
        let mut values =
            zeroizing_exact_secret_buffer_v1::<Field25519>(1).expect("one borrowed unwind slot");
        let allocation_capacity = values.capacity();
        let allocation_ptr = values.as_ptr();
        push_borrowed_secret_scalar_v1(&mut values, &source)
            .expect("borrowed handoff before unwind");
        assert_eq!(values.capacity(), allocation_capacity);
        assert_eq!(values.as_ptr(), allocation_ptr);
        assert_eq!(prover_secret_scalar_owner_drops(), 1);
        let _ = core::hint::black_box(&values);
        panic!("exercise borrowed path-coordinate unwind");
    });
    assert!(unwind.is_err());
    assert_eq!(prover_secret_scalar_owner_drops(), 1);
}
#[test]
fn owned_secret_scalar_handoff_keeps_preallocation_and_clears_source_on_every_exit() {
    reset_prover_secret_scalar_owner_drops();
    let mut field_values =
        zeroizing_exact_secret_buffer_v1::<Field25519>(1).expect("one Field25519 slot");
    let field_capacity = field_values.capacity();
    let field_ptr = field_values.as_ptr();
    push_owned_secret_scalar_v1(&mut field_values, ProverSecretScalarV1(Field25519::ONE))
        .expect("preallocated Field25519 owner handoff");
    assert_eq!(field_values.as_slice(), &[Field25519::ONE]);
    assert_eq!(field_values.capacity(), field_capacity);
    assert_eq!(field_values.as_ptr(), field_ptr);
    assert_eq!(prover_secret_scalar_owner_drops(), 1);

    reset_prover_secret_scalar_owner_drops();
    let mut helios_values =
        zeroizing_exact_secret_buffer_v1::<HelioseleneField>(1).expect("one Helioselene slot");
    let helios_capacity = helios_values.capacity();
    let helios_ptr = helios_values.as_ptr();
    push_owned_secret_scalar_v1(
        &mut helios_values,
        ProverSecretScalarV1(HelioseleneField::ONE),
    )
    .expect("preallocated Helioselene owner handoff");
    assert_eq!(helios_values.as_slice(), &[HelioseleneField::ONE]);
    assert_eq!(helios_values.capacity(), helios_capacity);
    assert_eq!(helios_values.as_ptr(), helios_ptr);
    assert_eq!(prover_secret_scalar_owner_drops(), 1);

    reset_prover_secret_scalar_owner_drops();
    let mut no_capacity = Zeroizing::new(Vec::new());
    assert_eq!(
        push_owned_secret_scalar_v1(&mut no_capacity, ProverSecretScalarV1(Field25519::ONE),),
        Err(FcmpNativeErrorV1::ArithmeticInvariant)
    );
    assert!(no_capacity.is_empty());
    assert_eq!(prover_secret_scalar_owner_drops(), 1);

    reset_prover_secret_scalar_owner_drops();
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut values =
            zeroizing_exact_secret_buffer_v1::<Field25519>(1).expect("one unwind slot");
        let allocation_capacity = values.capacity();
        let allocation_ptr = values.as_ptr();
        push_owned_secret_scalar_v1(&mut values, ProverSecretScalarV1(Field25519::ONE))
            .expect("preallocated owner handoff before unwind");
        assert_eq!(values.capacity(), allocation_capacity);
        assert_eq!(values.as_ptr(), allocation_ptr);
        assert_eq!(prover_secret_scalar_owner_drops(), 1);
        panic!("exercise downstream unwind after scalar owner handoff");
    }));
    assert!(unwind.is_err());
    assert_eq!(prover_secret_scalar_owner_drops(), 1);
}
#[test]
fn negated_scalar_owner_handoff_retains_source_and_clears_every_temporary() {
    reset_prover_secret_scalar_owner_drops();
    let source = ProverSecretScalarV1(Field25519::ONE);
    let mut values =
        zeroizing_exact_secret_buffer_v1::<Field25519>(1).expect("one negated Field25519 slot");
    let allocation_capacity = values.capacity();
    let allocation_ptr = values.as_ptr();
    push_owned_secret_scalar_v1(&mut values, source.negated_owner_v1())
        .expect("preallocated negated owner handoff");
    assert_eq!(source.expose_ref(), &Field25519::ONE);
    assert_eq!(
        values.as_slice(),
        &[Field25519::ZERO.sub_ref(&Field25519::ONE)]
    );
    assert_eq!(values.capacity(), allocation_capacity);
    assert_eq!(values.as_ptr(), allocation_ptr);
    assert_eq!(prover_secret_scalar_owner_drops(), 1);
    drop(source);
    assert_eq!(prover_secret_scalar_owner_drops(), 2);

    reset_prover_secret_scalar_owner_drops();
    let source = ProverSecretScalarV1(HelioseleneField::ONE);
    let mut no_capacity: Zeroizing<Vec<HelioseleneField>> = Zeroizing::new(Vec::new());
    let allocation_capacity = no_capacity.capacity();
    let allocation_ptr = no_capacity.as_ptr();
    assert_eq!(
        push_owned_secret_scalar_v1(&mut no_capacity, source.negated_owner_v1()),
        Err(FcmpNativeErrorV1::ArithmeticInvariant)
    );
    assert_eq!(source.expose_ref(), &HelioseleneField::ONE);
    assert!(no_capacity.is_empty());
    assert_eq!(no_capacity.capacity(), allocation_capacity);
    assert_eq!(no_capacity.as_ptr(), allocation_ptr);
    assert_eq!(prover_secret_scalar_owner_drops(), 1);
    drop(source);
    assert_eq!(prover_secret_scalar_owner_drops(), 2);

    reset_prover_secret_scalar_owner_drops();
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let source = ProverSecretScalarV1(Field25519::ONE);
        let mut values =
            zeroizing_exact_secret_buffer_v1::<Field25519>(1).expect("one negated unwind slot");
        let allocation_capacity = values.capacity();
        let allocation_ptr = values.as_ptr();
        push_owned_secret_scalar_v1(&mut values, source.negated_owner_v1())
            .expect("negated owner handoff before unwind");
        assert_eq!(source.expose_ref(), &Field25519::ONE);
        assert_eq!(
            values.as_slice(),
            &[Field25519::ZERO.sub_ref(&Field25519::ONE)]
        );
        assert_eq!(values.capacity(), allocation_capacity);
        assert_eq!(values.as_ptr(), allocation_ptr);
        assert_eq!(prover_secret_scalar_owner_drops(), 1);
        let _ = core::hint::black_box((&source, &values));
        panic!("exercise negated scalar owner unwind");
    }));
    assert!(unwind.is_err());
    assert_eq!(prover_secret_scalar_owner_drops(), 2);
}
#[test]
fn sampled_scalar_slots_are_owned_before_rejection_or_return() {
    let mut rng = ZeroThenOneRng::default();
    reset_prover_secret_scalar_owner_drops();
    let scalar: ProverSecretScalarV1<Field25519> =
        random_proof_scalar::<Field25519>(&mut rng).expect("second candidate is one");
    assert_eq!(
        scalar.expose_ref(),
        &Field25519::ONE,
        "returned candidate stays in its scalar owner"
    );
    assert_eq!(rng.calls, 2);
    assert_eq!(prover_secret_scalar_owner_drops(), 1);
    drop(scalar);
    assert_eq!(prover_secret_scalar_owner_drops(), 2);

    reset_prover_secret_scalar_owner_drops();
    let mut rng = ZeroThenOneRng::default();
    let scalar: ProverSecretScalarV1<Field25519> =
        random_proof_scalar::<Field25519>(&mut rng).expect("owned capacity-error candidate");
    assert_eq!(prover_secret_scalar_owner_drops(), 1);
    let mut no_capacity = Zeroizing::new(Vec::new());
    assert!(matches!(
        push_owned_secret_scalar_v1(&mut no_capacity, scalar),
        Err(FcmpNativeErrorV1::ArithmeticInvariant)
    ));
    assert_eq!(prover_secret_scalar_owner_drops(), 2);

    reset_prover_secret_scalar_owner_drops();
    let unwind = std::panic::catch_unwind(|| {
        let mut rng = ZeroThenOneRng::default();
        let scalar: ProverSecretScalarV1<Field25519> =
            random_proof_scalar::<Field25519>(&mut rng).expect("owned unwind candidate");
        assert_eq!(prover_secret_scalar_owner_drops(), 1);
        let _ = core::hint::black_box(&scalar);
        panic!("exercise sampled scalar owner unwind");
    });
    assert!(unwind.is_err());
    assert_eq!(prover_secret_scalar_owner_drops(), 2);

    let source = include_str!("../../prover.rs");
    let random = source_section(
        source,
        "fn random_proof_scalar<F: ProofScalar>",
        "fn root_nonce_commitment_v1<S: ProofSuite>",
    );
    assert!(random.contains(") -> Result<ProverSecretScalarV1<F>, FcmpNativeErrorV1>"));
    let candidate = random
        .find("if let Some(sampled)")
        .expect("owned upstream candidate");
    let transfer = random
        .find("ProverSecretScalarV1::copy_from_borrowed(sampled.expose_ref())")
        .expect("borrowed owner transfer");
    let upstream_drop = random.find("drop(sampled)").expect("upstream owner drop");
    let zero_check = random
        .find("if scalar.expose_ref() != &F::ZERO")
        .expect("borrowed owned zero check");
    let returned = random
        .find("return Ok(scalar)")
        .expect("move-only owner return");
    assert!(
        candidate < transfer
            && transfer < upstream_drop
            && upstream_drop < zero_check
            && zero_check < returned
    );
    assert_source_contract_group(
        "sampled_scalar_slots_are_owned_before_rejection_or_return/00",
        random,
    );
    let owner = source_section(
        source,
        "impl<F: ProofScalar> ProverSecretScalarV1<F>",
        "impl ProverSecretScalarV1<Field25519>",
    );
    let borrowed_constructor = source_section(
        owner,
        "fn copy_from_borrowed(value: &F) -> Self",
        "fn negated_owner_v1(&self) -> Self",
    );
    let negated_constructor = source_section(
        owner,
        "fn negated_owner_v1(&self) -> Self",
        "fn take(value: &mut F) -> Self",
    );
    assert!(borrowed_constructor.contains("Self(*value)"));
    assert_source_contract_group(
        "sampled_scalar_slots_are_owned_before_rejection_or_return/01",
        borrowed_constructor,
    );
    assert!(negated_constructor.contains("Self(-self.0)"));
    assert_eq!(negated_constructor.matches("self.0").count(), 1);
    assert_eq!(negated_constructor.matches("Self(").count(), 1);
    assert_source_contract_group(
        "sampled_scalar_slots_are_owned_before_rejection_or_return/02",
        negated_constructor,
    );
    assert!(!owner.contains("fn expose_copy(&self) -> F"));
    let proof_math = include_str!("../../proof_math.rs");
    let adapter = source_section(
        proof_math,
        "pub(super) fn random_scalar_from_fcmp_rng<F, R>(",
        "pub(super) struct ProverTranscript",
    );
    assert!(adapter.contains("Result<Option<SecretScalar<F>>, FcmpNativeErrorV1>"));
    assert!(!adapter.contains("Result<Option<F>"));

    let prove_once = source_section(
        source,
        "fn prove_fcmp_plus_plus_once_v1(",
        "fn retry_membership_prover<T>(",
    );
    assert_source_contract_group(
        "sampled_scalar_slots_are_owned_before_rejection_or_return/03",
        prove_once,
    );
    assert_source_contract_group(
        "sampled_scalar_slots_are_owned_before_rejection_or_return/04",
        prove_once,
    );
    assert_source_contract_group(
        "sampled_scalar_slots_are_owned_before_rejection_or_return/05",
        prove_once,
    );
}
#[test]
fn prepared_cycle_blind_owners_survive_handoff_until_success_drop() {
    let mut scalar_handoff =
        zeroizing_exact_secret_buffer_v1::<Field25519>(1).expect("raw scalar handoff buffer");
    push_secret_scalar_v1(&mut scalar_handoff, Field25519::ONE).expect("raw scalar owner handoff");
    assert_eq!(scalar_handoff.as_slice(), &[Field25519::ONE]);
    drop(scalar_handoff);

    reset_prover_secret_scalar_owner_drops();
    reset_prover_secret_point_owner_drops();
    let selene =
        prepare_selene_blind(ProverSecretScalarV1(Field25519::ONE)).expect("prepared Selene blind");
    assert_eq!(selene.scalar.expose_ref(), &Field25519::ONE);
    let expected_selene = selene_bp_generators().h.scale(Field25519::ONE);
    assert!(selene.point.expose_ref().eq(&expected_selene));
    let selene_coordinates = selene
        .point
        .expose_ref()
        .secret_coordinates_ref_v1()
        .expect("borrowed Selene coordinates");
    let mut c2_tape = ProverVectorCommitmentTape::new(512).expect("Selene claim tape");
    c2_tape
        .append_claimed_point(
            CYCLE_DLOG_PARAMETERS,
            &selene.decomposition,
            &selene.divisor,
            selene_coordinates.component_pair_ref(),
            &[],
        )
        .expect("borrowed Selene point claim");
    assert_eq!(prover_secret_scalar_owner_drops(), 0);
    assert_eq!(prover_secret_point_owner_drops(), 0);
    drop(selene_coordinates);
    drop(selene);
    assert_eq!(prover_secret_scalar_owner_drops(), 1);
    assert_eq!(prover_secret_point_owner_drops(), 1);

    reset_prover_secret_scalar_owner_drops();
    reset_prover_secret_point_owner_drops();
    let helios = prepare_helios_blind(ProverSecretScalarV1(HelioseleneField::ONE))
        .expect("prepared Helios blind");
    assert_eq!(helios.scalar.expose_ref(), &HelioseleneField::ONE);
    let expected_helios = helios_bp_generators().h.scale(HelioseleneField::ONE);
    assert!(helios.point.expose_ref().eq(&expected_helios));
    let helios_coordinates = helios
        .point
        .expose_ref()
        .secret_coordinates_ref_v1()
        .expect("borrowed Helios coordinates");
    let mut c1_tape = ProverVectorCommitmentTape::new(512).expect("Helios claim tape");
    c1_tape
        .append_claimed_point(
            CYCLE_DLOG_PARAMETERS,
            &helios.decomposition,
            &helios.divisor,
            helios_coordinates.component_pair_ref(),
            &[],
        )
        .expect("borrowed Helios point claim");
    assert_eq!(prover_secret_scalar_owner_drops(), 0);
    assert_eq!(prover_secret_point_owner_drops(), 0);
    drop(helios_coordinates);
    drop(helios);
    assert_eq!(prover_secret_scalar_owner_drops(), 1);
    assert_eq!(prover_secret_point_owner_drops(), 1);
}
#[test]
fn prepared_cycle_blind_identity_coordinates_fail_without_unwrapping_owners() {
    reset_prover_secret_scalar_owner_drops();
    reset_prover_secret_point_owner_drops();
    let mut selene =
        prepare_selene_blind(ProverSecretScalarV1(Field25519::ONE)).expect("prepared Selene blind");
    selene.point.0.clear_secret();
    let selene_identity = selene
        .point
        .expose_ref()
        .secret_coordinates_ref_v1()
        .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)
        .map(|_| ());
    assert_eq!(selene_identity, Err(FcmpNativeErrorV1::ArithmeticInvariant));
    assert_eq!(prover_secret_scalar_owner_drops(), 0);
    assert_eq!(prover_secret_point_owner_drops(), 0);
    drop(selene);
    assert_eq!(prover_secret_scalar_owner_drops(), 1);
    assert_eq!(prover_secret_point_owner_drops(), 1);

    reset_prover_secret_scalar_owner_drops();
    reset_prover_secret_point_owner_drops();
    let mut helios = prepare_helios_blind(ProverSecretScalarV1(HelioseleneField::ONE))
        .expect("prepared Helios blind");
    helios.point.0.clear_secret();
    let helios_identity = helios
        .point
        .expose_ref()
        .secret_coordinates_ref_v1()
        .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)
        .map(|_| ());
    assert_eq!(helios_identity, Err(FcmpNativeErrorV1::ArithmeticInvariant));
    assert_eq!(prover_secret_scalar_owner_drops(), 0);
    assert_eq!(prover_secret_point_owner_drops(), 0);
    drop(helios);
    assert_eq!(prover_secret_scalar_owner_drops(), 1);
    assert_eq!(prover_secret_point_owner_drops(), 1);
}
#[test]
fn prepared_cycle_blind_owners_clear_on_downstream_error_for_both_curves() {
    reset_prover_secret_scalar_owner_drops();
    reset_prover_secret_point_owner_drops();
    let selene_error = (|| -> Result<(), FcmpNativeErrorV1> {
        let blind = prepare_selene_blind(ProverSecretScalarV1(Field25519::ONE))?;
        let coordinates = blind
            .point
            .expose_ref()
            .secret_coordinates_ref_v1()
            .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?;
        let mut tape = ProverVectorCommitmentTape::new(512)?;
        let truncated = &blind.decomposition[..blind.decomposition.len() - 1];
        tape.append_claimed_point(
            CYCLE_DLOG_PARAMETERS,
            truncated,
            &blind.divisor,
            coordinates.component_pair_ref(),
            &[],
        )?;
        assert_eq!(prover_secret_scalar_owner_drops(), 0);
        assert_eq!(prover_secret_point_owner_drops(), 0);
        Ok(())
    })();
    assert_eq!(selene_error, Err(FcmpNativeErrorV1::ArithmeticInvariant));
    assert_eq!(prover_secret_scalar_owner_drops(), 1);
    assert_eq!(prover_secret_point_owner_drops(), 1);

    reset_prover_secret_scalar_owner_drops();
    reset_prover_secret_point_owner_drops();
    let helios_error = (|| -> Result<(), FcmpNativeErrorV1> {
        let blind = prepare_helios_blind(ProverSecretScalarV1(HelioseleneField::ONE))?;
        let coordinates = blind
            .point
            .expose_ref()
            .secret_coordinates_ref_v1()
            .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?;
        let mut tape = ProverVectorCommitmentTape::new(512)?;
        let truncated = &blind.decomposition[..blind.decomposition.len() - 1];
        tape.append_claimed_point(
            CYCLE_DLOG_PARAMETERS,
            truncated,
            &blind.divisor,
            coordinates.component_pair_ref(),
            &[],
        )?;
        assert_eq!(prover_secret_scalar_owner_drops(), 0);
        assert_eq!(prover_secret_point_owner_drops(), 0);
        Ok(())
    })();
    assert_eq!(helios_error, Err(FcmpNativeErrorV1::ArithmeticInvariant));
    assert_eq!(prover_secret_scalar_owner_drops(), 1);
    assert_eq!(prover_secret_point_owner_drops(), 1);
}
#[test]
fn prepared_cycle_blind_owners_clear_on_unwind_for_both_curves() {
    reset_prover_secret_scalar_owner_drops();
    reset_prover_secret_point_owner_drops();
    let selene_unwind = std::panic::catch_unwind(|| {
        let blind = prepare_selene_blind(ProverSecretScalarV1(Field25519::ONE))
            .expect("prepared Selene blind before unwind");
        let coordinates = blind
            .point
            .expose_ref()
            .secret_coordinates_ref_v1()
            .expect("borrowed Selene coordinates before unwind");
        assert_eq!(prover_secret_scalar_owner_drops(), 0);
        assert_eq!(prover_secret_point_owner_drops(), 0);
        let _ = core::hint::black_box(coordinates.component_pair_ref());
        panic!("exercise prepared Selene blind unwind");
    });
    assert!(selene_unwind.is_err());
    assert_eq!(prover_secret_scalar_owner_drops(), 1);
    assert_eq!(prover_secret_point_owner_drops(), 1);

    reset_prover_secret_scalar_owner_drops();
    reset_prover_secret_point_owner_drops();
    let helios_unwind = std::panic::catch_unwind(|| {
        let blind = prepare_helios_blind(ProverSecretScalarV1(HelioseleneField::ONE))
            .expect("prepared Helios blind before unwind");
        let coordinates = blind
            .point
            .expose_ref()
            .secret_coordinates_ref_v1()
            .expect("borrowed Helios coordinates before unwind");
        assert_eq!(prover_secret_scalar_owner_drops(), 0);
        assert_eq!(prover_secret_point_owner_drops(), 0);
        let _ = core::hint::black_box(coordinates.component_pair_ref());
        panic!("exercise prepared Helios blind unwind");
    });
    assert!(helios_unwind.is_err());
    assert_eq!(prover_secret_scalar_owner_drops(), 1);
    assert_eq!(prover_secret_point_owner_drops(), 1);
}
#[test]
fn root_nonce_commitment_encoding_clears_both_point_owners_on_every_exit() {
    let selene_nonce = Field25519::ONE.add_ref(&Field25519::ONE);
    let expected_selene = selene_bp_generators().h.scale(selene_nonce).encode();
    reset_prover_secret_point_owner_drops();
    let mut selene_commitment =
        root_nonce_commitment_v1::<SeleneSuite>(&selene_nonce).expect("Selene nonce commitment");
    assert_eq!(
        selene_commitment
            .encode_public_and_clear_v1()
            .expect("nonidentity Selene commitment encoding"),
        expected_selene,
        "Selene commitment remains h times nonce"
    );
    assert!(selene_commitment.expose_ref().is_identity());
    drop(selene_commitment);
    assert_eq!(prover_secret_point_owner_drops(), 1);

    let helios_nonce = HelioseleneField::ONE.add_ref(&HelioseleneField::ONE);
    let expected_helios = helios_bp_generators().h.scale(helios_nonce).encode();
    let mut helios_commitment =
        root_nonce_commitment_v1::<HeliosSuite>(&helios_nonce).expect("Helios nonce commitment");
    assert_eq!(
        helios_commitment
            .encode_public_and_clear_v1()
            .expect("nonidentity Helios commitment encoding"),
        expected_helios,
        "Helios commitment remains h times nonce"
    );
    assert!(helios_commitment.expose_ref().is_identity());
    drop(helios_commitment);
    assert_eq!(prover_secret_point_owner_drops(), 2);

    reset_prover_secret_point_owner_drops();
    let mut identity = ProverSecretPointV1(SelenePoint::identity());
    assert_eq!(
        identity.encode_public_and_clear_v1(),
        Err(FcmpNativeErrorV1::CyclePointIdentity)
    );
    assert!(identity.expose_ref().is_identity());
    drop(identity);
    assert_eq!(prover_secret_point_owner_drops(), 1);

    reset_prover_secret_point_owner_drops();
    let mut identity = ProverSecretPointV1(HeliosPoint::identity());
    assert_eq!(
        identity.encode_public_and_clear_v1(),
        Err(FcmpNativeErrorV1::CyclePointIdentity)
    );
    assert!(identity.expose_ref().is_identity());
    drop(identity);
    assert_eq!(prover_secret_point_owner_drops(), 1);

    reset_prover_secret_point_owner_drops();
    let later_error = (|| -> Result<(), FcmpNativeErrorV1> {
        let mut commitment = root_nonce_commitment_v1::<SeleneSuite>(&Field25519::ONE)?;
        let _public_commitment = commitment.encode_public_and_clear_v1()?;
        assert!(commitment.expose_ref().is_identity());
        Err(FcmpNativeErrorV1::ArithmeticInvariant)
    })();
    assert_eq!(later_error, Err(FcmpNativeErrorV1::ArithmeticInvariant));
    assert_eq!(prover_secret_point_owner_drops(), 1);

    reset_prover_secret_point_owner_drops();
    let unwind = std::panic::catch_unwind(|| {
        let mut commitment = root_nonce_commitment_v1::<HeliosSuite>(&HelioseleneField::ONE)
            .expect("owned Helios commitment before unwind");
        let _public_commitment = commitment
            .encode_public_and_clear_v1()
            .expect("nonidentity Helios commitment encoding before unwind");
        assert!(commitment.expose_ref().is_identity());
        panic!("exercise root nonce commitment owner unwind");
    });
    assert!(unwind.is_err());
    assert_eq!(prover_secret_point_owner_drops(), 1);
}
#[test]
fn root_blind_response_encoding_clears_both_nonce_owners_on_every_exit() {
    reset_prover_secret_scalar_owner_drops();
    let mut selene_nonce = ProverSecretScalarV1(Field25519::ONE);
    selene_nonce.add_product_assign(&Field25519::ONE, &Field25519::ONE);
    let expected_selene = Field25519::ONE.add_ref(&Field25519::ONE).encode();
    assert_eq!(
        selene_nonce.encode_public_and_clear_v1(),
        expected_selene,
        "Selene response remains challenge times mask plus nonce"
    );
    assert_eq!(selene_nonce.expose_ref(), &Field25519::ZERO);
    assert_eq!(prover_secret_scalar_owner_drops(), 1);
    drop(selene_nonce);
    assert_eq!(prover_secret_scalar_owner_drops(), 2);

    let mut helios_nonce = ProverSecretScalarV1(HelioseleneField::ONE);
    helios_nonce.add_product_assign(&HelioseleneField::ONE, &HelioseleneField::ONE);
    let expected_helios = HelioseleneField::ONE
        .add_ref(&HelioseleneField::ONE)
        .encode();
    assert_eq!(
        helios_nonce.encode_public_and_clear_v1(),
        expected_helios,
        "Helios response remains challenge times mask plus nonce"
    );
    assert_eq!(helios_nonce.expose_ref(), &HelioseleneField::ZERO);
    assert_eq!(prover_secret_scalar_owner_drops(), 3);
    drop(helios_nonce);
    assert_eq!(prover_secret_scalar_owner_drops(), 4);

    reset_prover_secret_scalar_owner_drops();
    let later_error = (|| -> Result<(), FcmpNativeErrorV1> {
        let mut nonce = ProverSecretScalarV1(Field25519::ONE);
        let _response = nonce.encode_public_and_clear_v1();
        assert_eq!(nonce.expose_ref(), &Field25519::ZERO);
        assert_eq!(prover_secret_scalar_owner_drops(), 1);
        Err(FcmpNativeErrorV1::ArithmeticInvariant)
    })();
    assert_eq!(later_error, Err(FcmpNativeErrorV1::ArithmeticInvariant));
    assert_eq!(prover_secret_scalar_owner_drops(), 2);

    reset_prover_secret_scalar_owner_drops();
    let unwind = std::panic::catch_unwind(|| {
        let mut nonce = ProverSecretScalarV1(HelioseleneField::ONE);
        let _response = nonce.encode_public_and_clear_v1();
        assert_eq!(nonce.expose_ref(), &HelioseleneField::ZERO);
        assert_eq!(prover_secret_scalar_owner_drops(), 1);
        panic!("exercise root-blind response owner unwind");
    });
    assert!(unwind.is_err());
    assert_eq!(prover_secret_scalar_owner_drops(), 2);
}
#[test]
fn membership_prover_retries_only_prover_honest_aborts_at_a_fixed_bound() {
    let mut attempts = 0;
    let recovered = retry_membership_prover(|| {
        attempts += 1;
        match attempts {
            1 => Err(FcmpNativeErrorV1::TranscriptChallengeExhausted),
            2 => Err(FcmpNativeErrorV1::DlogChallengeExhausted),
            3 => Err(FcmpNativeErrorV1::DlogWitnessPole),
            4 => Err(FcmpNativeErrorV1::CircuitProverCommitmentIdentity),
            5 => Err(FcmpNativeErrorV1::InnerProductRoundIdentity),
            _ => Ok(17_u8),
        }
    })
    .expect("sixth attempt succeeds");
    assert_eq!(recovered, 17);
    assert_eq!(attempts, 6);
    for retryable in [
        FcmpNativeErrorV1::TranscriptChallengeExhausted,
        FcmpNativeErrorV1::DlogChallengeExhausted,
        FcmpNativeErrorV1::DlogWitnessPole,
        FcmpNativeErrorV1::CircuitProverCommitmentIdentity,
        FcmpNativeErrorV1::InnerProductRoundIdentity,
    ] {
        attempts = 0;
        assert_eq!(
            retry_membership_prover::<()>(|| {
                attempts += 1;
                Err(retryable)
            }),
            Err(FcmpNativeErrorV1::MembershipProverRestartExhausted)
        );
        assert_eq!(attempts, MAX_MEMBERSHIP_PROVER_RESTARTS_V1);
    }
    for non_retryable in [
        FcmpNativeErrorV1::ArithmeticInvariant,
        FcmpNativeErrorV1::CircuitEquation,
    ] {
        attempts = 0;
        assert_eq!(
            retry_membership_prover::<()>(|| {
                attempts += 1;
                Err(non_retryable)
            }),
            Err(non_retryable)
        );
        assert_eq!(attempts, 1);
    }
}
#[test]
#[ignore = "manual release resource audit; run under `/usr/bin/time -l` for peak RSS"]
fn maximum_compiled_shape_release_resource_audit() {
    // Reproduce on macOS with:
    // /usr/bin/time -l cargo test -p iroha_core --release --lib
    // privacy_engines::fcmp_plus_plus::prover::tests::runtime::maximum_compiled_shape_release_resource_audit
    // -- --ignored --exact --nocapture --test-threads=1
    let setup_started = std::time::Instant::now();
    let (inputs, output_openings, root) = maximum_bound_fixture();
    let setup_ms = setup_started.elapsed().as_millis();
    let context = [0xa5_u8; 32];
    let mut rng = StdRng::seed_from_u64(0xfcff_ff01);
    let prove_started = std::time::Instant::now();
    let bundle = prove_fcmp_plus_plus_v1(&mut rng, context, &inputs, &output_openings, root)
        .expect("maximum-bound native proof");
    let prove_ms = prove_started.elapsed().as_millis();
    assert_eq!(bundle.proof_wire().len(), FCMP_MAX_PROOF_WIRE_BYTES_V1);
    let outputs = output_openings
        .iter()
        .map(FcmpOutputCommitmentOpeningV1::output)
        .collect::<Vec<_>>();
    let verify_started = std::time::Instant::now();
    verify_fcmp_transaction_v1(
        context,
        bundle.proof_wire(),
        bundle.public_inputs(),
        &outputs,
        root,
    )
    .expect("maximum-bound transaction verifies");
    let verify_ms = verify_started.elapsed().as_millis();
    let wire_bytes = bundle.proof_wire().len();
    eprintln!(
        "FCMP_RESOURCE_V1 inputs={} layers={} outputs={} wire_bytes={wire_bytes} \
         setup_ms={setup_ms} prove_ms={prove_ms} verify_ms={verify_ms}",
        inputs.len(),
        root.layers(),
        outputs.len(),
    );
}
#[test]
fn membership_rng_unavailability_fails_without_calling_infallible_rng_methods() {
    reset_prover_secret_scalar_owner_drops();
    assert!(matches!(
        random_proof_scalar::<Field25519>(&mut FailingRngV1),
        Err(FcmpNativeErrorV1::RandomnessUnavailable)
    ));
    assert_eq!(prover_secret_scalar_owner_drops(), 0);
}
#[test]
fn public_prover_rejects_unavailable_and_short_period_entropy_before_proving() {
    let context = [0x90_u8; 32];
    let (input, output, root) = one_layer_fixture();
    assert_eq!(
        prove_fcmp_plus_plus_v1(
            &mut FailingRngV1,
            context,
            std::slice::from_ref(&input),
            std::slice::from_ref(&output),
            root,
        ),
        Err(FcmpNativeErrorV1::RandomnessUnavailable)
    );
    for period in [1, 2, 4, 8, 16, 32] {
        let mut rng = PeriodicRng { period, cursor: 0 };
        assert_eq!(
            prove_fcmp_plus_plus_v1(
                &mut rng,
                context,
                std::slice::from_ref(&input),
                std::slice::from_ref(&output),
                root,
            ),
            Err(FcmpNativeErrorV1::RandomnessHealthCheckFailed),
            "period-{period} source was not rejected"
        );
    }
}
#[test]
fn deterministic_preflight_errors_take_precedence_over_entropy_failure() {
    let context = [0x90_u8; 32];
    let (input, _, root) = one_layer_fixture();
    assert_eq!(
        prove_fcmp_plus_plus_v1(&mut FailingRngV1, context, &[], &[], root),
        Err(FcmpNativeErrorV1::InputCount {
            actual: 0,
            max: FCMP_MAX_INPUTS_NATIVE_V1,
        })
    );
    let unbalanced_output = output_opening(43, 47, TEST_AMOUNT, 999);
    assert_eq!(
        prove_fcmp_plus_plus_v1(
            &mut FailingRngV1,
            context,
            std::slice::from_ref(&input),
            std::slice::from_ref(&unbalanced_output),
            root,
        ),
        Err(FcmpNativeErrorV1::CommitmentBalanceEquation)
    );
}
const LEGACY_PROVER_TEST_NAMES_V1: [&str; 39] = [
    "prover_copy_owner_clears_transfer_success_and_unwind_slots",
    "fixture_spendable_output_owns_inputs_and_secret_outputs_on_every_exit",
    "fixture_spendable_output_source_stays_owned_through_release_transfer",
    "fixture_u64_wrapper_owns_slots_on_success_error_and_inner_unwind",
    "fixture_u64_wrapper_source_takes_every_slot_before_inner_conversion",
    "fixture_output_opening_owns_success_error_mismatch_and_unwind_slots",
    "fixture_output_opening_source_stays_owned_until_borrowed_constructor",
    "fixture_rerandomization_owns_success_error_and_unwind_slots",
    "fixture_rerandomization_source_keeps_feature_secret_owners_in_order",
    "fixture_leaf_coordinate_scope_owns_success_error_and_unwind",
    "fixture_secret_selene_hash_matches_equation_and_owns_all_exit_paths",
    "fixture_secret_cycle_step_matches_public_equations_and_owns_copies",
    "fixture_secret_cycle_source_has_no_raw_coordinate_hash_or_branch_boundary",
    "fixture_leaf_coordinate_buffer_zeroizes_on_drop_and_unwind",
    "fixture_leaf_coordinate_source_keeps_exact_erasing_owners_through_hash",
    "fixture_secret_selene_hash_source_uses_borrowed_exact_builder_and_owned_result",
    "rerandomization_constructor_takes_all_bytes_before_decoding",
    "prover_input_constructor_takes_secret_bytes_before_validation",
    "public_input_keeps_private_products_in_borrowed_erasing_owners",
    "commitment_mask_openings_remain_borrowed_until_the_membership_boundary",
    "prover_witness_debug_is_redacted_and_explicit_zeroize_covers_the_full_path",
    "constant_work_scan_primitives_visit_every_element_and_pair",
    "typed_membership_and_duplicate_scans_cover_every_position",
    "hidden_leaf_membership_and_duplicates_cover_first_middle_last_and_absent",
    "shared_root_scan_covers_first_middle_last_and_absent_mismatches",
    "private_push_guard_forbids_vector_growth",
    "maximum_compiled_shape_has_canonical_paths_and_exact_resource_bound",
    "malicious_zero_rng_exhausts_a_fixed_bound_instead_of_hanging",
    "sampled_scalar_slots_are_owned_before_rejection_or_return",
    "membership_prover_retries_only_prover_honest_aborts_at_a_fixed_bound",
    "maximum_compiled_shape_release_resource_audit",
    "membership_rng_unavailability_fails_without_calling_infallible_rng_methods",
    "public_prover_rejects_unavailable_and_short_period_entropy_before_proving",
    "deterministic_preflight_errors_take_precedence_over_entropy_failure",
    "native_one_layer_prover_round_trips_end_to_end",
    "native_two_layer_prover_exercises_alternating_curve_path",
    "native_two_input_prover_round_trips_at_the_compiled_bound",
    "prover_rejects_duplicate_outputs_key_images_and_input_overflow_preflight",
    "prover_paths_reject_reordered_omitted_and_duplicated_layers",
];
#[test]
fn extracted_prover_test_module_retains_every_legacy_regression() {
    let source = concat!(
        include_str!("../tests.rs"),
        include_str!("commitment_mask.rs"),
        include_str!("runtime.rs"),
    );
    assert_eq!(LEGACY_PROVER_TEST_NAMES_V1.len(), 39);
    for name in LEGACY_PROVER_TEST_NAMES_V1 {
        let anchor = format!("fn {name}(");
        assert_eq!(
            source.matches(&anchor).count(),
            1,
            "legacy prover regression {name} is missing or duplicated"
        );
    }
    assert!(!source.contains("include!(\"tests/"));
}
#[test]
fn native_one_layer_prover_round_trips_end_to_end() {
    let context = [0x91_u8; 32];
    let (input, new_output, root) = one_layer_fixture();
    let mut rng = StdRng::seed_from_u64(0xfc_0001);
    let bundle = prove_fcmp_plus_plus_v1(
        &mut rng,
        context,
        &[input],
        std::slice::from_ref(&new_output),
        root,
    )
    .expect("native proof");
    let wire_digest: [u8; 32] = Sha256::digest(bundle.proof_wire()).into();
    let mut public_digest = Sha256::new();
    for public in bundle.public_inputs() {
        for field in [
            public.output_key_tilde,
            public.linking_tag_generator_tilde,
            public.rerandomization_commitment,
            public.pseudo_out,
            public.key_image,
        ] {
            public_digest.update(field);
        }
    }
    let public_digest: [u8; 32] = public_digest.finalize().into();
    // Pin the complete Iroha transfer wire and public relation. The
    // membership-only differential fixtures separately exercise the exact
    // upstream Ed25519, Selene, and Helios equations.
    assert_eq!(
        wire_digest, FCMP_NATIVE_KAT_WIRE_SHA256_V1,
        "deterministic IFC1 bytes drifted"
    );
    assert_eq!(
        public_digest, FCMP_NATIVE_KAT_PUBLIC_SHA256_V1,
        "deterministic public relation drifted"
    );
    assert_eq!(
        bundle.proof_wire().len(),
        fcmp_plus_plus_wire_size_v1(1, 1, 1).expect("wire size")
    );
    verify_fcmp_plus_plus_v1(context, bundle.proof_wire(), bundle.public_inputs(), root)
        .expect("native proof verifies");
    verify_fcmp_transaction_v1(
        context,
        bundle.proof_wire(),
        bundle.public_inputs(),
        &[new_output.output()],
        root,
    )
    .expect("complete native transaction verifies");
    let range_size = super::super::super::fcmp_range_proof_size_v1(1).expect("range proof size");
    let range_start = bundle.proof_wire().len() - range_size;
    for offset in [
        range_start,
        range_start + (range_size / 2),
        bundle.proof_wire().len() - 1,
    ] {
        let mut mutation = bundle.proof_wire().to_vec();
        mutation[offset] ^= 1;
        assert!(
            verify_fcmp_transaction_v1(
                context,
                &mutation,
                bundle.public_inputs(),
                &[new_output.output()],
                root,
            )
            .is_err(),
            "complete verifier accepted range-proof mutation at {offset}"
        );
    }
    let mut mismatching_output_count = bundle.proof_wire().to_vec();
    mismatching_output_count[6] = 2;
    assert!(
        verify_fcmp_transaction_v1(
            context,
            &mismatching_output_count,
            bundle.public_inputs(),
            &[new_output.output()],
            root,
        )
        .is_err()
    );
    let mut mutation = bundle.proof_wire().to_vec();
    let middle = mutation.len() / 2;
    mutation[middle] ^= 1;
    assert!(verify_fcmp_plus_plus_v1(context, &mutation, bundle.public_inputs(), root).is_err());
    let wrong_root = build_fcmp_frontier_v1(&[spendable_output(
        Scalar::from(41_u64),
        Scalar::from(43_u64),
        Scalar::from(47_u64),
        Scalar::from(53_u64),
    )])
    .expect("other tree")
    .root;
    assert!(
        verify_fcmp_plus_plus_v1(
            context,
            bundle.proof_wire(),
            bundle.public_inputs(),
            wrong_root,
        )
        .is_err()
    );
}
#[test]
fn native_two_layer_prover_exercises_alternating_curve_path() {
    let context = [0x92_u8; 32];
    let x = Scalar::from(101_u64);
    let y = Scalar::from(103_u64);
    let output = spendable_output(x, y, Scalar::from(107_u64), Scalar::from(109_u64));
    let mut outputs = (0..FCMP_LAYER_ONE_LEN_V1)
        .map(|index| {
            let base = 1_000 + (u64::try_from(index).expect("index") * 3);
            output_from_multiples(base, base + 1, base + 2)
        })
        .collect::<Vec<_>>();
    outputs.push(output);
    let frontier = build_fcmp_frontier_v1(&outputs).expect("two-layer tree");
    assert_eq!(frontier.root.layers(), 2);
    assert_eq!(frontier.active_outputs, vec![output]);
    assert_eq!(frontier.levels.len(), 1);
    let mut coordinates = Vec::new();
    let (output_key, linking_tag_generator, commitment) = output.components();
    for point in [output_key, linking_tag_generator, commitment] {
        let (x, y) = edwards_to_wei25519(point).expect("coordinates");
        coordinates.extend([x, y]);
    }
    let active_leaf = hash_selene(&coordinates).expect("active leaf");
    let mut root_branch = duplicate_zeroizing_slice(&frontier.levels[0]);
    root_branch.push(encode_helioselene_scalar(
        active_leaf.x().expect("nonidentity leaf"),
    ));
    let input = FcmpProverInputV1::new(
        output,
        x.to_bytes(),
        y.to_bytes(),
        rerandomization(137, 139, 149, 113),
        vec![output],
        vec![core::mem::take(&mut *root_branch)],
    )
    .expect("two-layer witness");
    let new_output = output_opening(127, 131, TEST_AMOUNT, 109 + 113);
    let mut rng = StdRng::seed_from_u64(0xfc_0002);
    let bundle = prove_fcmp_plus_plus_v1(
        &mut rng,
        context,
        &[input],
        std::slice::from_ref(&new_output),
        frontier.root,
    )
    .expect("native two-layer proof");
    assert_eq!(
        bundle.proof_wire().len(),
        fcmp_plus_plus_wire_size_v1(1, 2, 1).expect("wire size")
    );
    verify_fcmp_plus_plus_v1(
        context,
        bundle.proof_wire(),
        bundle.public_inputs(),
        frontier.root,
    )
    .expect("two-layer native proof verifies");
}
#[test]
fn native_two_input_prover_round_trips_at_the_compiled_bound() {
    let context = [0x93_u8; 32];
    let x_1 = Scalar::from(113_u64);
    let y_1 = Scalar::from(127_u64);
    let x_2 = Scalar::from(131_u64);
    let y_2 = Scalar::from(137_u64);
    let output_1 = spendable_output(x_1, y_1, Scalar::from(139_u64), Scalar::from(149_u64));
    let output_2 = spendable_output(x_2, y_2, Scalar::from(151_u64), Scalar::from(157_u64));
    let mut leaves = Zeroizing::new(vec![output_1, output_2]);
    let root = build_fcmp_frontier_v1(&leaves).expect("tree").root;
    let mut first_leaves = duplicate_zeroizing_slice(&leaves);
    let inputs = [
        FcmpProverInputV1::new(
            output_1,
            x_1.to_bytes(),
            y_1.to_bytes(),
            rerandomization(181, 191, 193, 163),
            core::mem::take(&mut *first_leaves),
            Vec::new(),
        )
        .expect("first witness"),
        FcmpProverInputV1::new(
            output_2,
            x_2.to_bytes(),
            y_2.to_bytes(),
            rerandomization(197, 199, 211, 167),
            core::mem::take(&mut *leaves),
            Vec::new(),
        )
        .expect("second witness"),
    ];
    let new_output = output_opening(173, 179, TEST_AMOUNT * 2, 149 + 163 + 157 + 167);
    let mut rng = StdRng::seed_from_u64(0xfc_0003);
    let bundle = prove_fcmp_plus_plus_v1(
        &mut rng,
        context,
        &inputs,
        std::slice::from_ref(&new_output),
        root,
    )
    .expect("two-input proof");
    assert_eq!(
        bundle.proof_wire().len(),
        fcmp_plus_plus_wire_size_v1(FCMP_MAX_INPUTS_NATIVE_V1, 1, 1).expect("wire size")
    );
    verify_fcmp_plus_plus_v1(context, bundle.proof_wire(), bundle.public_inputs(), root)
        .expect("two-input proof verifies");
    let mut duplicate_key_image = bundle.public_inputs().to_vec();
    duplicate_key_image[1].key_image = duplicate_key_image[0].key_image;
    assert_eq!(
        verify_fcmp_plus_plus_v1(context, bundle.proof_wire(), &duplicate_key_image, root,),
        Err(FcmpNativeErrorV1::DuplicateKeyImage)
    );
    let mut duplicate_pseudo_out = bundle.public_inputs().to_vec();
    duplicate_pseudo_out[1].pseudo_out = duplicate_pseudo_out[0].pseudo_out;
    assert_eq!(
        verify_fcmp_plus_plus_v1(context, bundle.proof_wire(), &duplicate_pseudo_out, root,),
        Err(FcmpNativeErrorV1::DuplicatePseudoOut)
    );
}
#[test]
fn prover_rejects_duplicate_outputs_key_images_and_input_overflow_preflight() {
    let x = Scalar::from(163_u64);
    let first = spendable_output(
        x,
        Scalar::from(167_u64),
        Scalar::from(173_u64),
        Scalar::from(179_u64),
    );
    assert!(matches!(
        FcmpProverInputV1::new(
            first,
            x.to_bytes(),
            Scalar::from(167_u64).to_bytes(),
            rerandomization(211, 223, 227, 181),
            vec![first, first],
            Vec::new(),
        ),
        Err(FcmpNativeErrorV1::DuplicateOutput)
    ));
    let second = spendable_output(
        x,
        Scalar::from(181_u64),
        Scalar::from(173_u64),
        Scalar::from(191_u64),
    );
    let mut leaves = Zeroizing::new(vec![first, second]);
    let root = build_fcmp_frontier_v1(&leaves).expect("tree").root;
    let mut first_leaves = duplicate_zeroizing_slice(&leaves);
    let first_input = FcmpProverInputV1::new(
        first,
        x.to_bytes(),
        Scalar::from(167_u64).to_bytes(),
        rerandomization(229, 233, 239, 193),
        core::mem::take(&mut *first_leaves),
        Vec::new(),
    )
    .expect("first input");
    let second_input = FcmpProverInputV1::new(
        second,
        x.to_bytes(),
        Scalar::from(181_u64).to_bytes(),
        rerandomization(241, 251, 257, 197),
        core::mem::take(&mut *leaves),
        Vec::new(),
    )
    .expect("second input");
    let new_output = output_opening(199, 211, TEST_AMOUNT, 179 + 193);
    let mut rng = StdRng::seed_from_u64(0xfc_0004);
    let duplicate_output_a = first_input.duplicate_for_test();
    let duplicate_output_b = first_input.duplicate_for_test();
    assert_eq!(
        prove_fcmp_plus_plus_v1(
            &mut rng,
            [0x94; 32],
            &[duplicate_output_a, duplicate_output_b],
            std::slice::from_ref(&new_output),
            root,
        ),
        Err(FcmpNativeErrorV1::DuplicateOutput)
    );
    let duplicate_key_image = first_input.duplicate_for_test();
    assert_eq!(
        prove_fcmp_plus_plus_v1(
            &mut rng,
            [0x94; 32],
            &[duplicate_key_image, second_input],
            std::slice::from_ref(&new_output),
            root,
        ),
        Err(FcmpNativeErrorV1::DuplicateKeyImage)
    );
    let overflow_a = first_input.duplicate_for_test();
    let overflow_b = first_input.duplicate_for_test();
    assert!(matches!(
        prove_fcmp_plus_plus_v1(
            &mut rng,
            [0x94; 32],
            &[overflow_a, overflow_b, first_input],
            std::slice::from_ref(&new_output),
            root,
        ),
        Err(FcmpNativeErrorV1::InputCount {
            actual: 3,
            max: FCMP_MAX_INPUTS_NATIVE_V1
        })
    ));
}
#[test]
fn prover_paths_reject_reordered_omitted_and_duplicated_layers() {
    let x = Scalar::from(193_u64);
    let y = Scalar::from(197_u64);
    let output = spendable_output(x, y, Scalar::from(199_u64), Scalar::from(211_u64));
    let completed_capacity = FCMP_LAYER_ONE_LEN_V1 * FCMP_LAYER_TWO_LEN_V1;
    let mut outputs = (0..completed_capacity)
        .map(|index| {
            let base = 20_000 + (u64::try_from(index).expect("index") * 3);
            output_from_multiples(base, base + 1, base + 2)
        })
        .collect::<Vec<_>>();
    outputs.push(output);
    let frontier = build_fcmp_frontier_v1(&outputs).expect("three-layer tree");
    assert_eq!(frontier.root.layers(), 3);
    assert_eq!(frontier.active_outputs, vec![output]);
    assert_eq!(frontier.levels.len(), 2);
    assert!(frontier.levels[0].is_empty());
    let mut coordinates = Vec::new();
    let (output_key, linking_tag_generator, commitment) = output.components();
    for point in [output_key, linking_tag_generator, commitment] {
        let (x, y) = edwards_to_wei25519(point).expect("coordinates");
        coordinates.extend([x, y]);
    }
    let leaf = hash_selene(&coordinates).expect("leaf");
    let leaf_x = leaf.x().expect("nonidentity leaf");
    let first_branch = vec![encode_helioselene_scalar(leaf_x)];
    let active_helios = hash_helios(&[leaf_x]).expect("second layer");
    let mut second_branch = duplicate_zeroizing_slice(&frontier.levels[1]);
    second_branch.push(encode_field25519_scalar(
        active_helios.x().expect("nonidentity second layer"),
    ));
    let valid = FcmpProverInputV1::new(
        output,
        x.to_bytes(),
        y.to_bytes(),
        rerandomization(227, 229, 233, 223),
        vec![output],
        vec![first_branch, core::mem::take(&mut *second_branch)],
    )
    .expect("canonical path");
    parse_path(&valid, frontier.root).expect("canonical path resolves");
    let mut reordered = valid.duplicate_for_test();
    reordered.additional_branches.swap(0, 1);
    assert!(matches!(
        parse_path(&reordered, frontier.root),
        Err(FcmpNativeErrorV1::ArithmeticInvariant)
    ));
    let mut omitted = valid.duplicate_for_test();
    omitted.additional_branches.remove(0);
    assert!(matches!(
        parse_path(&omitted, frontier.root),
        Err(FcmpNativeErrorV1::ProofHeaderMismatch)
    ));
    let mut duplicated = valid.duplicate_for_test();
    duplicated
        .additional_branches
        .push(valid.additional_branches[0].duplicate_for_test());
    assert!(matches!(
        parse_path(&duplicated, frontier.root),
        Err(FcmpNativeErrorV1::ProofHeaderMismatch)
    ));
}
