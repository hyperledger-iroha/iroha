use super::super::super::field::{
    reset_secret_cycle_scalar_decoder_owner_drops_v1, secret_cycle_scalar_decoder_owner_drops_v1,
};
use super::*;

#[test]
fn prover_input_constructor_takes_secret_bytes_before_validation() {
    let source = include_str!("../../prover.rs");
    let constructor = source_part!(source;
        "impl FcmpProverInputV1 {" => "#[cfg(test)]\n    fn duplicate_for_test");
    assert_source_point_order(
        constructor,
        &[
            SourcePoint::Last("ProverSecretCopyValueV1::take(&mut"),
            SourcePoint::First("let spend_x_encoding ="),
            SourcePoint::First("let output_y_encoding ="),
            SourcePoint::First("let mut spend_x_scalar = spend_x_encoding.into_scalar_owner_v1()"),
            SourcePoint::First(
                "let mut output_y_scalar = output_y_encoding.into_scalar_owner_v1()",
            ),
            SourcePoint::First("if leaves.is_empty()"),
            SourcePoint::First("let output_present = ct_digest_slice_contains("),
            SourcePoint::First("let duplicate_leaf = ct_has_duplicate_digests("),
            SourcePoint::First("let zero_spend = bool::from("),
            SourcePoint::First("if zero_spend || !output_present"),
            SourcePoint::First("if duplicate_leaf"),
            SourcePoint::Last("decoded.push("),
            SourcePoint::First("let mut input = Self {"),
            SourcePoint::First("core::mem::swap(&mut input.spend_x, &mut spend_x_scalar.0)"),
            SourcePoint::First("drop(spend_x_scalar)"),
            SourcePoint::First("core::mem::swap(&mut input.output_y, &mut output_y_scalar.0)"),
            SourcePoint::First("drop(output_y_scalar)"),
            SourcePoint::First("Ok(input)"),
        ],
    );
    source_has!(constructor; "spend_x: Scalar::ZERO", "output_y: Scalar::ZERO");
    source_lacks!(constructor;
        "Zeroizing::new(spend_x)",
        "Zeroizing::new(output_y)",
        ".expose_copy()",
        "callback",
        "FnOnce",
        "validate_edwards_scalar",
        "Scalar::from_canonical_bytes",
        "CtOption",
        "Option::<Scalar>",
        ".filter(",
        "*spend_x_bytes.expose_ref()",
        "*output_y_bytes.expose_ref()",
        "U256",
        "from_le_slice",
        "from_le_bytes",
        "Result<Scalar",
        ".clone()",
        "Deref",
        "decoded_branch.push(decode_",
        "decode_helioselene_scalar(*encoded)",
        "decode_field25519_scalar(*encoded)",
    );
    source_counts!(constructor;
        "ProverSecretCopyValueV1::take(&mut" => 2,
        "core::mem::swap(" => 2,
        "drop(" => 2,
        "ProverValidatedSecretEdwardsScalarEncodingV1::validate_v1(" => 2,
        ".into_scalar_owner_v1()" => 2,
        "let scalar = decode_secret_helioselene_scalar_v1(encoded)?" => 1,
        "let scalar = decode_secret_field25519_scalar_v1(encoded)?" => 1,
        "require_preallocated_push(decoded_branch.len(), decoded_branch.capacity())?" => 2,
        "push_owned_secret_cycle_scalar_v1(&mut decoded_branch, scalar)?" => 2,
        "push_secret_scalar_v1(" => 0,
    );
    assert_source_point_order(
        constructor,
        &[
            SourcePoint::Nth(
                "require_preallocated_push(decoded_branch.len(), decoded_branch.capacity())?",
                0,
            ),
            SourcePoint::First("let scalar = decode_secret_helioselene_scalar_v1(encoded)?"),
            SourcePoint::Nth(
                "push_owned_secret_cycle_scalar_v1(&mut decoded_branch, scalar)?",
                0,
            ),
            SourcePoint::Nth(
                "require_preallocated_push(decoded_branch.len(), decoded_branch.capacity())?",
                1,
            ),
            SourcePoint::First("let scalar = decode_secret_field25519_scalar_v1(encoded)?"),
            SourcePoint::Nth(
                "push_owned_secret_cycle_scalar_v1(&mut decoded_branch, scalar)?",
                1,
            ),
        ],
    );
    let lifecycle = source_part!(source;
        "impl Zeroize for FcmpProverInputV1" =>
        "impl core::fmt::Debug for FcmpProverInputV1");
    source_counts!(lifecycle; ".zeroize()" => 7);
    source_has!(lifecycle; "impl Drop for FcmpProverInputV1", "self.zeroize();");
    let owned_cycle_push = source_part!(source;
        "fn push_owned_secret_cycle_scalar_v1<F: ProofScalar + Zeroize>(" =>
        "fn push_secret_scalar_v1<F: ProofScalar + Zeroize>(");
    assert_source_point_order(
        owned_cycle_push,
        &[
            SourcePoint::First("let allocation_capacity = values.capacity()"),
            SourcePoint::First("let allocation_ptr = values.as_ptr()"),
            SourcePoint::First(
                "let preflight = require_preallocated_push(values.len(), allocation_capacity)",
            ),
            SourcePoint::First("if let Err(error) = preflight"),
            SourcePoint::First("drop(value)"),
            SourcePoint::First("values.push(F::ZERO)"),
            SourcePoint::First("let destination = values.len() - 1"),
            SourcePoint::First("value.move_into(&mut values[destination])"),
            SourcePoint::Nth(
                "debug_assert_eq!(values.capacity(), allocation_capacity)",
                1,
            ),
            SourcePoint::Nth("debug_assert_eq!(values.as_ptr(), allocation_ptr)", 1),
        ],
    );
    source_counts!(owned_cycle_push;
        "debug_assert_eq!(values.capacity(), allocation_capacity)" => 2,
        "debug_assert_eq!(values.as_ptr(), allocation_ptr)" => 2,
        "drop(value)" => 1,
        "values.push(F::ZERO)" => 1,
        "value.move_into(" => 1,
    );
    source_lacks!(owned_cycle_push;
        "value.expose_copy()",
        "value.as_ref()",
        "values.push(*",
        "core::mem::swap",
        "callback",
        "FnOnce",
        "Deref",
    );
}
#[test]
fn prover_input_scalar_owner_handoff_covers_every_exit() {
    let fixture = || {
        let (output, spend_x, output_y) =
            fcmp_test_spendable_output_v1(17, 23, 31, TEST_AMOUNT, 37);
        (output, spend_x, output_y, rerandomization(61, 67, 71, 41))
    };
    let mut cycle_one = [0_u8; 32];
    cycle_one[0] = 1;

    let (output, spend_x, output_y, input_rerandomization) = fixture();
    reset_rerandomization_scalar_decoder_owner_drops();
    reset_fcmp_prover_input_owner_drops();
    let mut input = FcmpProverInputV1::new(
        output,
        spend_x,
        output_y,
        input_rerandomization,
        vec![output],
        Vec::new(),
    )
    .expect("direct prover-input scalar owner handoff");
    assert_eq!(input.spend_x, Scalar::from(17_u64));
    assert_eq!(input.output_y, Scalar::from(23_u64));
    assert_eq!(rerandomization_canonicality_owner_drops(), 2);
    assert_eq!(rerandomization_wide_input_owner_drops(), 2);
    assert_eq!(prover_secret_copy_owner_drops(), 4);
    assert_eq!(fcmp_prover_input_owner_drops(), 0);
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
    drop(input);
    assert_eq!(prover_secret_copy_owner_drops(), 4);
    assert_eq!(fcmp_prover_input_owner_drops(), 1);

    let zero_output_y_output = spendable_output(
        Scalar::from(19_u64),
        Scalar::ZERO,
        Scalar::from(31_u64),
        Scalar::from(37_u64),
    );
    let zero_output_y_rerandomization = rerandomization(61, 67, 71, 41);
    reset_rerandomization_scalar_decoder_owner_drops();
    reset_fcmp_prover_input_owner_drops();
    let zero_output_y = FcmpProverInputV1::new(
        zero_output_y_output,
        Scalar::from(19_u64).to_bytes(),
        Scalar::ZERO.to_bytes(),
        zero_output_y_rerandomization,
        vec![zero_output_y_output],
        Vec::new(),
    )
    .expect("canonical zero output-y must remain valid");
    assert_eq!(zero_output_y.output_y, Scalar::ZERO);
    assert_eq!(rerandomization_canonicality_owner_drops(), 2);
    assert_eq!(rerandomization_wide_input_owner_drops(), 2);
    assert_eq!(prover_secret_copy_owner_drops(), 4);
    zero_output_y
        .public_input()
        .expect("zero output-y opens the matching public relation");
    drop(zero_output_y);
    assert_eq!(fcmp_prover_input_owner_drops(), 1);

    let (output, _spend_x, output_y, rerandomization) = fixture();
    reset_rerandomization_scalar_decoder_owner_drops();
    reset_fcmp_prover_input_owner_drops();
    assert_eq!(
        FcmpProverInputV1::new(
            output,
            [u8::MAX; 32],
            output_y,
            rerandomization,
            vec![output],
            Vec::new(),
        )
        .err()
        .expect("noncanonical spend-x must reject"),
        FcmpNativeErrorV1::ScalarEncoding
    );
    assert_eq!(rerandomization_canonicality_owner_drops(), 1);
    assert_eq!(rerandomization_wide_input_owner_drops(), 0);
    assert_eq!(prover_secret_copy_owner_drops(), 2);
    assert_eq!(fcmp_prover_input_owner_drops(), 0);

    let (output, spend_x, _output_y, rerandomization) = fixture();
    reset_rerandomization_scalar_decoder_owner_drops();
    reset_fcmp_prover_input_owner_drops();
    assert_eq!(
        FcmpProverInputV1::new(
            output,
            spend_x,
            [u8::MAX; 32],
            rerandomization,
            vec![output],
            Vec::new(),
        )
        .err()
        .expect("noncanonical output-y must reject"),
        FcmpNativeErrorV1::ScalarEncoding
    );
    assert_eq!(rerandomization_canonicality_owner_drops(), 2);
    assert_eq!(rerandomization_wide_input_owner_drops(), 0);
    assert_eq!(prover_secret_copy_owner_drops(), 2);
    assert_eq!(fcmp_prover_input_owner_drops(), 0);

    let (output, _spend_x, output_y, rerandomization) = fixture();
    reset_rerandomization_scalar_decoder_owner_drops();
    reset_fcmp_prover_input_owner_drops();
    assert_eq!(
        FcmpProverInputV1::new(
            output,
            Scalar::ZERO.to_bytes(),
            output_y,
            rerandomization,
            vec![output],
            Vec::new(),
        )
        .err()
        .expect("zero spend-x must reject after scalar decoding"),
        FcmpNativeErrorV1::ArithmeticInvariant
    );
    assert_eq!(rerandomization_canonicality_owner_drops(), 2);
    assert_eq!(rerandomization_wide_input_owner_drops(), 2);
    assert_eq!(prover_secret_copy_owner_drops(), 4);
    assert_eq!(fcmp_prover_input_owner_drops(), 0);

    let (output, spend_x, output_y, rerandomization) = fixture();
    reset_rerandomization_scalar_decoder_owner_drops();
    reset_secret_cycle_scalar_decoder_owner_drops_v1();
    reset_fcmp_prover_input_owner_drops();
    assert_eq!(
        FcmpProverInputV1::new(
            output,
            spend_x,
            output_y,
            rerandomization,
            vec![output],
            vec![vec![[u8::MAX; 32]]],
        )
        .err()
        .expect("deep branch decode must reject before publication"),
        FcmpNativeErrorV1::ScalarEncoding
    );
    assert_eq!(rerandomization_canonicality_owner_drops(), 2);
    assert_eq!(rerandomization_wide_input_owner_drops(), 2);
    assert_eq!(prover_secret_copy_owner_drops(), 4);
    assert_eq!(secret_cycle_scalar_decoder_owner_drops_v1(), (1, 0, 0));
    assert_eq!(fcmp_prover_input_owner_drops(), 0);

    let (output, spend_x, output_y, rerandomization) = fixture();
    reset_rerandomization_scalar_decoder_owner_drops();
    reset_secret_cycle_scalar_decoder_owner_drops_v1();
    reset_fcmp_prover_input_owner_drops();
    let decoded_branches = FcmpProverInputV1::new(
        output,
        spend_x,
        output_y,
        rerandomization,
        vec![output],
        vec![vec![cycle_one], vec![cycle_one]],
    )
    .expect("both cycle scalar owners transfer into the final input");
    assert_eq!(secret_cycle_scalar_decoder_owner_drops_v1(), (2, 2, 2));
    assert_eq!(prover_secret_copy_owner_drops(), 4);
    assert_eq!(fcmp_prover_input_owner_drops(), 0);
    drop(decoded_branches);
    assert_eq!(secret_cycle_scalar_decoder_owner_drops_v1(), (2, 2, 2));
    assert_eq!(fcmp_prover_input_owner_drops(), 1);

    for invalid_position in 0..3 {
        let (output, spend_x, output_y, rerandomization) = fixture();
        let mut branch = vec![cycle_one; 3];
        branch[invalid_position] = [u8::MAX; 32];
        reset_rerandomization_scalar_decoder_owner_drops();
        reset_secret_cycle_scalar_decoder_owner_drops_v1();
        reset_fcmp_prover_input_owner_drops();
        assert_eq!(
            FcmpProverInputV1::new(
                output,
                spend_x,
                output_y,
                rerandomization,
                vec![output],
                vec![branch],
            )
            .err()
            .expect("noncanonical Helioselene branch scalar must reject"),
            FcmpNativeErrorV1::ScalarEncoding
        );
        assert_eq!(
            secret_cycle_scalar_decoder_owner_drops_v1(),
            (invalid_position + 1, invalid_position, invalid_position)
        );
        assert_eq!(prover_secret_copy_owner_drops(), 4);
        assert_eq!(fcmp_prover_input_owner_drops(), 0);
    }

    for invalid_position in 0..3 {
        let (output, spend_x, output_y, rerandomization) = fixture();
        let mut branch = vec![cycle_one; 3];
        branch[invalid_position] = [u8::MAX; 32];
        reset_rerandomization_scalar_decoder_owner_drops();
        reset_secret_cycle_scalar_decoder_owner_drops_v1();
        reset_fcmp_prover_input_owner_drops();
        assert_eq!(
            FcmpProverInputV1::new(
                output,
                spend_x,
                output_y,
                rerandomization,
                vec![output],
                vec![vec![cycle_one], branch],
            )
            .err()
            .expect("noncanonical Field25519 branch scalar must reject"),
            FcmpNativeErrorV1::ScalarEncoding
        );
        assert_eq!(
            secret_cycle_scalar_decoder_owner_drops_v1(),
            (
                invalid_position + 2,
                invalid_position + 1,
                invalid_position + 1,
            )
        );
        assert_eq!(prover_secret_copy_owner_drops(), 4);
        assert_eq!(fcmp_prover_input_owner_drops(), 0);
    }

    reset_secret_cycle_scalar_decoder_owner_drops_v1();
    let scalar = decode_secret_helioselene_scalar_v1(&cycle_one)
        .expect("owned cycle scalar before capacity error");
    let mut no_capacity = Zeroizing::new(Vec::new());
    assert_eq!(
        push_owned_secret_cycle_scalar_v1(&mut no_capacity, scalar),
        Err(FcmpNativeErrorV1::ArithmeticInvariant)
    );
    assert!(no_capacity.is_empty());
    assert_eq!(secret_cycle_scalar_decoder_owner_drops_v1(), (1, 1, 1));

    let (output, spend_x, output_y, rerandomization) = fixture();
    reset_rerandomization_scalar_decoder_owner_drops();
    reset_secret_cycle_scalar_decoder_owner_drops_v1();
    reset_fcmp_prover_input_owner_drops();
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let input = FcmpProverInputV1::new(
            output,
            spend_x,
            output_y,
            rerandomization,
            vec![output],
            vec![vec![cycle_one], vec![cycle_one]],
        )
        .expect("direct prover-input owner handoff before unwind");
        assert_eq!(rerandomization_canonicality_owner_drops(), 2);
        assert_eq!(rerandomization_wide_input_owner_drops(), 2);
        assert_eq!(prover_secret_copy_owner_drops(), 4);
        assert_eq!(secret_cycle_scalar_decoder_owner_drops_v1(), (2, 2, 2));
        assert_eq!(fcmp_prover_input_owner_drops(), 0);
        let _ = core::hint::black_box(&input);
        panic!("exercise prover-input destination-owner unwind");
    }));
    assert!(unwind.is_err());
    assert_eq!(rerandomization_canonicality_owner_drops(), 2);
    assert_eq!(rerandomization_wide_input_owner_drops(), 2);
    assert_eq!(prover_secret_copy_owner_drops(), 4);
    assert_eq!(secret_cycle_scalar_decoder_owner_drops_v1(), (2, 2, 2));
    assert_eq!(fcmp_prover_input_owner_drops(), 1);
}
#[test]
fn public_input_private_point_owners_cover_success_error_and_unwind() {
    fn assert_point_owner(_: &ProverSecretCopyValueV1<EdwardsPoint>) {}

    let (mut input, _output, _root) = one_layer_fixture();
    reset_prover_secret_copy_owner_drops();
    let first = input.public_input().expect("owned public relation");
    assert_eq!(prover_secret_copy_owner_drops(), 19);
    reset_prover_secret_copy_owner_drops();
    let repeated = input
        .public_input()
        .expect("repeated owned public relation");
    assert_eq!(first, repeated);
    assert_eq!(prover_secret_copy_owner_drops(), 19);

    input.spend_x = Scalar::ZERO;
    reset_prover_secret_copy_owner_drops();
    assert_eq!(
        input.public_input(),
        Err(FcmpNativeErrorV1::SalWitnessMismatch)
    );
    assert_eq!(prover_secret_copy_owner_drops(), 9);

    let output = output_from_multiples(13, 17, 19);
    let output_bytes = output.component_refs_v1().0;
    reset_prover_secret_copy_owner_drops();
    let decoded = prover_secret_decode_edwards_point_v1(output_bytes)
        .expect("move-only decoded output owner");
    assert_point_owner(&decoded);
    assert_eq!(prover_secret_copy_owner_drops(), 2);
    drop(decoded);
    assert_eq!(prover_secret_copy_owner_drops(), 3);

    let mut identity = [0_u8; 32];
    identity[0] = 1;
    reset_prover_secret_copy_owner_drops();
    assert!(matches!(
        prover_secret_decode_edwards_point_v1(&identity),
        Err(FcmpNativeErrorV1::EdwardsPointIdentity)
    ));
    assert_eq!(prover_secret_copy_owner_drops(), 3);

    reset_prover_secret_copy_owner_drops();
    let unwind = std::panic::catch_unwind(|| {
        let point = prover_secret_decode_edwards_point_v1(output_bytes)
            .expect("owned decoded output before unwind");
        assert_eq!(prover_secret_copy_owner_drops(), 2);
        let encoded = prover_secret_edwards_encoding_v1(point.expose_ref());
        assert_eq!(encoded.expose_ref(), output_bytes);
        assert_eq!(prover_secret_copy_owner_drops(), 3);
        let _ = core::hint::black_box((&point, &encoded));
        panic!("exercise private public-input point unwind");
    });
    assert!(unwind.is_err());
    assert_eq!(prover_secret_copy_owner_drops(), 5);

    let (mut final_error_input, _output, _root) = one_layer_fixture();
    final_error_input.rerandomization.linking = Scalar::ZERO;
    final_error_input.rerandomization.rerandomization_blind = Scalar::ZERO;
    reset_prover_secret_copy_owner_drops();
    assert_eq!(
        final_error_input.public_input(),
        Err(FcmpNativeErrorV1::EdwardsPointIdentity)
    );
    assert_eq!(prover_secret_copy_owner_drops(), 19);
}
#[test]
fn public_input_keeps_private_products_in_borrowed_erasing_owners() {
    let source = include_str!("../../prover.rs");
    let decoder = source_part!(
        source;
        "fn prover_secret_decode_edwards_point_v1(" =>
        "fn prover_secret_edwards_encoding_v1("
    );
    source_has!(decoder; "bytes: &[u8; 32]", ") -> Result<ProverSecretCopyValueV1<EdwardsPoint>, FcmpNativeErrorV1>", "ProverSecretCopyValueV1::new(CompressedEdwardsY(*bytes))", ".decompress()", "ProverSecretCopyValueV1::new(point.expose_ref().compress())", "recompressed.expose_ref().as_bytes() != bytes", "!point.expose_ref().is_torsion_free()", "point.expose_ref() == &EdwardsPoint::identity()", "Ok(point)");
    source_counts!(decoder; "ProverSecretCopyValueV1::new(" => 3);
    source_lacks!(decoder; "decode_edwards_point(", "bytes: [u8; 32]", "Ok(point.expose_copy())");
    let encoder = source_part!(
        source;
        "fn prover_secret_edwards_encoding_v1(" =>
        "fn secret_edwards_product_v1("
    );
    source_has!(encoder; "point: &EdwardsPoint", ") -> ProverSecretCopyValueV1<[u8; 32]>", "ProverSecretCopyValueV1::new(point.compress())", "ProverSecretCopyValueV1::new(*compressed.expose_ref().as_bytes())");
    source_lacks!(encoder; "point.compress().to_bytes()", ") -> [u8; 32]");
    let product = source
        .split_once("fn secret_edwards_product_v1(")
        .expect("borrowed Edwards product")
        .1
        .split_once("fn secret_edwards_scalar_product_v1")
        .expect("Edwards product boundary")
        .0;
    assert!(product.contains("generator: &EdwardsPoint"));
    assert!(product.contains("scalar: &Scalar"));
    assert!(product.contains("Zeroizing::new(generator * scalar)"));
    let scalar_product = source
        .split_once("fn secret_edwards_scalar_product_v1(")
        .expect("borrowed scalar product")
        .1
        .split_once("fn ct_slice_contains_by")
        .expect("scalar product boundary")
        .0;
    assert!(scalar_product.contains("left: &Scalar, right: &Scalar"));
    assert!(scalar_product.contains("Zeroizing::new(left * right)"));
    let public_input = source
        .split_once("    pub fn public_input(&self)")
        .expect("public-input method")
        .1
        .split_once("    /// Borrow the complete canonical origin set")
        .expect("public-input boundary")
        .0;
    assert_eq!(
        public_input.matches("secret_edwards_product_v1(").count(),
        9
    );
    assert_eq!(
        public_input
            .matches("secret_edwards_scalar_product_v1(")
            .count(),
        1
    );
    source_counts!(public_input; "prover_secret_decode_edwards_point_v1(" => 3, "prover_secret_edwards_encoding_v1(" => 5, ".expose_copy()" => 5, "Zeroizing::new(" => 6);
    source_has!(public_input; "self.output.component_refs_v1()", "if &*expected_output != output.expose_ref()", "Zeroizing::new(output.expose_ref() + &*output_blind)", "Zeroizing::new(linking.expose_ref() + &*linking_blind)", "Zeroizing::new(&*rerandomization_v + &*rerandomization_t)", "Zeroizing::new(amount_commitment.expose_ref() + &*commitment_blind)", "Zeroizing::new(&*key_image_left - &*key_image_right)");
    source_order!(public_input; "self.output.component_refs_v1()", "prover_secret_decode_edwards_point_v1(output_bytes)?", "prover_secret_decode_edwards_point_v1(linking_bytes)?", "prover_secret_decode_edwards_point_v1(commitment_bytes)?", "let output_key_tilde = prover_secret_edwards_encoding_v1", "let key_image = prover_secret_edwards_encoding_v1", "FcmpProofInputPublicV1::new(", "output_key_tilde.expose_copy()", "key_image.expose_copy()");
    assert!(public_input.contains("Zeroizing::new(&*spend_component + &*output_component)"));
    source_lacks!(public_input; "self.output.components()", "decode_edwards_point(", ".compress().to_bytes()", "let output_key_tilde = &", "let linking_tilde = &", "let rerandomization = &", "let pseudo_out = &", "let key_image = &", "ED25519_BASEPOINT_POINT * self.spend_x", "generator_t() * self.output_y", "self.rerandomization.linking * self.spend_x");
    let production = source
        .split_once("#[cfg(test)]\n#[path = \"prover/tests.rs\"]\nmod tests")
        .expect("production prover boundary")
        .0;
    source_counts!(production; "input.public_input()?" => 2);
}
#[test]
fn commitment_mask_openings_remain_borrowed_until_the_membership_boundary() {
    fn between<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
        let start = source.find(start).expect("source start");
        let tail = &source[start..];
        let end = tail.find(end).expect("source end");
        &tail[..end]
    }
    let prover = include_str!("../../prover.rs");
    let field = include_str!("../../field.rs");
    let scalar_owner = between(
        prover,
        "impl<F: ProofScalar> ProverSecretScalarV1<F>",
        "impl ProverSecretScalarV1<Field25519>",
    );
    let negated_owner = between(
        scalar_owner,
        "fn negated_owner_v1(&self) -> Self",
        "fn take(value: &mut F) -> Self",
    );
    assert!(negated_owner.contains("Self(-self.0)"));
    assert_eq!(negated_owner.matches("self.0").count(), 1);
    assert_eq!(negated_owner.matches("Self(").count(), 1);
    for forbidden in [
        "-> F",
        "neg_ref",
        "expose_ref",
        "expose_copy",
        "copy_from_borrowed",
        "let negated",
        "callback",
        "getter",
        "FnOnce",
        "FnMut",
        "Deref",
        "Clone",
        ".clone(",
        ".copied(",
    ] {
        assert!(!negated_owner.contains(forbidden), "retained {forbidden}");
    }
    assert!(!prover.contains("c1_masks.iter().copied()"));
    assert!(!prover.contains("c2_masks.iter().copied()"));
    assert!(prover.contains(".zip(c1_masks.iter())"));
    assert!(prover.contains(".zip(c2_masks.iter())"));
    assert!(!prover.contains("then(|| c1_masks[root_commitment_index])"));
    assert!(!prover.contains("then(|| c2_masks[root_commitment_index])"));
    assert!(prover.contains("then(|| &c1_masks[root_commitment_index])"));
    assert!(prover.contains("then(|| &c2_masks[root_commitment_index])"));
    let raw_secret_push = between(
        prover,
        "fn push_secret_scalar_v1<F: ProofScalar + Zeroize>",
        "fn push_owned_secret_scalar_v1<F: ProofScalar + Zeroize>",
    );
    let take = raw_secret_push
        .find("ProverSecretScalarV1::take(&mut value)")
        .expect("incoming scalar take");
    let owner_handoff = raw_secret_push
        .find("push_owned_secret_scalar_v1(values, value)")
        .expect("owner handoff");
    assert!(take < owner_handoff);
    let owned_secret_push = between(
        prover,
        "fn push_owned_secret_scalar_v1<F: ProofScalar + Zeroize>",
        "fn ct_slice_contains_by",
    );
    let capacity_snapshot = owned_secret_push
        .find("let allocation_capacity = values.capacity()")
        .expect("allocation-capacity snapshot");
    let pointer_snapshot = owned_secret_push
        .find("let allocation_ptr = values.as_ptr()")
        .expect("allocation-pointer snapshot");
    let capacity = owned_secret_push
        .find("let preflight = require_preallocated_push(values.len(), allocation_capacity)")
        .expect("capacity preflight");
    let error_drop = owned_secret_push
        .find("if let Err(error) = preflight {\n        drop(value);")
        .expect("preflight-error owner drop");
    let zero_destination = owned_secret_push
        .find("values.push(F::ZERO)")
        .expect("public zero destination slot");
    let destination = owned_secret_push
        .find("let destination = values.len() - 1")
        .expect("final destination index");
    let transfer = owned_secret_push
        .find("core::mem::swap(&mut values[destination], &mut value.0)")
        .expect("direct owner-slot transfer");
    let success_drop = owned_secret_push[transfer..]
        .find("drop(value)")
        .map(|position| transfer + position)
        .expect("success owner drop");
    let post_capacity = owned_secret_push[success_drop..]
        .find("debug_assert_eq!(values.capacity(), allocation_capacity)")
        .map(|position| success_drop + position)
        .expect("post-push capacity check");
    let post_pointer = owned_secret_push[post_capacity..]
        .find("debug_assert_eq!(values.as_ptr(), allocation_ptr)")
        .map(|position| post_capacity + position)
        .expect("post-push pointer check");
    assert!(
        capacity_snapshot < pointer_snapshot
            && pointer_snapshot < capacity
            && capacity < error_drop
            && error_drop < zero_destination
            && zero_destination < destination
            && destination < transfer
            && transfer < success_drop
            && success_drop < post_capacity
            && post_capacity < post_pointer
    );
    assert!(owned_secret_push.contains("mut value: ProverSecretScalarV1<F>"));
    assert_eq!(owned_secret_push.matches("drop(value)").count(), 2);
    assert!(!owned_secret_push.contains("value.expose_copy()"));
    assert!(!owned_secret_push.contains("value.expose_ref()"));
    assert!(!owned_secret_push.contains("values.push(value.0)"));
    assert!(!owned_secret_push.contains("value.0.clear_secret()"));
    assert!(!owned_secret_push.contains("callback"));
    assert!(!owned_secret_push.contains("FnOnce"));
    assert!(!prover.contains("c1_branch_masks.push("));
    assert!(!prover.contains("c2_branch_masks.push("));
    assert!(!prover.contains("c1_masks.push("));
    assert!(!prover.contains("c2_masks.push("));
    let prove_once = between(
        prover,
        "fn prove_fcmp_plus_plus_once_v1(",
        "fn retry_membership_prover<T>(",
    );
    let secret_push_call = ["push_secret_scalar_v1(", "&mut"].concat();
    let owned_secret_push_call = ["push_owned_secret_scalar_v1(", "&mut"].concat();
    assert_eq!(prove_once.matches(&secret_push_call).count(), 0);
    assert_eq!(prove_once.matches(&owned_secret_push_call).count(), 6);
    assert!(!prover.contains("-blind.scalar"));
    assert_eq!(prover.matches("blind.scalar.negated_owner_v1()").count(), 2);
    for forbidden in [
        "push_secret_scalar_v1(&mut c1_branch_masks",
        "push_secret_scalar_v1(&mut c2_branch_masks",
        "blind.scalar.expose_ref().neg_ref()",
        "blind.scalar.expose_ref()",
        "blind.scalar.expose_copy()",
        "blind.scalar.neg_ref()",
        "blind.scalar.0",
        "c1_branch_masks.push(",
        "c2_branch_masks.push(",
    ] {
        assert!(!prove_once.contains(forbidden), "retained {forbidden}");
    }
    let root_nonce = between(
        prover,
        "let (root_blind_commitment, mut root_nonce_c1, mut root_nonce_c2)",
        "let public_inputs =",
    );
    assert_eq!(
        root_nonce
            .matches("let nonce = random_proof_scalar")
            .count(),
        2
    );
    assert!(!root_nonce.contains("let mut nonce = random_proof_scalar"));
    assert!(!root_nonce.contains("ProverSecretScalarV1::take(&mut nonce)"));
    assert!(root_nonce.contains("root_nonce_commitment_v1::<SeleneSuite>(nonce.expose_ref())"));
    assert!(root_nonce.contains("root_nonce_commitment_v1::<HeliosSuite>(nonce.expose_ref())"));
    assert_eq!(root_nonce.matches("let mut commitment =").count(), 2);
    assert_eq!(
        root_nonce
            .matches("commitment.encode_public_and_clear_v1()")
            .count(),
        2
    );
    assert_eq!(root_nonce.matches("Some(nonce)").count(), 2);
    assert!(!root_nonce.contains(".h.scale(nonce)"));
    assert!(!root_nonce.contains("?.encode()"));
    assert!(!root_nonce.contains("commitment.expose_copy()"));
    let root_commitment = between(
        prover,
        "fn root_nonce_commitment_v1<S: ProofSuite>",
        "fn prepared_secret_point_v1<S: ProofSuite>",
    );
    assert!(
        root_commitment.contains(") -> Result<ProverSecretPointV1<S::Point>, FcmpNativeErrorV1>")
    );
    assert!(root_commitment.contains("SecretMultiexpBuilder::<S>::new(1)"));
    assert!(root_commitment.contains("terms.push(nonce, &S::generators().h)"));
    assert!(root_commitment.contains("let point = terms.evaluate()?"));
    assert!(root_commitment.contains("ProverSecretPointV1::from_secret(point)"));
    assert!(!root_commitment.contains("ProverSecretPointV1::take(&mut point)"));
    assert!(!root_commitment.contains("Result<S::Point, FcmpNativeErrorV1>"));
    assert!(!root_commitment.contains("terms.evaluate().map_err(Into::into)"));
    let circuit_source = include_str!("../../circuit.rs");
    let commitment_producer = between(
        circuit_source,
        "pub(super) fn commitments_and_openings<S: ProofSuite<Scalar = F>>",
        "pub(super) struct Circuit<S: ProofSuite>",
    );
    source_has!(commitment_producer; "Vec<SecretPoint<S::Point>>", "owned_masks.extend_borrowed_v1(masks)?");
    source_lacks!(commitment_producer; "Vec<S::Point>", "owned_masks.extend_from_slice(masks)", "masks.to_vec()", "copy_from_slice(masks)", "masks[index]", "owned_masks.as_slice()[index]", "VectorCommitmentOpening::new(");
    source_order!(commitment_producer; "let mut commitments = Vec::new()", "let mut openings = Vec::new()", "ZeroizingScalarVec::new(masks.len())?", "owned_masks.extend_borrowed_v1(masks)?", "for (values, mask) in self.values.iter().zip(owned_masks.as_slice())", "for (values, mask) in self.values.iter_mut().zip(owned_masks.values.iter_mut())", "VectorCommitmentOpening::take_mask_from_slot(", "values.take()", "\n                mask,\n            ));");
    source_counts!(commitment_producer; "owned_masks.extend_borrowed_v1(masks)?" => 1, "VectorCommitmentOpening::take_mask_from_slot(" => 1);
    source_order!(prove_once; "c1_tape.commitments_and_openings", "c2_tape.commitments_and_openings");
    source_counts!(prove_once; ".commitments_and_openings::<" => 2);
    let proof_math_source = include_str!("../../proof_math.rs");
    let publication = between(
        proof_math_source,
        "pub(super) fn write_secret_commitments<S: ProofSuite>",
        "pub(super) fn challenge_bytes",
    );
    let reserve = publication
        .find("try_reserve_exact(vector.len())")
        .expect("public commitment allocation reserve");
    let capacity_check = publication
        .find("if allocation_capacity < vector.len()")
        .expect("public commitment allocation validation");
    let digest_update = publication
        .find("self.digest.update(")
        .expect("commitment count publication");
    let borrow = publication
        .find("push_point(self, commitment.expose_ref())?")
        .expect("borrowed commitment publication");
    let public_copy = publication
        .find("published.push(*commitment.expose_ref())")
        .expect("post-publication public commitment copy");
    assert!(reserve < capacity_check && capacity_check < digest_update);
    assert!(digest_update < borrow && borrow < public_copy);
    assert!(publication.contains("vector: Vec<SecretPoint<S::Point>>"));
    assert!(!publication.contains("push_point(self, *commitment"));
    assert_eq!(prove_once.matches("write_secret_commitments::<").count(), 2);
    assert!(prove_once.contains("write_secret_commitments::<SeleneSuite>(c1_secret_commitments)"));
    assert!(prove_once.contains("write_secret_commitments::<HeliosSuite>(c2_secret_commitments)"));
    let point_owner = between(
        prover,
        "impl<P: ProofPoint> ProverSecretPointV1<P>",
        "impl ProverSecretPointV1<SelenePoint>",
    );
    assert!(point_owner.contains("fn from_secret(point: SecretPoint<P>) -> Self"));
    assert!(point_owner.contains("point.move_into(&mut owned.0);"));
    assert!(!point_owner.contains("point.transfer"));
    assert!(!point_owner.contains("fn expose_copy(&self) -> P"));
    assert!(!point_owner.contains("encode_public_and_clear_v1"));
    assert!(!point_owner.contains("self.0.encode()"));
    for (owner_start, owner_end, identity) in [
        (
            "impl ProverSecretPointV1<SelenePoint>",
            "impl ProverSecretPointV1<HeliosPoint>",
            "SelenePoint::identity()",
        ),
        (
            "impl ProverSecretPointV1<HeliosPoint>",
            "impl<P: ProofPoint> Drop for ProverSecretPointV1<P>",
            "HeliosPoint::identity()",
        ),
    ] {
        let concrete_owner = between(prover, owner_start, owner_end);
        let public_encoding = concrete_owner
            .find("fn encode_public_and_clear_v1(&mut self) -> Result<[u8; 32], FcmpNativeErrorV1>")
            .expect("concrete owner-confined public point encoding");
        let transfer = concrete_owner[public_encoding..]
            .find(&format!("core::mem::replace(&mut self.0, {identity})"))
            .expect("original point transfer into erasing encoder");
        let encode = concrete_owner[public_encoding..]
            .find(".secret_encode_v1()")
            .expect("audited concrete point encoder");
        let failure = concrete_owner[public_encoding..]
            .find(".ok_or(FcmpNativeErrorV1::CyclePointIdentity)?")
            .expect("fail-closed identity encoding");
        let expose = concrete_owner[public_encoding..]
            .find("let public = *encoded.as_ref();")
            .expect("intentional public byte copy");
        let drop = concrete_owner[public_encoding..]
            .find("drop(encoded);")
            .expect("encoded owner clearing");
        assert!(transfer < encode && encode < failure && failure < expose && expose < drop);
        assert!(!concrete_owner.contains("self.0.encode()"));
        assert!(!concrete_owner.contains("ProofPoint::encode"));
    }
    let prepared_point = between(
        prover,
        "fn prepared_secret_point_v1<S: ProofSuite>",
        "struct PreparedEdBlind",
    );
    assert!(prepared_point.contains("SecretMultiexpBuilder::<S>::new(1)"));
    assert!(prepared_point.contains("terms.push(scalar, &S::generators().h)"));
    assert!(prepared_point.contains("let point = terms.evaluate()?"));
    assert!(prepared_point.contains("ProverSecretPointV1::from_secret(point)"));
    assert!(!prepared_point.contains("ProverSecretPointV1::take(&mut point)"));
    for (start, end, scalar_owner, point_owner) in [
        (
            "struct PreparedSeleneBlind {",
            "fn prepare_selene_blind(",
            "scalar: ProverSecretScalarV1<Field25519>",
            "point: ProverSecretPointV1<SelenePoint>",
        ),
        (
            "struct PreparedHeliosBlind {",
            "fn prepare_helios_blind(",
            "scalar: ProverSecretScalarV1<HelioseleneField>",
            "point: ProverSecretPointV1<HeliosPoint>",
        ),
    ] {
        let prepared_owner = between(prover, start, end);
        assert!(prepared_owner.contains(scalar_owner));
        assert!(prepared_owner.contains(point_owner));
        assert!(prepared_owner.contains("self.decomposition.zeroize();"));
        assert!(!prepared_owner.contains("self.scalar.zeroize();"));
        assert!(!prepared_owner.contains("self.point.zeroize();"));
    }
    for (start, end) in [
        ("fn prepare_selene_blind(", "struct PreparedHeliosBlind"),
        ("fn prepare_helios_blind(", "fn commitment_index"),
    ] {
        let blind = between(prover, start, end);
        let owner_input = blind
            .find("scalar: ProverSecretScalarV1<")
            .expect("scalar owner input");
        let decomposition = blind
            .find("scalar_decomposition(scalar.expose_ref()")
            .expect("borrowed decomposition");
        let point = blind
            .find("prepared_secret_point_v1::<")
            .expect("owned point");
        let divisor = blind
            .find("point.expose_ref()")
            .expect("borrowed divisor point");
        assert!(owner_input < decomposition && decomposition < point && point < divisor);
        assert!(!blind.contains("ProverSecretScalarV1::take(&mut scalar)"));
        assert!(!blind.contains("mut scalar:"));
        assert!(!blind.contains(".scale(scalar)"));
        assert!(!blind.contains("let point = generator.scale"));
        assert!(blind.contains("Ok(Prepared"));
        let handoff = blind
            .split_once("Ok(Prepared")
            .expect("prepared owner handoff")
            .1;
        assert_source_contract_group(
            "commitment_mask_openings_remain_borrowed_until_the_membership_boundary/00",
            handoff,
        );
        assert!(!blind.contains("scalar.expose_copy()"));
        assert!(!blind.contains("point.expose_copy()"));
    }
    let blind_consumers = between(
        prover,
        "let mut selene_blinds = Vec::with_capacity(c1_non_root_count)",
        "if c1_tape.commitment_count() > c1_rows",
    );
    assert_eq!(
        blind_consumers
            .matches("blind.scalar.negated_owner_v1()")
            .count(),
        2
    );
    assert_eq!(
        blind_consumers
            .matches(".point\n            .expose_ref()\n            .secret_coordinates_ref_v1()")
            .count(),
        2
    );
    assert_eq!(
        blind_consumers
            .matches("coordinates.component_pair_ref()")
            .count(),
        2
    );
    assert!(!blind_consumers.contains("blind.scalar.neg_ref()"));
    assert!(!blind_consumers.contains("blind.scalar.expose_ref().neg_ref()"));
    assert!(!blind_consumers.contains("blind.scalar.expose_ref()"));
    assert!(!blind_consumers.contains("blind.scalar.expose_copy()"));
    assert!(!blind_consumers.contains("blind.scalar.0"));
    assert!(!blind_consumers.contains("push_secret_scalar_v1(&mut c1_branch_masks"));
    assert!(!blind_consumers.contains("push_secret_scalar_v1(&mut c2_branch_masks"));
    assert!(!blind_consumers.contains("-blind.scalar"));
    let c1_branch_masks = between(
        prover,
        "for branch in &path.c1_non_root {",
        "let mut c2_non_root = Vec::with_capacity(path.c2_non_root.len())",
    );
    assert_source_contract_group(
        "commitment_mask_openings_remain_borrowed_until_the_membership_boundary/01",
        c1_branch_masks,
    );
    let c2_branch_masks = between(
        prover,
        "for branch in &path.c2_non_root {",
        "transcripted_paths.push(TranscriptedPath {",
    );
    assert_source_contract_group(
        "commitment_mask_openings_remain_borrowed_until_the_membership_boundary/02",
        c2_branch_masks,
    );
    for branch_masks in [c1_branch_masks, c2_branch_masks] {
        assert_eq!(branch_masks.matches("blind.scalar").count(), 1);
        for forbidden in [
            "push_secret_scalar_v1(",
            ".neg_ref()",
            "blind.scalar.expose_ref()",
            "blind.scalar.expose_copy()",
            "blind.scalar.0",
            ".copied()",
            ".cloned()",
            ".clone()",
            "callback",
            "getter",
            "FnOnce",
            "FnMut",
            "Deref",
            "Clone",
        ] {
            assert!(!branch_masks.contains(forbidden), "retained {forbidden}");
        }
    }
    assert_source_contract_group(
        "commitment_mask_openings_remain_borrowed_until_the_membership_boundary/03",
        blind_consumers,
    );
    assert!(
        !blind_consumers.contains(".point\n            .expose_ref()\n            .coordinates()")
    );
    assert!(
        !blind_consumers.contains("Zeroizing::new(\n            blind\n                .point")
    );
    for claim in [
        between(
            prover,
            "let mut c1_blind_claims = Vec::with_capacity(helios_blinds.len())",
            "let mut c2_blind_claims = Vec::with_capacity(selene_blinds.len())",
        ),
        between(
            prover,
            "let mut c2_blind_claims = Vec::with_capacity(selene_blinds.len())",
            "if c1_tape.commitment_count() > c1_rows",
        ),
    ] {
        assert_source_contract_group(
            "commitment_mask_openings_remain_borrowed_until_the_membership_boundary/04",
            claim,
        );
        assert!(!claim.contains(".coordinates()"));
        assert!(!claim.contains("Zeroizing::new("));
        assert!(!claim.contains("*coordinates.component_pair_ref()"));
    }
    let dlog_coefficient_declaration = between(
        circuit_source,
        "/// Owns one secret-derived discrete-log coefficient",
        "impl SecretDlogCoefficientV1",
    );
    assert!(!dlog_coefficient_declaration.contains("#[derive"));
    let dlog_coefficient_owner = between(
        circuit_source,
        "struct SecretDlogCoefficientV1",
        "/// Erases one callee-owned `Copy` scalar parameter",
    );
    assert_source_contract_group(
        "commitment_mask_openings_remain_borrowed_until_the_membership_boundary/05",
        dlog_coefficient_owner,
    );
    for forbidden in [
        "derive(",
        "-> u64",
        "fn expose",
        "fn get",
        "callback",
        "FnOnce",
        "FnMut",
        "Deref",
        "Clone",
        ".clone(",
        ".copied(",
    ] {
        assert!(
            !dlog_coefficient_owner.contains(forbidden),
            "retained dlog coefficient API {forbidden}"
        );
    }
    let scalar_vector = between(
        circuit_source,
        "impl<F: ProofScalar> ZeroizingScalarVec<F>",
        "impl<F: ProofScalar> Drop for ZeroizingScalarVec<F>",
    );
    let owned_insertion = between(scalar_vector, "fn push_owned(", "fn push_borrowed(");
    assert_source_contract_group(
        "commitment_mask_openings_remain_borrowed_until_the_membership_boundary/06",
        owned_insertion,
    );
    assert!(!owned_insertion.contains("expose_copy"));
    let borrowed_insertion = between(scalar_vector, "fn push_borrowed(", "fn extend_borrowed_v1(");
    assert_source_contract_group(
        "commitment_mask_openings_remain_borrowed_until_the_membership_boundary/07",
        borrowed_insertion,
    );
    assert!(!borrowed_insertion.contains("expose_copy"));
    assert!(!borrowed_insertion.contains("self.values.push("));
    source_lacks!(scalar_vector; "fn extend_from_slice(");
    let borrowed_extension = between(scalar_vector, "fn extend_borrowed_v1(", "fn take(");
    assert_source_contract_group(
        "commitment_mask_openings_remain_borrowed_until_the_membership_boundary/08",
        borrowed_extension,
    );
    assert_eq!(
        borrowed_extension
            .matches("self.push_borrowed(value)?")
            .count(),
        1
    );
    for forbidden in [
        "self.values.extend_from_slice(",
        "self.values.push(",
        "SecretScalarGuard",
        "*value",
        ".copied(",
        ".cloned(",
        ".clone(",
        "expose",
        "fn get",
        ".get(",
        "callback",
        "FnOnce",
        "FnMut",
        "Deref",
        "Clone",
    ] {
        assert!(
            !borrowed_extension.contains(forbidden),
            "retained borrowed-extension path {forbidden}"
        );
    }
    let append_word = between(
        circuit_source,
        "fn append_word(&mut self, values: &[F])",
        "pub(super) fn append_branch(",
    );
    assert_source_contract_group(
        "commitment_mask_openings_remain_borrowed_until_the_membership_boundary/09",
        append_word,
    );
    assert_eq!(
        append_word.matches("extend_borrowed_v1(values)?").count(),
        2
    );
    for forbidden in [
        "extend_from_slice(values)",
        "values.iter().copied",
        "values.iter().cloned",
        ".copied(",
        ".cloned(",
        ".clone(",
        "self.values.push(",
        "SecretScalarGuard",
        "expose",
        "fn get",
        ".get(",
        "callback",
        "FnOnce",
        "FnMut",
        "Deref",
        "Clone",
    ] {
        assert!(
            !append_word.contains(forbidden),
            "retained append-word path {forbidden}"
        );
    }
    let prover_tape = between(
        circuit_source,
        "impl<F: ProofScalar> ProverVectorCommitmentTape<F>",
        "/// Exact verifier-side arithmetic circuit",
    );
    let append_branch = between(
        prover_tape,
        "pub(super) fn append_branch(\n        &mut self,\n        branch: &[F],",
        "pub(super) fn append_dlog(",
    );
    assert_source_contract_group(
        "commitment_mask_openings_remain_borrowed_until_the_membership_boundary/10",
        append_branch,
    );
    assert_eq!(
        append_branch
            .matches("destination.extend_borrowed_v1(branch)?")
            .count(),
        1
    );
    for forbidden in [
        "extend_from_slice(branch)",
        ".copied(",
        ".cloned(",
        ".clone(",
        "expose",
        "callback",
        "FnOnce",
        "Deref",
        "Clone",
    ] {
        assert!(
            !append_branch.contains(forbidden),
            "retained append-branch path {forbidden}"
        );
    }
    let dlog_ingestion = between(
        circuit_source,
        "dlog: &[u64],",
        "pub(super) fn append_divisor(",
    );
    assert_source_contract_group(
        "commitment_mask_openings_remain_borrowed_until_the_membership_boundary/11",
        dlog_ingestion,
    );
    assert_eq!(
        dlog_ingestion
            .matches("witness.extend_borrowed_v1(padding)?")
            .count(),
        1
    );
    for forbidden in [
        "dlog.iter().copied()",
        "F::from_u64(value)",
        "witness.push_borrowed(&F::from_u64",
        "witness.extend_from_slice(padding)",
        "coefficient.expose",
        "coefficient.clone",
        "callback",
        "FnOnce",
        "FnMut",
        "Deref",
        "Clone",
    ] {
        assert!(
            !dlog_ingestion.contains(forbidden),
            "retained dlog path {forbidden}"
        );
    }
    let divisor_source = include_str!("../../divisor.rs");
    let cycle_decomposition = between(
        divisor_source,
        "pub(super) fn scalar_decomposition<F: ProofScalar>",
        "pub(super) fn ed25519_scalar_decomposition",
    );
    assert!(cycle_decomposition.contains("Result<Zeroizing<Vec<u64>>"));
    assert!(cycle_decomposition.contains("let scalar_bytes = Zeroizing::new"));
    assert!(cycle_decomposition.contains("scalar_decomposition_encoded(&scalar_bytes"));
    assert!(cycle_decomposition.contains("SecretDecompositionScalarV1(F::ZERO)"));
    assert!(cycle_decomposition.contains("for coefficient in decomposition.iter()"));
    let ed_decomposition = between(
        divisor_source,
        "pub(super) fn ed25519_scalar_decomposition",
        "fn scalar_decomposition_encoded(",
    );
    assert!(ed_decomposition.contains("for coefficient in decomposition.iter()"));
    let encoded_decomposition = between(
        divisor_source,
        "fn scalar_decomposition_encoded(",
        "pub(super) trait DivisorPoint",
    );
    assert!(encoded_decomposition.contains("scalar: &[u8; 32]"));
    assert!(encoded_decomposition.contains("let mut decomposition = Zeroizing::new("));
    assert!(encoded_decomposition.contains("let mut low_bytes = Zeroizing::new([0_u8; 8])"));
    assert!(encoded_decomposition.contains("let mut sum = Zeroizing::new("));
    let ed_blind = between(prover, "fn prepare_ed_blind(", "struct PreparedSeleneBlind");
    let scalar_owner = ed_blind
        .find("let scalar = Zeroizing::new(if negate")
        .expect("signed scalar owner");
    let decomposition = ed_blind
        .find("ed25519_scalar_decomposition(&scalar)")
        .expect("borrowed Ed decomposition");
    let point_owner = ed_blind
        .find("let point = Zeroizing::new(&generator * &*scalar)")
        .expect("borrowed Ed multiplication");
    let encoded_owner = ed_blind
        .find("let encoded_point = Zeroizing::new")
        .expect("encoded point owner");
    let coordinate_owner = ed_blind
        .find("let coordinates = secret_edwards_to_wei25519_v1(&encoded_point)?")
        .expect("coordinate owner");
    let divisor = ed_blind
        .find("scalar_mul_divisor")
        .expect("borrowed divisor");
    assert!(
        scalar_owner < decomposition
            && decomposition < point_owner
            && point_owner < encoded_owner
            && encoded_owner < coordinate_owner
            && coordinate_owner < divisor
    );
    let prepared_ed_owner = between(prover, "struct PreparedEdBlind", "fn prepare_ed_blind(");
    assert!(prepared_ed_owner.contains("coordinates: SecretCycleCoordinatesV1<Field25519>"));
    assert!(prepared_ed_owner.contains("self.decomposition.zeroize()"));
    assert!(!prepared_ed_owner.contains("self.coordinates.0.zeroize()"));
    assert!(!prepared_ed_owner.contains("self.coordinates.1.zeroize()"));
    assert!(ed_blind.contains("scalar: &Scalar"));
    assert!(ed_blind.contains("decomposition: core::mem::take(&mut *decomposition)"));
    assert!(ed_blind.contains("coordinates,"));
    assert!(!ed_blind.contains("generator * scalar"));
    assert!(ed_blind.contains("secret_edwards_to_wei25519_v1(&encoded_point)"));
    assert!(!ed_blind.contains("edwards_to_wei25519(*encoded_point)"));
    for forbidden in [
        "Zeroizing::new(secret_edwards_to_wei25519_v1",
        "coordinates: *coordinates",
        "coordinates.0",
        "coordinates.1",
        "coordinates.expose_copy()",
        "coordinates.clone()",
    ] {
        assert!(!ed_blind.contains(forbidden), "retained {forbidden}");
    }
    let secret_coordinates = between(
        field,
        "pub(super) fn secret_edwards_to_wei25519_v1",
        "pub(super) fn monero_varint",
    );
    assert!(secret_coordinates.contains("bytes: &[u8; 32]"));
    assert!(secret_coordinates.contains("SecretCopyValueV1::new(CompressedEdwardsY(*bytes))"));
    assert!(secret_coordinates.contains("let point = SecretCopyValueV1::new("));
    assert!(secret_coordinates.contains("let mut y_bytes = SecretCopyValueV1::new(*bytes)"));
    assert!(secret_coordinates.contains("secret_decode_field25519_v1(y_bytes.as_ref())"));
    assert!(secret_coordinates.contains("secret_invert_field25519_v1"));
    assert!(secret_coordinates.contains("secret_sqrt_field25519_v1"));
    assert!(
        secret_coordinates
            .contains("Ok(SecretCycleCoordinatesV1::from_secret_coordinate_owners_v1(")
    );
    assert!(secret_coordinates.contains("wei_x, wei_y,"));
    assert_eq!(secret_coordinates.matches("expose_copy()").count(), 0);
    assert!(!secret_coordinates.contains("Result<(Field25519, Field25519)"));
    assert!(!secret_coordinates.contains("Ok(("));
    assert!(!secret_coordinates.contains("field25519_is_odd(x.expose_copy())"));
    assert!(!secret_coordinates.contains("y_squared.expose_copy()"));
    assert!(!secret_coordinates.contains("y_plus_one.expose_copy()"));
    assert!(!secret_coordinates.contains("one_minus_y.expose_copy()"));
    let coordinate_constructor = between(
        field,
        "impl SecretCycleCoordinatesV1<Field25519>",
        "struct SecretU256V1",
    );
    assert_source_contract_group(
        "commitment_mask_openings_remain_borrowed_until_the_membership_boundary/12",
        coordinate_constructor,
    );
    for forbidden in [
        "expose_copy",
        "Result<(Field25519, Field25519)",
        "Ok((",
        "callback",
        "getter",
        "FnOnce",
        "FnMut",
        "Deref",
        "Clone",
        ".clone(",
        ".copied(",
    ] {
        assert!(
            !coordinate_constructor.contains(forbidden),
            "retained {forbidden}"
        );
    }
    let secret_sqrt = between(
        field,
        "fn secret_sqrt_field25519_v1",
        "pub(super) fn secret_edwards_to_wei25519_v1",
    );
    assert!(!secret_sqrt.contains("expose_copy()"));
    assert!(secret_sqrt.contains("first.as_ref().square().eq_ref(value)"));
    assert!(secret_sqrt.contains("first.as_ref()"));
    assert!(secret_sqrt.contains(".mul_ref(&Field25519::new"));
    let secret_invert = between(
        field,
        "fn secret_invert_field25519_v1",
        "fn secret_sqrt_field25519_v1",
    );
    let invert = secret_invert
        .find("value.invert()")
        .expect("field inversion");
    let take = secret_invert
        .find("SecretCopyValueV1::take(&mut inverse)")
        .expect("inverse take");
    let branch = secret_invert
        .find("then_some(inverse)")
        .expect("option branch");
    assert!(invert < take && take < branch);
    let input_blinds = between(
        prover,
        "let mut prepared_inputs = Vec::with_capacity(inputs.len())",
        "let sal = prove_fcmp_sal_with_checked_rng_v1",
    );
    assert!(input_blinds.contains("let rerandomization = &input.rerandomization"));
    for raw in ["let r_o =", "let r_i =", "let r_r_i =", "let r_c ="] {
        assert!(!input_blinds.contains(raw));
    }
    assert_eq!(input_blinds.matches("prepare_ed_blind(").count(), 5);
    assert!(
        input_blinds.contains(
            "prover_secret_edwards_scalar_sum_v1(&input.output_y, &rerandomization.output)"
        )
    );
    assert!(!input_blinds.contains("let sal_y = Zeroizing::new("));
    assert!(!input_blinds.contains("sal_y.to_bytes()"));
    let sal_handoff = between(
        prover,
        "let sal_y = prover_secret_edwards_scalar_sum_v1(&input.output_y, &rerandomization.output)",
        "prepared_inputs.push(PreparedInput {",
    );
    assert_source_contract_group(
        "commitment_mask_openings_remain_borrowed_until_the_membership_boundary/13",
        sal_handoff,
    );
    assert_eq!(
        sal_handoff
            .matches("FcmpSalSecretScalarEncodingV1::from_scalar_ref_v1(")
            .count(),
        4
    );
    for (needle, expected) in [
        (
            "FcmpSalWitnessV1::from_secret_scalar_encoding_owners_v1(",
            1,
        ),
        ("sal_spend_x_bytes,", 1),
        ("sal_y_bytes,", 1),
        ("sal_linking_bytes,", 1),
        ("sal_rerandomization_blind_bytes,", 1),
        ("prove_fcmp_sal_with_checked_rng_v1(", 1),
        ("drop(sal_witness)", 1),
    ] {
        assert_eq!(sal_handoff.matches(needle).count(), expected, "{needle}");
    }
    for forbidden in [
        "ProverSecretCopyValueV1::new(sal_y",
        "ProverSecretCopyValueV1::new(rerandomization.linking",
        "ProverSecretCopyValueV1::new(input.spend_x",
        "ProverSecretCopyValueV1::new(rerandomization.rerandomization_blind",
        ".expose_copy()",
        "FcmpSalWitnessV1::new(",
        "sal_y.expose_ref().to_bytes()",
        "rerandomization.linking.to_bytes()",
        "input.spend_x.to_bytes()",
        "rerandomization.rerandomization_blind.to_bytes()",
    ] {
        assert!(!sal_handoff.contains(forbidden), "retained {forbidden}");
    }
    let owner = between(
        prover,
        "impl<F: ProofScalar> ProverSecretScalarV1<F>",
        "impl ProverSecretScalarV1<Field25519>",
    );
    assert!(owner.contains("fn add_product_assign(&mut self, left: &F, right: &F)"));
    assert!(owner.contains("self.0 += *left * *right;"));
    assert!(!owner.contains("encode_public_and_clear_v1"));
    assert!(!owner.contains("self.0.encode()"));
    for (owner_start, owner_end, zero, encoder) in [
        (
            "impl ProverSecretScalarV1<Field25519>",
            "impl ProverSecretScalarV1<HelioseleneField>",
            "Field25519::ZERO",
            "encode_secret_field25519_scalar_v1",
        ),
        (
            "impl ProverSecretScalarV1<HelioseleneField>",
            "impl<F: ProofScalar> Drop for ProverSecretScalarV1<F>",
            "HelioseleneField::ZERO",
            "encode_secret_helioselene_scalar_v1",
        ),
    ] {
        let concrete_owner = between(prover, owner_start, owner_end);
        let public_encoding = concrete_owner
            .find("fn encode_public_and_clear_v1(&mut self) -> [u8; 32]")
            .expect("concrete owner-confined public response encoding");
        let transfer = concrete_owner[public_encoding..]
            .find(&format!(
                "let original = Self(core::mem::replace(&mut self.0, {zero}))"
            ))
            .expect("original scalar transfer into erasing owner");
        let encode = concrete_owner[public_encoding..]
            .find(&format!("let encoded = {encoder}(original.expose_ref())"))
            .expect("audited private scalar encoder");
        let expose = concrete_owner[public_encoding..]
            .find("let public = *encoded.as_ref();")
            .expect("intentional public byte copy");
        let drop_encoded = concrete_owner[public_encoding..]
            .find("drop(encoded);")
            .expect("encoded owner clearing");
        let drop_original = concrete_owner[public_encoding..]
            .find("drop(original);")
            .expect("original scalar owner clearing");
        assert!(
            transfer < encode
                && encode < expose
                && expose < drop_encoded
                && drop_encoded < drop_original
        );
        assert!(!concrete_owner.contains("self.0.encode()"));
        assert!(!concrete_owner.contains("ProofScalar::encode"));
        assert!(!concrete_owner.contains("original.expose_copy()"));
    }
    let response = between(
        prover,
        "let root_blind_response = match root.curve()",
        "let mut c1_circuit =",
    );
    assert_eq!(response.matches(".as_mut()").count(), 2);
    assert_eq!(
        response
            .matches("nonce.add_product_assign(&challenge, mask)")
            .count(),
        2
    );
    assert_eq!(
        response
            .matches("nonce.encode_public_and_clear_v1()")
            .count(),
        2
    );
    assert!(!response.contains("nonce.expose_copy().encode()"));
    assert!(!response.contains(".as_ref()"));
    assert!(!response.contains("challenge * *root_mask"));
    let mul_ref = between(
        field,
        "pub(super) fn mul_ref(&self, rhs: &Self)",
        "pub(super) const fn pow",
    );
    assert!(mul_ref.contains("Self(self.0 * rhs.0)"));
    assert!(field.contains("pub(super) fn add_ref(&self, rhs: &Self)"));
    assert!(field.contains("pub(super) fn sub_ref(&self, rhs: &Self)"));
    assert!(field.contains("pub(super) fn neg_ref(&self)"));
    assert!(field.contains("pub(super) fn is_odd_ref(&self)"));
    assert!(field.contains("pub(super) fn eq_ref(&self, rhs: &Self)"));
    let coordinates = between(
        field,
        "pub(super) fn secret_coordinates_v1(",
        "pub(super) fn secret_coordinates_ref_v1(",
    );
    let point_guard = coordinates
        .find("BorrowedZeroizingCopySlot(&mut self)")
        .unwrap();
    let invert = coordinates.find("point.as_ref().z.invert()").unwrap();
    let inverse_guard = coordinates
        .find("BorrowedZeroizingCopySlot(&mut inverse)")
        .unwrap();
    let branch = coordinates.find("if !bool::from(is_some)").unwrap();
    assert!(point_guard < invert && invert < inverse_guard && inverse_guard < branch);
    assert!(coordinates.contains("point.as_ref().x.mul_ref(inverse.as_ref())"));
    assert!(coordinates.contains("point.as_ref().y.mul_ref(inverse.as_ref())"));
    assert!(coordinates.contains("Option<SecretCycleCoordinatesV1<$field>>"));
    assert!(
        coordinates.contains("let coordinates = SecretCycleCoordinatesV1(SecretCopyValueV1::new((")
    );
    assert!(!coordinates.contains("Option<($field, $field)>"));
    assert!(coordinates.contains("drop(inverse);\n                drop(point);"));
    let borrowed_coordinates = between(
        field,
        "pub(super) fn secret_coordinates_ref_v1(",
        "pub(super) fn secret_x_ref_v1(&self)",
    );
    assert!(borrowed_coordinates.contains("&self"));
    assert!(borrowed_coordinates.contains("self.z.invert()"));
    assert!(borrowed_coordinates.contains("BorrowedZeroizingCopySlot(&mut inverse)"));
    assert!(borrowed_coordinates.contains("SecretCycleCoordinatesV1(SecretCopyValueV1::new(("));
    assert!(!borrowed_coordinates.contains("(*self)"));
    let coordinate_owner = between(
        field,
        "pub(super) struct SecretCycleCoordinatesV1",
        "struct SecretU256V1",
    );
    assert!(coordinate_owner.contains("SecretCopyValueV1<(F, F)>"));
    assert!(coordinate_owner.contains("fn component_pair_ref(&self) -> &(F, F)"));
    assert!(coordinate_owner.contains("self.0.as_ref()"));
    assert!(!coordinate_owner.contains("-> (F, F)"));
    let membership = include_str!("../../membership.rs");
    assert!(membership.contains("Option<&'c1 Field25519>"));
    assert!(membership.contains("Option<&'c2 HelioseleneField>"));
    assert!(membership.contains("None::<&Field25519>"));
    assert!(membership.contains("None::<&HelioseleneField>"));
    assert!(!membership.contains(".h.scale(*mask)"));
    assert!(!membership.contains("prior_commitment - borrowed_secret_scale_v1"));
    assert!(membership.contains("secret_unblind_helios_coordinates_v1"));
    assert!(membership.contains("secret_unblind_selene_coordinates_v1"));
    assert!(membership.contains(".secret_coordinates_ref_v1()"));
    assert!(!membership.contains("(*point.expose_ref())"));
    assert_eq!(membership.matches("let hash_witness =").count(), 2);
    assert_eq!(
        membership
            .matches("Some(hash_witness.component_refs())")
            .count(),
        2
    );
    assert!(!membership.contains("Some(secret_unblind"));
    assert!(membership.contains("let (hash_x, hash_y, _) = match prior_mask"));
    let helios = between(
        membership,
        "fn secret_unblind_helios_coordinates_v1",
        "fn secret_unblind_selene_coordinates_v1",
    );
    assert!(helios.contains("SecretMultiexpBuilder::<HeliosSuite>::new(2)"));
    assert!(helios.contains("terms.push(&HelioseleneField::ONE, prior_commitment)?"));
    assert!(helios.contains("terms.push(mask, &negative_h)?"));
    assert!(helios.contains("let point = terms.evaluate()?;"));
    assert!(helios.contains("point\n        .expose_ref()\n        .secret_coordinates_ref_v1()"));
    assert!(helios.contains("drop(point);"));
    let selene = between(
        membership,
        "fn secret_unblind_selene_coordinates_v1",
        "const ED25519_WEI_A",
    );
    assert!(selene.contains("SecretMultiexpBuilder::<SeleneSuite>::new(2)"));
    assert!(selene.contains("terms.push(&Field25519::ONE, prior_commitment)?"));
    assert!(selene.contains("terms.push(mask, &negative_h)?"));
    assert!(selene.contains("let point = terms.evaluate()?;"));
    assert!(selene.contains("point\n        .expose_ref()\n        .secret_coordinates_ref_v1()"));
    assert!(selene.contains("drop(point);"));
    let c1_branch = between(
        membership,
        "for branch in these_c1_branches",
        "for branch in these_c2_branches",
    );
    let c1_owner = c1_branch.find("let hash_witness =").unwrap();
    let c1_borrow = c1_branch
        .find("Some(hash_witness.component_refs())")
        .unwrap();
    assert!(c1_owner < c1_borrow);
    let c2_branch = between(
        membership,
        "for branch in these_c2_branches",
        "fn verify_membership",
    );
    let c2_owner = c2_branch.find("let hash_witness =").unwrap();
    let c2_borrow = c2_branch
        .find("Some(hash_witness.component_refs())")
        .unwrap();
    assert!(c2_owner < c2_borrow);
}
