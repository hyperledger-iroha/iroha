#[cfg(all(test, feature = "app_api", feature = "telemetry"))]
mod peer_telemetry_tests {
    use std::{collections::HashSet, str::FromStr};

    use iroha_data_model::peer::Peer;
    use tokio::sync::watch;

    use super::{OnlinePeersProvider, collect_peer_urls};
    use crate::telemetry::peers::ToriiUrl;
    use crate::tests_runtime_handlers::checked_torii_test_ed25519_keypair;

    #[test]
    fn collect_peer_urls_requires_configured_urls() {
        let (tx, rx) = watch::channel(HashSet::new());
        let provider = OnlinePeersProvider::new(rx);

        let peer_a = Peer::new(
            "127.0.0.1:1337".parse().expect("valid socket address"),
            checked_torii_test_ed25519_keypair(
                0xfe,
                "derive peer telemetry first peer fixture key",
            )
            .public_key()
            .clone(),
        );
        let peer_b = Peer::new(
            "10.0.0.5:8080".parse().expect("valid socket address"),
            checked_torii_test_ed25519_keypair(
                0xff,
                "derive peer telemetry second peer fixture key",
            )
            .public_key()
            .clone(),
        );

        tx.send(HashSet::from([peer_a.clone(), peer_b.clone()]))
            .expect("watch update should succeed");

        let urls = collect_peer_urls(&provider, &[]);

        assert!(
            urls.is_empty(),
            "no configured URLs means no peer telemetry"
        );
    }

    #[test]
    fn collect_peer_urls_prefers_configured_urls() {
        let (tx, rx) = watch::channel(HashSet::new());
        let provider = OnlinePeersProvider::new(rx);

        let peer = Peer::new(
            "127.0.0.1:1337".parse().expect("valid socket address"),
            checked_torii_test_ed25519_keypair(
                0xfd,
                "derive peer telemetry configured peer fixture key",
            )
            .public_key()
            .clone(),
        );
        tx.send(HashSet::from([peer]))
            .expect("watch update should succeed");

        let configured = vec![
            ToriiUrl::from_str("http://127.0.0.1:8080").expect("valid torii url"),
            ToriiUrl::from_str("http://127.0.0.1:8080").expect("duplicate torii url"),
            ToriiUrl::from_str("http://127.0.0.1:8081").expect("valid torii url"),
        ];
        let mut expected = configured.clone();
        expected.sort();
        expected.dedup();

        let urls = collect_peer_urls(&provider, &configured);

        assert_eq!(
            urls, expected,
            "configured URLs should override peer-derived telemetry targets"
        );
    }

    #[test]
    fn peer_response_snapshot_uses_a_deterministic_connection_ceiling() {
        let peers = (1_u8..=3)
            .map(|seed| {
                Peer::new(
                    format!("127.0.0.1:{}", 13_337 + u16::from(seed))
                        .parse()
                        .expect("valid socket address"),
                    checked_torii_test_ed25519_keypair(
                        seed,
                        "derive bounded peer response fixture key",
                    )
                    .public_key()
                    .clone(),
                )
            })
            .collect::<HashSet<_>>();
        let (_tx, rx) = watch::channel(peers.clone());
        let provider = OnlinePeersProvider::new_with_response_limit(rx, 2);

        let mut expected = peers.into_iter().collect::<Vec<_>>();
        expected.sort();
        expected.truncate(2);

        let mut actual = provider
            .bounded_response_snapshot()
            .into_iter()
            .collect::<Vec<_>>();
        actual.sort();
        assert_eq!(actual, expected);
    }
}
