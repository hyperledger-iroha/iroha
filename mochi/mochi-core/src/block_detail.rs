//! Readable projection of one committed block for the desktop's block drill-down.
//!
//! Every row comes from the decoded [`SignedBlock`] that the observed validator streamed, and
//! the canonical JSON and wire frame stay available for copying. The projection re-validates
//! nothing: signatures, finality and execution belong to the stream and to Core.

use crate::torii::ToriiError;
use iroha::client::AccountClient;
use iroha_data_model::{
    block::{
        SignedBlock,
        execution_output::{ExecutionOutputV1, PipelineExecutionOutputV2, TimeExecutionOutputV2},
    },
    isi::{
        BurnBox, CustomInstruction, ExecuteTrigger, GrantBox, Instruction, InstructionBox, Log,
        MintBox, RegisterBox, RemoveAssetKeyValue, RemoveKeyValueBox, RevokeBox, SetAssetKeyValue,
        SetKeyValueBox, SetParameter, TransferAssetBatch, TransferBox, UnregisterBox, Upgrade,
    },
    transaction::{
        Executable, ExecutableBatchItem, SignedTransaction, TransactionDomain,
        TransactionEntrypoint, TransactionResult,
    },
};
use norito::json;
use std::{
    fmt::Write as _,
    num::{NonZeroU32, NonZeroU64},
    panic::{AssertUnwindSafe, catch_unwind},
    time::Duration,
};
use tokio_stream::StreamExt as _;

/// Longest wait for a requested block before reporting that it is not committed.
const FETCH_TIMEOUT: Duration = Duration::from_secs(15);

/// One header commitment; `value` is `None` when the block carries none.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockField {
    /// Caption shown beside the value.
    pub label: &'static str,
    /// Committed value as lowercase hex or compact JSON.
    pub value: Option<String>,
}

/// One validator signature over the block header.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockSignatureRow {
    /// Validator index in the block's topology.
    pub validator: u64,
    /// Signature bytes as lowercase hex.
    pub signature: String,
}

/// Execution result recorded for one network input or internal invocation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Applied, with what the result carries besides the root execution.
    Applied {
        /// Data-trigger steps executed by the input.
        trigger_steps: usize,
        /// Independently settled transfer-batch receipts.
        transfer_receipts: usize,
        /// Whether a Nexus fee charge committed.
        fee_receipt: bool,
    },
    /// Rejected; nothing the input did remains applied.
    Rejected {
        /// The reason followed by each of its causes, outermost first.
        reason: String,
        /// The complete structured reason.
        detail: String,
    },
    /// The block carries no result for this entry, as for a proposal.
    Missing,
}

impl Outcome {
    fn of(result: &TransactionResult) -> Self {
        match result.as_ref() {
            Ok(steps) => Self::Applied {
                trigger_steps: steps.len(),
                transfer_receipts: result.batch_transfer_outcomes().len(),
                fee_receipt: result.nexus_fee_receipt().is_some(),
            },
            Err(reason) => Self::Rejected {
                reason: error_chain(reason),
                detail: format!("{reason:#?}"),
            },
        }
    }

    /// Whether the entry was rejected.
    #[must_use]
    pub fn is_rejected(&self) -> bool {
        matches!(self, Self::Rejected { .. })
    }
}

/// One instruction of a transaction, or one item of a mixed batch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstructionRow {
    /// Stable wire identifier, or the type name of an unregistered instruction.
    pub name: String,
    /// One-line form, for common instructions.
    pub summary: Option<String>,
    /// Complete structured form, for common instructions.
    pub detail: Option<String>,
}

/// Kind of network input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntrypointKind {
    /// Signed transaction.
    Transaction,
    /// Commitment to a sealed transaction, executed after its reveal.
    SealedCommitment,
    /// Reveal of a previously committed sealed transaction.
    SealedReveal,
    /// Signerless anonymous ballot.
    AnonymousBallot,
}

impl EntrypointKind {
    /// Human-readable label.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Transaction => "Transaction",
            Self::SealedCommitment => "Sealed commitment",
            Self::SealedReveal => "Sealed reveal",
            Self::AnonymousBallot => "Anonymous ballot",
        }
    }
}

