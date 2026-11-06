//! Native-suffix Unload stages (owner go of 2026-11-06).
//!
//! Begin records the immutable header and genuine anchor after verifying the anchor Ω and the
//! Unload σ. Each Append verifies one chunk of complete steps from the chunk's claimed start and
//! records the verified end. Appends do not depend on each other, so they may execute in any
//! order: Taira does not order one authority's transactions. Finalize chains every recorded
//! chunk from the anchor to the header's tip, verifies the Unload and pays it exactly once
//! through the existing reserve and `KagemushaWalletPayoutKeyV1::Unload(nullifier)` namespace.
//!
//! Incomplete staging has no monetary effect and reserves no nullifier. Every stage is
//! idempotent for its exact frame; any other frame at a staged ordinal conflicts. The header
//! lists the exact digest of every chunk, so nobody can stage a different partition of the same
//! genuine steps and strand the released Unload.
use super::*;
use iroha_core_zk::kagemusha_wallet_suffix_unload_v1::SuffixSegmentV1;
use iroha_data_model::kagemusha::kagemusha_wallet_suffix_unload_v1::{
    KAGEMUSHA_WALLET_SUFFIX_CHUNK_DATA_MAX_BYTES_V1, KAGEMUSHA_WALLET_SUFFIX_CHUNK_STEPS_MAX_V1,
    KagemushaWalletSuffixAnchorV1 as Anchor, KagemushaWalletSuffixChunkV1 as Chunk,
    KagemushaWalletSuffixDigestRoleV1 as Role, KagemushaWalletSuffixHeaderV1 as Header,
    KagemushaWalletSuffixOriginalRoleV1 as OriginalRole,
    KagemushaWalletSuffixStepKindV1 as StepKind, KagemushaWalletSuffixStepV1 as Step,
    KagemushaWalletSuffixUnloadV1 as Unload, kagemusha_wallet_suffix_authority_digest_v1,
    kagemusha_wallet_suffix_digest_v1, kagemusha_wallet_suffix_payout_source_v1,
};

/// Immutable Begin row: key `(16, scheme, authority, 0)`.
pub(super) const BEGIN: u8 = 16;
/// Immutable verified-chunk row: key `(17, scheme, authority, BE256(ordinal))`.
pub(super) const STAGE: u8 = 17;
/// Immutable final outcome row: key `(18, scheme, authority, 0)`.
pub(super) const FINAL: u8 = 18;
/// Row version of every suffix row.
const ROW_VERSION: u16 = 1;
/// Exact proof calls of Begin: the anchor Ω and an early check of the Unload σ.
const BEGIN_PROOF_CALLS: u32 = 2;
/// Exact proof calls of Finalize: the Unload σ. The anchor Ω was verified by Begin, whose
/// committed row Finalize reads.
const FINALIZE_PROOF_CALLS: u32 = 1;
/// Bound of one verifier segment record retained in a stage row.
pub const SUFFIX_SEGMENT_RECORD_MAX_BYTES: usize = 8_192;

/// Result of one completely verified chunk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuffixChunkVerdict {
    /// Adjusted public fields after the chunk's last step.
    pub end: KagemushaWalletLineagePublicV1,
    /// The verifier's own bounded segment record, retained in consensus state and handed back
    /// to [`SuffixUnloadVerifier::verify_finalize`] in ordinal order. It is DATA; it is
    /// authority only when read back from consensus state that this verification wrote.
    pub record: Vec<u8>,
}

/// The complete native verifier of suffix stages, bound to the scheme's installed artifacts.
///
/// Every input is already shape-checked, size-bounded and bound to the ledger's registration,
/// wallet row, installed manifest and verifier definition. Implementations verify every proof,
/// decide, signature and opening with the production owners; there is deliberately no default
/// or structural-only implementation. A failure must never be translated into success.
pub trait SuffixUnloadVerifier {
    /// Begin: the genuine anchor Ω under the installed keys (every decide), the anchor state
    /// opening of Ω's head, the receiver credential and certificates, and an early check of
    /// the header's Unload σ for `header.statement` (before the phone's irreversible Advance).
    ///
    /// # Errors
    /// `ArtifactsUnavailable` without an installed artifact set; `Proof` or a model error for
    /// anything that does not verify.
    fn verify_begin(
        &self,
        scheme: &KagemushaWalletSchemeV1,
        header: &Header,
        anchor: &Anchor,
    ) -> Result<()>;

