//! CLI helper that regenerates Norito fixtures and manifests for tests and docs.
use std::{
    collections::BTreeMap,
    fs,
    num::{NonZeroU32, NonZeroU64},
    path::{Path, PathBuf},
    str::FromStr,
    time::Duration,
};

use anyhow::{Context, Result, bail};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use clap::{ArgAction, Parser};
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, PublicKey, Signature, SignatureOf};
use iroha_data_model::name::Name;
use iroha_data_model::parameter::Parameter;
use iroha_data_model::{
    account::{Account, AccountId, address},
    asset::{AssetDefinition, AssetDefinitionId, AssetId},
    confidential::ConfidentialStatus,
    domain::{Domain, DomainId},
    events::pipeline::{
        BlockEventFilter, BlockStatus, MergeLedgerEventFilter, PipelineEventFilterBox,
        TransactionEventFilter, TransactionStatus, WitnessEventFilter,
    },
    events::time::{ExecutionTime, Schedule as TimeSchedule, TimeEventFilter},
    executor::Executor,
    ipfs::IpfsPath,
    isi::{
        Burn, ExecuteTrigger, Grant, Instruction, InstructionBox, Mint, Register,
        RemoveAssetKeyValue, RemoveKeyValue, Revoke, SetAssetKeyValue, SetKeyValue, SetParameter,
        Transfer, Unregister, frame_instruction_payload,
        Upgrade,
        register::RegisterPeerWithPop,
        repo::{RepoIsi, ReverseRepoIsi},
        settlement::{
            DvpIsi, PvpIsi, SettlementAtomicity, SettlementExecutionOrder, SettlementId,
            SettlementLeg, SettlementPlan,
        },
        verifying_keys,
    },
    metadata::Metadata,
    nft::{Nft, NftId},
    peer::PeerId,
    permission::Permission,
    prelude::*,
    proof::{VerifyingKeyBox, VerifyingKeyId, VerifyingKeyRecord},
    repo::{RepoAgreementId, RepoCashLeg, RepoCollateralLeg, RepoGovernance},
    role::Role,
    role::RoleId,
    transaction::signed::{TransactionPayload, TransactionSignature},
    transaction::{Executable, IvmBytecode, SignedTransaction, TransactionBuilder},
    trigger::{
        Trigger, TriggerId,
        action::{Action as TriggerAction, Repeats},
    },
    zk::BackendTag,
};

#[cfg(feature = "kaigi-fixtures")]
use iroha_data_model::{
    isi::kaigi::{
        CreateKaigi as KaigiCreate, EndKaigi as KaigiEnd, JoinKaigi as KaigiJoin,
        LeaveKaigi as KaigiLeave, RecordKaigiUsage as KaigiRecordUsage,
        RegisterKaigiRelay as KaigiRegisterRelay, SetKaigiRelayManifest as KaigiSetManifest,
    },
    kaigi::{
        KaigiId, KaigiParticipantCommitment, KaigiParticipantNullifier, KaigiPrivacyMode,
        KaigiRelayHop, KaigiRelayManifest, KaigiRelayRegistration, NewKaigi,
    },
};
use iroha_primitives::{json::Json, numeric::Numeric};
use iroha_schema::Ident;
use norito::codec::{Decode, Encode};
use norito::json::{self, Map, Number, Value};
use sha2::{Digest, Sha256};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

const DEFAULT_FIXTURES_PATH: &str =
    "java/iroha_android/src/test/resources/transaction_payloads.json";
const DEFAULT_OUT_DIR: &str = "java/iroha_android/src/test/resources";
const DEFAULT_MANIFEST_NAME: &str = "transaction_fixtures.manifest.json";
const DEFAULT_CHAIN_DISCRIMINANT: u16 = 42;
const PAYLOAD_SCHEMA_NAME: &str = "iroha.android.transaction.Payload.v1";
const SIGNED_SCHEMA_NAME: &str = "iroha.transaction.SignedTransaction.v1";
const SIGNING_SEED_HEX: &str = "616e64726f69642d666978747572652d7369676e696e672d6b65792d30313032";

#[derive(Parser)]
struct Args {
    #[arg(long, default_value = DEFAULT_FIXTURES_PATH)]
    fixtures: PathBuf,
    #[arg(long, default_value = DEFAULT_OUT_DIR)]
    out_dir: PathBuf,
    #[arg(long, default_value = DEFAULT_MANIFEST_NAME)]
    manifest: PathBuf,
    #[arg(long, default_value_t = true, action = ArgAction::Set)]
    check_encoded: bool,
    #[arg(long, action = ArgAction::SetTrue)]
    write_fixtures: bool,
    #[arg(long, default_value_t = DEFAULT_CHAIN_DISCRIMINANT)]
    chain_discriminant: u16,
}

fn main() -> Result<()> {
    let args = Args::parse();
    run(args)
}

fn run(args: Args) -> Result<()> {
    address::set_chain_discriminant(args.chain_discriminant);
    let fixtures_text = fs::read_to_string(&args.fixtures)
        .with_context(|| format!("failed to read {}", args.fixtures.display()))?;
    let fixtures_value: Value = json::from_str(&fixtures_text).context("invalid fixtures JSON")?;
    let raw_fixtures = parse_fixtures(&fixtures_value)?;
    let keypair = signing_keypair()?;
    let (_, pk_bytes) = keypair.public_key().to_bytes();
    let public_key_hex = hex::encode(pk_bytes);

    let check_hints = !args.write_fixtures;
    let mut fixtures = Vec::with_capacity(raw_fixtures.len());
    for raw in &raw_fixtures {
        fixtures.push(
            raw.clone()
                .into_fixture(&keypair, args.check_encoded, check_hints)?,
        );
    }

    fs::create_dir_all(&args.out_dir)
        .with_context(|| format!("failed to create {}", args.out_dir.display()))?;
    for fixture in &fixtures {
        let norito_path = args.out_dir.join(format!("{}.norito", fixture.name));
        fs::write(&norito_path, &fixture.encoded)
            .with_context(|| format!("failed to write {}", norito_path.display()))?;
    }

    let manifest_path = if args.manifest.is_absolute() {
        args.manifest.clone()
    } else {
        args.out_dir.join(&args.manifest)
    };
    let manifest_value = build_manifest(&fixtures, &public_key_hex);
    let manifest_json = json::to_json_pretty(&manifest_value).expect("manifest serialization");
    fs::write(&manifest_path, format!("{manifest_json}\n"))
        .with_context(|| format!("failed to write {}", manifest_path.display()))?;

    if args.write_fixtures {
        write_fixtures_json(&args.fixtures, &raw_fixtures, &fixtures)?;
    }

    Ok(())
}

fn signing_keypair() -> Result<KeyPair> {
    let seed = hex::decode(SIGNING_SEED_HEX).context("invalid signing seed hex")?;
    Ok(KeyPair::from_seed(seed, Algorithm::Ed25519))
}

#[derive(Clone)]
struct RawFixture {
    name: String,
    payload: Option<RawPayload>,
    payload_json: Option<Value>,
    payload_base64: Option<String>,
    signed_base64: Option<String>,
    chain_hint: Option<String>,
    authority_hint: Option<String>,
    creation_time_ms_hint: Option<u64>,
    ttl_ms_hint: Option<u64>,
    nonce_hint: Option<u32>,
    payload_hash_hint: Option<String>,
    signed_hash_hint: Option<String>,
    encoded: Option<String>,
}

#[derive(Clone)]
struct RawPayload {
    chain: String,
    authority: String,
    creation_time_ms: u64,
    executable: RawExecutable,
    ttl_ms: Option<u64>,
    nonce: Option<u32>,
    metadata: Vec<(Name, Json)>,
}

#[derive(Clone)]
enum RawExecutable {
    Ivm(Vec<u8>),
    Instructions(Vec<RawInstruction>),
}

#[derive(Clone)]
struct RawInstruction {
    kind: String,
    arguments: BTreeMap<String, String>,
}

struct Fixture {
    name: String,
    encoded: Vec<u8>,
    signed_bytes: Vec<u8>,
    summary: PayloadSummary,
}

struct PayloadSummary {
    chain: String,
    authority: String,
    creation_time_ms: u64,
    ttl_ms: Option<u64>,
    nonce: Option<u32>,
    payload_base64: String,
    signed_base64: String,
    payload_hash_hex: String,
    signed_hash_hex: String,
}

struct WireInstructionPayload {
    wire_name: String,
    payload_base64: String,
}

struct SignedEnvelopeFields {
    payload_field: Vec<u8>,
    attachments_field: Vec<u8>,
    multisig_field: Vec<u8>,
}

impl RawFixture {
    fn into_fixture(
        self,
        keypair: &KeyPair,
        check_encoded: bool,
        check_hints: bool,
    ) -> Result<Fixture> {
        let authority_source = self.authority_hint.as_deref().or_else(|| {
            self.payload
                .as_ref()
                .map(|payload| payload.authority.as_str())
        });
        let _chain_guard = authority_source
            .and_then(authority_chain_discriminant)
            .map(address::ChainDiscriminantGuard::enter);
        if let Some(payload) = self.payload {
            if let Some(chain_hint) = &self.chain_hint {
                if chain_hint != &payload.chain {
                    bail!(
                        "fixture '{}' chain mismatch: expected {}, got {}",
                        self.name,
                        chain_hint,
                        payload.chain
                    );
                }
            }
            if let Some(authority_hint) = &self.authority_hint {
                let expected = normalize_authority_hint(authority_hint);
                let actual = normalize_authority_hint(&payload.authority);
                if expected != actual {
                    bail!(
                        "fixture '{}' authority mismatch: expected {}, got {}",
                        self.name,
                        authority_hint,
                        payload.authority
                    );
                }
            }
            if let Some(creation_hint) = self.creation_time_ms_hint {
                if creation_hint != payload.creation_time_ms {
                    bail!(
                        "fixture '{}' creation_time_ms mismatch: expected {}, got {}",
                        self.name,
                        creation_hint,
                        payload.creation_time_ms
                    );
                }
            }
            if let Some(ttl_hint) = self.ttl_ms_hint {
                if Some(ttl_hint) != payload.ttl_ms {
                    bail!(
                        "fixture '{}' time_to_live_ms mismatch: expected {}, got {:?}",
                        self.name,
                        ttl_hint,
                        payload.ttl_ms
                    );
                }
            }
            if let Some(nonce_hint) = self.nonce_hint {
                if Some(nonce_hint) != payload.nonce {
                    bail!(
                        "fixture '{}' nonce mismatch: expected {}, got {:?}",
                        self.name,
                        nonce_hint,
                        payload.nonce
                    );
                }
            }
            let builder = payload.to_builder()?;
            let signed = builder.sign(keypair.private_key());
            let payload_value = signed.payload().clone();
            let encoded = payload_value.encode();
            let payload_base64 = BASE64.encode(&encoded);
            if check_encoded {
                if let Some(expected) = &self.encoded {
                    if expected != &payload_base64 {
                        bail!(
                            "encoded payload mismatch for '{}': expected {}, got {}",
                            self.name,
                            expected,
                            payload_base64
                        );
                    }
                }
            }
            let signed_bytes = signed.encode();
            let signed_base64 = BASE64.encode(&signed_bytes);
            let payload_hash_hex = format!("{}", HashOf::<TransactionPayload>::new(&payload_value));
            let signed_hash_hex = format!("{}", HashOf::<SignedTransaction>::new(&signed));

            let summary = PayloadSummary {
                chain: payload.chain,
                authority: payload.authority,
                creation_time_ms: payload.creation_time_ms,
                ttl_ms: payload.ttl_ms,
                nonce: payload.nonce,
                payload_base64,
                signed_base64,
                payload_hash_hex,
                signed_hash_hex,
            };

            return Ok(Fixture {
                name: self.name,
                encoded,
                signed_bytes,
                summary,
            });
        }

        let payload_base64_input = self.payload_base64.as_deref().ok_or_else(|| {
            anyhow::anyhow!("fixture '{}' missing payload and payload_base64", self.name)
        })?;
        let encoded_input = BASE64
            .decode(payload_base64_input.as_bytes())
            .context("failed to decode payload_base64")?;
        if check_encoded {
            if let Some(expected) = &self.encoded {
                if expected != payload_base64_input {
                    bail!(
                        "encoded payload mismatch for '{}': expected {}, got {}",
                        self.name,
                        expected,
                        payload_base64_input
                    );
                }
            }
        }
        let signed_bytes_input = self
            .signed_base64
            .as_ref()
            .map(|b64| BASE64.decode(b64.as_bytes()))
            .transpose()
            .context("failed to decode signed_base64")?;

        let mut cursor = encoded_input.as_slice();
        let payload = match TransactionPayload::decode(&mut cursor) {
            Ok(payload) => payload,
            Err(err) => {
                if check_hints {
                    eprintln!(
                        "fixture '{}' payload decode failed; using opaque hints: {err}",
                        self.name
                    );
                }
                return self.opaque_fixture_from_hints(
                    keypair,
                    payload_base64_input,
                    encoded_input,
                    signed_bytes_input,
                    check_hints,
                );
            }
        };
        if !cursor.is_empty() {
            if check_hints {
                eprintln!(
                    "fixture '{}' payload has trailing bytes; using opaque hints",
                    self.name
                );
            }
            return self.opaque_fixture_from_hints(
                keypair,
                payload_base64_input,
                encoded_input,
                signed_bytes_input,
                check_hints,
            );
        }
        let chain = payload.chain.to_string();
        let authority = payload.authority.to_string();
        let creation_time_ms = payload.creation_time_ms;
        let ttl_ms = payload.time_to_live_ms.map(NonZeroU64::get);
        let nonce = payload.nonce.map(NonZeroU32::get);

        if let Some(chain_hint) = &self.chain_hint {
            if chain_hint != &chain {
                bail!(
                    "fixture '{}' chain mismatch: expected {}, got {}",
                    self.name,
                    chain_hint,
                    chain
                );
            }
        }
        if let Some(authority_hint) = &self.authority_hint {
            let expected = normalize_authority_hint(authority_hint);
            let actual = normalize_authority_hint(&authority);
            if expected != actual {
                bail!(
                    "fixture '{}' authority mismatch: expected {}, got {}",
                    self.name,
                    authority_hint,
                    authority
                );
            }
        }
        if let Some(creation_hint) = self.creation_time_ms_hint {
            if creation_hint != creation_time_ms {
                bail!(
                    "fixture '{}' creation_time_ms mismatch: expected {}, got {}",
                    self.name,
                    creation_hint,
                    creation_time_ms
                );
            }
        }
        if let Some(ttl_hint) = self.ttl_ms_hint {
            if Some(ttl_hint) != ttl_ms {
                bail!(
                    "fixture '{}' time_to_live_ms mismatch: expected {}, got {:?}",
                    self.name,
                    ttl_hint,
                    ttl_ms
                );
            }
        }
        if let Some(nonce_hint) = self.nonce_hint {
            if Some(nonce_hint) != nonce {
                bail!(
                    "fixture '{}' nonce mismatch: expected {}, got {:?}",
                    self.name,
                    nonce_hint,
                    nonce
                );
            }
        }

        let payload_hash_hex = format!("{}", HashOf::<TransactionPayload>::new(&payload));
        if let Some(hash_hint) = &self.payload_hash_hint {
            if hash_hint != &payload_hash_hex {
                if check_hints {
                    bail!(
                        "fixture '{}' payload_hash mismatch: expected {}, got {}",
                        self.name,
                        hash_hint,
                        payload_hash_hex
                    );
                } else {
                    eprintln!(
                        "fixture '{}' payload_hash updated: {} -> {}",
                        self.name, hash_hint, payload_hash_hex
                    );
                }
            }
        }

        let signed_tx = if let Some(bytes) = &signed_bytes_input {
            let mut cursor = bytes.as_slice();
            match SignedTransaction::decode(&mut cursor) {
                Ok(signed) => {
                    if !cursor.is_empty() {
                        if check_hints {
                            eprintln!(
                                "fixture '{}' signed payload has trailing bytes; ignoring attachments",
                                self.name
                            );
                        }
                        None
                    } else {
                        Some(signed)
                    }
                }
                Err(err) => {
                    if check_hints {
                        eprintln!(
                            "fixture '{}' signed payload decode failed; ignoring attachments: {err}",
                            self.name
                        );
                    }
                    None
                }
            }
        } else if self.signed_hash_hint.is_none() {
            bail!(
                "fixture '{}' missing signed_base64 and signed hash hints for pre-encoded payload",
                self.name
            );
        } else {
            None
        };

        let mut builder = TransactionBuilder::new(payload.chain.clone(), payload.authority.clone())
            .with_executable(payload.instructions.clone())
            .with_metadata(payload.metadata.clone());
        builder.set_creation_time(Duration::from_millis(payload.creation_time_ms));
        if let Some(ttl) = payload.time_to_live_ms {
            builder.set_ttl(Duration::from_millis(ttl.get()));
        }
        if let Some(nonce) = payload.nonce {
            builder.set_nonce(nonce);
        }
        if let Some(signed) = signed_tx.as_ref() {
            if let Some(attachments) = signed.attachments() {
                builder = builder.with_attachments(attachments.clone());
            }
            if let Some(multisig_signatures) = signed.multisig_signatures() {
                builder = builder.with_multisig_signatures(multisig_signatures.clone());
            }
        }

        let signed = builder.sign(keypair.private_key());
        let payload_value = signed.payload().clone();
        let encoded = payload_value.encode();
        let payload_base64 = BASE64.encode(&encoded);
        if check_encoded && payload_base64_input != payload_base64 {
            bail!(
                "encoded payload mismatch for '{}': expected {}, got {}",
                self.name,
                payload_base64_input,
                payload_base64
            );
        }
        let signed_bytes = signed.encode();
        let signed_base64 = BASE64.encode(&signed_bytes);
        let payload_hash_hex = format!("{}", HashOf::<TransactionPayload>::new(&payload_value));
        let signed_hash_hex = format!("{}", HashOf::<SignedTransaction>::new(&signed));
        if let Some(hash_hint) = &self.signed_hash_hint {
            if hash_hint != &signed_hash_hex {
                if check_hints {
                    bail!(
                        "fixture '{}' signed_hash mismatch: expected {}, got {}",
                        self.name,
                        hash_hint,
                        signed_hash_hex
                    );
                } else {
                    eprintln!(
                        "fixture '{}' signed_hash updated: {} -> {}",
                        self.name, hash_hint, signed_hash_hex
                    );
                }
            }
        }

        let summary = PayloadSummary {
            chain: payload_value.chain.to_string(),
            authority: payload_value.authority.to_string(),
            creation_time_ms: payload_value.creation_time_ms,
            ttl_ms: payload_value.time_to_live_ms.map(NonZeroU64::get),
            nonce: payload_value.nonce.map(NonZeroU32::get),
            payload_base64,
            signed_base64,
            payload_hash_hex,
            signed_hash_hex,
        };

        Ok(Fixture {
            name: self.name,
            encoded,
            signed_bytes,
            summary,
        })
    }

