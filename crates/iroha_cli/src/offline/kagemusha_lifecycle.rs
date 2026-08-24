//! Exact-payload, detached-multisignature Kagemusha V4 lifecycle corridor.

use crate::{Run, RunContext};
use clap::{Args as ClapArgs, Subcommand, ValueEnum};
use eyre::{Result, WrapErr as _, bail, eyre};
use iroha::{
    client::{
        Client, PreparedTransactionPayload, canonical_network_request_hash,
        canonical_request_witness_message,
    },
    data_model::{
        account::{AccountController, AccountId, MultisigPolicy},
        isi::{
            InstructionBox,
            offline::{
                ActivateKagemushaRecursiveReleaseV4, CancelKagemushaRecursiveReleaseV4,
                DeactivateKagemushaRecursiveIssuanceV4, EnableKagemushaRecursiveIssuanceV4,
            },
        },
        name::Name,
        offline::KAGEMUSHA_V4_ACTIVATION_FINALITY_PROOF_MAX_COUNT_V1,
        soracloud::{
            CANONICAL_REQUEST_WITNESS_VERSION_V1, CanonicalRequestSignatureWitnessV1,
            CanonicalRequestWitnessV1,
        },
        transaction::{
            Executable, SignedTransaction, TransactionAdmissionIntent, TransactionBuilder,
            TransactionPayload, TransactionSubmissionReceipt,
            signed::{MultisigSignature, MultisigSignatures},
        },
    },
    http::Method as HttpMethod,
};
use iroha_crypto::{HashOf, PublicKey, Signature};
use iroha_torii_shared::{FeeQuoteRequest, uri as torii_uri};
use iroha_version::codec::DecodeVersioned as _;
use norito::derive::{Decode, Encode};
use rand::{TryRngCore as _, rngs::OsRng};
use sha2::{Digest as _, Sha256};
use std::{
    any::TypeId,
    fs::{self, File, Metadata},
    io::{Read as _, Write as _},
    num::NonZeroU32,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use url::{Host, Url};

const LIFECYCLE_FEE_QUOTE_DRAFT_SCHEMA_V1: &str =
    "iroha.offline.kagemusha.lifecycle_fee_quote_draft.v1";
const LIFECYCLE_FEE_QUOTE_DRAFT_VERSION_V1: u16 = 1;
const LIFECYCLE_ARTIFACT_MAX_BYTES: usize = 64 * 1024 * 1024;
const LIFECYCLE_MAX_SIGNATURES: usize = 64;
const LIFECYCLE_FEE_QUOTE_MAX_CLOCK_SKEW_MS: u64 = 60_000;

/// Phase-separated lifecycle command.
#[derive(ClapArgs, Debug)]
pub(crate) struct Args {
    #[command(subcommand)]
    command: Command,
}

impl Args {
    pub(super) const fn allows_fallback_config(&self) -> bool {
        matches!(
            &self.command,
            Command::SignFeeQuote(_)
                | Command::SignTransaction(_)
                | Command::AssembleTransaction(_)
        )
    }

    pub(super) fn preflight_before_operator_key_load(&self) -> Result<()> {
        match &self.command {
            Command::SignFeeQuote(args) => args.validated_signing_input().map(drop),
            Command::SignTransaction(args) => args.validated_signing_input().map(drop),
            Command::Prepare(_)
            | Command::FinalizeFeeQuote(_)
            | Command::AssembleTransaction(_)
            | Command::SubmitTransaction(_) => Ok(()),
        }
    }
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Prepare the exact ordinary transaction and a 60-second authenticated fee-quote draft.
    Prepare(Prepare),
    /// Produce one detached member signature for the exact fee-quote request.
    SignFeeQuote(SignFeeQuote),
    /// Verify the fee-quote quorum, obtain the quote, and freeze the exact payload.
    FinalizeFeeQuote(FinalizeFeeQuote),
    /// Produce one detached member signature for the exact frozen payload.
    SignTransaction(SignTransaction),
    /// Verify and assemble at least two detached signatures into exact transaction wire.
    AssembleTransaction(AssembleTransaction),
    /// Submit the exact assembled wire without rebuilding or re-signing it.
    SubmitTransaction(SubmitTransaction),
}

impl Run for Args {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        reject_legacy_instruction_io(context)?;
        match self.command {
            Command::Prepare(args) => args.run(context),
            Command::SignFeeQuote(args) => args.run(context),
            Command::FinalizeFeeQuote(args) => args.run(context),
            Command::SignTransaction(args) => args.run(context),
            Command::AssembleTransaction(args) => args.run(context),
            Command::SubmitTransaction(args) => args.run(context),
        }
    }
}

/// Exact direct lifecycle transition carried by the archive.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum, Encode, Decode)]
enum LifecycleKind {
    /// Stage the governed release (`ActivateKagemushaRecursiveReleaseV4` on wire).
    Stage,
    /// Enable issuance after the staged release closes its canary gates.
    Enable,
    /// Cancel a staged release.
    Cancel,
    /// Deactivate issuance for an enabled release.
    Deactivate,
}

#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
struct LifecycleFeeQuoteDraftV1 {
    schema: String,
    version: u16,
    kind: LifecycleKind,
    fee_quote_url: String,
    payload: TransactionPayload,
    witness: CanonicalRequestWitnessV1,
}

#[derive(ClapArgs, Debug)]
struct Prepare {
    /// Required lifecycle kind; Stage maps to the native Activate instruction.
    #[arg(long, value_enum)]
    kind: LifecycleKind,
    /// Canonical I105 multisig account that authorizes the transition.
    #[arg(long, value_name = "I105_ACCOUNT")]
    governance_authority: String,
    /// Already-finalized anchor height; required only for Stage to bind its receipt horizon.
    #[arg(long, value_name = "HEIGHT")]
    trusted_anchor_height: Option<u64>,
    /// Kagami's exact JSON array containing one native lifecycle instruction.
    #[arg(long, value_name = "PATH")]
    instruction_json: PathBuf,
    /// Absent destination for the canonical fee-quote draft; never replaced.
    #[arg(long, value_name = "PATH")]
    output: PathBuf,
}

impl Prepare {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let authority = crate::resolve_account_id(context, &self.governance_authority)
            .wrap_err("invalid governance authority")?;
        require_multisig_policy(&authority)?;
        let instruction_bytes = read_bounded_stable(
            &self.instruction_json,
            LIFECYCLE_ARTIFACT_MAX_BYTES,
            "lifecycle instruction JSON",
        )?;
        let instruction_json = std::str::from_utf8(&instruction_bytes)
            .wrap_err("lifecycle instruction JSON is not UTF-8")?;
        let instructions: Vec<InstructionBox> = crate::parse_json(instruction_json)
            .wrap_err("failed to decode lifecycle instruction JSON")?;
        let [instruction] = instructions.as_slice() else {
            bail!("lifecycle instruction JSON must contain exactly one instruction");
        };
        require_lifecycle_instruction(instruction, self.kind)?;

        let client = context.client_from_config();
        let fee_payment = context.transaction_fee_payment()?;
        fee_payment
            .validate()
            .wrap_err("invalid requested fee-payment selection")?;
        let metadata = lifecycle_metadata_with_stage_expiry(
            context.transaction_metadata().cloned().unwrap_or_default(),
            self.kind,
            self.trusted_anchor_height,
        )?;
        let mut builder = TransactionBuilder::new(client.network_id, authority, fee_payment)
            .with_instructions(instructions)
            .with_admission_intent(TransactionAdmissionIntent::Ordinary)
            .with_metadata(metadata);
        if let Some(ttl) = client.transaction_ttl {
            builder.set_ttl(ttl);
        }
        if client.add_transaction_nonce {
            builder.set_nonce(random_nonzero_u32()?);
        }
        let payload = builder
            .into_payload()
            .wrap_err("failed to construct exact ordinary lifecycle payload")?;
        require_lifecycle_payload(&payload, self.kind)?;

        let fee_quote_url = fee_quote_url(&client)?;
        let body = fee_quote_body(&payload)?;
        let witness = CanonicalRequestWitnessV1 {
            schema_version: CANONICAL_REQUEST_WITNESS_VERSION_V1,
            subject_account: payload.authority.clone(),
            timestamp_ms: current_unix_ms()?,
            nonce: random_nonce()?,
            canonical_request_hash: canonical_network_request_hash(
                &client.network_id,
                &HttpMethod::POST,
                &fee_quote_url,
                &body,
            )?,
            signatures: Vec::new(),
        };
        let draft = LifecycleFeeQuoteDraftV1 {
            schema: LIFECYCLE_FEE_QUOTE_DRAFT_SCHEMA_V1.to_owned(),
            version: LIFECYCLE_FEE_QUOTE_DRAFT_VERSION_V1,
            kind: self.kind,
            fee_quote_url: fee_quote_url.to_string(),
            payload,
            witness,
        };
        validate_fee_quote_draft(&draft, self.kind)?;
        write_canonical_no_replace(&self.output, &draft, "lifecycle fee-quote draft")?;
        context.println(format_args!(
            "prepared exact {:?} fee-quote draft at {}",
            self.kind,
            self.output.display()
        ))
    }
}

#[derive(ClapArgs, Debug)]
struct SignFeeQuote {
    /// Lifecycle kind expected in the draft.
    #[arg(long, value_enum)]
    kind: LifecycleKind,
    /// Independently expected canonical I105 multisig authority.
    #[arg(long, value_name = "I105_ACCOUNT")]
    governance_authority: String,
    /// Independently expected NetworkId embedded in the draft; does not consult client config.
    #[arg(long, value_name = "NETWORK_ID")]
    expected_network_id: String,
    /// Independently expected lowercase or uppercase 64-hex SHA-256 of the exact draft file.
    #[arg(long, value_name = "64_HEX")]
    expected_draft_sha256: String,
    /// Exact canonical fee-quote draft; stale or excessively future-dated drafts are rejected.
    #[arg(long, value_name = "PATH")]
    draft: PathBuf,
    /// Absent destination for this signer's canonical detached signature.
    #[arg(long, value_name = "PATH")]
    output: PathBuf,
}

impl SignFeeQuote {
    fn validated_signing_input(&self) -> Result<LifecycleFeeQuoteDraftV1> {
        let draft_bytes = read_bounded_stable(
            &self.draft,
            LIFECYCLE_ARTIFACT_MAX_BYTES,
            "lifecycle fee-quote draft",
        )?;
        require_expected_artifact_sha256(
            &draft_bytes,
            &self.expected_draft_sha256,
            "--expected-draft-sha256",
            "lifecycle fee-quote draft",
        )?;
        let draft: LifecycleFeeQuoteDraftV1 =
            decode_canonical(&draft_bytes, "lifecycle fee-quote draft")?;
        require_expected_signing_network(
            &self.expected_network_id,
            &draft.payload,
            "lifecycle fee-quote draft",
        )?;
        validate_fee_quote_draft(&draft, self.kind)?;
        require_expected_authority(&self.governance_authority, &draft.payload)?;
        Ok(draft)
    }

    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let draft = self.validated_signing_input()?;
        let key = context.operator_key_pair().ok_or_else(|| {
            eyre!("--operator-private-key-file is required for detached lifecycle signing")
        })?;
        require_policy_member(&draft.payload, key.public_key())?;
        let message = canonical_request_witness_message(&draft.witness)?;
        let signature = Signature::try_new(key.private_key(), &message)
            .wrap_err("failed to sign exact lifecycle fee-quote witness")?;
        let artifact = CanonicalRequestSignatureWitnessV1 {
            signer: key.public_key().clone(),
            signature,
        };
        write_canonical_no_replace(
            &self.output,
            &artifact,
            "lifecycle fee-quote detached signature",
        )?;
        context.println(format_args!(
            "wrote detached fee-quote signature at {}",
            self.output.display()
        ))
    }
}

#[derive(ClapArgs, Debug)]
struct FinalizeFeeQuote {
    /// Lifecycle kind expected in the draft.
    #[arg(long, value_enum)]
    kind: LifecycleKind,
    /// Independently expected canonical I105 multisig authority.
    #[arg(long, value_name = "I105_ACCOUNT")]
    governance_authority: String,
    /// Exact canonical fee-quote draft; all signatures and finalization must fit its 60-second window.
    #[arg(long, value_name = "PATH")]
    draft: PathBuf,
    /// At least two independently produced fee-quote signatures.
    #[arg(long = "signature", value_name = "PATH", num_args = 2..=64)]
    signatures: Vec<PathBuf>,
    /// Absent destination for the exact quoted transaction payload.
    #[arg(long, value_name = "PATH")]
    output: PathBuf,
}

impl FinalizeFeeQuote {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let draft: LifecycleFeeQuoteDraftV1 =
            read_canonical(&self.draft, "lifecycle fee-quote draft")?;
        validate_fee_quote_draft(&draft, self.kind)?;
        require_expected_authority(&self.governance_authority, &draft.payload)?;
        let client = context.client_from_config();
        require_configured_fee_quote_binding(&draft, &client)?;

        let signatures = read_fee_quote_signatures(&self.signatures)?;
        let mut witness = draft.witness.clone();
        witness.signatures = signatures;
        let quote = client
            .quote_fees_with_multisig_witness(&draft.payload, &witness)
            .wrap_err("failed to obtain authenticated lifecycle fee quote")?;
        let mut payload = draft.payload;
        apply_fee_quote_intent(&mut payload, quote.intent)?;
        require_lifecycle_payload(&payload, self.kind)?;
        let builder = TransactionBuilder::from_payload(payload)
            .wrap_err("failed to freeze exact quoted lifecycle payload")?;
        let bytes = builder.encode_payload();
        publish_no_replace(&self.output, &bytes, "quoted lifecycle transaction payload")?;
        context.println(format_args!(
            "froze exact quoted {:?} payload at {}",
            self.kind,
            self.output.display()
        ))
    }
}

#[derive(ClapArgs, Debug)]
struct SignTransaction {
    /// Lifecycle kind expected in the payload.
    #[arg(long, value_enum)]
    kind: LifecycleKind,
    /// Independently expected canonical I105 multisig authority.
    #[arg(long, value_name = "I105_ACCOUNT")]
    governance_authority: String,
    /// Independently expected NetworkId embedded in the payload; does not consult client config.
    #[arg(long, value_name = "NETWORK_ID")]
    expected_network_id: String,
    /// Independently expected lowercase or uppercase 64-hex SHA-256 of the exact payload file.
    #[arg(long, value_name = "64_HEX")]
    expected_payload_sha256: String,
    /// Exact frozen TransactionPayload archive.
    #[arg(long, value_name = "PATH")]
    payload: PathBuf,
    /// Absent destination for this signer's canonical detached signature.
    #[arg(long, value_name = "PATH")]
    output: PathBuf,
}