    /// Append: verify every step completely and in order, starting from the claimed `start`
    /// (the adjusted public fields before `steps[0]`, whose `before` opens `start.head`).
    /// Return the adjusted public fields after the last step and the verifier's segment
    /// record. Every claimed start is proven at Finalize by linking all segments to the anchor.
    ///
    /// # Errors
    /// As [`Self::verify_begin`].
    fn verify_chunk(
        &self,
        scheme: &KagemushaWalletSchemeV1,
        header: &Header,
        anchor: &Anchor,
        start: &KagemushaWalletLineagePublicV1,
        steps: &[Step],
    ) -> Result<SuffixChunkVerdict>;

    /// Finalize: link every committed segment record (`records`, in ordinal order) from the
    /// anchor that this suffix's committed Begin verified into the tip (`tip` opens
    /// `header.tip`), and verify the Unload transition to `unload.after`, the recovery-map
    /// insertion, its σ, every signature of the Unload terms and τ binding `authority` (the
    /// header's authority digest).
    ///
    /// # Errors
    /// As [`Self::verify_begin`].
    #[allow(clippy::too_many_arguments)]
    fn verify_finalize(
        &self,
        scheme: &KagemushaWalletSchemeV1,
        header: &Header,
        authority: &Digest,
        anchor: &Anchor,
        records: &[Vec<u8>],
        tip: &KagemushaWalletStateV1,
        unload: &Unload,
    ) -> Result<()>;
}

/// Begin row: the exact header and anchor originals every later stage reads.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::kagemusha_wallet_v1::SuffixBegin")]
pub struct SuffixBegin {
    /// Row version; exactly one.
    pub version: u16,
    /// Wallet incarnation named by the header.
    pub wallet: Digest,
    /// Registered asset of the wallet row.
    pub asset: Digest,
    /// Exact canonical header frame; its authority digest is the row key.
    pub header: Vec<u8>,
    /// Exact canonical anchor frame; its digest is the header's `anchor_digest`.
    pub anchor: Vec<u8>,
    /// Transaction that first executed this Begin.
    pub transaction: Digest,
}

/// Stage row: one completely verified chunk and its verified end.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::kagemusha_wallet_v1::SuffixStage")]
#[repr(align(16))]
pub struct SuffixStage {
    /// Row version; exactly one.
    pub version: u16,
    /// Wallet incarnation named by the header.
    pub wallet: Digest,
    /// Append ordinal, from one.
    pub ordinal: u128,
    /// Chunk frame digest; the header lists it at this ordinal.
    pub chunk: Digest,
    /// Stream bytes of all earlier chunks.
    pub stream_offset: u128,
    /// Stream bytes of this chunk.
    pub data_bytes: u128,
    /// Steps of this chunk.
    pub steps: u128,
    /// Sequence of the state before the first step.
    pub start_sequence: u128,
    /// Claimed adjusted public fields before the first step.
    pub start: KagemushaWalletLineagePublicV1,
    /// Verified adjusted public fields after the last step.
    pub end: KagemushaWalletLineagePublicV1,
    /// Full state after the last step; it opens `end.head`.
    pub end_state: KagemushaWalletStateV1,
    /// The verifier's segment record of this chunk.
    pub record: Vec<u8>,
    /// Transaction that first executed this Append.
    pub transaction: Digest,
}

/// Online charge leg of a suffix payout.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::kagemusha_wallet_v1::SuffixCharge")]
#[repr(align(16))]
pub struct SuffixCharge {
    /// Account the charge quote names.
    pub beneficiary: AccountId,
    /// Charge paid from the reserve.
    pub amount: u128,
}

/// Final row: the exact Finalize and the payout it made, read by the movement index.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::kagemusha_wallet_v1::SuffixFinal")]
#[repr(align(16))]
pub struct SuffixFinal {
    /// Row version; exactly one.
    pub version: u16,
    /// Wallet incarnation named by the header.
    pub wallet: Digest,
    /// Registered asset of the wallet row.
    pub asset: Digest,
    /// Finalize ordinal: the chunk count plus one.
    pub ordinal: u128,
    /// Finalize frame digest.
    pub unload: Digest,
    /// Exact copy of the unchanged Unload payout row.
    pub payout: KagemushaWalletPayoutRecordV1,
    /// Paid account.
    pub account: AccountId,
    /// Reserve to account leg.
    pub account_payout: u128,
    /// Reserve to beneficiary leg of a quoted charge.
    pub charge: Option<SuffixCharge>,
}

