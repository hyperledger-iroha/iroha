//! Acyclic exact incoming W1 operands. Decoding creates no Native, clock, proof or CAS owner.
use crate::kagemusha::{
    KAGEMUSHA_APP_OPERATION_APPROVAL_MAX_LIFETIME_MS_V1, KagemushaOrdinaryCashClockContextV1,
};
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use norito::codec::{Decode, Encode};
use sha2::{Digest as _, Sha256};

/// Exact fixed intent payload, excluding its purpose domain and length prefix.
pub const KAGEMUSHA_ORDINARY_INCOMING_TERMINAL_INTENT_BYTES_V1: usize = 661;
/// Full intent plus its separately domain-bound digest, excluding the body domain/length.
pub const KAGEMUSHA_ORDINARY_INCOMING_TERMINAL_BODY_BYTES_V1: usize = 693;
const INTENT_DOMAIN: &[u8] = b"iroha:kagemusha:v1:ordinary-incoming-terminal-intent\0";
const BODY_DOMAIN: &[u8] = b"iroha:kagemusha:v1:ordinary-incoming-terminal-body\0";

/// Complete DATA selected by Main only after authentic incoming Reserve, W2/Guard and State.
/// The full fresh FI and signed clock originals are retained separately by their actual owners.
/// No future W1 signature, incoming Commit proof or Commit receipt appears in this intent.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaOrdinaryIncomingTerminalIntentV1")]
pub struct KagemushaOrdinaryIncomingTerminalIntentV1 {
    /// Sole first-release version.
    pub version: u16,
    /// 1=MintFold, 3=ReceiveFold. Outgoing bodies belong to a separate protocol.
    pub operation: u8,
    /// Fresh Native operation, independent of the original W2/source operation.
    pub native_operation_id: [u8; 32],
    /// Fresh independently sampled Native W1 nonce.
    pub native_nonce: [u8; 32],
    /// Complete fixed incoming preparation binding, held before W2.
    pub preparation_digest: [u8; 32],
    /// Sole original source/head reservation digest, held before W2 and global Reserve.
    pub reservation_digest: [u8; 32],
    /// Full authentic finalized Mint source or received commit assertion original SHA.
    pub finalized_source_original_sha256: [u8; 32],
    /// Full original `MintCredit` proof or complete compact sender carrier SHA.
    pub source_proof_original_sha256: [u8; 32],
    /// Exact complete successor PUBLIC State original, including its proof/history envelope.
    pub state_original_sha256: [u8; 32],
    /// Exact whole transition statement original SHA, separate from its binding digest.
    pub transition_statement_original_sha256: [u8; 32],
    /// Complete paired W2 preparation Guard original SHA.
    pub preparation_guard_original_sha256: [u8; 32],
    /// Complete immutable incoming candidate original SHA.
    pub candidate_original_sha256: [u8; 32],
    /// Full earlier captured incoming purpose2 platform original SHA.
    pub purpose2_approval_original_sha256: [u8; 32],
    /// Exact fresh FI-control original captured separately for W1 after slow State proving.
    pub financial_control_original_sha256: [u8; 32],
    /// Actual preselection Main financial descriptor prefix digest.
    pub predecessor_descriptor_prefix_digest: [u8; 32],
    /// Exact acknowledged global Reserve request original SHA selected by the Native CAS WAL.
    pub reserve_request_original_sha256: [u8; 32],
    /// Complete independently authenticated global Reserve receipt original SHA.
    pub reserve_receipt_original_sha256: [u8; 32],
    /// Genuine nonce-bound Native context, checked against the held four signed observations.
    pub clock_context: KagemushaOrdinaryCashClockContextV1,
    /// Actual financial authorization index, never the platform's Apple counter.
    pub financial_index_before: u128,
    /// Exact next financial authorization index.
    pub financial_index_after: u128,
    /// Full logical financial State sequence before this edge.
    pub financial_sequence_before: u128,
    /// Exact next full logical financial State sequence.
    pub financial_sequence_after: u128,
    /// Main financial journal revision before this edge, independently represented as u64.
    pub logical_journal_sequence_before: u64,
    /// Exact next Main financial journal revision.
    pub logical_journal_sequence_after: u64,
    /// Original W1 start selected from the held inclusive Native interval.
    pub issued_at_ms: u64,
    /// Original exclusive W1 end, bounded by exact selected C/PI/FI validity.
    pub expires_at_ms: u64,
}
impl KagemushaOrdinaryIncomingTerminalIntentV1 {
    /// Check DATA shape only; all authority and original equality remains independently checked.
    /// # Errors
    /// Refuses another operation/version, missing originals, malformed time or overflowing edges.
    pub fn validate_shape(&self) -> Result<(), String> {
        self.clock_context.validate_shape()?;
        if self.version != 1
            || !matches!(self.operation, 1 | 3)
            || self.digests().contains(&[0; 32])
            || self.native_operation_id == self.native_nonce
            || self.financial_index_before.checked_add(1) != Some(self.financial_index_after)
            || self.financial_sequence_before.checked_add(1) != Some(self.financial_sequence_after)
            || self.logical_journal_sequence_before.checked_add(1)
                != Some(self.logical_journal_sequence_after)
            || self
                .expires_at_ms
                .checked_sub(self.issued_at_ms)
                .filter(|n| *n > 0 && *n <= KAGEMUSHA_APP_OPERATION_APPROVAL_MAX_LIFETIME_MS_V1)
                .is_none()
        {
            return Err(
                "ordinary incoming terminal exact originals or financial edge differ".into(),
            );
        }
        self.clock_context
            .validate_within_original_window(self.issued_at_ms, self.expires_at_ms)
    }
    fn digests(&self) -> [[u8; 32]; 15] {
        [
            self.native_operation_id,
            self.native_nonce,
            self.preparation_digest,
            self.reservation_digest,
            self.finalized_source_original_sha256,
            self.source_proof_original_sha256,
            self.state_original_sha256,
            self.transition_statement_original_sha256,
            self.preparation_guard_original_sha256,
            self.candidate_original_sha256,
            self.purpose2_approval_original_sha256,
            self.financial_control_original_sha256,
            self.predecessor_descriptor_prefix_digest,
            self.reserve_request_original_sha256,
            self.reserve_receipt_original_sha256,
        ]
    }
    /// Sole fixed payload, exact declared field order and full little-endian integer widths.
    /// # Errors
    /// Refuses invalid shape or another payload inventory.
    pub fn binding_transcript(
        &self,
    ) -> Result<[u8; KAGEMUSHA_ORDINARY_INCOMING_TERMINAL_INTENT_BYTES_V1], String> {
        self.validate_shape()?;
        let mut out = [0; KAGEMUSHA_ORDINARY_INCOMING_TERMINAL_INTENT_BYTES_V1];
        let mut at = 0;
        let mut add = |raw: &[u8]| {
            out[at..at + raw.len()].copy_from_slice(raw);
            at += raw.len();
        };
        add(&self.version.to_le_bytes());
        add(&[self.operation]);
        for value in self.digests() {
            add(&value);
        }
        add(&self.clock_context.version.to_le_bytes());
        add(&self.clock_context.request_nonce);
        add(&self.clock_context.signed_observations_original_digest);
        add(&self.clock_context.lower_at_ms.to_le_bytes());
        add(&self.clock_context.upper_at_ms.to_le_bytes());
        for value in [
            self.financial_index_before,
            self.financial_index_after,
            self.financial_sequence_before,
            self.financial_sequence_after,
        ] {
            add(&value.to_le_bytes());
        }
        for value in [
            self.logical_journal_sequence_before,
            self.logical_journal_sequence_after,
            self.issued_at_ms,
            self.expires_at_ms,
        ] {
            add(&value.to_le_bytes());
        }
        if at != out.len() {
            return Err("ordinary incoming terminal transcript width differs".into());
        }
        Ok(out)
    }
    /// Exact purpose-bound complete intent identity, without granting monetary authority.
    /// # Errors
    /// Refuses invalid DATA shape.
    pub fn binding_digest(&self) -> Result<[u8; 32], String> {
        Ok(terminal_hash(INTENT_DOMAIN, &self.binding_transcript()?))
    }
}

