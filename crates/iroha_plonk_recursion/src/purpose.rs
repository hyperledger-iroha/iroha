//! Fixed first-release fold domains chosen by the compiled consuming relation.

/// Role of one committed-mask fold. These tags are protocol transcript constants,
/// never witness-selected flags or a general numeric profile namespace.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FoldPurpose {
    /// Q sigma aggregation of own/incoming sigma openings plus the deciding filler.
    SigmaBatch,
    /// Pallas aggregation in initial A, before any W continuation.
    AInitial,
    /// Pallas aggregation in a nonterminal A stage after its preceding W.
    AContinuation,
    /// Pallas aggregation in final A, exactly four slots with explicit padding.
    ATerminal,
    /// Closed Vesta aggregation from the exact succinct A frame inside W/Omega.
    WrapperVesta,
    /// Pallas aggregation into acc_P in the folding Fp wrap P (KAGEMUSHA-Fold
    /// compression): the decided claims of the Q side and the restructured
    /// Pallas claim accumulator.
    FoldWrapPallas,
    /// Vesta aggregation into acc_V in the folding Omega wrap: P's own
    /// opening, the decided claims of the A side and the restructured Vesta
    /// claim accumulator.
    FoldWrapVesta,
}
impl FoldPurpose {
    /// Whether this compiled role permits the selected curve and fixed slot count.
    /// The terminal roles have exactly four inputs; generic roles stay nonempty.
    pub fn permits_shape<C: iroha_pasta::PastaCurve>(self, count: usize) -> bool {
        if count == 0 {
            return false;
        }
        match self {
            Self::SigmaBatch => C::CURVE_ID == "vesta",
            Self::AInitial | Self::AContinuation => C::CURVE_ID == "pallas",
            Self::ATerminal => C::CURVE_ID == "pallas" && count == 4,
            Self::WrapperVesta => C::CURVE_ID == "vesta" && count == 4,
            Self::FoldWrapPallas => C::CURVE_ID == "pallas",
            Self::FoldWrapVesta => C::CURVE_ID == "vesta",
        }
    }
    /// Canonical eight-byte domain identifier, also required by private storage.
    pub const fn tag(self) -> [u8; 8] {
        match self {
            Self::SigmaBatch => *b"sigfold1",
            Self::AInitial => *b"a0pfold1",
            Self::AContinuation => *b"acpfold1",
            Self::ATerminal => *b"atpfold1",
            Self::WrapperVesta => *b"wrapv001",
            Self::FoldWrapPallas => *b"fwpfold1",
            Self::FoldWrapVesta => *b"fwvfold1",
        }
    }
    /// Native integer placed in context word two, exactly as the circuit constant.
    pub const fn word(self) -> u64 {
        u64::from_le_bytes(self.tag())
    }
    /// Decode only a named first-release domain. There are no numeric aliases.
    pub fn from_tag(tag: [u8; 8]) -> Option<Self> {
        [
            Self::SigmaBatch,
            Self::AInitial,
            Self::AContinuation,
            Self::ATerminal,
            Self::WrapperVesta,
            Self::FoldWrapPallas,
            Self::FoldWrapVesta,
        ]
        .into_iter()
        .find(|purpose| purpose.tag() == tag)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roles_pin_curve_and_terminal_slot_count() {
        use iroha_pasta::{Ep, Eq};
        for count in 0..8 {
            assert_eq!(
                FoldPurpose::SigmaBatch.permits_shape::<Eq>(count),
                count != 0
            );
            assert!(!FoldPurpose::SigmaBatch.permits_shape::<Ep>(count));
            for p in [FoldPurpose::AInitial, FoldPurpose::AContinuation] {
                assert_eq!(p.permits_shape::<Ep>(count), count != 0);
                assert!(!p.permits_shape::<Eq>(count));
            }
            assert_eq!(
                FoldPurpose::ATerminal.permits_shape::<Ep>(count),
                count == 4
            );
            assert!(!FoldPurpose::ATerminal.permits_shape::<Eq>(count));
            assert_eq!(
                FoldPurpose::WrapperVesta.permits_shape::<Eq>(count),
                count == 4
            );
            assert!(!FoldPurpose::WrapperVesta.permits_shape::<Ep>(count));
            assert_eq!(
                FoldPurpose::FoldWrapPallas.permits_shape::<Ep>(count),
                count != 0
            );
            assert!(!FoldPurpose::FoldWrapPallas.permits_shape::<Eq>(count));
            assert_eq!(
                FoldPurpose::FoldWrapVesta.permits_shape::<Eq>(count),
                count != 0
            );
            assert!(!FoldPurpose::FoldWrapVesta.permits_shape::<Ep>(count));
        }
    }
    #[test]
    fn domains_are_exact_distinct_and_have_no_numeric_profile_alias() {
        let purposes = [
            FoldPurpose::SigmaBatch,
            FoldPurpose::AInitial,
            FoldPurpose::AContinuation,
            FoldPurpose::ATerminal,
            FoldPurpose::WrapperVesta,
            FoldPurpose::FoldWrapPallas,
            FoldPurpose::FoldWrapVesta,
        ];
        let mut tags = std::collections::BTreeSet::new();
        for purpose in purposes {
            assert!(tags.insert(purpose.tag()));
            assert_eq!(FoldPurpose::from_tag(purpose.tag()), Some(purpose));
            assert_eq!(purpose.word(), u64::from_le_bytes(purpose.tag()));
        }
        assert_eq!(FoldPurpose::from_tag(1u64.to_le_bytes()), None);
        assert_eq!(FoldPurpose::from_tag(*b"SIGFOLD1"), None);
        assert_eq!(FoldPurpose::from_tag([0; 8]), None);
    }
}
