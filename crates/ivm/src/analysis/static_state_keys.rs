//! Exact original-pool custody for symbolic state keys and immutable output.

use super::{StaticNoritoKey, StaticStatePath, static_state_literals::LiteralSource};
use crate::{
    VMError,
    cache_memory::MemoryReservation,
    error::ExecutionDeferral,
    execution_memory::{ExecutionBuffer, ExecutionMemoryLease, ExecutionMemoryPlan},
    instruction::wide,
    ivm_cache::DecodedOp,
};
use iroha_allocation::{AllocationBudget, AllocationRefusal, ChargedBuffer, ChargedShared};
use std::{alloc::Layout, cmp::Ordering, fmt};

/// Sorted, distinct scheduler keys retaining their original exact allocation.
///
/// Cloning shares the byte arena and indexes without copying keys. The original
/// pool remains charged until the final read or write view disappears. An empty
/// result allocates no shared shell.
#[derive(Clone, Default)]
pub struct StaticStateKeys {
    owner: Option<ChargedShared<KeyStorage>>,
    start: usize,
    count: usize,
}
struct KeyRange {
    start: usize,
    end: usize,
}
struct KeyStorage {
    ranges: ChargedBuffer<KeyRange>,
    bytes: ChargedBuffer<u8>,
    // Both physical buffers are released before their aggregate retention charge.
    _retention: MemoryReservation,
}
impl StaticStateKeys {
    /// Number of distinct keys in this view.
    pub fn len(&self) -> usize {
        self.count
    }
    /// Whether this view contains no keys.
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
    /// Borrow keys in their original byte-wise lexical order.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &str> + '_ {
        let (ranges, bytes) = match self.owner.as_ref() {
            Some(owner) => (
                &owner.ranges.as_slice()[self.start..self.start + self.count],
                owner.bytes.as_slice(),
            ),
            None => (&[][..], &[][..]),
        };
        ranges.iter().map(move |range| {
            std::str::from_utf8(&bytes[range.start..range.end])
                .expect("state keys contain validated text and ASCII framing")
        })
    }
    /// Test an exact key spelling without allocating.
    pub fn contains(&self, key: &str) -> bool {
        let Some(owner) = self.owner.as_ref() else {
            return false;
        };
        owner.ranges.as_slice()[self.start..self.start + self.count]
            .binary_search_by(|range| {
                owner.bytes.as_slice()[range.start..range.end].cmp(key.as_bytes())
            })
            .is_ok()
    }
}
impl fmt::Debug for StaticStateKeys {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.debug_set().entries(self.iter()).finish()
    }
}
impl PartialEq for StaticStateKeys {
    fn eq(&self, other: &Self) -> bool {
        self.iter().eq(other.iter())
    }
}
impl Eq for StaticStateKeys {}

