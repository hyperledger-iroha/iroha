//! Exact-payload, detached-multisignature Kagemusha V4 lifecycle corridor.

use crate::{Run, RunContext};
use clap::{Args as ClapArgs, Subcommand, ValueEnum};
use eyre::{Result, WrapErr as _, bail, eyre};
use iroha::{
    client::{Client, canonical_network_request_hash, canonical_request_witness_message},
    data_model::{
        account::{AccountController, MultisigPolicy},
        isi::{
            InstructionBox,
            offline::{
                ActivateKagemushaRecursiveReleaseV4, CancelKagemushaRecursiveReleaseV4,
                DeactivateKagemushaRecursiveIssuanceV4, EnableKagemushaRecursiveIssuanceV4,
            },
        },
        soracloud::{
            CANONICAL_REQUEST_WITNESS_VERSION_V1, CanonicalRequestSignatureWitnessV1,
            CanonicalRequestWitnessV1,
        },
        transaction::{
            Executable, SignedTransaction, TransactionAdmissionIntent, TransactionBuilder,
            TransactionPayload,
            signed::{MultisigSignature, MultisigSignatures},
        },
    },
    http::Method as HttpMethod,
};
use iroha_crypto::Signature;
use iroha_torii_shared::{FeeQuoteRequest, uri as torii_uri};
use iroha_version::codec::DecodeVersioned as _;
use norito::derive::{Decode, Encode};
use rand::{TryRngCore as _, rngs::OsRng};
use std::{
    any::TypeId,
    fs::{self, File, Metadata, OpenOptions},
    io::{Read as _, Write as _},
    num::NonZeroU32,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use url::Url;

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
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Prepare the exact ordinary transaction and authenticated fee-quote draft.
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
        let mut builder = TransactionBuilder::new(client.network_id, authority, fee_payment)
            .with_instructions(instructions)
            .with_admission_intent(TransactionAdmissionIntent::Ordinary)
            .with_metadata(context.transaction_metadata().cloned().unwrap_or_default());
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
    /// Exact canonical fee-quote draft.
    #[arg(long, value_name = "PATH")]
    draft: PathBuf,
    /// Absent destination for this signer's canonical detached signature.
    #[arg(long, value_name = "PATH")]
    output: PathBuf,
}

impl SignFeeQuote {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let draft: LifecycleFeeQuoteDraftV1 =
            read_canonical(&self.draft, "lifecycle fee-quote draft")?;
        validate_fee_quote_draft(&draft, self.kind)?;
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
    /// Exact canonical fee-quote draft.
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
        let client = context.client_from_config();
        if draft.payload.network_id() != Some(&client.network_id) {
            bail!("lifecycle draft network differs from the configured client network");
        }
        let expected_url = fee_quote_url(&client)?;
        let draft_url = Url::parse(&draft.fee_quote_url)
            .wrap_err("lifecycle draft fee-quote URL is invalid")?;
        if draft_url != expected_url {
            bail!("lifecycle draft fee-quote URL differs from the configured Torii route");
        }

        let signatures = read_fee_quote_signatures(&self.signatures)?;
        let mut witness = draft.witness.clone();
        witness.signatures = signatures;
        let quote = client
            .quote_fees_with_multisig_witness(&draft.payload, &witness)
            .wrap_err("failed to obtain authenticated lifecycle fee quote")?;
        if !draft
            .payload
            .fee_payment
            .has_same_payer_and_gas_bound(&quote.intent)
        {
            bail!("fee quote changed the selected payer, sponsor revision, or gas bound");
        }
        quote
            .intent
            .validate()
            .wrap_err("fee quote returned an invalid payment intent")?;
        let mut payload = draft.payload;
        payload.fee_payment = quote.intent;
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
    /// Exact frozen TransactionPayload archive.
    #[arg(long, value_name = "PATH")]
    payload: PathBuf,
    /// Absent destination for this signer's canonical detached signature.
    #[arg(long, value_name = "PATH")]
    output: PathBuf,
}

impl SignTransaction {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let (_, payload) = read_transaction_payload(&self.payload, self.kind)?;
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
    /// Exact canonical versioned SignedTransaction wire.
    #[arg(long, value_name = "PATH")]
    transaction: PathBuf,
    /// Explicit authorization for this production lifecycle write.
    #[arg(long, required = true, action = clap::ArgAction::SetTrue)]
    write_authorized: bool,
}