    fn opaque_fixture_from_hints(
        &self,
        keypair: &KeyPair,
        payload_base64_input: &str,
        encoded_input: Vec<u8>,
        signed_bytes_input: Option<Vec<u8>>,
        check_hints: bool,
    ) -> Result<Fixture> {
        let chain = self.chain_hint.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "fixture '{}' missing chain hint for opaque payload",
                self.name
            )
        })?;
        let authority = self.authority_hint.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "fixture '{}' missing authority hint for opaque payload",
                self.name
            )
        })?;
        let creation_time_ms = self.creation_time_ms_hint.ok_or_else(|| {
            anyhow::anyhow!(
                "fixture '{}' missing creation_time_ms hint for opaque payload",
                self.name
            )
        })?;
        let signed_bytes_input = signed_bytes_input.ok_or_else(|| {
            anyhow::anyhow!(
                "fixture '{}' missing signed_base64 for opaque payload",
                self.name
            )
        })?;
        let mut attachments_field = None;
        let mut multisig_field = None;
        match decode_signed_envelope_fields(&signed_bytes_input) {
            Ok(fields) => {
                if fields.payload_field != encoded_input {
                    if check_hints {
                        bail!(
                            "fixture '{}' signed payload mismatch vs payload_base64",
                            self.name
                        );
                    } else {
                        eprintln!(
                            "fixture '{}' signed payload updated to match payload_base64",
                            self.name
                        );
                    }
                }
                attachments_field = Some(fields.attachments_field);
                multisig_field = Some(fields.multisig_field);
            }
            Err(err) => {
                if check_hints {
                    bail!(
                        "fixture '{}' signed payload parse failed for opaque payload: {err}",
                        self.name
                    );
                } else {
                    eprintln!(
                        "fixture '{}' signed payload parse failed; dropping attachments: {err}",
                        self.name
                    );
                }
            }
        }

        let payload_hash_hex = format!("{}", Hash::new(&encoded_input));
        if let Some(hash_hint) = &self.payload_hash_hint {
            if hash_hint != &payload_hash_hex {
                if check_hints {
                    bail!(
                        "fixture '{}' payload_hash mismatch: expected {}, got {}",
                        self.name,
                        hash_hint,
                        payload_hash_hex
                    );
                } else {
                    eprintln!(
                        "fixture '{}' payload_hash updated: {} -> {}",
                        self.name, hash_hint, payload_hash_hex
                    );
                }
            }
        }

        let payload_hash = Hash::new(&encoded_input);
        let signature = Signature::new(keypair.private_key(), payload_hash.as_ref());
        let signature_bytes = signature.payload().to_vec();
        let signed_bytes = encode_signed_envelope(
            &signature_bytes,
            &encoded_input,
            attachments_field.as_deref(),
            multisig_field.as_deref(),
        );
        let signed_hash_hex = format!("{}", Hash::new(&signed_bytes));
        if let Some(hash_hint) = &self.signed_hash_hint {
            if hash_hint != &signed_hash_hex {
                if check_hints {
                    bail!(
                        "fixture '{}' signed_hash mismatch: expected {}, got {}",
                        self.name,
                        hash_hint,
                        signed_hash_hex
                    );
                } else {
                    eprintln!(
                        "fixture '{}' signed_hash updated: {} -> {}",
                        self.name, hash_hint, signed_hash_hex
                    );
                }
            }
        }

        let signed_base64 = BASE64.encode(&signed_bytes);
        let summary = PayloadSummary {
            chain: chain.clone(),
            authority: authority.clone(),
            creation_time_ms,
            ttl_ms: self.ttl_ms_hint,
            nonce: self.nonce_hint,
            payload_base64: payload_base64_input.to_string(),
            signed_base64,
            payload_hash_hex,
            signed_hash_hex,
        };

        Ok(Fixture {
            name: self.name.clone(),
            encoded: encoded_input,
            signed_bytes,
            summary,
        })
    }
}

fn decode_signed_envelope_fields(bytes: &[u8]) -> Result<SignedEnvelopeFields> {
    let _guard = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let mut cursor = 0usize;
    let _signature_field = read_len_prefixed_field(bytes, &mut cursor, "signature")?;
    let payload_field = read_len_prefixed_field(bytes, &mut cursor, "payload")?;
    let attachments_field = read_len_prefixed_field(bytes, &mut cursor, "attachments")?;
    let multisig_field = read_len_prefixed_field(bytes, &mut cursor, "multisig_signatures")?;
    if cursor != bytes.len() {
        bail!("signed transaction payload has trailing bytes");
    }
    Ok(SignedEnvelopeFields {
        payload_field,
        attachments_field,
        multisig_field,
    })
}

fn read_len_prefixed_field(bytes: &[u8], cursor: &mut usize, field: &str) -> Result<Vec<u8>> {
    if *cursor >= bytes.len() {
        bail!("signed transaction missing {field} field");
    }
    let (len, used) = norito::core::read_len_from_slice(&bytes[*cursor..])?;
    *cursor = cursor
        .checked_add(used)
        .ok_or_else(|| anyhow::anyhow!("signed transaction {field} length overflow"))?;
    let end = cursor
        .checked_add(len)
        .ok_or_else(|| anyhow::anyhow!("signed transaction {field} length overflow"))?;
    if end > bytes.len() {
        bail!("signed transaction {field} length exceeds payload");
    }
    let out = bytes[*cursor..end].to_vec();
    *cursor = end;
    Ok(out)
}

fn encode_signed_envelope(
    signature_bytes: &[u8],
    payload_bytes: &[u8],
    attachments_field: Option<&[u8]>,
    multisig_field: Option<&[u8]>,
) -> Vec<u8> {
    let _guard = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let signature = Signature::from_bytes(signature_bytes);
    let signature_of = SignatureOf::<TransactionPayload>::from_signature(signature);
    let signature_field = TransactionSignature(signature_of).encode();
    let attachments_field = attachments_field.map(Vec::from).unwrap_or_else(|| vec![0]);
    let multisig_field = multisig_field.map(Vec::from).unwrap_or_else(|| vec![0]);
    let mut out = Vec::new();
    norito::core::write_len_to_vec(&mut out, signature_field.len() as u64);
    out.extend_from_slice(&signature_field);
    norito::core::write_len_to_vec(&mut out, payload_bytes.len() as u64);
    out.extend_from_slice(payload_bytes);
    norito::core::write_len_to_vec(&mut out, attachments_field.len() as u64);
    out.extend_from_slice(&attachments_field);
    norito::core::write_len_to_vec(&mut out, multisig_field.len() as u64);
    out.extend_from_slice(&multisig_field);
    out
}

impl RawPayload {
    fn to_builder(&self) -> Result<TransactionBuilder> {
        let chain_id = ChainId::from_str(&self.chain).expect("ChainId infallible");
        let authority = parse_account_id(&self.authority)
            .with_context(|| format!("invalid authority id '{}'", self.authority))?;
        let mut builder = TransactionBuilder::new(chain_id.clone(), authority.clone());
        builder.set_creation_time(Duration::from_millis(self.creation_time_ms));
        if let Some(ttl) = self.ttl_ms {
            builder.set_ttl(Duration::from_millis(ttl));
        }
        if let Some(nonce) = self.nonce {
            let nz = NonZeroU32::new(nonce).ok_or_else(|| anyhow::anyhow!("nonce must be > 0"))?;
            builder.set_nonce(nz);
        }

        let mut metadata = Metadata::default();
        for (key, value) in &self.metadata {
            metadata.insert(key.clone(), value.clone());
        }
        builder = builder.with_metadata(metadata);

        builder = match &self.executable {
            RawExecutable::Ivm(bytes) => {
                builder.with_executable(Executable::Ivm(IvmBytecode::from_compiled(bytes.clone())))
            }
            RawExecutable::Instructions(raws) => {
                let instructions = raws
                    .iter()
                    .map(build_instruction)
                    .collect::<Result<Vec<_>>>()?;
                builder.with_instructions(instructions)
            }
        };

        Ok(builder)
    }
}

fn parse_fixtures(value: &Value) -> Result<Vec<RawFixture>> {
    let arr = value
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("fixture root must be an array"))?;
    arr.iter().map(parse_fixture).collect()
}

fn parse_fixture(value: &Value) -> Result<RawFixture> {
    let obj = value
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("fixture entries must be objects"))?;
    let name = expect_string(obj, "name")?.to_owned();
    let payload_value = obj.get("payload");
    let payload_json = payload_value.cloned();
    let payload = match payload_value {
        Some(value) => Some(
            parse_payload(value)
                .with_context(|| format!("invalid payload for fixture '{name}'"))?,
        ),
        None => None,
    };
    let payload_base64 = obj
        .get("payload_base64")
        .or_else(|| obj.get("encoded"))
        .and_then(|v| v.as_str())
        .map(str::to_owned);
    let signed_base64 = obj
        .get("signed_base64")
        .and_then(|v| v.as_str())
        .map(str::to_owned);
    let chain_hint = obj.get("chain").and_then(|v| v.as_str()).map(str::to_owned);
    let authority_hint = obj
        .get("authority")
        .and_then(|v| v.as_str())
        .map(str::to_owned);
    let creation_time_ms_hint = obj.get("creation_time_ms").and_then(|v| v.as_u64());
    let ttl_ms_hint = obj.get("time_to_live_ms").and_then(|v| v.as_u64());
    let nonce_hint = obj.get("nonce").and_then(|v| v.as_u64()).map(|n| n as u32);
    let payload_hash_hint = obj
        .get("payload_hash")
        .and_then(|v| v.as_str())
        .map(str::to_owned);
    let signed_hash_hint = obj
        .get("signed_hash")
        .and_then(|v| v.as_str())
        .map(str::to_owned);
    if payload.is_none() && payload_base64.is_none() {
        bail!("missing payload for '{name}'");
    }
    let encoded = obj
        .get("encoded")
        .and_then(|v| v.as_str())
        .map(|s| s.to_owned());
    Ok(RawFixture {
        name,
        payload,
        payload_json,
        payload_base64,
        signed_base64,
        chain_hint,
        authority_hint,
        creation_time_ms_hint,
        ttl_ms_hint,
        nonce_hint,
        payload_hash_hint,
        signed_hash_hint,
        encoded,
    })
}

