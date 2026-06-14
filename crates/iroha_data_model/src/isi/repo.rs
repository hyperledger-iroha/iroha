use super::*;
use crate::repo::{RepoAgreementId, RepoCashLeg, RepoCollateralLeg, RepoGovernance};

/// Maximum supported haircut in basis points (100%).
const MAX_HAIRCUT_BPS: u16 = 10_000;

isi! {
    /// Initiate or roll a repo agreement between two counterparties.
    pub struct RepoIsi {
        /// Stable agreement identifier shared across the lifecycle.
        pub agreement_id: RepoAgreementId,
        /// Initiating account submitting the instruction.
        pub initiator: AccountId,
        /// Counterparty accepting the repo terms.
        pub counterparty: AccountId,
        /// Optional custodian account holding collateral in tri-party agreements.
        pub custodian: Option<AccountId>,
        /// Cash leg exchanged at initiation.
        pub cash_leg: RepoCashLeg,
        /// Collateral leg pledged for the agreement.
        pub collateral_leg: RepoCollateralLeg,
        /// Fixed interest rate, measured in basis points.
        pub rate_bps: u16,
        /// Unix timestamp (milliseconds) of the agreed maturity.
        pub maturity_timestamp_ms: u64,
        /// Governance knobs applied to this agreement.
        pub governance: RepoGovernance,
    }
}

impl core::fmt::Display for RepoIsi {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "REPO `{}` INIT `{}` COUNTER `{}` RATE {}bps MAT {}ms",
            self.agreement_id,
            self.initiator,
            self.counterparty,
            self.rate_bps,
            self.maturity_timestamp_ms,
        )?;
        if let Some(custodian) = &self.custodian {
            write!(f, " CUST `{custodian}`")?;
        }
        Ok(())
    }
}

isi! {
    /// Unwind an active repo agreement (reverse leg).
    pub struct ReverseRepoIsi {
        /// Identifier of the repo agreement being unwound.
        pub agreement_id: RepoAgreementId,
        /// Initiating account performing the unwind.
        pub initiator: AccountId,
        /// Counterparty receiving the unwind settlement.
        pub counterparty: AccountId,
        /// Cash leg returned at unwind.
        pub cash_leg: RepoCashLeg,
        /// Collateral leg released at unwind.
        pub collateral_leg: RepoCollateralLeg,
        /// Timestamp (milliseconds) when the unwind was agreed.
        pub settlement_timestamp_ms: u64,
    }
}

impl core::fmt::Display for ReverseRepoIsi {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "REPO_REVERSE `{}` INIT `{}` COUNTER `{}` SETTLE {}ms",
            self.agreement_id, self.initiator, self.counterparty, self.settlement_timestamp_ms,
        )
    }
}

impl core::fmt::Display for RepoMarginCallIsi {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "REPO_MARGIN `{}`", self.agreement_id)
    }
}

impl RepoIsi {
    /// Stable Norito wire identifier for registry lookups.
    pub const WIRE_ID: &'static str = "iroha.repo.initiate";

    /// Construct a repo instruction while clamping governance haircuts.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        agreement_id: RepoAgreementId,
        initiator: AccountId,
        counterparty: AccountId,
        custodian: Option<AccountId>,
        cash_leg: RepoCashLeg,
        collateral_leg: RepoCollateralLeg,
        rate_bps: u16,
        maturity_timestamp_ms: u64,
        governance: RepoGovernance,
    ) -> Self {
        Self {
            agreement_id,
            initiator,
            counterparty,
            custodian,
            cash_leg,
            collateral_leg,
            rate_bps,
            maturity_timestamp_ms,
            governance: RepoGovernance {
                haircut_bps: governance.haircut_bps().min(MAX_HAIRCUT_BPS),
                margin_frequency_secs: governance.margin_frequency_secs(),
            },
        }
    }
}

impl ReverseRepoIsi {
    /// Stable Norito wire identifier for the unwind.
    pub const WIRE_ID: &'static str = "iroha.repo.reverse";

