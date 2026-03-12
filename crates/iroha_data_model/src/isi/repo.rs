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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo::RepoGovernance;

    const INITIATOR: &str = "6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw";
    const COUNTERPARTY: &str = "6cmzPVPX7WxKCts6hciUhyLdu7eZ7ZoHVuXXQ4YijdycaXbKykgP8jV";
    const CUSTODIAN: &str = "2CAE42qVd4hgS46pNUbsbgpK9UvsYSvnRkz15xzUiGc4QWLVzjpjhpg3KFuUyM3zDYfc7kc5QD3ct3BWmQgPDTa13kdC1k52T3Wgw7bUdccEKbhvMmX42d7tktNVdHSR8YjVJ3NyPN5jqBWWFCu6eefZ6E9nSw41JV4oRg";

    fn parse_account(raw: &str) -> AccountId {
        AccountId::parse_encoded(raw)
            .expect("valid account")
            .into_account_id()
    }

    #[test]
    fn repo_instruction_roundtrip() {
        let agreement_id: RepoAgreementId = "daily_repo".parse().expect("id");
        let initiator = parse_account(INITIATOR);
        let counterparty = parse_account(COUNTERPARTY);
        let cash_leg = RepoCashLeg {
            asset_definition_id: iroha_data_model::asset::AssetDefinitionId::new(
                "wonderland".parse().unwrap(),
                "usd".parse().unwrap(),
            ),
            quantity: 1_000u32.into(),
        };
        let collateral_leg = RepoCollateralLeg::new(
            iroha_data_model::asset::AssetDefinitionId::new(
                "wonderland".parse().unwrap(),
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
                "wonderland".parse().unwrap(),
                "usd".parse().unwrap(),
            ),
            quantity: 1_000u32.into(),
        };
        let collateral_leg = RepoCollateralLeg::new(
            iroha_data_model::asset::AssetDefinitionId::new(
                "wonderland".parse().unwrap(),
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
        let custodian = parse_account(CUSTODIAN);
        let cash_leg = RepoCashLeg {
            asset_definition_id: iroha_data_model::asset::AssetDefinitionId::new(
                "wonderland".parse().unwrap(),
                "usd".parse().unwrap(),
            ),
            quantity: 1_000u32.into(),
        };
        let collateral_leg = RepoCollateralLeg::new(
            iroha_data_model::asset::AssetDefinitionId::new(
                "wonderland".parse().unwrap(),
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
                "wonderland".parse().unwrap(),
                "usd".parse().unwrap(),
            ),
            quantity: 1_000u32.into(),
        };
        let collateral_leg = RepoCollateralLeg::new(
            iroha_data_model::asset::AssetDefinitionId::new(
                "wonderland".parse().unwrap(),
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
}
