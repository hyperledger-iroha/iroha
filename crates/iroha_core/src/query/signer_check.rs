//! Exact signed native custody Check execution and same-State finalized lineage.
//!
//! This crate-private owner authenticates closed native Check purposes. It never
//! accepts an eligibility callback, supplies a clock, or turns a caller-provided history into
//! authority. Purpose-owned wrappers must check current custody and both UTC endpoints before
//! producing their distinct successes. Historical finality alone is not a current-authority read.

use crate::{
    state::{State, StateReadOnly, StateView, TransactionsReadOnly},
    sumeragi::certified_chain::CertifiedChain,
};
use iroha_crypto::{Algorithm, HashOf, Signature};
use iroha_data_model::{
    account::AccountId,
    block::consensus_v2::HeightContextId,
    isi::{
        InstructionBox,
        sorafs::{
            MutateSorafsFinalPromotionAccountCustody, MutateSorafsFinalPromotionAuthority,
            MutateSorafsReleaseManifestAuthority, MutateSorafsStreamTokenAuthority,
            MutateSorafsTopologyAuthority,
        },
    },
    sorafs::{
        final_promotion_account_custody::FinalPromotionAccountCustodyActionV1,
        final_promotion_authority::FinalPromotionAuthorityActionV1,
        release_manifest_authority::ReleaseManifestActionV1,
        stream_token_authority::StreamTokenAuthorityActionV1, topology_authority::TopologyActionV1,
    },
    transaction::{
        Executable, SignedTransaction, TransactionBuilder, TransactionEntrypoint,
        TransactionPayload,
    },
};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

/// Maximum canonical signed External frame bytes for native final-promotion transactions.
/// This is a structural capacity bound, not signing, spending, custody or currentness authority.
pub const FINAL_PROMOTION_NATIVE_TRANSACTION_MAX_BYTES_V1: usize = 64 * 1024;
const MAX_ROUND: Duration = Duration::from_secs(60);
/// Maximum canonical lineage a one-use native Check may replay from its trusted floor.
/// Shared preflight runs before any Kura finality or block-body read.
const MAX_NATIVE_CHECK_HISTORY_BLOCKS_V1: u64 = 4_096;

fn check_history_span_v1(floor_height: u64, applied_height: u64) -> Result<(), Error> {
    if applied_height
        .checked_sub(floor_height)
        .and_then(|distance| distance.checked_add(1))
        .is_none_or(|span| span > MAX_NATIVE_CHECK_HISTORY_BLOCKS_V1)
    {
        return Err(Error::Finality);
    }
    Ok(())
}

/// Shared proof failures, mapped into each purpose's payload-free public errors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NativeCheckErrorV1 {
    Invalid,
    Expired,
    Entropy,
    Transaction,
    NotApplied,
    Finality,
    Execution,
}
use NativeCheckErrorV1 as Error;

/// Independent coordinates, not a decoded authority capability.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct NativeCheckFloorV1 {
    pub(crate) height: u64,
    pub(crate) block_hash: [u8; 32],
    pub(crate) context_id: HeightContextId,
}
impl NativeCheckFloorV1 {
    pub(crate) fn validate(&self) -> Result<(), Error> {
        if self.height == 0 || self.block_hash == [0; 32] || *self.context_id.0.as_ref() == [0; 32]
        {
            Err(Error::Invalid)
        } else {
            Ok(())
        }
    }
}

