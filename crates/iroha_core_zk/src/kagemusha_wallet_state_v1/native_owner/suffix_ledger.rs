//! Exact retained suffix stage instructions, signed attempts, and direct ledger finality.
//!
//! Begin's instruction is selected before Advance. Append and Finalize originals exist only
//! after Advance has durably released the exact completion. Every offered signed envelope
//! is stored before Awaiting is returned, so an application crash cannot erase its identity.
//! The authenticated root selects per-stage attempt indexes: retry reads never enumerate
//! the wallet history or reconstruct a signed envelope. Only Finalize success is a payout.

use super::ledger_outcome::{self, OfferedOutcome};
use super::*;
use crate::kagemusha_wallet_advance_v1::kagemusha_wallet_archive_object_digest_v1 as object_digest;
use crate::kagemusha_wallet_state_v1::suffix_custody::{self, *};
use iroha_data_model::{
    isi::kagemusha_wallet::{KagemushaWalletLedgerActionV1, KagemushaWalletLedgerV1},
    kagemusha::kagemusha_wallet_suffix_unload_v1::*,
    transaction::{Executable, SignedTransaction},
};
use sha2::{Digest, Sha256};

/// Only suffix envelopes have this larger bound; ordinary instructions retain 64 KiB.
pub const SUFFIX_SIGNED_WIRE_MAX_BYTES_V1: usize =
    KAGEMUSHA_WALLET_SUFFIX_CHUNK_MAX_BYTES_V1 + 65_536;
/// One retained canonical ledger instruction containing a bounded suffix chunk.
pub const SUFFIX_INSTRUCTION_MAX_BYTES_V1: usize =
    KAGEMUSHA_WALLET_SUFFIX_CHUNK_MAX_BYTES_V1 + 16_384;

/// Stage success carries no payout assertion until the final instruction ordinal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuffixStageOutcomeV1 {
    /// The exact retained transaction has no authenticated finality outcome yet.
    Awaiting {
        /// Entrypoint hash of that exact signed transaction.
        entrypoint: [u8; 32],
    },
    /// The signed validators certified rejection of this transaction attempt.
    Rejected(LedgerCommitV1),
    /// Certified Begin or Append progress, without an online payout assertion.
    StageConfirmed(LedgerCommitV1),
    /// Certified Finalize success, completing the selected online payout.
    UnloadConfirmed(LedgerCommitV1),
}

/// A direct-finality ingestion result, including a request for validator-epoch synchronization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuffixStageIngestV1 {
    /// The authenticated outcome of the exact retained transaction attempt.
    Outcome(SuffixStageOutcomeV1),
    /// The checkpoint-rooted validator history must advance before this proof can be verified.
    EpochRequired {
        /// Height of the supplied certificate whose validator epoch is needed.
        target_height: u64,
    },
}

/// Opaque local index keys. These are not monetary commitments or proof inputs.
fn index_key(role: &[u8], body: &[u8]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"iroha:kagemusha:wallet:suffix-native-index:v1:");
    hash.update(role);
    hash.update([0]);
    hash.update((body.len() as u64).to_le_bytes());
    hash.update(body);
    hash.finalize().into()
}
pub(super) fn suffix_ordinal_key(ordinal: u128) -> [u8; 32] {
    let mut key = [0; 32];
    key[16..].copy_from_slice(&ordinal.to_be_bytes());
    key
}
fn stage_key(request: &[u8; 32], ordinal: u128) -> [u8; 32] {
    let mut body = [0; 48];
    body[..32].copy_from_slice(request);
    body[32..].copy_from_slice(&ordinal.to_le_bytes());
    index_key(b"stage", &body)
}
fn address(value: &[u8]) -> Result<[u8; 32], Error> {
    let value: [u8; 32] = value
        .try_into()
        .map_err(|_| Error::WitnessLost("suffix index address"))?;
    if value == [0; 32] {
        return Err(Error::WitnessLost("suffix index zero address"));
    }
    Ok(value)
}
fn indexed_record<T>(
    archive: &mut impl ObjectStore,
    index: IndexRoot,
    key: &[u8; 32],
) -> Result<Option<(T, [u8; 32])>, Error>
where
    T: norito::NoritoSerialize,
    for<'de> T: norito::NoritoDeserialize<'de>,
{
    index
        .get(archive, key)?
        .map(|bytes| {
            let digest = address(&bytes)?;
            Ok((suffix_custody::read_record(archive, digest)?, digest))
        })
        .transpose()
}
fn exact_object(
    archive: &mut impl ObjectStore,
    digest: &[u8; 32],
    bound: usize,
) -> Result<Vec<u8>, Error> {
    let bytes = archive.read_object(digest, bound)?;
    if bytes.is_empty() || bytes.len() > bound || object_digest(&bytes) != *digest {
        return Err(Error::WitnessLost("suffix original identity"));
    }
    Ok(bytes)
}

