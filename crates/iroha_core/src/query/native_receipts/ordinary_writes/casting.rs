//! Allocation-free canonical casting leaves and their exact retained sequence backing.
//!
//! Every leaf field is fixed-size, including its optional compact release record. The scanner
//! reads the current model's field/sequence layout directly; it never creates decoder alignment
//! buffers, inferred-layout fallbacks or a second encoded frame. Whole-projection streamed
//! comparison in the parent keeps this materializer tied to the canonical model encoder.

use super::{WriteDecodeError, count, field, fields};
use iroha_crypto::Hash;
use iroha_data_model::{
    parliament_casting::{
        MAX_PARLIAMENT_CONCURRENT_CASTING_CONTEXTS_V1,
        ParliamentTimedOvnCastingContextBindingV1 as Binding,
        ParliamentTimedOvnCastingPhaseV1 as Phase,
        ParliamentTimedOvnRegistrationCorpusCommitmentV1 as Corpus,
        ParliamentTimedOvnReleaseBindingV1 as Release,
    },
    parliament_types::{
        BallotAttemptId, BodyInstanceId, GovernanceAttemptId, ProposalContentId, TleKeySessionId,
    },
};
use mv::allocation::{AllocationBudget, AllocationCharge, AllocationReservation, ChargedBuffer};
use norito::Error;

/// Immutable backing owner; each leaf has no heap-bearing fields. Field order frees values
/// before refunding the one original exact array charge. There is no cloning or extraction API.
pub(crate) struct CastingOwner {
    values: Vec<Binding>,
    charge: AllocationCharge,
}
impl CastingOwner {
    /// Borrow the original ordered compact leaves without duplicating their backing.
    pub(crate) fn as_slice(&self) -> &[Binding] {
        &self.values
    }
    /// Verify the exact original allocation pool; equal limits do not identify an owner.
    pub(crate) fn belongs_to(&self, budget: &AllocationBudget) -> bool {
        self.charge.belongs_to(budget)
    }
}

#[derive(Clone, Copy)]
pub(super) struct RawCasting<'a> {
    count: usize,
    bytes: &'a [u8],
    flags: u8,
}
impl<'a> RawCasting<'a> {
    pub(super) fn new(bytes: &'a [u8], flags: u8) -> Result<Self, Error> {
        norito::core::validate_header_flags(flags)?;
        let (count, bytes) = count(bytes)?;
        if count > MAX_PARLIAMENT_CONCURRENT_CASTING_CONTEXTS_V1 as usize || count > bytes.len() {
            return Err(Error::LengthMismatch);
        }
        let source = Self {
            count,
            bytes,
            flags,
        };
        source.visit(|_| Ok(()))?;
        Ok(source)
    }
    pub(super) fn count(&self) -> usize {
        self.count
    }
    fn visit(&self, mut callback: impl FnMut(Binding) -> Result<(), Error>) -> Result<(), Error> {
        let mut bytes = self.bytes;
        for _ in 0..self.count {
            callback(binding(field(&mut bytes, self.flags)?, self.flags)?)?;
        }
        if !bytes.is_empty() {
            return Err(Error::LengthMismatch);
        }
        Ok(())
    }
    pub(super) fn materialize(
        &self,
        reservation: &mut AllocationReservation,
    ) -> Result<ChargedBuffer<Binding>, WriteDecodeError> {
        let mut output = ChargedBuffer::from_reservation(self.count, reservation)?;
        self.visit(|value| output.try_push(value).map_err(|_| Error::LengthMismatch))?;
        Ok(output)
    }
}

/// Bind the originally allocated complete vector without reallocating or copying it.
/// Its charge stays separate only across the parent's guarded typed-frame construction.
#[allow(
    unsafe_code,
    reason = "exact compact leaf array and original charge move together into immutable custody"
)]
pub(super) fn split(buffer: ChargedBuffer<Binding>) -> (Vec<Binding>, AllocationCharge) {
    // SAFETY: only the parent's decoder calls this; it immediately retains the charge outside
    // the projection and destroys every partial projection before that charge on failure.
    unsafe { buffer.into_allocation_parts() }
}
pub(super) fn retain(
    values: Vec<Binding>,
    charge: AllocationCharge,
    budget: &AllocationBudget,
) -> Result<CastingOwner, WriteDecodeError> {
    let expected = super::array::<Binding>(values.capacity())?;
    if !charge.belongs_to(budget) || charge.layout() != expected {
        drop(values);
        drop(charge);
        return Err(WriteDecodeError::ForeignPool);
    }
    Ok(CastingOwner { values, charge })
}