/// One stage instruction's outer identifiers.
#[derive(Debug, Clone, Copy)]
pub struct StageId<'a> {
    /// Scheme of the instruction.
    pub scheme: Digest,
    /// Wallet incarnation.
    pub wallet: Digest,
    /// Authority digest of the Begin header.
    pub authority: Digest,
    /// Paid account.
    pub account: &'a AccountId,
    /// Stage ordinal.
    pub ordinal: u128,
}

/// Suffix rows and work reservation inside the same atomic WSV transaction as `apply`.
pub trait SuffixTransaction: Transaction {
    /// Read one validated suffix row's exact bytes; only a genuinely absent row is `None`.
    ///
    /// # Errors
    /// Unavailable storage or a row that fails validation.
    fn suffix_row(&self, key: &KagemushaWalletLedgerKeyV1) -> Result<Option<Vec<u8>>>;
    /// Installed verifier identity of `scheme`: the signed manifest digest and the
    /// verifier-definition digest of the exact installed pack original.
    ///
    /// # Errors
    /// `ArtifactsUnavailable` when the scheme has no installation.
    fn suffix_installation(&self, scheme: &Digest) -> Result<(Digest, Digest)>;
    /// Reserve a whole stage's exact proof work under the existing quotas before any proof.
    ///
    /// # Errors
    /// The existing per-transaction or per-block quota refuses the work.
    fn reserve_suffix_proofs(&mut self, proof_bytes: usize, verify_calls: u32) -> Result<()>;
    /// Insert immutable suffix rows: an identical row is a no-op, any other row conflicts.
    ///
    /// # Errors
    /// Encoding failure or a conflicting row.
    fn insert_suffix(&mut self, rows: Vec<(KagemushaWalletLedgerKeyV1, Vec<u8>)>) -> Result<()>;
}

pub(super) fn begin_key(scheme: Digest, authority: Digest) -> KagemushaWalletLedgerKeyV1 {
    storage::key(BEGIN, scheme, authority)
}
/// Key of the Final row of the suffix Unload with this authority digest.
#[must_use]
pub fn final_key(scheme: Digest, authority: Digest) -> KagemushaWalletLedgerKeyV1 {
    storage::key(FINAL, scheme, authority)
}
pub(super) fn ordinal_word(ordinal: u128) -> Digest {
    let mut word = [0; 32];
    word[16..].copy_from_slice(&ordinal.to_be_bytes());
    word
}
pub(super) fn stage_key(
    scheme: Digest,
    authority: Digest,
    ordinal: u128,
) -> KagemushaWalletLedgerKeyV1 {
    storage::entry_key(STAGE, scheme, authority, ordinal_word(ordinal))
}

fn read<T>(tx: &impl SuffixTransaction, key: &KagemushaWalletLedgerKeyV1) -> Result<Option<T>>
where
    T: norito::NoritoSerialize,
    for<'de> T: norito::NoritoDeserialize<'de>,
{
    tx.suffix_row(key)?
        .map(|bytes| storage::decode(&bytes, bytes.len()))
        .transpose()
}

fn length(value: usize) -> Result<u128> {
    u128::try_from(value).map_err(|_| Error::Overflow)
}

/// Header and anchor of a committed Begin, with the outer identifiers bound to it.
fn committed_begin(
    tx: &impl SuffixTransaction,
    stage: &StageId<'_>,
) -> Result<(SuffixBegin, Header, Anchor)> {
    let begin: SuffixBegin =
        read(tx, &begin_key(stage.scheme, stage.authority))?.ok_or(Error::Unavailable)?;
    let header = Header::decode_canonical(&begin.header, &stage.scheme)?;
    let anchor = Anchor::decode_canonical(&begin.anchor, &stage.scheme)?;
    if begin.wallet != stage.wallet || header.account != *stage.account {
        return Err(Error::Binding);
    }
    Ok((begin, header, anchor))
}