struct SelectedStage {
    checkpoint: SelectedCheckpoint,
    request: SuffixRequestRecordV1,
    selection: SuffixPublicationSelectionV1,
    instruction: Vec<u8>,
    instruction_digest: [u8; 32],
    ordinal: u128,
}
impl SelectedStage {
    fn suffix(&self) -> &SuffixArchiveManifestV1 {
        self.checkpoint.suffix.as_ref().expect("selected suffix")
    }
    fn final_ordinal(&self) -> u128 {
        self.selection.chunk_count + 1
    }
    fn outcome(&self, retained: &SuffixLedgerOutcomeV1) -> SuffixStageOutcomeV1 {
        let commit = LedgerCommitV1 {
            height: retained.certified_height,
            commit_hash: retained.native_commit_hash,
        };
        if !retained.accepted {
            SuffixStageOutcomeV1::Rejected(commit)
        } else if self.ordinal == self.final_ordinal() {
            SuffixStageOutcomeV1::UnloadConfirmed(commit)
        } else {
            SuffixStageOutcomeV1::StageConfirmed(commit)
        }
    }
}

/// Canonical decoding with a larger bound is available only to exact suffix stage intake.
/// This uses the ordinary signed-wire owner, then pins its complete single instruction.
fn signed_stage(
    bytes: &[u8],
    network: &[u8; 32],
    instruction: &[u8],
) -> Result<SignedTransaction, Error> {
    let transaction =
        ledger_outcome::signed_original_bounded(bytes, SUFFIX_SIGNED_WIRE_MAX_BYTES_V1)?;
    let public = transaction
        .authority()
        .try_signatory()
        .ok_or(Error::Invalid("suffix stage single account key"))?;
    let (algorithm, key) = public
        .try_to_bytes()
        .map_err(|_| Error::Invalid("suffix stage account key"))?;
    if transaction
        .network_id()
        .map(iroha_data_model::NetworkId::as_bytes)
        != Some(network)
        || transaction.multisig_signatures().is_some()
        || transaction.signature_count() != 1
        || algorithm != iroha_crypto::Algorithm::Ed25519
        || key.len() != 32
    {
        return Err(Error::Invalid("suffix stage signed transaction authority"));
    }
    transaction
        .verify_signature()
        .map_err(|_| Error::Proof("suffix stage submitter signature"))?;
    // The signed envelope authorizes exactly one native instruction. Filtering a
    // mixed batch for explicit instructions would silently ignore contract calls.
    let Executable::Instructions(instructions) = transaction.instructions() else {
        return Err(Error::Invalid("suffix stage exact instruction"));
    };
    let [value] = instructions.as_ref() else {
        return Err(Error::Invalid("suffix stage exact instruction"));
    };
    let value = value
        .as_any()
        .downcast_ref::<KagemushaWalletLedgerV1>()
        .ok_or(Error::Invalid("suffix stage exact instruction"))?;
    if archive::encode(value)? != instruction {
        return Err(Error::Invalid("suffix stage exact instruction"));
    }
    Ok(transaction)
}