fn fixed<const N: usize>(bytes: &[u8]) -> Result<[u8; N], Error> {
    bytes.try_into().map_err(|_| Error::LengthMismatch)
}
// Norito arrays encode each element with its canonical field-length prefix. They are not
// the special raw Vec<u8> sequence layout, and governance IDs retain this nested array form.
fn byte_array<const N: usize>(bytes: &[u8], flags: u8) -> Result<[u8; N], Error> {
    let encoded = fields::<N>(bytes, flags)?;
    let mut output = [0; N];
    for (byte, encoded) in output.iter_mut().zip(encoded) {
        *byte = fixed::<1>(encoded)?[0];
    }
    Ok(output)
}
fn u16_value(bytes: &[u8]) -> Result<u16, Error> {
    fixed(bytes).map(u16::from_le_bytes)
}
fn u32_value(bytes: &[u8]) -> Result<u32, Error> {
    fixed(bytes).map(u32::from_le_bytes)
}
fn u64_value(bytes: &[u8]) -> Result<u64, Error> {
    fixed(bytes).map(u64::from_le_bytes)
}
fn id(bytes: &[u8], flags: u8) -> Result<[u8; 32], Error> {
    let [version, length, value] = fields::<3>(bytes, flags)?;
    if u16_value(version)? != 1 || u16_value(length)? != 32 {
        return Err(Error::LengthMismatch);
    }
    byte_array(value, flags)
}
fn optional<T>(
    bytes: &[u8],
    flags: u8,
    decode: impl FnOnce(&[u8]) -> Result<T, Error>,
) -> Result<Option<T>, Error> {
    match bytes.split_first() {
        Some((0, [])) => Ok(None),
        Some((1, rest)) => {
            let [value] = fields::<1>(rest, flags)?;
            decode(value).map(Some)
        }
        _ => Err(Error::LengthMismatch),
    }
}
fn corpus(bytes: &[u8], flags: u8) -> Result<Corpus, Error> {
    let [version, count, digest] = fields::<3>(bytes, flags)?;
    Ok(Corpus {
        version: u16_value(version)?,
        record_count: u32_value(count)?,
        digest: Hash::from_marked_bytes(fixed(digest)?).ok_or(Error::LengthMismatch)?,
    })
}
fn release(bytes: &[u8], flags: u8) -> Result<Release, Error> {
    let [
        key,
        governance,
        body,
        ballot,
        survivor,
        recovery,
        target,
        parameter,
    ] = fields::<8>(bytes, flags)?;
    Ok(Release {
        tle_key_session_id: TleKeySessionId::new(id(key, flags)?),
        governance_attempt_id: GovernanceAttemptId::new(id(governance, flags)?),
        body_instance_id: BodyInstanceId::new(id(body, flags)?),
        ballot_attempt_id: BallotAttemptId::new(id(ballot, flags)?),
        survivor_corpus_root: byte_array(survivor, flags)?,
        no_recovery_root: byte_array(recovery, flags)?,
        target_finalized_height: u64_value(target)?,
        parameter_hash: byte_array(parameter, flags)?,
    })
}
fn binding(bytes: &[u8], flags: u8) -> Result<Binding, Error> {
    let [
        version,
        evaluated,
        phase,
        network,
        proposal,
        governance,
        body,
        ballot,
        parameter,
        key,
        transcript,
        public_key,
        opened,
        closed,
        frozen,
        committed,
        target,
        registration,
        survivors,
        dropout,
        future_release,
    ] = fields::<21>(bytes, flags)?;
    let phase = match u32_value(phase)? {
        0 => Phase::Registered,
        1 => Phase::RegistrationClosed,
        2 => Phase::SurvivorsFrozen,
        _ => return Err(Error::LengthMismatch),
    };
    Ok(Binding {
        version: u16_value(version)?,
        evaluated_height: u64_value(evaluated)?,
        phase,
        network_id: byte_array(network, flags)?,
        proposal_content_id: ProposalContentId::new(id(proposal, flags)?),
        governance_attempt_id: GovernanceAttemptId::new(id(governance, flags)?),
        body_instance_id: BodyInstanceId::new(id(body, flags)?),
        ballot_attempt_id: BallotAttemptId::new(id(ballot, flags)?),
        parameter_hash: byte_array(parameter, flags)?,
        tle_key_session_id: TleKeySessionId::new(id(key, flags)?),
        tle_key_transcript_hash: byte_array(transcript, flags)?,
        tle_master_public_key: byte_array(public_key, flags)?,
        registration_opened_at_finalized_height: u64_value(opened)?,
        registration_close_height: u64_value(closed)?,
        survivor_freeze_height: u64_value(frozen)?,
        commitment_close_height: u64_value(committed)?,
        target_finalized_height: u64_value(target)?,
        registration_corpus: corpus(registration, flags)?,
        survivor_count: optional(survivors, flags, u32_value)?,
        dropout_root: optional(dropout, flags, |bytes| byte_array(bytes, flags))?,
        release_identity: optional(future_release, flags, |bytes| release(bytes, flags))?,
    })
}
