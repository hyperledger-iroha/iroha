//! Component fixture: schedule a fully attested cutover, then use genuine certified blocks.

use crate::sumeragi::test_chain::{CertifiedTestChain, fixture_keys};
use iroha_data_model::runtime::lifecycle::{
    ChangeSetRecordV1, ChangeSetStatusV1, ChangeSetV1, RuntimeNoticeV1, SupportAttestationV1,
    committee_digest,
};

/// Stage the already-approved governance record in fixture World setup. The next block still
/// uses the baseline; the following block must execute native all-validator activation.
/// This isolates proof interpretation and does not replace Parliament/provenance tests.
pub(crate) fn schedule(chain: &CertifiedTestChain, time_ms: u64) -> u64 {
    let height = chain.height() + 2;
    let change = super::activation_change_set(chain.network_id(), height, [0x69; 32]).unwrap();
    schedule_change(chain, time_ms, change)
}

/// Component-only reuse of the original all-validator scheduling fixture for another exact
/// catalog change. This stages approved fixture input; it never claims Parliament admission.
pub(crate) fn schedule_change(
    chain: &CertifiedTestChain,
    time_ms: u64,
    change: ChangeSetV1,
) -> u64 {
    let height = change.effective_height;
    assert_eq!(height, chain.height() + 2);
    let id = change.id().unwrap();
    let committee: Vec<_> = chain
        .validators()
        .iter()
        .map(|(peer, _)| peer.public_key().clone())
        .collect();
    let mut record = ChangeSetRecordV1 {
        change_set: change.clone(),
        status: ChangeSetStatusV1::Scheduled,
        proposed_ms: 0,
        approved_height: chain.height(),
        approved_ms: 0,
        notice: RuntimeNoticeV1 {
            blocks: 1,
            ms: 0,
            approval_cushion_ms: 0,
        },
        attestations: Vec::new(),
    };
    for key in fixture_keys() {
        assert!(committee.contains(key.public_key()));
        let signed = SupportAttestationV1 {
            network: change.network,
            root: change.root,
            change_set: id,
            predecessor: change.predecessor,
            successor: change.successor,
            committee: committee_digest(&committee).unwrap(),
            tested_checkpoint: chain.height(),
            valid_from: chain.height(),
            valid_until: height + 1,
            release: [0x68; 32],
            validator: key.public_key().clone(),
        }
        .sign(key.private_key())
        .unwrap();
        super::super::change_set::record_attestation(&mut record, signed);
    }
    assert!(super::super::change_set::ready(
        &record, id, height, time_ms, &committee
    ));
    chain.setup_world_at(time_ms, |tx| {
        tx.world.runtime_change_sets.insert(id, record);
    });
    height
}
