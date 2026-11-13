//! Operator-local World replay verification and an immutable preparation capability.
use super::*;
use iroha_core_privacy::ironwood_replay::verify_world_replay_context_v1;
use iroha_data_model::runtime::definition::SelectionId;
use pyo3::types::{PyList, PyString, PyStringMethods, PyTuple};

const MAX_WORLD: usize = 32 * 1024 * 1024;
const MAX_ORIGINALS: usize = 4 * 1024 * 1024;

/// Verified preparation at one independently selected Global cut. Only the native
/// verifier constructs this object. It retains selected public context, never World bytes.
#[pyclass(frozen, name = "VerifiedIronwoodWorldReplayContextV1")]
pub(crate) struct PyIronwoodWorldReplayContext {
    context: String,
    network: String,
    chain_discriminant: u16,
    pool: String,
    asset: String,
    payer: String,
    profile: String,
}

#[pymethods]
impl PyIronwoodWorldReplayContext {
    /// Return the authenticated public preparation fields for the selected pool and payer.
    fn context_json(&self) -> &str {
        &self.context
    }

    /// Verify the original signed payment in the consecutive finality extension from this cut.
    fn verify_payment_effect(
        &self,
        py: Python<'_>,
        payment_proof_frames_hex: &Bound<'_, PyAny>,
        signed_transaction_versioned: &[u8],
        expected_amount_decimal: &str,
    ) -> PyResult<String> {
        let frames = bounded_frames(payment_proof_frames_hex)?;
        py.detach(|| {
            verify_payment_from_context(
                &self.context,
                frames,
                signed_transaction_versioned,
                &self.network,
                IronwoodSelection {
                    chain_discriminant: self.chain_discriminant,
                    pool_id_hex: &self.pool,
                    asset_definition_id: &self.asset,
                    payer_account_id: &self.payer,
                    expected_profile_digest_hex: &self.profile,
                },
                expected_amount_decimal,
            )
        })
        .map_err(NativeValidationError::into_py)
    }
}

// Admit concrete built-in sequences and cap the entire hex page before copying
// it into Rust. Python callbacks and arbitrary iterators cannot supply this page.
fn bounded_frames(raw: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    let count = if let Ok(list) = raw.cast_exact::<PyList>() {
        list.len()
    } else if let Ok(tuple) = raw.cast_exact::<PyTuple>() {
        tuple.len()
    } else {
        return Err(PyTypeError::new_err("proof frames must be a list or tuple"));
    };
    if count == 0 || count > 64 {
        return Err(PyValueError::new_err("proof page count exceeds bounds"));
    }
    let mut total = 0usize;
    let mut frames = Vec::with_capacity(count);
    for index in 0..count {
        let item = raw.get_item(index)?;
        let value = item.cast_exact::<PyString>()?;
        let length = value.code_point_len()?;
        total = total
            .checked_add(length)
            .filter(|total| *total <= 2 * MAX)
            .ok_or_else(|| PyValueError::new_err("proof page bytes exceed bounds"))?;
        let encoded = value.encode_utf8()?;
        let text = encoded.as_bytes();
        if text.is_empty()
            || text.len() != length
            || !text.len().is_multiple_of(2)
            || !text
                .iter()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(b))
        {
            return Err(PyValueError::new_err(
                "proof frame must be canonical lowercase hex",
            ));
        }
        frames.push(
            std::str::from_utf8(text)
                .map_err(|_| PyValueError::new_err("invalid proof hex encoding"))?
                .to_owned(),
        );
    }
    Ok(frames)
}

