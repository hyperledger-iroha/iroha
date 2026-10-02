//! Protocol-derived ordinary service/receiver carrier budgets before Native reserves W2.
//!
//! These finite byte counts reserve physical storage; they grant no FI, clock, DATA, proof or
//! Native owner. The service carrier retains three full clock originals. The compact receiver
//! carrier retains one full clock and the actual governed DATA receipt, in addition to Wrapper.
use super::{
    DigestV1, KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1, KAGEMUSHA_PAIRED_PROOF_MAX_BYTES_V1,
    KagemushaAuthenticatedRecursiveVerifierV1, KagemushaPairedProofV1,
    deferred_parent::ordinary_ipa_proof_profile_v1,
    ordinary_cash_terminal_verifier::{
        ORDINARY_INNER_TERMINAL_MAX_BYTES_V1, OrdinaryCashProofPairWireV1,
    },
    ordinary_guard_verifier::OrdinaryGuardProofWireV1,
};
use crate::kagemusha_v1_state::{
    KAGEMUSHA_ORDINARY_NATIVE_SIGNED_CLOCK_ORIGINAL_MAX_BYTES_V1,
    KAGEMUSHA_ORDINARY_RECEIVED_COMMIT_ORIGINAL_MAX_BYTES_V1,
};
use iroha_data_model::kagemusha::{
    KAGEMUSHA_APP_OPERATION_APPROVAL_MAX_BYTES_V1, KAGEMUSHA_ORDINARY_APP_ENROLLMENT_MAX_BYTES_V1,
    KAGEMUSHA_ORDINARY_CASH_RECORD_MAX_BYTES_V1, KAGEMUSHA_ORDINARY_CURRENT_CONTROL_MAX_BYTES_V1,
    KAGEMUSHA_ORDINARY_RETAIL_ENROLLMENT_MAX_BYTES_V1,
    KAGEMUSHA_OUTBOX_RETRY_METADATA_MAX_BYTES_V1, KAGEMUSHA_RECOVERY_SEEDS_MAX_BYTES_V1,
    KAGEMUSHA_RELEASE_MANIFEST_MAX_BYTES_V1, KAGEMUSHA_SEALED_TRANSITION_INPUTS_MAX_BYTES_V1,
};
/// Same existing Native four-original clock frame maximum, with explicit outer framing room.
const CLOCK_ORIGINAL_MAX: usize = KAGEMUSHA_ORDINARY_NATIVE_SIGNED_CLOCK_ORIGINAL_MAX_BYTES_V1;
/// Sole independently authenticated received Commit envelope: signature, exact DATA row,
/// full historical finality proof and canonical framing. This is not a World snapshot.
const DATA_RECEIPT_MAX: usize = KAGEMUSHA_ORDINARY_RECEIVED_COMMIT_ORIGINAL_MAX_BYTES_V1;
/// Whole ordinary State/Guard/pre-W2 original bundle, including the full output/manifest.
const STATE_BUNDLE_MAX: usize = 256 * 1024;
const PREDECESSOR_STATE_MAX: usize = 32 * 1024;
/// Fixed outer record/schema/vector overhead allowance, separate from JSON transport encoding.
const OUTER_FRAME_ROOM: usize = 64 * 1024;