impl<C: Custody, A: ArchiveStore, N: NativeProofs> Coordinator<C, A, N> {
    fn selected_suffix_stage(
        &mut self,
        request_id: [u8; 32],
        ordinal: u128,
    ) -> Result<SelectedStage, Error> {
        if request_id == [0; 32] || ordinal > 513 {
            return Err(Error::Invalid("suffix instruction selector"));
        }
        let mut checkpoint = self.selected_suffix_checkpoint()?.ok_or(Error::NoHead)?;
        let suffix = checkpoint
            .suffix
            .as_ref()
            .ok_or(Error::Invalid("suffix request absent"))?;
        let binding = (self.scheme_id, self.wallet_id);
        let (mut request, _) = indexed_record::<SuffixRequestRecordV1>(
            &mut self.archive,
            suffix.requests,
            &request_id,
        )?
        .ok_or(Error::Invalid("suffix request absent"))?;
        request.require(binding)?;
        if request.request_id != request_id {
            return Err(Error::WitnessLost("suffix request selector"));
        }
        let selection_address = request
            .selection
            .ok_or(Error::Invalid("suffix selection pending"))?;
        let selection: SuffixPublicationSelectionV1 =
            suffix_custody::read_record(&mut self.archive, selection_address)?;
        selection.require(binding)?;
        if selection.request_id != request_id
            || selection.source_checkpoint != request.source_checkpoint
            || selection.source_capsule != request.source_capsule
            || selection.source_operation != request.source_operation
            || selection.source_sequence != request.source_sequence
            || ordinal > selection.chunk_count + 1
        {
            return Err(Error::WitnessLost("suffix selection request binding"));
        }
        let header_bytes = exact_object(
            &mut self.archive,
            &selection.header_original,
            KAGEMUSHA_WALLET_SUFFIX_HEADER_MAX_BYTES_V1,
        )?;
        let header = read_model_original(
            || KagemushaWalletSuffixHeaderV1::decode_canonical(&header_bytes, &self.scheme_id),
            Error::WitnessLost("suffix retained header"),
        )?;
        let authority = read_model_original(
            || kagemusha_wallet_suffix_authority_digest_v1(&header_bytes),
            Error::WitnessLost("suffix retained authority"),
        )?;
        if authority != selection.authority
            || header.wallet_id != self.wallet_id
            || header.request_id != request_id
            || header.source_capsule != selection.source_capsule
            || header.anchor_sequence != request.anchor_sequence
            || header.tip_sequence != selection.source_sequence
            || header.chunk_count != selection.chunk_count
            || header.step_count != selection.step_count
            || header.stream_bytes != selection.stream_bytes
        {
            return Err(Error::WitnessLost("suffix retained header selection"));
        }
        let instruction_digest = if ordinal == 0 {
            selection.begin_instruction_original
        } else {
            let released_address = request
                .released
                .ok_or(Error::Invalid("suffix release pending"))?;
            let mut released: SuffixReleasedOriginalV1 =
                suffix_custody::read_record(&mut self.archive, released_address)?;
            released.require(binding)?;
            if released.request_id != request_id
                || released.authority != selection.authority
                || released.selection != selection_address
                || released.operation_id != selection.operation_id
                || released.instruction_count != selection.chunk_count + 2
            {
                return Err(Error::WitnessLost("suffix release selection binding"));
            }
            match released
                .instruction_originals
                .get(&mut self.archive, &suffix_ordinal_key(ordinal))?
            {
                Some(value) => address(&value)?,
                None if ordinal <= selection.chunk_count => self.materialize_suffix_append(
                    &mut checkpoint,
                    &mut request,
                    &selection,
                    &header,
                    &mut released,
                    ordinal,
                )?,
                None => {
                    return Err(Error::WitnessLost(
                        "suffix released Finalize instruction missing",
                    ));
                }
            }
        };
        let instruction = exact_object(
            &mut self.archive,
            &instruction_digest,
            SUFFIX_INSTRUCTION_MAX_BYTES_V1,
        )?;
        self.require_suffix_instruction(&selection, &header, ordinal, &instruction)?;
        Ok(SelectedStage {
            checkpoint,
            request,
            selection,
            instruction,
            instruction_digest,
            ordinal,
        })
    }

    /// Release selects the exact chunk originals. Construct and retain only the requested
    /// Append carriage, in bounded work, before any caller may sign or dispatch it. Repeated
    /// reads take the selected instruction original above and never regenerate it.
    fn materialize_suffix_append(
        &mut self,
        checkpoint: &mut SelectedCheckpoint,
        request: &mut SuffixRequestRecordV1,
        selection: &SuffixPublicationSelectionV1,
        header: &KagemushaWalletSuffixHeaderV1,
        released: &mut SuffixReleasedOriginalV1,
        ordinal: u128,
    ) -> Result<[u8; 32], Error> {
        if !(1..=selection.chunk_count).contains(&ordinal) {
            return Err(Error::WitnessLost("suffix materialized Append ordinal"));
        }
        if let Some(existing) = released
            .instruction_originals
            .get(&mut self.archive, &suffix_ordinal_key(ordinal))?
        {
            let digest = address(&existing)?;
            let bytes = exact_object(&mut self.archive, &digest, SUFFIX_INSTRUCTION_MAX_BYTES_V1)?;
            self.require_suffix_instruction(selection, header, ordinal, &bytes)?;
            return Ok(digest);
        }
        let chunk_address = selection
            .chunk_originals
            .get(&mut self.archive, &suffix_ordinal_key(ordinal - 1))?
            .ok_or(Error::WitnessLost("suffix released chunk absent"))?;
        let chunk = exact_object(
            &mut self.archive,
            &address(&chunk_address)?,
            KAGEMUSHA_WALLET_SUFFIX_CHUNK_MAX_BYTES_V1,
        )?;
        let instruction = KagemushaWalletLedgerV1::new(
            self.scheme_id,
            KagemushaWalletLedgerActionV1::SuffixUnloadAppend {
                wallet: self.wallet_id,
                authority: selection.authority,
                account: header.account.clone(),
                ordinal,
                chunk,
            },
        );
        let bytes = archive::encode(&instruction)?;
        if bytes.len() > SUFFIX_INSTRUCTION_MAX_BYTES_V1 {
            return Err(Error::WitnessLost(
                "suffix retained Append instruction bound",
            ));
        }
        self.require_suffix_instruction(selection, header, ordinal, &bytes)?;
        let digest = self
            .archive
            .write_object(&bytes, SUFFIX_INSTRUCTION_MAX_BYTES_V1)?;
        if digest != object_digest(&bytes) {
            return Err(Error::WitnessLost("suffix Append publication identity"));
        }
        released.instruction_originals = released.instruction_originals.set(
            &mut self.archive,
            suffix_ordinal_key(ordinal),
            &digest,
        )?;
        released.require((self.scheme_id, self.wallet_id))?;
        request.released = Some(suffix_custody::write_record(&mut self.archive, released)?);
        request.require((self.scheme_id, self.wallet_id))?;
        let request_address = suffix_custody::write_record(&mut self.archive, request)?;
        let mut suffix = checkpoint
            .suffix
            .clone()
            .ok_or(Error::WitnessLost("suffix namespace absent"))?;
        suffix.requests =
            suffix
                .requests
                .set(&mut self.archive, request.request_id, &request_address)?;
        let root =
            self.publish_suffix_checkpoint(checkpoint.digest, &checkpoint.manifest, &suffix)?;
        checkpoint.digest = root;
        checkpoint.suffix = Some(suffix);
        Ok(digest)
    }