impl SignTransaction {
    fn validated_signing_input(&self) -> Result<TransactionPayload> {
        let payload_bytes = read_bounded_stable(
            &self.payload,
            LIFECYCLE_ARTIFACT_MAX_BYTES,
            "quoted lifecycle transaction payload",
        )?;
        require_expected_artifact_sha256(
            &payload_bytes,
            &self.expected_payload_sha256,
            "--expected-payload-sha256",
            "quoted lifecycle transaction payload",
        )?;
        let payload = decode_transaction_payload_archive(&payload_bytes)?;
        require_expected_signing_network(
            &self.expected_network_id,
            &payload,
            "quoted lifecycle transaction payload",
        )?;
        require_lifecycle_payload(&payload, self.kind)?;
        require_expected_authority(&self.governance_authority, &payload)?;
        Ok(payload)
    }

    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let payload = self.validated_signing_input()?;
        let key = context.operator_key_pair().ok_or_else(|| {
            eyre!("--operator-private-key-file is required for detached lifecycle signing")
        })?;
        require_policy_member(&payload, key.public_key())?;
        let mut bundle = MultisigSignatures::from_signers(&payload, [key.private_key()])
            .wrap_err("failed to sign exact lifecycle transaction payload")?;
        let signature = bundle
            .signatures
            .pop()
            .ok_or_else(|| eyre!("lifecycle signer produced no detached signature"))?;
        write_canonical_no_replace(
            &self.output,
            &signature,
            "lifecycle transaction detached signature",
        )?;
        context.println(format_args!(
            "wrote detached transaction signature at {}",
            self.output.display()
        ))
    }
}

#[derive(ClapArgs, Debug)]
struct AssembleTransaction {
    /// Lifecycle kind expected in the payload.
    #[arg(long, value_enum)]
    kind: LifecycleKind,
    /// Independently expected canonical I105 multisig authority.
    #[arg(long, value_name = "I105_ACCOUNT")]
    governance_authority: String,
    /// Exact frozen TransactionPayload archive.
    #[arg(long, value_name = "PATH")]
    payload: PathBuf,
    /// At least two independently produced transaction signatures.
    #[arg(long = "signature", value_name = "PATH", num_args = 2..=64)]
    signatures: Vec<PathBuf>,
    /// Absent destination for canonical versioned SignedTransaction wire.
    #[arg(long, value_name = "PATH")]
    output: PathBuf,
}

impl AssembleTransaction {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let (_, payload) = read_transaction_payload(&self.payload, self.kind)?;
        require_expected_authority(&self.governance_authority, &payload)?;
        let signatures = read_transaction_signatures(&self.signatures)?;
        let transaction = assemble_transaction(payload, signatures)?;
        require_lifecycle_transaction(&transaction, self.kind)?;
        let bytes = transaction
            .encode_wire_v1()
            .map_err(|error| eyre!("failed to encode exact lifecycle transaction: {error}"))?;
        publish_no_replace(&self.output, &bytes, "assembled lifecycle transaction")?;
        context.println(format_args!(
            "assembled exact {:?} transaction at {}",
            self.kind,
            self.output.display()
        ))
    }
}

#[derive(ClapArgs, Debug)]
struct SubmitTransaction {
    /// Lifecycle kind expected in the signed wire.
    #[arg(long, value_enum)]
    kind: LifecycleKind,
    /// Independently expected canonical I105 multisig authority.
    #[arg(long, value_name = "I105_ACCOUNT")]
    governance_authority: String,
    /// Exact canonical versioned SignedTransaction wire.
    #[arg(long, value_name = "PATH")]
    transaction: PathBuf,
    /// Independently pinned Torii receipt signer public key.
    #[arg(long, value_name = "PUBLIC_KEY")]
    expected_receipt_signer: String,
    /// Absent destination for the verified canonical submission receipt.
    #[arg(long, value_name = "PATH")]
    receipt_output: PathBuf,
    /// Explicit authorization for this production lifecycle write.
    #[arg(long, required = true, action = clap::ArgAction::SetTrue)]
    write_authorized: bool,
}

impl SubmitTransaction {
    #[rustfmt::skip]
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        if !self.write_authorized { bail!("--write-authorized is required for lifecycle submission"); }
        let bytes = read_bounded_stable(&self.transaction, LIFECYCLE_ARTIFACT_MAX_BYTES, "assembled lifecycle transaction")?;
        let (transaction, prepared) = prepare_exact_lifecycle_submission(&bytes, self.kind)?;
        require_expected_authority(&self.governance_authority, transaction.payload())?;
        let expected_receipt_signer: PublicKey = self.expected_receipt_signer.parse().wrap_err("invalid --expected-receipt-signer")?;
        if expected_receipt_signer.to_string() != self.expected_receipt_signer { bail!("--expected-receipt-signer must use its canonical public-key spelling"); }
        require_publication_destination_absent(&self.receipt_output, "verified lifecycle submission receipt")?;
        let client = context.client_from_config();
        if transaction.network_id() != Some(&client.network_id) { bail!("lifecycle transaction network differs from the configured client network"); }
        let transaction_hash = transaction.hash();
        let receipt = client.submit_prepared_kagemusha_lifecycle_payload(&transaction, &prepared, &expected_receipt_signer)
            .wrap_err("failed to submit exact lifecycle transaction")?;
        encode_and_publish_verified_lifecycle_receipt(&receipt, transaction_hash.clone(), &self.receipt_output)
            .map_err(eyre::Report::new)?;
        context.println_data(transaction_hash).wrap_err_with(|| format!(
            "lifecycle transaction {} was durably acknowledged and its verified receipt was published at `{}`, but hash output failed; do not retry automatically",
            transaction.hash(), self.receipt_output.display()
        ))
    }
}

fn reject_legacy_instruction_io(context: &impl RunContext) -> Result<()> {
    if context.input_instructions() || context.output_instructions() {
        bail!("--input/--output cannot be combined with the exact lifecycle archive corridor");
    }
    Ok(())
}

fn require_lifecycle_instruction(
    instruction: &InstructionBox,
    expected: LifecycleKind,
) -> Result<()> {
    let actual = lifecycle_kind_for_type_id(instruction.as_any().type_id())
        .ok_or_else(|| eyre!("instruction is not a native Kagemusha V4 lifecycle transition"))?;
    if actual != expected {
        bail!("lifecycle instruction kind differs from --kind");
    }
    match actual {
        LifecycleKind::Stage => instruction
            .as_any()
            .downcast_ref::<ActivateKagemushaRecursiveReleaseV4>()
            .expect("kind classifier and downcast agree")
            .validate_promotion_id()
            .map_err(|error| eyre!(error)),
        LifecycleKind::Enable => instruction
            .as_any()
            .downcast_ref::<EnableKagemushaRecursiveIssuanceV4>()
            .expect("kind classifier and downcast agree")
            .validate()
            .map_err(|error| eyre!(error)),
        LifecycleKind::Cancel => instruction
            .as_any()
            .downcast_ref::<CancelKagemushaRecursiveReleaseV4>()
            .expect("kind classifier and downcast agree")
            .validate()
            .map_err(|error| eyre!(error)),
        LifecycleKind::Deactivate => instruction
            .as_any()
            .downcast_ref::<DeactivateKagemushaRecursiveIssuanceV4>()
            .expect("kind classifier and downcast agree")
            .validate()
            .map_err(|error| eyre!(error)),
    }
}

#[rustfmt::skip]
fn metadata_expires_at_height(metadata: &iroha::data_model::metadata::Metadata) -> Result<Option<u64>> {
    let key: Name = "expires_at_height".parse().expect("valid expiry metadata key");
    metadata.get(&key).map(|value| value.try_into_any_norito::<u64>()).transpose()
        .wrap_err("lifecycle expires_at_height metadata is not a canonical u64")
}

#[rustfmt::skip]
fn lifecycle_metadata_with_stage_expiry(
    mut metadata: iroha::data_model::metadata::Metadata,
    kind: LifecycleKind,
    trusted_anchor_height: Option<u64>,
) -> Result<iroha::data_model::metadata::Metadata> {
    if kind != LifecycleKind::Stage {
        if trusted_anchor_height.is_some() { bail!("--trusted-anchor-height is accepted only for --kind stage"); }
        return Ok(metadata);
    }
    let anchor = trusted_anchor_height.ok_or_else(|| eyre!("--trusted-anchor-height is required for --kind stage"))?;
    let proof_bound = u64::try_from(KAGEMUSHA_V4_ACTIVATION_FINALITY_PROOF_MAX_COUNT_V1).expect("proof-count bound fits u64");
    let expiry = anchor.checked_add(proof_bound)
        .and_then(|height| height.checked_add(1))
        .ok_or_else(|| eyre!("trusted anchor height cannot encode the Stage receipt horizon"))?;
    if let Some(configured) = metadata_expires_at_height(&metadata)? {
        if configured != expiry { bail!("configured expires_at_height differs from trusted anchor height plus the exact Stage receipt horizon"); }
        return Ok(metadata);
    }
    let key: Name = "expires_at_height".parse().expect("valid expiry metadata key");
    metadata.insert(key, expiry);
    Ok(metadata)
}

fn lifecycle_kind_for_type_id(type_id: TypeId) -> Option<LifecycleKind> {
    if type_id == TypeId::of::<ActivateKagemushaRecursiveReleaseV4>() {
        Some(LifecycleKind::Stage)
    } else if type_id == TypeId::of::<EnableKagemushaRecursiveIssuanceV4>() {
        Some(LifecycleKind::Enable)
    } else if type_id == TypeId::of::<CancelKagemushaRecursiveReleaseV4>() {
        Some(LifecycleKind::Cancel)
    } else if type_id == TypeId::of::<DeactivateKagemushaRecursiveIssuanceV4>() {
        Some(LifecycleKind::Deactivate)
    } else {
        None
    }
}

fn require_lifecycle_payload(payload: &TransactionPayload, expected: LifecycleKind) -> Result<()> {
    if payload.network_id().is_none() {
        bail!("lifecycle transaction must use an ordinary network domain");
    }
    if payload.admission_intent() != TransactionAdmissionIntent::Ordinary {
        bail!("lifecycle transaction must carry the Ordinary admission intent");
    }
    if payload.attachments.is_some() {
        bail!("direct lifecycle transaction must not carry proof attachments");
    }
    if expected == LifecycleKind::Stage
        && metadata_expires_at_height(&payload.metadata)?.is_none_or(|expiry| expiry == 0)
    {
        bail!("Stage lifecycle transaction must carry a nonzero expires_at_height");
    }
    require_multisig_policy(&payload.authority)?;
    let Executable::Instructions(instructions) = payload.instructions() else {
        bail!("lifecycle transaction must carry native instructions directly");
    };
    let [instruction] = instructions.as_ref() else {
        bail!("lifecycle transaction must carry exactly one native instruction");
    };
    require_lifecycle_instruction(instruction, expected)?;
    require_live_transaction_clock(payload)
}

fn require_live_transaction_clock(payload: &TransactionPayload) -> Result<()> {
    let now_ms = current_unix_ms()?;
    if payload.creation_time_ms > now_ms.saturating_add(LIFECYCLE_FEE_QUOTE_MAX_CLOCK_SKEW_MS) {
        bail!("lifecycle transaction creation time is too far in the future");
    }
    let ttl_ms = payload
        .time_to_live()
        .ok_or_else(|| eyre!("lifecycle transaction must carry a nonzero TTL"))?
        .as_millis();
    let ttl_ms =
        u64::try_from(ttl_ms).map_err(|_| eyre!("lifecycle transaction TTL is too large"))?;
    let expires_at = payload
        .creation_time_ms
        .checked_add(ttl_ms)
        .ok_or_else(|| eyre!("lifecycle transaction expiry overflows u64 milliseconds"))?;
    if now_ms >= expires_at {
        bail!("lifecycle transaction has expired");
    }
    Ok(())
}

fn require_lifecycle_transaction(
    transaction: &SignedTransaction,
    expected: LifecycleKind,
) -> Result<()> {
    require_lifecycle_payload(transaction.payload(), expected)?;
    let signatures = transaction
        .multisig_signatures()
        .ok_or_else(|| eyre!("lifecycle transaction is missing its multisig bundle"))?;
    if signatures.signatures.len() < 2 {
        bail!("lifecycle transaction requires at least two distinct signatures");
    }
    if signatures.signatures.len() > LIFECYCLE_MAX_SIGNATURES {
        bail!("lifecycle transaction exceeds the signature-count limit");
    }
    signatures
        .validate_canonical()
        .wrap_err("lifecycle multisig bundle is not canonical")?;
    transaction
        .verify_signature()
        .wrap_err("lifecycle multisig authorization failed verification")
}

fn require_multisig_policy(authority: &AccountId) -> Result<&MultisigPolicy> {
    let AccountController::Multisig(policy) = authority.controller() else {
        bail!("direct lifecycle authority must be a multisig account");
    };
    if policy.members().len() < 2 {
        bail!("direct lifecycle authority must have at least two policy members");
    }
    Ok(policy)
}

fn require_expected_authority(literal: &str, payload: &TransactionPayload) -> Result<()> {
    let expected = crate::resolve_account_id_with(literal)
        .wrap_err("invalid expected governance authority")?;
    require_multisig_policy(&expected)?;
    require_authority_match(&expected, payload)
}

fn require_authority_match(expected: &AccountId, payload: &TransactionPayload) -> Result<()> {
    if &payload.authority != expected {
        bail!("lifecycle payload authority differs from --governance-authority");
    }
    Ok(())
}

fn require_expected_signing_network(
    literal: &str,
    payload: &TransactionPayload,
    label: &str,
) -> Result<()> {
    let expected: iroha::data_model::NetworkId =
        literal.parse().wrap_err("invalid --expected-network-id")?;
    if payload.network_id() != Some(&expected) {
        bail!("{label} NetworkId differs from --expected-network-id");
    }
    Ok(())
}

fn require_expected_artifact_sha256(
    bytes: &[u8],
    literal: &str,
    flag: &str,
    label: &str,
) -> Result<()> {
    let decoded = hex::decode(literal).wrap_err_with(|| format!("invalid {flag}"))?;
    let expected: [u8; 32] = decoded
        .try_into()
        .map_err(|_| eyre!("{flag} must contain exactly 64 hexadecimal digits"))?;
    let actual: [u8; 32] = Sha256::digest(bytes).into();
    if actual != expected {
        bail!("exact {label} SHA-256 differs from {flag}");
    }
    Ok(())
}