/// One network input of the block with its execution result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransactionRow {
    /// Canonical position among the block's network inputs.
    pub index: usize,
    /// Canonical entrypoint hash; for a transaction it is the submitted transaction hash.
    pub hash: String,
    /// Kind of input.
    pub kind: EntrypointKind,
    /// Hash of the signed transaction that a sealed reveal executes.
    pub inner_hash: Option<String>,
    /// Account that authorized the input; ballots have none.
    pub authority: Option<String>,
    /// Signed creation time in Unix milliseconds.
    pub created_ms: Option<u64>,
    /// Signed lifetime in milliseconds.
    pub time_to_live_ms: Option<u64>,
    /// Signed nonce.
    pub nonce: Option<u32>,
    /// Network the signature binds, or `genesis`.
    pub domain: Option<String>,
    /// Executable form, such as `2 instructions` or `IVM bytecode · 812 bytes`.
    pub executable: String,
    /// Instructions or batch items in execution order.
    pub instructions: Vec<InstructionRow>,
    /// Metadata entries with compact JSON values.
    pub metadata: Vec<(String, String)>,
    /// Signed fee payment intent as compact JSON.
    pub fee_payment: Option<String>,
    /// Authority signature as lowercase hex.
    pub signature: Option<String>,
    /// Signatures in the authority's multisig bundle.
    pub multisig_signatures: usize,
    /// Position of this input's result among the block outputs.
    pub output_index: Option<usize>,
    /// Recorded execution result.
    pub outcome: Outcome,
}

impl TransactionRow {
    /// Whether `needle`, already lowercased, occurs in an identifying field.
    #[must_use]
    pub fn matches(&self, needle: &str) -> bool {
        needle.is_empty()
            || [
                Some(self.hash.as_str()),
                self.inner_hash.as_deref(),
                self.authority.as_deref(),
                Some(self.executable.as_str()),
            ]
            .into_iter()
            .flatten()
            .chain(
                self.instructions.iter().flat_map(|row| {
                    std::iter::once(row.name.as_str()).chain(row.summary.as_deref())
                }),
            )
            .any(|field| field.to_lowercase().contains(needle))
    }
}

/// One internal invocation, a time or pipeline trigger, executed by the block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InvocationRow {
    /// Position among the block outputs.
    pub output_index: usize,
    /// `Time trigger` or `Pipeline trigger`.
    pub kind: &'static str,
    /// Invoked trigger.
    pub trigger: String,
    /// Recorded execution result.
    pub outcome: Outcome,
}

/// Everything the drill-down shows for one block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockDetail {
    /// Block height (`1`-indexed).
    pub height: u64,
    /// Block hash as lowercase hex.
    pub hash: String,
    /// Previous block hash; genesis has none.
    pub previous_hash: Option<String>,
    /// Creation time in Unix milliseconds.
    pub created_ms: u64,
    /// Sumeragi view change index.
    pub view_change_index: u64,
    /// Whether this is the genesis block.
    pub genesis: bool,
    /// Length of the received wire frame.
    pub bytes: usize,
    /// Header commitments to the block payload.
    pub commitments: Vec<BlockField>,
    /// Validator signatures over the header.
    pub signatures: Vec<BlockSignatureRow>,
    /// Network inputs in execution order.
    pub transactions: Vec<TransactionRow>,
    /// Time and pipeline trigger invocations.
    pub invocations: Vec<InvocationRow>,
    /// Whether the block carries execution results.
    pub has_results: bool,
    /// Successful execution fragments recorded with the results.
    pub committed_fragments: Option<u64>,
    /// Parts of the attached Sumeragi commit certificate as lowercase hex.
    pub commit_certificate: Option<Vec<BlockField>>,
}

