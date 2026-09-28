// Borrowed evidence identity, complete-set rejection, and bounded local storage controls.

const BORROWED_ATTESTATION_SEED: u8 = 0x41;

fn borrowed_attestation_signer() -> KeyPair {
    KeyPair::try_from_seed(
        vec![BORROWED_ATTESTATION_SEED.wrapping_add(14); 32],
        Algorithm::Ed25519,
    )
    .expect("original provider signer")
}

fn sign_borrowed_attestation_record(record: &mut MusubiProviderBundleAttestationRecordV1) {
    let signer = borrowed_attestation_signer();
    record.attestation.approvals[0].signature = SignatureOf::try_from_hash(
        signer.private_key(),
        record.attestation.payload.signing_hash(),
    )
    .expect("provider statement signature");
}

fn borrowed_attestation_fixture(count: usize) -> (World, MusubiArchiveLocationKeyV1) {
    use iroha_data_model::sorafs::capacity::ProviderId;
    assert!((1..=MUSUBI_MAX_LOCATION_PROVIDERS_V1).contains(&count));
    let (mut world, _, key, _) = archive_location_replay_fixture(BORROWED_ATTESTATION_SEED);
    let mut location = world
        .musubi_archive_locations
        .view()
        .get(&key)
        .cloned()
        .expect("fixture location");
    let original_key = MusubiProviderBundleAttestationKeyV1 {
        archive_id: key.archive_id,
        replication_order: location.replication_order,
        provider_id: location.providers[0],
    };
    let template = world
        .musubi_provider_bundle_attestations
        .view()
        .get(&original_key)
        .cloned()
        .expect("fixture provider record");
    {
        let mut records = world.musubi_provider_bundle_attestations.block();
        let _ = records.remove(original_key);
        records.commit();
    }
    location.providers.clear();
    let mut references = Vec::new();
    for index in 1..=count {
        let provider_id = ProviderId::new([u8::try_from(index).expect("bounded count"); 32]);
        let mut record = template.clone();
        record.attestation.payload.binding.provider_id = provider_id;
        record.key = record.attestation.key();
        sign_borrowed_attestation_record(&mut record);
        record.attestation_digest = record.attestation.digest();
        record.validate().expect("canonical provider record");
        references.push(record.attestation.reference());
        location.providers.push(provider_id);
        world
            .musubi_provider_bundle_attestations
            .insert(record.key, record);
    }
    location.provider_attestation_set_digest = musubi_provider_bundle_attestation_set_digest_v1(
        key.archive_id,
        location.replication_order,
        &references,
    )
    .expect("complete provider set digest");
    world.musubi_archive_locations.insert(key, location);
    // This fixture supplies the exact helper sources. The multi-provider cases
    // do not claim a fully derived, publishable World/SoraFS availability cut.
    (world, key)
}

fn change_borrowed_attestation_record(
    world: &mut World,
    location_key: MusubiArchiveLocationKeyV1,
    index: usize,
    change: impl FnOnce(&mut MusubiProviderBundleAttestationRecordV1),
) {
    let mut location = world
        .musubi_archive_locations
        .view()
        .get(&location_key)
        .cloned()
        .expect("fixture location");
    let key = MusubiProviderBundleAttestationKeyV1 {
        archive_id: location.archive_id,
        replication_order: location.replication_order,
        provider_id: location.providers[index],
    };
    let mut record = world
        .musubi_provider_bundle_attestations
        .view()
        .get(&key)
        .cloned()
        .expect("fixture provider record");
    change(&mut record);
    record.attestation_digest = record.attestation.digest();
    // Retain the original storage key, so a coherently changed record identity
    // still has to pass the helper's independent lookup-key binding.
    world
        .musubi_provider_bundle_attestations
        .insert(key, record);
    let references = {
        let rows = world.musubi_provider_bundle_attestations.view();
        location
            .providers
            .iter()
            .map(|provider_id| {
                rows.get(&MusubiProviderBundleAttestationKeyV1 {
                    archive_id: location.archive_id,
                    replication_order: location.replication_order,
                    provider_id: *provider_id,
                })
                .expect("fixture reference source")
                .attestation
                .reference()
            })
            .collect::<Vec<_>>()
    };
    location.provider_attestation_set_digest = musubi_provider_bundle_attestation_set_digest_v1(
        location.archive_id,
        location.replication_order,
        &references,
    )
    .expect("changed set remains structurally canonical");
    world
        .musubi_archive_locations
        .insert(location_key, location);
}

fn assert_borrowed_attestation_error(
    world: &World,
    key: MusubiArchiveLocationKeyV1,
    expected: &str,
) {
    let view = world.view();
    let archive = view
        .musubi_archives()
        .get(&key.archive_id)
        .expect("archive source");
    let location = view
        .musubi_archive_locations()
        .get(&key)
        .expect("location source");
    let error = load_location_provider_attestations(archive, location, &view)
        .expect_err("no partial borrowed evidence may escape a failed complete-set check");
    assert!(error.to_string().contains(expected), "{error}");
}