#[derive(Clone, Copy)]
pub(super) struct Descriptor {
    path: StaticStatePath,
    write: bool,
    wildcard: bool,
}
impl Descriptor {
    pub(super) fn new(
        path: StaticStatePath,
        number: u32,
        literals: &impl LiteralSource,
    ) -> Option<Self> {
        let descriptor = Self {
            path,
            write: crate::syscalls::syscall_access(number)
                == crate::syscalls::SyscallAccess::StateWrite,
            wildcard: matches!(
                number,
                crate::syscalls::SYSCALL_STATE_COUNT | crate::syscalls::SYSCALL_STATE_SCAN
            ),
        };
        descriptor.parts(literals)?;
        Some(descriptor)
    }
    fn parts<'a>(&self, literals: &'a impl LiteralSource) -> Option<Parts<'a>> {
        let (text, hex) = match self.path {
            StaticStatePath::Literal(index) => (literals.path(usize::from(index))?, None),
            StaticStatePath::FromName(index) => (literals.name(usize::from(index))?, None),
            StaticStatePath::MapChild { base, key } => {
                let bytes = match key {
                    StaticNoritoKey::PointerEnvelope(index) => {
                        literals.envelope(usize::from(index))?
                    }
                    StaticNoritoKey::LiteralPayload(index) => {
                        literals.payload(usize::from(index))?
                    }
                };
                (literals.name(usize::from(base))?, Some(bytes))
            }
        };
        // Nested wildcards do not conflict with the scheduler's concrete keys.
        if self.wildcard && (text.contains('/') || hex.is_some()) {
            return None;
        }
        Some(Parts {
            text,
            hex,
            wildcard: self.wildcard,
        })
    }
    fn compare(&self, other: &Self, literals: &impl LiteralSource) -> Ordering {
        self.write.cmp(&other.write).then_with(|| {
            self.parts(literals)
                .expect("unchanged admitted literal evidence")
                .bytes()
                .cmp(
                    other
                        .parts(literals)
                        .expect("unchanged admitted literal evidence")
                        .bytes(),
                )
        })
    }
}
#[derive(Clone, Copy)]
struct Parts<'a> {
    text: &'a str,
    hex: Option<&'a [u8]>,
    wildcard: bool,
}
impl Parts<'_> {
    fn len(&self) -> Result<usize, VMError> {
        let hex = self.hex.map_or(Some(0), |bytes| {
            bytes
                .len()
                .checked_mul(2)
                .and_then(|len| len.checked_add(1))
        });
        6_usize
            .checked_add(self.text.len())
            .and_then(|len| len.checked_add(hex?))
            .and_then(|len| len.checked_add(if self.wildcard { 3 } else { 0 }))
            .ok_or(VMError::AllocationDeferred(
                AllocationRefusal::DemandOverflow,
            ))
    }
    fn bytes(self) -> impl Iterator<Item = u8> {
        let hex = self.hex.into_iter().flat_map(|bytes| {
            std::iter::once(b'/').chain(bytes.iter().flat_map(|byte| {
                let digits = b"0123456789abcdef";
                [
                    digits[usize::from(byte >> 4)],
                    digits[usize::from(byte & 15)],
                ]
            }))
        });
        b"state:"
            .iter()
            .copied()
            .chain(self.text.bytes())
            .chain(hex)
            .chain(
                if self.wildcard { &b"[*]"[..] } else { &[][..] }
                    .iter()
                    .copied(),
            )
    }
}
#[derive(Clone, Copy)]
struct Site {
    pc: u64,
    key: Option<Descriptor>,
}

