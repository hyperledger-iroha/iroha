//! Protected epoch authority and finality for completed Spartan activation and Unload.
use super::custody_archive::{Checkpoint, Objects, storage_error};
use super::*;
use crate::{
    kagemusha_wallet_advance_v1::KagemushaWalletSlotIdV1 as Slot,
    kagemusha_wallet_state_v1::{self as state, IndexRoot, ObjectStore},
};
use iroha_data_model::sumeragi_finality::{
    MAX_COMMIT_CERTIFICATE_BYTES_V1, MAX_COMMIT_CHECKPOINT_BYTES, SumeragiCommitCertificateV1,
    SumeragiCommitCheckpointV1, SumeragiCommitVerifierV1,
};

fn invalid(field: &'static str) -> ProviderError {
    ProviderError::Invalid { field }
}
fn key(domain: &[u8; 8], id: &[u8]) -> [u8; 32] {
    let mut h = blake3::Hasher::new();
    h.update(domain);
    h.update(id);
    *h.finalize().as_bytes()
}

impl<F: KagemushaWalletFsV1, P: KagemushaWalletPlatformV1 + 'static, S> FutureRuntime<F, P, S> {
    fn selected_epoch_record(
        &mut self,
        slot: &Slot,
        epoch: u64,
    ) -> Result<Option<SumeragiCommitCheckpointV1>, ProviderError> {
        self.require_account(slot, None)?;
        let wallet = self.authorized_wallet_id(slot)?;
        self.selected_epoch_for_wallet(slot, wallet, epoch)
    }

    fn selected_epoch_for_wallet(
        &mut self,
        slot: &Slot,
        wallet: [u8; 32],
        epoch: u64,
    ) -> Result<Option<SumeragiCommitCheckpointV1>, ProviderError> {
        let base = self.parameters.genesis_commit_verifier();
        let initial = base.initial_commit_epoch();
        if epoch == initial {
            return base
                .export_epoch_checkpoint(initial)
                .map(Some)
                .map_err(|_| invalid("installed genesis epoch"));
        }
        if epoch < initial {
            return Err(invalid("Load or settlement epoch predates installed genesis"));
        }
        let (_, marker) = self.custody_checkpoint(slot, wallet)?;
        if marker.epoch_tip.is_none_or(|tip| epoch > tip) {
            return Ok(None);
        }
        self.provider.with_archive(slot, |archive| {
            let mut store = Objects(archive);
            let row = IndexRoot(marker.epochs)
                .get(&mut store, &key(b"kgepoch1", &epoch.to_be_bytes()))
                .map_err(storage_error)?
                .ok_or_else(|| invalid("missing protected epoch record"))?;
            let refs: &[u8; 64] = row
                .as_slice()
                .try_into()
                .map_err(|_| invalid("protected epoch record"))?;
            let bytes = store
                .read_object(refs[..32].try_into().unwrap(), MAX_COMMIT_CHECKPOINT_BYTES)
                .map_err(storage_error)?;
            // Retain the exact signed boundary original as part of the selected row.
            let boundary = store
                .read_object(
                    refs[32..].try_into().unwrap(),
                    MAX_COMMIT_CERTIFICATE_BYTES_V1,
                )
                .map_err(storage_error)?;
            let selected =
                SumeragiCommitCheckpointV1::decode_canonical(&bytes).map_err(finality_error)?;
            let boundary =
                SumeragiCommitCertificateV1::decode_canonical(&boundary).map_err(finality_error)?;
            let authorization = &selected.selected_epoch().authorization;
            if authorization.epoch != epoch
                || boundary.epoch_id().map_err(finality_error)?.checked_add(1) != Some(epoch)
                || boundary.height().map_err(finality_error)?.checked_add(1)
                    != Some(authorization.first_height)
            {
                return Err(invalid("protected epoch identity"));
            }
            Ok(Some(selected))
        })
    }

    pub(super) fn engine_for_epoch(
        &mut self,
        slot: &Slot,
        wallet: [u8; 32],
        epoch: Option<u64>,
    ) -> Result<native::NativeEngine, ProviderError> {
        self.engine_with_epoch_checkpoint(slot, wallet, epoch)
            .map(|(engine, _)| engine)
    }

    pub(super) fn engine_with_epoch_checkpoint(
        &mut self,
        slot: &Slot,
        wallet: [u8; 32],
        epoch: Option<u64>,
    ) -> Result<(native::NativeEngine, Option<SumeragiCommitCheckpointV1>), ProviderError> {
        let Some(epoch) = epoch else {
            return Ok((self.engine.clone(), None));
        };
        if epoch
            == self
                .parameters
                .genesis_commit_verifier()
                .initial_commit_epoch()
        {
            return Ok((self.engine.clone(), None));
        }
        let checkpoint = self.selected_epoch_for_wallet(slot, wallet, epoch)?.ok_or(
            ProviderError::UnavailableCustodyData {
                object: "certified Load epoch",
            },
        )?;
        let engine = self.engine
            .with_protected_epoch(&self.parameters, &checkpoint)
            .map_err(provider_error)?;
        Ok((engine, Some(checkpoint)))
    }

    pub(super) fn engine_for_capsule(
        &mut self,
        slot: &Slot,
        capsule: &RecoveryCapsule,
    ) -> Result<native::NativeEngine, ProviderError> {
        self.engine_for_epoch(
            slot,
            *capsule.wallet(),
            capsule.load_epoch_selector().map_err(provider_error)?,
        )
    }

    pub(super) fn current_reconciliation_engine(
        &mut self,
        slot: &Slot,
    ) -> Result<native::NativeEngine, ProviderError> {
        use crate::kagemusha_wallet_advance_v1::KagemushaWalletSlotStatusV1 as Status;
        if let Status::Pending(record) = self.provider.status(slot)? {
            let capsule = self.provider.capsule_at_output_marker(slot, &record)?;
            capsule.require_marker(record.marker())?;
            return self.engine_for_capsule(slot, &capsule);
        }
        Ok(self.engine.clone())
    }

    /// Select an installed, genesis-rooted epoch from this wallet's protected archive.
    /// The peer's epoch number selects local data and never supplies a committee.
    pub fn selected_commit_epoch(
        &mut self,
        slot: &Slot,
        epoch: u64,
    ) -> Result<Option<SumeragiCommitVerifierV1>, ProviderError> {
        let base = self.parameters.genesis_commit_verifier();
        self.selected_epoch_record(slot, epoch)?
            .map(|record| {
                base.restore_trusted_local_epoch(&record)
                    .map_err(|_| invalid("protected epoch root"))
            })
            .transpose()
    }

    /// The current authenticated incumbent and its exact next synchronization height.
    pub fn commit_epoch_progress(
        &mut self,
        slot: &Slot,
    ) -> Result<state::NativeEpochProgressV1, ProviderError> {
        self.require_account(slot, None)?;
        let wallet = self.authorized_wallet_id(slot)?;
        let (_, marker) = self.custody_checkpoint(slot, wallet)?;
        let epoch = marker.epoch_tip.unwrap_or_else(|| {
            self.parameters
                .genesis_commit_verifier()
                .initial_commit_epoch()
        });
        let selected = self
            .selected_epoch_record(slot, epoch)?
            .ok_or_else(|| invalid("protected current epoch"))?;
        let term = &selected.selected_epoch().authorization;
        Ok(state::NativeEpochProgressV1 {
            epoch,
            first_height: term.first_height,
            boundary_height: term.last_height,
        })
    }

    /// Advance exactly one epoch with an original authenticated by its already selected incumbent.
    /// A repeated boundary may re-read the same successor without changing its retained original.
    pub fn sync_epoch_boundary(
        &mut self,
        slot: &Slot,
        expected_epoch: u64,
        bytes: &[u8],
    ) -> Result<state::NativeEpochProgressV1, ProviderError> {
        if bytes.is_empty() || bytes.len() > MAX_COMMIT_CERTIFICATE_BYTES_V1 {
            return Err(invalid("epoch certificate bound"));
        }
        let original =
            SumeragiCommitCertificateV1::decode_canonical(bytes).map_err(finality_error)?;
        if original.epoch_id().map_err(finality_error)? != expected_epoch {
            return Err(invalid("wrong epoch boundary"));
        }
        let mut incumbent = self
            .selected_commit_epoch(slot, expected_epoch)?
            .ok_or_else(|| invalid("unknown incumbent epoch"))?;
        let successor = incumbent
            .verify_epoch_boundary(&original)
            .map_err(finality_error)?;
        let next = successor.selected_epoch().authorization.epoch;
        if expected_epoch.checked_add(1) != Some(next) {
            return Err(invalid("epoch progression"));
        }
        let encoded = successor
            .encode_canonical()
            .map_err(|_| invalid("epoch checkpoint size"))?;
        if let Some(old) = self.selected_epoch_record(slot, next)? {
            if old != successor {
                return Err(invalid("conflicting certified epoch"));
            }
            return self.commit_epoch_progress(slot);
        }
        let wallet = self.authorized_wallet_id(slot)?;
        let (root, mut marker) = self.custody_checkpoint(slot, wallet)?;
        if marker
            .epoch_tip
            .unwrap_or_else(|| incumbent.initial_commit_epoch())
            != expected_epoch
        {
            return Err(invalid("epoch cursor changed"));
        }
        marker.epochs = self.provider.with_archive(slot, |archive| {
            let mut store = Objects(archive);
            let checkpoint = store
                .write_object(&encoded, MAX_COMMIT_CHECKPOINT_BYTES)
                .map_err(storage_error)?;
            let boundary = store
                .write_object(bytes, MAX_COMMIT_CERTIFICATE_BYTES_V1)
                .map_err(storage_error)?;
            let mut row = [0; 64];
            row[..32].copy_from_slice(&checkpoint);
            row[32..].copy_from_slice(&boundary);
            Ok(IndexRoot(marker.epochs)
                .set(&mut store, key(b"kgepoch1", &next.to_be_bytes()), &row)
                .map_err(storage_error)?
                .0)
        })?;
        marker.epoch_tip = Some(next);
        self.publish_custody_checkpoint(slot, root, &marker)?;
        self.commit_epoch_progress(slot)
    }
}

