//! CLI helper that regenerates Norito fixtures and manifests for tests and docs.
use std::{
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
use iroha_data_model::{
    account::{AccountAddressSource, AccountId, address},
    domain::{Domain, DomainId},
    isi::{
        Instruction, InstructionBox, Register, decode_instruction_from_pair,
        frame_instruction_payload,
    },
    metadata::Metadata,
    prelude::*,
    transaction::signed::{TransactionPayload, TransactionSignature},
    transaction::{Executable, IvmBytecode, SignedTransaction, TransactionBuilder},
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
use iroha_primitives::json::Json;
use norito::codec::{Decode, Encode};
use norito::json::{self, Map, Number, Value};
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
    let manifest_value = build_manifest(&fixtures);
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

#[derive(Clone, Debug)]
struct RawInstruction {
    wire_name: String,
    payload_base64: String,
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
            let encoded_struct = payload_value.encode();
            let encoded = normalize_payload_authority_bytes(&encoded_struct, &payload.authority)?;
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
            let signed_struct = signed.encode();
            let signed_bytes = reencode_signed_with_payload(keypair, &encoded, &signed_struct)?;
            let signed_base64 = BASE64.encode(&signed_bytes);
            let payload_hash_hex = format!("{}", Hash::new(&encoded));
            let signed_hash_hex = format!("{}", Hash::new(&signed_bytes));

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
        let encoded_struct = payload_value.encode();
        let authority_literal = payload_value.authority.to_string();
        let encoded = normalize_payload_authority_bytes(&encoded_struct, &authority_literal)?;
        let payload_base64 = BASE64.encode(&encoded);
        if check_encoded && payload_base64_input != payload_base64 {
            bail!(
                "encoded payload mismatch for '{}': expected {}, got {}",
                self.name,
                payload_base64_input,
                payload_base64
            );
        }
        let signed_struct = signed.encode();
        let signed_bytes = reencode_signed_with_payload(keypair, &encoded, &signed_struct)?;
        let signed_base64 = BASE64.encode(&signed_bytes);
        let payload_hash_hex = format!("{}", Hash::new(&encoded));
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

fn reencode_signed_with_payload(
    keypair: &KeyPair,
    payload_bytes: &[u8],
    signed_bytes: &[u8],
) -> Result<Vec<u8>> {
    let fields = decode_signed_envelope_fields(signed_bytes)?;
    let payload_hash = Hash::new(payload_bytes);
    let signature = Signature::new(keypair.private_key(), payload_hash.as_ref());
    Ok(encode_signed_envelope(
        signature.payload(),
        payload_bytes,
        Some(&fields.attachments_field),
        Some(&fields.multisig_field),
    ))
}

fn normalize_payload_authority_bytes(payload_bytes: &[u8], authority: &str) -> Result<Vec<u8>> {
    let _guard = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let mut cursor = 0usize;
    let chain_field = read_len_prefixed_field(payload_bytes, &mut cursor, "chain_id")?;
    let _authority_field = read_len_prefixed_field(payload_bytes, &mut cursor, "authority")?;
    let authority_field = authority.to_string().encode();
    let rest = &payload_bytes[cursor..];

    let mut out = Vec::with_capacity(
        chain_field.len()
            .saturating_add(authority_field.len())
            .saturating_add(rest.len())
            .saturating_add(64),
    );
    norito::core::write_len_to_vec(&mut out, chain_field.len() as u64);
    out.extend_from_slice(&chain_field);
    norito::core::write_len_to_vec(&mut out, authority_field.len() as u64);
    out.extend_from_slice(&authority_field);
    out.extend_from_slice(rest);
    Ok(out)
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
    let wire_name = obj
        .get("wire_name")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| anyhow::anyhow!("instruction wire payload requires wire_name"))?;
    let payload_base64 = obj
        .get("payload_base64")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| anyhow::anyhow!("instruction wire payload requires payload_base64"))?;
    if obj.contains_key("kind") || obj.contains_key("arguments") {
        bail!("legacy instruction fields are not supported; use wire_name/payload_base64");
    }
    Ok(RawInstruction {
        wire_name,
        payload_base64,
    })
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
    let payload_bytes = BASE64
        .decode(raw.payload_base64.as_bytes())
        .context("invalid instruction payload_base64")?;
    if payload_bytes.is_empty() {
        bail!("instruction payload_base64 must not decode to empty bytes");
    }
    decode_instruction_from_pair(&raw.wire_name, &payload_bytes)
        .map_err(|err| anyhow::anyhow!(err.to_string()))
        .with_context(|| format!("failed to decode wire instruction '{}'", raw.wire_name))
}

fn authority_chain_discriminant(authority: &str) -> Option<u16> {
    let parsed = AccountId::parse_encoded(authority.trim()).ok()?;
    match parsed.source() {
        AccountAddressSource::Encoded(address::AccountAddressFormat::IH58 { network_prefix }) => {
            Some(network_prefix)
        }
        AccountAddressSource::Encoded(address::AccountAddressFormat::Compressed) => None,
    }
}

fn normalize_authority_hint(authority: &str) -> String {
    let trimmed = authority.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    AccountId::parse_encoded(trimmed)
        .map(|parsed| parsed.into_account_id().to_string())
        .unwrap_or_else(|_| trimmed.to_string())
}

fn parse_account_id(value: &str) -> Result<AccountId> {
    let trimmed = value.trim();
    let (address, _) = AccountAddress::parse_encoded(trimmed, None)
        .map_err(|err| anyhow::anyhow!(err.to_string()))
        .with_context(|| format!("invalid encoded account id '{value}'"))?;
    let canonical = address
        .to_ih58(address::chain_discriminant())
        .map_err(|err| anyhow::anyhow!(err.to_string()))
        .with_context(|| format!("failed to canonicalize account id '{value}'"))?;
    AccountId::parse_encoded(&canonical)
        .map(|parsed| parsed.into_account_id())
        .with_context(|| format!("invalid encoded account id '{value}'"))
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
        if obj.contains_key("kind") || obj.contains_key("arguments") {
            bail!("instruction entries must not include legacy kind/arguments fields");
        }
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

fn optional_u32_value(value: Option<u32>) -> Value {
    match value {
        Some(v) => Value::Number(Number::U64(v as u64)),
        None => Value::Null,
    }
}

fn value_to_string(value: &Value) -> String {
    match value {
        Value::Null => "null".to_owned(),
        Value::Bool(true) => "true".to_owned(),
        Value::Bool(false) => "false".to_owned(),
        Value::Number(number) => match number {
            Number::I64(v) => v.to_string(),
            Number::U64(v) => v.to_string(),
            Number::F64(v) => v.to_string(),
        },
        Value::String(s) => s.clone(),
        Value::Array(_) | Value::Object(_) => json::to_json(value).unwrap_or_else(|_| {
            // Fall back to a debug representation when serialization fails.
            format!("{value:?}")
        }),
    }
}

fn expect_string<'a>(obj: &'a Map, key: &str) -> Result<&'a str> {
    obj.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("missing '{key}' string"))
}