/// One fixed slot per state syscall site, before any literal decoding occurs.
/// A flat dataflow fact can only lose a known value after its first visit; keep
/// that first key even when a later ambiguous merge makes the proof incomplete.
pub(super) struct KeyScratch {
    sites: ExecutionBuffer<Site>,
}
impl KeyScratch {
    pub(super) fn new(decoded: &[DecodedOp], budget: &AllocationBudget) -> Result<Self, VMError> {
        let sites = decoded.iter().filter(|op| state_syscall(op).is_some());
        let count = sites.clone().count();
        let plan =
            ExecutionMemoryPlan::array::<Site>(count).map_err(VMError::AllocationDeferred)?;
        let mut lease =
            ExecutionMemoryLease::reserve(budget, plan).map_err(VMError::AllocationDeferred)?;
        let mut storage = ExecutionBuffer::new(count, &mut lease)
            .map_err(|_| VMError::ExecutionDeferred(ExecutionDeferral::AllocationUnavailable))?;
        for op in sites {
            storage.push_reserved(Site {
                pc: op.pc,
                key: None,
            });
        }
        Ok(Self { sites: storage })
    }
    pub(super) fn record(
        &mut self,
        pc: u64,
        key: Descriptor,
        literals: &impl LiteralSource,
    ) -> Result<(), VMError> {
        key.parts(literals).ok_or(VMError::DecodeError)?.len()?;
        let index = self
            .sites
            .as_slice()
            .binary_search_by_key(&pc, |site| site.pc)
            .map_err(|_| VMError::DecodeError)?;
        let site = &mut self.sites.as_mut_slice()[index];
        match site.key {
            Some(original) if original.compare(&key, literals) != Ordering::Equal => {
                Err(VMError::DecodeError)
            }
            Some(_) => Ok(()),
            None => {
                site.key = Some(key);
                Ok(())
            }
        }
    }
    pub(super) fn finish(
        mut self,
        literals: &impl LiteralSource,
        budget: &AllocationBudget,
    ) -> Result<(StaticStateKeys, StaticStateKeys), VMError> {
        self.sites
            .as_mut_slice()
            .sort_unstable_by(|left, right| match (left.key, right.key) {
                (Some(left), Some(right)) => left.compare(&right, literals),
                (Some(_), None) => Ordering::Less,
                (None, Some(_)) => Ordering::Greater,
                (None, None) => Ordering::Equal,
            });
        let unique = || {
            let mut previous: Option<Descriptor> = None;
            self.sites.as_slice().iter().filter_map(move |site| {
                let key = site.key?;
                let duplicate = previous
                    .is_some_and(|previous| previous.compare(&key, literals) == Ordering::Equal);
                previous = Some(key);
                (!duplicate).then_some(key)
            })
        };
        let mut count = 0_usize;
        let mut reads = 0_usize;
        let mut bytes = 0_usize;
        for key in unique() {
            count += 1;
            reads += usize::from(!key.write);
            bytes = bytes
                .checked_add(key.parts(literals).ok_or(VMError::DecodeError)?.len()?)
                .ok_or(VMError::AllocationDeferred(
                    AllocationRefusal::DemandOverflow,
                ))?;
        }
        if count == 0 {
            return Ok((StaticStateKeys::default(), StaticStateKeys::default()));
        }
        let ranges_layout = Layout::array::<KeyRange>(count)
            .map_err(|_| VMError::AllocationDeferred(AllocationRefusal::DemandOverflow))?;
        let bytes_layout = Layout::array::<u8>(bytes)
            .map_err(|_| VMError::AllocationDeferred(AllocationRefusal::DemandOverflow))?;
        let total = ranges_layout
            .size()
            .checked_add(bytes_layout.size())
            .and_then(|bytes| {
                bytes.checked_add(ChargedShared::<KeyStorage>::allocation_layout().size())
            })
            .ok_or(VMError::AllocationDeferred(
                AllocationRefusal::DemandOverflow,
            ))?;
        // All destinations and the shared shell are admitted together while
        // symbolic scratch and its borrowed literal evidence are still alive.
        let mut admission = budget
            .try_reserve_bytes(total)
            .map_err(VMError::AllocationDeferred)?;
        let retention = MemoryReservation::active(total);
        let unavailable = |_| VMError::ExecutionDeferred(ExecutionDeferral::AllocationUnavailable);
        #[cfg(test)]
        allocation_gate(1)?;
        let shell =
            ChargedShared::<KeyStorage>::reserve_from(&mut admission).map_err(unavailable)?;
        #[cfg(test)]
        allocation_gate(2)?;
        let mut ranges = ChargedBuffer::from_reservation(count, &mut admission)
            .map_err(|_| VMError::ExecutionDeferred(ExecutionDeferral::AllocationUnavailable))?;
        #[cfg(test)]
        allocation_gate(3)?;
        let mut output = ChargedBuffer::from_reservation(bytes, &mut admission)
            .map_err(|_| VMError::ExecutionDeferred(ExecutionDeferral::AllocationUnavailable))?;
        #[cfg(test)]
        allocation_gate(4)?;
        for key in unique() {
            let start = output.as_slice().len();
            for byte in key
                .parts(literals)
                .expect("unchanged literal evidence")
                .bytes()
            {
                output.push_reserved(byte);
            }
            ranges.push_reserved(KeyRange {
                start,
                end: output.as_slice().len(),
            });
        }
        debug_assert_eq!(output.as_slice().len(), bytes);
        debug_assert_eq!(admission.remaining_bytes(), 0);
        let owner = shell.initialize(KeyStorage {
            ranges,
            bytes: output,
            _retention: retention,
        });
        let read_keys = if reads == 0 {
            StaticStateKeys::default()
        } else {
            StaticStateKeys {
                owner: Some(owner.clone()),
                start: 0,
                count: reads,
            }
        };
        let write_keys = if reads == count {
            StaticStateKeys::default()
        } else {
            StaticStateKeys {
                owner: Some(owner.clone()),
                start: reads,
                count: count - reads,
            }
        };
        Ok((read_keys, write_keys))
    }
}
fn state_syscall(op: &DecodedOp) -> Option<u32> {
    let number = match wide::opcode(op.inst) {
        wide::system::SCALL => u32::from(wide::imm8(op.inst) as u8),
        wide::system::SYSTEM => crate::encoding::wide::decode_syscallx(op.inst),
        _ => return None,
    };
    matches!(
        crate::syscalls::syscall_access(number),
        crate::syscalls::SyscallAccess::StateRead | crate::syscalls::SyscallAccess::StateWrite
    )
    .then_some(number)
}

#[cfg(test)]
thread_local! {
    static OUTPUT_FAULT: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
}
#[cfg(test)]
fn allocation_gate(stage: u8) -> Result<(), VMError> {
    if OUTPUT_FAULT.get() == stage {
        OUTPUT_FAULT.set(0);
        assert_ne!(stage, 4, "output construction interrupted");
        return Err(VMError::ExecutionDeferred(
            ExecutionDeferral::AllocationUnavailable,
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
