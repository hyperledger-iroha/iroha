//! Native asset escrow instruction handlers.

use std::collections::BTreeSet;

use eyre::Result;
use iroha_crypto::{Algorithm, Hash, KeyPair};
use iroha_data_model::{
    IntoKeyValue,
    account::{Account, AccountId},
    asset::{AssetDefinitionId, AssetId},
    escrow::{
        AnonymousAssetEscrowProofRecord, AnonymousAssetEscrowRecord,
        AnonymousAssetEscrowResolution, AssetEscrowKind, AssetEscrowRecord, AssetEscrowResolution,
        AssetEscrowStatus, EscrowId,
    },
    events::data::{
        escrow::{AssetEscrowDisputed, AssetEscrowResolved, EscrowEvent},
        prelude::{AssetChanged, AssetEvent},
    },
    fastpq::TransferDeltaTranscript,
    isi::escrow::{
        AcceptAnonymousAssetEscrow, AcceptAssetEscrow, CancelAnonymousAssetEscrow,
        CancelAssetEscrow, CancelAssetLock, DrawdownAssetLock, ExpireAssetLock,
        MarkAnonymousEscrowPaymentSent, MarkEscrowPaymentSent, OpenAnonymousAssetEscrow,
        OpenAnonymousEscrowDispute, OpenAssetEscrow, OpenAssetLock, OpenEscrowDispute,
        ReleaseAnonymousAssetEscrow, ReleaseAssetEscrow, ResolveAnonymousEscrowDispute,
        ResolveEscrowDispute,
    },
    permission::Permission,
    prelude::*,
    proof::ProofAttachment,
    query::{
        dsl::{CompoundPredicate, EvaluatePredicate},
        error::{FindError, QueryExecutionFail},
        escrow::prelude::{
            FindAnonymousAssetEscrowById, FindAnonymousAssetEscrows,
            FindAnonymousAssetEscrowsByBuyer, FindAnonymousAssetEscrowsBySeller,
            FindAnonymousAssetEscrowsByStatus, FindAssetEscrowById, FindAssetEscrows,
            FindAssetEscrowsByBuyer, FindAssetEscrowsBySeller, FindAssetEscrowsByStatus,
        },
        json::PredicateJson,
    },
    zk::{BackendTag, OpenVerifyEnvelope},
};
use iroha_primitives::numeric::Numeric;
use mv::storage::StorageReadOnly;
use norito::json::Value;

use super::{
    Error, Execute,
    asset::isi::{
        NumericAssetTransferSourcePolicy, apply_numeric_asset_transfer_delta,
        assert_numeric_spec_with, prepare_outbound_asset_transfer_control_update,
        update_control_record,
    },
};
use crate::{
    prelude::ValidSingularQuery,
    smartcontracts::ValidQuery,
    smartcontracts::isi::domain::isi::ensure_controller_capabilities,
    state::{StateReadOnly, StateTransaction, WorldReadOnly},
};

/// Permission name required to resolve disputed native escrows.
pub const CAN_RESOLVE_ESCROW_DISPUTE: &str = "CanResolveEscrowDispute";

const ESCROW_CUSTODY_SEED_LABEL: &str = "iroha-native-asset-escrow-v1";

fn validation_err(message: impl Into<String>) -> Error {
    iroha_data_model::isi::error::InstructionExecutionError::InvariantViolation(
        message.into().into(),
    )
}

fn ensure_non_negative(value: &Numeric) -> Result<(), Error> {
    if value.mantissa().is_negative() {
        return Err(validation_err("escrow amount must not be negative"));
    }
    Ok(())
}

fn ensure_positive(value: &Numeric) -> Result<(), Error> {
    ensure_non_negative(value)?;
    if value.is_zero() {
        return Err(validation_err("escrow amount must be non-zero"));
    }
    Ok(())
}

fn ensure_resolution_split(
    total_amount: &Numeric,
    buyer_amount: &Numeric,
    seller_amount: &Numeric,
) -> Result<(), Error> {
    ensure_non_negative(buyer_amount)?;
    ensure_non_negative(seller_amount)?;
    let split_total = buyer_amount
        .clone()
        .checked_add(seller_amount.clone())
        .ok_or_else(|| validation_err("escrow resolution amount overflow"))?;
    if split_total != *total_amount {
        return Err(validation_err("court split must equal escrow amount"));
    }
    Ok(())
}

fn ensure_asset_lock(record: &AssetEscrowRecord) -> Result<(), Error> {
    if record.kind != AssetEscrowKind::Lock {
        return Err(validation_err("escrow is not a generic asset lock"));
    }
    Ok(())
}

/// Derive the deterministic protocol custody account for an escrow.
#[must_use]
pub fn escrow_custody_account_id(
    chain_id: &iroha_data_model::ChainId,
    escrow_id: &EscrowId,
    asset_definition: &AssetDefinitionId,
) -> AccountId {
    let seed_material = format!(
        "{ESCROW_CUSTODY_SEED_LABEL}|{}|{}|{asset_definition}",
        chain_id.as_str(),
        hex::encode(escrow_id.as_hash().as_ref()),
    );
    let seed: [u8; Hash::LENGTH] = Hash::new(seed_material).into();
    let keypair = KeyPair::from_seed(seed.to_vec(), Algorithm::Ed25519);
    AccountId::new(keypair.public_key().clone())
}

fn ensure_custody_account(
    custody: &AccountId,
    state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<bool, Error> {
    ensure_controller_capabilities(
        custody.controller(),
        &state_transaction.crypto.allowed_signing,
        &state_transaction.crypto.allowed_curve_ids,
    )?;
    if state_transaction.world.account(custody).is_ok() {
        return Ok(false);
    }
    let account = Account {
        id: custody.clone(),
        metadata: Metadata::default(),
        label: None,
        uaid: None,
        opaque_ids: Vec::new(),
    };
    let (id, value) = account.into_key_value();
    state_transaction.world.accounts.insert(id, value);
    Ok(true)
}

fn has_permission(
    state_transaction: &StateTransaction<'_, '_>,
    authority: &AccountId,
    permission_name: &str,
) -> bool {
    let has_named_permission = |permission: &Permission| permission.name() == permission_name;

    if state_transaction
        .world
        .account_permissions
        .get(authority)
        .is_some_and(|permissions| permissions.iter().any(has_named_permission))
    {
        return true;
    }

    state_transaction
        .world
        .account_roles
        .iter()
        .filter_map(|(role_key, ())| {
            if &role_key.account == authority {
                state_transaction.world.roles.get(&role_key.id)
            } else {
                None
            }
        })
        .any(|role| role.permissions().any(has_named_permission))
}

fn custody_asset(record: &AssetEscrowRecord) -> AssetId {
    AssetId::new(record.asset_definition.clone(), record.custody.clone())
}

fn party_asset(record: &AssetEscrowRecord, account: &AccountId) -> AssetId {
    AssetId::new(record.asset_definition.clone(), account.clone())
}

fn transfer_numeric_asset_for_escrow(
    state_transaction: &mut StateTransaction<'_, '_>,
    source_id: &AssetId,
    destination_id: &AssetId,
    amount: &Numeric,
    source_policy: NumericAssetTransferSourcePolicy,
) -> Result<TransferDeltaTranscript, Error> {
    let control_update =
        prepare_outbound_asset_transfer_control_update(state_transaction, source_id, amount)?;
    let (source_id, destination_id, delta) = apply_numeric_asset_transfer_delta(
        state_transaction,
        source_id,
        destination_id,
        amount,
        source_policy,
    )?;
    if let Some(record) = control_update {
        update_control_record(state_transaction, source_id.account(), record)?;
    }

    #[allow(clippy::float_arithmetic)]
    #[cfg(feature = "telemetry")]
    state_transaction
        .telemetry
        .observe_tx_amount(amount.clone().to_f64());

    state_transaction.world.emit_events([
        AssetEvent::Removed(AssetChanged {
            asset: source_id,
            amount: amount.clone(),
        }),
        AssetEvent::Added(AssetChanged {
            asset: destination_id,
            amount: amount.clone(),
        }),
    ]);

    Ok(delta)
}

fn ensure_non_zero_bytes(label: &str, value: &[u8; 32]) -> Result<(), Error> {
    if *value == [0u8; 32] {
        return Err(validation_err(format!("{label} must not be zero")));
    }
    Ok(())
}

fn ensure_unique_non_zero_bytes(label: &str, values: &[[u8; 32]]) -> Result<(), Error> {
    if values.is_empty() {
        return Err(validation_err(format!("{label} must not be empty")));
    }
    ensure_optional_unique_non_zero_bytes(label, values)
}

fn ensure_optional_unique_non_zero_bytes(label: &str, values: &[[u8; 32]]) -> Result<(), Error> {
    let mut seen = std::collections::BTreeSet::new();
    for value in values {
        ensure_non_zero_bytes(label, value)?;
        if !seen.insert(*value) {
            return Err(validation_err(format!("{label} must be unique")));
        }
    }
    Ok(())
}

fn ensure_single_escrow_nullifier(nullifiers: &[[u8; 32]]) -> Result<(), Error> {
    ensure_unique_non_zero_bytes("escrow nullifier", nullifiers)?;
    if nullifiers.len() != 1 {
        return Err(validation_err(
            "anonymous escrow v1 spends exactly one escrow note",
        ));
    }
    Ok(())
}

fn ensure_close_proof_uses_canonical_transfer_v2_envelope(
    proof: &ProofAttachment,
) -> Result<(), Error> {
    if proof.backend != proof.proof.backend {
        return Err(validation_err(
            "anonymous escrow close proof backend mismatch",
        ));
    }
    if proof.backend != proof.vk_ref.backend {
        return Err(validation_err(
            "anonymous escrow close proof verifier-key backend mismatch",
        ));
    }
    if proof.backend.as_str() != crate::zk::ZK_BACKEND_HALO2_IPA {
        return Err(validation_err(
            "anonymous escrow close proof requires halo2/ipa backend",
        ));
    }
    let envelope: OpenVerifyEnvelope =
        norito::decode_from_bytes(&proof.proof.bytes).map_err(|_| {
            validation_err("anonymous escrow close proof must use OpenVerifyEnvelope payload")
        })?;
    if envelope.backend != BackendTag::Halo2IpaPasta {
        return Err(validation_err(
            "anonymous escrow close proof unexpected OpenVerifyEnvelope backend tag",
        ));
    }
    if !crate::zk::confidential_v2::is_confidential_transfer_v2_circuit_id(&envelope.circuit_id) {
        return Err(validation_err(
            "anonymous escrow close proof requires confidential transfer v2 circuit",
        ));
    }
    if envelope.public_inputs
        != crate::zk::confidential_v2::CONFIDENTIAL_TRANSFER_V2_PUBLIC_INPUTS_SCHEMA_V1
    {
        return Err(validation_err(
            "anonymous escrow close proof public inputs schema mismatch",
        ));
    }
    if !envelope.aux.is_empty() {
        return Err(validation_err(
            "anonymous escrow close proof envelope auxiliary bytes must be empty",
        ));
    }
    if envelope.vk_hash == [0u8; 32] {
        return Err(validation_err(
            "anonymous escrow close proof verifier key hash must be non-zero",
        ));
    }
    if proof
        .vk_commitment
        .is_some_and(|commitment| commitment != envelope.vk_hash)
    {
        return Err(validation_err(
            "anonymous escrow close proof verifier key commitment mismatch",
        ));
    }
    Ok(())
}

fn ensure_close_proof_spends_escrow_commitment(
    proof: &ProofAttachment,
    escrow_commitment: [u8; 32],
) -> Result<(), Error> {
    ensure_close_proof_uses_canonical_transfer_v2_envelope(proof)?;
    let (input_commitments, _nullifiers, _outputs, _root, _asset_tag, _chain_tag) =
        crate::zk::confidential_v2::parse_transfer_public_inputs(&proof.proof.bytes).map_err(
            |err| {
                validation_err(format!(
                    "invalid anonymous escrow close proof public inputs: {err}"
                ))
            },
        )?;

    let zero = [0u8; 32];
    let mut non_zero_inputs = input_commitments
        .iter()
        .copied()
        .filter(|commitment| commitment != &zero);
    let Some(proof_commitment) = non_zero_inputs.next() else {
        return Err(validation_err(
            "anonymous escrow close proof must spend exactly one escrow commitment",
        ));
    };
    if non_zero_inputs.next().is_some() {
        return Err(validation_err(
            "anonymous escrow close proof must spend exactly one escrow commitment",
        ));
    }
    if proof_commitment != escrow_commitment {
        return Err(validation_err(
            "anonymous escrow close proof input commitment mismatch",
        ));
    }
    Ok(())
}

fn proof_record(
    proof: &ProofAttachment,
    nullifiers: Vec<[u8; 32]>,
    output_commitments: Vec<[u8; 32]>,
    root_hint: Option<[u8; 32]>,
    recorded_at_ms: u64,
) -> AnonymousAssetEscrowProofRecord {
    AnonymousAssetEscrowProofRecord {
        nullifiers,
        output_commitments,
        proof_hash: crate::zk::hash_proof(&proof.proof),
        envelope_hash: proof.envelope_hash,
        root_hint,
        recorded_at_ms,
    }
}

fn ensure_anonymous_escrow_asset_uses_transfer_v2(
    state_transaction: &StateTransaction<'_, '_>,
    asset_definition: &AssetDefinitionId,
) -> Result<(), Error> {
    state_transaction.world.asset_definition(asset_definition)?;
    let Some(zk_state) = state_transaction.world.zk_assets.get(asset_definition) else {
        return Err(validation_err(
            "anonymous escrow requires a registered shielded asset",
        ));
    };
    let Some(binding) = zk_state.vk_transfer.as_ref() else {
        return Err(validation_err(
            "anonymous escrow requires a bound transfer verifying key",
        ));
    };
    let Some(record) = state_transaction.world.verifying_keys.get(&binding.id) else {
        return Err(validation_err(
            "anonymous escrow transfer verifying key is missing",
        ));
    };
    if !crate::zk::confidential_v2::is_confidential_transfer_v2_circuit_id(&record.circuit_id) {
        return Err(validation_err(
            "anonymous escrow requires confidential transfer v2 public input commitments",
        ));
    }
    Ok(())
}

fn execute_anonymous_escrow_transfer(
    authority: &AccountId,
    state_transaction: &mut StateTransaction<'_, '_>,
    asset_definition: AssetDefinitionId,
    nullifiers: Vec<[u8; 32]>,
    output_commitments: Vec<[u8; 32]>,
    proof: ProofAttachment,
    root_hint: Option<[u8; 32]>,
) -> Result<AnonymousAssetEscrowProofRecord, Error> {
    ensure_unique_non_zero_bytes("shielded nullifier", &nullifiers)?;
    ensure_unique_non_zero_bytes("shielded output commitment", &output_commitments)?;
    ensure_anonymous_escrow_asset_uses_transfer_v2(state_transaction, &asset_definition)?;
    let recorded_at_ms = state_transaction.block_unix_timestamp_ms();
    let record = proof_record(
        &proof,
        nullifiers.clone(),
        output_commitments.clone(),
        root_hint,
        recorded_at_ms,
    );
    let transfer = iroha_data_model::isi::zk::ZkTransfer::new(
        asset_definition,
        nullifiers,
        output_commitments,
        proof,
        root_hint,
    );
    state_transaction.native_anonymous_escrow_transfer_depth = state_transaction
        .native_anonymous_escrow_transfer_depth
        .saturating_add(1);
    let result = transfer.execute(authority, state_transaction);
    state_transaction.native_anonymous_escrow_transfer_depth = state_transaction
        .native_anonymous_escrow_transfer_depth
        .saturating_sub(1);
    result?;
    Ok(record)
}

fn anonymous_escrow_record(
    state_transaction: &StateTransaction<'_, '_>,
    escrow_id: &EscrowId,
) -> Result<AnonymousAssetEscrowRecord, Error> {
    state_transaction
        .world
        .anonymous_asset_escrows
        .get(escrow_id)
        .cloned()
        .ok_or_else(|| validation_err("anonymous escrow not found"))
}

/// Return whether the asset id points at a protocol custody account recorded by a native escrow.
///
/// The guard intentionally covers closed records too. Escrow ISIs should leave closed custody
/// balances at zero, and keeping the source permanently blocked avoids ever making public,
/// deterministically derived custody controllers useful as generic asset debit authorities.
pub(crate) fn is_native_escrow_custody_asset(
    state_transaction: &StateTransaction<'_, '_>,
    source_id: &AssetId,
) -> Result<bool, Error> {
    let resolved_id = if matches!(
        source_id.scope(),
        iroha_data_model::asset::AssetBalanceScope::Dataspace(_)
    ) {
        source_id.clone()
    } else {
        state_transaction
            .world
            .resolve_asset_id_for_current_scope(source_id)?
    };
    Ok(state_transaction
        .world
        .asset_escrows
        .iter()
        .any(|(_, record)| {
            record.asset_definition == *resolved_id.definition()
                && record.custody == *resolved_id.account()
        })
        || state_transaction
            .world
            .vpn_leases
            .iter()
            .any(|(_, record)| {
                record.asset_definition == *resolved_id.definition()
                    && record.custody_account_id == *resolved_id.account()
            }))
}

impl Execute for OpenAssetEscrow {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        if state_transaction
            .world
            .asset_escrows
            .get(&self.escrow_id)
            .is_some()
            || state_transaction
                .world
                .anonymous_asset_escrows
                .get(&self.escrow_id)
                .is_some()
        {
            return Err(validation_err("escrow already exists"));
        }

        ensure_positive(&self.amount)?;
        let spec = state_transaction
            .numeric_spec_for(&self.asset_definition)
            .map_err(Error::from)?;
        assert_numeric_spec_with(&self.amount, spec)?;
        state_transaction.world.account(authority)?;
        state_transaction
            .world
            .asset_definition(&self.asset_definition)?;

        let custody = escrow_custody_account_id(
            state_transaction.chain_id(),
            &self.escrow_id,
            &self.asset_definition,
        );

        let seller_asset = AssetId::new(self.asset_definition.clone(), authority.clone());
        let custody_asset = AssetId::new(self.asset_definition.clone(), custody.clone());
        let custody_created = ensure_custody_account(&custody, state_transaction)?;
        let transfer_result = transfer_numeric_asset_for_escrow(
            state_transaction,
            &seller_asset,
            &custody_asset,
            &self.amount,
            NumericAssetTransferSourcePolicy::User,
        );
        if transfer_result.is_err() && custody_created {
            state_transaction.world.accounts.remove(custody.clone());
        }
        let delta = transfer_result?;
        state_transaction.record_transfer_transcript(authority, delta)?;

        let record = AssetEscrowRecord {
            id: self.escrow_id,
            seller: authority.clone(),
            buyer: None,
            asset_definition: self.asset_definition,
            amount: self.amount.clone(),
            custody,
            status: AssetEscrowStatus::Open,
            kind: AssetEscrowKind::Marketplace,
            remaining_amount: self.amount,
            release_authority: None,
            expires_at_ms: None,
            evidence_hashes: self.evidence_hashes,
            created_at_ms: state_transaction.block_unix_timestamp_ms(),
            accepted_at_ms: None,
            payment_sent_at_ms: None,
            disputed_at_ms: None,
            closed_at_ms: None,
            resolution: None,
        };
        state_transaction
            .world
            .insert_asset_escrow_entry(record.clone());
        state_transaction
            .world
            .emit_events(Some(EscrowEvent::Opened(record)));
        Ok(())
    }
}