/// Original monotonic lifetime, started before pure expectation validation and entropy.
/// There is no Clone, decoded constructor or renewal method.
pub(crate) struct NativeCheckRoundV1 {
    started: Instant,
    max_elapsed: Duration,
    challenge: Option<[u8; 32]>,
    bound: bool,
}
impl NativeCheckRoundV1 {
    pub(crate) fn start(max_elapsed: Duration) -> Result<Self, Error> {
        let started = Instant::now();
        if max_elapsed.is_zero() || max_elapsed > MAX_ROUND {
            return Err(Error::Invalid);
        }
        Ok(Self {
            started,
            max_elapsed,
            challenge: None,
            bound: false,
        })
    }
    /// Called only after the purpose wrapper's pure preflight, once per prepared attempt.
    pub(crate) fn issue_challenge(&mut self) -> Result<[u8; 32], Error> {
        self.ensure_live()?;
        if self.challenge.is_some() {
            return Err(Error::Invalid);
        }
        // Mark spent before entropy; an error cannot be retried on this round.
        self.challenge = Some([0; 32]);
        let mut challenge = [0; 32];
        rand::TryRngCore::try_fill_bytes(&mut rand::rngs::OsRng, &mut challenge)
            .map_err(|_| Error::Entropy)?;
        if challenge == [0; 32] {
            return Err(Error::Entropy);
        }
        self.ensure_live()?;
        self.challenge = Some(challenge);
        Ok(challenge)
    }
    pub(crate) fn ensure_live(&self) -> Result<(), Error> {
        if self.started.elapsed() >= self.max_elapsed {
            Err(Error::Expired)
        } else {
            Ok(())
        }
    }
    #[cfg(test)]
    pub(crate) fn expire_for_test(&mut self) {
        self.started = Instant::now() - Duration::from_secs(61);
    }
}

/// Closed native purposes; none can substitute for another's proof consumer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NativeCustodyCheckPurposeV1 {
    FinalPromotion,
    FinalPromotionAccount,
    ReleaseManifest,
    StreamToken,
    /// Proof binding only; role-16 Core execution and current authority remain closed.
    Topology,
}
/// Purpose-typed native instructions can enter the common proof path.
///
/// The role-13 and role-16 variants bind signed Checks to execution evidence only. Their
/// Core instructions remain closed and cannot produce signer or topology authority.
pub(crate) enum NativeCustodyCheckRefV1<'a> {
    FinalPromotion(&'a MutateSorafsFinalPromotionAuthority),
    FinalPromotionAccount(&'a MutateSorafsFinalPromotionAccountCustody),
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "release-manifest native Check has no production caller yet"
        )
    )]
    ReleaseManifest(&'a MutateSorafsReleaseManifestAuthority),
    StreamToken(&'a MutateSorafsStreamTokenAuthority),
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "topology native Check has no production caller yet"
        )
    )]
    Topology(&'a MutateSorafsTopologyAuthority),
}
impl NativeCustodyCheckRefV1<'_> {
    fn coordinates(
        &self,
    ) -> Result<
        (
            NativeCustodyCheckPurposeV1,
            [u8; 32],
            [u8; 32],
            u64,
            [u8; 32],
            Option<HeightContextId>,
        ),
        Error,
    > {
        match self {
            Self::FinalPromotion(instruction) => {
                let FinalPromotionAuthorityActionV1::Check(check) = &instruction.action else {
                    return Err(Error::Transaction);
                };
                Ok((
                    NativeCustodyCheckPurposeV1::FinalPromotion,
                    check.challenge,
                    check.network_id,
                    check.minimum_height,
                    check.minimum_block_hash,
                    None,
                ))
            }
            Self::FinalPromotionAccount(instruction) => {
                let FinalPromotionAccountCustodyActionV1::Check(check) = &instruction.action else {
                    return Err(Error::Transaction);
                };
                Ok((
                    NativeCustodyCheckPurposeV1::FinalPromotionAccount,
                    check.challenge,
                    check.network_id,
                    check.minimum_height,
                    check.minimum_block_hash,
                    None,
                ))
            }
            Self::ReleaseManifest(instruction) => {
                let ReleaseManifestActionV1::Check(check) = &instruction.action else {
                    return Err(Error::Transaction);
                };
                // This binds a signed role-13 Check only to exact execution evidence. Its Core
                // instruction remains closed, so this cannot grant signer-operation authority.
                Ok((
                    NativeCustodyCheckPurposeV1::ReleaseManifest,
                    check.challenge,
                    check.network_id,
                    check.floor.height,
                    check.floor.block_hash,
                    None,
                ))
            }
            Self::StreamToken(instruction) => {
                let StreamTokenAuthorityActionV1::Check(check) = &instruction.request.action else {
                    return Err(Error::Transaction);
                };
                Ok((
                    NativeCustodyCheckPurposeV1::StreamToken,
                    check.challenge,
                    instruction.request.network_id,
                    check.floor.height,
                    check.floor.block_hash,
                    Some(check.floor.context_id),
                ))
            }
            Self::Topology(instruction) => {
                let TopologyActionV1::Check(check) = &instruction.transition.action else {
                    return Err(Error::Transaction);
                };
                // The signed role-16 Check carries height/hash, while the independent floor
                // owner supplies the context ID. This is proof plumbing, not topology authority.
                Ok((
                    NativeCustodyCheckPurposeV1::Topology,
                    check.challenge,
                    check.network_id,
                    check.floor.height,
                    check.floor.block_hash,
                    None,
                ))
            }
        }
    }
    fn instruction(&self) -> InstructionBox {
        match self {
            Self::FinalPromotion(instruction) => (*instruction).clone().into(),
            Self::FinalPromotionAccount(instruction) => (*instruction).clone().into(),
            Self::ReleaseManifest(instruction) => (*instruction).clone().into(),
            Self::StreamToken(instruction) => (*instruction).clone().into(),
            Self::Topology(instruction) => (*instruction).clone().into(),
        }
    }
}

