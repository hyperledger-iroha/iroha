//! Cache identities bind the profile that prepared an object: no key, lookup or eviction of one
//! profile reaches the object of another, whatever the code hash.

use super::*;
use ivm::{IvmEngine, IvmProfileId};
use ivm_abi::error::VmTrap;

const BASELINE: IvmProfileId = IvmProfileId::BASELINE_ABI_V1;
const NAMED: IvmProfileId = IvmProfileId::C10_RUNTIME_LIFECYCLE;
const GAS: u64 = 10_000;
const HEAP: u64 = ivm::Memory::HEAP_MAX_SIZE;

fn abi(profile: IvmProfileId) -> [u8; 32] {
    profile.abi_hash()
}

fn is_mismatch(error: &ivm::VMError, requested: IvmProfileId, bound: IvmProfileId) -> bool {
    matches!(
        error,
        ivm::VMError::Trap(VmTrap::ArtifactAbiHashMismatch { expected, actual })
            if *expected == abi(requested) && *actual == abi(bound)
    )
}

#[test]
fn keys_of_one_code_hash_differ_by_profile_and_engine_scope() {
    let hash = Hash::new(b"one artifact");
    let baseline = SummaryKey::for_profile(BASELINE, hash);
    let named = SummaryKey::for_profile(NAMED, hash);
    assert_ne!(baseline, named);
    assert_eq!(
        baseline,
        SummaryKey::new(hash),
        "tests address baseline keys"
    );
    assert_eq!(baseline, SummaryKey::for_profile(BASELINE, hash));
    let c10 = SummaryKey::for_profile(IvmProfileId::C10_ABI_V1, hash);
    assert_eq!(
        SummaryKey::every_profile(hash).collect::<Vec<_>>(),
        vec![baseline, named, c10]
    );
    assert_eq!(baseline.profile.engine(), IvmEngine::Baseline);
    assert_eq!(named.profile.engine(), IvmEngine::C10);
    assert_eq!(baseline.same_engine_foreign().count(), 0);
    assert_eq!(named.same_engine_foreign().collect::<Vec<_>>(), [c10]);
    assert_eq!(c10.same_engine_foreign().collect::<Vec<_>>(), [named]);
    let runtime = RuntimeKey::for_profile(baseline.profile, hash, 1, 2);
    let other = RuntimeKey::for_profile(named.profile, hash, 1, 2);
    assert_ne!(runtime, other);
    assert_eq!(runtime, RuntimeKey::new(hash, 1, 2));
    assert_eq!(runtime.summary_key(), baseline);
    assert_eq!(other.summary_key(), named);
    assert_eq!(
        generic_program_profile(),
        BASELINE,
        "contract-less programs exist only under the interface profile"
    );
}

#[test]
fn baseline_engine_keys_are_disjoint_from_every_c10_key() {
    let hash = Hash::new(b"one artifact");
    let baseline_engine = SummaryKey::for_profile(IvmProfileId::BASELINE_ABI_V1, hash);
    assert_eq!(baseline_engine.profile.engine(), IvmEngine::Baseline);
    assert_eq!(
        baseline_engine,
        SummaryKey::new(hash),
        "ABI V1 keys name the baseline engine"
    );
    for profile in [
        IvmProfileId::C10_ABI_V1,
        IvmProfileId::C10_RUNTIME_LIFECYCLE,
    ] {
        let key = SummaryKey::for_profile(profile, hash);
        assert_eq!(key.profile.engine(), IvmEngine::C10);
        assert_ne!(
            key, baseline_engine,
            "no C10 profile key names a baseline entry"
        );
        assert!(
            key.same_engine_foreign()
                .all(|other| other.profile.engine() == IvmEngine::C10),
            "same-engine mismatch reporting never consults another engine's entries"
        );
    }
    assert_eq!(
        baseline_engine.same_engine_foreign().count(),
        0,
        "no other implemented profile binds the baseline engine in this release"
    );
    let runtime = RuntimeKey::for_profile(IvmProfileId::C10_RUNTIME_LIFECYCLE, hash, 1, 2);
    assert_ne!(runtime, RuntimeKey::new(hash, 1, 2));
    assert_eq!(
        runtime.summary_key(),
        SummaryKey::for_profile(IvmProfileId::C10_RUNTIME_LIFECYCLE, hash)
    );
}

