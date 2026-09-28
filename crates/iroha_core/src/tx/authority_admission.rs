//! Narrow authority-shape exceptions used during transaction admission.

use iroha_data_model::{
    Registrable,
    account::{Account, AccountId},
    isi::InstructionBox,
    transaction::Executable,
};
use iroha_executor_data_model::isi::multisig::MultisigInstructionBox;

fn instruction_self_registers_authority(
    instruction: &InstructionBox,
    authority: &AccountId,
) -> bool {
    let maybe_registration = instruction
        .as_any()
        .downcast_ref::<iroha_data_model::isi::Register<Account>>()
        .map(|register| register.object())
        .or_else(|| {
            instruction
                .as_any()
                .downcast_ref::<iroha_data_model::isi::RegisterBox>()
                .and_then(|register| match register {
                    iroha_data_model::isi::RegisterBox::Account(register) => {
                        Some(register.object())
                    }
                    _ => None,
                })
        });
    let Some(registration) = maybe_registration else {
        return false;
    };
    registration.clone().build(authority).id == *authority
}

/// Return whether the executable's first instruction registers its exact authority.
///
/// Self-registering transactions are the only single-signature transactions that may enter
/// admission before their authority exists in world state. Keeping this recognition in Core lets
/// pre-admission services, such as fee quoting, apply the same instruction-shape rule.
#[must_use]
pub fn executable_self_registers_authority(executable: &Executable, authority: &AccountId) -> bool {
    match executable {
        Executable::Instructions(instructions) => {
            let Some((first, _rest)) = instructions.split_first() else {
                return false;
            };
            instruction_self_registers_authority(first, authority)
        }
        Executable::ContractCall(_)
        | Executable::Batch(_)
        | Executable::IvmProved(_)
        | Executable::Ivm(_) => false,
    }
}

/// Return whether admission may accept an authority that is absent from world state.
///
/// This includes exact first-instruction account self-registration, the existing multisig
/// proposal envelope path, whose authorisation is established from multisig membership rather
/// than a materialised authority account, and the SCCP bridge-key registration from the new
/// key's own account (`specs/sccp.md` §4.2.3).
#[must_use]
pub fn allows_unregistered_authority(executable: &Executable, authority: &AccountId) -> bool {
    executable_self_registers_authority(executable, authority)
        || matches!(
            executable,
            Executable::Instructions(instructions)
                if instructions_allow_multisig_envelope_authority(instructions)
        )
        || crate::smartcontracts::isi::sccp::admission::allows_unregistered_authority(
            executable, authority,
        )
}

pub(crate) fn instructions_allow_multisig_envelope_authority(
    instructions: &[InstructionBox],
) -> bool {
    !instructions.is_empty()
        && instructions.iter().all(|instruction| {
            matches!(
                MultisigInstructionBox::try_from(instruction),
                Ok(MultisigInstructionBox::Propose(_))
                    | Ok(MultisigInstructionBox::Approve(_))
                    | Ok(MultisigInstructionBox::Cancel(_))
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, KeyPair};

    fn account(seed: u8) -> AccountId {
        let key_pair =
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).expect("deterministic seed");
        AccountId::new(key_pair.public_key().clone())
    }

    #[test]
    fn only_the_listed_shapes_admit_an_absent_authority() {
        let authority = account(1);
        let self_registration = Executable::Instructions(
            vec![InstructionBox::from(
                iroha_data_model::isi::Register::account(Account::new(authority.clone())),
            )]
            .into(),
        );
        assert!(allows_unregistered_authority(
            &self_registration,
            &authority
        ));
        assert!(!allows_unregistered_authority(
            &self_registration,
            &account(2)
        ));
        let log = Executable::Instructions(
            vec![InstructionBox::from(iroha_data_model::isi::Log::new(
                iroha_data_model::Level::INFO,
                "not a registration".to_owned(),
            ))]
            .into(),
        );
        assert!(!allows_unregistered_authority(&log, &authority));
    }
}