use iroha_data_model::NetworkId;
use iroha_data_model::{
    account::AccountId,
    isi::kagemusha_wallet::spartan::{
        ActivateKagemushaSpartanV1 as Activate, UnloadKagemushaSpartanV1 as Unload,
    },
    kagemusha::{KAGEMUSHA_WALLET_LEDGER_FINALITY_MAX_BYTES_V1, KagemushaWalletLedgerFinalityV1},
    transaction::{Executable, SignedTransaction},
};

/// Native outcome of an exact signed envelope. An authenticated rejection applies to this attempt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnloadSettlement {
    /// The exact signed transaction entrypoint selected by the native verifier.
    pub entrypoint: [u8; 32],
    /// Zero while the submitted attempt has no locally authenticated outcome.
    pub height: u64,
    /// Zero before finalization, otherwise the certified native consensus block identity.
    pub block: [u8; 32],
    /// Whether this exact signed envelope was applied.
    pub accepted: bool,
    /// Whether this wallet already observed an applied envelope paying this same completed claim.
    pub paid: bool,
    /// Unknown peer-selected epoch that the wallet must synchronize before ingesting.
    pub epoch_required: Option<u64>,
    /// Unverified inclusion height used only to bound synchronization of that unknown epoch.
    pub required_height: Option<u64>,
}
struct ClaimScope<'a> {
    network: NetworkId,
    scheme: &'a [u8; 32],
    asset: &'a [u8; 32],
    manifest_digest: &'a [u8; 32],
    claim: &'a [u8],
    activation: Option<(&'a [u8; 32], &'a AccountId)>,
}
impl ClaimScope<'_> {
    fn attempt_key(&self, entrypoint: &[u8; 32]) -> [u8; 32] {
        key(
            if self.activation.is_some() {
                b"kgacent1"
            } else {
                b"kgunent1"
            },
            entrypoint,
        )
    }
    fn accepted_key(&self, operation: &[u8; 32]) -> [u8; 32] {
        key(
            if self.activation.is_some() {
                b"kgacok01"
            } else {
                b"kgunpay1"
            },
            operation,
        )
    }
    fn claim_digest(&self) -> [u8; 32] {
        key(
            if self.activation.is_some() {
                b"kgacclm1"
            } else {
                b"kgunclm1"
            },
            self.claim,
        )
    }
    fn envelope(&self, wire: &[u8]) -> Result<SignedTransaction, ProviderError> {
        let signed =
            state::signed_unload_relay_original(wire, &self.network).map_err(storage_error)?;
        if signed.attachments().is_some() {
            return Err(invalid("ledger envelope attachments"));
        }
        signed
            .payload()
            .validate_fee_payment_intent()
            .map_err(|_| invalid("ledger fee intent"))?;
        let Executable::Instructions(all) = signed.instructions() else {
            return Err(invalid("ledger instruction vector"));
        };
        if all.len() != 1 {
            return Err(invalid("single ledger original"));
        }
        if let Some((wallet, account)) = self.activation {
            let activate = all[0]
                .as_any()
                .downcast_ref::<Activate>()
                .ok_or_else(|| invalid("Activate instruction type"))?;
            if signed.authority() != account
                || activate.scheme != *self.scheme
                || activate.asset != *self.asset
                || activate.wallet != *wallet
                || activate.manifest_digest != *self.manifest_digest
                || activate.activation != self.claim
            {
                return Err(invalid(
                    "Activate instruction differs from retained Bootstrap",
                ));
            }
            return Ok(signed);
        }
        let unload = all[0]
            .as_any()
            .downcast_ref::<Unload>()
            .ok_or_else(|| invalid("Unload instruction type"))?;
        if unload.scheme != *self.scheme
            || unload.asset != *self.asset
            || unload.manifest_digest != *self.manifest_digest
            || unload.claim != self.claim
        {
            return Err(invalid("Unload instruction differs from retained claim"));
        }
        Ok(signed)
    }
}