fn parse_payload(value: &Value) -> Result<RawPayload> {
    let obj = value
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("payload entries must be objects"))?;
    let chain = expect_string(obj, "chain")?.to_owned();
    let authority = expect_string(obj, "authority")?.to_owned();
    let creation_time_ms = expect_u64(obj, "creation_time_ms")?;
    let executable_value = obj
        .get("executable")
        .ok_or_else(|| anyhow::anyhow!("missing executable"))?;
    let executable = parse_executable(executable_value)?;
    let ttl_ms = parse_optional_u64(obj, "time_to_live_ms")?;
    let nonce = parse_optional_u32(obj, "nonce")?;
    let metadata = match obj.get("metadata") {
        Some(value) => parse_metadata_object(value)?,
        None => Vec::new(),
    };

    Ok(RawPayload {
        chain,
        authority,
        creation_time_ms,
        executable,
        ttl_ms,
        nonce,
        metadata,
    })
}

fn parse_executable(value: &Value) -> Result<RawExecutable> {
    let obj = value
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("executable must be an object"))?;
    if let Some(ivm) = obj.get("Ivm") {
        let bytes = ivm
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Ivm value must be base64 string"))?;
        let decoded = BASE64
            .decode(bytes)
            .context("failed to decode Ivm base64")?;
        return Ok(RawExecutable::Ivm(decoded));
    }
    if let Some(instr) = obj.get("Instructions") {
        let arr = instr
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("Instructions must be an array"))?;
        let mut entries = Vec::with_capacity(arr.len());
        for entry in arr {
            entries.push(parse_instruction(entry)?);
        }
        return Ok(RawExecutable::Instructions(entries));
    }
    bail!("unknown executable variant")
}

fn parse_instruction(value: &Value) -> Result<RawInstruction> {
    let obj = value
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("instruction entries must be objects"))?;
    let kind = expect_string(obj, "kind")?.to_owned();
    let args_value = obj
        .get("arguments")
        .ok_or_else(|| anyhow::anyhow!("instruction arguments missing"))?;
    let args_obj = args_value
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("instruction arguments must be objects"))?;
    let mut arguments = BTreeMap::new();
    for (key, value) in args_obj {
        arguments.insert(key.clone(), value_to_string(value));
    }
    Ok(RawInstruction { kind, arguments })
}

fn parse_metadata_object(value: &Value) -> Result<Vec<(Name, Json)>> {
    let obj = value
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("metadata must be an object"))?;
    let mut entries = Vec::with_capacity(obj.len());
    for (key, value) in obj {
        let name: Name = key.parse().context("invalid metadata key")?;
        let json =
            Json::from_norito_value_ref(value).map_err(|err| anyhow::anyhow!(err.to_string()))?;
        entries.push((name, json));
    }
    Ok(entries)
}