    fn require_suffix_instruction(
        &mut self,
        selection: &SuffixPublicationSelectionV1,
        header: &KagemushaWalletSuffixHeaderV1,
        ordinal: u128,
        bytes: &[u8],
    ) -> Result<(), Error> {
        let instruction: KagemushaWalletLedgerV1 = archive::decode(bytes)?;
        if instruction.scheme != self.scheme_id {
            return Err(Error::WitnessLost("suffix instruction scheme"));
        }
        let (wallet, authority, account, held_ordinal) = match &instruction.action {
            KagemushaWalletLedgerActionV1::SuffixUnloadBegin {
                wallet,
                authority,
                account,
                ordinal: held,
                header: raw,
                anchor,
            } if ordinal == 0 => {
                if object_digest(raw) != selection.header_original
                    || object_digest(anchor) != selection.anchor_original
                    || kagemusha_wallet_suffix_digest_v1(
                        KagemushaWalletSuffixDigestRoleV1::Anchor,
                        anchor,
                    ) != header.anchor_digest
                {
                    return Err(Error::WitnessLost("suffix Begin original binding"));
                }
                (wallet, authority, account, held)
            }
            KagemushaWalletLedgerActionV1::SuffixUnloadAppend {
                wallet,
                authority,
                account,
                ordinal: held,
                chunk,
            } if (1..=selection.chunk_count).contains(&ordinal) => {
                let parsed = read_model_original(
                    || KagemushaWalletSuffixChunkV1::decode_canonical(chunk, &self.scheme_id),
                    Error::WitnessLost("suffix Append original"),
                )?;
                let digest = kagemusha_wallet_suffix_digest_v1(
                    KagemushaWalletSuffixDigestRoleV1::Chunk,
                    chunk,
                );
                if parsed.chunk_index != ordinal - 1 || header.chunk_digest(ordinal) != Some(digest)
                {
                    return Err(Error::WitnessLost("suffix Append header binding"));
                }
                (wallet, authority, account, held)
            }
            KagemushaWalletLedgerActionV1::SuffixUnloadFinalize {
                wallet,
                authority,
                account,
                ordinal: held,
                unload,
            } if ordinal == selection.chunk_count + 1 => {
                let parsed = read_model_original(
                    || KagemushaWalletSuffixUnloadV1::decode_canonical(unload, &self.scheme_id),
                    Error::WitnessLost("suffix Finalize original"),
                )?;
                read_model_original(
                    || parsed.require_header(header),
                    Error::WitnessLost("suffix Finalize header binding"),
                )?;
                (wallet, authority, account, held)
            }
            _ => return Err(Error::WitnessLost("suffix instruction stage")),
        };
        if *wallet != self.wallet_id
            || *authority != selection.authority
            || *held_ordinal != ordinal
            || *account != header.account
        {
            return Err(Error::WitnessLost("suffix instruction scope"));
        }
        Ok(())
    }

    /// Read exact selected bytes. Begin is available before the hardware debit; later
    /// instructions require the actual released completion. An absent Append is materialized
    /// once from its selected chunk and durably retained before return; retries read it exactly.
    /// # Errors
    /// Unknown request, unavailable release, malformed selection or failed durable publication.
    pub fn suffix_unload_instruction(
        &mut self,
        request_id: [u8; 32],
        ordinal: u128,
    ) -> Result<Vec<u8>, Error> {
        Ok(self.selected_suffix_stage(request_id, ordinal)?.instruction)
    }

