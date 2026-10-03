//! Move-only packet backing, funded before execution and scrubbed before refund.

use super::{CaptureError, PacketSpace, schedule::PACKET_SLOTS};
use crate::execution_memory::{ExecutionBuffer, ExecutionMemoryLease, ExecutionMemoryPlan};

/// One original native state access. No public constructor accepts claimed values.
///
/// Disabled slots are entirely zero. Payloads are canonical little-endian bytes;
/// conversion to field limbs belongs to the eventual constrained consumer.
#[repr(C)]
pub struct NativePacket {
    pub(crate) before: [u8; 16],
    pub(crate) after: [u8; 16],
    pub(crate) index: u32,
    pub(crate) clock: u32,
    pub(crate) generation: u16,
    pub(crate) before_private: u16,
    pub(crate) after_private: u16,
    pub(crate) space: u8,
    pub(crate) write: bool,
}
// Every backing byte belongs to one explicitly erased field; there is no
// uninitialized struct padding outside the confidential-discard path.
const _: () = assert!(std::mem::size_of::<NativePacket>() == 48);
impl NativePacket {
    pub(crate) fn zero() -> Self {
        Self {
            space: 0,
            generation: 0,
            index: 0,
            clock: 0,
            write: false,
            before: [0; 16],
            after: [0; 16],
            before_private: 0,
            after_private: 0,
        }
    }
    /// Whether this fixed slot contains a native access.
    pub fn enabled(&self) -> bool {
        self.space != 0
    }
    /// Disjoint native state class; padding has no class.
    pub fn space(&self) -> Option<PacketSpace> {
        match self.space {
            1 => Some(PacketSpace::Memory),
            2 => Some(PacketSpace::Register),
            3 => Some(PacketSpace::Initialization),
            4 => Some(PacketSpace::Owner),
            _ => None,
        }
    }
    /// Frame generation; registers and physical memory use zero.
    pub fn generation(&self) -> u16 {
        self.generation
    }
    /// Cell index within its class and generation.
    pub fn index(&self) -> u32 {
        self.index
    }
    /// Absolute slot in this single native invocation's schedule.
    pub fn clock(&self) -> u32 {
        self.clock
    }
    /// Whether this access may replace its state cell.
    pub fn is_write(&self) -> bool {
        self.write
    }
    /// Original pre-access bytes, borrowed from their retained owner.
    pub fn before(&self) -> &[u8; 16] {
        &self.before
    }
    /// Original post-access bytes, borrowed from their retained owner.
    pub fn after(&self) -> &[u8; 16] {
        &self.after
    }
    /// Original private-byte mask before access.
    pub fn before_private(&self) -> u16 {
        self.before_private
    }
    /// Original private-byte mask after access.
    pub fn after_private(&self) -> u16 {
        self.after_private
    }
    pub(crate) fn clear(&mut self) {
        iroha_crypto::zeroize_value_for_confidential_discard(&mut self.before);
        iroha_crypto::zeroize_value_for_confidential_discard(&mut self.after);
        iroha_crypto::zeroize_value_for_confidential_discard(&mut self.before_private);
        iroha_crypto::zeroize_value_for_confidential_discard(&mut self.after_private);
        iroha_crypto::zeroize_value_for_confidential_discard(&mut self.space);
        iroha_crypto::zeroize_value_for_confidential_discard(&mut self.generation);
        iroha_crypto::zeroize_value_for_confidential_discard(&mut self.index);
        iroha_crypto::zeroize_value_for_confidential_discard(&mut self.clock);
        iroha_crypto::zeroize_value_for_confidential_discard(&mut self.write);
    }
}
impl Drop for NativePacket {
    fn drop(&mut self) {
        self.clear();
    }
}

pub(crate) struct Storage(ExecutionBuffer<NativePacket>);
impl Storage {
    pub(crate) fn plan() -> Result<ExecutionMemoryPlan, CaptureError> {
        ExecutionMemoryPlan::array::<NativePacket>(PACKET_SLOTS).map_err(CaptureError::Plan)
    }
    pub(crate) fn new(parent: &mut ExecutionMemoryLease) -> Result<Self, CaptureError> {
        let mut lease = parent
            .partition(Self::plan()?)
            .map_err(CaptureError::Reservation)?;
        let mut backing =
            ExecutionBuffer::new(PACKET_SLOTS, &mut lease).map_err(CaptureError::Allocation)?;
        for _ in 0..PACKET_SLOTS {
            backing.push_reserved(NativePacket::zero());
        }
        Ok(Self(backing))
    }
    pub(crate) fn packets(&self) -> &[NativePacket] {
        self.0.as_slice()
    }
    pub(crate) fn put(&mut self, clock: usize, packet: NativePacket) {
        let slot = &mut self.0.as_mut_slice()[clock];
        assert!(
            !slot.enabled(),
            "native packet slot is committed exactly once"
        );
        *slot = packet;
    }
}
