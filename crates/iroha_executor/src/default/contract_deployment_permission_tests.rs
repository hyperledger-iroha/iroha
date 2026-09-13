//! Contract deployment authorization and native instruction dispatch tests.
use super::*;
use crate::{Iroha, prelude};
use iroha_crypto::{Algorithm, Hash, KeyPair};
use iroha_data_model::smart_contract::manifest::ContractManifest;
use iroha_model_base::topology::DataSpaceId;
use std::num::NonZeroU64;
fn account(seed: u8) -> AccountId {
    let key_pair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
        .expect("non-zero deterministic key seed");
    AccountId::new(key_pair.public_key().clone())
}
fn manifest() -> ContractManifest {
    ContractManifest {
        seiyaku_name: None,
        code_hash: Some(Hash::new(b"executor bootstrap manifest code")),
        abi_hash: Some(Hash::new(b"executor bootstrap manifest ABI")),
        compiler_fingerprint: None,
        features_bitmap: None,
        access_set_hints: None,
        entrypoints: None,
        states: None,
        error_types: None,
        kotoba: None,
        provenance: None,
    }
}
#[derive(Debug)]
struct TestExecutor {
    host: Iroha,
    context: prelude::Context,
    verdict: crate::data_model::executor::Result<(), ValidationFail>,
}
impl TestExecutor {
    fn non_genesis(authority: AccountId) -> Self {
        Self {
            host: Iroha,
            context: prelude::Context {
                authority,
                curr_block: BlockHeader::new(
                    NonZeroU64::new(2).expect("non-zero block height"),
                    None,
                    None,
                    None,
                    0,
                    0,
                ),
            },
            verdict: Ok(()),
        }
    }
}
impl Execute for TestExecutor {
    fn host(&self) -> &Iroha {
        &self.host
    }
    fn context(&self) -> &prelude::Context {
        &self.context
    }
    fn context_mut(&mut self) -> &mut prelude::Context {
        &mut self.context
    }
    fn verdict(&self) -> &crate::data_model::executor::Result<(), ValidationFail> {
        &self.verdict
    }
    fn deny(&mut self, reason: ValidationFail) {
        self.verdict = Err(reason);
    }
}
impl Visit for TestExecutor {
    fn visit_instruction(&mut self, instruction: &InstructionBox) {
        super::visit_instruction(self, instruction);
    }

    fn visit_register_account(&mut self, instruction: &Register<Account>) {
        super::account::visit_register_account(self, instruction);
    }

    fn visit_grant_account_permission(&mut self, instruction: &Grant<Permission, Account>) {
        super::permission::visit_grant_account_permission(self, instruction);
    }
}
#[test]
fn contract_lifecycle_instructions_reach_core_dispatch() {
    let authority = account(1);
    let code_hash = Hash::new(b"executor lifecycle dispatch code");
    let contract_address = ContractAddress::derive(
        &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
            .parse()
            .expect("canonical test network id"),
        &authority,
        7,
        DataSpaceId::UNIVERSAL,
    )
    .expect("contract address");
    let instructions: Vec<InstructionBox> = vec![
        RegisterSmartContractCode {
            manifest: manifest(),
        }
        .into(),
        DeactivateContractInstance {
            contract_address: contract_address.clone(),
            expected_revision: 1,
            reason: Some("dispatch fixture".to_owned()),
        }
        .into(),
        ActivateContractInstance {
            contract_address: contract_address.clone(),
            expected_revision: 1,
            code_hash,
        }
        .into(),
        CommitContractDeployment {
            expected_deploy_nonce: 7,
            contract_address: contract_address.clone(),
            code_hash,
            contract_alias: "payments::universal".parse().expect("contract alias"),
            lease_expiry_ms: None,
            expected_previous_contract_address: None,
        }
        .into(),
        RegisterSmartContractBytes {
            code_hash,
            code: vec![0x01],
        }
        .into(),
        UploadSmartContractCodeChunk {
            code_hash,
            total_size: 1,
            chunk_index: 0,
            chunk_count: 1,
            chunk: vec![0x01],
        }
        .into(),
        FinalizeSmartContractCodeUpload {
            code_hash,
            total_size: 1,
            chunk_count: 1,
        }
        .into(),
        CancelSmartContractCodeUpload { code_hash }.into(),
        RemoveSmartContractBytes {
            code_hash,
            reason: Some("dispatch fixture".to_owned()),
        }
        .into(),
        SetContractAlias::clear(contract_address).into(),
    ];
    for instruction in instructions {
        let mut executor = TestExecutor::non_genesis(authority.clone());
        visit_instruction(&mut executor, &instruction);
        assert!(
            executor.verdict().is_ok(),
            "known lifecycle instruction must reach Core dispatch: {instruction:?}"
        );
    }
}

#[test]
fn sponsored_registration_uses_ordinary_authorized_grant() {
    use iroha_executor_data_model::permission::smart_contract::{
        CanManageSmartContractCodeRegistrars, CanRegisterSmartContractCode,
    };
    let authority = account(1);
    let builder = account(2);
    let key_pair = KeyPair::try_from_seed(vec![1; 32], Algorithm::Ed25519).expect("key");
    let instructions: Vec<InstructionBox> = vec![
        Register::account(Account::new(builder.clone())).into(),
        Grant::account_permission(CanRegisterSmartContractCode, builder).into(),
    ];
    let transaction = TransactionBuilder::new(
        "hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
            .parse()
            .expect("network"),
        authority.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions(instructions)
    .sign(key_pair.private_key());
    for (held, expected) in [
        (Permission::from(CanRegisterSmartContractCode), false),
        (Permission::from(CanManageSmartContractCodeRegistrars), true),
    ] {
        let old = crate::permission::test_override::replace_permissions(vec![held]);
        let mut executor = TestExecutor::non_genesis(authority.clone());
        visit_transaction(&mut executor, &transaction);
        let verdict = executor.verdict().clone();
        crate::permission::test_override::replace_permissions(old);
        assert_eq!(verdict.is_ok(), expected, "{verdict:?}");
    }
}
#[test]
fn upload_prefix_cannot_self_grant_registrar_permission() {
    use iroha_executor_data_model::permission::smart_contract::CanRegisterSmartContractCode;
    let authority = account(1);
    let key_pair = KeyPair::try_from_seed(vec![1; 32], Algorithm::Ed25519).expect("key");
    let instructions: Vec<InstructionBox> = vec![
        Register::account(Account::new(authority.clone())).into(),
        Grant::account_permission(CanRegisterSmartContractCode, authority.clone()).into(),
        UploadSmartContractCodeChunk {
            code_hash: Hash::new(b"no special bootstrap permission"),
            total_size: 1,
            chunk_index: 0,
            chunk_count: 1,
            chunk: vec![1],
        }
        .into(),
    ];
    let transaction = TransactionBuilder::new(
        "hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
            .parse()
            .expect("network"),
        authority.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions(instructions)
    .sign(key_pair.private_key());
    let old = crate::permission::test_override::replace_permissions(vec![
        CanRegisterSmartContractCode.into(),
    ]);
    let mut executor = TestExecutor::non_genesis(authority);
    visit_transaction(&mut executor, &transaction);
    let verdict = executor.verdict().clone();
    crate::permission::test_override::replace_permissions(old);
    assert!(matches!(verdict, Err(ValidationFail::NotPermitted(message))
        if message.contains("CanManageSmartContractCodeRegistrars")));
}
