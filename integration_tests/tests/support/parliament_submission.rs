//! Shared signed Parliament submissions and canonical contract artifacts for real networks.

use std::time::{Duration, Instant};

use eyre::{Result, WrapErr as _, eyre};
use iroha::{
    blocking::Client,
    client::{AccountTransactionDraft, FeeQuoteRequest, TransactionWaitOptions},
    data_model::{
        isi::InstructionBox,
        prelude::{FeePaymentIntent, SignedTransaction},
    },
};
use iroha_model_base::metadata::Metadata;

pub const OPERATION_TIMEOUT: Duration = Duration::from_secs(300);

const FEE_QUOTE_BACKPRESSURE_ATTEMPTS: usize = 8;
// A single attempt would make the backpressure retry unreachable.
const _: () = assert!(FEE_QUOTE_BACKPRESSURE_ATTEMPTS > 1);
const FEE_QUOTE_BACKPRESSURE_RETRY_AFTER: Duration = Duration::from_secs(1);

// Exact transient admission backpressure of the read path; nothing else is retried.
fn is_query_capacity_backpressure(rendered: &str) -> bool {
    rendered.contains("429 Too Many Requests")
        && rendered.contains("Reached the limit of parallel queries")
}

pub fn fee() -> FeePaymentIntent {
    FeePaymentIntent::authority(Vec::new(), None)
}

// The blocking client is retained only as the test network's account/configuration
// holder. All writes use its current account-owned async signing and finality API.
pub async fn prepare_parliament_transaction(
    client: &Client,
    instructions: impl IntoIterator<Item = impl Into<InstructionBox>>,
) -> Result<SignedTransaction> {
    let account = client.account_client();
    let mut payload = account.prepare_transaction(AccountTransactionDraft::new(
        instructions.into_iter().map(Into::into).collect::<Vec<_>>(),
        fee(),
        Metadata::default(),
    ))?;
    // The fee quote is a read against the bounded live-query store. Under local load the
    // node answers 429 with a one-second retry hint; honour it a bounded number of times
    // and surface every other failure unchanged.
    let mut quote_attempt = 1_usize;
    let quote = loop {
        let quoted = tokio::time::timeout(
            OPERATION_TIMEOUT,
            account.quote_fees(FeeQuoteRequest::AccountSignature { payload: &payload }),
        )
        .await
        .map_err(|_| eyre!("Parliament fee quote exceeded {OPERATION_TIMEOUT:?}"))?;
        match quoted {
            Ok(quote) => break quote,
            Err(error)
                if quote_attempt < FEE_QUOTE_BACKPRESSURE_ATTEMPTS
                    && is_query_capacity_backpressure(&format!("{error:#}")) =>
            {
                quote_attempt += 1;
                tokio::time::sleep(FEE_QUOTE_BACKPRESSURE_RETRY_AFTER).await;
            }
            Err(error) => return Err(error),
        }
    };
    if !payload
        .fee_payment
        .has_same_payer_and_gas_bound(&quote.intent)
    {
        return Err(eyre!("Parliament fee quote changed payer or gas bound"));
    }
    payload.fee_payment = quote.intent;
    Ok(account.sign_transaction(payload)?)
}

pub async fn submit_parliament_instructions(
    client: &Client,
    instructions: impl IntoIterator<Item = impl Into<InstructionBox>>,
) -> Result<()> {
    let transaction = prepare_parliament_transaction(client, instructions).await?;
    let deadline = Instant::now() + OPERATION_TIMEOUT;
    submit_prepared_parliament_transaction_until(client, &transaction, deadline).await
}

// Global finality can be resolved by a different peer. Every subsequent fixture read
// uses this configured peer, so require its exact local Applied observation and status
// publication before returning. All phases consume the original submission deadline.
async fn submit_prepared_parliament_transaction_until(
    client: &Client,
    transaction: &SignedTransaction,
    deadline: Instant,
) -> Result<()> {
    let signed_hash = transaction.hash();
    let bounded = client.with_request_deadline(deadline)?;
    tokio::time::timeout_at(tokio::time::Instant::from_std(deadline), async {
        let applied_hash = bounded
            .account_client()
            .submit_transaction_and_wait(transaction)
            .await?;
        if applied_hash != signed_hash {
            return Err(eyre!(
                "Parliament Applied response substituted the signed transaction hash"
            ));
        }
        let local = bounded
            .client()
            .wait_until_transaction_applied_local(
                signed_hash,
                TransactionWaitOptions {
                    timeout: deadline.saturating_duration_since(Instant::now()),
                    ..TransactionWaitOptions::default()
                },
            )
            .await?;
        let applied_height = local.block_height.ok_or_else(|| {
            eyre!("Parliament local Applied observation has no block height for {signed_hash}")
        })?;
        observe_exact_height(
            &bounded,
            applied_height,
            "Parliament local Applied publication",
        )
        .await?;
        if Instant::now() >= deadline {
            return Err(eyre!(
                "Parliament Applied publication exceeded its original deadline for {signed_hash}"
            ));
        }
        Ok(())
    })
    .await
    .map_err(|_| {
        eyre!("Parliament Applied publication exceeded its original deadline for {signed_hash}")
    })?
    .wrap_err_with(|| format!("Parliament Applied publication unresolved for {signed_hash}"))
}