/// Verify and record Begin: the immutable header and the genuine anchor. No value moves.
///
/// # Errors
/// Rejects a nonzero ordinal, outer identifiers other than the header's, an unknown or
/// abandoned wallet, another network, asset, manifest or verifier definition, an anchor of
/// another digest or binding, quota refusal, any verification failure and a conflicting Begin.
pub fn begin(
    tx: &mut impl SuffixTransaction,
    verifier: &impl SuffixUnloadVerifier,
    stage: &StageId<'_>,
    header_frame: &[u8],
    anchor_frame: &[u8],
) -> Result<()> {
    if stage.ordinal != 0 {
        return Err(Error::Binding);
    }
    let header = Header::decode_canonical(header_frame, &stage.scheme)?;
    if kagemusha_wallet_suffix_authority_digest_v1(header_frame)? != stage.authority
        || header.wallet_id != stage.wallet
        || header.account != *stage.account
    {
        return Err(Error::Binding);
    }
    // An exact retry is answered from the immutable row before any proof work.
    let key = begin_key(stage.scheme, stage.authority);
    if let Some(prior) = read::<SuffixBegin>(tx, &key)? {
        return if prior.wallet == stage.wallet
            && prior.header == header_frame
            && prior.anchor == anchor_frame
        {
            Ok(())
        } else {
            Err(Error::Conflict)
        };
    }
    let wallet = tx
        .wallet(&stage.scheme, &stage.wallet)?
        .ok_or(Error::Lifecycle)?;
    if wallet.phase == Phase::Abandoned {
        return Err(Error::Lifecycle);
    }
    if wallet.asset != header.statement.asset_digest {
        return Err(Error::Binding);
    }
    let registration = tx.registration(&stage.scheme, &wallet.asset)?;
    registration.require(&stage.scheme, &wallet.asset)?;
    if header.network_id != registration.scheme.network_id {
        return Err(Error::Binding);
    }
    header.statement.validate_for_scheme(&registration.scheme)?;
    let (manifest, definition) = tx.suffix_installation(&stage.scheme)?;
    if header.artifact_manifest != manifest || header.verifier_definition != definition {
        return Err(Error::Binding);
    }
    if kagemusha_wallet_suffix_digest_v1(Role::Anchor, anchor_frame) != header.anchor_digest {
        return Err(Error::Binding);
    }
    let anchor = Anchor::decode_canonical(anchor_frame, &stage.scheme)?;
    anchor.require_header(&header)?;
    header
        .statement
        .validate_for_credential(&anchor.credential)?;
    let proof_bytes = anchor
        .lineage
        .proof
        .len()
        .checked_add(header.sigma.bytes.len())
        .ok_or(Error::Overflow)?;
    tx.reserve_suffix_proofs(proof_bytes, BEGIN_PROOF_CALLS)?;
    verifier.verify_begin(&registration.scheme, &header, &anchor)?;
    let row = SuffixBegin {
        version: ROW_VERSION,
        wallet: stage.wallet,
        asset: wallet.asset,
        header: header_frame.to_vec(),
        anchor: anchor_frame.to_vec(),
        transaction: tx.transaction_hash(),
    };
    tx.insert_suffix(vec![(key, storage::encode(&row)?)])
}

/// Exact proof work of one chunk: Receive charges its own σ and the payer's σ_send and Ω;
/// a prior suffix Unload charges its own σ.
fn chunk_proof_work(scheme: &Digest, steps: &[Step]) -> Result<(usize, u32)> {
    let mut bytes = 0_usize;
    let mut calls = 0_u32;
    for step in steps {
        let (step_bytes, step_calls) = match &step.kind {
            StepKind::Receive { .. } => {
                let payment = KagemushaWalletPaymentV1::decode_canonical(
                    step.original(OriginalRole::Payment).ok_or(Error::Binding)?,
                    scheme,
                )?;
                let lineage = payment.send.lineage.lineage();
                let payer = payment
                    .send
                    .step_proof
                    .bytes
                    .len()
                    .checked_add(lineage.map_or(0, |value| value.proof.len()))
                    .ok_or(Error::Overflow)?;
                (
                    step.sigma
                        .bytes
                        .len()
                        .checked_add(payer)
                        .ok_or(Error::Overflow)?,
                    if lineage.is_some() { 3 } else { 2 },
                )
            }
            StepKind::PriorUnload { .. } => (step.sigma.bytes.len(), 1),
        };
        bytes = bytes.checked_add(step_bytes).ok_or(Error::Overflow)?;
        calls = calls.checked_add(step_calls).ok_or(Error::Overflow)?;
    }
    Ok((bytes, calls))
}

