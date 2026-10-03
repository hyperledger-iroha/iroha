//! Fixed component geometry; no private opcode changes the padded packet length.

/// Private packet slots retained by this native component, including all padding.
pub const PACKET_SLOTS: usize = 16_384;
/// Maximum executed instructions, including the root return, in this component.
pub const MAX_STEPS: usize = 64;
/// Every return owns the complete ABI maximum scan, even for Unit.
pub const RETURN_CELLS: usize = 4_097;
/// Fixed initial-state window preceding all instruction packets.
pub const ROOT_SLOTS: usize = 64;
/// All fixed instruction windows: compact steps followed by the root return.
pub const INSTRUCTION_WINDOWS: usize = MAX_STEPS + 1;
pub(crate) const STEP_SLOTS: usize = 32;
pub(crate) const RETURN_FIRST: usize = ROOT_SLOTS + MAX_STEPS * STEP_SLOTS;
pub(crate) const SCAN_OFFSET: usize = 100;
pub(crate) const RETURN_SLOTS: usize = SCAN_OFFSET + 2 * RETURN_CELLS + 3;
pub(crate) const PADDING_FIRST: usize = RETURN_FIRST + RETURN_SLOTS;
const _: () = assert!(PADDING_FIRST + 2 <= PACKET_SLOTS);

/// Original dispatcher-port offsets. Memory and typed work occupy explicit gaps.
pub(crate) const COMPACT_DISPATCH: [usize; 21] = [
    0, 1, 2, 3, 4, 5, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 24, 25, 26,
];
/// Same return window positions as the callable relation's original producers.
pub(crate) const RETURN_DISPATCH: [usize; 21] = [
    0,
    1,
    2,
    3,
    4,
    5,
    30,
    31,
    32,
    45,
    46,
    47,
    48,
    49,
    50,
    51,
    52,
    53,
    SCAN_OFFSET + 2 * RETURN_CELLS,
    SCAN_OFFSET + 2 * RETURN_CELLS + 1,
    SCAN_OFFSET + 2 * RETURN_CELLS + 2,
];

/// Absolute original dispatcher-port clocks for one mandatory instruction window.
///
/// Every component evaluates all 64 compact windows and the fixed root return,
/// including wholly inactive windows. This shape-only accessor never derives
/// clocks from private execution length or accepts caller-selected addresses.
pub fn instruction_clocks(window: usize) -> Option<[u32; 21]> {
    let (first, offsets) = match window {
        0..MAX_STEPS => (ROOT_SLOTS + window * STEP_SLOTS, COMPACT_DISPATCH),
        MAX_STEPS => (RETURN_FIRST, RETURN_DISPATCH),
        _ => return None,
    };
    Some(offsets.map(|offset| (first + offset) as u32))
}