fn build_instruction(raw: &RawInstruction) -> Result<InstructionBox> {
    let action = raw
        .arguments
        .get("action")
        .ok_or_else(|| anyhow::anyhow!("instruction action missing"))?;
    match action.as_str() {
        "RegisterDomain" => {
            ensure_kind(raw, "Register")?;
            let domain_id: DomainId = get_arg(raw, "domain")?
                .parse()
                .context("invalid domain id")?;
            let mut domain = Domain::new(domain_id);
            if let Some(logo) = raw.arguments.get("logo") {
                domain = domain.with_logo(IpfsPath::from_str(logo).context("invalid domain logo")?);
            }
            let metadata =
                metadata_from_arguments(&raw.arguments, &["display_name", "description"])?;
            domain = domain.with_metadata(metadata);
            Ok(Register::domain(domain).into())
        }
        "RegisterAccount" => {
            ensure_kind(raw, "Register")?;
            let account_id = parse_account_id(get_arg(raw, "account")?)?;
            let metadata = metadata_from_arguments(&raw.arguments, &[])?;
            Ok(Register::account(Account::new(account_id).with_metadata(metadata)).into())
        }
        "RegisterAssetDefinition" => {
            ensure_kind(raw, "Register")?;
            let definition: AssetDefinitionId = get_arg(raw, "definition")?
                .parse()
                .context("invalid asset definition id")?;
            let mut asset = AssetDefinition::numeric(definition);
            if let Some(logo) = raw.arguments.get("logo") {
                asset = asset.with_logo(Some(
                    IpfsPath::from_str(logo).context("invalid asset logo")?,
                ));
            }
            let metadata =
                metadata_from_arguments(&raw.arguments, &["display_name", "description"])?;
            asset = asset.with_metadata(metadata);
            Ok(Register::asset_definition(asset).into())
        }
        "RegisterPeerWithPop" => {
            ensure_kind(raw, "Register")?;
            let peer: PeerId = get_arg(raw, "peer")?.parse().context("invalid peer id")?;
            let pop = BASE64
                .decode(get_arg(raw, "pop")?)
                .context("invalid peer proof-of-possession base64")?;
            Ok(RegisterPeerWithPop::new(peer, pop).into())
        }
        "RegisterRole" => {
            ensure_kind(raw, "Register")?;
            let role_id: RoleId = get_arg(raw, "role")?.parse().context("invalid role id")?;
            let owner = parse_account_id(get_arg(raw, "owner")?)?;
            let permissions_value = raw
                .arguments
                .get("permissions")
                .map(String::as_str)
                .unwrap_or("");
            let mut role = Role::new(role_id, owner);
            if !permissions_value.is_empty() {
                for permission in permissions_value.split(',') {
                    let trimmed = permission.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    role = role.add_permission(build_permission(trimmed)?);
                }
            }
            Ok(Register::role(role).into())
        }
        "RegisterNft" => {
            ensure_kind(raw, "Register")?;
            let nft_id: NftId = get_arg(raw, "nft")?.parse().context("invalid nft id")?;
            let metadata = metadata_from_arguments(&raw.arguments, &[])?;
            Ok(Register::nft(Nft::new(nft_id, metadata)).into())
        }
        "ExecuteTrigger" => {
            ensure_kind(raw, "ExecuteTrigger")?;
            let trigger_id: TriggerId = get_arg(raw, "trigger")?
                .parse()
                .context("invalid trigger id")?;
            Ok(ExecuteTrigger::new(trigger_id).into())
        }
        "RegisterTimeTrigger" => {
            ensure_kind(raw, "Register")?;
            let trigger_id: TriggerId = get_arg(raw, "trigger")?
                .parse()
                .context("invalid trigger id")?;
            let authority = parse_account_id(get_arg(raw, "authority")?)?;
            let start_ms: u64 = get_arg(raw, "start_ms")?
                .parse()
                .context("invalid start_ms value for time trigger")?;
            if start_ms == 0 {
                bail!("start_ms must be greater than zero");
            }
            let mut schedule = TimeSchedule::starting_at(Duration::from_millis(start_ms));
            if let Some(period) = raw.arguments.get("period_ms") {
                let period_ms: u64 = period
                    .parse()
                    .context("invalid period_ms value for time trigger")?;
                if period_ms == 0 {
                    bail!("period_ms must be greater than zero when provided");
                }
                schedule = schedule.with_period(Duration::from_millis(period_ms));
            }
            let repeats = parse_trigger_repeats(raw)?;
            let metadata = metadata_from_arguments(&raw.arguments, &[])?;
            let executable = Executable::from(parse_trigger_instructions(raw)?);
            let filter = TimeEventFilter::new(ExecutionTime::Schedule(schedule));
            let action = TriggerAction::new(executable, repeats, authority.clone(), filter)
                .with_metadata(metadata);
            let trigger = Trigger::new(trigger_id, action);
            Ok(Register::trigger(trigger).into())
        }
        "RegisterPrecommitTrigger" => {
            ensure_kind(raw, "Register")?;
            let trigger_id: TriggerId = get_arg(raw, "trigger")?
                .parse()
                .context("invalid trigger id")?;
            let authority = parse_account_id(get_arg(raw, "authority")?)?;
            let repeats = parse_trigger_repeats(raw)?;
            let metadata = metadata_from_arguments(&raw.arguments, &[])?;
            let executable = Executable::from(parse_trigger_instructions(raw)?);
            let filter = TimeEventFilter::new(ExecutionTime::PreCommit);
            let action = TriggerAction::new(executable, repeats, authority.clone(), filter)
                .with_metadata(metadata);
            let trigger = Trigger::new(trigger_id, action);
            Ok(Register::trigger(trigger).into())
        }
        "RegisterPipelineTrigger" => {
            ensure_kind(raw, "Register")?;
            let trigger_id: TriggerId = get_arg(raw, "trigger")?
                .parse()
                .context("invalid trigger id")?;
            let authority = parse_account_id(get_arg(raw, "authority")?)?;
            let repeats = parse_trigger_repeats(raw)?;
            let metadata = metadata_from_arguments(&raw.arguments, &[])?;
            let executable = Executable::from(parse_trigger_instructions(raw)?);
            let filter = parse_pipeline_filter(raw)?;
            let action = TriggerAction::new(executable, repeats, authority.clone(), filter)
                .with_metadata(metadata);
            let trigger = Trigger::new(trigger_id, action);
            Ok(Register::trigger(trigger).into())
        }
        #[cfg(feature = "kaigi-fixtures")]
        "CreateKaigi" => {
            ensure_kind(raw, "Custom")?;
            let call_id = parse_kaigi_id(raw, "call")?;
            let host = parse_account_id(get_arg(raw, "host")?)?;
            let mut call = NewKaigi::with_defaults(call_id, host);
            if let Some(title) = raw.arguments.get("title") {
                if !title.is_empty() {
                    call.title = Some(title.clone());
                }
            }
            if let Some(description) = raw.arguments.get("description") {
                if !description.is_empty() {
                    call.description = Some(description.clone());
                }
            }
            if let Some(max_participants) = raw.arguments.get("max_participants") {
                let parsed: u32 = max_participants
                    .parse()
                    .context("invalid max_participants for Kaigi")?;
                if parsed == 0 {
                    bail!("max_participants must be greater than zero");
                }
                call.max_participants = Some(parsed);
            }
            if let Some(rate) = raw.arguments.get("gas_rate_per_minute") {
                call.gas_rate_per_minute = rate
                    .parse()
                    .context("invalid gas_rate_per_minute for Kaigi")?;
            }
            call.metadata = metadata_from_arguments(&raw.arguments, &[])?;
            if let Some(start_ms) = raw.arguments.get("scheduled_start_ms") {
                call.scheduled_start_ms = Some(
                    start_ms
                        .parse()
                        .context("invalid scheduled_start_ms for Kaigi")?,
                );
            }
            if let Some(billing) = raw.arguments.get("billing_account") {
                if !billing.is_empty() {
                    call.billing_account = Some(
                        parse_account_id(billing)
                            .with_context(|| format!("invalid billing_account '{billing}'"))?,
                    );
                }
            }
            call.privacy_mode = parse_kaigi_privacy_mode(raw)?;
            call.relay_manifest = parse_optional_kaigi_manifest(raw, "relay_manifest")?;
            Ok(KaigiCreate { call }.into())
        }
        #[cfg(feature = "kaigi-fixtures")]
        "JoinKaigi" => {
            ensure_kind(raw, "Custom")?;
            let call_id = parse_kaigi_id(raw, "call")?;
            let participant = parse_account_id(get_arg(raw, "participant")?)?;
            let commitment = parse_kaigi_commitment(raw)?;
            let nullifier = parse_kaigi_nullifier(raw)?;
            let roster_root =
                parse_optional_hash_literal(raw.arguments.get("roster_root"), "roster_root")?;
            let proof = raw
                .arguments
                .get("proof")
                .map(|value| BASE64.decode(value).context("invalid Kaigi proof base64"))
                .transpose()?;
            Ok(KaigiJoin {
                call_id,
                participant,
                commitment,
                nullifier,
                roster_root,
                proof,
            }
            .into())
        }
        #[cfg(feature = "kaigi-fixtures")]
        "LeaveKaigi" => {
            ensure_kind(raw, "Custom")?;
            let call_id = parse_kaigi_id(raw, "call")?;
            let participant = parse_account_id(get_arg(raw, "participant")?)?;
            let commitment = parse_kaigi_commitment(raw)?;
            let nullifier = parse_kaigi_nullifier(raw)?;
            let roster_root =
                parse_optional_hash_literal(raw.arguments.get("roster_root"), "roster_root")?;
            let proof = raw
                .arguments
                .get("proof")
                .map(|value| BASE64.decode(value).context("invalid Kaigi proof base64"))
                .transpose()?;
            Ok(KaigiLeave {
                call_id,
                participant,
                commitment,
                nullifier,
                roster_root,
                proof,
            }
            .into())
        }
        #[cfg(feature = "kaigi-fixtures")]
        "EndKaigi" => {
            ensure_kind(raw, "Custom")?;
            let call_id = parse_kaigi_id(raw, "call")?;
            let ended_at_ms = raw
                .arguments
                .get("ended_at_ms")
                .map(|value| value.parse().context("invalid ended_at_ms for Kaigi"))
                .transpose()?;
            Ok(KaigiEnd {
                call_id,
                ended_at_ms,
            }
            .into())
        }
        #[cfg(feature = "kaigi-fixtures")]
        "RecordKaigiUsage" => {
            ensure_kind(raw, "Custom")?;
            let call_id = parse_kaigi_id(raw, "call")?;
            let duration_ms: u64 = get_arg(raw, "duration_ms")?
                .parse()
                .context("invalid duration_ms for Kaigi usage")?;
            if duration_ms == 0 {
                bail!("duration_ms must be greater than zero for Kaigi usage");
            }
            let billed_gas: u64 = raw
                .arguments
                .get("billed_gas")
                .map(|value| value.parse().context("invalid billed_gas for Kaigi usage"))
                .transpose()?
                .unwrap_or(0);
            let usage_commitment = parse_optional_hash_literal(
                raw.arguments.get("usage_commitment"),
                "usage_commitment",
            )?;
            let proof = raw
                .arguments
                .get("proof")
                .map(|value| {
                    BASE64
                        .decode(value)
                        .context("invalid Kaigi usage proof base64")
                })
                .transpose()?;
            Ok(KaigiRecordUsage {
                call_id,
                duration_ms,
                billed_gas,
                usage_commitment,
                proof,
            }
            .into())
        }
        #[cfg(feature = "kaigi-fixtures")]
        "SetKaigiRelayManifest" => {
            ensure_kind(raw, "Custom")?;
            let call_id = parse_kaigi_id(raw, "call")?;
            let relay_manifest = parse_optional_kaigi_manifest(raw, "relay_manifest")?;
            Ok(KaigiSetManifest {
                call_id,
                relay_manifest,
            }
            .into())
        }
        #[cfg(feature = "kaigi-fixtures")]
        "RegisterKaigiRelay" => {
            ensure_kind(raw, "Custom")?;
            let relay_id = parse_account_id(get_arg(raw, "relay.relay_id")?)?;
            let hpke_public_key = BASE64
                .decode(get_arg(raw, "relay.hpke_public_key")?)
                .context("invalid relay.hpke_public_key base64")?;
            let bandwidth_class: u8 = get_arg(raw, "relay.bandwidth_class")?
                .parse()
                .context("invalid relay.bandwidth_class for Kaigi")?;
            Ok(KaigiRegisterRelay {
                relay: KaigiRelayRegistration {
                    relay_id,
                    hpke_public_key,
                    bandwidth_class,
                },
            }
            .into())
        }
        "RegisterVerifyingKey" => {
            build_verifying_key_instruction(raw, VerifyingKeyAction::Register)
        }
        "UpdateVerifyingKey" => build_verifying_key_instruction(raw, VerifyingKeyAction::Update),
        "DeprecateVerifyingKey" => build_verifying_key_instruction(raw, VerifyingKeyAction::Update),
        "UnregisterPeer" => {
            ensure_kind(raw, "Unregister")?;
            let peer: PeerId = get_arg(raw, "peer")?.parse().context("invalid peer id")?;
            Ok(Unregister::peer(peer).into())
        }
        "TransferAsset" => {
            ensure_kind(raw, "Transfer")?;
            let asset_id = parse_asset_id(get_arg(raw, "asset")?)?;
            let quantity = parse_numeric(get_arg(raw, "quantity")?)?;
            let destination = parse_account_id(get_arg(raw, "destination")?)?;
            Ok(Transfer::asset_numeric(asset_id, quantity, destination).into())
        }
        "TransferDomain" => {
            ensure_kind(raw, "Transfer")?;
            let source = parse_account_id(get_arg(raw, "source")?)?;
            let domain: DomainId = get_arg(raw, "domain")?
                .parse()
                .context("invalid domain id")?;
            let destination = parse_account_id(get_arg(raw, "destination")?)?;
            Ok(Transfer::domain(source, domain, destination).into())
        }
        "TransferAssetDefinition" => {
            ensure_kind(raw, "Transfer")?;
            let source = parse_account_id(get_arg(raw, "source")?)?;
            let definition: AssetDefinitionId = get_arg(raw, "definition")?
                .parse()
                .context("invalid asset definition id")?;
            let destination = parse_account_id(get_arg(raw, "destination")?)?;
            Ok(Transfer::asset_definition(source, definition, destination).into())
        }
        "TransferNft" => {
            ensure_kind(raw, "Transfer")?;
            let source = parse_account_id(get_arg(raw, "source")?)?;
            let nft_id = parse_nft_id(get_arg(raw, "nft")?)?;
            let destination = parse_account_id(get_arg(raw, "destination")?)?;
            Ok(Transfer::nft(source, nft_id, destination).into())
        }
        "MintAsset" => {
            ensure_kind(raw, "Mint")?;
            let asset_id = parse_asset_id(get_arg(raw, "asset")?)?;
            let quantity = parse_numeric(get_arg(raw, "quantity")?)?;
            Ok(Mint::asset_numeric(quantity, asset_id).into())
        }
        "MintTriggerRepetitions" => {
            ensure_kind(raw, "Mint")?;
            let trigger: TriggerId = get_arg(raw, "trigger")?
                .parse()
                .context("invalid trigger id")?;
            let repetitions: u32 = get_arg(raw, "repetitions")?
                .parse()
                .context("invalid trigger repetitions count")?;
            Ok(Mint::trigger_repetitions(repetitions, trigger).into())
        }
        "BurnAsset" => {
            ensure_kind(raw, "Burn")?;
            let asset_id = parse_asset_id(get_arg(raw, "asset")?)?;
            let quantity = parse_numeric(get_arg(raw, "quantity")?)?;
            Ok(Burn::asset_numeric(quantity, asset_id).into())
        }
        "BurnTriggerRepetitions" => {
            ensure_kind(raw, "Burn")?;
            let trigger: TriggerId = get_arg(raw, "trigger")?
                .parse()
                .context("invalid trigger id")?;
            let repetitions: u32 = get_arg(raw, "repetitions")?
                .parse()
                .context("invalid trigger repetitions count")?;
            Ok(Burn::trigger_repetitions(repetitions, trigger).into())
        }
        "SetDomainKeyValue" => {
            ensure_kind(raw, "SetKeyValue")?;
            let domain: DomainId = get_arg(raw, "domain")?
                .parse()
                .context("invalid domain id")?;
            let key: Name = get_arg(raw, "key")?
                .parse()
                .context("invalid metadata key")?;
            let value = Json::new(get_arg(raw, "value")?.to_owned());
            Ok(SetKeyValue::domain(domain, key, value).into())
        }
        "SetAccountKeyValue" => {
            ensure_kind(raw, "SetKeyValue")?;
            let account = parse_account_id(get_arg(raw, "account")?)?;
            let key: Name = get_arg(raw, "key")?
                .parse()
                .context("invalid metadata key")?;
            let value = Json::new(get_arg(raw, "value")?.to_owned());
            Ok(SetKeyValue::account(account, key, value).into())
        }
        "SetAssetDefinitionKeyValue" => {
            ensure_kind(raw, "SetKeyValue")?;
            let definition: AssetDefinitionId = get_arg(raw, "definition")?
                .parse()
                .context("invalid asset definition id")?;
            let key: Name = get_arg(raw, "key")?
                .parse()
                .context("invalid metadata key")?;
            let value = Json::new(get_arg(raw, "value")?.to_owned());
            Ok(SetKeyValue::asset_definition(definition, key, value).into())
        }
        "SetParameter" => {
            ensure_kind(raw, "SetParameter")?;
            let parameter_json = get_arg(raw, "parameter")?;
            let parameter: Parameter =
                json::from_str(parameter_json).context("invalid parameter payload")?;
            Ok(SetParameter::new(parameter).into())
        }
        "SetAssetKeyValue" => {
            ensure_kind(raw, "SetKeyValue")?;
            let asset = parse_asset_id(get_arg(raw, "asset")?)?;
            let key: Name = get_arg(raw, "key")?
                .parse()
                .context("invalid metadata key")?;
            let value = Json::new(get_arg(raw, "value")?.to_owned());
            Ok(SetAssetKeyValue::new(asset, key, value).into())
        }
        "SetTriggerKeyValue" => {
            ensure_kind(raw, "SetKeyValue")?;
            let trigger: TriggerId = get_arg(raw, "trigger")?
                .parse()
                .context("invalid trigger id")?;
            let key: Name = get_arg(raw, "key")?
                .parse()
                .context("invalid metadata key")?;
            let value = Json::new(get_arg(raw, "value")?.to_owned());
            Ok(SetKeyValue::trigger(trigger, key, value).into())
        }
        "UpgradeExecutor" => {
            ensure_kind(raw, "Upgrade")?;
            let bytecode_b64 = get_arg(raw, "bytecode")?;
            let bytecode = BASE64
                .decode(bytecode_b64)
                .context("invalid executor bytecode base64")?;
            Ok(Upgrade::new(Executor::new(IvmBytecode::from_compiled(bytecode))).into())
        }
        "RemoveDomainKeyValue" => {
            ensure_kind(raw, "RemoveKeyValue")?;
            let domain: DomainId = get_arg(raw, "domain")?
                .parse()
                .context("invalid domain id")?;
            let key: Name = get_arg(raw, "key")?
                .parse()
                .context("invalid metadata key")?;
            Ok(RemoveKeyValue::domain(domain, key).into())
        }
        "RemoveAccountKeyValue" => {
            ensure_kind(raw, "RemoveKeyValue")?;
            let account = parse_account_id(get_arg(raw, "account")?)?;
            let key: Name = get_arg(raw, "key")?
                .parse()
                .context("invalid metadata key")?;
            Ok(RemoveKeyValue::account(account, key).into())
        }
        "RemoveAssetDefinitionKeyValue" => {
            ensure_kind(raw, "RemoveKeyValue")?;
            let definition: AssetDefinitionId = get_arg(raw, "definition")?
                .parse()
                .context("invalid asset definition id")?;
            let key: Name = get_arg(raw, "key")?
                .parse()
                .context("invalid metadata key")?;
            Ok(RemoveKeyValue::asset_definition(definition, key).into())
        }
        "RemoveAssetKeyValue" => {
            ensure_kind(raw, "RemoveKeyValue")?;
            let asset = parse_asset_id(get_arg(raw, "asset")?)?;
            let key: Name = get_arg(raw, "key")?
                .parse()
                .context("invalid metadata key")?;
            Ok(RemoveAssetKeyValue::new(asset, key).into())
        }
        "RemoveTriggerKeyValue" => {
            ensure_kind(raw, "RemoveKeyValue")?;
            let trigger: TriggerId = get_arg(raw, "trigger")?
                .parse()
                .context("invalid trigger id")?;
            let key: Name = get_arg(raw, "key")?
                .parse()
                .context("invalid metadata key")?;
            Ok(RemoveKeyValue::trigger(trigger, key).into())
        }
        "GrantPermission" => {
            ensure_kind(raw, "Grant")?;
            let destination = parse_account_id(get_arg(raw, "destination")?)?;
            let permission = build_permission(get_arg(raw, "permission")?)?;
            Ok(Grant::account_permission(permission, destination).into())
        }
        "RevokePermission" => {
            ensure_kind(raw, "Revoke")?;
            let destination = parse_account_id(get_arg(raw, "destination")?)?;
            let permission = build_permission(get_arg(raw, "permission")?)?;
            Ok(Revoke::account_permission(permission, destination).into())
        }
        "GrantRole" => {
            ensure_kind(raw, "Grant")?;
            let destination = parse_account_id(get_arg(raw, "destination")?)?;
            let role: RoleId = get_arg(raw, "role")?.parse().context("invalid role id")?;
            Ok(Grant::account_role(role, destination).into())
        }
        "RevokeRole" => {
            ensure_kind(raw, "Revoke")?;
            let destination = parse_account_id(get_arg(raw, "destination")?)?;
            let role: RoleId = get_arg(raw, "role")?.parse().context("invalid role id")?;
            Ok(Revoke::account_role(role, destination).into())
        }
        "GrantRolePermission" => {
            ensure_kind(raw, "Grant")?;
            let destination: RoleId = get_arg(raw, "destination")?
                .parse()
                .context("invalid role id")?;
            let permission = build_permission(get_arg(raw, "permission")?)?;
            Ok(Grant::role_permission(permission, destination).into())
        }
        "RevokeRolePermission" => {
            ensure_kind(raw, "Revoke")?;
            let destination: RoleId = get_arg(raw, "destination")?
                .parse()
                .context("invalid role id")?;
            let permission = build_permission(get_arg(raw, "permission")?)?;
            Ok(Revoke::role_permission(permission, destination).into())
        }
        "RepoInitiate" => build_repo_initiate_instruction(raw),
        "RepoReverse" => build_repo_reverse_instruction(raw),
        "SettlementDvp" => build_settlement_instruction(raw, SettlementInstructionKind::Dvp),
        "SettlementPvp" => build_settlement_instruction(raw, SettlementInstructionKind::Pvp),
        other => bail!("unsupported instruction action '{other}'"),
    }
}

fn ensure_kind(raw: &RawInstruction, expected: &str) -> Result<()> {
    if raw.kind != expected {
        bail!(
            "instruction kind mismatch: expected '{expected}', found '{}'",
            raw.kind
        );
    }
    Ok(())
}

fn get_arg<'a>(raw: &'a RawInstruction, key: &str) -> Result<&'a str> {
    raw.arguments
        .get(key)
        .map(|s| s.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing '{key}' argument"))
}

fn parse_numeric(value: &str) -> Result<Numeric> {
    value
        .parse()
        .map_err(|err| anyhow::anyhow!("invalid numeric value '{value}': {err}"))
}

fn authority_chain_discriminant(authority: &str) -> Option<u16> {
    let (address_part, _) = authority.split_once('@')?;
    match address::AccountAddress::parse_any(address_part, None) {
        Ok((_, address::AccountAddressFormat::IH58 { network_prefix })) => Some(network_prefix),
        Ok(_) => None,
        Err(_) => None,
    }
}

fn normalize_authority_hint(authority: &str) -> String {
    let trimmed = authority.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if let Ok(canonical) = AccountId::canonicalize(trimmed) {
        return canonical;
    }
    if let Some((address_part, _)) = trimmed.rsplit_once('@') {
        if let Ok(canonical) = AccountId::canonicalize(address_part) {
            return canonical;
        }
        return address_part.to_string();
    }
    trimmed.to_string()
}

fn parse_account_id(value: &str) -> Result<AccountId> {
    let (signatory_hint, domain_part) = value
        .split_once('@')
        .ok_or_else(|| anyhow::anyhow!("account id '{value}' must contain '@'"))?;
    let domain: DomainId = domain_part
        .parse()
        .with_context(|| format!("invalid domain id '{domain_part}'"))?;
    let expected_prefix = address::chain_discriminant();
    if let Ok((address, _)) =
        address::AccountAddress::parse_any(signatory_hint, Some(expected_prefix))
    {
        let account = address
            .to_account_id(&domain)
            .map_err(|err| anyhow::anyhow!(err.code_str()))?;
        return Ok(account);
    }
    if let Ok(signatory) = signatory_hint.parse::<PublicKey>() {
        return Ok(AccountId::of(domain, signatory));
    }
    let seed = derive_seed(signatory_hint, domain_part);
    let keypair = KeyPair::from_seed(seed, Algorithm::Ed25519);
    Ok(AccountId::of(domain, keypair.public_key().clone()))
}