impl BlockDetail {
    /// Project `block`, received as a `bytes`-long wire frame.
    #[must_use]
    pub fn from_block(block: &SignedBlock, bytes: usize) -> Self {
        let header = block.header();
        let commitments = vec![
            BlockField {
                label: "Transactions Merkle root",
                value: header.merkle_root().map(|hash| hash.to_string()),
            },
            BlockField {
                label: "Execution context",
                value: header.execution_context_hash().map(|hash| hash.to_string()),
            },
            BlockField {
                label: "DA commitments",
                value: header.da_commitments_hash().map(|hash| hash.to_string()),
            },
            BlockField {
                label: "DA proof policies",
                value: header.da_proof_policies_hash().map(|hash| hash.to_string()),
            },
            BlockField {
                label: "DA pin intents",
                value: header.da_pin_intents_hash().map(|hash| hash.to_string()),
            },
            BlockField {
                label: "NPoS effects",
                value: header.npos_effects_hash().map(|hash| hash.to_string()),
            },
            BlockField {
                label: "Beacon pulse",
                value: header
                    .global_beacon_pulse_hash()
                    .map(|hash| hash.to_string()),
            },
            BlockField {
                label: "Confidential features",
                value: header
                    .confidential_features()
                    .and_then(|digest| compact(&digest)),
            },
        ];
        let signatures = block
            .signatures()
            .map(|signature| BlockSignatureRow {
                validator: signature.index(),
                signature: lower_hex(signature.signature().payload()),
            })
            .collect();
        let transactions = block
            .network_entrypoints()
            .enumerate()
            .map(|(index, entrypoint)| transaction_row(block, index, entrypoint))
            .collect();
        let invocations = block
            .execution_outputs()
            .iter()
            .enumerate()
            .filter_map(|(output_index, output)| match output {
                ExecutionOutputV1::Network(_) | ExecutionOutputV1::NetworkWithLifecycle(_) => None,
                ExecutionOutputV1::Pipeline(row)
                | ExecutionOutputV1::PipelineWithLifecycle(PipelineExecutionOutputV2 {
                    execution: row,
                    ..
                }) => Some(InvocationRow {
                    output_index,
                    kind: "Pipeline trigger",
                    trigger: row.invocation.trigger.trigger_id.to_string(),
                    outcome: Outcome::of(&row.result),
                }),
                ExecutionOutputV1::Time(row)
                | ExecutionOutputV1::TimeWithLifecycle(TimeExecutionOutputV2 {
                    execution: row,
                    ..
                }) => Some(InvocationRow {
                    output_index,
                    kind: "Time trigger",
                    trigger: row.invocation.trigger.trigger_id.to_string(),
                    outcome: Outcome::of(&row.result),
                }),
            })
            .collect();
        Self {
            height: header.height().get(),
            hash: block.hash().to_string(),
            previous_hash: header.prev_block_hash().map(|hash| hash.to_string()),
            created_ms: header
                .creation_time()
                .as_millis()
                .try_into()
                .unwrap_or(u64::MAX),
            view_change_index: header.view_change_index(),
            genesis: header.is_genesis(),
            bytes,
            commitments,
            signatures,
            transactions,
            invocations,
            has_results: block.has_results(),
            committed_fragments: block.committed_fragment_count(),
            commit_certificate: block.commit_certificate().map(|certificate| {
                [
                    ("Consensus header", certificate.consensus_header()),
                    ("CommitQC", certificate.commit_qc()),
                    ("Result preimage", certificate.result_preimage()),
                    ("Availability", certificate.availability()),
                ]
                .into_iter()
                .map(|(label, bytes)| BlockField {
                    label,
                    value: (!bytes.is_empty()).then(|| lower_hex(bytes)),
                })
                .collect()
            }),
        }
    }

    /// Network inputs whose execution was rejected.
    #[must_use]
    pub fn rejected(&self) -> usize {
        self.transactions
            .iter()
            .filter(|row| row.outcome.is_rejected())
            .count()
    }
}