const STATUS_RETRY_DELAY: Duration = Duration::from_millis(150);
const STATUS_RETRY_WINDOW: Duration = Duration::from_secs(30);

/// Classify the two typed transient status refusals; every other error is final.
///
/// Same policy as `support/committee_status.rs`: only the SDK's typed `StateBusy` and
/// `DeadlineElapsed` reasons are retry authority, and never past the remaining window.
fn status_retry_delay(error: &iroha::Error, remaining: Duration) -> Option<Duration> {
    let iroha::Error::StatusUnavailable {
        reason:
            Some(iroha::StatusFailureReason::DeadlineElapsed | iroha::StatusFailureReason::StateBusy),
        retry_after,
    } = error
    else {
        return None;
    };
    let delay = retry_after
        .unwrap_or(STATUS_RETRY_DELAY)
        .max(STATUS_RETRY_DELAY);
    (delay < remaining).then_some(delay)
}

/// Read the finalized height from the status projection.
///
/// A status read that lost the State publication race answers HTTP 503 `status_state_busy`
/// without publishing a snapshot. That is a property of the read, not of the chain, so it is
/// retried inside one bounded window; any other failure is returned unchanged.
pub async fn current_height(client: &Client) -> Result<u64> {
    let deadline = Instant::now() + STATUS_RETRY_WINDOW;
    loop {
        match client.client().status().get().await {
            Ok(status) => return Ok(status.blocks),
            Err(error) => {
                let Some(delay) =
                    status_retry_delay(&error, deadline.saturating_duration_since(Instant::now()))
                else {
                    return Err(error.into());
                };
                tokio::time::sleep(delay).await;
            }
        }
    }
}

#[test]
fn status_retry_accepts_only_typed_busy_or_deadline_inside_the_window() {
    use iroha::StatusFailureReason;
    let window = Duration::from_secs(1);
    for reason in [
        StatusFailureReason::StateBusy,
        StatusFailureReason::DeadlineElapsed,
    ] {
        let error = iroha::Error::StatusUnavailable {
            reason: Some(reason),
            retry_after: None,
        };
        assert_eq!(status_retry_delay(&error, window), Some(STATUS_RETRY_DELAY));
        assert_eq!(status_retry_delay(&error, STATUS_RETRY_DELAY), None);
        let advised = iroha::Error::StatusUnavailable {
            reason: Some(reason),
            retry_after: Some(Duration::from_millis(400)),
        };
        assert_eq!(
            status_retry_delay(&advised, window),
            Some(Duration::from_millis(400))
        );
    }
    for reason in [
        None,
        Some(StatusFailureReason::StateUnavailable),
        Some(StatusFailureReason::MissingBlock),
        Some(StatusFailureReason::MetricsStale),
    ] {
        let error = iroha::Error::StatusUnavailable {
            reason,
            retry_after: None,
        };
        assert_eq!(status_retry_delay(&error, window), None);
    }
}