fn parse_asset_id(value: &str) -> Result<AssetId> {
    if let Ok(id) = value.parse::<AssetId>() {
        return Ok(id);
    }

    let (definition_part, owner_part) = value
        .rsplit_once('#')
        .ok_or_else(|| anyhow::anyhow!("asset id '{value}' must contain '#' separators"))?;
    let account = parse_account_id(owner_part)
        .with_context(|| format!("invalid account '{owner_part}' in asset id '{value}'"))?;
    let definition_str = if let Some(prefix) = definition_part.strip_suffix('#') {
        if prefix.is_empty() {
            bail!("asset id '{value}' missing definition name");
        }
        format!("{prefix}#{}", account.domain())
    } else {
        definition_part.to_string()
    };
    let definition: AssetDefinitionId = definition_str.parse().with_context(|| {
        format!("invalid definition portion '{definition_str}' in asset id '{value}'")
    })?;

    Ok(AssetId::new(definition, account))
}

fn parse_nft_id(value: &str) -> Result<NftId> {
    value
        .parse::<NftId>()
        .with_context(|| format!("invalid NFT id '{value}'"))
}

fn derive_seed(left: &str, right: &str) -> Vec<u8> {
    let mut seed = [0u8; 32];
    for (index, byte) in left
        .as_bytes()
        .iter()
        .chain(right.as_bytes().iter())
        .enumerate()
    {
        seed[index % seed.len()] ^= *byte;
    }
    seed.to_vec()
}

fn build_permission(name: &str) -> Result<Permission> {
    let ident = Ident::from_str(name).context("invalid permission name")?;
    Ok(Permission::new(ident, Json::new(Value::Object(Map::new()))))
}

fn metadata_from_arguments(
    args: &BTreeMap<String, String>,
    passthrough: &[&str],
) -> Result<Metadata> {
    let mut metadata = Metadata::default();
    for key in passthrough {
        if let Some(value) = args.get(*key) {
            metadata.insert((*key).parse()?, Json::new(value.clone()));
        }
    }
    for (key, value) in args {
        if let Some(stripped) = key.strip_prefix("metadata.") {
            metadata.insert(stripped.parse()?, Json::new(value.clone()));
        }
    }
    Ok(metadata)
}

enum VerifyingKeyAction {
    Register,
    Update,
}

fn build_verifying_key_instruction(
    raw: &RawInstruction,
    action: VerifyingKeyAction,
) -> Result<InstructionBox> {
    ensure_kind(raw, "Custom")?;
    let backend = get_arg(raw, "backend")?;
    let name = get_arg(raw, "name")?;
    let id = VerifyingKeyId::new(backend.to_string(), name.to_string());
    let record = build_verifying_key_record(raw, backend)?;
    let instruction = match action {
        VerifyingKeyAction::Register => verifying_keys::RegisterVerifyingKey { id, record }.into(),
        VerifyingKeyAction::Update => verifying_keys::UpdateVerifyingKey { id, record }.into(),
    };
    Ok(instruction)
}

fn build_verifying_key_record(raw: &RawInstruction, backend: &str) -> Result<VerifyingKeyRecord> {
    let version = parse_u32_field(get_arg(raw, "record.version")?, "record.version")?;
    let circuit_id = get_arg(raw, "record.circuit_id")?.to_string();
    let backend_tag = parse_backend_tag(get_arg(raw, "record.backend_tag")?)?;
    let curve = raw
        .arguments
        .get("record.curve")
        .and_then(|value| {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        })
        .unwrap_or_else(|| "unknown".to_string());
    let schema_hash = parse_hex32(
        get_arg(raw, "record.public_inputs_schema_hash_hex")?,
        "record.public_inputs_schema_hash_hex",
    )?;

    let inline_bytes = raw
        .arguments
        .get("record.vk_bytes_b64")
        .map(|value| {
            BASE64
                .decode(value)
                .context("invalid record.vk_bytes_b64 base64")
        })
        .transpose()?;
    let commitment = if inline_bytes.is_some() {
        verifying_key_commitment(backend, inline_bytes.as_ref().unwrap())
    } else {
        parse_hex32(
            get_arg(raw, "record.commitment_hex")?,
            "record.commitment_hex",
        )?
    };
    if let Some(commitment_hex) = raw.arguments.get("record.commitment_hex") {
        let expected = parse_hex32(commitment_hex, "record.commitment_hex")?;
        if expected != commitment {
            bail!("record.commitment_hex mismatch with provided vk_bytes");
        }
    }

    let mut record = VerifyingKeyRecord::new(
        version,
        circuit_id,
        backend_tag,
        curve,
        schema_hash,
        commitment,
    );

    let vk_len_opt = parse_opt_u32(raw.arguments.get("record.vk_len"), "record.vk_len")?;
    let vk_len = if let Some(bytes) = inline_bytes.as_ref() {
        let actual = u32::try_from(bytes.len()).context("vk_bytes length exceeds u32::MAX")?;
        if let Some(explicit) = vk_len_opt {
            if explicit != actual {
                bail!("record.vk_len mismatch with inline vk_bytes length");
            }
        }
        actual
    } else {
        vk_len_opt
            .ok_or_else(|| anyhow::anyhow!("record.vk_len is required when vk_bytes are omitted"))?
    };
    if vk_len == 0 {
        bail!("record.vk_len must be greater than zero");
    }
    record.vk_len = vk_len;

    let max_proof_bytes = parse_opt_u32(
        raw.arguments.get("record.max_proof_bytes"),
        "record.max_proof_bytes",
    )?
    .unwrap_or(0);
    record.max_proof_bytes = max_proof_bytes;

    let gas_schedule_id = get_arg(raw, "record.gas_schedule_id")?.trim();
    if gas_schedule_id.is_empty() {
        bail!("record.gas_schedule_id must not be empty");
    }
    record.gas_schedule_id = Some(gas_schedule_id.to_string());

    record.metadata_uri_cid = parse_opt_string(raw.arguments.get("record.metadata_uri_cid"));
    record.vk_bytes_cid = parse_opt_string(raw.arguments.get("record.vk_bytes_cid"));
    record.activation_height = parse_opt_u64(
        raw.arguments.get("record.activation_height"),
        "record.activation_height",
    )?;
    let deprecation_height = parse_opt_u64(
        raw.arguments.get("record.deprecation_height"),
        "record.deprecation_height",
    )?;
    let withdraw_height = parse_opt_u64(
        raw.arguments.get("record.withdraw_height"),
        "record.withdraw_height",
    )?;
    if deprecation_height.is_some()
        && withdraw_height.is_some()
        && deprecation_height != withdraw_height
    {
        bail!("record.deprecation_height must match record.withdraw_height when both are set");
    }
    record.withdraw_height = withdraw_height.or(deprecation_height);

    let status = raw
        .arguments
        .get("record.status")
        .map(|value| parse_confidential_status(value))
        .transpose()?
        .unwrap_or(ConfidentialStatus::Active);
    record.status = status;

    if let Some(bytes) = inline_bytes {
        record.key = Some(VerifyingKeyBox::new(backend.into(), bytes));
    }

    Ok(record)
}

fn parse_backend_tag(value: &str) -> Result<BackendTag> {
    match value {
        "halo2-ipa-pasta" => Ok(BackendTag::Halo2IpaPasta),
        "halo2-bn254" => Ok(BackendTag::Halo2Bn254),
        "groth16" => Ok(BackendTag::Groth16),
        "stark" => Ok(BackendTag::Stark),
        "unsupported" => Ok(BackendTag::Unsupported),
        other => bail!("unsupported record.backend_tag value '{other}'"),
    }
}

fn parse_confidential_status(value: &str) -> Result<ConfidentialStatus> {
    match value {
        "Proposed" => Ok(ConfidentialStatus::Proposed),
        "Active" => Ok(ConfidentialStatus::Active),
        "Deprecated" => Ok(ConfidentialStatus::Withdrawn),
        "Withdrawn" => Ok(ConfidentialStatus::Withdrawn),
        other => bail!("unsupported record.status value '{other}'"),
    }
}

fn parse_hex32(value: &str, field: &str) -> Result<[u8; 32]> {
    let trimmed = value.trim();
    let bytes = hex::decode(trimmed)
        .with_context(|| format!("{field} must be a 64-character hexadecimal string"))?;
    let array: [u8; 32] = bytes
        .as_slice()
        .try_into()
        .map_err(|_| anyhow::anyhow!("{field} must contain exactly 32 bytes"))?;
    Ok(array)
}