fn transaction_row(
    block: &SignedBlock,
    index: usize,
    entrypoint: &TransactionEntrypoint,
) -> TransactionRow {
    let (kind, signed) = match entrypoint {
        TransactionEntrypoint::External(transaction) => {
            (EntrypointKind::Transaction, Some(transaction))
        }
        TransactionEntrypoint::SealedReveal(reveal) => (
            EntrypointKind::SealedReveal,
            Some(reveal.signed_transaction()),
        ),
        TransactionEntrypoint::SealedCommitment(_) => (EntrypointKind::SealedCommitment, None),
        TransactionEntrypoint::AnonymousBallot(_) => (EntrypointKind::AnonymousBallot, None),
    };
    let output = u32::try_from(index)
        .ok()
        .and_then(|input| block.network_output_at(input));
    let outcome = output.map_or(Outcome::Missing, |(_, row)| Outcome::of(&row.result));
    let (executable, instructions) = signed.map_or_else(
        || (String::new(), Vec::new()),
        |tx| executable(tx.instructions()),
    );
    TransactionRow {
        index,
        hash: entrypoint.hash().to_string(),
        kind,
        inner_hash: (kind == EntrypointKind::SealedReveal)
            .then(|| signed.map(|tx| tx.hash().to_string()))
            .flatten(),
        authority: entrypoint.authority_opt().map(ToString::to_string),
        created_ms: entrypoint.creation_time_ms(),
        time_to_live_ms: signed
            .and_then(SignedTransaction::time_to_live)
            .map(|ttl| ttl.as_millis().try_into().unwrap_or(u64::MAX)),
        nonce: signed
            .and_then(SignedTransaction::nonce)
            .map(NonZeroU32::get),
        domain: signed.map(|tx| match tx.payload().domain {
            TransactionDomain::Network(network) => network.to_string(),
            TransactionDomain::Genesis => "genesis".to_owned(),
        }),
        executable,
        instructions,
        metadata: entrypoint
            .metadata()
            .map(|metadata| {
                metadata
                    .iter()
                    .map(|(key, value)| (key.to_string(), compact(value).unwrap_or_default()))
                    .collect()
            })
            .unwrap_or_default(),
        fee_payment: signed.and_then(|tx| compact(tx.fee_payment_intent())),
        signature: signed.map(|tx| lower_hex(tx.signature().0.payload())),
        multisig_signatures: signed
            .and_then(SignedTransaction::multisig_signatures)
            .map_or(0, |bundle| bundle.signatures.len()),
        // A network input's result is the output at its own position.
        output_index: output.is_some().then_some(index),
        outcome,
    }
}

fn executable(executable: &Executable) -> (String, Vec<InstructionRow>) {
    match executable {
        Executable::Instructions(instructions) => (
            count(instructions.len(), "instruction"),
            instructions.iter().map(instruction_row).collect(),
        ),
        Executable::ContractCall(call) => (
            format!(
                "Contract call {}::{}",
                call.contract_address, call.entrypoint
            ),
            Vec::new(),
        ),
        Executable::Ivm(bytecode) => (
            format!("IVM bytecode · {} bytes", bytecode.size_bytes()),
            Vec::new(),
        ),
        Executable::IvmProved(proved) => (
            format!(
                "Proved IVM bytecode · {} bytes · {}",
                proved.bytecode.size_bytes(),
                count(proved.overlay.len(), "overlay instruction")
            ),
            proved.overlay.iter().map(instruction_row).collect(),
        ),
        Executable::Batch(items) => (
            format!("Batch of {}", count(items.len(), "item")),
            items
                .iter()
                .map(|item| match item {
                    ExecutableBatchItem::Instruction(instruction) => instruction_row(instruction),
                    ExecutableBatchItem::ContractCall(call) => InstructionRow {
                        name: "contract call".to_owned(),
                        summary: Some(format!("{}::{}", call.contract_address, call.entrypoint)),
                        detail: Some(format!("{call:#?}")),
                    },
                })
                .collect(),
        ),
    }
}

fn instruction_row(instruction: &InstructionBox) -> InstructionRow {
    let any = (**instruction).as_any();
    // Common instructions render readably; the rest stay available in the copied JSON.
    macro_rules! readable {
        (display: $($shown:ty),+; debug: $($debugged:ty),+ $(,)?) => {
            $(if let Some(value) = any.downcast_ref::<$shown>() {
                (Some(value.to_string()), Some(format!("{value:#?}")))
            } else)+ $(if let Some(value) = any.downcast_ref::<$debugged>() {
                (Some(format!("{value:?}")), Some(format!("{value:#?}")))
            } else)+ {
                (None, None)
            }
        };
    }
    let (summary, detail) = readable!(
        display: RegisterBox, UnregisterBox, MintBox, BurnBox, TransferBox, SetKeyValueBox,
            RemoveKeyValueBox, GrantBox, RevokeBox, ExecuteTrigger, SetParameter, Log, Upgrade,
            CustomInstruction;
        debug: TransferAssetBatch, SetAssetKeyValue, RemoveAssetKeyValue,
    );
    InstructionRow {
        name: iroha_data_model::isi::instruction_wire_id(instruction).map_or_else(
            || short_type_name(Instruction::id(&**instruction)),
            str::to_owned,
        ),
        summary,
        detail,
    }
}

