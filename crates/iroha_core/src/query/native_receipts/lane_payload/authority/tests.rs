//! Exact selected graph demand, immutable source retry and canonical authority parity.

use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
use iroha_data_model::{
    NetworkId,
    sumeragi_lanes::{SumeragiLaneMember, SumeragiLaneRecord, SumeragiLaneState},
};
use iroha_model_base::{peer::PeerId, topology::DataSpaceId};
use norito::codec::Encode;

use super::*;

fn record() -> (NetworkId, SumeragiLaneRecord) {
    let network = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
        b"original prepaid lane authority",
    )));
    let mut members = (1..=4)
        .map(|seed| {
            let key = KeyPair::from_seed(vec![seed; 32], Algorithm::BlsNormal);
            SumeragiLaneMember {
                peer: PeerId::new(key.public_key().clone()),
                pop: iroha_crypto::bls_normal_pop_prove(key.private_key()).unwrap(),
            }
        })
        .collect::<Vec<_>>();
    members.sort_by(|a, b| a.peer.cmp(&b.peer));
    let mut record = SumeragiLaneRecord {
        da_layout: iroha_sumeragi::availability::recommended_data_availability_layout(),
        lane: LaneId::new(7),
        dataspace: DataSpaceId::new(11),
        incarnation: [17; 32],
        params: iroha_data_model::parameter::system::SumeragiParameters::default(),
        committee: members,
        created_at: 20,
        active_from: 22,
        closing: None,
        anchor_freshness: 16,
        merged: SumeragiLaneFrontier::default(),
        merged_at: 22,
        rescued: 0,
    };
    record.merged = SumeragiLaneFrontier {
        height: 0,
        block_hash: crate::sumeragi::lanes::lane_genesis_hash(&network, &record).0,
        result: crate::sumeragi::lanes::lane_genesis_result(&record).0,
    };
    (network, record)
}
fn source(
    network: NetworkId,
    record: &SumeragiLaneRecord,
    budget: &AllocationBudget,
) -> LanePayload {
    let state = SumeragiLaneState {
        lanes: vec![record.clone()],
        ..SumeragiLaneState::default()
    };
    let encoded = state.encode();
    let mut bytes = ChargedBuffer::new(encoded.len(), budget).unwrap();
    bytes.append(&encoded).unwrap();
    // Only this private fixture can select raw bytes directly. Production LanePayload is
    // constructed exclusively from an independently authenticated original receipt/write root.
    LanePayload {
        bytes,
        range: 0..encoded.len(),
        network,
        height: record.created_at,
        carrier: HashOf::from_untyped_unchecked(Hash::new(b"original creation carrier")),
    }
}

#[test]
fn selected_authority_matches_existing_creation_context_and_exact_original_credentials() {
    let (network, record) = record();
    let encoded = record.encode();
    let raw = RawAuthority::parse(&encoded).unwrap();
    assert_eq!(raw.context().unwrap().0, record.merged.result);
    assert_eq!(raw.frontier().unwrap(), record.merged);
    assert_eq!(
        raw.params,
        crate::sumeragi::schedule::ChainParamsRecord::from_parameters(&record.params)
    );
    let expected = crate::sumeragi::lanes::lane_height_config(&record).unwrap();
    let budget = AllocationBudget::new(1 << 20);
    let source = source(network, &record, &budget);
    let held = budget.reserved_bytes();
    let demand = raw.demand().unwrap().bytes;
    let owner = LaneAuthorityRead::new(source, record.incarnation)
        .complete(&budget)
        .unwrap_or_else(|(_, error)| panic!("selected original authority: {error}"));
    assert_eq!(owner.config(), &expected);
    assert_eq!(owner.lane(), record.lane);
    assert!(owner.belongs_to(&budget));
    assert_eq!(budget.reserved_bytes(), held + demand);
    let mut index = 0;
    owner
        .visit_members(|key, pop| {
            assert_eq!(
                key,
                record.committee[index]
                    .peer
                    .public_key()
                    .try_to_bytes()
                    .unwrap()
                    .1
            );
            assert_eq!(pop, record.committee[index].pop);
            index += 1;
            Ok(())
        })
        .unwrap();
    assert_eq!(index, record.committee.len());
    drop(owner);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn selected_authority_refuses_before_graph_allocation_then_retries_same_source_at_exact_demand() {
    let (network, record) = record();
    let budget = AllocationBudget::new(1 << 20);
    let source = source(network, &record, &budget);
    let pointer = source.bytes.as_slice().as_ptr();
    let held = budget.reserved_bytes();
    let demand = RawAuthority::parse(&record.encode())
        .unwrap()
        .demand()
        .unwrap()
        .bytes;
    let read = LaneAuthorityRead::new(source, record.incarnation);
    let foreign = AllocationBudget::new(1 << 20);
    let (read, error) = read
        .complete(&foreign)
        .err()
        .expect("foreign source refused");
    assert!(matches!(error, LanePayloadError::Source));
    assert_eq!(foreign.reserved_bytes(), 0);
    budget.set_limit_bytes(held + demand - 1);
    let (read, error) = read
        .complete(&budget)
        .err()
        .expect("one byte under exact demand");
    assert!(error.is_local_refusal());
    assert_eq!(budget.reserved_bytes(), held);
    assert_eq!(read.source.bytes.as_slice().as_ptr(), pointer);
    budget.set_limit_bytes(held + demand);
    let owner = read
        .complete(&budget)
        .unwrap_or_else(|(_, error)| panic!("same source retry: {error}"));
    assert_eq!(owner.source.bytes.as_slice().as_ptr(), pointer);
    assert_eq!(budget.reserved_bytes(), held + demand);
    budget.set_limit_bytes(0);
    assert_eq!(owner.config().committee.n(), 4);
    drop(owner);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn selected_authority_rejects_noncreation_or_changed_geometry_before_reserving_graph() {
    let (network, original) = record();
    for case in 0..7 {
        let mut record = original.clone();
        match case {
            0 => record.committee.swap(0, 1),
            1 => record.committee[1] = record.committee[0].clone(),
            2 => {
                record.committee.pop();
            }
            3 => record.closing = Some(record.created_at + 1),
            4 => record.merged.result = Hash::new(b"foreign creation result").into(),
            5 => record.active_from += 1,
            6 => record.da_layout.chunk_size_bytes = 0,
            _ => unreachable!(),
        }
        let budget = AllocationBudget::new(1 << 20);
        let source = source(network, &record, &budget);
        let held = budget.reserved_bytes();
        let (read, error) = LaneAuthorityRead::new(source, record.incarnation)
            .complete(&budget)
            .err()
            .expect("invalid independent creation authority");
        assert!(!error.is_local_refusal());
        assert_eq!(budget.reserved_bytes(), held);
        drop(read);
        assert_eq!(budget.reserved_bytes(), 0);
    }
    let budget = AllocationBudget::new(1 << 20);
    let mut source = source(network, &original, &budget);
    source.height += 1;
    let (read, error) = LaneAuthorityRead::new(source, original.incarnation)
        .complete(&budget)
        .err()
        .expect("newer carrier cannot substitute the original creation receipt");
    assert!(matches!(error, LanePayloadError::Source));
    drop(read);
    assert_eq!(budget.reserved_bytes(), 0);
}