impl Execute for AcceptAssetEscrow {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        state_transaction.world.account(authority)?;
        let Some(mut record) = state_transaction
            .world
            .asset_escrows
            .get(&self.escrow_id)
            .cloned()
        else {
            return Err(validation_err("escrow not found"));
        };
        if record.status != AssetEscrowStatus::Open {
            return Err(validation_err("only open escrows can be accepted"));
        }
        if &record.seller == authority {
            return Err(validation_err("seller cannot accept own escrow"));
        }
        record.buyer = Some(authority.clone());
        record.status = AssetEscrowStatus::Accepted;
        record.accepted_at_ms = Some(state_transaction.block_unix_timestamp_ms());
        state_transaction
            .world
            .insert_asset_escrow_entry(record.clone());
        state_transaction
            .world
            .emit_events(Some(EscrowEvent::Accepted(record)));
        Ok(())
    }
}

impl Execute for MarkEscrowPaymentSent {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let Some(mut record) = state_transaction
            .world
            .asset_escrows
            .get(&self.escrow_id)
            .cloned()
        else {
            return Err(validation_err("escrow not found"));
        };
        if record.status != AssetEscrowStatus::Accepted {
            return Err(validation_err("only accepted escrows can be marked paid"));
        }
        if record.buyer.as_ref() != Some(authority) {
            return Err(validation_err("only accepted buyer may mark payment sent"));
        }
        record.status = AssetEscrowStatus::PaymentSent;
        record.payment_sent_at_ms = Some(state_transaction.block_unix_timestamp_ms());
        state_transaction
            .world
            .insert_asset_escrow_entry(record.clone());
        state_transaction
            .world
            .emit_events(Some(EscrowEvent::PaymentSent(record)));
        Ok(())
    }
}

impl Execute for ReleaseAssetEscrow {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let Some(mut record) = state_transaction
            .world
            .asset_escrows
            .get(&self.escrow_id)
            .cloned()
        else {
            return Err(validation_err("escrow not found"));
        };
        if record.status != AssetEscrowStatus::PaymentSent {
            return Err(validation_err("only paid escrows can be released"));
        }
        if &record.seller != authority {
            return Err(validation_err("only seller may release escrow"));
        }
        let buyer = record
            .buyer
            .clone()
            .ok_or_else(|| validation_err("escrow buyer missing"))?;
        let escrow_asset = custody_asset(&record);
        let buyer_asset = party_asset(&record, &buyer);
        let delta = transfer_numeric_asset_for_escrow(
            state_transaction,
            &escrow_asset,
            &buyer_asset,
            &record.amount,
            NumericAssetTransferSourcePolicy::NativeEscrowCustody,
        )?;
        state_transaction.record_transfer_transcript(authority, delta)?;
        record.status = AssetEscrowStatus::Released;
        record.remaining_amount = Numeric::zero();
        record.closed_at_ms = Some(state_transaction.block_unix_timestamp_ms());
        state_transaction
            .world
            .insert_asset_escrow_entry(record.clone());
        state_transaction
            .world
            .emit_events(Some(EscrowEvent::Released(record)));
        Ok(())
    }
}

impl Execute for CancelAssetEscrow {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let Some(mut record) = state_transaction
            .world
            .asset_escrows
            .get(&self.escrow_id)
            .cloned()
        else {
            return Err(validation_err("escrow not found"));
        };
        if !matches!(
            record.status,
            AssetEscrowStatus::Open | AssetEscrowStatus::Accepted
        ) {
            return Err(validation_err(
                "escrow can only be cancelled before payment is marked",
            ));
        }
        if &record.seller != authority {
            return Err(validation_err("only seller may cancel escrow"));
        }
        let escrow_asset = custody_asset(&record);
        let seller_asset = party_asset(&record, &record.seller);
        let delta = transfer_numeric_asset_for_escrow(
            state_transaction,
            &escrow_asset,
            &seller_asset,
            &record.amount,
            NumericAssetTransferSourcePolicy::NativeEscrowCustody,
        )?;
        state_transaction.record_transfer_transcript(authority, delta)?;
        record.status = AssetEscrowStatus::Cancelled;
        record.remaining_amount = Numeric::zero();
        record.closed_at_ms = Some(state_transaction.block_unix_timestamp_ms());
        state_transaction
            .world
            .insert_asset_escrow_entry(record.clone());
        state_transaction
            .world
            .emit_events(Some(EscrowEvent::Cancelled(record)));
        Ok(())
    }
}

impl Execute for OpenEscrowDispute {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let Some(mut record) = state_transaction
            .world
            .asset_escrows
            .get(&self.escrow_id)
            .cloned()
        else {
            return Err(validation_err("escrow not found"));
        };
        if !matches!(
            record.status,
            AssetEscrowStatus::Accepted | AssetEscrowStatus::PaymentSent
        ) {
            return Err(validation_err(
                "only accepted or paid escrows can enter dispute",
            ));
        }
        let is_seller = &record.seller == authority;
        let is_buyer = record.buyer.as_ref() == Some(authority);
        if !(is_seller || is_buyer) {
            return Err(validation_err(
                "only escrow buyer or seller may open dispute",
            ));
        }
        record.status = AssetEscrowStatus::Disputed;
        record.disputed_at_ms = Some(state_transaction.block_unix_timestamp_ms());
        record
            .evidence_hashes
            .extend(self.evidence_hashes.iter().copied());
        state_transaction
            .world
            .insert_asset_escrow_entry(record.clone());
        state_transaction
            .world
            .emit_events(Some(EscrowEvent::Disputed(AssetEscrowDisputed {
                escrow: record,
                opened_by: authority.clone(),
                evidence_hashes: self.evidence_hashes,
            })));
        Ok(())
    }
}

impl Execute for ResolveEscrowDispute {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        if !has_permission(state_transaction, authority, CAN_RESOLVE_ESCROW_DISPUTE) {
            return Err(validation_err("not permitted: CanResolveEscrowDispute"));
        }
        let Some(mut record) = state_transaction
            .world
            .asset_escrows
            .get(&self.escrow_id)
            .cloned()
        else {
            return Err(validation_err("escrow not found"));
        };
        if record.status != AssetEscrowStatus::Disputed {
            return Err(validation_err("only disputed escrows can be resolved"));
        }
        ensure_resolution_split(&record.amount, &self.buyer_amount, &self.seller_amount)?;
        let buyer = record
            .buyer
            .clone()
            .ok_or_else(|| validation_err("escrow buyer missing"))?;
        let escrow_asset = custody_asset(&record);
        let mut deltas = Vec::new();
        if !self.buyer_amount.is_zero() {
            let buyer_asset = party_asset(&record, &buyer);
            let delta = transfer_numeric_asset_for_escrow(
                state_transaction,
                &escrow_asset,
                &buyer_asset,
                &self.buyer_amount,
                NumericAssetTransferSourcePolicy::NativeEscrowCustody,
            )?;
            deltas.push(delta);
        }
        if !self.seller_amount.is_zero() {
            let seller_asset = party_asset(&record, &record.seller);
            let delta = transfer_numeric_asset_for_escrow(
                state_transaction,
                &escrow_asset,
                &seller_asset,
                &self.seller_amount,
                NumericAssetTransferSourcePolicy::NativeEscrowCustody,
            )?;
            deltas.push(delta);
        }
        state_transaction.record_transfer_transcripts(authority, deltas)?;
        let resolved_at_ms = state_transaction.block_unix_timestamp_ms();
        record.status = AssetEscrowStatus::Resolved;
        record.remaining_amount = Numeric::zero();
        record.closed_at_ms = Some(resolved_at_ms);
        record.resolution = Some(AssetEscrowResolution {
            resolver: authority.clone(),
            buyer_amount: self.buyer_amount.clone(),
            seller_amount: self.seller_amount.clone(),
            evidence_hashes: self.evidence_hashes.clone(),
            resolved_at_ms,
        });
        state_transaction
            .world
            .insert_asset_escrow_entry(record.clone());
        state_transaction
            .world
            .emit_events(Some(EscrowEvent::Resolved(AssetEscrowResolved {
                escrow: record,
                resolver: authority.clone(),
                buyer_amount: self.buyer_amount,
                seller_amount: self.seller_amount,
            })));
        Ok(())
    }
}

impl Execute for OpenAssetLock {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        if state_transaction
            .world
            .asset_escrows
            .get(&self.escrow_id)
            .is_some()
            || state_transaction
                .world
                .anonymous_asset_escrows
                .get(&self.escrow_id)
                .is_some()
        {
            return Err(validation_err("escrow already exists"));
        }

        ensure_positive(&self.amount)?;
        let spec = state_transaction
            .numeric_spec_for(&self.asset_definition)
            .map_err(Error::from)?;
        assert_numeric_spec_with(&self.amount, spec)?;
        state_transaction.world.account(authority)?;
        state_transaction.world.account(&self.destination)?;
        if let Some(release_authority) = self.release_authority.as_ref() {
            state_transaction.world.account(release_authority)?;
        }
        state_transaction
            .world
            .asset_definition(&self.asset_definition)?;

        let custody = escrow_custody_account_id(
            state_transaction.chain_id(),
            &self.escrow_id,
            &self.asset_definition,
        );
        let source_asset = AssetId::new(self.asset_definition.clone(), authority.clone());
        let custody_asset = AssetId::new(self.asset_definition.clone(), custody.clone());
        let custody_created = ensure_custody_account(&custody, state_transaction)?;
        let transfer_result = transfer_numeric_asset_for_escrow(
            state_transaction,
            &source_asset,
            &custody_asset,
            &self.amount,
            NumericAssetTransferSourcePolicy::User,
        );
        if transfer_result.is_err() && custody_created {
            state_transaction.world.accounts.remove(custody.clone());
        }
        let delta = transfer_result?;
        state_transaction.record_transfer_transcript(authority, delta)?;

        let record = AssetEscrowRecord {
            id: self.escrow_id,
            seller: authority.clone(),
            buyer: Some(self.destination),
            asset_definition: self.asset_definition,
            amount: self.amount.clone(),
            custody,
            status: AssetEscrowStatus::Locked,
            kind: AssetEscrowKind::Lock,
            remaining_amount: self.amount,
            release_authority: self.release_authority,
            expires_at_ms: self.expires_at_ms,
            evidence_hashes: self.evidence_hashes,
            created_at_ms: state_transaction.block_unix_timestamp_ms(),
            accepted_at_ms: None,
            payment_sent_at_ms: None,
            disputed_at_ms: None,
            closed_at_ms: None,
            resolution: None,
        };
        state_transaction
            .world
            .insert_asset_escrow_entry(record.clone());
        state_transaction
            .world
            .emit_events(Some(EscrowEvent::Opened(record)));
        Ok(())
    }
}

impl Execute for DrawdownAssetLock {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        ensure_positive(&self.amount)?;
        let Some(mut record) = state_transaction
            .world
            .asset_escrows
            .get(&self.escrow_id)
            .cloned()
        else {
            return Err(validation_err("escrow not found"));
        };
        ensure_asset_lock(&record)?;
        if record.status != AssetEscrowStatus::Locked {
            return Err(validation_err("only locked asset locks can be drawn down"));
        }
        if let Some(release_authority) = record.release_authority.as_ref() {
            if release_authority != authority {
                return Err(validation_err("only release authority may draw down lock"));
            }
        } else if record.buyer.as_ref() != Some(authority) {
            return Err(validation_err("only lock destination may draw down lock"));
        }
        let spec = state_transaction
            .numeric_spec_for(&record.asset_definition)
            .map_err(Error::from)?;
        assert_numeric_spec_with(&self.amount, spec)?;
        if self.amount > record.remaining_amount {
            return Err(validation_err("lock drawdown exceeds remaining amount"));
        }

        let destination = record
            .buyer
            .clone()
            .ok_or_else(|| validation_err("lock destination missing"))?;
        let custody_asset = custody_asset(&record);
        let destination_asset = party_asset(&record, &destination);
        let delta = transfer_numeric_asset_for_escrow(
            state_transaction,
            &custody_asset,
            &destination_asset,
            &self.amount,
            NumericAssetTransferSourcePolicy::NativeEscrowCustody,
        )?;
        state_transaction.record_transfer_transcript(authority, delta)?;
        record.remaining_amount = record
            .remaining_amount
            .checked_sub(self.amount)
            .ok_or_else(|| validation_err("lock remaining amount underflow"))?;
        if record.remaining_amount.is_zero() {
            record.status = AssetEscrowStatus::DrawnDown;
            record.closed_at_ms = Some(state_transaction.block_unix_timestamp_ms());
        }
        state_transaction
            .world
            .insert_asset_escrow_entry(record.clone());
        state_transaction
            .world
            .emit_events(Some(EscrowEvent::Released(record)));
        Ok(())
    }
}

impl Execute for CancelAssetLock {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let Some(mut record) = state_transaction
            .world
            .asset_escrows
            .get(&self.escrow_id)
            .cloned()
        else {
            return Err(validation_err("escrow not found"));
        };
        ensure_asset_lock(&record)?;
        if record.status != AssetEscrowStatus::Locked {
            return Err(validation_err("only locked asset locks can be cancelled"));
        }
        if &record.seller != authority {
            return Err(validation_err("only lock opener may cancel lock"));
        }
        let custody_asset = custody_asset(&record);
        let seller_asset = party_asset(&record, &record.seller);
        let delta = transfer_numeric_asset_for_escrow(
            state_transaction,
            &custody_asset,
            &seller_asset,
            &record.remaining_amount,
            NumericAssetTransferSourcePolicy::NativeEscrowCustody,
        )?;
        state_transaction.record_transfer_transcript(authority, delta)?;
        record.status = AssetEscrowStatus::Cancelled;
        record.remaining_amount = Numeric::zero();
        record.closed_at_ms = Some(state_transaction.block_unix_timestamp_ms());
        state_transaction
            .world
            .insert_asset_escrow_entry(record.clone());
        state_transaction
            .world
            .emit_events(Some(EscrowEvent::Cancelled(record)));
        Ok(())
    }
}

impl Execute for ExpireAssetLock {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let Some(mut record) = state_transaction
            .world
            .asset_escrows
            .get(&self.escrow_id)
            .cloned()
        else {
            return Err(validation_err("escrow not found"));
        };
        ensure_asset_lock(&record)?;
        if record.status != AssetEscrowStatus::Locked {
            return Err(validation_err("only locked asset locks can expire"));
        }
        let expires_at_ms = record
            .expires_at_ms
            .ok_or_else(|| validation_err("asset lock has no expiry"))?;
        if state_transaction.block_unix_timestamp_ms() < expires_at_ms {
            return Err(validation_err("asset lock expiry has not been reached"));
        }
        let custody_asset = custody_asset(&record);
        let seller_asset = party_asset(&record, &record.seller);
        let delta = transfer_numeric_asset_for_escrow(
            state_transaction,
            &custody_asset,
            &seller_asset,
            &record.remaining_amount,
            NumericAssetTransferSourcePolicy::NativeEscrowCustody,
        )?;
        state_transaction.record_transfer_transcript(authority, delta)?;
        record.status = AssetEscrowStatus::Expired;
        record.remaining_amount = Numeric::zero();
        record.closed_at_ms = Some(state_transaction.block_unix_timestamp_ms());
        state_transaction
            .world
            .insert_asset_escrow_entry(record.clone());
        state_transaction
            .world
            .emit_events(Some(EscrowEvent::Expired(record)));
        Ok(())
    }
}

impl Execute for OpenAnonymousAssetEscrow {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        if state_transaction
            .world
            .asset_escrows
            .get(&self.escrow_id)
            .is_some()
            || state_transaction
                .world
                .anonymous_asset_escrows
                .get(&self.escrow_id)
                .is_some()
        {
            return Err(validation_err("escrow already exists"));
        }
        state_transaction.world.account(authority)?;
        ensure_non_zero_bytes("escrow commitment", &self.escrow_commitment)?;

        let opening = execute_anonymous_escrow_transfer(
            authority,
            state_transaction,
            self.asset_definition.clone(),
            self.funding_nullifiers,
            vec![self.escrow_commitment],
            self.proof,
            self.root_hint,
        )?;
        let created_at_ms = state_transaction.block_unix_timestamp_ms();
        let record = AnonymousAssetEscrowRecord {
            id: self.escrow_id,
            seller: authority.clone(),
            buyer: None,
            asset_definition: self.asset_definition,
            escrow_commitment: self.escrow_commitment,
            status: AssetEscrowStatus::Open,
            evidence_hashes: self.evidence_hashes,
            opening,
            release: None,
            cancellation: None,
            created_at_ms,
            accepted_at_ms: None,
            payment_sent_at_ms: None,
            disputed_at_ms: None,
            closed_at_ms: None,
            resolution: None,
        };
        state_transaction
            .world
            .insert_anonymous_asset_escrow_entry(record);
        Ok(())
    }
}

