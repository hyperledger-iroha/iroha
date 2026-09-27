//! Refund flow for outbound messages that did not mint (spec §7.3).
//!
//! Voids the unconsumed nonce on the destination and submits the void proof to Taira.

// TODO(ws51): implement the journaled refund flow.