/// Authenticate original bounded operator-local replay evidence at an independently selected cut.
/// The checkpoint, block hash, selection, World schema and privacy profile are caller trust pins.
#[pyfunction]
#[pyo3(name = "verify_ironwood_world_replay_v1")]
#[expect(
    clippy::too_many_arguments,
    reason = "Keep independently selected evidence and trust pins explicit across this verifier boundary."
)]
pub(crate) fn verify_ironwood_world_replay_v1_py(
    py: Python<'_>,
    world_frame: &[u8],
    originals_frame: &[u8],
    checkpoint_hex: &str,
    proof_frames_hex: &Bound<'_, PyAny>,
    network_id_hex: &str,
    #[pyo3(from_py_with = exact_chain_discriminant)] chain_discriminant: u16,
    height: u64,
    block_hash_hex: &str,
    expected_selection_hex: &str,
    expected_schema_hash_hex: &str,
    pool_id_hex: &str,
    asset_definition_id: &str,
    payer_account_id: &str,
    expected_profile_digest_hex: &str,
) -> PyResult<PyIronwoodWorldReplayContext> {
    if world_frame.is_empty()
        || world_frame.len() > MAX_WORLD
        || originals_frame.is_empty()
        || originals_frame.len() > MAX_ORIGINALS
        || asset_definition_id.len() > 512
        || payer_account_id.len() > 512
    {
        return Err(PyValueError::new_err(
            "operator replay inputs exceed bounds",
        ));
    }
    let frames = bounded_frames(proof_frames_hex)?;
    let (page, asset, payer, network, pool, selection, schema) = py
        .detach(|| {
            let page = finality_page(checkpoint_hex, &frames, network_id_hex)?;
            if page.tip().height() != height
                || page.tip().header().hash().as_ref() != &fixed(block_hash_hex)?
            {
                return Err(invalid());
            }
            let _guard = ChainDiscriminantGuard::enter(chain_discriminant);
            let asset: iroha_data_model::asset::AssetDefinitionId =
                asset_definition_id.parse().map_err(|_| invalid())?;
            if asset.to_string() != asset_definition_id {
                return Err(invalid());
            }
            let payer = parse_selected_account_id_native(
                payer_account_id,
                "payer_account_id",
                chain_discriminant,
            )?;
            let network = network_id_from_genesis_hash_bytes(fixed(network_id_hex)?);
            let pool = PrivacyPoolIdV1::new(fixed(pool_id_hex)?);
            let selection = SelectionId(fixed(expected_selection_hex)?);
            let schema = iroha_crypto::Hash::from_marked_bytes(fixed(expected_schema_hash_hex)?)
                .ok_or_else(invalid)?;
            Ok((page, asset, payer, network, pool, selection, schema))
        })
        .map_err(NativeValidationError::into_py)?;
    let verified = py
        .detach(|| {
            verify_world_replay_context_v1(
                page.tip(),
                world_frame,
                originals_frame,
                network,
                selection,
                schema,
                pool,
                &asset,
                &payer,
            )
        })
        .map_err(|error| {
            crate::public_finality_checkpoint::native_read_error(py, "ironwood_world_replay", error)
        })?;
    let context = py
        .detach(|| {
            let _guard = ChainDiscriminantGuard::enter(chain_discriminant);
            verified_context_json(
                &page,
                IronwoodSelection {
                    chain_discriminant,
                    pool_id_hex,
                    asset_definition_id,
                    payer_account_id,
                    expected_profile_digest_hex,
                },
                &asset,
                &payer,
                verified.state(),
                verified.payer_balance(),
                verified.reserve_balance(),
            )
        })
        .map_err(NativeValidationError::into_py)?;
    Ok(PyIronwoodWorldReplayContext {
        context,
        network: network_id_hex.to_owned(),
        chain_discriminant,
        pool: pool_id_hex.to_owned(),
        asset: asset_definition_id.to_owned(),
        payer: payer_account_id.to_owned(),
        profile: expected_profile_digest_hex.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signed_payment_original_resource_refusal_survives_scope_and_retries_exact_bytes() {
        use iroha_data_model::{
            isi::Log,
            level::Level,
            sumeragi_finality::FinalityReadError,
            transaction::{FeePaymentIntent, TransactionBuilder as ModelTransactionBuilder},
        };
        let key = KeyPair::from_seed(vec![0x51; 32], Algorithm::Ed25519);
        let signed = ModelTransactionBuilder::new(
            network_id_from_genesis_hash_bytes([0x61; 32]),
            AccountId::new(key.public_key().clone()),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(
            Level::INFO,
            "original resource refusal".to_owned(),
        )])
        .sign(key.private_key());
        let bytes = signed.encode_versioned();
        let limits = norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, 64);
        let error = norito::with_decode_limits_scope(limits, || {
            decode_canonical_signed_transaction_v1_native(&bytes)
        })
        .unwrap_err();
        let NativeValidationError::Read(
            "signed_transaction_decode",
            FinalityReadError::DecodeResource(original),
        ) = &error
        else {
            panic!("original signed decoder refusal was flattened: {error:?}");
        };
        assert_eq!(
            original.kind(),
            norito::core::DecodeAttemptErrorKind::EnclosingLimit
        );
        assert!(original.local_refusal().is_some());
        Python::initialize();
        Python::attach(|py| {
            let error = error.into_py();
            for (field, value) in [
                ("kind", "decode_resource"),
                ("phase", "signed_transaction_decode"),
            ] {
                assert_eq!(
                    error
                        .value(py)
                        .getattr(field)
                        .unwrap()
                        .extract::<String>()
                        .unwrap(),
                    value
                );
            }
        });
        let retry = decode_canonical_signed_transaction_v1_native(&bytes).unwrap();
        assert_eq!(retry.hash(), signed.hash());
        assert_eq!(retry.encode_versioned(), bytes);
        assert!(retry.verify_signature().is_ok());
        let mut malformed = bytes;
        malformed.push(0);
        assert!(matches!(
            decode_canonical_signed_transaction_v1_native(&malformed),
            Err(NativeValidationError::Value(_))
        ));
    }

    #[test]
    fn replay_page_admission_bounds_before_copy_and_preserves_exact_frames() {
        Python::initialize();
        Python::attach(|py| {
            let good = PyList::new(py, ["01", "ff"]).unwrap();
            assert_eq!(bounded_frames(good.as_any()).unwrap(), ["01", "ff"]);
            let tuple = PyTuple::new(py, ["02"]).unwrap();
            assert_eq!(bounded_frames(tuple.as_any()).unwrap(), ["02"]);
            for values in [
                vec![],
                vec!["00"; 65],
                vec!["0"],
                vec!["FF"],
                vec!["éé"],
                vec![""],
            ] {
                assert!(bounded_frames(PyList::new(py, values).unwrap().as_any()).is_err());
            }
            let oversized = "00".repeat(MAX + 1);
            assert!(bounded_frames(PyList::new(py, [oversized]).unwrap().as_any()).is_err());
            assert!(bounded_frames(PyString::new(py, "00").as_any()).is_err());
        });
    }

    #[test]
    fn verified_replay_handle_cannot_be_constructed_from_python() {
        Python::initialize();
        Python::attach(|py| {
            assert!(
                py.get_type::<PyIronwoodWorldReplayContext>()
                    .call0()
                    .is_err()
            );
        });
    }

    #[test]
    fn replay_factory_authenticates_exact_cut_and_keeps_originals_out_of_handle() {
        use iroha_core_privacy::{ironwood_replay::IronwoodReplayOriginalsV1, privacy_state::*};
        use iroha_data_model::{
            asset::{AssetBalanceScope, AssetDefinitionId},
            privacy::{PrivacyIronwoodPoolBootstrapV1, PrivacyRootRoleV1},
            sumeragi_finality::{
                WorldStateElementKindV1, WorldStateSnapshotEntryV1, WorldStateSnapshotV1,
                ironwood_world_schema_v1, test_fixtures::NativeFinalityFixture,
                world_state_value_hash_v1,
            },
        };
        let payer = AccountId::new(
            KeyPair::from_seed(vec![1; 32], Algorithm::Ed25519)
                .public_key()
                .clone(),
        );
        let reserve = AccountId::new(
            KeyPair::from_seed(vec![2; 32], Algorithm::Ed25519)
                .public_key()
                .clone(),
        );
        let asset: AssetDefinitionId = "839FV3NJC8NfgWQvghXU2hEFQm9a".parse().unwrap();
        let pool = PrivacyPoolIdV1::new([3; 32]);
        let bootstrap = PrivacyIronwoodPoolBootstrapV1::new(
            pool,
            asset.clone(),
            AssetBalanceScope::Global,
            reserve,
        )
        .unwrap();
        let namespace = bootstrap.namespace();
        let state = PrivacyOrchardPoolStateV1::bootstrap_ironwood(bootstrap).unwrap();
        let provenance =
            PrivacyRootProvenanceV1::orchard_pool_bootstrap(state.bootstrap_digest(), 2).unwrap();
        let head =
            PrivacyRootHeadRecordV1::new(state.epoch(), state.root(), provenance, None, 1).unwrap();
        let selection = iroha_core::runtime::release_catalog()
            .unwrap()
            .ironwood
            .as_ref()
            .unwrap()
            .selection
            .clone();
        let originals = IronwoodReplayOriginalsV1 {
            selection,
            state: PrivacyStateItemRecordV1::orchard_pool_state(state).unwrap(),
            head,
            payer_balance: None,
            reserve_balance: None,
        };
        let anchor = PrivacyRootRoleV1::NoteCommitmentAnchor;
        let table = |field: &str, key_hash, value_hash| WorldStateSnapshotEntryV1 {
            field_id: field.into(),
            kind: WorldStateElementKindV1::Table,
            key_hash: Some(key_hash),
            value_hash,
        };
        // Real native certificates authenticate synthetic complete commitment entries.
        // The Core scheduled-activation fixture separately checks executed World capture.
        let snapshot = WorldStateSnapshotV1 {
            schema_hash: ironwood_world_schema_v1(),
            entries: vec![
                table(
                    "world.privacy_commitments",
                    world_state_value_hash_v1(
                        &PrivacyCommitmentKeyV1::orchard_pool_state(namespace).unwrap(),
                    )
                    .unwrap(),
                    world_state_value_hash_v1(&originals.state).unwrap(),
                ),
                table(
                    "world.privacy_root_heads",
                    world_state_value_hash_v1(
                        &PrivacyRootHeadKeyV1::new(namespace, anchor).unwrap(),
                    )
                    .unwrap(),
                    world_state_value_hash_v1(&originals.head).unwrap(),
                ),
                table(
                    "world.privacy_roots",
                    world_state_value_hash_v1(
                        &PrivacyRootKeyV1::new(
                            namespace,
                            anchor,
                            originals.head.epoch(),
                            originals.head.root(),
                        )
                        .unwrap(),
                    )
                    .unwrap(),
                    world_state_value_hash_v1(originals.head.provenance()).unwrap(),
                ),
                WorldStateSnapshotEntryV1 {
                    field_id: "world.runtime_selection".into(),
                    kind: WorldStateElementKindV1::Cell,
                    key_hash: None,
                    value_hash: world_state_value_hash_v1(&originals.selection).unwrap(),
                },
            ],
        };
        let mut native = NativeFinalityFixture::start("python-ironwood-world");
        let checkpoint = hex::encode(native.checkpoint().encode_canonical().unwrap());
        let checkpoint_frame =
            hex::encode(norito::encode_canonical(native.genesis_proof()).unwrap());
        let block = native.block_with_submitted_work(native.next_header());
        let proof = native.certify_with_world_root(block, snapshot.root().unwrap());
        let verified = native.verifier().verify_retained_decision(&proof).unwrap();
        let height = verified.height();
        let block_hash = hex::encode(verified.header().hash().as_ref());
        let frame = hex::encode(norito::encode_canonical(&proof).unwrap());
        let network = hex::encode(native.network_id().as_bytes());
        let selection_pin = hex::encode(originals.selection.id().unwrap().0);
        let schema_pin = hex::encode(snapshot.schema_hash.as_ref());
        let profile =
            compiled_privacy_profile_v1(PrivacyRuntimeProtocolIdV1::IronwoodHalo2ActionsV1)
                .unwrap();
        let profile_pin = hex::encode(
            crate::privacy_wallet_worker::compiled_profile_digest(
                "ironwood-halo2-actions-v1",
                &crate::privacy_wallet_worker::CompiledProfileBinding {
                    parameter_id: *profile.parameter_id.as_bytes(),
                    parameter_digest: *profile.parameter_digest.as_bytes(),
                    verifier_digest: *profile.verifier_digest.as_bytes(),
                    statement_schema_digest: *profile.statement_schema_digest.as_bytes(),
                    engine_manifest_digest: *profile.engine_manifest_digest.as_bytes(),
                },
            )
            .unwrap(),
        );
        let world = norito::encode_canonical(&snapshot).unwrap();
        let typed = norito::encode_canonical(&originals).unwrap();
        Python::initialize();
        Python::attach(|py| {
            let frames = PyTuple::new(py, [checkpoint_frame, frame]).unwrap();
            let verify = |world: &[u8],
                          typed: &[u8],
                          selected_height,
                          selected_hash: &str,
                          selected_schema: &str,
                          selected_profile: &str| {
                verify_ironwood_world_replay_v1_py(
                    py,
                    world,
                    typed,
                    &checkpoint,
                    frames.as_any(),
                    &network,
                    369,
                    selected_height,
                    selected_hash,
                    &selection_pin,
                    selected_schema,
                    &hex::encode(pool.as_bytes()),
                    &asset.to_string(),
                    &payer.to_i105_for_discriminant(369).unwrap(),
                    selected_profile,
                )
            };
            let handle = verify(
                &world,
                &typed,
                height,
                &block_hash,
                &schema_pin,
                &profile_pin,
            )
            .unwrap();
            let context: norito::json::Value =
                norito::json::from_str(handle.context_json()).unwrap();
            assert_eq!(context["height"].as_u64(), Some(height));
            assert_eq!(context["payer_balance_decimal"].as_str(), Some("0"));
            assert_eq!(context["reserve_balance_decimal"].as_str(), Some("0"));
            assert!(context.get("world").is_none());
            let limit = norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, 64);
            let preparation_refusal = norito::with_decode_limits_scope(limit, || {
                verify(
                    &world,
                    &typed,
                    height,
                    &block_hash,
                    &schema_pin,
                    &profile_pin,
                )
            })
            .err()
            .unwrap();
            let payment_refusal = norito::with_decode_limits_scope(limit, || {
                handle.verify_payment_effect(py, frames.as_any(), b"not-yet-decoded", "17")
            })
            .unwrap_err();
            for (error, phase) in [
                (preparation_refusal, "checkpoint_decode"),
                (payment_refusal, "ironwood_context_decode"),
            ] {
                assert_eq!(
                    error
                        .value(py)
                        .getattr("kind")
                        .unwrap()
                        .extract::<String>()
                        .unwrap(),
                    "decode_resource"
                );
                assert_eq!(
                    error
                        .value(py)
                        .getattr("phase")
                        .unwrap()
                        .extract::<String>()
                        .unwrap(),
                    phase
                );
            }
            assert!(
                verify(
                    &world,
                    &typed,
                    height + 1,
                    &block_hash,
                    &schema_pin,
                    &profile_pin
                )
                .is_err()
            );
            assert!(
                verify(
                    &world,
                    &typed,
                    height,
                    &"01".repeat(32),
                    &schema_pin,
                    &profile_pin
                )
                .is_err()
            );
            assert!(
                verify(
                    &world,
                    &typed,
                    height,
                    &block_hash,
                    &"01".repeat(32),
                    &profile_pin
                )
                .is_err()
            );
            assert!(
                verify(
                    &world,
                    &typed,
                    height,
                    &block_hash,
                    &schema_pin,
                    &"01".repeat(32)
                )
                .is_err()
            );
            assert!(
                verify(
                    &world,
                    &typed[..typed.len() - 1],
                    height,
                    &block_hash,
                    &schema_pin,
                    &profile_pin
                )
                .is_err()
            );
            assert!(
                verify(
                    &world[..world.len() - 1],
                    &typed,
                    height,
                    &block_hash,
                    &schema_pin,
                    &profile_pin
                )
                .is_err()
            );
            assert!(
                handle
                    .verify_payment_effect(py, PyTuple::empty(py).as_any(), b"unsigned", "17")
                    .is_err()
            );
        });
    }
}
