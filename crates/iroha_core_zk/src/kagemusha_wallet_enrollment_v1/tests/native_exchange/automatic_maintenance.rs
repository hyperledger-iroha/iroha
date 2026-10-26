//! Genuine pre-Archive custody handoff to the separate bridge maintenance consumer.
//!
//! This producer stops the real exchange before either manual Archive. It exports DATA and
//! locations only; the consumer must independently admit the complete signed source graph.

use super::*;

#[test]
#[ignore = "requires fresh SAME A Load custody, finalized Load and complete current catalog; leaves A/B custody for the separate genuine bridge worker consumer"]
fn retain_real_receive_and_status_for_automatic_bridge_maintenance() {
    run_exchange(true);
}

/// Exact owners and originals of one completed Send awaiting its first Archive.
pub(super) struct PendingReceipt<'a> {
    pub(super) wallet: &'a mut Wallet,
    pub(super) device: &'a HostDevice,
    pub(super) frames: &'a [Vec<u8>; 4],
    pub(super) send: &'a OperationRequestV1,
    pub(super) payment: &'a [u8],
    pub(super) original: &'a [u8],
}

/// Retain one real receipt without verifying it or dispatching its Archive.
pub(super) fn retain(
    output: &Path,
    branch: &str,
    receipt: PendingReceipt<'_>,
) -> norito::json::Value {
    let PendingReceipt {
        wallet,
        device,
        frames,
        send,
        payment,
        original,
    } = receipt;
    let evidence: KagemushaWalletCreditedV1 = decode(original);
    assert!(matches!(
        (branch, &evidence.evidence),
        ("receive", KagemushaWalletCreditedEvidenceV1::Receive { .. })
            | ("status", KagemushaWalletCreditedEvidenceV1::Status { .. })
    ));
    fold(wallet);
    wallet.scheduler().set_activity(false, false);
    let before = wallet.snapshot().unwrap();
    assert_eq!(before.fold_backlog, 0);
    assert_eq!(before.verified_fold.unwrap().sequence, before.sequence);
    assert_eq!(wallet.deferred_archive_summary().unwrap().count, 0);
    let signatures = device.platform.with(|state| state.sign_calls);
    wallet
        .retain_deferred_credited(&send.request_id, original)
        .unwrap();
    assert_eq!(wallet.snapshot().unwrap(), before);
    assert_eq!(device.platform.with(|state| state.sign_calls), signatures);
    let directory = output.join(format!("maintenance-{branch}"));
    private_dir(&directory);
    let mut originals = Vec::new();
    for (name, bytes) in FRAME_NAMES
        .into_iter()
        .zip(frames.iter().map(Vec::as_slice))
        .chain([("payment.norito", payment), ("credited.norito", original)])
    {
        publish(&directory.join(name), bytes);
        originals.push(norito::json!({
            "name": name, "bytes": (bytes.len()), "sha256": (sha(bytes))
        }));
    }
    norito::json!({
        "branch": branch,
        "custody_device": (device.root.to_str().unwrap()),
        "originals_directory": (directory.to_str().unwrap()),
        "originals": originals,
        "send_request": (hex::encode(send.request_id)),
        "wallet_id": (hex::encode(before.wallet_id)),
        "head": (hex::encode(before.head)),
        "sequence": (before.sequence.to_string()),
        "owned_balance": (before.owned_balance.to_string()),
        "known_burned_total": (before.known_burned_total.to_string())
    })
}

/// Publish only after the caller has dropped all three exclusive wallet owners.
pub(super) fn publish_handoff(
    output: &Path,
    sources: &Sources,
    setup: &LedgerSetup,
    load_target: &[u8],
    occurrences: [norito::json::Value; 2],
) {
    let occurrences = norito::json::Value::Array(Vec::from(occurrences));
    let manifest = norito::json!({
        "schema": "iroha.kagemusha.native-maintenance-handoff.v1",
        "source_pins": (source_pins()),
        "sealed_originals": (source_reuse::locator(&sources.originals)),
        "producer_binary_sha256": (executable_hash()),
        "producer_source_sha256": (env_pin("KAGEMUSHA_NATIVE_SOURCE_SHA256")),
        "load_target_sha256": (sha(load_target)),
        "executed_ledger_setup_sha256": (setup.manifest_sha256.clone()),
        "occurrences": occurrences,
        "physical_device_qualified": false,
        "scope": "Real A and B host custody, actual Receive and folded Status originals; simulated hardware. No Archive has been executed and no automatic-worker, unload or network qualification is claimed."
    });
    publish(
        &output.join("automatic-maintenance-handoff.json"),
        &norito::json::to_vec(&manifest).unwrap(),
    );
}