/// Verify one chunk completely from its claimed start and record its verified end.
///
/// # Errors
/// Rejects an Append before its Begin, outer identifiers other than the Begin's, a chunk the
/// header does not list at this ordinal, malformed or out-of-range steps, a broken in-chunk
/// chain, quota refusal, any verification failure and a conflicting stage.
pub fn append(
    tx: &mut impl SuffixTransaction,
    verifier: &impl SuffixUnloadVerifier,
    stage: &StageId<'_>,
    chunk_frame: &[u8],
) -> Result<()> {
    let (begin, header, anchor) = committed_begin(tx, stage)?;
    let expected = header.chunk_digest(stage.ordinal).ok_or(Error::Binding)?;
    let digest = kagemusha_wallet_suffix_digest_v1(Role::Chunk, chunk_frame);
    if digest != expected {
        return Err(Error::Binding);
    }
    let key = stage_key(stage.scheme, stage.authority, stage.ordinal);
    if let Some(prior) = read::<SuffixStage>(tx, &key)? {
        return if prior.wallet == stage.wallet && prior.chunk == digest {
            Ok(())
        } else {
            Err(Error::Conflict)
        };
    }
    let chunk = Chunk::decode_canonical(chunk_frame, &stage.scheme)?;
    if chunk.chunk_index.checked_add(1) != Some(stage.ordinal)
        || chunk.start.wallet_id != stage.wallet
    {
        return Err(Error::Binding);
    }
    let data_bytes = length(chunk.data.len())?;
    if data_bytes > length(KAGEMUSHA_WALLET_SUFFIX_CHUNK_DATA_MAX_BYTES_V1)?
        || chunk
            .stream_offset
            .checked_add(data_bytes)
            .is_none_or(|end| end > header.stream_bytes)
    {
        return Err(Error::Binding);
    }
    let steps = chunk.decode_steps(&stage.scheme)?;
    let (Some(first), Some(last)) = (steps.first(), steps.last()) else {
        return Err(Error::Binding);
    };
    if steps.len() > KAGEMUSHA_WALLET_SUFFIX_CHUNK_STEPS_MAX_V1
        || first.before.core.sequence < header.anchor_sequence
        || last.after.core.sequence > header.tip_sequence
        || first.before.commitment()? != chunk.start.head
        || steps.windows(2).any(|pair| pair[0].after != pair[1].before)
    {
        return Err(Error::Binding);
    }
    let registration = tx.registration(&stage.scheme, &begin.asset)?;
    registration.require(&stage.scheme, &begin.asset)?;
    let (proof_bytes, verify_calls) = chunk_proof_work(&stage.scheme, &steps)?;
    tx.reserve_suffix_proofs(proof_bytes, verify_calls)?;
    let SuffixChunkVerdict { end, record } =
        verifier.verify_chunk(&registration.scheme, &header, &anchor, &chunk.start, &steps)?;
    if record.is_empty()
        || record.len() > SUFFIX_SEGMENT_RECORD_MAX_BYTES
        || end.head != last.after.commitment()?
        || end.wallet_id != stage.wallet
        || end.scheme_id != stage.scheme
    {
        return Err(Error::Binding);
    }
    let row = SuffixStage {
        version: ROW_VERSION,
        wallet: stage.wallet,
        ordinal: stage.ordinal,
        chunk: digest,
        stream_offset: chunk.stream_offset,
        data_bytes,
        steps: length(steps.len())?,
        start_sequence: first.before.core.sequence,
        start: chunk.start,
        end,
        end_state: last.after,
        record,
        transaction: tx.transaction_hash(),
    };
    tx.insert_suffix(vec![(key, storage::encode(&row)?)])
}