/// Closed budget derived only from the actually loaded authenticated compiled protocols.
/// A public decoded integer/profile cannot construct this value or reserve a Native slot.
#[derive(Clone, Copy)]
pub struct KagemushaOrdinaryCashCarrierBudgetV1 {
    release: DigestV1,
    profiles: [[usize; 2]; 4],
    state_pair_bytes: usize,
    guard_pair_bytes: usize,
    terminal_pair_bytes: usize,
    wrapper_pair_bytes: usize,
    private_service_max: u32,
    compact_receiver_max: u32,
    required_slot_bytes: u32,
}
impl KagemushaOrdinaryCashCarrierBudgetV1 {
    /// Exact authenticated release used to derive these protocol lengths.
    pub fn release_id(&self) -> DigestV1 {
        self.release
    }
    /// Exact per-parity transcript lengths: State, ordinary Guard, inner Terminal, then Wrapper.
    pub fn protocol_proof_lengths(&self) -> [[usize; 2]; 4] {
        self.profiles
    }
    /// Exact canonical paired proof-frame sizes at the loaded lengths, in the same four-role order.
    pub fn proof_frame_lengths(&self) -> [usize; 4] {
        [
            self.state_pair_bytes,
            self.guard_pair_bytes,
            self.terminal_pair_bytes,
            self.wrapper_pair_bytes,
        ]
    }
    /// Conservative complete private service frame, including up to three full clock originals.
    pub fn private_service_max_bytes(&self) -> u32 {
        self.private_service_max
    }
    /// Conservative self-contained receiver frame: Wrapper, one full clock, FI and genuine receipt.
    pub fn compact_receiver_max_bytes(&self) -> u32 {
        self.compact_receiver_max
    }
    /// Physical reservation needed for both separately retained carriers, recovery streams and retry.
    /// Exact-original deduplication may reduce actual occupied bytes; it cannot increase this bound.
    pub fn required_outbox_slot_bytes(&self) -> u32 {
        self.required_slot_bytes
    }
    /// Check the actual pre-W2 Native-held reserved byte count. This data check lends no authority.
    /// # Errors
    /// Refuses a slot that cannot retain every admitted maximum before the platform call.
    pub fn require_reserved_bytes(&self, reserved: u32) -> Result<(), String> {
        if reserved < self.required_slot_bytes {
            return Err(
                "ordinary Native outbox slot is below the actual proof/clock/receipt budget".into(),
            );
        }
        Ok(())
    }
    /// Check complete assembled canonical frames again before DATA commit/State/outbox effects.
    /// The real Native owner must supply its same descriptor-held slot, never a caller integer.
    /// # Errors
    /// Refuses an absent or oversized carrier and any capacity mismatch.
    pub fn require_assembled_frames(
        &self,
        private_service: &[u8],
        compact_receiver: &[u8],
        reserved: u32,
    ) -> Result<(), String> {
        self.require_reserved_bytes(reserved)?;
        if private_service.is_empty()
            || compact_receiver.is_empty()
            || private_service.len() > self.private_service_max as usize
            || compact_receiver.len() > self.compact_receiver_max as usize
        {
            return Err("ordinary complete carrier exceeds its reserved frame maximum".into());
        }
        let actual = private_service
            .len()
            .checked_add(compact_receiver.len())
            .ok_or("ordinary carrier size overflow")?;
        if actual > reserved as usize {
            return Err(
                "ordinary actual complete carriers exceed their Native held reservation".into(),
            );
        }
        Ok(())
    }
}
fn size<T: norito::NoritoSerialize>(value: &T) -> Result<usize, String> {
    norito::canonical_frame_len(value).map_err(|e| e.to_string())
}
fn pair_size(
    relation: u8,
    release: DigestV1,
    manifest: DigestV1,
    protocols: [DigestV1; 2],
    lengths: [usize; 2],
) -> Result<usize, String> {
    size(&OrdinaryCashProofPairWireV1 {
        version: 1,
        relation,
        release_id: release,
        artifact_manifest_digest: manifest,
        eq_protocol_digest: protocols[0],
        ep_protocol_digest: protocols[1],
        eq_deferred_audit: [0; 32],
        ep_deferred_audit: [0; 32],
        eq_proof: vec![0; lengths[0]],
        ep_proof: vec![0; lengths[1]],
        eq_history: [0; KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
        ep_history: [0; KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
    })
}
fn sum(parts: &[usize]) -> Result<usize, String> {
    parts.iter().try_fold(0usize, |n, p| {
        n.checked_add(*p)
            .ok_or_else(|| "ordinary carrier budget overflow".into())
    })
}
fn totals(
    guard_frame: usize,
    terminal_frame: usize,
    wrapper_frame: usize,
) -> Result<[u32; 3], String> {
    let private_service = sum(&[
        3 * CLOCK_ORIGINAL_MAX,
        2 * STATE_BUNDLE_MAX,
        PREDECESSOR_STATE_MAX,
        KAGEMUSHA_ORDINARY_CASH_RECORD_MAX_BYTES_V1,
        KAGEMUSHA_APP_OPERATION_APPROVAL_MAX_BYTES_V1,
        4096,
        guard_frame,
        terminal_frame,
        wrapper_frame,
        2 * KAGEMUSHA_ORDINARY_CASH_RECORD_MAX_BYTES_V1,
        KAGEMUSHA_ORDINARY_RETAIL_ENROLLMENT_MAX_BYTES_V1,
        3 * KAGEMUSHA_ORDINARY_CURRENT_CONTROL_MAX_BYTES_V1,
        OUTER_FRAME_ROOM,
    ])?;
    let compact_receiver = sum(&[
        CLOCK_ORIGINAL_MAX,
        DATA_RECEIPT_MAX,
        KAGEMUSHA_ORDINARY_RETAIL_ENROLLMENT_MAX_BYTES_V1,
        KAGEMUSHA_ORDINARY_CURRENT_CONTROL_MAX_BYTES_V1,
        KAGEMUSHA_ORDINARY_APP_ENROLLMENT_MAX_BYTES_V1,
        KAGEMUSHA_APP_OPERATION_APPROVAL_MAX_BYTES_V1,
        4096,
        5 * KAGEMUSHA_ORDINARY_CASH_RECORD_MAX_BYTES_V1,
        KAGEMUSHA_RELEASE_MANIFEST_MAX_BYTES_V1,
        wrapper_frame,
        OUTER_FRAME_ROOM,
    ])?;
    let slot = sum(&[
        private_service,
        compact_receiver,
        usize::try_from(KAGEMUSHA_SEALED_TRANSITION_INPUTS_MAX_BYTES_V1)
            .map_err(|_| "ordinary sealed transition bound exceeds platform usize")?,
        usize::try_from(KAGEMUSHA_RECOVERY_SEEDS_MAX_BYTES_V1)
            .map_err(|_| "ordinary recovery seed bound exceeds platform usize")?,
        usize::try_from(KAGEMUSHA_OUTBOX_RETRY_METADATA_MAX_BYTES_V1)
            .map_err(|_| "ordinary retry metadata bound exceeds platform usize")?,
    ])?;
    Ok([private_service, compact_receiver, slot]
        .map(|n| {
            u32::try_from(n).map_err(|_| "ordinary carrier budget exceeds Native u32".to_string())
        })
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| "ordinary carrier total count".to_string())?)
}
/// Derive real proof lengths and bounded full-original storage from the actual authorized verifier.
/// Profile-only lengths grant no monetary or DATA capability. No caller-provided profile is used.
/// # Errors
/// Refuses an unadmitted release, malformed protocol, zero/oversized proof profile or overflow.
pub fn ordinary_cash_carrier_budget_v1(
    verifier: &KagemushaAuthenticatedRecursiveVerifierV1,
) -> Result<KagemushaOrdinaryCashCarrierBudgetV1, String> {
    let release = verifier.monetary_release()?;
    let s = verifier.state_checkpoint_material();
    let g = verifier.ordinary_guard_verifier_material();
    let t = verifier.ordinary_cash_terminal_verifier_material()?;
    let profiles = [
        [
            ordinary_ipa_proof_profile_v1(s.outer_eq_protocol)?.byte_len,
            ordinary_ipa_proof_profile_v1(s.outer_ep_protocol)?.byte_len,
        ],
        [
            ordinary_ipa_proof_profile_v1(g.eq_protocol)?.byte_len,
            ordinary_ipa_proof_profile_v1(g.ep_protocol)?.byte_len,
        ],
        [
            ordinary_ipa_proof_profile_v1(t.terminal_eq_protocol)?.byte_len,
            ordinary_ipa_proof_profile_v1(t.terminal_ep_protocol)?.byte_len,
        ],
        [
            ordinary_ipa_proof_profile_v1(t.wrapper_eq_protocol)?.byte_len,
            ordinary_ipa_proof_profile_v1(t.wrapper_ep_protocol)?.byte_len,
        ],
    ];
    if profiles.iter().flatten().any(|n| *n == 0) {
        return Err("ordinary budget requires nonempty real protocol profiles".into());
    }
    // Encoding zeros here measures fixed codec framing only. These bytes are never admitted,
    // verified, returned as a proof or promoted to a Native mathematical/financial capability.
    let state = size(&KagemushaPairedProofV1 {
        version: 1,
        semantic_digest: [0; 32],
        eq_protocol_digest: s.binding.outer_eq_protocol_digest,
        ep_protocol_digest: s.binding.outer_ep_protocol_digest,
        eq_deferred_audit: [0; 32],
        ep_deferred_audit: [0; 32],
        guard_eq_credential_audit: [0; 32],
        guard_ep_credential_audit: [0; 32],
        eq_proof: vec![0; profiles[0][0]],
        ep_proof: vec![0; profiles[0][1]],
        eq_history: vec![0; KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
        ep_history: vec![0; KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
    })?;
    let guard = size(&OrdinaryGuardProofWireV1 {
        version: 1,
        release_id: g.release_id,
        artifact_manifest_digest: g.artifact_manifest_digest,
        eq_protocol_digest: g.eq_protocol_digest,
        ep_protocol_digest: g.ep_protocol_digest,
        normalized_guard_digest: [0; 32],
        credential_digest: [0; 32],
        authorization_transcript_digest: [0; 32],
        subject_signing_digest: [0; 32],
        provider_policy_root: [0; 32],
        eq_proof: vec![0; profiles[1][0]],
        ep_proof: vec![0; profiles[1][1]],
        eq_history: [0; KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
        ep_history: [0; KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
    })?;
    let terminal = pair_size(
        1,
        t.release_id,
        t.artifact_manifest_digest,
        t.terminal_protocol_digests,
        profiles[2],
    )?;
    let wrapper = pair_size(
        2,
        t.release_id,
        t.artifact_manifest_digest,
        t.wrapper_protocol_digests,
        profiles[3],
    )?;
    if state > KAGEMUSHA_PAIRED_PROOF_MAX_BYTES_V1
        || guard > crate::kagemusha_v1_state::KAGEMUSHA_GUARD_BUNDLE_MAX_BYTES_V1
        || terminal > ORDINARY_INNER_TERMINAL_MAX_BYTES_V1
        || wrapper > KAGEMUSHA_PAIRED_PROOF_MAX_BYTES_V1
    {
        return Err("ordinary loaded proof frames exceed the actual decoder bounds".into());
    }
    let [service_max, receiver_max, slot] = totals(guard, terminal, wrapper)?;
    Ok(KagemushaOrdinaryCashCarrierBudgetV1 {
        release: release.release_id(),
        profiles,
        state_pair_bytes: state,
        guard_pair_bytes: guard,
        terminal_pair_bytes: terminal,
        wrapper_pair_bytes: wrapper,
        private_service_max: service_max,
        compact_receiver_max: receiver_max,
        required_slot_bytes: slot,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn full_original_budget_includes_three_clocks_and_distinct_receiver_receipt() {
        let [service, receiver, slot] = totals(4096, 2 * 1024 * 1024, 6528).unwrap();
        assert!(service as usize > 3 * CLOCK_ORIGINAL_MAX);
        assert!(receiver as usize > CLOCK_ORIGINAL_MAX + DATA_RECEIPT_MAX);
        assert!(slot > service + receiver);
        assert!(slot > 192 * 1024);
        assert!(service < 64 * 1024 * 1024);
        let next = totals(4097, 2 * 1024 * 1024 + 1, 6529).unwrap();
        assert_eq!(next[0], service + 3);
        assert_eq!(next[1], receiver + 1);
        assert_eq!(next[2], slot + 4);
    }
    #[test]
    fn complete_carrier_totals_refuse_native_u32_capacity_overflow() {
        let cap =
            usize::try_from(u32::MAX).expect("Native u32 limit fits supported platform usize");
        assert_eq!(
            totals(cap, 0, 0),
            Err("ordinary carrier budget exceeds Native u32".to_owned())
        );
    }
    #[test]
    fn summation_refuses_overflow_before_native_capacity_projection() {
        assert!(sum(&[usize::MAX, 1]).is_err());
    }
}

#[cfg(test)]
mod received_envelope_budget_tests {
    use super::*;
    #[test]
    fn receiver_reservation_retains_every_allowed_receipt_component_before_w2() {
        assert_eq!(DATA_RECEIPT_MAX, 16 * 1024 * 1024 + 192 * 1024);
        let maxima = sum(&[16 * 1024 * 1024, 128 * 1024, 32 * 1024]).unwrap();
        assert!(DATA_RECEIPT_MAX > maxima);
        let [service, receiver, slot] = totals(4096, 2 * 1024 * 1024, 6528).unwrap();
        assert!(receiver as usize > CLOCK_ORIGINAL_MAX + DATA_RECEIPT_MAX);
        assert!(slot > service + receiver);
    }
}
