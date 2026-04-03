//! Protected namespace admission gate test for IVM deploys.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![allow(clippy::too_many_lines, clippy::items_after_statements)]

use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::Execute,
    state::{State, World},
};
use iroha_data_model::{nexus::DataSpaceId, smart_contract::manifest::ContractManifest};
use iroha_primitives::json::Json;

const TEST_GAS_LIMIT: u64 = 1_000_000;

fn insert_gas_limit(md: &mut iroha_data_model::metadata::Metadata) {
    let key: iroha_data_model::name::Name = "gas_limit".parse().expect("gas_limit key");
    md.insert(key, Json::new(TEST_GAS_LIMIT));
}

fn compute_proposal_id(
    contract_address: &iroha_data_model::smart_contract::ContractAddress,
    code_hex: &str,
    abi_hex: &str,
) -> [u8; 32] {
    use core::convert::TryInto;

    use iroha_crypto::blake2::{Blake2b512, Digest as _};
    let contract_address = contract_address.as_ref();
    let code = hex::decode(code_hex).expect("valid code hex");
    let abi = hex::decode(abi_hex).expect("valid abi hex");
    let contract_address_len: u32 = contract_address
        .len()
        .try_into()
        .expect("contract address length fits u32");
    let mut input = Vec::with_capacity(
        b"iroha:gov:proposal:v2|".len()
            + core::mem::size_of::<u32>()
            + contract_address.len()
            + code.len()
            + abi.len(),
    );
    input.extend_from_slice(b"iroha:gov:proposal:v2|");
    input.extend_from_slice(&contract_address_len.to_le_bytes());
    input.extend_from_slice(contract_address.as_bytes());
    input.extend_from_slice(&code);
    input.extend_from_slice(&abi);
    let digest = Blake2b512::digest(&input);
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest[..32]);
    out
}

fn sample_contract_address(
    authority: &iroha_data_model::account::AccountId,
) -> iroha_data_model::smart_contract::ContractAddress {
    iroha_data_model::smart_contract::ContractAddress::derive(0, authority, 0, DataSpaceId::new(0))
        .expect("contract address")
}

