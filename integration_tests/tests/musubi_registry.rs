#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Musubi registry flows over a representative multi-peer network.

use std::time::Duration;

use eyre::{Result, WrapErr, eyre};
use integration_tests::sandbox;
use iroha::{
    client::Client,
    data_model::{
        isi::{
            musubi::{PublishMusubiRelease, SetMusubiShortAlias, YankMusubiRelease},
            sorafs::RegisterPinManifest,
        },
        musubi::{
            MusubiArchiveRef, MusubiDependency, MusubiPackageId, MusubiPackageRef, MusubiRelease,
            MusubiReleaseStatus, MusubiShortAlias, MusubiSourceArchivePlan, MusubiSourceChunkPlan,
            MusubiSourceFilePlan,
        },
        name::Name,
        prelude::*,
        query::musubi::prelude::{
            FindMusubiPackageReleases, FindMusubiPackageVersions, FindMusubiReleaseByRef,
            FindMusubiShortAliasByName, SearchMusubiPackages,
        },
        sorafs::pin_registry::{ChunkerProfileHandle, ManifestDigest, PinPolicy},
    },
};
use iroha_executor_data_model::permission::musubi::CanSetMusubiShortAlias;
use iroha_test_network::NetworkBuilder;
use iroha_test_samples::ALICE_ID;
use sorafs_car::{CarBuildPlan, CarWriter, FileEntry, compute_chunk_plan_digest_sha3};
use sorafs_manifest::chunker_registry;

const QUERY_ATTEMPTS: usize = 160;
const QUERY_RETRY_DELAY: Duration = Duration::from_millis(250);