impl Execute for AcceptAnonymousAssetEscrow {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        state_transaction.world.account(authority)?;
        let mut record = anonymous_escrow_record(state_transaction, &self.escrow_id)?;
        if record.status != AssetEscrowStatus::Open {
            return Err(validation_err("only open escrows can be accepted"));
        }
        if &record.seller == authority {
            return Err(validation_err("seller cannot accept own escrow"));
        }
        record.buyer = Some(authority.clone());
        record.status = AssetEscrowStatus::Accepted;
        record.accepted_at_ms = Some(state_transaction.block_unix_timestamp_ms());
        state_transaction
            .world
            .insert_anonymous_asset_escrow_entry(record);
        Ok(())
    }
}

impl Execute for MarkAnonymousEscrowPaymentSent {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let mut record = anonymous_escrow_record(state_transaction, &self.escrow_id)?;
        if record.status != AssetEscrowStatus::Accepted {
            return Err(validation_err("only accepted escrows can be marked paid"));
        }
        if record.buyer.as_ref() != Some(authority) {
            return Err(validation_err("only accepted buyer may mark payment sent"));
        }
        record.status = AssetEscrowStatus::PaymentSent;
        record.payment_sent_at_ms = Some(state_transaction.block_unix_timestamp_ms());
        state_transaction
            .world
            .insert_anonymous_asset_escrow_entry(record);
        Ok(())
    }
}

impl Execute for ReleaseAnonymousAssetEscrow {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let mut record = anonymous_escrow_record(state_transaction, &self.escrow_id)?;
        if record.status != AssetEscrowStatus::PaymentSent {
            return Err(validation_err("only paid escrows can be released"));
        }
        if &record.seller != authority {
            return Err(validation_err("only seller may release escrow"));
        }
        if record.buyer.is_none() {
            return Err(validation_err("escrow buyer missing"));
        }
        ensure_single_escrow_nullifier(&self.escrow_nullifiers)?;
        ensure_unique_non_zero_bytes("buyer output commitment", &self.buyer_output_commitments)?;
        ensure_close_proof_spends_escrow_commitment(&self.proof, record.escrow_commitment)?;
        let release = execute_anonymous_escrow_transfer(
            authority,
            state_transaction,
            record.asset_definition.clone(),
            self.escrow_nullifiers,
            self.buyer_output_commitments,
            self.proof,
            self.root_hint,
        )?;
        record.status = AssetEscrowStatus::Released;
        record.closed_at_ms = Some(state_transaction.block_unix_timestamp_ms());
        record.release = Some(release);
        state_transaction
            .world
            .insert_anonymous_asset_escrow_entry(record);
        Ok(())
    }
}

impl Execute for CancelAnonymousAssetEscrow {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let mut record = anonymous_escrow_record(state_transaction, &self.escrow_id)?;
        if !matches!(
            record.status,
            AssetEscrowStatus::Open | AssetEscrowStatus::Accepted
        ) {
            return Err(validation_err(
                "escrow can only be cancelled before payment is marked",
            ));
        }
        if &record.seller != authority {
            return Err(validation_err("only seller may cancel escrow"));
        }
        ensure_single_escrow_nullifier(&self.escrow_nullifiers)?;
        ensure_unique_non_zero_bytes("seller output commitment", &self.seller_output_commitments)?;
        ensure_close_proof_spends_escrow_commitment(&self.proof, record.escrow_commitment)?;
        let cancellation = execute_anonymous_escrow_transfer(
            authority,
            state_transaction,
            record.asset_definition.clone(),
            self.escrow_nullifiers,
            self.seller_output_commitments,
            self.proof,
            self.root_hint,
        )?;
        record.status = AssetEscrowStatus::Cancelled;
        record.closed_at_ms = Some(state_transaction.block_unix_timestamp_ms());
        record.cancellation = Some(cancellation);
        state_transaction
            .world
            .insert_anonymous_asset_escrow_entry(record);
        Ok(())
    }
}

impl Execute for OpenAnonymousEscrowDispute {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let mut record = anonymous_escrow_record(state_transaction, &self.escrow_id)?;
        if !matches!(
            record.status,
            AssetEscrowStatus::Accepted | AssetEscrowStatus::PaymentSent
        ) {
            return Err(validation_err(
                "only accepted or paid escrows can enter dispute",
            ));
        }
        let is_seller = &record.seller == authority;
        let is_buyer = record.buyer.as_ref() == Some(authority);
        if !(is_seller || is_buyer) {
            return Err(validation_err(
                "only escrow buyer or seller may open dispute",
            ));
        }
        record.status = AssetEscrowStatus::Disputed;
        record.disputed_at_ms = Some(state_transaction.block_unix_timestamp_ms());
        record
            .evidence_hashes
            .extend(self.evidence_hashes.iter().copied());
        state_transaction
            .world
            .insert_anonymous_asset_escrow_entry(record);
        Ok(())
    }
}

impl Execute for ResolveAnonymousEscrowDispute {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        if !has_permission(state_transaction, authority, CAN_RESOLVE_ESCROW_DISPUTE) {
            return Err(validation_err("not permitted: CanResolveEscrowDispute"));
        }
        let mut record = anonymous_escrow_record(state_transaction, &self.escrow_id)?;
        if record.status != AssetEscrowStatus::Disputed {
            return Err(validation_err("only disputed escrows can be resolved"));
        }
        if record.buyer.is_none() {
            return Err(validation_err("escrow buyer missing"));
        }
        ensure_single_escrow_nullifier(&self.escrow_nullifiers)?;
        if self.buyer_output_commitments.is_empty() && self.seller_output_commitments.is_empty() {
            return Err(validation_err(
                "court resolution must create at least one shielded output",
            ));
        }
        ensure_optional_unique_non_zero_bytes(
            "buyer output commitment",
            &self.buyer_output_commitments,
        )?;
        ensure_optional_unique_non_zero_bytes(
            "seller output commitment",
            &self.seller_output_commitments,
        )?;
        let mut outputs = self.buyer_output_commitments.clone();
        outputs.extend(self.seller_output_commitments.iter().copied());
        ensure_unique_non_zero_bytes("resolution output commitment", &outputs)?;
        ensure_close_proof_spends_escrow_commitment(&self.proof, record.escrow_commitment)?;
        let proof = execute_anonymous_escrow_transfer(
            authority,
            state_transaction,
            record.asset_definition.clone(),
            self.escrow_nullifiers,
            outputs,
            self.proof,
            self.root_hint,
        )?;
        let resolved_at_ms = state_transaction.block_unix_timestamp_ms();
        record.status = AssetEscrowStatus::Resolved;
        record.closed_at_ms = Some(resolved_at_ms);
        record.resolution = Some(AnonymousAssetEscrowResolution {
            resolver: authority.clone(),
            buyer_output_commitments: self.buyer_output_commitments,
            seller_output_commitments: self.seller_output_commitments,
            proof,
            evidence_hashes: self.evidence_hashes,
            resolved_at_ms,
        });
        state_transaction
            .world
            .insert_anonymous_asset_escrow_entry(record);
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum AssetEscrowAccountIndex {
    Seller,
    Buyer,
}

fn asset_escrow_account_index(field: &str) -> Option<AssetEscrowAccountIndex> {
    match field {
        "seller" => Some(AssetEscrowAccountIndex::Seller),
        "buyer" => Some(AssetEscrowAccountIndex::Buyer),
        _ => None,
    }
}

fn asset_escrow_id_from_value(value: &Value) -> Option<EscrowId> {
    norito::json::from_value(value.clone()).ok()
}

fn account_id_from_value(value: &Value) -> Option<AccountId> {
    norito::json::from_value(value.clone()).ok()
}

fn asset_escrow_status_from_value(value: &Value) -> Option<AssetEscrowStatus> {
    norito::json::from_value(value.clone()).ok()
}

fn intersect_escrow_candidate_ids(
    best: &mut Option<BTreeSet<EscrowId>>,
    candidates: BTreeSet<EscrowId>,
) {
    let Some(current) = best.take() else {
        *best = Some(candidates);
        return;
    };
    *best = Some(current.intersection(&candidates).copied().collect());
}

fn asset_escrow_ids_for_accounts(
    world: &impl WorldReadOnly,
    index: AssetEscrowAccountIndex,
    accounts: impl IntoIterator<Item = AccountId>,
) -> BTreeSet<EscrowId> {
    let mut ids = BTreeSet::new();
    for account_id in accounts {
        let escrow_ids = match index {
            AssetEscrowAccountIndex::Seller => world.asset_escrows_by_seller().get(&account_id),
            AssetEscrowAccountIndex::Buyer => world.asset_escrows_by_buyer().get(&account_id),
        };
        if let Some(escrow_ids) = escrow_ids {
            ids.extend(escrow_ids.iter().copied());
        }
    }
    ids
}

fn anonymous_asset_escrow_ids_for_accounts(
    world: &impl WorldReadOnly,
    index: AssetEscrowAccountIndex,
    accounts: impl IntoIterator<Item = AccountId>,
) -> BTreeSet<EscrowId> {
    let mut ids = BTreeSet::new();
    for account_id in accounts {
        let escrow_ids = match index {
            AssetEscrowAccountIndex::Seller => {
                world.anonymous_asset_escrows_by_seller().get(&account_id)
            }
            AssetEscrowAccountIndex::Buyer => {
                world.anonymous_asset_escrows_by_buyer().get(&account_id)
            }
        };
        if let Some(escrow_ids) = escrow_ids {
            ids.extend(escrow_ids.iter().copied());
        }
    }
    ids
}

fn asset_escrow_ids_for_statuses(
    world: &impl WorldReadOnly,
    statuses: impl IntoIterator<Item = AssetEscrowStatus>,
) -> BTreeSet<EscrowId> {
    let mut ids = BTreeSet::new();
    for status in statuses {
        if let Some(escrow_ids) = world.asset_escrows_by_status().get(&status) {
            ids.extend(escrow_ids.iter().copied());
        }
    }
    ids
}

fn anonymous_asset_escrow_ids_for_statuses(
    world: &impl WorldReadOnly,
    statuses: impl IntoIterator<Item = AssetEscrowStatus>,
) -> BTreeSet<EscrowId> {
    let mut ids = BTreeSet::new();
    for status in statuses {
        if let Some(escrow_ids) = world.anonymous_asset_escrows_by_status().get(&status) {
            ids.extend(escrow_ids.iter().copied());
        }
    }
    ids
}

fn asset_escrow_candidate_ids(
    predicate: &PredicateJson,
    world: &impl WorldReadOnly,
) -> Option<BTreeSet<EscrowId>> {
    let mut best = None;

    for cond in &predicate.equals {
        if cond.field == "id" {
            intersect_escrow_candidate_ids(
                &mut best,
                asset_escrow_id_from_value(&cond.value)
                    .into_iter()
                    .collect(),
            );
            continue;
        }
        if let Some(index) = asset_escrow_account_index(&cond.field) {
            intersect_escrow_candidate_ids(
                &mut best,
                asset_escrow_ids_for_accounts(world, index, account_id_from_value(&cond.value)),
            );
            continue;
        }
        if cond.field == "status" {
            intersect_escrow_candidate_ids(
                &mut best,
                asset_escrow_ids_for_statuses(world, asset_escrow_status_from_value(&cond.value)),
            );
        }
    }

    for cond in &predicate.r#in {
        if cond.field == "id" {
            intersect_escrow_candidate_ids(
                &mut best,
                cond.values
                    .iter()
                    .filter_map(asset_escrow_id_from_value)
                    .collect(),
            );
            continue;
        }
        if let Some(index) = asset_escrow_account_index(&cond.field) {
            intersect_escrow_candidate_ids(
                &mut best,
                asset_escrow_ids_for_accounts(
                    world,
                    index,
                    cond.values.iter().filter_map(account_id_from_value),
                ),
            );
            continue;
        }
        if cond.field == "status" {
            intersect_escrow_candidate_ids(
                &mut best,
                asset_escrow_ids_for_statuses(
                    world,
                    cond.values
                        .iter()
                        .filter_map(asset_escrow_status_from_value),
                ),
            );
        }
    }

    best
}

fn anonymous_asset_escrow_candidate_ids(
    predicate: &PredicateJson,
    world: &impl WorldReadOnly,
) -> Option<BTreeSet<EscrowId>> {
    let mut best = None;

    for cond in &predicate.equals {
        if cond.field == "id" {
            intersect_escrow_candidate_ids(
                &mut best,
                asset_escrow_id_from_value(&cond.value)
                    .into_iter()
                    .collect(),
            );
            continue;
        }
        if let Some(index) = asset_escrow_account_index(&cond.field) {
            intersect_escrow_candidate_ids(
                &mut best,
                anonymous_asset_escrow_ids_for_accounts(
                    world,
                    index,
                    account_id_from_value(&cond.value),
                ),
            );
            continue;
        }
        if cond.field == "status" {
            intersect_escrow_candidate_ids(
                &mut best,
                anonymous_asset_escrow_ids_for_statuses(
                    world,
                    asset_escrow_status_from_value(&cond.value),
                ),
            );
        }
    }

    for cond in &predicate.r#in {
        if cond.field == "id" {
            intersect_escrow_candidate_ids(
                &mut best,
                cond.values
                    .iter()
                    .filter_map(asset_escrow_id_from_value)
                    .collect(),
            );
            continue;
        }
        if let Some(index) = asset_escrow_account_index(&cond.field) {
            intersect_escrow_candidate_ids(
                &mut best,
                anonymous_asset_escrow_ids_for_accounts(
                    world,
                    index,
                    cond.values.iter().filter_map(account_id_from_value),
                ),
            );
            continue;
        }
        if cond.field == "status" {
            intersect_escrow_candidate_ids(
                &mut best,
                anonymous_asset_escrow_ids_for_statuses(
                    world,
                    cond.values
                        .iter()
                        .filter_map(asset_escrow_status_from_value),
                ),
            );
        }
    }

    best
}

impl ValidQuery for FindAssetEscrows {
    fn execute(
        self,
        filter: CompoundPredicate<AssetEscrowRecord>,
        state_ro: &impl StateReadOnly,
    ) -> Result<impl Iterator<Item = AssetEscrowRecord>, QueryExecutionFail> {
        let world = state_ro.world();
        let predicate_json = filter
            .json_payload()
            .and_then(|raw| norito::json::from_str::<PredicateJson>(raw).ok());
        if let Some(candidate_ids) = predicate_json
            .as_ref()
            .and_then(|predicate| asset_escrow_candidate_ids(predicate, world))
        {
            let iter: Box<dyn Iterator<Item = AssetEscrowRecord> + '_> =
                Box::new(candidate_ids.into_iter().filter_map(move |escrow_id| {
                    world
                        .asset_escrows()
                        .get(&escrow_id)
                        .filter(|record| filter.applies(*record))
                        .cloned()
                }));
            return Ok(iter);
        }

        let iter: Box<dyn Iterator<Item = AssetEscrowRecord> + '_> = Box::new(
            world
                .asset_escrows()
                .iter()
                .filter_map(move |(_, record)| filter.applies(record).then(|| record.clone())),
        );
        Ok(iter)
    }
}

impl ValidSingularQuery for FindAssetEscrowById {
    fn execute(
        &self,
        state_ro: &impl StateReadOnly,
    ) -> Result<AssetEscrowRecord, QueryExecutionFail> {
        state_ro
            .world()
            .asset_escrows()
            .get(&self.escrow_id)
            .cloned()
            .ok_or_else(|| QueryExecutionFail::Find(FindError::AssetEscrow(self.escrow_id)))
    }
}

impl ValidQuery for FindAssetEscrowsBySeller {
    fn execute(
        self,
        filter: CompoundPredicate<AssetEscrowRecord>,
        state_ro: &impl StateReadOnly,
    ) -> Result<impl Iterator<Item = AssetEscrowRecord>, QueryExecutionFail> {
        let seller = self.seller;
        let world = state_ro.world();
        Ok(world
            .asset_escrows_by_seller()
            .get(&seller)
            .into_iter()
            .flat_map(BTreeSet::iter)
            .filter_map(move |escrow_id| world.asset_escrows().get(escrow_id).cloned())
            .filter(move |record| filter.applies(record)))
    }
}

impl ValidQuery for FindAssetEscrowsByBuyer {
    fn execute(
        self,
        filter: CompoundPredicate<AssetEscrowRecord>,
        state_ro: &impl StateReadOnly,
    ) -> Result<impl Iterator<Item = AssetEscrowRecord>, QueryExecutionFail> {
        let buyer = self.buyer;
        let world = state_ro.world();
        Ok(world
            .asset_escrows_by_buyer()
            .get(&buyer)
            .into_iter()
            .flat_map(BTreeSet::iter)
            .filter_map(move |escrow_id| world.asset_escrows().get(escrow_id).cloned())
            .filter(move |record| filter.applies(record)))
    }
}

impl ValidQuery for FindAssetEscrowsByStatus {
    fn execute(
        self,
        filter: CompoundPredicate<AssetEscrowRecord>,
        state_ro: &impl StateReadOnly,
    ) -> Result<impl Iterator<Item = AssetEscrowRecord>, QueryExecutionFail> {
        let status = self.status;
        let world = state_ro.world();
        Ok(world
            .asset_escrows_by_status()
            .get(&status)
            .into_iter()
            .flat_map(BTreeSet::iter)
            .filter_map(move |escrow_id| world.asset_escrows().get(escrow_id).cloned())
            .filter(move |record| filter.applies(record)))
    }
}

impl ValidQuery for FindAnonymousAssetEscrows {
    fn execute(
        self,
        filter: CompoundPredicate<AnonymousAssetEscrowRecord>,
        state_ro: &impl StateReadOnly,
    ) -> Result<impl Iterator<Item = AnonymousAssetEscrowRecord>, QueryExecutionFail> {
        let world = state_ro.world();
        let predicate_json = filter
            .json_payload()
            .and_then(|raw| norito::json::from_str::<PredicateJson>(raw).ok());
        if let Some(candidate_ids) = predicate_json
            .as_ref()
            .and_then(|predicate| anonymous_asset_escrow_candidate_ids(predicate, world))
        {
            let iter: Box<dyn Iterator<Item = AnonymousAssetEscrowRecord> + '_> =
                Box::new(candidate_ids.into_iter().filter_map(move |escrow_id| {
                    world
                        .anonymous_asset_escrows()
                        .get(&escrow_id)
                        .filter(|record| filter.applies(*record))
                        .cloned()
                }));
            return Ok(iter);
        }

        let iter: Box<dyn Iterator<Item = AnonymousAssetEscrowRecord> + '_> = Box::new(
            world
                .anonymous_asset_escrows()
                .iter()
                .filter_map(move |(_, record)| filter.applies(record).then(|| record.clone())),
        );
        Ok(iter)
    }
}