    fn retained_suffix_stage_attempts(
        &mut self,
        stage: &SelectedStage,
    ) -> Result<Option<SuffixStageAttemptsV1>, Error> {
        let held = indexed_record::<SuffixStageAttemptsV1>(
            &mut self.archive,
            stage.suffix().ledger_stages,
            &stage_key(&stage.request.request_id, stage.ordinal),
        )?;
        held.map(|(held, _)| {
            held.require((self.scheme_id, self.wallet_id))?;
            if held.request_id != stage.request.request_id
                || held.authority != stage.selection.authority
                || held.instruction_ordinal != stage.ordinal
                || held.instruction_original != stage.instruction_digest
            {
                return Err(Error::WitnessLost("suffix attempts stage binding"));
            }
            Ok(held)
        })
        .transpose()
    }

    fn retained_suffix_attempt(
        &mut self,
        stage: &SelectedStage,
        entrypoint: &[u8; 32],
        wire: &[u8],
    ) -> Result<Option<(SuffixLedgerAttemptV1, [u8; 32])>, Error> {
        let held = indexed_record::<SuffixLedgerAttemptV1>(
            &mut self.archive,
            stage.suffix().ledger_attempts,
            entrypoint,
        )?;
        held.map(|(held, digest)| {
            held.require((self.scheme_id, self.wallet_id))?;
            if held.request_id != stage.request.request_id
                || held.authority != stage.selection.authority
                || held.instruction_ordinal != stage.ordinal
                || held.instruction_original != stage.instruction_digest
                || held.entrypoint != *entrypoint
                || held.signed_wire_length as usize != wire.len()
                || held.signed_wire_original != object_digest(wire)
            {
                return Err(Error::OperationConflict);
            }
            if exact_object(
                &mut self.archive,
                &held.signed_wire_original,
                SUFFIX_SIGNED_WIRE_MAX_BYTES_V1,
            )? != wire
            {
                return Err(Error::WitnessLost("suffix attempt exact signed wire"));
            }
            Ok((held, digest))
        })
        .transpose()
    }

    fn retain_suffix_attempt(
        &mut self,
        stage: &mut SelectedStage,
        entrypoint: [u8; 32],
        wire: &[u8],
    ) -> Result<(SuffixLedgerAttemptV1, [u8; 32]), Error> {
        if let Some(held) = self.retained_suffix_attempt(stage, &entrypoint, wire)? {
            return Ok(held);
        }
        let mut attempts = self
            .retained_suffix_stage_attempts(stage)?
            .unwrap_or_else(|| SuffixStageAttemptsV1 {
                version: 1,
                discriminator: 17,
                scheme_id: self.scheme_id,
                wallet_id: self.wallet_id,
                request_id: stage.request.request_id,
                authority: stage.selection.authority,
                instruction_ordinal: stage.ordinal,
                instruction_original: stage.instruction_digest,
                attempts: IndexRoot::default(),
                attempt_count: 0,
                accepted_entrypoint: None,
            });
        let next = attempts
            .attempt_count
            .checked_add(1)
            .ok_or(Error::Invalid("suffix attempt index exhausted"))?;
        let signed_wire_original = self
            .archive
            .write_object(wire, SUFFIX_SIGNED_WIRE_MAX_BYTES_V1)?;
        if signed_wire_original != object_digest(wire) {
            return Err(Error::WitnessLost("suffix attempt publication identity"));
        }
        let attempt = SuffixLedgerAttemptV1 {
            version: 1,
            discriminator: 7,
            scheme_id: self.scheme_id,
            wallet_id: self.wallet_id,
            request_id: stage.request.request_id,
            authority: stage.selection.authority,
            instruction_ordinal: stage.ordinal,
            instruction_original: stage.instruction_digest,
            signed_wire_original,
            signed_wire_length: u32::try_from(wire.len())
                .map_err(|_| Error::Invalid("suffix signed wire length"))?,
            entrypoint,
        };
        attempt.require((self.scheme_id, self.wallet_id))?;
        let digest = suffix_custody::write_record(&mut self.archive, &attempt)?;
        attempts.attempts = attempts.attempts.set(
            &mut self.archive,
            suffix_ordinal_key(attempts.attempt_count.into()),
            &entrypoint,
        )?;
        attempts.attempt_count = next;
        let attempts_digest = suffix_custody::write_record(&mut self.archive, &attempts)?;
        let mut suffix = stage.suffix().clone();
        suffix.ledger_attempts =
            suffix
                .ledger_attempts
                .set(&mut self.archive, entrypoint, &digest)?;
        suffix.ledger_stages = suffix.ledger_stages.set(
            &mut self.archive,
            stage_key(&stage.request.request_id, stage.ordinal),
            &attempts_digest,
        )?;
        let root = self.publish_suffix_checkpoint(
            stage.checkpoint.digest,
            &stage.checkpoint.manifest,
            &suffix,
        )?;
        stage.checkpoint.digest = root;
        stage.checkpoint.suffix = Some(suffix);
        Ok((attempt, digest))
    }

