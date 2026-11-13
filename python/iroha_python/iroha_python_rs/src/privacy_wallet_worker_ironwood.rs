// Purpose-separated merchant operations. Only owner-only key-file paths enter the worker;
// no spending key, decrypted note opening or Merkle witness leaves native custody.
const MAX_IRONWOOD_READ_BYTES: usize = 16 * 1024 * 1024;

fn ironwood_profile_binding_v1() -> Result<[u8; 32], WorkerError> {
    let protocol = PrivacyRuntimeProtocolIdV1::IronwoodHalo2ActionsV1;
    let p = iroha_core_privacy::privacy_profiles::compiled_privacy_profile_v1(protocol)
        .map_err(|_| WorkerError::UnsupportedProtocol)?;
    compiled_profile_digest(
        protocol.canonical_label(),
        &CompiledProfileBinding {
            parameter_id: *p.parameter_id.as_bytes(),
            parameter_digest: *p.parameter_digest.as_bytes(),
            verifier_digest: *p.verifier_digest.as_bytes(),
            statement_schema_digest: *p.statement_schema_digest.as_bytes(),
            engine_manifest_digest: *p.engine_manifest_digest.as_bytes(),
        },
    )
}
fn ironwood_key_file(path: &str) -> Result<Zeroizing<[u8; 32]>, WorkerError> {
    let material = read_credential_file(Path::new(path))?;
    if material.len() != 32 {
        return Err(WorkerError::InvalidExecutionBundle);
    }
    let mut key = Zeroizing::new([0; 32]);
    key.copy_from_slice(&material);
    Ok(key)
}
fn ironwood_address(payload: &[u8]) -> Result<Vec<u8>, WorkerError> {
    let mut cursor = Cursor::new(payload);
    let path = cursor.text(MAX_PATH_BYTES)?;
    let index = cursor.u32()?;
    let [scope] = cursor.array()?;
    cursor.finish()?;
    if scope > 1 {
        return Err(WorkerError::InvalidPayload);
    }
    let key = ironwood_key_file(&path)?;
    let incoming =
        iroha_core_privacy::privacy_engines::orchard::ironwood_wallet::incoming_viewing_key_v1(
            &key,
            scope == 1,
        )
        .map_err(|_| WorkerError::InvalidExecutionBundle)?;
    let address = iroha_core_privacy::privacy_engines::orchard::ironwood_wallet::recipient_from_incoming_viewing_key_v1(&incoming, index)
        .map_err(|_| WorkerError::InvalidExecutionBundle)?;
    let mut result = address.to_vec();
    result.extend_from_slice(&ironwood_profile_binding_v1()?);
    Ok(result)
}
// Scope-tagged incoming-only credential: scope byte followed by raw IVK64. Native file creation
// is exclusive and owner-only; neither spending nor viewing bytes enter the public IPC result.
fn ironwood_export_viewing_key(payload: &[u8]) -> Result<Vec<u8>, WorkerError> {
    let mut cursor = Cursor::new(payload);
    let spending_path = cursor.text(MAX_PATH_BYTES)?;
    let viewing_path = cursor.text(MAX_PATH_BYTES)?;
    let [scope] = cursor.array()?;
    cursor.finish()?;
    if scope > 1 || !Path::new(&viewing_path).is_absolute() {
        return Err(WorkerError::InvalidCredentialPath);
    }
    let spending = ironwood_key_file(&spending_path)?;
    let incoming =
        iroha_core_privacy::privacy_engines::orchard::ironwood_wallet::incoming_viewing_key_v1(
            &spending,
            scope == 1,
        )
        .map_err(|_| WorkerError::InvalidExecutionBundle)?;
    let mut credential = Zeroizing::new([0; 65]);
    credential[0] = scope;
    credential[1..].copy_from_slice(incoming.as_ref());
    #[cfg(not(unix))]
    return Err(WorkerError::CredentialFileInsecure);
    #[cfg(unix)]
    {
        write_ironwood_viewing_credential(Path::new(&viewing_path), |file| {
            file.write_all(credential.as_ref())
        })?;
        Ok(ironwood_profile_binding_v1()?.to_vec())
    }
}
// Publish only a completely written and synced owner-only temporary; failures leave the
// destination absent and drop/remove the temporary. Existing credentials are never replaced.
#[cfg(unix)]
fn write_ironwood_viewing_credential(
    path: &Path,
    write: impl FnOnce(&mut std::fs::File) -> std::io::Result<()>,
) -> Result<(), WorkerError> {
    let parent = path.parent().ok_or(WorkerError::InvalidCredentialPath)?;
    let mut temporary =
        tempfile::NamedTempFile::new_in(parent).map_err(|e| WorkerError::Io(e.kind()))?;
    validate_file_security(
        &temporary
            .as_file()
            .metadata()
            .map_err(|e| WorkerError::Io(e.kind()))?,
    )?;
    write(temporary.as_file_mut()).map_err(|e| WorkerError::Io(e.kind()))?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|e| WorkerError::Io(e.kind()))?;
    temporary
        .persist_noclobber(path)
        .map_err(|e| WorkerError::Io(e.error.kind()))?;
    Ok(())
}