    /// Construct an unwind instruction.
    pub fn new(
        agreement_id: RepoAgreementId,
        initiator: AccountId,
        counterparty: AccountId,
        cash_leg: RepoCashLeg,
        collateral_leg: RepoCollateralLeg,
        settlement_timestamp_ms: u64,
    ) -> Self {
        Self {
            agreement_id,
            initiator,
            counterparty,
            cash_leg,
            collateral_leg,
            settlement_timestamp_ms,
        }
    }
}

isi! {
    /// Record a margin check for an active repo agreement.
    pub struct RepoMarginCallIsi {
        /// Identifier of the repo agreement undergoing a margin check.
        pub agreement_id: RepoAgreementId,
    }
}

impl RepoMarginCallIsi {
    /// Stable Norito wire identifier for a margin call notification.
    pub const WIRE_ID: &'static str = "iroha.repo.margin_call";

    /// Construct a margin call instruction.
    pub fn new(agreement_id: RepoAgreementId) -> Self {
        Self { agreement_id }
    }
}

impl crate::seal::Instruction for RepoIsi {}
impl crate::seal::Instruction for ReverseRepoIsi {}
impl crate::seal::Instruction for RepoMarginCallIsi {}

isi_box! {
    /// Grouping enum for repo-related instructions.
    pub enum RepoInstructionBox {
        /// Initiate or roll a repo agreement.
        Initiate(RepoIsi),
        /// Reverse (unwind) an existing repo agreement.
        Reverse(ReverseRepoIsi),
        /// Record a margin check for an active repo agreement.
        MarginCall(RepoMarginCallIsi),
    }
}

impl_into_box! {
    RepoIsi | ReverseRepoIsi | RepoMarginCallIsi => RepoInstructionBox
}

impl crate::seal::Instruction for RepoInstructionBox {}

impl RepoInstructionBox {
    /// Stable Norito wire identifier for boxed repo instructions.
    pub const WIRE_ID: &'static str = "iroha.repo";
}

fn repo_decode_flags() -> u8 {
    norito::core::effective_decode_flags().unwrap_or_else(norito::core::default_encode_flags)
}

macro_rules! impl_repo_decode_from_slice {
    ($ty:ty { $($field:ident : $field_ty:ty),+ $(,)? }) => {
        impl<'a> norito::core::DecodeFromSlice<'a> for $ty {
            fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
                let flags = repo_decode_flags();
                if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
                    return super::decode_packed_instruction_payload::<Self>(bytes);
                }

                let mut offset = 0usize;
                $(
                    let $field = super::decode_aos_canonical_field::<$field_ty>(
                        super::read_aos_field(bytes, &mut offset, flags)?,
                        flags,
                    )?;
                )+
                if offset != bytes.len() {
                    return Err(norito::core::Error::LengthMismatch);
                }
                norito::core::note_payload_access(bytes, offset);
                Ok((Self { $($field),+ }, offset))
            }
        }
    };
}

impl_repo_decode_from_slice!(RepoIsi {
    agreement_id: RepoAgreementId,
    initiator: AccountId,
    counterparty: AccountId,
    custodian: Option<AccountId>,
    cash_leg: RepoCashLeg,
    collateral_leg: RepoCollateralLeg,
    rate_bps: u16,
    maturity_timestamp_ms: u64,
    governance: RepoGovernance,
});

impl_repo_decode_from_slice!(ReverseRepoIsi {
    agreement_id: RepoAgreementId,
    initiator: AccountId,
    counterparty: AccountId,
    cash_leg: RepoCashLeg,
    collateral_leg: RepoCollateralLeg,
    settlement_timestamp_ms: u64,
});

impl_repo_decode_from_slice!(RepoMarginCallIsi {
    agreement_id: RepoAgreementId,
});