fn expect_u64(obj: &Map, key: &str) -> Result<u64> {
    obj.get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| anyhow::anyhow!("missing '{key}' integer"))
}

fn parse_optional_u64(obj: &Map, key: &str) -> Result<Option<u64>> {
    match obj.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(number)) => number
            .as_u64()
            .ok_or_else(|| anyhow::anyhow!("'{key}' must be an integer or null"))
            .map(Some),
        Some(other) => bail!(
            "'{key}' must be an integer or null, got {}",
            value_to_string(other)
        ),
    }
}

fn parse_optional_u32(obj: &Map, key: &str) -> Result<Option<u32>> {
    match obj.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(number)) => {
            let value = number
                .as_u64()
                .ok_or_else(|| anyhow::anyhow!("'{key}' must be an integer or null"))?;
            let value_u32 = u32::try_from(value)
                .with_context(|| format!("'{key}' must fit in u32 (got {value})"))?;
            Ok(Some(value_u32))
        }
        Some(other) => bail!(
            "'{key}' must be an integer or null, got {}",
            value_to_string(other)
        ),
    }
}

fn build_manifest(fixtures: &[Fixture]) -> Value {
    let entries = fixtures.iter().map(manifest_entry).collect::<Vec<_>>();
    let mut obj = Map::new();
    obj.insert("fixtures".to_owned(), Value::Array(entries));
    Value::Object(obj)
}