/// Only a verified, released capsule can introduce this original; the same selected root
/// permanently retains it alongside the first certified attempt, independently of capsule collection.
fn archived_claim(
    store: &mut impl state::ObjectStore,
    root: IndexRoot,
    operation: &[u8; 32],
) -> Result<Option<([u8; 32], Vec<u8>)>, ProviderError> {
    let Some(reference) = root
        .get(store, &key(b"kgunsrc1", operation))
        .map_err(storage_error)?
    else {
        return Ok(None);
    };
    let raw: &[u8; 64] = reference
        .as_slice()
        .try_into()
        .map_err(|_| invalid("protected Unload claim reference"))?;
    let asset = raw[..32].try_into().expect("fixed asset digest");
    let digest = raw[32..].try_into().expect("fixed original digest");
    let claim = store
        .read_object(digest, native::MAX_UNLOAD_CLAIM_BYTES)
        .map_err(storage_error)?;
    Ok(Some((asset, claim)))
}

impl ClaimScope<'_> {
    fn record(
        &self,
        store: &mut impl state::ObjectStore,
        root: IndexRoot,
        operation: &[u8; 32],
        signed: &[u8],
        proof: &[u8],
        verified: iroha_data_model::kagemusha::VerifiedKagemushaLedgerOutcomeV1,
        paid: bool,
    ) -> Result<IndexRoot, ProviderError> {
        let entrypoint = verified.entrypoint();
        let row = Row {
            operation: *operation,
            claim: self.claim_digest(),
            signed: store
                .write_object(signed, 64 * 1024)
                .map_err(storage_error)?,
            finality: store
                .write_object(proof, KAGEMUSHA_WALLET_LEDGER_FINALITY_MAX_BYTES_V1)
                .map_err(storage_error)?,
            entrypoint,
            height: verified.height(),
            block: verified.commit_hash(),
            accepted: verified.accepted(),
        };
        let bytes =
            norito::encode_canonical(&row).map_err(|_| invalid("ledger outcome encoding"))?;
        let claim_root = if self.activation.is_some() {
            root
        } else {
            match archived_claim(store, root, operation)? {
                Some((asset, original)) if asset == *self.asset && original == self.claim => root,
                Some(_) => return Err(invalid("conflicting protected Unload claim")),
                None => {
                    let mut reference = [0u8; 64];
                    reference[..32].copy_from_slice(self.asset);
                    reference[32..].copy_from_slice(
                        &store
                            .write_object(self.claim, native::MAX_UNLOAD_CLAIM_BYTES)
                            .map_err(storage_error)?,
                    );
                    root.set(store, key(b"kgunsrc1", operation), &reference)
                        .map_err(storage_error)?
                }
            }
        };
        let updated = claim_root
            .set(store, self.attempt_key(&entrypoint), &bytes)
            .map_err(storage_error)?;
        if row.accepted && !paid {
            updated
                .set(store, self.accepted_key(operation), &bytes)
                .map_err(storage_error)
        } else {
            Ok(updated)
        }
    }
}