/// `Mint` for `iroha_data_model::isi::mint_burn::Mint<u32, Trigger>`.
fn short_type_name(path: &str) -> String {
    let path = path.split('<').next().unwrap_or(path);
    path.rsplit("::").next().unwrap_or(path).to_owned()
}

fn count(n: usize, noun: &str) -> String {
    if n == 1 {
        format!("1 {noun}")
    } else {
        format!("{n} {noun}s")
    }
}

/// The reason followed by each distinct cause, as `reason: cause: cause`.
fn error_chain(error: &(dyn std::error::Error + 'static)) -> String {
    let mut parts = vec![error.to_string()];
    let mut source = error.source();
    while let Some(cause) = source {
        let text = cause.to_string();
        if parts.last() != Some(&text) {
            parts.push(text);
        }
        source = cause.source();
    }
    parts.join(": ")
}

fn lower_hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut out, byte| {
            let _ = write!(out, "{byte:02x}");
            out
        })
}

// Serializers of untrusted blocks are guarded like the state explorer's.
fn compact<T: json::JsonSerialize + ?Sized>(value: &T) -> Option<String> {
    catch_unwind(AssertUnwindSafe(|| json::to_json(value)))
        .ok()
        .and_then(Result::ok)
}

fn pretty<T: json::JsonSerialize + ?Sized>(value: &T) -> Option<String> {
    catch_unwind(AssertUnwindSafe(|| json::to_json_pretty(value)))
        .ok()
        .and_then(Result::ok)
}

/// The complete block as pretty canonical JSON.
#[must_use]
pub fn block_json(block: &SignedBlock) -> Option<String> {
    pretty(block)
}

/// One network input as pretty canonical JSON.
#[must_use]
pub fn input_json(block: &SignedBlock, index: usize) -> Option<String> {
    pretty(block.network_entrypoint_at(index)?)
}

/// One typed execution output as pretty canonical JSON.
#[must_use]
pub fn output_json(block: &SignedBlock, index: usize) -> Option<String> {
    pretty(block.execution_outputs().get(index)?)
}

/// The canonical `SignedBlockWire` frame as lowercase hex.
#[must_use]
pub fn block_wire_hex(block: &SignedBlock) -> Option<String> {
    block.encode_wire().ok().map(|wire| lower_hex(&wire))
}

/// `YYYY-MM-DD HH:MM:SS UTC` for a Unix time in milliseconds.
#[must_use]
pub fn format_utc_ms(ms: u64) -> String {
    let seconds = ms / 1_000;
    let (days, time) = (seconds / 86_400, seconds % 86_400);
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let shifted = i64::try_from(days).unwrap_or(i64::MAX / 4) + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02} UTC",
        time / 3_600,
        time / 60 % 60,
        time % 60
    )
}

/// A block read on demand from the observed validator.
#[derive(Debug)]
pub struct FetchedBlock {
    /// The decoded canonical block.
    pub block: SignedBlock,
    /// Length of the received wire frame.
    pub bytes: usize,
}

