//! Actual native credential-union interval predicates in both Pasta fields.
//!
//! Known-public fixture signatures are admitted by the actual model before deriving assigned
//! cells. These component tests create no installed owner, platform evidence or monetary grant.

use super::super::ordinary_credential_union::assign_ordinary_credential_union_v1;
use super::*;
use halo2_base::gates::{GateInstructions as _, circuit::builder::BaseCircuitBuilder};
use halo2_proofs::{
    dev::MockProver,
    halo2curves::pasta::{Fp, Fq},
};
use iroha_data_model::testing::ordinary_app_enrollment::KagemushaOrdinaryRetailEnrollmentFixtureV1;

#[derive(Clone, Copy)]
enum Branch {
    Android,
    Apple,
    AndroidIntegrity,
}

fn accepted<F: KagemushaPoseidonFieldV1>(
    branch: Branch,
    issued: u64,
    expires: u64,
    substituted_selector: Option<u64>,
) -> bool {
    let fixture = match branch {
        Branch::Android => KagemushaOrdinaryRetailEnrollmentFixtureV1::new(false),
        Branch::Apple => KagemushaOrdinaryRetailEnrollmentFixtureV1::new(true),
        Branch::AndroidIntegrity => {
            KagemushaOrdinaryRetailEnrollmentFixtureV1::android_with_integrity()
        }
    };
    fixture
        .verify(300)
        .expect("genuinely signed synthetic native fixture");
    let mut builder = BaseCircuitBuilder::<F>::new(false)
        .use_k(14)
        .use_lookup_bits(13)
        .use_instance_columns(1);
    let union =
        assign_ordinary_credential_union_v1(&mut builder, &fixture.selection.issuance.credential)
            .unwrap();
    let selector = if let Some(value) = substituted_selector {
        let attempted = builder.main(0).load_witness(F::from(value));
        // Production passes union.integrity itself. This equality models exactly that same-cell
        // boundary while making an adversarial input substitution observable to MockProver.
        builder
            .main(0)
            .constrain_equal(&attempted, &union.integrity);
        attempted
    } else {
        union.integrity
    };
    let issued = builder.main(0).load_witness(F::from(issued));
    let expires = builder.main(0).load_witness(F::from(expires));
    constrain_ordinary_initial_integrity_interval_v1(
        &mut builder,
        &union.cells,
        selector,
        issued,
        expires,
    )
    .unwrap();
    builder.calculate_params(Some(9));
    MockProver::run(14, &builder, vec![vec![]])
        .unwrap()
        .verify()
        .is_ok()
}

fn check(branch: Branch, issued: u64, expires: u64, selector: Option<u64>, expected: bool) {
    let eq = accepted::<Fp>(branch, issued, expires, selector);
    let ep = accepted::<Fq>(branch, issued, expires, selector);
    assert_eq!(
        eq, ep,
        "both parities independently evaluate the actual assigned predicate"
    );
    assert_eq!(eq, expected);
}

#[test]
fn original_no_integrity_apple_and_integrity_intervals_accept_in_both_fields() {
    for branch in [Branch::Android, Branch::Apple] {
        // Absent PI uses fixed zero cells, with the actual false selector masking those bounds.
        check(branch, 300, 9000, None, true);
    }
    check(Branch::AndroidIntegrity, 300, 900, None, true);
    check(Branch::AndroidIntegrity, 300, 1200, None, true);
}

#[test]
fn credential_and_selected_integrity_deadline_violations_reject_in_both_fields() {
    for branch in [Branch::Android, Branch::Apple, Branch::AndroidIntegrity] {
        check(branch, 199, 900, None, false);
        check(branch, 300, 10201, None, false);
    }
    // The actual signed PI refresh deadline is1200 while the credential expires10200.
    // This branch distinguishes the selected PI predicate from the credential-only bound.
    check(Branch::AndroidIntegrity, 300, 1201, None, false);
    check(Branch::AndroidIntegrity, 300, 9000, None, false);
}

#[test]
fn selected_integrity_bit_cannot_be_replaced_or_made_nonboolean_in_either_field() {
    check(Branch::AndroidIntegrity, 300, 900, Some(0), false);
    for branch in [Branch::Android, Branch::Apple] {
        check(branch, 300, 900, Some(1), false);
    }
    for branch in [Branch::Android, Branch::Apple, Branch::AndroidIntegrity] {
        check(branch, 300, 900, Some(2), false);
    }
}