fn parse_opt_string(value: Option<&String>) -> Option<String> {
    value.and_then(|raw| {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn parse_opt_u32(value: Option<&String>, field: &str) -> Result<Option<u32>> {
    value
        .map(|raw| {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                Ok(None)
            } else {
                trimmed
                    .parse::<u32>()
                    .with_context(|| format!("invalid {field} value"))
                    .map(Some)
            }
        })
        .transpose()
        .map(|opt| opt.flatten())
}

fn parse_opt_u64(value: Option<&String>, field: &str) -> Result<Option<u64>> {
    value
        .map(|raw| {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                Ok(None)
            } else {
                trimmed
                    .parse::<u64>()
                    .with_context(|| format!("invalid {field} value"))
                    .map(Some)
            }
        })
        .transpose()
        .map(|opt| opt.flatten())
}

fn parse_u16_field(value: &str, field: &str) -> Result<u16> {
    value
        .trim()
        .parse::<u16>()
        .with_context(|| format!("invalid {field} value"))
}

fn parse_u32_field(value: &str, field: &str) -> Result<u32> {
    value
        .trim()
        .parse::<u32>()
        .with_context(|| format!("invalid {field} value"))
}

fn parse_u64_field(value: &str, field: &str) -> Result<u64> {
    value
        .trim()
        .parse::<u64>()
        .with_context(|| format!("invalid {field} value"))
}

fn verifying_key_commitment(backend: &str, bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(backend.as_bytes());
    hasher.update(bytes);
    hasher.finalize().into()
}

fn parse_trigger_repeats(raw: &RawInstruction) -> Result<Repeats> {
    match raw.arguments.get("repeats") {
        None => Ok(Repeats::Indefinitely),
        Some(value) if value.eq_ignore_ascii_case("indefinite") => Ok(Repeats::Indefinitely),
        Some(value) => {
            let count: u32 = value.parse().context("invalid trigger repeats count")?;
            if count == 0 {
                bail!("trigger repeats must be greater than zero");
            }
            Ok(Repeats::Exactly(count))
        }
    }
}

fn parse_trigger_instructions(raw: &RawInstruction) -> Result<Vec<InstructionBox>> {
    let mut grouped: BTreeMap<usize, (Option<String>, BTreeMap<String, String>)> = BTreeMap::new();
    for (key, value) in &raw.arguments {
        let Some(rest) = key.strip_prefix("instruction.") else {
            continue;
        };
        let (index_str, suffix) = rest
            .split_once('.')
            .ok_or_else(|| anyhow::anyhow!("malformed trigger instruction key '{key}'"))?;
        let index: usize = index_str
            .parse()
            .context("trigger instruction index must be numeric")?;
        let entry = grouped
            .entry(index)
            .or_insert_with(|| (None, BTreeMap::new()));
        if suffix == "kind" {
            entry.0 = Some(value.clone());
        } else if let Some(arg_key) = suffix.strip_prefix("arg.") {
            if arg_key.is_empty() {
                bail!("trigger instruction argument key must not be empty");
            }
            entry.1.insert(arg_key.to_owned(), value.clone());
        } else {
            bail!("unrecognised trigger instruction key '{key}'");
        }
    }
    if grouped.is_empty() {
        bail!("trigger registration requires at least one nested instruction");
    }
    let mut instructions = Vec::with_capacity(grouped.len());
    for (index, (kind_opt, arguments)) in grouped {
        let kind = kind_opt.ok_or_else(|| {
            anyhow::anyhow!("instruction.{index}.kind missing for trigger registration")
        })?;
        let raw_nested = RawInstruction { kind, arguments };
        instructions.push(build_instruction(&raw_nested)?);
    }
    Ok(instructions)
}

fn parse_pipeline_filter(raw: &RawInstruction) -> Result<PipelineEventFilterBox> {
    let kind = raw
        .arguments
        .get("filter.kind")
        .ok_or_else(|| anyhow::anyhow!("filter.kind must be provided for RegisterPipelineTrigger"))?
        .as_str();
    match kind {
        "Transaction" | "transaction" => {
            let mut filter = TransactionEventFilter::default();
            if let Some(hash_literal) = raw.arguments.get("filter.transaction.hash") {
                let hash = parse_hash_literal(hash_literal)
                    .context("invalid pipeline transaction hash")?;
                filter.hash = Some(HashOf::from_untyped_unchecked(hash));
            }
            if let Some(height) = raw.arguments.get("filter.transaction.block_height") {
                let nz = parse_nonzero_u64(height, "filter.transaction.block_height")?;
                filter.block_height = Some(Some(nz));
            }
            if let Some(status) = raw.arguments.get("filter.transaction.status") {
                filter.status = Some(parse_transaction_status(status)?);
            }
            Ok(PipelineEventFilterBox::Transaction(filter))
        }
        "Block" | "block" => {
            let mut filter = BlockEventFilter::default();
            if let Some(height) = raw.arguments.get("filter.block.height") {
                let nz = parse_nonzero_u64(height, "filter.block.height")?;
                filter.height = Some(nz);
            }
            if let Some(status) = raw.arguments.get("filter.block.status") {
                filter.status = Some(parse_block_status(status)?);
            }
            Ok(PipelineEventFilterBox::Block(filter))
        }
        "Merge" | "merge" => {
            let mut filter = MergeLedgerEventFilter::default();
            if let Some(epoch) = raw.arguments.get("filter.merge.epoch_id") {
                let value = epoch
                    .parse::<u64>()
                    .context("invalid filter.merge.epoch_id value")?;
                filter.epoch_id = Some(value);
            }
            Ok(PipelineEventFilterBox::Merge(filter))
        }
        "Witness" | "witness" => {
            let mut filter = WitnessEventFilter::default();
            if let Some(block_hash) = raw.arguments.get("filter.witness.block_hash") {
                let hash = parse_hash_literal(block_hash)
                    .context("invalid filter.witness.block_hash value")?;
                filter.block_hash = Some(HashOf::from_untyped_unchecked(hash));
            }
            if let Some(height) = raw.arguments.get("filter.witness.height") {
                let nz = parse_nonzero_u64(height, "filter.witness.height")?;
                filter.height = Some(nz);
            }
            if let Some(view) = raw.arguments.get("filter.witness.view") {
                let value = view
                    .parse::<u64>()
                    .context("invalid filter.witness.view value")?;
                filter.view = Some(value);
            }
            Ok(PipelineEventFilterBox::Witness(filter))
        }
        other => bail!("Unsupported pipeline trigger filter kind '{other}'"),
    }
}

#[cfg(feature = "kaigi-fixtures")]
fn parse_kaigi_id(raw: &RawInstruction, prefix: &str) -> Result<KaigiId> {
    let domain_key = format!("{prefix}.domain_id");
    let call_key = format!("{prefix}.call_name");
    let domain_str = get_arg(raw, &domain_key)?;
    let call_str = get_arg(raw, &call_key)?;
    let domain: DomainId = domain_str
        .parse()
        .with_context(|| format!("invalid Kaigi domain id '{domain_str}'"))?;
    let call_name: Name = call_str
        .parse()
        .with_context(|| format!("invalid Kaigi call name '{call_str}'"))?;
    Ok(KaigiId::new(domain, call_name))
}

#[cfg(feature = "kaigi-fixtures")]
fn parse_kaigi_privacy_mode(raw: &RawInstruction) -> Result<KaigiPrivacyMode> {
    let mode = raw
        .arguments
        .get("privacy.mode")
        .map(String::as_str)
        .unwrap_or("Transparent");
    match mode {
        "Transparent" => Ok(KaigiPrivacyMode::Transparent),
        "ZkRosterV1" => Ok(KaigiPrivacyMode::ZkRosterV1),
        other => bail!("Unsupported Kaigi privacy mode '{other}'"),
    }
}

#[cfg(feature = "kaigi-fixtures")]
fn parse_optional_kaigi_manifest(
    raw: &RawInstruction,
    prefix: &str,
) -> Result<Option<KaigiRelayManifest>> {
    let expiry_key = format!("{prefix}.expiry_ms");
    let hop_prefix = format!("{prefix}.hop.");

    let mut hops: BTreeMap<usize, RelayHopParts> = BTreeMap::new();
    for (key, value) in &raw.arguments {
        if let Some(remainder) = key.strip_prefix(&hop_prefix) {
            let (index_str, field) = remainder
                .split_once('.')
                .ok_or_else(|| anyhow::anyhow!("Malformed Kaigi relay manifest key '{key}'"))?;
            let index: usize = index_str
                .parse()
                .context("invalid relay hop index for Kaigi manifest")?;
            let entry = hops.entry(index).or_default();
            match field {
                "relay_id" => {
                    entry.relay_id = Some(parse_account_id(value)?);
                }
                "hpke_public_key" => {
                    entry.hpke_public_key = Some(
                        BASE64
                            .decode(value)
                            .context("invalid relay hop hpke_public_key base64")?,
                    );
                }
                "weight" => {
                    let weight: u8 = value
                        .parse()
                        .context("invalid relay hop weight for Kaigi manifest")?;
                    entry.weight = Some(weight);
                }
                other => bail!("Unknown Kaigi relay manifest attribute '{other}'"),
            }
        }
    }

    let has_manifest = !hops.is_empty() || raw.arguments.contains_key(&expiry_key);
    if !has_manifest {
        return Ok(None);
    }

    let expiry_value = raw.arguments.get(&expiry_key).ok_or_else(|| {
        anyhow::anyhow!("relay_manifest.expiry_ms is required when manifest is provided")
    })?;
    let expiry_ms: u64 = expiry_value
        .parse()
        .context("invalid relay_manifest.expiry_ms for Kaigi")?;

    let mut hop_values = Vec::with_capacity(hops.len());
    for (index, parts) in hops {
        let relay_id = parts.relay_id.ok_or_else(|| {
            anyhow::anyhow!("relay_manifest.hop.{index}.relay_id is required for Kaigi manifest")
        })?;
        let hpke_public_key = parts.hpke_public_key.ok_or_else(|| {
            anyhow::anyhow!(
                "relay_manifest.hop.{index}.hpke_public_key is required for Kaigi manifest"
            )
        })?;
        let weight = parts.weight.ok_or_else(|| {
            anyhow::anyhow!("relay_manifest.hop.{index}.weight is required for Kaigi manifest")
        })?;
        hop_values.push(KaigiRelayHop {
            relay_id,
            hpke_public_key,
            weight,
        });
    }

    Ok(Some(KaigiRelayManifest {
        hops: hop_values,
        expiry_ms,
    }))
}

fn parse_hash_literal(value: &str) -> Result<Hash> {
    let trimmed = value.trim();
    if let Some(rest) = trimmed.strip_prefix("hash:") {
        let (body, checksum) = rest
            .rsplit_once('#')
            .ok_or_else(|| anyhow::anyhow!("hash literal must include checksum '#': {trimmed}"))?;
        if body.len() != 64 {
            bail!("hash literal body must contain 64 hexadecimal digits: {trimmed}");
        }
        if checksum.len() != 4 {
            bail!("hash literal checksum must contain four hexadecimal digits: {trimmed}");
        }
        let body_upper = body.to_ascii_uppercase();
        let expected = crc16("hash", &body_upper);
        let provided =
            u16::from_str_radix(checksum, 16).context("invalid hash literal checksum")?;
        if expected != provided {
            bail!(
                "hash literal checksum mismatch: expected {:04X}, found {:04X}",
                expected,
                provided
            );
        }
        Hash::from_str(&body_upper).context("invalid hash literal body")
    } else if trimmed.len() == 64 && trimmed.chars().all(|ch| ch.is_ascii_hexdigit()) {
        let mut buf = [0u8; Hash::LENGTH];
        hex::decode_to_slice(trimmed, &mut buf)
            .with_context(|| format!("invalid hex hash literal '{trimmed}'"))?;
        Ok(Hash::prehashed(buf))
    } else {
        Hash::from_str(trimmed).with_context(|| format!("invalid hash literal '{trimmed}'"))
    }
}

fn parse_optional_hash_literal(value: Option<&String>, field: &str) -> Result<Option<Hash>> {
    match value {
        Some(literal) if literal.trim().is_empty() => Ok(None),
        Some(literal) => parse_hash_literal(literal)
            .map(Some)
            .with_context(|| format!("invalid {field} hash literal")),
        None => Ok(None),
    }
}

#[cfg(feature = "kaigi-fixtures")]
fn parse_kaigi_commitment(raw: &RawInstruction) -> Result<Option<KaigiParticipantCommitment>> {
    if let Some(literal) = raw.arguments.get("commitment.commitment") {
        let commitment = parse_hash_literal(literal).context("invalid Kaigi commitment hash")?;
        let alias_tag = raw.arguments.get("commitment.alias_tag").cloned();
        Ok(Some(KaigiParticipantCommitment {
            commitment,
            alias_tag,
        }))
    } else {
        Ok(None)
    }
}

#[cfg(feature = "kaigi-fixtures")]
fn parse_kaigi_nullifier(raw: &RawInstruction) -> Result<Option<KaigiParticipantNullifier>> {
    if let Some(literal) = raw.arguments.get("nullifier.digest") {
        let digest = parse_hash_literal(literal).context("invalid Kaigi nullifier digest")?;
        let issued_key = "nullifier.issued_at_ms";
        let issued_value = raw.arguments.get(issued_key).ok_or_else(|| {
            anyhow::anyhow!("{issued_key} is required when nullifier digest is provided")
        })?;
        let issued_at_ms: u64 = issued_value
            .parse()
            .context("invalid nullifier.issued_at_ms for Kaigi")?;
        Ok(Some(KaigiParticipantNullifier {
            digest,
            issued_at_ms,
        }))
    } else {
        Ok(None)
    }
}

fn crc16(tag: &str, body: &str) -> u16 {
    let mut crc: u16 = 0xFFFF;
    for byte in tag.as_bytes() {
        crc = crc16_update(crc, *byte);
    }
    crc = crc16_update(crc, b':');
    for byte in body.as_bytes() {
        crc = crc16_update(crc, *byte);
    }
    crc
}

fn crc16_update(mut crc: u16, byte: u8) -> u16 {
    crc ^= (byte as u16) << 8;
    for _ in 0..8 {
        if (crc & 0x8000) != 0 {
            crc = ((crc << 1) ^ 0x1021) & 0xFFFF;
        } else {
            crc = (crc << 1) & 0xFFFF;
        }
    }
    crc
}

#[derive(Default)]
struct RelayHopParts {
    relay_id: Option<AccountId>,
    hpke_public_key: Option<Vec<u8>>,
    weight: Option<u8>,
}

fn parse_transaction_status(value: &str) -> Result<TransactionStatus> {
    match value {
        "Queued" | "queued" => Ok(TransactionStatus::Queued),
        "Expired" | "expired" => Ok(TransactionStatus::Expired),
        "Approved" | "approved" => Ok(TransactionStatus::Approved),
        other => bail!("Unsupported transaction status '{other}' for pipeline trigger filter"),
    }
}

fn parse_block_status(value: &str) -> Result<BlockStatus> {
    match value {
        "Created" | "created" => Ok(BlockStatus::Created),
        "Approved" | "approved" => Ok(BlockStatus::Approved),
        "Committed" | "committed" => Ok(BlockStatus::Committed),
        "Applied" | "applied" => Ok(BlockStatus::Applied),
        other => bail!("Unsupported block status '{other}' for pipeline trigger filter"),
    }
}

fn parse_nonzero_u64(value: &str, field: &str) -> Result<NonZeroU64> {
    let parsed = value
        .parse::<u64>()
        .with_context(|| format!("invalid {field} value"))?;
    NonZeroU64::new(parsed).ok_or_else(|| anyhow::anyhow!("{} must be greater than zero", field))
}

fn expect_string<'a>(map: &'a Map, key: &str) -> Result<&'a str> {
    map.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("missing string field '{key}'"))
}

fn expect_u64(map: &Map, key: &str) -> Result<u64> {
    let value = map
        .get(key)
        .ok_or_else(|| anyhow::anyhow!("missing field '{key}'"))?;
    match value {
        Value::Number(number) => number_to_u64(number),
        _ => bail!("field '{key}' must be number"),
    }
}

fn parse_optional_u64(map: &Map, key: &str) -> Result<Option<u64>> {
    match map.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(number)) => Ok(Some(number_to_u64(number)?)),
        _ => bail!("field '{key}' must be number or null"),
    }
}

fn parse_optional_u32(map: &Map, key: &str) -> Result<Option<u32>> {
    match parse_optional_u64(map, key)? {
        None => Ok(None),
        Some(value) if value <= u32::MAX as u64 => Ok(Some(value as u32)),
        Some(_) => bail!("field '{key}' does not fit u32"),
    }
}

fn number_to_u64(number: &Number) -> Result<u64> {
    match number {
        Number::U64(v) => Ok(*v),
        Number::I64(v) if *v >= 0 => Ok(*v as u64),
        _ => bail!("value does not fit u64"),
    }
}

fn value_to_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Number(Number::U64(v)) => v.to_string(),
        Value::Number(Number::I64(v)) => v.to_string(),
        Value::Number(Number::F64(v)) => v.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => "null".to_string(),
        Value::Array(_) | Value::Object(_) => {
            json::to_json(value).unwrap_or_else(|_| "{}".to_string())
        }
    }
}

fn build_manifest(fixtures: &[Fixture], public_key_hex: &str) -> Value {
    let generated_at = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .expect("timestamp formatting");
    let entries: Vec<Value> = fixtures.iter().map(manifest_entry).collect();

    let mut schema = Map::new();
    schema.insert(
        "payload".into(),
        Value::String(PAYLOAD_SCHEMA_NAME.to_string()),
    );
    schema.insert(
        "signed".into(),
        Value::String(SIGNED_SCHEMA_NAME.to_string()),
    );

    let mut signing_key = Map::new();
    signing_key.insert("algorithm".into(), Value::String("ed25519".into()));
    signing_key.insert("seed_hex".into(), Value::String(SIGNING_SEED_HEX.into()));
    signing_key.insert(
        "public_key_hex".into(),
        Value::String(public_key_hex.into()),
    );

    let mut root = Map::new();
    root.insert("generated_at".into(), Value::String(generated_at));
    root.insert("schema".into(), Value::Object(schema));
    root.insert("signing_key".into(), Value::Object(signing_key));
    root.insert("fixtures".into(), Value::Array(entries));
    Value::Object(root)
}

fn manifest_entry(fixture: &Fixture) -> Value {
    let mut map = Map::new();
    map.insert("name".into(), Value::String(fixture.name.clone()));
    map.insert(
        "payload_base64".into(),
        Value::String(fixture.summary.payload_base64.clone()),
    );
    map.insert(
        "signed_base64".into(),
        Value::String(fixture.summary.signed_base64.clone()),
    );
    map.insert(
        "payload_hash".into(),
        Value::String(fixture.summary.payload_hash_hex.clone()),
    );
    map.insert(
        "signed_hash".into(),
        Value::String(fixture.summary.signed_hash_hex.clone()),
    );
    map.insert("chain".into(), Value::String(fixture.summary.chain.clone()));
    map.insert(
        "authority".into(),
        Value::String(fixture.summary.authority.clone()),
    );
    map.insert(
        "creation_time_ms".into(),
        Value::Number(Number::U64(fixture.summary.creation_time_ms)),
    );
    map.insert(
        "time_to_live_ms".into(),
        optional_u64_value(fixture.summary.ttl_ms),
    );
    map.insert(
        "nonce".into(),
        optional_u64_value(fixture.summary.nonce.map(|v| v as u64)),
    );
    map.insert(
        "encoded_file".into(),
        Value::String(format!("{}.norito", fixture.name)),
    );
    map.insert(
        "encoded_len".into(),
        Value::Number(Number::U64(fixture.encoded.len() as u64)),
    );
    map.insert(
        "signed_len".into(),
        Value::Number(Number::U64(fixture.signed_bytes.len() as u64)),
    );
    Value::Object(map)
}

