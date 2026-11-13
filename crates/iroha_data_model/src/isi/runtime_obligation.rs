//! Register an obligation with the kernel and end it
//! (`crate::runtime::obligation`, `specs/live_runtime_upgrades.md` §7.2).
use super::*;
use crate::runtime::lifecycle::ObligationKindV1;
isi! {
 /// Register the obligation of an owner with the kernel, or replace it.
 ///
 /// The owner is the transaction authority or, when `contract` names one, the subject account
 /// of that contract, which the authority must control the lifecycle of. `terminal_effects` are
 /// the exact instructions that complete the obligation. The kernel captures their completion
 /// closure from the selection the root executes and, while the obligation is live, lets the
 /// owner perform each of them once whatever the root has since restricted short of disabling,
 /// and lets no change set alter that closure. An owner holds one obligation: registering again
 /// replaces it. Registering is new work: it is refused while the interface or implementation
 /// of a terminal effect is restricted.
 #[derive(crate::DeriveJsonSerialize,crate::DeriveJsonDeserialize)]
 #[norito(deny_unknown_fields)]
 #[norito_schema(name="iroha_data_model::isi::runtime_obligation::RegisterRuntimeObligationV1")]
 pub struct RegisterRuntimeObligationV1 {
  /// The contract whose subject account owns the obligation, or `None` for the authority.
  #[norito(required)]
  pub contract: Option<crate::smart_contract::ContractAddress>,
  /// What kind of work the obligation is.
  pub kind: ObligationKindV1,
  /// The exact instructions that complete the obligation, each performed at most once.
  pub terminal_effects: Vec<InstructionBox>,
  /// Committed time from which the obligation is no longer live.
  pub expires_ms: u64,
 }
}
impl crate::seal::Instruction for RegisterRuntimeObligationV1 {}
impl RegisterRuntimeObligationV1 {
    /// Stable wire identity.
    pub const WIRE_ID: &'static str = "iroha.runtime.obligation.register.v1";
}
isi! {
 /// End the obligation `owner` holds and release its completion closure.
 ///
 /// The owner ends its own obligation. The account that controls the lifecycle of the contract
 /// that owns an obligation ends it, because the contract itself cannot. Anyone ends an
 /// obligation that is no longer live: it has expired or performed every terminal effect.
 #[derive(crate::DeriveJsonSerialize,crate::DeriveJsonDeserialize)]
 #[norito(deny_unknown_fields)]
 #[norito_schema(name="iroha_data_model::isi::runtime_obligation::CompleteRuntimeObligationV1")]
 pub struct CompleteRuntimeObligationV1 {
  /// The owner of the obligation.
  pub owner: crate::account::AccountId,
 }
}
impl crate::seal::Instruction for CompleteRuntimeObligationV1 {}
impl CompleteRuntimeObligationV1 {
    /// Stable wire identity.
    pub const WIRE_ID: &'static str = "iroha.runtime.obligation.complete.v1";
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, KeyPair};

    use super::*;

    #[test]
    fn obligation_instructions_roundtrip_through_their_wire_ids() {
        let owner = crate::account::AccountId::new(
            KeyPair::from_seed(vec![1; 32], Algorithm::Ed25519)
                .public_key()
                .clone(),
        );
        let complete = CompleteRuntimeObligationV1 {
            owner: owner.clone(),
        };
        let register = RegisterRuntimeObligationV1 {
            contract: None,
            kind: ObligationKindV1::Escrow,
            terminal_effects: vec![InstructionBox::from(Log::new(
                crate::Level::INFO,
                "released".to_owned(),
            ))],
            expires_ms: 99,
        };
        for (instruction, wire_id) in [
            (
                InstructionBox::from(register.clone()),
                RegisterRuntimeObligationV1::WIRE_ID,
            ),
            (
                InstructionBox::from(complete.clone()),
                CompleteRuntimeObligationV1::WIRE_ID,
            ),
        ] {
            assert_eq!(instruction_wire_id(&instruction), Some(wire_id));
            let bytes = norito::encode_canonical(&instruction).unwrap();
            assert_eq!(
                norito::decode_canonical::<InstructionBox>(&bytes).unwrap(),
                instruction
            );
        }
        let bytes = norito::encode_canonical(&register).unwrap();
        assert_eq!(
            norito::decode_canonical::<RegisterRuntimeObligationV1>(&bytes).unwrap(),
            register
        );
        let json = norito::json::to_json(&complete).unwrap();
        assert_eq!(
            norito::json::from_str::<CompleteRuntimeObligationV1>(&json).unwrap(),
            complete
        );
    }
}

isi! {
    /// Register the authenticated executing contract's obligation through its named host profile.
    ///
    /// The kernel derives the contract address and subject from the original invocation's
    /// opaque authorization context. A signed instruction without that context is refused,
    /// even when its authority names the subject. The frozen account-controller registration
    /// instruction remains unchanged. Completion effects and expiry obey the same bounded
    /// registry contract; registering always requires ordinary admission to the canonical root.
    #[derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)]
    #[norito(deny_unknown_fields)]
    #[norito_schema(name = "iroha_data_model::isi::runtime_obligation::RegisterContractRuntimeObligationV1")]
    pub struct RegisterContractRuntimeObligationV1 {
        /// What kind of work the contract leaves outstanding.
        pub kind: ObligationKindV1,
        /// Exact terminal instructions, each performed at most once by this contract's subject.
        pub terminal_effects: Vec<InstructionBox>,
        /// Committed time from which the obligation is no longer live.
        pub expires_ms: u64,
    }
}
impl crate::seal::Instruction for RegisterContractRuntimeObligationV1 {}
impl RegisterContractRuntimeObligationV1 {
    /// Stable wire identity of the independently activated contract kernel call.
    pub const WIRE_ID: &'static str = "iroha.runtime.obligation.contract.register.v1";
}

#[cfg(test)]
mod contract_registration_tests {
    use super::*;
    use crate::{
        Level,
        isi::{InstructionBox, Log},
    };
    #[test]
    fn contract_registration_roundtrip_preserves_exact_terminal_instruction() {
        let instruction = RegisterContractRuntimeObligationV1 {
            kind: ObligationKindV1::Escrow,
            terminal_effects: vec![Log::new(Level::INFO, "terminal".to_owned()).into()],
            expires_ms: 42,
        };
        let envelope = InstructionBox::from(instruction.clone());
        let bytes = norito::to_bytes(&envelope).unwrap();
        let restored: InstructionBox = norito::decode_from_bytes(&bytes).unwrap();
        assert_eq!(
            restored
                .as_any()
                .downcast_ref::<RegisterContractRuntimeObligationV1>(),
            Some(&instruction)
        );
        assert_eq!(norito::to_bytes(&restored).unwrap(), bytes);
        let json = norito::json::to_json(&instruction).unwrap();
        let decoded: RegisterContractRuntimeObligationV1 = norito::json::from_str(&json).unwrap();
        assert_eq!(decoded, instruction);
    }
}