/// Chain every recorded chunk from the anchor Ω to the header's tip; return the tip state and
/// the verifier's segment records in ordinal order.
///
/// # Errors
/// `Unavailable` while any Append is not yet committed; `Binding` for any break in the chain.
fn chained_tip(
    tx: &impl SuffixTransaction,
    stage: &StageId<'_>,
    header: &Header,
    anchor: &Anchor,
) -> Result<(KagemushaWalletStateV1, Vec<Vec<u8>>)> {
    let mut start = anchor.lineage.public;
    let mut sequence = header.anchor_sequence;
    let mut offset = 0_u128;
    let mut steps = 0_u128;
    let mut tip = None;
    let mut records = Vec::new();
    for ordinal in 1..=header.chunk_count {
        let chunk: SuffixStage = read(tx, &stage_key(stage.scheme, stage.authority, ordinal))?
            .ok_or(Error::Unavailable)?;
        if chunk.wallet != stage.wallet
            || Some(chunk.chunk) != header.chunk_digest(ordinal)
            || chunk.start != start
            || chunk.start_sequence != sequence
            || chunk.stream_offset != offset
        {
            return Err(Error::Binding);
        }
        offset = offset
            .checked_add(chunk.data_bytes)
            .ok_or(Error::Overflow)?;
        steps = steps.checked_add(chunk.steps).ok_or(Error::Overflow)?;
        sequence = chunk.end_state.core.sequence;
        start = chunk.end;
        tip = Some(chunk.end_state);
        records.push(chunk.record);
    }
    let tip = tip.ok_or(Error::Binding)?;
    if offset != header.stream_bytes
        || steps != header.step_count
        || sequence != header.tip_sequence
        || start != header.tip_adjusted
        || tip.commitment()? != header.tip
    {
        return Err(Error::Binding);
    }
    Ok((tip, records))
}

/// Chain all chunks, verify the Unload and pay it once at face value, split only by its
/// signed online charge. An exact retry returns the original payout and moves nothing.
///
/// # Errors
/// Rejects a Finalize before its Begin or before every Append (`Unavailable`, resendable),
/// another ordinal or outer identifier, a broken chain, an Unload of other terms, any
/// verification failure, a nullifier already paid to another claim (`Conflict`), and an
/// unfunded reserve.
pub fn finalize(
    tx: &mut impl SuffixTransaction,
    verifier: &impl SuffixUnloadVerifier,
    stage: &StageId<'_>,
    unload_frame: &[u8],
) -> Result<KagemushaWalletPayoutRecordV1> {
    let (begin, header, anchor) = committed_begin(tx, stage)?;
    if Some(stage.ordinal) != header.finalize_ordinal().ok() {
        return Err(Error::Binding);
    }
    let unload_digest = kagemusha_wallet_suffix_digest_v1(Role::Unload, unload_frame);
    let key = final_key(stage.scheme, stage.authority);
    if let Some(prior) = read::<SuffixFinal>(tx, &key)? {
        return if prior.wallet == stage.wallet && prior.unload == unload_digest {
            Ok(prior.payout)
        } else {
            Err(Error::Conflict)
        };
    }
    let terms = header.payout_terms()?;
    let payout_key = KagemushaWalletPayoutKeyV1::Unload(terms.nullifier);
    // Another claim (ordinary Unload or another suffix) already paid this nullifier.
    if tx.payout(&stage.scheme, payout_key)?.is_some() {
        return Err(Error::Conflict);
    }
    let (tip, records) = chained_tip(tx, stage, &header, &anchor)?;
    let unload = Unload::decode_canonical(unload_frame, &stage.scheme)?;
    if unload.require_header(&header)? != terms {
        return Err(Error::Binding);
    }
    let registration = tx.registration(&stage.scheme, &begin.asset)?;
    registration.require(&stage.scheme, &begin.asset)?;
    tx.reserve_suffix_proofs(header.sigma.bytes.len(), FINALIZE_PROOF_CALLS)?;
    verifier.verify_finalize(
        &registration.scheme,
        &header,
        &stage.authority,
        &anchor,
        &records,
        &tip,
        &unload,
    )?;
    let payout = KagemushaWalletPayoutRecordV1 {
        key: payout_key,
        source: kagemusha_wallet_suffix_payout_source_v1(&stage.authority, &unload_digest),
        amount: terms.amount,
        transaction: tx.transaction_hash(),
    };
    let mut batch = Batch::new(stage.scheme, begin.asset);
    if terms.account_payout != 0 {
        batch.transfers.push(Transfer {
            from: registration.reserve.clone(),
            to: header.account.clone(),
            amount: terms.account_payout,
        });
    }
    let charge = header.charge.beneficiary().map(|beneficiary| SuffixCharge {
        beneficiary: beneficiary.clone(),
        amount: terms.online_charge,
    });
    if let Some(charge) = &charge {
        batch.transfers.push(Transfer {
            from: registration.reserve,
            to: charge.beneficiary.clone(),
            amount: charge.amount,
        });
    }
    batch.payout = Some(payout);
    tx.apply(batch)?;
    let row = SuffixFinal {
        version: ROW_VERSION,
        wallet: stage.wallet,
        asset: begin.asset,
        ordinal: stage.ordinal,
        unload: unload_digest,
        payout,
        account: header.account,
        account_payout: terms.account_payout,
        charge,
    };
    tx.insert_suffix(vec![(key, storage::encode(&row)?)])?;
    Ok(payout)
}

