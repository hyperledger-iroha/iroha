//! Explicit physical catalogs are exact; only an absent catalog selects the universal default.

use super::*;

fn private_nexus() -> Nexus {
    Nexus {
        dataspace_catalog: vec![DataSpaceDescriptor {
            alias: Some("private".into()),
            manifest_hash: Some("ff".repeat(32)),
            ..DataSpaceDescriptor::default()
        }],
        lane_catalog: vec![LaneDescriptor {
            index: Some(0),
            alias: Some("private".into()),
            dataspace: Some("private".into()),
            visibility: Some("restricted".into()),
            ..LaneDescriptor::default()
        }],
        routing_policy: RoutingPolicy {
            default_lane: Some(0),
            default_dataspace: Some("private".into()),
            ..RoutingPolicy::default()
        },
        ..Nexus::default()
    }
}

#[test]
fn omitted_and_empty_catalog_keep_the_universal_default() {
    for nexus in [
        Nexus::default(),
        Nexus {
            dataspace_catalog: vec![],
            ..Nexus::default()
        },
    ] {
        let mut emitter = Emitter::new();
        let actual = nexus.parse(&mut emitter).expect("default physical catalog");
        assert!(emitter.into_result().is_ok());
        assert_eq!(actual.dataspace_catalog, DataSpaceCatalog::default());
        assert_eq!(
            actual.configured_dataspace_catalog,
            actual.dataspace_catalog
        );
        assert_eq!(
            actual.lane_catalog.lanes()[0].dataspace_id,
            DataSpaceId::UNIVERSAL
        );
    }
}

#[test]
fn explicit_nonuniversal_catalog_retains_exact_full_width_identity() {
    let mut emitter = Emitter::new();
    let actual = private_nexus()
        .parse(&mut emitter)
        .expect("exact private catalog");
    assert!(emitter.into_result().is_ok());
    assert_eq!(actual.dataspace_catalog.entries().len(), 1);
    assert_eq!(
        actual.dataspace_catalog.entries()[0].id,
        DataSpaceId::new(u64::MAX)
    );
    assert_eq!(
        actual.configured_dataspace_catalog,
        actual.dataspace_catalog
    );
    assert_eq!(actual.lane_catalog.lanes().len(), 1);
    assert_eq!(actual.lane_catalog.lanes()[0].id, LaneId::SINGLE);
    assert_eq!(
        actual.lane_catalog.lanes()[0].dataspace_id,
        DataSpaceId::new(u64::MAX)
    );
    assert_eq!(
        actual.routing_policy.default_dataspace,
        DataSpaceId::new(u64::MAX)
    );
}

#[test]
fn explicit_catalog_must_include_universal_when_a_lane_requires_it() {
    let mut nexus = private_nexus();
    nexus.lane_catalog.clear();
    nexus.routing_policy = RoutingPolicy::default();
    let mut emitter = Emitter::new();
    assert!(nexus.clone().parse(&mut emitter).is_none());
    assert!(
        format!("{:?}", emitter.into_result().unwrap_err())
            .contains("missing from the explicit dataspace_catalog")
    );
    nexus.dataspace_catalog.push(DataSpaceDescriptor {
        alias: Some("universal".into()),
        id: Some(0),
        ..DataSpaceDescriptor::default()
    });
    let mut emitter = Emitter::new();
    let actual = nexus
        .parse(&mut emitter)
        .expect("explicit global and private catalogs");
    assert!(emitter.into_result().is_ok());
    assert_eq!(actual.dataspace_catalog.entries().len(), 2);
    assert!(
        actual
            .dataspace_catalog
            .by_id(DataSpaceId::UNIVERSAL)
            .is_some()
    );
}
