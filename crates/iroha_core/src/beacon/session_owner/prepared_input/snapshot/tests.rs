//! Independent exact-layout census for the three retained local publication banks.

use super::*;
use crate::{
    beacon::PreparedGlobalThresholdBeaconDkgPublicationV1, test_allocations::allocations_during,
};
use norito::core::PreparedDecodeWorkspace;

fn outer<D: rows::Row>(count: usize) -> usize {
    Layout::array::<D>(count).unwrap().size()
        + Layout::array::<D::Wire>(count).unwrap().size()
        + Layout::array::<SequenceSpan>(count).unwrap().size()
}
fn original_layout_bytes(
    session: &GlobalThresholdBeaconDkgSessionV1,
    roster: &[PeerId],
    phase: usize,
) -> usize {
    let n = usize::from(session.committee_size);
    let (rows, edges, acceptances) = match phase {
        1 => (1, 0, 0),
        2 => (n, n, 0),
        3 => (n, n * n, n),
        _ => unreachable!(),
    };
    let children = roster[..rows]
        .iter()
        .map(|peer| peer.public_key().retained_allocation_layout().size())
        .sum::<usize>()
        + rows
            * (soranet_pq::MlKemSuite::MlKem768.public_key_len()
                + 2 * iroha_crypto::Algorithm::BlsNormal.signature_payload_len()
                + usize::from(session.threshold)
                    * (std::mem::size_of::<[u8; 96]>() + std::mem::size_of::<SequenceSpan>()))
        + edges
            * (soranet_pq::MlKemSuite::MlKem768.ciphertext_len()
                + iroha_crypto::threshold_bls::DAS_REN_PRIVATE_SHARE_CIPHERTEXT_BYTES_V1
                + iroha_crypto::Algorithm::BlsNormal.signature_payload_len())
        + acceptances * iroha_crypto::Algorithm::BlsNormal.signature_payload_len();
    outer::<Recipient>(rows)
        + outer::<Dealer>(rows)
        + outer::<Edge>(edges)
        + outer::<Acceptance>(acceptances)
        + children
        + Layout::array::<AllocationCharge>(4 + rows * 5 + edges * 3 + acceptances)
            .unwrap()
            .size()
        + PreparedDecodeWorkspace::allocation_layouts()
            .iter()
            .map(Layout::size)
            .sum::<usize>()
}

#[test]
fn local_publication_bank_physical_layout_census_scales_actual_four_and_thirty_one_geometry() {
    for n in [4, 31] {
        let keys = crate::beacon::fixtures::adaptive_fixture_signing_keys(n);
        let roster = keys
            .iter()
            .map(|key| PeerId::new(key.public_key().clone()))
            .collect::<Vec<_>>();
        let mut session = crate::beacon::fixtures::adaptive_dkg_session_fixture();
        session.committee_size = n;
        session.threshold = (n - 1) / 3 + 1;
        session.roster_hash = crate::beacon::global_threshold_beacon_roster_hash_v1(&roster);
        let pool = crate::beacon::fixtures::fixture_budget();
        let mut prepared = Vec::new(); // Test containers are outside the measured production constructors.
        for (phase, prepare) in [
            PreparedGlobalThresholdBeaconDkgPublicationV1::new,
            PreparedGlobalThresholdBeaconDkgPublicationV1::new_delivery,
            PreparedGlobalThresholdBeaconDkgPublicationV1::new_acceptance,
        ]
        .into_iter()
        .enumerate()
        {
            let phase = phase + 1;
            let before = pool.reserved_bytes();
            let expected = original_layout_bytes(&session, &roster, phase);
            let rows = if phase == 1 { 1 } else { usize::from(n) };
            let edges = match phase {
                1 => 0,
                2 => usize::from(n),
                _ => usize::from(n) * usize::from(n),
            };
            let acceptances = if phase == 3 { usize::from(n) } else { 0 };
            let nonempty = 2 + usize::from(edges != 0) + usize::from(acceptances != 0);
            let count = 3 * nonempty + 6 * rows + 3 * edges + acceptances + 3;
            let mut bank = None;
            assert_eq!(
                allocations_during(|| bank = Some(prepare(session, &roster, 1, &pool).unwrap())),
                count
            );
            assert_eq!(pool.reserved_bytes() - before, expected);
            assert!(bank.as_ref().unwrap().belongs_to(&pool));
            eprintln!(
                "local publication bank n{n} phase{phase}: physical requested bytes={expected}, actual allocations={count}"
            );
            prepared.push(bank.unwrap());
        }
        assert_eq!(
            pool.reserved_bytes(),
            (1..=3)
                .map(|phase| original_layout_bytes(&session, &roster, phase))
                .sum::<usize>()
        );
        drop(prepared);
        assert_eq!(pool.reserved_bytes(), 0);
    }
}
