//! Ministry instructions.
//!
//! These instructions anchor Ministry transparency and agenda workflows in the
//! canonical ISI registry so Torii and SDKs can build signed transactions
//! without introducing ad-hoc payload formats.
use crate::ministry::AgendaProposalV1;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
/// Submit a citizen agenda proposal to the Ministry intake ledger.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SubmitAgendaProposal {
    /// Canonical agenda proposal payload.
    pub proposal: AgendaProposalV1,
}
impl crate::seal::Instruction for SubmitAgendaProposal {}
impl<'a> norito::core::DecodeFromSlice<'a> for SubmitAgendaProposal {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = norito::core::effective_decode_flags()
            .unwrap_or_else(norito::core::default_encode_flags);
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }
        let mut offset = 0usize;
        let proposal = super::decode_aos_canonical_field::<AgendaProposalV1>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((Self { proposal }, offset))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::isi::test_support::{
        assert_registry_decodes_type_name as assert_registry_decodes, assert_slice_roundtrip,
    };
    use crate::ministry::{
        AGENDA_PROPOSAL_VERSION_V1, AgendaEvidenceAttachment, AgendaEvidenceKind,
        AgendaProposalAction, AgendaProposalSubmitter, AgendaProposalSummary, AgendaProposalTarget,
    };
    fn proposal() -> AgendaProposalV1 {
        AgendaProposalV1 {
            version: AGENDA_PROPOSAL_VERSION_V1,
            proposal_id: "AC-2026-001".into(),
            submitted_at_unix_ms: 1_780_000_000_000,
            language: "en".into(),
            action: AgendaProposalAction::AddToDenylist,
            summary: AgendaProposalSummary {
                title: "Add abusive hash family".into(),
                motivation: "Multiple citizen reports for the same hash.".into(),
                expected_impact: "Removes known malicious payload".into(),
            },
            tags: vec!["csam".into()],
            targets: vec![AgendaProposalTarget {
                label: "Sample entry".into(),
                hash_family: "blake3-256".into(),
                hash_hex: "0d714bed4b7c63c23a2cf8ee9ce6c3cde1007907c427b4a0754e8ad31c91338d".into(),
                reason: "Flagged by moderators".into(),
            }],
            evidence: vec![AgendaEvidenceAttachment {
                kind: AgendaEvidenceKind::SorafsCid,
                uri: "sorafs://bafybei.../artifact.car".into(),
                digest_blake3_hex: Some(
                    "f1c02fb3bb194a9add242c3dfdf0bb2d94f9f3e1cf11f4a7d79d4012e4d0c2ad".into(),
                ),
                description: Some("Encrypted artifact bundle".into()),
            }],
            submitter: AgendaProposalSubmitter {
                name: "Citizen 42".into(),
                contact: "citizen42@example.org".into(),
                organization: Some("Wonderland Watch".into()),
                pgp_fingerprint: None,
            },
            duplicates: vec!["AC-2025-014".into()],
        }
    }
    #[test]
    fn ministry_decode_from_slice_roundtrips() {
        assert_slice_roundtrip(SubmitAgendaProposal {
            proposal: proposal(),
        });
    }
    #[test]
    fn ministry_registry_decodes_type_name() {
        let registry =
            crate::isi::InstructionRegistry::new().register_slice::<SubmitAgendaProposal>();
        assert_registry_decodes(
            &registry,
            SubmitAgendaProposal {
                proposal: proposal(),
            },
        );
    }
}