#[tokio::test]
async fn musubi_registry_flows_propagate_on_four_peers() -> Result<()> {
    init_instruction_registry();

    let short_alias_permission: Permission = CanSetMusubiShortAlias.into();
    let builder = NetworkBuilder::new()
        .with_min_peers(4)
        .with_genesis_instruction(Grant::account_permission(
            short_alias_permission,
            ALICE_ID.clone(),
        ));
    let Some(network) = sandbox::start_network_async_or_skip(
        builder,
        stringify!(musubi_registry_flows_propagate_on_four_peers),
    )
    .await?
    else {
        return Ok(());
    };

    network.ensure_blocks(1).await?;
    let client = network.client();
    let query_client = network
        .peers()
        .last()
        .ok_or_else(|| eyre!("network must include at least one peer"))?
        .client();

    let math_ref: MusubiPackageRef = "wonderland.universal/math@1.0.0".parse()?;
    let swap_ref: MusubiPackageRef = "wonderland.universal/swap@1.0.0".parse()?;

    let math_release = build_release(&math_ref, 0x21, b"fn add(a, b) { a + b }", [], ["add"])?;
    register_manifest(&client, &math_release)?;
    client.submit_blocking(PublishMusubiRelease::new(math_release.clone()))?;
    eventually_find_release(&query_client, &math_ref).await?;

    let math_dependency = MusubiDependency::new("math".parse()?, math_ref.clone());
    let swap_release = build_release(
        &swap_ref,
        0x31,
        b"fn quote(x) { math::add(x, x) }",
        [math_dependency],
        ["quote"],
    )?;
    register_manifest(&client, &swap_release)?;
    client.submit_blocking(PublishMusubiRelease::new(swap_release.clone()))?;

    let propagated_swap = eventually_find_release(&query_client, &swap_ref).await?;
    assert_eq!(propagated_swap.package, swap_ref);
    assert_eq!(propagated_swap.dependencies.len(), 1);
    assert_eq!(propagated_swap.dependencies[0].package, math_ref);
    assert!(propagated_swap.status.is_active());

    let versions: Vec<_> = query_client.query_single(FindMusubiPackageVersions {
        package: swap_ref.package.clone(),
    })?;
    assert_eq!(versions, vec![swap_ref.version.clone()]);

    let releases = query_client.query_single(FindMusubiPackageReleases {
        package: swap_ref.package.clone(),
        include_yanked: false,
    })?;
    assert_eq!(releases.len(), 1);
    assert_eq!(releases[0].package, swap_ref);

    let search = query_client.query_single(SearchMusubiPackages {
        namespace: Some("wonderland.universal".parse()?),
        query: "swap".to_owned(),
        include_yanked: false,
        offset: 0,
        limit: 10,
    })?;
    assert_eq!(search.len(), 1);
    assert_eq!(search[0].package, swap_ref.package);
    assert_eq!(search[0].latest_active, Some(swap_ref.version.clone()));

    client.submit_blocking(SetMusubiShortAlias::new(MusubiShortAlias::new(
        "swap".parse()?,
        swap_ref.package.clone(),
    )))?;
    let alias_target = eventually_find_alias(&query_client, "swap".parse()?).await?;
    assert_eq!(alias_target, swap_ref.package);

    let retarget_err = client
        .submit_blocking(SetMusubiShortAlias::new(MusubiShortAlias::new(
            "swap".parse()?,
            math_ref.package.clone(),
        )))
        .expect_err("short alias retargeting must be rejected");
    assert!(
        format!("{retarget_err:?}").contains("already targets"),
        "expected retarget rejection, got {retarget_err:?}"
    );

    client.submit_blocking(YankMusubiRelease::new(
        swap_ref.clone(),
        "superseded by 1.0.1",
    ))?;
    let yanked_swap = eventually_find_yanked_release(&query_client, &swap_ref).await?;
    assert!(matches!(yanked_swap.status, MusubiReleaseStatus::Yanked(_)));

    let active_releases: Vec<_> = query_client.query_single(FindMusubiPackageReleases {
        package: swap_ref.package.clone(),
        include_yanked: false,
    })?;
    assert!(active_releases.is_empty());

    let all_releases: Vec<_> = query_client.query_single(FindMusubiPackageReleases {
        package: swap_ref.package.clone(),
        include_yanked: true,
    })?;
    assert_eq!(all_releases.len(), 1);
    assert!(matches!(
        all_releases[0].status,
        MusubiReleaseStatus::Yanked(_)
    ));

    let active_search: Vec<_> = query_client.query_single(SearchMusubiPackages {
        namespace: Some("wonderland.universal".parse()?),
        query: "swap".to_owned(),
        include_yanked: false,
        offset: 0,
        limit: 10,
    })?;
    assert!(active_search.is_empty());

    Ok(())
}

fn register_manifest(client: &Client, release: &MusubiRelease) -> Result<()> {
    let source_plan = release
        .source_archive_plan
        .as_ref()
        .ok_or_else(|| eyre!("test release must include source plan"))?;
    client.submit_blocking(RegisterPinManifest::new(
        release.archive.sorafs_manifest,
        default_chunker_handle(),
        chunk_plan_digest(source_plan),
        PinPolicy::default(),
        0,
        None,
        None,
    ))?;
    Ok(())
}

fn build_release<const DEPS: usize, const EXPORTS: usize>(
    package: &MusubiPackageRef,
    seed: u8,
    source: &[u8],
    dependencies: [MusubiDependency; DEPS],
    exports: [&str; EXPORTS],
) -> Result<MusubiRelease> {
    let (source_plan, archive_hash, source_bytes, source_file_count) =
        build_source_archive_plan(package, source)?;
    let manifest = ManifestDigest::new([seed; 32]);
    let export_names = exports
        .into_iter()
        .map(str::parse)
        .collect::<Result<Vec<Name>, _>>()?;
    Ok(MusubiRelease::new(
        package.clone(),
        MusubiArchiveRef::new(manifest, archive_hash, source_bytes, source_file_count),
        dependencies.into(),
        export_names,
        None,
        ALICE_ID.clone(),
        0,
    )
    .with_source_archive_plan(source_plan))
}