#[derive(norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha.kagemusha.spartan.UnloadSettlement.v1")]
struct Row {
    operation: [u8; 32],
    claim: [u8; 32],
    signed: [u8; 32],
    finality: [u8; 32],
    entrypoint: [u8; 32],
    height: u64,
    block: [u8; 32],
    accepted: bool,
}
fn decode_model(bytes: &[u8]) -> Result<KagemushaWalletLedgerFinalityV1, ProviderError> {
    state::read_model_original(
        || KagemushaWalletLedgerFinalityV1::decode_canonical(bytes),
        state::Error::Invalid("ledger finality original"),
    )
    .map_err(storage_error)
}
fn finality_error(e: iroha_data_model::sumeragi_finality::FinalityReadError) -> ProviderError {
    storage_error(state::finality_read_error(
        e,
        state::Error::Invalid("certified ledger result"),
    ))
}
fn retained_row(
    store: &mut impl state::ObjectStore,
    root: IndexRoot,
    k: &[u8; 32],
    op: &[u8; 32],
    scope: &ClaimScope<'_>,
) -> Result<Option<Row>, ProviderError> {
    let Some(raw) = root.get(store, k).map_err(storage_error)? else {
        return Ok(None);
    };
    let row: Row = norito::core::classify_decode_attempt(|| {
        norito::decode_canonical_with_limits(&raw, norito::canonical_decode_limits(raw.len()))
    })
    .map_err(|error| {
        storage_error(state::decode_attempt_error(
            error,
            state::Error::WitnessLost("protected ledger outcome"),
        ))
    })?;
    let signed = store
        .read_object(&row.signed, 64 * 1024)
        .map_err(storage_error)?;
    let tx = scope.envelope(&signed)?;
    let evidence = store
        .read_object(&row.finality, KAGEMUSHA_WALLET_LEDGER_FINALITY_MAX_BYTES_V1)
        .map_err(storage_error)?;
    let original = decode_model(&evidence)?;
    let bindings = original.custody_bindings().map_err(finality_error)?;
    if row.operation != *op
        || row.claim != scope.claim_digest()
        || row.entrypoint != *tx.hash_as_entrypoint().as_ref()
        || row.entrypoint != *original.entrypoint_hash.as_ref()
        || bindings.height != row.height
        || bindings.qc_block_hash != row.block
        || bindings.accepted != row.accepted
    {
        return Err(invalid("inconsistent protected ledger outcome"));
    }
    Ok(Some(row))
}

