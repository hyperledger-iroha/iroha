//! Installed network selection and canonical native bundle custody regressions.

use super::*;

fn profile(name: &str) -> InstalledNetworkProfile {
    InstalledNetworkProfile::new(
        name.into(),
        "https://network.example/",
        "fixture-chain".into(),
        369,
    )
    .unwrap()
}

#[test]
fn installed_profiles_require_canonical_https_root_and_exact_network_expectations() {
    let valid = profile("fixture");
    assert_eq!(valid.network_name(), "fixture");
    assert_eq!(valid.torii_root().as_str(), "https://network.example/");
    assert_eq!(valid.chain_id(), "fixture-chain");
    assert_eq!(valid.account_chain_discriminant(), 369);
    for url in [
        "http://network.example/",
        "http://localhost/",
        "http://192.168.1.2/",
        "http://[::ffff:127.0.0.1]/",
        "ftp://127.0.0.1/",
        "https://NETWORK.example/",
        "https://network.example:443/",
        "https://network.example/a/../",
        "https://user:secret@network.example/",
        "https://network.example/?key=attacker",
        "https://network.example/#fragment",
        "https://network.example",
        "https://network.example/api",
    ] {
        assert!(
            InstalledNetworkProfile::new("fixture".into(), url, "fixture-chain".into(), 369)
                .is_err(),
            "{url}"
        );
    }
    for name in ["Fixture", "", "1fixture", "fixture_name"] {
        assert!(
            InstalledNetworkProfile::new(
                name.into(),
                valid.torii_root().as_str(),
                "fixture-chain".into(),
                369,
            )
            .is_err()
        );
    }
    for chain in ["", " fixture", "fixture ", "fixture/chain", "chain\0id"] {
        assert!(
            InstalledNetworkProfile::new(
                "fixture".into(),
                valid.torii_root().as_str(),
                chain.into(),
                369,
            )
            .is_err(),
            "{chain:?}"
        );
    }
    assert!(
        InstalledNetworkProfile::new(
            "fixture".into(),
            valid.torii_root().as_str(),
            "fixture-chain".into(),
            0,
        )
        .is_err()
    );
    assert!(
        InstalledNetworkProfile::new(
            "fixture".into(),
            &format!(
                "https://network.example/{}/",
                "a".repeat(MAX_TORII_ROOT_BYTES)
            ),
            "fixture-chain".into(),
            369,
        )
        .is_err()
    );
    let nested = InstalledNetworkProfile::new(
        "nested".into(),
        "https://network.example/native/",
        "fixture-chain".into(),
        369,
    )
    .unwrap();
    assert_eq!(nested.torii_root().path(), "/native/");
    for url in ["http://127.0.0.1:18080/", "http://[::1]:18080/"] {
        let local =
            InstalledNetworkProfile::new("local".into(), url, "fixture-chain".into(), 753).unwrap();
        assert_eq!(local.torii_root().as_str(), url);
    }
    let absent = InstalledNetworkProfiles::new(vec![]).unwrap();
    assert!(absent.select("taira").is_err());
    assert!(
        InstalledNetworkProfiles::from_installation_bytes(&absent.encode_installation().unwrap())
            .unwrap()
            .select("taira")
            .is_err()
    );
}

#[test]
fn installed_profile_set_roundtrips_sorted_and_loads_from_retained_custody() {
    let profiles =
        InstalledNetworkProfiles::new(vec![profile("second"), profile("first")]).unwrap();
    let bytes = profiles.encode_installation().unwrap();
    let decoded = InstalledNetworkProfiles::from_installation_bytes(&bytes).unwrap();
    assert_eq!(decoded.names().collect::<Vec<_>>(), ["first", "second"]);
    let first = decoded.select("first").unwrap();
    assert_eq!(first.network_name(), "first");
    assert_eq!(first.torii_root().as_str(), "https://network.example/");
    assert_eq!(first.chain_id(), "fixture-chain");
    assert_eq!(first.account_chain_discriminant(), 369);
    assert!(decoded.select("FIRST").is_err());
    assert_eq!(decoded.encode_installation().unwrap(), bytes);
    let temporary = tempfile::tempdir().unwrap();
    let directory =
        iroha_fs::PrivateDirectory::open_or_create(temporary.path().join("installation")).unwrap();
    directory
        .write_atomic(
            NETWORK_PROFILES_FILENAME,
            &bytes,
            iroha_fs::PublishMode::CreateNew,
        )
        .unwrap();
    assert_eq!(
        InstalledNetworkProfiles::load(&directory.path().join(NETWORK_PROFILES_FILENAME))
            .unwrap()
            .encode_installation()
            .unwrap(),
        bytes
    );
    assert!(InstalledNetworkProfiles::load(&directory.path().join("missing.nrt")).is_err());
}

#[test]
fn installed_profile_decoder_revalidates_each_network_selection() {
    let corruptions: [fn(&mut ProfileRecord); 4] = [
        |record| record.network_name = "Invalid".into(),
        |record| record.torii_root = "http://network.example/".into(),
        |record| record.expected_chain_id = "uncanonical chain".into(),
        |record| record.account_chain_discriminant = 0,
    ];
    for corrupt in corruptions {
        let mut record = profile("fixture").record();
        corrupt(&mut record);
        let records = ProfileRecords {
            profiles: vec![record],
        };
        assert!(
            InstalledNetworkProfiles::from_installation_bytes(
                &norito::encode_canonical(&records).unwrap()
            )
            .is_err()
        );
    }
}

#[test]
fn installed_profile_decoder_rejects_duplicate_unsorted_oversized_or_trailing_records() {
    assert!(InstalledNetworkProfiles::new(vec![profile("same"), profile("same")]).is_err());
    assert!(
        InstalledNetworkProfiles::new(
            (0..=MAX_INSTALLED_NETWORK_PROFILES)
                .map(|i| profile(&format!("network-{i}")))
                .collect()
        )
        .is_err()
    );
    for names in [["second", "first"], ["same", "same"]] {
        let records = ProfileRecords {
            profiles: names.iter().map(|name| profile(name).record()).collect(),
        };
        assert!(
            InstalledNetworkProfiles::from_installation_bytes(
                &norito::encode_canonical(&records).unwrap()
            )
            .is_err()
        );
    }
    let mut bytes = InstalledNetworkProfiles::new(vec![profile("fixture")])
        .unwrap()
        .encode_installation()
        .unwrap();
    bytes.push(0);
    assert!(InstalledNetworkProfiles::from_installation_bytes(&bytes).is_err());
    assert!(InstalledNetworkProfiles::from_installation_bytes(&[]).is_err());
    assert!(
        InstalledNetworkProfiles::from_installation_bytes(&vec![
            0;
            MAX_INSTALLED_PROFILE_BYTES + 1
        ])
        .is_err()
    );
}