fn build_source_archive_plan(
    package: &MusubiPackageRef,
    source: &[u8],
) -> Result<(MusubiSourceArchivePlan, [u8; 32], u64, u32)> {
    let descriptor = chunker_registry::default_descriptor();
    let path = format!("{}.ko", package.package.name.as_ref());
    let (plan, payload) = CarBuildPlan::from_files_with_profile(
        vec![FileEntry {
            path: vec![path],
            data: source.to_vec(),
        }],
        descriptor.profile,
    )?;
    let mut car_bytes = Vec::new();
    let stats = CarWriter::new(&plan, &payload)?.write_to(&mut car_bytes)?;
    let archive_hash = *stats.car_archive_digest.as_bytes();
    let files = plan
        .files
        .iter()
        .map(|file| {
            Ok(MusubiSourceFilePlan::new(
                file.path.clone(),
                u32::try_from(file.first_chunk)?,
                u32::try_from(file.chunk_count)?,
                file.size,
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    let chunks = plan
        .chunks
        .iter()
        .map(|chunk| MusubiSourceChunkPlan::new(chunk.offset, chunk.length, chunk.digest))
        .collect();
    let source_plan = MusubiSourceArchivePlan::new(
        *plan.payload_digest.as_bytes(),
        plan.content_length,
        archive_hash,
        stats.car_size,
        chunks,
        files,
    );
    Ok((
        source_plan,
        archive_hash,
        plan.content_length,
        u32::try_from(plan.files.len())?,
    ))
}

fn default_chunker_handle() -> ChunkerProfileHandle {
    let descriptor = chunker_registry::default_descriptor();
    ChunkerProfileHandle {
        profile_id: descriptor.id.0,
        namespace: descriptor.namespace.to_owned(),
        name: descriptor.name.to_owned(),
        semver: descriptor.semver.to_owned(),
        multihash_code: descriptor.multihash_code,
    }
}

fn chunk_plan_digest(plan: &MusubiSourceArchivePlan) -> [u8; 32] {
    let chunks = plan
        .chunks
        .iter()
        .map(|chunk| sorafs_car::CarChunk {
            offset: chunk.offset,
            length: chunk.length,
            digest: chunk.digest_blake3_256,
            taikai_segment_hint: None,
        })
        .collect::<Vec<_>>();
    compute_chunk_plan_digest_sha3(&chunks)
}

async fn eventually_find_release(
    client: &Client,
    package: &MusubiPackageRef,
) -> Result<MusubiRelease> {
    let mut last_error = None;
    for _ in 0..QUERY_ATTEMPTS {
        match client.query_single(FindMusubiReleaseByRef {
            package: package.clone(),
        }) {
            Ok(release) => return Ok(release),
            Err(err) => last_error = Some(format!("{err:?}")),
        }
        tokio::time::sleep(QUERY_RETRY_DELAY).await;
    }
    Err(eyre!(
        "timed out waiting for Musubi release `{package}`; last error: {}",
        last_error.unwrap_or_else(|| "none".to_owned())
    ))
}

async fn eventually_find_yanked_release(
    client: &Client,
    package: &MusubiPackageRef,
) -> Result<MusubiRelease> {
    for _ in 0..QUERY_ATTEMPTS {
        let release = eventually_find_release(client, package).await?;
        if !release.status.is_active() {
            return Ok(release);
        }
        tokio::time::sleep(QUERY_RETRY_DELAY).await;
    }
    Err(eyre!(
        "timed out waiting for Musubi release `{package}` to be yanked"
    ))
}

async fn eventually_find_alias(client: &Client, alias: Name) -> Result<MusubiPackageId> {
    let mut last_error = None;
    for _ in 0..QUERY_ATTEMPTS {
        match client.query_single(FindMusubiShortAliasByName {
            alias: alias.clone(),
        }) {
            Ok(target) => return Ok(target),
            Err(err) => last_error = Some(format!("{err:?}")),
        }
        tokio::time::sleep(QUERY_RETRY_DELAY).await;
    }
    Err(eyre!(
        "timed out waiting for Musubi short alias `{alias}`; last error: {}",
        last_error.unwrap_or_else(|| "none".to_owned())
    ))
}
