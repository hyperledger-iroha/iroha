//! Types for custom instructions

#[allow(unused_imports)]
use std::eprintln;
use std::{
    collections::BTreeMap,
    format,
    string::{String, ToString},
    vec::Vec,
};

use derive_more::{Constructor, From};
use iroha_data_model::{
    isi::{CustomInstruction, InstructionBox},
    prelude::{Json, *},
};
use iroha_schema::IntoSchema;

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
    use core::num::{NonZeroU16, NonZeroU64};
    #[allow(unused_imports)]
    use std::eprintln;
    use std::{borrow::ToOwned, collections::BTreeSet};

    use iroha_crypto::{HashOf, KeyPair};
    use norito::json::{self, JsonDeserialize, JsonSerialize, Value};

    use super::*;
    use crate::json_macros::{
        JsonDeserialize as DeriveJsonDeserialize, JsonSerialize as DeriveJsonSerialize,
    };

    /// Multisig-related instructions
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema, From)]
    pub enum MultisigInstructionBox {
        /// Register a multisig account, which is a prerequisite of multisig transactions
        Register(MultisigRegister),
        /// Propose a multisig transaction and initialize approvals with the proposer's one
        Propose(MultisigPropose),
        /// Approve a certain multisig transaction
        Approve(MultisigApprove),
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
            }
            out.push('}');
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
        Constructor,
        DeriveJsonSerialize,
    )]
    pub struct MultisigRegister {
        /// Account backing the multisig controller.
        ///
        /// Must live in the same domain as the signatories. The supplied id anchors the
        /// registration step, but the account is rekeyed to the canonical multisig controller
        /// derived from the spec after registration, so the key is never used for signing.
        pub account: AccountId,
        /// Specification of the multisig account
        pub spec: MultisigSpec,
    }

    impl MultisigRegister {
        /// Construct a multisig registration using an explicit account id.
        pub fn with_account(account: AccountId, spec: MultisigSpec) -> Self {
            Self { account, spec }
        }

        /// Construct a multisig registration using a freshly generated account id that lives in
        /// the signatory domain. The generated key is not meant for direct signing; it only
        /// anchors the registration step before the account is rekeyed to the canonical controller
        /// derived from the spec.
        pub fn from_spec(spec: MultisigSpec) -> Self {
            let domain = signatory_domain(&spec)
                .expect("multisig spec must include at least one signatory to derive the domain");
            let key_pair = KeyPair::random();
            let account = AccountId::new(domain, key_pair.public_key().clone());
            Self { account, spec }
        }
    }

    impl JsonDeserialize for MultisigRegister {
        fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
            let mut visitor = json::MapVisitor::new(parser)?;
            let mut account: Option<AccountId> = None;
            let mut spec: Option<MultisigSpec> = None;

            while let Some(key) = visitor.next_key()? {
                match key.as_str() {
                    "account" => {
                        let value = visitor.parse_value::<AccountId>()?;
                        account = Some(value);
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

            Ok(Self { account, spec })
        }
    }

    fn signatory_domain(spec: &MultisigSpec) -> Option<iroha_data_model::domain::DomainId> {
        spec.signatories
            .keys()
            .next()
            .map(|account| account.domain().clone())
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

    impl_custom_instruction!(
        MultisigInstructionBox,
        MultisigRegister | MultisigPropose | MultisigApprove
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

    /// Metadata value for a multisig account specification
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema, Constructor,
    )]
    #[cfg_attr(
        feature = "json",
        derive(crate::json_macros::FastJson, crate::json_macros::FastJsonWrite)
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
    #[cfg_attr(
        feature = "json",
        derive(crate::json_macros::FastJson, crate::json_macros::FastJsonWrite)
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
                                .map(iroha_data_model::account::ParsedAccountId::into_account_id)
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
        use std::collections::BTreeMap;

        use iroha_crypto::{Algorithm, KeyPair};

        use super::*;

        fn sample_instruction_box() -> InstructionBox {
            let domain: DomainId = "multisig".parse().expect("valid domain");
            let registrar = KeyPair::from_seed(vec![0; 32], Algorithm::Ed25519);
            let multisig_account = AccountId::new(domain.clone(), registrar.public_key().clone());
            let alice_key = KeyPair::from_seed(vec![1; 32], Algorithm::Ed25519);
            let bob_key = KeyPair::from_seed(vec![2; 32], Algorithm::Ed25519);
            let alice = AccountId::new(domain.clone(), alice_key.public_key().clone());
            let bob = AccountId::new(domain.clone(), bob_key.public_key().clone());
            let mut signatories = BTreeMap::new();
            signatories.insert(alice, 1);
            signatories.insert(bob, 1);
            let spec = MultisigSpec::new(
                signatories,
                NonZeroU16::new(2).expect("nonzero quorum"),
                NonZeroU64::new(DEFAULT_MULTISIG_TTL_MS).expect("nonzero ttl"),
            );
            let register = MultisigRegister::with_account(multisig_account, spec);
            InstructionBox::from(register)
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
            let domain: DomainId = "multisig".parse().expect("valid domain");
            let registrar = KeyPair::from_seed(vec![42; 32], Algorithm::Ed25519);
            let multisig_account = AccountId::new(domain.clone(), registrar.public_key().clone());
            let mut signatories = BTreeMap::new();
            signatories.insert(multisig_account.clone(), 1);
            let spec = MultisigSpec::new(
                signatories,
                NonZeroU16::new(1).expect("nonzero quorum"),
                NonZeroU64::new(DEFAULT_MULTISIG_TTL_MS).expect("nonzero ttl"),
            );
            let register = MultisigRegister::with_account(multisig_account, spec);
            let rendered =
                norito::json::to_json(&register).expect("multisig register should serialize");
            assert!(
                rendered.contains("\"account\""),
                "account field missing from serialized json: {rendered}"
            );
        }

        #[test]
        fn multisig_register_json_requires_account_field() {
            let domain: DomainId = "missing-account".parse().expect("valid domain");
            let registrar = KeyPair::from_seed(vec![7; 32], Algorithm::Ed25519);
            let mut signatories = BTreeMap::new();
            signatories.insert(
                AccountId::new(domain.clone(), registrar.public_key().clone()),
                1,
            );
            let spec = MultisigSpec::new(
                signatories,
                NonZeroU16::new(1).expect("nonzero quorum"),
                NonZeroU64::new(DEFAULT_MULTISIG_TTL_MS).expect("nonzero ttl"),
            );
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
            let domain: DomainId = "non-derived".parse().expect("valid domain");
            let signer = KeyPair::from_seed(vec![3; 32], Algorithm::Ed25519);
            let mut signatories = BTreeMap::new();
            signatories.insert(
                AccountId::new(domain.clone(), signer.public_key().clone()),
                1,
            );
            let spec = MultisigSpec::new(
                signatories,
                NonZeroU16::new(1).expect("nonzero quorum"),
                NonZeroU64::new(DEFAULT_MULTISIG_TTL_MS).expect("nonzero ttl"),
            );
            let first = MultisigRegister::from_spec(spec.clone());
            let second = MultisigRegister::from_spec(spec.clone());

            assert_eq!(
                first.account.domain(),
                &domain,
                "generated controller must stay in the signatory domain"
            );
            assert_ne!(
                first.account, second.account,
                "from_spec should randomize the controller id for each call"
            );
        }
    }
}