/// Why an on-demand block read failed.
#[derive(Debug, thiserror::Error)]
pub enum BlockFetchError {
    /// Subscribing, transport or decoding failed.
    #[error(transparent)]
    Torii(#[from] ToriiError),
    /// The validator streamed nothing for the height before the deadline.
    #[error("the validator has not committed block #{0}")]
    NotCommitted(u64),
    /// The validator closed the stream before sending the block.
    #[error("the validator closed the block stream before block #{0}")]
    Closed(u64),
    /// The stream started at another height.
    #[error("the validator returned block #{received} for block #{requested}")]
    Mismatch {
        /// Requested height.
        requested: u64,
        /// Height of the first streamed block.
        received: u64,
    },
}

/// Read one committed block from the observed validator's canonical block stream.
///
/// The subscription starts at `height` and its first block must have that height. A height the
/// validator has not committed fails after a bounded wait instead of waiting for it.
///
/// # Errors
/// Subscription, transport and decoding failures, an early close, the deadline, or another
/// height.
pub async fn fetch_block(
    reader: &AccountClient,
    height: NonZeroU64,
) -> Result<FetchedBlock, BlockFetchError> {
    let mut stream = reader
        .blocks()
        .subscribe(height)
        .await
        .map_err(ToriiError::from)?;
    let first = tokio::time::timeout(FETCH_TIMEOUT, stream.next()).await;
    let bytes = stream.last_message_bytes().unwrap_or_default();
    // Release the connection whatever arrived; a close failure does not hide the block.
    let _ = stream.close().await;
    let block = match first {
        Err(_) => return Err(BlockFetchError::NotCommitted(height.get())),
        Ok(None) => return Err(BlockFetchError::Closed(height.get())),
        Ok(Some(result)) => result.map_err(ToriiError::from)?,
    };
    let received = block.header().height().get();
    if received != height.get() {
        return Err(BlockFetchError::Mismatch {
            requested: height.get(),
            received,
        });
    }
    Ok(FetchedBlock { block, bytes })
}

/// A signed genesis block whose one `Log` transaction recorded `result`, followed by `extra`
/// outputs, for desktop tests.
///
/// # Panics
/// When `extra` does not form a valid output sequence after the transaction's result.
#[cfg(any(test, feature = "test"))]
#[must_use]
pub fn sample_block(
    result: iroha_data_model::transaction::TransactionResultInner,
    extra: Vec<ExecutionOutputV1>,
) -> SignedBlock {
    use iroha_crypto::SignatureOf;
    use iroha_data_model::{
        Level,
        block::{
            BlockSignature, BlockSignatures, execution_output::NetworkExecutionOutputV1,
            output_budget::ExecutionOutputLimits,
        },
        transaction::{FeePaymentIntent, TransactionBuilder},
    };
    use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR, PEER_KEYPAIR};

    const LIMITS: ExecutionOutputLimits = ExecutionOutputLimits {
        max_outputs: 4,
        max_output_bytes: 64 * 1024,
        max_total_output_bytes: 512 * 1024,
        max_executed_wire_bytes: 1024 * 1024,
    };
    let mut builder = TransactionBuilder::new_genesis(
        ALICE_ID.clone(),
        FeePaymentIntent::authority(Vec::new(), None),
    );
    builder.set_creation_time(Duration::from_secs(42));
    let transaction = builder
        .with_instructions([InstructionBox::from(Log::new(
            Level::INFO,
            "hello block".to_owned(),
        ))])
        .sign(ALICE_KEYPAIR.private_key());
    let mut block = SignedBlock::genesis(vec![transaction], PEER_KEYPAIR.private_key(), None, None);
    let fragments = u64::from(result.is_ok());
    let mut outputs = vec![ExecutionOutputV1::Network(NetworkExecutionOutputV1 {
        input_index: 0,
        result: result.into(),
        completions: Vec::new(),
    })];
    outputs.extend(extra);
    block
        .set_execution_outputs(
            outputs,
            fragments,
            Default::default(),
            Vec::new(),
            Default::default(),
            Default::default(),
            &LIMITS,
        )
        .expect("aligned sample outputs");
    let signature = BlockSignature::new(
        0,
        SignatureOf::try_from_hash(PEER_KEYPAIR.private_key(), block.hash())
            .expect("sign the result-bearing block"),
    );
    block
        .replace_signatures(
            BlockSignatures::try_from_iter([signature]).expect("one final signature"),
        )
        .expect("replace the proposal signature");
    block
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::Hash;
    use iroha_data_model::{
        block::execution_output::{TimeInvocationV1, TriggerUseV1},
        events::time::{TimeEvent, TimeInterval},
        transaction::{
            DataTriggerSequence,
            error::{TransactionLimitError, TransactionRejectionReason},
        },
    };
    use iroha_test_samples::ALICE_ID;

    fn applied() -> SignedBlock {
        sample_block(Ok(DataTriggerSequence::default()), Vec::new())
    }