    fn retained_suffix_outcome(
        &mut self,
        stage: &SelectedStage,
        attempt: &SuffixLedgerAttemptV1,
        attempt_digest: [u8; 32],
    ) -> Result<Option<SuffixLedgerOutcomeV1>, Error> {
        let held = indexed_record::<SuffixLedgerOutcomeV1>(
            &mut self.archive,
            stage.suffix().ledger_outcomes,
            &attempt.entrypoint,
        )?;
        held.map(|(held, _)| {
            held.require((self.scheme_id, self.wallet_id))?;
            if held.request_id != stage.request.request_id
                || held.authority != stage.selection.authority
                || held.attempt != attempt_digest
                || held.entrypoint != attempt.entrypoint
            {
                return Err(Error::WitnessLost("suffix outcome attempt binding"));
            }
            let bytes = exact_object(
                &mut self.archive,
                &held.finality_original,
                KAGEMUSHA_WALLET_LEDGER_FINALITY_MAX_BYTES_V1,
            )?;
            let original = read_model_original(
                || KagemushaWalletLedgerFinalityV1::decode_canonical(&bytes),
                Error::WitnessLost("suffix finality original"),
            )?;
            let bindings = original.custody_bindings().map_err(|error| {
                finality_read_error(error, Error::WitnessLost("suffix finality binding"))
            })?;
            if *original.entrypoint_hash.as_ref() != held.entrypoint
                || bindings.height != held.certified_height
                || bindings.qc_block_hash != held.native_commit_hash
                || bindings.accepted != held.accepted
            {
                return Err(Error::WitnessLost("suffix outcome original binding"));
            }
            Ok(held)
        })
        .transpose()
    }

    /// Select the verified outcome and its stage-success index in one fenced publication.
    fn retain_verified_suffix_outcome(
        &mut self,
        stage: &SelectedStage,
        attempt: &SuffixLedgerAttemptV1,
        attempt_digest: [u8; 32],
        verified: &VerifiedKagemushaLedgerOutcomeV1,
        finality: &[u8],
    ) -> Result<SuffixLedgerOutcomeV1, Error> {
        if verified.entrypoint() != attempt.entrypoint {
            return Err(Error::Proof("suffix finality exact attempt"));
        }
        if let Some(held) = self.retained_suffix_outcome(stage, attempt, attempt_digest)? {
            if held.accepted != verified.accepted()
                || held.certified_height != verified.height()
                || held.native_commit_hash != verified.commit_hash()
            {
                return Err(Error::Proof("conflicting authenticated suffix outcome"));
            }
            return Ok(held);
        }
        let finality_original = self
            .archive
            .write_object(finality, KAGEMUSHA_WALLET_LEDGER_FINALITY_MAX_BYTES_V1)?;
        if finality_original != object_digest(finality) {
            return Err(Error::WitnessLost("suffix finality publication identity"));
        }
        let held = SuffixLedgerOutcomeV1 {
            version: 1,
            discriminator: 8,
            scheme_id: self.scheme_id,
            wallet_id: self.wallet_id,
            request_id: stage.request.request_id,
            authority: stage.selection.authority,
            attempt: attempt_digest,
            entrypoint: verified.entrypoint(),
            accepted: verified.accepted(),
            certified_height: verified.height(),
            native_commit_hash: verified.commit_hash(),
            finality_original,
        };
        held.require((self.scheme_id, self.wallet_id))?;
        let digest = suffix_custody::write_record(&mut self.archive, &held)?;
        let mut suffix = stage.suffix().clone();
        suffix.ledger_outcomes =
            suffix
                .ledger_outcomes
                .set(&mut self.archive, attempt.entrypoint, &digest)?;
        if held.accepted {
            let mut attempts = self
                .retained_suffix_stage_attempts(stage)?
                .ok_or(Error::WitnessLost("suffix stage attempts absent"))?;
            if attempts.accepted_entrypoint.is_none() {
                attempts.accepted_entrypoint = Some(attempt.entrypoint);
                let digest = suffix_custody::write_record(&mut self.archive, &attempts)?;
                suffix.ledger_stages = suffix.ledger_stages.set(
                    &mut self.archive,
                    stage_key(&stage.request.request_id, stage.ordinal),
                    &digest,
                )?;
            }
        }
        self.publish_suffix_checkpoint(
            stage.checkpoint.digest,
            &stage.checkpoint.manifest,
            &suffix,
        )?;
        Ok(held)
    }