// ---------------------------------------------------------------------------------------
// Restored-row validation
// ---------------------------------------------------------------------------------------

/// Validate one restored suffix row's exact key, schema and self-contained bindings.
pub(super) fn validate_suffix_row(
    kind: u8,
    scheme: &Digest,
    owner: &Digest,
    entry: &Digest,
    bytes: &[u8],
) -> Result<()> {
    match kind {
        BEGIN => {
            let row: SuffixBegin = storage::decode(bytes, BEGIN_CAP)?;
            let header = Header::decode_canonical(&row.header, scheme)?;
            let anchor = Anchor::decode_canonical(&row.anchor, scheme)?;
            if row.version != ROW_VERSION
                || *entry != [0; 32]
                || row.transaction == [0; 32]
                || row.wallet != header.wallet_id
                || row.asset != header.statement.asset_digest
                || kagemusha_wallet_suffix_authority_digest_v1(&row.header)? != *owner
                || kagemusha_wallet_suffix_digest_v1(Role::Anchor, &row.anchor)
                    != header.anchor_digest
            {
                return Err(Error::Binding);
            }
            anchor.require_header(&header)?;
            Ok(())
        }
        STAGE => {
            let row: SuffixStage = storage::decode(bytes, STAGE_CAP)?;
            row.start.validate()?;
            row.end.validate()?;
            row.end_state.validate()?;
            let steps = usize::try_from(row.steps).map_err(|_| Error::Binding)?;
            let segment =
                SuffixSegmentV1::decode_canonical(&row.record).map_err(|_| Error::Binding)?;
            if segment.scheme_id != *scheme
                || segment.credential_digest != row.start.credential_digest
                || segment.start != row.start.head
                || segment.start_sequence != row.start_sequence
                || segment.start_adjusted != row.start
                || segment.end != row.end_state
                || segment.end_adjusted != row.end
                || u128::from(segment.steps) != row.steps
                || segment.receives.checked_add(segment.prior_unloads) != Some(segment.steps)
                || segment.manifest_digest == [0; 32]
                || segment.chain == [0; 32]
                || !kagemusha_wallet_is_canonical_field_v1(&segment.chain)
            {
                return Err(Error::Binding);
            }
            if row.version != ROW_VERSION
                || row.ordinal == 0
                || *entry != ordinal_word(row.ordinal)
                || row.chunk == [0; 32]
                || row.transaction == [0; 32]
                || steps == 0
                || steps > KAGEMUSHA_WALLET_SUFFIX_CHUNK_STEPS_MAX_V1
                || row.data_bytes == 0
                || row.data_bytes > length(KAGEMUSHA_WALLET_SUFFIX_CHUNK_DATA_MAX_BYTES_V1)?
                || row.record.is_empty()
                || row.record.len() > SUFFIX_SEGMENT_RECORD_MAX_BYTES
                || row.start.scheme_id != *scheme
                || row.end.scheme_id != *scheme
                || row.start.wallet_id != row.wallet
                || row.end.wallet_id != row.wallet
                || row.start_sequence.checked_add(row.steps) != Some(row.end_state.core.sequence)
                || row.end.head != row.end_state.commitment()?
            {
                return Err(Error::Binding);
            }
            Ok(())
        }
        FINAL => {
            let row: SuffixFinal = storage::decode(bytes, FINAL_CAP)?;
            let KagemushaWalletPayoutKeyV1::Unload(nullifier) = row.payout.key else {
                return Err(Error::Binding);
            };
            let charge = row.charge.as_ref().map_or(0, |charge| charge.amount);
            if row.version != ROW_VERSION
                || *entry != [0; 32]
                || nullifier == [0; 32]
                || row.ordinal < 2
                || row.unload == [0; 32]
                || row.payout.transaction == [0; 32]
                || row.payout.amount == 0
                || row.payout.source != kagemusha_wallet_suffix_payout_source_v1(owner, &row.unload)
                || row.account_payout.checked_add(charge) != Some(row.payout.amount)
                || row.charge.as_ref().is_some_and(|charge| charge.amount == 0)
            {
                return Err(Error::Binding);
            }
            Ok(())
        }
        _ => Err(Error::Binding),
    }
}