    #[test]
    fn applied_block_projects_its_header_signatures_and_transaction() {
        let block = applied();
        let detail = BlockDetail::from_block(&block, 1_234);
        assert_eq!(detail.height, 1);
        assert!(detail.genesis);
        assert_eq!(detail.previous_hash, None);
        assert_eq!(detail.hash, block.hash().to_string());
        assert_eq!(detail.bytes, 1_234);
        assert!(detail.has_results);
        assert_eq!(detail.committed_fragments, Some(1));
        assert_eq!(detail.commit_certificate, None);
        assert_eq!(detail.rejected(), 0);
        let merkle = detail
            .commitments
            .iter()
            .find(|field| field.label == "Transactions Merkle root")
            .expect("Merkle root row");
        assert_eq!(
            merkle.value,
            block.header().merkle_root().map(|root| root.to_string())
        );
        let [signature] = detail.signatures.as_slice() else {
            panic!("one validator signature: {:?}", detail.signatures);
        };
        assert_eq!(signature.validator, 0);
        assert_eq!(
            signature.signature,
            lower_hex(block.signatures().next().unwrap().signature().payload())
        );
        assert!(
            signature
                .signature
                .chars()
                .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
        );

        let [row] = detail.transactions.as_slice() else {
            panic!("one transaction: {:?}", detail.transactions);
        };
        let transaction = block.external_transactions().next().unwrap();
        assert_eq!(
            row.hash,
            transaction.hash().to_string(),
            "the hash a submitter keeps"
        );
        assert_eq!(row.kind, EntrypointKind::Transaction);
        assert_eq!(row.inner_hash, None);
        assert_eq!(row.authority, Some(ALICE_ID.to_string()));
        assert_eq!(row.created_ms, Some(42_000));
        assert_eq!(row.domain.as_deref(), Some("genesis"));
        assert_eq!(row.executable, "1 instruction");
        assert_eq!(
            row.signature,
            Some(lower_hex(transaction.signature().0.payload()))
        );
        assert_eq!(row.output_index, Some(0));
        assert_eq!(
            row.outcome,
            Outcome::Applied {
                trigger_steps: 0,
                transfer_receipts: 0,
                fee_receipt: false,
            }
        );
        let [instruction] = row.instructions.as_slice() else {
            panic!("one instruction: {:?}", row.instructions);
        };
        assert_eq!(instruction.name, "iroha.log");
        assert_eq!(
            instruction.summary.as_deref(),
            Some("LOG(INFO): hello block")
        );
        assert!(
            instruction
                .detail
                .as_deref()
                .unwrap()
                .contains("hello block")
        );
        assert!(detail.invocations.is_empty());
    }

    #[test]
    fn rejected_transactions_keep_their_reason_chain_and_structure() {
        let rejection = TransactionRejectionReason::LimitCheck(TransactionLimitError {
            reason: "too many instructions".to_owned(),
        });
        let block = sample_block(Err(rejection), Vec::new());
        let detail = BlockDetail::from_block(&block, 0);
        assert_eq!(detail.rejected(), 1);
        let Outcome::Rejected { reason, detail } = &detail.transactions[0].outcome else {
            panic!("rejected outcome: {:?}", detail.transactions[0].outcome);
        };
        assert_eq!(
            reason,
            "Failed to validate transaction limits: too many instructions"
        );
        assert!(detail.contains("LimitCheck"), "{detail}");
    }