impl<'a> norito::core::DecodeFromSlice<'a> for RepoInstructionBox {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = repo_decode_flags();
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }
        let tag_bytes = bytes.get(..4).ok_or(norito::core::Error::LengthMismatch)?;
        let tag = u32::from_le_bytes(
            tag_bytes
                .try_into()
                .map_err(|_| norito::core::Error::LengthMismatch)?,
        );
        let mut offset = 4usize;
        let value = match tag {
            0 => Self::Initiate(super::decode_aos_slice_field::<RepoIsi>(
                super::read_aos_field(bytes, &mut offset, flags)?,
                flags,
            )?),
            1 => Self::Reverse(super::decode_aos_slice_field::<ReverseRepoIsi>(
                super::read_aos_field(bytes, &mut offset, flags)?,
                flags,
            )?),
            2 => Self::MarginCall(super::decode_aos_slice_field::<RepoMarginCallIsi>(
                super::read_aos_field(bytes, &mut offset, flags)?,
                flags,
            )?),
            _ => {
                return Err(norito::core::Error::Message(format!(
                    "invalid RepoInstructionBox tag {tag}"
                )));
            }
        };
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((value, offset))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, KeyPair};
    use norito::core::DecodeFromSlice;

    use crate::repo::RepoGovernance;

    const INITIATOR: &str = "sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE";
    const COUNTERPARTY: &str = "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D";
    fn parse_account(raw: &str) -> AccountId {
        AccountId::parse_encoded(raw)
            .expect("valid account")
            .into_account_id()
    }

    fn seeded_account(seed: u8) -> AccountId {
        let keypair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("derive checked repo fixture account keypair");
        AccountId::new(keypair.public_key().clone())
    }

    fn agreement_id() -> RepoAgreementId {
        "daily_repo".parse().expect("id")
    }

    fn cash_leg() -> RepoCashLeg {
        RepoCashLeg {
            asset_definition_id: iroha_data_model::asset::AssetDefinitionId::new(
                DomainId::try_new("wonderland", "universal").unwrap(),
                "usd".parse().unwrap(),
            ),
            quantity: 1_000u32.into(),
        }
    }

    fn collateral_leg() -> RepoCollateralLeg {
        RepoCollateralLeg::new(
            iroha_data_model::asset::AssetDefinitionId::new(
                DomainId::try_new("wonderland", "universal").unwrap(),
                "bond".parse().unwrap(),
            ),
            1_100u32.into(),
        )
    }

    fn repo_instruction() -> RepoIsi {
        RepoIsi::new(
            agreement_id(),
            parse_account(INITIATOR),
            parse_account(COUNTERPARTY),
            Some(seeded_account(0xCD)),
            cash_leg(),
            collateral_leg(),
            250,
            1_704_000_000_000,
            RepoGovernance::with_defaults(1_500, 86_400),
        )
    }

    fn reverse_repo_instruction() -> ReverseRepoIsi {
        ReverseRepoIsi::new(
            agreement_id(),
            parse_account(INITIATOR),
            parse_account(COUNTERPARTY),
            cash_leg(),
            collateral_leg(),
            1_704_000_123_000,
        )
    }

    fn assert_slice_roundtrip<T>(value: T)
    where
        T: Clone + PartialEq + core::fmt::Debug + norito::codec::Encode,
        for<'a> T: DecodeFromSlice<'a>,
    {
        let bytes = value.encode();
        let (decoded, used) = T::decode_from_slice(&bytes).expect("decode from slice");
        assert_eq!(used, bytes.len());
        assert_eq!(decoded, value);
    }

    fn assert_registry_decodes<T>(
        registry: &crate::isi::InstructionRegistry,
        wire_id: &str,
        value: T,
    ) where
        T: crate::isi::Instruction
            + norito::codec::Encode
            + 'static
            + norito::core::NoritoSerialize,
        for<'de> T: norito::core::NoritoDeserialize<'de>,
    {
        let (payload, flags) = norito::codec::encode_with_header_flags(&value);
        let framed =
            norito::core::frame_bare_with_header_flags::<T>(&payload, flags).expect("frame");
        let decoded = crate::isi::InstructionRegistry::decode(registry, wire_id, &framed)
            .expect("registered")
            .expect("decode");
        assert_eq!(crate::isi::Instruction::dyn_encode(&*decoded), payload);
    }

    #[test]
    fn repo_instruction_roundtrip() {
        let agreement_id: RepoAgreementId = "daily_repo".parse().expect("id");
        let initiator = parse_account(INITIATOR);
        let counterparty = parse_account(COUNTERPARTY);
        let cash_leg = RepoCashLeg {
            asset_definition_id: iroha_data_model::asset::AssetDefinitionId::new(
                DomainId::try_new("wonderland", "universal").unwrap(),
                "usd".parse().unwrap(),
            ),
            quantity: 1_000u32.into(),
        };
        let collateral_leg = RepoCollateralLeg::new(
            iroha_data_model::asset::AssetDefinitionId::new(
                DomainId::try_new("wonderland", "universal").unwrap(),
                "bond".parse().unwrap(),
            ),
            1_100u32.into(),
        );
        let governance = RepoGovernance::with_defaults(1_500, 86_400);

        let instruction = RepoIsi::new(
            agreement_id.clone(),
            initiator.clone(),
            counterparty.clone(),
            None,
            cash_leg,
            collateral_leg,
            250,
            1_704_000_000_000,
            governance,
        );

        let bytes = instruction.encode();
        let decoded = RepoIsi::decode(&mut bytes.as_slice()).expect("decode");
        assert_eq!(instruction, decoded);
        assert_eq!(decoded.governance().haircut_bps(), governance.haircut_bps());
    }

    #[test]
    fn repo_display_includes_identifier() {
        let agreement_id: RepoAgreementId = "daily_repo".parse().expect("id");
        let initiator = parse_account(INITIATOR);
        let counterparty = parse_account(COUNTERPARTY);
        let cash_leg = RepoCashLeg {
            asset_definition_id: iroha_data_model::asset::AssetDefinitionId::new(
                DomainId::try_new("wonderland", "universal").unwrap(),
                "usd".parse().unwrap(),
            ),
            quantity: 1_000u32.into(),
        };
        let collateral_leg = RepoCollateralLeg::new(
            iroha_data_model::asset::AssetDefinitionId::new(
                DomainId::try_new("wonderland", "universal").unwrap(),
                "bond".parse().unwrap(),
            ),
            1_100u32.into(),
        );
        let governance = RepoGovernance::with_defaults(1_500, 86_400);

        let instruction = RepoIsi::new(
            agreement_id.clone(),
            initiator.clone(),
            counterparty.clone(),
            None,
            cash_leg,
            collateral_leg,
            250,
            1_704_000_000_000,
            governance,
        );

        let formatted = format!("{instruction}");
        assert!(formatted.contains("daily_repo"));
        assert!(formatted.contains("250bps"));
    }

    #[test]
    fn repo_display_includes_custodian_when_present() {
        let agreement_id: RepoAgreementId = "daily_repo".parse().expect("id");
        let initiator = parse_account(INITIATOR);
        let counterparty = parse_account(COUNTERPARTY);
        let custodian = seeded_account(0xCD);
        let cash_leg = RepoCashLeg {
            asset_definition_id: iroha_data_model::asset::AssetDefinitionId::new(
                DomainId::try_new("wonderland", "universal").unwrap(),
                "usd".parse().unwrap(),
            ),
            quantity: 1_000u32.into(),
        };
        let collateral_leg = RepoCollateralLeg::new(
            iroha_data_model::asset::AssetDefinitionId::new(
                DomainId::try_new("wonderland", "universal").unwrap(),
                "bond".parse().unwrap(),
            ),
            1_100u32.into(),
        );
        let governance = RepoGovernance::with_defaults(1_500, 86_400);

        let instruction = RepoIsi::new(
            agreement_id,
            initiator,
            counterparty,
            Some(custodian.clone()),
            cash_leg,
            collateral_leg,
            250,
            1_704_000_000_000,
            governance,
        );

        let formatted = format!("{instruction}");
        assert!(formatted.contains(&format!("CUST `{custodian}`")));
    }

    #[test]
    fn repo_margin_call_roundtrip() {
        let agreement_id: RepoAgreementId = "daily_repo".parse().expect("id");
        let instruction = RepoMarginCallIsi::new(agreement_id.clone());
        let bytes = instruction.encode();
        let decoded = RepoMarginCallIsi::decode(&mut bytes.as_slice()).expect("decode");
        assert_eq!(instruction, decoded);
        assert_eq!(decoded.agreement_id(), &agreement_id);
        assert_eq!(decoded.to_string(), "REPO_MARGIN `daily_repo`");
    }

    #[test]
    fn reverse_repo_display_includes_timestamp() {
        let agreement_id: RepoAgreementId = "daily_repo".parse().expect("id");
        let initiator = parse_account(INITIATOR);
        let counterparty = parse_account(COUNTERPARTY);
        let cash_leg = RepoCashLeg {
            asset_definition_id: iroha_data_model::asset::AssetDefinitionId::new(
                DomainId::try_new("wonderland", "universal").unwrap(),
                "usd".parse().unwrap(),
            ),
            quantity: 1_000u32.into(),
        };
        let collateral_leg = RepoCollateralLeg::new(
            iroha_data_model::asset::AssetDefinitionId::new(
                DomainId::try_new("wonderland", "universal").unwrap(),
                "bond".parse().unwrap(),
            ),
            1_100u32.into(),
        );

        let instruction = ReverseRepoIsi::new(
            agreement_id.clone(),
            initiator.clone(),
            counterparty.clone(),
            cash_leg,
            collateral_leg,
            1_704_000_123_000,
        );

        let formatted = format!("{instruction}");
        assert!(formatted.contains("REPO_REVERSE"));
        assert!(formatted.contains("1704000123000"));
    }

    #[test]
    fn repo_decode_from_slice_roundtrips() {
        assert_slice_roundtrip(repo_instruction());
        assert_slice_roundtrip(reverse_repo_instruction());
        assert_slice_roundtrip(RepoMarginCallIsi::new(agreement_id()));
    }

    #[test]
    fn repo_instruction_box_decode_from_slice_roundtrips() {
        assert_slice_roundtrip(RepoInstructionBox::Initiate(repo_instruction()));
        assert_slice_roundtrip(RepoInstructionBox::Reverse(reverse_repo_instruction()));
        assert_slice_roundtrip(RepoInstructionBox::MarginCall(RepoMarginCallIsi::new(
            agreement_id(),
        )));
    }

    #[test]
    fn repo_registry_decodes_type_names_and_stable_ids() {
        let registry = crate::isi::registry::default();
        assert_registry_decodes(
            &registry,
            std::any::type_name::<RepoInstructionBox>(),
            RepoInstructionBox::Initiate(repo_instruction()),
        );
        for value in [
            RepoInstructionBox::Initiate(repo_instruction()),
            RepoInstructionBox::Reverse(reverse_repo_instruction()),
            RepoInstructionBox::MarginCall(RepoMarginCallIsi::new(agreement_id())),
        ] {
            assert_registry_decodes(&registry, RepoInstructionBox::WIRE_ID, value);
        }
        for name in [
            std::any::type_name::<RepoIsi>(),
            std::any::type_name::<ReverseRepoIsi>(),
            std::any::type_name::<RepoMarginCallIsi>(),
            RepoIsi::WIRE_ID,
            ReverseRepoIsi::WIRE_ID,
            RepoMarginCallIsi::WIRE_ID,
        ] {
            assert!(!registry.contains(name), "{name} must remain boxed-only");
        }
    }
}