/// Frozen signed envelope and independent coordinates. Only exact native binding constructs it.
pub(crate) struct BoundNativeCheckV1 {
    purpose: NativeCustodyCheckPurposeV1,
    chain_id: String,
    network_id: [u8; 32],
    floor: NativeCheckFloorV1,
    started: Instant,
    max_elapsed: Duration,
    challenge: [u8; 32],
    signed: SignedTransaction,
    entry_bytes: Vec<u8>,
}
impl BoundNativeCheckV1 {
    pub(crate) const fn signed_transaction(&self) -> &SignedTransaction {
        &self.signed
    }
}

/// Freeze exact direct Check bytes, authority, independent network and original round.
pub(crate) fn bind_signed_check_v1(
    round: &mut NativeCheckRoundV1,
    instruction: NativeCustodyCheckRefV1<'_>,
    chain_id: &str,
    network_id: [u8; 32],
    authority: &AccountId,
    floor: NativeCheckFloorV1,
    signed: SignedTransaction,
) -> Result<BoundNativeCheckV1, Error> {
    round.ensure_live()?;
    if round.bound {
        return Err(Error::Invalid);
    }
    round.bound = true;
    floor.validate()?;
    let (purpose, challenge, check_network, minimum_height, minimum_block_hash, check_context_id) =
        instruction.coordinates()?;
    if round.challenge != Some(challenge)
        || challenge == [0; 32]
        || chain_id.is_empty()
        || network_id == [0; 32]
        || check_network != network_id
        || minimum_height != floor.height
        || minimum_block_hash != floor.block_hash
        || check_context_id.is_some_and(|context_id| context_id != floor.context_id)
    {
        return Err(Error::Transaction);
    }
    let Executable::Instructions(instructions) = signed.instructions() else {
        return Err(Error::Transaction);
    };
    if signed.authority() != authority
        || signed.network_id().map(|network| *network.as_bytes()) != Some(network_id)
        || instructions.len() != 1
        || instructions.first() != Some(&instruction.instruction())
    {
        return Err(Error::Transaction);
    }
    let entry = TransactionEntrypoint::External(signed);
    let entry_bytes = native_signed_entry_frame_v1(&entry)?;
    let TransactionEntrypoint::External(signed) = entry else {
        return Err(Error::Transaction);
    };
    round.ensure_live()?;
    Ok(BoundNativeCheckV1 {
        purpose,
        chain_id: chain_id.into(),
        network_id,
        floor,
        started: round.started,
        max_elapsed: round.max_elapsed,
        challenge,
        signed,
        entry_bytes,
    })
}