    #[test]
    fn scheduled_invocations_are_listed_apart_from_transactions() {
        use iroha_data_model::block::execution_output::{
            PipelineEventPositionV1, PipelineInvocationV1, TriggerLifecycleEffectsV1,
        };
        let scheduled = ExecutionOutputV1::time_output_limit_rejection(TimeInvocationV1 {
            schedule_index: 0,
            event: TimeEvent {
                interval: TimeInterval {
                    since_ms: 41_999,
                    length_ms: 1,
                },
            },
            trigger: TriggerUseV1 {
                trigger_id: "nightly_report".parse().unwrap(),
                registered_at_height: 0,
                action_hash: Hash::new(b"nightly report action"),
            },
        });
        let pipeline = ExecutionOutputV1::pipeline_output_limit_rejection(PipelineInvocationV1 {
            event: PipelineEventPositionV1::BlockApproved,
            candidate_index: 0,
            trigger: TriggerUseV1 {
                trigger_id: "approved_report".parse().unwrap(),
                registered_at_height: 0,
                action_hash: Hash::new(b"approved report action"),
            },
        });
        for lifecycle in [false, true] {
            let mut outputs = vec![pipeline.clone(), scheduled.clone()];
            if lifecycle {
                outputs = outputs
                    .into_iter()
                    .map(|output| {
                        let trigger_lifecycle = TriggerLifecycleEffectsV1::Complete {
                            effects: Vec::new(),
                        };
                        match output {
                            ExecutionOutputV1::Pipeline(execution) => {
                                ExecutionOutputV1::PipelineWithLifecycle(
                                    PipelineExecutionOutputV2 {
                                        execution,
                                        trigger_lifecycle,
                                    },
                                )
                            }
                            ExecutionOutputV1::Time(execution) => {
                                ExecutionOutputV1::TimeWithLifecycle(TimeExecutionOutputV2 {
                                    execution,
                                    trigger_lifecycle,
                                })
                            }
                            _ => unreachable!("fixture contains pipeline and time owners"),
                        }
                    })
                    .collect();
            }
            let block = sample_block(Ok(DataTriggerSequence::default()), outputs.clone());
            let original_hash = block.hash();
            let detail = BlockDetail::from_block(&block, 0);
            assert_eq!(detail.hash, original_hash.to_string());
            assert_eq!(
                block.hash(),
                original_hash,
                "projection must not rewrite the block"
            );
            assert_eq!(detail.transactions.len(), 1);
            assert_eq!(detail.rejected(), 0, "trigger runs are not transactions");
            assert_eq!(detail.invocations.len(), 2);
            for (index, (kind, trigger)) in [
                ("Pipeline trigger", "approved_report"),
                ("Time trigger", "nightly_report"),
            ]
            .into_iter()
            .enumerate()
            {
                let run = &detail.invocations[index];
                assert_eq!(run.output_index, index + 1);
                assert_eq!(run.kind, kind);
                assert_eq!(run.trigger, trigger);
                assert!(
                    run.outcome.is_rejected(),
                    "an output-limit terminal rejects"
                );
                assert_eq!(
                    output_json(&block, run.output_index),
                    pretty(&outputs[index])
                );
                if lifecycle {
                    assert!(
                        output_json(&block, run.output_index)
                            .unwrap()
                            .contains("trigger_lifecycle")
                    );
                }
            }
        }
    }

    #[test]
    fn canonical_payloads_cover_the_block_and_each_entry() {
        let block = applied();
        assert!(
            block_json(&block)
                .expect("block JSON")
                .contains("\"signatures\"")
        );
        assert!(
            input_json(&block, 0)
                .expect("input JSON")
                .contains("\"External\"")
        );
        assert!(
            output_json(&block, 0)
                .expect("output JSON")
                .contains("\"Ok\"")
        );
        assert_eq!(input_json(&block, 1), None);
        assert_eq!(output_json(&block, 1), None);
        assert_eq!(
            block_wire_hex(&block),
            Some(lower_hex(&block.encode_wire().unwrap()))
        );
    }

    #[test]
    fn transaction_filter_matches_identifiers_and_instructions() {
        let detail = BlockDetail::from_block(&applied(), 0);
        let row = &detail.transactions[0];
        assert!(row.matches(""));
        assert!(row.matches(&row.hash[..12]));
        assert!(row.matches("iroha.log"));
        assert!(row.matches("hello block"));
        assert!(!row.matches("absent transaction"));
    }

    #[test]
    fn utc_times_follow_the_civil_calendar() {
        assert_eq!(format_utc_ms(0), "1970-01-01 00:00:00 UTC");
        assert_eq!(format_utc_ms(951_782_400_000), "2000-02-29 00:00:00 UTC");
        assert_eq!(format_utc_ms(1_700_000_000_999), "2023-11-14 22:13:20 UTC");
        assert_eq!(format_utc_ms(4_102_444_799_000), "2099-12-31 23:59:59 UTC");
    }

    #[test]
    fn unregistered_instructions_are_named_by_their_type() {
        assert_eq!(
            short_type_name("iroha_data_model::isi::mint_burn::Mint<u32, iroha::Trigger>"),
            "Mint"
        );
        assert_eq!(short_type_name("Plain"), "Plain");
    }

    #[test]
    fn error_chains_name_each_distinct_cause_once() {
        #[derive(Debug)]
        struct Layer(&'static str, Option<Box<Layer>>);
        impl std::fmt::Display for Layer {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.0)
            }
        }
        impl std::error::Error for Layer {
            fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
                self.1
                    .as_deref()
                    .map(|layer| layer as &(dyn std::error::Error + 'static))
            }
        }
        let error = Layer(
            "Validation failed",
            Some(Box::new(Layer(
                "not permitted",
                Some(Box::new(Layer("not permitted", None))),
            ))),
        );
        assert_eq!(error_chain(&error), "Validation failed: not permitted");
    }
}