#[test]
fn a_prepared_contract_is_resident_and_served_only_under_its_own_profile() {
    let _retention = super::tests::default_retention();
    let bytes = super::tests::minimal_program();
    let hash = ivm::contract_code_hash(&bytes);
    let cache = PreparedContractCache::with_capacity(4);
    let prepared = cache
        .get_or_prepare_for_profile(hash, &bytes, BASELINE)
        .expect("prepare under the interface profile");
    assert_eq!(prepared.profile_id(), BASELINE);
    cache.with_store(|store| {
        assert_eq!(
            store.entries.keys().copied().collect::<Vec<_>>(),
            [SummaryKey::for_profile(BASELINE, hash)]
        );
    });

    // A lookup under another profile finds nothing and is not counted as a hit.
    let before = cache.stats();
    assert!(cache.get_for_profile(hash, NAMED).is_none());
    assert_eq!(cache.stats(), before);

    // The named policy runs the other engine, whose header check refuses the baseline bytes;
    // the warm refusal is the one a cold cache reports from the bytes themselves.
    let warm = cache
        .get_or_prepare_for_profile(hash, &bytes, NAMED)
        .expect_err("the resident object belongs to another profile");
    let cold = PreparedContractCache::with_capacity(4)
        .get_or_prepare_for_profile(hash, &bytes, NAMED)
        .expect_err("the header declares another profile");
    assert_eq!(format!("{warm:?}"), format!("{cold:?}"));
    cache.with_store(|store| {
        assert_eq!(store.entries.len(), 1, "a refusal publishes nothing");
        assert!(store.preparing.is_empty());
    });
    assert_eq!(cache.stats().preparations, 1);

    // The entry still serves exactly its own profile.
    let again = cache
        .get_for_profile(hash, BASELINE)
        .expect("the baseline entry is resident");
    assert_eq!(again.code_hash(), hash);
    assert_eq!(again.profile_id(), prepared.profile_id());
}

#[test]
fn an_entry_filed_under_a_foreign_profile_key_is_never_served() {
    let _retention = super::tests::default_retention();
    let bytes = super::tests::minimal_program();
    let hash = ivm::contract_code_hash(&bytes);
    let cache = PreparedContractCache::with_capacity(4);
    let prepared = cache
        .get_or_prepare_for_profile(hash, &bytes, BASELINE)
        .expect("prepare under the interface profile");
    // Simulate a defect that files the baseline object under the named profile's key.
    let forged = SummaryKey::for_profile(NAMED, hash);
    cache.with_store(|store| {
        store.entries.insert(forged, prepared.clone());
    });
    assert!(
        cache.get_for_profile(hash, NAMED).is_none(),
        "the object's own profile is checked against the key"
    );
    let refusal = cache
        .get_or_prepare_for_profile(hash, &bytes, NAMED)
        .expect_err("a forged residency is not a preparation");
    assert!(is_mismatch(&refusal, NAMED, BASELINE), "{refusal:?}");
    assert!(cache.get_for_profile(hash, BASELINE).is_some());
}