    /// A selected archive address is only an index. Re-open the exact wire and repeat
    /// signed-stage intake before it can be returned or used as debit authority.
    fn retained_signed_suffix_attempt(
        &mut self,
        stage: &SelectedStage,
        entrypoint: &[u8; 32],
    ) -> Result<(SuffixLedgerAttemptV1, [u8; 32], Vec<u8>, SignedTransaction), Error> {
        let (indexed, _) = indexed_record::<SuffixLedgerAttemptV1>(
            &mut self.archive,
            stage.suffix().ledger_attempts,
            entrypoint,
        )?
        .ok_or(Error::WitnessLost("suffix attempt absent"))?;
        indexed.require((self.scheme_id, self.wallet_id))?;
        let wire = exact_object(
            &mut self.archive,
            &indexed.signed_wire_original,
            SUFFIX_SIGNED_WIRE_MAX_BYTES_V1,
        )?;
        let (attempt, digest) = self
            .retained_suffix_attempt(stage, entrypoint, &wire)?
            .ok_or(Error::WitnessLost("suffix attempt absent"))?;
        let (scheme, _) = self.proofs.ledger_scope()?;
        if scheme.scheme_id() != self.scheme_id {
            return Err(Error::WitnessLost("suffix retained ledger scope"));
        }
        let lost = |error| match error {
            Error::Invalid(_) | Error::Proof(_) | Error::OperationConflict => {
                Error::WitnessLost("suffix retained signed wire")
            }
            other => other,
        };
        let signed = signed_stage(&wire, &scheme.network_id, &stage.instruction).map_err(lost)?;
        if ledger_outcome::entrypoint(&signed).map_err(lost)? != *entrypoint {
            return Err(Error::WitnessLost("suffix retained entrypoint"));
        }
        Ok((attempt, digest, wire, signed))
    }

    /// Re-authenticate the immutable Begin outcome under the installed genesis immediately
    /// before Advance. The selected success pointer alone cannot authorize the debit.
    fn accepted_suffix_begin(
        &mut self,
        stage: &SelectedStage,
        genesis: &SumeragiFinalityVerifier,
        authority: &[u8; 32],
    ) -> Result<bool, Error> {
        self.require_ledger_root(genesis)?;
        if stage.ordinal != 0 || stage.selection.authority != *authority {
            return Err(Error::OperationConflict);
        }
        let Some(attempts) = self.retained_suffix_stage_attempts(stage)? else {
            return Ok(false);
        };
        let Some(entrypoint) = attempts.accepted_entrypoint else {
            return Ok(false);
        };
        let (attempt, digest, _, signed) =
            self.retained_signed_suffix_attempt(stage, &entrypoint)?;
        let held = self
            .retained_suffix_outcome(stage, &attempt, digest)?
            .ok_or(Error::WitnessLost("suffix accepted Begin outcome absent"))?;
        if !held.accepted {
            return Err(Error::WitnessLost("suffix accepted Begin outcome rejected"));
        }
        let bytes = exact_object(
            &mut self.archive,
            &held.finality_original,
            KAGEMUSHA_WALLET_LEDGER_FINALITY_MAX_BYTES_V1,
        )?;
        match self.verify_ledger_original(&stage.checkpoint.manifest, genesis, &signed, &bytes)? {
            OfferedOutcome::Verified(verified)
                if verified.accepted()
                    && verified.entrypoint() == held.entrypoint
                    && verified.height() == held.certified_height
                    && verified.commit_hash() == held.native_commit_hash =>
            {
                Ok(true)
            }
            OfferedOutcome::Verified(_) | OfferedOutcome::EpochRequired(_) => Err(
                Error::WitnessLost("suffix accepted Begin certificate binding"),
            ),
        }
    }

    /// Exact Native-retained attempt at this local zero-based intake index. Only an absent
    /// index returns None; malformed, missing or mismatched retained bytes remain errors.
    /// Confirmed attempts remain readable after transaction expiry and Finalize, so exact
    /// Core receipt repair never requires signing another transaction or another debit.
    /// # Errors
    /// Invalid selector, unavailable custody or retained wire, signature and scope mismatch.
    pub fn suffix_unload_signed_wire(
        &mut self,
        request_id: [u8; 32],
        ordinal: u128,
        attempt_index: u64,
    ) -> Result<Option<Vec<u8>>, Error> {
        let stage = self.selected_suffix_stage(request_id, ordinal)?;
        let Some(attempts) = self.retained_suffix_stage_attempts(&stage)? else {
            return Ok(None);
        };
        if attempt_index >= attempts.attempt_count {
            return Ok(None);
        }
        let entrypoint = address(
            &attempts
                .attempts
                .get(&mut self.archive, &suffix_ordinal_key(attempt_index.into()))?
                .ok_or(Error::WitnessLost("suffix indexed attempt absent"))?,
        )?;
        let (_, _, wire, _) = self.retained_signed_suffix_attempt(&stage, &entrypoint)?;
        Ok(Some(wire))
    }
}

