// Exact current-provider evidence with fixed local storage and borrowed completions.

fn current_provider_fixture(count: usize) -> (World, MusubiArchiveLocationKeyV1) {
    let (mut world, key) = borrowed_attestation_fixture(count);
    let location = world
        .musubi_archive_locations
        .view()
        .get(&key)
        .cloned()
        .unwrap();
    let mut order = world
        .replication_orders
        .view()
        .get(&location.replication_order)
        .cloned()
        .unwrap();
    order.provider_completions.clear();
    for provider in &location.providers {
        let record = world
            .musubi_provider_bundle_attestations
            .view()
            .get(&MusubiProviderBundleAttestationKeyV1 {
                archive_id: key.archive_id,
                replication_order: location.replication_order,
                provider_id: *provider,
            })
            .cloned()
            .unwrap();
        let binding = &record.attestation.payload.binding;
        world
            .provider_owners
            .insert(*provider, binding.completed_by.clone());
        world
            .musubi_locations_by_provider
            .insert(MusubiProviderLocationKeyV1::new(*provider, key), ());
        order.provider_completions.push(
            iroha_data_model::sorafs::pin_registry::ReplicationOrderCompletionRecord {
                provider_id: *provider,
                completed_by: binding.completed_by.clone(),
                completion_epoch: binding.completion_epoch,
                assignment_revision: binding.assignment_revision,
                completion_authority: binding.completion_authority.clone(),
                finalized_anchor: binding.finalized_anchor,
            },
        );
    }
    // The existing predicate accepts unordered completion records, while
    // location providers and returned identities remain canonical.
    order.provider_completions.reverse();
    world
        .replication_orders
        .insert(location.replication_order, order);
    (world, key)
}

#[test]
fn current_providers_keep_exact_order_without_result_or_completion_heap_storage() {
    for count in [1, 2, MUSUBI_MAX_LOCATION_PROVIDERS_V1] {
        let (world, key) = current_provider_fixture(count);
        let view = world.view();
        let location = view.musubi_archive_locations().get(&key).unwrap();
        let providers =
            observe_musubi_helper_without_heap(|| current_location_providers(location, &view))
                .expect("current exact evidence");
        assert_eq!(&*providers, location.providers.as_slice());
        assert_eq!(
            providers.into_iter().collect::<Vec<_>>(),
            location.providers
        );
        assert!(
            std::mem::size_of_val(&providers)
                <= MUSUBI_MAX_LOCATION_PROVIDERS_V1
                    * std::mem::size_of::<iroha_data_model::sorafs::capacity::ProviderId>()
                    + std::mem::size_of::<usize>()
        );
    }
}

#[test]
fn current_providers_reject_missing_duplicate_and_oversized_completion_sets() {
    for case in 0..3 {
        let (mut world, key) = current_provider_fixture(2);
        let location = world
            .musubi_archive_locations
            .view()
            .get(&key)
            .cloned()
            .unwrap();
        let mut order = world
            .replication_orders
            .view()
            .get(&location.replication_order)
            .cloned()
            .unwrap();
        match case {
            0 => {
                let _ = order.provider_completions.pop();
            }
            1 => {
                order.provider_completions[1] = order.provider_completions[0].clone();
            }
            _ => {
                order.provider_completions = vec![
                    order.provider_completions[0].clone();
                    MUSUBI_MAX_LOCATION_PROVIDERS_V1 + 1
                ];
            }
        }
        world
            .replication_orders
            .insert(location.replication_order, order);
        let view = world.view();
        assert!(
            observe_musubi_helper_without_heap(|| current_location_providers(&location, &view))
                .is_none()
        );
    }
}

#[test]
fn current_providers_preserve_empty_evidence_for_current_authority_and_completion_conflicts() {
    for case in 0..6 {
        let (mut world, key) = current_provider_fixture(1);
        let location = world
            .musubi_archive_locations
            .view()
            .get(&key)
            .cloned()
            .unwrap();
        let mut order = world
            .replication_orders
            .view()
            .get(&location.replication_order)
            .cloned()
            .unwrap();
        let completion = &mut order.provider_completions[0];
        match case {
            0 => {
                world
                    .provider_owners
                    .insert(location.providers[0], account(0x79));
            }
            1 => completion.completed_by = account(0x79),
            2 => completion.completion_authority.provider_owner = account(0x79),
            3 => completion.assignment_revision += 1,
            4 => completion.completion_epoch += 1,
            _ => completion.finalized_anchor.height += 1,
        }
        world
            .replication_orders
            .insert(location.replication_order, order);
        let providers = current_location_providers(&location, &world.view())
            .expect("valid location with no current providers");
        assert!(providers.is_empty());
        assert_eq!(
            providers.into_iter().count(),
            0,
            "uninitialized slots must not escape"
        );
    }
}

#[test]
fn current_providers_keep_missing_reverse_and_retired_location_semantics() {
    let (world, key) = current_provider_fixture(1);
    let mut location = world
        .musubi_archive_locations
        .view()
        .get(&key)
        .cloned()
        .unwrap();
    {
        let mut reverse = world.musubi_locations_by_provider.block();
        let _ = reverse.remove(MusubiProviderLocationKeyV1::new(location.providers[0], key));
        reverse.commit();
    }
    assert!(
        current_location_providers(&location, &world.view())
            .unwrap()
            .is_empty()
    );
    location.state = MusubiArchiveLocationStateV1::Retired;
    let view = world.view();
    assert!(
        observe_musubi_helper_without_heap(|| current_location_providers(&location, &view))
            .is_none()
    );
}

#[test]
fn current_provider_complete_helper_has_no_cold_thread_heap_storage() {
    let (world, key) = current_provider_fixture(2);
    std::thread::spawn(move || {
        let view = world.view();
        let location = view.musubi_archive_locations().get(&key).unwrap();
        let providers =
            observe_musubi_helper_without_heap(|| current_location_providers(location, &view))
                .unwrap();
        assert_eq!(&*providers, location.providers.as_slice());
    })
    .join()
    .unwrap();
}