/// Structural envelope sizing only; no signature, custody, action or fee approval is established.
pub(crate) fn validate_account_transaction_envelope_v1(
    payload: &TransactionPayload,
) -> Result<(), Error> {
    // Bound the borrowed input before cloning it into the canonical builder. The same sole
    // envelope ceiling bounds this necessary payload prefix, without estimating wire overhead.
    if norito::canonical_frame_len(payload).map_err(|_| Error::Transaction)?
        > FINAL_PROMOTION_NATIVE_TRANSACTION_MAX_BYTES_V1
        || payload.attachments.is_some()
        || validate_native_signatory_v1(&payload.authority).is_err()
    {
        return Err(Error::Transaction);
    }
    let builder =
        TransactionBuilder::from_payload(payload.clone()).map_err(|_| Error::Transaction)?;
    // The ordinary builder has no multisig authorization. A single Ed25519 signature is
    // always 64 bytes, so this private, invalid placeholder has its exact canonical layout.
    // Neither this synthetic transaction nor its bytes may escape as signing authority.
    let entry = TransactionEntrypoint::External(
        builder.build_with_signature(Signature::from_bytes(&[0; 64])),
    );
    bounded_entry(&entry).map(|_| ())
}

/// The first-release native profile has one Ed25519 account and no algorithm fallback.
pub(crate) fn validate_native_signatory_v1(authority: &AccountId) -> Result<(), Error> {
    if authority
        .try_signatory()
        .and_then(|key| key.try_algorithm().ok())
        != Some(Algorithm::Ed25519)
    {
        return Err(Error::Transaction);
    }
    Ok(())
}

/// Sole canonical signed native profile. Action, independent scope, spending and current custody
/// remain the caller's owners; no successful structural/signature check grants those authorities.
pub(crate) fn native_signed_entry_frame_v1(
    entry: &TransactionEntrypoint,
) -> Result<Vec<u8>, Error> {
    let TransactionEntrypoint::External(signed) = entry else {
        return Err(Error::Transaction);
    };
    validate_account_transaction_envelope_v1(signed.payload())?;
    if signed.multisig_signatures().is_some() {
        return Err(Error::Transaction);
    }
    let bytes = bounded_entry(entry)?;
    signed.verify_signature().map_err(|_| Error::Transaction)?;
    Ok(bytes)
}

pub(crate) fn bounded_entry(entry: &TransactionEntrypoint) -> Result<Vec<u8>, Error> {
    if norito::canonical_frame_len(entry).map_err(|_| Error::Transaction)?
        > FINAL_PROMOTION_NATIVE_TRANSACTION_MAX_BYTES_V1
    {
        return Err(Error::Transaction);
    }
    norito::encode_canonical(entry).map_err(|_| Error::Transaction)
}

/// Actual same-State view retained until the purpose wrapper finishes eligibility checks.
/// This private execution proof is not itself a fresh-custody success.
pub(crate) struct AuthenticatedCheckExecutionCutV1<'state> {
    view: StateView<'state>,
    check_height: u64,
    applied_floor: NativeCheckFloorV1,
    entry_hash: HashOf<TransactionEntrypoint>,
    canonical_external: Vec<u8>,
    check_block_hash: [u8; 32],
}
impl<'state> AuthenticatedCheckExecutionCutV1<'state> {
    pub(crate) fn view(&self) -> &StateView<'state> {
        &self.view
    }
    pub(crate) const fn check_height(&self) -> u64 {
        self.check_height
    }
    pub(crate) const fn applied_floor(&self) -> NativeCheckFloorV1 {
        self.applied_floor
    }
    pub(crate) const fn entry_hash(&self) -> HashOf<TransactionEntrypoint> {
        self.entry_hash
    }
    /// Move already-authenticated bytes and H coordinates while releasing the applied State view.
    /// This retains verification output only; it runs no new proof or eligibility policy.
    pub(crate) fn into_verified_entry(self) -> (Vec<u8>, [u8; 32]) {
        (self.canonical_external, self.check_block_hash)
    }
}

