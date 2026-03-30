#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Torii handler test for zk vote tally convenience endpoint.
#![cfg(all(
    feature = "halo2-dev-tests",
    any(feature = "zk-halo2", feature = "zk-halo2-ipa")
))]
use std::{sync::Arc, time::Duration};

use axum::{extract::State, response::IntoResponse};
use http_body_util::BodyExt as _;
use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State as CoreState, World, WorldReadOnly as _},
};
use iroha_data_model::prelude::*;
use iroha_data_model::{isi::Grant, isi::verifying_keys, permission::Permission};
use iroha_primitives::json::Json;
use iroha_torii::{NoritoJson, ZkVoteGetTallyRequestDto, handle_v1_zk_vote_tally};
use nonzero_ext::nonzero;

#[path = "../../iroha_core/tests/zk_testkit.rs"]
mod zk_testkit;

const ACCOUNT_SIGNATORY: &str =
    "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03";

#[tokio::test]
async fn vote_tally_handler_returns_finalized_tally() {
    // Build minimal state
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let mut core_state = CoreState::new_for_testing(World::new(), kura, query);
    core_state.zk.halo2.enabled = true;
    core_state.zk.verify_timeout = Duration::ZERO;
    let state = Arc::new(core_state);

    // Seed one finalized election via ISIs
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    let eid = "election-alpha".to_string();
    let owner_domain: DomainId = "zkd".parse().expect("domain");
    let owner = AccountId::new(
        &owner,
        InstructionBox::from(Register::domain(Domain::new(owner_domain))),
    )
    .expect("register owner domain");
    stx.world
        .executor()
        .clone()
        .execute_instruction(
            &mut stx,
            &owner,
            InstructionBox::from(Register::account(
                NewAccount::new(owner.clone()),
            )),
        )
        .expect("register owner account");
    let bundle = zk_testkit::add2inst_public_bundle(7, 2);
    let backend = bundle.backend;
    let vk_id = bundle.vk_id.clone();
    let proof_box =
        iroha_data_model::proof::ProofBox::new(backend.into(), bundle.proof_bytes.clone());
    let vk_inline = bundle
        .vk_record
        .key
        .clone()
        .expect("vote tally bundle must include inline VK");
    stx.world
        .executor()
        .clone()
        .execute_instruction(
            &mut stx,
            &owner,
            InstructionBox::from(Grant::account_permission(
                Permission::new(
                    "CanManageParliament".parse().expect("permission id"),
                    Json::new(()),
                ),
                owner.clone(),
            )),
        )
        .expect("grant CanManageParliament");
    stx.world
        .executor()
        .clone()
        .execute_instruction(
            &mut stx,
            &owner,
            InstructionBox::from(Grant::account_permission(
                Permission::new(
                    "CanEnactGovernance".parse().expect("permission id"),
                    Json::new(()),
                ),
                owner.clone(),
            )),
        )
        .expect("grant CanEnactGovernance");
    let report = iroha_core::zk::verify_backend_with_timing(backend, &proof_box, Some(&vk_inline));
    assert!(report.ok, "vote tally proof must verify: {report:?}");
    stx.world
        .executor()
        .clone()
        .execute_instruction(
            &mut stx,
            &owner,
            InstructionBox::from(Grant::account_permission(
                Permission::new(
                    "CanManageVerifyingKeys"
                        .parse()
                        .expect("manage vk permission id"),
                    Json::new(()),
                ),
                owner.clone(),
            )),
        )
        .expect("grant CanManageVerifyingKeys");
    stx.world
        .executor()
        .clone()
        .execute_instruction(
            &mut stx,
            &owner,
            verifying_keys::RegisterVerifyingKey {
                id: vk_id.clone(),
                record: bundle.vk_record.clone(),
            }
            .into(),
        )
        .expect("register vote/tally verifying key");
    let create = iroha_data_model::isi::zk::CreateElection {
        election_id: eid.clone(),
        options: 2,
        eligible_root: [0u8; 32],
        start_ts: 0,
        end_ts: 0,
        vk_ballot: vk_id.clone(),
        vk_tally: vk_id.clone(),
        domain_tag: "ballot-domain".to_string(),
    };
    stx.world
        .executor()
        .clone()
        .execute_instruction(&mut stx, &owner, InstructionBox::from(create))
        .unwrap();
    let finalize = iroha_data_model::isi::zk::FinalizeElection {
        election_id: eid.clone(),
        tally: vec![7, 2],
        tally_proof: iroha_data_model::proof::ProofAttachment::new_inline(
            backend.into(),
            proof_box,
            vk_inline,
        ),
    };
    stx.world
        .executor()
        .clone()
        .execute_instruction(&mut stx, &owner, InstructionBox::from(finalize))
        .unwrap();
    stx.apply();
    block.commit().expect("commit block");

    // Call Torii handler directly
    let req = ZkVoteGetTallyRequestDto {
        election_id: eid.clone(),
    };
    let resp = handle_v1_zk_vote_tally(State(state.clone()), None, NoritoJson(req))
        .await
        .expect("handler ok")
        .into_response();
    let body = resp
        .into_body()
        .collect()
        .await
        .expect("read body")
        .to_bytes();
    // Parse using Norito JSON
    let v: norito::json::Value = norito::json::from_slice(&body).expect("json parse");
    assert_eq!(
        v.get("finalized").and_then(norito::json::Value::as_bool),
        Some(true)
    );
    let tally = v
        .get("tally")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();
    let ints: Vec<u64> = tally.into_iter().filter_map(|x| x.as_u64()).collect();
    assert_eq!(ints, vec![7, 2]);
}