/// Observe one exact finalized height through the client's status projection.
///
/// `ensure_blocks` can return from durable block storage before the status projection
/// publishes the same height. The projection may therefore trail by a bounded interval,
/// but it must never pass the expected height.
pub async fn observe_exact_height(
    client: &Client,
    expected_height: u64,
    label: &str,
) -> Result<()> {
    let deadline = Instant::now() + OPERATION_TIMEOUT;
    loop {
        let observed_height = current_height(client).await?;
        if observed_height == expected_height {
            return Ok(());
        }
        if observed_height > expected_height {
            return Err(eyre!(
                "{label}: expected exact height {expected_height}, observed {observed_height}"
            ));
        }
        if Instant::now() >= deadline {
            return Err(eyre!(
                "{label}: status stayed at height {observed_height} below committed height {expected_height} for {OPERATION_TIMEOUT:?}"
            ));
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

pub fn minimal_contract_artifact() -> Vec<u8> {
    minimal_contract_artifact_with_identity("ParliamentLifecycleSmoke", "integration-tests")
}

pub fn minimal_contract_artifact_with_identity(
    seiyaku_name: &str,
    compiler_fingerprint: &str,
) -> Vec<u8> {
    use ivm::{encoding::wide as enc, instruction::wide};
    let metadata = ivm::ProgramMetadata {
        version_major: 1,
        version_minor: 1,
        mode: 0,
        vector_length: 0,
        max_cycles: 1_000,
        abi_version: 1,
    };
    let interface = ivm::EmbeddedContractInterfaceV1 {
        callables: vec![ivm::call::EmbeddedCallableV1 {
            entry_pc: 0,
            frame_bytes: 0,
            arguments: ivm::call::CallSchemaV1::empty(),
            results: ivm::call::CallSchemaV1::unit(),
        }],
        seiyaku_name: seiyaku_name.to_owned(),
        compiler_fingerprint: compiler_fingerprint.to_owned(),
        abi_hash: ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1),
        features_bitmap: 0,
        access_set_hints: None,
        permissions: Vec::new(),
        events: Vec::new(),
        kotoba: Vec::new(),
        entrypoints: vec![ivm::EmbeddedEntrypointDescriptor {
            name: "main".to_owned(),
            kind: iroha::data_model::smart_contract::manifest::EntryPointKind::View,
            params: Vec::new(),
            argument_schema: None,
            return_type: Some("()".to_owned()),
            return_schema: Some(iroha_data_model::smart_contract::entrypoint::EntrypointValueTypeV1 {
                nodes: vec![iroha_data_model::smart_contract::entrypoint::EntrypointValueTypeNodeV1::Unit],
            }),
            authorization: iroha_data_model::smart_contract::manifest::EntrypointAuthorizationV1::Anyone,
            read_keys: Vec::new(),
            write_keys: Vec::new(),
            access_hints_complete: Some(true),
            access_hints_skipped: Vec::new(),
            triggers: Vec::new(),
            entry_pc: 0,
        }],
        error_types: Vec::new(),
        enum_types: Vec::new(),
        error_messages: Vec::new(),
        states: Vec::new(),
    };
    let mut artifact = metadata.encode();
    artifact.extend_from_slice(&interface.encode_section());
    // Return Unit through the caller-owned result table required by this ABI.
    for word in [
        enc::encode_store(wide::memory::STORE64, 12, 0, 0),
        enc::encode_ri(wide::arithmetic::ADDI, 10, 12, 0),
        enc::encode_ri(wide::arithmetic::ADDI, 11, 0, 1),
        enc::encode_rr(wide::control::JALR, 0, 1, 0),
    ] {
        artifact.extend_from_slice(&word.to_le_bytes());
    }
    artifact
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_exact_query_capacity_backpressure_is_retried() {
        assert!(is_query_capacity_backpressure(
            "failed to quote transaction fees; status: 429 Too Many Requests; response body: \
             query_validation_failed: Reached the limit of parallel queries. Either wait"
        ));
        assert!(!is_query_capacity_backpressure(
            "status: 429 Too Many Requests; response body: rate limited"
        ));
        assert!(!is_query_capacity_backpressure(
            "status: 400 Bad Request; Reached the limit of parallel queries"
        ));
    }

    #[test]
    fn parliament_artifacts_bind_identity_and_current_abi_before_submission() {
        let original = minimal_contract_artifact_with_identity("CommitteePulse", "native-tests");
        let verified =
            ivm::verify_contract_artifact(&original).expect("canonical fixture artifact");
        assert_eq!(verified.metadata.abi_version, 1);
        assert_eq!(
            verified.contract_interface.callables,
            [ivm::call::EmbeddedCallableV1 {
                entry_pc: 0,
                frame_bytes: 0,
                arguments: ivm::call::CallSchemaV1::empty(),
                results: ivm::call::CallSchemaV1::unit(),
            }],
        );
        assert_eq!(verified.manifest.code_hash, Some(verified.code_hash));
        assert_eq!(
            verified.abi_hash.as_ref(),
            &ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1)
        );
        assert_eq!(verified.contract_interface.seiyaku_name, "CommitteePulse");
        assert_eq!(
            verified.contract_interface.compiler_fingerprint,
            "native-tests"
        );
        let other = ivm::verify_contract_artifact(&minimal_contract_artifact_with_identity(
            "OtherPulse",
            "native-tests",
        ))
        .expect("distinct canonical artifact");
        assert_ne!(verified.code_hash, other.code_hash);
        let mut truncated = original;
        truncated.pop();
        assert!(ivm::verify_contract_artifact(&truncated).is_err());
        assert!(ivm::verify_contract_artifact(&minimal_contract_artifact()).is_ok());
    }
}