fn require_policy_member(
    payload: &TransactionPayload,
    signer: &iroha_crypto::PublicKey,
) -> Result<()> {
    if require_multisig_policy(&payload.authority)?
        .members()
        .iter()
        .all(|member| member.public_key() != signer)
    {
        bail!("detached signer is not a member of the lifecycle authority policy");
    }
    Ok(())
}

fn validate_fee_quote_draft(
    draft: &LifecycleFeeQuoteDraftV1,
    expected: LifecycleKind,
) -> Result<()> {
    if draft.schema != LIFECYCLE_FEE_QUOTE_DRAFT_SCHEMA_V1
        || draft.version != LIFECYCLE_FEE_QUOTE_DRAFT_VERSION_V1
        || draft.kind != expected
    {
        bail!("lifecycle fee-quote draft schema, version, or kind is invalid");
    }
    require_lifecycle_payload(&draft.payload, expected)?;
    if draft.witness.subject_account != draft.payload.authority {
        bail!("lifecycle fee-quote witness subject differs from the payload authority");
    }
    if !draft.witness.signatures.is_empty() {
        bail!("lifecycle fee-quote draft must not contain preassembled signatures");
    }
    if draft.witness.timestamp_ms == 0 {
        bail!("lifecycle fee-quote witness timestamp must be nonzero");
    }
    require_fresh_timestamp(draft.witness.timestamp_ms, current_unix_ms()?)?;
    canonical_request_witness_message(&draft.witness)
        .wrap_err("invalid lifecycle fee-quote witness envelope")?;
    let url = Url::parse(&draft.fee_quote_url).wrap_err("invalid lifecycle fee-quote URL")?;
    require_secure_fee_quote_origin(&url)?;
    if url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !url.path().ends_with(torii_uri::FEES_QUOTE)
    {
        bail!("lifecycle fee-quote URL is outside the exact Torii fee-quote route");
    }
    let body = fee_quote_body(&draft.payload)?;
    let network_id = draft
        .payload
        .network_id()
        .ok_or_else(|| eyre!("lifecycle fee-quote payload has no network"))?;
    let expected_hash = canonical_network_request_hash(network_id, &HttpMethod::POST, &url, &body)?;
    if draft.witness.canonical_request_hash != expected_hash {
        bail!("lifecycle fee-quote witness does not bind the exact draft request");
    }
    Ok(())
}

fn require_secure_fee_quote_origin(url: &Url) -> Result<()> {
    let loopback = match url.host() {
        Some(Host::Domain(domain)) => domain == "localhost",
        Some(Host::Ipv4(address)) => address.is_loopback(),
        Some(Host::Ipv6(address)) => address.is_loopback(),
        None => false,
    };
    if url.scheme() != "https" && (url.scheme() != "http" || !loopback) {
        bail!(
            "lifecycle fee-quote origin must use HTTPS; HTTP is allowed only for exact localhost, 127/8, or ::1 loopback hosts"
        );
    }
    Ok(())
}

fn require_configured_fee_quote_binding(
    draft: &LifecycleFeeQuoteDraftV1,
    client: &Client,
) -> Result<()> {
    if draft.payload.network_id() != Some(&client.network_id) {
        bail!("lifecycle draft network differs from the configured client network");
    }
    let expected_url = fee_quote_url(client)?;
    let draft_url =
        Url::parse(&draft.fee_quote_url).wrap_err("lifecycle draft fee-quote URL is invalid")?;
    if draft_url != expected_url {
        bail!("lifecycle draft fee-quote URL differs from the configured Torii route");
    }
    Ok(())
}

fn apply_fee_quote_intent(
    payload: &mut TransactionPayload,
    quoted: iroha::data_model::transaction::FeePaymentIntent,
) -> Result<()> {
    if !payload.fee_payment.has_same_payer_and_gas_bound(&quoted) {
        bail!("fee quote changed the selected payer, sponsor revision, or gas bound");
    }
    quoted
        .validate()
        .wrap_err("fee quote returned an invalid payment intent")?;
    payload.fee_payment = quoted;
    Ok(())
}

fn fee_quote_body(payload: &TransactionPayload) -> Result<Vec<u8>> {
    norito::json::to_vec(&FeeQuoteRequest {
        payload: payload.clone(),
    })
    .wrap_err("failed to encode exact lifecycle fee-quote request")
}

fn fee_quote_url(client: &Client) -> Result<Url> {
    if !client.torii_url.path().ends_with('/') {
        bail!("configured Torii base URL must end with '/'");
    }
    let url = client
        .torii_url
        .join(torii_uri::FEES_QUOTE.trim_start_matches('/'))
        .wrap_err("failed to construct exact Torii fee-quote URL")?;
    require_secure_fee_quote_origin(&url)?;
    Ok(url)
}

fn read_fee_quote_signatures(paths: &[PathBuf]) -> Result<Vec<CanonicalRequestSignatureWitnessV1>> {
    if paths.len() < 2 || paths.len() > LIFECYCLE_MAX_SIGNATURES {
        bail!("fee-quote assembly requires 2..={LIFECYCLE_MAX_SIGNATURES} signatures");
    }
    let signatures = paths
        .iter()
        .map(|path| read_canonical(path, "lifecycle fee-quote detached signature"))
        .collect::<Result<Vec<CanonicalRequestSignatureWitnessV1>>>()?;
    if signatures
        .windows(2)
        .any(|pair| pair[0].signer >= pair[1].signer)
    {
        bail!("fee-quote signature artifacts must be distinct and in canonical signer order");
    }
    Ok(signatures)
}

fn read_transaction_signatures(paths: &[PathBuf]) -> Result<Vec<MultisigSignature>> {
    if paths.len() < 2 || paths.len() > LIFECYCLE_MAX_SIGNATURES {
        bail!("transaction assembly requires 2..={LIFECYCLE_MAX_SIGNATURES} signatures");
    }
    let signatures = paths
        .iter()
        .map(|path| read_canonical(path, "lifecycle transaction detached signature"))
        .collect::<Result<Vec<MultisigSignature>>>()?;
    if signatures
        .windows(2)
        .any(|pair| pair[0].signer >= pair[1].signer)
    {
        bail!("transaction signature artifacts must be distinct and in canonical signer order");
    }
    Ok(signatures)
}

fn assemble_transaction(
    payload: TransactionPayload,
    signatures: Vec<MultisigSignature>,
) -> Result<SignedTransaction> {
    if signatures.len() < 2 || signatures.len() > LIFECYCLE_MAX_SIGNATURES {
        bail!("lifecycle transaction requires 2..={LIFECYCLE_MAX_SIGNATURES} signatures");
    }
    if signatures
        .windows(2)
        .any(|pair| pair[0].signer >= pair[1].signer)
    {
        bail!("lifecycle transaction signatures are duplicate or non-canonically ordered");
    }
    let bundle = MultisigSignatures::new(signatures);
    let primary: Signature = bundle
        .signatures
        .first()
        .ok_or_else(|| eyre!("lifecycle transaction contains no signatures"))?
        .signature
        .clone()
        .into();
    let transaction = TransactionBuilder::from_payload(payload)
        .wrap_err("failed to reconstruct exact lifecycle payload")?
        .with_multisig_signatures(bundle)
        .build_with_signature(primary);
    transaction
        .verify_signature()
        .wrap_err("lifecycle transaction signatures failed verification")?;
    Ok(transaction)
}

fn prepare_exact_lifecycle_submission(
    bytes: &[u8],
    expected: LifecycleKind,
) -> Result<(SignedTransaction, PreparedTransactionPayload)> {
    let transaction = SignedTransaction::decode_all_versioned(bytes)
        .wrap_err("failed to decode assembled lifecycle transaction")?;
    let canonical = transaction
        .encode_wire_v1()
        .map_err(|error| eyre!("failed to re-encode lifecycle transaction: {error}"))?;
    if canonical != bytes {
        bail!("assembled lifecycle transaction is not exact canonical V1 wire");
    }
    require_lifecycle_transaction(&transaction, expected)?;
    let prepared = Client::prepare_transaction_payload(&transaction);
    if prepared.as_bytes() != bytes {
        bail!("prepared lifecycle submission bytes differ from the authorized archive");
    }
    Ok((transaction, prepared))
}

fn read_transaction_payload(
    path: &Path,
    expected: LifecycleKind,
) -> Result<(Vec<u8>, TransactionPayload)> {
    let bytes = read_bounded_stable(
        path,
        LIFECYCLE_ARTIFACT_MAX_BYTES,
        "quoted lifecycle transaction payload",
    )?;
    let payload = decode_transaction_payload(&bytes, expected)?;
    Ok((bytes, payload))
}

fn decode_transaction_payload(bytes: &[u8], expected: LifecycleKind) -> Result<TransactionPayload> {
    let payload = decode_transaction_payload_archive(bytes)?;
    require_lifecycle_payload(&payload, expected)?;
    Ok(payload)
}

fn decode_transaction_payload_archive(bytes: &[u8]) -> Result<TransactionPayload> {
    let builder = TransactionBuilder::decode_payload(bytes)
        .wrap_err("failed to decode exact lifecycle transaction payload")?;
    builder
        .into_payload()
        .wrap_err("invalid lifecycle transaction payload")
}

fn read_canonical<T>(path: &Path, label: &str) -> Result<T>
where
    T: norito::NoritoSerialize + for<'de> norito::NoritoDeserialize<'de>,
{
    let bytes = read_bounded_stable(path, LIFECYCLE_ARTIFACT_MAX_BYTES, label)?;
    decode_canonical(&bytes, label)
}

fn decode_canonical<T>(bytes: &[u8], label: &str) -> Result<T>
where
    T: norito::NoritoSerialize + for<'de> norito::NoritoDeserialize<'de>,
{
    let value = norito::decode_canonical_with_limits::<T>(
        bytes,
        norito::canonical_decode_limits(bytes.len()),
    )
    .wrap_err_with(|| format!("failed to decode canonical {label}"))?;
    let canonical = norito::encode_canonical(&value)
        .wrap_err_with(|| format!("failed to re-encode canonical {label}"))?;
    if canonical.as_slice() != bytes {
        bail!("{label} is not exact canonical Norito");
    }
    Ok(value)
}

fn write_canonical_no_replace<T>(path: &Path, value: &T, label: &str) -> Result<()>
where
    T: norito::NoritoSerialize,
{
    let bytes = norito::encode_canonical(value)
        .wrap_err_with(|| format!("failed to encode canonical {label}"))?;
    publish_no_replace(path, &bytes, label)
}

#[derive(Debug, thiserror::Error)]
enum LifecyclePostAcknowledgementError {
    #[error(
        "lifecycle transaction {transaction_hash} was durably acknowledged, but verified receipt \
         {phase} failed; do not retry automatically and do not treat `{receipt_path}` as \
         lifecycle evidence: {detail}"
    )]
    ReceiptProcessing {
        transaction_hash: HashOf<SignedTransaction>,
        receipt_path: String,
        phase: &'static str,
        detail: String,
    },
}

fn lifecycle_post_acknowledgement_error(
    transaction_hash: HashOf<SignedTransaction>,
    receipt_path: &Path,
    phase: &'static str,
    detail: impl std::fmt::Display,
) -> LifecyclePostAcknowledgementError {
    LifecyclePostAcknowledgementError::ReceiptProcessing {
        transaction_hash,
        receipt_path: receipt_path.display().to_string(),
        phase,
        detail: detail.to_string(),
    }
}

fn encode_and_publish_verified_lifecycle_receipt(
    receipt: &TransactionSubmissionReceipt,
    transaction_hash: HashOf<SignedTransaction>,
    receipt_path: &Path,
) -> std::result::Result<(), LifecyclePostAcknowledgementError> {
    let receipt_bytes = norito::encode_canonical(receipt).map_err(|error| {
        lifecycle_post_acknowledgement_error(
            transaction_hash.clone(),
            receipt_path,
            "encoding",
            error,
        )
    })?;
    publish_no_replace(
        receipt_path,
        &receipt_bytes,
        "verified lifecycle submission receipt",
    )
    .map_err(|error| {
        lifecycle_post_acknowledgement_error(
            transaction_hash,
            receipt_path,
            "publication",
            format_args!("{error:#}"),
        )
    })
}

#[cfg(unix)]
#[derive(Debug, thiserror::Error)]
enum LifecycleArtifactPublicationError {
    #[error("{label} publication failed before commit at `{path}`: {detail}")]
    PreCommit {
        path: PathBuf,
        label: String,
        detail: String,
    },
    #[error(
        "{label} publication is commit-uncertain at `{path}`; do not retry automatically: {detail}"
    )]
    CommitUncertain {
        path: PathBuf,
        label: String,
        detail: String,
    },
}

fn publish_no_replace(path: &Path, bytes: &[u8], label: &str) -> Result<()> {
    #[cfg(unix)]
    {
        publish_no_replace_with_hooks(
            path,
            bytes,
            label,
            |_, _| Ok(()),
            rename_lifecycle_staging_no_replace,
            |_| Ok(()),
            File::sync_all,
        )
        .map_err(eyre::Report::new)
    }
    #[cfg(not(unix))]
    {
        let _ = (path, bytes, label);
        bail!(
            "production lifecycle artifact publication requires Unix descriptor-relative no-replace APIs"
        )
    }
}

#[cfg(unix)]
#[rustfmt::skip]
fn require_publication_destination_absent(path: &Path, label: &str) -> Result<()> {
    use rustix::fs::{AtFlags, statat};
    let parent_path = path.parent().filter(|parent| !parent.as_os_str().is_empty()).unwrap_or_else(|| Path::new("."));
    let target = path.file_name().ok_or_else(|| eyre!("{label} destination has no file name"))?;
    let parent = PinnedLifecyclePublicationParent::open(parent_path).wrap_err_with(|| format!("pin {label} publication parent"))?;
    match statat(&parent.file, target, AtFlags::SYMLINK_NOFOLLOW) { Err(error) if error == rustix::io::Errno::NOENT => Ok(()), Ok(_) => bail!("{label} destination already exists and will not be replaced"), Err(error) => Err(error).wrap_err_with(|| format!("inspect {label} destination")) }
}
#[cfg(not(unix))]
fn require_publication_destination_absent(_: &Path, _: &str) -> Result<()> {
    bail!(
        "production lifecycle artifact publication requires Unix descriptor-relative no-replace APIs"
    )
}

