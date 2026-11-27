//! Native later receiver-head delivery coverage against retained signed originals.
use super::{credit_opening::StatusOpening, *};
use crate::native::engine::credit_status_wire::later_send::{WORDS, packed_fixture};
use iroha_data_model::kagemusha::{
    KagemushaWalletIndexedInsertV1 as PublicInsert, KagemushaWalletIndexedLeafV1 as PublicLeaf,
    KagemushaWalletIndexedOpeningV1 as PublicPath,
};
const MAX: usize = 10_084 - crate::payment_wire::HEADER_BYTES;
struct FixtureRoute {
    root: [u8; 32],
    leaf: PublicLeaf,
    path: PublicPath,
}
impl native::SpartanWalletMaps for FixtureRoute {
    fn member_or_low(
        &mut self,
        map: native::SpartanWalletMap,
        root: [u8; 32],
        key: [u8; 32],
    ) -> Result<(PublicLeaf, PublicPath)> {
        if map != native::SpartanWalletMap::Credit || root != self.root || key != self.leaf.key {
            return Err(Error::AuthorityMismatch("fixture credit route"));
        }
        Ok((self.leaf, self.path))
    }
    fn insert(
        &mut self,
        _: native::SpartanWalletMap,
        _: [u8; 32],
        _: [u8; 32],
        _: [u8; 32],
    ) -> Result<PublicInsert> {
        panic!("read-only delivery must not mutate the indexed tree")
    }
}
fn reference(
    later: &native::VerifiedIncomingPayment,
    o: &StatusOpening,
    outcome: native::VerifiedCreditStatus,
    p: &native::AuthenticatedParameters,
) -> (Vec<u8>, usize, usize) {
    packed_fixture(later, o.next, o.index, o.siblings, outcome, p).unwrap()
}