/// Consume exact application against actual State/Kura and independently pinned floor continuity.
pub(crate) fn authenticate_applied_check_v1<'state>(
    state: &'state Arc<State>,
    purpose: NativeCustodyCheckPurposeV1,
    bound: BoundNativeCheckV1,
    round: &NativeCheckRoundV1,
) -> Result<AuthenticatedCheckExecutionCutV1<'state>, Error> {
    round.ensure_live()?;
    if !round.bound
        || bound.purpose != purpose
        || bound.started != round.started
        || bound.max_elapsed != round.max_elapsed
        || Some(bound.challenge) != round.challenge
    {
        return Err(Error::Invalid);
    }
    let view = state.view();
    if view.network_id().as_bytes() != &bound.network_id
        || view.chain_id().to_string() != bound.chain_id
    {
        return Err(Error::Finality);
    }
    let entry_hash = bound.signed.hash_as_entrypoint();
    let height_index = view
        .transactions
        .get(&entry_hash)
        .ok_or(Error::NotApplied)?;
    let check_height = u64::try_from(height_index.get()).map_err(|_| Error::NotApplied)?;
    let applied_height = u64::try_from(view.block_hashes().len()).map_err(|_| Error::NotApplied)?;
    if check_height <= bound.floor.height || check_height > applied_height {
        return Err(Error::NotApplied);
    }
    check_history_span_v1(bound.floor.height, applied_height)?;
    let chain = CertifiedChain::new(&view).map_err(|_| Error::Finality)?;
    let mut check_block_hash = None;
    let mut applied_floor = bound.floor;
    // Each block is the view's, certified by its local `CommitQC`, and extends the previous one.
    for block in chain.walk(bound.floor.height, applied_height) {
        round.ensure_live()?;
        let block = block.map_err(|_| Error::Finality)?;
        let height = block.height();
        let hash = *block.block_hash().as_ref();
        if height == bound.floor.height
            && (hash != bound.floor.block_hash || block.id() != bound.floor.context_id)
        {
            return Err(Error::Finality);
        }
        if height == check_height {
            // Verify execution against this same certified lineage body: the anchor binds the
            // executed wire the certified result commits.
            let anchor = block
                .entry_anchor(&entry_hash)
                .map_err(|_| Error::Execution)?;
            let body = block.block();
            let proofs = body
                .network_execution_proof(&entry_hash)
                .ok_or(Error::Execution)?;
            if !proofs.verify(&anchor) {
                return Err(Error::Execution);
            }
            let entry_index =
                usize::try_from(anchor.entry_index()).map_err(|_| Error::Execution)?;
            let actual = body
                .network_entrypoint_at(entry_index)
                .ok_or(Error::Execution)?;
            let (_, output) = body
                .network_output_at(anchor.entry_index())
                .ok_or(Error::Execution)?;
            if bounded_entry(actual).map_err(|_| Error::Execution)? != bound.entry_bytes
                || !output.result.is_ok()
            {
                return Err(Error::Execution);
            }
            check_block_hash = Some(hash);
        }
        applied_floor = NativeCheckFloorV1 {
            height,
            block_hash: hash,
            context_id: block.id(),
        };
    }
    round.ensure_live()?;
    let check_block_hash = check_block_hash.ok_or(Error::Execution)?;
    Ok(AuthenticatedCheckExecutionCutV1 {
        view,
        check_height,
        applied_floor,
        entry_hash,
        canonical_external: bound.entry_bytes,
        check_block_hash,
    })
}

#[cfg(any(test, feature = "iroha-core-tests"))]
pub(crate) mod fixture;

#[cfg(test)]
mod tests;
