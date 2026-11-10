//! Detached Ironwood verification preserves native refusals and permits Python progress.

use super::*;
use std::{
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    sync::{
        Arc, Barrier,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

/// Used by the real certified-cut and committed-payment fixtures, never a fake verification.
pub(crate) fn assert_python_thread_progress<T>(verify: impl FnOnce() -> T) -> T {
    Python::initialize();
    let progress = Arc::new(AtomicUsize::new(0));
    let stop = Arc::new(AtomicBool::new(false));
    let ready = Arc::new(Barrier::new(2));
    let helper = {
        let (progress, stop, ready) = (progress.clone(), stop.clone(), ready.clone());
        std::thread::spawn(move || {
            ready.wait();
            while !stop.load(Ordering::SeqCst) {
                Python::attach(|py| {
                    // Count actual Python execution, not merely a Rust helper's wake-up.
                    py.run(c"sum(range(32))", None, None).unwrap();
                    progress.fetch_add(1, Ordering::SeqCst);
                });
                std::thread::yield_now();
            }
        })
    };
    let (result, during) = Python::attach(|_| {
        // The helper cannot acquire Python until the production verifier detaches.
        ready.wait();
        let result = catch_unwind(AssertUnwindSafe(verify));
        // Reattached here: a helper that only runs after return cannot satisfy this check.
        let during = progress.load(Ordering::SeqCst);
        stop.store(true, Ordering::SeqCst);
        (result, during)
    });
    helper.join().unwrap();
    let result = result.unwrap_or_else(|error| resume_unwind(error));
    assert!(
        during > 0,
        "Python made no progress during real Ironwood verification"
    );
    result
}

fn assert_value_error(py: Python<'_>, error: PyErr, message: &str) {
    assert!(error.is_instance_of::<PyValueError>(py));
    assert_eq!(error.value(py).to_string(), message);
}

#[test]
fn ironwood_detached_validation_preserves_python_error_categories() {
    Python::initialize();
    Python::attach(|py| {
        let error = NativeValidationError::Runtime(
            "canonical transaction byte limit does not fit this platform",
        )
        .into_py();
        assert!(error.is_instance_of::<PyRuntimeError>(py));
        assert_eq!(
            error.value(py).to_string(),
            "canonical transaction byte limit does not fit this platform"
        );
        for (input, message) in [
            ("", "payer_account_id must not be blank"),
            (
                " payer ",
                "payer_account_id must not contain surrounding whitespace",
            ),
            (
                "payer@domain",
                "payer_account_id must be an exact canonical I105 account id",
            ),
        ] {
            assert_value_error(
                py,
                parse_selected_account_id(input, "payer_account_id", 369).unwrap_err(),
                message,
            );
            assert_value_error(
                py,
                parse_selected_account_id_native(input, "payer_account_id", 369)
                    .unwrap_err()
                    .into_py(),
                message,
            );
        }
        let maximum = iroha_data_model::parameter::system::TransactionParameters::default()
            .max_tx_bytes()
            .get();
        for input in [b"".as_slice(), b"malformed signed transaction".as_slice()] {
            let message = if input.is_empty() {
                format!("signed_transaction_versioned must contain between 1 and {maximum} bytes")
            } else {
                "signed_transaction_versioned is not a valid current signed transaction".into()
            };
            assert_value_error(
                py,
                decode_canonical_signed_transaction_v1(input).unwrap_err(),
                &message,
            );
            assert_value_error(
                py,
                decode_canonical_signed_transaction_v1_native(input)
                    .unwrap_err()
                    .into_py(),
                &message,
            );
        }
    });
}

#[test]
fn ironwood_state_rows_and_payment_effect_preserve_finality_refusals() {
    Python::initialize();
    Python::attach(|py| {
        // Even the payment path authenticates its preparation rows before inspecting
        // the signed transaction, amount or payment page. Keep that original refusal.
        use iroha_core::state::{World, WorldReadOnly};
        let mut world = World::new();
        iroha_core::runtime::ironwood::install_for_test(&mut world);
        iroha_core::runtime::keyed_state::install_for_test(&mut world);
        let view = world.view();
        let selection = view.runtime_selection();
        let schema = iroha_data_model::sumeragi_finality::SelectedStateSchemaV1::from_selection(
            selection,
            &iroha_core::runtime::release_catalog().unwrap().definitions,
        )
        .unwrap();
        let pin = schema.digest();
        let schema = state_rows::PyIronwoodStateSchema(
            iroha_core_privacy::ironwood_state_rows::IronwoodStateSchemaV1::from_selected(
                schema, pin,
            )
            .unwrap(),
        );
        for (proof, checkpoint, network, message) in [
            (b"".as_slice(), "", "", "State proof authentication failed"),
            (
                b"rows".as_slice(),
                "",
                "zz",
                "noncanonical or oversized State proof hex",
            ),
            (
                b"rows".as_slice(),
                "",
                "01",
                "State proof identity must be 32 bytes",
            ),
        ] {
            assert_value_error(
                py,
                state_rows::verify_ironwood_state_rows_v1_py(
                    py,
                    proof,
                    proof,
                    checkpoint,
                    vec!["00".into()],
                    network,
                    369,
                    "",
                    "",
                    "",
                    "",
                    &schema,
                )
                .unwrap_err(),
                message,
            );
            assert_value_error(
                py,
                state_rows::verify_ironwood_payment_effect_from_rows_v1_py(
                    py,
                    proof,
                    proof,
                    checkpoint,
                    vec!["00".into()],
                    vec![],
                    b"",
                    network,
                    369,
                    "",
                    "",
                    "",
                    "",
                    &schema,
                    "malformed amount",
                )
                .unwrap_err(),
                message,
            );
        }
    });
}
