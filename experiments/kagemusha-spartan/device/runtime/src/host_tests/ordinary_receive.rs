//! Cold-path measurement from authenticated custody after a retained Send.
//! Use different processes for Receive and recovery so the latter can only
//! read the exact completion persisted by the wallet.
use super::super::*;

pub(crate) fn exercise(
    root: &Path,
    runtime: &mut SessionAdapter,
    slot: &[u8; 32],
    wallet: &[u8; 32],
    stage: &str,
    measurements: &mut Measurements,
) -> Result<String> {
    let saved = root.join("ordinary-receive-completion.norito");
    if stage == "ordinary-recover" {
        let bytes = std::fs::read(saved).map_err(fail)?;
        let record = native::CompletionRecord::decode(&bytes, wallet).map_err(fail)?;
        for part in 0..=2 {
            let output = measurements.measure("Receive", "recover_output", || {
                runtime.output(slot, record.operation_id(), part)
            })?;
            if output.status != 1 || output.bytes != record.output_part(part).map_err(fail)? {
                return Err("cold recovery changed a retained Receive output".into());
            }
        }
        runtime.close()?;
        return Ok("Recovered three exact Receive output parts from cold custody.".into());
    }
    let start = Instant::now();
    let instrumentation = measurements.instrumentation_us();
    let payment = measurements.measure("Receive", "retained_incoming_bytes", || {
        std::fs::read(root.join("incoming-payment.bin")).map_err(fail)
    })?;
    measurements.incoming_payment(&payment);
    if stage != "ordinary-prepare" {
        measurements.measure("Receive", "c_payment_verification", || {
            runtime.verify_payment(&payment)
        })?;
    }
    let action = wallet_action(4, &[&[0x39; 32], &payment])?;
    let prepared =
        measurements.measure("Receive", "prepare", || runtime.prepare(slot, &action, &[]))?;
    if prepared.review.get(2) != Some(&4) {
        return Err("ordinary Receive did not prepare a Receive review".into());
    }
    let operation: [u8; 32] = prepared
        .review
        .get(67..99)
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or("ordinary Receive review has no operation id")?;
    let prior =
        measurements.measure("Receive", "lookup", || runtime.output(slot, &operation, 2))?;
    if prior.status != 0 {
        return Err("ordinary Receive requires an unused operation on this custody clone".into());
    }
    if matches!(stage, "ordinary-verify-prepare" | "ordinary-prepare") {
        measurements.receive_flow(start, instrumentation);
        runtime.discard(prepared.token)?;
        runtime.close()?;
        return Ok("Prepared the original Payment and discarded its unselected review.".into());
    }
    let completion =
        measurements.measure("Receive", "execute", || runtime.execute(prepared.token))?;
    if completion.status != 1 {
        return Err(format!(
            "ordinary Receive did not release: {} / {}",
            completion.status, completion.detail
        ));
    }
    let record = native::CompletionRecord::decode(&completion.bytes, wallet).map_err(fail)?;
    if record.operation_id() != &operation || !record.output_part(1).map_err(fail)?.is_empty() {
        return Err("ordinary Receive returned an unrelated completion".into());
    }
    let released =
        measurements.measure("Receive", "output", || runtime.output(slot, &operation, 2))?;
    if released.status != 1 || released.bytes != completion.bytes {
        return Err("retained Receive output differs from its exact completion".into());
    }
    measurements.receive_flow(start, instrumentation);
    save_fixture_input(&saved, &completion.bytes)?;
    runtime.discard(prepared.token)?;
    runtime.close()?;
    Ok("Verified the original Payment and released one ordinary Receive.".into())
}