fn ironwood_scan_with_viewing_key(payload: &[u8]) -> Result<Vec<u8>, WorkerError> {
    let mut cursor = Cursor::new(payload);
    let path = cursor.text(MAX_PATH_BYTES)?;
    let request = cursor.bytes_u32(MAX_FRAME_BYTES - 8192)?;
    cursor.finish()?;
    let material = read_credential_file(Path::new(&path))?;
    if material.len() != 65 || material[0] > 1 {
        return Err(WorkerError::InvalidExecutionBundle);
    }
    let incoming: &[u8; 64] = material[1..]
        .try_into()
        .map_err(|_| WorkerError::InvalidExecutionBundle)?;
    iroha_core_privacy::privacy_engines::orchard::ironwood_wallet::recipient_from_incoming_viewing_key_v1(incoming, 0)
        .map_err(|_| WorkerError::InvalidExecutionBundle)?;
    scan_ironwood_request_key(
        request,
        IronwoodScanKey::Incoming {
            incoming,
            internal: material[0] == 1,
        },
    )
}
enum IronwoodScanKey<'a> {
    Spending(&'a [u8; 32]),
    Incoming {
        incoming: &'a [u8; 64],
        internal: bool,
    },
}
fn ironwood_hex<const N: usize>(value: &norito::json::Value) -> Result<[u8; N], WorkerError> {
    let text = value.as_str().ok_or(WorkerError::InvalidPayload)?;
    if text.len() != 2 * N
        || !text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(WorkerError::InvalidPayload);
    }
    hex::decode(text)
        .map_err(|_| WorkerError::InvalidPayload)?
        .try_into()
        .map_err(|_| WorkerError::InvalidPayload)
}
fn ironwood_bytes(value: &norito::json::Value, max: usize) -> Result<Vec<u8>, WorkerError> {
    let text = value.as_str().ok_or(WorkerError::InvalidPayload)?;
    if text.is_empty()
        || text.len() > max * 2
        || text.len() % 2 != 0
        || !text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(WorkerError::InvalidPayload);
    }
    hex::decode(text).map_err(|_| WorkerError::InvalidPayload)
}
fn ironwood_finality_error(
    error: iroha_data_model::sumeragi_finality::FinalityReadError,
) -> WorkerError {
    use iroha_data_model::sumeragi_finality::FinalityReadError as E;
    match error {
        E::DecodeResource(_) | E::ScratchLimit(_) | E::Allocator(_) => {
            WorkerError::CapacityExceeded
        }
        _ => WorkerError::InvalidPayload,
    }
}

