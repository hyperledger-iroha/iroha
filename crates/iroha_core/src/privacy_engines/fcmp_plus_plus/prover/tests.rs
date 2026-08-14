use super::*;
use crate::privacy_engines::fcmp_plus_plus::{
    FCMP_MAX_INPUTS_NATIVE_V1, FCMP_MAX_OUTPUTS_NATIVE_V1, FCMP_MAX_PROOF_WIRE_BYTES_V1,
    FCMP_NATIVE_KAT_PUBLIC_SHA256_V1, FCMP_NATIVE_KAT_WIRE_SHA256_V1, FCMP_OUTPUT_TUPLE_BYTES_V1,
    FailingRngV1, build_fcmp_frontier_v1,
    field::{encode_field25519_scalar, encode_helioselene_scalar, hash_helios, hash_selene},
    output_from_multiples, verify_fcmp_plus_plus_v1, verify_fcmp_transaction_v1,
};
use core::cell::Cell;
use rand_08::{SeedableRng as _, rngs::StdRng};
use sha2::{Digest as _, Sha256};
const TEST_AMOUNT: u64 = 5;
thread_local! {
    static PROVER_COPY_CLEARS: Cell<usize> = const { Cell::new(0) };
}
fn reset_prover_secret_copy_owner_drops() {
    PROVER_SECRET_COPY_OWNER_DROPS_V1.with(|drops| drops.set(0));
}
fn prover_secret_copy_owner_drops() -> usize {
    PROVER_SECRET_COPY_OWNER_DROPS_V1.with(Cell::get)
}
fn reset_prover_secret_scalar_owner_drops() {
    PROVER_SECRET_SCALAR_OWNER_DROPS_V1.with(|drops| drops.set(0));
}
fn prover_secret_scalar_owner_drops() -> usize {
    PROVER_SECRET_SCALAR_OWNER_DROPS_V1.with(Cell::get)
}
fn reset_prover_secret_point_owner_drops() {
    PROVER_SECRET_POINT_OWNER_DROPS_V1.with(|drops| drops.set(0));
}
fn prover_secret_point_owner_drops() -> usize {
    PROVER_SECRET_POINT_OWNER_DROPS_V1.with(Cell::get)
}
fn source_section<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
    source
        .split_once(start)
        .unwrap_or_else(|| panic!("missing source boundary {start}"))
        .1
        .split_once(end)
        .unwrap_or_else(|| panic!("missing source boundary {end}"))
        .0
}
fn assert_source_contains_all(source: &str, expected: &[&str]) {
    for needle in expected {
        assert!(source.contains(needle), "missing {needle}");
    }
}
fn assert_source_excludes_all(source: &str, forbidden: &[&str]) {
    for needle in forbidden {
        assert!(!source.contains(needle), "retained {needle}");
    }
}
fn assert_source_order(source: &str, expected: &[&str]) {
    let positions = expected
        .iter()
        .map(|needle| {
            source
                .find(needle)
                .unwrap_or_else(|| panic!("missing ordered source {needle}"))
        })
        .collect::<Vec<_>>();
    assert!(positions.windows(2).all(|pair| pair[0] < pair[1]));
}
fn assert_source_counts(source: &str, expected: &[(&str, usize)]) {
    for (needle, count) in expected {
        assert_eq!(source.matches(needle).count(), *count, "count for {needle}");
    }
}
macro_rules! source_part {
    ($source:expr; $start:expr => $end:expr) => {
        source_section($source, $start, $end)
    };
}
macro_rules! source_has {
    ($source:expr; $($needle:expr),* $(,)?) => {
        assert_source_contains_all($source, &[$($needle),*])
    };
}
macro_rules! source_lacks {
    ($source:expr; $($needle:expr),* $(,)?) => {
        assert_source_excludes_all($source, &[$($needle),*])
    };
}
macro_rules! source_order {
    ($source:expr; $($needle:expr),* $(,)?) => {
        assert_source_order($source, &[$($needle),*])
    };
}
macro_rules! source_counts {
    ($source:expr; $($needle:expr => $count:expr),* $(,)?) => {
        assert_source_counts($source, &[$(($needle, $count)),*])
    };
}
#[derive(Clone, Copy)]
struct TrackingCopy(u64);
impl Zeroize for TrackingCopy {
    fn zeroize(&mut self) {
        self.0 = 0;
        PROVER_COPY_CLEARS.with(|calls| calls.set(calls.get() + 1));
    }
}
#[test]
fn prover_copy_owner_clears_transfer_success_and_unwind_slots() {
    PROVER_COPY_CLEARS.with(|calls| calls.set(0));
    let mut source = TrackingCopy(7);
    let owner = ProverSecretCopyValueV1::take(&mut source);
    assert_eq!(source.0, 0);
    assert_eq!(owner.expose_ref().0, 7);
    assert_eq!(PROVER_COPY_CLEARS.with(Cell::get), 1);
    drop(owner);
    assert_eq!(PROVER_COPY_CLEARS.with(Cell::get), 2);
    PROVER_COPY_CLEARS.with(|calls| calls.set(0));
    assert!(
        std::panic::catch_unwind(|| {
            let _owner = ProverSecretCopyValueV1::new(TrackingCopy(11));
            panic!("tracking unwind");
        })
        .is_err()
    );
    assert_eq!(PROVER_COPY_CLEARS.with(Cell::get), 2);
}
#[test]
fn fixture_spendable_output_owns_inputs_and_secret_outputs_on_every_exit() {
    let expected = FcmpOutputTupleV1::new(
        ((ED25519_BASEPOINT_POINT * Scalar::from(17_u64)) + (generator_t() * Scalar::from(23_u64)))
            .compress()
            .to_bytes(),
        (ED25519_BASEPOINT_POINT * Scalar::from(31_u64))
            .compress()
            .to_bytes(),
        (super::super::range::amount_generator().expect("amount generator")
            * Scalar::from(TEST_AMOUNT)
            + ED25519_BASEPOINT_POINT * Scalar::from(37_u64))
        .compress()
        .to_bytes(),
    )
    .expect("legacy fixture equation remains canonical");
    reset_prover_secret_copy_owner_drops();
    let fixture = fcmp_fixture_spendable_output_from_scalars_v1(
        Scalar::from(17_u64),
        Scalar::from(23_u64),
        Scalar::from(31_u64),
        TEST_AMOUNT,
        Scalar::from(37_u64),
    )
    .expect("owned fixture output");
    assert_eq!(fixture.0, expected);
    assert_eq!(fixture.1.expose_ref(), &Scalar::from(17_u64).to_bytes());
    assert_eq!(fixture.2.expose_ref(), &Scalar::from(23_u64).to_bytes());
    assert_eq!(prover_secret_copy_owner_drops(), 6);
    drop(fixture);
    assert_eq!(prover_secret_copy_owner_drops(), 8);
    reset_prover_secret_copy_owner_drops();
    let error = fcmp_fixture_spendable_output_from_scalars_v1(
        Scalar::ZERO,
        Scalar::ZERO,
        Scalar::ZERO,
        0,
        Scalar::ZERO,
    )
    .err()
    .expect("identity fixture output must reject");
    assert_eq!(error, FcmpNativeErrorV1::EdwardsPointIdentity);
    assert_eq!(prover_secret_copy_owner_drops(), 6);
    reset_prover_secret_copy_owner_drops();
    let unwind = std::panic::catch_unwind(|| {
        let _fixture = fcmp_fixture_spendable_output_from_scalars_v1(
            Scalar::from(41_u64),
            Scalar::from(43_u64),
            Scalar::from(47_u64),
            TEST_AMOUNT,
            Scalar::from(53_u64),
        )
        .expect("owned unwind fixture");
        assert_eq!(prover_secret_copy_owner_drops(), 6);
        panic!("exercise secret fixture-output owner unwind");
    });
    assert!(unwind.is_err());
    assert_eq!(prover_secret_copy_owner_drops(), 8);
}
#[test]
fn fixture_spendable_output_source_stays_owned_through_release_transfer() {
    let source = include_str!("../prover.rs");
    source_has!(source; "#[cfg(any(test, feature = \"privacy-release-evidence\"))]\ntype FcmpFixtureSpendableOutputV1 = (");
    let fixture = source_part!(source; "fn fcmp_fixture_spendable_output_from_scalars_v1(" => "fn fcmp_fixture_spendable_output_v1(");
    source_has!(fixture; "mut spend_x: Scalar", "mut output_y: Scalar", "mut linking: Scalar", "mut amount: u64", "mut commitment_mask: Scalar", "ProverSecretCopyValueV1::new(spend_x.expose_ref().to_bytes())", "ProverSecretCopyValueV1::new(output_y.expose_ref().to_bytes())", "Ok((output, spend_x_bytes, output_y_bytes))");
    source_counts!(fixture; "ProverSecretCopyValueV1::take(&mut" => 5, "secret_edwards_product_v1(" => 5, "Zeroizing::new(&*" => 2);
    assert!(
        fixture.rfind("ProverSecretCopyValueV1::take(&mut").unwrap()
            < fixture.find("amount_generator()?").unwrap()
    );
    source_lacks!(fixture; "ED25519_BASEPOINT_POINT * spend_x", "generator_t() * output_y", "ED25519_BASEPOINT_POINT * linking", "Scalar::from(amount)", "ED25519_BASEPOINT_POINT * commitment_mask", "Ok((output, spend_x.to_bytes(), output_y.to_bytes()))");
    source_has!(source_part!(source; "fn from_secret_byte_owners_v1(" => "#[cfg(test)]\n    fn duplicate_for_test"); "spend_x_bytes: ProverSecretCopyValueV1<[u8; 32]>", "output_y_bytes: ProverSecretCopyValueV1<[u8; 32]>");
    let release_fixture = source_part!(source; "pub(crate) fn fcmp_release_fixture_v1(" => "/// Build a maximum-shape fixture whose first canonical branch");
    source_counts!(release_fixture; "FcmpProverInputV1::from_secret_byte_owners_v1(" => 3);
    source_lacks!(release_fixture; "spend_x.expose_copy()", "output_y.expose_copy()", "FcmpProverInputV1::new(");
    source_has!(source; "#[cfg(test)]\npub(crate) fn fcmp_test_spendable_output_v1(");
    source_has!(source_part!(source; "pub(crate) fn fcmp_test_spendable_output_v1(" => "fn fcmp_fixture_output_opening_v1("); "spend_x.expose_copy()", "output_y.expose_copy()");
}
#[test]
fn fixture_u64_wrapper_owns_slots_on_success_error_and_inner_unwind() {
    let expected = fcmp_fixture_spendable_output_from_scalars_v1(
        Scalar::from(17_u64),
        Scalar::from(23_u64),
        Scalar::from(31_u64),
        TEST_AMOUNT,
        Scalar::from(37_u64),
    )
    .expect("owned scalar fixture");
    let expected_output = expected.0;
    drop(expected);
    reset_prover_secret_copy_owner_drops();
    let fixture =
        fcmp_fixture_spendable_output_v1(17, 23, 31, TEST_AMOUNT, 37).expect("owned u64 fixture");
    assert_eq!(fixture.0, expected_output);
    assert_eq!(fixture.1.expose_ref(), &Scalar::from(17_u64).to_bytes());
    assert_eq!(fixture.2.expose_ref(), &Scalar::from(23_u64).to_bytes());
    assert_eq!(prover_secret_copy_owner_drops(), 11);
    drop(fixture);
    assert_eq!(prover_secret_copy_owner_drops(), 13);
    reset_prover_secret_copy_owner_drops();
    let error = fcmp_fixture_spendable_output_v1(0, 0, 0, 0, 0)
        .err()
        .expect("identity u64 fixture must reject");
    assert_eq!(error, FcmpNativeErrorV1::EdwardsPointIdentity);
    assert_eq!(prover_secret_copy_owner_drops(), 11);
    reset_prover_secret_copy_owner_drops();
    let unwind = std::panic::catch_unwind(|| {
        let _result: Result<FcmpFixtureSpendableOutputV1, FcmpNativeErrorV1> =
            with_fcmp_fixture_u64_secret_owners_v1(
                ProverSecretCopyValueV1::new(41_u64),
                ProverSecretCopyValueV1::new(43_u64),
                ProverSecretCopyValueV1::new(47_u64),
                ProverSecretCopyValueV1::new(TEST_AMOUNT),
                ProverSecretCopyValueV1::new(53_u64),
                |spend_x, output_y, linking, amount, commitment_mask| {
                    assert_eq!(
                        (*spend_x, *output_y, *linking, *amount, *commitment_mask),
                        (41, 43, 47, TEST_AMOUNT, 53)
                    );
                    panic!("exercise fixture u64 inner unwind");
                },
            );
    });
    assert!(unwind.is_err());
    assert_eq!(prover_secret_copy_owner_drops(), 5);
}
#[test]
fn fixture_u64_wrapper_source_takes_every_slot_before_inner_conversion() {
    let source = include_str!("../prover.rs");
    source_has!(source; "#[cfg(any(test, feature = \"privacy-release-evidence\"))]\nfn with_fcmp_fixture_u64_secret_owners_v1<T>(", "#[cfg(any(test, feature = \"privacy-release-evidence\"))]\nfn fcmp_fixture_spendable_output_v1(");
    let owner_scope = source_part!(source; "fn with_fcmp_fixture_u64_secret_owners_v1<T>(" => "fn fcmp_fixture_spendable_output_v1(");
    source_counts!(owner_scope; "ProverSecretCopyValueV1<u64>" => 5, ".expose_ref()" => 5);
    source_has!(owner_scope; "operation: impl FnOnce(&u64, &u64, &u64, &u64, &u64) -> T");
    let wrapper = source_part!(source; "fn fcmp_fixture_spendable_output_v1(" => "#[cfg(test)]\npub(crate) fn fcmp_test_spendable_output_v1(");
    source_has!(wrapper; "mut spend_x: u64", "mut output_y: u64", "mut linking: u64", "mut amount: u64", "mut commitment_mask: u64", "*amount,");
    source_counts!(wrapper; "ProverSecretCopyValueV1::take(&mut" => 5, "Scalar::from(*" => 4);
    assert!(
        wrapper.rfind("ProverSecretCopyValueV1::take(&mut").unwrap()
            < wrapper
                .find("with_fcmp_fixture_u64_secret_owners_v1(")
                .unwrap()
    );
    source_order!(wrapper; "with_fcmp_fixture_u64_secret_owners_v1(", "Scalar::from(*spend_x)");
    source_lacks!(wrapper; "let spend_x_scalar", "let output_y_scalar", "?");
}
#[test]
fn fixture_output_opening_owns_success_error_mismatch_and_unwind_slots() {
    let expected_output = FcmpOutputTupleV1::new(
        (ED25519_BASEPOINT_POINT * Scalar::from(43_u64))
            .compress()
            .to_bytes(),
        (ED25519_BASEPOINT_POINT * Scalar::from(47_u64))
            .compress()
            .to_bytes(),
        (super::super::range::amount_generator().expect("amount generator")
            * Scalar::from(TEST_AMOUNT)
            + ED25519_BASEPOINT_POINT * Scalar::from(79_u64))
        .compress()
        .to_bytes(),
    )
    .expect("legacy fixture opening output");
    reset_prover_secret_copy_owner_drops();
    let opening =
        fcmp_fixture_output_opening_v1(43, 47, TEST_AMOUNT, 79).expect("owned fixture opening");
    assert_eq!(opening.output(), expected_output);
    assert_eq!(opening.amount(), &TEST_AMOUNT);
    assert_eq!(
        &*opening.commitment_mask(),
        &Scalar::from(79_u64).to_bytes()
    );
    assert_eq!(prover_secret_copy_owner_drops(), 9);
    drop(opening);
    assert_eq!(prover_secret_copy_owner_drops(), 9);
    reset_prover_secret_copy_owner_drops();
    let error = fcmp_fixture_output_opening_v1(0, 0, 0, 0)
        .err()
        .expect("identity fixture opening must reject");
    assert_eq!(error, FcmpNativeErrorV1::EdwardsPointIdentity);
    assert_eq!(prover_secret_copy_owner_drops(), 8);
    reset_prover_secret_copy_owner_drops();
    let mismatch = with_fcmp_fixture_output_u64_secret_owners_v1(
        ProverSecretCopyValueV1::new(11_u64),
        ProverSecretCopyValueV1::new(13_u64),
        ProverSecretCopyValueV1::new(TEST_AMOUNT),
        ProverSecretCopyValueV1::new(17_u64),
        |output_key, linking, amount, mask| {
            let mask_scalar = ProverSecretCopyValueV1::new(Scalar::from(*mask));
            let mask_bytes = ProverSecretCopyValueV1::new(mask_scalar.expose_ref().to_bytes());
            FcmpOutputCommitmentOpeningV1::new_borrowed(
                output_from_multiples(*output_key, *linking, *mask),
                amount,
                mask_bytes.expose_ref(),
            )
        },
    )
    .err()
    .expect("missing positive amount component must mismatch");
    assert_eq!(mismatch, FcmpNativeErrorV1::RangeCommitmentOpeningMismatch);
    assert_eq!(prover_secret_copy_owner_drops(), 6);
    reset_prover_secret_copy_owner_drops();
    let unwind = std::panic::catch_unwind(|| {
        let _result: Result<FcmpOutputCommitmentOpeningV1, FcmpNativeErrorV1> =
            with_fcmp_fixture_output_u64_secret_owners_v1(
                ProverSecretCopyValueV1::new(19_u64),
                ProverSecretCopyValueV1::new(23_u64),
                ProverSecretCopyValueV1::new(TEST_AMOUNT),
                ProverSecretCopyValueV1::new(29_u64),
                |output_key, linking, amount, mask| {
                    let output_key_scalar = ProverSecretCopyValueV1::new(Scalar::from(*output_key));
                    let linking_scalar = ProverSecretCopyValueV1::new(Scalar::from(*linking));
                    let amount_scalar = ProverSecretCopyValueV1::new(Scalar::from(*amount));
                    let mask_scalar = ProverSecretCopyValueV1::new(Scalar::from(*mask));
                    let output_key_point = secret_edwards_product_v1(
                        &ED25519_BASEPOINT_POINT,
                        output_key_scalar.expose_ref(),
                    );
                    let linking_point = secret_edwards_product_v1(
                        &ED25519_BASEPOINT_POINT,
                        linking_scalar.expose_ref(),
                    );
                    let amount_generator =
                        super::super::range::amount_generator().expect("amount generator");
                    let amount_component =
                        secret_edwards_product_v1(&amount_generator, amount_scalar.expose_ref());
                    let mask_component = secret_edwards_product_v1(
                        &ED25519_BASEPOINT_POINT,
                        mask_scalar.expose_ref(),
                    );
                    let amount_point = Zeroizing::new(&*amount_component + &*mask_component);
                    let _ = core::hint::black_box((
                        &*output_key_point,
                        &*linking_point,
                        &*amount_point,
                    ));
                    panic!("exercise fixture opening inner unwind");
                },
            );
    });
    assert!(unwind.is_err());
    assert_eq!(prover_secret_copy_owner_drops(), 8);
}
#[test]
fn fixture_output_opening_source_stays_owned_until_borrowed_constructor() {
    let source = include_str!("../prover.rs");
    source_has!(source; "#[cfg(any(test, feature = \"privacy-release-evidence\"))]\nfn with_fcmp_fixture_output_u64_secret_owners_v1<T>(", "#[cfg(any(test, feature = \"privacy-release-evidence\"))]\nfn fcmp_fixture_output_opening_v1(");
    let owner_scope = source_part!(source; "fn with_fcmp_fixture_output_u64_secret_owners_v1<T>(" => "fn fcmp_fixture_output_opening_v1(");
    source_counts!(owner_scope; "ProverSecretCopyValueV1<u64>" => 4, ".expose_ref()" => 4);
    source_has!(owner_scope; "operation: impl FnOnce(&u64, &u64, &u64, &u64) -> T");
    let fixture = source_part!(source; "fn fcmp_fixture_output_opening_v1(" => "fn fcmp_fixture_rerandomization_v1(");
    source_has!(fixture; "mut output_key: u64", "mut linking: u64", "mut amount: u64", "mut mask: u64", "FcmpOutputCommitmentOpeningV1::new_borrowed(output, amount, mask_bytes.expose_ref())");
    source_counts!(fixture; "ProverSecretCopyValueV1::take(&mut" => 4, "ProverSecretCopyValueV1::new(Scalar::from(*" => 4, "secret_edwards_product_v1(" => 4, "Zeroizing::new(&*" => 1);
    assert!(
        fixture.rfind("ProverSecretCopyValueV1::take(&mut").unwrap()
            < fixture
                .find("with_fcmp_fixture_output_u64_secret_owners_v1(")
                .unwrap()
    );
    source_order!(fixture; "with_fcmp_fixture_output_u64_secret_owners_v1(", "ProverSecretCopyValueV1::new(Scalar::from(*output_key))", "super::range::amount_generator()?", "let output = FcmpOutputTupleV1::new(", "ProverSecretCopyValueV1::new(mask_scalar.expose_ref().to_bytes())", "FcmpOutputCommitmentOpeningV1::new_borrowed(");
    source_lacks!(fixture; "let mask = Scalar::from(mask)", "ED25519_BASEPOINT_POINT * Scalar::from", "FcmpOutputCommitmentOpeningV1::new(");
}
#[test]
fn fixture_rerandomization_owns_success_error_and_unwind_slots() {
    reset_prover_secret_copy_owner_drops();
    let rerandomization =
        fcmp_fixture_rerandomization_v1(61, 67, 71, 41).expect("owned rerandomization fixture");
    assert_eq!(&rerandomization.output, &Scalar::from(61_u64));
    assert_eq!(&rerandomization.linking, &Scalar::from(67_u64));
    assert_eq!(
        &rerandomization.rerandomization_blind,
        &Scalar::from(71_u64)
    );
    assert_eq!(&rerandomization.commitment, &Scalar::from(41_u64));
    assert_eq!(prover_secret_copy_owner_drops(), 16);
    drop(rerandomization);
    assert_eq!(prover_secret_copy_owner_drops(), 16);
    reset_prover_secret_copy_owner_drops();
    let error = fcmp_fixture_rerandomization_v1(61, 67, 71, 0)
        .err()
        .expect("zero final rerandomization scalar must reject");
    assert_eq!(error, FcmpNativeErrorV1::ScalarEncoding);
    assert_eq!(prover_secret_copy_owner_drops(), 15);
    reset_prover_secret_copy_owner_drops();
    let unwind = std::panic::catch_unwind(|| {
        let _result: Result<FcmpInputRerandomizationV1, FcmpNativeErrorV1> =
            with_fcmp_fixture_rerandomization_u64_secret_owners_v1(
                ProverSecretCopyValueV1::new(73_u64),
                ProverSecretCopyValueV1::new(79_u64),
                ProverSecretCopyValueV1::new(83_u64),
                ProverSecretCopyValueV1::new(89_u64),
                |output, linking, blind, commitment| {
                    let scalars = [
                        ProverSecretCopyValueV1::new(Scalar::from(*output)),
                        ProverSecretCopyValueV1::new(Scalar::from(*linking)),
                        ProverSecretCopyValueV1::new(Scalar::from(*blind)),
                        ProverSecretCopyValueV1::new(Scalar::from(*commitment)),
                    ];
                    let bytes = [
                        ProverSecretCopyValueV1::new(scalars[0].expose_ref().to_bytes()),
                        ProverSecretCopyValueV1::new(scalars[1].expose_ref().to_bytes()),
                        ProverSecretCopyValueV1::new(scalars[2].expose_ref().to_bytes()),
                        ProverSecretCopyValueV1::new(scalars[3].expose_ref().to_bytes()),
                    ];
                    let _ = core::hint::black_box((&scalars, &bytes));
                    panic!("exercise rerandomization fixture inner unwind");
                },
            );
    });
    assert!(unwind.is_err());
    assert_eq!(prover_secret_copy_owner_drops(), 12);
}
#[test]
fn fixture_rerandomization_source_keeps_feature_secret_owners_in_order() {
    let source = include_str!("../prover.rs");
    source_has!(source; "#[cfg(any(test, feature = \"privacy-release-evidence\"))]\nfn with_fcmp_fixture_rerandomization_u64_secret_owners_v1<T>(", "#[cfg(any(test, feature = \"privacy-release-evidence\"))]\nfn fcmp_fixture_rerandomization_v1(");
    let owner_scope = source_part!(source; "fn with_fcmp_fixture_rerandomization_u64_secret_owners_v1<T>(" => "fn fcmp_fixture_rerandomization_v1(");
    source_counts!(owner_scope; "ProverSecretCopyValueV1<u64>" => 4, ".expose_ref()" => 4);
    source_has!(owner_scope; "operation: impl FnOnce(&u64, &u64, &u64, &u64) -> T");
    let fixture = source_part!(source; "fn fcmp_fixture_rerandomization_v1(" => "/// Build the canonical deterministic FCMP++ release fixture.");
    source_has!(fixture; "mut output: u64", "mut linking: u64", "mut blind: u64", "mut commitment: u64");
    source_counts!(fixture; "ProverSecretCopyValueV1::take(&mut" => 4, "ProverSecretCopyValueV1::new(Scalar::from(*" => 4, ".expose_ref().to_bytes())" => 4);
    assert!(
        fixture.rfind("ProverSecretCopyValueV1::take(&mut").unwrap()
            < fixture
                .find("with_fcmp_fixture_rerandomization_u64_secret_owners_v1(")
                .unwrap()
    );
    source_order!(fixture; "with_fcmp_fixture_rerandomization_u64_secret_owners_v1(", "ProverSecretCopyValueV1::new(Scalar::from(*output))", "ProverSecretCopyValueV1::new(output_scalar.expose_ref().to_bytes())", "FcmpInputRerandomizationV1::from_rerandomization_secret_byte_owners_v1(");
    source_lacks!(fixture; "FcmpInputRerandomizationV1::new(", "Scalar::from(output)", "Scalar::from(linking)", "Scalar::from(blind)", "Scalar::from(commitment)");
}
#[test]
fn fixture_leaf_coordinate_scope_owns_success_error_and_unwind() {
    let leaves = [
        output_from_multiples(101, 103, 107),
        output_from_multiples(109, 113, 127),
    ];
    let expected_root = build_fcmp_frontier_v1(&leaves)
        .expect("canonical hidden-leaf frontier")
        .root;
    reset_prover_secret_copy_owner_drops();
    let hash = with_fcmp_fixture_leaf_coordinate_owners_v1(
        &leaves,
        secret_edwards_to_wei25519_v1,
        prover_secret_hash_selene_v1,
    )
    .expect("owned hidden-leaf coordinates");
    assert_eq!(hash.expose_ref().encode(), expected_root.point());
    assert_eq!(prover_secret_copy_owner_drops(), 6);
    reset_prover_secret_copy_owner_drops();
    let conversions = Cell::new(0_usize);
    let conversion_error = with_fcmp_fixture_leaf_coordinate_owners_v1(
        &leaves,
        |point| {
            let index = conversions.get();
            conversions.set(index + 1);
            if index == 2 {
                return Err(FcmpNativeErrorV1::EdwardsPointEncoding);
            }
            secret_edwards_to_wei25519_v1(point)
        },
        hash_selene,
    )
    .err()
    .expect("injected third-point conversion failure");
    assert_eq!(conversion_error, FcmpNativeErrorV1::EdwardsPointEncoding);
    assert_eq!(conversions.get(), 3);
    assert_eq!(prover_secret_copy_owner_drops(), 2);
    reset_prover_secret_copy_owner_drops();
    let empty_hash_error = with_fcmp_fixture_leaf_coordinate_owners_v1(
        &[],
        secret_edwards_to_wei25519_v1,
        hash_selene,
    )
    .err()
    .expect("empty canonical Selene hash must reject");
    assert_eq!(empty_hash_error, FcmpNativeErrorV1::BranchWidth);
    assert_eq!(prover_secret_copy_owner_drops(), 0);
    reset_prover_secret_copy_owner_drops();
    let hash_error = with_fcmp_fixture_leaf_coordinate_owners_v1(
        &leaves,
        secret_edwards_to_wei25519_v1,
        |_| -> Result<SelenePoint, FcmpNativeErrorV1> {
            Err(FcmpNativeErrorV1::ArithmeticInvariant)
        },
    )
    .err()
    .expect("injected hash failure");
    assert_eq!(hash_error, FcmpNativeErrorV1::ArithmeticInvariant);
    assert_eq!(prover_secret_copy_owner_drops(), 6);
    reset_prover_secret_copy_owner_drops();
    let conversion_unwind = std::panic::catch_unwind(|| {
        let mut conversions = 0_usize;
        let _result: Result<SelenePoint, FcmpNativeErrorV1> =
            with_fcmp_fixture_leaf_coordinate_owners_v1(
                &leaves,
                |point| {
                    if conversions == 2 {
                        panic!("exercise hidden-leaf conversion unwind");
                    }
                    conversions += 1;
                    secret_edwards_to_wei25519_v1(point)
                },
                hash_selene,
            );
    });
    assert!(conversion_unwind.is_err());
    assert_eq!(prover_secret_copy_owner_drops(), 2);
    reset_prover_secret_copy_owner_drops();
    let hash_unwind = std::panic::catch_unwind(|| {
        let _result: Result<SelenePoint, FcmpNativeErrorV1> =
            with_fcmp_fixture_leaf_coordinate_owners_v1(
                &leaves,
                secret_edwards_to_wei25519_v1,
                |_| panic!("exercise hidden-leaf hash unwind"),
            );
    });
    assert!(hash_unwind.is_err());
    assert_eq!(prover_secret_copy_owner_drops(), 6);
}
#[test]
fn fixture_secret_selene_hash_matches_equation_and_owns_all_exit_paths() {
    let values = [Field25519::ONE, Field25519::ONE.add_ref(&Field25519::ONE)];
    let expected = hash_selene(&values).expect("public canonical Selene equation");
    reset_prover_secret_point_owner_drops();
    {
        let actual = prover_secret_hash_selene_v1(&values)
            .expect("complete private Selene multiexponentiation");
        assert_eq!(actual.expose_ref().encode(), expected.encode());
        assert_eq!(prover_secret_point_owner_drops(), 0);
    }
    assert_eq!(prover_secret_point_owner_drops(), 1);
    reset_prover_secret_point_owner_drops();
    let downstream_error: Result<(), FcmpNativeErrorV1> = prover_secret_hash_selene_v1(&values)
        .and_then(|owned| {
            assert_eq!(prover_secret_point_owner_drops(), 0);
            let _ = core::hint::black_box(owned.expose_ref());
            Err(FcmpNativeErrorV1::ArithmeticInvariant)
        });
    assert_eq!(
        downstream_error,
        Err(FcmpNativeErrorV1::ArithmeticInvariant)
    );
    assert_eq!(prover_secret_point_owner_drops(), 1);
    assert!(matches!(
        prover_secret_hash_selene_v1(&[]),
        Err(FcmpNativeErrorV1::BranchWidth)
    ));
    let mut incomplete = SecretMultiexpBuilder::<SeleneSuite>::new(values.len() + 2)
        .expect("one deliberately incomplete slot");
    incomplete
        .push(&Field25519::ONE, &selene_hash_initializer())
        .expect("initializer term");
    for (scalar, generator) in values.iter().zip(selene_generators()) {
        incomplete.push(scalar, generator).expect("ordered term");
    }
    let incomplete_result: Result<(), FcmpNativeErrorV1> =
        incomplete.evaluate().map(|_| ()).map_err(Into::into);
    assert_eq!(
        incomplete_result,
        Err(FcmpNativeErrorV1::ArithmeticInvariant)
    );
    let mut full =
        SecretMultiexpBuilder::<SeleneSuite>::new(1).expect("initializer-only exact capacity");
    full.push(&Field25519::ONE, &selene_hash_initializer())
        .expect("initializer fills builder");
    assert_eq!(full.capacity(), 1);
    assert_eq!(full.len(), 1);
    assert_eq!(
        full.push(&values[0], &selene_generators()[0])
            .map_err(FcmpNativeErrorV1::from),
        Err(FcmpNativeErrorV1::TreeFull)
    );
    assert_eq!(
        SecretMultiexpBuilder::<SeleneSuite>::new(usize::MAX)
            .err()
            .map(FcmpNativeErrorV1::from),
        Some(FcmpNativeErrorV1::TreeFull)
    );
    reset_prover_secret_point_owner_drops();
    let unwind = std::panic::catch_unwind(|| {
        let _owned =
            prover_secret_hash_selene_v1(&values).expect("owned Selene point before unwind");
        assert_eq!(prover_secret_point_owner_drops(), 0);
        panic!("exercise private Selene point-owner unwind");
    });
    assert!(unwind.is_err());
    assert_eq!(prover_secret_point_owner_drops(), 1);
}
#[test]
fn fixture_secret_cycle_step_matches_public_equations_and_owns_copies() {
    let helios_values = [
        HelioseleneField::ONE,
        HelioseleneField::ONE.add_ref(&HelioseleneField::ONE),
    ];
    let expected_helios = hash_helios(&helios_values).expect("public Helios equation");
    reset_prover_secret_point_owner_drops();
    reset_prover_secret_copy_owner_drops();
    {
        let actual_helios = prover_secret_hash_helios_v1(&helios_values)
            .expect("private Helios multiexponentiation");
        assert_eq!(
            actual_helios.expose_ref().encode(),
            expected_helios.encode()
        );
        let expected_x = expected_helios.x().expect("nonidentity public Helios hash");
        let child = prover_secret_helios_x_v1(&actual_helios).expect("owned Helios x coordinate");
        assert!(child.expose_ref().eq_ref(&expected_x));
        assert_eq!(prover_secret_point_owner_drops(), 0);
        assert_eq!(prover_secret_copy_owner_drops(), 0);
        let root_encoding = fcmp_fixture_secret_helios_encoding_v1(&actual_helios)
            .expect("owned Helios root encoding");
        assert_eq!(root_encoding.as_ref(), &expected_helios.encode());
        let encoded = encode_secret_field25519_scalar_v1(child.expose_ref());
        assert_eq!(encoded.as_ref(), &encode_field25519_scalar(expected_x));
        let mut branches =
            zeroizing_exact_secret_buffer_v1::<Vec<[u8; 32]>>(1).expect("one branch");
        push_fcmp_fixture_secret_branch_v1(&mut branches, encoded)
            .expect("preallocated branch insertion");
        assert_eq!(
            branches.as_slice(),
            &[vec![encode_field25519_scalar(expected_x)]]
        );
    }
    assert_eq!(prover_secret_point_owner_drops(), 1);
    assert_eq!(prover_secret_copy_owner_drops(), 1);
    reset_prover_secret_point_owner_drops();
    {
        let expected_selene = hash_selene(&[Field25519::ONE]).expect("public Selene equation");
        let actual_selene = prover_secret_hash_selene_v1(&[Field25519::ONE])
            .expect("private Selene multiexponentiation");
        let expected_x = expected_selene.x().expect("nonidentity public Selene hash");
        let child = prover_secret_selene_x_v1(&actual_selene).expect("owned Selene x coordinate");
        assert!(child.expose_ref().eq_ref(&expected_x));
        assert_eq!(
            encode_secret_helioselene_scalar_v1(child.expose_ref()).as_ref(),
            &encode_helioselene_scalar(expected_x)
        );
    }
    assert_eq!(prover_secret_point_owner_drops(), 1);
    reset_prover_secret_point_owner_drops();
    {
        let identity = ProverSecretPointV1(SelenePoint::identity());
        assert!(matches!(
            prover_secret_selene_x_v1(&identity),
            Err(FcmpNativeErrorV1::ArithmeticInvariant)
        ));
        assert_eq!(
            ct_secret_selene_point_eq_v1(
                &identity,
                &hash_selene(&[Field25519::ONE]).expect("public Selene comparison point"),
            ),
            Err(FcmpNativeErrorV1::ArithmeticInvariant)
        );
    }
    assert_eq!(prover_secret_point_owner_drops(), 1);
    reset_prover_secret_point_owner_drops();
    {
        let identity = ProverSecretPointV1(HeliosPoint::identity());
        assert!(matches!(
            prover_secret_helios_x_v1(&identity),
            Err(FcmpNativeErrorV1::ArithmeticInvariant)
        ));
        assert!(matches!(
            fcmp_fixture_secret_helios_encoding_v1(&identity),
            Err(FcmpNativeErrorV1::ArithmeticInvariant)
        ));
    }
    assert_eq!(prover_secret_point_owner_drops(), 1);
    let mut no_capacity: Zeroizing<Vec<Vec<[u8; 32]>>> = Zeroizing::new(Vec::new());
    assert!(matches!(
        push_fcmp_fixture_secret_branch_v1(
            &mut no_capacity,
            encode_secret_field25519_scalar_v1(&Field25519::ONE),
        ),
        Err(FcmpNativeErrorV1::ArithmeticInvariant)
    ));
}
#[test]
fn fixture_secret_cycle_source_has_no_raw_coordinate_hash_or_branch_boundary() {
    let source = include_str!("../prover.rs");
    let release_fixture = source_section(
        source,
        "pub(crate) fn fcmp_release_fixture_v1(",
        "/// Build a maximum-shape fixture whose first canonical branch",
    );
    assert_source_contains_all(
        release_fixture,
        &[
            "prover_secret_selene_x_v1(&current_selene)?",
            "prover_secret_helios_x_v1(",
            "encode_secret_helioselene_scalar_v1(child.expose_ref())",
            "encode_secret_field25519_scalar_v1(child.expose_ref())",
            "prover_secret_hash_helios_v1(core::slice::from_ref(",
            "prover_secret_hash_selene_v1(core::slice::from_ref(child.expose_ref()))?",
            "push_fcmp_fixture_secret_branch_v1(",
            "fcmp_fixture_secret_helios_encoding_v1(",
            ".expose_public_copy_v1()",
        ],
    );
    assert_source_excludes_all(
        release_fixture,
        &[
            ".expose_ref()\n                .x()",
            "branches.push(vec![",
            "Some(hash_helios(&[child])?)",
            "let mut next_selene = hash_selene(&[child])?",
            "current_helios\n                .ok_or",
            ".expose_copy()\n            .encode()",
        ],
    );
    assert_source_counts(release_fixture, &[(".expose_public_copy_v1()", 1)]);
    let x_helpers = source_section(
        source,
        "fn prover_secret_selene_x_v1(",
        "fn push_fcmp_fixture_secret_branch_v1(",
    );
    assert_source_counts(x_helpers, &[(".secret_x_owner_v1()", 2)]);
    assert_source_excludes_all(
        x_helpers,
        &[".secret_coordinates_v1()", "ProverSecretCopyValueV1::new("],
    );
    let field = include_str!("../field.rs");
    for helper in [
        "fn encode_secret_field25519_scalar_v1",
        "fn encode_secret_helioselene_scalar_v1",
    ] {
        assert_source_contains_all(
            source_section(field, helper, "}"),
            &[
                "let integer = SecretU256V1(value.retrieve())",
                "SecretEncodedScalarV1(SecretCopyValueV1::new(",
            ],
        );
    }
    assert_source_contains_all(
        source_section(
            field,
            "pub(super) fn secret_x_ref_v1(&self)",
            "/// Encode a private projective point",
        ),
        &[
            "self.z.invert()",
            "BorrowedZeroizingCopySlot(&mut inverse)",
            "SecretCycleScalarV1(SecretCopyValueV1::new(",
            "self.x.mul_ref(inverse.as_ref())",
            "drop(inverse);",
        ],
    );
    assert!(!field.contains("pub(super) fn secret_x_v1(mut self)"));
    let secret_encoding = source_section(
        field,
        "pub(super) fn secret_encode_v1(mut self)",
        "pub(super) fn secret_encode_ref_v1(&self)",
    );
    assert_source_contains_all(
        secret_encoding,
        &[
            "BorrowedZeroizingCopySlot(&mut self)",
            "BorrowedZeroizingCopySlot(&mut inverse)",
            "let x = SecretCopyValueV1::new(",
            "let y = SecretCopyValueV1::new(",
            "let integer = SecretU256V1(x.as_ref().retrieve())",
            "let y_integer = SecretU256V1(y.as_ref().retrieve())",
            "let y_bytes = SecretCopyValueV1::new(y_integer.0.to_le_bytes())",
            "encoded.as_mut()[31] |= (y_bytes.as_ref()[0] & 1) << 7",
            "Some(encoded)",
        ],
    );
    assert_source_order(
        secret_encoding,
        &[
            "BorrowedZeroizingCopySlot(&mut self)",
            "point.as_ref().z.invert()",
            "BorrowedZeroizingCopySlot(&mut inverse)",
            "if !bool::from(is_some)",
            "let x = SecretCopyValueV1::new(",
            "let y = SecretCopyValueV1::new(",
            "let integer = SecretU256V1(x.as_ref().retrieve())",
            "SecretEncodedScalarV1(SecretCopyValueV1::new(integer.0.to_le_bytes()))",
            "let y_integer = SecretU256V1(y.as_ref().retrieve())",
            "let y_bytes = SecretCopyValueV1::new(y_integer.0.to_le_bytes())",
            "encoded.as_mut()[31] |= (y_bytes.as_ref()[0] & 1) << 7",
            "drop(inverse);",
            "drop(point);",
            "Some(encoded)",
        ],
    );
    assert_source_excludes_all(secret_encoding, &["y.as_ref().is_odd_ref()"]);
}
#[test]
fn invalid_path_fixture_replacement_owns_success_error_and_unwind_slots() {
    reset_prover_secret_copy_owner_drops();
    let mut helios = [HelioseleneField::ONE];
    replace_first_secret_coordinate_v1(&mut helios, |original| {
        HelioseleneField::conditional_select(
            &HelioseleneField::ONE,
            &HelioseleneField::ONE.add_ref(&HelioseleneField::ONE),
            original.sub_ref(&HelioseleneField::ONE).ct_is_zero(),
        )
    })
    .expect("owned Helios replacement");
    assert!(helios[0].eq_ref(&HelioseleneField::ONE.add_ref(&HelioseleneField::ONE)));
    assert_eq!(prover_secret_copy_owner_drops(), 2);
    replace_first_secret_coordinate_v1(&mut helios, |original| {
        HelioseleneField::conditional_select(
            &HelioseleneField::ONE,
            &HelioseleneField::ONE.add_ref(&HelioseleneField::ONE),
            original.sub_ref(&HelioseleneField::ONE).ct_is_zero(),
        )
    })
    .expect("owned non-one Helios replacement");
    assert!(helios[0].eq_ref(&HelioseleneField::ONE));
    assert_eq!(prover_secret_copy_owner_drops(), 4);
    let mut selene = [Field25519::ONE];
    replace_first_secret_coordinate_v1(&mut selene, |original| {
        Field25519::conditional_select(
            &Field25519::ONE,
            &Field25519::ONE.add_ref(&Field25519::ONE),
            original.sub_ref(&Field25519::ONE).ct_is_zero(),
        )
    })
    .expect("owned Selene replacement");
    assert!(selene[0].eq_ref(&Field25519::ONE.add_ref(&Field25519::ONE)));
    assert_eq!(prover_secret_copy_owner_drops(), 6);
    let empty_error =
        replace_first_secret_coordinate_v1::<Field25519>(&mut [], |_| Field25519::ONE)
            .expect_err("empty branch must reject before ownership");
    assert_eq!(empty_error, FcmpNativeErrorV1::ArithmeticInvariant);
    assert_eq!(prover_secret_copy_owner_drops(), 6);
    reset_prover_secret_copy_owner_drops();
    let mut unwind_value = [Field25519::ONE];
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _result = replace_first_secret_coordinate_v1(&mut unwind_value, |_| {
            panic!("exercise invalid-path replacement unwind")
        });
    }));
    assert!(unwind.is_err());
    assert!(unwind_value[0].eq_ref(&Field25519::ZERO));
    assert_eq!(prover_secret_copy_owner_drops(), 1);
}
#[test]
fn invalid_path_fixture_source_takes_before_constant_time_replacement_and_restore() {
    let source = include_str!("../prover.rs");
    let helper = source
        .split_once("fn replace_first_secret_coordinate_v1")
        .expect("owned invalid-coordinate helper")
        .1
        .split_once("pub(crate) fn fcmp_release_invalid_path_fixture_v1")
        .expect("invalid-coordinate helper boundary")
        .0;
    let destination = helper.find(".first_mut()").expect("selected destination");
    let take = helper
        .find("ProverSecretCopyValueV1::take(destination)")
        .expect("immediate destination take");
    let replacement = helper
        .find("ProverSecretCopyValueV1::new(replacement(original.expose_ref()))")
        .expect("owned replacement");
    let restore = helper
        .find("*destination = replacement.expose_copy()")
        .expect("final-owner restore");
    let drop_replacement = helper.find("drop(replacement)").expect("replacement clear");
    let drop_original = helper.find("drop(original)").expect("original clear");
    assert!(
        destination < take
            && take < replacement
            && replacement < restore
            && restore < drop_replacement
            && drop_replacement < drop_original
    );
    let fixture = source
        .split_once("pub(crate) fn fcmp_release_invalid_path_fixture_v1")
        .expect("invalid-path fixture")
        .1
        .split_once("enum RootValues")
        .expect("invalid-path fixture boundary")
        .0;
    assert_eq!(
        fixture
            .matches("replace_first_secret_coordinate_v1(values")
            .count(),
        2
    );
    assert_eq!(fixture.matches("::conditional_select(").count(), 2);
    assert_eq!(fixture.matches(".sub_ref(").count(), 2);
    assert_eq!(fixture.matches(".ct_is_zero()").count(), 2);
    for forbidden in ["if *value", "*value ==", "*value = if", "first_mut()?;"] {
        assert!(!fixture.contains(forbidden), "retained {forbidden}");
    }
}
#[test]
fn root_value_equality_owns_every_coordinate_difference_and_scans_full_shape() {
    let one_c1 = Field25519::ONE;
    let two_c1 = Field25519::ONE.add_ref(&Field25519::ONE);
    let three_c1 = two_c1.add_ref(&Field25519::ONE);
    let left_c1 = RootValues::C1(vec![one_c1, two_c1]);
    let equal_c1 = RootValues::C1(vec![one_c1, two_c1]);
    let unequal_c1 = RootValues::C1(vec![three_c1, two_c1]);
    reset_prover_secret_copy_owner_drops();
    assert!(bool::from(root_values_ct_eq(&left_c1, &equal_c1)));
    assert_eq!(prover_secret_copy_owner_drops(), 2);
    reset_prover_secret_copy_owner_drops();
    assert!(!bool::from(root_values_ct_eq(&left_c1, &unequal_c1)));
    assert_eq!(prover_secret_copy_owner_drops(), 2);
    let one_c2 = HelioseleneField::ONE;
    let two_c2 = HelioseleneField::ONE.add_ref(&HelioseleneField::ONE);
    let left_c2 = RootValues::C2(vec![one_c2, two_c2]);
    let equal_c2 = RootValues::C2(vec![one_c2, two_c2]);
    reset_prover_secret_copy_owner_drops();
    assert!(bool::from(root_values_ct_eq(&left_c2, &equal_c2)));
    assert_eq!(prover_secret_copy_owner_drops(), 2);
    reset_prover_secret_copy_owner_drops();
    assert!(!bool::from(root_values_ct_eq(&left_c1, &left_c2)));
    assert_eq!(prover_secret_copy_owner_drops(), 0);
    let short_c1 = RootValues::C1(vec![one_c1]);
    assert!(!bool::from(root_values_ct_eq(&left_c1, &short_c1)));
    assert_eq!(prover_secret_copy_owner_drops(), 0);
}
#[test]
fn root_value_equality_source_uses_only_borrowed_subtraction_and_owned_differences() {
    let source = include_str!("../prover.rs");
    let equality = source
        .split_once("fn root_values_ct_eq")
        .expect("root-value equality")
        .1
        .split_once("fn all_paths_share_root")
        .expect("root-value equality boundary")
        .0;
    assert_eq!(equality.matches("ct_equal_slices_by(").count(), 2);
    assert_eq!(
        equality
            .matches("ProverSecretCopyValueV1::new(left.sub_ref(right))")
            .count(),
        2
    );
    assert_eq!(
        equality
            .matches("difference.expose_ref().ct_is_zero()")
            .count(),
        2
    );
    for forbidden in ["*left - *right", "*left -", "- *right"] {
        assert!(!equality.contains(forbidden), "retained {forbidden}");
    }
}
#[test]
fn fixture_leaf_coordinate_buffer_zeroizes_on_drop_and_unwind() {
    PROVER_COPY_CLEARS.with(|calls| calls.set(0));
    {
        let mut values =
            zeroizing_exact_secret_buffer_v1::<TrackingCopy>(2).expect("exact secret buffer");
        assert!(values.capacity() >= 2);
        values.push(TrackingCopy(131));
        values.push(TrackingCopy(137));
    }
    assert_eq!(PROVER_COPY_CLEARS.with(Cell::get), 2);
    PROVER_COPY_CLEARS.with(|calls| calls.set(0));
    let unwind = std::panic::catch_unwind(|| {
        let mut values =
            zeroizing_exact_secret_buffer_v1::<TrackingCopy>(2).expect("exact unwind buffer");
        values.push(TrackingCopy(139));
        values.push(TrackingCopy(149));
        panic!("exercise exact secret-buffer unwind");
    });
    assert!(unwind.is_err());
    assert_eq!(PROVER_COPY_CLEARS.with(Cell::get), 2);
    assert!(matches!(
        zeroizing_exact_secret_buffer_v1::<TrackingCopy>(usize::MAX),
        Err(FcmpNativeErrorV1::ArithmeticInvariant)
    ));
}
#[test]
fn fixture_leaf_coordinate_source_keeps_exact_erasing_owners_through_hash() {
    let source = include_str!("../prover.rs");
    assert_source_contains_all(
        source,
        &[
            "fn zeroizing_exact_secret_buffer_v1<T: Zeroize>(",
            "fn prover_secret_leaf_coordinates_v1(",
            "#[cfg(any(test, feature = \"privacy-release-evidence\"))]\n\
         fn with_fcmp_fixture_leaf_coordinate_owners_v1<T>(",
        ],
    );
    assert_source_order(
        source_section(
            source,
            "fn zeroizing_exact_secret_buffer_v1<T: Zeroize>(",
            "fn push_secret_scalar_v1<F: ProofScalar + Zeroize>(",
        ),
        &[
            "try_reserve_exact(exact_capacity)",
            "values.capacity() < exact_capacity",
            "Ok(Zeroizing::new(values))",
        ],
    );
    let coordinate_scope = source_section(
        source,
        "fn prover_secret_leaf_coordinates_v1(",
        "fn with_fcmp_fixture_leaf_coordinate_owners_v1<T>(",
    );
    assert_source_contains_all(
        coordinate_scope,
        &[
            "mut convert: impl FnMut(&[u8; 32]) -> Result<(Field25519, Field25519), FcmpNativeErrorV1>",
        ],
    );
    assert_source_order(
        coordinate_scope,
        &[
            ".checked_mul(6)",
            "zeroizing_exact_secret_buffer_v1::<Field25519>(padded_capacity)?",
            "ProverSecretCopyValueV1::new(convert(point)?)",
            "coordinate_pair.expose_ref().0",
            "coordinate_pair.expose_ref().1",
            "leaf_coordinates.len() != populated_len",
            "leaf_coordinates.resize(padded_capacity, Field25519::ZERO)",
        ],
    );
    assert_source_counts(coordinate_scope, &[("push_secret_scalar_v1(", 2)]);
    assert_source_excludes_all(
        coordinate_scope,
        &[
            ".components()",
            "let (x, y) = edwards_to_wei25519",
            "ProverSecretCopyValueV1::new(convert(&point)?)",
            "Vec::with_capacity(6 * leaves.len())",
            "leaf_coordinates.extend([x, y])",
            "leaf_coordinates.push(",
        ],
    );
    let release_fixture = source_section(
        source,
        "pub(crate) fn fcmp_release_fixture_v1(",
        "/// Build a maximum-shape fixture whose first canonical branch",
    );
    assert_source_order(
        release_fixture,
        &[
            "with_fcmp_fixture_leaf_coordinate_owners_v1(",
            "secret_edwards_to_wei25519_v1",
            "prover_secret_hash_selene_v1",
        ],
    );
    assert_source_excludes_all(
        release_fixture,
        &[
            "&leaves, edwards_to_wei25519, hash_selene",
            "let mut leaf_coordinates = Vec::with_capacity",
            "leaf_coordinates.extend([x, y])",
        ],
    );
}
#[test]
fn hidden_output_identifier_push_is_preallocated_and_owned_on_success_and_error() {
    let output = output_from_multiples(13, 17, 19);
    let expected = output.output_id();
    super::super::FCMP_SECRET_OUTPUT_ID_DROPS_V1.with(|drops| drops.set(0));
    let mut identifiers = Zeroizing::new(Vec::with_capacity(1));
    let allocation_capacity = identifiers.capacity();
    let allocation_pointer = identifiers.as_ptr();
    require_preallocated_push(identifiers.len(), identifiers.capacity())
        .expect("public capacity preflight");
    push_owned_secret_output_id_v1(&mut identifiers, output.secret_output_id_v1())
        .expect("preallocated hidden output identifier");
    assert_eq!(identifiers.as_slice(), &[expected]);
    assert_eq!(identifiers.capacity(), allocation_capacity);
    assert_eq!(identifiers.as_ptr(), allocation_pointer);
    assert_eq!(
        super::super::FCMP_SECRET_OUTPUT_ID_DROPS_V1.with(Cell::get),
        1
    );

    super::super::FCMP_SECRET_OUTPUT_ID_DROPS_V1.with(|drops| drops.set(0));
    let mut no_capacity = Zeroizing::new(Vec::new());
    assert!(matches!(
        push_owned_secret_output_id_v1(&mut no_capacity, output.secret_output_id_v1()),
        Err(FcmpNativeErrorV1::ArithmeticInvariant)
    ));
    assert!(no_capacity.is_empty());
    assert_eq!(
        super::super::FCMP_SECRET_OUTPUT_ID_DROPS_V1.with(Cell::get),
        1
    );
}
#[test]
fn private_output_identifier_callsites_use_only_borrowed_owned_insertion() {
    let source = include_str!("../prover.rs");
    let production = source
        .split_once("#[cfg(test)]\n#[path = \"prover/tests.rs\"]\nmod tests")
        .expect("production prover boundary")
        .0;
    let push = source_section(
        source,
        "fn push_owned_secret_output_id_v1(",
        "fn zeroizing_exact_secret_buffer_v1<T: Zeroize>(",
    );
    assert_source_order(
        push,
        &[
            "value: FcmpSecretOutputIdV1",
            "require_preallocated_push(values.len(), values.capacity())?",
            "values.push(*value.as_ref())",
            "drop(value)",
        ],
    );
    assert_source_excludes_all(push, &["value: [u8; 32]", "value.expose_copy()"]);

    let constructor = source_section(
        source,
        "fn from_secret_byte_owners_v1(",
        "#[cfg(test)]\n    fn duplicate_for_test(&self)",
    );
    assert_source_contains_all(
        constructor,
        &[
            "require_preallocated_push(leaf_ids.len(), leaf_ids.capacity())?",
            "push_owned_secret_output_id_v1(&mut leaf_ids, leaf.secret_output_id_v1())?",
            "let output_id = output.secret_output_id_v1()",
            "ct_digest_slice_contains(&leaf_ids, output_id.as_ref())",
        ],
    );
    assert_source_excludes_all(
        constructor,
        &["leaf.output_id()", "output.output_id()", "leaf_ids.push("],
    );

    let prove_once = source_section(
        source,
        "fn prove_fcmp_plus_plus_once_v1(",
        "fn retry_membership_prover<T>(",
    );
    let preflight = source_section(
        source,
        "fn preflight_fcmp_plus_plus_v1(",
        "/// Prove a complete first-release FCMP++ statement in native Rust.",
    );
    for scope in [prove_once, preflight] {
        assert_source_contains_all(
            scope,
            &[
                "require_preallocated_push(spent_outputs.len(), spent_outputs.capacity())?",
                "push_owned_secret_output_id_v1(",
                "input.output.secret_output_id_v1()",
            ],
        );
        assert_source_excludes_all(scope, &["spent_outputs.push(", "input.output.output_id()"]);
    }
    assert_source_counts(production, &[("input.output.secret_output_id_v1()", 2)]);
    assert_source_counts(production, &[("input.output.output_id()", 0)]);
    assert_source_counts(production, &[(".secret_output_id_v1()", 4)]);
    assert_source_counts(
        production,
        &[
            ("new_output_ids.push(output.output_id())", 2),
            (".output_id()", 2),
        ],
    );
}
#[test]
fn duplicate_key_image_precheck_owners_cover_success_decode_error_capacity_and_unwind() {
    let (input, _output, _root) = one_layer_fixture();
    let expected = input
        .public_input()
        .expect("canonical public input")
        .key_image;
    let linking_bytes = input.output.component_refs_v1().1;

    reset_prover_secret_copy_owner_drops();
    let key_image = prover_secret_key_image_id_v1(linking_bytes, &input.spend_x)
        .expect("owned duplicate-precheck key image");
    assert_eq!(key_image.expose_ref(), &expected);
    assert_eq!(prover_secret_copy_owner_drops(), 4);
    let mut identifiers = Zeroizing::new(Vec::with_capacity(1));
    let allocation_capacity = identifiers.capacity();
    let allocation_pointer = identifiers.as_ptr();
    push_owned_prover_secret_digest_v1(&mut identifiers, key_image)
        .expect("preallocated key-image insertion");
    assert_eq!(identifiers.as_slice(), &[expected]);
    assert_eq!(identifiers.capacity(), allocation_capacity);
    assert_eq!(identifiers.as_ptr(), allocation_pointer);
    assert_eq!(prover_secret_copy_owner_drops(), 5);

    reset_prover_secret_copy_owner_drops();
    let key_image = prover_secret_key_image_id_v1(linking_bytes, &input.spend_x)
        .expect("owned capacity-error key image");
    assert_eq!(prover_secret_copy_owner_drops(), 4);
    let mut no_capacity = Zeroizing::new(Vec::new());
    assert!(matches!(
        push_owned_prover_secret_digest_v1(&mut no_capacity, key_image),
        Err(FcmpNativeErrorV1::ArithmeticInvariant)
    ));
    assert!(no_capacity.is_empty());
    assert_eq!(prover_secret_copy_owner_drops(), 5);

    reset_prover_secret_copy_owner_drops();
    let small_order = curve25519_dalek::constants::EIGHT_TORSION[1]
        .compress()
        .to_bytes();
    assert!(matches!(
        prover_secret_key_image_id_v1(&small_order, &input.spend_x),
        Err(FcmpNativeErrorV1::EdwardsPointEncoding)
    ));
    assert_eq!(prover_secret_copy_owner_drops(), 3);

    reset_prover_secret_copy_owner_drops();
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let key_image = prover_secret_key_image_id_v1(linking_bytes, &input.spend_x)
            .expect("owned key image before unwind");
        assert_eq!(key_image.expose_ref(), &expected);
        assert_eq!(prover_secret_copy_owner_drops(), 4);
        let _ = core::hint::black_box(&key_image);
        panic!("exercise duplicate-key-image owner unwind");
    }));
    assert!(unwind.is_err());
    assert_eq!(prover_secret_copy_owner_drops(), 5);

    let duplicate = Zeroizing::new(vec![expected, expected]);
    assert!(ct_has_duplicate_digests(&duplicate));
}
#[test]
fn duplicate_key_image_precheck_source_is_borrowed_owned_and_constant_time() {
    let source = include_str!("../prover.rs");
    let production = source
        .split_once("#[cfg(test)]\n#[path = \"prover/tests.rs\"]\nmod tests")
        .expect("production prover boundary")
        .0;
    let owner_push = source_section(
        production,
        "fn push_owned_prover_secret_digest_v1(",
        "fn zeroizing_exact_secret_buffer_v1<T: Zeroize>(",
    );
    assert_source_order(
        owner_push,
        &[
            "value: ProverSecretCopyValueV1<[u8; 32]>",
            "require_preallocated_push(values.len(), values.capacity())?",
            "values.push(*value.expose_ref())",
            "drop(value)",
        ],
    );
    assert_source_excludes_all(
        owner_push,
        &["value: [u8; 32]", "value.expose_copy()", "values.reserve("],
    );

    let derivation = source_section(
        production,
        "fn prover_secret_key_image_id_v1(",
        "fn secret_edwards_product_v1(",
    );
    assert_source_contains_all(
        derivation,
        &[
            "linking_bytes: &[u8; 32]",
            "spend_x: &Scalar",
            "prover_secret_decode_edwards_point_v1(linking_bytes)?",
            "secret_edwards_product_v1(linking.expose_ref(), spend_x)",
            "Ok(prover_secret_edwards_encoding_v1(&key_image))",
        ],
    );
    assert_source_excludes_all(
        derivation,
        &[
            "decode_edwards_point(",
            "linking *",
            ".compress().to_bytes()",
            "spend_x: Scalar",
            "-> Result<[u8; 32]",
            "pub fn",
        ],
    );

    let prove_once = source_section(
        production,
        "fn prove_fcmp_plus_plus_once_v1(",
        "fn retry_membership_prover<T>(",
    );
    assert_source_order(
        prove_once,
        &[
            "push_owned_secret_output_id_v1(&mut spent_outputs",
            "require_preallocated_push(derived_key_images.len(), derived_key_images.capacity())?",
            "input.output.component_refs_v1().1",
            "prover_secret_key_image_id_v1(linking_bytes, &input.spend_x)?",
            "push_owned_prover_secret_digest_v1(&mut derived_key_images, key_image)?",
            "ct_has_duplicate_digests(&spent_outputs)",
            "ct_has_duplicate_digests(&derived_key_images)",
        ],
    );
    assert_source_excludes_all(
        prove_once,
        &[
            "input.output.components().1",
            "decode_edwards_point(input.output",
            "(linking * input.spend_x).compress().to_bytes()",
            "derived_key_images.push(",
            "derived_key_images.sort",
            "HashSet",
        ],
    );
    assert!(!production.contains(
        "#[cfg(any(test, feature = \"privacy-release-evidence\"))]\n\
         fn prover_secret_key_image_id_v1("
    ));
}
#[test]
fn proof_input_coordinate_owners_cover_success_decode_error_downstream_error_and_unwind() {
    let output = output_from_multiples(43, 47, 53);
    let (output_bytes, linking_bytes, commitment_bytes) = output.component_refs_v1();
    let expected_output = edwards_to_wei25519(*output_bytes).expect("public output coordinates");
    let expected_linking = edwards_to_wei25519(*linking_bytes).expect("public linking coordinates");
    let expected_commitment =
        edwards_to_wei25519(*commitment_bytes).expect("public commitment coordinates");

    reset_prover_secret_copy_owner_drops();
    {
        let coordinates = prover_secret_output_coordinate_owners_v1(
            output_bytes,
            linking_bytes,
            commitment_bytes,
        )
        .expect("owned proof-input coordinates");
        assert_eq!(
            coordinates.output.padding.expose_ref().as_slice(),
            &[expected_output.0, expected_output.1]
        );
        assert_eq!(
            coordinates.linking.padding.expose_ref().as_slice(),
            &[expected_linking.0, expected_linking.1]
        );
        assert_eq!(
            coordinates.commitment.padding.expose_ref().as_slice(),
            &[expected_commitment.0, expected_commitment.1]
        );
        assert_eq!(prover_secret_copy_owner_drops(), 0);
    }
    assert_eq!(prover_secret_copy_owner_drops(), 6);

    let torsion = curve25519_dalek::constants::EIGHT_TORSION[1]
        .compress()
        .to_bytes();
    reset_prover_secret_copy_owner_drops();
    assert!(matches!(
        prover_secret_output_coordinate_owners_v1(output_bytes, &torsion, commitment_bytes),
        Err(FcmpNativeErrorV1::EdwardsPointEncoding)
    ));
    assert_eq!(prover_secret_copy_owner_drops(), 2);

    reset_prover_secret_copy_owner_drops();
    let downstream_error = (|| -> Result<(), FcmpNativeErrorV1> {
        let coordinates = prover_secret_output_coordinate_owners_v1(
            output_bytes,
            linking_bytes,
            commitment_bytes,
        )?;
        assert_eq!(prover_secret_copy_owner_drops(), 0);
        let _ = core::hint::black_box(coordinates.output.padding.expose_ref());
        let mut tape = ProverVectorCommitmentTape::<Field25519>::new(128)?;
        tape.append_branch(&[])?;
        Ok(())
    })();
    assert_eq!(
        downstream_error,
        Err(FcmpNativeErrorV1::ArithmeticInvariant)
    );
    assert_eq!(prover_secret_copy_owner_drops(), 6);

    reset_prover_secret_copy_owner_drops();
    let unwind = std::panic::catch_unwind(|| {
        let coordinates = prover_secret_output_coordinate_owners_v1(
            output_bytes,
            linking_bytes,
            commitment_bytes,
        )
        .expect("owned proof-input coordinates before unwind");
        assert_eq!(prover_secret_copy_owner_drops(), 0);
        let _ = core::hint::black_box(&coordinates);
        panic!("exercise proof-input coordinate owner unwind");
    });
    assert!(unwind.is_err());
    assert_eq!(prover_secret_copy_owner_drops(), 6);
}
#[test]
fn input_blind_v_padding_owner_covers_success_downstream_error_and_unwind() {
    let scalar = Scalar::from(59_u64);
    let input_blind_v =
        prepare_ed_blind(generator_v(), &scalar, true).expect("prepared input V blind");
    let input_blind_blind =
        prepare_ed_blind(generator_t(), &scalar, false).expect("prepared input blind blind");
    let expected = [input_blind_v.coordinates.0, input_blind_v.coordinates.1];

    reset_prover_secret_copy_owner_drops();
    {
        let padding = ProverSecretCopyValueV1::new([
            input_blind_v.coordinates.0,
            input_blind_v.coordinates.1,
        ]);
        assert_eq!(padding.expose_ref(), &expected);
        assert_eq!(prover_secret_copy_owner_drops(), 0);
        let mut tape = ProverVectorCommitmentTape::<Field25519>::new(128)
            .expect("one exact prover commitment tape");
        let (_, variables) = tape
            .append_claimed_point(
                ED25519_DLOG_PARAMETERS,
                &input_blind_blind.decomposition,
                &input_blind_blind.divisor,
                &input_blind_blind.coordinates,
                padding.expose_ref().as_slice(),
            )
            .expect("owned input V padding insertion");
        assert_eq!(variables.len(), 2);
        assert_eq!(prover_secret_copy_owner_drops(), 0);
    }
    assert_eq!(prover_secret_copy_owner_drops(), 1);

    reset_prover_secret_copy_owner_drops();
    let downstream_error = (|| -> Result<(), FcmpNativeErrorV1> {
        let padding = ProverSecretCopyValueV1::new([
            input_blind_v.coordinates.0,
            input_blind_v.coordinates.1,
        ]);
        assert_eq!(padding.expose_ref(), &expected);
        let mut tape = ProverVectorCommitmentTape::<Field25519>::new(128)?;
        tape.append_claimed_point(
            ED25519_DLOG_PARAMETERS,
            &[],
            &input_blind_blind.divisor,
            &input_blind_blind.coordinates,
            padding.expose_ref().as_slice(),
        )?;
        Ok(())
    })();
    assert_eq!(
        downstream_error,
        Err(FcmpNativeErrorV1::ArithmeticInvariant)
    );
    assert_eq!(prover_secret_copy_owner_drops(), 1);

    reset_prover_secret_copy_owner_drops();
    let unwind = std::panic::catch_unwind(|| {
        let padding = ProverSecretCopyValueV1::new([
            input_blind_v.coordinates.0,
            input_blind_v.coordinates.1,
        ]);
        assert_eq!(padding.expose_ref(), &expected);
        let _ = core::hint::black_box(padding.expose_ref());
        panic!("exercise input V padding owner unwind");
    });
    assert!(unwind.is_err());
    assert_eq!(prover_secret_copy_owner_drops(), 1);
}
#[test]
fn sal_y_sum_owner_covers_success_constructor_error_and_unwind() {
    let output_y = Scalar::from(29_u64);
    let rerandomization_output = Scalar::from(31_u64);
    let expected = Scalar::from(60_u64);
    let canonical = Scalar::from(37_u64).to_bytes();

    reset_prover_secret_scalar_owner_drops();
    reset_prover_secret_copy_owner_drops();
    {
        let sal_y = prover_secret_edwards_scalar_sum_v1(&output_y, &rerandomization_output);
        assert_eq!(sal_y.expose_ref(), &expected);
        let sal_y_bytes = ProverSecretCopyValueV1::new(sal_y.expose_ref().to_bytes());
        let witness =
            FcmpSalWitnessV1::new(canonical, sal_y_bytes.expose_copy(), canonical, canonical)
                .expect("owned SAL y sum witness");
        let _ = core::hint::black_box(&witness);
        assert_eq!(prover_secret_scalar_owner_drops(), 0);
        assert_eq!(prover_secret_copy_owner_drops(), 0);
    }
    assert_eq!(prover_secret_scalar_owner_drops(), 0);
    assert_eq!(prover_secret_copy_owner_drops(), 2);

    reset_prover_secret_scalar_owner_drops();
    reset_prover_secret_copy_owner_drops();
    let downstream_error = (|| -> Result<(), FcmpNativeErrorV1> {
        let sal_y = prover_secret_edwards_scalar_sum_v1(&output_y, &rerandomization_output);
        let sal_y_bytes = ProverSecretCopyValueV1::new(sal_y.expose_ref().to_bytes());
        FcmpSalWitnessV1::new(
            [u8::MAX; 32],
            sal_y_bytes.expose_copy(),
            canonical,
            canonical,
        )?;
        Ok(())
    })();
    assert_eq!(downstream_error, Err(FcmpNativeErrorV1::ScalarEncoding));
    assert_eq!(prover_secret_scalar_owner_drops(), 0);
    assert_eq!(prover_secret_copy_owner_drops(), 2);

    reset_prover_secret_scalar_owner_drops();
    reset_prover_secret_copy_owner_drops();
    let unwind = std::panic::catch_unwind(|| {
        let sal_y = prover_secret_edwards_scalar_sum_v1(&output_y, &rerandomization_output);
        let sal_y_bytes = ProverSecretCopyValueV1::new(sal_y.expose_ref().to_bytes());
        let witness =
            FcmpSalWitnessV1::new(canonical, sal_y_bytes.expose_copy(), canonical, canonical)
                .expect("owned SAL y sum witness before unwind");
        assert_eq!(prover_secret_scalar_owner_drops(), 0);
        assert_eq!(prover_secret_copy_owner_drops(), 0);
        let _ = core::hint::black_box((sal_y.expose_ref(), sal_y_bytes.expose_ref(), &witness));
        panic!("exercise SAL y sum owner unwind");
    });
    assert!(unwind.is_err());
    assert_eq!(prover_secret_scalar_owner_drops(), 0);
    assert_eq!(prover_secret_copy_owner_drops(), 2);
}
#[test]
fn sal_linking_bytes_owner_covers_success_constructor_error_and_unwind() {
    let linking = Scalar::from(41_u64);
    let canonical = Scalar::from(43_u64).to_bytes();

    reset_prover_secret_copy_owner_drops();
    {
        let sal_linking_bytes = ProverSecretCopyValueV1::new(linking.to_bytes());
        let witness = FcmpSalWitnessV1::new(
            canonical,
            canonical,
            sal_linking_bytes.expose_copy(),
            canonical,
        )
        .expect("owned SAL linking witness");
        let _ = core::hint::black_box((sal_linking_bytes.expose_ref(), &witness));
        assert_eq!(prover_secret_copy_owner_drops(), 0);
    }
    assert_eq!(prover_secret_copy_owner_drops(), 1);

    reset_prover_secret_copy_owner_drops();
    let downstream_error = (|| -> Result<(), FcmpNativeErrorV1> {
        let sal_linking_bytes = ProverSecretCopyValueV1::new(linking.to_bytes());
        FcmpSalWitnessV1::new(
            canonical,
            canonical,
            sal_linking_bytes.expose_copy(),
            [u8::MAX; 32],
        )?;
        Ok(())
    })();
    assert_eq!(downstream_error, Err(FcmpNativeErrorV1::ScalarEncoding));
    assert_eq!(prover_secret_copy_owner_drops(), 1);

    reset_prover_secret_copy_owner_drops();
    let unwind = std::panic::catch_unwind(|| {
        let sal_linking_bytes = ProverSecretCopyValueV1::new(linking.to_bytes());
        let witness = FcmpSalWitnessV1::new(
            canonical,
            canonical,
            sal_linking_bytes.expose_copy(),
            canonical,
        )
        .expect("owned SAL linking witness before unwind");
        assert_eq!(prover_secret_copy_owner_drops(), 0);
        let _ = core::hint::black_box((sal_linking_bytes.expose_ref(), &witness));
        panic!("exercise SAL linking bytes owner unwind");
    });
    assert!(unwind.is_err());
    assert_eq!(prover_secret_copy_owner_drops(), 1);
}
#[test]
fn sal_linking_bytes_source_owns_encoding_through_constructor() {
    let source = include_str!("../prover.rs");
    let production = source
        .split_once("#[cfg(test)]\n#[path = \"prover/tests.rs\"]\nmod tests")
        .expect("production prover boundary")
        .0;
    let prove_once = source_section(
        production,
        "fn prove_fcmp_plus_plus_once_v1(",
        "fn retry_membership_prover<T>(",
    );
    assert_source_order(
        prove_once,
        &[
            "let sal_y_bytes = ProverSecretCopyValueV1::new(sal_y.expose_ref().to_bytes())",
            "let sal_linking_bytes =",
            "ProverSecretCopyValueV1::new(rerandomization.linking.to_bytes())",
            "input.spend_x.to_bytes()",
            "ProverSecretCopyValueV1::new(rerandomization.rerandomization_blind.to_bytes())",
            "let sal_witness = FcmpSalWitnessV1::new(",
            "sal_y_bytes.expose_copy()",
            "sal_linking_bytes.expose_copy()",
            "sal_rerandomization_blind_bytes.expose_copy()",
            "let sal = prove_fcmp_sal_with_checked_rng_v1(",
        ],
    );
    assert_source_counts(
        prove_once,
        &[
            (
                "ProverSecretCopyValueV1::new(rerandomization.linking.to_bytes())",
                1,
            ),
            ("sal_linking_bytes.expose_copy()", 1),
            ("input.spend_x.to_bytes()", 1),
            ("rerandomization.rerandomization_blind.to_bytes()", 1),
        ],
    );
    assert_source_excludes_all(
        prove_once,
        &[
            "            rerandomization.linking.to_bytes(),",
            "let sal_linking_bytes = Zeroizing::new(",
            "drop(sal_linking_bytes)",
            "mem::take(&mut sal_linking_bytes)",
        ],
    );

    let sal_source = include_str!("../sal.rs");
    let constructor = source_section(
        sal_source,
        "impl FcmpSalWitnessV1 {",
        "#[allow(clippy::too_many_arguments)]",
    );
    assert_source_order(
        constructor,
        &[
            "mut r_i: [u8; 32]",
            "let r_i_bytes = SalSecretCopyValueV1::take(&mut r_i)",
            "let x = secret_scalar_from_bytes_v1(x_bytes.expose_ref())?",
            "let r_i = secret_scalar_from_bytes_v1(r_i_bytes.expose_ref())?",
        ],
    );
}
#[test]
fn sal_spend_x_bytes_owner_covers_success_constructor_error_and_unwind() {
    let spend_x = Scalar::from(47_u64);
    let canonical = Scalar::from(53_u64).to_bytes();

    reset_prover_secret_copy_owner_drops();
    {
        let sal_spend_x_bytes = ProverSecretCopyValueV1::new(spend_x.to_bytes());
        let witness = FcmpSalWitnessV1::new(
            sal_spend_x_bytes.expose_copy(),
            canonical,
            canonical,
            canonical,
        )
        .expect("owned SAL spend x witness");
        let _ = core::hint::black_box((sal_spend_x_bytes.expose_ref(), &witness));
        assert_eq!(prover_secret_copy_owner_drops(), 0);
    }
    assert_eq!(prover_secret_copy_owner_drops(), 1);

    reset_prover_secret_copy_owner_drops();
    let downstream_error = (|| -> Result<(), FcmpNativeErrorV1> {
        let sal_spend_x_bytes = ProverSecretCopyValueV1::new(spend_x.to_bytes());
        FcmpSalWitnessV1::new(
            sal_spend_x_bytes.expose_copy(),
            canonical,
            canonical,
            [u8::MAX; 32],
        )?;
        Ok(())
    })();
    assert_eq!(downstream_error, Err(FcmpNativeErrorV1::ScalarEncoding));
    assert_eq!(prover_secret_copy_owner_drops(), 1);

    reset_prover_secret_copy_owner_drops();
    let unwind = std::panic::catch_unwind(|| {
        let sal_spend_x_bytes = ProverSecretCopyValueV1::new(spend_x.to_bytes());
        let witness = FcmpSalWitnessV1::new(
            sal_spend_x_bytes.expose_copy(),
            canonical,
            canonical,
            canonical,
        )
        .expect("owned SAL spend x witness before unwind");
        assert_eq!(prover_secret_copy_owner_drops(), 0);
        let _ = core::hint::black_box((sal_spend_x_bytes.expose_ref(), &witness));
        panic!("exercise SAL spend x bytes owner unwind");
    });
    assert!(unwind.is_err());
    assert_eq!(prover_secret_copy_owner_drops(), 1);
}
#[test]
fn sal_spend_x_bytes_source_owns_encoding_through_constructor() {
    let source = include_str!("../prover.rs");
    let production = source
        .split_once("#[cfg(test)]\n#[path = \"prover/tests.rs\"]\nmod tests")
        .expect("production prover boundary")
        .0;
    let prove_once = source_section(
        production,
        "fn prove_fcmp_plus_plus_once_v1(",
        "fn retry_membership_prover<T>(",
    );
    assert_source_order(
        prove_once,
        &[
            "let sal_y_bytes = ProverSecretCopyValueV1::new(sal_y.expose_ref().to_bytes())",
            "ProverSecretCopyValueV1::new(rerandomization.linking.to_bytes())",
            "let sal_spend_x_bytes =",
            "ProverSecretCopyValueV1::new(input.spend_x.to_bytes())",
            "ProverSecretCopyValueV1::new(rerandomization.rerandomization_blind.to_bytes())",
            "let sal_witness = FcmpSalWitnessV1::new(",
            "sal_spend_x_bytes.expose_copy()",
            "sal_y_bytes.expose_copy()",
            "sal_linking_bytes.expose_copy()",
            "sal_rerandomization_blind_bytes.expose_copy()",
            "let sal = prove_fcmp_sal_with_checked_rng_v1(",
        ],
    );
    assert_source_counts(
        prove_once,
        &[
            ("ProverSecretCopyValueV1::new(input.spend_x.to_bytes())", 1),
            ("sal_spend_x_bytes.expose_copy()", 1),
            ("input.spend_x.to_bytes()", 1),
            ("rerandomization.rerandomization_blind.to_bytes()", 1),
        ],
    );
    assert_source_excludes_all(
        prove_once,
        &[
            "            input.spend_x.to_bytes(),",
            "let sal_spend_x_bytes = Zeroizing::new(",
            "drop(sal_spend_x_bytes)",
            "mem::take(&mut sal_spend_x_bytes)",
        ],
    );

    let sal_source = include_str!("../sal.rs");
    let constructor = source_section(
        sal_source,
        "impl FcmpSalWitnessV1 {",
        "#[allow(clippy::too_many_arguments)]",
    );
    assert_source_order(
        constructor,
        &[
            "mut x: [u8; 32]",
            "let x_bytes = SalSecretCopyValueV1::take(&mut x)",
            "let y_bytes = SalSecretCopyValueV1::take(&mut y)",
            "let r_i_bytes = SalSecretCopyValueV1::take(&mut r_i)",
            "let r_r_i_bytes = SalSecretCopyValueV1::take(&mut r_r_i)",
            "let x = secret_scalar_from_bytes_v1(x_bytes.expose_ref())?",
        ],
    );
}
#[test]
fn sal_rerandomization_blind_bytes_owner_covers_success_constructor_error_and_unwind() {
    let rerandomization_blind = Scalar::from(59_u64);
    let canonical = Scalar::from(61_u64).to_bytes();

    reset_prover_secret_copy_owner_drops();
    {
        let sal_rerandomization_blind_bytes =
            ProverSecretCopyValueV1::new(rerandomization_blind.to_bytes());
        let witness = FcmpSalWitnessV1::new(
            canonical,
            canonical,
            canonical,
            sal_rerandomization_blind_bytes.expose_copy(),
        )
        .expect("owned SAL rerandomization blind witness");
        let _ = core::hint::black_box((sal_rerandomization_blind_bytes.expose_ref(), &witness));
        assert_eq!(prover_secret_copy_owner_drops(), 0);
    }
    assert_eq!(prover_secret_copy_owner_drops(), 1);

    reset_prover_secret_copy_owner_drops();
    let downstream_error = (|| -> Result<(), FcmpNativeErrorV1> {
        let sal_rerandomization_blind_bytes =
            ProverSecretCopyValueV1::new(rerandomization_blind.to_bytes());
        FcmpSalWitnessV1::new(
            [u8::MAX; 32],
            canonical,
            canonical,
            sal_rerandomization_blind_bytes.expose_copy(),
        )?;
        Ok(())
    })();
    assert_eq!(downstream_error, Err(FcmpNativeErrorV1::ScalarEncoding));
    assert_eq!(prover_secret_copy_owner_drops(), 1);

    reset_prover_secret_copy_owner_drops();
    let unwind = std::panic::catch_unwind(|| {
        let sal_rerandomization_blind_bytes =
            ProverSecretCopyValueV1::new(rerandomization_blind.to_bytes());
        let witness = FcmpSalWitnessV1::new(
            canonical,
            canonical,
            canonical,
            sal_rerandomization_blind_bytes.expose_copy(),
        )
        .expect("owned SAL rerandomization blind witness before unwind");
        assert_eq!(prover_secret_copy_owner_drops(), 0);
        let _ = core::hint::black_box((sal_rerandomization_blind_bytes.expose_ref(), &witness));
        panic!("exercise SAL rerandomization blind bytes owner unwind");
    });
    assert!(unwind.is_err());
    assert_eq!(prover_secret_copy_owner_drops(), 1);
}
#[test]
fn sal_rerandomization_blind_bytes_source_owns_encoding_through_constructor() {
    let source = include_str!("../prover.rs");
    let production = source
        .split_once("#[cfg(test)]\n#[path = \"prover/tests.rs\"]\nmod tests")
        .expect("production prover boundary")
        .0;
    let prove_once = source_section(
        production,
        "fn prove_fcmp_plus_plus_once_v1(",
        "fn retry_membership_prover<T>(",
    );
    assert_source_order(
        prove_once,
        &[
            "let sal_y_bytes = ProverSecretCopyValueV1::new(sal_y.expose_ref().to_bytes())",
            "ProverSecretCopyValueV1::new(rerandomization.linking.to_bytes())",
            "ProverSecretCopyValueV1::new(input.spend_x.to_bytes())",
            "let sal_rerandomization_blind_bytes =",
            "ProverSecretCopyValueV1::new(rerandomization.rerandomization_blind.to_bytes())",
            "let sal_witness = FcmpSalWitnessV1::new(",
            "sal_spend_x_bytes.expose_copy()",
            "sal_y_bytes.expose_copy()",
            "sal_linking_bytes.expose_copy()",
            "sal_rerandomization_blind_bytes.expose_copy()",
            "let sal = prove_fcmp_sal_with_checked_rng_v1(",
        ],
    );
    assert_source_counts(
        prove_once,
        &[
            (
                "ProverSecretCopyValueV1::new(rerandomization.rerandomization_blind.to_bytes())",
                1,
            ),
            ("sal_rerandomization_blind_bytes.expose_copy()", 1),
            ("rerandomization.rerandomization_blind.to_bytes()", 1),
        ],
    );
    assert_source_excludes_all(
        prove_once,
        &[
            "            rerandomization.rerandomization_blind.to_bytes(),",
            "let sal_rerandomization_blind_bytes = Zeroizing::new(",
            "drop(sal_rerandomization_blind_bytes)",
            "mem::take(&mut sal_rerandomization_blind_bytes)",
        ],
    );

    let sal_source = include_str!("../sal.rs");
    let constructor = source_section(
        sal_source,
        "impl FcmpSalWitnessV1 {",
        "#[allow(clippy::too_many_arguments)]",
    );
    assert_source_order(
        constructor,
        &[
            "mut r_r_i: [u8; 32]",
            "let x_bytes = SalSecretCopyValueV1::take(&mut x)",
            "let y_bytes = SalSecretCopyValueV1::take(&mut y)",
            "let r_i_bytes = SalSecretCopyValueV1::take(&mut r_i)",
            "let r_r_i_bytes = SalSecretCopyValueV1::take(&mut r_r_i)",
            "let x = secret_scalar_from_bytes_v1(x_bytes.expose_ref())?",
            "let r_r_i = secret_scalar_from_bytes_v1(r_r_i_bytes.expose_ref())?",
        ],
    );
}
#[test]
fn sal_y_sum_source_borrows_operands_and_retains_owners_through_constructor() {
    let source = include_str!("../prover.rs");
    let production = source
        .split_once("#[cfg(test)]\n#[path = \"prover/tests.rs\"]\nmod tests")
        .expect("production prover boundary")
        .0;
    let sum = source_section(
        production,
        "fn prover_secret_edwards_scalar_sum_v1(",
        "struct ProverSecretPointV1<P: ProofPoint>",
    );
    assert_source_contains_all(
        sum,
        &[
            "left: &Scalar",
            "right: &Scalar",
            "let mut sum = left + right",
            "ProverSecretCopyValueV1::take(&mut sum)",
        ],
    );
    assert_source_excludes_all(
        sum,
        &[
            "*left",
            "*right",
            ".clone()",
            ".to_owned()",
            "Zeroizing::new(",
        ],
    );

    let prove_once = source_section(
        production,
        "fn prove_fcmp_plus_plus_once_v1(",
        "fn retry_membership_prover<T>(",
    );
    assert_source_order(
        prove_once,
        &[
            "let sal_y =",
            "prover_secret_edwards_scalar_sum_v1(&input.output_y, &rerandomization.output)",
            "let sal_y_bytes = ProverSecretCopyValueV1::new(sal_y.expose_ref().to_bytes())",
            "let sal_linking_bytes =",
            "ProverSecretCopyValueV1::new(rerandomization.linking.to_bytes())",
            "let sal_rerandomization_blind_bytes =",
            "let sal_witness = FcmpSalWitnessV1::new(",
            "sal_y_bytes.expose_copy()",
            "sal_linking_bytes.expose_copy()",
            "sal_rerandomization_blind_bytes.expose_copy()",
            "let sal = prove_fcmp_sal_with_checked_rng_v1(",
            "prepared_inputs.push(PreparedInput {",
        ],
    );
    assert_source_counts(
        prove_once,
        &[
            ("prover_secret_edwards_scalar_sum_v1(", 1),
            ("let sal_y_bytes = ProverSecretCopyValueV1::new(", 1),
            ("sal_y_bytes.expose_copy()", 1),
        ],
    );
    assert_source_excludes_all(
        prove_once,
        &[
            "let sal_y = Zeroizing::new(",
            "input.output_y + rerandomization.output",
            "sal_y.to_bytes()",
            "drop(sal_y)",
            "drop(sal_y_bytes)",
            "mem::take(&mut sal_y)",
            "mem::take(&mut sal_y_bytes)",
        ],
    );

    let sal_source = include_str!("../sal.rs");
    let constructor = source_section(
        sal_source,
        "impl FcmpSalWitnessV1 {",
        "#[allow(clippy::too_many_arguments)]",
    );
    assert_source_order(
        constructor,
        &[
            "mut y: [u8; 32]",
            "let x_bytes = SalSecretCopyValueV1::take(&mut x)",
            "let y_bytes = SalSecretCopyValueV1::take(&mut y)",
            "let r_i_bytes = SalSecretCopyValueV1::take(&mut r_i)",
            "let r_r_i_bytes = SalSecretCopyValueV1::take(&mut r_r_i)",
            "let x = secret_scalar_from_bytes_v1(x_bytes.expose_ref())?",
        ],
    );
}
#[test]
fn proof_input_coordinate_source_is_borrowed_owned_and_production_visible() {
    let source = include_str!("../prover.rs");
    let production = source
        .split_once("#[cfg(test)]\n#[path = \"prover/tests.rs\"]\nmod tests")
        .expect("production prover boundary")
        .0;
    let owners = source_section(
        production,
        "struct ProverSecretEdwardsCoordinateOwnerV1 {",
        "fn secret_edwards_product_v1(",
    );
    assert_source_contains_all(
        owners,
        &[
            "_coordinates: ProverSecretCopyValueV1<(Field25519, Field25519)>",
            "padding: ProverSecretCopyValueV1<[Field25519; 2]>",
            "bytes: &[u8; 32]",
            "ProverSecretCopyValueV1::new(secret_edwards_to_wei25519_v1(bytes)?)",
            "coordinates.expose_ref().0",
            "coordinates.expose_ref().1",
            "output_bytes: &[u8; 32]",
            "linking_bytes: &[u8; 32]",
            "commitment_bytes: &[u8; 32]",
            "output: prover_secret_edwards_coordinate_owner_v1(output_bytes)?",
            "linking: prover_secret_edwards_coordinate_owner_v1(linking_bytes)?",
            "commitment: prover_secret_edwards_coordinate_owner_v1(commitment_bytes)?",
        ],
    );
    assert_source_counts(
        owners,
        &[
            ("ProverSecretCopyValueV1<(Field25519, Field25519)>", 1),
            ("ProverSecretCopyValueV1<[Field25519; 2]>", 1),
            ("prover_secret_edwards_coordinate_owner_v1(", 4),
        ],
    );
    assert_source_excludes_all(
        owners,
        &[
            "edwards_to_wei25519(*bytes)",
            "bytes: [u8; 32]",
            "#[cfg(",
            "pub struct ProverSecret",
            "pub(super) struct ProverSecret",
            "pub(crate) struct ProverSecret",
            "pub fn prover_secret",
            "pub(super) fn prover_secret",
            "pub(crate) fn prover_secret",
            "#[derive(Clone",
            "impl Clone for ProverSecret",
            "impl Copy for ProverSecret",
        ],
    );

    let prove_once = source_section(
        production,
        "fn prove_fcmp_plus_plus_once_v1(",
        "fn retry_membership_prover<T>(",
    );
    assert_source_order(
        prove_once,
        &[
            "let (output_bytes, linking_bytes, commitment_bytes) = input.output.component_refs_v1()",
            "let public = input.public_input()?",
            "prepare_ed_blind(generator_t(), &rerandomization.output, true)?",
            "prepare_ed_blind(generator_u(), &rerandomization.linking, true)?",
            "prepare_ed_blind(generator_v(), &rerandomization.linking, true)?",
            "prepare_ed_blind(generator_t(), &rerandomization.rerandomization_blind, false)?",
            "prepare_ed_blind(ED25519_BASEPOINT_POINT, &rerandomization.commitment, true)?",
            "prover_secret_output_coordinate_owners_v1(",
            "let input_blind_v_padding = ProverSecretCopyValueV1::new([",
            "let (output_blind_claim, output_variables) = c1_tape.append_claimed_point(",
            "output_coordinates.output.padding.expose_ref().as_slice()",
            "output_coordinates.linking.padding.expose_ref().as_slice()",
            "let (input_blind_v_divisor, _) = c1_tape.append_divisor(",
            "let (input_blind_blind_claim, input_blind_v_variables) = c1_tape.append_claimed_point(",
            "input_blind_v_padding.expose_ref().as_slice()",
            "output_coordinates\n                .commitment\n                .padding",
            "prover_secret_edwards_scalar_sum_v1(&input.output_y, &rerandomization.output)",
            "let sal_y_bytes = ProverSecretCopyValueV1::new(sal_y.expose_ref().to_bytes())",
            "ProverSecretCopyValueV1::new(rerandomization.linking.to_bytes())",
            "let sal_witness = FcmpSalWitnessV1::new(",
            "sal_y_bytes.expose_copy()",
            "sal_linking_bytes.expose_copy()",
            "prepared_inputs.push(PreparedInput {",
        ],
    );
    assert_source_counts(
        prove_once,
        &[
            (
                "let (output_bytes, linking_bytes, commitment_bytes) = input.output.component_refs_v1()",
                1,
            ),
            ("prover_secret_output_coordinate_owners_v1(", 1),
        ],
    );
    assert_source_counts(production, &[("input.public_input()?", 2)]);
    assert_source_excludes_all(
        prove_once,
        &[
            "input.output.components()",
            "edwards_to_wei25519(output_bytes)",
            "edwards_to_wei25519(linking_bytes)",
            "edwards_to_wei25519(commitment_bytes)",
            "let output_padding = Zeroizing::new(",
            "let linking_padding = Zeroizing::new(",
            "let commitment_padding = Zeroizing::new(",
            "Zeroizing::new([input_blind_v.coordinates.0, input_blind_v.coordinates.1])",
            "&input_blind_v_padding[..]",
            "drop(output_coordinates)",
            "mem::take(&mut output_coordinates)",
        ],
    );
    assert_source_counts(
        prove_once,
        &[(
            "let input_blind_v_padding = ProverSecretCopyValueV1::new([",
            1,
        )],
    );

    let field_source = include_str!("../field.rs");
    let private_conversion = source_section(
        field_source,
        "pub(super) fn secret_edwards_to_wei25519_v1(",
        "pub(super) fn monero_varint(",
    );
    assert_source_contains_all(
        private_conversion,
        &[
            "bytes: &[u8; 32]",
            "SecretCopyValueV1::new(CompressedEdwardsY(*bytes))",
            ".decompress()",
            "SecretCopyValueV1::new(point.as_ref().compress())",
            "secret_invert_field25519_v1(denominator.as_ref())",
            "let wei_x = SecretCopyValueV1::new(",
            "let wei_y = SecretCopyValueV1::new(",
            "Ok((wei_x.expose_copy(), wei_y.expose_copy()))",
        ],
    );
    assert!(!production.contains(
        "#[cfg(any(test, feature = \"privacy-release-evidence\"))]\n\
         fn prover_secret_output_coordinate_owners_v1("
    ));
}
#[test]
fn fixture_secret_selene_hash_source_uses_borrowed_exact_builder_and_owned_result() {
    let source = include_str!("../prover.rs");
    assert!(source.contains("fn prover_secret_hash_selene_v1("));
    let private_hash = source
        .split_once("fn prover_secret_hash_selene_v1(")
        .expect("private fixture Selene hash")
        .1
        .split_once("/// Build the canonical deterministic FCMP++ release fixture.")
        .expect("private fixture Selene hash boundary")
        .0;
    let width = private_hash
        .find("values.is_empty() || values.len() > SELENE_GENERATOR_COUNT_V1")
        .expect("canonical Selene width guard");
    let exact_capacity = private_hash
        .find(".checked_add(1)")
        .expect("initializer-inclusive exact capacity");
    let builder = private_hash
        .find("SecretMultiexpBuilder::<SeleneSuite>::new(exact_capacity)?")
        .expect("exact-capacity secret builder");
    let initializer = private_hash
        .find("terms.push(&Field25519::ONE, &selene_hash_initializer())?")
        .expect("canonical initializer term");
    let ordered_generators = private_hash
        .find("values.iter().zip(selene_generators())")
        .expect("ordered canonical generator prefix");
    let borrowed_push = private_hash
        .find("terms.push(scalar, generator)?")
        .expect("borrowed secret term insertion");
    let evaluate = private_hash
        .find("let point = terms.evaluate()?")
        .expect("complete secret multiexponentiation");
    let result_owner = private_hash
        .find("Ok(ProverSecretPointV1::from_secret(point))")
        .expect("controlled transfer into the next move-only owner");
    assert!(
        width < exact_capacity
            && exact_capacity < builder
            && builder < initializer
            && initializer < ordered_generators
            && ordered_generators < borrowed_push
            && borrowed_push < evaluate
            && evaluate < result_owner
    );
    for forbidden in [
        ".fold(",
        "generator.mul(*scalar)",
        "hash.add(",
        "terms.push(*scalar",
        "point.expose_copy()",
        "ProverSecretPointV1::take(&mut point)",
        ".transfer(",
    ] {
        assert!(!private_hash.contains(forbidden));
    }
    let release_fixture = source
        .split_once("pub(crate) fn fcmp_release_fixture_v1(")
        .expect("release fixture")
        .1
        .split_once("/// Build a maximum-shape fixture whose first canonical branch")
        .expect("release fixture boundary")
        .0;
    let private_call = release_fixture
        .find("prover_secret_hash_selene_v1")
        .expect("private initial hash call");
    let borrowed_x = release_fixture
        .find("prover_secret_selene_x_v1(&current_selene)?")
        .expect("secret-safe owned-point coordinate extraction");
    let next_owner = release_fixture
        .find("current_selene =\n                prover_secret_hash_selene_v1")
        .expect("later private hash returns an owner");
    assert!(private_call < borrowed_x && borrowed_x < next_owner);
    assert!(!release_fixture.contains("let mut next_selene = hash_selene(&[child])?"));
    assert!(!release_fixture.contains("hash_selene,\n    )?"));
    let field_source = include_str!("../field.rs");
    let public_hash = field_source
        .split_once("pub(super) fn hash_selene(")
        .expect("public Selene hash")
        .1
        .split_once("pub(super) fn hash_helios(")
        .expect("public Selene hash boundary")
        .0;
    assert!(public_hash.contains(".fold(selene_hash_initializer()"));
    assert!(public_hash.contains("hash.add(generator.mul(*scalar))"));
}
#[test]
fn rerandomization_constructor_takes_all_bytes_before_decoding() {
    let source = include_str!("../prover.rs");
    let constructor = source
        .split_once("impl FcmpInputRerandomizationV1 {")
        .expect("rerandomization impl")
        .1
        .split_once("#[cfg(test)]\n    fn duplicate_for_test")
        .expect("constructor boundary")
        .0;
    assert_eq!(
        constructor
            .matches("ProverSecretCopyValueV1::take(&mut")
            .count(),
        4
    );
    assert_eq!(
        constructor
            .matches(": ProverSecretCopyValueV1<[u8; 32]>")
            .count(),
        4
    );
    assert!(constructor.contains("fn from_rerandomization_secret_byte_owners_v1("));
    let last_take = constructor
        .rfind("ProverSecretCopyValueV1::take(&mut")
        .expect("last input take");
    let decode = constructor.find("let decode =").expect("decoder");
    assert!(last_take < decode);
    assert_eq!(
        constructor
            .matches("ProverSecretCopyValueV1::new(decode(")
            .count(),
        4
    );
    let last_scalar = constructor
        .rfind("ProverSecretCopyValueV1::new(decode(")
        .expect("last decoded owner");
    let publish = constructor.find("Ok(Self {").expect("final publication");
    assert!(decode < last_scalar && last_scalar < publish);
    assert!(!constructor.contains("Zeroizing::new(output)"));
    assert!(!constructor.contains("output: decode("));
}
#[test]
fn prover_input_constructor_takes_secret_bytes_before_validation() {
    let source = include_str!("../prover.rs");
    let constructor = source
        .split_once("impl FcmpProverInputV1 {")
        .expect("prover input impl")
        .1
        .split_once("#[cfg(test)]\n    fn duplicate_for_test")
        .expect("constructor boundary")
        .0;
    assert_eq!(
        constructor
            .matches("ProverSecretCopyValueV1::take(&mut")
            .count(),
        2
    );
    let last_take = constructor
        .rfind("ProverSecretCopyValueV1::take(&mut")
        .expect("last input take");
    let first_validation = constructor
        .find("validate_edwards_scalar(")
        .expect("first scalar validation");
    assert!(last_take < first_validation);
    assert_eq!(
        constructor.matches("ProverSecretCopyValueV1::new(").count(),
        2
    );
    let last_scalar = constructor
        .rfind("ProverSecretCopyValueV1::new(")
        .expect("last decoded owner");
    let publish = constructor.find("Ok(Self {").expect("final publication");
    assert!(first_validation < last_scalar && last_scalar < publish);
    assert!(!constructor.contains("Zeroizing::new(spend_x)"));
    assert!(!constructor.contains("Zeroizing::new(output_y)"));
    assert!(constructor.contains("spend_x: spend_x_scalar.expose_copy()"));
    assert!(constructor.contains("output_y: output_y_scalar.expose_copy()"));
    assert_eq!(
        constructor
            .matches("decode_secret_helioselene_scalar_v1(encoded)?")
            .count(),
        1
    );
    assert_eq!(
        constructor
            .matches("decode_secret_field25519_scalar_v1(encoded)?")
            .count(),
        1
    );
    assert_eq!(
        constructor
            .matches("require_preallocated_push(decoded_branch.len(), decoded_branch.capacity())?")
            .count(),
        2
    );
    assert_eq!(
        constructor
            .matches("push_secret_scalar_v1(\n                        &mut decoded_branch,")
            .count(),
        2
    );
    assert!(!constructor.contains("decoded_branch.push(decode_"));
    assert!(!constructor.contains("decode_helioselene_scalar(*encoded)"));
    assert!(!constructor.contains("decode_field25519_scalar(*encoded)"));
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
    let source = include_str!("../prover.rs");
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
    let prover = include_str!("../prover.rs");
    let field = include_str!("../field.rs");
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
    let push = owned_secret_push
        .find("values.push(value.0)")
        .expect("direct retained-slot copy");
    let clear = owned_secret_push
        .find("value.0.clear_secret()")
        .expect("source owner clear");
    let success_drop = owned_secret_push[clear..]
        .find("drop(value)")
        .map(|position| clear + position)
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
            && error_drop < push
            && push < clear
            && clear < success_drop
            && success_drop < post_capacity
            && post_capacity < post_pointer
    );
    assert!(owned_secret_push.contains("mut value: ProverSecretScalarV1<F>"));
    assert_eq!(owned_secret_push.matches("drop(value)").count(), 2);
    assert!(!owned_secret_push.contains("value.expose_copy()"));
    assert!(!owned_secret_push.contains("value.expose_ref()"));
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
    assert_eq!(prove_once.matches(&secret_push_call).count(), 2);
    assert_eq!(prove_once.matches(&owned_secret_push_call).count(), 4);
    assert!(!prover.contains("-blind.scalar"));
    assert_eq!(
        prover
            .matches("blind.scalar.expose_ref().neg_ref()")
            .count(),
        2
    );
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
    let circuit_source = include_str!("../circuit.rs");
    let commitment_producer = between(
        circuit_source,
        "pub(super) fn commitments_and_openings<S: ProofSuite<Scalar = F>>",
        "pub(super) struct Circuit<S: ProofSuite>",
    );
    assert!(commitment_producer.contains("Vec<SecretPoint<S::Point>>"));
    assert!(!commitment_producer.contains("Vec<S::Point>"));
    let proof_math_source = include_str!("../proof_math.rs");
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
        assert_source_order(
            handoff,
            &[
                "scalar,",
                "decomposition: core::mem::take(&mut *decomposition)",
                "divisor,",
                "point,",
            ],
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
            .matches("blind.scalar.expose_ref().neg_ref()")
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
        assert_source_order(
            claim,
            &[
                "let coordinates = blind",
                ".secret_coordinates_ref_v1()",
                ".append_claimed_point(",
                "coordinates.component_pair_ref()",
            ],
        );
        assert!(!claim.contains(".coordinates()"));
        assert!(!claim.contains("Zeroizing::new("));
        assert!(!claim.contains("*coordinates.component_pair_ref()"));
    }
    let divisor_source = include_str!("../divisor.rs");
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
        .find("let coordinates = Zeroizing::new")
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
    assert!(ed_blind.contains("scalar: &Scalar"));
    assert!(ed_blind.contains("decomposition: core::mem::take(&mut *decomposition)"));
    assert!(ed_blind.contains("coordinates: *coordinates"));
    assert!(!ed_blind.contains("generator * scalar"));
    assert!(ed_blind.contains("secret_edwards_to_wei25519_v1(&encoded_point)"));
    assert!(!ed_blind.contains("edwards_to_wei25519(*encoded_point)"));
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
    assert!(secret_coordinates.contains("Ok((wei_x.expose_copy(), wei_y.expose_copy()))"));
    assert_eq!(secret_coordinates.matches("expose_copy()").count(), 2);
    assert!(!secret_coordinates.contains("field25519_is_odd(x.expose_copy())"));
    assert!(!secret_coordinates.contains("y_squared.expose_copy()"));
    assert!(!secret_coordinates.contains("y_plus_one.expose_copy()"));
    assert!(!secret_coordinates.contains("one_minus_y.expose_copy()"));
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
    assert!(
        input_blinds.contains(
            "let sal_y_bytes = ProverSecretCopyValueV1::new(sal_y.expose_ref().to_bytes())"
        )
    );
    assert!(input_blinds.contains("sal_y_bytes.expose_copy()"));
    assert!(
        input_blinds.contains("ProverSecretCopyValueV1::new(rerandomization.linking.to_bytes())")
    );
    assert!(input_blinds.contains("sal_linking_bytes.expose_copy()"));
    assert!(!input_blinds.contains("let sal_y = Zeroizing::new("));
    assert!(!input_blinds.contains("sal_y.to_bytes()"));
    assert!(input_blinds.contains(
        "ProverSecretCopyValueV1::new(rerandomization.rerandomization_blind.to_bytes())"
    ));
    assert!(input_blinds.contains("sal_rerandomization_blind_bytes.expose_copy()"));
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
    let membership = include_str!("../membership.rs");
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
fn spendable_output(
    x: Scalar,
    y: Scalar,
    linking: Scalar,
    commitment: Scalar,
) -> FcmpOutputTupleV1 {
    fcmp_fixture_spendable_output_from_scalars_v1(x, y, linking, TEST_AMOUNT, commitment)
        .expect("valid output")
        .0
}
fn output_opening(
    output_key: u64,
    linking: u64,
    amount: u64,
    mask: u64,
) -> FcmpOutputCommitmentOpeningV1 {
    fcmp_fixture_output_opening_v1(output_key, linking, amount, mask).expect("valid output opening")
}
fn rerandomization(
    output: u64,
    linking: u64,
    blind: u64,
    commitment: u64,
) -> FcmpInputRerandomizationV1 {
    fcmp_fixture_rerandomization_v1(output, linking, blind, commitment)
        .expect("canonical test rerandomization")
}
fn one_layer_fixture() -> (
    FcmpProverInputV1,
    FcmpOutputCommitmentOpeningV1,
    FcmpTreeRootV1,
) {
    let (mut inputs, mut outputs, root) =
        fcmp_release_fixture_v1(false).expect("one-layer release fixture");
    assert_eq!(inputs.len(), 1);
    assert_eq!(outputs.len(), 1);
    (inputs.remove(0), outputs.remove(0), root)
}
fn maximum_bound_fixture() -> (
    Vec<FcmpProverInputV1>,
    Vec<FcmpOutputCommitmentOpeningV1>,
    FcmpTreeRootV1,
) {
    fcmp_release_fixture_v1(true).expect("maximum-bound release fixture")
}
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
    reset_prover_secret_copy_owner_drops();
    reset_prover_secret_point_owner_drops();
    let path = parse_path(&inputs[0], root).expect("owned maximum-depth path");
    assert_eq!(
        prover_secret_copy_owner_drops(),
        expected_pair_drops + expected_difference_drops
    );
    assert_eq!(prover_secret_point_owner_drops(), expected_point_drops);
    drop(path);
    assert_eq!(prover_secret_point_owner_drops(), expected_point_drops);
    match &mut inputs[0].additional_branches[0] {
        AdditionalBranch::ToHelios(values) => {
            replace_first_secret_coordinate_v1(values, |original| {
                HelioseleneField::conditional_select(
                    &HelioseleneField::ONE,
                    &HelioseleneField::ONE.add_ref(&HelioseleneField::ONE),
                    original.sub_ref(&HelioseleneField::ONE).ct_is_zero(),
                )
            })
            .expect("replace first private path coordinate");
        }
        AdditionalBranch::ToSelene(_) => panic!("first path branch must hash to Helios"),
    }
    reset_prover_secret_copy_owner_drops();
    reset_prover_secret_point_owner_drops();
    assert!(matches!(
        parse_path(&inputs[0], root),
        Err(FcmpNativeErrorV1::ArithmeticInvariant)
    ));
    assert_eq!(
        prover_secret_copy_owner_drops(),
        expected_pair_drops + FCMP_LAYER_TWO_LEN_V1
    );
    assert_eq!(prover_secret_point_owner_drops(), 1);
    reset_prover_secret_copy_owner_drops();
    reset_prover_secret_point_owner_drops();
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let path = parse_path(&inputs[1], root).expect("second owned maximum-depth path");
        assert_eq!(
            prover_secret_copy_owner_drops(),
            expected_pair_drops + expected_difference_drops
        );
        assert_eq!(prover_secret_point_owner_drops(), expected_point_drops);
        let _ = core::hint::black_box(&path);
        panic!("exercise parsed private path unwind");
    }));
    assert!(unwind.is_err());
    assert_eq!(
        prover_secret_copy_owner_drops(),
        expected_pair_drops + expected_difference_drops
    );
    assert_eq!(prover_secret_point_owner_drops(), expected_point_drops);
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
    let source = include_str!("../prover.rs");
    let parse = source_part!(source; "fn parse_path(" => "fn random_proof_scalar<F: ProofScalar>");
    source_has!(parse; "prover_secret_leaf_coordinates_v1(", "secret_edwards_to_wei25519_v1,", "Some(prover_secret_hash_selene_v1(&leaves)?)", "prover_secret_selene_x_v1(", "ct_helioselene_slice_contains(&padded, &prior_x)", "let next_c2 = prover_secret_hash_helios_v1(&padded)?", "current_c2 = Some(next_c2)", "prover_secret_helios_x_v1(", "ct_field25519_slice_contains(&padded, &prior_x)", "let next_c1 = prover_secret_hash_selene_v1(&padded)?", "current_c1 = Some(next_c1)", "ct_secret_selene_point_eq_v1(actual, &expected)?", "ct_secret_helios_point_eq_v1(actual, &expected)?");
    source_lacks!(parse; ".components()", "let (x, y) = edwards_to_wei25519", "hash_selene(&leaves)", "Some(hash_helios(&padded)?)", "Some(hash_selene(&padded)?)", ".and_then(SelenePoint::x)", ".and_then(HeliosPoint::x)", ".x()", "current_c1.take()", "current_c2.take()", "ct_selene_point_eq(actual.expose_ref(), &expected)", "ct_helios_point_eq(actual.expose_ref(), &expected)");
    source_order!(source_part!(parse; "AdditionalBranch::ToHelios(branch) => {" => "AdditionalBranch::ToSelene(branch) => {"); "prover_secret_selene_x_v1(", "ct_helioselene_slice_contains(&padded, &prior_x)", "let next_c2 = prover_secret_hash_helios_v1(&padded)?", "current_c2 = Some(next_c2)");
    source_order!(source_part!(parse; "AdditionalBranch::ToSelene(branch) => {" => "let matches_root = match root.curve()"); "prover_secret_helios_x_v1(", "ct_field25519_slice_contains(&padded, &prior_x)", "let next_c1 = prover_secret_hash_selene_v1(&padded)?", "current_c1 = Some(next_c1)");
    let containment =
        source_part!(source; "fn ct_field25519_slice_contains(" => "enum AdditionalBranch");
    source_counts!(containment; "target: &SecretCycleScalarV1<" => 2, "ProverSecretCopyValueV1::new(value.sub_ref(target))" => 2, "difference.expose_ref().ct_is_zero()" => 2);
    source_lacks!(containment; "target: Field25519", "target: HelioseleneField", "*value - *target");
    let root_comparison = source_part!(source; "fn ct_secret_selene_point_eq_v1(" => "fn ct_field25519_slice_contains(");
    source_counts!(root_comparison; ".secret_encoding_owner_v1()" => 2, "Zeroizing::new(public_right.encode())" => 2, "left.as_ref().as_slice().ct_eq(public_right.as_slice())" => 2);
    source_lacks!(root_comparison; "left.encode()", "left.expose_copy()", "left.expose_public_copy_v1()");
    let tuple_source = include_str!("../mod.rs");
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
    ] {
        assert!(source.contains(production_helper));
        assert!(!immediately_cfg_gated(source, production_helper));
    }
    let field_source = include_str!("../field.rs");
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

    let source = include_str!("../prover.rs");
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
    assert_source_excludes_all(
        random,
        &[
            "Result<F, FcmpNativeErrorV1>",
            "if let Some(mut scalar)",
            "ProverSecretScalarV1::take(&mut scalar)",
            "scalar.expose_copy().is_zero()",
            "return Ok(scalar.expose_copy())",
        ],
    );
    let owner = source_section(
        source,
        "impl<F: ProofScalar> ProverSecretScalarV1<F>",
        "impl ProverSecretScalarV1<Field25519>",
    );
    let borrowed_constructor = source_section(
        owner,
        "fn copy_from_borrowed(value: &F) -> Self",
        "fn take(value: &mut F) -> Self",
    );
    assert!(borrowed_constructor.contains("Self(*value)"));
    assert_source_excludes_all(
        borrowed_constructor,
        &["Self::take", "Self::new", "BorrowedProverScalarSlotV1"],
    );
    assert!(!owner.contains("fn expose_copy(&self) -> F"));
    let proof_math = include_str!("../proof_math.rs");
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
    assert_source_counts(
        prove_once,
        &[
            ("random_proof_scalar", 8),
            ("push_owned_secret_scalar_v1(&mut", 4),
            ("let nonce = random_proof_scalar::<", 2),
        ],
    );
    assert_source_contains_all(
        prove_once,
        &[
            "prepare_selene_blind(random_proof_scalar(rng)?)?",
            "prepare_helios_blind(random_proof_scalar(rng)?)?",
            "push_owned_secret_scalar_v1(&mut c1_branch_masks, random_proof_scalar(rng)?)?",
            "push_owned_secret_scalar_v1(&mut c2_branch_masks, random_proof_scalar(rng)?)?",
            "push_owned_secret_scalar_v1(&mut c1_masks, random_proof_scalar(rng)?)?",
            "push_owned_secret_scalar_v1(&mut c2_masks, random_proof_scalar(rng)?)?",
            "let nonce = random_proof_scalar::<Field25519>(rng)?",
            "let nonce = random_proof_scalar::<HelioseleneField>(rng)?",
        ],
    );
    assert_source_excludes_all(
        prove_once,
        &[
            "push_secret_scalar_v1(&mut c1_branch_masks, random_proof_scalar(rng)?)?",
            "push_secret_scalar_v1(&mut c2_branch_masks, random_proof_scalar(rng)?)?",
            "push_secret_scalar_v1(&mut c1_masks, random_proof_scalar(rng)?)?",
            "push_secret_scalar_v1(&mut c2_masks, random_proof_scalar(rng)?)?",
            "let mut nonce = random_proof_scalar",
            "ProverSecretScalarV1::take(&mut nonce)",
        ],
    );
}
#[test]
fn prepared_cycle_blind_owners_survive_handoff_until_success_drop() {
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
    // privacy_engines::fcmp_plus_plus::prover::tests::maximum_compiled_shape_release_resource_audit
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
    let source = include_str!("tests.rs");
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
    let range_size = super::super::fcmp_range_proof_size_v1(1).expect("range proof size");
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