impl ValidSingularQuery for FindAnonymousAssetEscrowById {
    fn execute(
        &self,
        state_ro: &impl StateReadOnly,
    ) -> Result<AnonymousAssetEscrowRecord, QueryExecutionFail> {
        state_ro
            .world()
            .anonymous_asset_escrows()
            .get(&self.escrow_id)
            .cloned()
            .ok_or_else(|| QueryExecutionFail::Find(FindError::AssetEscrow(self.escrow_id)))
    }
}

impl ValidQuery for FindAnonymousAssetEscrowsBySeller {
    fn execute(
        self,
        filter: CompoundPredicate<AnonymousAssetEscrowRecord>,
        state_ro: &impl StateReadOnly,
    ) -> Result<impl Iterator<Item = AnonymousAssetEscrowRecord>, QueryExecutionFail> {
        let seller = self.seller;
        let world = state_ro.world();
        Ok(world
            .anonymous_asset_escrows_by_seller()
            .get(&seller)
            .into_iter()
            .flat_map(BTreeSet::iter)
            .filter_map(move |escrow_id| world.anonymous_asset_escrows().get(escrow_id).cloned())
            .filter(move |record| filter.applies(record)))
    }
}

impl ValidQuery for FindAnonymousAssetEscrowsByBuyer {
    fn execute(
        self,
        filter: CompoundPredicate<AnonymousAssetEscrowRecord>,
        state_ro: &impl StateReadOnly,
    ) -> Result<impl Iterator<Item = AnonymousAssetEscrowRecord>, QueryExecutionFail> {
        let buyer = self.buyer;
        let world = state_ro.world();
        Ok(world
            .anonymous_asset_escrows_by_buyer()
            .get(&buyer)
            .into_iter()
            .flat_map(BTreeSet::iter)
            .filter_map(move |escrow_id| world.anonymous_asset_escrows().get(escrow_id).cloned())
            .filter(move |record| filter.applies(record)))
    }
}