#[test]
fn protected_namespace_requires_enacted_proposal() {
    use std::str::FromStr;

    use iroha_data_model::{
        isi::governance::{EnactReferendum, ProposeDeployContract},
        permission::Permission,
        prelude::{Grant, *},
        transaction::{Executable, TransactionBuilder},
    };
    use iroha_executor_data_model::permission::governance::{
        CanEnactGovernance, CanProposeContractDeployment,
    };
    use nonzero_ext::nonzero;

    // Build minimal world with one authority
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let kp = iroha_crypto::KeyPair::random();
    let (pk, sk) = kp.clone().into_parts();
    let domain_id: DomainId = DomainId::try_new("apps", "universal").unwrap();
    let authority = AccountId::of(pk);
    let domain = Domain::new(domain_id.clone()).build(&authority);
    let account = Account::new(authority.clone()).build(&authority);
    let world = World::with([domain], [account], std::iter::empty::<AssetDefinition>());
    let state = State::new_for_testing(world, kura, query);

    // Set custom parameter gov_protected_namespaces = ["apps"]
    let header1 = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block1 = state.block(header1);
    let mut stx1 = block1.transaction();
    let id = iroha_data_model::parameter::CustomParameterId(
        iroha_data_model::name::Name::from_str("gov_protected_namespaces").unwrap(),
    );
    let protected_namespaces = norito::json::array(["apps"]).expect("serialize protected set");
    let payload = iroha_primitives::json::Json::from(protected_namespaces);
    let custom = iroha_data_model::parameter::CustomParameter::new(id, payload);
    let set = iroha_data_model::isi::SetParameter::new(Parameter::Custom(custom));
    set.execute(&authority, &mut stx1).expect("set param");
    stx1.apply();
    block1.commit().unwrap();

    // Prepare minimal IVM program
    let prog = {
        let mut code = Vec::new();
        code.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
        let meta = ivm::ProgramMetadata {
            version_major: 1,
            version_minor: 0,
            mode: 0,
            vector_length: 0,
            max_cycles: 1,
            abi_version: 1,
        };
        let mut out = meta.encode();
        out.extend_from_slice(&code);
        out
    };
    let parsed = ivm::ProgramMetadata::parse(&prog).unwrap();
    let code_hash = iroha_crypto::Hash::new(&prog[parsed.header_len..]);
    let abi_hash =
        iroha_crypto::Hash::prehashed(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1));

    let contract_address = sample_contract_address(&authority);

    // Build tx with metadata for protected namespace
    let mut md = iroha_data_model::metadata::Metadata::default();
    md.insert(
        "gov_contract_address".parse().unwrap(),
        iroha_primitives::json::Json::new(contract_address.to_string()),
    );
    insert_gas_limit(&mut md);
    let chain: ChainId = "chain".parse().unwrap();
    let tx = TransactionBuilder::new(chain.clone(), authority.clone())
        .with_executable(Executable::Ivm(IvmBytecode::from_compiled(prog.clone())))
        .with_metadata(md)
        .sign(&sk);

    let header2 = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
    let mut block2 = state.block(header2);
    let mut ivm_cache = iroha_core::smartcontracts::ivm::cache::IvmCache::new();
    use std::borrow::Cow;
    let accepted = iroha_core::tx::AcceptedTransaction::new_unchecked(Cow::Owned(tx));
    let (_h, res) = block2.validate_transaction(accepted, &mut ivm_cache);
    assert!(res.is_err(), "should reject without enacted proposal");
    drop(block2);

    // Enact a matching proposal via public instructions
    let header3 = BlockHeader::new(nonzero!(3_u64), None, None, None, 0, 0);
    let mut block3 = state.block(header3);
    let mut stx3 = block3.transaction();
    // Grant propose/enact
    let gp: Permission = CanProposeContractDeployment {
        contract_address: contract_address.clone(),
    }
    .into();
    Grant::account_permission(gp, authority.clone())
        .execute(&authority, &mut stx3)
        .expect("grant propose");
    let ge: Permission = CanEnactGovernance.into();
    Grant::account_permission(ge, authority.clone())
        .execute(&authority, &mut stx3)
        .expect("grant enact");
    let want_code_hex = hex::encode(<[u8; 32]>::from(code_hash));
    let want_abi_hex = hex::encode(<[u8; 32]>::from(abi_hash));
    let mut code_arr = [0u8; 32];
    code_arr.copy_from_slice(
        &hex::decode(&want_code_hex).expect("code hash hex should decode to 32 bytes"),
    );
    let mut abi_arr = [0u8; 32];
    abi_arr.copy_from_slice(
        &hex::decode(&want_abi_hex).expect("abi hash hex should decode to 32 bytes"),
    );
    let manifest_provenance = ContractManifest {
        code_hash: Some(iroha_crypto::Hash::prehashed(code_arr)),
        abi_hash: Some(iroha_crypto::Hash::prehashed(abi_arr)),
        compiler_fingerprint: None,
        features_bitmap: None,
        access_set_hints: None,
        entrypoints: None,
        kotoba: None,
        provenance: None,
    }
    .signed(&iroha_crypto::KeyPair::random())
    .provenance
    .expect("manifest should be signed");
    ProposeDeployContract {
        contract_address: contract_address.clone(),
        code_hash_hex: want_code_hex.clone(),
        abi_hash_hex: want_abi_hex.clone(),
        abi_version: "1".into(),
        window: None,
        mode: None,
        manifest_provenance: Some(manifest_provenance),
    }
    .execute(&authority, &mut stx3)
    .expect("propose");
    // Recompute the proposal id like in core and enact it
    let pid = compute_proposal_id(&contract_address, &want_code_hex, &want_abi_hex);
    stx3.world
        .governance_proposals_mut()
        .get_mut(&pid)
        .expect("proposal exists")
        .status = iroha_core::state::GovernanceProposalStatus::Approved;
    EnactReferendum {
        referendum_id: pid,
        preimage_hash: [0u8; 32],
        at_window: iroha_data_model::governance::types::AtWindow { lower: 2, upper: 2 },
    }
    .execute(&authority, &mut stx3)
    .expect("enact");
    stx3.apply();
    block3.commit().unwrap();

    // Retry with same tx; should accept now
    let header4 = BlockHeader::new(nonzero!(4_u64), None, None, None, 0, 0);
    let mut block4 = state.block(header4);
    let tx2 = TransactionBuilder::new(chain, authority)
        .with_executable(Executable::Ivm(IvmBytecode::from_compiled(prog)))
        .with_metadata({
            let mut md = iroha_data_model::metadata::Metadata::default();
            md.insert(
                "gov_contract_address".parse().unwrap(),
                iroha_primitives::json::Json::new(contract_address.to_string()),
            );
            insert_gas_limit(&mut md);
            md
        })
        .sign(&sk);
    let mut ivm_cache = iroha_core::smartcontracts::ivm::cache::IvmCache::new();
    let accepted2 = iroha_core::tx::AcceptedTransaction::new_unchecked(Cow::Owned(tx2));
    let (_h2, res2) = block4.validate_transaction(accepted2, &mut ivm_cache);
    assert!(res2.is_ok(), "should accept with enacted proposal");
}