fn build_fixtures_json(raw_fixtures: &[RawFixture], fixtures: &[Fixture]) -> Result<Value> {
    let summaries: BTreeMap<&str, &PayloadSummary> = fixtures
        .iter()
        .map(|fixture| (fixture.name.as_str(), &fixture.summary))
        .collect();
    let fixtures_by_name: BTreeMap<&str, &Fixture> = fixtures
        .iter()
        .map(|fixture| (fixture.name.as_str(), fixture))
        .collect();
    let mut entries = Vec::with_capacity(raw_fixtures.len());
    for raw in raw_fixtures {
        let summary = summaries.get(raw.name.as_str()).ok_or_else(|| {
            anyhow::anyhow!(
                "missing regenerated payload summary for fixture '{}'",
                raw.name
            )
        })?;
        let mut map = Map::new();
        map.insert("name".into(), Value::String(raw.name.clone()));
        if let Some(payload) = &raw.payload_json {
            let mut payload_value = payload.clone();
            if let Some(fixture) = fixtures_by_name.get(raw.name.as_str()) {
                if let Some(wire_payloads) = wire_payloads_from_encoded(&fixture.encoded)
                    .with_context(|| {
                        format!(
                            "failed to derive wire instruction payloads for '{}'",
                            raw.name
                        )
                    })?
                {
                    apply_wire_payloads_to_payload_json(&mut payload_value, &wire_payloads)
                        .with_context(|| {
                            format!(
                                "failed to inject wire payloads into fixture '{}'",
                                raw.name
                            )
                        })?;
                }
            }
            map.insert("payload".into(), payload_value);
        }
        map.insert(
            "encoded".into(),
            Value::String(summary.payload_base64.clone()),
        );
        map.insert(
            "payload_base64".into(),
            Value::String(summary.payload_base64.clone()),
        );
        map.insert(
            "signed_base64".into(),
            Value::String(summary.signed_base64.clone()),
        );
        map.insert(
            "payload_hash".into(),
            Value::String(summary.payload_hash_hex.clone()),
        );
        map.insert(
            "signed_hash".into(),
            Value::String(summary.signed_hash_hex.clone()),
        );
        map.insert("chain".into(), Value::String(summary.chain.clone()));
        let authority_value = raw
            .authority_hint
            .clone()
            .unwrap_or_else(|| summary.authority.clone());
        map.insert("authority".into(), Value::String(authority_value));
        map.insert(
            "creation_time_ms".into(),
            Value::Number(Number::U64(summary.creation_time_ms)),
        );
        map.insert("time_to_live_ms".into(), optional_u64_value(summary.ttl_ms));
        map.insert(
            "nonce".into(),
            optional_u64_value(summary.nonce.map(|v| v as u64)),
        );
        entries.push(Value::Object(map));
    }

    Ok(Value::Array(entries))
}

fn wire_payloads_from_encoded(encoded: &[u8]) -> Result<Option<Vec<WireInstructionPayload>>> {
    let mut cursor = encoded;
    let payload = TransactionPayload::decode(&mut cursor)?;
    if !cursor.is_empty() {
        bail!("payload decoding left trailing bytes");
    }

    let instructions = match payload.instructions {
        Executable::Instructions(instructions) => instructions,
        Executable::Ivm(_) => return Ok(None),
    };

    let registry = iroha_data_model::instruction_registry::default();
    let mut out = Vec::with_capacity(instructions.len());
    for instruction in instructions.iter() {
        let type_name = Instruction::id(&**instruction);
        let wire_name = registry.wire_id(type_name).unwrap_or(type_name);
        let payload_bytes = Instruction::dyn_encode(&**instruction);
        let framed = frame_instruction_payload(type_name, &payload_bytes)?;
        out.push(WireInstructionPayload {
            wire_name: wire_name.to_string(),
            payload_base64: BASE64.encode(framed),
        });
    }

    Ok(Some(out))
}

fn apply_wire_payloads_to_payload_json(
    payload: &mut Value,
    wire_payloads: &[WireInstructionPayload],
) -> Result<()> {
    let payload_obj = payload
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("payload must be an object"))?;
    let executable_value = payload_obj
        .get_mut("executable")
        .ok_or_else(|| anyhow::anyhow!("payload missing executable"))?;
    let executable_obj = executable_value
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("payload executable must be an object"))?;
    let instructions_value = executable_obj
        .get_mut("Instructions")
        .ok_or_else(|| anyhow::anyhow!("payload executable missing Instructions"))?;
    let instructions = instructions_value
        .as_array_mut()
        .ok_or_else(|| anyhow::anyhow!("payload Instructions must be an array"))?;
    if instructions.len() != wire_payloads.len() {
        bail!(
            "payload instructions length mismatch: expected {}, got {}",
            wire_payloads.len(),
            instructions.len()
        );
    }
    for (entry, wire) in instructions.iter_mut().zip(wire_payloads) {
        let obj = entry
            .as_object_mut()
            .ok_or_else(|| anyhow::anyhow!("instruction entries must be objects"))?;
        obj.insert("wire_name".into(), Value::String(wire.wire_name.clone()));
        obj.insert(
            "payload_base64".into(),
            Value::String(wire.payload_base64.clone()),
        );
    }
    Ok(())
}