#[test]
fn summaries_analyses_and_runtimes_are_keyed_by_the_summary_profile() {
    let _retention = super::tests::default_retention();
    let bytes = super::tests::minimal_program();
    let hash = ivm::contract_code_hash(&bytes);
    let mut cache = IvmCache::with_capacity(4);
    let summary = cache
        .summarize_program_with_hash_for_profile(hash, &bytes, BASELINE)
        .expect("summarize under the interface profile");
    assert_eq!(summary.profile().id(), BASELINE);
    assert_eq!(
        summary.profile().id(),
        summary.prepared_contract().profile_id()
    );
    let baseline = SummaryKey::for_profile(BASELINE, hash);
    let named = SummaryKey::for_profile(NAMED, hash);
    cache.with_local(|local| {
        assert!(local.summaries.contains_key(&baseline));
        assert!(!local.summaries.contains_key(&named));
    });

    // The exact profile hits; the named policy, which runs the other engine, finds nothing,
    // and a summary filed under a foreign key is refused by its own profile.
    assert!(
        cache
            .cached_program_summary_for_profile(hash, BASELINE)
            .expect("own profile")
            .is_some()
    );
    assert!(
        cache
            .cached_program_summary_for_profile(hash, NAMED)
            .expect("another engine's summary is not consulted")
            .is_none()
    );
    cache.with_local(|local| {
        local.summaries.insert(named, summary.clone());
    });
    let refusal = cache
        .cached_program_summary_for_profile(hash, NAMED)
        .expect_err("a forged summary is not served");
    assert!(is_mismatch(&refusal, NAMED, BASELINE), "{refusal:?}");
    cache.with_local(|local| {
        local.summaries.remove(&named);
    });

    cache
        .analyze_program(&summary, &bytes)
        .expect("analyze the prepared contract");
    cache.with_local(|local| {
        assert_eq!(
            local.analyses.keys().copied().collect::<Vec<_>>(),
            [baseline]
        );
    });

    let stack = stack_limit_for_gas(GAS);
    drop(
        cache
            .checkout_runtime(&summary, &bytes, GAS, HEAP)
            .expect("check out a runtime"),
    );
    cache.with_local(|local| {
        let keys: Vec<_> = local.runtime_templates.keys().copied().collect();
        assert_eq!(
            keys,
            [RuntimeKey::for_profile(baseline.profile, hash, stack, HEAP)]
        );
        assert!(
            !local
                .runtime_templates
                .contains_key(&RuntimeKey::for_profile(named.profile, hash, stack, HEAP))
        );
    });

    drop(
        summary
            .checkout_runtime(GAS, HEAP)
            .expect("check out a nested runtime"),
    );
    cache.prepared_contract_cache().with_store(|store| {
        assert!(
            store
                .nested_runtimes
                .keys()
                .all(|key| key.profile == baseline.profile && key.summary_key() == baseline)
        );
    });
}

#[test]
fn generic_programs_are_summarized_and_run_under_the_interface_profile() {
    let _retention = super::tests::default_retention();
    let program = super::tests::minimal_generic_program();
    let mut cache = IvmCache::with_capacity(4);
    let summary = cache
        .summarize_generic_program(&program)
        .expect("valid generic program");
    let interface = super::super::profile::ResolvedIvmProfile::interface_default();
    assert_eq!(summary.profile(), interface);
    assert_eq!(summary.abi_hash, Hash::prehashed(interface.abi()));
    assert_eq!(
        summary.meta_hash,
        Hash::new(summary.metadata.encode_for_profile(interface.id(), 0)),
        "the metadata hash is that of the interface profile's header"
    );
    let key = SummaryKey::for_profile(interface.id(), summary.code_hash);
    cache.with_local(|local| {
        assert_eq!(
            local.generic_summaries.keys().copied().collect::<Vec<_>>(),
            [key]
        );
    });
    assert!(
        cache
            .cached_generic_program_summary(summary.code_hash)
            .is_some()
    );
    let lease = cache
        .checkout_generic_runtime(&summary, GAS, HEAP)
        .expect("check out a generic runtime");
    assert_eq!(lease.profile_id(), interface.id());
    assert_eq!(lease.engine(), IvmEngine::Baseline);
    drop(lease);
    // A named profile requires a self-describing contract: a generic program is refused.
    assert!(
        cache
            .summarize_executable_for_profile(&program, NAMED)
            .is_err()
    );
}