#[cfg(unix)]
fn lifecycle_publication_precommit(
    path: &Path,
    label: &str,
    detail: impl std::fmt::Display,
) -> LifecycleArtifactPublicationError {
    LifecycleArtifactPublicationError::PreCommit {
        path: path.to_path_buf(),
        label: label.to_owned(),
        detail: detail.to_string(),
    }
}

#[cfg(unix)]
fn lifecycle_publication_commit_uncertain(
    path: &Path,
    label: &str,
    detail: impl std::fmt::Display,
) -> LifecycleArtifactPublicationError {
    LifecycleArtifactPublicationError::CommitUncertain {
        path: path.to_path_buf(),
        label: label.to_owned(),
        detail: detail.to_string(),
    }
}

#[cfg(unix)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct LifecycleDirectorySnapshot {
    device: u64,
    inode: u64,
    mode: u32,
    uid: u32,
    gid: u32,
    links: u64,
}

#[cfg(unix)]
impl LifecycleDirectorySnapshot {
    fn from_metadata(metadata: &Metadata) -> Option<Self> {
        use std::os::unix::fs::MetadataExt as _;
        metadata.is_dir().then(|| Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            mode: metadata.mode(),
            uid: metadata.uid(),
            gid: metadata.gid(),
            links: metadata.nlink(),
        })
    }

    fn validate_trusted(self, label: &str) -> Result<()> {
        let effective_uid = rustix::process::geteuid().as_raw();
        let root_owned_sticky = self.uid == 0 && self.mode & 0o1000 != 0;
        if (self.uid != 0 && self.uid != effective_uid)
            || (self.mode & 0o022 != 0 && !root_owned_sticky)
            || self.links == 0
        {
            bail!(
                "{label} must be linked and owned by root or the effective uid; writable ancestors must be root-owned sticky directories"
            );
        }
        Ok(())
    }

    fn matches_identity(self, other: Self) -> bool {
        self.device == other.device
            && self.inode == other.inode
            && self.mode == other.mode
            && self.uid == other.uid
            && self.gid == other.gid
    }
}

#[cfg(unix)]
struct PinnedLifecyclePublicationParent {
    requested_path: PathBuf,
    canonical_path: PathBuf,
    file: File,
    snapshot: LifecycleDirectorySnapshot,
    canonical_chain: Vec<(PathBuf, LifecycleDirectorySnapshot)>,
}

#[cfg(unix)]
impl PinnedLifecyclePublicationParent {
    fn open(path: &Path) -> Result<Self> {
        use rustix::fs::{AtFlags, Mode, OFlags, open, openat, statat};
        use std::path::Component;

        let requested_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()
                .wrap_err("resolve lifecycle artifact current directory")?
                .join(path)
        };
        let canonical_path =
            fs::canonicalize(&requested_path).wrap_err("canonicalize lifecycle artifact parent")?;
        let root_path = Path::new("/");
        let root_snapshot = LifecycleDirectorySnapshot::from_metadata(
            &fs::symlink_metadata(root_path).wrap_err("inspect filesystem root")?,
        )
        .ok_or_else(|| eyre!("filesystem root is not a directory"))?;
        root_snapshot.validate_trusted("filesystem root")?;
        let mut file = File::from(
            open(
                root_path,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .wrap_err("pin filesystem root for lifecycle artifact publication")?,
        );
        let opened_root = LifecycleDirectorySnapshot::from_metadata(
            &file.metadata().wrap_err("inspect pinned filesystem root")?,
        )
        .ok_or_else(|| eyre!("pinned filesystem root is not a directory"))?;
        if !root_snapshot.matches_identity(opened_root) || opened_root.links == 0 {
            bail!("filesystem root changed while it was pinned");
        }

        let mut current_path = root_path.to_path_buf();
        let mut canonical_chain = vec![(current_path.clone(), opened_root)];
        let mut snapshot = opened_root;
        for component in canonical_path.components().skip(1) {
            let Component::Normal(name) = component else {
                bail!("lifecycle artifact parent has a non-canonical component");
            };
            let before = statat(&file, name, AtFlags::SYMLINK_NOFOLLOW)
                .wrap_err("inspect lifecycle artifact parent component")?;
            if rustix::fs::FileType::from_raw_mode(before.st_mode)
                != rustix::fs::FileType::Directory
            {
                bail!("lifecycle artifact parent chain contains a non-directory");
            }
            let next = File::from(
                openat(
                    &file,
                    name,
                    OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                    Mode::empty(),
                )
                .wrap_err("pin lifecycle artifact parent component")?,
            );
            let next_snapshot = LifecycleDirectorySnapshot::from_metadata(
                &next
                    .metadata()
                    .wrap_err("inspect pinned lifecycle artifact parent component")?,
            )
            .ok_or_else(|| {
                eyre!("pinned lifecycle artifact parent component is not a directory")
            })?;
            let after = statat(&file, name, AtFlags::SYMLINK_NOFOLLOW)
                .wrap_err("reinspect lifecycle artifact parent component")?;
            if !lifecycle_directory_snapshot_matches_stat(next_snapshot, &before)
                || !lifecycle_directory_snapshot_matches_stat(next_snapshot, &after)
            {
                bail!("lifecycle artifact parent component changed while it was pinned");
            }
            current_path.push(name);
            next_snapshot.validate_trusted(&format!(
                "lifecycle artifact parent component `{}`",
                current_path.display()
            ))?;
            file = next;
            snapshot = next_snapshot;
            canonical_chain.push((current_path.clone(), snapshot));
        }
        let parent = Self {
            requested_path,
            canonical_path,
            file,
            snapshot,
            canonical_chain,
        };
        parent.verify_path_identity_against(snapshot)?;
        Ok(parent)
    }

    fn snapshot_after_staging(&self) -> Result<LifecycleDirectorySnapshot> {
        let current = LifecycleDirectorySnapshot::from_metadata(
            &self
                .file
                .metadata()
                .wrap_err("inspect lifecycle artifact parent after staging")?,
        )
        .ok_or_else(|| eyre!("lifecycle artifact parent is no longer a directory"))?;
        if !self.snapshot.matches_identity(current)
            || !current
                .links
                .checked_sub(self.snapshot.links)
                .is_some_and(|delta| delta <= 1)
        {
            bail!("lifecycle artifact parent changed unexpectedly during staging");
        }
        self.verify_path_identity_against(current)?;
        Ok(current)
    }

    fn verify_path_identity_against(
        &self,
        expected_parent: LifecycleDirectorySnapshot,
    ) -> Result<()> {
        let opened = LifecycleDirectorySnapshot::from_metadata(
            &self
                .file
                .metadata()
                .wrap_err("reinspect pinned lifecycle artifact parent")?,
        );
        if opened != Some(expected_parent) {
            bail!("pinned lifecycle artifact parent changed identity");
        }
        let final_index = self.canonical_chain.len().saturating_sub(1);
        for (index, (path, expected)) in self.canonical_chain.iter().enumerate() {
            let named = fs::symlink_metadata(path).wrap_err_with(|| {
                format!("reinspect lifecycle artifact ancestor `{}`", path.display())
            })?;
            let current = LifecycleDirectorySnapshot::from_metadata(&named);
            let matches = current.is_some_and(|current| {
                if index == final_index {
                    current == expected_parent
                } else {
                    expected.matches_identity(current) && current.links > 0
                }
            });
            if named.file_type().is_symlink() || !matches {
                bail!(
                    "lifecycle artifact ancestor changed after pinning: {}",
                    path.display()
                );
            }
        }
        let resolved = fs::canonicalize(&self.requested_path)
            .wrap_err("re-resolve requested lifecycle artifact parent")?;
        if resolved != self.canonical_path {
            bail!("requested lifecycle artifact parent changed after pinning");
        }
        Ok(())
    }
}

#[cfg(unix)]
fn lifecycle_stat_field_matches<Actual, Expected>(actual: Actual, expected: Expected) -> bool
where
    Actual: TryInto<Expected>,
    Expected: PartialEq,
{
    actual.try_into().ok() == Some(expected)
}

#[cfg(unix)]
fn lifecycle_directory_snapshot_matches_stat(
    snapshot: LifecycleDirectorySnapshot,
    stat: &rustix::fs::Stat,
) -> bool {
    lifecycle_stat_field_matches(stat.st_dev, snapshot.device)
        && lifecycle_stat_field_matches(stat.st_ino, snapshot.inode)
        && lifecycle_stat_field_matches(stat.st_mode, snapshot.mode)
        && lifecycle_stat_field_matches(stat.st_uid, snapshot.uid)
        && lifecycle_stat_field_matches(stat.st_gid, snapshot.gid)
        && lifecycle_stat_field_matches(stat.st_nlink, snapshot.links)
}

#[cfg(unix)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct LifecycleFileSnapshot {
    device: u64,
    inode: u64,
    mode: u32,
    uid: u32,
    gid: u32,
    links: u64,
    length: u64,
}

#[cfg(unix)]
impl LifecycleFileSnapshot {
    fn from_metadata(metadata: &Metadata) -> Option<Self> {
        use std::os::unix::fs::MetadataExt as _;
        metadata.is_file().then(|| Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            mode: metadata.mode(),
            uid: metadata.uid(),
            gid: metadata.gid(),
            links: metadata.nlink(),
            length: metadata.len(),
        })
    }

    fn validate(self, expected_mode: u32, expected_length: u64, label: &str) -> Result<()> {
        if self.uid != rustix::process::geteuid().as_raw()
            || self.mode & 0o7777 != expected_mode
            || self.links != 1
            || self.length != expected_length
        {
            bail!("{label} has unsafe or unexpected inode custody");
        }
        Ok(())
    }

    fn same_inode(self, stat: &rustix::fs::Stat) -> bool {
        lifecycle_stat_field_matches(stat.st_dev, self.device)
            && lifecycle_stat_field_matches(stat.st_ino, self.inode)
    }
}

#[cfg(unix)]
fn lifecycle_file_snapshot_matches_stat(
    snapshot: LifecycleFileSnapshot,
    stat: &rustix::fs::Stat,
) -> bool {
    snapshot.same_inode(stat)
        && lifecycle_stat_field_matches(stat.st_mode, snapshot.mode)
        && lifecycle_stat_field_matches(stat.st_uid, snapshot.uid)
        && lifecycle_stat_field_matches(stat.st_gid, snapshot.gid)
        && lifecycle_stat_field_matches(stat.st_nlink, snapshot.links)
        && lifecycle_stat_field_matches(stat.st_size, snapshot.length)
}

#[cfg(unix)]
fn random_lifecycle_staging_name() -> Result<std::ffi::OsString> {
    use std::os::unix::ffi::OsStringExt as _;
    let mut random = [0_u8; 16];
    OsRng
        .try_fill_bytes(&mut random)
        .wrap_err("obtain OS entropy for lifecycle artifact staging name")?;
    Ok(std::ffi::OsString::from_vec(
        format!(".iroha-kagemusha-lifecycle.{}.tmp", hex::encode(random)).into_bytes(),
    ))
}

#[cfg(unix)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LifecycleStagingCleanupOutcome {
    Removed,
    AlreadyAbsent,
}

#[cfg(unix)]
fn cleanup_lifecycle_staging(
    parent: &PinnedLifecyclePublicationParent,
    staging_name: &std::ffi::OsStr,
    expected: LifecycleFileSnapshot,
) -> Result<LifecycleStagingCleanupOutcome> {
    use rustix::fs::{AtFlags, statat, unlinkat};
    let named = match statat(&parent.file, staging_name, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(named) => named,
        Err(error) if error == rustix::io::Errno::NOENT => {
            return Ok(LifecycleStagingCleanupOutcome::AlreadyAbsent);
        }
        Err(error) => return Err(error).wrap_err("inspect lifecycle staging file for cleanup"),
    };
    if !expected.same_inode(&named) {
        bail!("lifecycle staging name no longer identifies the owned inode; refusing cleanup");
    }
    unlinkat(&parent.file, staging_name, AtFlags::empty())
        .wrap_err("remove unpublished lifecycle staging file")?;
    parent
        .file
        .sync_all()
        .wrap_err("sync lifecycle staging cleanup")?;
    Ok(LifecycleStagingCleanupOutcome::Removed)
}