fn manifest_entry(fixture: &Fixture) -> Value {
    let mut obj = Map::new();
    obj.insert("name".to_owned(), Value::String(fixture.name.clone()));
    obj.insert(
        "authority".to_owned(),
        Value::String(fixture.summary.authority.clone()),
    );
    obj.insert(
        "chain".to_owned(),
        Value::String(fixture.summary.chain.clone()),
    );
    obj.insert(
        "creation_time_ms".to_owned(),
        Value::Number(Number::U64(fixture.summary.creation_time_ms)),
    );
    obj.insert(
        "encoded_file".to_owned(),
        Value::String(format!("{}.norito", fixture.name)),
    );
    obj.insert(
        "encoded_len".to_owned(),
        Value::Number(Number::U64(fixture.encoded.len() as u64)),
    );
    obj.insert(
        "signed_len".to_owned(),
        Value::Number(Number::U64(fixture.signed_bytes.len() as u64)),
    );
    obj.insert(
        "payload_base64".to_owned(),
        Value::String(fixture.summary.payload_base64.clone()),
    );
    obj.insert(
        "payload_hash".to_owned(),
        Value::String(fixture.summary.payload_hash_hex.clone()),
    );
    obj.insert(
        "signed_base64".to_owned(),
        Value::String(fixture.summary.signed_base64.clone()),
    );
    obj.insert(
        "signed_hash".to_owned(),
        Value::String(fixture.summary.signed_hash_hex.clone()),
    );
    obj.insert(
        "nonce".to_owned(),
        optional_u32_value(fixture.summary.nonce),
    );
    obj.insert(
        "time_to_live_ms".to_owned(),
        optional_u64_value(fixture.summary.ttl_ms),
    );
    Value::Object(obj)
}

fn wire_payloads_from_encoded(encoded: &[u8]) -> Result<Vec<WireInstructionPayload>> {
    let mut cursor = encoded;
    let payload = TransactionPayload::decode(&mut cursor).context("decode TransactionPayload")?;
    if !cursor.is_empty() {
        bail!("payload contains trailing bytes");
    }
    let Executable::Instructions(instructions) = &payload.instructions else {
        return Ok(Vec::new());
    };

    let registry = iroha_data_model::instruction_registry::default();
    let mut out = Vec::with_capacity(instructions.len());
    for instruction in instructions.iter() {
        let type_name = Instruction::id(&**instruction);
        let wire_name = registry.wire_id(type_name).unwrap_or(type_name);
        let payload = Instruction::dyn_encode(&**instruction);
        let framed = frame_instruction_payload(type_name, &payload)
            .map_err(|err| anyhow::anyhow!(err.to_string()))?;
        out.push(WireInstructionPayload {
            wire_name: wire_name.to_owned(),
            payload_base64: BASE64.encode(framed),
        });
    }
    Ok(out)
}

fn wire_payloads_from_raw_payload(raw: &RawPayload) -> Result<Vec<WireInstructionPayload>> {
    let RawExecutable::Instructions(instructions) = &raw.executable else {
        return Ok(Vec::new());
    };

    let registry = iroha_data_model::instruction_registry::default();
    let mut out = Vec::with_capacity(instructions.len());
    for raw_instruction in instructions {
        let instruction = build_instruction(raw_instruction)?;
        let type_name = Instruction::id(&*instruction);
        let wire_name = registry.wire_id(type_name).unwrap_or(type_name);
        let payload = Instruction::dyn_encode(&*instruction);
        let framed = frame_instruction_payload(type_name, &payload)
            .map_err(|err| anyhow::anyhow!(err.to_string()))?;
        out.push(WireInstructionPayload {
            wire_name: wire_name.to_owned(),
            payload_base64: BASE64.encode(framed),
        });
    }
    Ok(out)
}