#[test]
fn borrowed_attestations_return_original_records_in_order_with_fixed_local_storage() {
    for count in [1, 2, MUSUBI_MAX_LOCATION_PROVIDERS_V1] {
        let (world, key) = borrowed_attestation_fixture(count);
        let view = world.view();
        let archive = view.musubi_archives().get(&key.archive_id).unwrap();
        let location = view.musubi_archive_locations().get(&key).unwrap();
        let evidence = load_location_provider_attestations(archive, location, &view).unwrap();
        assert_eq!(evidence.len(), count);
        assert_eq!(evidence.iter().count(), count);
        assert!(
            std::mem::size_of_val(&evidence)
                <= (MUSUBI_MAX_LOCATION_PROVIDERS_V1 + 1) * std::mem::size_of::<usize>(),
            "the result stores bounded references and length, not owned records",
        );
        for (record, provider) in evidence.iter().zip(&location.providers) {
            let stored = view
                .musubi_provider_bundle_attestations()
                .get(&MusubiProviderBundleAttestationKeyV1 {
                    archive_id: key.archive_id,
                    replication_order: location.replication_order,
                    provider_id: *provider,
                })
                .unwrap();
            assert!(
                std::ptr::eq(record, stored),
                "record was copied out of World"
            );
            assert_eq!(record.key.provider_id, *provider);
        }
    }
}

#[test]
fn borrowed_attestations_reject_omitted_records_and_changed_set_digest() {
    let (world, key) = borrowed_attestation_fixture(2);
    let location = world
        .musubi_archive_locations
        .view()
        .get(&key)
        .cloned()
        .unwrap();
    let missing = MusubiProviderBundleAttestationKeyV1 {
        archive_id: key.archive_id,
        replication_order: location.replication_order,
        provider_id: location.providers[1],
    };
    {
        let mut records = world.musubi_provider_bundle_attestations.block();
        let _ = records.remove(missing);
        records.commit();
    }
    assert_borrowed_attestation_error(&world, key, "attestation record was not found");
    let (mut world, key) = borrowed_attestation_fixture(2);
    let mut location = world
        .musubi_archive_locations
        .view()
        .get(&key)
        .cloned()
        .unwrap();
    location.provider_attestation_set_digest =
        MusubiProviderBundleAttestationSetDigestV1::new([0xee; 32]);
    world.musubi_archive_locations.insert(key, location);
    assert_borrowed_attestation_error(&world, key, "attestation set digest is inconsistent");
}

#[test]
fn borrowed_attestations_reject_location_bounds_order_and_duplicates_before_lookup() {
    use iroha_data_model::sorafs::capacity::ProviderId;
    for case in 0..4 {
        let (mut world, key) = borrowed_attestation_fixture(2);
        let mut location = world
            .musubi_archive_locations
            .view()
            .get(&key)
            .cloned()
            .unwrap();
        match case {
            0 => location.providers.clear(),
            1 => {
                location.providers = (1..=MUSUBI_MAX_LOCATION_PROVIDERS_V1 + 1)
                    .map(|index| ProviderId::new([u8::try_from(index).unwrap(); 32]))
                    .collect()
            }
            2 => location.providers.reverse(),
            _ => location.providers[1] = location.providers[0],
        }
        // If record lookup occurred before location admission, this would return
        // the missing-record error instead of the required shape/bounds error.
        {
            let keys = world
                .musubi_provider_bundle_attestations
                .view()
                .iter()
                .map(|(key, _)| *key)
                .collect::<Vec<_>>();
            let mut rows = world.musubi_provider_bundle_attestations.block();
            for key in keys {
                let _ = rows.remove(key);
            }
            rows.commit();
        }
        world.musubi_archive_locations.insert(key, location);
        assert_borrowed_attestation_error(
            &world,
            key,
            if case < 2 {
                "Musubi archive location is invalid"
            } else {
                "providers must be sorted and distinct"
            },
        );
    }
}

#[test]
fn borrowed_attestations_keep_lookup_key_predecessor_time_and_immutable_bindings() {
    for case in 0..4 {
        let (mut world, key) = borrowed_attestation_fixture(1);
        change_borrowed_attestation_record(&mut world, key, 0, |record| match case {
            0 => {
                record.attestation.payload.binding.provider_id =
                    iroha_data_model::sorafs::capacity::ProviderId::new([0xf1; 32]);
                record.key = record.attestation.key();
                sign_borrowed_attestation_record(record);
            }
            1 => record.registered_at_height = 2,
            2 => {
                record.attestation.payload.binding.replication_order =
                    iroha_data_model::sorafs::pin_registry::ReplicationOrderId::new([0xf2; 32]);
                record.key = record.attestation.key();
                sign_borrowed_attestation_record(record);
            }
            _ => {
                record.attestation.payload.binding.bundle_digest =
                    MusubiContentDigestV1::new([0xf3; 32]);
                sign_borrowed_attestation_record(record);
            }
        });
        assert_borrowed_attestation_error(
            &world,
            key,
            if case < 3 {
                "not a finalized predecessor"
            } else {
                "does not match its immutable archive commitments"
            },
        );
    }
    let (mut world, key) = borrowed_attestation_fixture(1);
    let mut archive = world
        .musubi_archives
        .view()
        .get(&key.archive_id)
        .cloned()
        .unwrap();
    archive.registered_at_height = 2;
    world.musubi_archives.insert(key.archive_id, archive);
    assert_borrowed_attestation_error(&world, key, "not a finalized predecessor");
}

