//! Exact borrowed controller geometry shared by original structural relations.
//!
//! This prepays source inspection, never a controller validity predicate or a
//! physical allocation. It neither reparses malformed typed keys nor grants
//! account, execution, publication or finality authority.

use iroha_data_model::account::{AccountController, AccountId};

/// Fund complete retained geometry before a caller compares either account.
///
/// Preserve the original domain relation's exact prepayment order. The callback
/// retains its own typed refusal; no error string, normalized key or scratch is
/// constructed. Member advances are admitted before visiting the original slice.
pub(in crate::state) fn prepay_account_id<E>(
    account: &AccountId,
    mut prepay: impl FnMut(usize) -> Result<(), E>,
) -> Result<(), E> {
    prepay(1)?; // controller variant
    match account.controller() {
        AccountController::Single(key) => {
            prepay(1)?; // compact algorithm tag, even for malformed storage
            prepay(key.input_payload_len())
        }
        AccountController::Multisig(policy) => {
            prepay(1 + 2 + 8)?; // version, threshold and member count
            prepay(policy.members().len())?; // each actual member advance
            for member in policy.members() {
                prepay(1 + 2)?; // compact algorithm tag and weight
                prepay(member.public_key().input_payload_len())?;
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_allocations::allocations_during;
    use iroha_data_model::account::{MultisigMember, MultisigPolicy};
    use iroha_test_samples::{ALICE_ID, BOB_ID};

    fn multisig() -> AccountId {
        AccountId::new_multisig(
            MultisigPolicy::new(
                2,
                vec![
                    MultisigMember::new(ALICE_ID.expect_single_signatory().clone(), 1).unwrap(),
                    MultisigMember::new(BOB_ID.expect_single_signatory().clone(), 2).unwrap(),
                ],
            )
            .unwrap(),
        )
    }

    #[test]
    fn shared_controller_work_keeps_exact_original_domain_order_and_geometry() {
        for (account, expected) in [
            (ALICE_ID.clone(), &[1, 1, 32][..]),
            (multisig(), &[1, 11, 2, 3, 32, 3, 32][..]),
        ] {
            let mut actual = [0; 7];
            let mut count = 0;
            assert_eq!(
                allocations_during(|| {
                    prepay_account_id(&account, |amount| {
                        actual[count] = amount;
                        count += 1;
                        Ok::<_, core::convert::Infallible>(())
                    })
                    .unwrap();
                }),
                0
            );
            assert_eq!(&actual[..count], expected);
        }
    }

    #[test]
    fn shared_controller_work_preserves_typed_refusal_at_every_admission_boundary() {
        #[derive(Debug, PartialEq, Eq)]
        struct Refused(usize);
        let account = multisig();
        for rejected in 0..7 {
            let mut calls = 0;
            assert_eq!(
                prepay_account_id(&account, |_| {
                    let position = calls;
                    calls += 1;
                    if position == rejected {
                        Err(Refused(position))
                    } else {
                        Ok(())
                    }
                }),
                Err(Refused(rejected))
            );
            assert_eq!(calls, rejected + 1, "no later unadmitted component");
        }
    }

    #[test]
    fn shared_controller_work_measures_discarded_keys_without_parsing_or_allocating() {
        let mut key = ALICE_ID.expect_single_signatory().clone();
        key.zeroize_for_confidential_discard();
        let account = AccountId::new(key);
        let mut total = 0;
        assert_eq!(
            allocations_during(|| {
                prepay_account_id(&account, |amount| {
                    total += amount;
                    Ok::<_, core::convert::Infallible>(())
                })
                .unwrap();
            }),
            0
        );
        assert_eq!(total, 2);
    }
}