fn write_fixtures_json(
    path: &Path,
    raw_fixtures: &[RawFixture],
    fixtures: &[Fixture],
) -> Result<()> {
    let value = build_fixtures_json(raw_fixtures, fixtures)?;
    let fixtures_json = json::to_json_pretty(&value)?;
    fs::write(path, format!("{fixtures_json}\n"))
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

fn optional_u64_value(value: Option<u64>) -> Value {
    match value {
        Some(v) => Value::Number(Number::U64(v)),
        None => Value::Null,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_to_string_handles_primitives() {
        assert_eq!(value_to_string(&Value::String("abc".into())), "abc");
        assert_eq!(value_to_string(&Value::Bool(true)), "true");
        assert_eq!(value_to_string(&Value::Null), "null");
    }

    #[test]
    fn metadata_from_arguments_collects_keys() {
        let mut args = BTreeMap::new();
        args.insert("metadata.memo".to_string(), "hello".to_string());
        args.insert("display_name".to_string(), "Test".to_string());
        let metadata = metadata_from_arguments(&args, &["display_name"]).expect("metadata");
        assert!(metadata.contains(&Name::from_str("memo").unwrap()));
        assert!(metadata.contains(&Name::from_str("display_name").unwrap()));
    }

    #[test]
    fn encoded_check_can_be_toggled() {
        let keypair = signing_keypair().expect("test keypair");
        let base = sample_fixture(None);
        let actual = base
            .clone()
            .into_fixture(&keypair, true, true)
            .expect("baseline fixture");
        let mut mismatched = actual.summary.payload_base64.clone();
        mismatched.push('A');

        assert!(
            sample_fixture(Some(mismatched.clone()))
                .into_fixture(&keypair, true, true)
                .is_err()
        );
        assert!(
            sample_fixture(Some(mismatched))
                .into_fixture(&keypair, false, true)
                .is_ok()
        );
    }

    #[test]
    fn normalize_authority_hint_accepts_encoded_address_with_domain_suffix() {
        let keypair = signing_keypair().expect("test keypair");
        let domain: DomainId = "wonderland".parse().expect("valid domain");
        let account = AccountId::new(domain.clone(), keypair.public_key().clone());
        let ih58 = account.to_string();
        let with_domain = format!("{ih58}@{domain}");
        let normalized = normalize_authority_hint(&with_domain);
        assert_eq!(normalized, ih58);
    }

    #[test]
    fn encoded_fixture_validates_hints() {
        let keypair = signing_keypair().expect("test keypair");
        let generated = sample_fixture(None)
            .into_fixture(&keypair, true, true)
            .expect("fixture from payload");
        let payload_bytes = BASE64
            .decode(generated.summary.payload_base64.as_bytes())
            .expect("payload base64 decodes");
        let mut cursor = payload_bytes.as_slice();
        let decoded = TransactionPayload::decode(&mut cursor).expect("payload decodes");
        let normalized_authority = decoded.authority.to_string();

        let mut raw = RawFixture {
            name: generated.name.clone(),
            payload: None,
            payload_json: None,
            payload_base64: Some(generated.summary.payload_base64.clone()),
            signed_base64: Some(generated.summary.signed_base64.clone()),
            chain_hint: Some(generated.summary.chain.clone()),
            authority_hint: Some(normalized_authority),
            creation_time_ms_hint: Some(generated.summary.creation_time_ms),
            ttl_ms_hint: generated.summary.ttl_ms,
            nonce_hint: generated.summary.nonce,
            payload_hash_hint: Some(generated.summary.payload_hash_hex.clone()),
            signed_hash_hint: Some(generated.summary.signed_hash_hex.clone()),
            encoded: None,
        };

        raw.clone()
            .into_fixture(&keypair, true, true)
            .expect("encoded fixture should validate");

        raw.chain_hint = Some("00000003".to_string());
        assert!(
            raw.clone().into_fixture(&keypair, true, true).is_err(),
            "chain hint mismatch should fail"
        );

        raw.chain_hint = Some(generated.summary.chain.clone());
        raw.creation_time_ms_hint = Some(generated.summary.creation_time_ms + 1);
        assert!(
            raw.clone().into_fixture(&keypair, true, true).is_err(),
            "creation_time_ms mismatch should fail"
        );

        raw.creation_time_ms_hint = Some(generated.summary.creation_time_ms);
        raw.payload_hash_hint = Some("bad-hash".to_string());
        assert!(
            raw.into_fixture(&keypair, true, true).is_err(),
            "payload_hash mismatch should fail"
        );
    }

    #[test]
    fn opaque_fixture_fallback_uses_hints() {
        let keypair = signing_keypair().expect("test keypair");
        let payload_bytes = vec![0x01, 0x02, 0x03];
        let payload_base64 = BASE64.encode(&payload_bytes);
        let payload_hash = format!("{}", Hash::new(&payload_bytes));
        let signature = Signature::new(keypair.private_key(), Hash::new(&payload_bytes).as_ref());
        let signed_bytes = encode_signed_envelope(signature.payload(), &payload_bytes, None, None);
        let signed_base64 = BASE64.encode(&signed_bytes);
        let signed_hash = format!("{}", Hash::new(&signed_bytes));

        let raw = RawFixture {
            name: "opaque".to_string(),
            payload: None,
            payload_json: None,
            payload_base64: Some(payload_base64.clone()),
            signed_base64: Some(signed_base64.clone()),
            chain_hint: Some("00000002".to_string()),
            authority_hint: Some("alice@wonderland".to_string()),
            creation_time_ms_hint: Some(1_735_000_000_000),
            ttl_ms_hint: None,
            nonce_hint: None,
            payload_hash_hint: Some(payload_hash.clone()),
            signed_hash_hint: Some(signed_hash.clone()),
            encoded: Some(payload_base64.clone()),
        };

        let fixture = raw
            .into_fixture(&keypair, true, true)
            .expect("opaque fixture should validate");
        assert_eq!(fixture.summary.payload_base64, payload_base64);
        assert_eq!(fixture.summary.signed_base64, signed_base64);
        assert_eq!(fixture.summary.payload_hash_hex, payload_hash);
        assert_eq!(fixture.summary.signed_hash_hex, signed_hash);
        assert_eq!(fixture.summary.chain, "00000002");
        assert_eq!(fixture.summary.authority, "alice@wonderland");
    }

    #[test]
    fn manifest_entry_includes_creation_time_ms() {
        let keypair = signing_keypair().expect("test keypair");
        let generated = sample_fixture(None)
            .into_fixture(&keypair, true, true)
            .expect("fixture from payload");
        let entry = manifest_entry(&generated);
        let obj = entry.as_object().expect("manifest entry must be object");
        let creation_time_ms = obj
            .get("creation_time_ms")
            .and_then(|value| value.as_u64())
            .expect("creation_time_ms must be present");
        assert_eq!(creation_time_ms, generated.summary.creation_time_ms);
    }

    #[test]
    fn fixtures_json_rewrites_encoded_payloads() {
        let keypair = signing_keypair().expect("test keypair");
        let raw = sample_fixture(None);
        let fixture = raw
            .clone()
            .into_fixture(&keypair, true, true)
            .expect("fixture");

        let value =
            build_fixtures_json(&[raw], std::slice::from_ref(&fixture)).expect("fixtures json");
        let entries = value.as_array().expect("fixtures json must be array");
        let obj = entries[0]
            .as_object()
            .expect("fixture entry must be object");
        let encoded = obj
            .get("encoded")
            .and_then(|val| val.as_str())
            .expect("encoded must be string");
        assert_eq!(encoded, fixture.summary.payload_base64);
        let payload_base64 = obj
            .get("payload_base64")
            .and_then(|val| val.as_str())
            .expect("payload_base64 must be string");
        assert_eq!(payload_base64, fixture.summary.payload_base64);
        let nonce = obj.get("nonce").expect("nonce must be present");
        assert!(matches!(nonce, Value::Null));
    }

    #[test]
    fn fixtures_json_injects_wire_instruction_payloads() {
        let keypair = signing_keypair().expect("test keypair");
        let mut args = BTreeMap::new();
        args.insert("action".to_string(), "RegisterDomain".to_string());
        args.insert("domain".to_string(), "wonderland".to_string());
        let raw_instruction = RawInstruction {
            kind: "Register".to_string(),
            arguments: args,
        };
        let raw_payload = RawPayload {
            chain: "00000002".to_string(),
            authority: "alice@wonderland".to_string(),
            creation_time_ms: 1_735_000_000_123,
            executable: RawExecutable::Instructions(vec![raw_instruction.clone()]),
            ttl_ms: Some(1_000),
            nonce: None,
            metadata: Vec::new(),
        };

        let mut args_value = Map::new();
        args_value.insert("action".into(), Value::String("RegisterDomain".into()));
        args_value.insert("domain".into(), Value::String("wonderland".into()));
        let mut instruction_value = Map::new();
        instruction_value.insert("kind".into(), Value::String("Register".into()));
        instruction_value.insert("arguments".into(), Value::Object(args_value));
        let mut executable_value = Map::new();
        executable_value.insert("Instructions".into(), Value::Array(vec![Value::Object(instruction_value)]));
        let mut payload_value = Map::new();
        payload_value.insert("chain".into(), Value::String("00000002".into()));
        payload_value.insert("authority".into(), Value::String("alice@wonderland".into()));
        payload_value.insert(
            "creation_time_ms".into(),
            Value::Number(Number::U64(1_735_000_000_123)),
        );
        payload_value.insert("executable".into(), Value::Object(executable_value));
        payload_value.insert("time_to_live_ms".into(), Value::Number(Number::U64(1_000)));
        payload_value.insert("nonce".into(), Value::Null);
        payload_value.insert("metadata".into(), Value::Object(Map::new()));

        let raw = RawFixture {
            name: "wire-inject".to_string(),
            payload: Some(raw_payload),
            payload_json: Some(Value::Object(payload_value)),
            payload_base64: None,
            signed_base64: None,
            chain_hint: None,
            authority_hint: None,
            creation_time_ms_hint: None,
            ttl_ms_hint: None,
            nonce_hint: None,
            payload_hash_hint: None,
            signed_hash_hint: None,
            encoded: None,
        };

        let fixture = raw
            .clone()
            .into_fixture(&keypair, true, true)
            .expect("fixture");
        let expected_wire = wire_payloads_from_encoded(&fixture.encoded)
            .expect("wire payloads")
            .expect("instruction payloads");

        let value =
            build_fixtures_json(&[raw], std::slice::from_ref(&fixture)).expect("fixtures json");
        let entries = value.as_array().expect("fixtures json must be array");
        let payload = entries[0]
            .as_object()
            .and_then(|obj| obj.get("payload"))
            .expect("payload must be present");
        let instructions = payload
            .get("executable")
            .and_then(|exec| exec.get("Instructions"))
            .and_then(Value::as_array)
            .expect("instructions must be present");
        let instruction = instructions[0]
            .as_object()
            .expect("instruction must be object");
        let wire_name = instruction
            .get("wire_name")
            .and_then(Value::as_str)
            .expect("wire_name must be present");
        let payload_base64 = instruction
            .get("payload_base64")
            .and_then(Value::as_str)
            .expect("payload_base64 must be present");
        assert_eq!(wire_name, expected_wire[0].wire_name);
        assert_eq!(payload_base64, expected_wire[0].payload_base64);
    }

    #[test]
    fn parse_asset_id_supports_alias_owner() {
        let id = parse_asset_id("rose#wonderland#alice@wonderland").expect("parse alias asset id");
        assert_eq!(format!("{}", id.definition), "rose#wonderland");
        assert_eq!(format!("{}", id.account.domain()), "wonderland");
    }

    #[test]
    fn parse_asset_id_accepts_canonical_string() {
        let domain: DomainId = address::default_domain_name()
            .as_ref()
            .parse()
            .expect("domain");
        let kp = KeyPair::from_seed(vec![1; 32], Algorithm::Ed25519);
        let account = AccountId::of(domain.clone(), kp.public_key().clone());
        let definition: AssetDefinitionId = format!("rose#{domain}").parse().expect("definition");
        let canonical_id = AssetId::new(definition.clone(), account.clone());
        let canonical = canonical_id.to_string();
        let parsed = parse_asset_id(&canonical).expect("canonical asset id");
        assert_eq!(parsed, canonical_id);
    }

    #[test]
    fn build_instruction_maps_deprecate_verifying_key_to_update() {
        let raw = sample_verifying_key_instruction("DeprecateVerifyingKey");
        let instruction = build_instruction(&raw).expect("build verifying key instruction");
        assert_eq!(
            iroha_data_model::isi::Instruction::id(&*instruction),
            std::any::type_name::<verifying_keys::UpdateVerifyingKey>(),
        );
    }

    #[test]
    fn verifying_key_record_maps_deprecation_to_withdraw() {
        let mut raw = sample_verifying_key_instruction("RegisterVerifyingKey");
        raw.arguments
            .insert("record.deprecation_height".to_string(), "42".to_string());
        let record = build_verifying_key_record(&raw, "halo2-ipa-pasta")
            .expect("build verifying key record");
        assert_eq!(record.withdraw_height, Some(42));
    }

    #[test]
    fn verifying_key_record_rejects_mismatched_withdraw_heights() {
        let mut raw = sample_verifying_key_instruction("RegisterVerifyingKey");
        raw.arguments
            .insert("record.deprecation_height".to_string(), "7".to_string());
        raw.arguments
            .insert("record.withdraw_height".to_string(), "8".to_string());
        let err = build_verifying_key_record(&raw, "halo2-ipa-pasta")
            .expect_err("mismatched heights must error");
        assert!(
            err.to_string().contains("record.deprecation_height"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn parse_confidential_status_treats_deprecated_as_withdrawn() {
        assert_eq!(
            parse_confidential_status("Deprecated").expect("parse deprecated status"),
            ConfidentialStatus::Withdrawn
        );
    }

    fn sample_verifying_key_instruction(action: &str) -> RawInstruction {
        let mut args = BTreeMap::new();
        args.insert("action".to_string(), action.to_string());
        args.insert("backend".to_string(), "halo2-ipa-pasta".to_string());
        args.insert("name".to_string(), "example".to_string());
        args.insert("record.version".to_string(), "1".to_string());
        args.insert(
            "record.circuit_id".to_string(),
            "example-circuit".to_string(),
        );
        args.insert(
            "record.backend_tag".to_string(),
            "halo2-ipa-pasta".to_string(),
        );
        args.insert("record.curve".to_string(), "pasta".to_string());
        args.insert(
            "record.public_inputs_schema_hash_hex".to_string(),
            "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
        );
        args.insert(
            "record.commitment_hex".to_string(),
            "1111111111111111111111111111111111111111111111111111111111111111".to_string(),
        );
        args.insert("record.vk_len".to_string(), "32".to_string());
        args.insert("record.gas_schedule_id".to_string(), "default".to_string());
        RawInstruction {
            kind: "Custom".to_string(),
            arguments: args,
        }
    }

    fn sample_fixture(encoded: Option<String>) -> RawFixture {
        RawFixture {
            name: "sample".to_string(),
            payload: Some(RawPayload {
                chain: "00000002".to_string(),
                authority: "alice@wonderland".to_string(),
                creation_time_ms: 1_735_000_000_000,
                executable: RawExecutable::Ivm(vec![1, 2, 3, 4]),
                ttl_ms: Some(1_000),
                nonce: None,
                metadata: Vec::new(),
            }),
            payload_json: None,
            payload_base64: None,
            signed_base64: None,
            chain_hint: None,
            authority_hint: None,
            creation_time_ms_hint: None,
            ttl_ms_hint: None,
            nonce_hint: None,
            payload_hash_hint: None,
            signed_hash_hint: None,
            encoded,
        }
    }
}
fn build_repo_initiate_instruction(raw: &RawInstruction) -> Result<InstructionBox> {
    ensure_kind(raw, "Custom")?;
    let agreement_id: RepoAgreementId = get_arg(raw, "agreement_id")?
        .parse()
        .context("invalid repo agreement id")?;
    let initiator = parse_account_id(get_arg(raw, "initiator")?)?;
    let counterparty = parse_account_id(get_arg(raw, "counterparty")?)?;
    let custodian = parse_optional_account(raw, "custodian")?;
    let cash_asset: AssetDefinitionId = get_arg(raw, "cash_asset")?
        .parse()
        .context("invalid cash asset definition id")?;
    let cash_quantity = parse_numeric(get_arg(raw, "cash_quantity")?)?;
    let cash_leg = RepoCashLeg {
        asset_definition_id: cash_asset,
        quantity: cash_quantity,
    };
    let collateral_asset: AssetDefinitionId = get_arg(raw, "collateral_asset")?
        .parse()
        .context("invalid collateral asset definition id")?;
    let collateral_quantity = parse_numeric(get_arg(raw, "collateral_quantity")?)?;
    let collateral_metadata = metadata_from_prefixed_arguments(raw, "collateral")?;
    let collateral_leg = RepoCollateralLeg {
        asset_definition_id: collateral_asset,
        quantity: collateral_quantity,
        metadata: collateral_metadata,
    };
    let rate_bps = parse_u16_field(get_arg(raw, "rate_bps")?, "rate_bps")?;
    let maturity_timestamp_ms = parse_u64_field(
        get_arg(raw, "maturity_timestamp_ms")?,
        "maturity_timestamp_ms",
    )?;
    let governance = RepoGovernance::with_defaults(
        parse_u16_field(get_arg(raw, "haircut_bps")?, "haircut_bps")?,
        parse_u64_field(
            get_arg(raw, "margin_frequency_secs")?,
            "margin_frequency_secs",
        )?,
    );
    Ok(RepoIsi::new(
        agreement_id,
        initiator,
        counterparty,
        custodian,
        cash_leg,
        collateral_leg,
        rate_bps,
        maturity_timestamp_ms,
        governance,
    )
    .into())
}

fn build_repo_reverse_instruction(raw: &RawInstruction) -> Result<InstructionBox> {
    ensure_kind(raw, "Custom")?;
    let agreement_id: RepoAgreementId = get_arg(raw, "agreement_id")?
        .parse()
        .context("invalid repo agreement id")?;
    let initiator = parse_account_id(get_arg(raw, "initiator")?)?;
    let counterparty = parse_account_id(get_arg(raw, "counterparty")?)?;
    let cash_asset: AssetDefinitionId = get_arg(raw, "cash_asset")?
        .parse()
        .context("invalid cash asset definition id")?;
    let cash_quantity = parse_numeric(get_arg(raw, "cash_quantity")?)?;
    let cash_leg = RepoCashLeg {
        asset_definition_id: cash_asset,
        quantity: cash_quantity,
    };
    let collateral_asset: AssetDefinitionId = get_arg(raw, "collateral_asset")?
        .parse()
        .context("invalid collateral asset definition id")?;
    let collateral_quantity = parse_numeric(get_arg(raw, "collateral_quantity")?)?;
    let collateral_metadata = metadata_from_prefixed_arguments(raw, "collateral")?;
    let collateral_leg = RepoCollateralLeg {
        asset_definition_id: collateral_asset,
        quantity: collateral_quantity,
        metadata: collateral_metadata,
    };
    let settlement_timestamp_ms = parse_u64_field(
        get_arg(raw, "settlement_timestamp_ms")?,
        "settlement_timestamp_ms",
    )?;
    Ok(ReverseRepoIsi::new(
        agreement_id,
        initiator,
        counterparty,
        cash_leg,
        collateral_leg,
        settlement_timestamp_ms,
    )
    .into())
}

enum SettlementInstructionKind {
    Dvp,
    Pvp,
}

fn build_settlement_instruction(
    raw: &RawInstruction,
    kind: SettlementInstructionKind,
) -> Result<InstructionBox> {
    ensure_kind(raw, "Custom")?;
    let settlement_id: SettlementId = get_arg(raw, "settlement_id")?
        .parse()
        .context("invalid settlement id")?;
    let order = parse_settlement_order(get_arg(raw, "order")?)?;
    let atomicity = parse_settlement_atomicity(get_arg(raw, "atomicity")?)?;
    let plan = SettlementPlan::new(order, atomicity);
    match kind {
        SettlementInstructionKind::Dvp => {
            let delivery_leg = build_settlement_leg(raw, "delivery")?;
            let payment_leg = build_settlement_leg(raw, "payment")?;
            Ok(DvpIsi::new(settlement_id, delivery_leg, payment_leg, plan).into())
        }
        SettlementInstructionKind::Pvp => {
            let primary_leg = build_settlement_leg(raw, "primary")?;
            let counter_leg = build_settlement_leg(raw, "counter")?;
            Ok(PvpIsi::new(settlement_id, primary_leg, counter_leg, plan).into())
        }
    }
}

fn build_settlement_leg(raw: &RawInstruction, prefix: &str) -> Result<SettlementLeg> {
    let asset_key = format!("{prefix}_asset");
    let asset_definition_id: AssetDefinitionId = get_arg(raw, &asset_key)?
        .parse()
        .with_context(|| format!("invalid asset definition for {prefix} leg"))?;
    let quantity = parse_numeric(get_arg(raw, &format!("{prefix}_quantity"))?)
        .with_context(|| format!("invalid quantity for {prefix} leg"))?;
    let from = parse_account_id(get_arg(raw, &format!("{prefix}_from"))?)?;
    let to = parse_account_id(get_arg(raw, &format!("{prefix}_to"))?)?;
    let metadata = metadata_from_prefixed_arguments(raw, prefix)?;
    Ok(SettlementLeg {
        asset_definition_id,
        quantity,
        from,
        to,
        metadata,
    })
}

fn metadata_from_prefixed_arguments(raw: &RawInstruction, prefix: &str) -> Result<Metadata> {
    let mut metadata = Metadata::default();
    let needle = format!("{prefix}.metadata.");
    for (key, value) in &raw.arguments {
        if let Some(field) = key.strip_prefix(&needle) {
            metadata.insert(field.parse()?, Json::new(value.clone()));
        }
    }
    Ok(metadata)
}

fn parse_optional_account(raw: &RawInstruction, key: &str) -> Result<Option<AccountId>> {
    match raw.arguments.get(key) {
        None => Ok(None),
        Some(value) if value.trim().is_empty() => Ok(None),
        Some(value) => Ok(Some(parse_account_id(value)?)),
    }
}

fn parse_settlement_order(value: &str) -> Result<SettlementExecutionOrder> {
    match value {
        "DeliveryThenPayment" => Ok(SettlementExecutionOrder::DeliveryThenPayment),
        "PaymentThenDelivery" => Ok(SettlementExecutionOrder::PaymentThenDelivery),
        other => bail!("unsupported settlement order '{other}'"),
    }
}

fn parse_settlement_atomicity(value: &str) -> Result<SettlementAtomicity> {
    match value {
        "AllOrNothing" => Ok(SettlementAtomicity::AllOrNothing),
        "CommitFirstLeg" => Ok(SettlementAtomicity::CommitFirstLeg),
        "CommitSecondLeg" => Ok(SettlementAtomicity::CommitSecondLeg),
        other => bail!("unsupported settlement atomicity '{other}'"),
    }
}