#[test]
fn borrowed_attestations_keep_lock_agreement_and_reject_forged_signatures() {
    let (mut world, key) = borrowed_attestation_fixture(2);
    change_borrowed_attestation_record(&mut world, key, 1, |record| {
        record.attestation.payload.binding.verification_lock_digest =
            MusubiVerificationLockDigestV1::new([0xf4; 32]);
        sign_borrowed_attestation_record(record);
    });
    assert_borrowed_attestation_error(
        &world,
        key,
        "attestations disagree on the verification lock",
    );
    let (mut world, key) = borrowed_attestation_fixture(2);
    change_borrowed_attestation_record(&mut world, key, 1, |record| {
        // Keep all unsigned record/set digests coherent while leaving the old
        // signature over a different exact completion statement.
        record.attestation.payload.binding.assignment_revision += 1;
    });
    assert_borrowed_attestation_error(&world, key, "provider bundle signature failed");
}

#[test]
fn borrowed_attestations_preserve_multisig_controller_membership_and_threshold() {
    use iroha_data_model::account::{MultisigMember, MultisigPolicy};
    let (mut world, key) = borrowed_attestation_fixture(1);
    let first = borrowed_attestation_signer();
    let second = KeyPair::try_from_seed(vec![0x79; 32], Algorithm::Ed25519).unwrap();
    let policy = MultisigPolicy::new(
        2,
        vec![
            MultisigMember::new(first.public_key().clone(), 1).unwrap(),
            MultisigMember::new(second.public_key().clone(), 1).unwrap(),
        ],
    )
    .unwrap();
    let owner = AccountId::new_multisig(policy);
    change_borrowed_attestation_record(&mut world, key, 0, |record| {
        record.attestation.payload.binding.completed_by = owner.clone();
        record
            .attestation
            .payload
            .binding
            .completion_authority
            .provider_owner = owner;
        let digest = record.attestation.payload.signing_hash();
        record.attestation.approvals = [&first, &second]
            .into_iter()
            .map(|signer| MusubiProviderBundleVerificationApprovalV1 {
                public_key: signer.public_key().clone(),
                signature: SignatureOf::try_from_hash(signer.private_key(), digest).unwrap(),
            })
            .collect();
        record
            .attestation
            .approvals
            .sort_by(|left, right| left.public_key.cmp(&right.public_key));
    });
    {
        let view = world.view();
        let evidence = load_location_provider_attestations(
            view.musubi_archives().get(&key.archive_id).unwrap(),
            view.musubi_archive_locations().get(&key).unwrap(),
            &view,
        )
        .expect("both provider-owner approvals meet threshold");
        assert_eq!(evidence.len(), 1);
    }
    change_borrowed_attestation_record(&mut world, key, 0, |record| {
        let _ = record.attestation.approvals.pop();
    });
    assert_borrowed_attestation_error(&world, key, "do not meet provider-owner threshold");
    change_borrowed_attestation_record(&mut world, key, 0, |record| {
        let foreign = KeyPair::try_from_seed(vec![0x7a; 32], Algorithm::Ed25519).unwrap();
        record.attestation.approvals[0] = MusubiProviderBundleVerificationApprovalV1 {
            public_key: foreign.public_key().clone(),
            signature: SignatureOf::try_from_hash(
                foreign.private_key(),
                record.attestation.payload.signing_hash(),
            )
            .unwrap(),
        };
    });
    assert_borrowed_attestation_error(&world, key, "not a provider-owner key");
}

#[test]
fn borrowed_attestation_failure_preserves_current_location_unhealthy_semantics() {
    let (mut world, _, key, _) = archive_location_replay_fixture(BORROWED_ATTESTATION_SEED);
    {
        let view = world.view();
        let location = view.musubi_archive_locations().get(&key).unwrap();
        assert_eq!(
            current_location_providers(location, &view).map(|providers| providers.to_vec()),
            Some(location.providers.clone())
        );
    }
    change_borrowed_attestation_record(&mut world, key, 0, |record| {
        record.attestation.payload.binding.assignment_revision += 1;
    });
    let view = world.view();
    let location = view.musubi_archive_locations().get(&key).unwrap();
    assert!(current_location_providers(location, &view).is_none());
}