/// Receipt/discovery request is a closed public object. A null expected_receipt discovers notes;
/// a non-null expectation must identify exactly one original transaction/action and independently
/// fix recipient, atomic amount and all 512 memo bytes. No caller-supplied acceptance boolean.
/// The owner must independently select/persist the checkpoint and network/chain/profile/asset/pool
/// pins; they must never come from the payer or the untrusted proof supplier. Verification advances
/// that trust anchor and does not bootstrap authority for an arbitrary caller-supplied checkpoint.
fn ironwood_scan(payload: &[u8]) -> Result<Vec<u8>, WorkerError> {
    let mut cursor = Cursor::new(payload);
    let path = cursor.text(MAX_PATH_BYTES)?;
    let request = cursor.bytes_u32(MAX_FRAME_BYTES - 8192)?;
    cursor.finish()?;
    let key = ironwood_key_file(&path)?;
    scan_ironwood_request(request, &key)
}
fn scan_ironwood_request(request: &[u8], key: &[u8; 32]) -> Result<Vec<u8>, WorkerError> {
    scan_ironwood_request_key(request, IronwoodScanKey::Spending(key))
}
fn scan_ironwood_request_key(
    request: &[u8],
    key: IronwoodScanKey<'_>,
) -> Result<Vec<u8>, WorkerError> {
    use iroha_data_model::{
        asset::{AssetBalanceScope, AssetDefinitionId},
        block::decode_framed_signed_block,
        sumeragi_finality::{
            SumeragiFinalityCheckpoint, SumeragiFinalityProof, verify_checkpoint_page,
        },
    };
    let value = validate_canonical_json_bytes(
        request,
        MAX_FRAME_BYTES - 8192,
        WorkerError::InvalidPayload,
    )?;
    let object = value.as_object().ok_or(WorkerError::InvalidPayload)?;
    const FIELDS: &[&str] = &[
        "network_id_hex",
        "chain_id",
        "checkpoint_hex",
        "proofs_hex",
        "asset_definition_id",
        "pool_id_hex",
        "compiled_profile_digest_hex",
        "internal_scope",
        "expected_receipt",
    ];
    if object.len() != FIELDS.len() || !FIELDS.iter().all(|f| object.contains_key(*f)) {
        return Err(WorkerError::InvalidPayload);
    }
    let network_bytes = ironwood_hex::<32>(&object["network_id_hex"])?;
    // Hash::prehashed would normalize the marker bit; an independently supplied network pin
    // must instead be canonical and remain byte-for-byte identical.
    if network_bytes[31] & 1 == 0 {
        return Err(WorkerError::InvalidPayload);
    }
    let network = network_id_from_genesis_hash_bytes(network_bytes);
    let chain = object["chain_id"]
        .as_str()
        .filter(|s| !s.is_empty() && s.len() <= 1024 && !s.chars().any(char::is_control))
        .ok_or(WorkerError::InvalidPayload)?;
    let checkpoint_wire = ironwood_bytes(&object["checkpoint_hex"], MAX_IRONWOOD_READ_BYTES)?;
    let checkpoint = SumeragiFinalityCheckpoint::decode_canonical(&checkpoint_wire)
        .map_err(|_| WorkerError::InvalidPayload)?;
    if checkpoint.chain_id() != chain {
        return Err(WorkerError::WrongBinding);
    }
    let asset_text = object["asset_definition_id"]
        .as_str()
        .ok_or(WorkerError::InvalidPayload)?;
    let asset: AssetDefinitionId = asset_text
        .parse()
        .map_err(|_| WorkerError::InvalidPayload)?;
    if asset.to_string() != asset_text {
        return Err(WorkerError::InvalidPayload);
    }
    let pool =
        iroha_data_model::privacy::PrivacyPoolIdV1::new(ironwood_hex(&object["pool_id_hex"])?);
    if ironwood_hex::<32>(&object["compiled_profile_digest_hex"])? != ironwood_profile_binding_v1()?
    {
        return Err(WorkerError::WrongBinding);
    }
    let internal = object["internal_scope"]
        .as_bool()
        .ok_or(WorkerError::InvalidPayload)?;
    let derived;
    let incoming = match key {
        IronwoodScanKey::Spending(spending) => {
            derived = iroha_core_privacy::privacy_engines::orchard::ironwood_wallet::incoming_viewing_key_v1(spending, internal)
                .map_err(|_| WorkerError::InvalidExecutionBundle)?;
            &*derived
        }
        IronwoodScanKey::Incoming {
            incoming,
            internal: credential_scope,
        } => {
            if credential_scope != internal {
                return Err(WorkerError::WrongBinding);
            }
            incoming
        }
    };
    let encoded = object["proofs_hex"]
        .as_array()
        .filter(|a| !a.is_empty() && a.len() <= 4096)
        .ok_or(WorkerError::InvalidPayload)?;
    let mut total = 0usize;
    let mut proofs = Vec::with_capacity(encoded.len());
    for wire in encoded {
        let wire = ironwood_bytes(wire, MAX_IRONWOOD_READ_BYTES)?;
        total = total
            .checked_add(wire.len())
            .filter(|n| *n <= MAX_IRONWOOD_READ_BYTES)
            .ok_or(WorkerError::FrameTooLarge)?;
        let proof: SumeragiFinalityProof = norito::decode_canonical_for_admission(
            &wire,
            norito::canonical_decode_limits(wire.len()),
        )
        .map_err(|_| WorkerError::InvalidPayload)?;
        proofs.push(proof);
    }
    // Authenticate the exact complete consecutive prefix before interpreting any note bytes.
    let page = verify_checkpoint_page(network, &checkpoint, &proofs, 4096, MAX_IRONWOOD_READ_BYTES)
        .map_err(ironwood_finality_error)?;
    page.tip()
        .verify_global_scope(network, chain)
        .map_err(|_| WorkerError::WrongBinding)?;
    let expectation = IronwoodReceiptExpectation::parse(&object["expected_receipt"])?;
    let profile = iroha_core_privacy::privacy_profiles::compiled_privacy_profile_v1(
        PrivacyRuntimeProtocolIdV1::IronwoodHalo2ActionsV1,
    )
    .map_err(|_| WorkerError::UnsupportedProtocol)?;
    let mut receipts = Vec::new();
    let mut seen = std::collections::BTreeSet::<([u8; 32], usize)>::new();
    // The boundary proof only reauthenticates the caller's checkpoint. Scan the strict extension,
    // so adjacent pages cannot credit the shared boundary twice. Replaying an old page returns
    // stable identities; persistent exactly-once credit remains the caller's atomic responsibility.
    for proof in proofs.iter().skip(1) {
        let block = decode_framed_signed_block(&proof.block_wire)
            .map_err(|_| WorkerError::InvalidPayload)?;
        for output in block.execution_outputs() {
            use iroha_data_model::block::execution_output::ExecutionOutputV1 as O;
            let execution = match output {
                O::Network(n) => n,
                O::NetworkWithLifecycle(n) => &n.execution,
                _ => continue,
            };
            if execution.result.0.is_err() {
                continue;
            }
            let input_index =
                usize::try_from(execution.input_index).map_err(|_| WorkerError::InvalidPayload)?;
            let Some((hash, signed)) = block.external_signed_transaction_at(input_index) else {
                continue;
            };
            let Some((_, submission)) = signed
                .privacy_transaction_intent_binding_if_present_v1()
                .map_err(|_| WorkerError::InvalidPayload)?
            else {
                continue;
            };
            let submission_envelope = submission.runtime_envelope();
            if submission_envelope.protocol_id != PrivacyRuntimeProtocolIdV1::IronwoodHalo2ActionsV1
            {
                continue;
            }
            if !profile.matches_envelope_v1(&submission_envelope) {
                return Err(WorkerError::WrongBinding);
            }
            let iroha_data_model::privacy::PrivacyRuntimeStatementV1::IronwoodHalo2ActionsV1(
                statement,
            ) = &submission_envelope.statement
            else {
                return Err(WorkerError::WrongBinding);
            };
            if statement.context.network_id != network
                || statement.asset_definition_id != asset
                || statement.pool_id != pool
                || statement.public_balance_scope != AssetBalanceScope::Global
            {
                continue;
            }
            let direct_index = ironwood_direct_index(signed)?;
            let notes =
                iroha_core_privacy::privacy_engines::orchard::ironwood_wallet::decrypt_outputs_with_incoming_viewing_key_v1(
                    statement, incoming,
                )
                .map_err(|_| WorkerError::InvalidPayload)?;
            for note in notes {
                if let Some(expected) = &expectation {
                    if expected.transaction != *hash.as_ref()
                        || expected.action_index != note.action_index()
                        || expected.direct_index != direct_index
                    {
                        continue;
                    }
                    if !expected.matches(&note) {
                        return Err(WorkerError::PublicActionMismatch);
                    }
                }
                // The route is obtained from the authenticated complete block at its original input index.
                let route = block
                    .execution_context()
                    .and_then(|context| context.external.get(input_index))
                    .ok_or(WorkerError::InvalidPayload)?;
                if route.entrypoint_hash != hash {
                    return Err(WorkerError::WrongBinding);
                }
                if !seen.insert((*hash.as_ref(), note.action_index())) {
                    continue;
                }
                receipts.push(norito::json!({
                    "receipt_identity": {"transaction_hash_hex": (hex::encode(hash.as_ref())), "action_index": (note.action_index())},
                    "height": (block.header().height().get()),
                    "transaction_hash_hex": (hex::encode(hash.as_ref())),
                    "input_index": (execution.input_index),
                    "direct_instruction_index": direct_index,
                    "action_index": (note.action_index()),
                    "note_commitment_hex": (hex::encode(note.commitment())),
                    "recipient_hex": (hex::encode(note.recipient())),
                    "amount_decimal": (note.value().to_string()),
                    "route": (norito::json::to_value(route).map_err(|_| WorkerError::InvalidPayload)?),
                }));
            }
        }
    }
    if expectation.is_some() && receipts.len() != 1 {
        return Err(WorkerError::PublicActionMismatch);
    }
    let checkpoint = page
        .into_checkpoint()
        .encode_canonical()
        .map_err(|_| WorkerError::InvalidPayload)?;
    norito::json::to_json_bounded(
        &norito::json!({"receipts": receipts, "checkpoint_hex": (hex::encode(checkpoint))}),
        MAX_FRAME_BYTES - 8192,
    )
    .map(String::into_bytes)
    .map_err(|_| WorkerError::CapacityExceeded)
}
pub(crate) fn ironwood_direct_index(
    signed: &iroha_data_model::transaction::SignedTransaction,
) -> Result<usize, WorkerError> {
    use iroha_data_model::{
        isi::privacy::SubmitPrivacyIronwoodProofV1,
        transaction::{Executable, ExecutableBatchItem},
    };
    let found: Vec<_> = match signed.instructions() {
        Executable::Instructions(values) => values.iter().enumerate().filter_map(|(i,v)| v.as_any().is::<SubmitPrivacyIronwoodProofV1>().then_some(i)).collect(),
        Executable::Batch(values) => values.iter().enumerate().filter_map(|(i,v)| matches!(v, ExecutableBatchItem::Instruction(v) if v.as_any().is::<SubmitPrivacyIronwoodProofV1>()).then_some(i)).collect(),
        _ => return Err(WorkerError::InvalidPayload),
    };
    if found.len() != 1 {
        return Err(WorkerError::InvalidPayload);
    }
    Ok(found[0])
}
struct IronwoodReceiptExpectation {
    transaction: [u8; 32],
    direct_index: usize,
    action_index: usize,
    recipient: [u8; 43],
    amount: u64,
    memo: [u8; 512],
}
impl IronwoodReceiptExpectation {
    fn parse(value: &norito::json::Value) -> Result<Option<Self>, WorkerError> {
        if matches!(value, norito::json::Value::Null) {
            return Ok(None);
        }
        let object = value.as_object().ok_or(WorkerError::InvalidPayload)?;
        const FIELDS: &[&str] = &[
            "transaction_hash_hex",
            "direct_instruction_index",
            "action_index",
            "recipient_hex",
            "amount_decimal",
            "memo_hex",
        ];
        if object.len() != FIELDS.len() || !FIELDS.iter().all(|f| object.contains_key(*f)) {
            return Err(WorkerError::InvalidPayload);
        }
        let amount = object["amount_decimal"]
            .as_str()
            .ok_or(WorkerError::InvalidPayload)?;
        let parsed: u64 = amount.parse().map_err(|_| WorkerError::InvalidPayload)?;
        if parsed == 0 || parsed.to_string() != amount {
            return Err(WorkerError::InvalidPayload);
        }
        let index = |key: &str| {
            object[key]
                .as_u64()
                .and_then(|v| usize::try_from(v).ok())
                .ok_or(WorkerError::InvalidPayload)
        };
        Ok(Some(Self {
            transaction: ironwood_hex(&object["transaction_hash_hex"])?,
            direct_index: index("direct_instruction_index")?,
            action_index: index("action_index")?,
            recipient: ironwood_hex(&object["recipient_hex"])?,
            amount: parsed,
            memo: ironwood_hex(&object["memo_hex"])?,
        }))
    }
    fn matches(
        &self,
        note: &iroha_core_privacy::privacy_engines::orchard::ironwood_wallet::DecryptedIronwoodNoteV1,
    ) -> bool {
        self.recipient == note.recipient()
            && self.amount == note.value()
            && &self.memo == note.memo()
    }
}