#[cfg(unix)]
fn verify_lifecycle_artifact_file(
    parent: &PinnedLifecyclePublicationParent,
    name: &std::ffi::OsStr,
    expected: LifecycleFileSnapshot,
    bytes: &[u8],
    label: &str,
) -> Result<()> {
    use rustix::fs::{AtFlags, Mode, OFlags, openat, statat};
    let before = statat(&parent.file, name, AtFlags::SYMLINK_NOFOLLOW)
        .wrap_err_with(|| format!("inspect {label} binding"))?;
    if !lifecycle_file_snapshot_matches_stat(expected, &before) {
        bail!("{label} changed inode, mode, owner, link count, or length");
    }
    let mut opened = File::from(
        openat(
            &parent.file,
            name,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .wrap_err_with(|| format!("pin {label}"))?,
    );
    if LifecycleFileSnapshot::from_metadata(
        &opened
            .metadata()
            .wrap_err_with(|| format!("inspect pinned {label}"))?,
    ) != Some(expected)
    {
        bail!("{label} binding does not identify the staged inode");
    }
    let limit = u64::try_from(bytes.len())?
        .checked_add(1)
        .ok_or_else(|| eyre!("{label} verification bound overflow"))?;
    let mut observed = Vec::new();
    observed
        .try_reserve_exact(bytes.len().saturating_add(1))
        .wrap_err_with(|| format!("reserve {label} verification buffer"))?;
    std::io::Read::by_ref(&mut opened)
        .take(limit)
        .read_to_end(&mut observed)
        .wrap_err_with(|| format!("read pinned {label}"))?;
    if observed != bytes {
        bail!("{label} content differs from the exact requested bytes");
    }
    let after = LifecycleFileSnapshot::from_metadata(
        &opened
            .metadata()
            .wrap_err_with(|| format!("reinspect pinned {label}"))?,
    );
    let linked_after = statat(&parent.file, name, AtFlags::SYMLINK_NOFOLLOW)
        .wrap_err_with(|| format!("reinspect {label} binding"))?;
    if after != Some(expected) || !lifecycle_file_snapshot_matches_stat(expected, &linked_after) {
        bail!("{label} changed during exact verification");
    }
    Ok(())
}

#[cfg(unix)]
fn lifecycle_precommit_after_cleanup(
    parent: &PinnedLifecyclePublicationParent,
    staging_name: &std::ffi::OsStr,
    snapshot: LifecycleFileSnapshot,
    path: &Path,
    label: &str,
    detail: impl std::fmt::Display,
) -> LifecycleArtifactPublicationError {
    let detail = detail.to_string();
    match cleanup_lifecycle_staging(parent, staging_name, snapshot) {
        Ok(_) => lifecycle_publication_precommit(path, label, detail),
        Err(cleanup) => lifecycle_publication_precommit(
            path,
            label,
            format!("{detail}; staging cleanup is uncertain: {cleanup:#}"),
        ),
    }
}

#[cfg(unix)]
fn rename_lifecycle_staging_no_replace(
    parent: &File,
    staging_name: &std::ffi::OsStr,
    target_name: &std::ffi::OsStr,
) -> Result<()> {
    rustix::fs::renameat_with(
        parent,
        staging_name,
        parent,
        target_name,
        rustix::fs::RenameFlags::NOREPLACE,
    )
    .wrap_err("execute atomic lifecycle no-replace rename")
}

#[cfg(unix)]
#[expect(
    clippy::too_many_arguments,
    reason = "rename-error reconciliation keeps both pinned namespace names and the expected inode explicit"
)]
fn reconcile_lifecycle_rename_error(
    parent: &PinnedLifecyclePublicationParent,
    publication_parent: LifecycleDirectorySnapshot,
    staging_name: &std::ffi::OsStr,
    target_name: &std::ffi::OsStr,
    staged: LifecycleFileSnapshot,
    path: &Path,
    label: &str,
    rename_error: &eyre::Report,
) -> LifecycleArtifactPublicationError {
    use rustix::fs::{AtFlags, statat};

    let final_path = parent.canonical_path.join(target_name);
    let detail = format!("atomic no-replace rename returned an error: {rename_error:#}");
    match statat(&parent.file, target_name, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(binding) if staged.same_inode(&binding) => {
            return lifecycle_publication_commit_uncertain(
                &final_path,
                label,
                format!("{detail}; destination binds the owned staged inode"),
            );
        }
        Ok(_) | Err(rustix::io::Errno::NOENT) => {}
        Err(error) => {
            return lifecycle_publication_commit_uncertain(
                &final_path,
                label,
                format!("{detail}; cannot inspect the destination binding: {error}"),
            );
        }
    }
    let staging_binding = match statat(&parent.file, staging_name, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(binding) if lifecycle_file_snapshot_matches_stat(staged, &binding) => binding,
        Ok(_) => {
            return lifecycle_publication_commit_uncertain(
                &final_path,
                label,
                format!("{detail}; staging name no longer binds the exact owned inode"),
            );
        }
        Err(error) => {
            return lifecycle_publication_commit_uncertain(
                &final_path,
                label,
                format!("{detail}; cannot prove the exact staging binding remains: {error}"),
            );
        }
    };
    debug_assert!(staged.same_inode(&staging_binding));

    if let Err(error) = parent.verify_path_identity_against(publication_parent) {
        return lifecycle_publication_commit_uncertain(
            &final_path,
            label,
            format!("{detail}; publication parent changed before rename-error cleanup: {error:#}"),
        );
    }

    match cleanup_lifecycle_staging(parent, staging_name, staged) {
        Ok(LifecycleStagingCleanupOutcome::Removed) => {}
        Ok(LifecycleStagingCleanupOutcome::AlreadyAbsent) => {
            return lifecycle_publication_commit_uncertain(
                &final_path,
                label,
                format!("{detail}; exact staging binding disappeared during reconciliation"),
            );
        }
        Err(error) => {
            return lifecycle_publication_commit_uncertain(
                &final_path,
                label,
                format!("{detail}; exact staging cleanup could not be proven: {error:#}"),
            );
        }
    }

    let destination_is_not_owned =
        match statat(&parent.file, target_name, AtFlags::SYMLINK_NOFOLLOW) {
            Ok(binding) => !staged.same_inode(&binding),
            Err(error) if error == rustix::io::Errno::NOENT => true,
            Err(error) => {
                return lifecycle_publication_commit_uncertain(
                    &final_path,
                    label,
                    format!("{detail}; cannot reinspect the destination after cleanup: {error}"),
                );
            }
        };
    if !destination_is_not_owned {
        return lifecycle_publication_commit_uncertain(
            &final_path,
            label,
            format!("{detail}; destination acquired the owned staged inode during reconciliation"),
        );
    }
    if let Err(error) = parent.verify_path_identity_against(publication_parent) {
        return lifecycle_publication_commit_uncertain(
            &final_path,
            label,
            format!("{detail}; publication parent changed during reconciliation: {error:#}"),
        );
    }

    lifecycle_publication_precommit(
        path,
        label,
        format!("{detail}; exact staging inode was removed and destination is absent or foreign"),
    )
}

#[cfg(unix)]
#[expect(
    clippy::too_many_lines,
    reason = "private staging and the pre-rename versus commit-uncertain boundary must remain visibly ordered"
)]
fn publish_no_replace_with_hooks<BeforeRename, Rename, AfterRename, SyncParent>(
    path: &Path,
    bytes: &[u8],
    label: &str,
    before_rename: BeforeRename,
    rename: Rename,
    after_rename: AfterRename,
    sync_parent: SyncParent,
) -> std::result::Result<(), LifecycleArtifactPublicationError>
where
    BeforeRename: FnOnce(&mut File, &Path) -> Result<()>,
    Rename: FnOnce(&File, &std::ffi::OsStr, &std::ffi::OsStr) -> Result<()>,
    AfterRename: FnOnce(&Path) -> Result<()>,
    SyncParent: FnOnce(&File) -> std::io::Result<()>,
{
    if bytes.is_empty() || bytes.len() > LIFECYCLE_ARTIFACT_MAX_BYTES {
        return Err(lifecycle_publication_precommit(
            path,
            label,
            format!("artifact must be 1..={LIFECYCLE_ARTIFACT_MAX_BYTES} bytes"),
        ));
    }
    let parent_path = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let target_name = path.file_name().ok_or_else(|| {
        lifecycle_publication_precommit(path, label, "destination has no file name")
    })?;
    let mut target_components = Path::new(target_name).components();
    if !matches!(
        target_components.next(),
        Some(std::path::Component::Normal(name)) if name == target_name
    ) || target_components.next().is_some()
    {
        return Err(lifecycle_publication_precommit(
            path,
            label,
            "destination file name is not one normal component",
        ));
    }
    let parent = PinnedLifecyclePublicationParent::open(parent_path)
        .map_err(|error| lifecycle_publication_precommit(path, label, error))?;
    use rustix::fs::{AtFlags, Mode, OFlags, openat, statat};
    match statat(&parent.file, target_name, AtFlags::SYMLINK_NOFOLLOW) {
        Err(error) if error == rustix::io::Errno::NOENT => {}
        Ok(_) => {
            return Err(lifecycle_publication_precommit(
                path,
                label,
                "destination already exists and will not be replaced",
            ));
        }
        Err(error) => {
            return Err(lifecycle_publication_precommit(
                path,
                label,
                format!("inspect destination: {error}"),
            ));
        }
    }
    let staging_name = random_lifecycle_staging_name()
        .map_err(|error| lifecycle_publication_precommit(path, label, error))?;
    let staging_path = parent.canonical_path.join(&staging_name);
    let mut staging = File::from(
        openat(
            &parent.file,
            &staging_name,
            OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::from_raw_mode(0o600),
        )
        .map_err(|error| {
            lifecycle_publication_precommit(
                path,
                label,
                format!("create private staging file: {error}"),
            )
        })?,
    );
    let expected_length = u64::try_from(bytes.len()).map_err(|error| {
        lifecycle_publication_precommit(path, label, format!("artifact length conversion: {error}"))
    })?;
    let initial = LifecycleFileSnapshot::from_metadata(&staging.metadata().map_err(|error| {
        lifecycle_publication_precommit(
            path,
            label,
            format!("inspect private staging file: {error}"),
        )
    })?)
    .ok_or_else(|| {
        lifecycle_publication_precommit(path, label, "staging inode is not a regular file")
    })?;
    if let Err(error) = initial.validate(0o600, 0, "lifecycle staging file") {
        return Err(lifecycle_precommit_after_cleanup(
            &parent,
            &staging_name,
            initial,
            path,
            label,
            error,
        ));
    }
    let publication_parent = match parent.snapshot_after_staging() {
        Ok(snapshot) => snapshot,
        Err(error) => {
            return Err(lifecycle_precommit_after_cleanup(
                &parent,
                &staging_name,
                initial,
                path,
                label,
                error,
            ));
        }
    };
    let staged = (|| -> Result<LifecycleFileSnapshot> {
        staging
            .write_all(bytes)
            .wrap_err_with(|| format!("write exact staged {label}"))?;
        staging
            .sync_all()
            .wrap_err_with(|| format!("sync staged {label}"))?;
        rustix::fs::fchmod(&staging, Mode::from_raw_mode(0o400))
            .wrap_err_with(|| format!("make staged {label} read-only"))?;
        staging
            .sync_all()
            .wrap_err_with(|| format!("sync read-only staged {label}"))?;
        let snapshot = LifecycleFileSnapshot::from_metadata(
            &staging
                .metadata()
                .wrap_err_with(|| format!("inspect staged {label}"))?,
        )
        .ok_or_else(|| eyre!("staged {label} is not a regular file"))?;
        snapshot.validate(0o400, expected_length, &format!("staged {label}"))?;
        Ok(snapshot)
    })();
    let staged = match staged {
        Ok(snapshot) => snapshot,
        Err(error) => {
            return Err(lifecycle_precommit_after_cleanup(
                &parent,
                &staging_name,
                initial,
                path,
                label,
                error,
            ));
        }
    };
    let pre_rename = before_rename(&mut staging, &staging_path)
        .and_then(|()| parent.verify_path_identity_against(publication_parent))
        .and_then(|()| {
            verify_lifecycle_artifact_file(
                &parent,
                &staging_name,
                staged,
                bytes,
                &format!("staged {label}"),
            )
        });
    if let Err(error) = pre_rename {
        return Err(lifecycle_precommit_after_cleanup(
            &parent,
            &staging_name,
            initial,
            path,
            label,
            error,
        ));
    }
    if let Err(error) = rename(&parent.file, &staging_name, target_name) {
        drop(staging);
        return Err(reconcile_lifecycle_rename_error(
            &parent,
            publication_parent,
            &staging_name,
            target_name,
            staged,
            path,
            label,
            &error,
        ));
    }
    drop(staging);

    let final_path = parent.canonical_path.join(target_name);
    let committed = after_rename(&final_path)
        .and_then(|()| {
            verify_lifecycle_artifact_file(
                &parent,
                target_name,
                staged,
                bytes,
                &format!("published {label}"),
            )
        })
        .and_then(|()| {
            sync_parent(&parent.file).wrap_err_with(|| format!("sync published {label} parent"))
        })
        .and_then(|()| parent.verify_path_identity_against(publication_parent))
        .and_then(|()| {
            verify_lifecycle_artifact_file(
                &parent,
                target_name,
                staged,
                bytes,
                &format!("durably published {label}"),
            )
        });
    committed.map_err(|error| lifecycle_publication_commit_uncertain(&final_path, label, error))
}