impl<F: KagemushaWalletFsV1, P: KagemushaWalletPlatformV1, S: OriginalSourceV1 + Send>
    Coordinator<AdvanceHandle<F, P>, ProviderArchive<F, P>, NativeWalletProofsV1<F, P, S>>
{
    pub(super) fn suffix_begin_accepted(
        &mut self,
        request_id: &[u8; 32],
        authority: &[u8; 32],
    ) -> Result<bool, Error> {
        let stage = self.selected_suffix_stage(*request_id, 0)?;
        let genesis = Arc::clone(&self.proofs.genesis);
        self.accepted_suffix_begin(&stage, &genesis, authority)
    }

    /// Intake and durably retain this exact signed stage before the caller may dispatch it.
    /// # Errors
    /// Wrong network, signature, instruction or selection; unavailable storage or conflicting outcome.
    pub fn suffix_unload_attempt_outcome(
        &mut self,
        request_id: [u8; 32],
        ordinal: u128,
        signed_wire: &[u8],
    ) -> Result<SuffixStageOutcomeV1, Error> {
        self.require_ledger_root(&self.proofs.genesis)?;
        let mut stage = self.selected_suffix_stage(request_id, ordinal)?;
        let signed = signed_stage(
            signed_wire,
            self.proofs.genesis.initial_epoch().network_id.as_bytes(),
            &stage.instruction,
        )?;
        let entrypoint = ledger_outcome::entrypoint(&signed)?;
        let (attempt, digest) = self.retain_suffix_attempt(&mut stage, entrypoint, signed_wire)?;
        Ok(self
            .retained_suffix_outcome(&stage, &attempt, digest)?
            .as_ref()
            .map_or(SuffixStageOutcomeV1::Awaiting { entrypoint }, |value| {
                stage.outcome(value)
            }))
    }

    /// Verify one direct finality original under the selected signed-validator epoch. Stage
    /// rejection never changes the monetary head or permits request replacement.
    /// # Errors
    /// Invalid signed stage or certificate, conflicting authenticated outcome or custody failure.
    pub fn suffix_unload_ingest_finality(
        &mut self,
        request_id: [u8; 32],
        ordinal: u128,
        signed_wire: &[u8],
        finality: &[u8],
    ) -> Result<SuffixStageIngestV1, Error> {
        self.require_ledger_root(&self.proofs.genesis)?;
        let mut stage = self.selected_suffix_stage(request_id, ordinal)?;
        let genesis = Arc::clone(&self.proofs.genesis);
        let signed = signed_stage(
            signed_wire,
            genesis.initial_epoch().network_id.as_bytes(),
            &stage.instruction,
        )?;
        let entrypoint = ledger_outcome::entrypoint(&signed)?;
        let (attempt, attempt_digest) =
            self.retain_suffix_attempt(&mut stage, entrypoint, signed_wire)?;
        let retained = self.retained_suffix_outcome(&stage, &attempt, attempt_digest)?;
        if let Some(held) = &retained {
            if object_digest(finality) == held.finality_original {
                return Ok(SuffixStageIngestV1::Outcome(stage.outcome(held)));
            }
        }
        let verified = match self.verify_ledger_original(
            &stage.checkpoint.manifest,
            &genesis,
            &signed,
            finality,
        )? {
            OfferedOutcome::EpochRequired(target_height) if retained.is_none() => {
                return Ok(SuffixStageIngestV1::EpochRequired { target_height });
            }
            OfferedOutcome::EpochRequired(_) => {
                return Err(Error::Invalid("suffix finality of another block"));
            }
            OfferedOutcome::Verified(value) => value,
        };
        if let Some(held) = retained {
            if held.entrypoint != verified.entrypoint()
                || held.accepted != verified.accepted()
                || held.certified_height != verified.height()
                || held.native_commit_hash != verified.commit_hash()
            {
                return Err(Error::Proof("conflicting authenticated suffix outcome"));
            }
            return Ok(SuffixStageIngestV1::Outcome(stage.outcome(&held)));
        }
        let held = self.retain_verified_suffix_outcome(
            &stage,
            &attempt,
            attempt_digest,
            &verified,
            finality,
        )?;
        Ok(SuffixStageIngestV1::Outcome(stage.outcome(&held)))
    }
}

#[cfg(test)]
mod tests;
