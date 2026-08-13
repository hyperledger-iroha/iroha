//! `CreateElection` should seed a Zk referendum when none exists, using governance window defaults.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::Execute,
    state::{State, World},
    zk::hash_vk,
};
use iroha_data_model::{
    Registrable,
    account::Account,
    asset::AssetDefinition,
    block::BlockHeader,
    confidential::ConfidentialStatus,
    domain::Domain,
    isi::{
        error::{InstructionExecutionError, InvalidParameterError},
        verifying_keys,
        zk::{CreateElection, FinalizeElection, MAX_ELECTION_OPTIONS_V1},
    },
    permission::Permission,
    prelude::Grant,
    proof::{ProofAttachment, ProofBox, VerifyingKeyBox, VerifyingKeyId, VerifyingKeyRecord},
    zk::BackendTag,
};
use iroha_executor_data_model::permission::governance::{CanEnactGovernance, CanManageParliament};
use iroha_primitives::json::Json;
use mv::storage::StorageReadOnly;
use nonzero_ext::nonzero;
#[test]
fn create_election_inserts_referendum_with_configured_window() {
    // Build minimal state
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let alice_id = iroha_test_samples::ALICE_ID.clone();
    let domain_id: iroha_data_model::domain::DomainId =
        iroha_data_model::domain::DomainId::try_new("wonderland", "universal").expect("domain");
    let domain = Domain::new(domain_id.clone()).build(&alice_id);
    let account = Account::new(alice_id.clone()).build(&alice_id);
    let world = World::with([domain], [account], Vec::<AssetDefinition>::new());
    let mut state = State::new_for_testing(world, kura, query_handle);
    // Configure governance window defaults
    let mut cfg = state.gov.clone();
    cfg.min_enactment_delay = 3;
    cfg.window_span = 5;
    state.set_gov(cfg);
    let header = BlockHeader::new(nonzero!(10_u64), None, None, None, 0, 0);
    let mut sblock = state.block(header);
    let mut stx = sblock.transaction();
    // No referendum exists yet
    assert!(
        stx.world
            .governance_referenda_mut()
            .get("ref-auto")
            .is_none()
    );
    let vk_box = VerifyingKeyBox::new("halo2/ipa".into(), vec![1, 2, 3, 4]);
    let vk_id = VerifyingKeyId::new("halo2/ipa", "vk-auto");
    let mut vk_record = VerifyingKeyRecord::new(
        1,
        "halo2/pasta/ipa/vote-bool-commit-merkle8",
        BackendTag::Halo2IpaPasta,
        "pallas",
        [0x11; 32],
        hash_vk(&vk_box),
    );
    vk_record.status = ConfidentialStatus::Active;
    vk_record.key = Some(vk_box);
    vk_record.vk_len = vk_record.key.as_ref().map_or(0_u32, |k| {
        u32::try_from(k.bytes.len()).expect("vk length fits in u32")
    });
    vk_record.max_proof_bytes = 1024;
    vk_record.gas_schedule_id = Some("halo2_default".into());
    let perm_vk = Permission::new("CanManageVerifyingKeys".to_string(), Json::new(()));
    let perm_parliament: Permission = CanManageParliament.into();
    let perm_enact: Permission = CanEnactGovernance.into();
    Grant::account_permission(perm_vk, alice_id.clone())
        .execute(&alice_id, &mut stx)
        .expect("grant vk permission");
    Grant::account_permission(perm_parliament, alice_id.clone())
        .execute(&alice_id, &mut stx)
        .expect("grant parliament permission");
    Grant::account_permission(perm_enact, alice_id.clone())
        .execute(&alice_id, &mut stx)
        .expect("grant enact permission");
    verifying_keys::RegisterVerifyingKey {
        id: vk_id.clone(),
        record: vk_record.clone(),
    }
    .execute(&alice_id, &mut stx)
    .expect("register verifying key");
    for options in [0, MAX_ELECTION_OPTIONS_V1 + 1] {
        let election_id = format!("ref-invalid-{options}");
        let election_count_before = stx.world.elections_mut().iter().count();
        let referendum_count_before = stx.world.governance_referenda_mut().iter().count();
        let invalid = CreateElection {
            election_id: election_id.clone(),
            options,
            eligible_root: [0u8; 32],
            start_ts: 0,
            end_ts: 0,
            vk_ballot: vk_id.clone(),
            vk_tally: vk_id.clone(),
            domain_tag: "gov:ballot:v1".to_string(),
        };
        let error = invalid
            .execute(&alice_id, &mut stx)
            .expect_err("out-of-range election options must be rejected");
        assert!(
            matches!(
                error,
                InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                    _
                ))
            ),
            "caller-supplied option count must be an invalid parameter"
        );
        assert!(
            stx.world.elections_mut().get(&election_id).is_none(),
            "rejected election must not mutate election state"
        );
        assert!(
            stx.world
                .governance_referenda_mut()
                .get(&election_id)
                .is_none(),
            "rejected election must not seed a referendum"
        );
        assert_eq!(
            stx.world.elections_mut().iter().count(),
            election_count_before,
            "rejected election must preserve the election registry"
        );
        assert_eq!(
            stx.world.governance_referenda_mut().iter().count(),
            referendum_count_before,
            "rejected election must preserve the referendum registry"
        );
    }
    let election_count_before = stx.world.elections_mut().iter().count();
    let referendum_count_before = stx.world.governance_referenda_mut().iter().count();
    let bounded_election_id = "ref-auto".to_owned();
    let create = CreateElection {
        election_id: bounded_election_id.clone(),
        options: MAX_ELECTION_OPTIONS_V1,
        eligible_root: [0u8; 32],
        start_ts: 0,
        end_ts: 0,
        vk_ballot: vk_id.clone(),
        vk_tally: vk_id.clone(),
        domain_tag: "gov:ballot:v1".to_string(),
    };
    create
        .execute(&alice_id, &mut stx)
        .expect("create election seeds referendum");
    let election = stx
        .world
        .elections_mut()
        .get(bounded_election_id.as_str())
        .cloned()
        .expect("bounded election inserted");
    assert_eq!(election.options, MAX_ELECTION_OPTIONS_V1);
    assert_eq!(election.tally.len(), MAX_ELECTION_OPTIONS_V1 as usize);
    assert_eq!(
        stx.world.elections_mut().iter().count(),
        election_count_before + 1
    );
    assert_eq!(
        stx.world.governance_referenda_mut().iter().count(),
        referendum_count_before + 1
    );
    for tally_len in [0, 64, 65] {
        stx.world
            .elections_mut()
            .get_mut(&bounded_election_id)
            .expect("bounded election remains present")
            .tally = vec![0; tally_len];
        let finalize = FinalizeElection {
            election_id: bounded_election_id.clone(),
            tally: vec![0; MAX_ELECTION_OPTIONS_V1 as usize],
            tally_proof: ProofAttachment::new_ref(
                "halo2/ipa".into(),
                ProofBox::new("halo2/ipa".into(), Vec::new()),
                vk_id.clone(),
            ),
        };
        let error = finalize
            .execute(&alice_id, &mut stx)
            .expect_err("invalid or unproved tally must not finalize");
        if tally_len == MAX_ELECTION_OPTIONS_V1 as usize {
            assert!(
                error.to_string().contains("invalid tally proof"),
                "a 64-counter stored tally must pass the shape gate: {error}"
            );
        } else {
            assert!(
                error.to_string().contains("invalid stored election shape"),
                "stored tally length {tally_len} must fail the shape gate: {error}"
            );
        }
        let rejected = stx
            .world
            .elections_mut()
            .get(bounded_election_id.as_str())
            .cloned()
            .expect("rejected finalize must preserve election state");
        assert!(!rejected.finalized);
        assert_eq!(
            rejected.tally.len(),
            tally_len,
            "rejection must not truncate or replace the stored tally"
        );
    }
    let rec = stx
        .world
        .governance_referenda_mut()
        .get(bounded_election_id.as_str())
        .copied()
        .expect("referendum inserted");
    assert_eq!(rec.mode, iroha_core::state::GovernanceReferendumMode::Zk);
    // h_start = current height (10) + min_enactment_delay (3) = 13
    assert_eq!(rec.h_start, 13);
    // h_end = h_start + span - 1 = 13 + 5 - 1 = 17
    assert_eq!(rec.h_end, 17);
    assert_eq!(
        rec.status,
        iroha_core::state::GovernanceReferendumStatus::Proposed
    );
}