#[test]
#[ignore = "requires signed receiver Receive and later Send originals"]
fn successor_archive_status_at_a_later_receiver_send() {
    use crate::payment_wire;
    with_retained_inputs(|payer, send, sent, engine, parameters, wallet| {
        let later = engine
            .verify_payment(
                &std::fs::read(wallet.join("incoming-payment.bin")).unwrap(),
                parameters,
            )
            .unwrap();
        let original = payer.delivery.payment();
        let open = payer.delivery.open_request();
        send.verify_recovered_send_request(open).unwrap();
        assert!(send.verify_recovered_send_request(&[]).is_err());
        let competing = std::fs::read(wallet.join("competing-send-request.bin")).unwrap();
        let framed = competing.strip_prefix(b"KSO1\x03").unwrap();
        let (size, other) = framed.split_at(4);
        assert_eq!(
            u32::from_le_bytes(size.try_into().unwrap()) as usize,
            other.len()
        );
        native::decode_open_request_peer(other).unwrap();
        assert_ne!(other, open);
        assert!(send.verify_recovered_send_request(other).is_err());
        let (earlier, root) = retained_receiver_receipt(payer.delivery, engine, parameters, wallet);
        assert_eq!(later.statement()[14], earlier.expected[2]);
        assert_ne!(later.statement()[15], earlier.expected[2]);
        assert_ne!(later.statement()[17], original.statement()[17]);
        assert_eq!(later.profile()[bc::LINEAGE_IO + bc::CREDIT_ROOT], root);
        let opening = StatusOpening::from_verified(payer.delivery, root).unwrap();
        let outcome = native::VerifiedCreditStatus::Credited;
        let credit = original.statement()[17];
        let leaf = PublicLeaf {
            key: credit.to_repr(),
            value: poseidon::hash_with_domain(
                crate::receive_lineage::CREDIT_DOMAIN,
                &[credit, original.digest(), Fp::ZERO],
            )
            .to_repr(),
            next_key: opening.next.to_repr(),
        };
        let path = PublicPath {
            slot: opening.index,
            siblings: opening.siblings.map(|v| v.to_repr()),
        };
        let mut maps = FixtureRoute {
            root: root.to_repr(),
            leaf,
            path,
        };
        let wire = engine
            .encode_credit_status_at_send_head(&later, original, open, &mut maps, parameters)
            .unwrap();
        let (independent, compact, maximum) = reference(&later, &opening, outcome, parameters);
        assert_eq!(wire, independent);
        let verify = |wire: &[u8],
                      o: &native::VerifiedIncomingPayment,
                      req: &[u8],
                      p: &native::AuthenticatedParameters| {
            NativeEngine::default().verify_credit_status_peer(wire, req, o, p)
        };
        assert_eq!(verify(&wire, original, open, parameters).unwrap(), outcome);
        let delivery = engine
            .verify_sent_delivery(send, sent, open, &wire, parameters)
            .unwrap();
        assert_eq!(delivery.outcome(), outcome);
        assert_eq!(delivery.payment().exact_bytes(), original.exact_bytes());
        assert_eq!(delivery.credit_status(), wire);
        assert_eq!(
            delivery
                .verify_pending_removal(payer.released, payer.adjusted_removal)
                .unwrap(),
            payer
                .delivery
                .verify_pending_removal(payer.released, payer.adjusted_removal)
                .unwrap()
        );
        if let Some(dir) = std::env::var_os("KAGEMUSHA_SPARTAN_TEST_OUTPUT") {
            std::fs::write(
                std::path::PathBuf::from(dir).join("later-send-credit-status.ipm1"),
                &wire,
            )
            .unwrap();
        }
        let mut dense = opening.clone();
        dense.siblings = [Fp::from(987654u64); 32];
        let (dense_wire, _, dense_max) = reference(&later, &dense, outcome, parameters);
        assert_eq!(dense_wire.len(), dense_max);
        assert_eq!(dense_max, maximum);
        assert!(maximum <= 10_084);
        assert!(
            {
                maps.path.siblings = dense.siblings.map(|v| v.to_repr());
                engine
                    .encode_credit_status_at_send_head(
                        &later, original, open, &mut maps, parameters,
                    )
                    .is_err()
            },
            "producer refuses unproved route"
        );
        let dense_payload = payment_wire::decode_peer_payload(4, &dense_wire, MAX).unwrap();
        let mut full_budget = dense_payload.to_vec();
        full_budget.resize(MAX, 0);
        assert_eq!(full_budget.len() + payment_wire::HEADER_BYTES, 10_084);
        assert!(
            verify(
                &payment_wire::encode_peer_payload(4, &full_budget),
                original,
                open,
                parameters
            )
            .is_err(),
            "trailing bytes at exact transport maximum"
        );
        assert!(
            verify(&dense_wire, original, open, parameters).is_err(),
            "dense path fits and fails the actual signed credit root"
        );
        eprintln!(
            "ArchiveSent later receiver Send raw_pair={} compact_pair={} ipm1={} dense_ipm1={} limit=10084",
            later.exact_pair().len(),
            compact,
            wire.len(),
            maximum
        );
        let raw = payment_wire::decode_peer_payload(4, &wire, MAX).unwrap();
        let fields_end = 1 + WORDS.iter().map(|&(_, n)| n).sum::<usize>();
        for (name, at) in [
            ("verdict", 0),
            ("head receipt signature", fields_end + 64),
            ("index", fields_end + 128 + 40 + 32),
            ("folded proof", raw.len() - 1),
        ] {
            let mut changed = raw.to_vec();
            changed[at] ^= 1;
            assert!(
                verify(
                    &payment_wire::encode_peer_payload(4, &changed),
                    original,
                    open,
                    parameters
                )
                .is_err(),
                "{name}"
            );
        }
        let mut wrong_profile = raw.to_vec();
        wrong_profile[0] ^= 0x80;
        assert!(
            verify(
                &payment_wire::encode_peer_payload(4, &wrong_profile),
                original,
                open,
                parameters
            )
            .is_err()
        );
        assert!(verify(&wire, &later, open, parameters).is_err());
        assert_eq!(
            verify(payer.delivery.credit_status(), original, open, parameters).unwrap(),
            outcome
        );
        assert!(verify(&wire[..wire.len() - 1], original, open, parameters).is_err());
        assert!(verify(&vec![0; 10_085], original, open, parameters).is_err());
    });
}