// The path import remains available for independently owned worker credentials.
// Scenario-account custody uses command 15's ephemeral, authenticated private pipe instead.
fn ironwood_import_deposit(
    vault: &mut WitnessVault,
    payload: &[u8],
) -> Result<WitnessLease, WorkerError> {
    let mut cursor = Cursor::new(payload);
    let path = cursor.text(MAX_PATH_BYTES)?;
    ironwood_import_deposit_from(vault, &mut cursor, || ironwood_key_file(&path), false)
}

fn ironwood_import_deposit_seed(
    vault: &mut WitnessVault,
    payload: &[u8],
) -> Result<WitnessLease, WorkerError> {
    let mut cursor = Cursor::new(payload);
    ironwood_import_deposit_from(
        vault,
        &mut cursor,
        || Err(WorkerError::InvalidPayload),
        true,
    )
}

fn ironwood_import_deposit_from(
    vault: &mut WitnessVault,
    cursor: &mut Cursor<'_>,
    read_key: impl FnOnce() -> Result<Zeroizing<[u8; 32]>, WorkerError>,
    pipe_seed: bool,
) -> Result<WitnessLease, WorkerError> {
    let binding = decode_binding(cursor)?;
    let ttl = cursor.u64()?;
    let chain_discriminant = u16::from_be_bytes(cursor.array()?);
    let action = cursor.bytes_u32(512 * 1024)?;
    let seed_bytes = if pipe_seed {
        Some(cursor.take(32)?)
    } else {
        None
    };
    cursor.finish()?;
    let _scope =
        iroha_data_model::account::address::ChainDiscriminantGuard::enter(chain_discriminant);
    binding.validate()?;
    if binding.protocol != "ironwood-halo2-actions-v1"
        || binding.profile_digest != ironwood_profile_binding_v1()?
    {
        return Err(WorkerError::WrongBinding);
    }
    let now = unix_time_millis()?;
    validate_clock_and_ttl(now, ttl)?;
    vault.purge_expired_at(now);
    if vault.entries.len() >= vault.maximum_handles {
        return Err(WorkerError::CapacityExceeded);
    }
    let value: norito::json::Value =
        norito::json::from_slice(action).map_err(|_| WorkerError::InvalidExecutionBundle)?;
    if value
        .get("changes")
        .and_then(norito::json::Value::as_array)
        .is_none_or(|a| !a.is_empty())
    {
        return Err(WorkerError::InvalidExecutionBundle);
    }
    let seed = if let Some(bytes) = seed_bytes {
        let mut seed = Zeroizing::new([0; 32]);
        seed.copy_from_slice(bytes);
        if seed.iter().all(|b| *b == 0) {
            return Err(WorkerError::InvalidExecutionBundle);
        }
        seed
    } else {
        read_key()?
    };
    let private =
        iroha_crypto::PrivateKey::from_bytes(iroha_crypto::Algorithm::Ed25519, seed.as_ref())
            .map_err(|_| WorkerError::InvalidExecutionBundle)?;
    let public = iroha_crypto::PublicKey::from(private);
    let authority = iroha_data_model::account::AccountId::new(public);
    let mut bundle = Zeroizing::new(b"IPWB\x01".to_vec());
    for field in [
        &binding.signer,
        &authority.to_string(),
        "ironwood-halo2-actions-v1",
        "ironwood_note_action_v1",
    ] {
        put_text(&mut bundle, field);
    }
    put_bytes_u32(&mut bundle, action)?;
    let witness = br#"{"changes":[],"spends":[]}"#;
    // Reserve the complete suffix before the first seed byte enters this allocation.
    // No subsequent growth may leave a seed in a freed Vec backing allocation.
    bundle.reserve_exact(32 + 4 + witness.len());
    let capacity = bundle.capacity();
    bundle.extend_from_slice(seed.as_ref());
    put_bytes_u32(&mut bundle, witness)?;
    debug_assert_eq!(bundle.capacity(), capacity);
    // Full typed validation occurs before a lease is created. It does not prove or sign.
    decode_privacy_wallet_execution_bundle_v1(&mut bundle, action)
        .map_err(|_| WorkerError::InvalidExecutionBundle)?;
    vault.import_material(binding, ttl, bundle, now, Some(chain_discriminant))
}