/// Distinct complete incoming W1 body. Its fixed stream opens the whole original intent and
/// its purpose digest together, avoiding a self-reference to any future proof or signature.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaOrdinaryIncomingTerminalBodyV1")]
pub struct KagemushaOrdinaryIncomingTerminalBodyV1 {
    /// Sole complete original intent; no caller-selected digest-only replacement.
    pub intent: KagemushaOrdinaryIncomingTerminalIntentV1,
}
impl KagemushaOrdinaryIncomingTerminalBodyV1 {
    /// Sole fixed full incoming body stream, including the purpose-separated intent digest.
    /// # Errors
    /// Refuses invalid original intent shape.
    pub fn binding_transcript(
        &self,
    ) -> Result<[u8; KAGEMUSHA_ORDINARY_INCOMING_TERMINAL_BODY_BYTES_V1], String> {
        let intent = self.intent.binding_transcript()?;
        let mut out = [0; KAGEMUSHA_ORDINARY_INCOMING_TERMINAL_BODY_BYTES_V1];
        out[..intent.len()].copy_from_slice(&intent);
        out[intent.len()..].copy_from_slice(&self.intent.binding_digest()?);
        Ok(out)
    }
    /// Purpose-bound complete incoming body identity, distinct from its intent and canonical SHA.
    /// # Errors
    /// Refuses invalid original intent shape.
    pub fn binding_digest(&self) -> Result<[u8; 32], String> {
        Ok(terminal_hash(BODY_DOMAIN, &self.binding_transcript()?))
    }
}
fn terminal_hash(domain: &[u8], body: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(domain);
    h.update((body.len() as u64).to_le_bytes());
    h.update(body);
    h.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn specimen(operation: u8) -> KagemushaOrdinaryIncomingTerminalBodyV1 {
        KagemushaOrdinaryIncomingTerminalBodyV1 {
            intent: KagemushaOrdinaryIncomingTerminalIntentV1 {
                version: 1,
                operation,
                native_operation_id: [1; 32],
                native_nonce: [2; 32],
                preparation_digest: [3; 32],
                reservation_digest: [4; 32],
                finalized_source_original_sha256: [5; 32],
                source_proof_original_sha256: [6; 32],
                state_original_sha256: [7; 32],
                transition_statement_original_sha256: [8; 32],
                preparation_guard_original_sha256: [9; 32],
                candidate_original_sha256: [10; 32],
                purpose2_approval_original_sha256: [11; 32],
                financial_control_original_sha256: [12; 32],
                predecessor_descriptor_prefix_digest: [13; 32],
                reserve_request_original_sha256: [14; 32],
                reserve_receipt_original_sha256: [15; 32],
                clock_context: KagemushaOrdinaryCashClockContextV1 {
                    version: 1,
                    request_nonce: [16; 32],
                    signed_observations_original_digest: [17; 32],
                    lower_at_ms: 101,
                    upper_at_ms: 102,
                },
                financial_index_before: (1u128 << 110) + 1,
                financial_index_after: (1u128 << 110) + 2,
                financial_sequence_before: (1u128 << 100) + 3,
                financial_sequence_after: (1u128 << 100) + 4,
                logical_journal_sequence_before: 21,
                logical_journal_sequence_after: 22,
                issued_at_ms: 100,
                expires_at_ms: 200,
            },
        }
    }
    #[test]
    fn incoming_terminal_pins_every_original_and_keeps_full_indices_and_purposes() {
        for operation in [1, 3] {
            let body = specimen(operation);
            let before = body.binding_digest().unwrap();
            assert_ne!(before, body.intent.binding_digest().unwrap());
            assert_eq!(body.binding_transcript().unwrap().len(), 693);
            let raw = norito::encode_canonical(&body).unwrap();
            let decoded: KagemushaOrdinaryIncomingTerminalBodyV1 =
                norito::decode_canonical_with_limits(
                    &raw,
                    norito::canonical_decode_limits(raw.len()),
                )
                .unwrap();
            assert_eq!(decoded, body);
            for n in 0..15 {
                let mut changed = body;
                let p = &mut changed.intent;
                let field = match n {
                    0 => &mut p.native_operation_id,
                    1 => &mut p.native_nonce,
                    2 => &mut p.preparation_digest,
                    3 => &mut p.reservation_digest,
                    4 => &mut p.finalized_source_original_sha256,
                    5 => &mut p.source_proof_original_sha256,
                    6 => &mut p.state_original_sha256,
                    7 => &mut p.transition_statement_original_sha256,
                    8 => &mut p.preparation_guard_original_sha256,
                    9 => &mut p.candidate_original_sha256,
                    10 => &mut p.purpose2_approval_original_sha256,
                    11 => &mut p.financial_control_original_sha256,
                    12 => &mut p.predecessor_descriptor_prefix_digest,
                    13 => &mut p.reserve_request_original_sha256,
                    _ => &mut p.reserve_receipt_original_sha256,
                };
                field[0] ^= 128;
                assert_ne!(changed.binding_digest().unwrap(), before);
            }
            for n in 0..5 {
                let mut changed = body;
                match n {
                    0 => {
                        changed.intent.financial_index_before += 1u128 << 90;
                        changed.intent.financial_index_after += 1u128 << 90;
                    }
                    1 => {
                        changed.intent.financial_sequence_before += 1u128 << 90;
                        changed.intent.financial_sequence_after += 1u128 << 90;
                    }
                    2 => changed.intent.clock_context.request_nonce[0] ^= 128,
                    3 => {
                        changed
                            .intent
                            .clock_context
                            .signed_observations_original_digest[0] ^= 128
                    }
                    _ => {
                        changed.intent.logical_journal_sequence_before += 1u64 << 40;
                        changed.intent.logical_journal_sequence_after += 1u64 << 40;
                    }
                }
                assert_ne!(changed.binding_digest().unwrap(), before);
            }
        }
        assert_ne!(
            specimen(1).binding_digest().unwrap(),
            specimen(3).binding_digest().unwrap()
        );
    }
    #[test]
    fn incoming_terminal_refuses_outgoing_reuse_overflow_and_original_window_substitution() {
        for n in 0..10 {
            let mut body = specimen(1);
            let p = &mut body.intent;
            match n {
                0 => p.operation = 2,
                1 => p.operation = 4,
                2 => p.version = 2,
                3 => p.reserve_receipt_original_sha256 = [0; 32],
                4 => {
                    p.financial_index_before = u128::MAX;
                    p.financial_index_after = 0;
                }
                5 => {
                    p.financial_sequence_before = u128::MAX;
                    p.financial_sequence_after = 0;
                }
                6 => {
                    p.logical_journal_sequence_before = u64::MAX;
                    p.logical_journal_sequence_after = 0;
                }
                7 => p.clock_context.upper_at_ms = p.expires_at_ms,
                8 => p.issued_at_ms = p.clock_context.lower_at_ms + 1,
                _ => {
                    p.expires_at_ms =
                        p.issued_at_ms + KAGEMUSHA_APP_OPERATION_APPROVAL_MAX_LIFETIME_MS_V1 + 1
                }
            }
            assert!(body.binding_digest().is_err());
        }
    }
}