fn build_fixtures_json(raw_fixtures: &[RawFixture], fixtures: &[Fixture]) -> Result<Value> {
    let fixtures_by_name: std::collections::BTreeMap<&str, &Fixture> = fixtures
        .iter()
        .map(|fixture| (fixture.name.as_str(), fixture))
        .collect();

    let mut out = Vec::with_capacity(raw_fixtures.len());
    for raw in raw_fixtures {
        let fixture = fixtures_by_name
            .get(raw.name.as_str())
            .copied()
            .ok_or_else(|| anyhow::anyhow!("fixture '{}' missing generated payload", raw.name))?;

        let mut entry = Map::new();
        entry.insert("name".to_owned(), Value::String(fixture.name.clone()));
        entry.insert(
            "chain".to_owned(),
            Value::String(fixture.summary.chain.clone()),
        );
        entry.insert(
            "authority".to_owned(),
            Value::String(fixture.summary.authority.clone()),
        );
        entry.insert(
            "creation_time_ms".to_owned(),
            Value::Number(Number::U64(fixture.summary.creation_time_ms)),
        );
        entry.insert(
            "time_to_live_ms".to_owned(),
            optional_u64_value(fixture.summary.ttl_ms),
        );
        entry.insert(
            "nonce".to_owned(),
            optional_u32_value(fixture.summary.nonce),
        );
        entry.insert(
            "payload_base64".to_owned(),
            Value::String(fixture.summary.payload_base64.clone()),
        );
        entry.insert(
            "signed_base64".to_owned(),
            Value::String(fixture.summary.signed_base64.clone()),
        );
        entry.insert(
            "payload_hash".to_owned(),
            Value::String(fixture.summary.payload_hash_hex.clone()),
        );
        entry.insert(
            "signed_hash".to_owned(),
            Value::String(fixture.summary.signed_hash_hex.clone()),
        );
        // Keep `encoded` for Android tests; it must equal `payload_base64`.
        entry.insert(
            "encoded".to_owned(),
            Value::String(fixture.summary.payload_base64.clone()),
        );

        if let Some(mut payload) = raw.payload_json.clone() {
            if payload_json_uses_instruction_list(&payload) {
                let wire_payloads = if let Some(raw_payload) = raw.payload.as_ref() {
                    wire_payloads_from_raw_payload(raw_payload).with_context(|| {
                        format!("failed to derive wire payloads for '{}'", fixture.name)
                    })?
                } else {
                    wire_payloads_from_encoded(&fixture.encoded).with_context(|| {
                        format!("failed to derive wire payloads for '{}'", fixture.name)
                    })?
                };
                if !wire_payloads.is_empty() {
                    apply_wire_payloads_to_payload_json(&mut payload, &wire_payloads)?;
                }
            }
            entry.insert("payload".to_owned(), payload);
        } else if let Some(payload) = raw.payload.as_ref() {
            let mut payload_obj = Map::new();
            payload_obj.insert("chain".to_owned(), Value::String(payload.chain.clone()));
            payload_obj.insert(
                "authority".to_owned(),
                Value::String(payload.authority.clone()),
            );
            payload_obj.insert(
                "creation_time_ms".to_owned(),
                Value::Number(Number::U64(payload.creation_time_ms)),
            );
            payload_obj.insert(
                "time_to_live_ms".to_owned(),
                optional_u64_value(payload.ttl_ms),
            );
            payload_obj.insert("nonce".to_owned(), optional_u32_value(payload.nonce));

            let mut executable_obj = Map::new();
            match &payload.executable {
                RawExecutable::Ivm(bytes) => {
                    executable_obj.insert("Ivm".to_owned(), Value::String(BASE64.encode(bytes)));
                }
                RawExecutable::Instructions(instructions) => {
                    let mut values = Vec::with_capacity(instructions.len());
                    for raw_instruction in instructions {
                        let mut inst_obj = Map::new();
                        inst_obj.insert(
                            "wire_name".to_owned(),
                            Value::String(raw_instruction.wire_name.clone()),
                        );
                        inst_obj.insert(
                            "payload_base64".to_owned(),
                            Value::String(raw_instruction.payload_base64.clone()),
                        );
                        values.push(Value::Object(inst_obj));
                    }
                    executable_obj.insert("Instructions".to_owned(), Value::Array(values));
                }
            }
            payload_obj.insert("executable".to_owned(), Value::Object(executable_obj));

            let mut metadata_obj = Map::new();
            for (key, value) in &payload.metadata {
                let parsed = json::parse_value(value.get())
                    .map_err(|err| anyhow::anyhow!(err.to_string()))?;
                metadata_obj.insert(key.to_string(), parsed);
            }
            payload_obj.insert("metadata".to_owned(), Value::Object(metadata_obj));

            entry.insert("payload".to_owned(), Value::Object(payload_obj));
        }

        out.push(Value::Object(entry));
    }

    Ok(Value::Array(out))
}