impl<F: KagemushaWalletFsV1, P: KagemushaWalletPlatformV1 + 'static, S> FutureRuntime<F, P, S> {
    /// Verify direct global BLS finality and classify one exact envelope for a locally released Unload.
    /// An empty proof reads the protected outcome. A retry cannot clear an earlier payout.
    pub fn unload_settlement(
        &mut self,
        slot: &Slot,
        operation: &[u8; 32],
        signed_wire: &[u8],
        proof: &[u8],
    ) -> Result<UnloadSettlement, ProviderError> {
        if proof.len() > KAGEMUSHA_WALLET_LEDGER_FINALITY_MAX_BYTES_V1
            || signed_wire.is_empty()
            || signed_wire.len() > 64 * 1024
        {
            return Err(invalid("Unload original bounds"));
        }
        self.require_account(slot, None)?;
        let wallet = self.authorized_wallet_id(slot)?;
        let (origin, checkpoint) = self.custody_checkpoint(slot, wallet)?;
        let root = IndexRoot(checkpoint.settlements);
        let scheme = self.scheme.scheme_id();
        let manifest_digest = *self.parameters.manifest_digest();
        let archived = self.provider.with_archive(slot, |archive| {
            archived_claim(&mut Objects(archive), root, operation)
        })?;
        let (asset, claim) = if let Some(original) = archived {
            original
        } else {
            let (capsule, retained) = self.provider.retained_transition(slot, operation)?;
            if capsule.wallet() != &wallet
                || capsule.operation_id() != operation
                || capsule.kind() != 6
                || retained.record.wallet() != &wallet
                || capsule.scheme() != &scheme
            {
                return Err(invalid("released Unload scope"));
            }
            self.engine
                .verify_completion(&capsule, &retained.record, &self.parameters)
                .map_err(provider_error)?;
            (
                *capsule.asset(),
                retained.record.output_part(3).map_err(provider_error)?,
            )
        };
        let scope = ClaimScope {
            network: self.parameters.genesis_commit_verifier().network(),
            scheme: &scheme,
            asset: &asset,
            manifest_digest: &manifest_digest,
            claim: &claim,
            activation: None,
        };
        self.settle_original(
            slot,
            operation,
            signed_wire,
            proof,
            &scope,
            (origin, checkpoint),
        )
    }

    /// Authenticate the signed transaction containing this incarnation's retained Bootstrap activation.
    /// An empty proof reads its existing outcome; a rejected retry cannot undo an applied activation.
    pub fn activation_settlement(
        &mut self,
        slot: &Slot,
        signed_wire: &[u8],
        proof: &[u8],
    ) -> Result<UnloadSettlement, ProviderError> {
        if signed_wire.is_empty()
            || signed_wire.len() > 64 * 1024
            || proof.len() > KAGEMUSHA_WALLET_LEDGER_FINALITY_MAX_BYTES_V1
        {
            return Err(invalid("Activate original bounds"));
        }
        let account = self.payout_account(slot)?;
        let wallet = self.authorized_wallet_id(slot)?;
        let operation = self.bootstrap_operation_id(slot)?;
        let scheme = self.scheme.scheme_id();
        let manifest_digest = *self.parameters.manifest_digest();
        let asset = self.admitted_asset_digest(slot)?;
        let claim = self.activation_original(slot)?;
        let checkpoint = self.custody_checkpoint(slot, wallet)?;
        let scope = ClaimScope {
            network: self.parameters.genesis_commit_verifier().network(),
            scheme: &scheme,
            asset: &asset,
            manifest_digest: &manifest_digest,
            claim: &claim,
            activation: Some((&wallet, &account)),
        };
        self.settle_original(slot, &operation, signed_wire, proof, &scope, checkpoint)
    }

    fn settle_original(
        &mut self,
        slot: &Slot,
        operation: &[u8; 32],
        signed_wire: &[u8],
        proof: &[u8],
        scope: &ClaimScope<'_>,
        (origin, mut checkpoint): ([u8; 32], Checkpoint),
    ) -> Result<UnloadSettlement, ProviderError> {
        let root = IndexRoot(checkpoint.settlements);
        let signed = scope.envelope(signed_wire)?;
        let entrypoint = *signed.hash_as_entrypoint().as_ref();
        let attempt_key = scope.attempt_key(&entrypoint);
        let paid_key = scope.accepted_key(operation);
        let (prior, paid) = self.provider.with_archive(slot, |archive| {
            let mut store = Objects(archive);
            let prior = retained_row(&mut store, root, &attempt_key, operation, scope)?;
            let paid = retained_row(&mut store, root, &paid_key, operation, scope)?;
            if prior.as_ref().is_some_and(|v| v.entrypoint != entrypoint)
                || paid.as_ref().is_some_and(|v| !v.accepted)
                || (prior.as_ref().is_some_and(|v| v.accepted) && paid.is_none())
            {
                return Err(invalid("protected ledger outcome index"));
            }
            Ok((prior, paid.is_some()))
        })?;
        let mut result = UnloadSettlement {
            entrypoint,
            height: 0,
            block: [0; 32],
            accepted: false,
            paid,
            epoch_required: None,
            required_height: None,
        };
        if let Some(row) = &prior {
            result.height = row.height;
            result.block = row.block;
            result.accepted = row.accepted;
            if !proof.is_empty()
                && row.finality
                    == crate::kagemusha_wallet_advance_v1::kagemusha_wallet_archive_object_digest_v1(
                        proof,
                    )
            {
                return Ok(result);
            }
        }
        if proof.is_empty() {
            return Ok(result);
        }
        let original = decode_model(proof)?;
        if *original.entrypoint_hash.as_ref() != entrypoint {
            return Err(invalid("ledger finality names another original"));
        }
        let (epoch, target_height) = original.selectors().map_err(finality_error)?;
        let Some(reader) = self.selected_commit_epoch(slot, epoch)? else {
            if prior.is_some() {
                return Err(invalid("conflicting ledger certificate epoch"));
            }
            result.epoch_required = Some(epoch);
            result.required_height = Some(target_height);
            return Ok(result);
        };
        let verified = original
            .verify(&reader, reader.chain_id(), &signed)
            .map_err(finality_error)?;
        if let Some(row) = prior {
            if row.height != verified.height()
                || row.block != verified.commit_hash()
                || row.accepted != verified.accepted()
            {
                return Err(invalid("conflicting authenticated ledger outcome"));
            }
            return Ok(result);
        }
        checkpoint.settlements = self.provider.with_archive(slot, |archive| {
            let mut store = Objects(archive);
            scope
                .record(
                    &mut store,
                    root,
                    operation,
                    signed_wire,
                    proof,
                    verified,
                    paid,
                )
                .map(|root| root.0)
        })?;
        self.publish_custody_checkpoint(slot, origin, &checkpoint)?;
        result.height = verified.height();
        result.block = verified.commit_hash();
        result.accepted = verified.accepted();
        result.paid |= result.accepted;
        Ok(result)
    }
}

#[cfg(test)]
mod tests;