impl ValidQuery for FindAnonymousAssetEscrowsByStatus {
    fn execute(
        self,
        filter: CompoundPredicate<AnonymousAssetEscrowRecord>,
        state_ro: &impl StateReadOnly,
    ) -> Result<impl Iterator<Item = AnonymousAssetEscrowRecord>, QueryExecutionFail> {
        let status = self.status;
        let world = state_ro.world();
        Ok(world
            .anonymous_asset_escrows_by_status()
            .get(&status)
            .into_iter()
            .flat_map(BTreeSet::iter)
            .filter_map(move |escrow_id| world.anonymous_asset_escrows().get(escrow_id).cloned())
            .filter(move |record| filter.applies(record)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{kura::Kura, query::store::LiveQueryStore, state::State};
    use iroha_data_model::{
        asset::{
            ASSET_ISSUER_USAGE_POLICY_METADATA_KEY, AssetIssuerUsagePolicyV1,
            AssetSubjectBindingV1, definition::AssetConfidentialPolicy,
        },
        events::{EventBox, data::prelude as data_pre},
        isi::SetAssetTransferFreeze,
        permission::Permissions,
    };
    use iroha_executor_data_model::permission::{Permission as _, escrow::CanResolveEscrowDispute};
    use iroha_primitives::json::Json;
    use std::collections::BTreeMap;

    fn fixture_account(label: &str) -> AccountId {
        let seed: Vec<u8> = label.as_bytes().iter().copied().cycle().take(32).collect();
        let (public_key, _) = KeyPair::from_seed(seed, Algorithm::Ed25519).into_parts();
        AccountId::new(public_key)
    }

    fn fixture_asset_definition_id() -> AssetDefinitionId {
        AssetDefinitionId::new(
            DomainId::try_new("aitai", "universal").expect("domain id"),
            "xor".parse().expect("asset name"),
        )
    }

    fn fixture_escrow_id(label: &str) -> EscrowId {
        EscrowId::new(Hash::new(format!("native-escrow-test:{label}")))
    }

    fn block_header(timestamp_ms: u64) -> iroha_data_model::block::BlockHeader {
        iroha_data_model::block::BlockHeader::new(
            nonzero_ext::nonzero!(1_u64),
            None,
            None,
            None,
            timestamp_ms,
            0,
        )
    }

    fn seed_test_call_hash(state_transaction: &mut StateTransaction<'_, '_>, byte: u8) {
        state_transaction.tx_call_hash = Some(Hash::prehashed([byte; Hash::LENGTH]));
    }

    fn state_with_parties(
        seller: &AccountId,
        buyer: &AccountId,
        court: &AccountId,
        asset_definition: &AssetDefinitionId,
        seller_balance: Numeric,
    ) -> State {
        let asset_definition_entry = AssetDefinition::numeric(asset_definition.clone())
            .with_name("XOR".to_owned())
            .build(seller);
        state_with_parties_and_definition(
            seller,
            buyer,
            court,
            asset_definition,
            asset_definition_entry,
            seller_balance,
        )
    }

    fn state_with_parties_and_definition(
        seller: &AccountId,
        buyer: &AccountId,
        court: &AccountId,
        asset_definition: &AssetDefinitionId,
        asset_definition_entry: AssetDefinition,
        seller_balance: Numeric,
    ) -> State {
        let domain = Domain::new(asset_definition.domain().clone()).build(seller);
        let seller_asset_id = AssetId::of(asset_definition.clone(), seller.clone());
        let seller_asset = Asset::new(seller_asset_id, seller_balance);
        let world = crate::state::World::with_assets(
            [domain],
            [
                Account::new(seller.clone()).build(seller),
                Account::new(buyer.clone()).build(buyer),
                Account::new(court.clone()).build(court),
            ],
            [asset_definition_entry],
            [seller_asset],
            [],
        );
        State::new_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        )
    }

    fn balance(
        state_transaction: &StateTransaction<'_, '_>,
        account: &AccountId,
        asset_definition: &AssetDefinitionId,
    ) -> Numeric {
        let asset_id = AssetId::of(asset_definition.clone(), account.clone());
        state_transaction
            .world
            .assets
            .get(&asset_id)
            .map(|value| value.as_ref().clone())
            .unwrap_or_else(Numeric::zero)
    }

    fn asset_transfer_events(events: &[EventBox]) -> Vec<(&'static str, AssetId, Numeric)> {
        events
            .iter()
            .filter_map(|event| {
                let EventBox::Data(data_event) = event else {
                    return None;
                };
                let data_pre::DataEvent::Domain(data_pre::DomainEvent::Account(
                    data_pre::AccountEvent::Asset(asset_event),
                )) = data_event.as_ref()
                else {
                    return None;
                };
                match asset_event {
                    data_pre::AssetEvent::Removed(changed) => {
                        Some(("removed", changed.asset().clone(), changed.amount().clone()))
                    }
                    data_pre::AssetEvent::Added(changed) => {
                        Some(("added", changed.asset().clone(), changed.amount().clone()))
                    }
                    _ => None,
                }
            })
            .collect()
    }

    fn assert_asset_transfer_event(
        events: &[(&'static str, AssetId, Numeric)],
        kind: &'static str,
        asset: &AssetId,
        amount: &Numeric,
    ) {
        assert!(
            events
                .iter()
                .any(|(event_kind, event_asset, event_amount)| {
                    *event_kind == kind && event_asset == asset && event_amount == amount
                }),
            "missing {kind} event for {asset} amount {amount}; events: {events:?}"
        );
    }

    fn assert_transfer_delta(
        delta: &TransferDeltaTranscript,
        from: &AccountId,
        to: &AccountId,
        asset_definition: &AssetDefinitionId,
        amount: &Numeric,
    ) {
        assert_eq!(&delta.from_account, from);
        assert_eq!(&delta.to_account, to);
        assert_eq!(&delta.asset_definition, asset_definition);
        assert_eq!(&delta.amount, amount);
    }

    fn escrow_record(
        state_transaction: &StateTransaction<'_, '_>,
        escrow_id: &EscrowId,
    ) -> AssetEscrowRecord {
        state_transaction
            .world
            .asset_escrows
            .get(escrow_id)
            .cloned()
            .expect("escrow record")
    }

    fn query_asset_escrow_ids(
        state_transaction: &StateTransaction<'_, '_>,
        predicate: CompoundPredicate<AssetEscrowRecord>,
    ) -> Vec<EscrowId> {
        FindAssetEscrows
            .execute(predicate, state_transaction)
            .expect("query asset escrows")
            .map(|record| record.id)
            .collect()
    }

    fn query_anonymous_asset_escrow_ids(
        state_transaction: &StateTransaction<'_, '_>,
        predicate: CompoundPredicate<AnonymousAssetEscrowRecord>,
    ) -> Vec<EscrowId> {
        FindAnonymousAssetEscrows
            .execute(predicate, state_transaction)
            .expect("query anonymous asset escrows")
            .map(|record| record.id)
            .collect()
    }

    fn parse_predicate_json<T>(
        predicate: &CompoundPredicate<T>,
    ) -> iroha_data_model::query::json::PredicateJson {
        predicate
            .json_payload()
            .and_then(|raw| {
                norito::json::from_str::<iroha_data_model::query::json::PredicateJson>(raw).ok()
            })
            .expect("predicate JSON")
    }

    fn asset_escrow_fixture(
        escrow_id: EscrowId,
        seller: AccountId,
        buyer: Option<AccountId>,
        asset_definition: AssetDefinitionId,
        status: AssetEscrowStatus,
    ) -> AssetEscrowRecord {
        AssetEscrowRecord {
            id: escrow_id,
            seller,
            buyer,
            asset_definition,
            amount: Numeric::new(1_u32, 0),
            custody: fixture_account("asset-escrow-custody"),
            status,
            kind: AssetEscrowKind::Marketplace,
            remaining_amount: Numeric::new(1_u32, 0),
            release_authority: None,
            expires_at_ms: None,
            evidence_hashes: Vec::new(),
            created_at_ms: 1,
            accepted_at_ms: None,
            payment_sent_at_ms: None,
            disputed_at_ms: None,
            closed_at_ms: None,
            resolution: None,
        }
    }

    fn anonymous_proof_record() -> AnonymousAssetEscrowProofRecord {
        AnonymousAssetEscrowProofRecord {
            nullifiers: vec![[0x11; 32]],
            output_commitments: vec![[0x22; 32]],
            proof_hash: [0x33; 32],
            envelope_hash: None,
            root_hint: None,
            recorded_at_ms: 1,
        }
    }

    fn anonymous_escrow_fixture(
        escrow_id: EscrowId,
        seller: AccountId,
        buyer: Option<AccountId>,
        asset_definition: AssetDefinitionId,
        status: AssetEscrowStatus,
    ) -> AnonymousAssetEscrowRecord {
        AnonymousAssetEscrowRecord {
            id: escrow_id,
            seller,
            buyer,
            asset_definition,
            escrow_commitment: [0x22; 32],
            status,
            evidence_hashes: Vec::new(),
            opening: anonymous_proof_record(),
            release: None,
            cancellation: None,
            created_at_ms: 1,
            accepted_at_ms: None,
            payment_sent_at_ms: None,
            disputed_at_ms: None,
            closed_at_ms: None,
            resolution: None,
        }
    }

    fn anonymous_escrow_record_for_test(
        state_transaction: &StateTransaction<'_, '_>,
        escrow_id: &EscrowId,
    ) -> AnonymousAssetEscrowRecord {
        state_transaction
            .world
            .anonymous_asset_escrows
            .get(escrow_id)
            .cloned()
            .expect("anonymous escrow record")
    }

    fn anonymous_close_proof_with_input_commitments(
        input_commitments: [[u8; 32]; 2],
    ) -> ProofAttachment {
        let zero = [0u8; 32];
        let public_inputs = vec![
            input_commitments[0],
            input_commitments[1],
            [0x11; 32],
            zero,
            [0x22; 32],
            zero,
            [0x33; 32],
            [0x44; 32],
            [0x55; 32],
        ];
        let inner = iroha_zkp_halo2::Halo2ProofEnvelope::new(
            18,
            2,
            4,
            iroha_zkp_halo2::FLAG_LOOKUPS,
            public_inputs,
            vec![0xAB; 64],
        )
        .expect("confidential transfer public-input envelope")
        .to_bytes();
        let outer = iroha_data_model::zk::OpenVerifyEnvelope {
            backend: iroha_data_model::zk::BackendTag::Halo2IpaPasta,
            circuit_id: crate::zk::confidential_v2::CONFIDENTIAL_TRANSFER_V2_CIRCUIT_ID.to_owned(),
            vk_hash: [0x99; 32],
            public_inputs:
                crate::zk::confidential_v2::CONFIDENTIAL_TRANSFER_V2_PUBLIC_INPUTS_SCHEMA_V1
                    .to_vec(),
            proof_bytes: inner,
            aux: Vec::new(),
        };
        let proof_bytes = norito::to_bytes(&outer).expect("encode proof envelope");
        ProofAttachment {
            backend: crate::zk::ZK_BACKEND_HALO2_IPA.into(),
            proof: iroha_data_model::proof::ProofBox::new(
                crate::zk::ZK_BACKEND_HALO2_IPA.into(),
                proof_bytes,
            ),
            vk_ref: iroha_data_model::proof::VerifyingKeyId::new(
                crate::zk::ZK_BACKEND_HALO2_IPA,
                "anonymous_escrow",
            ),
            vk_commitment: None,
            envelope_hash: None,
            lane_privacy: None,
        }
    }

    fn tamper_anonymous_close_proof_envelope(
        mut proof: ProofAttachment,
        mutate: impl FnOnce(&mut OpenVerifyEnvelope),
    ) -> ProofAttachment {
        let mut envelope: OpenVerifyEnvelope =
            norito::decode_from_bytes(&proof.proof.bytes).expect("decode proof envelope");
        mutate(&mut envelope);
        proof.proof = iroha_data_model::proof::ProofBox::new(
            proof.proof.backend.clone(),
            norito::to_bytes(&envelope).expect("encode proof envelope"),
        );
        proof
    }

    fn grant_court_permission(state_transaction: &mut StateTransaction<'_, '_>, court: &AccountId) {
        let mut permissions = Permissions::default();
        permissions.insert(CanResolveEscrowDispute.into());
        state_transaction
            .world
            .account_permissions
            .insert(court.clone(), permissions);
    }

    fn state_transaction_deposit_closed_custody_dust(
        state_transaction: &mut StateTransaction<'_, '_>,
        custody_asset: &AssetId,
        amount: Numeric,
    ) {
        state_transaction
            .world
            .deposit_numeric_asset(custody_asset, &amount)
            .expect("deposit closed custody dust");
    }

    fn freeze_outbound_asset_transfers(
        state_transaction: &mut StateTransaction<'_, '_>,
        authority: &AccountId,
        account: &AccountId,
        asset_definition: &AssetDefinitionId,
    ) {
        SetAssetTransferFreeze::new(
            account.clone(),
            asset_definition.clone(),
            true,
            Some("escrow custody hold".to_owned()),
        )
        .execute(authority, state_transaction)
        .expect("freeze outbound asset transfers");
    }

    #[test]
    fn court_permission_constant_matches_typed_permission() {
        assert_eq!(
            CanResolveEscrowDispute::name().as_str(),
            CAN_RESOLVE_ESCROW_DISPUTE
        );
    }

    #[test]
    fn custody_account_derivation_is_stable() {
        let chain_id: iroha_data_model::ChainId = "00000000-0000-0000-0000-000000000001"
            .parse()
            .expect("chain id");
        let asset_definition: AssetDefinitionId =
            "61CtjvNd9T3THAR65GsMVHr82Bjc".parse().expect("asset");
        let escrow_id = EscrowId::new(Hash::new("escrow"));
        assert_eq!(
            escrow_custody_account_id(&chain_id, &escrow_id, &asset_definition),
            escrow_custody_account_id(&chain_id, &escrow_id, &asset_definition)
        );
    }

    #[test]
    fn resolution_split_must_equal_escrow_amount() {
        let total = Numeric::new(100_u32, 0);
        assert!(
            ensure_resolution_split(&total, &Numeric::new(40_u32, 0), &Numeric::new(60_u32, 0))
                .is_ok()
        );
        assert!(
            ensure_resolution_split(&total, &Numeric::new(40_u32, 0), &Numeric::new(59_u32, 0))
                .is_err()
        );
        assert!(
            ensure_resolution_split(&total, &Numeric::new(-1_i32, 0), &Numeric::new(101_u32, 0))
                .is_err()
        );
    }

    #[test]
    fn anonymous_escrow_byte_guards_reject_empty_zero_and_duplicate_values() {
        assert!(ensure_unique_non_zero_bytes("test", &[[0x01; 32]]).is_ok());
        assert!(ensure_unique_non_zero_bytes("test", &[]).is_err());
        assert!(ensure_unique_non_zero_bytes("test", &[[0; 32]]).is_err());
        assert!(ensure_unique_non_zero_bytes("test", &[[0x01; 32], [0x01; 32]]).is_err());
        assert!(ensure_single_escrow_nullifier(&[[0x01; 32]]).is_ok());
        assert!(ensure_single_escrow_nullifier(&[[0x01; 32], [0x02; 32]]).is_err());
    }

    #[test]
    fn anonymous_escrow_close_proof_must_bind_stored_commitment() {
        let escrow_commitment = [0x22; 32];

        let matching = anonymous_close_proof_with_input_commitments([escrow_commitment, [0; 32]]);
        ensure_close_proof_spends_escrow_commitment(&matching, escrow_commitment)
            .expect("matching close proof must pass");

        let wrong = anonymous_close_proof_with_input_commitments([[0x44; 32], [0; 32]]);
        let err = ensure_close_proof_spends_escrow_commitment(&wrong, escrow_commitment)
            .expect_err("wrong close proof input commitment must fail");
        assert!(
            err.to_string().contains("input commitment mismatch"),
            "unexpected error: {err}"
        );

        let missing = anonymous_close_proof_with_input_commitments([[0; 32], [0; 32]]);
        let err = ensure_close_proof_spends_escrow_commitment(&missing, escrow_commitment)
            .expect_err("close proof without a non-zero input must fail");
        assert!(
            err.to_string().contains("exactly one escrow commitment"),
            "unexpected error: {err}"
        );

        let extra = anonymous_close_proof_with_input_commitments([escrow_commitment, [0x55; 32]]);
        let err = ensure_close_proof_spends_escrow_commitment(&extra, escrow_commitment)
            .expect_err("close proof with multiple non-zero inputs must fail");
        assert!(
            err.to_string().contains("exactly one escrow commitment"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn anonymous_escrow_close_proof_rejects_noncanonical_envelope_before_public_input_trust() {
        let escrow_commitment = [0x22; 32];

        for (suffix, proof, expected_msg) in [
            (
                "backend_tag",
                tamper_anonymous_close_proof_envelope(
                    anonymous_close_proof_with_input_commitments([escrow_commitment, [0; 32]]),
                    |envelope| envelope.backend = BackendTag::Stark,
                ),
                "unexpected OpenVerifyEnvelope backend tag",
            ),
            (
                "circuit_id",
                tamper_anonymous_close_proof_envelope(
                    anonymous_close_proof_with_input_commitments([escrow_commitment, [0; 32]]),
                    |envelope| envelope.circuit_id = "halo2/pasta/ipa/vote-ballot".to_owned(),
                ),
                "requires confidential transfer v2 circuit",
            ),
            (
                "schema",
                tamper_anonymous_close_proof_envelope(
                    anonymous_close_proof_with_input_commitments([escrow_commitment, [0; 32]]),
                    |envelope| envelope.public_inputs = b"wrong-schema".to_vec(),
                ),
                "public inputs schema mismatch",
            ),
            (
                "aux",
                tamper_anonymous_close_proof_envelope(
                    anonymous_close_proof_with_input_commitments([escrow_commitment, [0; 32]]),
                    |envelope| envelope.aux = b"side-channel".to_vec(),
                ),
                "envelope auxiliary bytes must be empty",
            ),
            (
                "zero_vk_hash",
                tamper_anonymous_close_proof_envelope(
                    anonymous_close_proof_with_input_commitments([escrow_commitment, [0; 32]]),
                    |envelope| envelope.vk_hash = [0u8; 32],
                ),
                "verifier key hash must be non-zero",
            ),
            (
                "vk_commitment",
                {
                    let mut proof =
                        anonymous_close_proof_with_input_commitments([escrow_commitment, [0; 32]]);
                    proof.vk_commitment = Some([0x55; 32]);
                    proof
                },
                "verifier key commitment mismatch",
            ),
            (
                "attachment_backend",
                {
                    let mut proof =
                        anonymous_close_proof_with_input_commitments([escrow_commitment, [0; 32]]);
                    let bytes = proof.proof.bytes.clone();
                    proof.proof =
                        iroha_data_model::proof::ProofBox::new("halo2/ipa/other".into(), bytes);
                    proof
                },
                "backend mismatch",
            ),
            (
                "verifier_key_backend",
                {
                    let mut proof =
                        anonymous_close_proof_with_input_commitments([escrow_commitment, [0; 32]]);
                    proof.vk_ref = iroha_data_model::proof::VerifyingKeyId::new(
                        "stark/fri/sha256-goldilocks",
                        "anonymous_escrow",
                    );
                    proof
                },
                "verifier-key backend mismatch",
            ),
        ] {
            let err = ensure_close_proof_spends_escrow_commitment(&proof, escrow_commitment)
                .expect_err("noncanonical close proof should fail before public input trust");
            let msg = err.to_string();
            assert!(
                msg.contains(expected_msg),
                "case {suffix}: expected {expected_msg:?}, got {msg:?}"
            );
        }
    }

    #[test]
    fn anonymous_escrow_accept_mark_and_dispute_updates_lifecycle() {
        let seller = fixture_account("seller");
        let buyer = fixture_account("buyer");
        let court = fixture_account("court");
        let asset_definition = fixture_asset_definition_id();
        let escrow_id = fixture_escrow_id("anonymous-lifecycle");
        let state = state_with_parties(
            &seller,
            &buyer,
            &court,
            &asset_definition,
            Numeric::new(100_u32, 0),
        );
        let mut block = state.block(block_header(5_000));
        let mut tx = block.transaction();
        tx.world
            .insert_anonymous_asset_escrow_entry(anonymous_escrow_fixture(
                escrow_id,
                seller.clone(),
                None,
                asset_definition,
                AssetEscrowStatus::Open,
            ));

        assert!(
            AcceptAnonymousAssetEscrow { escrow_id }
                .execute(&seller, &mut tx)
                .is_err(),
            "seller cannot accept own anonymous escrow"
        );
        AcceptAnonymousAssetEscrow { escrow_id }
            .execute(&buyer, &mut tx)
            .expect("accept anonymous escrow");
        MarkAnonymousEscrowPaymentSent { escrow_id }
            .execute(&buyer, &mut tx)
            .expect("mark anonymous payment sent");
        OpenAnonymousEscrowDispute {
            escrow_id,
            evidence_hashes: vec![Hash::new("anonymous-dispute-evidence")],
        }
        .execute(&seller, &mut tx)
        .expect("open anonymous dispute");

        let record = anonymous_escrow_record_for_test(&tx, &escrow_id);
        assert_eq!(record.status, AssetEscrowStatus::Disputed);
        assert_eq!(record.buyer, Some(buyer.clone()));
        assert_eq!(record.disputed_at_ms, Some(5_000));
        assert_eq!(record.evidence_hashes.len(), 1);
        assert_eq!(
            FindAnonymousAssetEscrowById { escrow_id }
                .execute(&tx)
                .expect("query anonymous escrow by id")
                .status,
            AssetEscrowStatus::Disputed
        );
        let by_status = FindAnonymousAssetEscrowsByStatus {
            status: AssetEscrowStatus::Disputed,
        }
        .execute(CompoundPredicate::PASS, &tx)
        .expect("query anonymous escrows by status")
        .collect::<Vec<_>>();
        assert_eq!(by_status.len(), 1);

        let by_id = query_anonymous_asset_escrow_ids(
            &tx,
            CompoundPredicate::<AnonymousAssetEscrowRecord>::build(|predicate| {
                predicate.equals("id", escrow_id)
            }),
        );
        assert_eq!(by_id, vec![escrow_id]);

        let by_seller = query_anonymous_asset_escrow_ids(
            &tx,
            CompoundPredicate::<AnonymousAssetEscrowRecord>::build(|predicate| {
                predicate.equals("seller", seller.clone())
            }),
        );
        assert_eq!(by_seller, vec![escrow_id]);

        let by_buyer = query_anonymous_asset_escrow_ids(
            &tx,
            CompoundPredicate::<AnonymousAssetEscrowRecord>::build(|predicate| {
                predicate.equals("buyer", buyer.clone())
            }),
        );
        assert_eq!(by_buyer, vec![escrow_id]);

        let by_generic_status = query_anonymous_asset_escrow_ids(
            &tx,
            CompoundPredicate::<AnonymousAssetEscrowRecord>::build(|predicate| {
                predicate.equals("status", AssetEscrowStatus::Disputed)
            }),
        );
        assert_eq!(by_generic_status, vec![escrow_id]);

        let missing_buyer = fixture_account("missing-anonymous-buyer");
        let missing = query_anonymous_asset_escrow_ids(
            &tx,
            CompoundPredicate::<AnonymousAssetEscrowRecord>::build(|predicate| {
                predicate.equals("buyer", missing_buyer)
            }),
        );
        assert!(missing.is_empty());
    }

    #[test]
    fn anonymous_escrow_close_authorization_runs_before_proof() {
        let seller = fixture_account("seller");
        let buyer = fixture_account("buyer");
        let court = fixture_account("court");
        let asset_definition = fixture_asset_definition_id();
        let state = state_with_parties(
            &seller,
            &buyer,
            &court,
            &asset_definition,
            Numeric::new(100_u32, 0),
        );
        let mut block = state.block(block_header(6_000));
        let mut tx = block.transaction();
        let dummy_proof = ProofAttachment {
            backend: "dummy".into(),
            proof: iroha_data_model::proof::ProofBox::new("dummy".into(), Vec::new()),
            vk_ref: iroha_data_model::proof::VerifyingKeyId::new("dummy", "dummy"),
            vk_commitment: None,
            envelope_hash: None,
            lane_privacy: None,
        };

        let release_id = fixture_escrow_id("anonymous-release-auth");
        tx.world
            .insert_anonymous_asset_escrow_entry(anonymous_escrow_fixture(
                release_id,
                seller.clone(),
                Some(buyer.clone()),
                asset_definition.clone(),
                AssetEscrowStatus::PaymentSent,
            ));
        assert!(
            ReleaseAnonymousAssetEscrow {
                escrow_id: release_id,
                escrow_nullifiers: vec![[0x44; 32]],
                buyer_output_commitments: vec![[0x55; 32]],
                proof: dummy_proof.clone(),
                root_hint: None,
            }
            .execute(&buyer, &mut tx)
            .is_err(),
            "buyer cannot release anonymous escrow"
        );

        let cancel_id = fixture_escrow_id("anonymous-cancel-auth");
        tx.world
            .insert_anonymous_asset_escrow_entry(anonymous_escrow_fixture(
                cancel_id,
                seller.clone(),
                Some(buyer.clone()),
                asset_definition.clone(),
                AssetEscrowStatus::Accepted,
            ));
        assert!(
            CancelAnonymousAssetEscrow {
                escrow_id: cancel_id,
                escrow_nullifiers: vec![[0x44; 32]],
                seller_output_commitments: vec![[0x66; 32]],
                proof: dummy_proof.clone(),
                root_hint: None,
            }
            .execute(&buyer, &mut tx)
            .is_err(),
            "buyer cannot cancel anonymous escrow"
        );

        let dispute_id = fixture_escrow_id("anonymous-resolve-auth");
        tx.world
            .insert_anonymous_asset_escrow_entry(anonymous_escrow_fixture(
                dispute_id,
                seller,
                Some(buyer),
                asset_definition,
                AssetEscrowStatus::Disputed,
            ));
        assert!(
            ResolveAnonymousEscrowDispute {
                escrow_id: dispute_id,
                escrow_nullifiers: vec![[0x44; 32]],
                buyer_output_commitments: vec![[0x55; 32]],
                seller_output_commitments: vec![],
                proof: dummy_proof,
                root_hint: None,
                evidence_hashes: Vec::new(),
            }
            .execute(&court, &mut tx)
            .is_err(),
            "court needs CanResolveEscrowDispute before proof verification"
        );
    }

    #[test]
    fn escrow_open_rejects_id_used_by_anonymous_escrow() {
        let seller = fixture_account("seller");
        let buyer = fixture_account("buyer");
        let court = fixture_account("court");
        let asset_definition = fixture_asset_definition_id();
        let escrow_id = fixture_escrow_id("public-collides-with-anonymous");
        let state = state_with_parties(
            &seller,
            &buyer,
            &court,
            &asset_definition,
            Numeric::new(100_u32, 0),
        );
        let mut block = state.block(block_header(7_000));
        let mut tx = block.transaction();
        tx.world
            .insert_anonymous_asset_escrow_entry(anonymous_escrow_fixture(
                escrow_id,
                seller.clone(),
                None,
                asset_definition.clone(),
                AssetEscrowStatus::Open,
            ));

        let err = OpenAssetEscrow {
            escrow_id,
            asset_definition: asset_definition.clone(),
            amount: Numeric::new(40_u32, 0),
            evidence_hashes: Vec::new(),
        }
        .execute(&seller, &mut tx)
        .expect_err("public escrow id must not collide with anonymous escrow id");

        assert!(
            err.to_string().contains("escrow already exists"),
            "unexpected error: {err}"
        );
        assert!(tx.world.asset_escrows.get(&escrow_id).is_none());
        assert_eq!(
            balance(&tx, &seller, &asset_definition),
            Numeric::new(100_u32, 0)
        );
    }

    #[test]
    fn escrow_open_rejects_shielded_only_asset() {
        let seller = fixture_account("seller");
        let buyer = fixture_account("buyer");
        let court = fixture_account("court");
        let asset_definition = fixture_asset_definition_id();
        let escrow_id = fixture_escrow_id("shielded-open");
        let asset_definition_entry = AssetDefinition::numeric(asset_definition.clone())
            .with_name("XOR".to_owned())
            .confidential_policy(AssetConfidentialPolicy::shielded_only())
            .build(&seller);
        let state = state_with_parties_and_definition(
            &seller,
            &buyer,
            &court,
            &asset_definition,
            asset_definition_entry,
            Numeric::new(100_u32, 0),
        );
        let mut block = state.block(block_header(1_000));
        let mut tx = block.transaction();

        let err = OpenAssetEscrow {
            escrow_id,
            asset_definition: asset_definition.clone(),
            amount: Numeric::new(40_u32, 0),
            evidence_hashes: Vec::new(),
        }
        .execute(&seller, &mut tx)
        .expect_err("shielded-only assets cannot enter transparent escrow custody");

        assert!(
            err.to_string()
                .contains("transparent transfer not permitted by policy"),
            "unexpected error: {err}"
        );
        assert!(tx.world.asset_escrows.get(&escrow_id).is_none());
        assert_eq!(
            balance(&tx, &seller, &asset_definition),
            Numeric::new(100_u32, 0)
        );
        let custody = escrow_custody_account_id(tx.chain_id(), &escrow_id, &asset_definition);
        assert_eq!(balance(&tx, &custody, &asset_definition), Numeric::zero());
    }

    #[test]
    fn escrow_open_rejects_issuer_policy_without_custody_binding() {
        let seller = fixture_account("seller");
        let buyer = fixture_account("buyer");
        let court = fixture_account("court");
        let asset_definition = fixture_asset_definition_id();
        let escrow_id = fixture_escrow_id("issuer-policy-open");
        let mut asset_definition_entry = AssetDefinition::numeric(asset_definition.clone())
            .with_name("XOR".to_owned())
            .build(&seller);
        let issuer_policy = AssetIssuerUsagePolicyV1 {
            require_subject_binding: true,
            subject_bindings: BTreeMap::from([(seller.clone(), AssetSubjectBindingV1::default())]),
        };
        asset_definition_entry.metadata_mut().insert(
            ASSET_ISSUER_USAGE_POLICY_METADATA_KEY
                .parse()
                .expect("metadata key"),
            Json::new(issuer_policy),
        );
        let state = state_with_parties_and_definition(
            &seller,
            &buyer,
            &court,
            &asset_definition,
            asset_definition_entry,
            Numeric::new(100_u32, 0),
        );
        let mut block = state.block(block_header(1_000));
        let mut tx = block.transaction();

        let err = OpenAssetEscrow {
            escrow_id,
            asset_definition: asset_definition.clone(),
            amount: Numeric::new(40_u32, 0),
            evidence_hashes: Vec::new(),
        }
        .execute(&seller, &mut tx)
        .expect_err("issuer policy must apply to escrow custody account");

        assert!(
            err.to_string()
                .contains("requires explicit subject binding"),
            "unexpected error: {err}"
        );
        assert!(tx.world.asset_escrows.get(&escrow_id).is_none());
        assert_eq!(
            balance(&tx, &seller, &asset_definition),
            Numeric::new(100_u32, 0)
        );
    }

    #[test]
    fn escrow_open_rejects_frozen_outbound_asset() {
        let seller = fixture_account("seller");
        let buyer = fixture_account("buyer");
        let court = fixture_account("court");
        let asset_definition = fixture_asset_definition_id();
        let escrow_id = fixture_escrow_id("frozen-open");
        let state = state_with_parties(
            &seller,
            &buyer,
            &court,
            &asset_definition,
            Numeric::new(100_u32, 0),
        );
        let mut block = state.block(block_header(1_000));
        let mut tx = block.transaction();

        SetAssetTransferFreeze::new(
            seller.clone(),
            asset_definition.clone(),
            true,
            Some("compliance hold".to_owned()),
        )
        .execute(&seller, &mut tx)
        .expect("freeze succeeds");

        let err = OpenAssetEscrow {
            escrow_id,
            asset_definition: asset_definition.clone(),
            amount: Numeric::new(40_u32, 0),
            evidence_hashes: Vec::new(),
        }
        .execute(&seller, &mut tx)
        .expect_err("outbound transfer controls must apply to escrow opening");

        assert!(
            err.to_string().contains("frozen"),
            "unexpected error: {err}"
        );
        assert!(tx.world.asset_escrows.get(&escrow_id).is_none());
        assert_eq!(
            balance(&tx, &seller, &asset_definition),
            Numeric::new(100_u32, 0)
        );
    }

    #[test]
    fn escrow_open_accept_mark_and_release_moves_custody_to_buyer() {
        let seller = fixture_account("seller");
        let buyer = fixture_account("buyer");
        let court = fixture_account("court");
        let asset_definition = fixture_asset_definition_id();
        let escrow_id = fixture_escrow_id("release");
        let amount = Numeric::new(40_u32, 0);
        let state = state_with_parties(
            &seller,
            &buyer,
            &court,
            &asset_definition,
            Numeric::new(100_u32, 0),
        );
        let mut block = state.block(block_header(1_000));
        let mut tx = block.transaction();
        tx.tx_call_hash = Some(Hash::prehashed([0xA2; Hash::LENGTH]));

        OpenAssetEscrow {
            escrow_id,
            asset_definition: asset_definition.clone(),
            amount: amount.clone(),
            evidence_hashes: Vec::new(),
        }
        .execute(&seller, &mut tx)
        .expect("open escrow");

        let record = escrow_record(&tx, &escrow_id);
        assert_eq!(record.status, AssetEscrowStatus::Open);
        assert_eq!(
            balance(&tx, &seller, &asset_definition),
            Numeric::new(60_u32, 0)
        );
        assert_eq!(balance(&tx, &record.custody, &asset_definition), amount);

        AcceptAssetEscrow { escrow_id }
            .execute(&buyer, &mut tx)
            .expect("accept escrow");
        MarkEscrowPaymentSent { escrow_id }
            .execute(&buyer, &mut tx)
            .expect("mark payment sent");
        ReleaseAssetEscrow { escrow_id }
            .execute(&seller, &mut tx)
            .expect("release escrow");

        let record = escrow_record(&tx, &escrow_id);
        assert_eq!(record.status, AssetEscrowStatus::Released);
        assert_eq!(
            balance(&tx, &buyer, &asset_definition),
            Numeric::new(40_u32, 0)
        );
        assert_eq!(
            balance(&tx, &record.custody, &asset_definition),
            Numeric::zero()
        );
    }

    #[test]
    fn asset_lock_drawdown_respects_release_authority_and_tracks_remaining() {
        let source = fixture_account("lock-source");
        let destination = fixture_account("lock-destination");
        let release_authority = fixture_account("lock-release-authority");
        let asset_definition = fixture_asset_definition_id();
        let escrow_id = fixture_escrow_id("lock-drawdown-authority");
        let amount = Numeric::new(40_u32, 0);
        let state = state_with_parties(
            &source,
            &destination,
            &release_authority,
            &asset_definition,
            Numeric::new(100_u32, 0),
        );
        let mut block = state.block(block_header(1_000));
        let mut tx = block.transaction();
        seed_test_call_hash(&mut tx, 0xC1);

        OpenAssetLock::with_options(
            escrow_id,
            asset_definition.clone(),
            destination.clone(),
            amount.clone(),
            Some(release_authority.clone()),
            None,
            vec![Hash::new("lock-open-evidence")],
        )
        .execute(&source, &mut tx)
        .expect("open asset lock");

        let record = escrow_record(&tx, &escrow_id);
        assert_eq!(record.kind, AssetEscrowKind::Lock);
        assert_eq!(record.status, AssetEscrowStatus::Locked);
        assert_eq!(record.remaining_amount, amount);
        assert_eq!(record.buyer, Some(destination.clone()));
        assert_eq!(record.release_authority, Some(release_authority.clone()));
        assert_eq!(
            balance(&tx, &source, &asset_definition),
            Numeric::new(60_u32, 0)
        );
        assert_eq!(balance(&tx, &record.custody, &asset_definition), amount);

        let err = DrawdownAssetLock::new(escrow_id, Numeric::new(15_u32, 0))
            .execute(&destination, &mut tx)
            .expect_err("destination cannot draw down when release authority is set");
        assert!(
            err.to_string().contains("release authority"),
            "unexpected error: {err}"
        );

        DrawdownAssetLock::new(escrow_id, Numeric::new(15_u32, 0))
            .execute(&release_authority, &mut tx)
            .expect("release authority draws down partial amount");
        let record = escrow_record(&tx, &escrow_id);
        assert_eq!(record.status, AssetEscrowStatus::Locked);
        assert_eq!(record.remaining_amount, Numeric::new(25_u32, 0));
        assert_eq!(
            balance(&tx, &destination, &asset_definition),
            Numeric::new(15_u32, 0)
        );
        assert_eq!(
            balance(&tx, &record.custody, &asset_definition),
            Numeric::new(25_u32, 0)
        );

        DrawdownAssetLock::new(escrow_id, Numeric::new(25_u32, 0))
            .execute(&release_authority, &mut tx)
            .expect("release authority draws down remaining amount");
        let record = escrow_record(&tx, &escrow_id);
        assert_eq!(record.status, AssetEscrowStatus::DrawnDown);
        assert_eq!(record.remaining_amount, Numeric::zero());
        assert_eq!(record.closed_at_ms, Some(1_000));
        assert_eq!(
            balance(&tx, &destination, &asset_definition),
            Numeric::new(40_u32, 0)
        );
        assert_eq!(
            balance(&tx, &record.custody, &asset_definition),
            Numeric::zero()
        );
    }

    #[test]
    fn asset_lock_cancel_refunds_remaining_after_destination_drawdown() {
        let source = fixture_account("lock-cancel-source");
        let destination = fixture_account("lock-cancel-destination");
        let observer = fixture_account("lock-cancel-observer");
        let asset_definition = fixture_asset_definition_id();
        let escrow_id = fixture_escrow_id("lock-cancel-after-drawdown");
        let state = state_with_parties(
            &source,
            &destination,
            &observer,
            &asset_definition,
            Numeric::new(100_u32, 0),
        );
        let mut block = state.block(block_header(2_000));
        let mut tx = block.transaction();
        seed_test_call_hash(&mut tx, 0xC2);

        OpenAssetLock::new(
            escrow_id,
            asset_definition.clone(),
            destination.clone(),
            Numeric::new(40_u32, 0),
        )
        .execute(&source, &mut tx)
        .expect("open destination-drawn lock");
        DrawdownAssetLock::new(escrow_id, Numeric::new(15_u32, 0))
            .execute(&destination, &mut tx)
            .expect("destination draws down when no release authority is set");

        let err = CancelAssetLock::new(escrow_id)
            .execute(&destination, &mut tx)
            .expect_err("destination cannot cancel source-opened lock");
        assert!(
            err.to_string().contains("lock opener"),
            "unexpected error: {err}"
        );

        CancelAssetLock::new(escrow_id)
            .execute(&source, &mut tx)
            .expect("source cancels remaining lock");
        let record = escrow_record(&tx, &escrow_id);
        assert_eq!(record.status, AssetEscrowStatus::Cancelled);
        assert_eq!(record.remaining_amount, Numeric::zero());
        assert_eq!(record.closed_at_ms, Some(2_000));
        assert_eq!(
            balance(&tx, &source, &asset_definition),
            Numeric::new(85_u32, 0)
        );
        assert_eq!(
            balance(&tx, &destination, &asset_definition),
            Numeric::new(15_u32, 0)
        );
        assert_eq!(
            balance(&tx, &record.custody, &asset_definition),
            Numeric::zero()
        );
    }

    #[test]
    fn asset_lock_expire_requires_deadline_and_refunds_after_deadline() {
        let source = fixture_account("lock-expire-source");
        let destination = fixture_account("lock-expire-destination");
        let observer = fixture_account("lock-expire-observer");
        let asset_definition = fixture_asset_definition_id();
        let no_deadline_id = fixture_escrow_id("lock-expire-no-deadline");
        let future_id = fixture_escrow_id("lock-expire-future");
        let expired_id = fixture_escrow_id("lock-expire-past");
        let state = state_with_parties(
            &source,
            &destination,
            &observer,
            &asset_definition,
            Numeric::new(100_u32, 0),
        );
        let mut block = state.block(block_header(3_000));
        let mut tx = block.transaction();
        seed_test_call_hash(&mut tx, 0xC3);

        OpenAssetLock::new(
            no_deadline_id,
            asset_definition.clone(),
            destination.clone(),
            Numeric::new(10_u32, 0),
        )
        .execute(&source, &mut tx)
        .expect("open lock without deadline");
        OpenAssetLock::with_options(
            future_id,
            asset_definition.clone(),
            destination.clone(),
            Numeric::new(20_u32, 0),
            None,
            Some(4_000),
            Vec::new(),
        )
        .execute(&source, &mut tx)
        .expect("open future-expiring lock");
        OpenAssetLock::with_options(
            expired_id,
            asset_definition.clone(),
            destination.clone(),
            Numeric::new(30_u32, 0),
            None,
            Some(2_000),
            Vec::new(),
        )
        .execute(&source, &mut tx)
        .expect("open already-expirable lock");

        let err = ExpireAssetLock::new(no_deadline_id)
            .execute(&observer, &mut tx)
            .expect_err("lock without deadline cannot expire");
        assert!(
            err.to_string().contains("no expiry"),
            "unexpected error: {err}"
        );
        let err = ExpireAssetLock::new(future_id)
            .execute(&observer, &mut tx)
            .expect_err("future deadline cannot expire");
        assert!(
            err.to_string().contains("has not been reached"),
            "unexpected error: {err}"
        );

        ExpireAssetLock::new(expired_id)
            .execute(&observer, &mut tx)
            .expect("any account expires a past-deadline lock");
        let record = escrow_record(&tx, &expired_id);
        assert_eq!(record.status, AssetEscrowStatus::Expired);
        assert_eq!(record.remaining_amount, Numeric::zero());
        assert_eq!(record.closed_at_ms, Some(3_000));
        assert_eq!(
            balance(&tx, &source, &asset_definition),
            Numeric::new(70_u32, 0)
        );
        assert_eq!(
            balance(&tx, &destination, &asset_definition),
            Numeric::zero()
        );
        assert_eq!(
            balance(&tx, &record.custody, &asset_definition),
            Numeric::zero()
        );
        assert_eq!(
            escrow_record(&tx, &no_deadline_id).status,
            AssetEscrowStatus::Locked
        );
        assert_eq!(
            escrow_record(&tx, &future_id).status,
            AssetEscrowStatus::Locked
        );
    }

    #[test]
    fn asset_lock_open_rejects_invalid_inputs_without_state_changes() {
        let source = fixture_account("lock-open-source");
        let destination = fixture_account("lock-open-destination");
        let observer = fixture_account("lock-open-observer");
        let asset_definition = fixture_asset_definition_id();
        let state = state_with_parties(
            &source,
            &destination,
            &observer,
            &asset_definition,
            Numeric::new(40_u32, 0),
        );
        let mut block = state.block(block_header(3_100));
        let mut tx = block.transaction();
        seed_test_call_hash(&mut tx, 0xC4);

        let zero_id = fixture_escrow_id("lock-open-zero");
        let err = OpenAssetLock::new(
            zero_id,
            asset_definition.clone(),
            destination.clone(),
            Numeric::zero(),
        )
        .execute(&source, &mut tx)
        .expect_err("zero lock amount must be rejected");
        assert!(
            err.to_string().contains("non-zero"),
            "unexpected error: {err}"
        );
        assert!(tx.world.asset_escrows.get(&zero_id).is_none());

        let negative_id = fixture_escrow_id("lock-open-negative");
        let err = OpenAssetLock::new(
            negative_id,
            asset_definition.clone(),
            destination.clone(),
            Numeric::new(-1_i32, 0),
        )
        .execute(&source, &mut tx)
        .expect_err("negative lock amount must be rejected");
        assert!(
            err.to_string().contains("negative"),
            "unexpected error: {err}"
        );
        assert!(tx.world.asset_escrows.get(&negative_id).is_none());

        let missing_destination_id = fixture_escrow_id("lock-open-missing-destination");
        let missing_destination = fixture_account("lock-open-missing-destination-account");
        assert!(
            OpenAssetLock::new(
                missing_destination_id,
                asset_definition.clone(),
                missing_destination,
                Numeric::new(5_u32, 0),
            )
            .execute(&source, &mut tx)
            .is_err(),
            "missing destination account must be rejected"
        );
        assert!(
            tx.world
                .asset_escrows
                .get(&missing_destination_id)
                .is_none()
        );

        let missing_authority_id = fixture_escrow_id("lock-open-missing-authority");
        let missing_authority = fixture_account("lock-open-missing-authority-account");
        assert!(
            OpenAssetLock::with_options(
                missing_authority_id,
                asset_definition.clone(),
                destination.clone(),
                Numeric::new(5_u32, 0),
                Some(missing_authority),
                None,
                Vec::new(),
            )
            .execute(&source, &mut tx)
            .is_err(),
            "missing release authority account must be rejected"
        );
        assert!(tx.world.asset_escrows.get(&missing_authority_id).is_none());

        let over_balance_id = fixture_escrow_id("lock-open-over-balance");
        assert!(
            OpenAssetLock::new(
                over_balance_id,
                asset_definition.clone(),
                destination.clone(),
                Numeric::new(41_u32, 0),
            )
            .execute(&source, &mut tx)
            .is_err(),
            "insufficient source balance must be rejected"
        );
        let custody = escrow_custody_account_id(tx.chain_id(), &over_balance_id, &asset_definition);
        assert!(tx.world.asset_escrows.get(&over_balance_id).is_none());
        assert!(tx.world.account(&custody).is_err());

        let duplicate_id = fixture_escrow_id("lock-open-duplicate-anonymous");
        tx.world
            .insert_anonymous_asset_escrow_entry(anonymous_escrow_fixture(
                duplicate_id,
                source.clone(),
                None,
                asset_definition.clone(),
                AssetEscrowStatus::Open,
            ));
        let err = OpenAssetLock::new(
            duplicate_id,
            asset_definition.clone(),
            destination.clone(),
            Numeric::new(5_u32, 0),
        )
        .execute(&source, &mut tx)
        .expect_err("duplicate anonymous escrow id must be rejected");
        assert!(
            err.to_string().contains("already exists"),
            "unexpected error: {err}"
        );

        assert_eq!(
            balance(&tx, &source, &asset_definition),
            Numeric::new(40_u32, 0)
        );
        assert_eq!(
            balance(&tx, &destination, &asset_definition),
            Numeric::zero()
        );
    }

    #[test]
    fn asset_lock_drawdown_rejects_wrong_kind_status_and_amounts() {
        let source = fixture_account("lock-drawdown-negative-source");
        let destination = fixture_account("lock-drawdown-negative-destination");
        let observer = fixture_account("lock-drawdown-negative-observer");
        let asset_definition = fixture_asset_definition_id();
        let state = state_with_parties(
            &source,
            &destination,
            &observer,
            &asset_definition,
            Numeric::new(100_u32, 0),
        );
        let mut block = state.block(block_header(3_200));
        let mut tx = block.transaction();
        seed_test_call_hash(&mut tx, 0xC5);

        let marketplace_id = fixture_escrow_id("lock-drawdown-marketplace");
        OpenAssetEscrow {
            escrow_id: marketplace_id,
            asset_definition: asset_definition.clone(),
            amount: Numeric::new(10_u32, 0),
            evidence_hashes: Vec::new(),
        }
        .execute(&source, &mut tx)
        .expect("open marketplace escrow");
        let err = DrawdownAssetLock::new(marketplace_id, Numeric::new(1_u32, 0))
            .execute(&destination, &mut tx)
            .expect_err("marketplace escrow cannot be drawn down as a lock");
        assert!(
            err.to_string().contains("generic asset lock"),
            "unexpected error: {err}"
        );

        let missing_id = fixture_escrow_id("lock-drawdown-missing");
        let err = DrawdownAssetLock::new(missing_id, Numeric::new(1_u32, 0))
            .execute(&destination, &mut tx)
            .expect_err("missing lock cannot be drawn down");
        assert!(
            err.to_string().contains("not found"),
            "unexpected error: {err}"
        );

        let lock_id = fixture_escrow_id("lock-drawdown-negative");
        OpenAssetLock::new(
            lock_id,
            asset_definition.clone(),
            destination.clone(),
            Numeric::new(30_u32, 0),
        )
        .execute(&source, &mut tx)
        .expect("open lock");
        let custody = escrow_record(&tx, &lock_id).custody;

        let err = DrawdownAssetLock::new(lock_id, Numeric::zero())
            .execute(&destination, &mut tx)
            .expect_err("zero drawdown must be rejected");
        assert!(
            err.to_string().contains("non-zero"),
            "unexpected error: {err}"
        );
        let err = DrawdownAssetLock::new(lock_id, Numeric::new(-1_i32, 0))
            .execute(&destination, &mut tx)
            .expect_err("negative drawdown must be rejected");
        assert!(
            err.to_string().contains("negative"),
            "unexpected error: {err}"
        );
        let err = DrawdownAssetLock::new(lock_id, Numeric::new(31_u32, 0))
            .execute(&destination, &mut tx)
            .expect_err("overdraw must be rejected");
        assert!(
            err.to_string().contains("exceeds remaining"),
            "unexpected error: {err}"
        );
        let err = DrawdownAssetLock::new(lock_id, Numeric::new(1_u32, 0))
            .execute(&observer, &mut tx)
            .expect_err("observer cannot draw down destination-controlled lock");
        assert!(
            err.to_string().contains("destination"),
            "unexpected error: {err}"
        );

        assert_eq!(
            escrow_record(&tx, &lock_id).status,
            AssetEscrowStatus::Locked
        );
        assert_eq!(
            escrow_record(&tx, &lock_id).remaining_amount,
            Numeric::new(30_u32, 0)
        );
        assert_eq!(
            balance(&tx, &custody, &asset_definition),
            Numeric::new(30_u32, 0)
        );
        assert_eq!(
            balance(&tx, &destination, &asset_definition),
            Numeric::zero()
        );

        DrawdownAssetLock::new(lock_id, Numeric::new(30_u32, 0))
            .execute(&destination, &mut tx)
            .expect("draw down all funds");
        assert_eq!(
            escrow_record(&tx, &lock_id).status,
            AssetEscrowStatus::DrawnDown
        );
        assert!(
            DrawdownAssetLock::new(lock_id, Numeric::new(1_u32, 0))
                .execute(&destination, &mut tx)
                .is_err(),
            "closed lock cannot be drawn down again"
        );
        assert!(
            CancelAssetLock::new(lock_id)
                .execute(&source, &mut tx)
                .is_err(),
            "closed lock cannot be cancelled"
        );
        assert!(
            ExpireAssetLock::new(lock_id)
                .execute(&observer, &mut tx)
                .is_err(),
            "closed lock cannot expire"
        );
        assert_eq!(
            balance(&tx, &destination, &asset_definition),
            Numeric::new(30_u32, 0)
        );
        assert_eq!(balance(&tx, &custody, &asset_definition), Numeric::zero());
    }

    #[test]
    fn asset_lock_custody_freeze_blocks_drawdown_cancel_and_expire() {
        let source = fixture_account("lock-freeze-source");
        let destination = fixture_account("lock-freeze-destination");
        let observer = fixture_account("lock-freeze-observer");
        let asset_definition = fixture_asset_definition_id();
        let drawdown_id = fixture_escrow_id("lock-freeze-drawdown");
        let cancel_id = fixture_escrow_id("lock-freeze-cancel");
        let expire_id = fixture_escrow_id("lock-freeze-expire");
        let state = state_with_parties(
            &source,
            &destination,
            &observer,
            &asset_definition,
            Numeric::new(100_u32, 0),
        );
        let mut block = state.block(block_header(3_300));
        let mut tx = block.transaction();
        seed_test_call_hash(&mut tx, 0xC6);

        OpenAssetLock::new(
            drawdown_id,
            asset_definition.clone(),
            destination.clone(),
            Numeric::new(10_u32, 0),
        )
        .execute(&source, &mut tx)
        .expect("open drawdown lock");
        OpenAssetLock::new(
            cancel_id,
            asset_definition.clone(),
            destination.clone(),
            Numeric::new(20_u32, 0),
        )
        .execute(&source, &mut tx)
        .expect("open cancel lock");
        OpenAssetLock::with_options(
            expire_id,
            asset_definition.clone(),
            destination.clone(),
            Numeric::new(30_u32, 0),
            None,
            Some(3_000),
            Vec::new(),
        )
        .execute(&source, &mut tx)
        .expect("open expired lock");

        let drawdown_custody = escrow_record(&tx, &drawdown_id).custody;
        freeze_outbound_asset_transfers(&mut tx, &source, &drawdown_custody, &asset_definition);
        let err = DrawdownAssetLock::new(drawdown_id, Numeric::new(5_u32, 0))
            .execute(&destination, &mut tx)
            .expect_err("custody freeze must block drawdown");
        assert!(
            err.to_string().contains("frozen"),
            "unexpected error: {err}"
        );

        let cancel_custody = escrow_record(&tx, &cancel_id).custody;
        freeze_outbound_asset_transfers(&mut tx, &source, &cancel_custody, &asset_definition);
        let err = CancelAssetLock::new(cancel_id)
            .execute(&source, &mut tx)
            .expect_err("custody freeze must block cancellation refund");
        assert!(
            err.to_string().contains("frozen"),
            "unexpected error: {err}"
        );

        let expire_custody = escrow_record(&tx, &expire_id).custody;
        freeze_outbound_asset_transfers(&mut tx, &source, &expire_custody, &asset_definition);
        let err = ExpireAssetLock::new(expire_id)
            .execute(&observer, &mut tx)
            .expect_err("custody freeze must block expiry refund");
        assert!(
            err.to_string().contains("frozen"),
            "unexpected error: {err}"
        );

        assert_eq!(
            escrow_record(&tx, &drawdown_id).status,
            AssetEscrowStatus::Locked
        );
        assert_eq!(
            escrow_record(&tx, &cancel_id).status,
            AssetEscrowStatus::Locked
        );
        assert_eq!(
            escrow_record(&tx, &expire_id).status,
            AssetEscrowStatus::Locked
        );
        assert_eq!(
            balance(&tx, &drawdown_custody, &asset_definition),
            Numeric::new(10_u32, 0)
        );
        assert_eq!(
            balance(&tx, &cancel_custody, &asset_definition),
            Numeric::new(20_u32, 0)
        );
        assert_eq!(
            balance(&tx, &expire_custody, &asset_definition),
            Numeric::new(30_u32, 0)
        );
        assert_eq!(
            balance(&tx, &destination, &asset_definition),
            Numeric::zero()
        );
        assert_eq!(
            balance(&tx, &source, &asset_definition),
            Numeric::new(40_u32, 0)
        );
    }

    #[test]
    fn asset_lock_adversarial_ordering_keeps_balances_consistent() {
        let source = fixture_account("lock-order-source");
        let destination = fixture_account("lock-order-destination");
        let observer = fixture_account("lock-order-observer");
        let asset_definition = fixture_asset_definition_id();
        let escrow_id = fixture_escrow_id("lock-order");
        let state = state_with_parties(
            &source,
            &destination,
            &observer,
            &asset_definition,
            Numeric::new(100_u32, 0),
        );
        let mut block = state.block(block_header(3_400));
        let mut tx = block.transaction();
        seed_test_call_hash(&mut tx, 0xC7);

        OpenAssetLock::new(
            escrow_id,
            asset_definition.clone(),
            destination.clone(),
            Numeric::new(30_u32, 0),
        )
        .execute(&source, &mut tx)
        .expect("open lock");
        let custody = escrow_record(&tx, &escrow_id).custody;

        assert!(
            CancelAssetLock::new(escrow_id)
                .execute(&destination, &mut tx)
                .is_err(),
            "destination cannot cancel source-opened lock"
        );
        DrawdownAssetLock::new(escrow_id, Numeric::new(10_u32, 0))
            .execute(&destination, &mut tx)
            .expect("destination draws down partial amount");
        assert!(
            ExpireAssetLock::new(escrow_id)
                .execute(&observer, &mut tx)
                .is_err(),
            "lock without deadline cannot expire"
        );
        assert!(
            DrawdownAssetLock::new(escrow_id, Numeric::new(25_u32, 0))
                .execute(&destination, &mut tx)
                .is_err(),
            "cannot draw down more than remaining amount"
        );
        CancelAssetLock::new(escrow_id)
            .execute(&source, &mut tx)
            .expect("source cancels remaining funds");

        assert!(
            DrawdownAssetLock::new(escrow_id, Numeric::new(1_u32, 0))
                .execute(&destination, &mut tx)
                .is_err(),
            "closed lock must reject drawdown"
        );
        assert!(
            CancelAssetLock::new(escrow_id)
                .execute(&source, &mut tx)
                .is_err(),
            "closed lock must reject cancellation"
        );
        assert!(
            ExpireAssetLock::new(escrow_id)
                .execute(&observer, &mut tx)
                .is_err(),
            "closed lock must reject expiry"
        );

        let record = escrow_record(&tx, &escrow_id);
        assert_eq!(record.status, AssetEscrowStatus::Cancelled);
        assert_eq!(record.remaining_amount, Numeric::zero());
        assert_eq!(
            balance(&tx, &source, &asset_definition),
            Numeric::new(90_u32, 0)
        );
        assert_eq!(
            balance(&tx, &destination, &asset_definition),
            Numeric::new(10_u32, 0)
        );
        assert_eq!(balance(&tx, &custody, &asset_definition), Numeric::zero());
    }

    #[test]
    fn asset_escrow_indexes_track_seller_buyer_and_status_updates() {
        let seller = fixture_account("seller");
        let buyer = fixture_account("buyer");
        let court = fixture_account("court");
        let asset_definition = fixture_asset_definition_id();
        let escrow_id = fixture_escrow_id("indexed-query");
        let amount = Numeric::new(40_u32, 0);
        let state = state_with_parties(
            &seller,
            &buyer,
            &court,
            &asset_definition,
            Numeric::new(100_u32, 0),
        );
        let mut block = state.block(block_header(1_000));
        let mut tx = block.transaction();
        tx.tx_call_hash = Some(Hash::prehashed([0xA3; Hash::LENGTH]));

        OpenAssetEscrow {
            escrow_id,
            asset_definition: asset_definition.clone(),
            amount,
            evidence_hashes: Vec::new(),
        }
        .execute(&seller, &mut tx)
        .expect("open escrow");
        AcceptAssetEscrow { escrow_id }
            .execute(&buyer, &mut tx)
            .expect("accept escrow");
        MarkEscrowPaymentSent { escrow_id }
            .execute(&buyer, &mut tx)
            .expect("mark payment sent");

        assert!(
            tx.world
                .asset_escrows_by_seller
                .get(&seller)
                .expect("seller index entry")
                .contains(&escrow_id)
        );
        assert!(
            tx.world
                .asset_escrows_by_buyer
                .get(&buyer)
                .expect("buyer index entry")
                .contains(&escrow_id)
        );
        assert!(
            tx.world
                .asset_escrows_by_status
                .get(&AssetEscrowStatus::PaymentSent)
                .expect("payment-sent index entry")
                .contains(&escrow_id)
        );
        let stale_open_entry = tx
            .world
            .asset_escrows_by_status
            .get(&AssetEscrowStatus::Open)
            .is_some_and(|escrows| escrows.contains(&escrow_id));
        assert!(
            !stale_open_entry,
            "status update should remove stale open index entry"
        );
        let stale_accepted_entry = tx
            .world
            .asset_escrows_by_status
            .get(&AssetEscrowStatus::Accepted)
            .is_some_and(|escrows| escrows.contains(&escrow_id));
        assert!(
            !stale_accepted_entry,
            "status update should remove stale accepted index entry"
        );

        let by_seller = FindAssetEscrowsBySeller {
            seller: seller.clone(),
        }
        .execute(CompoundPredicate::PASS, &tx)
        .expect("query escrows by seller")
        .map(|record| record.id)
        .collect::<Vec<_>>();
        assert_eq!(by_seller, vec![escrow_id]);

        let by_buyer = FindAssetEscrowsByBuyer {
            buyer: buyer.clone(),
        }
        .execute(CompoundPredicate::PASS, &tx)
        .expect("query escrows by buyer")
        .map(|record| record.id)
        .collect::<Vec<_>>();
        assert_eq!(by_buyer, vec![escrow_id]);

        let by_status = FindAssetEscrowsByStatus {
            status: AssetEscrowStatus::PaymentSent,
        }
        .execute(CompoundPredicate::PASS, &tx)
        .expect("query escrows by status")
        .map(|record| record.id)
        .collect::<Vec<_>>();
        assert_eq!(by_status, vec![escrow_id]);

        let by_generic_id = query_asset_escrow_ids(
            &tx,
            CompoundPredicate::<AssetEscrowRecord>::build(|predicate| {
                predicate.equals("id", escrow_id)
            }),
        );
        assert_eq!(by_generic_id, vec![escrow_id]);

        let by_generic_seller = query_asset_escrow_ids(
            &tx,
            CompoundPredicate::<AssetEscrowRecord>::build(|predicate| {
                predicate.equals("seller", seller.clone())
            }),
        );
        assert_eq!(by_generic_seller, vec![escrow_id]);

        let by_generic_buyer = query_asset_escrow_ids(
            &tx,
            CompoundPredicate::<AssetEscrowRecord>::build(|predicate| {
                predicate.equals("buyer", buyer.clone())
            }),
        );
        assert_eq!(by_generic_buyer, vec![escrow_id]);

        let by_generic_status = query_asset_escrow_ids(
            &tx,
            CompoundPredicate::<AssetEscrowRecord>::build(|predicate| {
                predicate.in_values(
                    "status",
                    [AssetEscrowStatus::Open, AssetEscrowStatus::PaymentSent],
                )
            }),
        );
        assert_eq!(by_generic_status, vec![escrow_id]);

        let missing_buyer = fixture_account("missing-escrow-buyer");
        let missing = query_asset_escrow_ids(
            &tx,
            CompoundPredicate::<AssetEscrowRecord>::build(|predicate| {
                predicate
                    .equals("status", AssetEscrowStatus::PaymentSent)
                    .equals("buyer", missing_buyer)
            }),
        );
        assert!(missing.is_empty());
    }

    #[test]
    fn asset_escrow_candidates_intersect_status_and_buyer_indexes() {
        let seller = fixture_account("candidate-seller");
        let buyer = fixture_account("candidate-buyer");
        let wrong_buyer = fixture_account("candidate-wrong-buyer");
        let wrong_seller = fixture_account("candidate-wrong-seller");
        let court = fixture_account("candidate-court");
        let asset_definition = fixture_asset_definition_id();
        let state = state_with_parties(
            &seller,
            &buyer,
            &court,
            &asset_definition,
            Numeric::new(100_u32, 0),
        );
        let mut block = state.block(block_header(1_000));
        let mut tx = block.transaction();
        let target = fixture_escrow_id("asset-candidate-target");
        let same_status = fixture_escrow_id("asset-candidate-same-status");
        let same_buyer = fixture_escrow_id("asset-candidate-same-buyer");

        for record in [
            asset_escrow_fixture(
                target,
                seller.clone(),
                Some(buyer.clone()),
                asset_definition.clone(),
                AssetEscrowStatus::PaymentSent,
            ),
            asset_escrow_fixture(
                same_status,
                seller,
                Some(wrong_buyer),
                asset_definition.clone(),
                AssetEscrowStatus::PaymentSent,
            ),
            asset_escrow_fixture(
                same_buyer,
                wrong_seller,
                Some(buyer.clone()),
                asset_definition,
                AssetEscrowStatus::Accepted,
            ),
        ] {
            tx.world.insert_asset_escrow_entry(record);
        }

        let predicate = CompoundPredicate::<AssetEscrowRecord>::build(|predicate| {
            predicate
                .equals("status", AssetEscrowStatus::PaymentSent)
                .equals("buyer", buyer)
        });
        let predicate_json = parse_predicate_json(&predicate);
        let candidate_ids = asset_escrow_candidate_ids(&predicate_json, &tx.world)
            .expect("indexed asset escrow candidates");

        assert_eq!(candidate_ids, std::collections::BTreeSet::from([target]));
    }

    #[test]
    fn anonymous_asset_escrow_candidates_intersect_status_and_buyer_indexes() {
        let seller = fixture_account("anonymous-candidate-seller");
        let buyer = fixture_account("anonymous-candidate-buyer");
        let wrong_buyer = fixture_account("anonymous-candidate-wrong-buyer");
        let wrong_seller = fixture_account("anonymous-candidate-wrong-seller");
        let court = fixture_account("anonymous-candidate-court");
        let asset_definition = fixture_asset_definition_id();
        let state = state_with_parties(
            &seller,
            &buyer,
            &court,
            &asset_definition,
            Numeric::new(100_u32, 0),
        );
        let mut block = state.block(block_header(1_000));
        let mut tx = block.transaction();
        let target = fixture_escrow_id("anonymous-candidate-target");
        let same_status = fixture_escrow_id("anonymous-candidate-same-status");
        let same_buyer = fixture_escrow_id("anonymous-candidate-same-buyer");

        for record in [
            anonymous_escrow_fixture(
                target,
                seller.clone(),
                Some(buyer.clone()),
                asset_definition.clone(),
                AssetEscrowStatus::PaymentSent,
            ),
            anonymous_escrow_fixture(
                same_status,
                seller,
                Some(wrong_buyer),
                asset_definition.clone(),
                AssetEscrowStatus::PaymentSent,
            ),
            anonymous_escrow_fixture(
                same_buyer,
                wrong_seller,
                Some(buyer.clone()),
                asset_definition,
                AssetEscrowStatus::Accepted,
            ),
        ] {
            tx.world.insert_anonymous_asset_escrow_entry(record);
        }

        let predicate = CompoundPredicate::<AnonymousAssetEscrowRecord>::build(|predicate| {
            predicate
                .equals("status", AssetEscrowStatus::PaymentSent)
                .equals("buyer", buyer)
        });
        let predicate_json = parse_predicate_json(&predicate);
        let candidate_ids = anonymous_asset_escrow_candidate_ids(&predicate_json, &tx.world)
            .expect("indexed anonymous asset escrow candidates");

        assert_eq!(candidate_ids, std::collections::BTreeSet::from([target]));
    }

    #[test]
    fn escrow_open_and_release_record_transfer_artifacts() {
        let seller = fixture_account("seller");
        let buyer = fixture_account("buyer");
        let court = fixture_account("court");
        let asset_definition = fixture_asset_definition_id();
        let escrow_id = fixture_escrow_id("release-artifacts");
        let amount = Numeric::new(40_u32, 0);
        let state = state_with_parties(
            &seller,
            &buyer,
            &court,
            &asset_definition,
            Numeric::new(100_u32, 0),
        );
        let mut block = state.block(block_header(1_000));
        let mut tx = block.transaction();
        let call_hash = Hash::prehashed([0xA1; Hash::LENGTH]);
        tx.tx_call_hash = Some(call_hash);

        OpenAssetEscrow {
            escrow_id,
            asset_definition: asset_definition.clone(),
            amount: amount.clone(),
            evidence_hashes: Vec::new(),
        }
        .execute(&seller, &mut tx)
        .expect("open escrow");
        let record = escrow_record(&tx, &escrow_id);
        let custody = record.custody.clone();
        let seller_asset = AssetId::of(asset_definition.clone(), seller.clone());
        let custody_asset = AssetId::of(asset_definition.clone(), custody.clone());
        let open_events = asset_transfer_events(&tx.world.take_external_events());
        assert_asset_transfer_event(&open_events, "removed", &seller_asset, &amount);
        assert_asset_transfer_event(&open_events, "added", &custody_asset, &amount);

        AcceptAssetEscrow { escrow_id }
            .execute(&buyer, &mut tx)
            .expect("accept escrow");
        MarkEscrowPaymentSent { escrow_id }
            .execute(&buyer, &mut tx)
            .expect("mark payment sent");
        ReleaseAssetEscrow { escrow_id }
            .execute(&seller, &mut tx)
            .expect("release escrow");
        let buyer_asset = AssetId::of(asset_definition.clone(), buyer.clone());
        let release_events = asset_transfer_events(&tx.world.take_external_events());
        assert_asset_transfer_event(&release_events, "removed", &custody_asset, &amount);
        assert_asset_transfer_event(&release_events, "added", &buyer_asset, &amount);

        tx.apply();
        let transcripts = block.drain_transfer_transcripts();
        let entry = transcripts
            .get(&call_hash)
            .expect("escrow transfer transcripts");
        assert_eq!(entry.len(), 2);
        assert_eq!(entry[0].deltas.len(), 1);
        assert_eq!(entry[1].deltas.len(), 1);
        assert_transfer_delta(
            &entry[0].deltas[0],
            &seller,
            &custody,
            &asset_definition,
            &amount,
        );
        assert_transfer_delta(
            &entry[1].deltas[0],
            &custody,
            &buyer,
            &asset_definition,
            &amount,
        );
    }

    #[test]
    fn escrow_release_rechecks_transparent_transfer_policy() {
        let seller = fixture_account("seller");
        let buyer = fixture_account("buyer");
        let court = fixture_account("court");
        let asset_definition = fixture_asset_definition_id();
        let escrow_id = fixture_escrow_id("release-policy");
        let amount = Numeric::new(40_u32, 0);
        let state = state_with_parties(
            &seller,
            &buyer,
            &court,
            &asset_definition,
            Numeric::new(100_u32, 0),
        );
        let mut block = state.block(block_header(1_000));
        let mut tx = block.transaction();
        seed_test_call_hash(&mut tx, 0xB1);

        OpenAssetEscrow {
            escrow_id,
            asset_definition: asset_definition.clone(),
            amount: amount.clone(),
            evidence_hashes: Vec::new(),
        }
        .execute(&seller, &mut tx)
        .expect("open escrow");
        AcceptAssetEscrow { escrow_id }
            .execute(&buyer, &mut tx)
            .expect("accept escrow");
        MarkEscrowPaymentSent { escrow_id }
            .execute(&buyer, &mut tx)
            .expect("mark payment sent");

        tx.world
            .asset_definition_mut(&asset_definition)
            .expect("asset definition")
            .set_confidential_policy(AssetConfidentialPolicy::shielded_only());

        let err = ReleaseAssetEscrow { escrow_id }
            .execute(&seller, &mut tx)
            .expect_err("release must obey current transparent transfer policy");
        assert!(
            err.to_string()
                .contains("transparent transfer not permitted by policy"),
            "unexpected error: {err}"
        );

        let record = escrow_record(&tx, &escrow_id);
        assert_eq!(record.status, AssetEscrowStatus::PaymentSent);
        assert_eq!(balance(&tx, &record.custody, &asset_definition), amount);
        assert_eq!(balance(&tx, &buyer, &asset_definition), Numeric::zero());
    }

    #[test]
    fn escrow_release_rejects_frozen_custody_outbound_asset() {
        let seller = fixture_account("seller");
        let buyer = fixture_account("buyer");
        let court = fixture_account("court");
        let asset_definition = fixture_asset_definition_id();
        let escrow_id = fixture_escrow_id("release-custody-freeze");
        let amount = Numeric::new(40_u32, 0);
        let state = state_with_parties(
            &seller,
            &buyer,
            &court,
            &asset_definition,
            Numeric::new(100_u32, 0),
        );
        let mut block = state.block(block_header(1_000));
        let mut tx = block.transaction();
        seed_test_call_hash(&mut tx, 0xB2);

        OpenAssetEscrow {
            escrow_id,
            asset_definition: asset_definition.clone(),
            amount: amount.clone(),
            evidence_hashes: Vec::new(),
        }
        .execute(&seller, &mut tx)
        .expect("open escrow");
        let custody = escrow_record(&tx, &escrow_id).custody;
        freeze_outbound_asset_transfers(&mut tx, &seller, &custody, &asset_definition);
        AcceptAssetEscrow { escrow_id }
            .execute(&buyer, &mut tx)
            .expect("accept escrow");
        MarkEscrowPaymentSent { escrow_id }
            .execute(&buyer, &mut tx)
            .expect("mark payment sent");

        let err = ReleaseAssetEscrow { escrow_id }
            .execute(&seller, &mut tx)
            .expect_err("custody outbound freeze must block release");
        assert!(
            err.to_string().contains("frozen"),
            "unexpected error: {err}"
        );

        let record = escrow_record(&tx, &escrow_id);
        assert_eq!(record.status, AssetEscrowStatus::PaymentSent);
        assert_eq!(balance(&tx, &record.custody, &asset_definition), amount);
        assert_eq!(balance(&tx, &buyer, &asset_definition), Numeric::zero());
    }

    #[test]
    fn escrow_cancel_before_payment_refunds_seller() {
        let seller = fixture_account("seller");
        let buyer = fixture_account("buyer");
        let court = fixture_account("court");
        let asset_definition = fixture_asset_definition_id();
        let escrow_id = fixture_escrow_id("cancel");
        let state = state_with_parties(
            &seller,
            &buyer,
            &court,
            &asset_definition,
            Numeric::new(100_u32, 0),
        );
        let mut block = state.block(block_header(2_000));
        let mut tx = block.transaction();
        seed_test_call_hash(&mut tx, 0xB3);

        OpenAssetEscrow {
            escrow_id,
            asset_definition: asset_definition.clone(),
            amount: Numeric::new(40_u32, 0),
            evidence_hashes: Vec::new(),
        }
        .execute(&seller, &mut tx)
        .expect("open escrow");
        AcceptAssetEscrow { escrow_id }
            .execute(&buyer, &mut tx)
            .expect("accept escrow");
        CancelAssetEscrow { escrow_id }
            .execute(&seller, &mut tx)
            .expect("cancel escrow");

        let record = escrow_record(&tx, &escrow_id);
        assert_eq!(record.status, AssetEscrowStatus::Cancelled);
        assert_eq!(
            balance(&tx, &seller, &asset_definition),
            Numeric::new(100_u32, 0)
        );
        assert_eq!(balance(&tx, &buyer, &asset_definition), Numeric::zero());
    }

    #[test]
    fn escrow_cancel_rejects_frozen_custody_outbound_asset() {
        let seller = fixture_account("seller");
        let buyer = fixture_account("buyer");
        let court = fixture_account("court");
        let asset_definition = fixture_asset_definition_id();
        let escrow_id = fixture_escrow_id("cancel-custody-freeze");
        let amount = Numeric::new(40_u32, 0);
        let state = state_with_parties(
            &seller,
            &buyer,
            &court,
            &asset_definition,
            Numeric::new(100_u32, 0),
        );
        let mut block = state.block(block_header(2_000));
        let mut tx = block.transaction();
        seed_test_call_hash(&mut tx, 0xB4);

        OpenAssetEscrow {
            escrow_id,
            asset_definition: asset_definition.clone(),
            amount: amount.clone(),
            evidence_hashes: Vec::new(),
        }
        .execute(&seller, &mut tx)
        .expect("open escrow");
        let custody = escrow_record(&tx, &escrow_id).custody;
        freeze_outbound_asset_transfers(&mut tx, &seller, &custody, &asset_definition);

        let err = CancelAssetEscrow { escrow_id }
            .execute(&seller, &mut tx)
            .expect_err("custody outbound freeze must block cancellation");
        assert!(
            err.to_string().contains("frozen"),
            "unexpected error: {err}"
        );

        let record = escrow_record(&tx, &escrow_id);
        assert_eq!(record.status, AssetEscrowStatus::Open);
        assert_eq!(balance(&tx, &record.custody, &asset_definition), amount);
        assert_eq!(
            balance(&tx, &seller, &asset_definition),
            Numeric::new(60_u32, 0)
        );
    }

    #[test]
    fn escrow_dispute_requires_court_permission_and_valid_split() {
        let seller = fixture_account("seller");
        let buyer = fixture_account("buyer");
        let court = fixture_account("court");
        let asset_definition = fixture_asset_definition_id();
        let escrow_id = fixture_escrow_id("dispute");
        let state = state_with_parties(
            &seller,
            &buyer,
            &court,
            &asset_definition,
            Numeric::new(100_u32, 0),
        );
        let mut block = state.block(block_header(3_000));
        let mut tx = block.transaction();
        seed_test_call_hash(&mut tx, 0xB5);

        OpenAssetEscrow {
            escrow_id,
            asset_definition: asset_definition.clone(),
            amount: Numeric::new(40_u32, 0),
            evidence_hashes: vec![Hash::new("open-evidence")],
        }
        .execute(&seller, &mut tx)
        .expect("open escrow");
        AcceptAssetEscrow { escrow_id }
            .execute(&buyer, &mut tx)
            .expect("accept escrow");
        OpenEscrowDispute {
            escrow_id,
            evidence_hashes: vec![Hash::new("dispute-evidence")],
        }
        .execute(&buyer, &mut tx)
        .expect("open dispute");

        assert!(
            ResolveEscrowDispute {
                escrow_id,
                buyer_amount: Numeric::new(20_u32, 0),
                seller_amount: Numeric::new(20_u32, 0),
                evidence_hashes: Vec::new(),
            }
            .execute(&seller, &mut tx)
            .is_err(),
            "seller cannot resolve dispute without court permission"
        );

        grant_court_permission(&mut tx, &court);
        assert!(
            ResolveEscrowDispute {
                escrow_id,
                buyer_amount: Numeric::new(20_u32, 0),
                seller_amount: Numeric::new(19_u32, 0),
                evidence_hashes: Vec::new(),
            }
            .execute(&court, &mut tx)
            .is_err(),
            "court split must exactly match held amount"
        );

        ResolveEscrowDispute {
            escrow_id,
            buyer_amount: Numeric::new(25_u32, 0),
            seller_amount: Numeric::new(15_u32, 0),
            evidence_hashes: vec![Hash::new("resolution-evidence")],
        }
        .execute(&court, &mut tx)
        .expect("resolve dispute");

        let record = escrow_record(&tx, &escrow_id);
        assert_eq!(record.status, AssetEscrowStatus::Resolved);
        assert_eq!(
            balance(&tx, &buyer, &asset_definition),
            Numeric::new(25_u32, 0)
        );
        assert_eq!(
            balance(&tx, &seller, &asset_definition),
            Numeric::new(75_u32, 0)
        );
        assert_eq!(
            balance(&tx, &record.custody, &asset_definition),
            Numeric::zero()
        );
    }

    #[test]
    fn escrow_resolution_records_split_transfer_batch() {
        let seller = fixture_account("seller");
        let buyer = fixture_account("buyer");
        let court = fixture_account("court");
        let asset_definition = fixture_asset_definition_id();
        let escrow_id = fixture_escrow_id("resolution-artifacts");
        let amount = Numeric::new(40_u32, 0);
        let buyer_amount = Numeric::new(25_u32, 0);
        let seller_amount = Numeric::new(15_u32, 0);
        let state = state_with_parties(
            &seller,
            &buyer,
            &court,
            &asset_definition,
            Numeric::new(100_u32, 0),
        );
        let mut block = state.block(block_header(3_000));
        let mut tx = block.transaction();
        let call_hash = Hash::prehashed([0xA2; Hash::LENGTH]);
        tx.tx_call_hash = Some(call_hash);

        OpenAssetEscrow {
            escrow_id,
            asset_definition: asset_definition.clone(),
            amount,
            evidence_hashes: Vec::new(),
        }
        .execute(&seller, &mut tx)
        .expect("open escrow");
        let custody = escrow_record(&tx, &escrow_id).custody;
        AcceptAssetEscrow { escrow_id }
            .execute(&buyer, &mut tx)
            .expect("accept escrow");
        OpenEscrowDispute {
            escrow_id,
            evidence_hashes: Vec::new(),
        }
        .execute(&buyer, &mut tx)
        .expect("open dispute");
        grant_court_permission(&mut tx, &court);

        ResolveEscrowDispute {
            escrow_id,
            buyer_amount: buyer_amount.clone(),
            seller_amount: seller_amount.clone(),
            evidence_hashes: Vec::new(),
        }
        .execute(&court, &mut tx)
        .expect("resolve dispute");

        tx.apply();
        let transcripts = block.drain_transfer_transcripts();
        let entry = transcripts
            .get(&call_hash)
            .expect("escrow transfer transcripts");
        assert_eq!(entry.len(), 2);
        let resolution = entry
            .iter()
            .find(|transcript| transcript.deltas.len() == 2)
            .expect("resolution transfer batch");
        assert!(resolution.poseidon_preimage_digest.is_none());
        assert_transfer_delta(
            &resolution.deltas[0],
            &custody,
            &buyer,
            &asset_definition,
            &buyer_amount,
        );
        assert_transfer_delta(
            &resolution.deltas[1],
            &custody,
            &seller,
            &asset_definition,
            &seller_amount,
        );
    }

    #[test]
    fn escrow_resolution_rejects_frozen_custody_outbound_asset() {
        let seller = fixture_account("seller");
        let buyer = fixture_account("buyer");
        let court = fixture_account("court");
        let asset_definition = fixture_asset_definition_id();
        let escrow_id = fixture_escrow_id("resolution-custody-freeze");
        let amount = Numeric::new(40_u32, 0);
        let state = state_with_parties(
            &seller,
            &buyer,
            &court,
            &asset_definition,
            Numeric::new(100_u32, 0),
        );
        let mut block = state.block(block_header(3_000));
        let mut tx = block.transaction();
        seed_test_call_hash(&mut tx, 0xB6);

        OpenAssetEscrow {
            escrow_id,
            asset_definition: asset_definition.clone(),
            amount: amount.clone(),
            evidence_hashes: Vec::new(),
        }
        .execute(&seller, &mut tx)
        .expect("open escrow");
        let custody = escrow_record(&tx, &escrow_id).custody;
        AcceptAssetEscrow { escrow_id }
            .execute(&buyer, &mut tx)
            .expect("accept escrow");
        OpenEscrowDispute {
            escrow_id,
            evidence_hashes: Vec::new(),
        }
        .execute(&buyer, &mut tx)
        .expect("open dispute");
        grant_court_permission(&mut tx, &court);
        freeze_outbound_asset_transfers(&mut tx, &seller, &custody, &asset_definition);

        let err = ResolveEscrowDispute {
            escrow_id,
            buyer_amount: Numeric::new(25_u32, 0),
            seller_amount: Numeric::new(15_u32, 0),
            evidence_hashes: Vec::new(),
        }
        .execute(&court, &mut tx)
        .expect_err("custody outbound freeze must block resolution");
        assert!(
            err.to_string().contains("frozen"),
            "unexpected error: {err}"
        );

        let record = escrow_record(&tx, &escrow_id);
        assert_eq!(record.status, AssetEscrowStatus::Disputed);
        assert_eq!(balance(&tx, &record.custody, &asset_definition), amount);
        assert_eq!(balance(&tx, &buyer, &asset_definition), Numeric::zero());
        assert_eq!(
            balance(&tx, &seller, &asset_definition),
            Numeric::new(60_u32, 0)
        );
    }

    #[test]
    fn generic_debits_from_native_escrow_custody_are_rejected() {
        let seller = fixture_account("seller");
        let buyer = fixture_account("buyer");
        let court = fixture_account("court");
        let asset_definition = fixture_asset_definition_id();
        let escrow_id = fixture_escrow_id("direct-transfer");
        let state = state_with_parties(
            &seller,
            &buyer,
            &court,
            &asset_definition,
            Numeric::new(100_u32, 0),
        );
        let mut block = state.block(block_header(4_000));
        let mut tx = block.transaction();
        seed_test_call_hash(&mut tx, 0xB7);

        OpenAssetEscrow {
            escrow_id,
            asset_definition: asset_definition.clone(),
            amount: Numeric::new(40_u32, 0),
            evidence_hashes: Vec::new(),
        }
        .execute(&seller, &mut tx)
        .expect("open escrow");

        let record = escrow_record(&tx, &escrow_id);
        let custody_asset = AssetId::of(asset_definition.clone(), record.custody.clone());
        assert!(
            Transfer::asset_numeric(custody_asset.clone(), Numeric::new(1_u32, 0), buyer.clone())
                .execute(&seller, &mut tx)
                .is_err(),
            "generic asset transfer must not drain active native escrow custody"
        );
        assert!(
            Burn::asset_numeric(Numeric::new(1_u32, 0), custody_asset.clone())
                .execute(&seller, &mut tx)
                .is_err(),
            "generic asset burn must not drain active native escrow custody"
        );

        AcceptAssetEscrow { escrow_id }
            .execute(&buyer, &mut tx)
            .expect("accept escrow");
        MarkEscrowPaymentSent { escrow_id }
            .execute(&buyer, &mut tx)
            .expect("mark payment sent");
        ReleaseAssetEscrow { escrow_id }
            .execute(&seller, &mut tx)
            .expect("release escrow");
        state_transaction_deposit_closed_custody_dust(
            &mut tx,
            &custody_asset,
            Numeric::new(1_u32, 0),
        );

        assert!(
            Transfer::asset_numeric(custody_asset.clone(), Numeric::new(1_u32, 0), buyer)
                .execute(&seller, &mut tx)
                .is_err(),
            "generic asset transfer must not drain recorded native escrow custody after close"
        );
        assert!(
            Burn::asset_numeric(Numeric::new(1_u32, 0), custody_asset.clone())
                .execute(&seller, &mut tx)
                .is_err(),
            "generic asset burn must not drain recorded native escrow custody after close"
        );
    }

    #[test]
    fn generic_debits_from_native_asset_lock_custody_are_rejected() {
        let source = fixture_account("lock-direct-source");
        let destination = fixture_account("lock-direct-destination");
        let observer = fixture_account("lock-direct-observer");
        let asset_definition = fixture_asset_definition_id();
        let escrow_id = fixture_escrow_id("lock-direct-transfer");
        let state = state_with_parties(
            &source,
            &destination,
            &observer,
            &asset_definition,
            Numeric::new(100_u32, 0),
        );
        let mut block = state.block(block_header(4_100));
        let mut tx = block.transaction();
        seed_test_call_hash(&mut tx, 0xC8);

        OpenAssetLock::new(
            escrow_id,
            asset_definition.clone(),
            destination.clone(),
            Numeric::new(40_u32, 0),
        )
        .execute(&source, &mut tx)
        .expect("open asset lock");

        let record = escrow_record(&tx, &escrow_id);
        let custody_asset = AssetId::of(asset_definition.clone(), record.custody.clone());
        assert!(
            Transfer::asset_numeric(
                custody_asset.clone(),
                Numeric::new(1_u32, 0),
                destination.clone()
            )
            .execute(&source, &mut tx)
            .is_err(),
            "generic asset transfer must not drain active native lock custody"
        );
        assert!(
            Burn::asset_numeric(Numeric::new(1_u32, 0), custody_asset.clone())
                .execute(&source, &mut tx)
                .is_err(),
            "generic asset burn must not drain active native lock custody"
        );

        DrawdownAssetLock::new(escrow_id, Numeric::new(40_u32, 0))
            .execute(&destination, &mut tx)
            .expect("draw down lock");
        state_transaction_deposit_closed_custody_dust(
            &mut tx,
            &custody_asset,
            Numeric::new(1_u32, 0),
        );

        assert!(
            Transfer::asset_numeric(
                custody_asset.clone(),
                Numeric::new(1_u32, 0),
                destination.clone()
            )
            .execute(&source, &mut tx)
            .is_err(),
            "generic asset transfer must not drain recorded native lock custody after close"
        );
        assert!(
            Burn::asset_numeric(Numeric::new(1_u32, 0), custody_asset.clone())
                .execute(&source, &mut tx)
                .is_err(),
            "generic asset burn must not drain recorded native lock custody after close"
        );
    }
}