impl SubmitTransaction {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        if !self.write_authorized {
            bail!("--write-authorized is required for lifecycle submission");
        }
        let bytes = read_bounded_stable(
            &self.transaction,
            LIFECYCLE_ARTIFACT_MAX_BYTES,
            "assembled lifecycle transaction",
        )?;
        let transaction = SignedTransaction::decode_all_versioned(&bytes)
            .wrap_err("failed to decode assembled lifecycle transaction")?;
        let canonical = transaction
            .encode_wire_v1()
            .map_err(|error| eyre!("failed to re-encode lifecycle transaction: {error}"))?;
        if canonical != bytes {
            bail!("assembled lifecycle transaction is not exact canonical V1 wire");
        }
        require_lifecycle_transaction(&transaction, self.kind)?;
        let client = context.client_from_config();
        if transaction.network_id() != Some(&client.network_id) {
            bail!("lifecycle transaction network differs from the configured client network");
        }
        let prepared = Client::prepare_transaction_payload(&transaction);
        if prepared.as_bytes() != bytes {
            bail!("prepared lifecycle submission bytes differ from the authorized archive");
        }
        let hash = client
            .submit_prepared_transaction_payload(&prepared)
            .wrap_err("failed to submit exact lifecycle transaction")?;
        context.println_data(hash)
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
    require_multisig_policy(&payload.authority)?;
    let Executable::Instructions(instructions) = payload.instructions() else {
        bail!("lifecycle transaction must carry native instructions directly");
    };
    let [instruction] = instructions.as_ref() else {
        bail!("lifecycle transaction must carry exactly one native instruction");
    };
    require_lifecycle_instruction(instruction, expected)
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

fn require_multisig_policy(
    authority: &iroha::data_model::account::AccountId,
) -> Result<&MultisigPolicy> {
    let AccountController::Multisig(policy) = authority.controller() else {
        bail!("direct lifecycle authority must be a multisig account");
    };
    if policy.members().len() < 2 {
        bail!("direct lifecycle authority must have at least two policy members");
    }
    Ok(policy)
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
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
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
    client
        .torii_url
        .join(torii_uri::FEES_QUOTE.trim_start_matches('/'))
        .wrap_err("failed to construct exact Torii fee-quote URL")
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

fn read_transaction_payload(
    path: &Path,
    expected: LifecycleKind,
) -> Result<(Vec<u8>, TransactionPayload)> {
    let bytes = read_bounded_stable(
        path,
        LIFECYCLE_ARTIFACT_MAX_BYTES,
        "quoted lifecycle transaction payload",
    )?;
    let builder = TransactionBuilder::decode_payload(&bytes)
        .wrap_err("failed to decode exact lifecycle transaction payload")?;
    let payload = builder
        .into_payload()
        .wrap_err("invalid lifecycle transaction payload")?;
    require_lifecycle_payload(&payload, expected)?;
    Ok((bytes, payload))
}

fn read_canonical<T>(path: &Path, label: &str) -> Result<T>
where
    T: norito::NoritoSerialize + for<'de> norito::NoritoDeserialize<'de>,
{
    let bytes = read_bounded_stable(path, LIFECYCLE_ARTIFACT_MAX_BYTES, label)?;
    let value = norito::decode_canonical_with_limits::<T>(
        &bytes,
        norito::canonical_decode_limits(bytes.len()),
    )
    .wrap_err_with(|| format!("failed to decode canonical {label}"))?;
    let canonical = norito::encode_canonical(&value)
        .wrap_err_with(|| format!("failed to re-encode canonical {label}"))?;
    if canonical != bytes {
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

fn publish_no_replace(path: &Path, bytes: &[u8], label: &str) -> Result<()> {
    if bytes.is_empty() || bytes.len() > LIFECYCLE_ARTIFACT_MAX_BYTES {
        bail!("{label} must be 1..={LIFECYCLE_ARTIFACT_MAX_BYTES} bytes");
    }
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .wrap_err_with(|| format!("create absent {label} destination"))?;
    file.write_all(bytes)
        .wrap_err_with(|| format!("write exact {label}"))?;
    file.sync_all()
        .wrap_err_with(|| format!("sync exact {label}"))?;
    let metadata = file
        .metadata()
        .wrap_err_with(|| format!("inspect published {label}"))?;
    if !metadata.is_file() || metadata.len() != u64::try_from(bytes.len())? {
        bail!("published {label} changed during no-replace write");
    }
    let mut permissions = metadata.permissions();
    permissions.set_readonly(true);
    file.set_permissions(permissions)
        .wrap_err_with(|| format!("make published {label} read-only"))?;
    file.sync_all()
        .wrap_err_with(|| format!("sync read-only {label}"))?;
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .wrap_err_with(|| format!("sync {label} parent directory"))?;
    let observed = read_bounded_stable(path, LIFECYCLE_ARTIFACT_MAX_BYTES, label)?;
    if observed != bytes {
        bail!("published {label} differs from the exact requested bytes");
    }
    Ok(())
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
    use iroha::{
        crypto::{Algorithm, KeyPair},
        data_model::{
            account::{AccountId, MultisigMember},
            metadata::Metadata,
            offline::{
                KAGEMUSHA_V4_RELEASE_CANCELLATION_SCHEMA_V1,
                KAGEMUSHA_V4_RELEASE_LIFECYCLE_VERSION_V1, KagemushaExactBytesDigestV1,
                KagemushaV4ReleaseCancellationV1, KagemushaV4ReleaseLifecycleReasonV1,
            },
            transaction::FeePaymentIntent,
        },
    };

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
            "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0"
                .parse()
                .expect("fixture network id"),
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

    #[test]
    fn classifier_covers_stage_enable_cancel_and_deactivate() {
        assert_eq!(
            lifecycle_kind_for_type_id(TypeId::of::<ActivateKagemushaRecursiveReleaseV4>()),
            Some(LifecycleKind::Stage)
        );
        assert_eq!(
            lifecycle_kind_for_type_id(TypeId::of::<EnableKagemushaRecursiveIssuanceV4>()),
            Some(LifecycleKind::Enable)
        );
        assert_eq!(
            lifecycle_kind_for_type_id(TypeId::of::<CancelKagemushaRecursiveReleaseV4>()),
            Some(LifecycleKind::Cancel)
        );
        assert_eq!(
            lifecycle_kind_for_type_id(TypeId::of::<DeactivateKagemushaRecursiveIssuanceV4>()),
            Some(LifecycleKind::Deactivate)
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
        let signatures = MultisigSignatures::from_signers(
            &payload,
            [signers[0].private_key(), signers[1].private_key()],
        )
        .expect("detached signatures")
        .signatures;
        let mut drifted = payload;
        drifted.creation_time_ms = drifted.creation_time_ms.saturating_add(1);
        assert!(assemble_transaction(drifted, signatures).is_err());
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
}