fn payload_json_uses_instruction_list(payload: &Value) -> bool {
    let Some(obj) = payload.as_object() else {
        return false;
    };
    let Some(executable) = obj.get("executable").and_then(Value::as_object) else {
        return false;
    };
    executable
        .get("Instructions")
        .and_then(Value::as_array)
        .is_some()
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
    fn normalize_authority_hint_accepts_encoded_account_literal() {
        let keypair = signing_keypair().expect("test keypair");
        let domain: DomainId = "wonderland".parse().expect("valid domain");
        let account = AccountId::new(domain.clone(), keypair.public_key().clone());
        let ih58 = account.to_string();
        let normalized = normalize_authority_hint(&ih58);
        assert_eq!(normalized, ih58);
    }

    #[test]
    fn encoded_fixture_validates_hints() {
        let keypair = signing_keypair().expect("test keypair");
        let generated = sample_fixture(None)
            .into_fixture(&keypair, true, true)
            .expect("fixture from payload");
        let normalized_authority = generated.summary.authority.clone();

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
        raw.clone()
            .into_fixture(&keypair, true, true)
            .expect("opaque payload path relies on provided chain hint");

        raw.chain_hint = Some(generated.summary.chain.clone());
        raw.creation_time_ms_hint = Some(generated.summary.creation_time_ms + 1);
        raw.clone()
            .into_fixture(&keypair, true, true)
            .expect("opaque payload path relies on provided creation_time_ms hint");

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
        let authority = sample_authority_literal();
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
            authority_hint: Some(authority.clone()),
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
        assert_eq!(fixture.summary.authority, authority);
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
        let authority = sample_authority_literal();
        let domain: DomainId = "wonderland".parse().expect("domain");
        let instruction: InstructionBox = Register::domain(Domain::new(domain)).into();
        let type_name = Instruction::id(&*instruction);
        let registry = iroha_data_model::instruction_registry::default();
        let wire_name = registry.wire_id(type_name).unwrap_or(type_name);
        let payload = Instruction::dyn_encode(&*instruction);
        let framed = frame_instruction_payload(type_name, &payload).expect("framed payload");
        let raw_instruction = RawInstruction {
            wire_name: wire_name.to_string(),
            payload_base64: BASE64.encode(framed),
        };
        let raw_payload = RawPayload {
            chain: "00000002".to_string(),
            authority: authority.clone(),
            creation_time_ms: 1_735_000_000_123,
            executable: RawExecutable::Instructions(vec![raw_instruction.clone()]),
            ttl_ms: Some(1_000),
            nonce: None,
            metadata: Vec::new(),
        };

        let mut instruction_value = Map::new();
        instruction_value.insert("wire_name".into(), Value::String("placeholder".into()));
        instruction_value.insert("payload_base64".into(), Value::String("AA==".into()));
        let mut executable_value = Map::new();
        executable_value.insert(
            "Instructions".into(),
            Value::Array(vec![Value::Object(instruction_value)]),
        );
        let mut payload_value = Map::new();
        payload_value.insert("chain".into(), Value::String("00000002".into()));
        payload_value.insert("authority".into(), Value::String(authority));
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
        let expected_wire = wire_payloads_from_raw_payload(
            raw.payload.as_ref().expect("raw payload must be present"),
        )
        .expect("wire payloads");

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
    fn parse_instruction_requires_wire_payload() {
        let instruction_value = Map::new();
        let err = parse_instruction(&Value::Object(instruction_value))
            .expect_err("wire payload should be required");
        assert!(
            err.to_string().contains("wire payload requires wire_name"),
            "unexpected error: {err}"
        );
    }
    #[test]
    fn build_instruction_accepts_wire_payload() {
        let domain: DomainId = "wonderland".parse().expect("domain");
        let instruction: InstructionBox = Register::domain(Domain::new(domain)).into();
        let type_name = Instruction::id(&*instruction);
        let registry = iroha_data_model::instruction_registry::default();
        let wire_name = registry.wire_id(type_name).unwrap_or(type_name);
        let payload = Instruction::dyn_encode(&*instruction);
        let framed = frame_instruction_payload(type_name, &payload).expect("framed payload");
        let raw = RawInstruction {
            wire_name: wire_name.to_string(),
            payload_base64: BASE64.encode(framed),
        };
        let built = build_instruction(&raw).expect("wire payload decodes");
        assert_eq!(Instruction::id(&*built), type_name);
    }

    fn sample_fixture(encoded: Option<String>) -> RawFixture {
        RawFixture {
            name: "sample".to_string(),
            payload: Some(RawPayload {
                chain: "00000002".to_string(),
                authority: sample_authority_literal(),
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

    fn sample_authority_literal() -> String {
        let keypair = signing_keypair().expect("test keypair");
        let domain: DomainId = "wonderland".parse().expect("valid domain");
        AccountId::new(domain, keypair.public_key().clone()).to_string()
    }
}
