//! Explicit current-binding consent for a new deferred execution occurrence.

use super::*;

isi! {
    /// Register a trigger while explicitly consenting to current bindings at each firing.
    /// Ordinary trigger registration captures its resolved semantics by default once the
    /// deferred-consent service is selected. This distinct instruction makes opting out part
    /// of the signed registration and never modifies consent of an existing incarnation.
    #[derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)]
    #[norito(deny_unknown_fields)]
    #[norito_schema(name = "iroha_data_model::isi::runtime_deferred::RegisterCurrentBindingTriggerV1")]
    pub struct RegisterCurrentBindingTriggerV1 {
        /// The new trigger, with the ordinary registration authorization and invariants.
        pub trigger: crate::trigger::Trigger,
    }
}
impl crate::seal::Instruction for RegisterCurrentBindingTriggerV1 {}
impl RegisterCurrentBindingTriggerV1 {
    /// Stable wire identity.
    pub const WIRE_ID: &'static str = "iroha.runtime.consent.trigger.current.v1";
}

isi! {
    /// Propose a new multisig occurrence with explicit consent to its execution-time bindings.
    /// This preserves the frozen proposal and approval layouts. The kernel stores consent
    /// separately; a fresh occurrence nonce is required just as for ordinary proposals.
    #[derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)]
    #[norito(deny_unknown_fields)]
    #[norito_schema(name = "iroha_data_model::isi::runtime_deferred::ProposeCurrentBindingMultisigV1")]
    pub struct ProposeCurrentBindingMultisigV1 {
        /// Multisig account whose signatories approve the instructions.
        pub account: crate::account::AccountId,
        /// Nonzero occurrence nonce, interpreted by the native multisig implementation.
        pub proposal_nonce: [u8; 32],
        /// Exact instructions the signatories approve.
        pub instructions: Vec<InstructionBox>,
        /// Optional lifetime, subject to the account's existing maximum.
        #[norito(required)]
        pub transaction_ttl_ms: Option<core::num::NonZeroU64>,
    }
}
impl crate::seal::Instruction for ProposeCurrentBindingMultisigV1 {}
impl ProposeCurrentBindingMultisigV1 {
    /// Stable wire identity.
    pub const WIRE_ID: &'static str = "iroha.runtime.consent.multisig.current.v1";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_current_binding_registrations_roundtrip_without_changing_existing_layouts() {
        let owner = crate::account::AccountId::new(
            iroha_crypto::KeyPair::from_seed(vec![7; 32], iroha_crypto::Algorithm::Ed25519)
                .public_key()
                .clone(),
        );
        let id: crate::trigger::TriggerId = "current_bindings".parse().unwrap();
        let action = crate::trigger::action::Action::new(
            Vec::<InstructionBox>::new(),
            crate::trigger::action::Repeats::Exactly(1),
            owner.clone(),
            crate::events::execute_trigger::ExecuteTriggerEventFilter::new()
                .for_trigger(id.clone())
                .under_authority(owner.clone()),
        )
        .unwrap();
        let trigger = RegisterCurrentBindingTriggerV1 {
            trigger: crate::trigger::Trigger::new(id, action),
        };
        let multisig = ProposeCurrentBindingMultisigV1 {
            account: owner,
            proposal_nonce: [1; 32],
            instructions: Vec::new(),
            transaction_ttl_ms: None,
        };
        for (value, wire) in [
            (
                InstructionBox::from(trigger),
                RegisterCurrentBindingTriggerV1::WIRE_ID,
            ),
            (
                InstructionBox::from(multisig.clone()),
                ProposeCurrentBindingMultisigV1::WIRE_ID,
            ),
        ] {
            assert_eq!(instruction_wire_id(&value), Some(wire));
            assert_eq!(
                norito::decode_canonical::<InstructionBox>(
                    &norito::encode_canonical(&value).unwrap()
                )
                .unwrap(),
                value
            );
        }
        let json = norito::json::to_json(&multisig).unwrap();
        assert_eq!(
            norito::json::from_str::<ProposeCurrentBindingMultisigV1>(&json).unwrap(),
            multisig
        );
    }
}