fn read_bounded_stable(path: &Path, maximum: usize, label: &str) -> Result<Vec<u8>> {
    let named_before = fs::symlink_metadata(path).wrap_err_with(|| format!("inspect {label}"))?;
    if named_before.file_type().is_symlink() || !named_before.is_file() {
        bail!("{label} must be a non-symlink regular file");
    }
    let length = usize::try_from(named_before.len()).map_err(|_| eyre!("{label} is too large"))?;
    if length == 0 || length > maximum {
        bail!("{label} must be 1..={maximum} bytes");
    }
    #[cfg(unix)]
    let mut file = File::from(
        rustix::fs::open(
            path,
            rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .wrap_err_with(|| format!("securely open {label}"))?,
    );
    #[cfg(not(unix))]
    let mut file = File::open(path).wrap_err_with(|| format!("open {label}"))?;
    let opened = file
        .metadata()
        .wrap_err_with(|| format!("inspect open {label}"))?;
    if !same_file_snapshot(&named_before, &opened) {
        bail!("{label} changed while it was opened");
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .wrap_err_with(|| format!("reserve {label} buffer"))?;
    std::io::Read::by_ref(&mut file)
        .take(u64::try_from(maximum)?.saturating_add(1))
        .read_to_end(&mut bytes)
        .wrap_err_with(|| format!("read {label}"))?;
    let opened_after = file
        .metadata()
        .wrap_err_with(|| format!("reinspect open {label}"))?;
    let named_after = fs::symlink_metadata(path).wrap_err_with(|| format!("reinspect {label}"))?;
    if bytes.len() != length
        || bytes.len() > maximum
        || !same_file_snapshot(&opened, &opened_after)
        || !same_file_snapshot(&opened_after, &named_after)
    {
        bail!("{label} changed during bounded read");
    }
    Ok(bytes)
}

#[cfg(unix)]
fn same_file_snapshot(left: &Metadata, right: &Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.nlink() == right.nlink()
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
}

#[cfg(not(unix))]
fn same_file_snapshot(left: &Metadata, right: &Metadata) -> bool {
    left.len() == right.len() && left.modified().ok() == right.modified().ok()
}

fn random_nonzero_u32() -> Result<NonZeroU32> {
    let mut rng = OsRng;
    for _ in 0..16 {
        let value = rng
            .try_next_u32()
            .map_err(|error| eyre!("transaction nonce OS RNG failed: {error}"))?;
        if let Some(value) = NonZeroU32::new(value) {
            return Ok(value);
        }
    }
    bail!("transaction nonce OS RNG returned zero repeatedly")
}

fn random_nonce() -> Result<String> {
    let mut bytes = [0_u8; 32];
    OsRng
        .try_fill_bytes(&mut bytes)
        .map_err(|error| eyre!("fee-quote nonce OS RNG failed: {error}"))?;
    Ok(hex::encode(bytes))
}

fn current_unix_ms() -> Result<u64> {
    u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .wrap_err("host clock is before the Unix epoch")?
            .as_millis(),
    )
    .wrap_err("Unix millisecond clock does not fit u64")
}

fn require_fresh_timestamp(timestamp_ms: u64, now_ms: u64) -> Result<()> {
    if timestamp_ms.abs_diff(now_ms) > LIFECYCLE_FEE_QUOTE_MAX_CLOCK_SKEW_MS {
        bail!(
            "lifecycle fee-quote witness is stale or too far in the future; prepare a fresh draft"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use eyre::WrapErr as _;
    use iroha::{
        crypto::{Algorithm, Hash, HashOf, KeyPair},
        data_model::{
            account::{AccountId, MultisigMember},
            block::BlockHeader,
            metadata::Metadata,
            offline::{
                KAGEMUSHA_V4_RELEASE_CANCELLATION_SCHEMA_V1,
                KAGEMUSHA_V4_RELEASE_LIFECYCLE_VERSION_V1, KagemushaExactBytesDigestV1,
                KagemushaV4ReleaseCancellationV1, KagemushaV4ReleaseLifecycleReasonV1,
            },
            transaction::{FeePaymentIntent, TransactionDomain},
        },
    };
    use iroha_primitives::json::Json;
    use std::{num::NonZeroU64, str::FromStr as _};

    fn key(seed: u8) -> KeyPair {
        KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).expect("fixture key")
    }

    fn cancel_instruction() -> InstructionBox {
        CancelKagemushaRecursiveReleaseV4::new(KagemushaV4ReleaseCancellationV1 {
            schema: KAGEMUSHA_V4_RELEASE_CANCELLATION_SCHEMA_V1.to_owned(),
            version: KAGEMUSHA_V4_RELEASE_LIFECYCLE_VERSION_V1,
            promotion_id: [1; 32],
            manifest_sha256: [2; 32],
            expected_predecessor_lifecycle: KagemushaExactBytesDigestV1 {
                byte_len: 1,
                sha256: [3; 32],
            },
            transition_id: [4; 32],
            reason: KagemushaV4ReleaseLifecycleReasonV1::GovernanceCancelled,
            evidence: None,
        })
        .into()
    }

    fn cancel_payload_with_policy(policy: MultisigPolicy) -> TransactionPayload {
        TransactionBuilder::new(
            iroha::data_model::NetworkId::from_genesis_hash(
                HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
                    b"Kagemusha lifecycle CLI fixture network",
                )),
            ),
            AccountId::new_multisig(policy),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([cancel_instruction()])
        .with_admission_intent(TransactionAdmissionIntent::Ordinary)
        .with_metadata(Metadata::default())
        .into_payload()
        .expect("fixture payload")
    }

    fn cancel_payload(signers: &[KeyPair]) -> TransactionPayload {
        cancel_payload_with_policy(
            MultisigPolicy::new(
                2,
                vec![
                    MultisigMember::new(signers[0].public_key().clone(), 2).expect("member A"),
                    MultisigMember::new(signers[1].public_key().clone(), 1).expect("member B"),
                ],
            )
            .expect("weighted fixture policy"),
        )
    }

    fn bind_payload_to_client(payload: &mut TransactionPayload, client: &Client) {
        payload.domain = TransactionDomain::Network(client.network_id);
    }

    fn fee_quote_draft(payload: TransactionPayload, client: &Client) -> LifecycleFeeQuoteDraftV1 {
        let url = fee_quote_url(client).expect("fixture fee-quote URL");
        let body = fee_quote_body(&payload).expect("fixture fee-quote body");
        LifecycleFeeQuoteDraftV1 {
            schema: LIFECYCLE_FEE_QUOTE_DRAFT_SCHEMA_V1.to_owned(),
            version: LIFECYCLE_FEE_QUOTE_DRAFT_VERSION_V1,
            kind: LifecycleKind::Cancel,
            fee_quote_url: url.to_string(),
            witness: CanonicalRequestWitnessV1 {
                schema_version: CANONICAL_REQUEST_WITNESS_VERSION_V1,
                subject_account: payload.authority.clone(),
                timestamp_ms: current_unix_ms().expect("fixture time"),
                nonce: "fixture-lifecycle-fee-quote".to_owned(),
                canonical_request_hash: canonical_network_request_hash(
                    &client.network_id,
                    &HttpMethod::POST,
                    &url,
                    &body,
                )
                .expect("fixture request hash"),
                signatures: Vec::new(),
            },
            payload,
        }
    }

    fn witness_signatures(
        witness: &CanonicalRequestWitnessV1,
        keys: &[&KeyPair],
    ) -> Vec<CanonicalRequestSignatureWitnessV1> {
        let message = canonical_request_witness_message(witness).expect("fixture witness message");
        let mut signatures = keys
            .iter()
            .map(|key| CanonicalRequestSignatureWitnessV1 {
                signer: key.public_key().clone(),
                signature: Signature::try_new(key.private_key(), &message)
                    .expect("fixture witness signature"),
            })
            .collect::<Vec<_>>();
        signatures.sort_unstable_by(|left, right| left.signer.cmp(&right.signer));
        signatures
    }

    #[test]
    #[rustfmt::skip]
    fn classifier_covers_stage_enable_cancel_and_deactivate() {
        assert_eq!(lifecycle_kind_for_type_id(TypeId::of::<ActivateKagemushaRecursiveReleaseV4>()), Some(LifecycleKind::Stage));
        assert_eq!(lifecycle_kind_for_type_id(TypeId::of::<EnableKagemushaRecursiveIssuanceV4>()), Some(LifecycleKind::Enable));
        assert_eq!(lifecycle_kind_for_type_id(TypeId::of::<CancelKagemushaRecursiveReleaseV4>()), Some(LifecycleKind::Cancel));
        assert_eq!(lifecycle_kind_for_type_id(TypeId::of::<DeactivateKagemushaRecursiveIssuanceV4>()), Some(LifecycleKind::Deactivate));
    }

    #[test]
    #[rustfmt::skip]
    fn stage_metadata_is_anchor_bounded_and_conflicts_fail_closed() {
        let expiry_delta = u64::try_from(KAGEMUSHA_V4_ACTIVATION_FINALITY_PROOF_MAX_COUNT_V1).unwrap() + 1;
        assert!(lifecycle_metadata_with_stage_expiry(Metadata::default(), LifecycleKind::Stage, None).is_err());
        let metadata = lifecycle_metadata_with_stage_expiry(Metadata::default(), LifecycleKind::Stage, Some(10)).expect("synthesize expiry");
        assert_eq!(metadata_expires_at_height(&metadata).unwrap(), Some(10 + expiry_delta));
        lifecycle_metadata_with_stage_expiry(metadata.clone(), LifecycleKind::Stage, Some(10))
            .expect("an exact configured expiry is accepted");
        let mut conflicting = Metadata::default();
        conflicting.insert("expires_at_height".parse().unwrap(), 11_u64);
        assert!(lifecycle_metadata_with_stage_expiry(conflicting, LifecycleKind::Stage, Some(10)).is_err());
        let mut malformed = Metadata::default();
        malformed.insert("expires_at_height".parse().unwrap(), "not-a-height");
        assert!(lifecycle_metadata_with_stage_expiry(malformed, LifecycleKind::Stage, Some(10)).is_err());
        assert!(lifecycle_metadata_with_stage_expiry(Metadata::default(), LifecycleKind::Stage, Some(u64::MAX)).is_err());
        assert!(lifecycle_metadata_with_stage_expiry(Metadata::default(), LifecycleKind::Cancel, Some(10)).is_err());
    }

    #[test] #[rustfmt::skip]
    fn stage_archives_without_height_expiry_fail_before_instruction_use() { let signers = [key(0x35), key(0x36)]; let missing = require_lifecycle_payload(&cancel_payload(&signers), LifecycleKind::Stage).expect_err("missing Stage expiry"); assert!(missing.to_string().contains("expires_at_height")); }

    #[test]
    #[rustfmt::skip]
    fn lifecycle_detached_signing_preserves_fallback_config() {
        let fee_quote = Args { command: Command::SignFeeQuote(SignFeeQuote {
            kind: LifecycleKind::Cancel, governance_authority: "fixture-authority".to_owned(), expected_network_id: "fixture-network".to_owned(),
            expected_draft_sha256: "00".repeat(32), draft: PathBuf::from("draft.norito"), output: PathBuf::from("fee-signature.norito"),
        }) };
        assert!(fee_quote.allows_fallback_config());
        let transaction = Args { command: Command::SignTransaction(SignTransaction {
            kind: LifecycleKind::Cancel, governance_authority: "fixture-authority".to_owned(), expected_network_id: "fixture-network".to_owned(),
            expected_payload_sha256: "00".repeat(32), payload: PathBuf::from("payload.norito"), output: PathBuf::from("transaction-signature.norito"),
        }) };
        assert!(transaction.allows_fallback_config());
    }

    #[test]
    fn lifecycle_operator_key_preflight_rejects_bad_digest_and_network() {
        let signers = [key(0x39), key(0x3a)];
        let client = Client::new(crate::fallback_config());
        let mut payload = cancel_payload(&signers);
        bind_payload_to_client(&mut payload, &client);
        let authority = payload.authority.to_string();
        let expected_network = payload.network_id().expect("fixture network").to_string();
        let draft = fee_quote_draft(payload.clone(), &client);
        let draft_bytes = norito::encode_canonical(&draft).expect("canonical draft");
        let root = tempfile::tempdir().expect("signing preflight root");
        let draft_path = root.path().join("draft.norito");
        fs::write(&draft_path, &draft_bytes).expect("write draft fixture");

        let bad_digest = Args {
            command: Command::SignFeeQuote(SignFeeQuote {
                kind: LifecycleKind::Cancel,
                governance_authority: authority.clone(),
                expected_network_id: expected_network,
                expected_draft_sha256: "00".repeat(32),
                draft: draft_path,
                output: root.path().join("fee-signature.norito"),
            }),
        };
        assert!(
            bad_digest.preflight_before_operator_key_load().is_err(),
            "a wrong draft digest must fail the key-free root preflight"
        );

        let payload_bytes = TransactionBuilder::from_payload(payload)
            .expect("fixture payload builder")
            .encode_payload();
        let payload_path = root.path().join("payload.norito");
        fs::write(&payload_path, &payload_bytes).expect("write payload fixture");
        let wrong_network = iroha::data_model::NetworkId::from_genesis_hash(
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"preflight wrong network")),
        );
        let bad_network = Args {
            command: Command::SignTransaction(SignTransaction {
                kind: LifecycleKind::Cancel,
                governance_authority: authority,
                expected_network_id: wrong_network.to_string(),
                expected_payload_sha256: hex::encode(Sha256::digest(&payload_bytes)),
                payload: payload_path,
                output: root.path().join("transaction-signature.norito"),
            }),
        };
        assert!(
            bad_network.preflight_before_operator_key_load().is_err(),
            "a wrong NetworkId must fail the key-free root preflight"
        );
    }

    #[test]
    fn lifecycle_operator_key_preflight_precedes_key_file_load_in_root() {
        let root_source = include_str!("../main_shared.rs");
        let chain_guard = root_source
            .find("ChainDiscriminantGuard::enter(config.account_chain_discriminant)")
            .expect("root chain-discriminant guard");
        let preflight = root_source
            .find(".preflight_before_operator_key_load()")
            .expect("root lifecycle signing preflight");
        let operator_key_load = root_source
            .find(".map(operator_key::load_operator_key_pair)")
            .expect("root operator-key loader");
        assert!(
            chain_guard < preflight && preflight < operator_key_load,
            "the configured I105 guard and complete pin preflight must precede operator-key file access"
        );
    }

    #[test]
    fn weighted_single_member_threshold_still_requires_two_distinct_signers() {
        let signers = [key(0x41), key(0x42)];
        let payload = cancel_payload(&signers);
        let one = MultisigSignatures::from_signers(&payload, [signers[0].private_key()])
            .expect("weight-two signer")
            .signatures;
        assert!(assemble_transaction(payload.clone(), one).is_err());

        let two = MultisigSignatures::from_signers(
            &payload,
            [signers[0].private_key(), signers[1].private_key()],
        )
        .expect("two distinct signers")
        .signatures;
        let transaction = assemble_transaction(payload, two).expect("two-signer assembly");
        require_lifecycle_transaction(&transaction, LifecycleKind::Cancel)
            .expect("exact cancel transaction");
    }

    #[test]
    fn payload_and_signature_drift_fail_closed() {
        let signers = [key(0x51), key(0x52)];
        let payload = cancel_payload(&signers);
        require_authority_match(&payload.authority, &payload).expect("matching authority pin");
        let other_authority = cancel_payload(&[key(0x57), key(0x58)]).authority;
        assert!(
            require_authority_match(&other_authority, &payload).is_err(),
            "an independently supplied governance authority must pin the archive"
        );
        let signatures = MultisigSignatures::from_signers(
            &payload,
            [signers[0].private_key(), signers[1].private_key()],
        )
        .expect("detached signatures")
        .signatures;
        macro_rules! assert_bound {
            ($label:literal, $mutate:expr) => {{
                let mut drifted = payload.clone();
                ($mutate)(&mut drifted);
                assert!(
                    assemble_transaction(drifted, signatures.clone()).is_err(),
                    "{} drift must invalidate the detached signatures",
                    $label
                );
            }};
        }
        assert_bound!("network", |drifted: &mut TransactionPayload| {
            drifted.domain =
                TransactionDomain::Network(iroha::data_model::NetworkId::from_genesis_hash(
                    HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"foreign network")),
                ));
        });
        assert_bound!("authority", |drifted: &mut TransactionPayload| {
            drifted.authority = AccountId::new(key(0x59).public_key().clone());
        });
        assert_bound!("creation time", |drifted: &mut TransactionPayload| {
            drifted.creation_time_ms = drifted.creation_time_ms.saturating_add(1);
        });
        assert_bound!("TTL", |drifted: &mut TransactionPayload| {
            drifted.time_to_live_ms = NonZeroU64::new(200_000);
        });
        assert_bound!("nonce", |drifted: &mut TransactionPayload| {
            drifted.nonce = NonZeroU32::new(7);
        });
        assert_bound!("fee intent", |drifted: &mut TransactionPayload| {
            drifted.fee_payment = FeePaymentIntent::authority(Vec::new(), NonZeroU64::new(11));
        });
        assert_bound!("metadata", |drifted: &mut TransactionPayload| {
            let _ = drifted.metadata.insert(
                iroha::data_model::name::Name::from_str("lifecycle_drift").expect("metadata name"),
                Json::new(1_u32),
            );
        });
    }

    #[test]
    fn assembly_rejects_duplicate_nonmember_reordered_and_below_threshold_signatures() {
        let signers = [key(0x53), key(0x54), key(0x55)];
        let payload = cancel_payload(&signers);
        let one = MultisigSignatures::from_signers(&payload, [signers[0].private_key()])
            .expect("member signature")
            .signatures
            .pop()
            .expect("one signature");
        assert!(
            assemble_transaction(payload.clone(), vec![one.clone(), one]).is_err(),
            "duplicate signer keys must fail"
        );

        let nonmember = MultisigSignatures::from_signers(
            &payload,
            [signers[0].private_key(), signers[2].private_key()],
        )
        .expect("raw member and nonmember signatures")
        .signatures;
        assert!(
            assemble_transaction(payload.clone(), nonmember).is_err(),
            "nonmember signatures must fail"
        );

        let mut reordered = MultisigSignatures::from_signers(
            &payload,
            [signers[0].private_key(), signers[1].private_key()],
        )
        .expect("ordered signatures")
        .signatures;
        reordered.reverse();
        assert!(
            assemble_transaction(payload, reordered).is_err(),
            "reordered artifacts must fail"
        );

        let threshold_payload = cancel_payload_with_policy(
            MultisigPolicy::new(
                3,
                signers
                    .iter()
                    .map(|key| {
                        MultisigMember::new(key.public_key().clone(), 1).expect("threshold member")
                    })
                    .collect(),
            )
            .expect("three-of-three policy"),
        );
        let below_threshold = MultisigSignatures::from_signers(
            &threshold_payload,
            [signers[0].private_key(), signers[1].private_key()],
        )
        .expect("two signatures below threshold")
        .signatures;
        assert!(
            assemble_transaction(threshold_payload, below_threshold).is_err(),
            "two distinct signers must not bypass a higher policy threshold"
        );
    }

    #[test]
    fn fee_quote_witness_enforces_distinct_floor_membership_order_and_weight() {
        let signers = [key(0x71), key(0x72), key(0x73)];
        let client = Client::new(crate::fallback_config());

        let mut weighted_payload = cancel_payload(&signers);
        bind_payload_to_client(&mut weighted_payload, &client);
        let weighted_draft = fee_quote_draft(weighted_payload, &client);
        let mut one_weighted = weighted_draft.witness.clone();
        one_weighted.signatures = witness_signatures(&one_weighted, &[&signers[0]]);
        assert!(
            client
                .quote_fees_with_multisig_witness(&weighted_draft.payload, &one_weighted)
                .is_err(),
            "one weight-two signer must not bypass the distinct-member floor"
        );

        let threshold_policy = MultisigPolicy::new(
            3,
            signers
                .iter()
                .map(|key| MultisigMember::new(key.public_key().clone(), 1).expect("member"))
                .collect(),
        )
        .expect("three-of-three policy");
        let mut threshold_payload = cancel_payload_with_policy(threshold_policy);
        bind_payload_to_client(&mut threshold_payload, &client);
        let threshold_draft = fee_quote_draft(threshold_payload, &client);
        let mut below_threshold = threshold_draft.witness.clone();
        below_threshold.signatures =
            witness_signatures(&below_threshold, &[&signers[0], &signers[1]]);
        assert!(
            client
                .quote_fees_with_multisig_witness(&threshold_draft.payload, &below_threshold)
                .is_err(),
            "two distinct signers must not bypass a higher weight threshold"
        );

        let mut duplicate = weighted_draft.witness.clone();
        let one = witness_signatures(&duplicate, &[&signers[0]])
            .pop()
            .expect("one witness signature");
        duplicate.signatures = vec![one.clone(), one];
        assert!(
            client
                .quote_fees_with_multisig_witness(&weighted_draft.payload, &duplicate)
                .is_err()
        );

        let mut nonmember = weighted_draft.witness.clone();
        nonmember.signatures = witness_signatures(&nonmember, &[&signers[0], &signers[2]]);
        assert!(
            client
                .quote_fees_with_multisig_witness(&weighted_draft.payload, &nonmember)
                .is_err()
        );

        let mut out_of_order = weighted_draft.witness.clone();
        out_of_order.signatures = witness_signatures(&out_of_order, &[&signers[0], &signers[1]]);
        out_of_order.signatures.reverse();
        assert!(
            client
                .quote_fees_with_multisig_witness(&weighted_draft.payload, &out_of_order)
                .is_err()
        );

        let mut bad_signature = weighted_draft.witness.clone();
        bad_signature.signatures = witness_signatures(&bad_signature, &[&signers[0], &signers[1]]);
        bad_signature.signatures[0].signature =
            Signature::try_new(signers[0].private_key(), b"wrong fee-quote request")
                .expect("bad fixture signature");
        assert!(
            client
                .quote_fees_with_multisig_witness(&weighted_draft.payload, &bad_signature)
                .is_err()
        );

        let mut wrong_nonce = weighted_draft.witness.clone();
        wrong_nonce.signatures = witness_signatures(&wrong_nonce, &[&signers[0], &signers[1]]);
        wrong_nonce.nonce.push_str("-drift");
        assert!(
            client
                .quote_fees_with_multisig_witness(&weighted_draft.payload, &wrong_nonce)
                .is_err(),
            "witness signatures must bind the exact freshness nonce"
        );
    }

    #[test]
    fn fee_quote_draft_binds_request_network_authority_payload_url_and_response_intent() {
        let signers = [key(0x74), key(0x75)];
        let client = Client::new(crate::fallback_config());
        let mut payload = cancel_payload(&signers);
        bind_payload_to_client(&mut payload, &client);
        let draft = fee_quote_draft(payload, &client);
        validate_fee_quote_draft(&draft, LifecycleKind::Cancel).expect("valid draft");
        require_configured_fee_quote_binding(&draft, &client).expect("configured binding");

        let mut wrong_hash = draft.clone();
        wrong_hash.witness.canonical_request_hash = Hash::new(b"wrong request");
        assert!(validate_fee_quote_draft(&wrong_hash, LifecycleKind::Cancel).is_err());

        let mut wrong_authority = draft.clone();
        wrong_authority.witness.subject_account = AccountId::new(key(0x76).public_key().clone());
        assert!(validate_fee_quote_draft(&wrong_authority, LifecycleKind::Cancel).is_err());

        let mut payload_drift = draft.clone();
        payload_drift.payload.nonce = NonZeroU32::new(99);
        assert!(validate_fee_quote_draft(&payload_drift, LifecycleKind::Cancel).is_err());
        assert!(validate_fee_quote_draft(&draft, LifecycleKind::Enable).is_err());

        let mut wrong_url = draft.clone();
        wrong_url.fee_quote_url = "https://different.invalid/v1/fees/quote".to_owned();
        let body = fee_quote_body(&wrong_url.payload).expect("request body");
        wrong_url.witness.canonical_request_hash = canonical_network_request_hash(
            wrong_url.payload.network_id().expect("network"),
            &HttpMethod::POST,
            &Url::parse(&wrong_url.fee_quote_url).expect("different URL"),
            &body,
        )
        .expect("different request hash");
        validate_fee_quote_draft(&wrong_url, LifecycleKind::Cancel)
            .expect("self-consistent alternate URL");
        assert!(require_configured_fee_quote_binding(&wrong_url, &client).is_err());

        let mut wrong_network_client = client.clone();
        wrong_network_client.network_id = iroha::data_model::NetworkId::from_genesis_hash(
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"other client network")),
        );
        assert!(require_configured_fee_quote_binding(&draft, &wrong_network_client).is_err());

        let mut quoted_payload = draft.payload;
        assert!(
            apply_fee_quote_intent(
                &mut quoted_payload,
                FeePaymentIntent::authority(Vec::new(), NonZeroU64::new(1)),
            )
            .is_err(),
            "quote response must not change the selected gas bound"
        );
    }

    #[test]
    fn lifecycle_signing_pins_reject_wrong_network_and_artifact_digest() {
        let signers = [key(0x77), key(0x78)];
        let client = Client::new(crate::fallback_config());
        let mut payload = cancel_payload(&signers);
        bind_payload_to_client(&mut payload, &client);
        let draft = fee_quote_draft(payload.clone(), &client);
        let draft_bytes = norito::encode_canonical(&draft).expect("canonical draft");
        let draft_sha256 = hex::encode(Sha256::digest(&draft_bytes));
        let expected_network = payload.network_id().expect("fixture network").to_string();

        require_expected_artifact_sha256(
            &draft_bytes,
            &draft_sha256.to_uppercase(),
            "--expected-draft-sha256",
            "lifecycle fee-quote draft",
        )
        .expect("exact draft digest pin");
        require_expected_signing_network(&expected_network, &payload, "lifecycle fee-quote draft")
            .expect("exact draft network pin");

        assert!(
            require_expected_artifact_sha256(
                &draft_bytes,
                &"00".repeat(32),
                "--expected-draft-sha256",
                "lifecycle fee-quote draft",
            )
            .is_err(),
            "a signer must reject a wrong draft digest"
        );
        assert!(
            require_expected_artifact_sha256(
                &draft_bytes,
                "00",
                "--expected-draft-sha256",
                "lifecycle fee-quote draft",
            )
            .is_err(),
            "a signer must reject an ambiguously sized digest"
        );

        let wrong_network = iroha::data_model::NetworkId::from_genesis_hash(
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"wrong signing network")),
        );
        assert!(
            require_expected_signing_network(
                &wrong_network.to_string(),
                &payload,
                "quoted lifecycle transaction payload",
            )
            .is_err(),
            "a signer must reject a wrong independently supplied NetworkId"
        );

        let payload_bytes = TransactionBuilder::from_payload(payload)
            .expect("fixture payload builder")
            .encode_payload();
        let payload_sha256 = hex::encode(Sha256::digest(&payload_bytes));
        require_expected_artifact_sha256(
            &payload_bytes,
            &payload_sha256,
            "--expected-payload-sha256",
            "quoted lifecycle transaction payload",
        )
        .expect("exact payload digest pin");
        assert!(
            require_expected_artifact_sha256(
                &payload_bytes,
                &"ff".repeat(32),
                "--expected-payload-sha256",
                "quoted lifecycle transaction payload",
            )
            .is_err(),
            "a transaction signer must reject a wrong payload digest"
        );
    }

    #[test]
    fn lifecycle_fee_quote_transport_requires_https_except_exact_loopback() {
        for allowed in [
            "https://fees.example/v1/fees/quote",
            "http://localhost:8080/v1/fees/quote",
            "http://127.0.0.1:8080/v1/fees/quote",
            "http://127.200.30.40:8080/v1/fees/quote",
            "http://[::1]:8080/v1/fees/quote",
        ] {
            require_secure_fee_quote_origin(&Url::parse(allowed).expect("allowed URL"))
                .unwrap_or_else(|error| panic!("allowed fee-quote origin {allowed}: {error:#}"));
        }
        for rejected in [
            "http://fees.example/v1/fees/quote",
            "http://localhost.example/v1/fees/quote",
            "http://localhost./v1/fees/quote",
            "http://126.255.255.255/v1/fees/quote",
            "http://[::2]/v1/fees/quote",
        ] {
            assert!(
                require_secure_fee_quote_origin(&Url::parse(rejected).expect("rejected URL"))
                    .is_err(),
                "non-loopback HTTP origin must fail: {rejected}"
            );
        }

        let signers = [key(0x79), key(0x7a)];
        let client = Client::new(crate::fallback_config());
        let mut payload = cancel_payload(&signers);
        bind_payload_to_client(&mut payload, &client);
        let local = fee_quote_draft(payload, &client);
        validate_fee_quote_draft(&local, LifecycleKind::Cancel)
            .expect("fallback localhost HTTP remains available offline");

        let mut remote_http = local;
        remote_http.fee_quote_url = "http://fees.example/v1/fees/quote".to_owned();
        let url = Url::parse(&remote_http.fee_quote_url).expect("remote HTTP URL");
        let body = fee_quote_body(&remote_http.payload).expect("remote request body");
        remote_http.witness.canonical_request_hash = canonical_network_request_hash(
            remote_http.payload.network_id().expect("fixture network"),
            &HttpMethod::POST,
            &url,
            &body,
        )
        .expect("self-consistent remote HTTP request hash");
        assert!(
            validate_fee_quote_draft(&remote_http, LifecycleKind::Cancel).is_err(),
            "a self-consistent witness must not make remote HTTP acceptable"
        );
    }

    #[test]
    fn nonordinary_and_multi_instruction_carriers_are_rejected() {
        let signers = [key(0x61), key(0x62)];
        let mut payload = cancel_payload(&signers);
        payload.admission_intent = TransactionAdmissionIntent::QueuePlanSynced;
        assert!(require_lifecycle_payload(&payload, LifecycleKind::Cancel).is_err());

        payload.admission_intent = TransactionAdmissionIntent::Ordinary;
        payload.instructions =
            Executable::Instructions(vec![cancel_instruction(), cancel_instruction()].into());
        assert!(require_lifecycle_payload(&payload, LifecycleKind::Cancel).is_err());
        assert!(
            require_lifecycle_payload(&cancel_payload(&signers), LifecycleKind::Enable).is_err()
        );
    }

    #[test]
    fn fee_quote_timestamp_rejects_stale_and_future_material() {
        let now = 1_000_000;
        require_fresh_timestamp(now, now).expect("current witness");
        assert!(
            require_fresh_timestamp(now - LIFECYCLE_FEE_QUOTE_MAX_CLOCK_SKEW_MS - 1, now).is_err()
        );
        assert!(
            require_fresh_timestamp(now + LIFECYCLE_FEE_QUOTE_MAX_CLOCK_SKEW_MS + 1, now).is_err()
        );

        let signers = [key(0x63), key(0x64)];
        let mut expired = cancel_payload(&signers);
        expired.creation_time_ms = 1;
        assert!(require_live_transaction_clock(&expired).is_err());
        let mut future = cancel_payload(&signers);
        future.creation_time_ms = current_unix_ms()
            .expect("current time")
            .saturating_add(LIFECYCLE_FEE_QUOTE_MAX_CLOCK_SKEW_MS + 1);
        assert!(require_live_transaction_clock(&future).is_err());

        let client = Client::new(crate::fallback_config());
        let mut payload = cancel_payload(&signers);
        bind_payload_to_client(&mut payload, &client);
        let draft = fee_quote_draft(payload, &client);
        let current = current_unix_ms().expect("current time");
        let mut stale = draft.clone();
        stale.witness.timestamp_ms =
            current.saturating_sub(LIFECYCLE_FEE_QUOTE_MAX_CLOCK_SKEW_MS + 1);
        assert!(validate_fee_quote_draft(&stale, LifecycleKind::Cancel).is_err());
        let mut not_yet_valid = draft;
        not_yet_valid.witness.timestamp_ms =
            current.saturating_add(LIFECYCLE_FEE_QUOTE_MAX_CLOCK_SKEW_MS + 1);
        assert!(validate_fee_quote_draft(&not_yet_valid, LifecycleKind::Cancel).is_err());
    }

    #[test]
    fn raw_submission_preserves_authorized_wire_before_io() {
        let signers = [key(0x65), key(0x66)];
        let payload = cancel_payload(&signers);
        let signatures = MultisigSignatures::from_signers(
            &payload,
            [signers[0].private_key(), signers[1].private_key()],
        )
        .expect("detached signatures")
        .signatures;
        let transaction = assemble_transaction(payload, signatures).expect("assembled transaction");
        let authorized = transaction.encode_wire_v1().expect("authorized wire");
        let (decoded, prepared) =
            prepare_exact_lifecycle_submission(&authorized, LifecycleKind::Cancel)
                .expect("exact prepared transaction");
        assert_eq!(decoded.hash(), transaction.hash());
        assert_eq!(prepared.as_bytes(), authorized);

        let mut drifted = authorized;
        drifted.push(0);
        assert!(
            prepare_exact_lifecycle_submission(&drifted, LifecycleKind::Cancel).is_err(),
            "noncanonical or drifted wire must fail before a client is asked to submit"
        );
    }

    #[cfg(unix)]
    #[test]
    fn verified_receipt_publication_race_is_post_acknowledgement_and_no_retry() {
        let signers = [key(0x67), key(0x68)];
        let payload = cancel_payload(&signers);
        let signatures = MultisigSignatures::from_signers(
            &payload,
            [signers[0].private_key(), signers[1].private_key()],
        )
        .expect("detached signatures")
        .signatures;
        let transaction = assemble_transaction(payload, signatures).expect("assembled transaction");
        let receipt_signer = key(0x69);
        let receipt = TransactionSubmissionReceipt::sign(
            iroha::data_model::transaction::TransactionSubmissionReceiptPayload {
                entrypoint_hash: transaction.hash_as_entrypoint(),
                signed_transaction_hash: Some(transaction.hash()),
                submitted_at_ms: 1,
                submitted_at_height: 1,
                signer: receipt_signer.public_key().clone(),
            },
            &receipt_signer,
        );
        receipt.verify().expect("valid signed receipt fixture");
        let (_root, _parent, target) = lifecycle_publication_fixture("receipt.norito");
        fs::write(&target, b"foreign receipt").expect("inject post-precheck destination race");

        let error =
            encode_and_publish_verified_lifecycle_receipt(&receipt, transaction.hash(), &target)
                .expect_err("occupied receipt target must fail after durable acknowledgement");
        assert!(matches!(
            &error,
            LifecyclePostAcknowledgementError::ReceiptProcessing { phase, .. }
                if *phase == "publication"
        ));
        let rendered = error.to_string();
        assert!(rendered.contains("was durably acknowledged"));
        assert!(rendered.contains("do not retry automatically"));
        assert!(rendered.contains("do not treat"));
        assert!(rendered.contains(&target.display().to_string()));
        assert_eq!(
            fs::read(target).expect("preserved raced destination"),
            b"foreign receipt"
        );
    }

    #[cfg(unix)]
    #[test]
    fn archive_publication_is_no_replace_and_read_only() {
        use std::os::unix::fs::PermissionsExt as _;

        let directory = tempfile::tempdir().expect("lifecycle archive directory");
        let path = directory.path().join("payload.norito");
        publish_no_replace(&path, b"exact-payload", "test lifecycle payload")
            .expect("first publication");
        assert_eq!(
            fs::metadata(&path)
                .expect("published metadata")
                .permissions()
                .mode()
                & 0o777,
            0o400
        );
        assert!(
            publish_no_replace(&path, b"replacement", "test lifecycle payload").is_err(),
            "an existing archive must never be replaced"
        );
        assert_eq!(fs::read(path).expect("published bytes"), b"exact-payload");
    }

    #[cfg(unix)]
    fn lifecycle_publication_fixture(name: &str) -> (tempfile::TempDir, PathBuf, PathBuf) {
        use std::os::unix::fs::PermissionsExt as _;

        let root = tempfile::tempdir().expect("lifecycle publication root");
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700))
            .expect("secure lifecycle publication root");
        let parent = root.path().join("archive");
        fs::create_dir(&parent).expect("create lifecycle publication parent");
        fs::set_permissions(&parent, fs::Permissions::from_mode(0o700))
            .expect("secure lifecycle publication parent");
        let target = parent.join(name);
        (root, parent, target)
    }

    #[cfg(unix)]
    #[test]
    fn lifecycle_publication_rejects_partial_staged_write_before_commit() {
        let (_root, parent, target) = lifecycle_publication_fixture("payload.norito");
        let error = publish_no_replace_with_hooks(
            &target,
            b"exact lifecycle payload",
            "test lifecycle payload",
            |staging, _| {
                staging.set_len(4).wrap_err("inject partial staged write")?;
                Ok(())
            },
            rename_lifecycle_staging_no_replace,
            |_| Ok(()),
            File::sync_all,
        )
        .expect_err("a partial staged write must fail before publication");
        assert!(matches!(
            error,
            LifecycleArtifactPublicationError::PreCommit { .. }
        ));
        assert!(!target.exists());
        assert_eq!(
            fs::read_dir(parent)
                .expect("inspect cleaned publication parent")
                .count(),
            0
        );
    }

    #[cfg(unix)]
    #[test]
    fn lifecycle_publication_rejects_parent_substitution_before_commit() {
        use std::os::unix::fs::PermissionsExt as _;

        let (_root, parent, target) = lifecycle_publication_fixture("payload.norito");
        let displaced = parent.with_file_name("displaced-archive");
        let error = publish_no_replace_with_hooks(
            &target,
            b"exact lifecycle payload",
            "test lifecycle payload",
            |_, _| {
                fs::rename(&parent, &displaced).wrap_err("displace pinned parent")?;
                fs::create_dir(&parent).wrap_err("create impostor parent")?;
                fs::set_permissions(&parent, fs::Permissions::from_mode(0o700))
                    .wrap_err("secure impostor parent")?;
                fs::write(parent.join("attacker-sentinel"), b"preserve")
                    .wrap_err("write attacker sentinel")?;
                Ok(())
            },
            rename_lifecycle_staging_no_replace,
            |_| Ok(()),
            File::sync_all,
        )
        .expect_err("a substituted parent must fail before publication");
        assert!(matches!(
            error,
            LifecycleArtifactPublicationError::PreCommit { .. }
        ));
        assert!(
            !target.exists(),
            "the impostor parent must receive no artifact"
        );
        assert!(
            !displaced.join("payload.norito").exists(),
            "the pinned parent must receive no final artifact after substitution"
        );
        assert_eq!(
            fs::read(parent.join("attacker-sentinel")).expect("read attacker sentinel"),
            b"preserve"
        );
    }

    #[cfg(unix)]
    #[test]
    fn lifecycle_publication_noreplace_race_is_precommit_and_preserves_destination() {
        let (_root, parent, target) = lifecycle_publication_fixture("payload.norito");
        let error = publish_no_replace_with_hooks(
            &target,
            b"exact lifecycle payload",
            "test lifecycle payload",
            |_, _| {
                fs::write(&target, b"attacker destination").wrap_err("inject destination race")?;
                Ok(())
            },
            rename_lifecycle_staging_no_replace,
            |_| Ok(()),
            File::sync_all,
        )
        .expect_err("an occupied destination must defeat the no-replace rename");
        assert!(matches!(
            error,
            LifecycleArtifactPublicationError::PreCommit { .. }
        ));
        assert_eq!(
            fs::read(&target).expect("read preserved destination"),
            b"attacker destination"
        );
        assert_eq!(
            fs::read_dir(parent)
                .expect("inspect no-replace publication parent")
                .count(),
            1,
            "the owned staging inode must be cleaned without deleting the destination"
        );
    }

    #[cfg(unix)]
    #[test]
    fn lifecycle_publication_rename_error_with_intact_staging_is_precommit() {
        let (_root, parent, target) = lifecycle_publication_fixture("payload.norito");
        let error = publish_no_replace_with_hooks(
            &target,
            b"exact lifecycle payload",
            "test lifecycle payload",
            |_, _| Ok(()),
            |_, _, _| Err(eyre!("injected pre-commit rename failure")),
            |_| Ok(()),
            File::sync_all,
        )
        .expect_err("an unchanged namespace must reconcile as pre-commit");
        assert!(matches!(
            error,
            LifecycleArtifactPublicationError::PreCommit { .. }
        ));
        assert!(!target.exists());
        assert_eq!(
            fs::read_dir(parent)
                .expect("inspect reconciled publication parent")
                .count(),
            0,
            "the exact owned staging inode must be removed"
        );
    }

    #[cfg(unix)]
    #[test]
    fn lifecycle_publication_lost_rename_ack_is_commit_uncertain() {
        let (_root, parent, target) = lifecycle_publication_fixture("payload.norito");
        let error = publish_no_replace_with_hooks(
            &target,
            b"exact lifecycle payload",
            "test lifecycle payload",
            |_, _| Ok(()),
            |parent, staging_name, target_name| {
                rename_lifecycle_staging_no_replace(parent, staging_name, target_name)?;
                Err(eyre!("injected lost rename acknowledgement"))
            },
            |_| Ok(()),
            File::sync_all,
        )
        .expect_err("a lost successful-rename acknowledgement must be commit-uncertain");
        match error {
            LifecycleArtifactPublicationError::CommitUncertain { detail, .. } => {
                assert!(detail.contains("destination binds the owned staged inode"));
            }
            LifecycleArtifactPublicationError::PreCommit { .. } => {
                panic!("a visible owned destination cannot be reported as pre-commit")
            }
        }
        assert_eq!(
            fs::read(&target).expect("read visible commit-uncertain artifact"),
            b"exact lifecycle payload"
        );
        assert_eq!(
            fs::read_dir(parent)
                .expect("inspect lost-ack publication parent")
                .count(),
            1,
            "reconciliation must leave the final owned inode in place"
        );
    }

    #[cfg(unix)]
    #[test]
    fn lifecycle_publication_missing_names_after_rename_error_is_commit_uncertain() {
        let (_root, parent, target) = lifecycle_publication_fixture("payload.norito");
        let error = publish_no_replace_with_hooks(
            &target,
            b"exact lifecycle payload",
            "test lifecycle payload",
            |_, _| Ok(()),
            |parent, staging_name, _| {
                rustix::fs::unlinkat(parent, staging_name, rustix::fs::AtFlags::empty())
                    .wrap_err("inject missing staging binding")?;
                Err(eyre!("injected ambiguous rename failure"))
            },
            |_| Ok(()),
            File::sync_all,
        )
        .expect_err("missing namespace evidence must be commit-uncertain");
        assert!(matches!(
            error,
            LifecycleArtifactPublicationError::CommitUncertain { .. }
        ));
        assert!(!target.exists());
        assert_eq!(
            fs::read_dir(parent)
                .expect("inspect ambiguous publication parent")
                .count(),
            0
        );
    }

    #[cfg(unix)]
    #[test]
    fn lifecycle_publication_parent_drift_preserves_staging_during_rename_reconciliation() {
        use std::os::unix::fs::PermissionsExt as _;

        let (_root, parent, target) = lifecycle_publication_fixture("payload.norito");
        let displaced = parent.with_file_name("displaced-archive-after-rename");
        let error = publish_no_replace_with_hooks(
            &target,
            b"exact lifecycle payload",
            "test lifecycle payload",
            |_, _| Ok(()),
            |_, _, _| {
                fs::rename(&parent, &displaced).wrap_err("displace publication parent")?;
                fs::create_dir(&parent).wrap_err("create replacement parent")?;
                fs::set_permissions(&parent, fs::Permissions::from_mode(0o700))
                    .wrap_err("secure replacement parent")?;
                Err(eyre!("injected rename error after parent drift"))
            },
            |_| Ok(()),
            File::sync_all,
        )
        .expect_err("parent drift during rename reconciliation must be commit-uncertain");
        assert!(matches!(
            error,
            LifecycleArtifactPublicationError::CommitUncertain { .. }
        ));
        assert!(!target.exists());
        assert_eq!(
            fs::read_dir(displaced)
                .expect("inspect pinned displaced parent")
                .count(),
            1,
            "parent drift must leave the exact staging evidence untouched"
        );
    }

    #[cfg(unix)]
    #[test]
    fn lifecycle_publication_reports_post_rename_replacement_as_commit_uncertain() {
        let (_root, _parent, target) = lifecycle_publication_fixture("payload.norito");
        let error = publish_no_replace_with_hooks(
            &target,
            b"exact lifecycle payload",
            "test lifecycle payload",
            |_, _| Ok(()),
            rename_lifecycle_staging_no_replace,
            |published| {
                fs::remove_file(published).wrap_err("remove committed inode")?;
                fs::write(published, b"attacker replacement")
                    .wrap_err("replace committed inode")?;
                Ok(())
            },
            File::sync_all,
        )
        .expect_err("post-rename replacement must be an uncertain commit");
        assert!(matches!(
            error,
            LifecycleArtifactPublicationError::CommitUncertain { .. }
        ));
        assert_eq!(
            fs::read(target).expect("read hostile replacement"),
            b"attacker replacement"
        );
    }

    #[cfg(unix)]
    #[test]
    fn lifecycle_publication_reports_parent_sync_failure_as_commit_uncertain() {
        use std::io;

        let (_root, _parent, target) = lifecycle_publication_fixture("payload.norito");
        let error = publish_no_replace_with_hooks(
            &target,
            b"exact lifecycle payload",
            "test lifecycle payload",
            |_, _| Ok(()),
            rename_lifecycle_staging_no_replace,
            |_| Ok(()),
            |_| Err(io::Error::other("injected lifecycle parent sync failure")),
        )
        .expect_err("a failed parent sync must be an uncertain commit");
        match error {
            LifecycleArtifactPublicationError::CommitUncertain { detail, .. } => {
                assert!(detail.contains("injected lifecycle parent sync failure"));
            }
            LifecycleArtifactPublicationError::PreCommit { .. } => {
                panic!("post-rename sync failure cannot be reported as pre-commit")
            }
        }
        assert_eq!(
            fs::read(target).expect("read visible commit-uncertain artifact"),
            b"exact lifecycle payload"
        );
    }
}