/// Cross-check one restored suffix row against its Begin (and a Final against its payout row)
/// in the same snapshot generation.
pub(super) fn validate_snapshot_row<'a>(
    key: &KagemushaWalletLedgerKeyV1,
    bytes: &[u8],
    lookup: &impl Fn(&KagemushaWalletLedgerKeyV1) -> Option<&'a [u8]>,
) -> Result<()> {
    let (kind, scheme, authority, _) = key.components();
    if kind == BEGIN {
        return Ok(());
    }
    // The Begin row is itself a row of this snapshot and is validated in full there; here it
    // only has to exist and name this stage.
    let begin_bytes = lookup(&begin_key(scheme, authority)).ok_or(Error::Unavailable)?;
    let begin: SuffixBegin = storage::decode(begin_bytes, BEGIN_CAP)?;
    let header = Header::decode_canonical(&begin.header, &scheme)?;
    match kind {
        STAGE => {
            let row: SuffixStage = storage::decode(bytes, STAGE_CAP)?;
            let segment =
                SuffixSegmentV1::decode_canonical(&row.record).map_err(|_| Error::Binding)?;
            if row.wallet != begin.wallet
                || header.chunk_digest(row.ordinal) != Some(row.chunk)
                || segment.manifest_digest != header.artifact_manifest
                || segment.credential_digest != header.tip_adjusted.credential_digest
                || row.start_sequence < header.anchor_sequence
                || row.end_state.core.sequence > header.tip_sequence
                || row
                    .stream_offset
                    .checked_add(row.data_bytes)
                    .is_none_or(|end| end > header.stream_bytes)
            {
                return Err(Error::Binding);
            }
        }
        FINAL => {
            let row: SuffixFinal = storage::decode(bytes, FINAL_CAP)?;
            let terms = header.payout_terms()?;
            let payout_key = row.payout.key.ledger_key(scheme);
            let payout_bytes = lookup(&payout_key).ok_or(Error::Unavailable)?;
            storage::validate_row(&payout_key, payout_bytes)?;
            let payout: KagemushaWalletPayoutRecordV1 =
                storage::decode(payout_bytes, storage::PAYOUT_CAP)?;
            if row.wallet != begin.wallet
                || row.asset != begin.asset
                || Some(row.ordinal) != header.finalize_ordinal().ok()
                || payout != row.payout
                || row.payout.key != KagemushaWalletPayoutKeyV1::Unload(terms.nullifier)
                || row.payout.amount != terms.amount
                || row.account != header.account
                || row.account_payout != terms.account_payout
                || row.charge
                    != header.charge.beneficiary().map(|beneficiary| SuffixCharge {
                        beneficiary: beneficiary.clone(),
                        amount: terms.online_charge,
                    })
            {
                return Err(Error::Binding);
            }
        }
        _ => return Err(Error::Binding),
    }
    Ok(())
}

/// Begin row bound: both bounded originals and the fixed fields.
pub(super) const BEGIN_CAP: usize = iroha_data_model::kagemusha::kagemusha_wallet_suffix_unload_v1::KAGEMUSHA_WALLET_SUFFIX_HEADER_MAX_BYTES_V1
    + iroha_data_model::kagemusha::kagemusha_wallet_suffix_unload_v1::KAGEMUSHA_WALLET_SUFFIX_ANCHOR_MAX_BYTES_V1
    + 1_024;
/// Stage row bound: two adjusted public records, one state, the segment record and fixed
/// fields.
pub(super) const STAGE_CAP: usize = SUFFIX_SEGMENT_RECORD_MAX_BYTES + 8_192;
/// Final row bound: fixed fields and two full canonical accounts.
pub(super) const FINAL_CAP: usize = 16_384;
