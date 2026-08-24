//! Types for custom instructions
use derive_more::{Constructor, From};
use iroha_data_model::{
    isi::{CustomInstruction, InstructionBox},
    prelude::{Json, *},
};
use iroha_schema::IntoSchema;
#[allow(unused_imports)]
use std::eprintln;
use std::{
    collections::BTreeMap,
    format,
    string::{String, ToString},
    vec::Vec,
};
macro_rules! impl_custom_instruction {
    ($box:ty, $($instruction:ty)|+) => {
        impl From<$box> for CustomInstruction {
            fn from(value: $box) -> Self {
                let payload = norito::json::to_value(&value)
                    .expect(concat!("INTERNAL BUG: Couldn't serialize ", stringify!($box)));
                Self::new(payload)
            }
        }
        impl From<$box> for InstructionBox {
            fn from(value: $box) -> Self {
                InstructionBox::from(CustomInstruction::from(value))
            }
        }
        impl TryFrom<&Json> for $box {
            type Error = norito::Error;
            fn try_from(payload: &Json) -> Result<Self, norito::Error> {
                norito::json::from_str::<Self>(payload.as_ref())
                    .map_err(|e| norito::Error::from(e.to_string()))
            }
        }
        $(
            impl From<$instruction> for InstructionBox {
                fn from(value: $instruction) -> Self {
                    InstructionBox::from(<$box>::from(value))
                }
            }
        )+
    };
}
/// Types for multisig instructions
pub mod multisig {
    use super::*;
    use crate::json_macros::{
        JsonDeserialize as DeriveJsonDeserialize, JsonSerialize as DeriveJsonSerialize,
    };
    use core::num::{NonZeroU16, NonZeroU64};
    use iroha_crypto::{HashOf, KeyPair};
    use norito::json::{self, JsonDeserialize, JsonSerialize, Value};
    #[allow(unused_imports)]
    use std::eprintln;
    use std::{borrow::ToOwned, collections::BTreeSet};
    /// Multisig-related instructions
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema, From)]
    pub enum MultisigInstructionBox {
        /// Register a multisig account, which is a prerequisite of multisig transactions
        Register(MultisigRegister),
        /// Propose a multisig transaction and initialize approvals with the proposer's one
        Propose(MultisigPropose),
        /// Approve a certain multisig transaction
        Approve(MultisigApprove),
        /// Cancel a certain multisig transaction before it reaches quorum
        Cancel(MultisigCancel),
        /// Atomically invalidate every outstanding proposal owned by a multisig account
        InvalidateOutstanding(MultisigInvalidateOutstanding),
    }
    impl JsonSerialize for MultisigInstructionBox {
        fn json_serialize(&self, out: &mut String) {
            out.push('{');
            match self {
                Self::Register(value) => {
                    norito::json::write_json_string("Register", out);
                    out.push(':');
                    value.json_serialize(out);
                }
                Self::Propose(value) => {
                    norito::json::write_json_string("Propose", out);
                    out.push(':');
                    value.json_serialize(out);
                }
                Self::Approve(value) => {
                    norito::json::write_json_string("Approve", out);
                    out.push(':');
                    value.json_serialize(out);
                }
                Self::Cancel(value) => {
                    norito::json::write_json_string("Cancel", out);
                    out.push(':');
                    value.json_serialize(out);
                }
                Self::InvalidateOutstanding(value) => {
                    norito::json::write_json_string("InvalidateOutstanding", out);
                    out.push(':');
                    value.json_serialize(out);
                }
            }
            out.push('}');
        }

        fn json_serialize_to(
            &self,
            out: &mut dyn json::JsonWriteSink,
        ) -> Result<(), json::BoundedJsonError> {
            out.begin_container()?;
            out.push('{')?;
            match self {
                Self::Register(value) => {
                    json::write_json_string_to("Register", out)?;
                    out.push(':')?;
                    value.json_serialize_to(out)?;
                }
                Self::Propose(value) => {
                    json::write_json_string_to("Propose", out)?;
                    out.push(':')?;
                    value.json_serialize_to(out)?;
                }
                Self::Approve(value) => {
                    json::write_json_string_to("Approve", out)?;
                    out.push(':')?;
                    value.json_serialize_to(out)?;
                }
                Self::Cancel(value) => {
                    json::write_json_string_to("Cancel", out)?;
                    out.push(':')?;
                    value.json_serialize_to(out)?;
                }
                Self::InvalidateOutstanding(value) => {
                    json::write_json_string_to("InvalidateOutstanding", out)?;
                    out.push(':')?;
                    value.json_serialize_to(out)?;
                }
            }
            out.push('}')?;
            out.end_container();
            Ok(())
        }
    }
    impl JsonDeserialize for MultisigInstructionBox {
        fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
            let mut visitor = json::MapVisitor::new(parser)?;
            let mut variant: Option<Self> = None;
            while let Some(key) = visitor.next_key()? {
                let name = key.as_str().to_owned();
                match name.as_str() {
                    "Register" => {
                        if variant.is_some() {
                            visitor.skip_value()?;
                            return Err(json::Error::duplicate_field(name));
                        }
                        let value = visitor.parse_value::<MultisigRegister>()?;
                        variant = Some(Self::Register(value));
                    }
                    "Propose" => {
                        if variant.is_some() {
                            visitor.skip_value()?;
                            return Err(json::Error::duplicate_field(name));
                        }
                        let value = visitor.parse_value::<MultisigPropose>()?;
                        variant = Some(Self::Propose(value));
                    }
                    "Approve" => {
                        if variant.is_some() {
                            visitor.skip_value()?;
                            return Err(json::Error::duplicate_field(name));
                        }
                        let value = visitor.parse_value::<MultisigApprove>()?;
                        variant = Some(Self::Approve(value));
                    }
                    "Cancel" => {
                        if variant.is_some() {
                            visitor.skip_value()?;
                            return Err(json::Error::duplicate_field(name));
                        }
                        let value = visitor.parse_value::<MultisigCancel>()?;
                        variant = Some(Self::Cancel(value));
                    }
                    "InvalidateOutstanding" => {
                        if variant.is_some() {
                            visitor.skip_value()?;
                            return Err(json::Error::duplicate_field(name));
                        }
                        let value = visitor.parse_value::<MultisigInvalidateOutstanding>()?;
                        variant = Some(Self::InvalidateOutstanding(value));
                    }
                    other => {
                        visitor.skip_value()?;
                        return Err(json::Error::unknown_field(other));
                    }
                }
            }
            visitor.finish()?;
            variant.ok_or_else(|| json::Error::missing_field("variant"))
        }
    }
    /// Register a multisig account, which is a prerequisite of multisig transactions
    #[derive(
        Debug,
        Clone,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        Decode,
        Encode,
        IntoSchema,
        DeriveJsonSerialize,
    )]
    pub struct MultisigRegister {
        /// Account backing the multisig controller.
        ///
        /// The supplied id anchors the registration step, but the account is rekeyed to the
        /// canonical multisig controller derived from the spec after registration, so the key is
        /// never used for signing.
        pub account: AccountId,
        /// Optional home domain used for registration authorization, linking, and RBAC namespacing.
        pub home_domain: Option<DomainId>,
        /// Specification of the multisig account
        pub spec: MultisigSpec,
    }
    impl MultisigRegister {
        /// Construct a multisig registration.
        pub fn new(
            account: AccountId,
            home_domain: impl Into<Option<DomainId>>,
            spec: MultisigSpec,
        ) -> Self {
            Self {
                account,
                home_domain: home_domain.into(),
                spec,
            }
        }
        /// Construct a multisig registration using an explicit account id.
        pub fn with_account(
            account: AccountId,
            home_domain: impl Into<Option<DomainId>>,
            spec: MultisigSpec,
        ) -> Self {
            Self::new(account, home_domain, spec)
        }
        /// Construct a multisig registration using a freshly generated domainless account id.
        /// The generated key is not meant for direct signing; it only anchors the registration
        /// step before the account is rekeyed to the canonical controller derived from the spec.
        ///
        /// # Errors
        ///
        /// Returns an error if fresh account key generation fails.
        pub fn from_spec(
            home_domain: impl Into<Option<DomainId>>,
            spec: MultisigSpec,
        ) -> Result<Self, iroha_crypto::Error> {
            let key_pair = KeyPair::try_random()?;
            let account = AccountId::new(key_pair.public_key().clone());
            Ok(Self::new(account, home_domain, spec))
        }
    }
    impl JsonDeserialize for MultisigRegister {
        fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
            let mut visitor = json::MapVisitor::new(parser)?;
            let mut account: Option<AccountId> = None;
            let mut home_domain: Option<Option<DomainId>> = None;
            let mut spec: Option<MultisigSpec> = None;
            while let Some(key) = visitor.next_key()? {
                match key.as_str() {
                    "account" => {
                        let value = visitor.parse_value::<AccountId>()?;
                        account = Some(value);
                    }
                    "home_domain" => {
                        let value = visitor.parse_value::<Option<DomainId>>()?;
                        home_domain = Some(value);
                    }
                    "spec" => {
                        let value = visitor.parse_value::<MultisigSpec>()?;
                        spec = Some(value);
                    }
                    _ => {
                        visitor.skip_value()?;
                    }
                }
            }
            visitor.finish()?;
            let spec = spec.ok_or_else(|| json::Error::missing_field("spec"))?;
            let account = account.ok_or_else(|| json::Error::missing_field("account"))?;
            Ok(Self {
                account,
                home_domain: home_domain.unwrap_or(None),
                spec,
            })
        }
    }
    /// Relative weight of responsibility for the multisig account.
    /// 0 is allowed for observers who don't join governance
    type Weight = u8;
    /// Default multisig transaction time-to-live in milliseconds based on block timestamps
    pub const DEFAULT_MULTISIG_TTL_MS: u64 = 60 * 60 * 1_000; // 1 hour
    /// Propose a multisig transaction and initialize approvals with the proposer's one
    #[derive(
        Debug,
        Clone,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        Decode,
        Encode,
        IntoSchema,
        Constructor,
        DeriveJsonSerialize,
        DeriveJsonDeserialize,
    )]
    pub struct MultisigPropose {
        /// Multisig account to propose
        pub account: AccountId,
        /// Proposal contents
        pub instructions: Vec<InstructionBox>,
        /// Optional TTL to override the account default. Cannot be longer than the account default
        pub transaction_ttl_ms: Option<NonZeroU64>,
    }
    /// Approve a certain multisig transaction
    #[derive(
        Debug,
        Clone,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        Decode,
        Encode,
        IntoSchema,
        Constructor,
        DeriveJsonSerialize,
        DeriveJsonDeserialize,
    )]
    pub struct MultisigApprove {
        /// Multisig account to approve
        pub account: AccountId,
        /// Proposal to approve
        pub instructions_hash: HashOf<Vec<InstructionBox>>,
    }
    /// Cancel a certain multisig transaction before it reaches quorum
    #[derive(
        Debug,
        Clone,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        Decode,
        Encode,
        IntoSchema,
        Constructor,
        DeriveJsonSerialize,
        DeriveJsonDeserialize,
    )]
    pub struct MultisigCancel {
        /// Multisig account that owns the target proposal
        pub account: AccountId,
        /// Proposal to cancel
        pub instructions_hash: HashOf<Vec<InstructionBox>>,
    }
    /// Atomically invalidate every outstanding proposal owned by a multisig account.
    ///
    /// This instruction must execute as `account` itself, normally as the first instruction in an
    /// approved multisig policy-change proposal. The proposal executing this instruction is already
    /// terminal before its payload runs, so only other outstanding proposals are invalidated.
    #[derive(
        Debug,
        Clone,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        Decode,
        Encode,
        IntoSchema,
        Constructor,
        DeriveJsonSerialize,
        DeriveJsonDeserialize,
    )]
    pub struct MultisigInvalidateOutstanding {
        /// Multisig account whose outstanding proposals must be invalidated.
        pub account: AccountId,
    }
    impl_custom_instruction!(
        MultisigInstructionBox,
        MultisigRegister
            | MultisigPropose
            | MultisigApprove
            | MultisigCancel
            | MultisigInvalidateOutstanding
    );
    impl TryFrom<&InstructionBox> for MultisigInstructionBox {
        type Error = norito::Error;
        fn try_from(instruction: &InstructionBox) -> Result<Self, norito::Error> {
            if let Some(multisig) = instruction
                .as_any()
                .downcast_ref::<MultisigInstructionBox>()
            {
                return Ok(multisig.clone());
            }
            let custom = instruction
                .as_any()
                .downcast_ref::<CustomInstruction>()
                .ok_or_else(|| {
                    norito::Error::Message("instruction is not CustomInstruction".into())
                })?;
            Self::try_from(custom.payload())
        }
    }
    /// Native ledger value for a multisig account state entry.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    pub struct MultisigAccountState {
        /// Canonical multisig account id for this state entry.
        pub account_id: AccountId,
        /// Optional home domain used to materialize missing signatory accounts and roles.
        pub home_domain: Option<DomainId>,
        /// Multisig policy specification.
        pub spec: MultisigSpec,
    }
    impl MultisigAccountState {
        /// Construct a multisig account state snapshot.
        pub fn new(
            account_id: AccountId,
            home_domain: impl Into<Option<DomainId>>,
            spec: MultisigSpec,
        ) -> Self {
            Self {
                account_id,
                home_domain: home_domain.into(),
                spec,
            }
        }
    }
    /// Native ledger value for a multisig proposal state entry.
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema, Constructor,
    )]
    pub struct MultisigProposalState {
        /// Canonical multisig account id that owns this proposal.
        pub multisig_account_id: AccountId,
        /// Hash of the proposed instruction list.
        pub instructions_hash: HashOf<Vec<InstructionBox>>,
        /// Proposal contents.
        pub instructions: Vec<InstructionBox>,
        /// Time in milliseconds at which the proposal was made.
        pub proposed_at_ms: u64,
        /// Time in milliseconds at which the proposal will expire.
        pub expires_at_ms: u64,
        /// List of approvers of the proposal so far.
        pub approvals: BTreeSet<AccountId>,
        /// In case this proposal is some relaying approval, indicates if it has executed or not.
        pub is_relayed: Option<bool>,
    }
    /// Terminal lifecycle states persisted for top-level multisig proposals.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    pub enum MultisigProposalTerminalStatus {
        /// Proposal executed after reaching quorum.
        Finalized,
        /// Proposal was canceled by a separate multisig action.
        Canceled,
        /// Proposal expired before reaching quorum.
        Expired,
    }
    /// Native ledger value for a persisted terminal multisig proposal entry.
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema, Constructor,
    )]
    pub struct MultisigProposalTerminalState {
        /// Canonical multisig account id that owns this proposal.
        pub multisig_account_id: AccountId,
        /// Hash of the proposed instruction list.
        pub instructions_hash: HashOf<Vec<InstructionBox>>,
        /// Proposal contents and collected approvals at the time it became terminal.
        pub proposal: MultisigProposalValue,
        /// Terminal lifecycle state.
        pub status: MultisigProposalTerminalStatus,
        /// Time in milliseconds at which the proposal became terminal.
        pub terminal_at_ms: u64,
    }
    /// Immutable transaction-bound evidence that a multisig proposal became terminal.
    ///
    /// This is intentionally a distinct versioned value instead of extending
    /// [`MultisigProposalTerminalState`], whose persisted Norito schema must remain decodable for
    /// existing ledgers.
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema, Constructor,
    )]
    pub struct MultisigProposalTerminalExecutionStateV1 {
        /// Terminal proposal payload captured at execution time.
        pub terminal: MultisigProposalTerminalState,
        /// Exact multisig account identifier carried by the entrypoint instruction.
        ///
        /// Current native approval admission requires this to resolve to the existing canonical
        /// account in `terminal.multisig_account_id`; persisting both makes that signed-to-resolved
        /// binding independently verifiable.
        pub entrypoint_account_id: AccountId,
        /// Finalised block height at which the proposal became terminal.
        pub terminal_block_height: u64,
        /// Hash of the block entrypoint that made the proposal terminal.
        pub terminal_entrypoint_hash: [u8; 32],
    }
    /// Whether one successful multisig approval entrypoint executed its proposal.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    pub enum MultisigApprovalOutcomeStatusV1 {
        /// The approval reached quorum and executed the concrete proposal instructions.
        Executed,
        /// The approval succeeded but did not execute proposal instructions.
        NotExecuted,
    }
    /// Immutable transaction-bound classification of one successful multisig approval.
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema, Constructor,
    )]
    pub struct MultisigApprovalOutcomeV1 {
        /// Exact account identifier carried by the signed approval instruction.
        pub entrypoint_account_id: AccountId,
        /// Canonical multisig account to which the entrypoint resolved during execution.
        pub resolved_multisig_account_id: AccountId,
        /// Hash carried by the signed approval instruction.
        pub instructions_hash: HashOf<Vec<InstructionBox>>,
        /// Whether this approval actually executed its concrete proposal instructions.
        pub status: MultisigApprovalOutcomeStatusV1,
        /// Finalised block height at which the approval entrypoint ran.
        pub block_height: u64,
        /// Hash of the block entrypoint containing the signed approval.
        pub entrypoint_hash: [u8; 32],
    }
    /// Metadata value for a multisig account specification
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema, Constructor,
    )]
    pub struct MultisigSpec {
        /// List of signatories and their relative weights of responsibility for the multisig account
        pub signatories: BTreeMap<AccountId, Weight>,
        /// Threshold of total weight at which the multisig account is considered authenticated
        pub quorum: NonZeroU16,
        /// Multisig transaction time-to-live in milliseconds based on block timestamps. Defaults to [`DEFAULT_MULTISIG_TTL_MS`]
        pub transaction_ttl_ms: NonZeroU64,
    }
    /// Metadata value for a multisig transaction proposal
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema, Constructor,
    )]
    pub struct MultisigProposalValue {
        /// Proposal contents
        pub instructions: Vec<InstructionBox>,
        /// Time in milliseconds at which the proposal was made
        pub proposed_at_ms: u64,
        /// Time in milliseconds at which the proposal will expire
        pub expires_at_ms: u64,
        /// List of approvers of the proposal so far
        pub approvals: BTreeSet<AccountId>,
        /// In case this proposal is some relaying approval, indicates if it has executed or not
        pub is_relayed: Option<bool>,
    }
    impl JsonSerialize for MultisigSpec {
        fn json_serialize(&self, out: &mut String) {
            out.push('{');
            norito::json::write_json_string("signatories", out);
            out.push(':');
            self.signatories.json_serialize(out);
            out.push(',');
            norito::json::write_json_string("quorum", out);
            out.push(':');
            self.quorum.json_serialize(out);
            out.push(',');
            norito::json::write_json_string("transaction_ttl_ms", out);
            out.push(':');
            self.transaction_ttl_ms.json_serialize(out);
            out.push('}');
        }
    }
    impl JsonDeserialize for MultisigSpec {
        fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
            let mut visitor = json::MapVisitor::new(parser)?;
            let mut signatories: Option<BTreeMap<AccountId, Weight>> = None;
            let mut quorum: Option<NonZeroU16> = None;
            let mut transaction_ttl_ms: Option<NonZeroU64> = None;
            while let Some(key) = visitor.next_key()? {
                match key.as_str() {
                    "signatories" => {
                        let raw = visitor.parse_value::<Value>()?;
                        let map = match raw {
                            Value::Object(map) => map,
                            _ => {
                                return Err(json::Error::InvalidField {
                                    field: "signatories".into(),
                                    message: "expected object".into(),
                                });
                            }
                        };
                        let mut parsed = BTreeMap::new();
                        for (account, weight_value) in map {
                            let account_id = AccountId::parse_encoded(&account)
                                .map(ParsedAccountId::into_account_id)
                                .map_err(|err| json::Error::InvalidField {
                                    field: format!("signatories.{account}"),
                                    message: err.to_string(),
                                })?;
                            let weight: Weight = json::from_value(weight_value)?;
                            parsed.insert(account_id, weight);
                        }
                        signatories = Some(parsed);
                    }
                    "quorum" => {
                        let value = visitor.parse_value::<NonZeroU16>()?;
                        quorum = Some(value);
                    }
                    "transaction_ttl_ms" => {
                        let value = visitor.parse_value::<NonZeroU64>()?;
                        transaction_ttl_ms = Some(value);
                    }
                    _ => {
                        visitor.skip_value()?;
                    }
                }
            }
            visitor.finish()?;
            let signatories =
                signatories.ok_or_else(|| json::Error::missing_field("signatories"))?;
            let quorum = quorum.ok_or_else(|| json::Error::missing_field("quorum"))?;
            let transaction_ttl_ms = transaction_ttl_ms
                .ok_or_else(|| json::Error::missing_field("transaction_ttl_ms"))?;
            Ok(Self {
                signatories,
                quorum,
                transaction_ttl_ms,
            })
        }
    }
    impl JsonSerialize for MultisigProposalValue {
        fn json_serialize(&self, out: &mut String) {
            out.push('{');
            norito::json::write_json_string("instructions", out);
            out.push(':');
            self.instructions.json_serialize(out);
            out.push(',');
            norito::json::write_json_string("proposed_at_ms", out);
            out.push(':');
            self.proposed_at_ms.json_serialize(out);
            out.push(',');
            norito::json::write_json_string("expires_at_ms", out);
            out.push(':');
            self.expires_at_ms.json_serialize(out);
            out.push(',');
            norito::json::write_json_string("approvals", out);
            out.push(':');
            self.approvals.json_serialize(out);
            out.push(',');
            norito::json::write_json_string("is_relayed", out);
            out.push(':');
            self.is_relayed.json_serialize(out);
            out.push('}');
        }
    }
    impl JsonDeserialize for MultisigProposalValue {
        fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
            let mut visitor = json::MapVisitor::new(parser)?;
            let mut instructions: Option<Vec<InstructionBox>> = None;
            let mut proposed_at_ms: Option<u64> = None;
            let mut expires_at_ms: Option<u64> = None;
            let mut approvals: Option<BTreeSet<AccountId>> = None;
            let mut is_relayed: Option<Option<bool>> = None;
            while let Some(key) = visitor.next_key()? {
                match key.as_str() {
                    "instructions" => {
                        let value = visitor.parse_value::<Vec<InstructionBox>>()?;
                        instructions = Some(value);
                    }
                    "proposed_at_ms" => {
                        let value = visitor.parse_value::<u64>()?;
                        proposed_at_ms = Some(value);
                    }
                    "expires_at_ms" => {
                        let value = visitor.parse_value::<u64>()?;
                        expires_at_ms = Some(value);
                    }
                    "approvals" => {
                        let value = visitor.parse_value::<BTreeSet<AccountId>>()?;
                        approvals = Some(value);
                    }
                    "is_relayed" => {
                        let value = visitor.parse_value::<Option<bool>>()?;
                        is_relayed = Some(value);
                    }
                    _ => {
                        visitor.skip_value()?;
                    }
                }
            }
            visitor.finish()?;
            let instructions =
                instructions.ok_or_else(|| json::Error::missing_field("instructions"))?;
            let proposed_at_ms =
                proposed_at_ms.ok_or_else(|| json::Error::missing_field("proposed_at_ms"))?;
            let expires_at_ms =
                expires_at_ms.ok_or_else(|| json::Error::missing_field("expires_at_ms"))?;
            let approvals = approvals.ok_or_else(|| json::Error::missing_field("approvals"))?;
            let is_relayed = is_relayed.unwrap_or(None);
            Ok(Self {
                instructions,
                proposed_at_ms,
                expires_at_ms,
                approvals,
                is_relayed,
            })
        }
    }
    impl From<MultisigSpec> for Json {
        fn from(details: MultisigSpec) -> Self {
            Json::new(details)
        }
    }
    impl TryFrom<&Json> for MultisigSpec {
        type Error = norito::Error;
        fn try_from(payload: &Json) -> Result<Self, norito::Error> {
            norito::json::from_str::<Self>(payload.as_ref())
                .map_err(|e| norito::Error::from(e.to_string()))
        }
    }
    impl From<MultisigProposalValue> for Json {
        fn from(details: MultisigProposalValue) -> Self {
            Json::new(details)
        }
    }
    impl TryFrom<&Json> for MultisigProposalValue {
        type Error = norito::Error;
        fn try_from(payload: &Json) -> Result<Self, norito::Error> {
            norito::json::from_str::<Self>(payload.as_ref())
                .map_err(|e| norito::Error::from(e.to_string()))
        }
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        use iroha_crypto::{Algorithm, KeyPair};
        use std::{
            collections::BTreeMap,
            num::{NonZeroU16, NonZeroU64},
        };
        fn fixture_key_pair(seed: u8) -> KeyPair {
            assert_ne!(seed, 0, "multisig fixture seeds must be nonzero");
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
                .expect("derive multisig fixture key")
        }
        fn fixture_account(seed: u8) -> AccountId {
            AccountId::of(fixture_key_pair(seed).public_key().clone())
        }
        fn sample_spec() -> MultisigSpec {
            let mut signatories = BTreeMap::new();
            signatories.insert(fixture_account(1), 1);
            signatories.insert(fixture_account(2), 1);
            MultisigSpec::new(
                signatories,
                NonZeroU16::new(2).expect("nonzero quorum"),
                NonZeroU64::new(DEFAULT_MULTISIG_TTL_MS).expect("nonzero ttl"),
            )
        }
        fn sample_instruction_box() -> InstructionBox {
            let domain: DomainId =
                DomainId::try_new("multisig", "universal").expect("valid domain");
            let multisig_account = fixture_account(3);
            let spec = sample_spec();
            let register = MultisigRegister::with_account(multisig_account, Some(domain), spec);
            InstructionBox::from(register)
        }
        #[test]
        fn multisig_instruction_batch_hash_matches_sdk_golden() {
            let instruction = InstructionBox::from(CustomInstruction::new(Json::new(())));
            let hash = HashOf::new(&vec![instruction]);
            assert_eq!(
                hash.as_ref(),
                &[
                    0x5f, 0x95, 0x7f, 0x67, 0xa4, 0x23, 0x6e, 0xb1, 0x6f, 0x9d, 0xf0,
                    0xd8, 0x11, 0x70, 0xf3, 0xa7, 0x06, 0x56, 0x94, 0x2b, 0x4e, 0x17,
                    0x1a, 0x20, 0x8c, 0x26, 0xde, 0x02, 0xe8, 0xe9, 0x9a, 0xcf,
                ],
                "HashOf<Vec<InstructionBox>> must match the Java/JS compact-v5 golden",
            );
        }
        #[test]
        fn fixture_account_uses_checked_seed_derivation() {
            let account = fixture_account(42);
            let expected = AccountId::of(fixture_key_pair(42).public_key().clone());
            assert_eq!(account, expected);
            assert!(KeyPair::try_from_seed(vec![0; 32], Algorithm::Ed25519).is_err());
        }
        #[test]
        fn try_from_instruction_box_roundtrip() {
            let instruction_box = sample_instruction_box();
            let decoded = MultisigInstructionBox::try_from(&instruction_box)
                .expect("decode multisig instruction");
            match decoded {
                MultisigInstructionBox::Register(register) => {
                    assert_eq!(register.spec.signatories.len(), 2);
                }
                _ => panic!("expected register variant"),
            }
        }
        #[test]
        fn multisig_register_json_includes_account_field() {
            let domain: DomainId =
                DomainId::try_new("multisig", "universal").expect("valid domain");
            let multisig_account = fixture_account(42);
            let spec = sample_spec();
            let register = MultisigRegister::with_account(multisig_account, Some(domain), spec);
            let rendered =
                norito::json::to_json(&register).expect("multisig register should serialize");
            assert!(
                rendered.contains("\"account\""),
                "account field missing from serialized json: {rendered}"
            );
            assert!(
                rendered.contains("\"home_domain\""),
                "home_domain field missing from serialized json: {rendered}"
            );
        }
        #[test]
        fn multisig_register_json_requires_account_field() {
            let spec = sample_spec();
            let spec_json = norito::json::to_json(&spec).expect("spec should serialize");
            let payload = format!(r#"{{"spec": {spec_json}}}"#);
            let err = norito::json::from_str::<MultisigRegister>(&payload)
                .expect_err("missing account should be rejected");
            let rendered = err.to_string();
            assert!(
                rendered.contains("account"),
                "missing account error should mention account field: {rendered}"
            );
        }
        #[test]
        fn multisig_register_from_spec_randomizes_controller() {
            let domain: DomainId =
                DomainId::try_new("non-derived", "universal").expect("valid domain");
            let spec = sample_spec();
            let first = MultisigRegister::from_spec(Some(domain.clone()), spec.clone())
                .expect("checked multisig controller account generation");
            let second = MultisigRegister::from_spec(Some(domain.clone()), spec.clone())
                .expect("checked multisig controller account generation");
            assert_eq!(
                first.home_domain.as_ref(),
                Some(&domain),
                "generated controller must carry the explicit home domain"
            );
            assert_ne!(
                first.account, second.account,
                "from_spec should randomize the controller id for each call"
            );
        }
        #[test]
        fn multisig_register_json_defaults_home_domain_to_none() {
            let account = fixture_account(7);
            let account_json = norito::json::to_json(&account).expect("account json");
            let spec = sample_spec();
            let spec_json = norito::json::to_json(&spec).expect("spec json");
            let payload = format!(r#"{{"account": {account_json}, "spec": {spec_json}}}"#);
            let register = norito::json::from_str::<MultisigRegister>(&payload)
                .expect("missing home_domain should default to none");
            assert_eq!(register.home_domain, None);
        }
        #[test]
        fn multisig_cancel_instruction_roundtrip_preserves_target_hash() {
            let multisig_account = fixture_account(11);
            let instructions_hash = HashOf::new(&vec![sample_instruction_box()]);
            let cancel = MultisigCancel::new(multisig_account.clone(), instructions_hash);
            let instruction_box = InstructionBox::from(cancel.clone());
            let decoded = MultisigInstructionBox::try_from(&instruction_box)
                .expect("decode multisig cancel instruction");
            match decoded {
                MultisigInstructionBox::Cancel(decoded_cancel) => {
                    assert_eq!(decoded_cancel.account, multisig_account);
                    assert_eq!(decoded_cancel.instructions_hash, cancel.instructions_hash);
                }
                _ => panic!("expected cancel variant"),
            }
        }
        #[test]
        fn multisig_invalidate_outstanding_instruction_roundtrips_exact_account() {
            let multisig_account = fixture_account(13);
            let invalidate = MultisigInvalidateOutstanding::new(multisig_account.clone());
            let instruction_box = InstructionBox::from(invalidate);
            let decoded = MultisigInstructionBox::try_from(&instruction_box)
                .expect("decode multisig invalidation instruction");
            match &decoded {
                MultisigInstructionBox::InvalidateOutstanding(decoded) => {
                    assert_eq!(decoded.account, multisig_account);
                }
                _ => panic!("expected invalidate-outstanding variant"),
            }
            let json = norito::json::to_json(&decoded)
                .expect("encode multisig invalidation instruction JSON");
            assert_eq!(
                norito::json::to_json_bounded(&decoded, json.len())
                    .expect("encode bounded multisig invalidation instruction JSON"),
                json
            );
            assert!(json.contains("\"InvalidateOutstanding\""));
            let decoded_json = norito::json::from_str::<MultisigInstructionBox>(&json)
                .expect("decode multisig invalidation instruction JSON");
            assert_eq!(decoded_json, decoded);
        }
        #[test]
        fn multisig_terminal_state_roundtrip_preserves_status() {
            let multisig_account = fixture_account(12);
            let instructions = vec![sample_instruction_box()];
            let instructions_hash = HashOf::new(&instructions);
            let proposal = MultisigProposalValue::new(
                instructions,
                1_700_000_000_000,
                1_700_000_060_000,
                BTreeSet::from([multisig_account.clone()]),
                None,
            );
            let terminal = MultisigProposalTerminalState::new(
                multisig_account.clone(),
                instructions_hash,
                proposal.clone(),
                MultisigProposalTerminalStatus::Canceled,
                1_700_000_010_000,
            );
            let bytes = norito::to_bytes(&terminal).expect("encode terminal state");
            let decoded = norito::decode_from_bytes::<MultisigProposalTerminalState>(&bytes)
                .expect("decode terminal state");
            assert_eq!(decoded.multisig_account_id, multisig_account);
            assert_eq!(decoded.instructions_hash, instructions_hash);
            assert_eq!(decoded.proposal, proposal);
            assert_eq!(decoded.status, MultisigProposalTerminalStatus::Canceled);
            assert_eq!(decoded.terminal_at_ms, 1_700_000_010_000);
        }
        #[test]
        fn multisig_terminal_execution_state_roundtrip_preserves_binding() {
            let multisig_account = fixture_account(13);
            let instructions = vec![sample_instruction_box()];
            let instructions_hash = HashOf::new(&instructions);
            let terminal = MultisigProposalTerminalState::new(
                multisig_account.clone(),
                instructions_hash,
                MultisigProposalValue::new(
                    instructions,
                    1_700_000_000_000,
                    1_700_000_060_000,
                    BTreeSet::new(),
                    None,
                ),
                MultisigProposalTerminalStatus::Finalized,
                1_700_000_010_000,
            );
            let execution = MultisigProposalTerminalExecutionStateV1::new(
                terminal.clone(),
                multisig_account.clone(),
                42,
                [0xabu8; 32],
            );
            let bytes = norito::to_bytes(&execution).expect("encode terminal execution state");
            let decoded =
                norito::decode_from_bytes::<MultisigProposalTerminalExecutionStateV1>(&bytes)
                    .expect("decode terminal execution state");
            assert_eq!(decoded.terminal, terminal);
            assert_eq!(decoded.entrypoint_account_id, multisig_account);
            assert_eq!(decoded.terminal_block_height, 42);
            assert_eq!(decoded.terminal_entrypoint_hash, [0xabu8; 32]);
        }
        #[test]
        fn multisig_approval_outcome_roundtrip_preserves_resolution_and_binding() {
            let entrypoint_account = fixture_account(14);
            let resolved_account = fixture_account(15);
            let instructions_hash = HashOf::new(&vec![sample_instruction_box()]);
            let outcome = MultisigApprovalOutcomeV1::new(
                entrypoint_account.clone(),
                resolved_account.clone(),
                instructions_hash,
                MultisigApprovalOutcomeStatusV1::Executed,
                43,
                [0xcdu8; 32],
            );
            let bytes = norito::to_bytes(&outcome).expect("encode approval outcome");
            let decoded = norito::decode_from_bytes::<MultisigApprovalOutcomeV1>(&bytes)
                .expect("decode approval outcome");
            assert_eq!(decoded.entrypoint_account_id, entrypoint_account);
            assert_eq!(decoded.resolved_multisig_account_id, resolved_account);
            assert_eq!(decoded.instructions_hash, instructions_hash);
            assert_eq!(decoded.status, MultisigApprovalOutcomeStatusV1::Executed);
            assert_eq!(decoded.block_height, 43);
            assert_eq!(decoded.entrypoint_hash, [0xcdu8; 32]);
        }
    }
}
